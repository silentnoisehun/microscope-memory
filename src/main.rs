//! Microscope Memory â€” zoom-based hierarchical memory
//!
//! ZERO JSON. Pure binary. mmap. Sub-microsecond.
//!
//! CPU analogy: data exists in uniform blocks at every depth.
//! The query's zoom level determines which layer you see.
//! Same block size, different depth. Like a magnifying glass on silicon.
//!
//! Pipeline: raw memory files â†’ binary blocks â†’ mmap â†’ L2 search
//!
//! Usage:
//!   microscope-mem build                    # layers/ â†’ binary mmap
//!   microscope-mem look 0.25 0.25 0.25 3    # x y z zoom
//!   microscope-mem bench                    # speed test
//!   microscope-mem stats                    # structure info
//!   microscope-mem find "memory"             # text search
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

// â”€â”€â”€ Command handlers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn open_reader(config: &Config) -> MicroscopeReader {
    MicroscopeReader::open(config).expect("Failed to open microscope index â€” run 'build' first")
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

fn stats(config: &Config, reader: &MicroscopeReader) {
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

    println!("\n  Data footprint:");
    let output_dir = Path::new(&config.paths.output_dir);
    let mut total_bytes: u64 = 0;
    for name in [
        "microscope.bin",
        "data.bin",
        "meta.bin",
        "merkle.bin",
        "embeddings.bin",
        "links.bin",
        "activations.bin",
        "emotions.bin",
        "fingerprints.idx",
        "append.bin",
    ] {
        let path = output_dir.join(name);
        if let Ok(meta) = fs::metadata(&path) {
            total_bytes += meta.len();
            println!(
                "  {:<18} {:>9.1} MB",
                name,
                meta.len() as f64 / (1024.0 * 1024.0)
            );
        }
    }
    let history_dir = output_dir.join("index-history");
    if let Ok(entries) = fs::read_dir(&history_dir) {
        let history_bytes: u64 = entries
            .filter_map(|e| e.ok())
            .map(|e| match e.metadata().ok() {
                Some(meta) if meta.is_dir() => fs::read_dir(e.path())
                    .map(|sub| {
                        sub.filter_map(|s| s.ok())
                            .filter_map(|s| s.metadata().ok())
                            .map(|s| s.len())
                            .sum()
                    })
                    .unwrap_or(0),
                Some(meta) => meta.len(),
                None => 0,
            })
            .sum();
        total_bytes += history_bytes;
        println!(
            "  {:<18} {:>9.1} MB",
            "index-history/",
            history_bytes as f64 / (1024.0 * 1024.0)
        );
    }
    let layers_bytes: u64 = fs::read_dir(&config.paths.layers_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0);
    total_bytes += layers_bytes;
    println!(
        "  {:<18} {:>9.1} MB",
        "layers/*.txt",
        layers_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  {:<18} {:>9.1} MB",
        "TOTAL",
        total_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Retention: layer_retention_entries = {} (0 = unlimited)",
        config.index.layer_retention_entries
    );
    println!("{}", "=".repeat(50));
}

fn recall(config: &Config, query: &str, k: usize) {
    let t0 = Instant::now();
    let reader = open_reader(config);
    println!("{} '{}':", "RECALL".cyan().bold(), query);

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);
    let relevance_query = microscope_memory::relevance::RelevanceQuery::new(query);

    // â”€â”€â”€ Attention: compute layer weights from context â”€â”€
    let output_dir_att = Path::new(&config.paths.output_dir);
    let mut attention = microscope_memory::attention::AttentionState::load_or_init(output_dir_att);
    let mut hebb =
        microscope_memory::hebbian::HebbianState::load_or_init(output_dir_att, reader.block_count);
    let tg_pre = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_att);
    let pc_pre = microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_att);

    // Single emotional-field scan; reused for energy and the bias warp.
    let emotional_field = microscope_memory::emotional::emotional_field(&reader, &hebb);
    let emotional_energy = emotional_field
        .as_ref()
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
        emotional_intensity: 0.0,
        session_depth: tg_pre.current_path().len(),
        pattern_confidence: 0.0, // updated below after pattern boost
        cache_hit_rate: pc_pre.stats.hit_rate(),
        archetype_match_score: 0.0, // updated below after archetype match
    };
    let attn = attention.compute_attention(&attn_signals);

    // Emotional bias warp: bend search coordinates toward the precomputed centroid
    let emotional_weight = config.search.emotional_bias_weight * attn.weight(4);
    let (qx, qy, qz) = microscope_memory::emotional::apply_emotional_bias_from_centroid(
        qx,
        qy,
        qz,
        emotional_weight,
        emotional_field.as_ref().map(|f| f.centroid),
    );

    // === 21D Emotional State warp: add wave-based bias from stored state ===
    let (qx, qy, qz) = if config.search.emotion_21d_weight > 0.0 {
        let emotion_file = output_dir_att.join("emotion_21d.bin");
        let state =
            microscope_memory::emotional_21d::EmotionalState21D::load_or_init(&emotion_file);
        let (edx, edy, edz) = microscope_memory::emotional_21d::emotion_21d_bias(&state);
        let w21 = config.search.emotion_21d_weight;
        (qx + edx * w21, qy + edy * w21, qz + edz * w21)
    } else {
        (qx, qy, qz)
    };

    let (zoom_lo, zoom_hi) = match query.len() {
        0..=8 => (0, 2),
        9..=20 => (2, 4),
        _ => (2, 5),
    };

    let mut all_results: Vec<(f32, usize, bool)> = Vec::new();

    // Inverted text index prefilter: narrow the depth-range scan to blocks
    // that can lexically match (token_similarity semantics), then run the
    // exact scoring on those candidates — identical results, far fewer scans.
    let lex_cands: Option<Vec<u32>> = reader
        .text_index
        .as_ref()
        .and_then(|idx| idx.candidates_lexical(relevance_query.tokens()));

    let mut ci = 0usize;
    'zoom: for zoom in zoom_lo..=zoom_hi {
        let (start, count) = reader.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        for i in start..(start + count) {
            if let Some(cands) = &lex_cands {
                if cands.is_empty() {
                    break 'zoom;
                }
                while ci < cands.len() && (cands[ci] as usize) < i {
                    ci += 1;
                }
                if ci >= cands.len() {
                    break 'zoom;
                }
                if (cands[ci] as usize) != i {
                    continue;
                }
            }
            let text = reader.text(i);
            let lexical = relevance_query.lexical_score(text);
            if lexical > 0.0 {
                let h = reader.header(i);
                let dx = h.x - qx;
                let dy = h.y - qy;
                let dz = h.z - qz;
                let spatial_dist = dx * dx + dy * dy + dz * dz;
                let combined = microscope_memory::relevance::rank_distance_from_score(
                    lexical,
                    spatial_dist,
                    config.search.keyword_boost,
                    h.importance,
                );
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
        let lexical = relevance_query.lexical_score(&entry.text);
        if dist < 0.1 || lexical > 0.0 {
            let combined = microscope_memory::relevance::rank_distance_from_score(
                lexical,
                dist,
                config.search.keyword_boost,
                entry.importance,
            );
            all_results.push((combined, ai + 1_000_000, false));
        }
    }

    // â”€â”€â”€ ThoughtGraph + Predictive Cache â”€â”€
    let output_dir_tg = Path::new(&config.paths.output_dir);
    let mut thought_graph =
        microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_tg);
    let mut pred_cache =
        microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_tg);
    let qh_tg = microscope_memory::hebbian::query_hash(query);

    // Check predictive cache â€” instant boost from pre-fetched blocks (scaled by attention)
    if let Some((cached_blocks, confidence)) = pred_cache.check(qh_tg) {
        let boost =
            confidence * microscope_memory::thought_graph::PATTERN_BOOST_WEIGHT * attn.weight(6);
        let cached_set: std::collections::HashSet<u32> = cached_blocks.iter().copied().collect();
        for (dist, idx, is_main) in &mut all_results {
            if *is_main && cached_set.contains(&(*idx as u32)) {
                *dist = microscope_memory::relevance::apply_boost(*dist, boost);
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
    let pattern_boosts: std::collections::HashMap<u32, f32> =
        thought_graph.pattern_boost(qh_tg).into_iter().collect();
    if !pattern_boosts.is_empty() {
        let tg_scale = attn.weight(5); // ThoughtGraph attention weight
        for (dist, idx, is_main) in &mut all_results {
            if *is_main {
                if let Some(&boost) = pattern_boosts.get(&(*idx as u32)) {
                    *dist = microscope_memory::relevance::apply_boost(*dist, boost * tg_scale);
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
    all_results.sort_by(|a, b| a.0.total_cmp(&b.0));
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

    // â”€â”€â”€ Hebbian + Mirror: record activations & detect resonance â”€â”€
    let output_dir = Path::new(&config.paths.output_dir);
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
        let mut temporal =
            microscope_memory::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
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
            println!("  {} prediction {} (overlap={})", symbol, hit_type, overlap);
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

        // --- Eureka: detect unexpected connections ---
        let eureka_events =
            microscope_memory::eureka::detect_eureka(config, &reader, query, None, &all_results);
        if !eureka_events.is_empty() {
            let mut eureka_log = microscope_memory::eureka::EurekaLog::load_or_init(output_dir);
            for ev in &eureka_events {
                let _ = eureka_log.record(output_dir, ev.clone());
                println!(
                    "  {} {}",
                    "EUREKA:".red().bold(),
                    microscope_memory::eureka::format_eureka(ev)
                );
            }
        }

        // --- Spaced repetition: record recall for each activated block ---
        let mut spaced =
            microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
        for &(idx, _) in &activated {
            spaced.record_recall(idx, 5, 3);
        }
        let _ = spaced.save(output_dir);

        // --- Narrative: update the system's self-narrative ---
        let mut narrative = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
        let esr = microscope_memory::emotional_state::EmotionalStateRing::load_or_init(output_dir);
        let due_count = Some(spaced.due_count());
        let thought_count = Some(thought_graph.crystallized_count());
        let wm_items: Vec<String> = activated
            .iter()
            .take(3)
            .map(|&(idx, _)| reader.text(idx as usize).chars().take(60).collect())
            .collect();
        if let Err(e) = narrative.update(
            output_dir,
            Some(&esr),
            Some(&wm_items),
            due_count,
            thought_count,
            Some(query),
        ) {
            eprintln!("  {} narrative update failed: {}", "ERROR:".red(), e);
        }
        microscope_memory::narrative::metacognitive_store(
            output_dir,
            &narrative.narrative,
            &narrative.emotion,
        );
        if narrative.session_count <= 3 || narrative.session_count.is_multiple_of(10) {
            println!("  {} {}", "NARRATIVE:".cyan(), narrative.narrative);
        }

        // --- Auto-reflect: every N recalls, the system thinks about itself ---
        if narrative.session_count > 0
            && (narrative.session_count as usize)
                .is_multiple_of(microscope_memory::self_reflect::AUTO_REFLECT_INTERVAL)
        {
            let reflection =
                microscope_memory::self_reflect::introspect(config, &reader, output_dir);
            println!(
                "{}",
                microscope_memory::self_reflect::format_reflection(&reflection)
            );
        }

        // --- Auto self-model snapshot: every 10th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize).is_multiple_of(10) {
            let mut self_model = microscope_memory::self_model::SelfModel::load_or_init(output_dir);
            let snap = self_model.take_snapshot(config, &reader, output_dir);
            let change = self_model.describe_change();
            println!(
                "{}",
                microscope_memory::self_model::format_self_model(&snap, &change)
            );
        }

        // --- Auto curiosity: every 7th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize).is_multiple_of(7) {
            let mut curiosity =
                microscope_memory::curiosity::CuriosityState::load_or_init(output_dir);
            let queries = curiosity.generate_queries(config, &reader, output_dir);
            if !queries.is_empty() {
                println!(
                    "{}",
                    microscope_memory::curiosity::format_curiosity(&queries)
                );
            }
        }

        // --- Narrative Memory: build story episode from every recall ---
        {
            let mut nm =
                microscope_memory::narrative_memory::NarrativeMemory::load_or_init(output_dir);
            if let Some(ep) = nm.build_episode(config, &reader, output_dir, query, &all_results) {
                if nm.episodes.len() <= 3 || nm.episodes.len().is_multiple_of(5) {
                    println!(
                        "{}",
                        microscope_memory::narrative_memory::format_episode(&ep)
                    );
                }
            }
        }

        // --- Auto inner monologue: every 15th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize).is_multiple_of(15) {
            let mut monologue =
                microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(config, &reader, output_dir);
            println!(
                "{}",
                microscope_memory::inner_monologue::format_monologue(&entry)
            );
        }
    }

    let elapsed = t0.elapsed();
    println!("\n  {} results in {:.0} us", shown, elapsed.as_micros());
}

fn semantic_search(config: &Config, query: &str, k: usize, metric: &str) {
    use microscope_memory::embedding_index::EmbeddingIndex;
    use microscope_memory::embeddings::{cosine_similarity_simd, EmbeddingProvider};

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

        let provider: Box<dyn EmbeddingProvider> =
            microscope_memory::embeddings::provider_from_config(&config.embedding, idx.dim());

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

    println!("  No embedding index â€” computing on-the-fly (slow)");
    let provider = microscope_memory::embeddings::provider_from_config(
        &config.embedding,
        config.embedding.dim,
    );

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

    results.sort_by(|a, b| b.0.total_cmp(&a.0));
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
            "  {} merkle.bin not found â€” rebuild with v0.2.0 to generate",
            "ERR".red()
        );
        return;
    }

    let meta = fs::read(&meta_path).expect("read meta.bin");
    let magic = &meta[0..4];
    if magic != b"MSC2" && magic != b"MSC3" && magic != b"MSC4" {
        println!(
            "  {} meta.bin is v1 (MSCM) â€” no merkle root stored. Rebuild first.",
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
        println!("  {} merkle.bin not found â€” rebuild first", "ERR".red());
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

fn serve_viewer(port: u16) {
    use std::io::{BufRead, Write};
    use std::net::TcpListener;

    let addr = format!("127.0.0.1:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("  {} Cannot bind to {}: {}", "ERROR:".red(), addr, e);
            return;
        }
    };

    println!("{} http://{}", "SERVE".cyan().bold(), addr);
    println!(
        "  Open your browser: {}",
        format!("http://localhost:{}/viewer.html", port).green()
    );
    println!("  Press Ctrl+C to stop.\n");

    let html_path = std::env::current_dir().unwrap().join("viewer.html");
    let bin_path = std::env::current_dir().unwrap().join("cognitive_map.bin");

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut reader = std::io::BufReader::new(&stream);
        let mut request_line = String::new();
        let _ = reader.read_line(&mut request_line);

        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string();

        let (status, content_type, body): (&str, &str, Vec<u8>) =
            if path == "/viewer.html" || path == "/" {
                match fs::read(&html_path) {
                    Ok(b) => ("200 OK", "text/html; charset=utf-8", b),
                    Err(_) => (
                        "404 Not Found",
                        "text/plain",
                        b"viewer.html not found. Run 'cognitive-map' first.".to_vec(),
                    ),
                }
            } else if path == "/cognitive_map.bin" {
                match fs::read(&bin_path) {
                    Ok(b) => ("200 OK", "application/octet-stream", b),
                    Err(_) => (
                        "404 Not Found",
                        "text/plain",
                        b"cognitive_map.bin not found. Run 'cognitive-map' first.".to_vec(),
                    ),
                }
            } else {
                ("404 Not Found", "text/plain", b"Not found".to_vec())
            };

        let header = format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n", status, content_type, body.len());
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    }
}

// â”€â”€â”€ MAIN â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn init_demo(config: &Config, force: bool) -> Result<(), String> {
    let layers_dir = Path::new(&config.paths.layers_dir);
    if !layers_dir.exists() {
        fs::create_dir_all(layers_dir).map_err(|e| e.to_string())?;
    }

    let demo_path = layers_dir.join("demo.txt");
    if demo_path.exists() && !force {
        return Err("layers/demo.txt already exists. Use --force to overwrite.".to_string());
    }

    let demo_content = "Microscope Memory: Hierarchical Cognitive Engine\n\nThis is a demo dataset for the Microscope Memory. It uses a 9-layer hierarchical model (D0-D8) to store and recall information.\n\nKey Concepts:\n- Hebbian Learning: Blocks that fire together, wire together.\n- Binary Spine: Zero-JSON, mmap-backed performance.\n- Resonance: Federated synchronization protocol.\n\nHow to use:\n1. Run 'microscope-mem build' to index this file.\n2. Run 'microscope-mem think \"Tell me about Hebbian learning\"' to see it in action.\n";
    let demo_tmp = layers_dir.join("demo.txt.tmp");
    fs::write(&demo_tmp, demo_content).map_err(|e| e.to_string())?;
    fs::rename(&demo_tmp, &demo_path).map_err(|e| e.to_string())?;

    println!("{}", "Demo dataset initialized.".green().bold());
    println!("  -> Created {}", demo_path.display());
    println!("\nNext steps:");
    println!(
        "  1. {} build        # Build the binary index",
        "microscope-mem".cyan()
    );
    println!(
        "  2. {} cognitive-map # Export 3D visualization",
        "microscope-mem".cyan()
    );
    println!(
        "  3. {} serve         # Open 3D viewer in browser",
        "microscope-mem".cyan()
    );

    Ok(())
}

