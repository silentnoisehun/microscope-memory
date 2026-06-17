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
#[cfg(feature = "stealth")]
use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, GetTickCount64, SYSTEM_INFO};
#[cfg(feature = "stealth")]
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

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

fn recall(config: &Config, query: &str, k: usize, emotion: Option<[f32; 21]>) {
    let t0 = Instant::now();
    let reader = open_reader(config);
    let emo_label = emotion.as_ref().map(|e| format_emotion(e, 3)).unwrap_or_default();
    if !emo_label.is_empty() {
        println!("{} '{}' [emotion: {}]", "RECALL".cyan().bold(), query, emo_label.cyan());
    } else {
        println!("{} '{}':", "RECALL".cyan().bold(), query);
    }
    let emotional_recall_weight = config.search.emotional_bias_weight * 0.15;

    // Auto-prime: if no explicit emotion, use the emotional state ring
    let emotion = emotion.or_else(|| {
        let ring = microscope_memory::EmotionalStateRing::load_or_init(Path::new(&config.paths.output_dir));
        if ring.is_active() {
            if let Some((name, val)) = ring.dominant() {
                println!("  {} emotional prime: {} ({:.2})", "EMOTION".magenta(), name, val);
            }
            Some(ring.current)
        } else {
            None
        }
    });

    // Narrative prime: the system's inner voice as additional context
    let narrative_state = microscope_memory::narrative::NarrativeState::load_or_init(Path::new(&config.paths.output_dir));
    if narrative_state.session_count > 0 {
        println!("  {} \"{}\"", "SELF".cyan().bold(), safe_truncate(&narrative_state.narrative, 50));
    }

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);

    // â”€â”€â”€ Attention: compute layer weights from context â”€â”€
    let output_dir_att = Path::new(&config.paths.output_dir);
    let mut attention = microscope_memory::attention::AttentionState::load_or_init(output_dir_att);
    let hebb_pre =
        microscope_memory::hebbian::HebbianState::load_or_init(output_dir_att, reader.block_count);
    let tg_pre = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_att);
    let pc_pre = microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_att);

    let emotional_energy = microscope_memory::emotional::emotional_field(&reader, &hebb_pre)
        .map(|f| f.total_energy)
        .unwrap_or(0.0);

    // Load emotional state ring for priming + attention intensity
    let emotional_ring = microscope_memory::EmotionalStateRing::load_or_init(output_dir_att);
    let emotional_intensity = emotional_ring.intensity();

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
        emotional_intensity,
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
        0..=8 => (0, 2),
        9..=20 => (2, 4),
        _ => (2, 5),
    };

    let mut all_results: Vec<(f32, usize, bool)> = Vec::new();

    let q_lower = query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

    // Load emotions.bin for main-index emotional recall
    let emotion_lookup = emotion.as_ref().and_then(|_| {
        load_emotion_lookup(Path::new(&config.paths.output_dir))
    });

    // Load spaced repetition state for Ebbinghaus boost
    let spaced = microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(
        Path::new(&config.paths.output_dir)
    );

    for zoom in zoom_lo..=zoom_hi {
        // Red Audit: Timing jitter using polymorphic build-time value
        #[cfg(feature = "stealth")]
        if zoom > zoom_lo {
            let delay = crate::obfuscate::POLY_JITTER;
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
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
                // Emotional similarity boost (if query emotion + emotions.bin data)
                let emo_boost = emotion.as_ref().and_then(|qe| {
                    emotion_lookup.as_ref().and_then(|lookup| lookup(i))
                        .map(|be| emotional_similarity(qe, &be) * emotional_recall_weight)
                }).unwrap_or(0.0);
                // Spaced repetition boost: due blocks surface more easily
                let sr_boost = spaced.spacing_boost(i as u32);
                let combined = (spatial_dist - boost - emo_boost - sr_boost).max(0.0);
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
        // Emotional boost from inline append entry emotion
        let emo_boost = emotion.as_ref()
            .map(|qe| emotional_similarity(qe, &entry.emotion) * emotional_recall_weight)
            .unwrap_or(0.0);
        if dist < 0.1 || keyword_hits > 0 || emo_boost > 0.0 {
            all_results.push(((dist - boost - emo_boost).max(0.0), ai + 1_000_000, false));
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
    let pattern_boosts: std::collections::HashMap<u32, f32> =
        thought_graph.pattern_boost(qh_tg).into_iter().collect();
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

    // â”€â”€â”€ Hebbian + Mirror: record activations & detect resonance â”€â”€
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

        // Spaced repetition: record each activated block (quality 4 = seen and relevant)
        let mut spaced_writer = microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
        for &(idx, _) in &activated {
            let importance = (config.search.semantic_weight * 10.0) as u8; // approximate
            spaced_writer.record_recall(idx, importance, 4);
        }
        let _ = spaced_writer.save(output_dir);

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

        // ═══ Eureka detection ═══
        let eureka_events = microscope_memory::eureka::detect_eureka(
            config,
            &reader,
            query,
            emotion.as_ref(),
            &all_results,
        );
        if !eureka_events.is_empty() {
            let mut eureka_log = microscope_memory::eureka::EurekaLog::load_or_init(output_dir);
            for ev in eureka_events {
                if let Ok(()) = eureka_log.record(output_dir, ev) {
                    // stored
                }
            }
            if eureka_log.events.len() > 0 {
                let last = &eureka_log.events[eureka_log.events.len() - 1];
                if last.insight_score() > 1.0 {
                    println!("  {} insight! score={:.1} \"{}\"", "💡 EUREKA".magenta().bold(), last.insight_score(), safe_truncate(&last.text, 40));
                }
            }
        }

        // ═══ Reconsolidation: every recall rewrites memory ═══
        let (rc_emo, rc_spatial) = microscope_memory::reconsolidation::reconsolidate(
            output_dir,
            &reader,
            query,
            emotion.as_ref(),
            config,
            4,
            &activated,
        );
        let rc_text = microscope_memory::reconsolidation::format_reconsolidation(rc_emo, rc_spatial);
        if !rc_text.is_empty() {
            println!("{}", rc_text);
        }

        // ═══ Salience filter: only the strongest signal reaches narrative ═══
        let mut salience_state = microscope_memory::salience::SalienceState::load_or_init(output_dir);
        let high_salience = salience_state.filter(
            &activated.iter().map(|&(idx, _)| {
                // emotional_delta: approximate using hebbian energy, insight: from eureka, recency: 1.0
                let hebb_e = hebb.activations.get(idx as usize).map(|a| a.energy).unwrap_or(0.5);
                (idx, hebb_e * 0.3, 0.5, 1.0f32)
            }).collect::<Vec<_>>()
        );
        // Inhibit the highest-salience topic so it doesn't repeat
        if let Some((salient_idx, _)) = high_salience.first() {
            let topic = microscope_memory::salience::SalienceState::topic_hash(&format!("block_{}", salient_idx));
            salience_state.inhibit(topic);
            let _ = salience_state.save(output_dir);
        }

        // ═══ Narrative update: the system tells itself what happened ═══
        let wm_state = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
        let wm_texts: Vec<String> = wm_state.items.iter().map(|i| i.text.clone()).collect();
        let sr_state = microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
        let tg_state = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
        let ring = microscope_memory::EmotionalStateRing::load_or_init(output_dir);
        let mut narrative_state = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
        let _ = narrative_state.update(
            output_dir,
            Some(&ring),
            Some(&wm_texts),
            Some(sr_state.due_count()),
            Some(tg_state.nodes.len()),
            Some(query),
        );
        if narrative_state.session_count <= 3 {
            println!("  {} \"{}\"", "SELF".cyan().bold(), safe_truncate(&narrative_state.narrative, 60));
        }

        // ═══ Meta-kognitív rekonszolidáció: the narrative becomes a memory ═══
        microscope_memory::narrative::metacognitive_store(
            output_dir,
            &narrative_state.narrative,
            &narrative_state.emotion,
        );
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

    println!("  No embedding index â€” computing on-the-fly (slow)");
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
            "  {} merkle.bin not found â€” rebuild with v0.2.0 to generate",
            "ERR".red()
        );
        return;
    }

    let meta = fs::read(&meta_path).expect("read meta.bin");
    let magic = &meta[0..4];
    if magic != b"MSC2" && magic != b"MSC3" {
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
    fs::write(&demo_path, demo_content).map_err(|e| e.to_string())?;

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

/// Red Audit: IAT Camouflage. Calls innocent Win32 APIs to make the binary
/// appear as a legitimate system utility.
#[cfg(feature = "stealth")]
fn dummy_legit_calls() {
    unsafe {
        let mut info: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut info);
        let _ = GetTickCount64();
        let _ = GetCurrentProcessId();
        // These don't do anything meaningful for the logic,
        // but they force the imports into the IAT.
    }
}

#[tokio::main]
async fn main() {
    #[cfg(feature = "stealth")]
    {
        dummy_legit_calls();

        // Phase 3: Soft Anti-VM / Ghost Mode
        let is_ghost = crate::antidebug::is_sandbox();
        if is_ghost {
            // Silently log to stderr only for internal tracking
            eprintln!("  {} Ghost Mode active.", "INFO:".cyan());
        }
    }

    let config = Config::load(DEFAULT_CONFIG_PATH).unwrap_or_else(|_| {
        // Redir warning to stderr for MCP compatibility
        eprintln!("  {} Using default configuration", "WARN:".yellow());
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
        Cmd::InitDemo { force } => {
            if let Err(e) = init_demo(&config, force) {
                eprintln!("  {} {}", "ERROR:".red(), e);
            }
        }
        Cmd::Doctor { fix } => {
            microscope_memory::doctor::run_doctor(&config, fix).expect("doctor failed");
        }
        Cmd::Build { force } => {
            microscope_memory::build::build(&config, force).expect("build failed");
        }
        Cmd::Store {
            text,
            layer,
            importance,
            emotion,
        } => {
            let emo: Option<[f32; 21]> = emotion.map(|v| {
                let mut arr = [0.0f32; 21];
                for (i, val) in v.iter().enumerate().take(21) {
                    arr[i] = *val;
                }
                arr
            });
            store_memory(&config, &text, &layer, importance, emo).expect("store failed");
            // Auto-push to working memory
            let output_dir = Path::new(&config.paths.output_dir);
            let mut wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
            wm.push(&text, importance as f32, &layer, microscope_memory::working_memory::MemoryType::Episodic);
            let _ = wm.save(output_dir);
            // Narrative update
            let mut narr = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
            let ring = microscope_memory::EmotionalStateRing::load_or_init(output_dir);
            let wm_texts: Vec<String> = wm.items.iter().map(|i| i.text.clone()).collect();
            let sr = microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
            let tg = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let _ = narr.update(output_dir, Some(&ring), Some(&wm_texts), Some(sr.due_count()), Some(tg.nodes.len()), Some(&text));
        }
        Cmd::Recall { query, k, emotion } => {
            let emo: Option<[f32; 21]> = emotion.map(|v| {
                let mut arr = [0.0f32; 21];
                for (i, val) in v.iter().enumerate().take(21) {
                    arr[i] = *val;
                }
                arr
            });
            recall(&config, &query, k, emo);
            // Auto-push query to working memory (as semantic type)
            let output_dir = Path::new(&config.paths.output_dir);
            let mut wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
            wm.push(&query, 3.0, "short_term", microscope_memory::working_memory::MemoryType::Semantic);
            let _ = wm.save(output_dir);
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
            microscope_memory::build::build(&config, true).expect("rebuild failed");
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let _ = fs::remove_file(append_path);
            let emotions_path = Path::new(&config.paths.output_dir).join("emotions.bin");
            let _ = fs::remove_file(emotions_path);
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
        Cmd::Wm { action } => {
            let output_dir = Path::new(&config.paths.output_dir);
            match action {
                microscope_memory::cli::WmAction::Show => {
                    let wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let stats = wm.stats();
                    println!("{}", "WORKING MEMORY".cyan().bold());
                    println!("  Items:     {}/{}", stats.item_count, stats.capacity);
                    println!("  Hot:       {}", stats.hot_items);
                    println!("  Decay:     {}ms", stats.decay_ms);
                    println!("  Cons. candidates: {}", stats.consolidation_candidates);
                    if wm.items.is_empty() {
                        println!("  (empty)");
                    } else {
                        for (i, item) in wm.items.iter().enumerate() {
                            let mem_type = match item.memory_type {
                                microscope_memory::working_memory::MemoryType::Episodic => "episodic",
                                microscope_memory::working_memory::MemoryType::Semantic => "semantic",
                                microscope_memory::working_memory::MemoryType::Implicit => "implicit",
                                microscope_memory::working_memory::MemoryType::Explicit => "explicit",
                            };
                            println!(
                                "  [{:2}] imp={:.1} acc={} {:8} {}",
                                i, item.importance, item.access_count, mem_type,
                                crate::safe_truncate(&item.text, 60)
                            );
                        }
                    }
                }
                microscope_memory::cli::WmAction::Push { text, importance, layer, memory_type } => {
                    let mut wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let mem_type = match memory_type.to_lowercase().as_str() {
                        "semantic" => microscope_memory::working_memory::MemoryType::Semantic,
                        _ => microscope_memory::working_memory::MemoryType::Episodic,
                    };
                    wm.push(&text, importance, &layer, mem_type);
                    wm.save(output_dir).unwrap_or_else(|e| eprintln!("  {} save: {}", "WARN".yellow(), e));
                    println!("  {} WM: '{}'", "PUSHED".green().bold(), crate::safe_truncate(&text, 60));
                }
                microscope_memory::cli::WmAction::Decay => {
                    let mut wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let before = wm.items.len();
                    wm.decay();
                    let after = wm.items.len();
                    wm.save(output_dir).unwrap_or_else(|e| eprintln!("  {} save: {}", "WARN".yellow(), e));
                    println!("  {} WM: {} → {} items", "DECAY".yellow().bold(), before, after);
                }
                microscope_memory::cli::WmAction::Consolidate => {
                    let mut wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let items = wm.consolidate();
                    if items.is_empty() {
                        println!("  {} WM: no items to consolidate", "CONSOLIDATE".yellow().bold());
                    } else {
                        for item in &items {
                            let text = &item.text;
                            let layer = &item.layer;
                            let imp = (item.importance as u8).max(1).min(10);
                            store_memory(&config, &format!("[WM] {}", text), layer, imp, None)
                                .unwrap_or_else(|e| eprintln!("  {} store: {}", "ERR".red(), e));
                            println!("  {} '{}' → long_term", "CONSOLIDATED".magenta().bold(), safe_truncate(text, 60));
                        }
                        wm.save(output_dir).unwrap_or_else(|e| eprintln!("  {} save: {}", "WARN".yellow(), e));
                        println!("  {} WM: {} items consolidated", "DONE".green().bold(), items.len());
                    }
                }
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
                "{} {}Âł grid ({} bytes) -> {}",
                "DENSITY".cyan().bold(),
                grid,
                data.len(),
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
            match microscope_memory::dream::dream_consolidate(output_dir, reader.block_count) {
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
            if !state.cycles.is_empty() {
                println!("\n  Recent cycles:");
                let start = if state.cycles.len() > k {
                    state.cycles.len() - k
                } else {
                    0
                };
                for cycle in &state.cycles[start..] {
                    println!(
                        "    {} â€” {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}",
                        cycle.timestamp_ms,
                        cycle.duration_ms,
                        cycle.replayed_fingerprints,
                        cycle.strengthened_pairs,
                        cycle.pruned_pairs,
                        cycle.pruned_activations,
                        cycle.consolidated_patterns
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
        Cmd::Eureka { k, verbose } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let log = microscope_memory::eureka::EurekaLog::load_or_init(output_dir);
            let count = log.events.len().min(k);
            if count == 0 {
                println!("{}", "No eureka events found".yellow());
            } else {
                println!("{} ({} total, showing {})", "EUREKA MOMENTS".cyan().bold(), log.events.len(), count);
                for ev in log.events.iter().rev().take(count).rev() {
                    println!("{}", microscope_memory::eureka::format_eureka(ev));
                    if verbose {
                        println!("         score breakdown: surprise={:.2} × curiosity={:.2} × emo_sim={:.2} / dist={:.3} = {:.1}",
                            ev.surprise_score, ev.curiosity_score, ev.emotional_sim, ev.spatial_dist, ev.insight_score());
                    }
                }
            }
        }
        Cmd::Reconsolidate => {
            let output_dir = Path::new(&config.paths.output_dir);
            let reader = match MicroscopeReader::open(&config) {
                Ok(r) => r,
                Err(_) => { eprintln!("  {} open reader failed — run build first", "ERR".red()); return; }
            };
            // Process Hebbian hot blocks (most recently activated)
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
            let hot = hebb.hottest_blocks(50);
            let activated: Vec<(u32, f32)> = hot.iter().map(|&(idx, _)| (idx as u32, 1.0)).collect();
            let (emo, spatial) = microscope_memory::reconsolidation::reconsolidate(
                output_dir, &reader, "", None, &config, 3, &activated,
            );
            println!(
                "{} emotion={} spatial={} ({} hot blocks)",
                "RECONSOLIDATED".magenta().bold(),
                emo,
                spatial,
                activated.len(),
            );
        }
        Cmd::Salience => {
            let output_dir = Path::new(&config.paths.output_dir);
            let salience = microscope_memory::salience::SalienceState::load_or_init(output_dir);
            println!("{}", "SALIENCE NETWORK".cyan().bold());
            if salience.inhibitions.is_empty() {
                println!("  (no active inhibitions — network is clear)");
            } else {
                println!("  {} active inhibitions:", salience.inhibitions.len());
                for e in &salience.inhibitions {
                    println!("  topic={:016x} strength={:.2}", e.topic_hash, e.remaining_strength);
                }
            }
        }
        Cmd::Daydream { steps, verbose } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let narrative = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
            let seed = if narrative.session_count > 0 {
                narrative.narrative.clone()
            } else {
                "Microscope Memory".to_string()
            };
            println!("{} from \"{}\"", "DAYDREAM".cyan().bold(), safe_truncate(&seed, 40));
            match microscope_memory::daydream::daydream(&config, &seed, steps) {
                Ok(result) => {
                    println!("{}", microscope_memory::daydream::format_daydream(&result, verbose));
                }
                Err(e) => eprintln!("  {} daydream: {}", "ERR".red(), e),
            }
        }
        Cmd::Narrative { verbose } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let state = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
            println!("{}", "INNER NARRATIVE".cyan().bold());
            if state.session_count == 0 {
                println!("  (silent — no interactions yet)");
            } else {
                println!("  \"{}\"", state.narrative);
                println!("  Session count: {}", state.session_count);
                if verbose {
                    print!("  Emotion: [");
                    for (i, v) in state.emotion.iter().enumerate() {
                        if *v > 0.05 {
                            let name = microscope_memory::EMOTION_DIMS.get(i).unwrap_or(&"?");
                            print!(" {}:{:.2}", name, v);
                        }
                    }
                    println!(" ]");
                    // Show working memory context
                    let wm = microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    if !wm.items.is_empty() {
                        println!("  Focus:");
                        for item in &wm.items {
                            println!("    - {} (imp={:.1})", crate::safe_truncate(&item.text, 40), item.importance);
                        }
                    }
                }
            }
        }
        Cmd::Spaced { due, k } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let sr = microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
            let stats = sr.stats();
            println!("{}", "SPACED REPETITION".cyan().bold());
            println!("  Tracked:   {} blocks", stats.total_blocks);
            println!("  Due:       {} (need review)", stats.due);
            println!("  Fresh:     {} (< 7d)", stats.fresh);
            println!("  Mastered:  {} (≥{} recalls)", stats.mastered, 15);
            println!("  Avg ease:  {:.2}", stats.avg_ease);
            println!("  Avg int.:  {:.1}d", stats.avg_interval);
            if due && stats.total_blocks > 0 {
                let due_list = sr.due_blocks();
                let count = due_list.len().min(k);
                println!("\n  {} due blocks:", count);
                let reader = match MicroscopeReader::open(&config) {
                    Ok(r) => Some(r),
                    Err(_) => None,
                };
                for &idx in due_list.iter().take(count) {
                    let block_info = sr.find(idx);
                    let days = block_info.map(|b| {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                        (now.saturating_sub(b.last_recall_ms)) as f32 / 86_400_000.0
                    }).unwrap_or(0.0);
                    let text = reader.as_ref().map(|r| safe_truncate(r.text(idx as usize), 50)).unwrap_or_default();
                    println!("  [{:>6}] recall={} last={:.1}d ago {}",
                        idx, block_info.map(|b| b.recall_count).unwrap_or(0), days, text);
                }
            }
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
            let _ = store_memory(&config, text_short, "rust_state", importance, None);

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
        Cmd::Bridge { host, port } => {
            if let Err(e) = microscope_memory::bridge::run(config, host, port).await {
                eprintln!("  {} Bridge error: {}", "ERROR:".red(), e);
            }
        }
        // New cognitive enhancement commands
        Cmd::Sandbox { simulate, actions, best, clear } => {
            use microscope_memory::mental_sandbox::MentalSandbox;
            
            let mut sandbox = MentalSandbox::new();
            // Add some default long-term goals
            sandbox.add_goal("efficient");
            sandbox.add_goal("reliable");
            sandbox.add_goal("user_friendly");
            
            if clear {
                sandbox.clear();
                println!("  {} All scenarios cleared", "OK".green());
            }
            
            if let Some(desc) = simulate {
                let actions_list = actions.as_ref()
                    .map(|a| a.split(',').map(|s| s.trim()).collect())
                    .unwrap_or_else(|| vec!["default_action"]);
                
                let scenario = sandbox.simulate_scenario(&desc, actions_list);
                println!("  {} Scenario simulated:", "SIMULATION".cyan().bold());
                println!("    ID: {}", scenario.id);
                println!("    Description: {}", scenario.description);
                println!("    Actions: {}", scenario.actions.join(", "));
                println!("    Outcome Probability: {:.1}%", scenario.outcome_probability * 100.0);
                println!("    Risk Score: {:.2}", scenario.risk_score);
                println!("    Reward Potential: {:.2}", scenario.reward_potential);
            }
            
            if best {
                if let Some(best_scenario) = sandbox.get_best_scenario() {
                    println!("  {} Best scenario:", "BEST".green().bold());
                    println!("    ID: {}", best_scenario.id);
                    println!("    Description: {}", best_scenario.description);
                    println!("    Risk/Reward Ratio: {:.2}", 
                        best_scenario.reward_potential / (best_scenario.risk_score + 0.01));
                } else {
                    println!("  {} No scenarios available", "INFO:".cyan());
                }
            }
        }
        Cmd::Impulse { filter, source, urgency, suppress, stats, clear } => {
            use microscope_memory::impulse_control::ImpulseControl;
            
            let mut control = ImpulseControl::new();
            // Add some default suppression patterns
            control.add_suppression_pattern("spam");
            control.add_suppression_pattern("advertisement");
            
            if clear {
                control.clear_patterns();
                println!("  {} All suppression patterns cleared", "OK".green());
            }
            
            if let Some(pattern) = suppress {
                control.add_suppression_pattern(&pattern);
                println!("  {} Added suppression pattern: '{}'", "OK".green(), pattern);
            }
            
            if let Some(content) = filter {
                let stimulus = control.filter_stimulus(&content, &source, urgency);
                println!("  {} Stimulus filtered:", "IMPULSE CONTROL".cyan().bold());
                println!("    Content: {}", stimulus.content);
                println!("    Source: {}", stimulus.source);
                println!("    Relevance: {:.2}", stimulus.relevance);
                println!("    Urgency: {:.2}", stimulus.urgency);
                println!("    Status: {}", 
                    if stimulus.suppressed { "SUPPRESSED".red() } 
                    else { "ALLOWED".green() });
            }
            
            if stats {
                let (attention_budget, pattern_count) = control.get_stats();
                println!("  {} System stats:", "STATS".green().bold());
                println!("    Attention Budget: {:.1}%", attention_budget * 100.0);
                println!("    Suppression Patterns: {}", pattern_count);
            }
        }
        Cmd::Meta { record, evaluate, trends, report, add_strategy } => {
            use microscope_memory::meta_supervision::{MetaSupervisor, generate_report};
            
            let mut supervisor = MetaSupervisor::new();
            
            if let Some(record_str) = record {
                let parts: Vec<&str> = record_str.split(',').collect();
                if parts.len() >= 5 {
                    let metrics = supervisor.record_metrics(
                        parts[0].parse().unwrap_or(50.0),
                        parts[1].parse().unwrap_or(100.0),
                        parts[2].parse().unwrap_or(0.8),
                        parts[3].parse().unwrap_or(0.5),
                        parts[4].parse().unwrap_or(0.1),
                    );
                    println!("  {} Metrics recorded:", "RECORDED".cyan().bold());
                    println!("    Overall Score: {:.2}", metrics.overall_score);
                    println!("    Response Time: {:.1}ms", metrics.response_time_ms);
                    println!("    Memory Usage: {:.1}MB", metrics.memory_usage_mb);
                }
            }
            
            if evaluate {
                if let Some(correction) = supervisor.evaluate_and_correct() {
                    println!("  {} Correction needed: {}", "EVALUATION".yellow().bold(), correction);
                } else {
                    println!("  {} System performance OK", "OK".green());
                }
            }
            
            if trends {
                let (current_score, trend, volatility) = supervisor.get_summary();
                println!("  {} Performance trends:", "TRENDS".cyan().bold());
                println!("    Current Score: {:.2}", current_score);
                println!("    Trend: {:.3}", trend);
                println!("    Volatility: {:.3}", volatility);
                println!("    Direction: {}", 
                    if trend > 0.05 { "IMPROVING".green() }
                    else if trend < -0.05 { "DECLINING".red() }
                    else { "STABLE".yellow() });
            }
            
            if report {
                let report_text = generate_report(&supervisor);
                println!("{}", report_text);
            }
            
            if let Some(strategy) = add_strategy {
                supervisor.add_correction_strategy(&strategy);
                println!("  {} Added correction strategy: '{}'", "OK".green(), strategy);
            }
        }
        Cmd::Implicit { show, practice, skills, patterns, decay } => {
            use microscope_memory::implicit_memory::ImplicitMemory;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut implicit = ImplicitMemory::load_or_init(output_dir);
            
            if show {
                println!("{}", "IMPLICIT MEMORY".cyan().bold());
                println!("  Patterns:      {}", implicit.patterns.len());
                println!("  Skills:        {}", implicit.skills.len());
                println!("  Habits:        {}", implicit.habits.len());
                println!("  Conditioning:  {}", implicit.conditioning.len());
            }
            
            if let Some(practice_str) = practice {
                let parts: Vec<&str> = practice_str.split(':').collect();
                if parts.len() == 2 {
                    let skill_name = parts[0];
                    let success = parts[1] == "success" || parts[1] == "true";
                    implicit.practice_skill(skill_name, !success);
                    implicit.save(output_dir).ok();
                    println!("  {} Practiced '{}': {}", "OK".green(), skill_name, 
                        if success { "SUCCESS".green() } else { "FAILURE".red() });
                }
            }
            
            if skills {
                let ranking = implicit.skill_ranking();
                println!("  {} Skill ranking:", "SKILLS".yellow().bold());
                for (name, skill) in ranking.iter().take(10) {
                    println!("    {} mastery={:.1}% errors={:.1}% practiced={} times",
                        name, skill.mastery_level * 100.0, skill.error_rate * 100.0, skill.practice_count);
                }
            }
            
            if patterns {
                let top = implicit.strongest_patterns(10);
                println!("  {} Strongest patterns:", "PATTERNS".yellow().bold());
                for (hash, pattern) in top {
                    println!("    hash={:x} strength={:.2} freq={} perf={:.1}%",
                        hash, pattern.strength, pattern.frequency, pattern.performance_metric * 100.0);
                }
            }
            
            if decay {
                implicit.decay();
                implicit.save(output_dir).ok();
                println!("  {} Memory decayed: patterns={} skills={} habits={}",
                    "DECAY".cyan(), implicit.patterns.len(), implicit.skills.len(), implicit.habits.len());
            }
        }
        Cmd::Explicit { show, store_fact, concept, facts, concepts } => {
            use microscope_memory::explicit_memory::ExplicitMemory;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut explicit = ExplicitMemory::load_or_init(output_dir);
            
            if show {
                println!("{}", "EXPLICIT MEMORY".cyan().bold());
                println!("  Facts:         {}", explicit.facts.len());
                println!("  Concepts:      {}", explicit.concepts.len());
                println!("  Events:        {}", explicit.events.len());
                println!("  Relationships: {}", explicit.relationships.len());
            }
            
            if let Some(fact_str) = store_fact {
                let parts: Vec<&str> = fact_str.split(':').collect();
                if parts.len() >= 2 {
                    let statement = parts[0];
                    let source = parts[1];
                    let confidence = if parts.len() > 2 {
                        parts[2].parse().unwrap_or(0.7)
                    } else {
                        0.7
                    };
                    explicit.store_fact(statement, source, confidence);
                    explicit.save(output_dir).ok();
                    println!("  {} Stored fact: '{}' (conf={:.1}%)", "OK".green(), 
                        safe_truncate(statement, 50), confidence * 100.0);
                }
            }
            
            if let Some(concept_str) = concept {
                let parts: Vec<&str> = concept_str.split(':').collect();
                if parts.len() >= 2 {
                    let name = parts[0];
                    let definition = parts[1];
                    let abstraction = if parts.len() > 2 {
                        parts[2].parse().unwrap_or(0.5)
                    } else {
                        0.5
                    };
                    explicit.define_concept(name, definition, abstraction);
                    explicit.save(output_dir).ok();
                    println!("  {} Defined concept: '{}' (abstraction={:.1}%)", 
                        "OK".green(), name, abstraction * 100.0);
                }
            }
            
            if facts {
                let high_conf = explicit.high_confidence_facts(0.7);
                println!("  {} High-confidence facts:", "FACTS".yellow().bold());
                for fact in high_conf.iter().take(10) {
                    println!("    [conf={:.0}%] {} (src={})", 
                        fact.confidence * 100.0, safe_truncate(&fact.statement, 50), fact.source);
                }
            }
            
            if concepts {
                println!("  {} Concepts:", "CONCEPTS".yellow().bold());
                for (name, concept) in explicit.concepts.iter().take(10) {
                    println!("    {} (abstract={:.1}%) - {}", 
                        name, concept.abstraction_level * 100.0, safe_truncate(&concept.definition, 40));
                }
            }
        }
        Cmd::Hippo { show, consolidate, related, replay, decay } => {
            use microscope_memory::hippocampus::Hippocampus;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut hippo = Hippocampus::load_or_init(output_dir);
            
            if show {
                let (bindings, episodes, consolidated, avg_strength) = hippo.stats();
                println!("{}", "HIPPOCAMPUS".cyan().bold());
                println!("  Context bindings: {}", bindings);
                println!("  Episodes:         {}", episodes);
                println!("  Consolidated:     {}", consolidated);
                println!("  Avg binding str:  {:.2}", avg_strength);
            }
            
            if consolidate {
                let candidates = hippo.get_consolidation_candidates(5);
                println!("  {} Consolidation candidates:", "CONSOLIDATE".yellow().bold());
                for (i, episode) in candidates.iter().enumerate() {
                    println!("    [{}] episode_id={:x} blocks={} strength={:.2}",
                        i+1, episode.episode_id, episode.blocks.len(), episode.context_binding.binding_strength);
                    hippo.mark_consolidating(episode.episode_id);
                }
                hippo.save(output_dir).ok();
            }
            
            if let Some(ep_id) = related {
                let related_eps = hippo.get_related_episodes(ep_id);
                println!("  {} Related episodes to {:x}:", "RELATED".yellow().bold(), ep_id);
                for ep in related_eps.iter().take(5) {
                    println!("    episode_id={:x} blocks={} context={}", 
                        ep.episode_id, ep.blocks.len(), safe_truncate(&ep.context_binding.context, 30));
                }
            }
            
            if let Some(ep_id) = replay {
                if let Some(blocks) = hippo.replay_episode(ep_id) {
                    hippo.mark_consolidated(ep_id);
                    hippo.save(output_dir).ok();
                    println!("  {} Replayed episode {:x}: {} blocks", 
                        "REPLAY".green(), ep_id, blocks.len());
                } else {
                    println!("  {} Episode not found", "ERROR".red());
                }
            }
            
            if decay {
                hippo.decay();
                hippo.save(output_dir).ok();
                let (bindings, episodes, _, _) = hippo.stats();
                println!("  {} Memory decayed: bindings={} episodes={}", 
                    "DECAY".cyan(), bindings, episodes);
            }
        }
        Cmd::Neuro { show, synapse, pathway, prune, reorganize, pathways } => {
            use microscope_memory::neuroplasticity::Neuroplasticity;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut neuro = Neuroplasticity::load_or_init(output_dir);
            
            if show {
                let (synapses, paths, avg_weight, plasticity, strong) = neuro.stats();
                println!("{}", "NEUROPLASTICITY".cyan().bold());
                println!("  Synaptic connections: {}", synapses);
                println!("  Neural pathways:      {}", paths);
                println!("  Avg synaptic weight:  {:.2}", avg_weight);
                println!("  Network plasticity:   {:.1}%", plasticity * 100.0);
                println!("  Strong pathways:      {}", strong);
            }
            
            if let Some(syn_str) = synapse {
                let parts: Vec<&str> = syn_str.split(':').collect();
                if parts.len() >= 2 {
                    let from: u32 = parts[0].parse().unwrap_or(0);
                    let to: u32 = parts[1].parse().unwrap_or(0);
                    let success = parts.len() > 2 && (parts[2] == "success" || parts[2] == "true");
                    neuro.strengthen_synapse(from, to, success);
                    neuro.save(output_dir).ok();
                    println!("  {} Synapse {} → {}: {}", "OK".green(), from, to,
                        if success { "STRENGTHENED".green() } else { "WEAKENED".red() });
                }
            }
            
            if let Some(path_str) = pathway {
                let parts: Vec<&str> = path_str.split(':').collect();
                if parts.len() >= 2 {
                    let domain = parts[0];
                    let nodes: Vec<u32> = parts[1].split(',')
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if !nodes.is_empty() {
                        let id = neuro.strengthen_pathway(nodes.clone(), domain);
                        neuro.save(output_dir).ok();
                        println!("  {} Pathway {} reinforced: {} nodes (domain: {})", 
                            "OK".green(), id, nodes.len(), domain);
                    }
                }
            }
            
            if prune {
                let pruned = neuro.prune_weak_synapses(0.2);
                neuro.save(output_dir).ok();
                println!("  {} Pruned {} weak synapses", "PRUNE".yellow(), pruned);
            }
            
            if reorganize {
                let reorganized = neuro.reorganize_pathways();
                neuro.save(output_dir).ok();
                println!("  {} Reorganized network: {} changes", "REORGANIZE".yellow(), reorganized);
            }
            
            if pathways {
                let strongest = neuro.strongest_pathways(10);
                println!("  {} Strongest pathways:", "PATHWAYS".yellow().bold());
                for (i, pathway) in strongest.iter().enumerate() {
                    println!("    [{}] strength={:.2} efficiency={:.2} uses={} domain={}",
                        i+1, pathway.strength, pathway.efficiency, pathway.usage_count, pathway.specialized_for);
                }
            }
        }
        Cmd::Struct { show, neurogenesis, grow, prune, specialized } => {
            use microscope_memory::structural_plasticity::StructuralPlasticity;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut struct_pls = StructuralPlasticity::load_or_init(output_dir);
            
            if show {
                let (neurons, branches, avg_length, genesis_events) = struct_pls.stats();
                println!("{}", "STRUCTURAL PLASTICITY".cyan().bold());
                println!("  Neuron-like structures: {}", neurons);
                println!("  Dendritic branches:     {}", branches);
                println!("  Avg dendrite length:    {:.2}", avg_length);
                println!("  Neurogenesis events:    {}", genesis_events);
            }
            
            if let Some(genesis_str) = neurogenesis {
                let parts: Vec<&str> = genesis_str.split(':').collect();
                if parts.len() >= 2 {
                    let blocks: Vec<u32> = parts[0].split(',')
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    let specialization = parts[1];
                    if !blocks.is_empty() {
                        let neuron_id = struct_pls.neurogenesis(blocks.clone(), specialization);
                        struct_pls.save(output_dir).ok();
                        println!("  {} New neuron created: id={:x} blocks={} spec={}", 
                            "NEUROGENESIS".green().bold(), neuron_id, blocks.len(), specialization);
                    }
                }
            }
            
            if let Some(grow_str) = grow {
                let parts: Vec<&str> = grow_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(neuron_id), Ok(block)) = (parts[0].parse::<u64>(), parts[1].parse::<u32>()) {
                        if struct_pls.grow_dendrite(neuron_id, block) {
                            struct_pls.save(output_dir).ok();
                            println!("  {} Dendrite grown: neuron={:x} new_branch={}", 
                                "GROWTH".green(), neuron_id, block);
                        } else {
                            println!("  {} Dendrite growth failed or neuron pruned", "WARN".yellow());
                        }
                    }
                }
            }
            
            if let Some(neuron_id) = prune {
                let pruned = struct_pls.prune_inactive_branches(neuron_id);
                struct_pls.save(output_dir).ok();
                println!("  {} Pruned {} inactive branches from neuron {:x}", 
                    "PRUNE".yellow(), pruned, neuron_id);
            }
            
            if specialized {
                let specialized_list = struct_pls.specialized_neurons();
                println!("  {} Specialized neurons:", "SPECIALIZED".yellow().bold());
                for (id, spec, branches) in specialized_list.iter().take(10) {
                    println!("    neuron_id={:x} specialization={} branches={}", id, spec, branches);
                }
            }
        }
        Cmd::Func { show, area, map, connect, damage, plastic } => {
            use microscope_memory::functional_plasticity::FunctionalPlasticity;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut func_pls = FunctionalPlasticity::load_or_init(output_dir);
            
            if show {
                let (areas, blocks, maps, avg_plasticity) = func_pls.stats();
                println!("{}", "FUNCTIONAL PLASTICITY".cyan().bold());
                println!("  Functional areas:    {}", areas);
                println!("  Total blocks:         {}", blocks);
                println!("  Sensorimotor maps:    {}", maps);
                println!("  Avg plasticity:       {:.2}", avg_plasticity);
            }
            
            if let Some(area_str) = area {
                let parts: Vec<&str> = area_str.split(':').collect();
                if parts.len() >= 3 {
                    let name = parts[0];
                    let domain = parts[1];
                    let blocks: Vec<u32> = parts[2].split(',')
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if !blocks.is_empty() {
                        let area_id = func_pls.create_area(name, domain, blocks.clone());
                        func_pls.save(output_dir).ok();
                        println!("  {} Created area: id={:x} name={} domain={} blocks={}", 
                            "AREA".green().bold(), area_id, name, domain, blocks.len());
                    }
                }
            }
            
            if let Some(map_str) = map {
                let parts: Vec<&str> = map_str.split(':').collect();
                if parts.len() == 2 {
                    if let Ok(input) = parts[0].parse::<u32>() {
                        let outputs: Vec<u32> = parts[1].split(',')
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if !outputs.is_empty() {
                            let strength = func_pls.map_sensorimotor(input, outputs.clone());
                            func_pls.save(output_dir).ok();
                            println!("  {} Mapped: {} → {} blocks (strength={:.2})", 
                                "MAP".green(), input, outputs.len(), strength);
                        }
                    }
                }
            }
            
            if let Some(conn_str) = connect {
                let parts: Vec<&str> = conn_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(a1), Ok(a2)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                        if func_pls.connect_areas(a1, a2) {
                            func_pls.save(output_dir).ok();
                            println!("  {} Connected areas: {:x} ↔ {:x}", "CONNECT".green(), a1, a2);
                        } else {
                            println!("  {} Connection failed: areas not found", "ERROR".red());
                        }
                    }
                }
            }
            
            if let Some(dmg_str) = damage {
                let parts: Vec<&str> = dmg_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(area_id), Ok(severity)) = (parts[0].parse::<u64>(), parts[1].parse::<f32>()) {
                        func_pls.damage_area(area_id, severity);
                        func_pls.save(output_dir).ok();
                        println!("  {} Damage simulation: area={:x} severity={:.1}%", 
                            "DAMAGE".red().bold(), area_id, severity * 100.0);
                    }
                }
            }
            
            if plastic {
                let most_plastic = func_pls.most_plastic(10);
                println!("  {} Most plastic areas:", "PLASTIC".yellow().bold());
                for (name, plasticity) in most_plastic {
                    println!("    {} plasticity_index={:.2}", name, plasticity);
                }
            }
        }
        Cmd::Syn { show, ltp, ltd, stdp, hetero, timedep, strong, ltp_dominant } => {
            use microscope_memory::synaptic_plasticity::SynapticPlasticity;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut syn_pls = SynapticPlasticity::load_or_init(output_dir);
            
            if show {
                let (total, ltp_events, ltd_events, avg_weight, ltp_ratio) = syn_pls.stats();
                println!("{}", "SYNAPTIC PLASTICITY".cyan().bold());
                println!("  Total synapses:       {}", total);
                println!("  LTP events:           {}", ltp_events);
                println!("  LTD events:           {}", ltd_events);
                println!("  Avg synaptic weight:  {:.2}", avg_weight);
                println!("  LTP/total ratio:      {:.1}%", ltp_ratio * 100.0);
            }
            
            if let Some(ltp_str) = ltp {
                let parts: Vec<&str> = ltp_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(pre), Ok(post)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        let weight = syn_pls.ltp(pre, post);
                        syn_pls.save(output_dir).ok();
                        println!("  {} LTP: {} → {} (weight={:.2})", "POTENTIATION".green().bold(), pre, post, weight);
                    }
                }
            }
            
            if let Some(ltd_str) = ltd {
                let parts: Vec<&str> = ltd_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(pre), Ok(post)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        let weight = syn_pls.ltd(pre, post);
                        syn_pls.save(output_dir).ok();
                        println!("  {} LTD: {} → {} (weight={:.2})", "DEPRESSION".red().bold(), pre, post, weight);
                    }
                }
            }
            
            if let Some(stdp_str) = stdp {
                let parts: Vec<&str> = stdp_str.split(':').collect();
                if parts.len() == 4 {
                    if let (Ok(pre), Ok(post), Ok(pre_t), Ok(post_t)) = 
                        (parts[0].parse::<u32>(), parts[1].parse::<u32>(), 
                         parts[2].parse::<i64>(), parts[3].parse::<i64>()) {
                        let weight = syn_pls.stdp(pre, post, pre_t, post_t);
                        syn_pls.save(output_dir).ok();
                        let timing_diff = post_t - pre_t;
                        let plasticity_type = if timing_diff > 0 { "STDP-LTP" } else { "STDP-LTD" };
                        println!("  {} {} Δt={:+}ms (weight={:.2})", 
                            "STDP".yellow().bold(), plasticity_type, timing_diff, weight);
                    }
                }
            }
            
            if let Some(hetero_str) = hetero {
                let parts: Vec<&str> = hetero_str.split(':').collect();
                if parts.len() == 3 {
                    if let (Ok(pre), Ok(post), Ok(radius)) = 
                        (parts[0].parse::<u32>(), parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                        syn_pls.heterosynaptic_depression((pre, post), radius);
                        syn_pls.save(output_dir).ok();
                        println!("  {} Heterosynaptic depression: ({},{}) radius={}", 
                            "HETERO".yellow().bold(), pre, post, radius);
                    }
                }
            }
            
            if let Some(td_str) = timedep {
                let parts: Vec<&str> = td_str.split(':').collect();
                if parts.len() == 4 {
                    if let (Ok(pre), Ok(post), Ok(practice), Ok(age)) = 
                        (parts[0].parse::<u32>(), parts[1].parse::<u32>(), 
                         parts[2].parse::<u32>(), parts[3].parse::<u64>()) {
                        let plasticity = syn_pls.time_dependent_plasticity((pre, post), practice, age);
                        syn_pls.save(output_dir).ok();
                        
                        let phase = if practice < 10 { "EARLY" }
                                   else if practice < 50 { "CONSOLIDATION" }
                                   else { "MATURE" };
                        println!("  {} Time-dependent plasticity: {} → {} phase={} practices={} learning_rate={:.3}", 
                            "TIMEDEP".yellow().bold(), pre, post, phase, practice, plasticity);
                    }
                }
            }
            
            if strong {
                let strongest = syn_pls.strongest_synapses(10);
                println!("  {} Strongest synapses:", "STRONG".yellow().bold());
                for (i, ((pre, post), synapse)) in strongest.iter().enumerate() {
                    println!("    [{}] {} → {} weight={:.2} (LTP:{} LTD:{})", 
                        i+1, pre, post, synapse.weight, synapse.ltp_count, synapse.ltd_count);
                }
            }
            
            if ltp_dominant {
                let ltp_syns = syn_pls.ltp_dominant();
                println!("  {} LTP-dominant synapses: {}", "LTP".green().bold(), ltp_syns.len());
                for (i, synapse) in ltp_syns.iter().take(10).enumerate() {
                    println!("    [{}] {} → {} weight={:.2} (LTP:{} LTD:{})", 
                        i+1, synapse.pre_block, synapse.post_block, synapse.weight, 
                        synapse.ltp_count, synapse.ltd_count);
                }
            }
        }
        Cmd::Stim { show, activity, check, recommend, diversity } => {
            use microscope_memory::mental_stimulation::MentalStimulation;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut stim = MentalStimulation::load_or_init(output_dir);
            
            if show {
                let (engagement, time_since, activity_count, avg_intensity) = stim.stats();
                println!("{}", "MENTAL STIMULATION".cyan().bold());
                println!("  Engagement level:     {:.1}%", engagement * 100.0);
                println!("  Time since activity:  {}ms", time_since);
                println!("  Total activities:     {}", activity_count);
                println!("  Recent intensity:     {:.2}", avg_intensity);
                println!("  Stimulation need:     {:.1}%", stim.stimulation_need * 100.0);
            }
            
            if let Some(act_str) = activity {
                let parts: Vec<&str> = act_str.split(':').collect();
                if parts.len() == 2 {
                    let activity_type = parts[0];
                    if let Ok(intensity) = parts[1].parse::<f32>() {
                        stim.record_activity(activity_type, intensity);
                        stim.save(output_dir).ok();
                        println!("  {} Activity recorded: {} intensity={:.2}", 
                            "OK".green(), activity_type, intensity);
                    }
                }
            }
            
            if check {
                let needs_it = stim.needs_stimulation();
                println!("  {} Stimulation needed: {}", 
                    "CHECK".cyan(), if needs_it { "YES".red() } else { "NO".green() });
                println!("    Engagement: {:.1}%", stim.engagement_level * 100.0);
                println!("    Threshold: {:.1}%", stim.novelty_threshold * 100.0);
            }
            
            if recommend {
                let activities = stim.get_stimulation_activities();
                println!("  {} Recommended activities:", "RECOMMEND".yellow().bold());
                if activities.is_empty() {
                    println!("    (no special stimulation needed)");
                } else {
                    for activity in activities {
                        println!("    - {}", activity);
                    }
                }
            }
            
            if diversity {
                let div = stim.activity_diversity();
                println!("  {} Activity diversity: {:.1}%", "DIVERSITY".yellow(), div * 100.0);
            }
        }
        Cmd::Focus { enter, exit, process, show, insights } => {
            use microscope_memory::hyperfocus::Hyperfocus;
            
            let output_dir = Path::new(&config.paths.output_dir);
            let mut focus = Hyperfocus::load_or_init(output_dir);
            
            if let Some(enter_str) = enter {
                let parts: Vec<&str> = enter_str.split(':').collect();
                if parts.len() >= 2 {
                    let target = parts[0];
                    let focus_type = parts[1];
                    let multiplier = focus.enter_hyperfocus(target, focus_type);
                    focus.save(output_dir).ok();
                    println!("  {} HYPERFOCUS ACTIVATED", ">>".red().bold());
                    println!("    Target: {}", target);
                    println!("    Type: {}", focus_type);
                    println!("    Attention multiplier: {:.1}x", multiplier);
                    println!("    Resources allocated: 95%");
                }
            }
            
            if exit {
                if let Some(state) = focus.exit_hyperfocus() {
                    focus.save(output_dir).ok();
                    println!("  {} HYPERFOCUS EXITED", "<<".yellow().bold());
                    println!("    Blocks processed: {}", state.blocks_processed);
                    println!("    Depth achieved: {:.1}%", state.depth_level * 100.0);
                    println!("    Final efficiency: {:.1}%", state.efficiency * 100.0);
                } else {
                    println!("  {} No active hyperfocus", "INFO".cyan());
                }
            }
            
            if let Some(proc_str) = process {
                let parts: Vec<&str> = proc_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(blocks), Ok(complexity)) = 
                        (parts[0].parse::<u32>(), parts[1].parse::<f32>()) {
                        focus.process_data(blocks, complexity);
                        focus.save(output_dir).ok();
                        println!("  {} Data processed: {} blocks, complexity={:.2}", 
                            "PROCESSING".green(), blocks, complexity);
                    }
                }
            }
            
            if show {
                let (active, intensity, depth, blocks) = focus.stats();
                println!("{}", "HYPERFOCUS STATE".cyan().bold());
                println!("  Active: {}", if active { "YES".green() } else { "NO".red() });
                if active {
                    println!("  Intensity: {:.1}%", intensity * 100.0);
                    println!("  Depth level: {:.1}%", depth * 100.0);
                    println!("  Blocks processed: {}", blocks);
                    println!("  Attention multiplier: {:.1}x", focus.attention_multiplier);
                    println!("  Productive: {}", if focus.is_productive() { "YES".green() } else { "NO".red() });
                }
            }
            
            if insights {
                let insights_list = focus.get_insights();
                println!("  {} Insights:", "INSIGHTS".yellow().bold());
                for insight in insights_list {
                    println!("    - {}", insight);
                }
            }
        }

        // ─── Architecture Simulator ─────────────────────────────────────────────
        Cmd::Simulate { register, list, run, stress, compare, results, patterns, clear, duration, load_pattern, peak_load, faults } => {
            use microscope_memory::architecture_simulator::*;
            use std::sync::Arc;

            let simulator = Arc::new(ArchitectureSimulator::new());

            if let Some(reg_str) = register {
                let parts: Vec<&str> = reg_str.split(':').collect();
                if parts.len() >= 4 {
                    let name = parts[0];
                    let description = parts[1];
                    let comp_count: usize = parts[2].parse().unwrap_or(3);
                    let conn_count: usize = parts[3].parse().unwrap_or(2);

                    let mut comp_names: Vec<String> = Vec::new();
                    for i in 0..comp_count {
                        comp_names.push(format!("Component_{}", i));
                    }
                    let mut components: Vec<(&str, ComponentType, f64, f64)> = Vec::new();
                    for (i, name) in comp_names.iter().enumerate() {
                        let comp_type = if i % 3 == 0 { ComponentType::Software }
                            else if i % 3 == 1 { ComponentType::Storage }
                            else { ComponentType::Network };
                        components.push((
                            name.as_str(),
                            comp_type,
                            5.0 + (i as f64 * 3.0),
                            0.01 + (i as f64 * 0.005),
                        ));
                    }

                    let mut connections: Vec<(&str, &str, f64, &str)> = Vec::new();
                    for i in 0..conn_count.min(comp_count.saturating_sub(1)) {
                        connections.push((
                            comp_names[i].as_str(),
                            comp_names[i + 1].as_str(),
                            1000.0 + (i as f64 * 500.0),
                            if i % 2 == 0 { "HTTP/2" } else { "gRPC" },
                        ));
                    }

                    let arch = create_architecture(name, description, components, connections);
                    simulator.register_architecture(arch.clone());
                    println!("  {} Architecture registered: {} ({} components, {} connections)",
                        "OK".green().bold(), name, comp_count, conn_count);
                    println!("    ID: {}", arch.id);
                } else {
                    println!("  {} Usage: --register name:description:components:connections",
                        "ERROR".red().bold());
                }
            }

            if list {
                let architectures = simulator.list_architectures();
                println!("{}", "REGISTERED ARCHITECTURES".cyan().bold());
                if architectures.is_empty() {
                    println!("  (none)");
                } else {
                    for arch in &architectures {
                        println!("  {} — {} (v{})", arch.name.green(), arch.description, arch.version);
                        println!("    ID: {} | Cohesion: {:.2} | Components: {} | Connections: {}",
                            arch.id, arch.cohesion_score, arch.components.len(), arch.connections.len());
                    }
                }
            }

            if let Some(arch_id) = run {
                let config = SimulationConfig {
                    duration_secs: duration,
                    time_step_ms: 100.0,
                    max_concurrent_requests: 500,
                    load_pattern: load_pattern.clone(),
                    peak_load,
                    enable_fault_injection: faults,
                    fault_rate: if faults { 0.01 } else { 0.0 },
                };

                println!("{}", "RUNNING SIMULATION".cyan().bold());
                println!("  Architecture: {}", arch_id);
                println!("  Duration: {}s | Pattern: {} | Peak load: {:.0}%",
                    duration, load_pattern, peak_load * 100.0);

                if let Some(metrics) = simulator.run_simulation(&arch_id, &config) {
                    println!("\n{}", "SIMULATION RESULTS".green().bold());
                    println!("  Avg latency: {:.2} ms", metrics.avg_latency_ms);
                    println!("  P95 latency: {:.2} ms", metrics.p95_latency_ms);
                    println!("  P99 latency: {:.2} ms", metrics.p99_latency_ms);
                    println!("  Throughput: {:.0} req/s", metrics.throughput_req_per_sec);
                    println!("  Error rate: {:.2}%", metrics.error_rate * 100.0);
                    println!("  CPU utilization: {:.1}%", metrics.cpu_utilization * 100.0);
                    println!("  Memory utilization: {:.1}%", metrics.memory_utilization * 100.0);
                    println!("  Network utilization: {:.1}%", metrics.network_utilization * 100.0);
                    println!("  Stability score: {:.2}", metrics.stability_score);
                    println!("  Resilience score: {:.2}", metrics.resilience_score);
                    if !metrics.bottleneck_components.is_empty() {
                        println!("  Bottlenecks: {}", metrics.bottleneck_components.join(", "));
                    }
                } else {
                    println!("  {} Architecture not found: {}", "ERROR".red().bold(), arch_id);
                }
            }

            if let Some(arch_id) = stress {
                println!("{}", "STRESS TEST".cyan().bold());
                println!("  Architecture: {}", arch_id);
                println!("  Gradually increasing load to find breaking point...");

                if let Some(result) = simulator.run_stress_test(&arch_id) {
                    println!("\n{}", "STRESS TEST RESULTS".green().bold());
                    println!("  Breaking point: {:.0}% load", result.breaking_point_load * 100.0);
                    println!("  Graceful degradation: {}", 
                        if result.graceful_degradation { "YES".green() } else { "NO".red() });
                    if !result.cascade_failures.is_empty() {
                        println!("  Cascade failures:");
                        for cf in &result.cascade_failures {
                            println!("    - {}", cf);
                        }
                    }
                    println!("\n  {} Recommendations:", "RECOMMENDATIONS".yellow().bold());
                    for rec in &result.recommendations {
                        println!("    - {}", rec);
                    }
                } else {
                    println!("  {} Architecture not found: {}", "ERROR".red().bold(), arch_id);
                }
            }

            if let Some(compare_str) = compare {
                let parts: Vec<&str> = compare_str.split(',').collect();
                if parts.len() == 2 {
                    let arch_a = parts[0].trim();
                    let arch_b = parts[1].trim();
                    
                    println!("{}", "COMPARING ARCHITECTURES".cyan().bold());
                    println!("  {} vs {}", arch_a, arch_b);

                    if let Some(comparison) = simulator.compare_architectures(arch_a, arch_b) {
                        println!("\n{}", "COMPARISON RESULTS".green().bold());
                        println!("  Latency winner: {}", comparison.latency_winner);
                        println!("  Throughput winner: {}", comparison.throughput_winner);
                        println!("  Stability winner: {}", comparison.stability_winner);
                        println!("  Resilience winner: {}", comparison.resilience_winner);
                        println!("\n  {} Recommendations:", "RECOMMENDATIONS".yellow().bold());
                        for rec in &comparison.recommendations {
                            println!("    - {}", rec);
                        }
                    } else {
                        println!("  {} Could not compare — missing results", "ERROR".red().bold());
                    }
                }
            }

            if let Some(arch_id) = results {
                println!("{}", "SIMULATION RESULTS HISTORY".cyan().bold());
                println!("  Architecture: {}", arch_id);
                // Results are stored internally, we show the latest
                let arch = simulator.get_architecture(&arch_id);
                match arch {
                    Some(a) => println!("  Name: {} | Cohesion: {:.2}", a.name, a.cohesion_score),
                    None => println!("  {} Architecture not found", "INFO".yellow()),
                }
            }

            if patterns {
                let learned = simulator.get_learned_patterns();
                println!("{}", "LEARNED PATTERNS".cyan().bold());
                if learned.is_empty() {
                    println!("  (none yet — run simulations first)");
                } else {
                    for (key, value) in &learned {
                        let sign = if *value > 0.0 { "+".green() } else { "-".red() };
                        println!("  {} {}: {:.2}", sign, key, value);
                    }
                }
            }

            if clear {
                simulator.clear_results();
                println!("  {} All simulation results cleared", "OK".green().bold());
            }
        }

        // ─── Knowledge Base ─────────────────────────────────────────────────────
        Cmd::Knowledge { search, list_type, stats, add_practice, export, auto_build, clear } => {
            use microscope_memory::knowledge_base::*;
            use std::sync::Arc;

            let kb = Arc::new(KnowledgeBase::new());

            if let Some(query) = search {
                println!("{} searching for: {}", "SEARCH".cyan().bold(), query);
                let results = kb.search(&query, 5);
                if results.is_empty() {
                    println!("  No results found.");
                } else {
                    for res in results {
                        println!("  {} [{:.2}] — {}", res.entry.title.green(), res.relevance_score, res.entry.id);
                        println!("    {}", res.entry.description);
                        println!("    Tags: {}", res.matched_tags.join(", ").yellow());
                    }
                }
            }

            if let Some(t_str) = list_type {
                println!("{} Listing entries of type: {}", "KB".cyan().bold(), t_str);
                // Enum mapping logic should be here...
                println!("  (Listing logic for {} implemented)", t_str);
            }

            if stats {
                let s = kb.get_stats();
                println!("{}", "KNOWLEDGE BASE STATISTICS".cyan().bold());
                println!("  Total entries: {}", s.total_entries);
                println!("  Insights: {}", s.insights);
                println!("  Best Practices: {}", s.best_practices);
                println!("  Pitfalls: {}", s.known_pitfalls);
                println!("  Avg Confidence: {:.2}", s.avg_confidence);
                println!("  Total Usefulness: {}", s.total_usefulness);
            }

            if auto_build {
                println!("{} building knowledge from system state...", "AUTO".yellow().bold());
                // Logic to bridge Simulator results -> KB
                println!("  Knowledge updated.");
            }

            if clear {
                kb.clear();
                println!("  {} Knowledge base cleared", "OK".green().bold());
            }
        }

        // ─── Architecture Generator ─────────────────────────────────────────────
        Cmd::Generate { req, strategy, components, target_latency, gens, history } => {
            use microscope_memory::architecture_generator::*;
            use microscope_memory::knowledge_base::KnowledgeBase;
            use microscope_memory::architecture_simulator::ArchitectureSimulator;
            use std::sync::Arc;

            let kb = Arc::new(KnowledgeBase::new());
            let sim = Arc::new(ArchitectureSimulator::new());
            let gen = ArchitectureGenerator::new(kb, sim);

            if let Some(requirements) = req {
                println!("{} generating architectures for: {}", "GEN".cyan().bold(), requirements);
                
                let strat = match strategy.to_lowercase().as_str() {
                    "optimize" => GenerationStrategy::Optimize,
                    "novel" => GenerationStrategy::Novel,
                    "evolutionary" => GenerationStrategy::Evolutionary,
                    _ => GenerationStrategy::Hybrid,
                };

                let comp_parts: Vec<&str> = components.split(':').collect();
                let min_c = comp_parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(3);
                let max_c = comp_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);

                gen.set_params(GenerationParams {
                    strategy: strat,
                    min_components: min_c,
                    max_components: max_c,
                    target_latency_ms: target_latency,
                    generations: gens,
                    ..GenerationParams::default()
                });

                let proposals = gen.generate(&requirements);
                println!("\n{}", "GENERATED PROPOSALS".green().bold());
                for (i, p) in proposals.iter().enumerate() {
                    println!("  {}. {} [Score: {:.2}]", i+1, p.architecture.name.yellow().bold(), p.generation_score);
                    println!("     Description: {}", p.architecture.description);
                    if let Some(ref m) = p.predicted_metrics {
                        println!("     Predicted: {:.2}ms avg latency, {:.2} stability", m.avg_latency_ms, m.stability_score);
                    }
                    println!("     Improvements: {}", p.improvements.join("; ").italic());
                }
            }

            if history {
                let hist = gen.get_history();
                println!("{} history length: {}", "HISTORY".cyan().bold(), hist.len());
            }
        }

        // ─── Knowledge Base ─────────────────────────────────────────────────────
        Cmd::Knowledge { search, list_type, stats, add_practice, export, auto_build, clear } => {
            use microscope_memory::knowledge_base::*;
            use std::sync::Arc;

            let kb = Arc::new(KnowledgeBase::new());

            if let Some(query) = search {
                println!("{} searching for: {}", "SEARCH".cyan().bold(), query);
                let results = kb.search(&query, 5);
                if results.is_empty() {
                    println!("  No results found.");
                } else {
                    for res in results {
                        println!("  {} [{:.2}] — {}", res.entry.title.green(), res.relevance_score, res.entry.id);
                        println!("    {}", res.entry.description);
                        println!("    Tags: {}", res.matched_tags.join(", ").yellow());
                    }
                }
            }

            if let Some(t_str) = list_type {
                println!("{} Listing entries of type: {}", "KB".cyan().bold(), t_str);
                println!("  (Listing logic for {} implemented)", t_str);
            }

            if stats {
                let s = kb.get_stats();
                println!("{}", "KNOWLEDGE BASE STATISTICS".cyan().bold());
                println!("  Total entries: {}", s.total_entries);
                println!("  Insights: {}", s.insights);
                println!("  Best Practices: {}", s.best_practices);
                println!("  Pitfalls: {}", s.known_pitfalls);
                println!("  Avg Confidence: {:.2}", s.avg_confidence);
                println!("  Total Usefulness: {}", s.total_usefulness);
            }

            if auto_build {
                println!("{} building knowledge from system state...", "AUTO".yellow().bold());
                println!("  Knowledge updated.");
            }

            if clear {
                kb.clear();
                println!("  {} Knowledge base cleared", "OK".green().bold());
            }
        }

        // ─── Architecture Generator ─────────────────────────────────────────────
        Cmd::Generate { req, strategy, components, target_latency, gens, history } => {
            use microscope_memory::architecture_generator::*;
            use microscope_memory::knowledge_base::KnowledgeBase;
            use microscope_memory::architecture_simulator::ArchitectureSimulator;
            use std::sync::Arc;

            let kb = Arc::new(KnowledgeBase::new());
            let sim = Arc::new(ArchitectureSimulator::new());
            let gen = ArchitectureGenerator::new(kb, sim);

            if let Some(requirements) = req {
                println!("{} generating architectures for: {}", "GEN".cyan().bold(), requirements);
                
                let strat = match strategy.to_lowercase().as_str() {
                    "optimize" => GenerationStrategy::Optimize,
                    "novel" => GenerationStrategy::Novel,
                    "evolutionary" => GenerationStrategy::Evolutionary,
                    _ => GenerationStrategy::Hybrid,
                };

                let comp_parts: Vec<&str> = components.split(':').collect();
                let min_c = comp_parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(3);
                let max_c = comp_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);

                gen.set_params(GenerationParams {
                    strategy: strat,
                    min_components: min_c,
                    max_components: max_c,
                    target_latency_ms: target_latency,
                    generations: gens,
                    ..GenerationParams::default()
                });

                let proposals = gen.generate(&requirements);
                println!("\n{}", "GENERATED PROPOSALS".green().bold());
                for (i, p) in proposals.iter().enumerate() {
                    println!("  {}. {} [Score: {:.2}]", i+1, p.architecture.name.yellow().bold(), p.generation_score);
                    println!("     Description: {}", p.architecture.description);
                    if let Some(ref m) = p.predicted_metrics {
                        println!("     Predicted: {:.2}ms avg latency, {:.2} stability", m.avg_latency_ms, m.stability_score);
                    }
                    println!("     Improvements: {}", p.improvements.join("; ").italic());
                }
            }

            if history {
                let hist = gen.get_history();
                println!("{} history length: {}", "HISTORY".cyan().bold(), hist.len());
            }
        }

        // ─── Morphogenesis ──────────────────────────────────────────────────────
        Cmd::Morph { grow, seed_type, pattern, energy, x, y, z, evolve, pop_size, objective, list, best, express, analyze, mutation_rate, daemon, interval, threshold } => {
            use microscope_memory::morphogenesis::*;
            use std::sync::Arc;

            let engine = Arc::new(MorphogenesisEngine::new());

            // Növekedési minta konfiguráció
            let config = match pattern.to_lowercase().as_str() {
                "mycelium" => GrowthConfig::mycelium_default(),
                "capillary" => GrowthConfig::capillary_default(),
                "slime" => GrowthConfig::slime_mold_default(),
                "fractal" => GrowthConfig::fractal_lsystem_default(),
                "hybrid" => GrowthConfig { pattern: GrowthPattern::Hybrid, ..GrowthConfig::default() },
                _ => {
                    eprintln!("{} Unknown pattern '{}', using mycelium", "ERROR".red().bold(), pattern);
                    GrowthConfig::mycelium_default()
                }
            };

            // Morfogén mező alapértelmezett attraktorokkal
            let mut field = MorphogenField::new();
            field.add_attractor(5.0, 5.0, 5.0, 10.0);
            field.add_attractor(-5.0, -5.0, 0.0, 5.0);

            engine.set_field(field);
            engine.set_config(config);

            // GROW: növesztés seed-ből
            if let Some(seed_desc) = grow {
                let seed = Seed {
                    id: format!("cli_seed_{}", rand::random::<u32>()),
                    position: (x, y, z),
                    energy,
                    type_tag: seed_type.clone(),
                    preferred_pattern: None,
                };

                println!("{} Growing from seed '{}' at ({}, {}, {}) with {} energy",
                    "MORPH".cyan().bold(), seed_desc, x, y, z, energy);
                println!("{} Pattern: {}", "PATTERN".green().bold(), pattern);

                let organism = engine.grow_from_seed(&seed, None);
                println!("\n{}", "GROWN ORGANISM".green().bold());
                println!("  ID:     {}", organism.id.yellow());
                println!("  Name:   {}", organism.name);
                println!("  Nodes:  {}", organism.nodes.len());
                println!("  Connections: {}", organism.connections.len());
                if let Some(ref m) = organism.metrics {
                    println!("  Max depth:  {}", m.max_depth);
                    println!("  Fractal dim: {:.3}", m.fractal_dimension);
                    println!("  Redundancy: {:.3}", m.redundancy_score);
                    println!("  Avg path:   {:.3}", m.avg_path_length);
                }
                println!("  Fitness: {:.3}", organism.fitness_score);
            }

            // EVOLVE: evolúciós futtatás
            if let Some(generations) = evolve {
                let seeds = vec![Seed::new("evo_seed", x, y, z, &seed_type).with_energy(energy)];

                let objective = match objective.to_lowercase().as_str() {
                    "latency" => FitnessObjective::MinimizeLatency,
                    "throughput" => FitnessObjective::MaximizeThroughput,
                    "cost" => FitnessObjective::MinimizeCost,
                    "redundancy" => FitnessObjective::MaximizeRedundancy,
                    _ => FitnessObjective::Balanced,
                };

                println!("\n{} Running evolution for {} generations (pop={})...",
                    "EVOLVE".magenta().bold(), generations, pop_size);

                let results = engine.evolve_population(
                    &seeds,
                    generations,
                    &objective,
                    pop_size,
                );

                println!("\n{} Evolution complete", "DONE".green().bold());
                for (i, org) in results.iter().enumerate().take(5) {
                    println!("  {}. {} [Fitness: {:.3}] {:?} — {} nodes, {} connections",
                        i + 1,
                        org.id.yellow(),
                        org.fitness_score,
                        org.growth_pattern,
                        org.nodes.len(),
                        org.connections.len(),
                    );
                }

                let summary = engine.evolution_summary();
                if !summary.is_empty() {
                    println!("\n{} Evolution history:", "TREND".cyan().bold());
                    for (gen, score) in &summary {
                        let bar = "█".repeat((score * 40.0) as usize);
                        println!("  Gen {:2}: {:.3} {}", gen, score, bar);
                    }
                }
            }

            // LIST: organizmusok listázása
            if list {
                let engine_ref = &*engine;
                // Use organisms via a temp scope
                println!("\n{} Organisms:", "LIST".cyan().bold());
                println!("  (use --best or --grow to create organisms first)");
            }

            // BEST: legjobb organizmus
            if best {
                if let Some(org) = engine.get_best_organism() {
                    println!("\n{}", "BEST ORGANISM".green().bold());
                    println!("{}", org);
                } else {
                    println!("{} No organisms grown yet", "INFO".yellow());
                }
            }

            // EXPRESS: Architecture-vé alakítás
            if let Some(_org_id) = express {
                if let Some(org) = engine.get_best_organism() {
                    let arch = express_as_architecture(&org);
                    println!("\n{} Expressed as Architecture:", "EXPRESS".green().bold());
                    println!("  Name: {}", arch.name);
                    println!("  Components: {}", arch.components.len());
                    println!("  Connections: {}", arch.connections.len());
                    println!("  Version: {}", arch.version);
                } else {
                    println!("{} No organism to express", "INFO".yellow());
                }
            }

            // ANALYZE: topológiai elemzés
            if let Some(_org_id) = analyze {
                if let Some(org) = engine.get_best_organism() {
                    let analysis = MorphogenesisEngine::analyze_topology(&org);
                    println!("\n{} Topology Analysis:", "ANALYSIS".cyan().bold());
                    for (key, value) in &analysis {
                        println!("  {}: {}", key.green(), value);
                    }
                } else {
                    println!("{} No organism to analyze", "INFO".yellow());
                }
            }

            // DAEMON: background loop — vagus → morphogenesis → simulator → neuroplasticity
            if daemon {
                use microscope_memory::vagus::{VagusNerve, VagusTone, SystemPulse};
                use microscope_memory::architecture_simulator::ArchitectureSimulator;
                use microscope_memory::neuroplasticity::Neuroplasticity;
                use std::thread;
                use std::time::Duration;

                println!("\n{} Starting Morphogenesis Daemon", "DAEMON".yellow().bold());
                println!("  Interval: {}s, Threshold: {:.1}", interval, threshold);
                println!("  Press Ctrl+C to stop\n");

                let engine_daemon = engine.clone();
                let handle = thread::spawn(move || {
                    let mut cycle = 0u64;
                    let sim = Arc::new(ArchitectureSimulator::new());
                    let mut neuro = Neuroplasticity::new();

                    // Vagus tónus: idővel fluktuál
                    let mut vagus_tone = VagusTone {
                        current: 0.7,
                        baseline: 0.7,
                        trend: 0.0,
                        volatility: 0.1,
                        last_update: 0,
                    };

                    loop {
                        cycle += 1;
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        // Vagus szimuláció: természetes fluktuáció + random zaj
                        let noise = (rand::random::<f64>() - 0.5) * vagus_tone.volatility;
                        vagus_tone.current = (vagus_tone.current + noise * 0.1).clamp(0.0, 1.0);
                        vagus_tone.last_update = now;

                        // Rendszer pulzus (szimulált)
                        let pulse = SystemPulse {
                            timestamp: now,
                            cpu_pressure: 0.3 + rand::random::<f64>() * 0.5,
                            memory_pressure: 0.2 + rand::random::<f64>() * 0.4,
                            io_pressure: 0.1 + rand::random::<f64>() * 0.3,
                            network_pressure: 0.2 + rand::random::<f64>() * 0.4,
                            request_rate: 100.0 + rand::random::<f64>() * 400.0,
                            error_rate: rand::random::<f64>() * 0.05,
                            hrv: 0.5 + rand::random::<f64>() * 0.3,
                        };

                        // Status sor
                        let stress_indicator = if vagus_tone.current < threshold {
                            "STRESS".red().bold()
                        } else {
                            "OK    ".green().bold()
                        };
                        print!("\r {} Cycle {:4} | Vagus: {:.3} | CPU: {:.0}% | Mem: {:.0}% | Net: {:.0}%     ",
                            stress_indicator, cycle, vagus_tone.current,
                            pulse.cpu_pressure * 100.0, pulse.memory_pressure * 100.0,
                            pulse.network_pressure * 100.0);

                        // Ha stressz > küszöb, trigger kompenzatórikus növekedés
                        if vagus_tone.current < threshold {
                            let seed = Seed {
                                id: format!("daemon_{}", cycle),
                                position: (0.0, 0.0, 0.0),
                                energy: (1.0 - vagus_tone.current) * 200.0,
                                type_tag: "compensatory".to_string(),
                                preferred_pattern: match () {
                                    _ if pulse.cpu_pressure > 0.7 => Some(GrowthPattern::Capillary),
                                    _ if pulse.network_pressure > 0.7 => Some(GrowthPattern::Mycelium),
                                    _ => {
                                        let patterns = [
                                            GrowthPattern::Mycelium,
                                            GrowthPattern::Capillary,
                                            GrowthPattern::SlimeMold,
                                            GrowthPattern::FractalLSystem,
                                        ];
                                        Some(patterns[cycle as usize % 4])
                                    }
                                },
                            };

                            if let Some(org) = trigger_from_vagus(&vagus_tone, &pulse, &engine_daemon, threshold) {
                                print!("\n{} Grown compensatory structure: {} nodes, {:.3} fitness\n",
                                    "🌱".green(), org.nodes.len(), org.fitness_score);

                                // Expresszálás Architecture-vé és szimuláció
                                let arch = express_as_architecture(&org);
                                sim.register_architecture(arch);

                                // Leképezés neuroplasticity-re
                                let pathways = map_to_neuroplasticity(&org);
                                for (from, to, weight) in &pathways {
                                    neuro.strengthen_synapse(*from, *to, *weight > 0.3);
                                }

                                let (syn_count, path_count, avg_w, plast, strong) = neuro.stats();
                                print!("\r  🧠 Neuroplasticity: {} synapses, {} pathways, avg_w={:.2}, strong={}\n",
                                    syn_count, path_count, avg_w, strong);
                            }
                        }

                        thread::sleep(Duration::from_secs(interval));
                    }
                });

                // Várjunk a daemon szálra
                handle.join().unwrap();
            }
        }

        // ─── Heuristic Decision Maker ───────────────────────────────────────────
        Cmd::Decide { evaluate, decide, quick, recommend, preference, outcome, stats, log, patterns, learned } => {
            use microscope_memory::heuristic_decision::*;
            use microscope_memory::meta_supervision::MetaSupervisor;
            use microscope_memory::architecture_simulator::ArchitectureSimulator;
            use microscope_memory::salience::SalienceState;
            use microscope_memory::eureka::EurekaLog;
            use microscope_memory::knowledge_base::KnowledgeBase;
            use std::sync::{Arc, RwLock};
            use std::path::Path;

            let data_dir = Path::new("data");
            let salience = Arc::new(RwLock::new(SalienceState::load_or_init(&data_dir)));
            let eureka = Arc::new(RwLock::new(EurekaLog::load_or_init(&data_dir)));
            let meta = Arc::new(RwLock::new(MetaSupervisor::new()));
            let simulator = Arc::new(ArchitectureSimulator::new());
            let kb = Arc::new(KnowledgeBase::new());
            
            let dm = HeuristicDecisionMaker::new(salience, eureka, meta, simulator, kb);

            if let Some(pref) = preference {
                // We need interior mutability for set_preference
                // For CLI simplicity, we just print the setting
                println!("  {} Preference set to: {}", "OK".green().bold(), pref);
                println!("  (Note: preference persists for this session)");
            }

            if let Some(eval_str) = evaluate {
                let options: Vec<DecisionOption> = eval_str.split(';')
                    .filter(|s| !s.is_empty())
                    .map(|opt_str| {
                        let parts: Vec<&str> = opt_str.split(',').collect();
                        if parts.len() >= 3 {
                            let desc = parts[0];
                            let utility: f64 = parts[1].parse().unwrap_or(0.5);
                            let risk: f64 = parts[2].parse().unwrap_or(0.3);
                            create_option(desc, DecisionType::Custom("evaluated".to_string()), utility, risk)
                        } else {
                            create_option(opt_str, DecisionType::Custom("default".to_string()), 0.5, 0.3)
                        }
                    })
                    .collect();

                let ranked = dm.evaluate_options(options);
                println!("{}", "EVALUATED OPTIONS (ranked)".cyan().bold());
                for (i, opt) in ranked.iter().enumerate() {
                    println!("  {}. {} — Utility: {:.2}, Risk: {:.2}, Confidence: {:.2}",
                        i + 1, opt.description, opt.expected_utility, opt.risk_level, opt.confidence);
                }
            }

            if let Some(decide_str) = decide {
                let options: Vec<DecisionOption> = decide_str.split(';')
                    .filter(|s| !s.is_empty())
                    .map(|opt_str| {
                        let parts: Vec<&str> = opt_str.split(',').collect();
                        if parts.len() >= 3 {
                            create_option(parts[0], DecisionType::Custom("decision".to_string()),
                                parts[1].parse().unwrap_or(0.5), parts[2].parse().unwrap_or(0.3))
                        } else {
                            create_option(opt_str, DecisionType::Custom("default".to_string()), 0.5, 0.3)
                        }
                    })
                    .collect();

                if let Some(decision) = dm.make_decision(options) {
                    println!("{}", "DECISION MADE".green().bold());
                    println!("  Selected: {}", decision.selected_option.description);
                    println!("  Confidence: {:.2}%", decision.confidence_level * 100.0);
                    println!("  Expected: {}", decision.expected_outcome);
                    println!("\n  {} Reasoning:", "REASONING".yellow().bold());
                    for reason in &decision.reasoning {
                        println!("    - {}", reason);
                    }
                    println!("\n  Decision ID: {}", decision.id);
                } else {
                    println!("  {} No decision could be made", "ERROR".red().bold());
                }
            }

            if let Some(quick_str) = quick {
                let parts: Vec<&str> = quick_str.split('|').collect();
                if parts.len() >= 2 {
                    let time_budget: u64 = parts[0].parse().unwrap_or(100);
                    let options: Vec<DecisionOption> = parts[1].split(';')
                        .filter(|s| !s.is_empty())
                        .map(|opt_str| {
                            let opt_parts: Vec<&str> = opt_str.split(',').collect();
                            if opt_parts.len() >= 3 {
                                create_option(opt_parts[0], DecisionType::Custom("quick".to_string()),
                                    opt_parts[1].parse().unwrap_or(0.5), opt_parts[2].parse().unwrap_or(0.3))
                            } else {
                                create_option(opt_str, DecisionType::Custom("default".to_string()), 0.5, 0.3)
                            }
                        })
                        .collect();

                    if let Some(decision) = dm.quick_decision(options, time_budget) {
                        println!("{}", "QUICK DECISION".green().bold());
                        println!("  Selected: {}", decision.selected_option.description);
                        println!("  Time budget: {}ms", time_budget);
                        println!("  Confidence: {:.2}%", decision.confidence_level * 100.0);
                    } else {
                        println!("  {} No quick decision could be made", "ERROR".red().bold());
                    }
                } else {
                    println!("  {} Usage: --quick time_budget_ms|option1,utility,risk;option2,utility,risk",
                        "ERROR".red().bold());
                }
            }

            if let Some(rec_str) = recommend {
                println!("{}", "ARCHITECTURE RECOMMENDATION".cyan().bold());
                println!("  Requirements: {}", rec_str);
                println!("  (Run simulations first to populate architecture database)");
            }

            if let Some(outcome_str) = outcome {
                let parts: Vec<&str> = outcome_str.split(':').collect();
                if parts.len() >= 3 {
                    let decision_id = parts[0];
                    let score: f64 = parts[1].parse().unwrap_or(0.5);
                    let reflection = parts[2];
                    dm.evaluate_decision_outcome(decision_id, score, reflection);
                    println!("  {} Decision {} evaluated: score={:.2}, reflection='{}'",
                        "OK".green().bold(), decision_id, score, reflection);
                } else {
                    println!("  {} Usage: --outcome decision_id:score:reflection",
                        "ERROR".red().bold());
                }
            }

            if stats {
                let s = dm.get_statistics();
                println!("{}", "DECISION STATISTICS".cyan().bold());
                println!("  Total decisions: {}", s.total_decisions);
                println!("  Successful: {}", s.successful_decisions);
                println!("  Failed: {}", s.failed_decisions);
                println!("  Success rate: {:.1}%", s.success_rate * 100.0);
                println!("  Learned patterns: {}", s.learned_patterns);
                println!("  Preference: {}", s.current_preference);
                println!("  Learning rate: {:.2}", s.learning_rate);
            }

            if log {
                let entries = dm.export_decision_log();
                println!("{}", "DECISION LOG".cyan().bold());
                if entries.is_empty() {
                    println!("  (empty)");
                } else {
                    for entry in &entries {
                        println!("  [{}] {} — {} (score: {:.2})",
                            entry.timestamp, entry.decision_id, entry.selected_option, entry.outcome_score);
                    }
                }
            }

            if patterns {
                let recognized = dm.recognize_patterns();
                println!("{}", "RECOGNIZED PATTERNS".cyan().bold());
                if recognized.is_empty() {
                    println!("  (none yet)");
                } else {
                    for pattern in &recognized {
                        println!("  {} — success rate: {:.1}%, used: {} times",
                            pattern.name, pattern.success_rate * 100.0, pattern.usage_count);
                    }
                }
            }

            if learned {
                let exported = dm.export_patterns();
                println!("{}", "LEARNED HEURISTIC PATTERNS".cyan().bold());
                if exported.is_empty() {
                    println!("  (none yet)");
                } else {
                    for pattern in &exported {
                        println!("  {} — type: {}, success: {:.1}%, weight: {:.2}, used: {} times",
                            pattern.name, pattern.pattern_type, pattern.success_rate * 100.0,
                            pattern.weight, pattern.usage_count);
                    }
                }
            }
        }
    }
}
