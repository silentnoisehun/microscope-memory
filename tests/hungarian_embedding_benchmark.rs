//! Hungarian retrieval benchmark: lexical baseline versus real multilingual
//! embeddings.

use microscope_memory::embeddings::{provider_from_config, EmbeddingProvider};
use microscope_memory::relevance::RelevanceQuery;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
struct Metrics {
    recall_at_1: f32,
    recall_at_5: f32,
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

fn corpus_by_id(input: &str) -> Vec<(&str, &str)> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (id, text) = line
                .split_once('\t')
                .expect("fixture rows must be tab separated");
            (id, text)
        })
        .collect()
}

fn evaluate<'a>(
    rankings: &HashMap<&'a str, Vec<&'a str>>,
    cases: &[(&'a str, &'a str)],
) -> Metrics {
    let mut r1 = 0usize;
    let mut r5 = 0usize;
    let mut reciprocal = 0.0f32;
    for (query, expected) in cases {
        if let Some(rank) = rankings[query].iter().position(|id| *id == *expected) {
            if rank == 0 {
                r1 += 1;
            }
            if rank < 5 {
                r5 += 1;
            }
            reciprocal += 1.0 / (rank + 1) as f32;
        }
    }
    let n = cases.len() as f32;
    Metrics {
        recall_at_1: r1 as f32 / n,
        recall_at_5: r5 as f32 / n,
        mrr: reciprocal / n,
    }
}

fn lexical_rankings<'a>(
    corpus: &[(&'a str, &'a str)],
    cases: &[(&'a str, &'a str)],
) -> HashMap<&'a str, Vec<&'a str>> {
    cases
        .iter()
        .map(|(query, _)| {
            let relevance = RelevanceQuery::new(query);
            let mut ranked: Vec<(f32, usize, &str)> = corpus
                .iter()
                .enumerate()
                .filter_map(|(position, (id, text))| {
                    let score = relevance.lexical_score(text);
                    (score > 0.0).then_some((score, position, *id))
                })
                .collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            (*query, ranked.into_iter().map(|(_, _, id)| id).collect())
        })
        .collect()
}

struct SemanticModel<'a> {
    rankings: HashMap<&'a str, Vec<&'a str>>,
    similarities: HashMap<&'a str, HashMap<&'a str, f32>>,
}

fn semantic_model<'a>(
    provider: &dyn EmbeddingProvider,
    corpus: &[(&'a str, &'a str)],
    cases: &[(&'a str, &'a str)],
) -> SemanticModel<'a> {
    let corpus_vectors: HashMap<&'a str, Vec<f32>> = corpus
        .iter()
        .map(|(id, text)| (*id, provider.embed(text).expect("corpus embedding")))
        .collect();
    let mut rankings = HashMap::new();
    let mut similarities = HashMap::new();
    for (query, _) in cases {
        let q = provider.embed(query).expect("query embedding");
        let mut scored: Vec<(f32, usize, &str)> = corpus
            .iter()
            .enumerate()
            .map(|(position, (id, _))| {
                let vector = &corpus_vectors[id];
                (
                    microscope_memory::embeddings::cosine_similarity_simd(&q, vector),
                    position,
                    *id,
                )
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        similarities.insert(
            *query,
            scored.iter().map(|(sim, _, id)| (*id, *sim)).collect(),
        );
        rankings.insert(*query, scored.into_iter().map(|(_, _, id)| id).collect());
    }
    SemanticModel {
        rankings,
        similarities,
    }
}

fn hybrid_rankings<'a>(
    corpus: &[(&'a str, &'a str)],
    lexical: &HashMap<&'a str, Vec<&'a str>>,
    semantic: &SemanticModel<'a>,
    cases: &[(&'a str, &'a str)],
    semantic_rank_gain: f32,
) -> HashMap<&'a str, Vec<&'a str>> {
    let mut out = HashMap::new();
    for (query, _) in cases {
        let relevance = RelevanceQuery::new(query);
        let mut candidates: Vec<(&'a str, f32)> = Vec::new();
        let mut seen = HashSet::new();
        for id in semantic.rankings[query].iter().chain(lexical[query].iter()) {
            if !seen.insert(*id) {
                continue;
            }
            let text = corpus
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, text)| *text)
                .unwrap_or("");
            let lexical_score = relevance.lexical_score(text);
            let similarity = semantic.similarities[query].get(id).copied().unwrap_or(0.0);
            let distance =
                microscope_memory::relevance::rank_distance_from_score(lexical_score, 0.0, 0.4, 5)
                    - similarity * semantic_rank_gain;
            candidates.push((id, distance));
        }
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(b.0)));
        out.insert(*query, candidates.into_iter().map(|(id, _)| id).collect());
    }
    out
}

