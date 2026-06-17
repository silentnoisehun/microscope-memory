//! CLI command handlers for cognitive state display commands.
//!
//! These handlers load various cognitive module states from the output directory,
//! display their statistics, and optionally save modifications.
//!
//! Extracted from `main.rs` match block.

use std::path::Path;
use std::time::Instant;

use colored::Colorize;

use microscope_memory::*;
use microscope_memory::config::Config;

/// Show Hebbian learning state (activations, co-activations, energy).
pub fn hebbian(config: &Config) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
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

/// Apply Hebbian drift — co-activated blocks pull coordinates closer.
pub fn hebbian_drift(config: &Config) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let mut hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);

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

/// Show hottest blocks (most recently/frequently activated).
pub fn hottest(config: &Config, k: usize) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
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
            rec.drift_x, rec.drift_y, rec.drift_z,
            safe_truncate(text, 50)
        );
    }
}

/// Show emerged archetypes (crystallized activation patterns).
pub fn archetypes(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let arc = archetype::ArchetypeState::load_or_init(output_dir);
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
                a.id, a.label, a.strength, a.members.len(),
                a.reinforcement_count, a.centroid.0, a.centroid.1, a.centroid.2,
            );
        }
    }
}

/// Detect new archetypes from resonance field and Hebbian state.
pub fn emerge(config: &Config) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let resonance = resonance::ResonanceState::load_or_init(output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);

    let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
        .map(|i| {
            let h = reader.header(i);
            (h.x, h.y, h.z)
        })
        .collect();
    let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();

    let mut arc = archetype::ArchetypeState::load_or_init(output_dir);
    let emerged = arc.detect(&resonance, &hebb, &headers, &texts);
    arc.decay();
    arc.save(output_dir).expect("save archetypes");

    println!(
        "{} {} new archetypes emerged ({} total)",
        "EMERGE".cyan().bold(), emerged, arc.archetypes.len()
    );
    for a in arc.archetypes.iter().rev().take(5) {
        println!(
            "  #{} '{}' str={:.3} members={}",
            a.id, a.label, a.strength, a.members.len()
        );
    }
}

/// Show resonance protocol state (pulses, field energy).
pub fn resonance(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let resonance = resonance::ResonanceState::load_or_init(output_dir);
    let s = resonance.stats();
    println!("{}", "RESONANCE PROTOCOL".magenta().bold());
    println!("  Instance ID:        {:x}", s.instance_id);
    println!("  Outgoing pulses:    {}", s.outgoing_pulses);
    println!("  Incoming pulses:    {}", s.incoming_pulses);
    println!("  Pending integration:{}", s.pending_integration);
    println!("  Unique sources:     {}", s.unique_sources);
    println!("  Field cells:        {}", s.field_cells);
    println!("  Field energy:       {:.3}", s.field_energy);

    if !resonance.outgoing.is_empty() {
        println!("\n  Recent outgoing:");
        for p in resonance.outgoing.iter().rev().take(5) {
            println!(
                "    str={:.3} blocks={} layer={} hash={:x}",
                p.strength, p.activations.len(), p.layer_hint, p.query_hash,
            );
        }
    }
}

/// Integrate received pulses into local Hebbian state.
pub fn integrate(config: &Config) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let mut hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let mut resonance = resonance::ResonanceState::load_or_init(output_dir);

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
        "INTEGRATE".magenta().bold(), influenced
    );
}

/// Show mirror neuron state (resonance echoes, boosted blocks).
pub fn mirror(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let mirror = mirror::MirrorState::load_or_init(output_dir);
    let stats = mirror.stats();
    println!("{}", "MIRROR NEURON STATE".magenta().bold());
    println!("  Resonance echoes:   {}", stats.total_echoes);
    println!("  Resonant blocks:    {}", stats.resonant_blocks);
    println!("  Avg similarity:     {:.3}", stats.avg_similarity);
    if let Some((idx, strength)) = stats.strongest_block {
        let reader = crate::open_reader(config);
        let text = reader.text(idx as usize);
        println!("  Strongest:          block {} (str={:.3}) {}", idx, strength, safe_truncate(text, 50));
    }

    if !mirror.echoes.is_empty() {
        println!("\n  Recent echoes:");
        for echo in mirror.echoes.iter().rev().take(5) {
            println!(
                "    sim={:.3} shared={} blocks  trigger={:x} echo={:x}",
                echo.similarity, echo.shared_blocks.len(), echo.trigger_hash, echo.echo_hash,
            );
        }
    }
}

/// Show most resonant blocks (strongest mirror neuron signal).
pub fn resonant(config: &Config, k: usize) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let mirror = mirror::MirrorState::load_or_init(output_dir);
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

