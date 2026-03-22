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
//!   microscope-mem serve                    # Start the unified endpoint server (TCP/HTTP)

mod embeddings;
mod embedding_index;
mod merkle;
mod streaming;
mod query;
mod snapshot;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "gpu")]
mod gpu;
mod config;

use config::Config;

use std::fs;
use std::io::{Write, Seek, BufWriter, BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use clap::{Parser, Subcommand};
use colored::Colorize;
use rayon::prelude::*;

// Defaults (will be overridden by config)
const DEFAULT_CONFIG_PATH: &str = "config.toml";
pub const BLOCK_DATA_SIZE: usize = 256;
pub const HEADER_SIZE: usize = 32;
pub const META_HEADER_SIZE: usize = 16;
pub const DEPTH_ENTRY_SIZE: usize = 8;
pub const LAYER_NAMES: &[&str] = &[
    "identity", "long_term", "short_term", "associative", "emotional",
    "relational", "reflections", "crypto_chain", "echo_cache", "rust_state",
];

/// ─── Block header: 32 bytes, packed, mmap-ready ──────
/// This structure represents a single memory block in the hierarchical index.
/// It is designed to be memory-mapped and accessed with zero-copy overhead.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct BlockHeader {
    pub(crate) x: f32,            // 4  — spatial position
    pub(crate) y: f32,            // 4
    pub(crate) z: f32,            // 4
    pub(crate) zoom: f32,         // 4  — depth / 8.0 (normalized)
    pub(crate) depth: u8,         // 1  — 0..8
    pub(crate) layer_id: u8,      // 1  — which memory layer
    pub(crate) data_offset: u32,  // 4  — byte offset into data.bin
    pub(crate) data_len: u16,     // 2  — actual text bytes (<= 256)
    pub(crate) parent_idx: u32,   // 4  — parent block index (u32::MAX = root)
    pub(crate) child_count: u16,  // 2  — number of children
    pub(crate) crc16: [u8; 2],    // 2  — CRC16-CCITT (0x0000 = no checksum, backward compat)
}


// ─── Meta header: 48 bytes at start of meta.bin ──────
#[repr(C, packed)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct MetaHeader {
    magic: [u8; 4],       // "MSCM"
    version: u32,         // 1
    block_count: u32,     // total blocks
    depth_count: u32,     // 6
    // depth_ranges: 6 x (start: u32, count: u32) = 48 bytes follow
}



fn layer_color(id: u8) -> &'static str {
    match id {
        0 => "white", 1 => "blue", 2 => "cyan", 3 => "green", 4 => "red",
        5 => "yellow", 6 => "magenta", 7 => "orange", 8 => "lime", 9 => "purple",
        _ => "white",
    }
}

pub fn layer_to_id(name: &str) -> u8 {
    LAYER_NAMES.iter().position(|&n| n == name).unwrap_or(0) as u8
}

// ─── CRC16-CCITT checksum ─────────────────────────────
/// CRC16-CCITT (poly=0x1021, init=0xFFFF) over arbitrary data.
/// Used for block-level corruption detection in the binary index.
fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

