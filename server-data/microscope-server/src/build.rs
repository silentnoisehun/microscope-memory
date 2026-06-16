//! Extracts text from RAW TEXT layer files, constructs a 9-depth block hierarchy
//! (identity → layers → clusters → items → sentences → tokens → syllables → chars → bytes),
//! and writes the binary output files (microscope.bin, data.bin, meta.bin, merkle.bin, embeddings.bin).

use crate::config::Config;
use crate::reader::{BlockHeader, MicroscopeReader};
use crate::{
    content_coords_blended, crc16_ccitt, hex_str, layer_to_id, merkle, safe_truncate, to_block,
    BLOCK_DATA_SIZE, DEPTH_ENTRY_SIZE, HEADER_SIZE, META_HEADER_SIZE,
};

use colored::Colorize;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

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

// ─── Extract text values from RAW files ───────────────────
// Zero JSON dependency. Standard UTF-8 text files.
// Files are read and split into blocks by default.

fn extract_texts_from_file(path: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return texts,
    };

    // Split by double newline or chunking
    for chunk in raw.split("\n\n") {
        let trimmed = chunk.trim();
        if trimmed.len() > 3 {
            texts.push(trimmed.to_string());
        }
    }

    // Fallback if no doubles: chunk by size
    if texts.len() < 2 {
        texts.clear();
        let chars: Vec<char> = raw.chars().collect();
        for chunk in chars.chunks(BLOCK_DATA_SIZE) {
            let s: String = chunk.iter().collect();
            if s.trim().len() > 5 {
                texts.push(s);
            }
        }
    }

    texts
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