#[test]
fn hungarian_lexical_baseline_is_measured() {
    let corpus = corpus_by_id(include_str!("fixtures/relevance/hungarian_corpus.tsv"));
    let cases = rows(include_str!("fixtures/relevance/hungarian_cases.tsv"));
    let started = Instant::now();
    let rankings = lexical_rankings(&corpus, &cases);
    let elapsed = started.elapsed();
    let metrics = evaluate(&rankings, &cases);
    println!(
        "lexical R@1={:.3} R@5={:.3} MRR={:.3} elapsed={:?}",
        metrics.recall_at_1, metrics.recall_at_5, metrics.mrr, elapsed
    );
    assert!(metrics.recall_at_5 >= 0.70, "{metrics:?}");
}

#[test]
fn hungarian_semantic_and_hybrid_quality_gate() {
    let corpus = corpus_by_id(include_str!("fixtures/relevance/hungarian_corpus.tsv"));
    let cases = rows(include_str!("fixtures/relevance/hungarian_cases.tsv"));

    let mut config = microscope_memory::config::Config::default();
    if let Ok(model) = std::env::var("MICROSCOPE_EMBED_MODEL") {
        config.embedding.model = model;
    } else {
        // The production native provider is multilingual MiniLM L12. The
        // generic Config default is the older English all-MiniLM-L6 model;
        // using it here measured a different system than production and made
        // the Hungarian quality gate meaningless.
        config.embedding.model =
            "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2".to_string();
    }
    // Keep this benchmark tied to the production provider unless the caller
    // explicitly overrides it. The generic Config default is the English
    // all-MiniLM model, which is not the model deployed by the live instance.
    config.embedding.provider =
        std::env::var("MICROSCOPE_EMBED_PROVIDER").unwrap_or_else(|_| "python".to_string());
    config.embedding.dim = std::env::var("MICROSCOPE_EMBED_DIM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(384);

    if matches!(config.embedding.provider.as_str(), "none" | "mock") {
        println!(
            "semantic skipped: provider is {}",
            config.embedding.provider
        );
        return;
    }

    let provider = provider_from_config(&config.embedding, config.embedding.dim);
    let started = Instant::now();
    let semantic = semantic_model(provider.as_ref(), &corpus, &cases);
    let semantic_elapsed = started.elapsed();
    let semantic_metrics = evaluate(&semantic.rankings, &cases);
    println!(
        "semantic R@1={:.3} R@5={:.3} MRR={:.3} elapsed={:?}",
        semantic_metrics.recall_at_1,
        semantic_metrics.recall_at_5,
        semantic_metrics.mrr,
        semantic_elapsed
    );
    for (query, expected) in &cases {
        let rank = semantic.rankings[query]
            .iter()
            .position(|id| id == expected)
            .map(|rank| rank + 1)
            .unwrap_or(0);
        println!("semantic_case rank={rank} query={query} expected={expected}");
    }

    let lexical = lexical_rankings(&corpus, &cases);
    let lexical_metrics = evaluate(&lexical, &cases);
    let mut best: Option<(f32, Metrics)> = None;
    for gain in [0.5f32, 1.0, 1.5, 2.0, 3.0, 4.0] {
        let hybrid = hybrid_rankings(&corpus, &lexical, &semantic, &cases, gain);
        let metrics = evaluate(&hybrid, &cases);
        println!(
            "hybrid gain={gain:.1} R@1={:.3} R@5={:.3} MRR={:.3}",
            metrics.recall_at_1, metrics.recall_at_5, metrics.mrr
        );
        if best.map(|(_, m)| metrics.mrr > m.mrr).unwrap_or(true) {
            best = Some((gain, metrics));
        }
    }
    let (best_gain, best_metrics) = best.expect("at least one gain value");
    println!(
        "best gain={best_gain:.1} hybrid R@1={:.3} R@5={:.3} MRR={:.3}",
        best_metrics.recall_at_1, best_metrics.recall_at_5, best_metrics.mrr
    );
    println!(
        "baseline lexical R@1={:.3} R@5={:.3} MRR={:.3}",
        lexical_metrics.recall_at_1, lexical_metrics.recall_at_5, lexical_metrics.mrr
    );

    // The semantic-only metric is intentionally stricter than the hybrid one:
    // the two hard cases are an indirect numeric question and a negated
    // dietary question. The production ranker combines both signals.
    assert!(semantic_metrics.recall_at_5 >= 0.80, "{semantic_metrics:?}");
    assert!(
        best_metrics.mrr >= lexical_metrics.mrr,
        "best={best_metrics:?} lexical={lexical_metrics:?}"
    );
    assert!(
        best_metrics.recall_at_5 > lexical_metrics.recall_at_5,
        "best={best_metrics:?} lexical={lexical_metrics:?}"
    );
}
