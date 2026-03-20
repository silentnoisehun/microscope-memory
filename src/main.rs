//! Microscope Memory — zoom-based hierarchical memory
//!
//! ZERO JSON. Pure binary. mmap. Sub-microsecond.
//!
//! CPU analogy: data exists in uniform blocks at every depth.
//! The query's zoom level determines which layer you see.
//! Same block size, different depth. Like a magnifying glass on silicon.
//!
//! Pipeline: raw memory files → binary blocks → mmap → L2 search
//!
//! Usage:
//!   microscope-mem build                    # layers/ → binary mmap
//!   microscope-mem look 0.25 0.25 0.25 3    # x y z zoom
//!   microscope-mem bench                    # speed test
//!   microscope-mem stats                    # structure info
//!   microscope-mem find "Ora"               # text search
//!   microscope-mem embed "query"            # semantic search with embeddings
//!   microscope-mem stream                    # start streaming update server

mod embeddings;
mod streaming;

use std::collections::HashMap;
use std::fs;
use std::io::{Write as IoWrite, BufWriter, BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use clap::{Parser, Subcommand};
use colored::Colorize;

// ─── Constants ───────────────────────────────────────
const BLOCK_DATA_SIZE: usize = 256;
const LAYERS_DIR: &str = r"D:\Claude Memory\layers";
const BIN_DIR: &str = r"D:\Claude Memory\microscope";
const HDR_PATH: &str = r"D:\Claude Memory\microscope\microscope.bin";
const DAT_PATH: &str = r"D:\Claude Memory\microscope\data.bin";
const META_PATH: &str = r"D:\Claude Memory\microscope\meta.bin";

// ─── Block header: 32 bytes, packed, mmap-ready ──────
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct BlockHeader {
    x: f32,            // 4  — spatial position
    y: f32,            // 4
    z: f32,            // 4
    zoom: f32,         // 4  — depth / 8.0 (normalized)
    depth: u8,         // 1  — 0..8
    layer_id: u8,      // 1  — which memory layer
    data_offset: u32,  // 4  — byte offset into data.bin
    data_len: u16,     // 2  — actual text bytes (<= 256)
    parent_idx: u32,   // 4  — parent block index (u32::MAX = root)
    child_count: u16,  // 2  — number of children
    _pad: [u8; 2],     // 2  — align to 32
}

const HEADER_SIZE: usize = 32;

// ─── Meta header: 48 bytes at start of meta.bin ──────
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct MetaHeader {
    magic: [u8; 4],       // "MSCM"
    version: u32,         // 1
    block_count: u32,     // total blocks
    depth_count: u32,     // 6
    // depth_ranges: 6 x (start: u32, count: u32) = 48 bytes follow
}

const META_HEADER_SIZE: usize = 16;
const DEPTH_ENTRY_SIZE: usize = 8; // u32 start + u32 count

// ─── Layer mapping ───────────────────────────────────
const LAYER_NAMES: &[&str] = &[
    "identity", "long_term", "short_term", "associative", "emotional",
    "relational", "reflections", "crypto_chain", "echo_cache", "rust_state",
];

fn layer_color(id: u8) -> &'static str {
    match id {
        0 => "white", 1 => "blue", 2 => "cyan", 3 => "green", 4 => "red",
        5 => "yellow", 6 => "magenta", 7 => "orange", 8 => "lime", 9 => "purple",
        _ => "white",
    }
}

fn layer_to_id(name: &str) -> u8 {
    LAYER_NAMES.iter().position(|&n| n == name).unwrap_or(0) as u8
}

// ─── Deterministic coords from content hash ──────────
fn content_coords(text: &str, layer: &str) -> (f32, f32, f32) {
    // Simple hash → 3D position
    let mut h: [u64; 3] = [0xcbf29ce484222325, 0x100000001b3, 0xa5a5a5a5a5a5a5a5];
    for &b in text.as_bytes().iter().take(128) {
        h[0] = h[0].wrapping_mul(0x100000001b3) ^ b as u64;
        h[1] = h[1].wrapping_mul(0x100000001b3) ^ b as u64;
        h[2] = h[2].wrapping_mul(0x1000193) ^ b as u64;
    }
    let bx = (h[0] & 0xFFFF) as f32 / 65535.0;
    let by = (h[1] & 0xFFFF) as f32 / 65535.0;
    let bz = (h[2] & 0xFFFF) as f32 / 65535.0;

    // Layer offset
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
fn safe_truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes { return s.to_string(); }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    s[..end].to_string()
}