// ─── Deterministic coords from content hash ──────────
pub fn content_coords(text: &str, layer: &str) -> (f32, f32, f32) {
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

// ─── Semantic coords from mock embedding ────────────
/// Extract pseudo-semantic coordinates from mock embedding (first 3 dims → [0,1]).
/// Returns None if weight is 0 (disabled) to skip embedding computation.
fn semantic_coords(text: &str, weight: f32) -> Option<(f32, f32, f32)> {
    if weight <= 0.0 { return None; }
    use embeddings::{MockEmbeddingProvider, EmbeddingProvider};
    let provider = MockEmbeddingProvider::new(128);
    if let Ok(emb) = provider.embed(text) {
        if emb.len() >= 3 {
            // Normalize from [-1,1] to [0,1]
            let sx = (emb[0] + 1.0) / 2.0;
            let sy = (emb[1] + 1.0) / 2.0;
            let sz = (emb[2] + 1.0) / 2.0;
            return Some((sx, sy, sz));
        }
    }
    None
}

// ─── Blended coords: hash + semantic ────────────────
/// Blends deterministic hash coords with embedding-based semantic coords.
/// weight=0.0 → pure hash (backward compatible), weight=1.0 → pure semantic.
pub fn content_coords_blended(text: &str, layer: &str, weight: f32) -> (f32, f32, f32) {
    let (hx, hy, hz) = content_coords(text, layer);
    if weight <= 0.0 { return (hx, hy, hz); }
    match semantic_coords(text, weight) {
        Some((sx, sy, sz)) => {
            let w = weight.clamp(0.0, 1.0);
            (
                (1.0 - w) * hx + w * sx,
                (1.0 - w) * hy + w * sy,
                (1.0 - w) * hz + w * sz,
            )
        }
        None => (hx, hy, hz),
    }
}

// ─── Hex string helper ──────────────────────────────
fn hex_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("")
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
    let _current_text = String::new();
    let _in_content = false;

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
fn build(config: &Config) {
    println!("{}", "Building microscope from raw layers (zero JSON)...".cyan().bold());

    let layers_dir = Path::new(&config.paths.layers_dir);
    let output_dir = Path::new(&config.paths.output_dir);
    
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).expect("create output dir");
    }

    let layer_files = &config.memory_layers.layers;

    // Collect all raw texts per layer
    let mut layer_texts: Vec<(String, Vec<String>)> = Vec::new();
    for name in layer_files {
        let path = layers_dir.join(format!("{}.json", name));
        let texts = extract_texts_from_file(&path);
        println!("  {} {}: {} items", ">".green(), name, texts.len());
        layer_texts.push((name.clone(), texts));
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
    let sw = config.search.semantic_weight;
    let depth1_start = blocks.len();
    for (name, texts) in &layer_texts {
        let preview: Vec<String> = texts.iter().take(3).map(|s| safe_truncate(s, 40)).collect();
        let summary = format!("[{}] {} elem. {}", name, texts.len(), preview.join(" | "));
        let (x, y, z) = content_coords_blended(name, name, sw);
        blocks.push(RawBlock {
            data: to_block(&summary),
            depth: 1, x, y, z,
            layer_id: layer_to_id(name),
            parent_idx: 0,
            child_count: texts.len().div_ceil(5) as u16,  // cluster count
        });
    }

    // ═══ DEPTH 2: Clusters (5 items each) ═══
    let _depth2_start = blocks.len();
    let mut depth2_layer_offsets: Vec<(usize, usize)> = Vec::new(); // (start_in_blocks, count)
    for (li, (name, texts)) in layer_texts.iter().enumerate() {
        let cluster_start = blocks.len();
        for ci in (0..texts.len()).step_by(5) {
            let chunk: Vec<String> = texts[ci..texts.len().min(ci + 5)]
                .iter().map(|s| safe_truncate(s, 40)).collect();
            let summary = format!("[{} #{}] {}", name, ci / 5, chunk.join(" | "));
            let (x, y, z) = content_coords_blended(&summary, name, sw);
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
            let (x, y, z) = content_coords_blended(text, name, sw);
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
    let _depth4_start = blocks.len();
    let mut depth4_parents: Vec<usize> = Vec::new();
    
    let d4_results: Vec<Vec<RawBlock>> = (depth3_start..(depth3_start + depth3_positions.len()))
        .into_par_iter()
        .map(|d3i| {
            let text = std::str::from_utf8(&blocks[d3i].data).unwrap_or("");
            let sentences = split_sentences(text);
            let mut local_blocks = Vec::new();
            for sent in &sentences {
                if sent.len() < 10 { continue; }
                let (px, py, pz) = depth3_positions[d3i - depth3_start];
                let h = sent.as_bytes().iter().fold(0u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
                let ox = ((h & 0xFF) as f32 - 128.0) / 25500.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 25500.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 25500.0;

                local_blocks.push(RawBlock {
                    data: to_block(sent),
                    depth: 4, x: px + ox, y: py + oy, z: pz + oz,
                    layer_id: blocks[d3i].layer_id,
                    parent_idx: d3i as u32,
                    child_count: 0,
                });
            }
            local_blocks
        })
        .collect();

    for (i, local) in d4_results.into_iter().enumerate() {
        let d3i = depth3_start + i;
        blocks[d3i].child_count = local.len() as u16;
        for b in local {
            blocks.push(b);
            depth4_parents.push(blocks.len() - 1);
        }
    }

    // ═══ DEPTH 5: Tokens (words) ═══
    let mut depth5_parents: Vec<usize> = Vec::new();
    let depth4_parents_clone = depth4_parents.clone();
    let d5_results: Vec<Vec<RawBlock>> = depth4_parents
        .into_par_iter()
        .map(|d4i| {
            let text_owned = String::from_utf8_lossy(&blocks[d4i].data).to_string();
            let px = blocks[d4i].x;
            let py = blocks[d4i].y;
            let pz = blocks[d4i].z;
            let lid = blocks[d4i].layer_id;

            let tokens: Vec<String> = text_owned.split_whitespace().take(8).map(|s| s.to_string()).collect();
            let mut local_blocks = Vec::new();
            for tok in &tokens {
                if tok.len() < 2 { continue; }
                let h = tok.as_bytes().iter().fold(0u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
                let ox = ((h & 0xFF) as f32 - 128.0) / 255000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 255000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 255000.0;

                local_blocks.push(RawBlock {
                    data: to_block(tok),
                    depth: 5, x: px + ox, y: py + oy, z: pz + oz,
                    layer_id: lid,
                    parent_idx: d4i as u32,
                    child_count: 0,
                });
            }
            local_blocks
        })
        .collect();

    for (i, local) in d5_results.into_iter().enumerate() {
        let d4i = depth4_parents_clone[i];
        blocks[d4i].child_count = local.len() as u16;
        for b in local {
            blocks.push(b);
            depth5_parents.push(blocks.len() - 1);
        }
    }

    // ═══ DEPTH 6: Syllables / morphemes (sub-word) ═══
    let mut depth6_parents: Vec<usize> = Vec::new();
    let d6_results: Vec<Vec<RawBlock>> = depth5_parents
        .clone()
        .into_par_iter()
        .map(|d5i| {
            let text_owned = String::from_utf8_lossy(&blocks[d5i].data).to_string();
            let px = blocks[d5i].x;
            let py = blocks[d5i].y;
            let pz = blocks[d5i].z;
            let lid = blocks[d5i].layer_id;

            let chars: Vec<char> = text_owned.chars().collect();
            if chars.len() < 3 { return vec![]; }
            let chunk_size = 3.max(chars.len() / 3).min(5);
            let mut local_blocks = Vec::new();
            for chunk in chars.chunks(chunk_size) {
                let syl: String = chunk.iter().collect();
                if syl.trim().is_empty() { continue; }
                let h = syl.as_bytes().iter().fold(0u64, |a, &b| a.wrapping_mul(37).wrapping_add(b as u64));
                let ox = ((h & 0xFF) as f32 - 128.0) / 2550000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 2550000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 2550000.0;

                local_blocks.push(RawBlock {
                    data: to_block(&syl),
                    depth: 6, x: px + ox, y: py + oy, z: pz + oz,
                    layer_id: lid,
                    parent_idx: d5i as u32,
                    child_count: 0,
                });
            }
            local_blocks
        })
        .collect();

    for (i, local) in d6_results.into_iter().enumerate() {
        let d5i = depth5_parents[i];
        blocks[d5i].child_count = local.len() as u16;
        for b in local {
            blocks.push(b);
            depth6_parents.push(blocks.len() - 1);
        }
    }

    // ═══ DEPTH 7: Characters ═══
    let mut depth7_parents: Vec<usize> = Vec::new();
    let d7_results: Vec<Vec<RawBlock>> = depth6_parents
        .clone()
        .into_par_iter()
        .map(|d6i| {
            let text_owned = String::from_utf8_lossy(&blocks[d6i].data).to_string();
            let px = blocks[d6i].x;
            let py = blocks[d6i].y;
            let pz = blocks[d6i].z;
            let lid = blocks[d6i].layer_id;

            let mut local_blocks = Vec::new();
            for ch in text_owned.chars() {
                if ch.is_whitespace() { continue; }
                let h = (ch as u64).wrapping_mul(0x517cc1b727220a95);
                let ox = ((h & 0xFF) as f32 - 128.0) / 25500000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 25500000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 25500000.0;

                let ch_str = ch.to_string();
                local_blocks.push(RawBlock {
                    data: to_block(&ch_str),
                    depth: 7, x: px + ox, y: py + oy, z: pz + oz,
                    layer_id: lid,
                    parent_idx: d6i as u32,
                    child_count: 0,
                });
            }
            local_blocks
        })
        .collect();

    for (i, local) in d7_results.into_iter().enumerate() {
        let d6i = depth6_parents[i];
        blocks[d6i].child_count = local.len() as u16;
        for b in local {
            blocks.push(b);
            depth7_parents.push(blocks.len() - 1);
        }
    }

    // ═══ DEPTH 8: Raw bytes — the atomic level. Below this, data corrupts. ═══
    let d8_results: Vec<Vec<RawBlock>> = depth7_parents
        .clone()
        .into_par_iter()
        .map(|d7i| {
            let text_owned = String::from_utf8_lossy(&blocks[d7i].data).to_string();
            let px = blocks[d7i].x;
            let py = blocks[d7i].y;
            let pz = blocks[d7i].z;
            let lid = blocks[d7i].layer_id;

            let bytes = text_owned.as_bytes();
            let mut local_blocks = Vec::new();
            for &byte in bytes {
                let hex = format!("0x{:02X}", byte);
                let h = (byte as u64).wrapping_mul(0x9E3779B97F4A7C15);
                let ox = ((h & 0xFF) as f32 - 128.0) / 255000000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 255000000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 255000000.0;

                local_blocks.push(RawBlock {
                    data: to_block(&hex),
                    depth: 8, x: px + ox, y: py + oy, z: pz + oz,
                    layer_id: lid,
                    parent_idx: d7i as u32,
                    child_count: 0,  // LEAF. Below = corruption.
                });
            }
            local_blocks
        })
        .collect();

    for (i, local) in d8_results.into_iter().enumerate() {
        let d7i = depth7_parents[i];
        blocks[d7i].child_count = local.len() as u16;
        for b in local {
            blocks.push(b);
        }
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
    let output_dir = Path::new(&config.paths.output_dir);
    fs::create_dir_all(output_dir).ok();

    let hdr_path = output_dir.join("microscope.bin");
    let dat_path = output_dir.join("data.bin");
    let meta_path = output_dir.join("meta.bin");

    let mut hdr_file = BufWriter::new(fs::File::create(&hdr_path).expect("create headers"));
    let mut dat_file = BufWriter::new(fs::File::create(&dat_path).expect("create data"));

    let mut depth_ranges: Vec<(u32, u32)> = vec![(0, 0); 9];
    let mut cur_depth: u8 = 0;
    let mut range_start: u32 = 0;

    for (new_i, &old_i) in indices.iter().enumerate() {
        let b = &blocks[old_i];
        let offset = dat_file.stream_position().unwrap() as u32; // Get current write position
        let len = b.data.len().min(BLOCK_DATA_SIZE) as u16;
        dat_file.write_all(&b.data[..len as usize]).unwrap();

        let parent = if b.parent_idx == u32::MAX { u32::MAX } else { old_to_new[b.parent_idx as usize] };

        let crc = crc16_ccitt(&b.data[..len as usize]);
        let hdr = BlockHeader {
            x: b.x, y: b.y, z: b.z,
            zoom: b.depth as f32 / 8.0,
            depth: b.depth,
            layer_id: b.layer_id,
            data_offset: offset,
            data_len: len,
            parent_idx: parent,
            child_count: b.child_count,
            crc16: crc.to_le_bytes(),
        };

        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(&hdr as *const BlockHeader as *const u8, HEADER_SIZE)
        };
        hdr_file.write_all(bytes).expect("write hdr");

        // Track depth ranges
        if b.depth != cur_depth {
            depth_ranges[cur_depth as usize] = (range_start, new_i as u32 - range_start);
            range_start = new_i as u32;
            cur_depth = b.depth;
        }
    }
    depth_ranges[cur_depth as usize] = (range_start, n as u32 - range_start);
    hdr_file.flush().unwrap();
    dat_file.flush().unwrap();

    // ═══ Merkle tree: SHA-256 over all block data ═══
    let merkle_path = output_dir.join("merkle.bin");
    // Re-read data.bin to get all block data slices for Merkle leaves
    hdr_file.flush().unwrap();
    dat_file.flush().unwrap();

    let dat_bytes = fs::read(&dat_path).expect("read data.bin for merkle");
    let hdr_bytes = fs::read(&hdr_path).expect("read microscope.bin for merkle");
    let mut leaf_slices: Vec<&[u8]> = Vec::with_capacity(n);
    for i in 0..n {
        let hdr_off = i * HEADER_SIZE;
        let data_offset = u32::from_le_bytes(hdr_bytes[hdr_off + 16..hdr_off + 20].try_into().unwrap()) as usize;
        let data_len = u16::from_le_bytes(hdr_bytes[hdr_off + 20..hdr_off + 22].try_into().unwrap()) as usize;
        if data_offset + data_len <= dat_bytes.len() {
            leaf_slices.push(&dat_bytes[data_offset..data_offset + data_len]);
        } else {
            leaf_slices.push(&[]);
        }
    }

    let merkle_tree = merkle::MerkleTree::build(&leaf_slices);
    fs::write(&merkle_path, merkle_tree.to_bytes()).expect("write merkle.bin");
    println!("  {}: {} leaves, root={}",
        "merkle".green(), merkle_tree.leaf_count,
        hex_str(&merkle_tree.root));

    // meta.bin — MSC2 format with merkle root
    let mut meta_buf = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE + 32);
    meta_buf.extend_from_slice(b"MSC2");                           // magic v2
    meta_buf.extend_from_slice(&2u32.to_le_bytes());               // version
    meta_buf.extend_from_slice(&(n as u32).to_le_bytes());         // block_count
    meta_buf.extend_from_slice(&9u32.to_le_bytes());               // depth_count
    for &(start, count) in &depth_ranges {
        meta_buf.extend_from_slice(&start.to_le_bytes());
        meta_buf.extend_from_slice(&count.to_le_bytes());
    }
    meta_buf.extend_from_slice(&merkle_tree.root);                 // 32 bytes merkle root
    fs::write(meta_path, &meta_buf).expect("write meta");

    // Report
    let hdr_size = n * HEADER_SIZE;
    let dat_size = dat_file.stream_position().unwrap() as usize; // Get final data size
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

    // ═══ Embedding index (mock provider, or candle if enabled) ═══
    if config.embedding.provider != "none" {
        println!("\n  Building embedding index...");
        let emb_path = output_dir.join("embeddings.bin");
        let reader = MicroscopeReader::open(config);
        let max_depth = config.embedding.max_depth;

        #[cfg(feature = "embeddings")]
        let provider: Box<dyn embeddings::EmbeddingProvider> = if config.embedding.provider == "candle" {
            match embeddings::CandleEmbeddingProvider::new(&config.embedding.model) {
                Ok(p) => Box::new(p),
                Err(e) => {
                    eprintln!("  {} Candle init failed: {:?}, using mock", "WARN".yellow(), e);
                    Box::new(embeddings::MockEmbeddingProvider::new(config.embedding.dim))
                }
            }
        } else {
            Box::new(embeddings::MockEmbeddingProvider::new(config.embedding.dim))
        };

        #[cfg(not(feature = "embeddings"))]
        let provider: Box<dyn embeddings::EmbeddingProvider> =
            Box::new(embeddings::MockEmbeddingProvider::new(config.embedding.dim));

        match embedding_index::build_embedding_index(&*provider, &reader, max_depth, &emb_path) {
            Ok(()) => println!("  {} embeddings.bin built", "OK".green()),
            Err(e) => eprintln!("  {} embedding build: {}", "ERR".red(), e),
        }
    }

    println!("\n{}", "ZERO JSON. Pure binary. Done.".green().bold());
}

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[inline(always)]
fn l2_dist_sq_simd(h: &BlockHeader, x: f32, y: f32, z: f32, qz: f32, zw: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // Load x, y, z, zoom (16 bytes) — use raw pointer to avoid unaligned reference
        let h_vals = _mm_loadu_ps(h as *const BlockHeader as *const f32);
        let q_vals = _mm_set_ps(qz, z, y, x);
        let diff = _mm_sub_ps(h_vals, q_vals);
        
        // Apply zoom weight to the W component (zoom)
        let weights = _mm_set_ps(zw, 1.0, 1.0, 1.0);
        let weighted_diff = _mm_mul_ps(diff, weights);
        
        let sq = _mm_mul_ps(weighted_diff, weighted_diff);
        
        // Horizontal sum
        let res = _mm_hadd_ps(sq, sq);
        let res2 = _mm_hadd_ps(res, res);
        let mut dist = 0.0f32;
        _mm_store_ss(&mut dist, res2);
        dist
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let dx = h.x - x;
        let dy = h.y - y;
        let dz = h.z - z;
        let dw = (h.zoom - qz) * zw;
        dx*dx + dy*dy + dz*dz + dw*dw
    }
}

/// High-performance memory-mapped reader for the Microscope index.
/// Handles spatial and hierarchical queries using SIMD-accelerated distance metrics.
pub struct MicroscopeReader {
    /// Mmaped headers (32 bytes per block)
    pub headers: memmap2::Mmap,
    /// Mmaped raw text data
    pub data: memmap2::Mmap,
    /// Total number of blocks in the index
    pub block_count: usize,
    /// Start index and count for each depth level (0-8)
    pub depth_ranges: [(u32, u32); 9],
}

impl MicroscopeReader {
    pub fn open(config: &Config) -> Self {
        // Paths from config
        let output_dir = Path::new(&config.paths.output_dir);
        let meta_path = output_dir.join("meta.bin");
        let hdr_path = output_dir.join("microscope.bin");
        let dat_path = output_dir.join("data.bin");

        // Read meta.bin — supports both MSCM (v1) and MSC2 (v2) formats
        let meta = fs::read(meta_path).expect("open meta.bin — run 'build' first");
        let magic = &meta[0..4];
        assert!(magic == b"MSCM" || magic == b"MSC2", "invalid magic: expected MSCM or MSC2");
        let block_count = u32::from_le_bytes(meta[8..12].try_into().unwrap()) as usize;
        let mut depth_ranges = [(0u32, 0u32); 9];
        for (d, range) in depth_ranges.iter_mut().enumerate() {
            let off = META_HEADER_SIZE + d * DEPTH_ENTRY_SIZE;
            let start = u32::from_le_bytes(meta[off..off+4].try_into().unwrap());
            let count = u32::from_le_bytes(meta[off+4..off+8].try_into().unwrap());
            *range = (start, count);
        }

        let hdr_file = fs::File::open(hdr_path).expect("open headers");
        let dat_file = fs::File::open(dat_path).expect("open data");
        let headers = unsafe { memmap2::Mmap::map(&hdr_file).expect("mmap headers") };
        let data = unsafe { memmap2::Mmap::map(&dat_file).expect("mmap data") };

        MicroscopeReader { headers, data, block_count, depth_ranges }
    }

    #[inline(always)]
    pub(crate) fn header(&self, i: usize) -> &BlockHeader {
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

    /// The MICROSCOPE: exact depth + spatial L2 search.
    /// Returns the k-nearest neighbors at a specific zoom level.
    pub fn look(&self, config: &Config, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<(f32, usize, bool)> {
        let (start, count) = self.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        
        let mut results: Vec<(f32, usize, bool)> = Vec::with_capacity(count + 10);
        if count > 0 {
            for i in start..(start + count) {
                let h = self.header(i);
                let dx = h.x - x;
                let dy = h.y - y;
                let dz = h.z - z;
                results.push((dx*dx + dy*dy + dz*dz, i, true));
            }
        }

        // Search append log (hot memory) — filter by depth
        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            if entry.depth != zoom { continue; }
            let dx = entry.x - x;
            let dy = entry.y - y;
            let dz = entry.z - z;
            results.push((dx*dx + dy*dy + dz*dz, ai + 1_000_000, false));
        }

        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    /// 4D soft zoom search.
    /// Considers zoom (normalized depth) as a weighted spatial dimension.
    /// This scans ALL blocks in the index using SIMD-accelerated L2 distance.
    #[allow(clippy::too_many_arguments)]
    fn look_soft(&self, config: &Config, x: f32, y: f32, z: f32, zoom: u8, k: usize, zw: f32) -> Vec<(f32, usize, bool)> {
        let qz = zoom as f32 / 8.0;
        let mut results: Vec<(f32, usize, bool)> = (0..self.block_count)
            .into_par_iter()
            .map(|i| {
                let h = self.header(i);
                (l2_dist_sq_simd(h, x, y, z, qz, zw), i, true)
            })
            .collect();

        // Search append log (hot memory)
        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            let dx = entry.x - x;
            let dy = entry.y - y;
            let dz = entry.z - z;
            let entry_zoom = entry.depth as f32 / 8.0;
            let dw = (entry_zoom - qz) * zw;
            results.push((dx*dx + dy*dy + dz*dz + dw*dw, ai + 1_000_000, false));
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
        let mut results: Vec<(u8, usize)> = (0..self.block_count)
            .into_par_iter()
            .filter_map(|i| {
                if self.text(i).to_lowercase().contains(&q) {
                    Some((self.header(i).depth, i))
                } else {
                    None
                }
            })
            .collect();
            
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
fn bench(config: &Config, reader: &MicroscopeReader) {
    println!("{}", "Benchmark: 10,000 queries per zoom level".cyan());
    println!("  Mode: SIMD={} Rayon=true",
        cfg!(target_arch = "x86_64"));

    let mut rng: u64 = 42;
    let mut next_f32 = || -> f32 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 33) as f32 / (u32::MAX as f32) * 0.5
    };

    let iters = 10_000u64;
    let mut total_ns: u64 = 0;

    for zoom in 0..9u8 {
        let t0 = Instant::now();
        let config_clone = config.clone();
        for _ in 0..iters {
            let r = reader.look(&config_clone, next_f32(), next_f32(), next_f32(), zoom, 5);
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
    let config_clone = config.clone();
    for _ in 0..iters {
        let z = (next_f32() * 10.0) as u8 % 6;
        let r = reader.look_soft(&config_clone, next_f32(), next_f32(), next_f32(), z, 5, 2.0);
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

// Append format: [u32 text_len][u8 layer_id][f32 x][f32 y][f32 z][text bytes]
// = 17 byte header + text

pub fn store_memory(config: &Config, text: &str, layer: &str, importance: u8) {
    let t0 = Instant::now();
    let (x, y, z) = content_coords_blended(text, layer, config.search.semantic_weight);
    let lid = layer_to_id(layer);
    let depth = auto_depth(text);

    // Write to append log
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");

    // Write APv2 magic if file is empty or doesn't exist
    let needs_magic = !append_path.exists() || fs::metadata(&append_path).map(|m| m.len() == 0).unwrap_or(true);

    let mut file = fs::OpenOptions::new()
        .create(true).append(true)
        .open(&append_path).expect("open append log");

    if needs_magic {
        file.write_all(b"APv2").unwrap();
    }

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    // APv2 record: len(u32) + layer(u8) + importance(u8) + depth(u8) + x(f32) + y(f32) + z(f32) + text
    file.write_all(&(len as u32).to_le_bytes()).unwrap();
    file.write_all(&[lid]).unwrap();
    file.write_all(&[importance]).unwrap();
    file.write_all(&[depth]).unwrap();
    file.write_all(&x.to_le_bytes()).unwrap();
    file.write_all(&y.to_le_bytes()).unwrap();
    file.write_all(&z.to_le_bytes()).unwrap();
    file.write_all(&text_bytes[..len]).unwrap();

    let elapsed = t0.elapsed();
    println!("  {} D{} [{}/{}] ({:.3},{:.3},{:.3}) {}",
        "STORED".green().bold(), depth, layer, layer_color(lid),
        x, y, z, safe_truncate(text, 60));
    println!("  {} ns", elapsed.as_nanos());
}

// Read append log entries
#[allow(dead_code)]
pub struct AppendEntry {
    pub text: String,
    pub layer_id: u8,
    pub importance: u8,
    pub depth: u8,
    pub x: f32, pub y: f32, pub z: f32,
}

pub fn read_append_log(path: &Path) -> Vec<AppendEntry> {
    if !path.exists() { return vec![]; }
    let data = fs::read(path).unwrap_or_default();
    if data.is_empty() { return vec![]; }

    let mut entries = Vec::new();
    let mut pos = 0;

    // Detect APv2 magic
    let is_v2 = data.len() >= 4 && &data[0..4] == b"APv2";
    if is_v2 { pos = 4; }

    let header_size = if is_v2 { 19 } else { 18 };

    while pos + header_size <= data.len() {
        let len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        let lid = data[pos+4];
        let imp = data[pos+5];

        let (depth, coords_start) = if is_v2 {
            (data[pos+6], pos + 7)
        } else {
            (4u8, pos + 6) // Legacy: default depth D4
        };

        let x = f32::from_le_bytes(data[coords_start..coords_start+4].try_into().unwrap());
        let y = f32::from_le_bytes(data[coords_start+4..coords_start+8].try_into().unwrap());
        let z = f32::from_le_bytes(data[coords_start+8..coords_start+12].try_into().unwrap());
        pos += header_size;
        if pos + len > data.len() { break; }
        let text = String::from_utf8_lossy(&data[pos..pos+len]).to_string();
        pos += len;
        entries.push(AppendEntry { text, layer_id: lid, importance: imp, depth, x, y, z });
    }
    entries
}

/// Display a single append-log result entry.
fn print_append_result(appended: &[AppendEntry], idx: usize, dist: f32) {
    let ai = idx - 1_000_000;
    if ai < appended.len() {
        let e = &appended[ai];
        let layer = LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
        println!("  {} {} {} {}",
            format!("D{}", e.depth).cyan(),
            format!("L2={:.5}", dist).yellow(),
            format!("[{}/new]", layer).green(),
            safe_truncate(&e.text, 70));
    }
}

// ─── AUTO ZOOM: query → zoom level ──────────────────
pub fn auto_zoom(query: &str) -> (u8, u8) {
    // Stopwords for better complexity estimation
    let stopwords = ["a", "the", "is", "of", "and", "to", "in", "it", "on", "for"];
    let unique_content_words = query.to_lowercase()
        .split_whitespace()
        .filter(|w| !stopwords.contains(w) && w.len() > 2)
        .count();

    // broad (identity/summary)
    if unique_content_words <= 1 {
        return (1, 1);  // search D0-D2
    }
    // topic level
    if unique_content_words <= 3 {
        return (2, 1);  // search D1-D3
    }
    // individual memories
    if unique_content_words <= 6 {
        return (3, 1);  // search D2-D4
    }
    // sentence level
    if unique_content_words <= 10 {
        return (4, 1);  // search D3-D5
    }
    // token level
    (5, 1)  // search D4-D6
}

// ─── AUTO DEPTH: text length → virtual depth level ──
/// Assign a virtual depth to append entries based on text length.
fn auto_depth(text: &str) -> u8 {
    let len = text.len();
    if len >= 100 { 3 }      // Items
    else if len >= 40 { 4 }  // Sentences
    else if len >= 15 { 5 }  // Tokens
    else { 6 }               // Syllables
}

// ─── RECALL: Semantic/Coord Search ──────────────────
fn recall(config: &Config, query: &str, k: usize) {
    let t0 = Instant::now();
    let reader = MicroscopeReader::open(config);
    println!("{} '{}':", "RECALL".cyan().bold(), query);

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);
    let (zoom_lo, zoom_hi) = match query.len() {
        0..=10 => (0, 3), // broad/top
        11..=40 => (3, 6), // sentences/tokens
        _ => (6, 8), // chars/bytes
    };

    let mut all_results: Vec<(f32, usize, bool)> = Vec::new(); // (dist, idx, is_main)

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
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
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
            print_append_result(&appended, *idx, *dist);
        }
        shown += 1;
    }

    let elapsed = t0.elapsed();
    println!("\n  {} results in {:.0} us", shown, elapsed.as_micros());
}

// ─── SEMANTIC SEARCH with embeddings ─────────────────
fn semantic_search(config: &Config, query: &str, k: usize, metric: &str) {
    use embeddings::{MockEmbeddingProvider, EmbeddingProvider, cosine_similarity_simd};
    use embedding_index::EmbeddingIndex;

    let t0 = Instant::now();
    println!("{} '{}' using {} metric",
        "SEMANTIC SEARCH".cyan().bold(),
        safe_truncate(query, 50),
        metric.green());

    let reader = MicroscopeReader::open(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let emb_path = output_dir.join("embeddings.bin");

    // Try to use pre-built embedding index first
    if let Some(idx) = EmbeddingIndex::open(&emb_path) {
        println!("  Using pre-built embedding index ({} blocks, {} dim)", idx.block_count(), idx.dim());

        // Create provider for query embedding
        #[cfg(feature = "embeddings")]
        let provider: Box<dyn EmbeddingProvider> = if config.embedding.provider == "candle" {
            match embeddings::CandleEmbeddingProvider::new(&config.embedding.model) {
                Ok(p) => Box::new(p),
                Err(_) => Box::new(MockEmbeddingProvider::new(idx.dim())),
            }
        } else {
            Box::new(MockEmbeddingProvider::new(idx.dim()))
        };

        #[cfg(not(feature = "embeddings"))]
        let provider: Box<dyn EmbeddingProvider> = Box::new(MockEmbeddingProvider::new(idx.dim()));

        let query_embedding = match provider.embed(query) {
            Ok(e) => e,
            Err(_) => { println!("  {} Failed to embed query", "ERROR:".red()); return; }
        };

        let results = idx.search(&query_embedding, k);
        println!("\n  {} {} results:", "Found".green(), results.len());
        for (sim, block_idx) in results {
            let h = reader.header(block_idx);
            let text = reader.text(block_idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            let preview: String = text.chars().take(70).filter(|&c| c != '\n').collect();
            println!("  {} {} {} {}",
                format!("D{}", h.depth).cyan(),
                format!("Sim={:.3}", sim).yellow(),
                format!("[{}/{}]", layer, layer_color(h.layer_id)).green(),
                preview);
        }

        let elapsed = t0.elapsed();
        println!("\n  Semantic search (indexed) in {:.1} ms", elapsed.as_micros() as f64 / 1000.0);
        return;
    }

    // Fallback: compute embeddings on-the-fly
    println!("  No embedding index — computing on-the-fly (slow)");
    let provider = MockEmbeddingProvider::new(128);

    let query_embedding = match provider.embed(query) {
        Ok(e) => e,
        Err(_) => { println!("  {} Failed to generate embedding", "ERROR:".red()); return; }
    };

    let mut results: Vec<(f32, usize)> = Vec::new();
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
                    1.0 / (1.0 + dist)
                },
                _ => cosine_similarity_simd(&query_embedding, &block_embedding),
            };
            if similarity > 0.5 {
                results.push((similarity, i));
            }
        }
    }

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    results.truncate(k);

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
    println!("\n  Semantic search (on-the-fly) in {:.1} ms", elapsed.as_micros() as f64 / 1000.0);
}

// ─── VERIFY: CRC16 integrity check ──────────────────
fn verify_integrity(config: &Config) {
    let reader = MicroscopeReader::open(config);
    println!("{} {} blocks...", "VERIFY".cyan().bold(), reader.block_count);

    let mut checked = 0u64;
    let mut skipped = 0u64;
    let mut bad = 0u64;

    for i in 0..reader.block_count {
        let h = reader.header(i);
        let stored = u16::from_le_bytes(h.crc16);
        if stored == 0x0000 {
            skipped += 1;
            continue;
        }
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        if end > reader.data.len() {
            println!("  {} Block {} offset out of bounds", "ERR".red(), i);
            bad += 1;
            continue;
        }
        let computed = crc16_ccitt(&reader.data[start..end]);
        if computed != stored {
            println!("  {} Block {} D{}: CRC mismatch (stored=0x{:04X}, computed=0x{:04X})",
                "FAIL".red().bold(), i, h.depth, stored, computed);
            bad += 1;
        } else {
            checked += 1;
        }
    }

    if bad == 0 {
        println!("  {} {} blocks verified, {} skipped (no CRC)",
            "OK".green().bold(), checked, skipped);
    } else {
        println!("  {} {} corrupted, {} ok, {} skipped",
            "FAIL".red().bold(), bad, checked, skipped);
    }
}

// ─── GPU BENCH ───────────────────────────────────────
fn gpu_bench(config: &Config) {
    let reader = MicroscopeReader::open(config);
    println!("{} {} blocks", "GPU BENCH".cyan().bold(), reader.block_count);

    // CPU baseline
    let iters = 1000u64;
    let mut rng: u64 = 42;
    let mut next_f32 = || -> f32 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 33) as f32 / (u32::MAX as f32) * 0.5
    };

    let config_clone = config.clone();
    let t0 = Instant::now();
    for _ in 0..iters {
        let z = (next_f32() * 10.0) as u8 % 6;
        let r = reader.look_soft(&config_clone, next_f32(), next_f32(), next_f32(), z, 5, config.search.zoom_weight);
        std::hint::black_box(&r);
    }
    let cpu_ns = t0.elapsed().as_nanos() / iters as u128;
    println!("  CPU: {} ns/query", cpu_ns);

    #[cfg(feature = "gpu")]
    {
        match gpu::GpuAccelerator::new(&reader) {
            Ok(accel) => {
                // Warmup
                for _ in 0..10 {
                    let z = (next_f32() * 10.0) as u8 % 6;
                    let _ = accel.l2_search_4d(next_f32(), next_f32(), next_f32(), z, config.search.zoom_weight, 5);
                }

                let t0 = Instant::now();
                for _ in 0..iters {
                    let z = (next_f32() * 10.0) as u8 % 6;
                    let r = accel.l2_search_4d(next_f32(), next_f32(), next_f32(), z, config.search.zoom_weight, 5);
                    std::hint::black_box(&r);
                }
                let gpu_ns = t0.elapsed().as_nanos() / iters as u128;
                println!("  GPU: {} ns/query", gpu_ns);

                if gpu_ns > 0 {
                    let speedup = cpu_ns as f64 / gpu_ns as f64;
                    println!("  Speedup: {:.1}x", speedup);
                }
            }
            Err(e) => {
                eprintln!("  {} GPU init failed: {}", "ERR".red(), e);
            }
        }
    }

    #[cfg(not(feature = "gpu"))]
    {
        println!("  {} GPU feature not compiled. Use: cargo build --features gpu", "WARN".yellow());
    }
}

// ─── VERIFY MERKLE ───────────────────────────────────
fn verify_merkle(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let merkle_path = output_dir.join("merkle.bin");
    let meta_path = output_dir.join("meta.bin");

    // Check if merkle.bin exists
    if !merkle_path.exists() {
        println!("  {} merkle.bin not found — rebuild with v0.2.0 to generate", "ERR".red());
        return;
    }

    // Read stored merkle root from meta.bin
    let meta = fs::read(&meta_path).expect("read meta.bin");
    let magic = &meta[0..4];
    if magic != b"MSC2" {
        println!("  {} meta.bin is v1 (MSCM) — no merkle root stored. Rebuild first.", "WARN".yellow());
        return;
    }
    let meta_root_offset = META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE;
    let mut stored_root = [0u8; 32];
    stored_root.copy_from_slice(&meta[meta_root_offset..meta_root_offset + 32]);

    // Read the stored merkle tree
    let merkle_data = fs::read(&merkle_path).expect("read merkle.bin");
    let stored_tree = merkle::MerkleTree::from_bytes(&merkle_data)
        .expect("parse merkle.bin");

    println!("{} {} blocks...", "VERIFY MERKLE".cyan().bold(), stored_tree.leaf_count);
    println!("  Stored root:   {}", hex_str(&stored_root));
    println!("  Merkle root:   {}", hex_str(&stored_tree.root));

    if stored_root != stored_tree.root {
        println!("  {} meta.bin root != merkle.bin root!", "MISMATCH".red().bold());
        return;
    }

    // Recompute from data.bin
    let reader = MicroscopeReader::open(config);
    let mut bad_blocks = Vec::new();
    for i in 0..reader.block_count {
        let h = reader.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        if end > reader.data.len() {
            bad_blocks.push(i);
            continue;
        }
        let data = &reader.data[start..end];
        if !stored_tree.verify_leaf(i, data) {
            bad_blocks.push(i);
        }
    }

    if bad_blocks.is_empty() {
        println!("  {} All {} blocks verified against Merkle root",
            "OK".green().bold(), reader.block_count);
    } else {
        println!("  {} {} block(s) failed verification:",
            "FAIL".red().bold(), bad_blocks.len());
        for &idx in bad_blocks.iter().take(20) {
            println!("    Block {}", idx);
        }
        if bad_blocks.len() > 20 {
            println!("    ... and {} more", bad_blocks.len() - 20);
        }
    }
}

// ─── MERKLE PROOF ────────────────────────────────────
fn merkle_proof(config: &Config, block_index: usize) {
    let output_dir = Path::new(&config.paths.output_dir);
    let merkle_path = output_dir.join("merkle.bin");

    if !merkle_path.exists() {
        println!("  {} merkle.bin not found — rebuild first", "ERR".red());
        return;
    }

    let merkle_data = fs::read(&merkle_path).expect("read merkle.bin");
    let tree = merkle::MerkleTree::from_bytes(&merkle_data)
        .expect("parse merkle.bin");

    if block_index >= tree.leaf_count {
        println!("  {} Block index {} out of range (max: {})",
            "ERR".red(), block_index, tree.leaf_count - 1);
        return;
    }

    let reader = MicroscopeReader::open(config);
    let h = reader.header(block_index);
    let text = reader.text(block_index);
    let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");

    println!("{} Block #{}", "MERKLE PROOF".cyan().bold(), block_index);
    println!("  D{} [{}] {}", h.depth, layer, safe_truncate(text, 60));
    println!("  Leaf hash: {}", hex_str(&tree.nodes[block_index]));

    let proof = tree.proof(block_index);
    println!("  Proof path ({} steps):", proof.len());
    for (i, (hash, is_right)) in proof.iter().enumerate() {
        let side = if *is_right { "R" } else { "L" };
        println!("    [{}] {} sibling={}", i, side, hex_str(hash));
    }

    // Verify
    let data_start = h.data_offset as usize;
    let data_end = data_start + h.data_len as usize;
    let block_data = &reader.data[data_start..data_end];
    let valid = merkle::MerkleTree::verify_proof(&tree.root, block_data, &proof);
    if valid {
        println!("  {} Proof valid against root {}",
            "VERIFIED".green().bold(), hex_str(&tree.root));
    } else {
        println!("  {} Proof INVALID", "FAIL".red().bold());
    }
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
    Soft {
        x: f32, y: f32, z: f32, zoom: u8,
        #[arg(default_value = "10")]
        k: usize,
        /// Use GPU acceleration (requires gpu feature)
        #[arg(long)]
        gpu: bool,
    },
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
    /// GPU vs CPU benchmark (requires gpu feature)
    GpuBench,
    /// Verify CRC16 integrity of all blocks
    Verify,
    /// Verify Merkle tree integrity of the entire index
    VerifyMerkle,
    /// Show Merkle proof for a specific block
    Proof {
        #[arg(help = "Block index")]
        block_index: usize,
    },
    /// Start the HTTP server
    Serve {
        #[arg(short, long, default_value_t = 6060)]
        port: u16,
    },
    /// MQL query (Microscope Query Language)
    Query {
        /// MQL expression, e.g. 'layer:long_term depth:2..5 "Ora"'
        mql: String,
    },
    /// Export index to .mscope archive
    Export {
        /// Output archive path
        output: String,
    },
    /// Import .mscope archive
    Import {
        /// Input archive path
        input: String,
        /// Output directory (defaults to config output_dir)
        #[arg(long)]
        output_dir: Option<String>,
    },
    /// Diff two .mscope archives
    Diff {
        /// First archive
        a: String,
        /// Second archive
        b: String,
    },
}

fn main() {
    let cli = Cli::parse();
    
    // Load config from default path or use default if not found
    let config = Config::load(DEFAULT_CONFIG_PATH).unwrap_or_else(|_| {
        println!("  {} Using default configuration", "WARN:".yellow());
        Config::default()
    });

    match cli.cmd {
        Cmd::Build => build(&config),
        Cmd::Store { text, layer, importance } => {
            store_memory(&config, &text, &layer, importance);
        }
        Cmd::Recall { query, k } => {
            recall(&config, &query, k);
        }
        Cmd::Look { x, y, z, zoom, k } => {
            let config_clone = config.clone();
            let r = MicroscopeReader::open(&config);
            println!("{} ({:.2},{:.2},{:.2}) zoom={}:", "MICROSCOPE".cyan().bold(), x, y, z, zoom);
            let res = r.look(&config_clone, x, y, z, zoom, k);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            for (dist, idx, is_main) in res {
                if is_main {
                    r.print_result(idx, dist);
                } else {
                    print_append_result(&appended, idx, dist);
                }
            }
        }
        Cmd::Soft { x, y, z, zoom, k, gpu: use_gpu } => {
            let r = MicroscopeReader::open(&config);
            let use_gpu = use_gpu || config.performance.use_gpu;
            println!("{} 4D ({:.2},{:.2},{:.2}) z={} {}:",
                "MICROSCOPE".cyan().bold(), x, y, z, zoom,
                if use_gpu { "[GPU]" } else { "[CPU]" });

            #[cfg(feature = "gpu")]
            if use_gpu {
                match gpu::GpuAccelerator::new(&r) {
                    Ok(accel) => {
                        let res = accel.l2_search_4d(x, y, z, zoom, config.search.zoom_weight, k);
                        for (dist, idx) in res {
                            r.print_result(idx, dist);
                        }
                        return;
                    }
                    Err(e) => {
                        eprintln!("  {} GPU init failed: {}, falling back to CPU", "WARN".yellow(), e);
                    }
                }
            }

            #[cfg(not(feature = "gpu"))]
            if use_gpu {
                eprintln!("  {} GPU feature not compiled. Use --features gpu", "WARN".yellow());
            }

            let config_clone = config.clone();
            let res = r.look_soft(&config_clone, x, y, z, zoom, k, config.search.zoom_weight);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            for (dist, idx, is_main) in res {
                if is_main {
                    r.print_result(idx, dist);
                } else {
                    print_append_result(&appended, idx, dist);
                }
            }
        }
        Cmd::Bench => bench(&config, &MicroscopeReader::open(&config)),
        Cmd::Stats => {
            let r = MicroscopeReader::open(&config);
            stats(&r);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            if !appended.is_empty() {
                println!("  {}: {} entries (pending rebuild)",
                    "Append log".yellow(), appended.len());
            }
        }
        Cmd::Find { query, k } => {
            let r = MicroscopeReader::open(&config);
            println!("{} '{}':", "FIND".cyan().bold(), query);
            let res = r.find_text(&query, k);
            if res.is_empty() { println!("  (none)"); }
            for (_d, i) in res { r.print_result(i, 0.0); }
        }
        Cmd::Rebuild => {
            println!("{}", "Rebuilding with append log...".cyan());
            build(&config);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let _ = fs::remove_file(append_path);
            println!("  Append log cleared.");
        }
        Cmd::GpuBench => {
            gpu_bench(&config);
        }
        Cmd::Embed { query, k, metric } => {
            semantic_search(&config, &query, k, &metric);
        }
        Cmd::Verify => {
            verify_integrity(&config);
        }
        Cmd::VerifyMerkle => {
            verify_merkle(&config);
        }
        Cmd::Proof { block_index } => {
            merkle_proof(&config, block_index);
        }
        Cmd::Serve { port } => {
            streaming::start_endpoint_server(config, port);
        }
        Cmd::Query { mql } => {
            let t0 = Instant::now();
            let q = query::parse(&mql);
            let reader = MicroscopeReader::open(&config);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            let results = query::execute(&q, &reader, &appended);

            println!("{} '{}':", "MQL".cyan().bold(), mql);
            if results.is_empty() {
                println!("  (no results)");
            }
            for r in &results {
                if r.is_main {
                    reader.print_result(r.block_idx, r.score);
                } else {
                    print_append_result(&appended, r.block_idx, r.score);
                }
            }
            println!("\n  {} results in {:.0} us", results.len(), t0.elapsed().as_micros());
        }
        Cmd::Export { output } => {
            let output_dir = Path::new(&config.paths.output_dir);
            println!("{}", "EXPORT".cyan().bold());
            match snapshot::export(output_dir, Path::new(&output)) {
                Ok(()) => println!("  {}", "Done.".green()),
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
        Cmd::Import { input, output_dir } => {
            let out = output_dir.as_deref().unwrap_or(&config.paths.output_dir);
            println!("{}", "IMPORT".cyan().bold());
            match snapshot::import(Path::new(&input), Path::new(out)) {
                Ok(()) => println!("  {}", "Done.".green()),
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
        Cmd::Diff { a, b } => {
            println!("{}", "DIFF".cyan().bold());
            match snapshot::diff(Path::new(&a), Path::new(&b)) {
                Ok(()) => {}
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_ccitt_known_vector() {
        // Standard CRC16-CCITT test vector: "123456789" → 0x29B1
        let data = b"123456789";
        assert_eq!(crc16_ccitt(data), 0x29B1);
    }

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16_ccitt(b""), 0xFFFF);
    }

    #[test]
    fn test_crc16_deterministic() {
        let a = crc16_ccitt(b"hello world");
        let b = crc16_ccitt(b"hello world");
        assert_eq!(a, b);
        assert_ne!(a, crc16_ccitt(b"hello worl!"));
    }
}
