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

    // ─── Attention: compute layer weights from context ──
    let output_dir_att = Path::new(&config.paths.output_dir);
    let mut attention =
        microscope_memory::attention::AttentionState::load_or_init(output_dir_att);
    let hebb_pre =
        microscope_memory::hebbian::HebbianState::load_or_init(output_dir_att, reader.block_count);
    let tg_pre =
        microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_att);
    let pc_pre =
        microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_att);

    let emotional_energy = microscope_memory::emotional::emotional_field(&reader, &hebb_pre)
        .map(|f| f.total_energy)
        .unwrap_or(0.0);

    // Infer quality of previous recall and record outcome
    if attention.total_recalls > 0 {
        let quality = attention.infer_quality();
        if let Some(last) = attention.history.last() {
            let prev_weights = last.weights;
            attention.record_outcome(quality, &prev_weights);
        }
    }

    let attn_signals = microscope_memory::attention::AttentionSignals {
        query_length: query.len(),
        emotional_energy,
        session_depth: tg_pre.current_path().len(),
        pattern_confidence: 0.0, // updated below after pattern boost
        cache_hit_rate: pc_pre.stats.hit_rate(),
        archetype_match_score: 0.0, // updated below after archetype match
    };
    let attn = attention.compute_attention(&attn_signals);

    // Emotional bias warp: bend search coordinates toward emotional attractors
    let output_dir_eb = Path::new(&config.paths.output_dir);
    let hebb_eb =
        microscope_memory::hebbian::HebbianState::load_or_init(output_dir_eb, reader.block_count);
    let emotional_weight = config.search.emotional_bias_weight * attn.weight(4);
    let (qx, qy, qz) = microscope_memory::emotional::apply_emotional_bias(
        qx,
        qy,
        qz,
        emotional_weight,
        &reader,
        &hebb_eb,
    );

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

    // ─── ThoughtGraph + Predictive Cache ──
    let output_dir_tg = Path::new(&config.paths.output_dir);
    let mut thought_graph =
        microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_tg);
    let mut pred_cache =
        microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_tg);
    let qh_tg = microscope_memory::hebbian::query_hash(query);

    // Check predictive cache — instant boost from pre-fetched blocks (scaled by attention)
    if let Some((cached_blocks, confidence)) = pred_cache.check(qh_tg) {
        let boost = confidence * microscope_memory::thought_graph::PATTERN_BOOST_WEIGHT * attn.weight(6);
        let cached_set: std::collections::HashSet<u32> =
            cached_blocks.iter().copied().collect();
        for (dist, idx, is_main) in &mut all_results {
            if *is_main && cached_set.contains(&(*idx as u32)) {
                *dist = (*dist - boost).max(0.0);
            }
        }
        println!(
            "  {} {} blocks pre-fetched (confidence={:.0}%)",
            "PREDICT:".green(),
            cached_blocks.len(),
            confidence * 100.0
        );
    }

    // Pattern boost from ThoughtGraph
    let pattern_boosts: std::collections::HashMap<u32, f32> = thought_graph
        .pattern_boost(qh_tg)
        .into_iter()
        .collect();
    if !pattern_boosts.is_empty() {
        let tg_scale = attn.weight(5); // ThoughtGraph attention weight
        for (dist, idx, is_main) in &mut all_results {
            if *is_main {
                if let Some(&boost) = pattern_boosts.get(&(*idx as u32)) {
                    *dist = (*dist - boost * tg_scale).max(0.0);
                }
            }
        }
        println!(
            "  {} {} blocks boosted by thought patterns",
            "PATTERN:".yellow(),
            pattern_boosts.len()
        );
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

    // ─── Hebbian + Mirror: record activations & detect resonance ──
    let output_dir = Path::new(&config.paths.output_dir);
    let mut hebb =
        microscope_memory::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let mut mirror = microscope_memory::mirror::MirrorState::load_or_init(output_dir);
    let activated: Vec<(u32, f32)> = all_results
        .iter()
        .filter(|(_, _, is_main)| *is_main)
        .take(k)
        .map(|(score, idx, _)| (*idx as u32, *score))
        .collect();
    if !activated.is_empty() {
        let qh = microscope_memory::hebbian::query_hash(query);
        // Mirror: detect resonance before recording (so new fingerprint doesn't match itself)
        let boosts = microscope_memory::mirror::mirror_boost(&hebb, &mut mirror, &activated, qh);
        if !boosts.is_empty() {
            println!(
                "  {} {} blocks resonated",
                "MIRROR:".magenta(),
                boosts.len()
            );
        }
        hebb.record_activation(&activated, qh);

        // Resonance: emit pulse with spatial coordinates
        let mut resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
        let headers: Vec<(f32, f32, f32)> = activated
            .iter()
            .map(|&(idx, _)| {
                let h = reader.header(idx as usize);
                (h.x, h.y, h.z)
            })
            .collect();
        resonance.emit_pulse(&activated, qh, &headers, 1);

        // Archetype: reinforce + temporal tracking
        let mut archetypes = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
        let mut temporal = microscope_memory::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
        if let Some((idx, score)) = archetypes.match_archetype(&activated) {
            let arch_id = archetypes.archetypes[idx].id;
            let time_boost = temporal.boost(arch_id);
            temporal.record_activation(arch_id, microscope_memory::hebbian::now_epoch_ms_pub());
            let window = microscope_memory::temporal_archetype::current_time_window();
            println!(
                "  {} '{}' (score={:.3} temporal={:.2} window={})",
                "ARCHETYPE:".cyan(),
                archetypes.archetypes[idx].label,
                score,
                time_boost,
                microscope_memory::temporal_archetype::WINDOW_LABELS[window]
            );
        }
        temporal.decay();
        archetypes.reinforce(&activated);

        // ThoughtGraph: record recall and detect patterns
        let dominant_layer = activated
            .first()
            .map(|&(idx, _)| reader.header(idx as usize).layer_id)
            .unwrap_or(0);
        thought_graph.record_recall(qh, &activated, dominant_layer);
        let result_block_ids: Vec<u32> = activated.iter().map(|&(idx, _)| idx).collect();
        thought_graph.update_pattern_blocks(qh, &result_block_ids);
        thought_graph.detect_patterns();

        // Predictive cache: evaluate prediction accuracy and predict next
        let (hit_type, overlap) = pred_cache.evaluate(qh, &result_block_ids, &mut thought_graph);
        if hit_type != "none" {
            let symbol = match hit_type {
                "hit" => "+".green(),
                "partial" => "~".yellow(),
                _ => "-".red(),
            };
            println!(
                "  {} prediction {} (overlap={})",
                symbol, hit_type, overlap
            );
        }
        pred_cache.predict_next(&thought_graph);

        // Attention: mark recall and save
        attention.mark_recall();

        let _ = hebb.save(output_dir);
        let _ = mirror.save(output_dir);
        let _ = resonance.save(output_dir);
        let _ = archetypes.save(output_dir);
        let _ = temporal.save(output_dir);
        let _ = thought_graph.save(output_dir);
        let _ = pred_cache.save(output_dir);
        let _ = attention.save(output_dir);
    }

    // ─── Auto-populate depth cache after recall ──
    {
        let mut depth_map: std::collections::HashMap<u8, Vec<microscope_memory::depth_cache::CachedResult>> =
            std::collections::HashMap::new();

        for (dist, idx, is_main) in &all_results {
            if !*is_main { continue; }
            if *idx >= reader.block_count { continue; }

            let h = reader.header(*idx);
            let text = reader.text(*idx);
            let preview: String = text.chars().take(64).filter(|&c| c != '\n').collect();
            let energy = if *idx < hebb.activations.len() {
                hebb.activations[*idx].energy
            } else {
                0.0
            };

            let entry = depth_map.entry(h.depth).or_insert_with(Vec::new);
            if entry.len() < 5 {
                entry.push(microscope_memory::depth_cache::CachedResult {
                    block_idx: *idx as u32,
                    distance: *dist,
                    energy,
                    layer_id: h.layer_id,
                    depth: h.depth,
                    text: preview,
                    x: h.x,
                });
            }
        }

        if !depth_map.is_empty() {
            if let Ok(mut cache) = microscope_memory::depth_cache::DepthCache::open(output_dir) {
                let pairs: Vec<(u8, Vec<microscope_memory::depth_cache::CachedResult>)> =
                    depth_map.into_iter().collect();
                let _ = cache.write_recall(qh_tg, &pairs);
            }
        }
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
        Cmd::Radial {
            x,
            y,
            z,
            depth,
            radius,
            k,
        } => {
            let t0 = Instant::now();
            let reader = open_reader(&config);
            println!(
                "{} ({:.2},{:.2},{:.2}) D{} r={:.3}:",
                "RADIAL".cyan().bold(),
                x,
                y,
                z,
                depth,
                radius
            );

            let result_set = reader.radial_search(&config, x, y, z, depth, radius, k);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);

            if let Some(ref primary) = result_set.primary {
                println!("  {}", "PRIMARY:".green().bold());
                if primary.is_main {
                    reader.print_result(primary.block_idx, primary.dist_sq);
                } else {
                    print_append_result(&appended, primary.block_idx, primary.dist_sq);
                }
            }

            if !result_set.neighbors.is_empty() {
                println!(
                    "  {} ({}):",
                    "NEIGHBORS".yellow(),
                    result_set.neighbors.len()
                );
                for n in &result_set.neighbors {
                    if n.is_main {
                        let h = reader.header(n.block_idx);
                        let text = reader.text(n.block_idx);
                        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                        let preview: String =
                            text.chars().take(60).filter(|&c| c != '\n').collect();
                        println!(
                            "    {} {} {} w={:.3} {}",
                            format!("D{}", h.depth).cyan(),
                            format!("L2={:.5}", n.dist_sq).yellow(),
                            format!("[{}]", layer).green(),
                            n.weight,
                            preview
                        );
                    } else {
                        print_append_result(&appended, n.block_idx, n.dist_sq);
                    }
                }
            }

            println!(
                "\n  {} within radius, {} shown, {:.0} us",
                result_set.total_within_radius,
                result_set.all().len(),
                t0.elapsed().as_micros()
            );

            // Hebbian: record radial activation
            let output_dir = Path::new(&config.paths.output_dir);
            let mut hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let activated = result_set.block_indices();
            if !activated.is_empty() {
                let qh = microscope_memory::hebbian::query_hash(&format!(
                    "radial:{:.3},{:.3},{:.3}",
                    x, y, z
                ));
                hebb.record_activation(&activated, qh);
                let _ = hebb.save(output_dir);
            }
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
        Cmd::Fingerprint => {
            let t0 = Instant::now();
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            println!(
                "{} {} blocks...",
                "FINGERPRINT".cyan().bold(),
                reader.block_count
            );

            let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();
            let table = microscope_memory::fingerprint::LinkTable::build(&texts);
            table.save(output_dir).expect("save fingerprints");

            let stats = table.stats();
            println!("  Avg entropy:        {:.3}", stats.avg_entropy);
            println!("  Unique hashes:      {}", stats.unique_hashes);
            println!("  Largest cluster:    {}", stats.largest_cluster);
            println!("  Structural links:   {}", stats.link_count);
            println!("  {:.0} ms", t0.elapsed().as_millis());
        }
        Cmd::Links { block_index } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let table = microscope_memory::fingerprint::LinkTable::load(output_dir);

            match table {
                Some(t) => {
                    let links = t.linked_blocks(block_index as u32);
                    let h = reader.header(block_index);
                    let text = reader.text(block_index);
                    let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                    println!(
                        "{} Block #{} D{} [{}] {}",
                        "LINKS".cyan().bold(),
                        block_index,
                        h.depth,
                        layer,
                        safe_truncate(text, 50)
                    );

                    if links.is_empty() {
                        println!("  (no structural links)");
                    } else {
                        println!("  {} wormholes:", links.len());
                        for (target, sim) in &links {
                            let th = reader.header(*target as usize);
                            let tt = reader.text(*target as usize);
                            let tl = LAYER_NAMES.get(th.layer_id as usize).unwrap_or(&"?");
                            println!(
                                "    -> #{} {} {} sim={:.3} {}",
                                target,
                                format!("D{}", th.depth).cyan(),
                                format!("[{}]", tl).green(),
                                sim,
                                safe_truncate(tt, 50)
                            );
                        }
                    }
                }
                None => {
                    println!(
                        "  {} fingerprints.idx not found — run 'fingerprint' first",
                        "ERR".red()
                    );
                }
            }
        }
        Cmd::Similar { text, k } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let table = microscope_memory::fingerprint::LinkTable::load(output_dir);

            match table {
                Some(t) => {
                    let results = t.find_similar(&text, k);
                    println!(
                        "{} '{}' ({} results):",
                        "SIMILAR".cyan().bold(),
                        safe_truncate(&text, 40),
                        results.len()
                    );
                    for (idx, sim) in &results {
                        let h = reader.header(*idx as usize);
                        let bt = reader.text(*idx as usize);
                        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                        println!(
                            "  #{} {} {} sim={:.3} {}",
                            idx,
                            format!("D{}", h.depth).cyan(),
                            format!("[{}]", layer).green(),
                            sim,
                            safe_truncate(bt, 50)
                        );
                    }
                }
                None => {
                    println!(
                        "  {} fingerprints.idx not found — run 'fingerprint' first",
                        "ERR".red()
                    );
                }
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
        Cmd::Hebbian => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let stats = hebb.stats();
            println!("{}", "HEBBIAN STATE".cyan().bold());
            println!("  Blocks:             {}", stats.block_count);
            println!("  Active blocks:      {}", stats.active_blocks);
            println!("  Total activations:  {}", stats.total_activations);
            println!("  Hot blocks (>0.1):  {}", stats.hot_blocks);
            println!("  Drifted blocks:     {}", stats.drifted_blocks);
            println!("  Co-activation pairs:{}", stats.coactivation_pairs);
            println!("  Fingerprints:       {}", stats.fingerprint_count);

            let top = hebb.strongest_pairs(5);
            if !top.is_empty() {
                println!("\n  Strongest co-activations:");
                for pair in top {
                    let text_a = safe_truncate(reader.text(pair.block_a as usize), 30);
                    let text_b = safe_truncate(reader.text(pair.block_b as usize), 30);
                    println!("    {}x  [{}] <-> [{}]", pair.count, text_a, text_b);
                }
            }
        }
        Cmd::HebbianDrift => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );

            let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
                .map(|i| {
                    let h = reader.header(i);
                    (h.x, h.y, h.z)
                })
                .collect();

            let before_drifted = hebb.stats().drifted_blocks;
            hebb.apply_drift(&headers);
            let after_drifted = hebb.stats().drifted_blocks;

            hebb.save(output_dir).expect("save Hebbian state");
            println!(
                "{} Drift applied ({} -> {} drifted blocks)",
                "HEBBIAN".cyan().bold(),
                before_drifted,
                after_drifted
            );
        }
        Cmd::Hottest { k } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let hot = hebb.hottest_blocks(k);

            println!("{} top {} blocks:", "HOTTEST".cyan().bold(), k);
            if hot.is_empty() {
                println!("  (no active blocks — run some queries first)");
            }
            for (idx, energy) in &hot {
                let h = reader.header(*idx);
                let text = reader.text(*idx);
                let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                let rec = &hebb.activations[*idx];
                println!(
                    "  {} {} {} count={} drift=({:.3},{:.3},{:.3}) {}",
                    format!("E={:.3}", energy).yellow(),
                    format!("D{}", h.depth).cyan(),
                    format!("[{}]", layer).green(),
                    rec.activation_count,
                    rec.drift_x,
                    rec.drift_y,
                    rec.drift_z,
                    safe_truncate(text, 50)
                );
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
        Cmd::PulseExchange => {
            println!(
                "{} across {} indices...",
                "PULSE EXCHANGE".magenta().bold(),
                config.federation.indices.len()
            );
            match microscope_memory::federation::exchange_pulses(&config) {
                Ok(count) => {
                    println!("  {} pulses exchanged", count);
                }
                Err(e) => {
                    eprintln!("  {} {}", "ERR".red(), e);
                }
            }
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
        Cmd::Archetypes => {
            let output_dir = Path::new(&config.paths.output_dir);
            let arc = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
            let stats = arc.stats();
            println!("{}", "ARCHETYPES".cyan().bold());
            println!("  Emerged:            {}", stats.archetype_count);
            println!("  Total members:      {}", stats.total_members);
            if let (Some(label), Some(str)) = (&stats.strongest_label, stats.strongest_strength) {
                println!("  Strongest:          '{}' (str={:.3})", label, str);
            }

            if !arc.archetypes.is_empty() {
                println!();
                for a in &arc.archetypes {
                    println!(
                        "  #{} '{}' str={:.3} members={} reinforced={}x ({:.2},{:.2},{:.2})",
                        a.id,
                        a.label,
                        a.strength,
                        a.members.len(),
                        a.reinforcement_count,
                        a.centroid.0,
                        a.centroid.1,
                        a.centroid.2,
                    );
                }
            }
        }
        Cmd::Emerge => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );

            let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
                .map(|i| {
                    let h = reader.header(i);
                    (h.x, h.y, h.z)
                })
                .collect();
            let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();

            let mut arc = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
            let emerged = arc.detect(&resonance, &hebb, &headers, &texts);
            arc.decay();
            arc.save(output_dir).expect("save archetypes");

            println!(
                "{} {} new archetypes emerged ({} total)",
                "EMERGE".cyan().bold(),
                emerged,
                arc.archetypes.len()
            );
            for a in arc.archetypes.iter().rev().take(5) {
                println!(
                    "  #{} '{}' str={:.3} members={}",
                    a.id,
                    a.label,
                    a.strength,
                    a.members.len()
                );
            }
        }
        Cmd::Resonance => {
            let output_dir = Path::new(&config.paths.output_dir);
            let resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
            let stats = resonance.stats();
            println!("{}", "RESONANCE PROTOCOL".magenta().bold());
            println!("  Instance ID:        {:x}", stats.instance_id);
            println!("  Outgoing pulses:    {}", stats.outgoing_pulses);
            println!("  Incoming pulses:    {}", stats.incoming_pulses);
            println!("  Pending integration:{}", stats.pending_integration);
            println!("  Unique sources:     {}", stats.unique_sources);
            println!("  Field cells:        {}", stats.field_cells);
            println!("  Field energy:       {:.3}", stats.field_energy);

            if !resonance.outgoing.is_empty() {
                println!("\n  Recent outgoing:");
                for p in resonance.outgoing.iter().rev().take(5) {
                    println!(
                        "    str={:.3} blocks={} layer={} hash={:x}",
                        p.strength,
                        p.activations.len(),
                        p.layer_hint,
                        p.query_hash,
                    );
                }
            }
        }
        Cmd::Integrate => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let mut resonance =
                microscope_memory::resonance::ResonanceState::load_or_init(output_dir);

            let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
                .map(|i| {
                    let h = reader.header(i);
                    (h.x, h.y, h.z)
                })
                .collect();

            let influenced = resonance.integrate_into_hebbian(&mut hebb, &headers, 0.05);
            resonance.decay_field(0.95);
            resonance.expire_pulses();

            hebb.save(output_dir).expect("save Hebbian");
            resonance.save(output_dir).expect("save resonance");

            println!(
                "{} {} blocks influenced by resonance pulses",
                "INTEGRATE".magenta().bold(),
                influenced
            );
        }
        Cmd::Mirror => {
            let output_dir = Path::new(&config.paths.output_dir);
            let mirror = microscope_memory::mirror::MirrorState::load_or_init(output_dir);
            let stats = mirror.stats();
            println!("{}", "MIRROR NEURON STATE".magenta().bold());
            println!("  Resonance echoes:   {}", stats.total_echoes);
            println!("  Resonant blocks:    {}", stats.resonant_blocks);
            println!("  Avg similarity:     {:.3}", stats.avg_similarity);
            if let Some((idx, strength)) = stats.strongest_block {
                let reader = open_reader(&config);
                let text = reader.text(idx as usize);
                println!(
                    "  Strongest:          block {} (str={:.3}) {}",
                    idx,
                    strength,
                    safe_truncate(text, 50)
                );
            }

            if !mirror.echoes.is_empty() {
                println!("\n  Recent echoes:");
                for echo in mirror.echoes.iter().rev().take(5) {
                    println!(
                        "    sim={:.3} shared={} blocks  trigger={:x} echo={:x}",
                        echo.similarity,
                        echo.shared_blocks.len(),
                        echo.trigger_hash,
                        echo.echo_hash,
                    );
                }
            }
        }
        Cmd::Resonant { k } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mirror = microscope_memory::mirror::MirrorState::load_or_init(output_dir);
            let top = mirror.most_resonant(k);

            println!("{} top {} blocks:", "RESONANT".magenta().bold(), k);
            if top.is_empty() {
                println!("  (no resonant blocks — run queries to build mirror state)");
            }
            for (idx, res) in &top {
                let h = reader.header(*idx as usize);
                let text = reader.text(*idx as usize);
                let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                println!(
                    "  {} {} {} echoes={} {}",
                    format!("S={:.3}", res.strength).magenta(),
                    format!("D{}", h.depth).cyan(),
                    format!("[{}]", layer).green(),
                    res.echo_count,
                    safe_truncate(text, 50)
                );
            }
        }
        Cmd::Viz { output } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let mirror = microscope_memory::mirror::MirrorState::load_or_init(output_dir);
            let resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
            let archetypes = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);

            let dest = Path::new(&output);
            microscope_memory::viz::export_to_file(
                output_dir,
                &reader,
                &hebb,
                &mirror,
                &resonance,
                &archetypes,
                dest,
            )
            .expect("export viz");

            let hebb_stats = hebb.stats();
            let arc_stats = archetypes.stats();
            println!(
                "{} {} blocks, {} edges, {} archetypes -> {}",
                "VIZ".cyan().bold(),
                reader.block_count,
                hebb_stats.coactivation_pairs,
                arc_stats.archetype_count,
                output
            );
        }
        Cmd::Density { output, grid } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );

            let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
                .map(|i| {
                    let h = reader.header(i);
                    (h.x, h.y, h.z)
                })
                .collect();

            let data = microscope_memory::viz::export_density_map(&hebb, &headers, grid);
            fs::write(&output, &data).expect("write density map");
            println!(
                "{} {}³ grid ({} bytes) -> {}",
                "DENSITY".cyan().bold(),
                grid,
                data.len(),
                output
            );
        }
        Cmd::Mcp => {
            microscope_memory::mcp::run(config);
        }

        Cmd::Patterns { k } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let tg = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let stats = tg.stats();
            println!("{}", "THOUGHT GRAPH".cyan().bold());
            println!(
                "  nodes={} edges={} patterns={} (crystallized={}) session=#{}",
                stats.node_count,
                stats.edge_count,
                stats.pattern_count,
                stats.crystallized,
                stats.current_session_id
            );

            let top = tg.top_patterns(k);
            if top.is_empty() {
                println!("  (no patterns yet — recall more to form thought paths)");
            } else {
                println!("\n  {}", "Top patterns:".yellow());
                for (i, p) in top.iter().enumerate() {
                    let seq_str: Vec<String> = p
                        .sequence
                        .iter()
                        .map(|h| format!("{:04x}", h & 0xFFFF))
                        .collect();
                    let crystallized = if p.frequency >= 3 { "*" } else { " " };
                    println!(
                        "  {}#{} {} freq={} str={:.2} blocks={}",
                        crystallized,
                        i + 1,
                        seq_str.join(" → "),
                        p.frequency,
                        p.strength,
                        p.result_blocks.len()
                    );
                }
            }
        }

        Cmd::Paths { sessions } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let tg = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let recent = tg.recent_sessions(sessions);

            if recent.is_empty() {
                println!("  (no recall sessions recorded yet)");
            } else {
                println!("{}", "THOUGHT PATHS".cyan().bold());
                for (si, session) in recent.iter().enumerate() {
                    if let Some(first) = session.first() {
                        println!(
                            "\n  {} Session #{} ({} recalls):",
                            "▸".green(),
                            first.session_id,
                            session.len()
                        );
                        let path_str: Vec<String> = session
                            .iter()
                            .map(|n| format!("{:04x}", n.query_hash & 0xFFFF))
                            .collect();
                        println!("    {}", path_str.join(" → "));
                    }
                    if si >= sessions {
                        break;
                    }
                }
            }
        }

        Cmd::Predictions => {
            let output_dir = Path::new(&config.paths.output_dir);
            let cache =
                microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir);
            let stats = &cache.stats;
            println!("{}", "PREDICTIVE CACHE".cyan().bold());
            println!(
                "  predictions={} hits={} misses={} partial={} hit_rate={:.1}%",
                stats.total_predictions,
                stats.total_hits,
                stats.total_misses,
                stats.total_partial_hits,
                stats.hit_rate() * 100.0
            );
            println!(
                "  active={} avg_confidence={:.1}%",
                stats.current_predictions,
                stats.avg_confidence * 100.0
            );

            if !cache.predictions.is_empty() {
                println!("\n  {}", "Active predictions:".yellow());
                for (i, p) in cache.predictions.iter().enumerate() {
                    println!(
                        "  #{} hash={:04x} blocks={} conf={:.0}% pattern=#{}",
                        i + 1,
                        p.predicted_query_hash & 0xFFFF,
                        p.blocks.len(),
                        p.confidence * 100.0,
                        p.pattern_id
                    );
                }
            }
        }

        Cmd::TemporalPatterns => {
            let output_dir = Path::new(&config.paths.output_dir);
            let temporal =
                microscope_memory::temporal_archetype::TemporalArchetypeState::load_or_init(
                    output_dir,
                );
            let window = microscope_memory::temporal_archetype::current_time_window();
            println!(
                "{} (current window: {})",
                "TEMPORAL ARCHETYPES".cyan().bold(),
                microscope_memory::temporal_archetype::WINDOW_LABELS[window]
            );

            if temporal.profiles.is_empty() {
                println!("  (no temporal data yet — recall with archetype matches to build profiles)");
            } else {
                for p in &temporal.profiles {
                    let dominant = p
                        .dominant_window()
                        .map(|w| microscope_memory::temporal_archetype::WINDOW_LABELS[w])
                        .unwrap_or("?");
                    println!(
                        "\n  Archetype #{} (total={}, dominant={})",
                        p.archetype_id, p.total_activations, dominant
                    );
                    for (i, label) in
                        microscope_memory::temporal_archetype::WINDOW_LABELS.iter().enumerate()
                    {
                        let bar_len = (p.window_weights[i] * 5.0) as usize;
                        let bar: String = "█".repeat(bar_len);
                        let marker = if i == window { " ◀" } else { "" };
                        println!(
                            "    {} {:>3} {:.1} {}{}",
                            label, p.window_counts[i], p.window_weights[i], bar, marker
                        );
                    }
                }
            }
        }

        Cmd::Attention => {
            let output_dir = Path::new(&config.paths.output_dir);
            let attn_state =
                microscope_memory::attention::AttentionState::load_or_init(output_dir);
            println!("{}", "ATTENTION".cyan().bold());
            println!(
                "  total_recalls={} history={}",
                attn_state.total_recalls,
                attn_state.history.len()
            );

            println!("\n  {}", "Learned layer weights:".yellow());
            for (i, name) in microscope_memory::attention::LAYER_NAMES.iter().enumerate() {
                let w = attn_state.learned_weights[i];
                let bar_len = (w * 10.0) as usize;
                let bar: String = "█".repeat(bar_len.min(30));
                println!("    {:<16} {:.3} {}", name, w, bar);
            }

            if !attn_state.history.is_empty() {
                let recent: Vec<&microscope_memory::attention::AttentionOutcome> =
                    attn_state.history.iter().rev().take(5).collect();
                println!("\n  {}", "Recent outcomes:".yellow());
                for o in recent {
                    let symbol = if o.quality >= 0.7 {
                        "+".green()
                    } else if o.quality <= 0.3 {
                        "-".red()
                    } else {
                        "~".yellow()
                    };
                    println!("    {} quality={:.2}", symbol, o.quality);
                }
            }
        }

        Cmd::PatternExchange => {
            let output_dir = Path::new(&config.paths.output_dir);
            match microscope_memory::federation::exchange_patterns(&config) {
                Ok(count) => {
                    println!(
                        "{} exchanged {} patterns",
                        "PATTERN EXCHANGE".cyan().bold(),
                        count
                    );
                }
                Err(e) => {
                    println!("{} {}", "ERROR:".red(), e);
                }
            }
            let _ = output_dir;
        }
        Cmd::Dream => {
            let output_dir = Path::new(&config.paths.output_dir);
            let reader = open_reader(&config);
            println!("{}", "DREAM CONSOLIDATION".cyan().bold());
            match microscope_memory::dream::dream_consolidate(output_dir, reader.block_count) {
                Ok(cycle) => {
                    let mut dream_state = microscope_memory::dream::DreamState::load_or_init(output_dir);
                    dream_state.last_dream_ms = cycle.timestamp_ms;
                    dream_state.cycles.push(cycle.clone());
                    if dream_state.cycles.len() > 200 {
                        dream_state.cycles.drain(0..dream_state.cycles.len() - 200);
                    }
                    let _ = dream_state.save(output_dir);
                    println!("  Duration:      {} ms", cycle.duration_ms);
                    println!("  Replayed:      {} fingerprints", cycle.replayed_fingerprints);
                    println!("  Strengthened:  {} pairs", cycle.strengthened_pairs);
                    println!("  Pruned pairs:  {}", cycle.pruned_pairs);
                    println!("  Pruned blocks: {}", cycle.pruned_activations);
                    println!("  Patterns:      +{}", cycle.consolidated_patterns);
                    println!("  Energy:        {:.1} → {:.1}", cycle.energy_before, cycle.energy_after);
                }
                Err(e) => println!("{} {}", "ERROR:".red(), e),
            }
        }
        Cmd::DreamLog { k } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let state = microscope_memory::dream::DreamState::load_or_init(output_dir);
            let stats = state.stats();
            println!("{}", "DREAM LOG".cyan().bold());
            println!("  Total cycles:  {}", stats.total_cycles);
            println!("  Total pruned:  {} pairs, {} activations", stats.total_pruned_pairs, stats.total_pruned_activations);
            println!("  Total strengthened: {} pairs", stats.total_strengthened);
            println!("  Total replayed: {} fingerprints", stats.total_replayed);
            if !state.cycles.is_empty() {
                println!("\n  Recent cycles:");
                let start = if state.cycles.len() > k { state.cycles.len() - k } else { 0 };
                for cycle in &state.cycles[start..] {
                    println!("    {} — {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}",
                        cycle.timestamp_ms, cycle.duration_ms,
                        cycle.replayed_fingerprints, cycle.strengthened_pairs,
                        cycle.pruned_pairs, cycle.pruned_activations, cycle.consolidated_patterns);
                }
            }
        }
        Cmd::EmotionalField => {
            let output_dir = Path::new(&config.paths.output_dir);
            let state = microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(output_dir);
            let stats = state.stats();
            println!("{}", "EMOTIONAL FIELD".cyan().bold());
            println!("  Instance ID:  {:016x}", stats.instance_id);
            println!("  Local field:  {}", if stats.has_local { "active" } else { "inactive" });
            println!("  Local energy: {:.2}", stats.local_energy);
            println!("  Local valence: {:.2}", stats.local_valence);
            println!("  Remote fields: {}", stats.remote_count);
            println!("  Blended valence: {:.2}", stats.blended_valence);
            if let Some((cx, cy, cz)) = state.blended_centroid(0.7) {
                println!("  Blended centroid: ({:.3}, {:.3}, {:.3})", cx, cy, cz);
            }
        }
        Cmd::EmotionalExchange => {
            let output_dir = Path::new(&config.paths.output_dir);
            let reader = open_reader(&config);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
            let mut local = microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(output_dir);
            local.capture_local(&reader, &hebb);

            let mut exchanged = 0usize;
            for idx_config in &config.federation.indices {
                if let Ok(idx_cfg) = microscope_memory::config::Config::load(&idx_config.config_path) {
                    let idx_dir = Path::new(&idx_cfg.paths.output_dir);
                    let mut remote = microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(idx_dir);

                    // Send ours to them
                    let our_wire = local.export_snapshot();
                    if let Some(snap) = microscope_memory::emotional_contagion::EmotionalContagionState::import_snapshot(&our_wire) {
                        remote.receive_remote(snap);
                        exchanged += 1;
                    }

                    // Receive theirs
                    let their_wire = remote.export_snapshot();
                    if let Some(snap) = microscope_memory::emotional_contagion::EmotionalContagionState::import_snapshot(&their_wire) {
                        local.receive_remote(snap);
                        exchanged += 1;
                    }

                    let _ = remote.save(idx_dir);
                }
            }

            let _ = local.save(output_dir);
            println!("{} exchanged {} emotional snapshots", "EMOTIONAL EXCHANGE".cyan().bold(), exchanged);
        }
        Cmd::Modalities => {
            let output_dir = Path::new(&config.paths.output_dir);
            let index = microscope_memory::multimodal::ModalityIndex::load_or_init(output_dir);
            let stats = index.stats();
            println!("{}", "MULTIMODAL INDEX".cyan().bold());
            println!("  Total entries: {}", stats.total_entries);
            println!("  Text:          {}", stats.text_count);
            println!("  Image:         {}", stats.image_count);
            println!("  Audio:         {}", stats.audio_count);
            println!("  Structured:    {}", stats.structured_count);
        }
        Cmd::CognitiveMap { output } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let mirror = microscope_memory::mirror::MirrorState::load_or_init(output_dir);
            let resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
            let archetypes = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
            let thought_graph =
                microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let pred_cache =
                microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir);
            let temporal =
                microscope_memory::temporal_archetype::TemporalArchetypeState::load_or_init(
                    output_dir,
                );
            let attention =
                microscope_memory::attention::AttentionState::load_or_init(output_dir);
            let dream = microscope_memory::dream::DreamState::load_or_init(output_dir);
            let emotional =
                microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(
                    output_dir,
                );
            let modalities =
                microscope_memory::multimodal::ModalityIndex::load_or_init(output_dir);

            let dest = Path::new(&output);
            microscope_memory::viz::export_cognitive_map_to_file(
                output_dir,
                &reader,
                &hebb,
                &mirror,
                &resonance,
                &archetypes,
                &thought_graph,
                &pred_cache,
                &temporal,
                &attention,
                &dream,
                &emotional,
                &modalities,
                dest,
            )
            .expect("export cognitive map");

            let file_size = std::fs::metadata(dest)
                .map(|m| m.len())
                .unwrap_or(0);
            println!(
                "{} 13-layer cognitive map → {} ({} bytes)",
                "COGNITIVE MAP".cyan().bold(),
                output,
                file_size
            );

            // Copy viewer.html next to the JSON and open in browser
            let json_dir = dest.parent().unwrap_or(Path::new("."));
            let viewer_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("viewer.html");
            let viewer_dst = json_dir.join("viewer.html");
            if viewer_src.exists() {
                let _ = std::fs::copy(&viewer_src, &viewer_dst);
            }
            if viewer_dst.exists() {
                println!(
                    "{}",
                    "Opening viewer in browser...".green()
                );
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &viewer_dst.display().to_string()])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open")
                        .arg(&viewer_dst)
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&viewer_dst)
                        .spawn();
                }
            }
        }
        Cmd::StoreData { pairs, importance } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let mut fields = Vec::new();
            for pair in &pairs {
                if let Some((k, v)) = pair.split_once('=') {
                    let value = if let Ok(i) = v.parse::<i64>() {
                        microscope_memory::multimodal::FieldValue::Int(i)
                    } else if let Ok(f) = v.parse::<f64>() {
                        microscope_memory::multimodal::FieldValue::Float(f)
                    } else if v == "true" || v == "false" {
                        microscope_memory::multimodal::FieldValue::Bool(v == "true")
                    } else {
                        microscope_memory::multimodal::FieldValue::Str(v.to_string())
                    };
                    fields.push((k.to_string(), value));
                }
            }
            if fields.is_empty() {
                println!("{} no valid key=value pairs", "ERROR:".red());
                return;
            }

            // Create text representation and store as memory
            let text_repr: String = fields.iter().map(|(k, v)| format!("DAT:{}={:?}", k, v)).collect::<Vec<_>>().join(" ");
            let text_short = if text_repr.len() > 200 { &text_repr[..200] } else { &text_repr };
            let _ = store_memory(&config, text_short, "rust_state", importance);

            // Register in multimodal index
            let mut index = microscope_memory::multimodal::ModalityIndex::load_or_init(output_dir);
            let block_idx = index.entries.len() as u32 + 1_000_000; // virtual idx for append entries
            index.register(block_idx, microscope_memory::multimodal::Modality::Structured(
                microscope_memory::multimodal::StructuredMeta { fields: fields.clone() }
            ));
            let _ = index.save(output_dir);

            println!("{} stored {} fields as structured data", "STORE-DATA".green().bold(), fields.len());
        }

        // ─── New user-facing commands ────────────────────

        Cmd::Watch { dir, interval } => {
            microscope_memory::watch::watch_directory(&config, &dir, interval);
        }
        Cmd::Scan { dir } => {
            let count = microscope_memory::watch::scan_and_store(&config, &dir);
            println!("{} {} files indexed", "SCAN".green().bold(), count);
        }
        Cmd::Insights => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let report = microscope_memory::insights::generate_insights(output_dir, &reader);

            println!("\n{}", "INSIGHTS".cyan().bold());
            println!("{}", "=".repeat(60));
            println!(
                "  Blocks: {}  |  Activations: {}  |  Archetypes: {}  |  Patterns: {}",
                report.total_blocks,
                report.total_activations,
                report.total_archetypes,
                report.active_patterns
            );
            println!("{}", "-".repeat(60));

            if report.insights.is_empty() {
                println!("  No insights yet — use the system more to generate patterns.");
            }

            for insight in &report.insights {
                let cat_label = match insight.category {
                    microscope_memory::insights::InsightCategory::TopTheme => "THEME",
                    microscope_memory::insights::InsightCategory::CoOccurrence => "LINKED",
                    microscope_memory::insights::InsightCategory::TemporalPattern => "TIME",
                    microscope_memory::insights::InsightCategory::EmotionalTrend => "EMOTION",
                    microscope_memory::insights::InsightCategory::FadingMemory => "FADING",
                    microscope_memory::insights::InsightCategory::EmergingTheme => "EMERGING",
                    microscope_memory::insights::InsightCategory::DreamPattern => "DREAM",
                    microscope_memory::insights::InsightCategory::AttentionBias => "ATTENTION",
                };

                let strength_bar = "#".repeat((insight.strength * 10.0) as usize);
                println!(
                    "\n  {} [{}] {} ({})",
                    format!("[{:>8}]", cat_label).cyan(),
                    strength_bar.yellow(),
                    insight.title.bold(),
                    format!("{:.0}%", insight.strength * 100.0).dimmed()
                );
                println!("  {}", insight.description.dimmed());
                for ev in &insight.evidence {
                    println!("    {} {}", ">".green(), ev);
                }
            }
            println!("\n{}", "=".repeat(60));
        }
        Cmd::Morning => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let brief = microscope_memory::morning::generate_morning(output_dir, &reader);

            println!("\n{}", "MORNING BRIEF".cyan().bold());
            println!("{}", "=".repeat(60));
            println!("  {} — Time window: {}", brief.greeting.bold(), brief.current_window.cyan());
            println!("{}", "-".repeat(60));

            // Relevant now
            if !brief.relevant_now.is_empty() {
                println!("\n  {}", "Relevant right now:".bold());
                for item in &brief.relevant_now {
                    let bar = "#".repeat((item.temporal_boost * 10.0) as usize);
                    println!(
                        "    {} {} ({} members, boost={:.0}%)",
                        bar.cyan(),
                        item.archetype_label,
                        item.member_count,
                        item.temporal_boost * 100.0
                    );
                }
            }

            // Spaced repetition
            if !brief.spaced_repetition.is_empty() {
                println!("\n  {}", "Remember — from your past:".bold());
                for item in &brief.spaced_repetition {
                    println!(
                        "    {} {}",
                        format!("[{}]", item.interval_label).yellow(),
                        item.text_preview
                    );
                }
            }

            // Emotional forecast
            if let Some(ref forecast) = brief.emotional_forecast {
                println!("\n  {}", "Emotional forecast:".bold());
                println!(
                    "    {} (valence={:.2}, energy={:.2})",
                    forecast.predicted_mood, forecast.valence, forecast.energy
                );
                println!("    {}", forecast.basis.dimmed());
            }

            // Emerging themes
            if !brief.emerging_themes.is_empty() {
                println!("\n  {}", "Emerging themes:".bold());
                for theme in &brief.emerging_themes {
                    println!("    {} {}", ">".green(), theme);
                }
            }

            // Dream summary
            if let Some(ref dream) = brief.dream_summary {
                println!("\n  {}", "Dream consolidation:".bold());
                println!("    {}", dream);
            }

            // Action items
            if !brief.action_items.is_empty() {
                println!("\n  {}", "Action items:".bold());
                for action in &brief.action_items {
                    println!("    {} {}", "!".red().bold(), action);
                }
            }

            println!("\n{}", "=".repeat(60));
        }
        Cmd::Timeline { topic, limit } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let report = microscope_memory::timeline::generate_timeline(output_dir, &reader, &config, &topic, limit);

            println!("\n{} '{}'", "TIMELINE".cyan().bold(), topic);
            println!("{}", "=".repeat(60));
            println!("  Total mentions: {}", report.total_mentions);

            if let (Some(first), Some(last)) = (report.first_mention_ms, report.last_mention_ms) {
                if first > 0 && last > 0 {
                    let span_days = (last - first) / 86_400_000;
                    println!("  Time span: {} days", span_days);
                }
            }
            println!("{}", "-".repeat(60));

            if report.points.is_empty() {
                println!("  No memories found matching '{}'.", topic);
            } else {
                println!("\n  {}", "Chronological entries:".bold());
                for (i, point) in report.points.iter().enumerate() {
                    let preview: String = point.text.chars().take(70).filter(|&c| c != '\n').collect();
                    let src = if point.is_append { "APPEND" } else { "INDEX" };
                    let energy_bar = if point.energy > 0.0 {
                        format!(" [{}]", "#".repeat((point.energy * 5.0) as usize).yellow())
                    } else {
                        String::new()
                    };
                    println!(
                        "    {:>3}. {} D{} {} {}{}",
                        i + 1,
                        format!("[{}]", point.layer).green(),
                        point.depth,
                        format!("({})", src).dimmed(),
                        preview,
                        energy_bar
                    );
                }
            }

            // Related topics
            if !report.related_topics.is_empty() {
                println!("\n  {}", "Related topics:".bold());
                for (word, count) in &report.related_topics {
                    println!("    {} ({}x)", word, count);
                }
            }

            // Turning points
            if !report.turning_points.is_empty() {
                println!("\n  {}", "Turning points:".bold());
                for tp in &report.turning_points {
                    println!("    {} {}", ">".red(), tp.description);
                }
            }

            // Emotional shifts
            if !report.emotional_shifts.is_empty() {
                println!("\n  {}", "Emotional shifts:".bold());
                for shift in &report.emotional_shifts {
                    println!(
                        "    {} ({:.2} -> {:.2})",
                        shift.description, shift.from_valence, shift.to_valence
                    );
                }
            }

            println!("\n{}", "=".repeat(60));
        }
        Cmd::Prove { query, output } => {
            let reader = open_reader(&config);
            let verification = microscope_memory::prove::prove_memory(&config, &reader, &query);

            let status_color = match verification.integrity {
                microscope_memory::prove::IntegrityStatus::Verified => "VERIFIED".green().bold(),
                microscope_memory::prove::IntegrityStatus::CrcOnly => "CRC ONLY".yellow().bold(),
                microscope_memory::prove::IntegrityStatus::Failed => "FAILED".red().bold(),
                microscope_memory::prove::IntegrityStatus::NotFound => "NOT FOUND".red().bold(),
            };

            println!("\n{} {}", "PROVE".cyan().bold(), status_color);
            println!("{}", "=".repeat(60));
            println!("{}", verification.human_readable);

            if let Some(output_path) = output {
                let json = microscope_memory::prove::export_proof_json(&verification);
                std::fs::write(&output_path, &json).expect("Failed to write proof");
                println!("\n  {} Proof exported to: {}", "SAVED".green(), output_path);
            }

            println!("{}", "=".repeat(60));
        }

        // ─── Per-depth mmap cache ────────────────────────

        Cmd::CachedRecall { depth } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let cache = microscope_memory::depth_cache::DepthCache::open(output_dir)
                .expect("Failed to open depth cache");

            let t0 = Instant::now();
            let results = cache.read_depth(depth);
            let elapsed_ns = t0.elapsed().as_nanos();

            println!("{} D{} ({} ns)", "CACHED-RECALL".cyan().bold(), depth, elapsed_ns);

            if results.results.is_empty() {
                println!("  (empty — run 'cache-warm' first)");
            } else {
                for (i, r) in results.results.iter().enumerate() {
                    println!("  {} #{} dist={:.5} e={:.2} [L{}] {}",
                        format!("[{}]", i).green(),
                        r.block_idx, r.distance, r.energy, r.layer_id,
                        r.text);
                }
            }
            println!("  {} {} ns", "TIME:".yellow(), elapsed_ns);
        }
        Cmd::CacheWarm { query } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);

            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir, reader.block_count,
            );

            println!("{} '{}'", "CACHE-WARM".cyan().bold(), query);

            let (qx, qy, qz) = content_coords_blended(&query, "long_term", config.search.semantic_weight);
            let q_lower = query.to_lowercase();
            let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

            let mut depth_results: Vec<(u8, Vec<microscope_memory::depth_cache::CachedResult>)> = Vec::new();

            for zoom in 0..9u8 {
                let (start, count) = reader.depth_ranges[zoom as usize];
                let (start, count) = (start as usize, count as usize);

                let mut results: Vec<(f32, usize)> = Vec::new();

                for i in start..(start + count) {
                    let h = reader.header(i);
                    let dx = h.x - qx;
                    let dy = h.y - qy;
                    let dz = h.z - qz;
                    let dist = dx * dx + dy * dy + dz * dz;

                    let text = reader.text(i).to_lowercase();
                    let kw_hits = keywords.iter().filter(|&&kw| text.contains(kw)).count();
                    let boost = kw_hits as f32 * 0.1;

                    if dist < 0.1 || kw_hits > 0 {
                        results.push(((dist - boost).max(0.0), i));
                    }
                }

                results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(5);

                let cached: Vec<microscope_memory::depth_cache::CachedResult> = results
                    .iter()
                    .map(|(dist, idx)| {
                        let h = reader.header(*idx);
                        let text = reader.text(*idx);
                        let preview: String = text.chars().take(64).filter(|&c| c != '\n').collect();
                        let energy = if *idx < hebb.activations.len() {
                            hebb.activations[*idx].energy
                        } else {
                            0.0
                        };

                        microscope_memory::depth_cache::CachedResult {
                            block_idx: *idx as u32,
                            distance: *dist,
                            energy,
                            layer_id: h.layer_id,
                            depth: h.depth,
                            text: preview,
                            x: h.x,
                        }
                    })
                    .collect();

                let cached_count = cached.len();
                depth_results.push((zoom, cached));
                println!("  D{}: {} results cached", zoom, cached_count);
            }

            let mut cache = microscope_memory::depth_cache::DepthCache::open(output_dir)
                .expect("Failed to open depth cache");

            let qh = microscope_memory::hebbian::query_hash(&query);
            cache.write_recall(qh, &depth_results).expect("Failed to write cache");
            println!("{} All 9 depths cached", "DONE".green().bold());
        }
        Cmd::CacheStatus => {
            let output_dir = Path::new(&config.paths.output_dir);
            let cache = microscope_memory::depth_cache::DepthCache::open(output_dir)
                .expect("Failed to open depth cache");

            let hdr = cache.header();

            println!("\n{}", "DEPTH CACHE STATUS".cyan().bold());
            println!("{}", "=".repeat(50));
            println!("  Valid:     {}", if cache.is_valid() { "YES" } else { "NO" });
            println!("  Write seq: {}", hdr.write_seq);
            println!("  Query:     0x{:016x}", hdr.last_query_hash);
            println!("  Timestamp: {}", hdr.timestamp_ms);
            println!();

            for d in 0..9u8 {
                let results = cache.read_depth(d);
                if results.results.is_empty() {
                    println!("  D{}: (empty)", d);
                } else {
                    println!("  D{}: {} results, best=#{} dist={:.5}",
                        d, results.results.len(),
                        results.results[0].block_idx,
                        results.results[0].distance);
                }
            }
            println!("{}", "=".repeat(50));
        }
        Cmd::CacheBench => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);

            // First ensure cache is populated
            let cache = microscope_memory::depth_cache::DepthCache::open(output_dir)
                .expect("Failed to open depth cache");

            if !cache.is_valid() || cache.header().write_seq == 0 {
                println!("  {} Cache empty — run 'cache-warm' first", "WARN:".yellow());
                return;
            }

            println!("{}", "CACHE BENCHMARK: cached vs normal recall".cyan().bold());
            println!("{}", "=".repeat(60));

            let iters = 10_000u64;

            // Benchmark cached reads per depth
            for depth in 0..9u8 {
                let t0 = Instant::now();
                for _ in 0..iters {
                    let r = cache.read_depth(depth);
                    std::hint::black_box(&r);
                }
                let ns = t0.elapsed().as_nanos() / iters as u128;

                // Normal recall for comparison
                let config_clone = config.clone();
                let t1 = Instant::now();
                let mut rng: u64 = 42;
                for _ in 0..iters.min(1000) {
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let fx = (rng >> 33) as f32 / (u32::MAX as f32) * 0.5;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let fy = (rng >> 33) as f32 / (u32::MAX as f32) * 0.5;
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let fz = (rng >> 33) as f32 / (u32::MAX as f32) * 0.5;
                    let r = reader.look(&config_clone, fx, fy, fz, depth, 5);
                    std::hint::black_box(&r);
                }
                let normal_ns = t1.elapsed().as_nanos() / iters.min(1000) as u128;

                let speedup = if ns > 0 { normal_ns / ns } else { 0 };
                println!("  D{}: cached={} ns  normal={} ns  {}x faster",
                    depth, ns, normal_ns,
                    format!("{}",speedup).green().bold());
            }
            println!("{}", "=".repeat(60));
        }

        // ─── Shared Memory (Ora IPC) ────────────────────

        Cmd::ShmWrite => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);

            let mut bridge = microscope_memory::shm::ShmBridge::open_default()
                .expect("Failed to open SHM");

            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir, reader.block_count,
            );
            let attention = microscope_memory::attention::AttentionState::load_or_init(output_dir);
            let archetypes = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);

            let hottest = hebb.activations.iter().enumerate()
                .max_by(|a, b| a.1.energy.partial_cmp(&b.1.energy).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i as u32)
                .unwrap_or(0);

            let active_arch = archetypes.archetypes.first().map(|a| a.id).unwrap_or(0);

            let emotional = microscope_memory::emotional::emotional_field(&reader, &hebb)
                .map(|f| f.total_energy)
                .unwrap_or(0.0);

            bridge.update_cognitive_state(
                0, hottest, active_arch,
                &attention.learned_weights,
                emotional,
                reader.block_count as u32,
            ).expect("Failed to write SHM");

            println!("{} Cognitive state -> SHM", "SHM-WRITE".green().bold());
            println!("  Hottest block: #{}", hottest);
            println!("  Total blocks:  {}", reader.block_count);
            println!("  Archetype:     #{}", active_arch);
        }
        Cmd::ShmRead => {
            let bridge = microscope_memory::shm::ShmBridge::open_default()
                .expect("Failed to open SHM");

            let slot = bridge.read_slot();

            println!("\n{}", "SHM-READ".cyan().bold());
            println!("{}", "=".repeat(50));
            println!("  Valid:   {}", if bridge.is_valid() { "YES" } else { "NO" });
            println!("  Query:   0x{:016x}", slot.last_query_hash);
            println!("  Block:   #{}", slot.last_recall_block);
            println!("  Arch:    #{}", slot.active_archetype_id);
            println!("  Attn:    [{:.2}, {:.2}, {:.2}, {:.2}]",
                slot.attention_weights[0], slot.attention_weights[1],
                slot.attention_weights[2], slot.attention_weights[3]);
            println!("  Valence: {:.3}", slot.emotional_valence);
            println!("  Blocks:  {}", slot.total_blocks);
            println!("  Seq:     {}", slot.write_seq);

            let ring = bridge.read_ring();
            if !ring.is_empty() {
                println!("\n  {} ({}):", "Ring".bold(), ring.len());
                for (i, e) in ring.iter().enumerate() {
                    println!("    [{}] #{} D{} '{}'", i, e.block_idx, e.depth, e.text_str());
                }
            }
            println!("{}", "=".repeat(50));
        }
        Cmd::ShmStatus => {
            match microscope_memory::shm::ShmBridge::open_default() {
                Ok(bridge) => {
                    let st = bridge.status();
                    println!("\n{}", "SHM STATUS".cyan().bold());
                    println!("  Path:     {}", microscope_memory::shm::DEFAULT_SHM_PATH);
                    println!("  Valid:    {}", if st.valid { "YES" } else { "NO" });
                    println!("  Ora seq:  {}", st.ora_write_seq);
                    println!("  Micro:    seq={} blocks={}", st.microscope_slot.write_seq, st.microscope_slot.total_blocks);
                    println!("  Ring:     {} entries", st.ring_entries);
                }
                Err(e) => {
                    println!("  {} Cannot open SHM: {}", "ERROR:".red(), e);
                }
            }
        }
        Cmd::ShmDaemon { shm_path, interval_ms } => {
            println!("{} SHM daemon @ {}ms (with predictive cache)", "DAEMON".cyan().bold(), interval_ms);

            let mut bridge = microscope_memory::shm::ShmBridge::open(&shm_path)
                .expect("Failed to open SHM");
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut last_seq = 0u32;
            let mut last_predictive_window: usize = 99; // force first prediction

            loop {
                let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                    output_dir, reader.block_count,
                );
                let attention = microscope_memory::attention::AttentionState::load_or_init(output_dir);
                let archetypes = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
                let temporal = microscope_memory::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);

                let hottest = hebb.activations.iter().enumerate()
                    .max_by(|a, b| a.1.energy.partial_cmp(&b.1.energy).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i as u32)
                    .unwrap_or(0);
                let active_arch = archetypes.archetypes.first().map(|a| a.id).unwrap_or(0);
                let emo = microscope_memory::emotional::emotional_field(&reader, &hebb)
                    .map(|f| f.total_energy).unwrap_or(0.0);

                let _ = bridge.update_cognitive_state(
                    0, hottest, active_arch, &attention.learned_weights, emo, reader.block_count as u32,
                );

                let slot = bridge.read_slot();
                if slot.write_seq != last_seq {
                    println!("  SYNC seq={} blocks={} arch=#{}", slot.write_seq, slot.total_blocks, active_arch);
                    last_seq = slot.write_seq;
                }

                // ─── Predictive: pre-warm depth cache when time window changes ──
                let current_window = microscope_memory::temporal_archetype::current_time_window();
                if current_window != last_predictive_window {
                    last_predictive_window = current_window;

                    // Find archetypes with high temporal boost in current window
                    let mut predicted_blocks: Vec<(u8, Vec<microscope_memory::depth_cache::CachedResult>)> = Vec::new();

                    for archetype in &archetypes.archetypes {
                        let boost = temporal.boost(archetype.id);
                        if boost > 0.5 {
                            // Pre-cache this archetype's member blocks
                            for &member_idx in archetype.members.iter().take(5) {
                                let idx = member_idx as usize;
                                if idx < reader.block_count {
                                    let h = reader.header(idx);
                                    let text = reader.text(idx);
                                    let preview: String = text.chars().take(64).filter(|&c| c != '\n').collect();
                                    let energy = if idx < hebb.activations.len() {
                                        hebb.activations[idx].energy
                                    } else {
                                        0.0
                                    };

                                    let entry = predicted_blocks
                                        .iter_mut()
                                        .find(|(d, _)| *d == h.depth);

                                    let result = microscope_memory::depth_cache::CachedResult {
                                        block_idx: member_idx,
                                        distance: 0.0,
                                        energy,
                                        layer_id: h.layer_id,
                                        depth: h.depth,
                                        text: preview,
                                        x: h.x,
                                    };

                                    match entry {
                                        Some((_, results)) if results.len() < 5 => {
                                            results.push(result);
                                        }
                                        None => {
                                            predicted_blocks.push((h.depth, vec![result]));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    if !predicted_blocks.is_empty() {
                        if let Ok(mut cache) = microscope_memory::depth_cache::DepthCache::open(output_dir) {
                            let _ = cache.write_recall(0, &predicted_blocks);
                            println!("  {} Pre-warmed {} depths for window {}",
                                "PREDICT:".green(),
                                predicted_blocks.len(),
                                microscope_memory::temporal_archetype::WINDOW_LABELS[current_window]);
                        }
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(interval_ms));
            }
        }
    }
}
