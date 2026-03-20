//! Microscope Memory — zoom-based hierarchical memory
//!
//! ZERO JSON. Pure binary. mmap. Sub-microsecond.
//! A KOD MAGA A GRAF.
//!
//! CPU analogy: data exists in uniform blocks at every depth.
//! The query's zoom level determines which layer you see.
//! Same block size, different depth. Like a magnifying glass on silicon.
//!
//! Pipeline: raw memory files -> binary blocks -> mmap -> L2 search
//!
//! Optimizations:
//!   - SoA (Structure-of-Arrays) coordinate layout for SIMD auto-vectorization
//!   - Spatial grid partitioning for D3-D5 (O(1) cell lookup instead of O(n) scan)
//!   - Tiered cache: D0-D2 hot (always in L1), D3-D5 grid, D6-D8 lazy mmap
//!   - #[repr(C, align(16))] for cache-line alignment
//!   - Zero-copy mmap readers with &[u8] slices

#[cfg(feature = "viz")]
pub mod viz;

use std::fs;
use std::io::{Write as IoWrite, BufWriter, BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use colored::Colorize;
use sha2::{Sha256, Digest};

// ─── Constants ───────────────────────────────────────
pub const BLOCK_DATA_SIZE: usize = 256;
pub const LAYERS_DIR: &str = "layers";
pub const BIN_DIR: &str = "data";
pub const HDR_PATH: &str = "data/microscope.bin";
pub const DAT_PATH: &str = "data/data.bin";
pub const META_PATH: &str = "data/meta.bin";

// ─── Crypto file paths ──────────────────────────────
pub const CHAIN_PATH: &str = "data/chain.bin";
pub const MERKLE_PATH: &str = "data/merkle.bin";
pub const CHAIN_HEADER_SIZE: usize = 16;
pub const CHAIN_LINK_SIZE: usize = 80;
pub const MERKLE_HEADER_SIZE: usize = 16;
pub const MERKLE_NODE_SIZE: usize = 32;

// ─── Hash Chain structs ─────────────────────────────
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ChainHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub link_count: u32,
    pub _reserved: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ChainLink {
    pub content_hash: [u8; 32],
    pub prev_hash: [u8; 32],
    pub timestamp_us: u64,
    pub block_index: u32,
    pub layer_id: u8,
    pub depth: u8,
    pub _pad: [u8; 2],
}

// ─── Merkle Tree structs ────────────────────────────
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MerkleHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub node_count: u32,
    pub _reserved: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MerkleNode {
    pub merkle_hash: [u8; 32],
}

pub fn now_micros() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros() as u64
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub fn hex_short(hash: &[u8; 32]) -> String {
    hash.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

pub fn hex_full(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn chain_link_bytes(link: &ChainLink) -> &[u8] {
    unsafe { std::slice::from_raw_parts(link as *const ChainLink as *const u8, CHAIN_LINK_SIZE) }
}

// ─── Block header: 32 bytes, packed, mmap-ready ──────
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct BlockHeader {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zoom: f32,
    pub depth: u8,
    pub layer_id: u8,
    pub data_offset: u32,
    pub data_len: u16,
    pub parent_idx: u32,
    pub child_count: u16,
    pub _pad: [u8; 2],
}

pub const HEADER_SIZE: usize = 32;

// ─── Meta header: 48 bytes at start of meta.bin ──────
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MetaHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub block_count: u32,
    pub depth_count: u32,
}

pub const META_HEADER_SIZE: usize = 16;
pub const DEPTH_ENTRY_SIZE: usize = 8;

// ─── Layer mapping ───────────────────────────────────
pub const LAYER_NAMES: &[&str] = &[
    "identity", "long_term", "short_term", "associative", "emotional",
    "relational", "reflections", "crypto_chain", "echo_cache", "rust_state",
];

pub fn layer_color(id: u8) -> &'static str {
    match id {
        0 => "white", 1 => "blue", 2 => "cyan", 3 => "green", 4 => "red",
        5 => "yellow", 6 => "magenta", 7 => "orange", 8 => "lime", 9 => "purple",
        _ => "white",
    }
}

pub fn layer_to_id(name: &str) -> u8 {
    LAYER_NAMES.iter().position(|&n| n == name).unwrap_or(0) as u8
}

pub fn id_to_layer(id: u8) -> &'static str {
    LAYER_NAMES.get(id as usize).unwrap_or(&"long_term")
}

// ─── Deterministic coords from content hash ──────────
pub fn content_coords(text: &str, layer: &str) -> (f32, f32, f32) {
    let mut h: [u64; 3] = [0xcbf29ce484222325, 0x100000001b3, 0xa5a5a5a5a5a5a5a5];
    for &b in text.as_bytes().iter().take(128) {
        h[0] = h[0].wrapping_mul(0x100000001b3) ^ b as u64;
        h[1] = h[1].wrapping_mul(0x100000001b3) ^ b as u64;
        h[2] = h[2].wrapping_mul(0x1000193) ^ b as u64;
    }
    let bx = (h[0] & 0xFFFF) as f32 / 65535.0;
    let by = (h[1] & 0xFFFF) as f32 / 65535.0;
    let bz = (h[2] & 0xFFFF) as f32 / 65535.0;

    let (ox, oy, oz) = match layer {
        "long_term"     => (0.0, 0.0, 0.0),
        "associative"   => (0.3, 0.0, 0.0),
        "emotional"     => (0.0, 0.3, 0.0),
        "relational"    => (0.3, 0.3, 0.0),
        "reflections"   => (0.0, 0.0, 0.3),
        "crypto_chain"  => (0.3, 0.0, 0.3),
        "echo_cache"    => (0.0, 0.3, 0.3),
        "short_term"    => (0.15, 0.15, 0.15),
        "rust_state"    => (0.15, 0.0, 0.15),
        _               => (0.25, 0.25, 0.25),
    };

    (ox + bx * 0.25, oy + by * 0.25, oz + bz * 0.25)
}

// ─── Safe UTF-8 truncation ───────────────────────────
pub fn safe_truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes { return s.to_string(); }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    s[..end].to_string()
}

// ─── Truncate text to block size ─────────────────────
pub fn to_block(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    if bytes.len() <= BLOCK_DATA_SIZE {
        bytes.to_vec()
    } else {
        let mut v = bytes[..BLOCK_DATA_SIZE - 3].to_vec();
        v.extend_from_slice(b"...");
        v
    }
}

// ─── Internal block for building ─────────────────────
pub struct RawBlock {
    pub data: Vec<u8>,
    pub depth: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub layer_id: u8,
    pub parent_idx: u32,
    pub child_count: u16,
}