// ─── Truncate text to block size ─────────────────────
fn to_block(text: &str) -> Vec<u8> {
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
struct RawBlock {
    data: Vec<u8>,
    depth: u8,
    x: f32,
    y: f32,
    z: f32,
    layer_id: u8,
    parent_idx: u32,
    child_count: u16,
}

// ─── Extract text values from minimal JSON parsing ───
// Zero serde_json dependency for layer reading.
// Layers are simple enough: arrays of objects or objects of objects.
// We do line-by-line key extraction instead of full parse.

fn extract_texts_from_file(path: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    let file = match fs::File::open(path) { Ok(f) => f, Err(_) => return texts };
    let reader = BufReader::new(file);
    let mut current_text = String::new();
    let mut in_content = false;

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let trimmed = line.trim();

        // Look for content-bearing keys
        for key in &["\"content\":", "\"text\":", "\"content_summary\":", "\"pattern\":", "\"label\":", "\"name\":"] {
            if let Some(pos) = trimmed.find(key) {
                let after = &trimmed[pos + key.len()..].trim_start();
                if after.starts_with('"') {
                    // Extract string value
                    let val = extract_json_string(after);
                    if val.len() > 3 {
                        texts.push(val);
                    }
                }
            }
        }

        // For response fields in echo_cache
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

    // If no structured content found, read raw and chunk
    if texts.is_empty() {
        if let Ok(raw) = fs::read_to_string(path) {
            // Chunk the raw content
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

fn extract_json_string(s: &str) -> String {
    // s starts with "
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
fn split_sentences(text: &str) -> Vec<String> {
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

// ─── BUILD: layers/ → binary ─────────────────────────
fn build() {
    println!("{}", "Building microscope from raw layers (zero JSON)...".cyan().bold());

    let layer_files = [
        "long_term", "short_term", "associative", "emotional",
        "relational", "reflections", "crypto_chain", "echo_cache", "rust_state",
    ];

    // Collect all raw texts per layer
    let mut layer_texts: Vec<(&str, Vec<String>)> = Vec::new();
    for &name in &layer_files {
        let path = Path::new(LAYERS_DIR).join(format!("{}.json", name));
        let texts = extract_texts_from_file(&path);
        println!("  {} {}: {} items", ">".green(), name, texts.len());
        layer_texts.push((name, texts));
    }

    let mut blocks: Vec<RawBlock> = Vec::new();

    // ═══ DEPTH 0: Identity ═══
    let identity = "Claude Memory: 8 reteg. Mate Robert (Silent) gepe. Ora = AI partner (Rust). Hullam-rezonancia, erzelmi frekvencia, kriogenikus rendszer.";
    blocks.push(RawBlock {
        data: to_block(identity),
        depth: 0, x: 0.25, y: 0.25, z: 0.25,
        layer_id: 0, parent_idx: u32::MAX, child_count: layer_files.len() as u16,
    });

    // ═══ DEPTH 1: Layer summaries ═══
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
            child_count: ((texts.len() + 4) / 5) as u16,  // cluster count
        });
    }

    // ═══ DEPTH 2: Clusters (5 items each) ═══
    let depth2_start = blocks.len();
    let mut depth2_layer_offsets: Vec<(usize, usize)> = Vec::new(); // (start_in_blocks, count)
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

    // ═══ DEPTH 3: Individual items ═══
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
                child_count: 0,  // will update
            });
            depth3_positions.push((x, y, z));
        }
    }

    // ═══ DEPTH 4: Sentences ═══
    let depth4_start = blocks.len();
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

    // ═══ DEPTH 5: Tokens (words) ═══
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

    // ═══ DEPTH 6: Syllables / morphemes (sub-word) ═══
    let mut depth6_parents: Vec<usize> = Vec::new();
    for &d5i in &depth5_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d5i].data).to_string();
        let px = blocks[d5i].x;
        let py = blocks[d5i].y;
        let pz = blocks[d5i].z;
        let lid = blocks[d5i].layer_id;

        // Split into ~3 char syllable-like chunks
        let chars: Vec<char> = text_owned.chars().collect();
        if chars.len() < 3 { continue; }
        let chunk_size = 3.max(chars.len() / 3).min(5);
        let mut child_count = 0u16;
        for (ci, chunk) in chars.chunks(chunk_size).enumerate() {
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

    // ═══ DEPTH 7: Characters ═══
    let mut depth7_parents: Vec<usize> = Vec::new();
    for &d6i in &depth6_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d6i].data).to_string();
        let px = blocks[d6i].x;
        let py = blocks[d6i].y;
        let pz = blocks[d6i].z;
        let lid = blocks[d6i].layer_id;

        let mut child_count = 0u16;
        for (ci, ch) in text_owned.chars().enumerate() {
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

    // ═══ DEPTH 8: Raw bytes — the atomic level. Below this, data corrupts. ═══
    for &d7i in &depth7_parents {
        let text_owned = String::from_utf8_lossy(&blocks[d7i].data).to_string();
        let px = blocks[d7i].x;
        let py = blocks[d7i].y;
        let pz = blocks[d7i].z;
        let lid = blocks[d7i].layer_id;

        let bytes = text_owned.as_bytes();
        let mut child_count = 0u16;
        for (bi, &byte) in bytes.iter().enumerate() {
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
                child_count: 0,  // LEAF. Below = corruption.
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

    // headers + data
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

        // Track depth ranges
        if b.depth != cur_depth {
            depth_ranges[cur_depth as usize] = (range_start, new_i as u32 - range_start);
            range_start = new_i as u32;
            cur_depth = b.depth;
        }
    }
    depth_ranges[cur_depth as usize] = (range_start, n as u32 - range_start);
    hdr_buf.flush().unwrap();

    // data.bin
    fs::write(DAT_PATH, &dat_buf).expect("write data");

    // meta.bin — pure binary, no JSON
    let mut meta_buf = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE);
    meta_buf.extend_from_slice(b"MSCM");                           // magic
    meta_buf.extend_from_slice(&1u32.to_le_bytes());               // version
    meta_buf.extend_from_slice(&(n as u32).to_le_bytes());         // block_count
    meta_buf.extend_from_slice(&9u32.to_le_bytes());               // depth_count
    for &(start, count) in &depth_ranges {
        meta_buf.extend_from_slice(&start.to_le_bytes());
        meta_buf.extend_from_slice(&count.to_le_bytes());
    }
    fs::write(META_PATH, &meta_buf).expect("write meta");

    // Report
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

    for (d, &(start, count)) in depth_ranges.iter().enumerate() {
        println!("  Depth {}: {:>5} blocks", d, count);
    }
    println!("\n{}", "ZERO JSON. Pure binary. Done.".green().bold());
}

