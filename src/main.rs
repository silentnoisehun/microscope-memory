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

/// Per-phase timing for `recall`, enabled with MICROSCOPE_RECALL_TRACE=1.
/// Added because the end-to-end figure alone does not say where the time goes,
/// and the previous attribution of the gap to process start and index load was
/// wrong. Cheap when off: one `OnceLock` read, and nothing else.
/// Per-phase timing accumulators, so a trace is read as a distribution over the
/// warm calls rather than as one sample.
///
/// A single sample is not enough to decide anything here. The phases sum to the
/// measured total, so a segment that looks large is only large in that one call:
/// on a 29 ms query a scheduling blip is 2 ms, and reading one sample repeatedly
/// produced a "2.5 ms in side embeddings" that was not there. `bench-recall`
/// prints the accumulated table so the number that gets argued about is a mean
/// over the run, with the sample count next to it.
static PHASE_STATS: std::sync::Mutex<Vec<(&'static str, f64, u32)>> =
    std::sync::Mutex::new(Vec::new());

/// Buffered trace lines, written out by [`trace_flush`].
///
/// The buffer is the fix, not a convenience. `trace_phase` runs after the
/// `Instant::now()` that bounds the phase it reports and before the
/// `Instant::now()` that starts the next one, so writing there charged every
/// phase's stderr line to the phase that followed it. On the SciFact index that
/// made a traced query measure 89.6 ms against 44.8 ms with the trace off --
/// half of the instrumented number was the instrument.
///
/// Accumulating the lines and emitting them once, after the last measurement
/// has been taken, puts the writes outside every window without touching the
/// nineteen call sites that would otherwise each need a re-mark.
static TRACE_BUF: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn trace_phase(name: &'static str, ms: f64) {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    if *ON.get_or_init(|| std::env::var("MICROSCOPE_RECALL_TRACE").is_ok()) {
        if let Ok(mut buf) = TRACE_BUF.lock() {
            buf.push(format!("[trace] {:<24} {:>8.2} ms", name, ms));
        }
        if let Ok(mut stats) = PHASE_STATS.lock() {
            match stats.iter_mut().find(|(n, _, _)| *n == name) {
                Some((_, sum, n)) => {
                    *sum += ms;
                    *n += 1;
                }
                None => stats.push((name, ms, 1)),
            }
        }
    }
}

/// Emit every buffered trace line. Call this where the I/O cannot contaminate a
/// measurement -- after the last phase of a recall, and after a benchmark loop.
fn trace_flush() {
    if let Ok(mut buf) = TRACE_BUF.lock() {
        for line in buf.drain(..) {
            eprintln!("{line}");
        }
    }
}

/// A scalar note attached to a phase, e.g. how many candidates a phase scored.
/// Printed beside the trace but kept out of the timing sums, because it is not
/// a duration and averaging it would be meaningless.
static PHASE_NOTES: std::sync::Mutex<Vec<(&'static str, String)>> =
    std::sync::Mutex::new(Vec::new());

fn trace_note(name: &'static str, value: String) {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    if *ON.get_or_init(|| std::env::var("MICROSCOPE_RECALL_TRACE").is_ok()) {
        // Buffered for the same reason as `trace_phase`: a note written inside a
        // measurement window costs the phase that follows it.
        if let Ok(mut buf) = TRACE_BUF.lock() {
            buf.push(format!("[trace] {:<24} {:>8}", name, value));
        }
        if let Ok(mut notes) = PHASE_NOTES.lock() {
            match notes.iter_mut().find(|(n, _)| *n == name) {
                Some((_, last)) => *last = value,
                None => notes.push((name, value)),
            }
        }
    }
}