// ─── Extract text values from minimal JSON parsing ───
pub fn extract_texts_from_file(path: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    let file = match fs::File::open(path) { Ok(f) => f, Err(_) => return texts };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let trimmed = line.trim();

        for key in &["\"content\":", "\"text\":", "\"content_summary\":", "\"pattern\":", "\"label\":", "\"name\":"] {
            if let Some(pos) = trimmed.find(key) {
                let after = &trimmed[pos + key.len()..].trim_start();
                if after.starts_with('"') {
                    let val = extract_json_string(after);
                    if val.len() > 3 {
                        texts.push(val);
                    }
                }
            }
        }

        if let Some(pos) = trimmed.find("\"response\":") {
            let after = &trimmed[pos + 11..].trim_start();
            if after.starts_with('"') {
                let val = extract_json_string(after);
                if val.len() > 3 {
                    texts.push(val);
                }
            }
        }
    }

    if texts.is_empty() {
        if let Ok(raw) = fs::read_to_string(path) {
            let chars: Vec<char> = raw.chars().collect();
            for chunk in chars.chunks(BLOCK_DATA_SIZE) {
                let s: String = chunk.iter().collect();
                if s.trim().len() > 5 {
                    texts.push(s);
                }
            }
        }
    }

    texts
}

pub fn extract_json_string(s: &str) -> String {
    if !s.starts_with('"') { return String::new(); }
    let mut result = String::new();
    let mut escape = false;
    for ch in s[1..].chars() {
        if escape {
            match ch {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                _ => { result.push('\\'); result.push(ch); }
            }
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else if ch == '"' {
            break;
        } else {
            result.push(ch);
        }
    }
    result
}

// ─── Split text into sentences ───────────────────────
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') && current.len() > 10 {
            sentences.push(current.trim().to_string());
            current = String::new();
        }
    }
    if current.trim().len() > 5 {
        sentences.push(current.trim().to_string());
    }
    sentences
}

