//! CLI command handlers for `recall`, `embed` (semantic search), and related
//! cognitive pipeline commands.
//!
//! Both functions accept a `Config` reference and display results via stdout.
//! They access the MicroscopeReader via `crate::open_reader()`.

use std::path::Path;
use std::time::Instant;

use colored::Colorize;

use microscope_memory::*;
use microscope_memory::config::Config;
use microscope_memory::reader::{layer_color, print_append_result};

/// Perform a cognitive recall: spatial + keyword + emotional + spaced-repetition
/// search across the memory index and the append log, then run the full
/// Hebbian / Mirror / Resonance / Archetype / ThoughtGraph post-processing
/// pipeline.
pub fn recall(config: &Config, query: &str, k: usize, emotion: Option<[f32; 21]>) {
    let t0 = Instant::now();
    let reader = crate::open_reader(config);
    let emo_label = emotion.as_ref().map(|e| format_emotion(e, 3)).unwrap_or_default();
    if !emo_label.is_empty() {
        println!("{} '{}' [emotion: {}]", "RECALL".cyan().bold(), query, emo_label.cyan());
    } else {
        println!("{} '{}':", "RECALL".cyan().bold(), query);
    }
    let emotional_recall_weight = config.search.emotional_bias_weight * 0.15;

    // Auto-prime: if no explicit emotion, use the emotional state ring
    let emotion = emotion.or_else(|| {
        let ring = EmotionalStateRing::load_or_init(Path::new(&config.paths.output_dir));
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
    let narrative_state = narrative::NarrativeState::load_or_init(Path::new(&config.paths.output_dir));
    if narrative_state.session_count > 0 {
        println!("  {} \"{}\"", "SELF".cyan().bold(), safe_truncate(&narrative_state.narrative, 50));
    }

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);

    // █████ Attention: compute layer weights from context █████
    let output_dir_att = Path::new(&config.paths.output_dir);
    let mut attention = attention::AttentionState::load_or_init(output_dir_att);
    let hebb_pre =
        hebbian::HebbianState::load_or_init(output_dir_att, reader.block_count);
    let tg_pre = thought_graph::ThoughtGraphState::load_or_init(output_dir_att);
    let pc_pre = predictive_cache::PredictiveCache::load_or_init(output_dir_att);

    let emotional_energy = emotional::emotional_field(&reader, &hebb_pre)
        .map(|f| f.total_energy)
        .unwrap_or(0.0);

    // Load emotional state ring for priming + attention intensity
    let emotional_ring = EmotionalStateRing::load_or_init(output_dir_att);
    let emotional_intensity = emotional_ring.intensity();

    // Infer quality of previous recall and record outcome
    if attention.total_recalls > 0 {
        let quality = attention.infer_quality();
        if let Some(last) = attention.history.last() {
            let prev_weights = last.weights;
            attention.record_outcome(quality, &prev_weights);
        }
    }

    let attn_signals = attention::AttentionSignals {
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
        hebbian::HebbianState::load_or_init(output_dir_eb, reader.block_count);
    let emotional_weight = config.search.emotional_bias_weight * attn.weight(4);
    let (qx, qy, qz) = emotional::apply_emotional_bias(
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
    let spaced = spaced_repetition::SpacedRepetition::load_or_init(
        Path::new(&config.paths.output_dir)
    );

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

    // █████ ThoughtGraph + Predictive Cache █████
    let output_dir_tg = Path::new(&config.paths.output_dir);
    let mut thought_graph =
        thought_graph::ThoughtGraphState::load_or_init(output_dir_tg);
    let mut pred_cache =
        predictive_cache::PredictiveCache::load_or_init(output_dir_tg);
    let qh_tg = hebbian::query_hash(query);

    // Check predictive cache — instant boost from pre-fetched blocks (scaled by attention)
    if let Some((cached_blocks, confidence)) = pred_cache.check(qh_tg) {
        let boost =
            confidence * thought_graph::PATTERN_BOOST_WEIGHT * attn.weight(6);
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

    // █████ Hebbian + Mirror: record activations & detect resonance █████
    let output_dir = Path::new(&config.paths.output_dir);
    let mut hebb =
        hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let mut mirror_state = mirror::MirrorState::load_or_init(output_dir);
    let activated: Vec<(u32, f32)> = all_results
        .iter()
        .filter(|(_, _, is_main)| *is_main)
        .take(k)
        .map(|(score, idx, _)| (*idx as u32, *score))
        .collect();
    if !activated.is_empty() {
        let qh = hebbian::query_hash(query);
        // Mirror: detect resonance before recording (so new fingerprint doesn't match itself)
        let boosts = mirror::mirror_boost(&hebb, &mut mirror_state, &activated, qh);
        if !boosts.is_empty() {
            println!(
                "  {} {} blocks resonated",
                "MIRROR:".magenta(),
                boosts.len()
            );
        }
        hebb.record_activation(&activated, qh);

        // Spaced repetition: record each activated block (quality 4 = seen and relevant)
        let mut spaced_writer = spaced_repetition::SpacedRepetition::load_or_init(output_dir);
        for &(idx, _) in &activated {
            let importance = (config.search.semantic_weight * 10.0) as u8; // approximate
            spaced_writer.record_recall(idx, importance, 4);
        }
        let _ = spaced_writer.save(output_dir);

        // Resonance: emit pulse with spatial coordinates
        let mut resonance_state = resonance::ResonanceState::load_or_init(output_dir);
        let headers: Vec<(f32, f32, f32)> = activated
            .iter()
            .map(|&(idx, _)| {
                let h = reader.header(idx as usize);
                (h.x, h.y, h.z)
            })
            .collect();
        resonance_state.emit_pulse(&activated, qh, &headers, 1);

        // Archetype: reinforce + temporal tracking
        let mut archetypes = archetype::ArchetypeState::load_or_init(output_dir);
        let mut temporal =
            temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
        if let Some((idx, score)) = archetypes.match_archetype(&activated) {
            let arch_id = archetypes.archetypes[idx].id;
            let time_boost = temporal.boost(arch_id);
            temporal.record_activation(arch_id, hebbian::now_epoch_ms_pub());
            let window = temporal_archetype::current_time_window();
            println!(
                "  {} '{}' (score={:.3} temporal={:.2} window={})",
                "ARCHETYPE:".cyan(),
                archetypes.archetypes[idx].label,
                score,
                time_boost,
                temporal_archetype::WINDOW_LABELS[window]
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
        let _ = mirror_state.save(output_dir);
        let _ = resonance_state.save(output_dir);
        let _ = archetypes.save(output_dir);
        let _ = temporal.save(output_dir);
        let _ = thought_graph.save(output_dir);
        let _ = pred_cache.save(output_dir);
        let _ = attention.save(output_dir);

        // ── Eureka detection ──
        let eureka_events = eureka::detect_eureka(
            config,
            &reader,
            query,
            emotion.as_ref(),
            &all_results,
        );
        if !eureka_events.is_empty() {
            let mut eureka_log = eureka::EurekaLog::load_or_init(output_dir);
            for ev in eureka_events {
                if let Ok(()) = eureka_log.record(output_dir, ev) {
                    // stored
                }
            }
            if eureka_log.events.len() > 0 {
                let last = &eureka_log.events[eureka_log.events.len() - 1];
                if last.insight_score() > 1.0 {
                    println!("  {} insight! score={:.1} \"{}\"", "💗 EUREKA".magenta().bold(), last.insight_score(), safe_truncate(&last.text, 40));
                }
            }
        }

        // ── Reconsolidation: every recall rewrites memory ──
        let (rc_emo, rc_spatial) = reconsolidation::reconsolidate(
            output_dir,
            &reader,
            query,
            emotion.as_ref(),
            config,
            4,
            &activated,
        );
        let rc_text = reconsolidation::format_reconsolidation(rc_emo, rc_spatial);
        if !rc_text.is_empty() {
            println!("{}", rc_text);
        }

        // ── Salience filter: only the strongest signal reaches narrative ──
        let mut salience_state = salience::SalienceState::load_or_init(output_dir);
        let high_salience = salience_state.filter(
            &activated.iter().map(|&(idx, _)| {
                // emotional_delta: approximate using hebbian energy, insight: from eureka, recency: 1.0
                let hebb_e = hebb.activations.get(idx as usize).map(|a| a.energy).unwrap_or(0.5);
                (idx, hebb_e * 0.3, 0.5, 1.0f32)
            }).collect::<Vec<_>>()
        );
        // Inhibit the highest-salience topic so it doesn't repeat
        if let Some((salient_idx, _)) = high_salience.first() {
            let topic = salience::SalienceState::topic_hash(&format!("block_{}", salient_idx));
            salience_state.inhibit(topic);
            let _ = salience_state.save(output_dir);
        }

        // ── Narrative update: the system tells itself what happened ──
        let wm_state = working_memory::WorkingMemory::load_or_init(output_dir);
        let wm_texts: Vec<String> = wm_state.items.iter().map(|i| i.text.clone()).collect();
        let sr_state = spaced_repetition::SpacedRepetition::load_or_init(output_dir);
        let tg_state = thought_graph::ThoughtGraphState::load_or_init(output_dir);
        let ring = EmotionalStateRing::load_or_init(output_dir);
        let mut narrative_state = narrative::NarrativeState::load_or_init(output_dir);
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

        // ── Meta-kognitív rekonszolidáció: the narrative becomes a memory ──
        narrative::metacognitive_store(
            output_dir,
            &narrative_state.narrative,
            &narrative_state.emotion,
        );
    }

    let elapsed = t0.elapsed();
    println!("\n  {} results in {:.0} us", shown, elapsed.as_micros());
}

/// Perform semantic (embedding-based) search against the memory index.
/// Supports cosine, dot, and L2 metrics.
pub fn semantic_search(config: &Config, query: &str, k: usize, metric: &str) {
    use embedding_index::EmbeddingIndex;
    use embeddings::{
        cosine_similarity_simd, EmbeddingProvider, MockEmbeddingProvider,
    };

    let t0 = Instant::now();
    println!(
        "{} '{}' using {} metric",
        "SEMANTIC SEARCH".cyan().bold(),
        safe_truncate(query, 50),
        metric.green()
    );

    let reader = crate::open_reader(config);
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
            match embeddings::CandleEmbeddingProvider::new(
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
            println!("  {} Failed to embed query", "ERROR:".red());
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
