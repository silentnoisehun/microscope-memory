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

use microscope_memory::config::Config;
use microscope_memory::reader::{layer_color, print_append_result};
use microscope_memory::Cli;
use microscope_memory::Cmd;
use microscope_memory::*;

use std::fs;
use std::path::Path;
use std::time::Instant;

use clap::Parser;
use colored::Colorize;

// ─── Command handlers ────────────────────────────────

fn open_reader(config: &Config) -> MicroscopeReader {
    MicroscopeReader::open(config).expect("Failed to open microscope index — run 'build' first")
}

fn bench(config: &Config, reader: &MicroscopeReader) {
    println!("{}", "Benchmark: 10,000 queries per zoom level".cyan());
    println!("  Mode: SIMD={} Rayon=true", cfg!(target_arch = "x86_64"));

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
        let label = if avg < 1000 {
            format!("{} ns", avg)
        } else {
            format!("{:.1} us", avg as f64 / 1000.0)
        };
        println!(
            "  ZOOM {}: {} / query  ({} blocks)",
            zoom,
            label.yellow(),
            c
        );
    }

    println!(
        "\n  {}: {:.0} ns avg",
        "OVERALL".green().bold(),
        total_ns as f64 / (iters * 9) as f64
    );

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

fn stats(reader: &MicroscopeReader) {
    let hdr_size = reader.block_count * HEADER_SIZE;
    let dat_size = reader.data.len();
    println!("{}", "=".repeat(50));
    println!("  {}", "MICROSCOPE MEMORY (pure binary)".cyan().bold());
    println!("{}", "=".repeat(50));
    println!("  Blocks:    {}", reader.block_count);
    println!("  Headers:   {:.1} KB", hdr_size as f64 / 1024.0);
    println!("  Data:      {:.1} KB", dat_size as f64 / 1024.0);
    println!(
        "  Total:     {:.1} KB",
        (hdr_size + dat_size) as f64 / 1024.0
    );
    println!("  Viewport:  {} chars/block", BLOCK_DATA_SIZE);

    let fits = if hdr_size < 32768 {
        "L1d"
    } else if hdr_size < 262144 {
        "L2"
    } else {
        "L3"
    };
    println!("  Cache:     {}", fits.green().bold());

    println!("\n  Depths:");
    for (d, &(_s, c)) in reader.depth_ranges.iter().enumerate() {
        let bar_len = (c as f64 / reader.block_count as f64 * 40.0) as usize;
        println!("    D{}: {:>5}  {}", d, c, "|".repeat(bar_len).cyan());
    }
    println!("{}", "=".repeat(50));
}