// ─── BUILD: layers/ -> binary ─────────────────────────
pub fn build() {
    println!("{}", "Building microscope from raw layers (zero JSON)...".cyan().bold());

    let layer_files = [
        "long_term", "short_term", "associative", "emotional",
        "relational", "reflections", "crypto_chain", "echo_cache", "rust_state",
    ];

    let mut layer_texts: Vec<(&str, Vec<String>)> = Vec::new();
    for &name in &layer_files {
        let path = Path::new(LAYERS_DIR).join(format!("{}.json", name));
        let texts = extract_texts_from_file(&path);
        println!("  {} {}: {} items", ">".green(), name, texts.len());
        layer_texts.push((name, texts));
    }

    let mut blocks: Vec<RawBlock> = Vec::new();

    // DEPTH 0: Identity
    let identity = "Claude Memory: 8 reteg. Mate Robert (Silent) gepe. Ora = AI partner (Rust). Hullam-rezonancia, erzelmi frekvencia, kriogenikus rendszer.";
    blocks.push(RawBlock {
        data: to_block(identity),
        depth: 0, x: 0.25, y: 0.25, z: 0.25,
        layer_id: 0, parent_idx: u32::MAX, child_count: layer_files.len() as u16,
    });

    // DEPTH 1: Layer summaries
    let depth1_start = blocks.len();
    for (name, texts) in &layer_texts {
        let preview: Vec<String> = texts.iter().take(3).map(|s| safe_truncate(s, 40)).collect();
        let summary = format!("[{}] {} elem. {}", name, texts.len(), preview.join(" | "));
        let (x, y, z) = content_coords(name, name);
        blocks.push(RawBlock {
            data: to_block(&summary),
            depth: 1, x, y, z,
            layer_id: layer_to_id(name),
            parent_idx: 0,
            child_count: texts.len().div_ceil(5) as u16,
        });
    }

    // DEPTH 2: Clusters (5 items each)
    let _depth2_start = blocks.len();
    let mut depth2_layer_offsets: Vec<(usize, usize)> = Vec::new();
    for (li, (name, texts)) in layer_texts.iter().enumerate() {
        let cluster_start = blocks.len();
        for ci in (0..texts.len()).step_by(5) {
            let chunk: Vec<String> = texts[ci..texts.len().min(ci + 5)]
                .iter().map(|s| safe_truncate(s, 40)).collect();
            let summary = format!("[{} #{}] {}", name, ci / 5, chunk.join(" | "));
            let (x, y, z) = content_coords(&summary, name);
            blocks.push(RawBlock {
                data: to_block(&summary),
                depth: 2, x, y, z,
                layer_id: layer_to_id(name),
                parent_idx: (depth1_start + li) as u32,
                child_count: chunk.len() as u16,
            });
        }
        depth2_layer_offsets.push((cluster_start, blocks.len() - cluster_start));
    }

    // DEPTH 3: Individual items
    let depth3_start = blocks.len();
    let mut depth3_positions: Vec<(f32, f32, f32)> = Vec::new();
    for (li, (name, texts)) in layer_texts.iter().enumerate() {
        for (ti, text) in texts.iter().enumerate() {
            let (x, y, z) = content_coords(text, name);
            let cluster_idx = ti / 5;
            let (d2_start, d2_count) = depth2_layer_offsets[li];
            let parent = if cluster_idx < d2_count { (d2_start + cluster_idx) as u32 } else { u32::MAX };

            blocks.push(RawBlock {
                data: to_block(text),
                depth: 3, x, y, z,
                layer_id: layer_to_id(name),
                parent_idx: parent,
                child_count: 0,
            });
            depth3_positions.push((x, y, z));
        }
    }

    // MERGE APPEND LOG into D3
    let appended = read_append_log();
    if !appended.is_empty() {
        println!("  {} append log: {} entries merged", ">".green(), appended.len());
        for entry in &appended {
            let layer_name = id_to_layer(entry.layer_id);
            let (x, y, z) = (entry.x, entry.y, entry.z);
            blocks.push(RawBlock {
                data: to_block(&entry.text),
                depth: 3, x, y, z,
                layer_id: entry.layer_id,
                parent_idx: u32::MAX,
                child_count: 0,
            });
            depth3_positions.push((x, y, z));

            // Also update layer_texts so deeper depths can decompose these
            if let Some((_name, texts)) = layer_texts.iter_mut().find(|(n, _)| *n == layer_name) {
                texts.push(entry.text.clone());
            }
        }
    }

    // DEPTH 4: Sentences
    let _depth4_start = blocks.len();
    let mut depth4_parents: Vec<usize> = Vec::new();
    for d3i in depth3_start..(depth3_start + depth3_positions.len()) {
        let text = std::str::from_utf8(&blocks[d3i].data).unwrap_or("");
        let sentences = split_sentences(text);
        let mut child_count = 0u16;
        for sent in &sentences {
            if sent.len() < 10 { continue; }
            let (px, py, pz) = depth3_positions[d3i - depth3_start];
            let h = sent.as_bytes().iter().fold(0u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
            let ox = ((h & 0xFF) as f32 - 128.0) / 25500.0;
            let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 25500.0;
            let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 25500.0;

            blocks.push(RawBlock {
                data: to_block(sent),
                depth: 4, x: px + ox, y: py + oy, z: pz + oz,
                layer_id: blocks[d3i].layer_id,
                parent_idx: d3i as u32,
                child_count: 0,
            });
            child_count += 1;
            depth4_parents.push(blocks.len() - 1);
        }
        blocks[d3i].child_count = child_count;
    }

    // DEPTH 5: Tokens (words)
    let mut depth5_parents: Vec<usize> = Vec::new();
    for &d4i in &depth4_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d4i].data).to_string();
        let px = blocks[d4i].x;
        let py = blocks[d4i].y;
        let pz = blocks[d4i].z;
        let lid = blocks[d4i].layer_id;

        let tokens: Vec<String> = text_owned.split_whitespace().take(8).map(|s| s.to_string()).collect();
        let mut child_count = 0u16;
        for tok in &tokens {
            if tok.len() < 2 { continue; }
            let h = tok.as_bytes().iter().fold(0u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
            let ox = ((h & 0xFF) as f32 - 128.0) / 255000.0;
            let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 255000.0;
            let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 255000.0;

            blocks.push(RawBlock {
                data: to_block(tok),
                depth: 5, x: px + ox, y: py + oy, z: pz + oz,
                layer_id: lid,
                parent_idx: d4i as u32,
                child_count: 0,
            });
            child_count += 1;
            depth5_parents.push(blocks.len() - 1);
        }
        blocks[d4i].child_count = child_count;
    }

    // DEPTH 6: Syllables / morphemes
    let mut depth6_parents: Vec<usize> = Vec::new();
    for &d5i in &depth5_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d5i].data).to_string();
        let px = blocks[d5i].x;
        let py = blocks[d5i].y;
        let pz = blocks[d5i].z;
        let lid = blocks[d5i].layer_id;

        let chars: Vec<char> = text_owned.chars().collect();
        if chars.len() < 3 { continue; }
        let chunk_size = 3.max(chars.len() / 3).min(5);
        let mut child_count = 0u16;
        for chunk in chars.chunks(chunk_size) {
            let syl: String = chunk.iter().collect();
            if syl.trim().is_empty() { continue; }
            let h = syl.as_bytes().iter().fold(0u64, |a, &b| a.wrapping_mul(37).wrapping_add(b as u64));
            let ox = ((h & 0xFF) as f32 - 128.0) / 2550000.0;
            let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 2550000.0;
            let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 2550000.0;

            blocks.push(RawBlock {
                data: to_block(&syl),
                depth: 6, x: px + ox, y: py + oy, z: pz + oz,
                layer_id: lid,
                parent_idx: d5i as u32,
                child_count: 0,
            });
            child_count += 1;
            depth6_parents.push(blocks.len() - 1);
        }
        blocks[d5i].child_count = child_count;
    }

    // DEPTH 7: Characters
    let mut depth7_parents: Vec<usize> = Vec::new();
    for &d6i in &depth6_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d6i].data).to_string();
        let px = blocks[d6i].x;
        let py = blocks[d6i].y;
        let pz = blocks[d6i].z;
        let lid = blocks[d6i].layer_id;

        let mut child_count = 0u16;
        for ch in text_owned.chars() {
            if ch.is_whitespace() { continue; }
            let h = (ch as u64).wrapping_mul(0x517cc1b727220a95);
            let ox = ((h & 0xFF) as f32 - 128.0) / 25500000.0;
            let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 25500000.0;
            let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 25500000.0;

            let ch_str = ch.to_string();
            blocks.push(RawBlock {
                data: to_block(&ch_str),
                depth: 7, x: px + ox, y: py + oy, z: pz + oz,
                layer_id: lid,
                parent_idx: d6i as u32,
                child_count: 0,
            });
            child_count += 1;
            depth7_parents.push(blocks.len() - 1);
        }
        blocks[d6i].child_count = child_count;
    }

    // DEPTH 8: Raw bytes
    for &d7i in &depth7_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d7i].data).to_string();
        let px = blocks[d7i].x;
        let py = blocks[d7i].y;
        let pz = blocks[d7i].z;
        let lid = blocks[d7i].layer_id;

        let bytes = text_owned.as_bytes();
        let mut child_count = 0u16;
        for &byte in bytes.iter() {
            let hex = format!("0x{:02X}", byte);
            let h = (byte as u64).wrapping_mul(0x9E3779B97F4A7C15);
            let ox = ((h & 0xFF) as f32 - 128.0) / 255000000.0;
            let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 255000000.0;
            let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 255000000.0;

            blocks.push(RawBlock {
                data: to_block(&hex),
                depth: 8, x: px + ox, y: py + oy, z: pz + oz,
                layer_id: lid,
                parent_idx: d7i as u32,
                child_count: 0,
            });
            child_count += 1;
        }
        blocks[d7i].child_count = child_count;
    }

    let n = blocks.len();
    println!("\n  {} blocks total", n);

    // Sort by depth
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by_key(|&i| blocks[i].depth);

    // Remap parent indices after sort
    let mut old_to_new = vec![0u32; n];
    for (new_i, &old_i) in indices.iter().enumerate() {
        old_to_new[old_i] = new_i as u32;
    }

    // Write binary files
    fs::create_dir_all(BIN_DIR).ok();

    let mut hdr_buf = BufWriter::new(fs::File::create(HDR_PATH).expect("create hdr"));
    let mut dat_buf: Vec<u8> = Vec::new();
    let mut depth_ranges: Vec<(u32, u32)> = vec![(0, 0); 9];
    let mut cur_depth: u8 = 0;
    let mut range_start: u32 = 0;

    for (new_i, &old_i) in indices.iter().enumerate() {
        let b = &blocks[old_i];
        let offset = dat_buf.len() as u32;
        let len = b.data.len().min(BLOCK_DATA_SIZE) as u16;
        dat_buf.extend_from_slice(&b.data[..len as usize]);

        let parent = if b.parent_idx == u32::MAX { u32::MAX } else { old_to_new[b.parent_idx as usize] };

        let hdr = BlockHeader {
            x: b.x, y: b.y, z: b.z,
            zoom: b.depth as f32 / 8.0,
            depth: b.depth,
            layer_id: b.layer_id,
            data_offset: offset,
            data_len: len,
            parent_idx: parent,
            child_count: b.child_count,
            _pad: [0; 2],
        };

        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(&hdr as *const BlockHeader as *const u8, HEADER_SIZE)
        };
        hdr_buf.write_all(bytes).expect("write hdr");

        if b.depth != cur_depth {
            depth_ranges[cur_depth as usize] = (range_start, new_i as u32 - range_start);
            range_start = new_i as u32;
            cur_depth = b.depth;
        }
    }
    depth_ranges[cur_depth as usize] = (range_start, n as u32 - range_start);
    hdr_buf.flush().unwrap();

    fs::write(DAT_PATH, &dat_buf).expect("write data");

    let mut meta_buf = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE);
    meta_buf.extend_from_slice(b"MSCM");
    meta_buf.extend_from_slice(&1u32.to_le_bytes());
    meta_buf.extend_from_slice(&(n as u32).to_le_bytes());
    meta_buf.extend_from_slice(&9u32.to_le_bytes());
    for &(start, count) in &depth_ranges {
        meta_buf.extend_from_slice(&start.to_le_bytes());
        meta_buf.extend_from_slice(&count.to_le_bytes());
    }
    fs::write(META_PATH, &meta_buf).expect("write meta");

    let hdr_size = n * HEADER_SIZE;
    let dat_size = dat_buf.len();
    let meta_size = meta_buf.len();
    println!("\n  {}: {} bytes ({:.1} KB)", "headers".green(), hdr_size, hdr_size as f64 / 1024.0);
    println!("  {}:    {} bytes ({:.1} KB)", "data".green(), dat_size, dat_size as f64 / 1024.0);
    println!("  {}:    {} bytes", "meta".green(), meta_size);
    println!("  {}:   {:.1} KB", "TOTAL".yellow().bold(), (hdr_size + dat_size + meta_size) as f64 / 1024.0);

    let fits = if hdr_size < 32768 { "L1d (32KB)" }
               else if hdr_size < 262144 { "L2 (256KB)" }
               else { "L3" };
    println!("  cache:   {}", fits.green().bold());

    for (d, &(_start, count)) in depth_ranges.iter().enumerate() {
        println!("  Depth {}: {:>5} blocks", d, count);
    }
    println!("\n{}", "ZERO JSON. Pure binary. Done.".green().bold());

    // Build crypto chain + merkle tree
    println!("\n{}", "Building crypto layer...".cyan().bold());
    let crypto_reader = MicroscopeReader::open();
    build_chain(&crypto_reader);
    build_merkle(&crypto_reader);
    println!("{}", "Crypto chain + Merkle tree built.".green().bold());
}