/// Show attention mechanism state (layer weights, quality history).
pub fn attention(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let attn_state = attention::AttentionState::load_or_init(output_dir);
    println!("{}", "ATTENTION STATE".cyan().bold());
    println!("  Total recalls:      {}", attn_state.total_recalls);
    println!("  Learned weights:    {:?}", attn_state.learned_weights);

    if !attn_state.history.is_empty() {
        println!("\n  Recent history:");
        let recent_qualities: Vec<f32> = attn_state.history.iter().rev().take(5).map(|o| o.quality).collect();
        for (i, q) in recent_qualities.iter().enumerate() {
            println!("    outcome quality={:.2}", q);
        }
    }
}

/// Show temporal archetype patterns (time-of-day activation profiles).
pub fn temporal_patterns(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let temporal = temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
    let window = temporal_archetype::current_time_window();
    println!(
        "{} (current window: {})",
        "TEMPORAL ARCHETYPES".cyan().bold(),
        temporal_archetype::WINDOW_LABELS[window]
    );

    if temporal.profiles.is_empty() {
        println!("  (no temporal data yet — recall with archetype matches to build profiles)");
    } else {
        for p in &temporal.profiles {
            let dominant = p.dominant_window()
                .map(|w| temporal_archetype::WINDOW_LABELS[w])
                .unwrap_or("?");
            println!(
                "\n  Archetype #{} (total={}, dominant={})",
                p.archetype_id, p.total_activations, dominant
            );
            for (i, label) in temporal_archetype::WINDOW_LABELS.iter().enumerate() {
                let bar_len = (p.window_weights[i] * 5.0) as usize;
                let bar: String = "█".repeat(bar_len);
                let marker = if i == window { " ●" } else { "" };
                println!("    {} {:>3} {:.1} {}{}", label, p.window_counts[i], p.window_weights[i], bar, marker);
            }
        }
    }
}

/// Show thought patterns (crystallized recall sequences).
pub fn patterns(config: &Config, k: usize) {
    let output_dir = Path::new(&config.paths.output_dir);
    let tg = thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let stats = tg.stats();
    println!("{}", "THOUGHT GRAPH".cyan().bold());
    println!(
        "  nodes={} edges={} patterns={} (crystallized={}) session=#{}",
        stats.node_count, stats.edge_count, stats.pattern_count,
        stats.crystallized, stats.current_session_id
    );

    let top = tg.top_patterns(k);
    if top.is_empty() {
        println!("  (no patterns yet — recall more to form thought paths)");
    } else {
        println!("\n  {}", "Top patterns:".yellow());
        for (i, p) in top.iter().enumerate() {
            let seq_str: Vec<String> = p.sequence.iter()
                .map(|h| format!("{:04x}", h & 0xFFFF)).collect();
            let crystallized = if p.frequency >= 3 { "*" } else { " " };
            println!(
                "  {}#{} {} freq={} str={:.2} blocks={}",
                crystallized, i + 1, seq_str.join(" → "),
                p.frequency, p.strength, p.result_blocks.len()
            );
        }
    }
}

/// Show recent thought paths (recall sequences by session).
pub fn paths(config: &Config, sessions: usize) {
    let output_dir = Path::new(&config.paths.output_dir);
    let tg = thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let recent = tg.recent_sessions(sessions);

    if recent.is_empty() {
        println!("  (no recall sessions recorded yet)");
    } else {
        println!("{}", "THOUGHT PATHS".cyan().bold());
        for (si, session) in recent.iter().enumerate() {
            if let Some(first) = session.first() {
                println!("\n  {} Session #{} ({} recalls):", "▸".green(), first.session_id, session.len());
                let path_str: Vec<String> = session.iter()
                    .map(|n| format!("{:04x}", n.query_hash & 0xFFFF)).collect();
                println!("    {}", path_str.join(" → "));
            }
            if si >= sessions { break; }
        }
    }
}

/// Show predictive cache stats and active predictions.
pub fn predictions(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let cache = predictive_cache::PredictiveCache::load_or_init(output_dir);
    let s = &cache.stats;
    println!("{}", "PREDICTIVE CACHE".cyan().bold());
    println!(
        "  predictions={} hits={} misses={} partial={} hit_rate={:.1}%",
        s.total_predictions, s.total_hits, s.total_misses, s.total_partial_hits,
        s.hit_rate() * 100.0
    );
    println!(
        "  active={} avg_confidence={:.1}%",
        s.current_predictions, s.avg_confidence * 100.0
    );

    if !cache.predictions.is_empty() {
        println!("\n  {}", "Active predictions:".yellow());
        for (i, p) in cache.predictions.iter().enumerate() {
            println!(
                "  #{} hash={:04x} blocks={} conf={:.0}% pattern=#{}",
                i + 1, p.predicted_query_hash & 0xFFFF,
                p.blocks.len(), p.confidence * 100.0, p.pattern_id
            );
        }
    }
}