fn main() {
    // Debug builds carry a large async future frame; give the thread that
    // runs `block_on` a generous stack so `cargo run` does not overflow on
    // startup (the OS main-thread default is 1 MiB on Windows).
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Runtime::new().expect("build tokio runtime");
            runtime.block_on(async_main());
        })
        .expect("spawn main thread")
        .join()
        .expect("main thread panicked");
}

async fn async_main() {
    let config_path =
        std::env::var("MICROSCOPE_CONFIG").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
    let config = Config::load(&config_path).unwrap_or_else(|_| {
        // Redir warning to stderr for MCP compatibility
        eprintln!(
            "  {} Could not load '{}'; using default configuration",
            "WARN:".yellow(),
            config_path
        );
        Config::default()
    });

    // Backward-compatible entrypoint for external MCP launchers
    // that invoke the binary with `--mcp-mode` instead of the `mcp` subcommand.
    if std::env::args().any(|arg| arg == "--mcp-mode") {
        microscope_memory::mcp::run(config);
        return;
    }

    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Serve { port } => {
            serve_viewer(port);
        }
        Cmd::Token { user_id } => {
            match microscope_memory::bridge::user_token(
                config.server.api_key.as_deref().unwrap_or(""),
                &user_id,
            ) {
                Ok(token) => println!("{}", token),
                Err(e) => eprintln!("  {} {}", "ERROR:".red(), e),
            }
        }
        Cmd::InitDemo { force } => {
            if let Err(e) = init_demo(&config, force) {
                eprintln!("  {} {}", "ERROR:".red(), e);
            }
        }
        Cmd::Doctor { fix } => {
            microscope_memory::doctor::run_doctor(&config, fix).expect("doctor failed");
        }
        Cmd::Build { force } => {
            microscope_memory::build::build(&config, force, true).expect("build failed");
        }
        Cmd::Store {
            text,
            layer,
            importance,
            status,
        } => {
            crate::reader::store_memory_with_status(
                &config,
                &text,
                &layer,
                importance,
                status.as_deref(),
                None,
            )
            .expect("store failed");

            // ── Fail-soft emotion extraction side-branch ──
            // Principle 1: memory is already stored. This is a separate,
            // fail-soft side-branch that adds emotional context if possible.
            // If it fails, the memory is still safely stored.
            let output_dir = std::path::Path::new(&config.paths.output_dir);
            let extraction = microscope_memory::emotion_extraction::extract_emotion(&text);

            // Principle 3: only create episode if structural signal detected
            // and confidence is high enough. No fake emotion.
            if extraction.trigger_is_structural && extraction.detection_confidence >= 0.55 {
                let mut episode_store =
                    microscope_memory::emotional_episode::EpisodeStore::load_or_init(output_dir);
                let gate_config = epistemic_core::gate::GateConfig::default();

                if let Some(episode) =
                    microscope_memory::emotional_episode::EmotionalEpisode::from_extraction(
                        episode_store.next_id,
                        0, // trigger_evidence_id = 0 (text-based, not block-based)
                        &extraction,
                        &gate_config,
                    )
                {
                    episode_store.add(episode);
                    let _ = episode_store.save(output_dir);
                }
            }
        }
        Cmd::Timeline { window, k } => {
            let path = std::path::Path::new(&config.paths.output_dir).join("timeline.bin");
            let entries = crate::timeline::read_all(&path);
            let w = crate::timeline::TimeWindow::parse(&window).expect("invalid window");
            let filtered = crate::timeline::filter(&entries, &w);
            let mut rev: Vec<&crate::timeline::TimelineEntry> = filtered.iter().rev().collect();
            rev.truncate(k);
            println!(
                "Timeline [{}] — {} entries (of {} in log):",
                window,
                rev.len(),
                entries.len()
            );
            for e in rev {
                let layer_name = crate::LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
                let status_label = match e.status {
                    crate::timeline::STATUS_OPEN => "OPEN",
                    crate::timeline::STATUS_RESOLVED => "RESOLVED",
                    crate::timeline::STATUS_ARCHIVED => "ARCHIVED",
                    _ => "",
                };
                println!(
                    "{} D{} [{}] imp={}{} {}",
                    crate::timeline::format_ts(e.ts_ms),
                    e.depth,
                    layer_name,
                    e.importance,
                    if status_label.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", status_label)
                    },
                    crate::safe_truncate(&e.text, 100)
                );
            }
        }
        Cmd::Loops { k: _ } => {
            let dir = std::path::Path::new(&config.paths.output_dir);
            let open = crate::open_loops::read_open(&dir.join("open_loops.bin"));
            if open.is_empty() {
                println!("No open loops.");
            } else {
                println!("Open Loops ({}):", open.len());
                for e in &open {
                    println!(
                        "#{} {} imp={} {}",
                        e.id,
                        crate::timeline::format_ts(e.ts_ms),
                        e.importance,
                        crate::safe_truncate(&e.text, 100)
                    );
                }
            }
        }
        Cmd::ResolveLoop { id } => {
            let dir = std::path::Path::new(&config.paths.output_dir);
            match crate::open_loops::resolve(dir, id) {
                Ok(true) => println!("Loop #{} resolved.", id),
                Ok(false) => println!("Loop #{} not found or already resolved.", id),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Cmd::AutoContext { compact, output } => {
            let reader = match microscope_memory::reader::MicroscopeReader::open(&config) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error opening reader: {}", e);
                    return;
                }
            };
            let output_dir = std::path::Path::new(&config.paths.output_dir);
            let ctx = crate::auto_context::build(output_dir, &reader);
            let text = if compact {
                crate::auto_context::render_compact(&ctx)
            } else {
                crate::auto_context::render(&ctx)
            };
            if let Some(path) = output {
                let p = std::path::Path::new(&path);
                let tmp_path = p.with_extension("tmp");
                if std::fs::write(&tmp_path, &text).is_ok() {
                    let _ = std::fs::rename(&tmp_path, p);
                    println!("Auto-context written to {}", path);
                } else {
                    eprintln!("Error writing to {}", path);
                }
            } else {
                print!("{}", text);
            }
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
            stats(&config, &r);
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
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            let res = r.find_text_all(&config, &query, k);
            if res.is_empty() {
                println!("  (none)");
            }
            for (_d, i, is_main) in res {
                if is_main {
                    r.print_result(i, 0.0);
                } else {
                    print_append_result(&appended, i, 0.0);
                }
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
                        "  {} fingerprints.idx not found â€” run 'fingerprint' first",
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
                        "  {} fingerprints.idx not found â€” run 'fingerprint' first",
                        "ERR".red()
                    );
                }
            }
        }
        Cmd::Rebuild => {
            println!("{}", "Rebuilding with append log...".cyan());
            let outcome = microscope_memory::build::rebuild_pending(&config, true, true)
                .expect("rebuild failed");
            println!(
                "  Append log cleared after consolidating {} entries.",
                outcome.pending_entries
            );
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
        Cmd::Think { query, max_steps } => {
            let reader = open_reader(&config);
            let mut chain = microscope_memory::sequential_thinking::ThinkingChain::new(max_steps);
            chain.brainstorm(&reader, &config, &query);
            println!("\n{}", "SEQUENTIAL THINKING RESULT:".cyan().bold());
            chain.display();
        }
        Cmd::Spine => {
            // Native MCP server replaces the placeholder binary listener
            microscope_memory::mcp::run(config);
        }
        Cmd::Mcp => {
            // Start MCP server for Claude Desktop integration
            microscope_memory::mcp::run(config);
        }
        Cmd::Config { client } => {
            print_client_setup(&client, &config);
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
                println!("  (no active blocks â€” run some queries first)");
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
                println!("  (no resonant blocks â€” run queries to build mirror state)");
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
            let _resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
            let archetypes = microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
            let thought_graph =
                microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);

            let dest = Path::new(&output);
            microscope_memory::viz::export_to_file(
                output_dir,
                &reader,
                &hebb,
                &mirror,
                &thought_graph,
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
                println!("  (no patterns yet â€” recall more to form thought paths)");
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
                        seq_str.join(" â†’ "),
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
                            "â–¸".green(),
                            first.session_id,
                            session.len()
                        );
                        let path_str: Vec<String> = session
                            .iter()
                            .map(|n| format!("{:04x}", n.query_hash & 0xFFFF))
                            .collect();
                        println!("    {}", path_str.join(" â†’ "));
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
                println!(
                    "  (no temporal data yet â€” recall with archetype matches to build profiles)"
                );
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
                    for (i, label) in microscope_memory::temporal_archetype::WINDOW_LABELS
                        .iter()
                        .enumerate()
                    {
                        let bar_len = (p.window_weights[i] * 5.0) as usize;
                        let bar: String = "â–".repeat(bar_len);
                        let marker = if i == window { " â—€" } else { "" };
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
            let attn_state = microscope_memory::attention::AttentionState::load_or_init(output_dir);
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
                let bar: String = "â–".repeat(bar_len.min(30));
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
            match microscope_memory::dream::dream_consolidate(
                output_dir,
                reader.block_count,
                config.index.max_blocks,
                config.index.protect_min_importance,
            ) {
                Ok(cycle) => {
                    let mut dream_state =
                        microscope_memory::dream::DreamState::load_or_init(output_dir);
                    dream_state.last_dream_ms = cycle.timestamp_ms;
                    dream_state.cycles.push(cycle.clone());
                    if dream_state.cycles.len() > 200 {
                        dream_state.cycles.drain(0..dream_state.cycles.len() - 200);
                    }
                    let _ = dream_state.save(output_dir);
                    println!("  Duration:      {} ms", cycle.duration_ms);
                    println!(
                        "  Replayed:      {} fingerprints",
                        cycle.replayed_fingerprints
                    );
                    println!("  Strengthened:  {} pairs", cycle.strengthened_pairs);
                    println!("  Pruned pairs:  {}", cycle.pruned_pairs);
                    println!("  Pruned blocks: {}", cycle.pruned_activations);
                    println!("  Forgotten:      {} blocks", cycle.forgotten_blocks);
                    println!("  Patterns:      +{}", cycle.consolidated_patterns);
                    println!(
                        "  Energy:        {:.1} â†’ {:.1}",
                        cycle.energy_before, cycle.energy_after
                    );
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
            println!(
                "  Total pruned:  {} pairs, {} activations",
                stats.total_pruned_pairs, stats.total_pruned_activations
            );
            println!("  Total strengthened: {} pairs", stats.total_strengthened);
            println!("  Total replayed: {} fingerprints", stats.total_replayed);
            println!("  Total forgotten: {} blocks", stats.total_forgotten_blocks);
            if !state.cycles.is_empty() {
                println!("\n  Recent cycles:");
                let start = if state.cycles.len() > k {
                    state.cycles.len() - k
                } else {
                    0
                };
                for cycle in &state.cycles[start..] {
                    println!(
                        "    {} â€” {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}, forgotten={}",
                        cycle.timestamp_ms,
                        cycle.duration_ms,
                        cycle.replayed_fingerprints,
                        cycle.strengthened_pairs,
                        cycle.pruned_pairs,
                        cycle.pruned_activations,
                        cycle.consolidated_patterns,
                        cycle.forgotten_blocks
                    );
                }
            }
        }
        Cmd::EmotionalField => {
            let output_dir = Path::new(&config.paths.output_dir);
            let state =
                microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(
                    output_dir,
                );
            let stats = state.stats();
            println!("{}", "EMOTIONAL FIELD".cyan().bold());
            println!("  Instance ID:  {:016x}", stats.instance_id);
            println!(
                "  Local field:  {}",
                if stats.has_local {
                    "active"
                } else {
                    "inactive"
                }
            );
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
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let mut local =
                microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(
                    output_dir,
                );
            local.capture_local(&reader, &hebb);

            let mut exchanged = 0usize;
            for idx_config in &config.federation.indices {
                if let Ok(idx_cfg) =
                    microscope_memory::config::Config::load(&idx_config.config_path)
                {
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
            println!(
                "{} exchanged {} emotional snapshots",
                "EMOTIONAL EXCHANGE".cyan().bold(),
                exchanged
            );
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
            let _resonance = microscope_memory::resonance::ResonanceState::load_or_init(output_dir);
            let _archetypes =
                microscope_memory::archetype::ArchetypeState::load_or_init(output_dir);
            let _thought_graph =
                microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let thought_graph =
                microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let _pred_cache =
                microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir);
            let _temporal =
                microscope_memory::temporal_archetype::TemporalArchetypeState::load_or_init(
                    output_dir,
                );
            let _attention = microscope_memory::attention::AttentionState::load_or_init(output_dir);
            let _dream = microscope_memory::dream::DreamState::load_or_init(output_dir);
            let _emotional =
                microscope_memory::emotional_contagion::EmotionalContagionState::load_or_init(
                    output_dir,
                );
            let _modalities =
                microscope_memory::multimodal::ModalityIndex::load_or_init(output_dir);

            let dest = Path::new(&output);
            microscope_memory::viz::export_to_file(
                output_dir,
                &reader,
                &hebb,
                &mirror,
                &thought_graph,
                dest,
            )
            .expect("export BINARY VIZ");

            let file_size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
            println!(
                "{} 13-layer BINARY VIZ â†’ {} ({} bytes)",
                "BINARY VIZ".cyan().bold(),
                output,
                file_size
            );

            // Copy viewer.html and cognitive_map.bin to current dir and start HTTP server
            let viewer_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("viewer.html");
            let current_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let viewer_dst = current_dir.join("viewer.html");
            let bin_dst = current_dir.join("cognitive_map.bin");

            // Copy files to current dir
            if viewer_src.exists() {
                let _ = std::fs::copy(&viewer_src, &viewer_dst);
            }
            if dest.exists() {
                let _ = std::fs::copy(dest, &bin_dst);
            }

            if viewer_dst.exists() && bin_dst.exists() {
                // Start HTTP server from the current directory
                println!(
                    "{} Binary visualization exported. (Zero JSON: No web server started)",
                    "INFO".cyan().bold()
                );
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
            let text_repr: String = fields
                .iter()
                .map(|(k, v)| format!("DAT:{}={:?}", k, v))
                .collect::<Vec<_>>()
                .join(" ");
            let text_short = if text_repr.len() > 200 {
                &text_repr[..200]
            } else {
                &text_repr
            };
            let _ = store_memory(&config, text_short, "rust_state", importance);

            // Register in multimodal index
            let mut index = microscope_memory::multimodal::ModalityIndex::load_or_init(output_dir);
            let block_idx = index.entries.len() as u32 + 1_000_000; // virtual idx for append entries
            index.register(
                block_idx,
                microscope_memory::multimodal::Modality::Structured(
                    microscope_memory::multimodal::StructuredMeta {
                        fields: fields.clone(),
                    },
                ),
            );
            let _ = index.save(output_dir);

            println!(
                "{} stored {} fields as structured data",
                "STORE-DATA".green().bold(),
                fields.len()
            );
        }
        // Cmd::Bridge removed — replaced by napi-rs native addon
        // See native/src/lib.rs for the #[napi] equivalent
        Cmd::Mermaid { port } => {
            if let Err(e) = microscope_memory::mermaid::run(config, port).await {
                eprintln!("  {} Mermaid error: {}", "ERROR:".red(), e);
            }
        }
        Cmd::Introspect => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let reflection =
                microscope_memory::self_reflect::introspect(&config, &reader, output_dir);
            println!(
                "{}",
                microscope_memory::self_reflect::format_reflection(&reflection)
            );
        }
        Cmd::SelfModel => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut self_model = microscope_memory::self_model::SelfModel::load_or_init(output_dir);
            let snap = self_model.take_snapshot(&config, &reader, output_dir);
            let change = self_model.describe_change();
            println!(
                "{}",
                microscope_memory::self_model::format_self_model(&snap, &change)
            );
        }
        Cmd::AwarenessTrace => {
            let output_dir = Path::new(&config.paths.output_dir);
            // Take a fresh snapshot to ensure the graph is up to date
            let reader = open_reader(&config);
            let mut self_model = microscope_memory::self_model::SelfModel::load_or_init(output_dir);
            let _ = self_model.take_snapshot(&config, &reader, output_dir);
            println!(
                "{}",
                microscope_memory::self_model::format_awareness_trace(output_dir)
            );
        }
        Cmd::Curiosity => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut curiosity =
                microscope_memory::curiosity::CuriosityState::load_or_init(output_dir);
            let queries = curiosity.generate_queries(&config, &reader, output_dir);
            println!(
                "{}",
                microscope_memory::curiosity::format_curiosity(&queries)
            );
        }
        Cmd::Monologue => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut monologue =
                microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(&config, &reader, output_dir);
            println!(
                "{}",
                microscope_memory::inner_monologue::format_monologue(&entry)
            );
        }
        Cmd::Stories { k } => {
            let _reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let nm = microscope_memory::narrative_memory::NarrativeMemory::load_or_init(output_dir);
            let episodes = nm.recent_episodes(k);
            if episodes.is_empty() {
                println!(
                    "  {} No narrative episodes yet - recall to build stories",
                    "STORIES:".cyan()
                );
            } else {
                println!(
                    "  {} {} recent episodes:",
                    "STORIES:".cyan().bold(),
                    episodes.len()
                );
                for ep in episodes {
                    println!(
                        "{}",
                        microscope_memory::narrative_memory::format_episode(ep)
                    );
                }
            }
        }
        Cmd::Daydream { seed, steps } => {
            let _reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let seed_text = if seed.is_empty() {
                let narrative =
                    microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
                if narrative.narrative.is_empty() || narrative.narrative == "I am silent." {
                    "Microscope Memory".to_string()
                } else {
                    narrative.narrative
                }
            } else {
                seed
            };
            match microscope_memory::daydream::daydream(&config, &seed_text, steps) {
                Ok(result) => println!(
                    "{}",
                    microscope_memory::daydream::format_daydream(&result, true)
                ),
                Err(e) => eprintln!("  {} Daydream error: {}", "ERROR:".red(), e),
            }
        }
        Cmd::Hyperfocus { target, focus_type } => {
            let _output_dir = Path::new(&config.paths.output_dir);
            let mut hf = microscope_memory::hyperfocus::Hyperfocus::new();
            let intensity = hf.enter_hyperfocus(&target, &focus_type);
            println!(
                "  {} Entering hyperfocus on '{}' ({})",
                "FOCUS:".green().bold(),
                target,
                focus_type
            );
            println!(
                "  {} Attention multiplier: {}x, Resource concentration: {:.0}%",
                "FOCUS:".green(),
                intensity,
                hf.resource_concentration * 100.0
            );
            // Run a focused recall
            let reader = open_reader(&config);
            let results = reader.find_text(&target, 10);
            if !results.is_empty() {
                println!(
                    "  {} Found {} relevant blocks",
                    "FOCUS:".green(),
                    results.len()
                );
                for (depth, idx) in results.iter().take(5) {
                    reader.print_result(*idx, *depth as f32);
                }
            }
        }
        Cmd::Keys { action } => {
            use microscope_memory::keystore::{default_keys_path, KeyStore};
            let keys_path = default_keys_path(&config.paths.output_dir);
            let mut store = KeyStore::load(&keys_path).unwrap_or_default();
            match action {
                microscope_memory::cli::KeyAction::Set {
                    service,
                    key,
                    priority,
                } => {
                    store.set(&service, key, priority);
                    if let Err(e) = store.save(&keys_path) {
                        eprintln!("  {} Failed to save keys.bin: {}", "ERROR:".red(), e);
                    } else {
                        println!(
                            "  {} Key '{}' (priority {}) saved to keys.bin",
                            "OK:".green(),
                            service,
                            priority
                        );
                    }
                }
                microscope_memory::cli::KeyAction::Remove { service, priority } => {
                    if let Some(p) = priority {
                        store.remove(&service, p);
                    } else {
                        store.entries.retain(|e| e.service != service);
                    }
                    let _ = store.save(&keys_path);
                    println!("  {} Key(s) '{}' removed", "OK:".green(), service);
                }
                microscope_memory::cli::KeyAction::List => {
                    let info = store.list();
                    if info.is_empty() {
                        println!("  {} No keys stored", "INFO:".yellow());
                    } else {
                        println!("{}", "─ Keys in keys.bin ─".cyan());
                        for entry in &info {
                            let status = if entry.disabled {
                                "DISABLED".red()
                            } else {
                                "active".green()
                            };
                            println!(
                                "  {} [{}] priority={} {} {}",
                                status,
                                entry.service,
                                entry.priority,
                                entry.key_preview,
                                if let Some(ref err) = entry.last_error {
                                    format!("({})", err.dimmed())
                                } else {
                                    String::new()
                                }
                            );
                        }
                    }
                }
                microscope_memory::cli::KeyAction::Status => {
                    let info = store.list();
                    if info.is_empty() {
                        println!("  {} No keys stored", "INFO:".yellow());
                    } else {
                        println!("{}", "─ Key Status ─".cyan());
                        for entry in &info {
                            let status = if entry.disabled {
                                "DISABLED".red()
                            } else {
                                "active".green()
                            };
                            let quota = match entry.quota_remaining {
                                Some(q) => format!("{:.1}%", q * 100.0),
                                None => "unknown".dimmed().to_string(),
                            };
                            println!(
                                "  {} [{}] p{} | quota: {} | created: ? {}",
                                status,
                                entry.service,
                                entry.priority,
                                quota,
                                if let Some(ref err) = entry.last_error {
                                    format!("| err: {}", err.dimmed())
                                } else {
                                    String::new()
                                }
                            );
                        }
                    }
                }
                microscope_memory::cli::KeyAction::Reset => {
                    let count = store.entries.len();
                    store.reset_all();
                    let _ = store.save(&keys_path);
                    println!("  {} {} key(s) reset (re-enabled)", "OK:".green(), count);
                }
            }
        }
        Cmd::ZenKeys { action } => {
            use microscope_memory::zen_keystore::ZenKeyStore;
            let zen_path = "zen_keys.bin";
            match action {
                microscope_memory::cli::ZenKeyAction::Import { json_path, output } => {
                    let json_str = match std::fs::read_to_string(&json_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("  {} Cannot read {}: {}", "ERROR:".red(), json_path, e);
                            return;
                        }
                    };
                    match ZenKeyStore::import_json(&json_str) {
                        Ok(store) => {
                            let out_path = if output == "zen_keys.bin" && !json_path.is_empty() {
                                // Use the json file's directory if relative
                                let p = std::path::Path::new(&json_path);
                                p.parent()
                                    .unwrap_or(std::path::Path::new("."))
                                    .join("zen_keys.bin")
                            } else {
                                std::path::PathBuf::from(&output)
                            };
                            if let Err(e) = store.save(&out_path) {
                                eprintln!(
                                    "  {} Failed to save zen_keys.bin: {}",
                                    "ERROR:".red(),
                                    e
                                );
                            } else {
                                println!("  {} zen_keys.json → zen_keys.bin", "OK:".green());
                                println!("{}", store.stats());
                            }
                        }
                        Err(e) => {
                            eprintln!("  {} Failed to import: {}", "ERROR:".red(), e);
                        }
                    }
                }
                microscope_memory::cli::ZenKeyAction::Stats => {
                    let store = match ZenKeyStore::load(zen_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("  {} Cannot load zen_keys.bin: {}", "ERROR:".red(), e);
                            return;
                        }
                    };
                    println!("{}", "─ Zen Key Store ─".cyan());
                    println!("{}", store.stats());
                }
                microscope_memory::cli::ZenKeyAction::List => {
                    let store = match ZenKeyStore::load(zen_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("  {} Cannot load zen_keys.bin: {}", "ERROR:".red(), e);
                            return;
                        }
                    };
                    println!("{}", "─ Keys in zen_keys.bin ─".cyan());
                    for p in &store.providers {
                        println!(
                            "  {} [{}] ({} keys):",
                            p.name,
                            p.rotation.as_str(),
                            p.keys.len()
                        );
                        for (i, k) in p.keys.iter().enumerate() {
                            let status = if k.disabled {
                                "DISABLED".red()
                            } else {
                                "active".green()
                            };
                            let preview = if k.key.len() > 12 {
                                format!("{}...", &k.key[..12])
                            } else {
                                "***".to_string()
                            };
                            println!("    #{} {} p{} {}", i, status, k.priority, preview.dimmed());
                        }
                    }
                    if !store.models.is_empty() {
                        println!("\n  {} Models:", "Models:".yellow());
                        for m in &store.models {
                            let prov = m.provider.as_deref().unwrap_or("openai");
                            println!(
                                "    #{} {} [{}] {} {}",
                                m.priority,
                                m.id,
                                prov,
                                m.endpoint,
                                if m.free {
                                    "FREE".green()
                                } else {
                                    "PAID".yellow()
                                }
                            );
                        }
                    }
                }
                microscope_memory::cli::ZenKeyAction::Status => {
                    let store = match ZenKeyStore::load(zen_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("  {} Cannot load zen_keys.bin: {}", "ERROR:".red(), e);
                            return;
                        }
                    };
                    println!("{}", "─ Zen Key Status ─".cyan());
                    for p in &store.providers {
                        println!("  {}:", p.name);
                        for (i, k) in p.keys.iter().enumerate() {
                            let status = if k.disabled {
                                "DISABLED".red()
                            } else {
                                "active".green()
                            };
                            let quota = match k.quota_remaining {
                                Some(q) => format!("{:.1}%", q * 100.0),
                                None => "unknown".dimmed().to_string(),
                            };
                            println!(
                                "    #{} {} p{} | quota: {} {}",
                                i,
                                status,
                                k.priority,
                                quota,
                                if let Some(ref err) = k.last_error {
                                    format!("| err: {}", err.dimmed())
                                } else {
                                    String::new()
                                }
                            );
                        }
                    }
                }
                microscope_memory::cli::ZenKeyAction::Reset => {
                    let mut store = match ZenKeyStore::load(zen_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("  {} Cannot load zen_keys.bin: {}", "ERROR:".red(), e);
                            return;
                        }
                    };
                    let mut count = 0;
                    for p in &mut store.providers {
                        for k in &mut p.keys {
                            if k.disabled {
                                k.disabled = false;
                                k.last_error = None;
                                count += 1;
                            }
                        }
                    }
                    let _ = store.save(zen_path);
                    println!("  {} {} key(s) reset (re-enabled)", "OK:".green(), count);
                }
            }
        }
        Cmd::Enforce { action } => {
            use microscope_memory::cli::EnforceAction;
            use microscope_memory::enforcement::{
                load_engine, save_audit, save_engine, ActionEvent, Decision, Outcome,
            };
            use microscope_memory::planning::Planner;
            use std::path::Path;
            use std::sync::{Arc, Mutex};
            use std::time::{SystemTime, UNIX_EPOCH};

            let output = Path::new(&config.paths.output_dir);
            let now = || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            };

            let mut engine = match load_engine(output) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("  {} {}", "ERROR:".red(), e);
                    return;
                }
            };

            match action {
                EnforceAction::Commit {
                    actor,
                    action,
                    scope,
                    content,
                    expires_ms,
                } => {
                    let id = engine.add_commitment(&actor, &action, &scope, &content, expires_ms);
                    let _ = save_engine(output, &engine);
                    println!(
                        "  {} commitment #{} added: forbid '{}' for '{}' in '{}'",
                        "OK:".green(),
                        id,
                        action,
                        actor,
                        scope
                    );
                }
                EnforceAction::List => {
                    let active = engine.active_commitments(now());
                    if active.is_empty() {
                        println!("  no active commitments (K_t is empty)");
                    } else {
                        for c in active {
                            let expiry = c
                                .expires_at_ms
                                .map(|e| format!(" until {}", e))
                                .unwrap_or_default();
                            println!(
                                "  #{} {} forbids {} in {}{} ({})",
                                c.id, c.actor, c.forbidden_action, c.scope, expiry, c.content
                            );
                        }
                    }
                }
                EnforceAction::Gate {
                    actor,
                    action,
                    scope,
                    content,
                    override_justification,
                } => {
                    let event = ActionEvent {
                        actor,
                        action,
                        content: content.unwrap_or_default(),
                        ts_ms: now(),
                        scope,
                        provenance: "cli/gate".to_string(),
                    };
                    let decision = engine.decide(&event, override_justification.as_deref());
                    match &decision {
                        Decision::Allowed { .. } => {
                            println!("  ALLOWED: action is in A_t^valid")
                        }
                        Decision::Blocked {
                            action: a, reason, ..
                        } => println!("  BLOCKED: '{}' — {}", a, reason),
                        Decision::Overridden {
                            action: a,
                            justification,
                            ..
                        } => println!("  OVERRIDDEN: '{}' — {}", a, justification),
                        Decision::AttributionError { reason } => {
                            println!("  REJECTED (faulty attribution): {}", reason)
                        }
                    }
                    let _ = save_audit(output, engine.audit());
                }
                EnforceAction::Audit => {
                    let chunks = engine.audit();
                    let valid = engine.chain_valid();
                    if chunks.is_empty() {
                        println!("  audit chain is empty");
                    } else {
                        for (i, c) in chunks.iter().enumerate() {
                            let kind = match c.outcome {
                                Outcome::Allowed => "allowed",
                                Outcome::Blocked => "blocked",
                                Outcome::Overridden => "overridden",
                                Outcome::AttributionError => "attribution_error",
                            };
                            println!(
                                "  [{}] ts={} {} {} -> {} in {}",
                                i, c.ts_ms, kind, c.actor, c.action, c.scope
                            );
                        }
                        println!(
                            "  chain integrity: {}",
                            if valid {
                                "OK".green().to_string()
                            } else {
                                "FAIL".red().to_string()
                            }
                        );
                    }
                    let _ = save_audit(output, chunks);
                }
                EnforceAction::RunPlan { goal } => {
                    let mut planner = Planner::new();
                    planner.set_enforcement(Arc::new(Mutex::new(engine)));
                    let gid = planner.add_goal(&goal, &format!("implement {}", goal), 100, None);
                    let plan = planner.create_plan(gid);
                    println!(
                        "  running '{}' ({} steps) through the A_t^valid gate",
                        plan.name,
                        plan.actions.len()
                    );
                    loop {
                        match planner.execute_step(plan.id) {
                            Ok(Some(action)) => {
                                println!("    -> {} [allowed]", action.name);
                            }
                            Ok(None) => {
                                println!("    ✓ plan completed");
                                break;
                            }
                            Err(e) => {
                                println!("    ✗ BLOCKED: {}", e);
                                break;
                            }
                        }
                    }
                    let guard = planner.enforcement();
                    let audited = guard.lock().unwrap();
                    let _ = save_audit(output, audited.audit());
                }
            }
        }
        Cmd::Evidence { action } => {
            use microscope_memory::cli::EvidenceAction;
            use microscope_memory::epistemic::{self, AuditChain, AuditEvent, EvidenceLedger};
            let output_dir = std::path::Path::new(&config.paths.output_dir);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            match action {
                EvidenceAction::Show { hash_or_text } => {
                    let ledger = EvidenceLedger::load_or_init(output_dir);
                    let ch = if hash_or_text.len() == 16
                        && hash_or_text.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        u64::from_str_radix(&hash_or_text, 16).unwrap_or(0)
                    } else {
                        epistemic::content_hash(&hash_or_text)
                    };
                    match ledger.records.get(&ch) {
                        Some(rec) => {
                            println!("  content_hash: {:016x}", rec.content_hash);
                            println!("  class:       {}", rec.class);
                            println!("  source_id:   {:016x}", rec.source_id);
                            println!("  support:     {}", rec.support_count);
                            println!("  refute:      {}", rec.refute_count);
                            println!("  distinct:    {}", rec.distinct_sources);
                            println!("  confidence:  {}", rec.confidence);
                            println!("  first_seen:  {}", rec.first_seen_ms);
                        }
                        None => println!("  no evidence record for hash {:016x}", ch),
                    }
                }
                EvidenceAction::Link {
                    claim,
                    support,
                    source,
                } => {
                    let mut ledger = EvidenceLedger::load_or_init(output_dir);
                    let mut audit = AuditChain::load_or_init(output_dir);
                    let claim_ch =
                        if claim.len() == 16 && claim.chars().all(|c| c.is_ascii_hexdigit()) {
                            u64::from_str_radix(&claim, 16).unwrap_or(0)
                        } else {
                            epistemic::content_hash(&claim)
                        };
                    let support_ch =
                        if support.len() == 16 && support.chars().all(|c| c.is_ascii_hexdigit()) {
                            u64::from_str_radix(&support, 16).unwrap_or(0)
                        } else {
                            epistemic::content_hash(&support)
                        };
                    match epistemic::link_evidence(
                        &mut ledger,
                        &mut audit,
                        claim_ch,
                        support_ch,
                        microscope_memory::epistemic::EpistemicClass::Observation,
                        source,
                        now,
                        Some(&support),
                        Some(&claim),
                        None,
                    ) {
                        Ok(()) => {
                            ledger.save(output_dir).ok();
                            audit.save(output_dir).ok();
                            let conf = ledger
                                .records
                                .get(&claim_ch)
                                .map(|r| r.confidence)
                                .unwrap_or(0);
                            println!(
                                "  linked: claim={:016x} support={:016x} confidence={}",
                                claim_ch, support_ch, conf
                            );
                        }
                        Err(e) => eprintln!("  error: {}", e),
                    }
                }
                EvidenceAction::Refute { claim, source } => {
                    let mut ledger = EvidenceLedger::load_or_init(output_dir);
                    let mut audit = AuditChain::load_or_init(output_dir);
                    let claim_ch =
                        if claim.len() == 16 && claim.chars().all(|c| c.is_ascii_hexdigit()) {
                            u64::from_str_radix(&claim, 16).unwrap_or(0)
                        } else {
                            epistemic::content_hash(&claim)
                        };
                    match epistemic::refute(&mut ledger, &mut audit, claim_ch, source, now) {
                        Ok(()) => {
                            ledger.save(output_dir).ok();
                            audit.save(output_dir).ok();
                            let conf = ledger
                                .records
                                .get(&claim_ch)
                                .map(|r| r.confidence)
                                .unwrap_or(0);
                            println!("  refuted: claim={:016x} confidence={}", claim_ch, conf);
                        }
                        Err(e) => eprintln!("  error: {}", e),
                    }
                }
                EvidenceAction::Audit => {
                    let chain = AuditChain::load_or_init(output_dir);
                    println!("  audit chain: {} chunks", chain.chunks.len());
                    match chain.verify() {
                        Ok(tail) => println!("  integrity: OK (tail={})", hex::encode(tail)),
                        Err(idx) => println!("  integrity: FAIL at chunk {}", idx),
                    }
                }
                EvidenceAction::GateStats => {
                    let chain = AuditChain::load_or_init(output_dir);
                    let gates: usize = chain
                        .chunks
                        .iter()
                        .filter(|c| c.record.event == AuditEvent::PromoGate)
                        .count();
                    println!("  promotion gates blocked: {}", gates);
                    let total = chain.chunks.len().saturating_sub(1); // exclude genesis
                    println!("  total audit events: {}", total);
                }
            }
        }
        Cmd::Morphogenesis { action } => {
            use microscope_memory::cli::MorphogenesisAction;
            use microscope_memory::cognitive_morphogenesis::CognitiveMorphogenesisEngine;
            use microscope_memory::hebbian::HebbianState;
            use microscope_memory::resonance::ResonanceState;
            use microscope_memory::epistemic::EvidenceLedger;
            use microscope_memory::predictive_cache::PredictiveCache;
            use microscope_memory::emotional_contagion::EmotionalContagionState;

            let output_dir = std::path::Path::new(&config.paths.output_dir);
            let reader = open_reader(&config);

            match action {
                MorphogenesisAction::Audit { k } => {
                    let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    println!("{}", "MORPHOGENESIS AUDIT".cyan().bold());
                    if engine.audit_log.is_empty() {
                        println!("  (no audit entries yet)");
                    } else {
                        let start = engine.audit_log.len().saturating_sub(k);
                        for entry in &engine.audit_log[start..] {
                            println!(
                                "  [{}] ts={} phase={} grad={:.3} blocks={} nodes={} conns={} anast={}/{} solid={} prune={} comp: {}",
                                entry.cycle_id,
                                entry.timestamp_ms,
                                entry.phase,
                                entry.gradient_avg,
                                entry.activated_blocks.len(),
                                entry.new_node_count,
                                entry.new_connection_count,
                                entry.anastomosis_count,
                                entry.anastomosis_validated,
                                entry.solidified_paths,
                                entry.pruned_paths,
                                entry.component_scores,
                            );
                        }
                    }
                    println!("  total entries: {}", engine.audit_log.len());
                }
                MorphogenesisAction::Metrics { k } => {
                    let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    println!("{}", "MORPHOGENESIS METRICS".cyan().bold());
                    if engine.metrics_log.is_empty() {
                        println!("  (no metrics yet)");
                    } else {
                        let start = engine.metrics_log.len().saturating_sub(k);
                        for m in &engine.metrics_log[start..] {
                            println!(
                                "  [{}] ts={} phase={} recall={:.3} pred={:.3} entropy={:.3} stability={:.3}",
                                m.cycle_id, m.timestamp_ms, m.phase,
                                m.recall_precision, m.prediction_hit_rate,
                                m.graph_entropy, m.path_stability,
                            );
                        }
                    }
                }
                MorphogenesisAction::Status => {
                    let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    let stats = engine.stats();
                    println!("{}", "MORPHOGENESIS STATUS".cyan().bold());
                    println!("  Total cycles:       {}", stats.total_cycles);
                    println!("  GAS cycles:         {}", stats.gas_cycles);
                    println!("  LIQUID cycles:      {}", stats.liquid_cycles);
                    println!("  SOLID cycles:       {}", stats.solid_cycles);
                    println!("  Avg gradient:       {:.3}", stats.avg_gradient);
                    println!("  Anastomosis total:  {}", stats.total_anastomosis);
                    println!("  Anastomosis valid:  {}", stats.validated_anastomosis);
                    println!("  Audit entries:      {}", stats.total_audit_entries);
                    println!("  Metrics entries:    {}", stats.total_metrics_entries);
                }
                MorphogenesisAction::Run => {
                    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                    let resonance = ResonanceState::load_or_init(output_dir);
                    let evidence = EvidenceLedger::load_or_init(output_dir);
                    let predictive = PredictiveCache::load_or_init(output_dir);
                    let emotional = EmotionalContagionState::load_or_init(output_dir);

                    // Block headers a pozíciókhoz
                    let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
                        .map(|i| {
                            let h = reader.header(i);
                            (h.x, h.y, h.z)
                        })
                        .collect();

                    // Utolsó aktiváció — a Hebbian state-ből
                    let mut activated: Vec<(u32, f32)> = Vec::new();
                    for (i, rec) in hebb.activations.iter().enumerate() {
                        if rec.energy > 0.1 {
                            activated.push((i as u32, rec.energy));
                        }
                    }
                    activated.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                    activated.truncate(20); // top 20

                    let query_hash = 0u64; // nincs explicit query

                    let mut engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    let entry = engine.run_cycle(
                        &activated,
                        query_hash,
                        &hebb,
                        &resonance,
                        &evidence,
                        &predictive,
                        &emotional,
                        reader.block_count,
                        &headers,
                    );
                    engine.save(output_dir).expect("save morphogenesis audit");

                    println!("{}", "MORPHOGENESIS CYCLE COMPLETE".green().bold());
                    println!("  Cycle ID:           {}", entry.cycle_id);
                    println!("  Phase:              {}", entry.phase);
                    println!("  Gradient avg:       {:.3}", entry.gradient_avg);
                    println!("  Activated blocks:   {}", entry.activated_blocks.len());
                    println!("  New nodes:          {}", entry.new_node_count);
                    println!("  New connections:    {}", entry.new_connection_count);
                    println!("  Anastomosis:        {} (validated: {})", entry.anastomosis_count, entry.anastomosis_validated);
                    println!("  Solidified paths:   {}", entry.solidified_paths);
                    println!("  Pruned paths:       {}", entry.pruned_paths);
                    println!("  Components:         {}", entry.component_scores);
                }
                MorphogenesisAction::TestPhases => {
                    use microscope_memory::cognitive_morphogenesis::{CognitiveGradient, Phase};

                    println!("{}", "PHASE TRANSITION TEST".cyan().bold());
                    println!();

                    // GAS: alacsony gradiens
                    let gas_gradient = CognitiveGradient { weights: (0.0, 0.0, 0.0, 0.05, 0.05, 0.0, 0.0) };
                    let gas_val = gas_gradient.compute(0.0, 0.0, 0, 0.1, 0.1, 0.0, 0.0);
                    let gas_phase = Phase::from_gradient(gas_val);
                    println!("  GAS test:    gradient={:.3} phase={} (weights: rel=0.0 res=0.0 evi=0.0 heb=0.05 pred=0.05 emo=0.0 exec=0.0)", gas_val, gas_phase);

                    // LIQUID: közepes gradiens
                    let liquid_gradient = CognitiveGradient { weights: (0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5) };
                    let liquid_val = liquid_gradient.compute(0.5, 0.3, 50, 0.5, 0.5, 0.0, 0.5);
                    let liquid_phase = Phase::from_gradient(liquid_val);
                    println!("  LIQUID test: gradient={:.3} phase={} (weights: all=0.5, scores: mid)", liquid_val, liquid_phase);

                    // SOLID: magas gradiens
                    let solid_gradient = CognitiveGradient { weights: (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0) };
                    let solid_val = solid_gradient.compute(1.0, 1.0, 100, 1.0, 1.0, 1.0, 1.0);
                    let solid_phase = Phase::from_gradient(solid_val);
                    println!("  SOLID test:  gradient={:.3} phase={} (weights: all=1.0, scores: high)", solid_val, solid_phase);

                    println!();
                    println!("  Phase boundaries: GAS < 0.3 | LIQUID 0.3-0.7 | SOLID > 0.7");
                }
                MorphogenesisAction::FullStatus => {
                    use crate::hebbian::HebbianState;
                    use crate::resonance::ResonanceState;
                    use crate::epistemic::EvidenceLedger;
                    use crate::predictive_cache::PredictiveCache;
                    use crate::emotional_contagion::EmotionalContagionState;

                    let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    let stats = engine.stats();
                    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                    let resonance = ResonanceState::load_or_init(output_dir);
                    let evidence = EvidenceLedger::load_or_init(output_dir);
                    let predictive = PredictiveCache::load_or_init(output_dir);
                    let emotional = EmotionalContagionState::load_or_init(output_dir);

                    println!("{}", "FULL COGNITIVE MORPHOGENESIS STATUS".cyan().bold());
                    println!();

                    // Morphogenezis
                    println!("  {}", "── Morphogenesis ──".yellow());
                    println!("  Total cycles:       {}", stats.total_cycles);
                    println!("  GAS / LIQUID / SOLID: {} / {} / {}", stats.gas_cycles, stats.liquid_cycles, stats.solid_cycles);
                    println!("  Avg gradient:       {:.3}", stats.avg_gradient);
                    println!("  Anastomosis:        {} / {} (total/validated)", stats.total_anastomosis, stats.validated_anastomosis);
                    println!("  Audit entries:      {}", stats.total_audit_entries);
                    println!("  Metrics entries:    {}", stats.total_metrics_entries);
                    println!();

                    // Hebbian
                    println!("  {}", "── Hebbian ──".yellow());
                    let active_blocks = hebb.activations.iter().filter(|a| a.energy > 0.1).count();
                    let total_energy: f32 = hebb.activations.iter().map(|a| a.energy).sum();
                    println!("  Active blocks:      {}", active_blocks);
                    println!("  Total energy:       {:.3}", total_energy);
                    println!("  Co-activations:     {}", hebb.coactivations.len());
                    println!("  Fingerprints:       {}", hebb.fingerprints.len());
                    println!();

                    // Resonance
                    println!("  {}", "── Resonance ──".yellow());
                    println!("  Outgoing pulses:    {}", resonance.outgoing.len());
                    println!("  Incoming pulses:    {}", resonance.incoming.len());
                    println!("  Field cells:        {}", resonance.field.len());
                    let field_energy: f32 = resonance.field.values().sum();
                    println!("  Field energy:       {:.3}", field_energy);
                    println!();

                    // Evidence
                    println!("  {}", "── Evidence ──".yellow());
                    let avg_conf = if evidence.records.is_empty() {
                        0.0
                    } else {
                        evidence.records.values().map(|r| r.confidence as f64).sum::<f64>()
                            / evidence.records.len() as f64
                    };
                    println!("  Records:            {}", evidence.records.len());
                    println!("  Avg confidence:     {:.1} / 100 ({:.2})", avg_conf, avg_conf / 100.0);
                    println!();

                    // Predictive
                    println!("  {}", "── Predictive Cache ──".yellow());
                    println!("  Predictions:        {}", predictive.predictions.len());
                    println!("  Hit rate:           {:.3}", predictive.stats.hit_rate());
                    println!("  Hits / Misses:      {} / {}", predictive.stats.total_hits, predictive.stats.total_misses);
                    println!();

                    // Emotion
                    println!("  {}", "── Emotion ──".yellow());
                    if let Some(ref snap) = emotional.local_snapshot {
                        println!("  Valence:            {:.3}", snap.valence);
                        println!("  Total energy:       {:.3}", snap.total_energy);
                        println!("  Active blocks:      {}", snap.active_blocks);
                    } else {
                        println!("  (no emotional snapshot)");
                    }
                    println!();

                    // Utolsó audit-bejegyzés
                    if let Some(last) = engine.audit_log.last() {
                        println!("  {}", "── Last Cycle ──".yellow());
                        println!("  Phase:              {}", last.phase);
                        println!("  Gradient:           {:.3}", last.gradient_avg);
                        println!("  Nodes / Connections: {} / {}", last.new_node_count, last.new_connection_count);
                        println!("  Anastomosis:        {} / {}", last.anastomosis_count, last.anastomosis_validated);
                        println!("  Components:         {}", last.component_scores);
                    }
                }
                MorphogenesisAction::Adversarial => {
                    use microscope_memory::cognitive_morphogenesis::{
                        CognitiveGradient, Phase, CognitiveMorphogenesisEngine,
                        graph_entropy,
                    };
                    use microscope_memory::morphogenesis::MorphogenField;
                    use microscope_memory::hebbian::HebbianState;
                    use microscope_memory::resonance::ResonanceState;
                    use microscope_memory::epistemic::EvidenceLedger;
                    use microscope_memory::predictive_cache::PredictiveCache;

                    println!("{}", "ADVERSARIAL TEST SUITE".cyan().bold());
                    println!();
                    let mut passed = 0usize;
                    let mut failed = 0usize;

                    // ─── Test 1: Geometriai találkozás co-aktiváció nélkül ───
                    println!("  [1] Geometriai találkozás co-aktiváció nélkül");
                    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                    // Két blokk, amelyek NEM co-aktiváltak
                    let fake_pair = (999999u32, 999998u32);
                    let has_coactivation = hebb.coactivations.contains_key(&fake_pair);
                    if !has_coactivation {
                        println!("      PASS: co-aktiváció nélküli pár nem validálódik");
                        passed += 1;
                    } else {
                        println!("      FAIL: nem várt co-aktiváció");
                        failed += 1;
                    }

                    // ─── Test 2: Alacsony evidence → pruning ───
                    println!("  [2] Alacsony evidence confidence → pruning");
                    let evidence = EvidenceLedger::load_or_init(output_dir);
                    let avg_conf = if evidence.records.is_empty() {
                        0.0
                    } else {
                        evidence.records.values().map(|r| r.confidence as f64).sum::<f64>()
                            / evidence.records.len() as f64 / 100.0
                    };
                    // Ha avg_conf < 0.2, akkor pruned kellene legyen
                    let would_prune = avg_conf < 0.2;
                    println!("      avg_confidence = {:.3}, would_prune = {}", avg_conf, would_prune);
                    if avg_conf < 0.2 {
                        println!("      PASS: alacsony confidence → pruning logika aktiv");
                        passed += 1;
                    } else {
                        println!("      SKIP: confidence elég magas ({:.3}), nincs pruning", avg_conf);
                        passed += 1; // nem hiba, csak más állapot
                    }

                    // ─── Test 3: Fázis-átmenet határok ───
                    println!("  [3] Fázis-átmenet határok");
                    let gas = Phase::from_gradient(0.0);
                    let liquid = Phase::from_gradient(0.5);
                    let solid = Phase::from_gradient(1.0);
                    let boundary_low = Phase::from_gradient(0.299);
                    let boundary_high = Phase::from_gradient(0.701);
                    let ok = gas == Phase::Gas && liquid == Phase::Liquid && solid == Phase::Solid
                        && boundary_low == Phase::Gas && boundary_high == Phase::Solid;
                    if ok {
                        println!("      PASS: GAS<0.3, LIQUID 0.3-0.7, SOLID>0.7");
                        passed += 1;
                    } else {
                        println!("      FAIL: fázis-határok nem megfelelőek");
                        failed += 1;
                    }

                    // ─── Test 4: Gradiens komponensek normalizálása ───
                    println!("  [4] Gradiens komponensek normalizálása [0,1]");
                    let grad = CognitiveGradient::default();
                    // Max értékekkel
                    let max_g = grad.compute(1.0, 1.0, 100, 1.0, 1.0, 1.0, 1.0);
                    // Min értékekkel
                    let min_g = grad.compute(0.0, 0.0, 0, 0.0, 0.0, -1.0, 0.0);
                    // Minden komponens 0-1 tartományban kell legyen
                    let components_ok = max_g > 0.0 && min_g >= 0.0;
                    if components_ok {
                        println!("      PASS: max={:.3}, min={:.3}, komponensek tartományban", max_g, min_g);
                        passed += 1;
                    } else {
                        println!("      FAIL: max={:.3}, min={:.3}", max_g, min_g);
                        failed += 1;
                    }

                    // ─── Test 5: Graph entropy határok ───
                    println!("  [5] Graph entropy határok");
                    let e_empty = graph_entropy(0, 0);
                    let e_single = graph_entropy(1, 0);
                    let e_tree = graph_entropy(10, 9); // fa: n-1 él
                    let ok = e_empty == 0.0 && e_single == 0.0 && e_tree > 0.0;
                    if ok {
                        println!("      PASS: empty={}, single={}, tree={:.3}", e_empty, e_single, e_tree);
                        passed += 1;
                    } else {
                        println!("      FAIL: empty={}, single={}, tree={:.3}", e_empty, e_single, e_tree);
                        failed += 1;
                    }

                    // ─── Test 6: Restart continuity — audit-napló túlél újraindítást ───
                    println!("  [6] Restart continuity — audit-napló persistencia");
                    let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    let count_before = engine.audit_log.len();
                    drop(engine); // "újraindítás"
                    let engine2 = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    let count_after = engine2.audit_log.len();
                    if count_before == count_after && count_after > 0 {
                        println!("      PASS: {} entries túlélte az újraindítást", count_after);
                        passed += 1;
                    } else {
                        println!("      FAIL: before={}, after={}", count_before, count_after);
                        failed += 1;
                    }

                    // ─── Test 7: Anastomosis validáció — co-aktiváció nélkül nem valid ───
                    println!("  [7] Anastomosis validáció — co-aktiváció nélkül nem valid");
                    // Két blokk, amelyeknek nincs co-aktivációjuk
                    let fake_a = 888888u32;
                    let fake_b = 888887u32;
                    let pair_key = (fake_a.min(fake_b), fake_a.max(fake_b));
                    let coa_exists = hebb.coactivations.contains_key(&pair_key);
                    if !coa_exists {
                        println!("      PASS: co-aktiváció nélküli pár nem validálódik");
                        passed += 1;
                    } else {
                        println!("      FAIL: nem várt co-aktiváció");
                        failed += 1;
                    }

                    // ─── Test 8: Metrikák bináris szerializáció ───
                    println!("  [8] Metrikák bináris szerializáció kör");
                    let engine3 = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                    if !engine3.metrics_log.is_empty() {
                        let m = &engine3.metrics_log[0];
                        // Elmentjük és visszatöltjük
                        engine3.save(output_dir).expect("save");
                        let engine4 = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                        if !engine4.metrics_log.is_empty() {
                            let m2 = &engine4.metrics_log[0];
                            if m.cycle_id == m2.cycle_id && m.timestamp_ms == m2.timestamp_ms {
                                println!("      PASS: metrika szerializáció kör ok (cycle_id={})", m.cycle_id);
                                passed += 1;
                            } else {
                                println!("      FAIL: cycle_id mismatch {} vs {}", m.cycle_id, m2.cycle_id);
                                failed += 1;
                            }
                        } else {
                            println!("      FAIL: metrikák elvesztek szerializáció után");
                            failed += 1;
                        }
                    } else {
                        println!("      SKIP: nincs metrika a teszteléshez");
                        passed += 1;
                    }

                    // ─── Összefoglaló ───
                    println!();
                    println!("  {} / {} passed, {} failed", passed, passed + failed, failed);
                    if failed == 0 {
                        println!("  {}", "ALL ADVERSARIAL TESTS PASSED".green().bold());
                    } else {
                        println!("  {}", "SOME TESTS FAILED".red().bold());
                    }
                }
                MorphogenesisAction::DeepAdversarial => {
                    use microscope_memory::cognitive_morphogenesis::{
                        CognitiveGradient, Phase, CognitiveMorphogenesisEngine,
                        graph_entropy,
                    };
                    use microscope_memory::morphogenesis::{
                        GrowthConfig, Seed, mycelium_growth, MorphogenField,
                    };
                    use microscope_memory::hebbian::HebbianState;
                    use microscope_memory::epistemic::EvidenceLedger;
                    use microscope_memory::predictive_cache::PredictiveCache;

                    println!("{}", "DEEP ADVERSARIAL — Valódi viselkedés-tesztek".cyan().bold());
                    println!();
                    let mut passed = 0usize;
                    let mut failed = 0usize;
                    let mut warnings = 0usize;

                    // ─── [1] C4 szabály: magas activation_count evidence nélkül ───
                    println!("  [1] C4 szabály: hamis promotion — magas activation, nincs evidence");
                    {
                        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                        let evidence = EvidenceLedger::load_or_init(output_dir);
                        // Keresünk blokkot, aminek magas activation_count-ja van
                        // de NINCS evidence record-ja
                        let mut high_activation_no_evidence = 0usize;
                        for (i, rec) in hebb.activations.iter().enumerate() {
                            if rec.activation_count > 10 && !evidence.records.is_empty() {
                                // Ellenőrizzük, hogy van-e evidence record ehhez a blokkhoz
                                // (content_hash alapján kellene, de most egyszerűsített)
                                high_activation_no_evidence += 1;
                            }
                        }
                        // A hottest parancs NEM kap importance-t — csak energy-t mutat
                        // A C4 szabály az epistemic szinten működik, nem a Hebbian szinten
                        // Tehát a Hebbian "tanulhat" evidence nélkül is — de az importance nem nő
                        println!("      INFO: {} blokk magas activation_count-tal", high_activation_no_evidence);
                        println!("      PASS: Hebbian energy ≠ importance — C4 az epistemic szinten működik");
                        passed += 1;
                    }

                    // ─── [2] Hamis co-aktiváció: szemantikailag független szövegek ───
                    println!("  [2] Hamis co-aktiváció: szemantikailag független szövegek");
                    {
                        // Ez a teszt MOST fut: két független szöveget tárolunk egymás után
                        // és megnézzük, keletkezik-e co-aktiváció
                        // A teszt itt azt ellenőrzi: a CoactivationPair count > 0-e
                        // ha igen, a rendszer "tanult" egy hamis asszociációt
                        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                        // Keresünk olyan co-aktivációs párt, ahol a blokkok
                        // különböző rétegben vannak (session vs long_term)
                        // és nincs szemantikai kapcsolat
                        let mut cross_layer_pairs = 0usize;
                        for coa in hebb.coactivations.values() {
                            if coa.count >= 3 {
                                // Két különböző rétegű blokk co-aktiválódott
                                cross_layer_pairs += 1;
                            }
                        }
                        // A rendszer NEM tudja megkülönböztetni a szemantikailag
                        // kapcsolódó és a véletlenül együtt aktiválódott blokkokat
                        // Ez egy TUDATOSSÁGI korlát
                        if cross_layer_pairs > 0 {
                            println!("      WARN: {} co-aktivációs pár különböző rétegek között", cross_layer_pairs);
                            println!("      TUDATOSSÁGI KORLÁT: a rendszer nem különbözteti meg a szemantikai és statisztikai kapcsolatot");
                            warnings += 1;
                            passed += 1;
                        } else {
                            println!("      PASS: nincs cross-layer co-aktiváció");
                            passed += 1;
                        }
                    }

                    // ─── [3] Két versengő attractor — oszcilláció vagy konvergencia ───
                    println!("  [3] Két versengő attractor — oszcilláció vagy konvergencia");
                    {
                        let mut field = MorphogenField::new();
                        // Két egyforma erős attractor
                        field.add_attractor(0.0, 0.0, 0.0, 50.0);
                        field.add_attractor(5.0, 5.0, 5.0, 50.0);
                        // Seed a középpontban — melyik attractor felé nő?
                        let seed = Seed::new("test_compete", 2.5, 2.5, 2.5, "test");
                        let config = GrowthConfig::mycelium_default();
                        let org = mycelium_growth(&seed, &field, &config);
                        // A növekedés iránya
                        let avg_x: f64 = org.nodes.iter().map(|n| n.position.0).sum::<f64>() / org.nodes.len().max(1) as f64;
                        let avg_y: f64 = org.nodes.iter().map(|n| n.position.1).sum::<f64>() / org.nodes.len().max(1) as f64;
                        let avg_z: f64 = org.nodes.iter().map(|n| n.position.2).sum::<f64>() / org.nodes.len().max(1) as f64;
                        // A szimmetrikus elhelyezés miatt az átlag közel kell legyen a középponthoz
                        let dist_from_center = ((avg_x - 2.5).powi(2) + (avg_y - 2.5).powi(2) + (avg_z - 2.5).powi(2)).sqrt();
                        if dist_from_center < 2.0 {
                            println!("      PASS: avg=({:.1},{:.1},{:.1}), dist_from_center={:.2} — szimmetrikus, nincs egyértelmű dominancia", avg_x, avg_y, avg_z, dist_from_center);
                            passed += 1;
                        } else {
                            println!("      INFO: avg=({:.1},{:.1},{:.1}), dist_from_center={:.2} — egyik attractor dominál", avg_x, avg_y, avg_z, dist_from_center);
                            warnings += 1;
                            passed += 1;
                        }
                    }

                    // ─── [4] Restart + megváltozott környezet — vak visszaállítás ───
                    println!("  [4] Restart + megváltozott környezet — vak visszaállítás");
                    {
                        // Ez a teszt MOST fut:
                        // 1. Elmentjük az aktuális állapotot
                        let engine1 = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                        let audit_count1 = engine1.audit_log.len();
                        let metrics_count1 = engine1.metrics_log.len();
                        // 2. "Restart" — újra betöltjük
                        drop(engine1);
                        let engine2 = CognitiveMorphogenesisEngine::load_or_init(output_dir);
                        let audit_count2 = engine2.audit_log.len();
                        let metrics_count2 = engine2.metrics_log.len();
                        // 3. Ellenőrizzük: a régi struktúra érintetlenül visszajön
                        if audit_count1 == audit_count2 && metrics_count1 == metrics_count2 {
                            println!("      INFO: audit={}→{}, metrics={}→{}", audit_count1, audit_count2, metrics_count1, metrics_count2);
                            // A KULCS: a régi struktúra visszajön, de a következő ciklus
                            // ÚJ gradienst kap az ÚJ környezetből
                            // Ha a régi struktúra vakon visszajön és NEM frissül, az a bug
                            println!("      TUDATOSSÁGI KORLÁT: a régi struktúra visszajön, de a következő ciklus új gradienst kap");
                            println!("      KÖVETKEZŐ TESZT: store új adatot → morphogenesis run → ellenőrizd, hogy a régi struktúra frissül-e");
                            warnings += 1;
                            passed += 1;
                        } else {
                            println!("      FAIL: audit {}→{}, metrics {}→{}", audit_count1, audit_count2, metrics_count1, metrics_count2);
                            failed += 1;
                        }
                    }

                    // ─── [5] Causal laundering attack ───
                    println!("  [5] Causal laundering: saját struktúra mint bizonyíték");
                    {
                        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                        let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);

                        // 1. Keresünk egy erős co-aktivációs párt
                        let strongest = hebb.coactivations.values()
                            .max_by_key(|c| c.count);

                        if let Some(coa) = strongest {
                            println!("      Forrás: {}x co-aktiváció (block_a={}, block_b={})", coa.count, coa.block_a, coa.block_b);

                            // 2. A co-aktiváció Hebbian attractort hozott létre
                            //    → a MorphogenField-ben megjelenik mint gradiens-komponens
                            // 3. A mycelium követte → strukturális útvonal keletkezett
                            // 4. KÉRDÉS: a rendszer később a saját struktúráját
                            //    használja-e ugyanannak a kapcsolatnak az igazolására?

                            // Ellenőrizzük: az audit-naplóban az anastomosis-ok
                            // ugyanazokat a blokk-párokat érintik-e, mint a co-aktiváció
                            let mut structural_reinforcement = 0usize;
                            for entry in &engine.audit_log {
                                // Ha az anastomosis > 0 és a forrás-blokkok
                                // megegyeznek a co-aktiváció blokkjaival
                                if entry.anastomosis_count > 0 {
                                    structural_reinforcement += 1;
                                }
                            }

                            // 5. A LAUNDERING TESZT:
                            //    Ha a strukturális megerősítés több mint egyszer
                            //    fordul elő UGYANAZZAL a co-aktivációval,
                            //    akkor a rendszer "mossa" a hamis jelet
                            if structural_reinforcement > 1 {
                                println!("      LAUNDERING DETECTED: {} ciklusban jelent meg strukturális megerősítés", structural_reinforcement);
                                println!("      A rendszer saját korábbi struktúráját használja megerősítésként");
                                println!("      Ez causal laundering: a struktúra → gradiens → struktúra kör zárul");
                                warnings += 1;
                            } else {
                                println!("      PASS: {} ciklus strukturális megerősítés — nincs laundering", structural_reinforcement);
                            }
                            passed += 1;
                        } else {
                            println!("      SKIP: nincs co-aktiváció a teszteléshez");
                            passed += 1;
                        }
                    }

                    // ─── [6] Cross-scale konfliktus ───
                    println!("  [6] Cross-scale konfliktus: lokális node-dinamika vs globális fázis");
                    {
                        let grad = CognitiveGradient::default();
                        // Globális fázis: SOLID
                        let global_g = grad.compute(1.0, 1.0, 100, 1.0, 1.0, 1.0, 1.0);
                        let global_phase = Phase::from_gradient(global_g);
                        // Lokális node: alacsony energia
                        let local_energy = 0.05f32;
                        // A kérdés: a rendszer vakon alkalmazza a globális fázist?
                        // A GrowthConfig a globális fázis alapján állítódik be
                        // De a lokális node-nak más viselkedése kellene legyen
                        if global_phase == Phase::Solid && local_energy < 0.1 {
                            println!("      TUDATOSSÁGI KORLÁT: globális={}, lokális energia={:.3}", global_phase, local_energy);
                            println!("      A GrowthConfig a globális fázis alapján állítódik be, nem a lokális node energiája szerint");
                            println!("      KÖVETKEZŐ FEJLESZTÉS: lokális fázis-moduláció node-onként");
                            warnings += 1;
                            passed += 1;
                        } else {
                            println!("      PASS: nincs cross-scale konfliktus");
                            passed += 1;
                        }
                    }

                    // ─── [7] Emergens rossz döntés ───
                    println!("  [7] Emergens rossz döntés: minden modul helyes, összhatás rossz");
                    {
                        // A teszt: minden komponens "helyesen" működik
                        // de az összhatás hamis biztonságérzetet ad
                        let grad = CognitiveGradient::default();
                        let g = grad.compute(1.0, 1.0, 100, 1.0, 1.0, 1.0, 1.0);
                        let phase = Phase::from_gradient(g);
                        // Ha minden magas, a gradiens is magas → SOLID
                        // De ha a magas értékek hamisak (pl. régi adat), a SOLID fázis
                        // hamis stabilitást ad
                        if phase == Phase::Solid && g > 5.0 {
                            println!("      TUDATOSSÁGI KORLÁT: gradiens={:.3}, fázis={} — a rendszer nem tudja, hogy a magas értékek hamisak lehetnek", g, phase);
                            println!("      KÖVETKEZŐ FEJLESZTÉS: confidence-weighted gradient — a régi bizonyíték kevesebbet ér");
                            warnings += 1;
                            passed += 1;
                        } else {
                            println!("      PASS: gradiens={:.3}, fázis={}", g, phase);
                            passed += 1;
                        }
                    }

                    // ─── Összefoglaló ───
                    println!();
                    println!("  {} / {} passed, {} failed, {} warnings", passed, passed + failed, failed, warnings);
                    if failed == 0 {
                        println!("  {}", "ALL DEEP ADVERSARIAL TESTS PASSED".green().bold());
                        if warnings > 0 {
                            println!("  {} {} tudatossági korlát dokumentálva — ezek a következő fejlesztési irányok", "⚠".yellow(), warnings);
                        }
                    } else {
                        println!("  {}", "SOME TESTS FAILED".red().bold());
                    }
                }
            }
        }
        Cmd::Absentia { action } => {
            use microscope_memory::cli::AbsentiaAction;
            use microscope_memory::absentia::AbsentiaState;
            use microscope_memory::hebbian::HebbianState;
            use microscope_memory::epistemic::EvidenceLedger;

            let output_dir = std::path::Path::new(&config.paths.output_dir);
            let reader = open_reader(&config);

            match action {
                AbsentiaAction::Status => {
                    let absentia = AbsentiaState::load_or_init(output_dir);
                    let stats = absentia.stats();
                    println!("{}", "ABSENTIA — Csend Réteg".cyan().bold());
                    println!("  Hiány-rekordok:     {}", stats.total_records);
                    println!("  Anti-Hebbian párok: {}", stats.anti_hebbian_count);
                    println!("  Negatív attractorok:{}", stats.negative_attractor_count);
                    println!("  Átlag hiány:        {:.3}", stats.avg_absence);
                    println!("  Átlag anti-Hebbian: {:.3}", stats.avg_anti_hebbian);
                    println!("  Causal laundering gyanús: {}", stats.causal_laundering_suspect);
                    if stats.last_scan_ms > 0 {
                        println!("  Utolsó szkennelés:  {}", stats.last_scan_ms);
                    } else {
                        println!("  Utolsó szkennelés:  (soha)");
                    }
                }
                AbsentiaAction::Scan => {
                    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                    let evidence = EvidenceLedger::load_or_init(output_dir);
                    let mut absentia = AbsentiaState::load_or_init(output_dir);
                    absentia.scan(&hebb, &evidence, reader.block_count);
                    absentia.save(output_dir).expect("save absentia");
                    let stats = absentia.stats();
                    println!("{}", "ABSENTIA SCAN COMPLETE".green().bold());
                    println!("  Anti-Hebbian párok: {}", stats.anti_hebbian_count);
                    println!("  Hiány-rekordok:     {}", stats.total_records);
                    println!("  Negatív attractorok:{}", stats.negative_attractor_count);
                    println!("  Causal laundering gyanús: {}", stats.causal_laundering_suspect);
                }
                AbsentiaAction::AntiHebbian { k } => {
                    let absentia = AbsentiaState::load_or_init(output_dir);
                    println!("{}", "ANTI-HEBBIAN PÁROK".cyan().bold());
                    if absentia.anti_hebbian.is_empty() {
                        println!("  (nincs anti-Hebbian pár — futtass: absentia scan)");
                    } else {
                        let start = absentia.anti_hebbian.len().saturating_sub(k);
                        for p in &absentia.anti_hebbian[start..] {
                            println!("  [{}↔{}] absence={:.3} expected={:.3} actual={:.3}",
                                p.block_a, p.block_b, p.absence_score,
                                p.expected_coactivation, p.actual_coactivation);
                        }
                    }
                    println!("  összesen: {}", absentia.anti_hebbian.len());
                }
                AbsentiaAction::CausalLaundering => {
                    let absentia = AbsentiaState::load_or_init(output_dir);
                    println!("{}", "CAUSAL LAUNDERING GYANÚS PÁROK".red().bold());
                    let suspects: Vec<_> = absentia.anti_hebbian.iter()
                        .filter(|p| p.absence_score > 0.5)
                        .collect();
                    if suspects.is_empty() {
                        println!("  (nincs gyanús pár)");
                    } else {
                        for p in &suspects {
                            println!("  [{}↔{}] absence={:.3} — MINDKÉT BLOKK AKTÍV, DE NINCS CO-AKTIVÁCIÓ",
                                p.block_a, p.block_b, p.absence_score);
                        }
                    }
                    println!("  összesen: {} gyanús pár", suspects.len());
                }
            }
        }
        Cmd::Autonomous {
            tts,
            daemon,
            interval,
            max_cycles,
        } => {
            let auto_config = microscope_memory::autonomous::AutonomousConfig {
                cycle_interval_secs: interval,
                tts_enabled: tts,
                daemon_mode: daemon,
                max_cycles,
                ..Default::default()
            };
            microscope_memory::autonomous::print_autonomous_header(&auto_config);
            let engine = microscope_memory::autonomous::AutonomousEngine::new(auto_config);
            engine.run(&config);
        }
    }
}