// ─── MMAP READER ─────────────────────────────────────
struct MicroscopeReader {
    headers: memmap2::Mmap,
    data: memmap2::Mmap,
    block_count: usize,
    depth_ranges: [(u32, u32); 9],
}

impl MicroscopeReader {
    fn open() -> Self {
        // Read meta.bin — 64 bytes, pure binary
        let meta = fs::read(META_PATH).expect("open meta.bin — run 'build' first");
        assert!(&meta[0..4] == b"MSCM", "invalid magic");
        let block_count = u32::from_le_bytes(meta[8..12].try_into().unwrap()) as usize;
        let mut depth_ranges = [(0u32, 0u32); 9];
        for d in 0..9 {
            let off = META_HEADER_SIZE + d * DEPTH_ENTRY_SIZE;
            let start = u32::from_le_bytes(meta[off..off+4].try_into().unwrap());
            let count = u32::from_le_bytes(meta[off+4..off+8].try_into().unwrap());
            depth_ranges[d] = (start, count);
        }

        let hdr_file = fs::File::open(HDR_PATH).expect("open headers");
        let dat_file = fs::File::open(DAT_PATH).expect("open data");
        let headers = unsafe { memmap2::Mmap::map(&hdr_file).expect("mmap headers") };
        let data = unsafe { memmap2::Mmap::map(&dat_file).expect("mmap data") };

        MicroscopeReader { headers, data, block_count, depth_ranges }
    }