/// Run dream consolidation (offline memory replay and pruning).
pub fn dream(config: &Config) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    println!("{} consolidating...", "DREAM".magenta().bold());

    match dream::dream_consolidate(output_dir, reader.block_count) {
        Ok(cycle) => {
            println!(
                "  {} fingerprints replayed, {} strengthened, {} pairs pruned, {} activations pruned ({} ms)",
                cycle.replayed_fingerprints, cycle.strengthened_pairs,
                cycle.pruned_pairs, cycle.pruned_activations, cycle.duration_ms
            );
        }
        Err(e) => {
            eprintln!("  {} dream consolidation failed: {}", "ERR".red(), e);
        }
    }
}

/// Show dream consolidation history.
pub fn dream_log(config: &Config, k: usize) {
    let output_dir = Path::new(&config.paths.output_dir);
    let state = dream::DreamState::load_or_init(output_dir);
    let stats = state.stats();
    println!("{}", "DREAM LOG".magenta().bold());
    println!("  Total cycles:    {}", stats.total_cycles);
    println!("  Total replayed:  {}", stats.total_replayed);
    println!("  Total pruned:    {}", stats.total_pruned_pairs + stats.total_pruned_activations);

    let recent: Vec<_> = state.cycles.iter().rev().take(k).collect();
    if !recent.is_empty() {
        println!("\n  Recent cycles:");
        for cycle in recent {
            println!(
                "    ts={} replayed={} pruned={} strengthened={} dur={}ms",
                cycle.timestamp_ms, cycle.replayed_fingerprints,
                cycle.pruned_pairs + cycle.pruned_activations,
                cycle.strengthened_pairs, cycle.duration_ms
            );
        }
    }
}

/// Show emotional contagion state (local + remote emotional fields).
pub fn emotional_field(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let contagion = emotional_contagion::EmotionalContagionState::load_or_init(output_dir);
    let s = contagion.stats();
    println!("{}", "EMOTIONAL FIELD".cyan().bold());
    println!("  Instance ID:    {}", s.instance_id);
    println!("  Has local:      {}", s.has_local);
    println!("  Remote count:   {}", s.remote_count);
    println!("  Local energy:   {:.3}", s.local_energy);
    println!("  Local valence:  {:.3}", s.local_valence);
    println!("  Blended valence:{:.3}", s.blended_valence);
}

/// Exchange emotional snapshots across federated indices.
pub fn emotional_exchange(config: &Config) {
    // The emotional contagion module doesn't have a direct exchange_snapshots function.
    // This is a placeholder that shows the current state.
    let output_dir = Path::new(&config.paths.output_dir);
    let contagion = emotional_contagion::EmotionalContagionState::load_or_init(output_dir);
    let s = contagion.stats();
    println!(
        "{} across {} indices (local: {} energy, {} remote snapshots)",
        "EMOTIONAL EXCHANGE".magenta().bold(),
        config.federation.indices.len(),
        s.local_energy, s.remote_count
    );
}

/// Show eureka/insight events.
pub fn eureka(config: &Config, k: usize, verbose: bool) {
    let output_dir = Path::new(&config.paths.output_dir);
    let log = eureka::EurekaLog::load_or_init(output_dir);
    println!("{} ({} total events found)", "EUREKA LOG".cyan().bold(), log.events.len());

    let recent: Vec<_> = log.events.iter().rev().take(k).collect();
    if recent.is_empty() {
        println!("  (no eureka events recorded yet)");
    } else {
        for ev in &recent {
            let score = ev.insight_score();
            if verbose {
                println!("\n  {}", "---".cyan());
                println!("  Text:   {}", safe_truncate(&ev.text, 80));
                println!("  Score:  {:.2}", score);
                println!("  Time:   {}", ev.timestamp_ms);
            } else if score > 1.0 {
                println!("  💡 score={:.1} \"{}\"", score, safe_truncate(&ev.text, 50));
            }
        }
    }
}

/// Run manual reconsolidation.
pub fn reconsolidate(config: &Config) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let activated: Vec<(u32, f32)> = hebb.hottest_blocks(10).iter()
        .map(|&(idx, energy)| (idx as u32, energy)).collect();

    let (rc_emo, rc_spatial) = reconsolidation::reconsolidate(
        output_dir, &reader, "", None, config, 4, &activated,
    );
    let rc_text = reconsolidation::format_reconsolidation(rc_emo, rc_spatial);
    if !rc_text.is_empty() {
        println!("{}", rc_text);
    } else {
        println!("  {} No reconsolidation candidates", "RECONSOLIDATE".yellow().bold());
    }
}