fn recall(config: &Config, query: &str, k: usize) {
    let t0 = Instant::now();
    let reader = open_reader(config);
    println!("{} '{}':", "RECALL".cyan().bold(), query);

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);
    let (zoom_lo, zoom_hi) = match query.len() {
        0..=10 => (0, 3),
        11..=40 => (3, 6),
        _ => (6, 8),
    };

    let mut all_results: Vec<(f32, usize, bool)> = Vec::new();

    let q_lower = query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

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
                let spatial_dist = dx * dx + dy * dy + dz * dz;
                let boost = keyword_hits as f32 * 0.1;
                let combined = (spatial_dist - boost).max(0.0);
                all_results.push((combined, i, true));
            }
        }
    }

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (ai, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx * dx + dy * dy + dz * dz;
        let text_lower = entry.text.to_lowercase();
        let keyword_hits = keywords
            .iter()
            .filter(|&&kw| text_lower.contains(kw))
            .count();
        let boost = keyword_hits as f32 * 0.1;
        if dist < 0.1 || keyword_hits > 0 {
            all_results.push(((dist - boost).max(0.0), ai + 1_000_000, false));
        }
    }

    let mut seen = std::collections::HashSet::new();
    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut shown = 0;

    for (dist, idx, is_main) in &all_results {
        if shown >= k {
            break;
        }
        if !seen.insert((*idx, *is_main)) {
            continue;
        }

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

fn semantic_search(config: &Config, query: &str, k: usize, metric: &str) {
    use microscope_memory::embedding_index::EmbeddingIndex;
    use microscope_memory::embeddings::{
        cosine_similarity_simd, EmbeddingProvider, MockEmbeddingProvider,
    };

    let t0 = Instant::now();
    println!(
        "{} '{}' using {} metric",
        "SEMANTIC SEARCH".cyan().bold(),
        safe_truncate(query, 50),
        metric.green()
    );

    let reader = open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let emb_path = output_dir.join("embeddings.bin");

    if let Some(idx) = EmbeddingIndex::open(&emb_path) {
        println!(
            "  Using pre-built embedding index ({} blocks, {} dim)",
            idx.block_count(),
            idx.dim()
        );

        #[cfg(feature = "embeddings")]
        let provider: Box<dyn EmbeddingProvider> = if config.embedding.provider == "candle" {
            match microscope_memory::embeddings::CandleEmbeddingProvider::new(
                &config.embedding.model,
            ) {
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
            Err(_) => {
                println!("  {} Failed to embed query", "ERROR:".red());
                return;
            }
        };

        let results = idx.search(&query_embedding, k);
        println!("\n  {} {} results:", "Found".green(), results.len());
        for (sim, block_idx) in results {
            let h = reader.header(block_idx);
            let text = reader.text(block_idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            let preview: String = text.chars().take(70).filter(|&c| c != '\n').collect();
            println!(
                "  {} {} {} {}",
                format!("D{}", h.depth).cyan(),
                format!("Sim={:.3}", sim).yellow(),
                format!("[{}/{}]", layer, layer_color(h.layer_id)).green(),
                preview
            );
        }

        let elapsed = t0.elapsed();
        println!(
            "\n  Semantic search (indexed) in {:.1} ms",
            elapsed.as_micros() as f64 / 1000.0
        );
        return;
    }

    println!("  No embedding index — computing on-the-fly (slow)");
    let provider = MockEmbeddingProvider::new(128);

    let query_embedding = match provider.embed(query) {
        Ok(e) => e,
        Err(_) => {
            println!("  {} Failed to generate embedding", "ERROR:".red());
            return;
        }
    };

    let mut results: Vec<(f32, usize)> = Vec::new();
    for i in 0..reader.block_count {
        let text = reader.text(i);
        if let Ok(block_embedding) = provider.embed(text) {
            let similarity = match metric {
                "cosine" => cosine_similarity_simd(&query_embedding, &block_embedding),
                "dot" => query_embedding
                    .iter()
                    .zip(block_embedding.iter())
                    .map(|(a, b)| a * b)
                    .sum(),
                "l2" => {
                    let dist: f32 = query_embedding
                        .iter()
                        .zip(block_embedding.iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f32>()
                        .sqrt();
                    1.0 / (1.0 + dist)
                }
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
        println!(
            "  {} {} {} {}",
            format!("D{}", h.depth).cyan(),
            format!("Sim={:.3}", sim).yellow(),
            format!("[{}/{}]", layer, layer_color(h.layer_id)).green(),
            preview
        );
    }

    let elapsed = t0.elapsed();
    println!(
        "\n  Semantic search (on-the-fly) in {:.1} ms",
        elapsed.as_micros() as f64 / 1000.0
    );
}

fn verify_integrity(config: &Config) {
    let reader = open_reader(config);
    println!(
        "{} {} blocks...",
        "VERIFY".cyan().bold(),
        reader.block_count
    );

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
            println!(
                "  {} Block {} D{}: CRC mismatch (stored=0x{:04X}, computed=0x{:04X})",
                "FAIL".red().bold(),
                i,
                h.depth,
                stored,
                computed
            );
            bad += 1;
        } else {
            checked += 1;
        }
    }

    if bad == 0 {
        println!(
            "  {} {} blocks verified, {} skipped (no CRC)",
            "OK".green().bold(),
            checked,
            skipped
        );
    } else {
        println!(
            "  {} {} corrupted, {} ok, {} skipped",
            "FAIL".red().bold(),
            bad,
            checked,
            skipped
        );
    }
}

fn gpu_bench(config: &Config) {
    let reader = open_reader(config);
    println!(
        "{} {} blocks",
        "GPU BENCH".cyan().bold(),
        reader.block_count
    );

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
        let r = reader.look_soft(
            &config_clone,
            next_f32(),
            next_f32(),
            next_f32(),
            z,
            5,
            config.search.zoom_weight,
        );
        std::hint::black_box(&r);
    }
    let cpu_ns = t0.elapsed().as_nanos() / iters as u128;
    println!("  CPU: {} ns/query", cpu_ns);

    #[cfg(feature = "gpu")]
    {
        match microscope_memory::gpu::GpuAccelerator::new(&reader) {
            Ok(accel) => {
                for _ in 0..10 {
                    let z = (next_f32() * 10.0) as u8 % 6;
                    let _ = accel.l2_search_4d(
                        next_f32(),
                        next_f32(),
                        next_f32(),
                        z,
                        config.search.zoom_weight,
                        5,
                    );
                }

                let t0 = Instant::now();
                for _ in 0..iters {
                    let z = (next_f32() * 10.0) as u8 % 6;
                    let r = accel.l2_search_4d(
                        next_f32(),
                        next_f32(),
                        next_f32(),
                        z,
                        config.search.zoom_weight,
                        5,
                    );
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
        println!(
            "  {} GPU feature not compiled. Use: cargo build --features gpu",
            "WARN".yellow()
        );
    }
}

fn verify_merkle(config: &Config) {
    use microscope_memory::merkle;

    let output_dir = Path::new(&config.paths.output_dir);
    let merkle_path = output_dir.join("merkle.bin");
    let meta_path = output_dir.join("meta.bin");

    if !merkle_path.exists() {
        println!(
            "  {} merkle.bin not found — rebuild with v0.2.0 to generate",
            "ERR".red()
        );
        return;
    }

    let meta = fs::read(&meta_path).expect("read meta.bin");
    let magic = &meta[0..4];
    if magic != b"MSC2" && magic != b"MSC3" {
        println!(
            "  {} meta.bin is v1 (MSCM) — no merkle root stored. Rebuild first.",
            "WARN".yellow()
        );
        return;
    }
    let meta_root_offset = META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE;
    let mut stored_root = [0u8; 32];
    stored_root.copy_from_slice(&meta[meta_root_offset..meta_root_offset + 32]);

    let merkle_data = fs::read(&merkle_path).expect("read merkle.bin");
    let stored_tree = merkle::MerkleTree::from_bytes(&merkle_data).expect("parse merkle.bin");

    println!(
        "{} {} blocks...",
        "VERIFY MERKLE".cyan().bold(),
        stored_tree.leaf_count
    );
    println!("  Stored root:   {}", hex_str(&stored_root));
    println!("  Merkle root:   {}", hex_str(&stored_tree.root));

    if stored_root != stored_tree.root {
        println!(
            "  {} meta.bin root != merkle.bin root!",
            "MISMATCH".red().bold()
        );
        return;
    }

    let reader = open_reader(config);
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
        println!(
            "  {} All {} blocks verified against Merkle root",
            "OK".green().bold(),
            reader.block_count
        );
    } else {
        println!(
            "  {} {} block(s) failed verification:",
            "FAIL".red().bold(),
            bad_blocks.len()
        );
        for &idx in bad_blocks.iter().take(20) {
            println!("    Block {}", idx);
        }
        if bad_blocks.len() > 20 {
            println!("    ... and {} more", bad_blocks.len() - 20);
        }
    }
}

fn merkle_proof(config: &Config, block_index: usize) {
    use microscope_memory::merkle;

    let output_dir = Path::new(&config.paths.output_dir);
    let merkle_path = output_dir.join("merkle.bin");

    if !merkle_path.exists() {
        println!("  {} merkle.bin not found — rebuild first", "ERR".red());
        return;
    }

    let merkle_data = fs::read(&merkle_path).expect("read merkle.bin");
    let tree = merkle::MerkleTree::from_bytes(&merkle_data).expect("parse merkle.bin");

    if block_index >= tree.leaf_count {
        println!(
            "  {} Block index {} out of range (max: {})",
            "ERR".red(),
            block_index,
            tree.leaf_count - 1
        );
        return;
    }

    let reader = open_reader(config);
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

    let data_start = h.data_offset as usize;
    let data_end = data_start + h.data_len as usize;
    let block_data = &reader.data[data_start..data_end];
    let valid = merkle::MerkleTree::verify_proof(&tree.root, block_data, &proof);
    if valid {
        println!(
            "  {} Proof valid against root {}",
            "VERIFIED".green().bold(),
            hex_str(&tree.root)
        );
    } else {
        println!("  {} Proof INVALID", "FAIL".red().bold());
    }
}

// ─── MAIN ────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    let config = Config::load(DEFAULT_CONFIG_PATH).unwrap_or_else(|_| {
        println!("  {} Using default configuration", "WARN:".yellow());
        Config::default()
    });

    match cli.cmd {
        Cmd::Build { force } => {
            microscope_memory::build::build(&config, force).expect("build failed");
        }
        Cmd::Store {
            text,
            layer,
            importance,
        } => {
            store_memory(&config, &text, &layer, importance).expect("store failed");
        }
        Cmd::Recall { query, k } => {
            recall(&config, &query, k);
        }
        Cmd::Look { x, y, z, zoom, k } => {
            let config_clone = config.clone();
            let r = open_reader(&config);
            println!(
                "{} ({:.2},{:.2},{:.2}) zoom={}:",
                "MICROSCOPE".cyan().bold(),
                x,
                y,
                z,
                zoom
            );
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
        Cmd::Soft {
            x,
            y,
            z,
            zoom,
            k,
            gpu: use_gpu,
        } => {
            let r = open_reader(&config);
            let use_gpu = use_gpu || config.performance.use_gpu;
            println!(
                "{} 4D ({:.2},{:.2},{:.2}) z={} {}:",
                "MICROSCOPE".cyan().bold(),
                x,
                y,
                z,
                zoom,
                if use_gpu { "[GPU]" } else { "[CPU]" }
            );

            #[cfg(feature = "gpu")]
            if use_gpu {
                match microscope_memory::gpu::GpuAccelerator::new(&r) {
                    Ok(accel) => {
                        let res = accel.l2_search_4d(x, y, z, zoom, config.search.zoom_weight, k);
                        for (dist, idx) in res {
                            r.print_result(idx, dist);
                        }
                        return;
                    }
                    Err(e) => {
                        eprintln!(
                            "  {} GPU init failed: {}, falling back to CPU",
                            "WARN".yellow(),
                            e
                        );
                    }
                }
            }

            #[cfg(not(feature = "gpu"))]
            if use_gpu {
                eprintln!(
                    "  {} GPU feature not compiled. Use --features gpu",
                    "WARN".yellow()
                );
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
        Cmd::Bench => bench(&config, &open_reader(&config)),
        Cmd::Stats => {
            let r = open_reader(&config);
            stats(&r);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            if !appended.is_empty() {
                println!(
                    "  {}: {} entries (pending rebuild)",
                    "Append log".yellow(),
                    appended.len()
                );
            }
        }
        Cmd::Find { query, k } => {
            let r = open_reader(&config);
            println!("{} '{}':", "FIND".cyan().bold(), query);
            let res = r.find_text(&query, k);
            if res.is_empty() {
                println!("  (none)");
            }
            for (_d, i) in res {
                r.print_result(i, 0.0);
            }
        }
        Cmd::Rebuild => {
            println!("{}", "Rebuilding with append log...".cyan());
            microscope_memory::build::build(&config, true).expect("rebuild failed");
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
            microscope_memory::streaming::start_endpoint_server(config, port);
        }
        Cmd::Query { mql } => {
            let t0 = Instant::now();
            let q = microscope_memory::query::parse(&mql);
            let reader = open_reader(&config);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            let results = microscope_memory::query::execute(&q, &reader, &appended);

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
            println!(
                "\n  {} results in {:.0} us",
                results.len(),
                t0.elapsed().as_micros()
            );
        }
        Cmd::Export { output } => {
            let output_dir = Path::new(&config.paths.output_dir);
            println!("{}", "EXPORT".cyan().bold());
            match microscope_memory::snapshot::export(output_dir, Path::new(&output)) {
                Ok(()) => println!("  {}", "Done.".green()),
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
        Cmd::Import { input, output_dir } => {
            let out = output_dir.as_deref().unwrap_or(&config.paths.output_dir);
            println!("{}", "IMPORT".cyan().bold());
            match microscope_memory::snapshot::import(Path::new(&input), Path::new(out)) {
                Ok(()) => println!("  {}", "Done.".green()),
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
        Cmd::Diff { a, b } => {
            println!("{}", "DIFF".cyan().bold());
            match microscope_memory::snapshot::diff(Path::new(&a), Path::new(&b)) {
                Ok(()) => {}
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
        Cmd::FederatedRecall { query, k } => {
            let fed = microscope_memory::federation::FederatedSearch::from_config(&config)
                .expect("federation config");
            let results = fed.recall(&query, k);
            println!(
                "{} '{}' across {} indices:",
                "FEDERATED RECALL".cyan().bold(),
                query,
                config.federation.indices.len()
            );
            if results.is_empty() {
                println!("  (no results)");
            }
            for r in &results {
                println!(
                    "  [D{} {} score={:.3} src={}] {}",
                    r.depth,
                    r.layer,
                    r.score,
                    r.source_index.cyan(),
                    microscope_memory::safe_truncate(&r.text, 80)
                );
            }
            println!("\n  {} results", results.len());
        }
        Cmd::FederatedFind { query, k } => {
            let fed = microscope_memory::federation::FederatedSearch::from_config(&config)
                .expect("federation config");
            let results = fed.find_text(&query, k);
            println!(
                "{} '{}' across {} indices:",
                "FEDERATED FIND".cyan().bold(),
                query,
                config.federation.indices.len()
            );
            if results.is_empty() {
                println!("  (no results)");
            }
            for r in &results {
                println!(
                    "  [D{} {} src={}] {}",
                    r.depth,
                    r.layer,
                    r.source_index.cyan(),
                    microscope_memory::safe_truncate(&r.text, 80)
                );
            }
        }
        Cmd::Mcp => {
            microscope_memory::mcp::run(config);
        }
    }
}