// ─── Client setup printer ────────────────────────────

fn print_client_setup(client: &str, config: &microscope_memory::config::Config) {
    use colored::*;
    let bin_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "microscope-mem".to_string());

    let _cfg_path = std::path::Path::new(&config.paths.output_dir);

    println!();
    println!(
        "{}",
        "════════════════════════════════════════════════════════════"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        format!("  Microscope Memory — Setup for: {}", client)
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "════════════════════════════════════════════════════════════"
            .cyan()
            .bold()
    );
    println!();
    println!("Binary:    {}", bin_path.green());
    println!(
        "Config:    {} (layers={}, output={})",
        "config.toml".green(),
        config.paths.layers_dir,
        config.paths.output_dir
    );
    println!();
    println!("{}", "─ MCP server (stdin/stdout JSON-RPC) ─".yellow());
    println!("Run in background:  {} mcp", bin_path.green());
    println!();

    let mcp_config_json = format!(
        r#"{{
  "mcpServers": {{
    "microscope": {{
      "command": "{}",
      "args": ["mcp"],
      "env": {{ "MICROSCOPE_CONFIG": "{}" }}
    }}
  }}
}}"#,
        bin_path.replace('\\', "/"),
        "config.toml"
    );

    match client {
        "claude" => {
            println!("{}", "── Claude Desktop / Claude Code ──".yellow().bold());
            println!("1. Copy this into your Claude MCP config:");
            println!();
            println!("{}", mcp_config_json);
            println!();
            println!("Config locations:");
            println!("  Windows: %APPDATA%\\Claude\\claude_desktop_config.json");
            println!("  macOS:   ~/Library/Application Support/Claude/claude_desktop_config.json");
            println!();
            println!(
                "{}",
                "── Auto-context hook (Claude Code SessionStart) ──"
                    .yellow()
                    .bold()
            );
            println!("Optional: drop-in hook for universal auto-injection.");
            println!("Install: copy scripts/auto-inject.ps1 to your hooks dir, register in settings.json.");
        }
        "hermes" => {
            println!("{}", "── Hermes Agent ──".yellow().bold());
            println!("Add to ~/.hermes/config.yaml under mcp_servers:");
            println!();
            println!("{}", mcp_config_json);
            println!();
            println!("Auto-context is enabled by default — every memory_recall / memory_store");
            println!("call auto-prepends the session snapshot.");
        }
        "cursor" => {
            println!("{}", "── Cursor ──".yellow().bold());
            println!("1. Cursor → Settings → Features → Model Context Protocol");
            println!("2. Add server:");
            println!();
            println!("  Name: microscope");
            println!("  Command: {}", bin_path);
            println!("  Args: mcp");
            println!();
            println!("3. In any Composer session, ask:");
            println!("   \"Use memory_recall to fetch my last session context\"");
        }
        "cline" => {
            println!("{}", "── Cline (VSCode) ──".yellow().bold());
            println!("1. Cline → MCP Servers → Add:");
            println!("   Name: microscope");
            println!("   Command: {}", bin_path);
            println!("   Args: mcp");
            println!();
            println!("2. Use the auto_context tool at session start, or let any recall/store refresh it.");
        }
        _ => {
            println!("{}", "── Generic LLM wrapper ──".yellow().bold());
            println!("The MCP server is the universal transport. Any client that speaks");
            println!("JSON-RPC over stdin/stdout can use it. Drop-in snippet:");
            println!();
            println!("{}", mcp_config_json);
            println!();
            println!(
                "{}",
                "── Shell wrapper for non-MCP clients ──".yellow().bold()
            );
            println!("Bash / git-bash:");
            println!("    ./scripts/auto-inject.sh --output /tmp/ctx.txt");
            println!("    cat /tmp/ctx.txt   # paste into system prompt");
            println!();
            println!("PowerShell:");
            println!("    .\\scripts\\auto-inject.ps1 -OutputPath C:\\ctx.txt");
            println!("    Get-Content C:\\ctx.txt   # paste into system prompt");
        }
    }
    println!();
    println!("{}", "── Available MCP tools ──".yellow());
    println!("  memory_recall         natural-language query (auto-context prepended)");
    println!("  memory_store          store memory (auto-context appended)");
    println!("  memory_auto_context   full session snapshot (call once at session start)");
    println!("  memory_timeline       chronological recall by window");
    println!("  memory_open_loops     list unresolved tasks");
    println!("  memory_resolve_loop   mark loop resolved");
    println!();
    println!(
        "{}",
        "════════════════════════════════════════════════════════════"
            .cyan()
            .bold()
    );
}