/// Show the salience network state (inhibitions, mask).
pub fn salience(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let salience = salience::SalienceState::load_or_init(output_dir);
    println!("{}", "SALIENCE NETWORK".cyan().bold());
    println!("  Active inhibitions: {}", salience.inhibitions.len());
    println!("  Threshold:          {:.3}", 0.3); // SALIENCE_THRESHOLD constant
}

/// Run associative daydreaming.
pub fn daydream(config: &Config, steps: usize, verbose: bool) {
    let output_dir = Path::new(&config.paths.output_dir);
    // Use a default seed from the current narrative state
    let narr = narrative::NarrativeState::load_or_init(output_dir);
    let seed = if narr.narrative.is_empty() {
        "daydream"
    } else {
        &narr.narrative
    };

    match daydream::daydream(config, seed, steps) {
        Ok(result) => {
            let formatted = daydream::format_daydream(&result, verbose);
            print!("{}", formatted);
        }
        Err(e) => {
            eprintln!("  {} daydream failed: {}", "ERR".red(), e);
        }
    }
}

/// Show the inner narrative.
pub fn narrative(config: &Config, verbose: bool) {
    let output_dir = Path::new(&config.paths.output_dir);
    let narr = narrative::NarrativeState::load_or_init(output_dir);
    println!("{}", "NARRATIVE".cyan().bold());
    println!("  Session:     #{}", narr.session_count);
    println!("  Narrative:   \"{}\"", safe_truncate(&narr.narrative, 100));

    if verbose {
        // emotion is [f32; 21], show magnitude
        let emo_mag: f32 = narr.emotion.iter().map(|x| x * x).sum::<f32>().sqrt();
        let emo_sum: f32 = narr.emotion.iter().sum();
        println!(
            "  Emotion:     mag={:.3} sum={:.3}",
            emo_mag, emo_sum
        );
    }
}

/// Spaced repetition — Ebbinghaus forgetting curve management.
pub fn spaced(config: &Config, due: bool, k: usize) {
    let output_dir = Path::new(&config.paths.output_dir);
    let sr = spaced_repetition::SpacedRepetition::load_or_init(output_dir);
    let s = sr.stats();

    if due {
        let due_blocks = sr.due_blocks();
        let due_items: Vec<_> = due_blocks.iter().take(k).collect();
        println!("{} {} items due for review:", "SPACED REPETITION".cyan().bold(), due_blocks.len());
        if due_items.is_empty() {
            println!("  (nothing due)");
        }
        for block_id in &due_items {
            let block = sr.find(**block_id);
            let reader = crate::open_reader(config);
            let text = if (**block_id as usize) < reader.block_count {
                reader.text(**block_id as usize)
            } else { "(unknown)" };
            if let Some(b) = block {
                println!(
                    "  B#{:6}  e={:.3}  interval={:4.0}h  due={}  {}",
                    block_id, b.ease_factor, b.interval_days * 24.0, b.recall_count,
                    safe_truncate(text, 50)
                );
            }
        }
    } else {
        println!("{}", "SPACED REPETITION".cyan().bold());
        println!("  Total items: {}", s.total_blocks);
        println!("  Due items:   {}", s.due);
        println!("  Avg easiness:{:.3}", s.avg_ease);
    }
}

/// Show multimodal index statistics.
pub fn modalities(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    let idx = multimodal::ModalityIndex::load_or_init(output_dir);
    let s = idx.stats();
    println!("{}", "MODALITIES".cyan().bold());
    println!("  Images:       {}", s.image_count);
    println!("  Audio:        {}", s.audio_count);
    println!("  Structured:   {}", s.structured_count);
    println!("  Total:        {}", s.total_entries);
}

/// Show vagus nerve state (statistics via VagusStatistics display).
pub fn vagus(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    // VagusNerve requires simulator and kb — can't instantiate from CLI easily.
    // Show a placeholder message.
    println!("{}", "VAGUS NERVE STATE".cyan().bold());
    println!("  (VagusNerve requires runtime context — use the bridge/Spine API for live status)");
    // Check for saved vagus state
    let state_path = output_dir.join("vagus_state.bin");
    if state_path.exists() {
        println!("  Saved state file found: {:?}", state_path);
    }
}

/// Meta-cognitive supervision.
pub fn meta_supervision(config: &Config) {
    let output_dir = Path::new(&config.paths.output_dir);
    // MetaSupervisor is not file-backed — it's a runtime monitor.
    // Show placeholder.
    println!("{}", "META-SUPERVISION".cyan().bold());
    println!("  (MetaSupervisor is a runtime monitor — no on-disk state)");
    // Check for saved report
    let report_path = output_dir.join("meta_supervision.bin");
    if report_path.exists() {
        println!("  Saved state file found: {:?}", report_path);
    }
}