// ─── Compute deterministic SHA-256 hash of all layer source files ────
pub fn compute_layers_hash(config: &Config) -> [u8; 32] {
    let layers_dir = Path::new(&config.paths.layers_dir);
    let layer_files = &config.memory_layers.layers;
    let mut sorted_names: Vec<&String> = layer_files.iter().collect();
    sorted_names.sort();
    let mut hasher = Sha256::new();
    for name in &sorted_names {
        let path = layers_dir.join(format!("{}.txt", name));
        if let Ok(contents) = fs::read(&path) {
            hasher.update(&contents);
        }
    }
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

// ─── BUILD: layers/ → binary ─────────────────────────
pub fn build(config: &Config, force: bool) -> Result<(), String> {
    let layers_hash = compute_layers_hash(config);

    // Incremental build check — skip if layers unchanged
    if !force {
        let output_dir = Path::new(&config.paths.output_dir);
        let meta_path = output_dir.join("meta.bin");
        if let Ok(meta) = fs::read(&meta_path) {
            if meta.len() >= 152 && &meta[0..4] == b"MSC3" {
                let stored_hash = &meta[120..152];
                if stored_hash == &layers_hash[..] {
                    println!("{}", "Layers unchanged — skipping rebuild".green().bold());
                    return Ok(());
                }
            }
        }
    }

    println!(
        "{}",
        "Building microscope from raw layers (zero JSON)..."
            .cyan()
            .bold()
    );

    let layers_dir = Path::new(&config.paths.layers_dir);
    let output_dir = Path::new(&config.paths.output_dir);

    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| format!("create output dir: {}", e))?;
    }

    let layer_files = &config.memory_layers.layers;

    // Collect all raw texts per layer
    let mut layer_texts: Vec<(String, Vec<String>)> = Vec::new();
    for name in layer_files {
        let path = layers_dir.join(format!("{}.txt", name));
        let texts = extract_texts_from_file(&path);
        println!("  {} {}: {} items", ">".green(), name, texts.len());
        layer_texts.push((name.clone(), texts));
    }

    let mut blocks: Vec<RawBlock> = Vec::new();

    // ═══ DEPTH 0: Identity ═══
    let identity = "Microscope Memory: 9-depth hierarchical cognitive engine. Binary mmap, sub-microsecond spatial search, Hebbian learning, Merkle integrity.";
    blocks.push(RawBlock {
        data: to_block(identity),
        depth: 0,
        x: 0.25,
        y: 0.25,
        z: 0.25,
        layer_id: 0,
        parent_idx: u32::MAX,
        child_count: layer_files.len() as u16,
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
            depth: 1,
            x,
            y,
            z,
            layer_id: layer_to_id(name),
            parent_idx: 0,
            child_count: texts.len().div_ceil(5) as u16, // cluster count
        });
    }

    // ═══ DEPTH 2: Clusters (5 items each) ═══
    let _depth2_start = blocks.len();
    let mut depth2_layer_offsets: Vec<(usize, usize)> = Vec::new(); // (start_in_blocks, count)
    for (li, (name, texts)) in layer_texts.iter().enumerate() {
        let cluster_start = blocks.len();
        for ci in (0..texts.len()).step_by(5) {
            let chunk: Vec<String> = texts[ci..texts.len().min(ci + 5)]
                .iter()
                .map(|s| safe_truncate(s, 40))
                .collect();
            let summary = format!("[{} #{}] {}", name, ci / 5, chunk.join(" | "));
            let (x, y, z) = content_coords_blended(&summary, name, sw);
            blocks.push(RawBlock {
                data: to_block(&summary),
                depth: 2,
                x,
                y,
                z,
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
            let parent = if cluster_idx < d2_count {
                (d2_start + cluster_idx) as u32
            } else {
                u32::MAX
            };

            blocks.push(RawBlock {
                data: to_block(text),
                depth: 3,
                x,
                y,
                z,
                layer_id: layer_to_id(name),
                parent_idx: parent,
                child_count: 0, // will update
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
                if sent.len() < 10 {
                    continue;
                }
                let (px, py, pz) = depth3_positions[d3i - depth3_start];
                let h = sent
                    .as_bytes()
                    .iter()
                    .fold(0u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
                let ox = ((h & 0xFF) as f32 - 128.0) / 25500.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 25500.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 25500.0;

                local_blocks.push(RawBlock {
                    data: to_block(sent),
                    depth: 4,
                    x: px + ox,
                    y: py + oy,
                    z: pz + oz,
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

            let tokens: Vec<String> = text_owned
                .split_whitespace()
                .take(8)
                .map(|s| s.to_string())
                .collect();
            let mut local_blocks = Vec::new();
            for tok in &tokens {
                if tok.len() < 2 {
                    continue;
                }
                let h = tok
                    .as_bytes()
                    .iter()
                    .fold(0u64, |a, &b| a.wrapping_mul(31).wrapping_add(b as u64));
                let ox = ((h & 0xFF) as f32 - 128.0) / 255000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 255000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 255000.0;

                local_blocks.push(RawBlock {
                    data: to_block(tok),
                    depth: 5,
                    x: px + ox,
                    y: py + oy,
                    z: pz + oz,
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
            if chars.len() < 3 {
                return vec![];
            }
            let chunk_size = 3.max(chars.len() / 3).min(5);
            let mut local_blocks = Vec::new();
            for chunk in chars.chunks(chunk_size) {
                let syl: String = chunk.iter().collect();
                if syl.trim().is_empty() {
                    continue;
                }
                let h = syl
                    .as_bytes()
                    .iter()
                    .fold(0u64, |a, &b| a.wrapping_mul(37).wrapping_add(b as u64));
                let ox = ((h & 0xFF) as f32 - 128.0) / 2550000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 2550000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 2550000.0;

                local_blocks.push(RawBlock {
                    data: to_block(&syl),
                    depth: 6,
                    x: px + ox,
                    y: py + oy,
                    z: pz + oz,
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
                if ch.is_whitespace() {
                    continue;
                }
                let h = (ch as u64).wrapping_mul(0x517cc1b727220a95);
                let ox = ((h & 0xFF) as f32 - 128.0) / 25500000.0;
                let oy = (((h >> 8) & 0xFF) as f32 - 128.0) / 25500000.0;
                let oz = (((h >> 16) & 0xFF) as f32 - 128.0) / 25500000.0;

                let ch_str = ch.to_string();
                local_blocks.push(RawBlock {
                    data: to_block(&ch_str),
                    depth: 7,
                    x: px + ox,
                    y: py + oy,
                    z: pz + oz,
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
                    depth: 8,
                    x: px + ox,
                    y: py + oy,
                    z: pz + oz,
                    layer_id: lid,
                    parent_idx: d7i as u32,
                    child_count: 0, // LEAF. Below = corruption.
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

    let mut hdr_file = BufWriter::new(
        fs::File::create(&hdr_path).map_err(|e| format!("create microscope.bin: {}", e))?,
    );
    let mut dat_file =
        BufWriter::new(fs::File::create(&dat_path).map_err(|e| format!("create data.bin: {}", e))?);

    let mut depth_ranges: Vec<(u32, u32)> = vec![(0, 0); 9];
    let mut cur_depth: u8 = 0;
    let mut range_start: u32 = 0;

    for (new_i, &old_i) in indices.iter().enumerate() {
        let b = &blocks[old_i];
        let offset = dat_file
            .stream_position()
            .map_err(|e| format!("data.bin stream_position: {}", e))? as u32;
        let len = b.data.len().min(BLOCK_DATA_SIZE) as u16;
        dat_file
            .write_all(&b.data[..len as usize])
            .map_err(|e| format!("write data.bin: {}", e))?;

        let parent = if b.parent_idx == u32::MAX {
            u32::MAX
        } else {
            old_to_new[b.parent_idx as usize]
        };

        let crc = crc16_ccitt(&b.data[..len as usize]);
        let hdr = BlockHeader {
            x: b.x,
            y: b.y,
            z: b.z,
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
        hdr_file
            .write_all(bytes)
            .map_err(|e| format!("write microscope.bin: {}", e))?;

        // Track depth ranges
        if b.depth != cur_depth {
            depth_ranges[cur_depth as usize] = (range_start, new_i as u32 - range_start);
            range_start = new_i as u32;
            cur_depth = b.depth;
        }
    }
    depth_ranges[cur_depth as usize] = (range_start, n as u32 - range_start);
    hdr_file
        .flush()
        .map_err(|e| format!("flush microscope.bin: {}", e))?;
    dat_file
        .flush()
        .map_err(|e| format!("flush data.bin: {}", e))?;

    // ═══ Optional zstd compression of data.bin ═══
    #[cfg(feature = "compression")]
    if config.performance.compression {
        let raw_data =
            fs::read(&dat_path).map_err(|e| format!("read data.bin for compression: {}", e))?;
        let raw_size = raw_data.len();
        let compressed = zstd::encode_all(std::io::Cursor::new(&raw_data), 3)
            .map_err(|e| format!("zstd compress: {}", e))?;
        let comp_size = compressed.len();
        let zst_path = output_dir.join("data.bin.zst");
        fs::write(&zst_path, &compressed).map_err(|e| format!("write data.bin.zst: {}", e))?;
        let ratio = if comp_size > 0 {
            raw_size as f64 / comp_size as f64
        } else {
            0.0
        };
        println!(
            "  {}: {} → {} bytes ({:.1}x ratio)",
            "zstd".green(),
            raw_size,
            comp_size,
            ratio,
        );
    }

    // ═══ Merkle tree: SHA-256 over all block data ═══
    let merkle_path = output_dir.join("merkle.bin");
    // Re-read data.bin to get all block data slices for Merkle leaves
    hdr_file
        .flush()
        .map_err(|e| format!("flush microscope.bin: {}", e))?;
    dat_file
        .flush()
        .map_err(|e| format!("flush data.bin: {}", e))?;

    let dat_bytes = fs::read(&dat_path).map_err(|e| format!("read data.bin for merkle: {}", e))?;
    let hdr_bytes =
        fs::read(&hdr_path).map_err(|e| format!("read microscope.bin for merkle: {}", e))?;
    let mut leaf_slices: Vec<&[u8]> = Vec::with_capacity(n);
    for i in 0..n {
        let hdr_off = i * HEADER_SIZE;
        let data_offset =
            u32::from_le_bytes(hdr_bytes[hdr_off + 18..hdr_off + 22].try_into().unwrap()) as usize;
        let data_len =
            u16::from_le_bytes(hdr_bytes[hdr_off + 22..hdr_off + 24].try_into().unwrap()) as usize;
        if data_offset + data_len <= dat_bytes.len() {
            leaf_slices.push(&dat_bytes[data_offset..data_offset + data_len]);
        } else {
            leaf_slices.push(&[]);
        }
    }

    let merkle_tree = merkle::MerkleTree::build(&leaf_slices);
    fs::write(&merkle_path, merkle_tree.to_bytes())
        .map_err(|e| format!("write merkle.bin: {}", e))?;
    println!(
        "  {}: {} leaves, root={}",
        "merkle".green(),
        merkle_tree.leaf_count,
        hex_str(&merkle_tree.root)
    );

    // meta.bin — MSC3 format with merkle root + layers hash
    let mut meta_buf = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE + 32 + 32);
    meta_buf.extend_from_slice(b"MSC3"); // magic v3
    meta_buf.extend_from_slice(&3u32.to_le_bytes()); // version
    meta_buf.extend_from_slice(&(n as u32).to_le_bytes()); // block_count
    meta_buf.extend_from_slice(&9u32.to_le_bytes()); // depth_count
    for &(start, count) in &depth_ranges {
        meta_buf.extend_from_slice(&start.to_le_bytes());
        meta_buf.extend_from_slice(&count.to_le_bytes());
    }
    meta_buf.extend_from_slice(&merkle_tree.root); // 32 bytes merkle root
    meta_buf.extend_from_slice(&layers_hash); // 32 bytes layers content hash
    fs::write(meta_path, &meta_buf).map_err(|e| format!("write meta.bin: {}", e))?;

    // Report
    let hdr_size = n * HEADER_SIZE;
    let dat_size = dat_file.stream_position().unwrap_or(0) as usize; // Get final data size
    let meta_size = meta_buf.len();
    println!(
        "\n  {}: {} bytes ({:.1} KB)",
        "headers".green(),
        hdr_size,
        hdr_size as f64 / 1024.0
    );
    println!(
        "  {}:    {} bytes ({:.1} KB)",
        "data".green(),
        dat_size,
        dat_size as f64 / 1024.0
    );
    println!("  {}:    {} bytes", "meta".green(), meta_size);
    println!(
        "  {}:   {:.1} KB",
        "TOTAL".yellow().bold(),
        (hdr_size + dat_size + meta_size) as f64 / 1024.0
    );

    let fits = if hdr_size < 32768 {
        "L1d (32KB)"
    } else if hdr_size < 262144 {
        "L2 (256KB)"
    } else {
        "L3"
    };
    println!("  cache:   {}", fits.green().bold());

    for (d, &(_start, count)) in depth_ranges.iter().enumerate() {
        println!("  Depth {}: {:>5} blocks", d, count);
    }

    // ═══ Embedding index (mock provider, or candle if enabled) ═══
    if config.embedding.provider != "none" {
        println!("\n  Building embedding index...");
        let emb_path = output_dir.join("embeddings.bin");
        let reader = MicroscopeReader::open(config)?;
        let max_depth = config.embedding.max_depth;

        #[cfg(feature = "embeddings")]
        let provider: Box<dyn crate::embeddings::EmbeddingProvider> =
            if config.embedding.provider == "candle" {
                match crate::embeddings::CandleEmbeddingProvider::new(&config.embedding.model) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        eprintln!(
                            "  {} Candle init failed: {:?}, using mock",
                            "WARN".yellow(),
                            e
                        );
                        Box::new(crate::embeddings::MockEmbeddingProvider::new(
                            config.embedding.dim,
                        ))
                    }
                }
            } else {
                Box::new(crate::embeddings::MockEmbeddingProvider::new(
                    config.embedding.dim,
                ))
            };

        #[cfg(not(feature = "embeddings"))]
        let provider: Box<dyn crate::embeddings::EmbeddingProvider> = Box::new(
            crate::embeddings::MockEmbeddingProvider::new(config.embedding.dim),
        );

        match crate::embedding_index::build_embedding_index(
            &*provider, &reader, max_depth, &emb_path,
        ) {
            Ok(()) => println!("  {} embeddings.bin built", "OK".green()),
            Err(e) => eprintln!("  {} embedding build: {}", "ERR".red(), e),
        }
    }

    // ═══ Hebbian delta integration ═══
    let hebb_path = output_dir.join("activations.bin");
    if hebb_path.exists() {
        let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, n);
        let drifted = hebb
            .activations
            .iter()
            .filter(|r| {
                r.drift_x.abs() > 0.001 || r.drift_y.abs() > 0.001 || r.drift_z.abs() > 0.001
            })
            .count();

        if drifted > 0 {
            apply_hebbian_deltas(output_dir, &hebb, n)?;
            println!(
                "  {} Hebbian deltas applied to {} blocks",
                "HEBBIAN".magenta(),
                drifted
            );
        }
    }

    // ═══ Structural fingerprinting ═══
    {
        let reader = MicroscopeReader::open(config)?;
        let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();
        let table = crate::fingerprint::LinkTable::build(&texts);
        table.save(output_dir)?;
        let stats = table.stats();
        println!(
            "  {} {} links across {} blocks",
            "FINGERPRINT".cyan(),
            stats.link_count,
            stats.block_count
        );
    }

    println!("\n{}", "ZERO JSON. Pure binary. Done.".green().bold());
    Ok(())
}

/// Post-process: apply Hebbian drift deltas to microscope.bin header coordinates.
fn apply_hebbian_deltas(
    output_dir: &Path,
    hebb: &crate::hebbian::HebbianState,
    block_count: usize,
) -> Result<(), String> {
    let hdr_path = output_dir.join("microscope.bin");
    let mut data = fs::read(&hdr_path).map_err(|e| format!("read microscope.bin: {}", e))?;

    for i in 0..block_count.min(hebb.activations.len()) {
        let rec = &hebb.activations[i];
        if rec.drift_x.abs() < 0.001 && rec.drift_y.abs() < 0.001 && rec.drift_z.abs() < 0.001 {
            continue;
        }

        let off = i * HEADER_SIZE;
        if off + 12 > data.len() {
            break;
        }

        // Read current x, y, z (first 12 bytes of header, 3×f32 LE)
        let x = f32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        let y = f32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap());
        let z = f32::from_le_bytes(data[off + 8..off + 12].try_into().unwrap());

        // Apply drift
        let new_x = x + rec.drift_x;
        let new_y = y + rec.drift_y;
        let new_z = z + rec.drift_z;

        data[off..off + 4].copy_from_slice(&new_x.to_le_bytes());
        data[off + 4..off + 8].copy_from_slice(&new_y.to_le_bytes());
        data[off + 8..off + 12].copy_from_slice(&new_z.to_le_bytes());
    }

    fs::write(&hdr_path, &data).map_err(|e| format!("write microscope.bin: {}", e))?;

    // Clear drift values after integration (they're now baked in)
    let mut hebb_clone = crate::hebbian::HebbianState {
        activations: hebb.activations.clone(),
        coactivations: hebb.coactivations.clone(),
        fingerprints: hebb.fingerprints.clone(),
    };
    for rec in &mut hebb_clone.activations {
        rec.drift_x = 0.0;
        rec.drift_y = 0.0;
        rec.drift_z = 0.0;
    }
    hebb_clone
        .save(output_dir)
        .map_err(|e| format!("save cleared Hebbian: {}", e))
}