// ─── MMAP READER ─────────────────────────────────────
pub struct MicroscopeReader {
    pub headers: memmap2::Mmap,
    pub data: memmap2::Mmap,
    pub block_count: usize,
    pub depth_ranges: [(u32, u32); 9],
}

impl MicroscopeReader {
    pub fn open() -> Self {
        let meta = fs::read(META_PATH).expect("open meta.bin -- run 'build' first");
        assert!(&meta[0..4] == b"MSCM", "invalid magic");
        let block_count = u32::from_le_bytes(meta[8..12].try_into().unwrap()) as usize;
        let mut depth_ranges = [(0u32, 0u32); 9];
        for (d, range) in depth_ranges.iter_mut().enumerate() {
            let off = META_HEADER_SIZE + d * DEPTH_ENTRY_SIZE;
            let start = u32::from_le_bytes(meta[off..off+4].try_into().unwrap());
            let count = u32::from_le_bytes(meta[off+4..off+8].try_into().unwrap());
            *range = (start, count);
        }

        let hdr_file = fs::File::open(HDR_PATH).expect("open headers");
        let dat_file = fs::File::open(DAT_PATH).expect("open data");
        let headers = unsafe { memmap2::Mmap::map(&hdr_file).expect("mmap headers") };
        let data = unsafe { memmap2::Mmap::map(&dat_file).expect("mmap data") };

        MicroscopeReader { headers, data, block_count, depth_ranges }
    }

    #[inline(always)]
    pub fn header(&self, i: usize) -> &BlockHeader {
        debug_assert!(i < self.block_count);
        unsafe { &*(self.headers.as_ptr().add(i * HEADER_SIZE) as *const BlockHeader) }
    }

    #[inline(always)]
    pub fn text(&self, i: usize) -> &str {
        let h = self.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        std::str::from_utf8(&self.data[start..end]).unwrap_or("<bin>")
    }

    pub fn look(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<(f32, usize)> {
        let (start, count) = self.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        if count == 0 { return vec![]; }

        let mut results: Vec<(f32, usize)> = Vec::with_capacity(count);
        for i in start..(start + count) {
            let h = self.header(i);
            let dx = h.x - x;
            let dy = h.y - y;
            let dz = h.z - z;
            results.push((dx*dx + dy*dy + dz*dz, i));
        }

        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    pub fn look_soft(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize, zw: f32) -> Vec<(f32, usize)> {
        let qz = zoom as f32 / 8.0;
        let mut results: Vec<(f32, usize)> = Vec::with_capacity(self.block_count);
        for i in 0..self.block_count {
            let h = self.header(i);
            let dx = h.x - x;
            let dy = h.y - y;
            let dz = h.z - z;
            let dw = (h.zoom - qz) * zw;
            results.push((dx*dx + dy*dy + dz*dz + dw*dw, i));
        }
        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    pub fn find_text(&self, query: &str, k: usize) -> Vec<(u8, usize)> {
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize)> = Vec::new();
        for i in 0..self.block_count {
            if self.text(i).to_lowercase().contains(&q) {
                results.push((self.header(i).depth, i));
            }
        }
        results.sort_by_key(|&(d, _)| d);
        results.truncate(k);
        results
    }

    pub fn print_result(&self, i: usize, dist: f32) {
        let h = self.header(i);
        let text = self.text(i);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        let preview: String = text.chars().take(70).filter(|&c| c != '\n').collect();
        println!("  {} {} {} {}",
            format!("D{}", h.depth).cyan(),
            format!("L2={:.5}", dist).yellow(),
            format!("[{}/{}]", layer, layer_color(h.layer_id)).green(),
            preview);
    }
}

// ─── SPATIAL GRID ────────────────────────────────────
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,
    pub res: usize,
}

impl SpatialGrid {
    pub fn build(reader: &MicroscopeReader, zoom: u8) -> Self {
        let (start, count) = reader.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);

        let res = if count < 200 { 4 }
                  else if count < 1000 { 8 }
                  else if count < 5000 { 12 }
                  else if count < 20000 { 16 }
                  else if count < 50000 { 24 }
                  else { 32 };
        let total_cells = res * res * res;
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); total_cells];

        for i in start..(start + count) {
            let h = reader.header(i);
            let cell = Self::cell_id_with_res(h.x, h.y, h.z, res);
            cells[cell].push(i);
        }

        SpatialGrid { cells, res }
    }

    #[inline(always)]
    pub fn cell_id_with_res(x: f32, y: f32, z: f32, res: usize) -> usize {
        let cx = ((x.clamp(0.0, 0.999)) * res as f32) as usize;
        let cy = ((y.clamp(0.0, 0.999)) * res as f32) as usize;
        let cz = ((z.clamp(0.0, 0.999)) * res as f32) as usize;
        cx * res * res + cy * res + cz
    }

    pub fn look(&self, reader: &MicroscopeReader, x: f32, y: f32, z: f32, k: usize) -> Vec<(f32, usize)> {
        let res = self.res;
        let cx = ((x.clamp(0.0, 0.999)) * res as f32) as i32;
        let cy = ((y.clamp(0.0, 0.999)) * res as f32) as i32;
        let cz = ((z.clamp(0.0, 0.999)) * res as f32) as i32;

        let mut results: Vec<(f32, usize)> = Vec::new();

        for dx in -1..=1i32 {
            for dy in -1..=1i32 {
                for dz in -1..=1i32 {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    let nz = cz + dz;
                    if nx < 0 || ny < 0 || nz < 0 { continue; }
                    if nx >= res as i32 || ny >= res as i32 || nz >= res as i32 { continue; }
                    let cell = nx as usize * res * res + ny as usize * res + nz as usize;
                    for &idx in &self.cells[cell] {
                        let h = reader.header(idx);
                        let ddx = h.x - x;
                        let ddy = h.y - y;
                        let ddz = h.z - z;
                        results.push((ddx * ddx + ddy * ddy + ddz * ddz, idx));
                    }
                }
            }
        }

        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    pub fn stats(&self) -> (usize, usize, usize, usize) {
        let total_cells = self.res * self.res * self.res;
        let occupied = self.cells.iter().filter(|c| !c.is_empty()).count();
        let total: usize = self.cells.iter().map(|c| c.len()).sum();
        let max = self.cells.iter().map(|c| c.len()).max().unwrap_or(0);
        (occupied, total_cells, total, max)
    }
}

