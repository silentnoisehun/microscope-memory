//! Real-corpus retrieval benchmark: lexical baseline vs. hybrid retrieval.
//!
//! The earlier Hungarian fixture (10 questions over 19 statements) was a
//! smoke test.  This suite runs against a **live instance** and asks the
//! production retrieval path the only question that matters: does the
//! semantic path actually return the right memory more often than the
//! lexical path?
//!
//! Everything is measured through `retrieval::rank_candidates`, the same
//! function the CLI and the MCP server call: lexical prefilter from the
//! inverted text index, union with the semantic candidates from the vector
//! index, then the hybrid distance.  Nothing here re-implements retrieval.
//!
//! Ground truth is the exact stored text.  The block is resolved at run time,
//! so the fixture survives rebuilds that renumber blocks.
//!
//! Run it against an instance:
//!
//! ```text
//! MICROSCOPE_CONFIG=<config.toml> \
//!   cargo test --release --features embeddings --test real_corpus_benchmark -- --nocapture
//! ```
//!
//! Without `MICROSCOPE_CONFIG` the test skips instead of failing: it needs a
//! live instance, and a normal `cargo test` run must stay green on a machine
//! that has none.

use microscope_memory::config::Config;
use microscope_memory::reader::MicroscopeReader;
use microscope_memory::retrieval::{
    collect_semantic_candidates, rank_candidates, RankedHit, SemanticCandidates,
};
use std::collections::HashMap;
use std::time::Instant;

const GAINS: &[f32] = &[0.5, 1.0, 1.5, 2.0, 3.0];
const K: usize = 10;

#[derive(Debug, Clone)]
struct Case {
    id: String,
    /// Single-line substring of the stored block text.  The exact stored text
    /// of a statement differs from a fixture copy in whitespace and line
    /// breaks -- the hierarchy keeps it inside a larger session text at one
    /// depth and on its own at another -- so the ground truth is resolved by
    /// containment, the same way the CLI diagnostic matches.
    needle: String,
    queries: Vec<(String, String)>, // (lang, query)
}