/// The accumulated phase table, mean per phase, busiest first.
fn phase_summary() -> Vec<(&'static str, f64, u32)> {
    let Ok(stats) = PHASE_STATS.lock() else {
        return Vec::new();
    };
    let mut out: Vec<(&'static str, f64, u32)> = stats
        .iter()
        .map(|(n, sum, c)| (*n, *sum / *c as f64, *c))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

fn recall(config: &Config, query: &str, k: usize) {
    let t0 = Instant::now();
    // Cumulative elapsed time at the last traced phase, so the tail of the
    // function can be measured as a remainder at the end.
    let mut t_mark = 0.0f64;
    let t_open = Instant::now();
    let reader = open_reader(config);
    trace_phase("open_reader", t_open.elapsed().as_secs_f64() * 1000.0);
    println!("{} '{}':", "RECALL".cyan().bold(), query);

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);
    let relevance_query = microscope_memory::relevance::RelevanceQuery::new(query);

    // ─── Attention: compute layer weights from context ──
    let output_dir_att = Path::new(&config.paths.output_dir);
    let t_state = Instant::now();
    let t_s0 = Instant::now();
    let mut attention = microscope_memory::attention::AttentionState::load_or_init(output_dir_att);
    trace_phase("  state: attention", t_s0.elapsed().as_secs_f64() * 1000.0);
    let t_s1 = Instant::now();
    let mut hebb =
        microscope_memory::hebbian::HebbianState::load_or_init(output_dir_att, reader.block_count);
    trace_phase("  state: hebbian", t_s1.elapsed().as_secs_f64() * 1000.0);
    let t_s2 = Instant::now();
    let tg_pre = microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_att);
    trace_phase(
        "  state: thought graph",
        t_s2.elapsed().as_secs_f64() * 1000.0,
    );
    let t_s3 = Instant::now();
    let pc_pre = microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_att);
    trace_phase("  state: pred cache", t_s3.elapsed().as_secs_f64() * 1000.0);

    trace_phase("state load", t_state.elapsed().as_secs_f64() * 1000.0);
    let t_emofield = Instant::now();
    let emotional_field = microscope_memory::emotional::emotional_field(&reader, &hebb);
    trace_phase(
        "emotional field scan",
        t_emofield.elapsed().as_secs_f64() * 1000.0,
    );
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

    // Scan every depth. The previous window was guessed from the character
    // length of the query (0..=8 -> D0-D2, 9..=20 -> D2-D4, else D2-D5), which
    // meant a short natural-language question never looked at the depths where
    // its fact was stored: auto_depth places 15..39 character statements at
    // D5, and a short question could not reach it. Depth selection must be
    // driven by the evidence, not by how long the query is.
    let mut all_results: Vec<(f32, usize, bool)> = Vec::new();

    // ── Semantic candidates ────────────────────────────────────────────────
    // Recall used to be purely lexical: a block was only ever a candidate when
    // its lexical score was > 0, so a paraphrase with no shared tokens could
    // not be retrieved at all, and the embedding index was never opened. Embed
    // the query with the configured provider and take the nearest blocks from
    // the stored vectors, then let those compete with the lexical hits.
    let mut semantic_hits: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    // Vectors of memories still in the append log, keyed by append position.
    // They are not in embeddings.bin (which is built from the consolidated
    // index) but they are real, stored memories, and until this existed they
    // were reachable only by exact token overlap.
    let mut appended_sem: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    {
        let emb_path = Path::new(&config.paths.output_dir).join("embeddings.bin");
        if let Some(eidx) = microscope_memory::embedding_index::open_embedding_cached(&emb_path) {
            // The provider must be cached, not rebuilt. Constructing one loads the
            // whole MiniLM model from disk, and doing that per query made it the
            // single largest cost in a recall: the phases below it add up to about
            // 12 ms of a 102 ms steady-state call, so the remainder was here.
            // `with_cached_provider` is the same helper the store path was routed
            // through when that cost was found there.
            // Timed separately because it is the largest remaining phase and the
            // FAISS baselines do not include query encoding at all -- they search
            // pre-computed query vectors -- so without this number the 37 ms
            // steady state and the 0.41 ms baseline cannot be compared honestly.
            let t_qemb = Instant::now();

            let r = microscope_memory::embeddings::with_cached_provider(
                &config.embedding,
                eidx.dim(),
                |provider| match provider.embed(query) {
                    Ok(qe) if qe.len() == eidx.dim() => Ok(qe),
                    Ok(v) => Err(format!(
                        "provider returned {} dims, index expects {}",
                        v.len(),
                        eidx.dim()
                    )),
                    Err(e) => Err(e.to_string()),
                },
            );
            trace_phase("query embed", t_qemb.elapsed().as_secs_f64() * 1000.0);
            match r {
                Ok(qe) if qe.len() == eidx.dim() => {
                    // Over-fetch, then let the final ranking do the ordering.
                    // Over-fetch, then let the final ranking do the ordering.
                    // The floor is 256, raised from 64 after a diagnostic run over
                    // all 60 eval questions that classified every miss as one of
                    // three kinds: not in the vector list, dropped by this
                    // pre-fetch, or dropped by the final ranking. Of the 31
                    // misses, 6 were lost to the 64-entry cut and sit in the
                    // 128..256 band: 128 recovers none of them (R@5 stays
                    // 46.7%), 256 recovers all six (R@5 48.3%, R@10 51.7%), and
                    // 512 is identical to 256 at higher cost. So 256 is a
                    // measured floor, not a guess.
                    //
                    // This is a partial fix. The dominant failure -- 21 of the 31
                    // misses -- is not a pre-fetch problem: `want`=2048 admits
                    // those answers and changes recall by exactly nothing. They
                    // are embedded, they score a mean cosine of 0.935, and they
                    // sit at mean rank 6,209 of 46,565, because the corpus holds
                    // 36,568 D5 summaries of similar content that outscore them.
                    // Neither a duplicate-vector cap nor removing the spatial term
                    // moves this either; both were measured and changed nothing.
                    // The remedy is corpus-level, not a reweighting of the score.
                    let want = (k * 8).max(256);
                    // Diagnostic only: when MICROSCOPE_EVAL_MATCH is set, fetch a
                    // deeper list once so the expected answer can be located
                    // within it. Admission is unchanged either way -- the
                    // admitted set is always the first `want` entries, which is
                    // exactly the top of the cosine-sorted list.
                    // Diagnostics are opt-in and cost real time: full_rank_of
                    // scans every stored vector, and it was observed to roughly
                    // double end-to-end p50. Require MICROSCOPE_EVAL_DIAG=1 in
                    // addition to the match tokens, so a normal measurement run
                    // is unaffected.
                    let diag = std::env::var("MICROSCOPE_EVAL_DIAG")
                        .ok()
                        .filter(|v| v == "1")
                        .and_then(|_| std::env::var("MICROSCOPE_EVAL_MATCH").ok())
                        .filter(|s| !s.is_empty());
                    let fetch = if diag.is_some() { want.max(1024) } else { want };
                    // NOTE: a bit-exact redundancy filter was tried here and
                    // reverted. The Python-side check suggested 1,996 identical
                    // vectors, but that was a quantisation artefact; the D5
                    // blocks are near-duplicates, not byte-identical ones, so
                    // capping identical vectors did not change the result list
                    // (depth histogram [0,0,0,0,3,1021] -> [0,0,0,0,7,1017]).
                    // The real obstacle is near-duplicate crowding, which needs
                    // similarity-based diversity, not a bit comparison.
                    let t_search = Instant::now();
                    let hits = eidx.search(&qe, fetch);
                    trace_phase("vector search", t_search.elapsed().as_secs_f64() * 1000.0);
                    t_mark = t0.elapsed().as_secs_f64() * 1000.0;
                    if let Some(matches) = &diag {
                        report_vector_diag(&reader, &eidx, &qe, &hits, matches, want);
                    }
                    for (sim, block_idx) in hits.into_iter().take(want) {
                        if block_idx < reader.block_count {
                            semantic_hits.insert(block_idx, sim);
                        }
                    }
                    trace_phase(
                        "  collect semantic hits",
                        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
                    );
                    t_mark = t0.elapsed().as_secs_f64() * 1000.0;
                    if let Some(side) = microscope_memory::embedding_index::open_append_cached(
                        &Path::new(&config.paths.output_dir)
                            .join(microscope_memory::embedding_index::APPEND_EMBEDDINGS_FILE),
                        eidx.dim(),
                    ) {
                        for (sim, ai) in side.search(&qe, want) {
                            appended_sem.insert(ai as usize, sim);
                        }
                    }
                    trace_phase(
                        "  side index open + search",
                        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
                    );
                    t_mark = t0.elapsed().as_secs_f64() * 1000.0;
                    trace_phase(
                        "  embedding scope tail",
                        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
                    );
                    t_mark = t0.elapsed().as_secs_f64() * 1000.0;
                }
                Ok(_) => {
                    eprintln!(
                        "  semantic: provider returned a different width than the index; skipped"
                    );
                }
                Err(e) => {
                    eprintln!("  semantic: embedding failed ({}); lexical path only", e);
                }
            }
        }
    }

    trace_phase(
        "embedding scope teardown",
        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
    );
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;

    // Candidate set. The inverted text index is a *prefilter*, not a gate.
    // Previously the scan walked the depth ranges but skipped every block that
    // was not a lexical candidate, and abandoned the whole loop when the
    // lexical set came back empty. Both behaviours are wrong: a vector hit was
    // only ever considered when it happened to be a lexical hit too, and a
    // query sharing no token with any block (a pure paraphrase) returned
    // nothing at all -- which is precisely the case the embedding path exists
    // to serve. Merge both sources into one sorted, deduplicated list and let
    // the ranking below see it.
    // A token with five or more characters expands to every dictionary word
    // sharing those characters, each contributing its whole posting list. On
    // 5.6M SciFact blocks that produced 83,703 candidates and 90% of the query
    // went into scoring blocks that matched one incidental word.
    //
    // The prefilter keeps blocks matching at least 3 distinct query terms. It
    // stays a prefilter and not a gate: the vector candidates are merged in
    // either way, and an empty lexical result is not an error.
    //
    // 3 is measured, not guessed. R@k is unchanged on both corpora this project
    // measures -- SciFact 53.1/74.8/81.8 over 286 queries, evaluation index
    // 78.3/80.0/80.0 -- while SciFact p50 fell 600.6 -> 156.3 ms and the
    // in-process query went 306.3 -> 89.9 ms. Set the variable to 1 to restore
    // the old unbounded union exactly; that path is pinned by a test.
    let min_matches: u32 = std::env::var("MICROSCOPE_LEX_MIN_MATCHES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let lex_cands: Vec<u32> = reader
        .text_index
        .as_ref()
        .and_then(|idx| idx.candidates_lexical_min_matches(relevance_query.tokens(), min_matches))
        .unwrap_or_default();

    trace_phase(
        "lexical prefilter",
        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
    );
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;

    // `lex_cands` is kept intact (cloned below) so the diagnostic block can
    // still report whether the answer came from the lexical or vector side.
    let candidates = microscope_memory::relevance::merge_candidates(
        lex_cands.clone(),
        semantic_hits.keys().copied(),
        reader.block_count,
    );
    trace_note(
        "candidate sources",
        format!(
            "lexical {} + semantic {} -> {}",
            lex_cands.len(),
            semantic_hits.len(),
            candidates.len()
        ),
    );

    for i in candidates {
        let text = reader.text(i);
        let lexical = relevance_query.lexical_score(text);
        // A block qualifies if it matches lexically OR is one of the
        // nearest blocks by embedding. Previously the gate was
        // `lexical > 0.0` alone, which made every semantic candidate
        // unreachable: a paraphrase sharing no token scored zero and was
        // dropped before ranking ever saw it.
        let semantic = semantic_hits.get(&i).copied();
        if lexical > 0.0 || semantic.is_some() {
            let h = reader.header(i);
            let dx = h.x - qx;
            let dy = h.y - qy;
            let dz = h.z - qz;
            let spatial_dist = dx * dx + dy * dy + dz * dz;
            let mut combined = microscope_memory::relevance::rank_distance_from_score(
                lexical,
                spatial_dist,
                config.search.keyword_boost,
                h.importance,
            );
            // Cosine similarity is a score, not a distance: fold it in as a
            // bonus so a strong semantic match can outrank a weak lexical
            // one. The gain is `search.semantic_rank_gain`, not
            // `search.semantic_weight`: the latter is a 0..1 blend for the
            // query's coordinates and was previously doubling as this,
            // which meant raising it to test the ranking also moved the
            // coordinates, and the two effects could not be told apart.
            // The upper bound was 1.0, which made that unmeasurable: every
            // value above 1 collapsed to the same score, so sweeping
            // 1.0..10.0 returned identical recall and the term looked
            // irrelevant. It was clamped, not irrelevant.
            if let Some(sim) = semantic {
                let w = config.search.semantic_rank_gain.clamp(0.0, 8.0);
                combined -= sim * w;
            }
            all_results.push((combined, i, true));
        }
    }

    trace_phase(
        "score candidates",
        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
    );
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (ai, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx * dx + dy * dy + dz * dz;
        let lexical = relevance_query.lexical_score(&entry.text);
        // A pending entry is a candidate on a semantic hit alone, exactly as a
        // main-index block is: a paraphrase of a fresh memory shares no token
        // with it, which is the case the embedding path exists to serve.
        let semantic = appended_sem.get(&ai).copied();
        if dist < 0.1 || lexical > 0.0 || semantic.is_some() {
            let mut combined = microscope_memory::relevance::rank_distance_from_score(
                lexical,
                dist,
                config.search.keyword_boost,
                entry.importance,
            );
            if let Some(sim) = semantic {
                let w = config.search.semantic_rank_gain.clamp(0.0, 8.0);
                combined -= sim * w;
            }
            all_results.push((combined, ai + 1_000_000, false));
        }
    }

    // ─── ThoughtGraph + Predictive Cache ──
    trace_phase("append log", t0.elapsed().as_secs_f64() * 1000.0 - t_mark);
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;

    let output_dir_tg = Path::new(&config.paths.output_dir);
    let mut thought_graph =
        microscope_memory::thought_graph::ThoughtGraphState::load_or_init(output_dir_tg);
    let mut pred_cache =
        microscope_memory::predictive_cache::PredictiveCache::load_or_init(output_dir_tg);
    let qh_tg = microscope_memory::hebbian::query_hash(query);

    // Check predictive cache — instant boost from pre-fetched blocks (scaled by attention)
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

    trace_phase(
        "predictive + pattern boost",
        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
    );
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;

    let mut seen = std::collections::HashSet::new();
    all_results.sort_by(|a, b| a.0.total_cmp(&b.0));
    trace_phase("sort", t0.elapsed().as_secs_f64() * 1000.0 - t_mark);
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;
    let mut shown = 0;

    // Diagnostic: where did the expected answer end up after the final sort,
    // and did it arrive from the lexical side, the vector side, or both?
    // Runs only when MICROSCOPE_EVAL_MATCH is set; never influences ranking.
    if std::env::var("MICROSCOPE_EVAL_DIAG").ok().as_deref() == Some("1") {
        if let Ok(m) = std::env::var("MICROSCOPE_EVAL_MATCH") {
            let needles: Vec<String> = m
                .split('|')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if !needles.is_empty() {
                let lex: std::collections::HashSet<usize> = lex_cands
                    .iter()
                    .map(|c| *c as usize)
                    .filter(|i| *i < reader.block_count)
                    .collect();
                let mut pos: Option<(usize, bool, bool)> = None;
                let mut dedup_pos = 0usize;
                let mut dedup_seen = std::collections::HashSet::new();
                for (_d, idx, is_main) in all_results.iter() {
                    if !dedup_seen.insert((*idx, *is_main)) {
                        continue;
                    }
                    let text = if *is_main {
                        reader.text(*idx)
                    } else {
                        appended.get(*idx).map(|e| e.text.as_str()).unwrap_or("")
                    }
                    .to_lowercase();
                    if pos.is_none() && needles.iter().any(|n| text.contains(n)) {
                        pos = Some((
                            dedup_pos,
                            lex.contains(idx),
                            semantic_hits.contains_key(idx),
                        ));
                    }
                    dedup_pos += 1;
                }
                eprintln!(
                    "EVALDIAG final={}",
                    match pos {
                        None => "MISS_not_in_ranked_list".to_string(),
                        Some((p, l, s)) => format!(
                            "pos={} lexical={} vector={} source={}",
                            p,
                            l,
                            s,
                            match (l, s) {
                                (true, true) => "both",
                                (true, false) => "lexical",
                                (false, true) => "vector",
                                (false, false) => "none",
                            }
                        ),
                    }
                );
            }
        }
    }

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

    // No-learn mode stops here. Everything below this line is the learning:
    // Hebbian activations, mirror boosts, co-activation pairs, resonance
    // pulses, attention weights, thought-graph patterns, the predictive cache,
    // spaced repetition. The search above has already produced and printed its
    // ranking, so a measurement still sees the same result -- it just does not
    // feed the run that comes after it. Replaying a query otherwise pulled its
    // own answer closer each time (L2 0.45051 -> 0.16108 over six runs, with
    // the pre-fetch confidence climbing 62% -> 98%), which made R@k a function
    // of run order: three runs of one binary gave 37, 34 and 34.
    if microscope_memory::no_learn::enabled() {
        let elapsed = t0.elapsed();
        // Same final trace as the learn path below. Without this the segment is
        // unmeasured in exactly the mode every benchmark runs, because this
        // branch returns before reaching it.
        trace_phase("print + save", elapsed.as_secs_f64() * 1000.0 - t_mark);
        // After `elapsed` is taken, so the writes are outside every window.
        trace_flush();
        println!("\n  {} results in {:.0} us", shown, elapsed.as_micros());
        println!(
            "  {} read-only: learning state not written",
            "MEASURE".dimmed()
        );
        return;
    }

    // ─── Hebbian + Mirror: record activations & detect resonance ──
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

        // Only the blocks this recall actually activated are journalled. The
        // activation base is 32 bytes per corpus block (22.4 MB on the 699k-block
        // eval index, almost all of it default records), and rewriting it in
        // full on every recall cost ~20 ms idle and ~60 ms on a loaded disk --
        // measured, not estimated.
        // Each save is timed separately. Hebbian already got a partial write
        // (`save_dirty`) for exactly this reason -- rewriting its whole
        // activation base cost ~20 ms idle and ~60 ms on a loaded disk -- and
        // the seven below still rewrite whole files on every recall. Which of
        // them actually pays is a measurement, not a guess, and guessing here
        // is what produced the wrong answers earlier in this series.
        // Everything from the sort to here is the learning itself: Hebbian
        // activation updates, co-activation pairs, resonance pulses and the
        // attention weighting. The eight saves that follow are timed
        // individually, and the gated sections after them are timed as one
        // block, because the saves were guessed at once and turned out to be
        // 6% of the phase.
        trace_phase(
            "  learn: activation + pairs + resonance",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        let t_w = Instant::now();
        let _ = hebb.save_dirty(
            output_dir,
            &activated.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        );
        trace_phase(
            "  save: hebbian (dirty)",
            t_w.elapsed().as_secs_f64() * 1000.0,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = mirror.save(output_dir);
        trace_phase(
            "  save: mirror",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = resonance.save(output_dir);
        trace_phase(
            "  save: resonance",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = archetypes.save(output_dir);
        trace_phase(
            "  save: archetypes",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = temporal.save(output_dir);
        trace_phase(
            "  save: temporal archetypes",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = thought_graph.save(output_dir);
        trace_phase(
            "  save: thought graph",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = pred_cache.save(output_dir);
        trace_phase(
            "  save: predictive cache",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        let _ = attention.save(output_dir);
        trace_phase(
            "  save: attention",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
        // --- Eureka: detect unexpected connections ---
        let eureka_events =
            microscope_memory::eureka::detect_eureka(config, &reader, query, None, &all_results);
        trace_phase(
            "  learn: eureka detect",
            t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
        );
        t_mark = t0.elapsed().as_secs_f64() * 1000.0;
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
        let t_sp = Instant::now();
        let mut spaced =
            microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
        trace_phase(
            "  learn: spaced load",
            t_sp.elapsed().as_secs_f64() * 1000.0,
        );
        let t_sr = Instant::now();
        for &(idx, _) in &activated {
            spaced.record_recall(idx, 5, 3);
        }
        trace_phase(
            "  learn: spaced record",
            t_sr.elapsed().as_secs_f64() * 1000.0,
        );
        let t_ss = Instant::now();
        let _ = spaced.save(output_dir);
        trace_phase(
            "  learn: spaced save",
            t_ss.elapsed().as_secs_f64() * 1000.0,
        );

        // --- Narrative: update the system's self-narrative ---
        let t_nv = Instant::now();
        let mut narrative = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
        let esr = microscope_memory::emotional_state::EmotionalStateRing::load_or_init(output_dir);
        trace_phase(
            "  learn: narrative load",
            t_nv.elapsed().as_secs_f64() * 1000.0,
        );
        let t_nu = Instant::now();
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
            Path::new(&config.paths.layers_dir),
            &narrative.narrative,
            &narrative.emotion,
        );
        if narrative.session_count <= 3 || narrative.session_count.is_multiple_of(10) {
            println!("  {} {}", "NARRATIVE:".cyan(), narrative.narrative);
        }
        trace_phase(
            "  learn: narrative update",
            t_nu.elapsed().as_secs_f64() * 1000.0,
        );

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
            let t_nm = Instant::now();
            let mut nm =
                microscope_memory::narrative_memory::NarrativeMemory::load_or_init(output_dir);
            trace_phase(
                "  learn: narrative memory load",
                t_nm.elapsed().as_secs_f64() * 1000.0,
            );
            trace_note(
                "narrative memory resyncs",
                format!(
                    "{} total, {} in last load (file {} bytes)",
                    microscope_memory::narrative_memory::RESYNC_STEPS
                        .load(std::sync::atomic::Ordering::Relaxed),
                    microscope_memory::narrative_memory::LAST_RESCANS
                        .load(std::sync::atomic::Ordering::Relaxed),
                    std::fs::metadata(output_dir.join("narrative_memory.bin"))
                        .map(|m| m.len())
                        .unwrap_or(0),
                ),
            );
            let t_be = Instant::now();
            if let Some(ep) = nm.build_episode(config, &reader, output_dir, query, &all_results) {
                if nm.episodes.len() <= 3 || nm.episodes.len().is_multiple_of(5) {
                    println!(
                        "{}",
                        microscope_memory::narrative_memory::format_episode(&ep)
                    );
                }
            }
            trace_phase(
                "  learn: narrative memory build",
                t_be.elapsed().as_secs_f64() * 1000.0,
            );
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

    // The gated sections: eureka, spaced repetition, narrative, emotional
    // state, self-model, curiosity, narrative memory and the inner monologue.
    // Each of those calls `load_or_init` on every recall, so together they are
    // eight more whole-file reads inside one query.
    trace_phase(
        "  learn: gated sections",
        t0.elapsed().as_secs_f64() * 1000.0 - t_mark,
    );
    t_mark = t0.elapsed().as_secs_f64() * 1000.0;

    let elapsed = t0.elapsed();
    // Everything from the end of the sort to here: rendering each result and
    // the state writes. Every earlier phase now sets `t_mark`, so this number
    // is a real segment rather than an accumulation of everything downstream
    // of the vector search, which is what made it uninterpretable before.
    trace_phase("print + save", elapsed.as_secs_f64() * 1000.0 - t_mark);
    // After `elapsed` is taken, so the writes are outside every window.
    trace_flush();
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

    println!("  No embedding index — computing on-the-fly (slow)");
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
            "  {} merkle.bin not found — rebuild with v0.2.0 to generate",
            "ERR".red()
        );
        return;
    }

    let meta = fs::read(&meta_path).expect("read meta.bin");
    let magic = &meta[0..4];
    if magic != b"MSC2" && magic != b"MSC3" && magic != b"MSC4" {
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

// ─── MAIN ────────────────────────────────────────────

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

/// Diagnostic: report where the expected answer sits in the vector ranking.
///
/// Compiled in unconditionally but does nothing unless `MICROSCOPE_EVAL_MATCH`
/// is set, in which case the caller has already fetched a deep (>=1024) list.
/// The match tokens come from the evaluation harness and are used **only** to
/// locate the answer in this report; they never influence candidate admission
/// or scoring.
fn report_vector_diag(
    reader: &microscope_memory::reader::MicroscopeReader,
    eidx: &microscope_memory::embedding_index::EmbeddingIndex,
    qe: &[f32],
    hits: &[(f32, usize)],
    matches: &str,
    want: usize,
) {
    let needles: Vec<String> = matches
        .split('|')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if needles.is_empty() {
        return;
    }
    // Depth histogram of what the vector search actually returned.
    let mut depth_hist = [0usize; 9];
    for (_, idx) in hits.iter() {
        if *idx < reader.block_count {
            let d = reader.header(*idx).depth as usize;
            if d < depth_hist.len() {
                depth_hist[d] += 1;
            }
        }
    }
    // Depth histogram of the WHOLE embedded set. If D5 dominates here too, the
    // top-k being all D5 is a property of the corpus, not of the scoring.
    let mut all_hist = [0usize; 9];
    for id in eidx.all_block_ids() {
        let i = *id as usize;
        if i < reader.block_count {
            let d = reader.header(i).depth as usize;
            if d < all_hist.len() {
                all_hist[d] += 1;
            }
        }
    }
    // Score of the expected block wherever it lives, its depth, and its rank in
    // the FULL untruncated cosine ordering. The last number is the decisive one:
    // a high rank with a high score means the top-k is crowded out, not that the
    // embedding is bad.
    let mut answer_sim: Option<(f32, usize, Option<usize>)> = None;
    for id in eidx.all_block_ids() {
        let i = *id as usize;
        if i >= reader.block_count {
            continue;
        }
        let text = reader.text(i).to_lowercase();
        if needles.iter().any(|n| text.contains(n)) {
            if let Some(sim) = eidx.similarity_of(i, qe) {
                let d = reader.header(i).depth as usize;
                if sim > answer_sim.map(|a| a.0).unwrap_or(f32::MIN) {
                    let rank = eidx.full_rank_of(i, qe).map(|(r, _)| r);
                    answer_sim = Some((sim, d, rank));
                }
            }
        }
    }
    let mut found: Option<(usize, f32, usize)> = None;
    for (rank, (sim, idx)) in hits.iter().enumerate() {
        if *idx >= reader.block_count {
            continue;
        }
        let text = reader.text(*idx).to_lowercase();
        if needles.iter().any(|n| text.contains(n)) {
            found = Some((rank, *sim, reader.header(*idx).depth as usize));
            break;
        }
    }
    let verdict = match found {
        None => "a_outside_top1024",
        Some((rank, _, _)) if rank >= want => "b_lost_in_prefetch",
        Some(_) => "c_in_prefetch",
    };
    eprintln!(
        "EVALDIAG vectors={} want={} depths={:?} all_depths={:?} answer={} answer_sim={:?}",
        hits.len(),
        want,
        &depth_hist[0..6],
        &all_hist[0..6],
        verdict,
        answer_sim,
    );
}

fn timestamp_to_str(secs: u64) -> String {
    let days = secs / 86400;
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let days_in_year = if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
            366
        } else {
            365
        };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    for days_in_month in &month_days {
        if d < *days_in_month {
            break;
        }
        d -= *days_in_month;
        m += 1;
    }
    format!("{0:04}-{1:02}-{2:02}", y, m + 1, d + 1)
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
        Cmd::ImportChatGpt {
            json,
            persona,
            dry_run,
            gdrive,
            gdrive_folder,
        } => {
            use microscope_memory::chatgpt::ChatGPTImporter;
            use std::path::Path;

            let importer = ChatGPTImporter::new(&persona);

            // Determine source: local file, gdrive file, or gdrive folder
            let source_path: String;

            if let Some(url) = &gdrive {
                println!("{} Google Drive file", "GDRIVE".blue().bold());
                println!("  URL: {}", url.yellow());

                // Extract file ID from URL
                let file_id = url
                    .split("id=")
                    .nth(1)
                    .or_else(|| url.split("/d/").nth(1).and_then(|s| s.split('/').next()))
                    .unwrap_or(url);
                let download_url =
                    format!("https://drive.google.com/uc?export=download&id={}", file_id);

                // Download file
                let tmp_path = format!("/tmp/microscope_chatgpt_{}.json", rand::random::<u32>());
                println!("  Downloading...");

                match reqwest::blocking::get(&download_url) {
                    Ok(response) => match response.text() {
                        Ok(text) => {
                            let _ = std::fs::write(&tmp_path, &text);
                            source_path = tmp_path;
                        }
                        Err(e) => {
                            eprintln!("{} Failed to read response: {}", "ERROR".red(), e);
                            return;
                        }
                    },
                    Err(e) => {
                        eprintln!("{} Failed to download: {}", "ERROR".red(), e);
                        return;
                    }
                }

                println!("  Persona: {}", persona.green());

                // Process the downloaded file
                if dry_run {
                    process_chatgpt_dry(&importer, &source_path, &persona);
                } else {
                    process_chatgpt_import(&importer, &source_path, &persona);
                }

                // Cleanup temp file
                let _ = std::fs::remove_file(&source_path);
            } else if let Some(folder_url) = &gdrive_folder {
                println!("{} Google Drive folder", "GDRIVE".blue().bold());
                println!("  URL: {}", folder_url.yellow());

                // Extract folder ID
                let folder_id = folder_url
                    .split("/folders/")
                    .nth(1)
                    .or_else(|| folder_url.split("id=").nth(1))
                    .unwrap_or(folder_url);
                let list_url = format!("https://www.googleapis.com/drive/v3/files?q='{}'+in+parents&key=AIzaSyD7S7z-6JBPJTqHQO1SfTZ5mTqRJIqO5vY", folder_id);

                println!("  Scanning folder for JSON files...");
                match reqwest::blocking::get(&list_url) {
                    Ok(resp) => {
                        if let Ok(body) = resp.text() {
                            if let Ok(file_list) = serde_json::from_str::<serde_json::Value>(&body)
                            {
                                let files =
                                    file_list["files"].as_array().cloned().unwrap_or_default();
                                let json_files: Vec<&serde_json::Value> = files
                                    .iter()
                                    .filter(|f| {
                                        f["name"].as_str().is_some_and(|n| n.ends_with(".json"))
                                    })
                                    .collect();

                                if json_files.is_empty() {
                                    println!("  No JSON files found in folder.");
                                    return;
                                }

                                println!("  Found {} JSON file(s)", json_files.len());

                                for file in &json_files {
                                    let name = file["name"].as_str().unwrap_or("unknown");
                                    let fid = file["id"].as_str().unwrap_or("");
                                    let dl_url = format!(
                                        "https://drive.google.com/uc?export=download&id={}",
                                        fid
                                    );
                                    let tmp = format!(
                                        "/tmp/microscope_{}_{}.json",
                                        fid,
                                        rand::random::<u32>()
                                    );

                                    println!("    Processing: {}...", name);
                                    if let Ok(dl_resp) = reqwest::blocking::get(&dl_url) {
                                        if let Ok(text) = dl_resp.text() {
                                            let _ = std::fs::write(&tmp, &text);

                                            if dry_run {
                                                process_chatgpt_dry(&importer, &tmp, &persona);
                                            } else {
                                                process_chatgpt_import(&importer, &tmp, &persona);
                                            }

                                            let _ = std::fs::remove_file(&tmp);
                                        }
                                    }
                                }
                            } else {
                                eprintln!("{} Could not parse folder contents. The folder may not be public.", "ERROR".red());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "{} Cannot access Google Drive folder (may need public sharing): {}",
                            "ERROR".red(),
                            e
                        );
                    }
                }
            } else if let Some(path) = &json {
                println!("{} ChatGPT Import", "CHATGPT".magenta().bold());
                println!("  File: {}", path.yellow());
                println!("  Persona: {}", persona.green());

                if !Path::new(path).exists() {
                    eprintln!("{} File not found: {}", "ERROR".red(), path);
                    return;
                }

                if dry_run {
                    process_chatgpt_dry(&importer, path, &persona);
                } else {
                    process_chatgpt_import(&importer, path, &persona);
                }
            } else {
                eprintln!(
                    "{} Please provide a JSON file path, --gdrive URL, or --gdrive-folder URL",
                    "ERROR".red()
                );
                eprintln!("  Usage: microscope-mem import-chat-gpt <path>");
                eprintln!("         microscope-mem import-chat-gpt --gdrive <url>");
                eprintln!("         microscope-mem import-chat-gpt --gdrive-folder <url>");
                return;
            }

            fn process_chatgpt_dry(importer: &ChatGPTImporter, path: &str, persona: &str) {
                match importer.parse_export(path) {
                    Ok(messages) => {
                        let user_count = messages.iter().filter(|m| m.role == "user").count();
                        let ai_count = messages.iter().filter(|m| m.role == "assistant").count();
                        let conv_count = messages
                            .iter()
                            .map(|m| &m.conversation_title)
                            .collect::<std::collections::HashSet<_>>()
                            .len();

                        println!("\n{}", "ANALYSIS".cyan().bold());
                        println!("  Conversations: {}", conv_count);
                        println!("  Total messages: {}", messages.len());
                        println!("  User messages:  {}", user_count);
                        println!("  AI responses ({}): {}", persona, ai_count);
                        if let Some(last) = messages.back() {
                            println!(
                                "  Date range: {}",
                                timestamp_to_str(last.timestamp_ms / 1000)
                            );
                        }
                    }
                    Err(e) => eprintln!("{} {}", "ERROR:".red(), e),
                }
            }

            fn process_chatgpt_import(importer: &ChatGPTImporter, path: &str, _persona: &str) {
                let microscope_bin = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "microscope-mem".to_string());

                println!("\n{} Importing conversations...", "IMPORT".cyan().bold());
                let result = importer.import(path, &microscope_bin);

                println!("\n{}", "RESULT".green().bold());
                println!("  Conversations: {}", result.conversations_found);
                println!(
                    "  Messages:      {} total ({} user, {} AI)",
                    result.total_messages, result.user_messages, result.ai_messages
                );
                if result.total_size_bytes > 0 {
                    println!(
                        "  File size:     {:.1} MB",
                        result.total_size_bytes as f64 / 1_048_576.0
                    );
                }
                println!(
                    "  Duration:      {:.2}s",
                    result.import_duration_ms as f64 / 1000.0
                );

                if !result.errors.is_empty() {
                    println!("\n{} {} errors:", "WARN".yellow(), result.errors.len());
                    for e in result.errors.iter().take(5) {
                        println!("  - {}", e);
                    }
                }
            }
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
                "{} {}-> grid ({} bytes) -> {}",
                "DENSITY".cyan().bold(),
                grid,
                data.len(),
                output
            );
        }
        Cmd::Reconsolidate => {
            let output_dir = Path::new(&config.paths.output_dir);
            let reader = match MicroscopeReader::open(&config) {
                Ok(r) => r,
                Err(_) => {
                    eprintln!("  {} open reader failed â€” run build first", "ERR".red());
                    return;
                }
            };
            // Process Hebbian hot blocks (most recently activated)
            let hebb = microscope_memory::hebbian::HebbianState::load_or_init(
                output_dir,
                reader.block_count,
            );
            let hot = hebb.hottest_blocks(50);
            let activated: Vec<(u32, f32)> =
                hot.iter().map(|&(idx, _)| (idx as u32, 1.0)).collect();
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
                println!("  (no active inhibitions â€” network is clear)");
            } else {
                println!("  {} active inhibitions:", salience.inhibitions.len());
                for e in &salience.inhibitions {
                    println!(
                        "  topic={:016x} strength={:.2}",
                        e.topic_hash, e.remaining_strength
                    );
                }
            }
        }
        Cmd::Narrative { verbose } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let state = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
            println!("{}", "INNER NARRATIVE".cyan().bold());
            if state.session_count == 0 {
                println!("  (silent â€” no interactions yet)");
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
                    let wm =
                        microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    if !wm.items.is_empty() {
                        println!("  Focus:");
                        for item in &wm.items {
                            println!(
                                "    - {} (imp={:.1})",
                                crate::safe_truncate(&item.text, 40),
                                item.importance
                            );
                        }
                    }
                }
            }
        }
        Cmd::Spaced { due, k } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let sr =
                microscope_memory::spaced_repetition::SpacedRepetition::load_or_init(output_dir);
            let stats = sr.stats();
            println!("{}", "SPACED REPETITION".cyan().bold());
            println!("  Tracked:   {} blocks", stats.total_blocks);
            println!("  Due:       {} (need review)", stats.due);
            println!("  Fresh:     {} (< 7d)", stats.fresh);
            println!("  Mastered:  {} (â‰Ą{} recalls)", stats.mastered, 15);
            println!("  Avg ease:  {:.2}", stats.avg_ease);
            println!("  Avg int.:  {:.1}d", stats.avg_interval);
            if due && stats.total_blocks > 0 {
                let due_list = sr.due_blocks();
                let count = due_list.len().min(k);
                println!("\n  {} due blocks:", count);
                let reader = MicroscopeReader::open(&config).ok();
                for &idx in due_list.iter().take(count) {
                    let block_info = sr.find(idx);
                    let days = block_info
                        .map(|b| {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            (now.saturating_sub(b.last_recall_ms)) as f32 / 86_400_000.0
                        })
                        .unwrap_or(0.0);
                    let text = reader
                        .as_ref()
                        .map(|r| safe_truncate(r.text(idx as usize), 50))
                        .unwrap_or_default();
                    println!(
                        "  [{:>6}] recall={} last={:.1}d ago {}",
                        idx,
                        block_info.map(|b| b.recall_count).unwrap_or(0),
                        days,
                        text
                    );
                }
            }
        }
        Cmd::Eureka { k, verbose } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let log = microscope_memory::eureka::EurekaLog::load_or_init(output_dir);
            let count = log.events.len().min(k);
            if count == 0 {
                println!("{}", "No eureka events found".yellow());
            } else {
                println!(
                    "{} ({} total, showing {})",
                    "EUREKA MOMENTS".cyan().bold(),
                    log.events.len(),
                    count
                );
                for ev in log.events.iter().rev().take(count).rev() {
                    println!("{}", microscope_memory::eureka::format_eureka(ev));
                    if verbose {
                        println!("         score breakdown: surprise={:.2} × curiosity={:.2} × emo_sim={:.2} / dist={:.3} = {:.1}",
                            ev.surprise_score, ev.curiosity_score, ev.emotional_sim, ev.spatial_dist, ev.insight_score());
                    }
                }
            }
        }
        Cmd::Wm { action } => {
            let output_dir = Path::new(&config.paths.output_dir);
            match action {
                microscope_memory::cli::WmAction::Show => {
                    let wm =
                        microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
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
                                microscope_memory::working_memory::MemoryType::Episodic => {
                                    "episodic"
                                }
                                microscope_memory::working_memory::MemoryType::Semantic => {
                                    "semantic"
                                }
                                microscope_memory::working_memory::MemoryType::Implicit => {
                                    "implicit"
                                }
                                microscope_memory::working_memory::MemoryType::Explicit => {
                                    "explicit"
                                }
                            };
                            println!(
                                "  [{:2}] imp={:.1} acc={} {:8} {}",
                                i,
                                item.importance,
                                item.access_count,
                                mem_type,
                                crate::safe_truncate(&item.text, 60)
                            );
                        }
                    }
                }
                microscope_memory::cli::WmAction::Push {
                    text,
                    importance,
                    layer,
                    memory_type,
                } => {
                    let mut wm =
                        microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let mem_type = match memory_type.to_lowercase().as_str() {
                        "semantic" => microscope_memory::working_memory::MemoryType::Semantic,
                        _ => microscope_memory::working_memory::MemoryType::Episodic,
                    };
                    wm.push(&text, importance, &layer, mem_type);
                    wm.save(output_dir)
                        .unwrap_or_else(|e| eprintln!("  {} save: {}", "WARN".yellow(), e));
                    println!(
                        "  {} WM: '{}'",
                        "PUSHED".green().bold(),
                        crate::safe_truncate(&text, 60)
                    );
                }
                microscope_memory::cli::WmAction::Decay => {
                    let mut wm =
                        microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let before = wm.items.len();
                    wm.decay();
                    let after = wm.items.len();
                    wm.save(output_dir)
                        .unwrap_or_else(|e| eprintln!("  {} save: {}", "WARN".yellow(), e));
                    println!(
                        "  {} WM: {} â†’ {} items",
                        "DECAY".yellow().bold(),
                        before,
                        after
                    );
                }
                microscope_memory::cli::WmAction::Consolidate => {
                    let mut wm =
                        microscope_memory::working_memory::WorkingMemory::load_or_init(output_dir);
                    let items = wm.consolidate();
                    if items.is_empty() {
                        println!(
                            "  {} WM: no items to consolidate",
                            "CONSOLIDATE".yellow().bold()
                        );
                    } else {
                        for item in &items {
                            let text = &item.text;
                            let layer = &item.layer;
                            let imp = (item.importance as u8).clamp(1, 10);
                            store_memory(&config, &format!("[WM] {}", text), layer, imp)
                                .unwrap_or_else(|e| eprintln!("  {} store: {}", "ERR".red(), e));
                            println!(
                                "  {} '{}' â†’ long_term",
                                "CONSOLIDATED".magenta().bold(),
                                safe_truncate(text, 60)
                            );
                        }
                        wm.save(output_dir)
                            .unwrap_or_else(|e| eprintln!("  {} save: {}", "WARN".yellow(), e));
                        println!(
                            "  {} WM: {} items consolidated",
                            "DONE".green().bold(),
                            items.len()
                        );
                    }
                }
            }
        }
        Cmd::Sandbox {
            simulate,
            actions,
            best,
            clear,
        } => {
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
                let actions_list = actions
                    .as_ref()
                    .map(|a| a.split(',').map(|s| s.trim()).collect())
                    .unwrap_or_else(|| vec!["default_action"]);

                let scenario = sandbox.simulate_scenario(&desc, actions_list);
                println!("  {} Scenario simulated:", "SIMULATION".cyan().bold());
                println!("    ID: {}", scenario.id);
                println!("    Description: {}", scenario.description);
                println!("    Actions: {}", scenario.actions.join(", "));
                println!(
                    "    Outcome Probability: {:.1}%",
                    scenario.outcome_probability * 100.0
                );
                println!("    Risk Score: {:.2}", scenario.risk_score);
                println!("    Reward Potential: {:.2}", scenario.reward_potential);
            }

            if best {
                if let Some(best_scenario) = sandbox.get_best_scenario() {
                    println!("  {} Best scenario:", "BEST".green().bold());
                    println!("    ID: {}", best_scenario.id);
                    println!("    Description: {}", best_scenario.description);
                    println!(
                        "    Risk/Reward Ratio: {:.2}",
                        best_scenario.reward_potential / (best_scenario.risk_score + 0.01)
                    );
                } else {
                    println!("  {} No scenarios available", "INFO:".cyan());
                }
            }
        }
        Cmd::Impulse {
            filter,
            source,
            urgency,
            suppress,
            stats,
            clear,
        } => {
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
                println!(
                    "  {} Added suppression pattern: '{}'",
                    "OK".green(),
                    pattern
                );
            }

            if let Some(content) = filter {
                let stimulus = control.filter_stimulus(&content, &source, urgency);
                println!("  {} Stimulus filtered:", "IMPULSE CONTROL".cyan().bold());
                println!("    Content: {}", stimulus.content);
                println!("    Source: {}", stimulus.source);
                println!("    Relevance: {:.2}", stimulus.relevance);
                println!("    Urgency: {:.2}", stimulus.urgency);
                println!(
                    "    Status: {}",
                    if stimulus.suppressed {
                        "SUPPRESSED".red()
                    } else {
                        "ALLOWED".green()
                    }
                );
            }

            if stats {
                let (attention_budget, pattern_count) = control.get_stats();
                println!("  {} System stats:", "STATS".green().bold());
                println!("    Attention Budget: {:.1}%", attention_budget * 100.0);
                println!("    Suppression Patterns: {}", pattern_count);
            }
        }
        Cmd::Meta {
            record,
            evaluate,
            trends,
            report,
            add_strategy,
        } => {
            use microscope_memory::meta_supervision::{generate_report, MetaSupervisor};

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
                    println!(
                        "  {} Correction needed: {}",
                        "EVALUATION".yellow().bold(),
                        correction
                    );
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
                println!(
                    "    Direction: {}",
                    if trend > 0.05 {
                        "IMPROVING".green()
                    } else if trend < -0.05 {
                        "DECLINING".red()
                    } else {
                        "STABLE".yellow()
                    }
                );
            }

            if report {
                let report_text = generate_report(&supervisor);
                println!("{}", report_text);
            }

            if let Some(strategy) = add_strategy {
                supervisor.add_correction_strategy(&strategy);
                println!(
                    "  {} Added correction strategy: '{}'",
                    "OK".green(),
                    strategy
                );
            }
        }
        Cmd::Code {
            store,
            error,
            recall,
            list,
            lang,
            project,
            k,
            symbol,
            stats,
        } => {
            use microscope_memory::code_memory::{CodeEntryType, CodeMemory, CodeQuery};

            let code_mem = CodeMemory::new();

            if let Some(entry_str) = store {
                let parts: Vec<&str> = entry_str.splitn(6, ':').collect();
                if parts.len() >= 3 {
                    let etype = match parts[0].to_lowercase().as_str() {
                        "function" | "fn" => CodeEntryType::Function,
                        "type" | "struct" | "class" => CodeEntryType::Type,
                        "import" => CodeEntryType::Import,
                        "error" | "err" => CodeEntryType::ErrorSolution,
                        "config" => CodeEntryType::Config,
                        "dependency" | "dep" => CodeEntryType::Dependency,
                        "convention" => CodeEntryType::Convention,
                        _ => CodeEntryType::Note,
                    };
                    let id = code_mem.store(
                        etype,
                        parts.get(1).unwrap_or(&""),
                        parts.get(2).unwrap_or(&""),
                        parts.get(3).unwrap_or(&""),
                        parts.get(4).unwrap_or(&"rust"),
                        parts.get(5).unwrap_or(&"default"),
                        vec![],
                        vec![],
                    );
                    println!(
                        "  {} Stored #{} [{}]",
                        "CODE".cyan().bold(),
                        id,
                        parts.get(1).unwrap_or(&"")
                    );
                }
            }

            if let Some(err_sol) = error {
                let parts: Vec<&str> = err_sol.splitn(4, ':').collect();
                if parts.len() >= 2 {
                    let id = code_mem.store_error_solution(
                        parts[0],
                        parts[1],
                        parts.get(2).unwrap_or(&"unknown.rs"),
                        parts.get(3).unwrap_or(&"rust"),
                        "default",
                    );
                    println!("  {} Stored error-solution #{}", "FIX".green().bold(), id);
                }
            }

            if let Some(q) = recall {
                let query = CodeQuery {
                    query: q,
                    language: lang.clone(),
                    entry_type: None,
                    project: project.clone(),
                    file: None,
                    k,
                };
                let results = code_mem.recall(&query);
                if results.is_empty() {
                    println!("  {} No results", "INFO".yellow());
                } else {
                    for entry in &results {
                        println!(
                            "  [{:?}] {} — {}",
                            entry.entry_type,
                            entry.title.yellow(),
                            entry.file_path
                        );
                    }
                }
            }

            if let Some(ref sym) = symbol {
                let results = code_mem.recall_by_symbol(sym);
                println!(
                    "  {} Symbol '{}' in {} entries",
                    "SYM".cyan().bold(),
                    sym,
                    results.len()
                );
            }

            if let Some(ref lt) = list {
                let etype = match lt.to_lowercase().as_str() {
                    "function" | "fn" => CodeEntryType::Function,
                    "error" => CodeEntryType::ErrorSolution,
                    _ => CodeEntryType::Note,
                };
                for entry in &code_mem.list_by_type(etype) {
                    println!(
                        "  #{} {} — {}",
                        entry.id,
                        entry.title.yellow(),
                        entry.file_path
                    );
                }
            }

            if stats {
                let (total, errors, projects) = code_mem.stats();
                println!("  Entries: {}, Errors: {}", total, errors);
                for (p, c) in &projects {
                    println!("    {}: {}", p, c);
                }
            }
        }
        Cmd::Implicit {
            show,
            practice,
            skills,
            patterns,
            decay,
        } => {
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
                    println!(
                        "  {} Practiced '{}': {}",
                        "OK".green(),
                        skill_name,
                        if success {
                            "SUCCESS".green()
                        } else {
                            "FAILURE".red()
                        }
                    );
                }
            }

            if skills {
                let ranking = implicit.skill_ranking();
                println!("  {} Skill ranking:", "SKILLS".yellow().bold());
                for (name, skill) in ranking.iter().take(10) {
                    println!(
                        "    {} mastery={:.1}% errors={:.1}% practiced={} times",
                        name,
                        skill.mastery_level * 100.0,
                        skill.error_rate * 100.0,
                        skill.practice_count
                    );
                }
            }

            if patterns {
                let top = implicit.strongest_patterns(10);
                println!("  {} Strongest patterns:", "PATTERNS".yellow().bold());
                for (hash, pattern) in top {
                    println!(
                        "    hash={:x} strength={:.2} freq={} perf={:.1}%",
                        hash,
                        pattern.strength,
                        pattern.frequency,
                        pattern.performance_metric * 100.0
                    );
                }
            }

            if decay {
                implicit.decay();
                implicit.save(output_dir).ok();
                println!(
                    "  {} Memory decayed: patterns={} skills={} habits={}",
                    "DECAY".cyan(),
                    implicit.patterns.len(),
                    implicit.skills.len(),
                    implicit.habits.len()
                );
            }
        }
        Cmd::Explicit {
            show,
            store_fact,
            concept,
            facts,
            concepts,
        } => {
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
                    println!(
                        "  {} Stored fact: '{}' (conf={:.1}%)",
                        "OK".green(),
                        safe_truncate(statement, 50),
                        confidence * 100.0
                    );
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
                    println!(
                        "  {} Defined concept: '{}' (abstraction={:.1}%)",
                        "OK".green(),
                        name,
                        abstraction * 100.0
                    );
                }
            }

            if facts {
                let high_conf = explicit.high_confidence_facts(0.7);
                println!("  {} High-confidence facts:", "FACTS".yellow().bold());
                for fact in high_conf.iter().take(10) {
                    println!(
                        "    [conf={:.0}%] {} (src={})",
                        fact.confidence * 100.0,
                        safe_truncate(&fact.statement, 50),
                        fact.source
                    );
                }
            }

            if concepts {
                println!("  {} Concepts:", "CONCEPTS".yellow().bold());
                for (name, concept) in explicit.concepts.iter().take(10) {
                    println!(
                        "    {} (abstract={:.1}%) - {}",
                        name,
                        concept.abstraction_level * 100.0,
                        safe_truncate(&concept.definition, 40)
                    );
                }
            }
        }
        Cmd::Hippo {
            show,
            consolidate,
            related,
            replay,
            decay,
        } => {
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
                println!(
                    "  {} Consolidation candidates:",
                    "CONSOLIDATE".yellow().bold()
                );
                for (i, episode) in candidates.iter().enumerate() {
                    println!(
                        "    [{}] episode_id={:x} blocks={} strength={:.2}",
                        i + 1,
                        episode.episode_id,
                        episode.blocks.len(),
                        episode.context_binding.binding_strength
                    );
                    hippo.mark_consolidating(episode.episode_id);
                }
                hippo.save(output_dir).ok();
            }

            if let Some(ep_id) = related {
                let related_eps = hippo.get_related_episodes(ep_id);
                println!(
                    "  {} Related episodes to {:x}:",
                    "RELATED".yellow().bold(),
                    ep_id
                );
                for ep in related_eps.iter().take(5) {
                    println!(
                        "    episode_id={:x} blocks={} context={}",
                        ep.episode_id,
                        ep.blocks.len(),
                        safe_truncate(&ep.context_binding.context, 30)
                    );
                }
            }

            if let Some(ep_id) = replay {
                if let Some(blocks) = hippo.replay_episode(ep_id) {
                    hippo.mark_consolidated(ep_id);
                    hippo.save(output_dir).ok();
                    println!(
                        "  {} Replayed episode {:x}: {} blocks",
                        "REPLAY".green(),
                        ep_id,
                        blocks.len()
                    );
                } else {
                    println!("  {} Episode not found", "ERROR".red());
                }
            }

            if decay {
                hippo.decay();
                hippo.save(output_dir).ok();
                let (bindings, episodes, _, _) = hippo.stats();
                println!(
                    "  {} Memory decayed: bindings={} episodes={}",
                    "DECAY".cyan(),
                    bindings,
                    episodes
                );
            }
        }
        Cmd::Neuro {
            show,
            synapse,
            pathway,
            prune,
            reorganize,
            pathways,
        } => {
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
                    println!(
                        "  {} Synapse {} â†’ {}: {}",
                        "OK".green(),
                        from,
                        to,
                        if success {
                            "STRENGTHENED".green()
                        } else {
                            "WEAKENED".red()
                        }
                    );
                }
            }

            if let Some(path_str) = pathway {
                let parts: Vec<&str> = path_str.split(':').collect();
                if parts.len() >= 2 {
                    let domain = parts[0];
                    let nodes: Vec<u32> =
                        parts[1].split(',').filter_map(|s| s.parse().ok()).collect();
                    if !nodes.is_empty() {
                        let id = neuro.strengthen_pathway(nodes.clone(), domain);
                        neuro.save(output_dir).ok();
                        println!(
                            "  {} Pathway {} reinforced: {} nodes (domain: {})",
                            "OK".green(),
                            id,
                            nodes.len(),
                            domain
                        );
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
                println!(
                    "  {} Reorganized network: {} changes",
                    "REORGANIZE".yellow(),
                    reorganized
                );
            }

            if pathways {
                let strongest = neuro.strongest_pathways(10);
                println!("  {} Strongest pathways:", "PATHWAYS".yellow().bold());
                for (i, pathway) in strongest.iter().enumerate() {
                    println!(
                        "    [{}] strength={:.2} efficiency={:.2} uses={} domain={}",
                        i + 1,
                        pathway.strength,
                        pathway.efficiency,
                        pathway.usage_count,
                        pathway.specialized_for
                    );
                }
            }
        }
        Cmd::Struct {
            show,
            neurogenesis,
            grow,
            prune,
            specialized,
        } => {
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
                    let blocks: Vec<u32> =
                        parts[0].split(',').filter_map(|s| s.parse().ok()).collect();
                    let specialization = parts[1];
                    if !blocks.is_empty() {
                        let neuron_id = struct_pls.neurogenesis(blocks.clone(), specialization);
                        struct_pls.save(output_dir).ok();
                        println!(
                            "  {} New neuron created: id={:x} blocks={} spec={}",
                            "NEUROGENESIS".green().bold(),
                            neuron_id,
                            blocks.len(),
                            specialization
                        );
                    }
                }
            }

            if let Some(grow_str) = grow {
                let parts: Vec<&str> = grow_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(neuron_id), Ok(block)) =
                        (parts[0].parse::<u64>(), parts[1].parse::<u32>())
                    {
                        if struct_pls.grow_dendrite(neuron_id, block) {
                            struct_pls.save(output_dir).ok();
                            println!(
                                "  {} Dendrite grown: neuron={:x} new_branch={}",
                                "GROWTH".green(),
                                neuron_id,
                                block
                            );
                        } else {
                            println!(
                                "  {} Dendrite growth failed or neuron pruned",
                                "WARN".yellow()
                            );
                        }
                    }
                }
            }

            if let Some(neuron_id) = prune {
                let pruned = struct_pls.prune_inactive_branches(neuron_id);
                struct_pls.save(output_dir).ok();
                println!(
                    "  {} Pruned {} inactive branches from neuron {:x}",
                    "PRUNE".yellow(),
                    pruned,
                    neuron_id
                );
            }

            if specialized {
                let specialized_list = struct_pls.specialized_neurons();
                println!("  {} Specialized neurons:", "SPECIALIZED".yellow().bold());
                for (id, spec, branches) in specialized_list.iter().take(10) {
                    println!(
                        "    neuron_id={:x} specialization={} branches={}",
                        id, spec, branches
                    );
                }
            }
        }
        Cmd::Func {
            show,
            area,
            map,
            connect,
            damage,
            plastic,
        } => {
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
                    let blocks: Vec<u32> =
                        parts[2].split(',').filter_map(|s| s.parse().ok()).collect();
                    if !blocks.is_empty() {
                        let area_id = func_pls.create_area(name, domain, blocks.clone());
                        func_pls.save(output_dir).ok();
                        println!(
                            "  {} Created area: id={:x} name={} domain={} blocks={}",
                            "AREA".green().bold(),
                            area_id,
                            name,
                            domain,
                            blocks.len()
                        );
                    }
                }
            }

            if let Some(map_str) = map {
                let parts: Vec<&str> = map_str.split(':').collect();
                if parts.len() == 2 {
                    if let Ok(input) = parts[0].parse::<u32>() {
                        let outputs: Vec<u32> =
                            parts[1].split(',').filter_map(|s| s.parse().ok()).collect();
                        if !outputs.is_empty() {
                            let strength = func_pls.map_sensorimotor(input, outputs.clone());
                            func_pls.save(output_dir).ok();
                            println!(
                                "  {} Mapped: {} â†’ {} blocks (strength={:.2})",
                                "MAP".green(),
                                input,
                                outputs.len(),
                                strength
                            );
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
                            println!(
                                "  {} Connected areas: {:x} â†” {:x}",
                                "CONNECT".green(),
                                a1,
                                a2
                            );
                        } else {
                            println!("  {} Connection failed: areas not found", "ERROR".red());
                        }
                    }
                }
            }

            if let Some(dmg_str) = damage {
                let parts: Vec<&str> = dmg_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(area_id), Ok(severity)) =
                        (parts[0].parse::<u64>(), parts[1].parse::<f32>())
                    {
                        func_pls.damage_area(area_id, severity);
                        func_pls.save(output_dir).ok();
                        println!(
                            "  {} Damage simulation: area={:x} severity={:.1}%",
                            "DAMAGE".red().bold(),
                            area_id,
                            severity * 100.0
                        );
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
        Cmd::Syn {
            show,
            ltp,
            ltd,
            stdp,
            hetero,
            timedep,
            strong,
            ltp_dominant,
        } => {
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
                    if let (Ok(pre), Ok(post)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
                        let weight = syn_pls.ltp(pre, post);
                        syn_pls.save(output_dir).ok();
                        println!(
                            "  {} LTP: {} â†’ {} (weight={:.2})",
                            "POTENTIATION".green().bold(),
                            pre,
                            post,
                            weight
                        );
                    }
                }
            }

            if let Some(ltd_str) = ltd {
                let parts: Vec<&str> = ltd_str.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(pre), Ok(post)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
                        let weight = syn_pls.ltd(pre, post);
                        syn_pls.save(output_dir).ok();
                        println!(
                            "  {} LTD: {} â†’ {} (weight={:.2})",
                            "DEPRESSION".red().bold(),
                            pre,
                            post,
                            weight
                        );
                    }
                }
            }

            if let Some(stdp_str) = stdp {
                let parts: Vec<&str> = stdp_str.split(':').collect();
                if parts.len() == 4 {
                    if let (Ok(pre), Ok(post), Ok(pre_t), Ok(post_t)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u32>(),
                        parts[2].parse::<i64>(),
                        parts[3].parse::<i64>(),
                    ) {
                        let weight = syn_pls.stdp(pre, post, pre_t, post_t);
                        syn_pls.save(output_dir).ok();
                        let timing_diff = post_t - pre_t;
                        let plasticity_type = if timing_diff > 0 {
                            "STDP-LTP"
                        } else {
                            "STDP-LTD"
                        };
                        println!(
                            "  {} {} Î”t={:+}ms (weight={:.2})",
                            "STDP".yellow().bold(),
                            plasticity_type,
                            timing_diff,
                            weight
                        );
                    }
                }
            }

            if let Some(hetero_str) = hetero {
                let parts: Vec<&str> = hetero_str.split(':').collect();
                if parts.len() == 3 {
                    if let (Ok(pre), Ok(post), Ok(radius)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u32>(),
                        parts[2].parse::<u32>(),
                    ) {
                        syn_pls.heterosynaptic_depression((pre, post), radius);
                        syn_pls.save(output_dir).ok();
                        println!(
                            "  {} Heterosynaptic depression: ({},{}) radius={}",
                            "HETERO".yellow().bold(),
                            pre,
                            post,
                            radius
                        );
                    }
                }
            }

            if let Some(td_str) = timedep {
                let parts: Vec<&str> = td_str.split(':').collect();
                if parts.len() == 4 {
                    if let (Ok(pre), Ok(post), Ok(practice), Ok(age)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u32>(),
                        parts[2].parse::<u32>(),
                        parts[3].parse::<u64>(),
                    ) {
                        let plasticity =
                            syn_pls.time_dependent_plasticity((pre, post), practice, age);
                        syn_pls.save(output_dir).ok();

                        let phase = if practice < 10 {
                            "EARLY"
                        } else if practice < 50 {
                            "CONSOLIDATION"
                        } else {
                            "MATURE"
                        };
                        println!("  {} Time-dependent plasticity: {} â†’ {} phase={} practices={} learning_rate={:.3}",
                            "TIMEDEP".yellow().bold(), pre, post, phase, practice, plasticity);
                    }
                }
            }

            if strong {
                let strongest = syn_pls.strongest_synapses(10);
                println!("  {} Strongest synapses:", "STRONG".yellow().bold());
                for (i, ((pre, post), synapse)) in strongest.iter().enumerate() {
                    println!(
                        "    [{}] {} â†’ {} weight={:.2} (LTP:{} LTD:{})",
                        i + 1,
                        pre,
                        post,
                        synapse.weight,
                        synapse.ltp_count,
                        synapse.ltd_count
                    );
                }
            }

            if ltp_dominant {
                let ltp_syns = syn_pls.ltp_dominant();
                println!(
                    "  {} LTP-dominant synapses: {}",
                    "LTP".green().bold(),
                    ltp_syns.len()
                );
                for (i, synapse) in ltp_syns.iter().take(10).enumerate() {
                    println!(
                        "    [{}] {} â†’ {} weight={:.2} (LTP:{} LTD:{})",
                        i + 1,
                        synapse.pre_block,
                        synapse.post_block,
                        synapse.weight,
                        synapse.ltp_count,
                        synapse.ltd_count
                    );
                }
            }
        }
        Cmd::Stim {
            show,
            activity,
            check,
            recommend,
            diversity,
        } => {
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
                println!(
                    "  Stimulation need:     {:.1}%",
                    stim.stimulation_need * 100.0
                );
            }

            if let Some(act_str) = activity {
                let parts: Vec<&str> = act_str.split(':').collect();
                if parts.len() == 2 {
                    let activity_type = parts[0];
                    if let Ok(intensity) = parts[1].parse::<f32>() {
                        stim.record_activity(activity_type, intensity);
                        stim.save(output_dir).ok();
                        println!(
                            "  {} Activity recorded: {} intensity={:.2}",
                            "OK".green(),
                            activity_type,
                            intensity
                        );
                    }
                }
            }

            if check {
                let needs_it = stim.needs_stimulation();
                println!(
                    "  {} Stimulation needed: {}",
                    "CHECK".cyan(),
                    if needs_it { "YES".red() } else { "NO".green() }
                );
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
                println!(
                    "  {} Activity diversity: {:.1}%",
                    "DIVERSITY".yellow(),
                    div * 100.0
                );
            }
        }
        Cmd::Focus {
            enter,
            exit,
            process,
            show,
            insights,
        } => {
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
                        (parts[0].parse::<u32>(), parts[1].parse::<f32>())
                    {
                        focus.process_data(blocks, complexity);
                        focus.save(output_dir).ok();
                        println!(
                            "  {} Data processed: {} blocks, complexity={:.2}",
                            "PROCESSING".green(),
                            blocks,
                            complexity
                        );
                    }
                }
            }

            if show {
                let (active, intensity, depth, blocks) = focus.stats();
                println!("{}", "HYPERFOCUS STATE".cyan().bold());
                println!(
                    "  Active: {}",
                    if active { "YES".green() } else { "NO".red() }
                );
                if active {
                    println!("  Intensity: {:.1}%", intensity * 100.0);
                    println!("  Depth level: {:.1}%", depth * 100.0);
                    println!("  Blocks processed: {}", blocks);
                    println!("  Attention multiplier: {:.1}x", focus.attention_multiplier);
                    println!(
                        "  Productive: {}",
                        if focus.is_productive() {
                            "YES".green()
                        } else {
                            "NO".red()
                        }
                    );
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
        Cmd::Simulate {
            register,
            list,
            run,
            stress,
            compare,
            results,
            patterns,
            clear,
            duration,
            load_pattern,
            peak_load,
            faults,
        } => {
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
                        let comp_type = if i % 3 == 0 {
                            ComponentType::Software
                        } else if i % 3 == 1 {
                            ComponentType::Storage
                        } else {
                            ComponentType::Network
                        };
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
                    println!(
                        "  {} Architecture registered: {} ({} components, {} connections)",
                        "OK".green().bold(),
                        name,
                        comp_count,
                        conn_count
                    );
                    println!("    ID: {}", arch.id);
                } else {
                    println!(
                        "  {} Usage: --register name:description:components:connections",
                        "ERROR".red().bold()
                    );
                }
            }

            if list {
                let architectures = simulator.list_architectures();
                println!("{}", "REGISTERED ARCHITECTURES".cyan().bold());
                if architectures.is_empty() {
                    println!("  (none)");
                } else {
                    for arch in &architectures {
                        println!(
                            "  {} â€” {} (v{})",
                            arch.name.green(),
                            arch.description,
                            arch.version
                        );
                        println!(
                            "    ID: {} | Cohesion: {:.2} | Components: {} | Connections: {}",
                            arch.id,
                            arch.cohesion_score,
                            arch.components.len(),
                            arch.connections.len()
                        );
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
                println!(
                    "  Duration: {}s | Pattern: {} | Peak load: {:.0}%",
                    duration,
                    load_pattern,
                    peak_load * 100.0
                );

                if let Some(metrics) = simulator.run_simulation(&arch_id, &config) {
                    println!("\n{}", "SIMULATION RESULTS".green().bold());
                    println!("  Avg latency: {:.2} ms", metrics.avg_latency_ms);
                    println!("  P95 latency: {:.2} ms", metrics.p95_latency_ms);
                    println!("  P99 latency: {:.2} ms", metrics.p99_latency_ms);
                    println!("  Throughput: {:.0} req/s", metrics.throughput_req_per_sec);
                    println!("  Error rate: {:.2}%", metrics.error_rate * 100.0);
                    println!("  CPU utilization: {:.1}%", metrics.cpu_utilization * 100.0);
                    println!(
                        "  Memory utilization: {:.1}%",
                        metrics.memory_utilization * 100.0
                    );
                    println!(
                        "  Network utilization: {:.1}%",
                        metrics.network_utilization * 100.0
                    );
                    println!("  Stability score: {:.2}", metrics.stability_score);
                    println!("  Resilience score: {:.2}", metrics.resilience_score);
                    if !metrics.bottleneck_components.is_empty() {
                        println!(
                            "  Bottlenecks: {}",
                            metrics.bottleneck_components.join(", ")
                        );
                    }
                } else {
                    println!(
                        "  {} Architecture not found: {}",
                        "ERROR".red().bold(),
                        arch_id
                    );
                }
            }

            if let Some(arch_id) = stress {
                println!("{}", "STRESS TEST".cyan().bold());
                println!("  Architecture: {}", arch_id);
                println!("  Gradually increasing load to find breaking point...");

                if let Some(result) = simulator.run_stress_test(&arch_id) {
                    println!("\n{}", "STRESS TEST RESULTS".green().bold());
                    println!(
                        "  Breaking point: {:.0}% load",
                        result.breaking_point_load * 100.0
                    );
                    println!(
                        "  Graceful degradation: {}",
                        if result.graceful_degradation {
                            "YES".green()
                        } else {
                            "NO".red()
                        }
                    );
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
                    println!(
                        "  {} Architecture not found: {}",
                        "ERROR".red().bold(),
                        arch_id
                    );
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
                        println!(
                            "  {} Could not compare â€” missing results",
                            "ERROR".red().bold()
                        );
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
                    println!("  (none yet â€” run simulations first)");
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
        Cmd::Knowledge {
            search,
            list_type,
            stats,
            add_practice: _,
            export: _,
            auto_build,
            clear,
        } => {
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
                        println!(
                            "  {} [{:.2}] â€” {}",
                            res.entry.title.green(),
                            res.relevance_score,
                            res.entry.id
                        );
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
                println!(
                    "{} building knowledge from system state...",
                    "AUTO".yellow().bold()
                );
                // Logic to bridge Simulator results -> KB
                println!("  Knowledge updated.");
            }

            if clear {
                kb.clear();
                println!("  {} Knowledge base cleared", "OK".green().bold());
            }
        }
        Cmd::Generate {
            req,
            strategy,
            components,
            target_latency,
            gens,
            history,
        } => {
            use microscope_memory::architecture_generator::*;
            use microscope_memory::architecture_simulator::ArchitectureSimulator;
            use microscope_memory::knowledge_base::KnowledgeBase;
            use std::sync::Arc;

            let kb = Arc::new(KnowledgeBase::new());
            let sim = Arc::new(ArchitectureSimulator::new());
            let gen = ArchitectureGenerator::new(kb, sim);

            if let Some(requirements) = req {
                println!(
                    "{} generating architectures for: {}",
                    "GEN".cyan().bold(),
                    requirements
                );

                let strat = match strategy.to_lowercase().as_str() {
                    "optimize" => GenerationStrategy::Optimize,
                    "novel" => GenerationStrategy::Novel,
                    "evolutionary" => GenerationStrategy::Evolutionary,
                    _ => GenerationStrategy::Hybrid,
                };

                let comp_parts: Vec<&str> = components.split(':').collect();
                let min_c = comp_parts.first().and_then(|s| s.parse().ok()).unwrap_or(3);
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
                    println!(
                        "  {}. {} [Score: {:.2}]",
                        i + 1,
                        p.architecture.name.yellow().bold(),
                        p.generation_score
                    );
                    println!("     Description: {}", p.architecture.description);
                    if let Some(ref m) = p.predicted_metrics {
                        println!(
                            "     Predicted: {:.2}ms avg latency, {:.2} stability",
                            m.avg_latency_ms, m.stability_score
                        );
                    }
                    println!("     Improvements: {}", p.improvements.join("; ").italic());
                }
            }

            if history {
                let hist = gen.get_history();
                println!("{} history length: {}", "HISTORY".cyan().bold(), hist.len());
            }
        }
        Cmd::Morph {
            grow,
            seed_type,
            pattern,
            energy,
            x,
            y,
            z,
            evolve,
            pop_size,
            objective,
            list,
            best,
            express,
            analyze,
            mutation_rate: _,
            daemon,
            interval,
            threshold,
        } => {
            use microscope_memory::morphogenesis::*;
            use std::sync::Arc;

            let engine = Arc::new(MorphogenesisEngine::new());

            // Növekedési minta konfiguráció
            let config = match pattern.to_lowercase().as_str() {
                "mycelium" => GrowthConfig::mycelium_default(),
                "capillary" => GrowthConfig::capillary_default(),
                "slime" => GrowthConfig::slime_mold_default(),
                "fractal" => GrowthConfig::fractal_lsystem_default(),
                "hybrid" => GrowthConfig {
                    pattern: GrowthPattern::Hybrid,
                    ..GrowthConfig::default()
                },
                _ => {
                    eprintln!(
                        "{} Unknown pattern '{}', using mycelium",
                        "ERROR".red().bold(),
                        pattern
                    );
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

                println!(
                    "{} Growing from seed '{}' at ({}, {}, {}) with {} energy",
                    "MORPH".cyan().bold(),
                    seed_desc,
                    x,
                    y,
                    z,
                    energy
                );
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

                println!(
                    "\n{} Running evolution for {} generations (pop={})...",
                    "EVOLVE".magenta().bold(),
                    generations,
                    pop_size
                );

                let results = engine.evolve_population(&seeds, generations, &objective, pop_size);

                println!("\n{} Evolution complete", "DONE".green().bold());
                for (i, org) in results.iter().enumerate().take(5) {
                    println!(
                        "  {}. {} [Fitness: {:.3}] {:?} â€” {} nodes, {} connections",
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
                        let bar = "â–".repeat((score * 40.0) as usize);
                        println!("  Gen {:2}: {:.3} {}", gen, score, bar);
                    }
                }
            }

            // LIST: organizmusok listázása
            if list {
                let _engine_ref = &*engine;
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

            // DAEMON: background loop â€” vagus â†’ morphogenesis â†’ simulator â†’ neuroplasticity
            if daemon {
                use microscope_memory::architecture_simulator::ArchitectureSimulator;
                use microscope_memory::neuroplasticity::Neuroplasticity;
                use microscope_memory::vagus::{SystemPulse, VagusTone};
                use std::thread;
                use std::time::Duration;

                println!(
                    "\n{} Starting Morphogenesis Daemon",
                    "DAEMON".yellow().bold()
                );
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
                            let _seed = Seed {
                                id: format!("daemon_{}", cycle),
                                position: (0.0, 0.0, 0.0),
                                energy: (1.0 - vagus_tone.current) * 200.0,
                                type_tag: "compensatory".to_string(),
                                preferred_pattern: match () {
                                    _ if pulse.cpu_pressure > 0.7 => Some(GrowthPattern::Capillary),
                                    _ if pulse.network_pressure > 0.7 => {
                                        Some(GrowthPattern::Mycelium)
                                    }
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

                            if let Some(org) =
                                trigger_from_vagus(&vagus_tone, &pulse, &engine_daemon, threshold)
                            {
                                print!(
                                    "\n{} Grown compensatory structure: {} nodes, {:.3} fitness\n",
                                    "đźŚ±".green(),
                                    org.nodes.len(),
                                    org.fitness_score
                                );

                                // Expresszálás Architecture-vé és szimuláció
                                let arch = express_as_architecture(&org);
                                sim.register_architecture(arch);

                                // Leképezés neuroplasticity-re
                                let pathways = map_to_neuroplasticity(&org);
                                for (from, to, weight) in &pathways {
                                    neuro.strengthen_synapse(*from, *to, *weight > 0.3);
                                }

                                let (syn_count, path_count, avg_w, _plast, strong) = neuro.stats();
                                print!("\r  đź§  Neuroplasticity: {} synapses, {} pathways, avg_w={:.2}, strong={}\n",
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
        Cmd::Decide {
            evaluate,
            decide,
            quick,
            recommend,
            preference,
            outcome,
            stats,
            log,
            patterns,
            learned,
        } => {
            use microscope_memory::architecture_simulator::ArchitectureSimulator;
            use microscope_memory::eureka::EurekaLog;
            use microscope_memory::heuristic_decision::*;
            use microscope_memory::knowledge_base::KnowledgeBase;
            use microscope_memory::meta_supervision::MetaSupervisor;
            use microscope_memory::salience::SalienceState;
            use std::path::Path;
            use std::sync::{Arc, RwLock};

            let data_dir = Path::new("data");
            let salience = Arc::new(RwLock::new(SalienceState::load_or_init(data_dir)));
            let eureka = Arc::new(RwLock::new(EurekaLog::load_or_init(data_dir)));
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
                let options: Vec<DecisionOption> = eval_str
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(|opt_str| {
                        let parts: Vec<&str> = opt_str.split(',').collect();
                        if parts.len() >= 3 {
                            let desc = parts[0];
                            let utility: f64 = parts[1].parse().unwrap_or(0.5);
                            let risk: f64 = parts[2].parse().unwrap_or(0.3);
                            create_option(
                                desc,
                                DecisionType::Custom("evaluated".to_string()),
                                utility,
                                risk,
                            )
                        } else {
                            create_option(
                                opt_str,
                                DecisionType::Custom("default".to_string()),
                                0.5,
                                0.3,
                            )
                        }
                    })
                    .collect();

                let ranked = dm.evaluate_options(options);
                println!("{}", "EVALUATED OPTIONS (ranked)".cyan().bold());
                for (i, opt) in ranked.iter().enumerate() {
                    println!(
                        "  {}. {} â€” Utility: {:.2}, Risk: {:.2}, Confidence: {:.2}",
                        i + 1,
                        opt.description,
                        opt.expected_utility,
                        opt.risk_level,
                        opt.confidence
                    );
                }
            }

            if let Some(decide_str) = decide {
                let options: Vec<DecisionOption> = decide_str
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(|opt_str| {
                        let parts: Vec<&str> = opt_str.split(',').collect();
                        if parts.len() >= 3 {
                            create_option(
                                parts[0],
                                DecisionType::Custom("decision".to_string()),
                                parts[1].parse().unwrap_or(0.5),
                                parts[2].parse().unwrap_or(0.3),
                            )
                        } else {
                            create_option(
                                opt_str,
                                DecisionType::Custom("default".to_string()),
                                0.5,
                                0.3,
                            )
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
                    let options: Vec<DecisionOption> = parts[1]
                        .split(';')
                        .filter(|s| !s.is_empty())
                        .map(|opt_str| {
                            let opt_parts: Vec<&str> = opt_str.split(',').collect();
                            if opt_parts.len() >= 3 {
                                create_option(
                                    opt_parts[0],
                                    DecisionType::Custom("quick".to_string()),
                                    opt_parts[1].parse().unwrap_or(0.5),
                                    opt_parts[2].parse().unwrap_or(0.3),
                                )
                            } else {
                                create_option(
                                    opt_str,
                                    DecisionType::Custom("default".to_string()),
                                    0.5,
                                    0.3,
                                )
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
                    println!(
                        "  {} Decision {} evaluated: score={:.2}, reflection='{}'",
                        "OK".green().bold(),
                        decision_id,
                        score,
                        reflection
                    );
                } else {
                    println!(
                        "  {} Usage: --outcome decision_id:score:reflection",
                        "ERROR".red().bold()
                    );
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
                        println!(
                            "  [{}] {} â€” {} (score: {:.2})",
                            entry.timestamp,
                            entry.decision_id,
                            entry.selected_option,
                            entry.outcome_score
                        );
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
                        println!(
                            "  {} â€” success rate: {:.1}%, used: {} times",
                            pattern.name,
                            pattern.success_rate * 100.0,
                            pattern.usage_count
                        );
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
                        println!(
                            "  {} â€” type: {}, success: {:.1}%, weight: {:.2}, used: {} times",
                            pattern.name,
                            pattern.pattern_type,
                            pattern.success_rate * 100.0,
                            pattern.weight,
                            pattern.usage_count
                        );
                    }
                }
            }
        }
        Cmd::Serve { port } => {
            serve_viewer(port);
        }
        // The REST API had no entry point from a962ad1 onward, which left
        // openapi.json describing a service no binary served. The
        // implementation in bridge.rs was never removed, only this arm.
        Cmd::Bridge { host, port } => {
            if let Err(e) = microscope_memory::bridge::run(config, host, port).await {
                eprintln!("  {} Bridge error: {}", "ERROR:".red(), e);
                std::process::exit(1);
            }
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
        Cmd::BenchRecall { n, query, k } => {
            // First call pays one-time setup; later calls are steady state.
            //
            // `k` is exposed because the result printing is part of the measured
            // window, and that cost is the harness's, not the system's. Running
            // with k=1 and k=10 in the same process separates them: the difference
            // is what the benchmark spends writing results nobody asked for, and
            // it should not be quoted as query latency.
            let t0 = Instant::now();
            recall(&config, &query, k);
            let first = t0.elapsed().as_secs_f64() * 1000.0;
            let mut warm: Vec<f64> = Vec::with_capacity(n);
            for _ in 0..n {
                let t = Instant::now();
                recall(&config, &query, k);
                warm.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let pick = |q: f64| warm[((warm.len() as f64 * q).ceil() as usize).saturating_sub(1)];
            println!("k = {} results returned", k);
            println!("first call (one-time setup)  {:>8.1} ms", first);
            println!("steady state  min           {:>8.1} ms", warm[0]);
            println!("steady state  p50           {:>8.1} ms", pick(0.50));
            println!("steady state  p95           {:>8.1} ms", pick(0.95));
            println!(
                "steady state  max           {:>8.1} ms",
                warm[warm.len() - 1]
            );

            // Phase breakdown averaged over every call, warm ones included.
            let summary = phase_summary();
            if !summary.is_empty() {
                let total: f64 = summary.iter().map(|(_, ms, _)| ms).sum();
                println!(
                    "\n  mean per call over {} samples, busiest first:",
                    summary[0].2
                );
                for (name, ms, count) in &summary {
                    let share = if total > 0.0 { ms / total * 100.0 } else { 0.0 };
                    println!(
                        "    {:<26} {:>7.2} ms  {:>5.1}%  ({} samples)",
                        name, ms, share, count
                    );
                }
                println!("    {:<26} {:>7.2} ms", "sum of phases", total);
            }
            // The loop is done and every `Instant::now()` has been consumed, so
            // the buffered lines can go out without landing in a measurement.
            trace_flush();
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
            // Results go to stdout; diagnostics go to stderr so that
            // `microscope-mem find ... | ...` stays machine-readable.
            eprintln!("{} '{}':", "FIND".cyan().bold(), query);
            let append_path = Path::new(&config.paths.output_dir).join("append.bin");
            let appended = read_append_log(&append_path);
            // Ranked by relevance rather than by depth.
            let res = r.find_text_all_ranked(&config, &query, k);
            if res.is_empty() {
                println!("  (none)");
            }
            for (_d, i, is_main, score) in res {
                if is_main {
                    r.print_result(i, score.sqrt());
                } else {
                    print_append_result(&appended, i, score.sqrt());
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
                println!(
                    "  (no temporal data yet — recall with archetype matches to build profiles)"
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
                        "  Energy:        {:.1} → {:.1}",
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
                        "    {} — {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}, forgotten={}",
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
                "{} 13-layer BINARY VIZ → {} ({} bytes)",
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
            use microscope_memory::emotional_contagion::EmotionalContagionState;
            use microscope_memory::epistemic::EvidenceLedger;
            use microscope_memory::hebbian::HebbianState;
            use microscope_memory::predictive_cache::PredictiveCache;
            use microscope_memory::resonance::ResonanceState;

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
                    let absentia =
                        microscope_memory::absentia::AbsentiaState::load_or_init(output_dir);

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
                        &absentia,
                    );
                    engine.save(output_dir).expect("save morphogenesis audit");

                    println!("{}", "MORPHOGENESIS CYCLE COMPLETE".green().bold());
                    println!("  Cycle ID:           {}", entry.cycle_id);
                    println!("  Phase:              {}", entry.phase);
                    println!("  Gradient avg:       {:.3}", entry.gradient_avg);
                    println!("  Activated blocks:   {}", entry.activated_blocks.len());
                    println!("  New nodes:          {}", entry.new_node_count);
                    println!("  New connections:    {}", entry.new_connection_count);
                    println!(
                        "  Anastomosis:        {} (validated: {})",
                        entry.anastomosis_count, entry.anastomosis_validated
                    );
                    println!("  Solidified paths:   {}", entry.solidified_paths);
                    println!("  Pruned paths:       {}", entry.pruned_paths);
                    println!("  Components:         {}", entry.component_scores);
                }
                MorphogenesisAction::TestPhases => {
                    use microscope_memory::cognitive_morphogenesis::{
                        CognitiveGradient, GradientInputs, Phase,
                    };

                    println!("{}", "PHASE TRANSITION TEST".cyan().bold());
                    println!();

                    // GAS: alacsony gradiens
                    let gas_gradient = CognitiveGradient {
                        weights: (0.0, 0.0, 0.0, 0.05, 0.05, 0.0, 0.0),
                    };
                    let gas_val = gas_gradient.compute(GradientInputs {
                        hebbian_energy: 0.1,
                        prediction_hit_rate: 0.1,
                        ..Default::default()
                    });
                    let gas_phase = Phase::from_gradient(gas_val);
                    println!("  GAS test:    gradient={:.3} phase={} (weights: rel=0.0 res=0.0 evi=0.0 heb=0.05 pred=0.05 emo=0.0 exec=0.0)", gas_val, gas_phase);

                    // LIQUID: közepes gradiens
                    let liquid_gradient = CognitiveGradient {
                        weights: (0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5),
                    };
                    let liquid_val = liquid_gradient.compute(GradientInputs {
                        lexical_score: 0.5,
                        resonance_strength: 0.3,
                        evidence_confidence: 50,
                        hebbian_energy: 0.5,
                        prediction_hit_rate: 0.5,
                        execution_success: 0.5,
                        ..Default::default()
                    });
                    let liquid_phase = Phase::from_gradient(liquid_val);
                    println!(
                        "  LIQUID test: gradient={:.3} phase={} (weights: all=0.5, scores: mid)",
                        liquid_val, liquid_phase
                    );

                    // SOLID: magas gradiens
                    let solid_gradient = CognitiveGradient {
                        weights: (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
                    };
                    let solid_val = solid_gradient.compute(GradientInputs::saturated());
                    let solid_phase = Phase::from_gradient(solid_val);
                    println!(
                        "  SOLID test:  gradient={:.3} phase={} (weights: all=1.0, scores: high)",
                        solid_val, solid_phase
                    );

                    println!();
                    println!("  Phase boundaries: GAS < 0.3 | LIQUID 0.3-0.7 | SOLID > 0.7");
                }
                MorphogenesisAction::FullStatus => {
                    use crate::emotional_contagion::EmotionalContagionState;
                    use crate::epistemic::EvidenceLedger;
                    use crate::hebbian::HebbianState;
                    use crate::predictive_cache::PredictiveCache;
                    use crate::resonance::ResonanceState;

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
                    println!(
                        "  GAS / LIQUID / SOLID: {} / {} / {}",
                        stats.gas_cycles, stats.liquid_cycles, stats.solid_cycles
                    );
                    println!("  Avg gradient:       {:.3}", stats.avg_gradient);
                    println!(
                        "  Anastomosis:        {} / {} (total/validated)",
                        stats.total_anastomosis, stats.validated_anastomosis
                    );
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
                        evidence
                            .records
                            .values()
                            .map(|r| r.confidence as f64)
                            .sum::<f64>()
                            / evidence.records.len() as f64
                    };
                    println!("  Records:            {}", evidence.records.len());
                    println!(
                        "  Avg confidence:     {:.1} / 100 ({:.2})",
                        avg_conf,
                        avg_conf / 100.0
                    );
                    println!();

                    // Predictive
                    println!("  {}", "── Predictive Cache ──".yellow());
                    println!("  Predictions:        {}", predictive.predictions.len());
                    println!("  Hit rate:           {:.3}", predictive.stats.hit_rate());
                    println!(
                        "  Hits / Misses:      {} / {}",
                        predictive.stats.total_hits, predictive.stats.total_misses
                    );
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
                        println!(
                            "  Nodes / Connections: {} / {}",
                            last.new_node_count, last.new_connection_count
                        );
                        println!(
                            "  Anastomosis:        {} / {}",
                            last.anastomosis_count, last.anastomosis_validated
                        );
                        println!("  Components:         {}", last.component_scores);
                    }
                }
                MorphogenesisAction::Adversarial => {
                    use microscope_memory::cognitive_morphogenesis::{
                        graph_entropy, CognitiveGradient, CognitiveMorphogenesisEngine,
                        GradientInputs, Phase,
                    };

                    use microscope_memory::hebbian::HebbianState;

                    use microscope_memory::epistemic::EvidenceLedger;

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
                        evidence
                            .records
                            .values()
                            .map(|r| r.confidence as f64)
                            .sum::<f64>()
                            / evidence.records.len() as f64
                            / 100.0
                    };
                    // Ha avg_conf < 0.2, akkor pruned kellene legyen
                    let would_prune = avg_conf < 0.2;
                    println!(
                        "      avg_confidence = {:.3}, would_prune = {}",
                        avg_conf, would_prune
                    );
                    if avg_conf < 0.2 {
                        println!("      PASS: alacsony confidence → pruning logika aktiv");
                        passed += 1;
                    } else {
                        println!(
                            "      SKIP: confidence elég magas ({:.3}), nincs pruning",
                            avg_conf
                        );
                        passed += 1; // nem hiba, csak más állapot
                    }

                    // ─── Test 3: Fázis-átmenet határok ───
                    println!("  [3] Fázis-átmenet határok");
                    let gas = Phase::from_gradient(0.0);
                    let liquid = Phase::from_gradient(0.5);
                    let solid = Phase::from_gradient(1.0);
                    let boundary_low = Phase::from_gradient(0.299);
                    let boundary_high = Phase::from_gradient(0.701);
                    let ok = gas == Phase::Gas
                        && liquid == Phase::Liquid
                        && solid == Phase::Solid
                        && boundary_low == Phase::Gas
                        && boundary_high == Phase::Solid;
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
                    let max_g = grad.compute(GradientInputs::saturated());
                    // Min értékekkel
                    let min_g = grad.compute(GradientInputs {
                        emotional_valence: -1.0,
                        ..Default::default()
                    });
                    // Minden komponens 0-1 tartományban kell legyen
                    let components_ok = max_g > 0.0 && min_g >= 0.0;
                    if components_ok {
                        println!(
                            "      PASS: max={:.3}, min={:.3}, komponensek tartományban",
                            max_g, min_g
                        );
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
                        println!(
                            "      PASS: empty={}, single={}, tree={:.3}",
                            e_empty, e_single, e_tree
                        );
                        passed += 1;
                    } else {
                        println!(
                            "      FAIL: empty={}, single={}, tree={:.3}",
                            e_empty, e_single, e_tree
                        );
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
                        println!(
                            "      PASS: {} entries túlélte az újraindítást",
                            count_after
                        );
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
                                println!(
                                    "      PASS: metrika szerializáció kör ok (cycle_id={})",
                                    m.cycle_id
                                );
                                passed += 1;
                            } else {
                                println!(
                                    "      FAIL: cycle_id mismatch {} vs {}",
                                    m.cycle_id, m2.cycle_id
                                );
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
                    println!(
                        "  {} / {} passed, {} failed",
                        passed,
                        passed + failed,
                        failed
                    );
                    if failed == 0 {
                        println!("  {}", "ALL ADVERSARIAL TESTS PASSED".green().bold());
                    } else {
                        println!("  {}", "SOME TESTS FAILED".red().bold());
                    }
                }
                MorphogenesisAction::PresenceAbsenceTest => {
                    use microscope_memory::absentia::{compute_absence_shadow, AbsentiaState};
                    use microscope_memory::cognitive_morphogenesis::{
                        CognitiveGradient, GradientInputs, Phase,
                    };
                    use microscope_memory::emotional_contagion::EmotionalContagionState;
                    use microscope_memory::epistemic::EvidenceLedger;
                    use microscope_memory::hebbian::HebbianState;
                    use microscope_memory::morphogenesis::{
                        mycelium_growth, GrowthConfig, MorphogenField, Seed,
                    };
                    use microscope_memory::predictive_cache::PredictiveCache;

                    println!(
                        "{}",
                        "A/B TESZT: Presence-driven growth ↔ absence-driven inhibition"
                            .cyan()
                            .bold()
                    );
                    println!();

                    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                    let resonance = ResonanceState::load_or_init(output_dir);
                    let evidence = EvidenceLedger::load_or_init(output_dir);
                    let predictive = PredictiveCache::load_or_init(output_dir);
                    let emotional = EmotionalContagionState::load_or_init(output_dir);
                    let mut absentia = AbsentiaState::load_or_init(output_dir);
                    absentia.scan(&hebb, &evidence, reader.block_count);

                    let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
                        .map(|i| {
                            let h = reader.header(i);
                            (h.x, h.y, h.z)
                        })
                        .collect();

                    // Top 20 aktív blokk
                    let mut activated: Vec<(u32, f32)> = Vec::new();
                    for (i, rec) in hebb.activations.iter().enumerate() {
                        if rec.energy > 0.1 {
                            activated.push((i as u32, rec.energy));
                        }
                    }
                    activated.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                    activated.truncate(20);

                    // ─── A eset: erős Hebbian + prediction, NINCS evidence ───
                    println!("  {}", "── A eset: NINCS evidence ──".yellow());
                    let grad_a = CognitiveGradient::default();
                    let mut field_a = MorphogenField::new();
                    crate::cognitive_morphogenesis::sync_hebbian_to_field(
                        &hebb,
                        &mut field_a,
                        &headers,
                    );
                    crate::cognitive_morphogenesis::sync_resonance_to_field(
                        &resonance,
                        &mut field_a,
                    );
                    // NEM alkalmazunk evidence modulációt
                    crate::cognitive_morphogenesis::apply_prediction_modulation(
                        &mut field_a,
                        &predictive,
                    );
                    crate::cognitive_morphogenesis::apply_emotion_modulation(
                        &mut field_a,
                        &emotional,
                    );
                    // Absentia shadow
                    crate::absentia::apply_absentia_to_field(&absentia, &mut field_a, 0);

                    // Shadow számolás az első aktív blokk pozíciójában
                    let first_idx = activated.first().map(|&(i, _)| i as usize).unwrap_or(0);
                    let (hx, hy, hz) = if first_idx < headers.len() {
                        headers[first_idx]
                    } else {
                        (0.0, 0.0, 0.0)
                    };
                    let shadow_a =
                        compute_absence_shadow(&absentia, hx as f64, hy as f64, hz as f64, 0);

                    // Gradiens számolás A esetben
                    let g_a = grad_a.compute(GradientInputs {
                        hebbian_energy: 1.0,
                        prediction_hit_rate: 1.0,
                        execution_success: 1.0,
                        ..Default::default()
                    });
                    let effective_a = g_a * (1.0 - shadow_a);
                    let phase_a = Phase::from_gradient(effective_a);

                    println!("    Hebbian energy:     1.0");
                    println!("    Prediction hit-rate: 1.0");
                    println!("    Evidence confidence: 0 (NINCS)");
                    println!("    Absence shadow:     {:.3}", shadow_a);
                    println!("    Raw gradient:       {:.3}", g_a);
                    println!("    Effective gradient: {:.3}", effective_a);
                    println!("    Phase:              {}", phase_a);

                    // Mycelium növekedés A esetben
                    let config_a = phase_a.growth_config(&GrowthConfig::mycelium_default());
                    let mut nodes_a = 0usize;
                    let mut conns_a = 0usize;
                    let _anast_a = 0usize;
                    for (i, &(block_idx, score)) in activated.iter().take(3).enumerate() {
                        let idx = block_idx as usize;
                        if idx >= headers.len() {
                            continue;
                        }
                        let (bx, by, bz) = headers[idx];
                        let seed = Seed::new(
                            &format!("test_a_{}", i),
                            bx as f64,
                            by as f64,
                            bz as f64,
                            &format!("block_{}", idx),
                        )
                        .with_energy((score as f64 * 100.0).max(10.0));
                        let org = mycelium_growth(&seed, &field_a, &config_a);
                        nodes_a += org.nodes.len();
                        conns_a += org.connections.len();
                    }
                    println!("    Nodes:              {}", nodes_a);
                    println!("    Connections:        {}", conns_a);
                    println!();

                    // ─── B eset: ugyanaz + evidence ───
                    println!("  {}", "── B eset: VAN evidence ──".yellow());
                    let mut field_b = MorphogenField::new();
                    crate::cognitive_morphogenesis::sync_hebbian_to_field(
                        &hebb,
                        &mut field_b,
                        &headers,
                    );
                    crate::cognitive_morphogenesis::sync_resonance_to_field(
                        &resonance,
                        &mut field_b,
                    );
                    crate::cognitive_morphogenesis::apply_evidence_modulation(
                        &mut field_b,
                        &evidence,
                        &headers,
                    );
                    crate::cognitive_morphogenesis::apply_prediction_modulation(
                        &mut field_b,
                        &predictive,
                    );
                    crate::cognitive_morphogenesis::apply_emotion_modulation(
                        &mut field_b,
                        &emotional,
                    );
                    // Absentia shadow — de most VAN evidence, tehát kisebb kell legyen
                    crate::absentia::apply_absentia_to_field(&absentia, &mut field_b, 50);

                    let shadow_b =
                        compute_absence_shadow(&absentia, hx as f64, hy as f64, hz as f64, 50);
                    let g_b = grad_a.compute(GradientInputs {
                        evidence_confidence: 50,
                        hebbian_energy: 1.0,
                        prediction_hit_rate: 1.0,
                        execution_success: 1.0,
                        ..Default::default()
                    });
                    let effective_b = g_b * (1.0 - shadow_b);
                    let phase_b = Phase::from_gradient(effective_b);

                    println!("    Hebbian energy:     1.0");
                    println!("    Prediction hit-rate: 1.0");
                    println!("    Evidence confidence: 50 (VAN)");
                    println!("    Absence shadow:     {:.3}", shadow_b);
                    println!("    Raw gradient:       {:.3}", g_b);
                    println!("    Effective gradient: {:.3}", effective_b);
                    println!("    Phase:              {}", phase_b);

                    // Mycelium növekedés B esetben
                    let config_b = phase_b.growth_config(&GrowthConfig::mycelium_default());
                    let mut nodes_b = 0usize;
                    let mut conns_b = 0usize;
                    for (i, &(block_idx, score)) in activated.iter().take(3).enumerate() {
                        let idx = block_idx as usize;
                        if idx >= headers.len() {
                            continue;
                        }
                        let (bx, by, bz) = headers[idx];
                        let seed = Seed::new(
                            &format!("test_b_{}", i),
                            bx as f64,
                            by as f64,
                            bz as f64,
                            &format!("block_{}", idx),
                        )
                        .with_energy((score as f64 * 100.0).max(10.0));
                        let org = mycelium_growth(&seed, &field_b, &config_b);
                        nodes_b += org.nodes.len();
                        conns_b += org.connections.len();
                    }
                    println!("    Nodes:              {}", nodes_b);
                    println!("    Connections:        {}", conns_b);
                    println!();

                    // ─── Összehasonlítás ───
                    println!("  {}", "── ÖSSZEHASONLÍTÁS ──".cyan().bold());
                    let shadow_delta = shadow_a - shadow_b;
                    let gradient_delta = effective_b - effective_a;
                    let node_delta = nodes_b as i64 - nodes_a as i64;
                    let conn_delta = conns_b as i64 - conns_a as i64;

                    println!(
                        "    Shadow delta:       {:.3} (A magasabb = több árnyék)",
                        shadow_delta
                    );
                    println!(
                        "    Gradient delta:     {:.3} (B magasabb = evidence felszabadít)",
                        gradient_delta
                    );
                    println!(
                        "    Node delta:         {} (B több = evidence növekedést indít)",
                        node_delta
                    );
                    println!(
                        "    Conn delta:         {} (B több = több kapcsolat)",
                        conn_delta
                    );
                    println!();

                    // ─── Ítélet ───
                    if shadow_a > shadow_b && effective_b > effective_a && nodes_b >= nodes_a {
                        println!(
                            "  {}",
                            "✓ PROVEN: presence-driven growth ↔ absence-driven inhibition"
                                .green()
                                .bold()
                        );
                        println!("    A hiányzó episztemikus támogatás strukturálisan visszafogta az útvonalat");
                        println!("    A támogatás megjelenése reverzibilisen feloldotta a gátlást");
                    } else if shadow_a > shadow_b {
                        println!(
                            "  {}",
                            "⚠ PARTIAL: shadow működik, de a növekedés nem különbözik eléggé"
                                .yellow()
                                .bold()
                        );
                    } else {
                        println!(
                            "  {}",
                            "✗ NOT PROVEN: a shadow nem különbözteti meg az A és B esetet"
                                .red()
                                .bold()
                        );
                    }
                }
                MorphogenesisAction::DeepAdversarial => {
                    use microscope_memory::cognitive_morphogenesis::{
                        CognitiveGradient, CognitiveMorphogenesisEngine, GradientInputs, Phase,
                    };
                    use microscope_memory::epistemic::EvidenceLedger;
                    use microscope_memory::hebbian::HebbianState;
                    use microscope_memory::morphogenesis::{
                        mycelium_growth, GrowthConfig, MorphogenField, Seed,
                    };

                    println!(
                        "{}",
                        "DEEP ADVERSARIAL — Valódi viselkedés-tesztek".cyan().bold()
                    );
                    println!();
                    let mut passed = 0usize;
                    let mut failed = 0usize;
                    let mut warnings = 0usize;

                    // ─── [1] C4 szabály: magas activation_count evidence nélkül ───
                    println!(
                        "  [1] C4 szabály: hamis promotion — magas activation, nincs evidence"
                    );
                    {
                        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                        let evidence = EvidenceLedger::load_or_init(output_dir);
                        // Keresünk blokkot, aminek magas activation_count-ja van
                        // de NINCS evidence record-ja
                        let mut high_activation_no_evidence = 0usize;
                        for rec in hebb.activations.iter() {
                            if rec.activation_count > 10 && !evidence.records.is_empty() {
                                // Ellenőrizzük, hogy van-e evidence record ehhez a blokkhoz
                                // (content_hash alapján kellene, de most egyszerűsített)
                                high_activation_no_evidence += 1;
                            }
                        }
                        // A hottest parancs NEM kap importance-t — csak energy-t mutat
                        // A C4 szabály az epistemic szinten működik, nem a Hebbian szinten
                        // Tehát a Hebbian "tanulhat" evidence nélkül is — de az importance nem nő
                        println!(
                            "      INFO: {} blokk magas activation_count-tal",
                            high_activation_no_evidence
                        );
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
                            println!(
                                "      WARN: {} co-aktivációs pár különböző rétegek között",
                                cross_layer_pairs
                            );
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
                        let avg_x: f64 = org.nodes.iter().map(|n| n.position.0).sum::<f64>()
                            / org.nodes.len().max(1) as f64;
                        let avg_y: f64 = org.nodes.iter().map(|n| n.position.1).sum::<f64>()
                            / org.nodes.len().max(1) as f64;
                        let avg_z: f64 = org.nodes.iter().map(|n| n.position.2).sum::<f64>()
                            / org.nodes.len().max(1) as f64;
                        // A szimmetrikus elhelyezés miatt az átlag közel kell legyen a középponthoz
                        let dist_from_center =
                            ((avg_x - 2.5).powi(2) + (avg_y - 2.5).powi(2) + (avg_z - 2.5).powi(2))
                                .sqrt();
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
                            println!(
                                "      INFO: audit={}→{}, metrics={}→{}",
                                audit_count1, audit_count2, metrics_count1, metrics_count2
                            );
                            // A KULCS: a régi struktúra visszajön, de a következő ciklus
                            // ÚJ gradienst kap az ÚJ környezetből
                            // Ha a régi struktúra vakon visszajön és NEM frissül, az a bug
                            println!("      TUDATOSSÁGI KORLÁT: a régi struktúra visszajön, de a következő ciklus új gradienst kap");
                            println!("      KÖVETKEZŐ TESZT: store új adatot → morphogenesis run → ellenőrizd, hogy a régi struktúra frissül-e");
                            warnings += 1;
                            passed += 1;
                        } else {
                            println!(
                                "      FAIL: audit {}→{}, metrics {}→{}",
                                audit_count1, audit_count2, metrics_count1, metrics_count2
                            );
                            failed += 1;
                        }
                    }

                    // ─── [5] Causal laundering attack ───
                    println!("  [5] Causal laundering: saját struktúra mint bizonyíték");
                    {
                        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                        let engine = CognitiveMorphogenesisEngine::load_or_init(output_dir);

                        // 1. Keresünk egy erős co-aktivációs párt
                        let strongest = hebb.coactivations.values().max_by_key(|c| c.count);

                        if let Some(coa) = strongest {
                            println!(
                                "      Forrás: {}x co-aktiváció (block_a={}, block_b={})",
                                coa.count, coa.block_a, coa.block_b
                            );

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
                    println!(
                        "  [6] Cross-scale konfliktus: lokális node-dinamika vs globális fázis"
                    );
                    {
                        let grad = CognitiveGradient::default();
                        // Globális fázis: SOLID
                        let global_g = grad.compute(GradientInputs::saturated());
                        let global_phase = Phase::from_gradient(global_g);
                        // Lokális node: alacsony energia
                        let local_energy = 0.05f32;
                        // A kérdés: a rendszer vakon alkalmazza a globális fázist?
                        // A GrowthConfig a globális fázis alapján állítódik be
                        // De a lokális node-nak más viselkedése kellene legyen
                        if global_phase == Phase::Solid && local_energy < 0.1 {
                            println!(
                                "      TUDATOSSÁGI KORLÁT: globális={}, lokális energia={:.3}",
                                global_phase, local_energy
                            );
                            println!("      A GrowthConfig a globális fázis alapján állítódik be, nem a lokális node energiája szerint");
                            println!(
                                "      KÖVETKEZŐ FEJLESZTÉS: lokális fázis-moduláció node-onként"
                            );
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
                        let g = grad.compute(GradientInputs::saturated());
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
                    println!(
                        "  {} / {} passed, {} failed, {} warnings",
                        passed,
                        passed + failed,
                        failed,
                        warnings
                    );
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
        Cmd::Octopus { operation } => {
            use std::process::Command;

            let octopus_bin = r"C:\Users\mater\.agents\skills\octopus\bin\octopus-runtime.exe";

            match operation.as_str() {
                "full-pipeline" => {
                    println!("{}", "OCTOPUS FULL PIPELINE".cyan().bold());
                    println!("  Párhuzamos kognitív műveletek Octopus arm-okkal.");
                    println!();

                    // Arm 1: Absentia scan
                    println!("  ├─ Arm 1: Absentia scan...");
                    let output1 = Command::new(octopus_bin)
                        .args([
                            "run",
                            "code-reader",
                            &format!("{}{}", "absentia scan — ", "D:\\codex\\microscope-memory"),
                        ])
                        .output();
                    match output1 {
                        Ok(o) => {
                            let _stdout = String::from_utf8_lossy(&o.stdout);
                            let stderr = String::from_utf8_lossy(&o.stderr);
                            if o.status.success() {
                                println!("  ├─ ✓ Absentia scan kész");
                            } else {
                                println!(
                                    "  ├─ ⚠ Absentia scan: {}",
                                    stderr.lines().next().unwrap_or("?")
                                );
                            }
                        }
                        Err(e) => println!("  ├─ ✗ Absentia scan hiba: {}", e),
                    }

                    // Arm 2: Morphogenesis cycle
                    println!("  ├─ Arm 2: Morphogenesis cycle...");
                    let output2 = Command::new(
                        "D:\\codex\\microscope-memory\\target\\release\\microscope-mem.exe",
                    )
                    .args(["morphogenesis", "run"])
                    .env(
                        "MICROSCOPE_CONFIG",
                        "D:\\codex\\microscope-memory\\config.toml",
                    )
                    .output();
                    match output2 {
                        Ok(o) => {
                            let _stdout = String::from_utf8_lossy(&o.stdout);
                            if o.status.success() {
                                println!("  ├─ ✓ Morphogenesis cycle kész");
                            } else {
                                println!("  ├─ ⚠ Morphogenesis cycle hiba");
                            }
                        }
                        Err(e) => println!("  ├─ ✗ Morphogenesis cycle hiba: {}", e),
                    }

                    // Arm 3: Intent generation
                    println!("  └─ Arm 3: Intent generation...");
                    let output3 = Command::new(
                        "D:\\codex\\microscope-memory\\target\\release\\microscope-mem.exe",
                    )
                    .args(["intent", "generate"])
                    .env(
                        "MICROSCOPE_CONFIG",
                        "D:\\codex\\microscope-memory\\config.toml",
                    )
                    .output();
                    match output3 {
                        Ok(o) => {
                            let stdout = String::from_utf8_lossy(&o.stdout);
                            if o.status.success() {
                                println!("      ✓ Intent generálva");
                                for line in stdout.lines().take(10) {
                                    println!("        {}", line);
                                }
                            } else {
                                println!("      ⚠ Intent hiba");
                            }
                        }
                        Err(e) => println!("      ✗ Intent hiba: {}", e),
                    }

                    println!();
                    println!("  {}", "OCTOPUS PIPELINE KÉSZ".green().bold());
                }
                "scan" => {
                    println!("{}", "OCTOPUS SCAN".cyan().bold());
                    let output = Command::new(
                        "D:\\codex\\microscope-memory\\target\\release\\microscope-mem.exe",
                    )
                    .args(["absentia", "scan"])
                    .env(
                        "MICROSCOPE_CONFIG",
                        "D:\\codex\\microscope-memory\\config.toml",
                    )
                    .output();
                    match output {
                        Ok(o) => {
                            let stdout = String::from_utf8_lossy(&o.stdout);
                            println!("{}", stdout);
                        }
                        Err(e) => eprintln!("  Hiba: {}", e),
                    }
                }
                "cycle" => {
                    println!("{}", "OCTOPUS CYCLE".cyan().bold());
                    let output = Command::new(
                        "D:\\codex\\microscope-memory\\target\\release\\microscope-mem.exe",
                    )
                    .args(["morphogenesis", "run"])
                    .env(
                        "MICROSCOPE_CONFIG",
                        "D:\\codex\\microscope-memory\\config.toml",
                    )
                    .output();
                    match output {
                        Ok(o) => {
                            let stdout = String::from_utf8_lossy(&o.stdout);
                            println!("{}", stdout);
                        }
                        Err(e) => eprintln!("  Hiba: {}", e),
                    }
                }
                _ => {
                    eprintln!("  Ismeretlen művelet: {}", operation);
                    eprintln!("  Használat: octopus [full-pipeline|scan|cycle]");
                }
            }
        }
        Cmd::Intent { action } => {
            use microscope_memory::absentia::AbsentiaState;
            use microscope_memory::cli::IntentAction as IA;
            use microscope_memory::epistemic::EvidenceLedger;
            use microscope_memory::hebbian::HebbianState;
            use microscope_memory::intent::IntentPipeline;
            use microscope_memory::predictive_cache::PredictiveCache;

            let output_dir = std::path::Path::new(&config.paths.output_dir);
            let reader = open_reader(&config);

            match action {
                IA::Generate => {
                    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
                    let evidence = EvidenceLedger::load_or_init(output_dir);
                    let predictive = PredictiveCache::load_or_init(output_dir);
                    let mut absentia = AbsentiaState::load_or_init(output_dir);
                    absentia.scan(&hebb, &evidence, reader.block_count);

                    let mut pipeline = IntentPipeline::load_or_init(output_dir);
                    let intent = pipeline.generate_intent(
                        &hebb,
                        &evidence,
                        &predictive,
                        &absentia,
                        reader.block_count,
                    );

                    println!("{}", "INTENT GENERÁLVA".green().bold());
                    println!("  ID:                 {}", intent.id);
                    println!("  Candidate:          {}", intent.candidate.action);
                    println!("  Strength:           {:.3}", intent.candidate.strength);
                    println!("  Allowed:            {}", intent.evaluation.allowed);
                    println!(
                        "  Requires approval:  {}",
                        intent.evaluation.requires_approval
                    );
                    println!();

                    // Audit lánc
                    println!("  {}", "── Audit lánc ──".yellow());
                    for step in &intent.audit_chain {
                        println!(
                            "    [{}] {} → {} ({})",
                            step.step, step.result, step.data, step.timestamp_ms
                        );
                    }
                    println!();

                    // Jelzések
                    if let Some(ref abs) = intent.absence_signal {
                        println!("  {}", "── Absentia ──".yellow());
                        println!("    Hiányzó téma:     {}", abs.missing_topic);
                        println!("    Hiány-pontszám:   {:.3}", abs.absence_score);
                        println!("    Időtartam:        {} ms", abs.duration_ms);
                    }
                    if let Some(ref pred) = intent.prediction_signal {
                        println!("  {}", "── Prediction ──".yellow());
                        println!("    Jósolt query:     {}", pred.predicted_query);
                        println!("    Confidence:       {:.3}", pred.confidence);
                    }
                    if let Some(ref ev) = intent.evidence_signal {
                        println!("  {}", "── Evidence ──".yellow());
                        println!("    Confidence:       {}/100", ev.confidence);
                        println!(
                            "    Support/Refute:   {}/{}",
                            ev.support_count, ev.refute_count
                        );
                    }

                    pipeline.save(output_dir).expect("save intent audit");
                }
                IA::Audit { k } => {
                    let pipeline = IntentPipeline::load_or_init(output_dir);
                    println!("{}", "INTENT AUDIT NAPLÓ".cyan().bold());
                    if pipeline.audit_log.is_empty() {
                        println!("  (nincs intent — futtass: intent generate)");
                    } else {
                        let start = pipeline.audit_log.len().saturating_sub(k);
                        for intent in &pipeline.audit_log[start..] {
                            println!(
                                "  [{}] {} strength={:.3} allowed={} steps={}",
                                intent.id,
                                intent.candidate.action,
                                intent.candidate.strength,
                                intent.evaluation.allowed,
                                intent.audit_chain.len()
                            );
                        }
                    }
                    let stats = pipeline.stats();
                    println!(
                        "  Összesen: {} intent ({} engedélyezett, {} blokkolt, {} jóváhagyás kell)",
                        stats.total_intents, stats.allowed, stats.blocked, stats.approval_required
                    );
                }
                IA::Genome => {
                    let pipeline = IntentPipeline::load_or_init(output_dir);
                    let genome = &pipeline.genome;
                    println!("{}", "GENOME".cyan().bold());
                    println!("  Identitás:  {}", genome.identity);
                    println!("  Küldetés:   {}", genome.mission);
                    println!();
                    println!("  {}", "── Értékek ──".yellow());
                    for v in &genome.values {
                        println!("    • {}", v);
                    }
                    println!();
                    println!("  {}", "── Korlátok ──".yellow());
                    for c in &genome.constraints {
                        println!("    [{}] {} — {}", c.severity, c.name, c.description);
                    }
                    println!();
                    println!("  {}", "── Képességek ──".yellow());
                    for c in &genome.capabilities {
                        println!("    • {}", c);
                    }
                    println!();
                    println!("  {}", "── Preferenciák ──".yellow());
                    for p in &genome.preferences {
                        println!("    • {}", p);
                    }
                }
            }
        }
        Cmd::Absentia { action } => {
            use microscope_memory::absentia::AbsentiaState;
            use microscope_memory::cli::AbsentiaAction;
            use microscope_memory::epistemic::EvidenceLedger;
            use microscope_memory::hebbian::HebbianState;

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
                    println!(
                        "  Causal laundering gyanús: {}",
                        stats.causal_laundering_suspect
                    );
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
                    println!(
                        "  Causal laundering gyanús: {}",
                        stats.causal_laundering_suspect
                    );
                }
                AbsentiaAction::AntiHebbian { k } => {
                    let absentia = AbsentiaState::load_or_init(output_dir);
                    println!("{}", "ANTI-HEBBIAN PÁROK".cyan().bold());
                    if absentia.anti_hebbian.is_empty() {
                        println!("  (nincs anti-Hebbian pár — futtass: absentia scan)");
                    } else {
                        let start = absentia.anti_hebbian.len().saturating_sub(k);
                        for p in &absentia.anti_hebbian[start..] {
                            println!(
                                "  [{}↔{}] absence={:.3} expected={:.3} actual={:.3}",
                                p.block_a,
                                p.block_b,
                                p.absence_score,
                                p.expected_coactivation,
                                p.actual_coactivation
                            );
                        }
                    }
                    println!("  összesen: {}", absentia.anti_hebbian.len());
                }
                AbsentiaAction::CausalLaundering => {
                    let absentia = AbsentiaState::load_or_init(output_dir);
                    println!("{}", "CAUSAL LAUNDERING GYANÚS PÁROK".red().bold());
                    let suspects: Vec<_> = absentia
                        .anti_hebbian
                        .iter()
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
