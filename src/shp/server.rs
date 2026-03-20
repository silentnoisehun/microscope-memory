//! SHP TCP Server — async handler dispatching to microscope-memory engine.

use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{MicroscopeReader, TieredIndex, id_to_layer};
use crate::genome;
use crate::teacher::TeachingContext;
use super::protocol::*;

pub struct ShpServer {
    reader: Arc<MicroscopeReader>,
    tiered: Arc<TieredIndex>,
    port: u16,
    write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl ShpServer {
    pub fn new(port: u16) -> Self {
        let reader = Arc::new(MicroscopeReader::open());
        let tiered = Arc::new(TieredIndex::build(&reader));
        ShpServer {
            reader,
            tiered,
            port,
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", self.port)).await?;
        let gh = genome::genome_hash();
        println!("SHP server listening on port {}", self.port);
        println!("  Genome hash: {}", crate::hex_short(&gh.hash));
        println!("  Blocks: {}", self.reader.block_count);

        loop {
            let (stream, addr) = listener.accept().await?;
            let reader = Arc::clone(&self.reader);
            let tiered = Arc::clone(&self.tiered);
            let write_lock = Arc::clone(&self.write_lock);

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, reader, tiered, write_lock).await {
                    eprintln!("  [{}] error: {}", addr, e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    reader: Arc<MicroscopeReader>,
    tiered: Arc<TieredIndex>,
    write_lock: Arc<tokio::sync::Mutex<()>>,
) -> std::io::Result<()> {
    let our_hash = genome::genome_hash().hash;

    loop {
        // Read 41-byte request header
        let mut hdr_buf = [0u8; SHP_HEADER_SIZE];
        match stream.read_exact(&mut hdr_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        }

        let req = match RequestHeader::from_bytes(&hdr_buf) {
            Some(r) => r,
            None => {
                send_response(&mut stream, Status::InvalidCommand, &our_hash, b"bad magic").await?;
                continue;
            }
        };

        // Genome authentication
        if req.genome_hash != our_hash {
            send_response(&mut stream, Status::GenomeMismatch, &our_hash, b"genome hash mismatch").await?;
            continue;
        }

        // Payload size check
        if req.payload_len > SHP_MAX_PAYLOAD {
            send_response(&mut stream, Status::PayloadTooLarge, &our_hash, b"too large").await?;
            continue;
        }

        // Read payload
        let mut payload = vec![0u8; req.payload_len as usize];
        if req.payload_len > 0 {
            stream.read_exact(&mut payload).await?;
        }

        // Dispatch
        match req.cmd {
            Command::Ping => {
                send_response(&mut stream, Status::Ok, &our_hash, b"PONG").await?;
            }
            Command::Store => {
                handle_store(&mut stream, &payload, &our_hash, &write_lock, &reader, &tiered).await?;
            }
            Command::Recall => {
                handle_recall(&mut stream, &payload, &our_hash, &reader, &tiered).await?;
            }
            Command::Look => {
                handle_look(&mut stream, &payload, &our_hash, &reader, &tiered).await?;
            }
            Command::Find => {
                handle_find(&mut stream, &payload, &our_hash, &reader).await?;
            }
            Command::Verify => {
                handle_verify(&mut stream, &payload, &our_hash).await?;
            }
            Command::Stats => {
                handle_stats(&mut stream, &our_hash, &reader).await?;
            }
            Command::Teach => {
                handle_teach(&mut stream, &payload, &our_hash, &reader, &tiered).await?;
            }
        }
    }
}

async fn send_response(
    stream: &mut TcpStream,
    status: Status,
    genome_hash: &[u8; 32],
    payload: &[u8],
) -> std::io::Result<()> {
    let hdr = ResponseHeader {
        status,
        payload_len: payload.len() as u32,
        genome_hash: *genome_hash,
    };
    stream.write_all(&hdr.to_bytes()).await?;
    if !payload.is_empty() {
        stream.write_all(payload).await?;
    }
    Ok(())
}

async fn handle_store(
    stream: &mut TcpStream,
    payload: &[u8],
    gh: &[u8; 32],
    write_lock: &tokio::sync::Mutex<()>,
    reader: &MicroscopeReader,
    tiered: &TieredIndex,
) -> std::io::Result<()> {
    let (layer_id, importance, text) = match decode_store(payload) {
        Some(v) => v,
        None => return send_response(stream, Status::Error, gh, b"bad store payload").await,
    };

    // Guarded Store: validate through Silent Worker before writing
    let ctx = TeachingContext::new(reader, tiered);
    let verdict = ctx.verify_response("store", &text);
    match &verdict {
        crate::teacher::TeachVerdict::Denied { reason, .. } => {
            let msg = format!("store rejected: {}", reason);
            return send_response(stream, Status::TeachDenied, gh, msg.as_bytes()).await;
        }
        crate::teacher::TeachVerdict::Approved { .. } => {}
    }

    let layer_name = id_to_layer(layer_id).to_string();

    let _guard = write_lock.lock().await;
    let result = tokio::task::spawn_blocking(move || {
        crate::store_memory(&text, &layer_name, importance);
    }).await;

    match result {
        Ok(_) => send_response(stream, Status::Ok, gh, b"stored").await,
        Err(e) => send_response(stream, Status::Error, gh, e.to_string().as_bytes()).await,
    }
}

async fn handle_recall(
    stream: &mut TcpStream,
    payload: &[u8],
    gh: &[u8; 32],
    reader: &MicroscopeReader,
    tiered: &TieredIndex,
) -> std::io::Result<()> {
    let (query, k) = match decode_query(payload) {
        Some(v) => v,
        None => return send_response(stream, Status::Error, gh, b"bad recall payload").await,
    };

    let (center_zoom, radius) = crate::auto_zoom(&query);
    let (qx, qy, qz) = crate::content_coords(&query, "query");
    let zoom_lo = center_zoom.saturating_sub(radius);
    let zoom_hi = (center_zoom + radius).min(8);

    let mut all: Vec<(f32, usize)> = Vec::new();
    for zoom in zoom_lo..=zoom_hi {
        all.extend(tiered.look(reader, qx, qy, qz, zoom, k as usize));
    }
    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    all.truncate(k as usize);

    let entries: Vec<ResultEntry> = all.iter().map(|(dist, idx)| {
        let h = reader.header(*idx);
        ResultEntry {
            distance: *dist,
            depth: h.depth,
            layer_id: h.layer_id,
            text: reader.text(*idx).to_string(),
        }
    }).collect();

    let result_payload = encode_results(&entries);
    send_response(stream, Status::Ok, gh, &result_payload).await
}

async fn handle_look(
    stream: &mut TcpStream,
    payload: &[u8],
    gh: &[u8; 32],
    reader: &MicroscopeReader,
    tiered: &TieredIndex,
) -> std::io::Result<()> {
    let (x, y, z, zoom, k) = match decode_look(payload) {
        Some(v) => v,
        None => return send_response(stream, Status::Error, gh, b"bad look payload").await,
    };

    let results = tiered.look(reader, x, y, z, zoom, k as usize);
    let entries: Vec<ResultEntry> = results.iter().map(|(dist, idx)| {
        let h = reader.header(*idx);
        ResultEntry {
            distance: *dist,
            depth: h.depth,
            layer_id: h.layer_id,
            text: reader.text(*idx).to_string(),
        }
    }).collect();

    let result_payload = encode_results(&entries);
    send_response(stream, Status::Ok, gh, &result_payload).await
}

async fn handle_find(
    stream: &mut TcpStream,
    payload: &[u8],
    gh: &[u8; 32],
    reader: &MicroscopeReader,
) -> std::io::Result<()> {
    let (query, k) = match decode_query(payload) {
        Some(v) => v,
        None => return send_response(stream, Status::Error, gh, b"bad find payload").await,
    };

    let results = reader.find_text(&query, k as usize);
    let entries: Vec<ResultEntry> = results.iter().map(|(depth, idx)| {
        let h = reader.header(*idx);
        ResultEntry {
            distance: 0.0,
            depth: *depth,
            layer_id: h.layer_id,
            text: reader.text(*idx).to_string(),
        }
    }).collect();

    let result_payload = encode_results(&entries);
    send_response(stream, Status::Ok, gh, &result_payload).await
}

async fn handle_verify(
    stream: &mut TcpStream,
    payload: &[u8],
    gh: &[u8; 32],
) -> std::io::Result<()> {
    let target = if payload.is_empty() { 0u8 } else { payload[0] };

    let result = tokio::task::spawn_blocking(move || {
        match target {
            0 => {
                let cr = crate::verify_chain_result();
                let mr = crate::verify_merkle_result();
                let valid = cr.valid && mr.valid;
                let msg = format!("chain:{} merkle:{} links:{} nodes:{}",
                    cr.valid, mr.valid, cr.link_count, mr.node_count);
                (valid, msg)
            }
            1 => {
                let cr = crate::verify_chain_result();
                let msg = format!("valid:{} links:{}", cr.valid, cr.link_count);
                (cr.valid, msg)
            }
            2 => {
                let mr = crate::verify_merkle_result();
                let msg = format!("valid:{} nodes:{}", mr.valid, mr.node_count);
                (mr.valid, msg)
            }
            _ => (false, "unknown target".into()),
        }
    }).await.unwrap_or((false, "task failed".into()));

    let status = if result.0 { Status::Ok } else { Status::Error };
    send_response(stream, status, gh, result.1.as_bytes()).await
}

async fn handle_stats(
    stream: &mut TcpStream,
    gh: &[u8; 32],
    reader: &MicroscopeReader,
) -> std::io::Result<()> {
    let sr = crate::stats_result(reader);
    let mut buf = Vec::with_capacity(80);
    buf.extend_from_slice(&(sr.block_count as u32).to_le_bytes());
    for (start, count) in &sr.depth_ranges {
        buf.extend_from_slice(&start.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
    }
    send_response(stream, Status::Ok, gh, &buf).await
}

async fn handle_teach(
    stream: &mut TcpStream,
    payload: &[u8],
    gh: &[u8; 32],
    reader: &MicroscopeReader,
    tiered: &TieredIndex,
) -> std::io::Result<()> {
    let (query, response) = match decode_teach(payload) {
        Some(v) => v,
        None => return send_response(stream, Status::Error, gh, b"bad teach payload").await,
    };

    let ctx = TeachingContext::new(reader, tiered);
    let verdict = ctx.verify_response(&query, &response);

    match verdict {
        crate::teacher::TeachVerdict::Approved { confidence, supporting_blocks } => {
            // Include Merkle proof: verify the chain + supporting block indices
            let mr = crate::verify_merkle_result();
            let cr = crate::verify_chain_result();
            let block_indices: Vec<String> = supporting_blocks.iter()
                .take(5)
                .map(|sb| format!("D{}:{}", sb.depth, sb.block_idx))
                .collect();
            let msg = format!(
                "confidence:{:.1}% merkle:{} chain:{} root:{} blocks:[{}]",
                confidence * 100.0,
                mr.valid,
                cr.valid,
                crate::hex_short(&mr.root_hash),
                block_indices.join(","),
            );
            send_response(stream, Status::TeachApproved, gh, msg.as_bytes()).await
        }
        crate::teacher::TeachVerdict::Denied { reason, .. } => {
            send_response(stream, Status::TeachDenied, gh, reason.as_bytes()).await
        }
    }
}