fn load_cases() -> Vec<Case> {
    let raw = include_str!("fixtures/relevance/real_corpus_cases.json");
    let json: serde_json::Value = serde_json::from_str(raw).expect("fixture is valid JSON");
    json["cases"]
        .as_array()
        .expect("cases array")
        .iter()
        .map(|c| Case {
            id: c["id"].as_str().unwrap_or_default().to_string(),
            needle: c["needle"].as_str().unwrap_or_default().to_string(),
            queries: c["queries"]
                .as_array()
                .expect("queries array")
                .iter()
                .map(|q| {
                    (
                        q["lang"].as_str().unwrap_or_default().to_string(),
                        q["q"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect(),
        })
        .collect()
}

#[derive(Default, Clone, Copy, Debug)]
struct Metrics {
    n: usize,
    r1: usize,
    r5: usize,
    r10: usize,
    reciprocal: f64,
    millis: f64,
}

impl Metrics {
    fn add(&mut self, rank: Option<usize>, millis: f64) {
        self.n += 1;
        self.millis += millis;
        if let Some(r) = rank {
            if r == 0 {
                self.r1 += 1;
            }
            if r < 5 {
                self.r5 += 1;
            }
            if r < 10 {
                self.r10 += 1;
            }
            self.reciprocal += 1.0 / (r + 1) as f64;
        }
    }

    fn report(&self, label: &str) {
        if self.n == 0 {
            println!("  {label:<22} (no queries)");
            return;
        }
        let n = self.n as f64;
        println!(
            "  {label:<22} R@1={:.3}  R@5={:.3}  R@10={:.3}  MRR={:.3}  n={}  {:.1} ms/query",
            self.r1 as f64 / n,
            self.r5 as f64 / n,
            self.r10 as f64 / n,
            self.reciprocal / n,
            self.n,
            self.millis / n
        );
    }
}

/// Rank of the expected memory in a ranked hit list, matched on the needle.
fn rank_of(hits: &[RankedHit], needle: &str) -> Option<usize> {
    let needle = needle.to_lowercase();
    hits.iter()
        .position(|hit| hit.text.to_lowercase().contains(&needle))
}

fn rank_for(
    config: &Config,
    reader: &MicroscopeReader,
    appended: &[microscope_memory::types::AppendEntry],
    query: &str,
    semantic: &SemanticCandidates,
) -> Vec<RankedHit> {
    let (qx, qy, qz) = microscope_memory::content_coords_blended_with_config(
        query,
        "long_term",
        config.search.semantic_weight,
        &config.embedding,
    );
    rank_candidates(
        config, reader, appended, query, semantic, qx, qy, qz, None, None,
    )
}

#[test]
fn real_corpus_lexical_vs_hybrid() {
    let Ok(config_path) = std::env::var("MICROSCOPE_CONFIG") else {
        println!("SKIP: MICROSCOPE_CONFIG is not set (needs a live instance)");
        return;
    };
    let config = Config::load(&config_path).expect("config loads");
    let reader = MicroscopeReader::open(&config).expect("instance opens");
    let cases = load_cases();
    // Read the append log the way production does -- `AppendEntry` is not
    // `Clone`, so the reader's cached copy cannot be cloned out.
    let append_path = std::path::Path::new(&config.paths.output_dir).join("append.bin");
    let appended = microscope_memory::read_append_log(&append_path);

    println!(
        "\ninstance : {} ({} blocks)",
        config.paths.output_dir, reader.block_count
    );
    println!(
        "provider : {} / {} (dim {})",
        config.embedding.provider, config.embedding.model, config.embedding.dim
    );
    println!(
        "search   : semantic_weight={} semantic_rank_gain={} keyword_boost={}",
        config.search.semantic_weight,
        config.search.semantic_rank_gain,
        config.search.keyword_boost
    );

    // Resolve every expected memory to a block index once.
    let mut by_needle: HashMap<String, usize> = HashMap::new();
    for i in 0..reader.block_count {
        let text = reader.text(i).to_lowercase();
        for case in &cases {
            let needle = case.needle.to_lowercase();
            if !needle.is_empty() && text.contains(&needle) {
                by_needle.entry(case.needle.clone()).or_insert(i);
            }
        }
    }

    let mut missing = Vec::new();
    let mut resolved = Vec::new();
    for case in &cases {
        match by_needle.get(&case.needle) {
            Some(idx) => resolved.push((case, *idx)),
            None => missing.push(case.id.clone()),
        }
    }
    if !missing.is_empty() {
        panic!(
            "fixture out of sync with the instance: {} case(s) not found: {}",
            missing.len(),
            missing.join(", ")
        );
    }
    println!(
        "cases    : {} statements, {} queries\n",
        resolved.len(),
        resolved.iter().map(|(c, _)| c.queries.len()).sum::<usize>()
    );

    let mut lexical_all = Metrics::default();
    let mut lexical_hu = Metrics::default();
    let mut lexical_en = Metrics::default();
    let mut hybrid: Vec<(f32, Metrics, Metrics, Metrics)> = GAINS
        .iter()
        .map(|g| {
            (
                *g,
                Metrics::default(),
                Metrics::default(),
                Metrics::default(),
            )
        })
        .collect();

    let mut semantic_candidates_total = 0usize;
    let mut query_count = 0usize;

    for (case, _block_idx) in &resolved {
        let needle = case.needle.clone();
        for (lang, query) in &case.queries {
            query_count += 1;
            // ---- lexical only: no semantic candidates, no semantic bonus ----
            let empty = SemanticCandidates::default();
            let t0 = Instant::now();
            let hits = rank_for(&config, &reader, &appended, query, &empty);
            let lex_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let lex_rank = rank_of(&hits, &needle);
            lexical_all.add(lex_rank, lex_ms);
            if lang == "hu" {
                lexical_hu.add(lex_rank, lex_ms);
            } else {
                lexical_en.add(lex_rank, lex_ms);
            }

            // ---- hybrid: real semantic candidates + ranked bonus ----
            let t1 = Instant::now();
            let semantic = collect_semantic_candidates(&config, &reader, query, K);
            let candidate_ms = t1.elapsed().as_secs_f64() * 1000.0;
            semantic_candidates_total += semantic.main.len();

            for (gain, all, hu, en) in hybrid.iter_mut() {
                let mut cfg = config.clone();
                cfg.search.semantic_rank_gain = *gain;
                let t2 = Instant::now();
                let hits = rank_for(&cfg, &reader, &appended, query, &semantic);
                let ms = t2.elapsed().as_secs_f64() * 1000.0 + candidate_ms;
                let rank = rank_of(&hits, &needle);
                all.add(rank, ms);
                if lang == "hu" {
                    hu.add(rank, ms);
                } else {
                    en.add(rank, ms);
                }
            }
        }
    }

    println!("LEXICAL ONLY (no semantic candidates, no semantic bonus)");
    lexical_all.report("all");
    lexical_hu.report("hungarian");
    lexical_en.report("english");

    println!("\nHYBRID (semantic candidates + semantic_rank_gain)");
    for (gain, all, hu, en) in &hybrid {
        println!("  gain = {gain}");
        all.report("all");
        hu.report("hungarian");
        en.report("english");
    }

    println!(
        "\nsemantic candidates per query: {:.1} (average)",
        semantic_candidates_total as f64 / query_count.max(1) as f64
    );
}