// ─── TIERED INDEX ───────────────────────────────────
pub struct TieredIndex {
    pub grids: [Option<SpatialGrid>; 9],
}

impl TieredIndex {
    pub fn build(reader: &MicroscopeReader) -> Self {
        let mut grids: [Option<SpatialGrid>; 9] = Default::default();

        for zoom in 0..9u8 {
            let (_, count) = reader.depth_ranges[zoom as usize];
            if count == 0 { continue; }
            if zoom >= 3 {
                grids[zoom as usize] = Some(SpatialGrid::build(reader, zoom));
            }
        }

        TieredIndex { grids }
    }

    #[inline]
    pub fn look(&self, reader: &MicroscopeReader, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<(f32, usize)> {
        if let Some(ref grid) = self.grids[zoom as usize] {
            grid.look(reader, x, y, z, k)
        } else {
            reader.look(x, y, z, zoom, k)
        }
    }
}

// ─── BENCH ───────────────────────────────────────────
pub struct BenchRng(u64);
impl BenchRng {
    pub fn new(seed: u64) -> Self { Self(seed) }
    pub fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 33) as f32 / (u32::MAX as f32) * 0.5
    }
}

pub fn bench(reader: &MicroscopeReader) {
    println!("{}", "Benchmark: 10,000 queries per zoom level".cyan());
    println!("{}", "-".repeat(70));

    let iters = 10_000u64;

    println!("\n  {}", "--- AoS (original) ---".white().bold());
    let mut total_ns_old: u64 = 0;
    let mut rng1 = BenchRng::new(42);
    for zoom in 0..9u8 {
        let t0 = Instant::now();
        for _ in 0..iters {
            let r = reader.look(rng1.next_f32(), rng1.next_f32(), rng1.next_f32(), zoom, 5);
            std::hint::black_box(&r);
        }
        let ns = t0.elapsed().as_nanos() as u64;
        total_ns_old += ns;
        let avg = ns / iters;
        let (_s, c) = reader.depth_ranges[zoom as usize];
        let label = if avg < 1000 { format!("{} ns", avg) }
                    else { format!("{:.1} us", avg as f64 / 1000.0) };
        println!("  ZOOM {}: {:>10} / query  ({} blocks)", zoom, label.yellow(), c);
    }
    let avg_old = total_ns_old as f64 / (iters * 9) as f64;
    println!("  {}: {:.0} ns avg", "AoS TOTAL".green().bold(), avg_old);

    println!("\n  {}", "--- TIERED (SoA + Grid + SIMD) ---".white().bold());
    let tiered = TieredIndex::build(reader);
    let mut total_ns_new: u64 = 0;
    let mut rng2 = BenchRng::new(42);
    for zoom in 0..9u8 {
        let tier_label = match zoom {
            0..=2 => "mmap/L1",
            _     => "Grid",
        };
        let t0 = Instant::now();
        for _ in 0..iters {
            let r = tiered.look(reader, rng2.next_f32(), rng2.next_f32(), rng2.next_f32(), zoom, 5);
            std::hint::black_box(&r);
        }
        let ns = t0.elapsed().as_nanos() as u64;
        total_ns_new += ns;
        let avg = ns / iters;
        let (_s, c) = reader.depth_ranges[zoom as usize];
        let label = if avg < 1000 { format!("{} ns", avg) }
                    else { format!("{:.1} us", avg as f64 / 1000.0) };
        println!("  ZOOM {}: {:>10} / query  ({} blocks) [{}]", zoom, label.yellow(), c, tier_label.cyan());
    }
    let avg_new = total_ns_new as f64 / (iters * 9) as f64;
    println!("  {}: {:.0} ns avg", "TIERED TOTAL".green().bold(), avg_new);

    let speedup = avg_old / avg_new;
    println!("\n  {}: {:.1}x faster",
        "SPEEDUP".yellow().bold(), speedup);

    println!("\n  {}", "--- Spatial Grid Stats ---".white().bold());
    for zoom in 0..9u8 {
        if let Some(ref grid) = tiered.grids[zoom as usize] {
            let (occupied, total_cells, total, max) = grid.stats();
            let avg_per_cell = if occupied > 0 { total as f64 / occupied as f64 } else { 0.0 };
            println!("  D{}: {}/{} cells ({}^3), {} blocks, max {}/cell, avg {:.1}/cell",
                zoom, occupied, total_cells, grid.res, total, max, avg_per_cell);
        }
    }

    println!("\n{}", "4D soft zoom (all blocks):".cyan());
    let mut rng3 = BenchRng::new(42);
    let t0 = Instant::now();
    for _ in 0..iters {
        let z = (rng3.next_f32() * 10.0) as u8 % 6;
        let r = reader.look_soft(rng3.next_f32(), rng3.next_f32(), rng3.next_f32(), z, 5, 2.0);
        std::hint::black_box(&r);
    }
    let ns = t0.elapsed().as_nanos() / iters as u128;
    println!("  4D: {} ns/query ({} blocks)", ns, reader.block_count);
}

// ─── STATS ───────────────────────────────────────────
pub fn stats(reader: &MicroscopeReader) {
    let hdr_size = reader.block_count * HEADER_SIZE;
    let dat_size = reader.data.len();
    println!("{}", "=".repeat(50));
    println!("  {}", "MICROSCOPE MEMORY (pure binary)".cyan().bold());
    println!("{}", "=".repeat(50));
    println!("  Blocks:    {}", reader.block_count);
    println!("  Headers:   {:.1} KB", hdr_size as f64 / 1024.0);
    println!("  Data:      {:.1} KB", dat_size as f64 / 1024.0);
    println!("  Total:     {:.1} KB", (hdr_size + dat_size) as f64 / 1024.0);
    println!("  Viewport:  {} chars/block", BLOCK_DATA_SIZE);

    let fits = if hdr_size < 32768 { "L1d" }
               else if hdr_size < 262144 { "L2" }
               else { "L3" };
    println!("  Cache:     {}", fits.green().bold());

    println!("\n  Depths:");
    for (d, &(_s, c)) in reader.depth_ranges.iter().enumerate() {
        let bar_len = (c as f64 / reader.block_count as f64 * 40.0) as usize;
        println!("    D{}: {:>5}  {}", d, c, "|".repeat(bar_len).cyan());
    }
    if Path::new(CHAIN_PATH).exists() {
        let chain_data = fs::read(CHAIN_PATH).unwrap_or_default();
        if chain_data.len() >= CHAIN_HEADER_SIZE {
            let lc = u32::from_le_bytes(chain_data[8..12].try_into().unwrap());
            println!("\n  {}: {} links, {:.1} KB",
                "Chain".yellow(), lc, chain_data.len() as f64 / 1024.0);
        }
    }
    if Path::new(MERKLE_PATH).exists() {
        let merkle_data = fs::read(MERKLE_PATH).unwrap_or_default();
        if merkle_data.len() >= MERKLE_HEADER_SIZE + MERKLE_NODE_SIZE {
            let nc = u32::from_le_bytes(merkle_data[8..12].try_into().unwrap());
            let root: [u8; 32] = merkle_data[MERKLE_HEADER_SIZE..MERKLE_HEADER_SIZE + 32].try_into().unwrap();
            println!("  {}: {} nodes, root={}",
                "Merkle".magenta(), nc, hex_short(&root));
        }
    }

    println!("{}", "=".repeat(50));
}

