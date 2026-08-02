//! Deterministic relevance quality gate.
//!
//! Run with:
//! `cargo test --test relevance_benchmark -- --nocapture`

use microscope_memory::content_coords;
use microscope_memory::relevance::{rank_distance_from_score, RelevanceQuery};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
struct Metrics {
    recall_at_3: f32,
    mrr: f32,
}

fn rows(input: &str) -> Vec<(&str, &str)> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.split_once('\t')
                .expect("fixture rows must be tab separated")
        })
        .collect()
}

fn spatial_distance(query: &str, text: &str) -> f32 {
    let (qx, qy, qz) = content_coords(query, "long_term");
    let (x, y, z) = content_coords(text, "long_term");
    (x - qx).powi(2) + (y - qy).powi(2) + (z - qz).powi(2)
}

fn legacy_ranking<'a>(query: &str, corpus: &[(&'a str, &'a str)]) -> Vec<&'a str> {
    let lower = query.to_lowercase();
    let keywords: Vec<&str> = lower
        .split_whitespace()
        .filter(|word| word.len() > 2)
        .collect();
    let mut ranked: Vec<(f32, usize, &'a str)> = corpus
        .iter()
        .enumerate()
        .filter_map(|(position, (id, text))| {
            let text_lower = text.to_lowercase();
            let hits = keywords
                .iter()
                .filter(|keyword| text_lower.contains(**keyword))
                .count();
            (hits > 0).then(|| {
                let distance = (spatial_distance(query, text) - hits as f32 * 0.4).max(0.0);
                (distance, position, *id)
            })
        })
        .collect();
    ranked.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    ranked.into_iter().map(|(_, _, id)| id).collect()
}

fn hybrid_ranking<'a>(query: &str, corpus: &[(&'a str, &'a str)]) -> Vec<&'a str> {
    let relevance = RelevanceQuery::new(query);
    let mut ranked: Vec<(f32, &'a str)> = corpus
        .iter()
        .filter_map(|(id, text)| {
            let lexical = relevance.lexical_score(text);
            (lexical > 0.0).then(|| {
                (
                    rank_distance_from_score(lexical, spatial_distance(query, text), 0.4, 5),
                    *id,
                )
            })
        })
        .collect();
    ranked.sort_by(|left, right| left.0.total_cmp(&right.0).then_with(|| left.1.cmp(right.1)));
    ranked.into_iter().map(|(_, id)| id).collect()
}

fn evaluate(cases: &[(&str, &str)], rankings: &HashMap<&str, Vec<&str>>) -> Metrics {
    let mut top_three_hits = 0usize;
    let mut reciprocal_rank = 0.0f32;
    for (query, expected) in cases {
        if let Some(rank) = rankings[query].iter().position(|id| id == expected) {
            if rank < 3 {
                top_three_hits += 1;
            }
            reciprocal_rank += 1.0 / (rank + 1) as f32;
        }
    }
    Metrics {
        recall_at_3: top_three_hits as f32 / cases.len() as f32,
        mrr: reciprocal_rank / cases.len() as f32,
    }
}

#[test]
fn relevance_quality_gate() {
    let corpus = rows(include_str!("fixtures/relevance/corpus.tsv"));
    let cases = rows(include_str!("fixtures/relevance/cases.tsv"));

    let started = Instant::now();
    let legacy: HashMap<&str, Vec<&str>> = cases
        .iter()
        .map(|(query, _)| (*query, legacy_ranking(query, &corpus)))
        .collect();
    let legacy_elapsed = started.elapsed();

    let started = Instant::now();
    let hybrid: HashMap<&str, Vec<&str>> = cases
        .iter()
        .map(|(query, _)| (*query, hybrid_ranking(query, &corpus)))
        .collect();
    let hybrid_elapsed = started.elapsed();

    let baseline = evaluate(&cases, &legacy);
    let improved = evaluate(&cases, &hybrid);
    println!(
        "legacy Recall@3={:.3} MRR={:.3} elapsed={:?}",
        baseline.recall_at_3, baseline.mrr, legacy_elapsed
    );
    println!(
        "hybrid Recall@3={:.3} MRR={:.3} elapsed={:?}",
        improved.recall_at_3, improved.mrr, hybrid_elapsed
    );

    assert!(improved.recall_at_3 >= 0.90, "{improved:?}");
    assert!(improved.mrr >= 0.80, "{improved:?}");
    assert!(
        improved.recall_at_3 > baseline.recall_at_3,
        "baseline={baseline:?}, improved={improved:?}"
    );
    assert!(
        improved.mrr > baseline.mrr,
        "baseline={baseline:?}, improved={improved:?}"
    );
}