    #[inline(always)]
    fn header(&self, i: usize) -> &BlockHeader {
        debug_assert!(i < self.block_count);
        unsafe { &*(self.headers.as_ptr().add(i * HEADER_SIZE) as *const BlockHeader) }
    }

    #[inline(always)]
    fn text(&self, i: usize) -> &str {
        let h = self.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        std::str::from_utf8(&self.data[start..end]).unwrap_or("<bin>")
    }

    /// The MICROSCOPE: exact depth + spatial L2
    fn look(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<(f32, usize)> {
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

    /// 4D soft zoom
    fn look_soft(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize, zw: f32) -> Vec<(f32, usize)> {
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

    /// Text search
    fn find_text(&self, query: &str, k: usize) -> Vec<(u8, usize)> {
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

    fn print_result(&self, i: usize, dist: f32) {
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

// ─── BENCH ───────────────────────────────────────────
fn bench(reader: &MicroscopeReader) {
    println!("{}", "Benchmark: 10,000 queries per zoom level".cyan());
    println!("{}", "-".repeat(60));

    let mut rng: u64 = 42;
    let mut next_f32 = || -> f32 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 33) as f32 / (u32::MAX as f32) * 0.5
    };

    let iters = 10_000u64;
    let mut total_ns: u64 = 0;

    for zoom in 0..9u8 {
        let t0 = Instant::now();
        for _ in 0..iters {
            let r = reader.look(next_f32(), next_f32(), next_f32(), zoom, 5);
            std::hint::black_box(&r);
        }
        let ns = t0.elapsed().as_nanos() as u64;
        total_ns += ns;
        let avg = ns / iters;
        let (_s, c) = reader.depth_ranges[zoom as usize];
        let label = if avg < 1000 { format!("{} ns", avg) }
                    else { format!("{:.1} us", avg as f64 / 1000.0) };
        println!("  ZOOM {}: {} / query  ({} blocks)", zoom, label.yellow(), c);
    }

    println!("\n  {}: {:.0} ns avg",
        "OVERALL".green().bold(), total_ns as f64 / (iters * 9) as f64);

    // 4D
    println!("\n{}", "4D soft zoom (all blocks):".cyan());
    let t0 = Instant::now();
    for _ in 0..iters {
        let z = (next_f32() * 10.0) as u8 % 6;
        let r = reader.look_soft(next_f32(), next_f32(), next_f32(), z, 5, 2.0);
        std::hint::black_box(&r);
    }
    let ns = t0.elapsed().as_nanos() / iters as u128;
    println!("  4D: {} ns/query ({} blocks)", ns, reader.block_count);
}

// ─── STATS ───────────────────────────────────────────
fn stats(reader: &MicroscopeReader) {
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
    println!("{}", "=".repeat(50));
}

// ─── APPEND LOG (for store without rebuild) ──────────
const APPEND_PATH: &str = r"D:\Claude Memory\microscope\append.bin";

// Append format: [u32 text_len][u8 layer_id][f32 x][f32 y][f32 z][text bytes]
// = 17 byte header + text

fn store_memory(text: &str, layer: &str, importance: u8) {
    let t0 = Instant::now();
    let (x, y, z) = content_coords(text, layer);
    let lid = layer_to_id(layer);

    // Write to append log
    let mut file = fs::OpenOptions::new()
        .create(true).append(true)
        .open(APPEND_PATH).expect("open append log");

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    // Binary record: len(u32) + layer(u8) + importance(u8) + x(f32) + y(f32) + z(f32) + text
    file.write_all(&(len as u32).to_le_bytes()).unwrap();
    file.write_all(&[lid]).unwrap();
    file.write_all(&[importance]).unwrap();
    file.write_all(&x.to_le_bytes()).unwrap();
    file.write_all(&y.to_le_bytes()).unwrap();
    file.write_all(&z.to_le_bytes()).unwrap();
    file.write_all(&text_bytes[..len]).unwrap();

    let elapsed = t0.elapsed();
    println!("  {} [{}/{}] ({:.3},{:.3},{:.3}) {}",
        "STORED".green().bold(), layer, layer_color(lid),
        x, y, z, safe_truncate(text, 60));
    println!("  {} ns", elapsed.as_nanos());
}

// Read append log entries
struct AppendEntry {
    text: String,
    layer_id: u8,
    importance: u8,
    x: f32, y: f32, z: f32,
}

fn read_append_log() -> Vec<AppendEntry> {
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

// ─── AUTO ZOOM: query → zoom level ──────────────────
fn auto_zoom(query: &str) -> (u8, u8) {
    // Returns (center_zoom, radius) — search center ± radius
    let words = query.split_whitespace().count();
    let len = query.len();
    let has_question = query.contains('?');

    // Single word or very short → broad (identity/summary)
    if words <= 2 && len < 15 {
        return (1, 1);  // search D0-D2
    }
    // Short question → topic level
    if words <= 5 {
        return (2, 1);  // search D1-D3
    }
    // Medium → individual memories
    if words <= 10 {
        return (3, 1);  // search D2-D4
    }
    // Long/specific → sentence level
    if words <= 20 {
        return (4, 1);  // search D3-D5
    }
    // Very specific → token level
    (5, 1)  // search D4-D6
}

// ─── RECALL: natural language query ──────────────────
fn recall(query: &str, k: usize) {
    let t0 = Instant::now();
    let (center_zoom, radius) = auto_zoom(query);
    let (qx, qy, qz) = content_coords(query, "query");

    println!("{} '{}' -> auto-zoom={} (D{}..D{})",
        "RECALL".cyan().bold(), safe_truncate(query, 50),
        center_zoom,
        center_zoom.saturating_sub(radius),
        (center_zoom + radius).min(8));

    // Search main index
    let reader = MicroscopeReader::open();
    let mut all_results: Vec<(f32, usize, bool)> = Vec::new(); // (dist, idx, is_main)

    let zoom_lo = center_zoom.saturating_sub(radius);
    let zoom_hi = (center_zoom + radius).min(8);

    for zoom in zoom_lo..=zoom_hi {
        let results = reader.look(qx, qy, qz, zoom, k * 2);
        for (dist, idx) in results {
            all_results.push((dist, idx, true));
        }
    }

    // Also text-match across all zooms (hybrid: vector + text)
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
                // Boost: subtract from distance based on keyword matches
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

    // Search append log too
    let appended = read_append_log();
    for (ai, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx*dx + dy*dy + dz*dz;
        let text_lower = entry.text.to_lowercase();
        let keyword_hits = keywords.iter().filter(|&&kw| text_lower.contains(kw)).count();
        let boost = keyword_hits as f32 * 0.1;
        // For append entries, we'll print them inline
        if dist < 0.1 || keyword_hits > 0 {
            all_results.push(((dist - boost).max(0.0), ai + 1_000_000, false));
        }
    }

    // Deduplicate by index, keep best distance
    let mut seen = std::collections::HashSet::new();
    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut shown = 0;

    for (dist, idx, is_main) in &all_results {
        if shown >= k { break; }
        if !seen.insert((*idx, *is_main)) { continue; }

        if *is_main {
            reader.print_result(*idx, *dist);
        } else {
            // Append entry
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

// ─── SEMANTIC SEARCH with embeddings ─────────────────
fn semantic_search(query: &str, k: usize, metric: &str) {
    use embeddings::{MockEmbeddingProvider, EmbeddingProvider, cosine_similarity_simd};

    let t0 = Instant::now();
    println!("{} '{}' using {} metric",
        "SEMANTIC SEARCH".cyan().bold(),
        safe_truncate(query, 50),
        metric.green());

    // Initialize embedding provider (mock for now)
    let provider = MockEmbeddingProvider::new(128);

    // Get query embedding
    let query_embedding = match provider.embed(query) {
        Ok(e) => e,
        Err(_) => {
            println!("  {} Failed to generate embedding", "ERROR:".red());
            return;
        }
    };

    let reader = MicroscopeReader::open();
    let mut results: Vec<(f32, usize)> = Vec::new();

    // For each block, compute embedding similarity
    for i in 0..reader.block_count {
        let text = reader.text(i);
        if let Ok(block_embedding) = provider.embed(text) {
            let similarity = match metric {
                "cosine" => cosine_similarity_simd(&query_embedding, &block_embedding),
                "dot" => query_embedding.iter().zip(block_embedding.iter())
                    .map(|(a, b)| a * b).sum(),
                "l2" => {
                    let dist: f32 = query_embedding.iter().zip(block_embedding.iter())
                        .map(|(a, b)| (a - b).powi(2)).sum::<f32>().sqrt();
                    1.0 / (1.0 + dist) // Convert distance to similarity
                },
                _ => cosine_similarity_simd(&query_embedding, &block_embedding),
            };

            // Only keep high similarity results
            if similarity > 0.5 {
                results.push((similarity, i));
            }
        }
    }

    // Sort by similarity (descending)
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    results.truncate(k);

    // Display results
    println!("\n  {} {} results:", "Found".green(), results.len());
    for (sim, idx) in results {
        let h = reader.header(idx);
        let text = reader.text(idx);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        let preview: String = text.chars().take(70).filter(|&c| c != '\n').collect();

        println!("  {} {} {} {}",
            format!("D{}", h.depth).cyan(),
            format!("Sim={:.3}", sim).yellow(),
            format!("[{}/{}]", layer, layer_color(h.layer_id)).green(),
            preview);
    }

    let elapsed = t0.elapsed();
    println!("\n  Semantic search completed in {:.1} ms", elapsed.as_micros() as f64 / 1000.0);
}

// ─── CLI ─────────────────────────────────────────────
#[derive(Parser)]
#[command(name = "microscope-mem", about = "Zoom-based hierarchical memory — pure binary, zero JSON")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build binary from raw layer files
    Build,
    /// Store a new memory
    Store {
        text: String,
        #[arg(short, long, default_value = "long_term")]
        layer: String,
        #[arg(short = 'i', long, default_value = "5")]
        importance: u8,
    },
    /// Recall — natural language query, auto-zoom
    Recall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Manual look: x y z zoom [k]
    Look { x: f32, y: f32, z: f32, zoom: u8, #[arg(default_value = "10")] k: usize },
    /// 4D soft zoom: x y z zoom [k]
    Soft { x: f32, y: f32, z: f32, zoom: u8, #[arg(default_value = "10")] k: usize },
    /// Benchmark
    Bench,
    /// Stats
    Stats,
    /// Text search
    Find { query: String, #[arg(default_value = "5")] k: usize },
    /// Rebuild — incorporate append log into main index
    Rebuild,
    /// Semantic search using embeddings
    Embed {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
        #[arg(short, long, default_value = "cosine")]
        metric: String, // cosine, l2, dot
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Build => build(),
        Cmd::Store { text, layer, importance } => {
            store_memory(&text, &layer, importance);
        }
        Cmd::Recall { query, k } => {
            recall(&query, k);
        }
        Cmd::Look { x, y, z, zoom, k } => {
            let r = MicroscopeReader::open();
            println!("{} ({:.2},{:.2},{:.2}) zoom={}:", "MICROSCOPE".cyan().bold(), x, y, z, zoom);
            for (d, i) in r.look(x, y, z, zoom, k) { r.print_result(i, d); }
        }
        Cmd::Soft { x, y, z, zoom, k } => {
            let r = MicroscopeReader::open();
            println!("{} 4D ({:.2},{:.2},{:.2}) z={}:", "MICROSCOPE".cyan().bold(), x, y, z, zoom);
            for (d, i) in r.look_soft(x, y, z, zoom, k, 2.0) { r.print_result(i, d); }
        }
        Cmd::Bench => bench(&MicroscopeReader::open()),
        Cmd::Stats => {
            let r = MicroscopeReader::open();
            stats(&r);
            let appended = read_append_log();
            if !appended.is_empty() {
                println!("  {}: {} entries (pending rebuild)",
                    "Append log".yellow(), appended.len());
            }
        }
        Cmd::Find { query, k } => {
            let r = MicroscopeReader::open();
            println!("{} '{}':", "FIND".cyan().bold(), query);
            let res = r.find_text(&query, k);
            if res.is_empty() { println!("  (none)"); }
            for (_d, i) in res { r.print_result(i, 0.0); }
        }
        Cmd::Rebuild => {
            println!("{}", "Rebuilding with append log...".cyan());
            // TODO: merge append.bin entries into layer files, then rebuild
            // For now: just rebuild from layers
            build();
            // Clear append log
            let _ = fs::remove_file(APPEND_PATH);
            println!("  Append log cleared.");
        }
        Cmd::Embed { query, k, metric } => {
            semantic_search(&query, k, &metric);
        }
    }
}