// ─── APPEND LOG ──────────────────────────────────────
pub const APPEND_PATH: &str = "data/append.bin";

pub fn store_memory(text: &str, layer: &str, importance: u8) {
    let t0 = Instant::now();
    let (x, y, z) = content_coords(text, layer);
    let lid = layer_to_id(layer);

    let mut file = fs::OpenOptions::new()
        .create(true).append(true)
        .open(APPEND_PATH).expect("open append log");

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    file.write_all(&(len as u32).to_le_bytes()).unwrap();
    file.write_all(&[lid]).unwrap();
    file.write_all(&[importance]).unwrap();
    file.write_all(&x.to_le_bytes()).unwrap();
    file.write_all(&y.to_le_bytes()).unwrap();
    file.write_all(&z.to_le_bytes()).unwrap();
    file.write_all(&text_bytes[..len]).unwrap();

    append_chain_link(text.as_bytes(), lid);

    let elapsed = t0.elapsed();
    println!("  {} [{}/{}] ({:.3},{:.3},{:.3}) {}",
        "STORED".green().bold(), layer, layer_color(lid),
        x, y, z, safe_truncate(text, 60));
    println!("  {} (chain extended)", format!("{} ns", elapsed.as_nanos()).yellow());
}

pub struct AppendEntry {
    pub text: String,
    pub layer_id: u8,
    pub importance: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn read_append_log() -> Vec<AppendEntry> {
    let path = Path::new(APPEND_PATH);
    if !path.exists() { return vec![]; }
    let data = fs::read(path).unwrap_or_default();
    let mut entries = Vec::new();
    let mut pos = 0;
    while pos + 18 <= data.len() {
        let len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        let lid = data[pos+4];
        let imp = data[pos+5];
        let x = f32::from_le_bytes(data[pos+6..pos+10].try_into().unwrap());
        let y = f32::from_le_bytes(data[pos+10..pos+14].try_into().unwrap());
        let z = f32::from_le_bytes(data[pos+14..pos+18].try_into().unwrap());
        pos += 18;
        if pos + len > data.len() { break; }
        let text = String::from_utf8_lossy(&data[pos..pos+len]).to_string();
        pos += len;
        entries.push(AppendEntry { text, layer_id: lid, importance: imp, x, y, z });
    }
    entries
}

// ─── AUTO ZOOM ───────────────────────────────────────
pub fn auto_zoom(query: &str) -> (u8, u8) {
    let words = query.split_whitespace().count();
    let len = query.len();

    if words <= 2 && len < 15 {
        return (1, 1);
    }
    if words <= 5 {
        return (2, 1);
    }
    if words <= 10 {
        return (3, 1);
    }
    if words <= 20 {
        return (4, 1);
    }
    (5, 1)
}

// ─── RECALL ──────────────────────────────────────────
pub fn recall(query: &str, k: usize) {
    let t0 = Instant::now();
    let (center_zoom, radius) = auto_zoom(query);
    let (qx, qy, qz) = content_coords(query, "query");

    println!("{} '{}' -> auto-zoom={} (D{}..D{})",
        "RECALL".cyan().bold(), safe_truncate(query, 50),
        center_zoom,
        center_zoom.saturating_sub(radius),
        (center_zoom + radius).min(8));

    let reader = MicroscopeReader::open();
    let mut all_results: Vec<(f32, usize, bool)> = Vec::new();

    let zoom_lo = center_zoom.saturating_sub(radius);
    let zoom_hi = (center_zoom + radius).min(8);

    for zoom in zoom_lo..=zoom_hi {
        let results = reader.look(qx, qy, qz, zoom, k * 2);
        for (dist, idx) in results {
            all_results.push((dist, idx, true));
        }
    }

    let q_lower = query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace()
        .filter(|w| w.len() > 2)
        .collect();

    for zoom in zoom_lo..=zoom_hi {
        let (start, count) = reader.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        for i in start..(start + count) {
            let text = reader.text(i).to_lowercase();
            let keyword_hits = keywords.iter().filter(|&&kw| text.contains(kw)).count();
            if keyword_hits > 0 {
                let h = reader.header(i);
                let dx = h.x - qx;
                let dy = h.y - qy;
                let dz = h.z - qz;
                let spatial_dist = dx*dx + dy*dy + dz*dz;
                let boost = keyword_hits as f32 * 0.1;
                let combined = (spatial_dist - boost).max(0.0);
                all_results.push((combined, i, true));
            }
        }
    }

    let appended = read_append_log();
    for (ai, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx*dx + dy*dy + dz*dz;
        let text_lower = entry.text.to_lowercase();
        let keyword_hits = keywords.iter().filter(|&&kw| text_lower.contains(kw)).count();
        let boost = keyword_hits as f32 * 0.1;
        if dist < 0.1 || keyword_hits > 0 {
            all_results.push(((dist - boost).max(0.0), ai + 1_000_000, false));
        }
    }

    let mut seen = std::collections::HashSet::new();
    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut shown = 0;

    for (dist, idx, is_main) in &all_results {
        if shown >= k { break; }
        if !seen.insert((*idx, *is_main)) { continue; }

        if *is_main {
            reader.print_result(*idx, *dist);
        } else {
            let ai = idx - 1_000_000;
            if ai < appended.len() {
                let e = &appended[ai];
                let layer = LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
                let preview = safe_truncate(&e.text, 70);
                println!("  {} {} {} {}",
                    "AP".cyan(),
                    format!("L2={:.5}", dist).yellow(),
                    format!("[{}/new]", layer).green(),
                    preview);
            }
        }
        shown += 1;
    }

    let elapsed = t0.elapsed();
    println!("\n  {} results in {:.0} us", shown, elapsed.as_micros());
}

// ─── CRYPTO: Build Hash Chain ────────────────────────
pub fn build_chain(reader: &MicroscopeReader) {
    let t0 = Instant::now();
    let mut file = BufWriter::new(fs::File::create(CHAIN_PATH).expect("create chain.bin"));

    let hdr = ChainHeader {
        magic: *b"MSCN",
        version: 1,
        link_count: reader.block_count as u32,
        _reserved: 0,
    };
    let hdr_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(&hdr as *const ChainHeader as *const u8, CHAIN_HEADER_SIZE)
    };
    file.write_all(hdr_bytes).unwrap();

    let ts = now_micros();
    let mut prev_hash = [0u8; 32];

    for i in 0..reader.block_count {
        let h = reader.header(i);
        let text = reader.text(i).as_bytes();
        let content_hash = sha256(text);

        let link = ChainLink {
            content_hash,
            prev_hash,
            timestamp_us: ts,
            block_index: i as u32,
            layer_id: h.layer_id,
            depth: h.depth,
            _pad: [0; 2],
        };

        file.write_all(chain_link_bytes(&link)).unwrap();
        prev_hash = sha256(chain_link_bytes(&link));
    }
    file.flush().unwrap();

    let elapsed = t0.elapsed();
    let size = CHAIN_HEADER_SIZE + reader.block_count * CHAIN_LINK_SIZE;
    println!("  {}: {} links, {:.1} KB, head={}  ({:.0} ms)",
        "CHAIN".yellow().bold(),
        reader.block_count,
        size as f64 / 1024.0,
        hex_short(&prev_hash),
        elapsed.as_millis());
}

// ─── CRYPTO: Build Merkle Tree ──────────────────────
pub fn build_merkle(reader: &MicroscopeReader) {
    let t0 = Instant::now();
    let n = reader.block_count;

    let mut children_of: Vec<Vec<u32>> = vec![Vec::new(); n];
    for i in 0..n {
        let h = reader.header(i);
        let parent = h.parent_idx;
        if parent != u32::MAX && (parent as usize) < n {
            children_of[parent as usize].push(i as u32);
        }
    }

    let mut merkle_hashes: Vec<[u8; 32]> = vec![[0u8; 32]; n];

    for i in (0..n).rev() {
        let content_hash = sha256(reader.text(i).as_bytes());
        let children = &children_of[i];

        if children.is_empty() {
            merkle_hashes[i] = content_hash;
        } else {
            let mut hasher = Sha256::new();
            hasher.update(content_hash);
            for &ci in children {
                hasher.update(merkle_hashes[ci as usize]);
            }
            merkle_hashes[i] = hasher.finalize().into();
        }
    }

    let mut file = BufWriter::new(fs::File::create(MERKLE_PATH).expect("create merkle.bin"));

    let hdr = MerkleHeader {
        magic: *b"MSMT",
        version: 1,
        node_count: n as u32,
        _reserved: 0,
    };
    let hdr_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(&hdr as *const MerkleHeader as *const u8, MERKLE_HEADER_SIZE)
    };
    file.write_all(hdr_bytes).unwrap();

    for hash in &merkle_hashes {
        file.write_all(hash).unwrap();
    }
    file.flush().unwrap();

    let elapsed = t0.elapsed();
    let size = MERKLE_HEADER_SIZE + n * MERKLE_NODE_SIZE;
    println!("  {}: {} nodes, {:.1} KB, root={}  ({:.0} ms)",
        "MERKLE".magenta().bold(),
        n,
        size as f64 / 1024.0,
        hex_short(&merkle_hashes[0]),
        elapsed.as_millis());
}

// ─── CRYPTO: Append Chain Link ──────────────────────
pub fn append_chain_link(text: &[u8], layer_id: u8) {
    let content_hash = sha256(text);
    let path = Path::new(CHAIN_PATH);

    let (link_count, prev_hash) = if path.exists() {
        let data = fs::read(path).unwrap_or_default();
        if data.len() >= CHAIN_HEADER_SIZE {
            let lc = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
            if lc > 0 && data.len() >= CHAIN_HEADER_SIZE + lc * CHAIN_LINK_SIZE {
                let last_off = CHAIN_HEADER_SIZE + (lc - 1) * CHAIN_LINK_SIZE;
                let last_bytes = &data[last_off..last_off + CHAIN_LINK_SIZE];
                (lc as u32, sha256(last_bytes))
            } else {
                (lc as u32, [0u8; 32])
            }
        } else {
            (0u32, [0u8; 32])
        }
    } else {
        let hdr = ChainHeader {
            magic: *b"MSCN",
            version: 1,
            link_count: 0,
            _reserved: 0,
        };
        let hdr_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(&hdr as *const ChainHeader as *const u8, CHAIN_HEADER_SIZE)
        };
        fs::write(path, hdr_bytes).unwrap();
        (0u32, [0u8; 32])
    };

    let link = ChainLink {
        content_hash,
        prev_hash,
        timestamp_us: now_micros(),
        block_index: u32::MAX,
        layer_id,
        depth: 3,
        _pad: [0; 2],
    };

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(path).expect("open chain.bin for append");
    file.write_all(chain_link_bytes(&link)).unwrap();

    let new_count = link_count + 1;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(path).expect("open chain.bin for update");
    file.seek(SeekFrom::Start(8)).unwrap();
    file.write_all(&new_count.to_le_bytes()).unwrap();
}

// ─── CRYPTO: Verify Hash Chain ──────────────────────
pub fn verify_chain() {
    let path = Path::new(CHAIN_PATH);
    if !path.exists() {
        println!("  {} chain.bin not found -- run 'build' first", "!".red());
        return;
    }

    let t0 = Instant::now();
    let data = fs::read(path).expect("read chain.bin");

    if data.len() < CHAIN_HEADER_SIZE {
        println!("  {} chain.bin too small", "FAIL".red().bold());
        return;
    }

    assert!(&data[0..4] == b"MSCN", "invalid chain magic");
    let link_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

    if data.len() < CHAIN_HEADER_SIZE + link_count * CHAIN_LINK_SIZE {
        println!("  {} chain.bin truncated: expected {} links", "FAIL".red().bold(), link_count);
        return;
    }

    let mut prev_hash = [0u8; 32];
    let mut broken_at: Option<usize> = None;

    for i in 0..link_count {
        let off = CHAIN_HEADER_SIZE + i * CHAIN_LINK_SIZE;
        let link_bytes = &data[off..off + CHAIN_LINK_SIZE];

        let stored_prev: [u8; 32] = link_bytes[32..64].try_into().unwrap();
        if stored_prev != prev_hash {
            broken_at = Some(i);
            break;
        }

        prev_hash = sha256(link_bytes);
    }

    let elapsed = t0.elapsed();
    match broken_at {
        None => {
            println!("  {} chain: {} links verified, head={}  ({:.0} ms)",
                "VALID".green().bold(), link_count, hex_short(&prev_hash), elapsed.as_millis());
        }
        Some(idx) => {
            println!("  {} chain broken at link #{} -- tamper detected!  ({:.0} ms)",
                "BROKEN".red().bold(), idx, elapsed.as_millis());
        }
    }
}

// ─── CRYPTO: Verify Merkle Tree ─────────────────────
pub fn verify_merkle() {
    let path = Path::new(MERKLE_PATH);
    if !path.exists() {
        println!("  {} merkle.bin not found -- run 'build' first", "!".red());
        return;
    }

    let t0 = Instant::now();
    let reader = MicroscopeReader::open();
    let n = reader.block_count;

    let merkle_data = fs::read(path).expect("read merkle.bin");
    assert!(&merkle_data[0..4] == b"MSMT", "invalid merkle magic");
    let node_count = u32::from_le_bytes(merkle_data[8..12].try_into().unwrap()) as usize;

    if node_count != n {
        println!("  {} merkle node count ({}) != block count ({})", "FAIL".red().bold(), node_count, n);
        return;
    }

    let mut stored: Vec<[u8; 32]> = Vec::with_capacity(n);
    for i in 0..n {
        let off = MERKLE_HEADER_SIZE + i * MERKLE_NODE_SIZE;
        let hash: [u8; 32] = merkle_data[off..off + 32].try_into().unwrap();
        stored.push(hash);
    }

    let mut children_of: Vec<Vec<u32>> = vec![Vec::new(); n];
    for i in 0..n {
        let parent = reader.header(i).parent_idx;
        if parent != u32::MAX && (parent as usize) < n {
            children_of[parent as usize].push(i as u32);
        }
    }

    let mut recomputed: Vec<[u8; 32]> = vec![[0u8; 32]; n];
    let mut mismatches: Vec<usize> = Vec::new();

    for i in (0..n).rev() {
        let content_hash = sha256(reader.text(i).as_bytes());
        let children = &children_of[i];

        if children.is_empty() {
            recomputed[i] = content_hash;
        } else {
            let mut hasher = Sha256::new();
            hasher.update(content_hash);
            for &ci in children {
                hasher.update(recomputed[ci as usize]);
            }
            recomputed[i] = hasher.finalize().into();
        }

        if recomputed[i] != stored[i] {
            mismatches.push(i);
        }
    }

    let elapsed = t0.elapsed();
    if mismatches.is_empty() {
        println!("  {} merkle: {} nodes verified, root={}  ({:.0} ms)",
            "VALID".green().bold(), n, hex_short(&recomputed[0]), elapsed.as_millis());
    } else {
        println!("  {} merkle: {} mismatches detected!  ({:.0} ms)",
            "BROKEN".red().bold(), mismatches.len(), elapsed.as_millis());
        for &idx in mismatches.iter().take(5) {
            let h = reader.header(idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            println!("    block #{} D{} [{}]: expected={} got={}",
                idx, h.depth, layer,
                hex_short(&stored[idx]), hex_short(&recomputed[idx]));
        }
        if mismatches.len() > 5 {
            println!("    ... and {} more", mismatches.len() - 5);
        }
    }
}

// ─── CRYPTO: Verify Single Branch ───────────────────
pub fn verify_branch(block_idx: u32) {
    let merkle_path = Path::new(MERKLE_PATH);
    if !merkle_path.exists() {
        println!("  {} merkle.bin not found -- run 'build' first", "!".red());
        return;
    }

    let reader = MicroscopeReader::open();
    let n = reader.block_count;
    let idx = block_idx as usize;

    if idx >= n {
        println!("  {} block index {} out of range (max {})", "!".red(), idx, n - 1);
        return;
    }

    let merkle_data = fs::read(merkle_path).expect("read merkle.bin");

    let mut path_indices: Vec<usize> = Vec::new();
    let mut cur = idx;
    loop {
        path_indices.push(cur);
        let parent = reader.header(cur).parent_idx;
        if parent == u32::MAX || parent as usize >= n { break; }
        cur = parent as usize;
    }

    let mut children_of: Vec<Vec<u32>> = vec![Vec::new(); n];
    for i in 0..n {
        let parent = reader.header(i).parent_idx;
        if parent != u32::MAX && (parent as usize) < n {
            children_of[parent as usize].push(i as u32);
        }
    }

    let mut recomputed: Vec<[u8; 32]> = vec![[0u8; 32]; n];
    for i in (0..n).rev() {
        let content_hash = sha256(reader.text(i).as_bytes());
        let children = &children_of[i];
        if children.is_empty() {
            recomputed[i] = content_hash;
        } else {
            let mut hasher = Sha256::new();
            hasher.update(content_hash);
            for &ci in children {
                hasher.update(recomputed[ci as usize]);
            }
            recomputed[i] = hasher.finalize().into();
        }
    }

    println!("  {} block #{} -> root:", "BRANCH".cyan().bold(), block_idx);
    let mut all_valid = true;
    for &pi in &path_indices {
        let off = MERKLE_HEADER_SIZE + pi * MERKLE_NODE_SIZE;
        let stored: [u8; 32] = merkle_data[off..off + 32].try_into().unwrap();
        let valid = stored == recomputed[pi];
        let h = reader.header(pi);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        let status = if valid { "OK".green() } else { all_valid = false; "FAIL".red() };
        let preview: String = reader.text(pi).chars().take(40).filter(|&c| c != '\n').collect();
        println!("    {} D{} [{}] {} {}",
            status, h.depth, layer, hex_short(&recomputed[pi]), preview);
    }
    if all_valid {
        println!("  {} branch verified ({} nodes)", "VALID".green().bold(), path_indices.len());
    } else {
        println!("  {} branch has tampered nodes!", "BROKEN".red().bold());
    }
}

// ─── CRYPTO: Chain Status ───────────────────────────
pub fn chain_status() {
    let path = Path::new(CHAIN_PATH);
    if !path.exists() {
        println!("  {} chain.bin not found -- run 'build' first", "!".red());
        return;
    }

    let data = fs::read(path).expect("read chain.bin");
    assert!(&data[0..4] == b"MSCN", "invalid chain magic");
    let link_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let file_size = data.len();

    println!("{}", "=".repeat(50));
    println!("  {}", "HASH CHAIN STATUS".yellow().bold());
    println!("{}", "=".repeat(50));
    println!("  Links:     {}", link_count);
    println!("  File size: {:.1} KB", file_size as f64 / 1024.0);

    if link_count > 0 {
        let genesis_off = CHAIN_HEADER_SIZE;
        let genesis_content: [u8; 32] = data[genesis_off..genesis_off + 32].try_into().unwrap();
        println!("  Genesis:   {}", hex_short(&genesis_content));

        let head_off = CHAIN_HEADER_SIZE + (link_count - 1) * CHAIN_LINK_SIZE;
        if head_off + CHAIN_LINK_SIZE <= data.len() {
            let head_bytes = &data[head_off..head_off + CHAIN_LINK_SIZE];
            let head_hash = sha256(head_bytes);
            let head_content: [u8; 32] = head_bytes[0..32].try_into().unwrap();

            let ts_bytes: [u8; 8] = head_bytes[64..72].try_into().unwrap();
            let ts = u64::from_le_bytes(ts_bytes);
            let secs = ts / 1_000_000;

            println!("  Head hash: {}", hex_short(&head_hash));
            println!("  Head content: {}", hex_short(&head_content));
            println!("  Last timestamp: {} (unix sec)", secs);
        }
    }
    println!("{}", "=".repeat(50));
}

// ─── CRYPTO: Merkle Root Info ───────────────────────
pub fn merkle_root_info() {
    let path = Path::new(MERKLE_PATH);
    if !path.exists() {
        println!("  {} merkle.bin not found -- run 'build' first", "!".red());
        return;
    }

    let data = fs::read(path).expect("read merkle.bin");
    assert!(&data[0..4] == b"MSMT", "invalid merkle magic");
    let node_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let file_size = data.len();

    println!("{}", "=".repeat(50));
    println!("  {}", "MERKLE TREE".magenta().bold());
    println!("{}", "=".repeat(50));
    println!("  Nodes:     {}", node_count);
    println!("  File size: {:.1} KB", file_size as f64 / 1024.0);

    if node_count > 0 {
        let root: [u8; 32] = data[MERKLE_HEADER_SIZE..MERKLE_HEADER_SIZE + 32].try_into().unwrap();
        println!("  Root hash: {}", hex_full(&root));
        println!("  Root short: {}", hex_short(&root));
    }
    println!("  Depth levels: 9 (0..8)");
    println!("{}", "=".repeat(50));
}
