//! Shared semantic candidate generation and hybrid ranking.
//!
//! The CLI, the agent-facing MCP server and the legacy command surface used
//! to implement retrieval separately.  They disagreed on how many semantic
//! candidates to fetch and, more importantly, on which gain applied to the
//! cosine score.  The MCP path used `semantic_weight` (a 0..1 coordinate
//! blend) where the CLI used `semantic_rank_gain`; with the production value
//! of `semantic_weight = 0.0`, a real semantic match could be admitted as a
//! candidate but could not outrank a weak lexical hit.
//!
//! This module is the single source of truth for that part of retrieval:
//! provider selection, provider/model fingerprint validation, query encoding,
//! candidate generation and score fusion.  Surface-specific state such as
//! emotional boosts remains with the caller and is applied afterwards.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::config::{Config, Search};
use crate::reader::MicroscopeReader;

/// Stable identity of a result returned by the shared retrieval surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemoryId {
    Main(usize),
    Append(usize),
}

/// A ranked result before surface-specific cognitive boosts.
#[derive(Clone, Debug)]
pub struct RankedHit {
    pub id: MemoryId,
    pub distance: f32,
    pub text: String,
}

/// Convert shared ranked hits into the legacy `(distance, index, is_main)`
/// tuple used by the CLI and MCP renderers.
///
/// Keeping this adapter shared prevents a surface-specific index convention
/// from silently changing the top-k identity.
pub fn ranked_hits_to_results(hits: Vec<RankedHit>) -> Vec<(f32, usize, bool)> {
    hits.into_iter()
        .map(|hit| {
            let (index, is_main) = match hit.id {
                MemoryId::Main(index) => (index, true),
                MemoryId::Append(index) => (index + 1_000_000, false),
            };
            (hit.distance, index, is_main)
        })
        .collect()
}

/// Stable top-k identities for parity assertions.
pub fn ranked_ids(hits: &[RankedHit], k: usize) -> Vec<MemoryId> {
    hits.iter().take(k).map(|hit| hit.id).collect()
}

/// Semantic matches from the consolidated index and from the append log.
#[derive(Debug, Default)]
pub struct SemanticCandidates {
    /// Block id -> cosine similarity for the main index.
    pub main: HashMap<usize, f32>,
    /// Append-log position -> cosine similarity for pending memories.
    pub appended: HashMap<usize, f32>,
}

impl SemanticCandidates {
    pub fn is_empty(&self) -> bool {
        self.main.is_empty() && self.appended.is_empty()
    }

    pub fn main_score(&self, block_idx: usize) -> Option<f32> {
        self.main.get(&block_idx).copied()
    }

    pub fn appended_score(&self, append_idx: usize) -> Option<f32> {
        self.appended.get(&append_idx).copied()
    }
}

/// Minimum number of semantic candidates fetched for a query.
///
/// The larger value is deliberate: the vector index is a candidate source,
/// not a final ranking.  A shallow fetch discards answers before lexical and
/// semantic evidence can be fused.
pub const MIN_SEMANTIC_CANDIDATES: usize = 256;

/// Canonical text used to collapse hierarchical duplicates.  Depth-4 blocks
/// are sentence projections of depth-3 memories; embedding and returning both
/// spends the candidate budget on the same statement more than once.
pub fn canonical_result_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Remove repeated statements while preserving the best-ranked occurrence.
pub fn deduplicate_ranked_hits(hits: Vec<RankedHit>) -> Vec<RankedHit> {
    let mut hits = hits;
    hits.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut seen = HashSet::new();
    hits.into_iter()
        .filter(|hit| seen.insert(canonical_result_text(&hit.text)))
        .collect()
}

/// Generate semantic candidates with the configured real provider.
///
/// A missing, stale or mismatched index is not an error: retrieval continues
/// through the lexical path.  A configured real provider that cannot be
/// initialized is reported, but this function does not silently substitute a
/// hash provider: `with_cached_provider` deliberately exits the process for
/// an unavailable real provider instead of returning plausible nonsense.
pub fn collect_semantic_candidates(
    config: &Config,
    reader: &MicroscopeReader,
    query: &str,
    k: usize,
) -> SemanticCandidates {
    let mut out = SemanticCandidates::default();
    let output_dir = Path::new(&config.paths.output_dir);
    let emb_path = output_dir.join("embeddings.bin");

    let Some(index) = crate::embedding_index::open_embedding_cached(&emb_path) else {
        return out;
    };
    if !index.matches_config(&config.embedding) {
        eprintln!(
            "  semantic: embedding index provider/model mismatch — ignored; rebuild required"
        );
        return out;
    }

    let embedded = crate::embeddings::with_cached_provider(&config.embedding, index.dim(), |p| {
        match p.embed(query) {
            Ok(v) if v.len() == index.dim() => Ok(v),
            Ok(v) => Err(format!(
                "provider returned {} dims, index expects {}",
                v.len(),
                index.dim()
            )),
            Err(e) => Err(e.to_string()),
        }
    });

    let query_embedding = match embedded {
        Ok(v) => v,
        Err(e) => {
            eprintln!("  semantic: embedding failed ({}); lexical path only", e);
            return out;
        }
    };

    let want = (k.saturating_mul(8)).max(MIN_SEMANTIC_CANDIDATES);
    // Over-fetch before de-duplication because the index also contains derived
    // sentence nodes.  Without this, hundreds of D3/D4 copies of the same
    // memory could consume the whole semantic candidate budget.
    let raw_want = want.saturating_mul(4).max(want);
    let mut seen_text = HashSet::new();
    for (similarity, block_idx) in index.search(&query_embedding, raw_want) {
        if block_idx < reader.block_count
            && seen_text.insert(canonical_result_text(reader.text(block_idx)))
        {
            out.main.insert(block_idx, similarity);
            if out.main.len() >= want {
                break;
            }
        }
    }

    let sidecar_path = output_dir.join(crate::embedding_index::APPEND_EMBEDDINGS_FILE);
    if let Some(sidecar) = crate::embedding_index::open_append_cached(&sidecar_path, index.dim()) {
        if sidecar.matches_config(&config.embedding) {
            for (similarity, append_idx) in sidecar.search(&query_embedding, want) {
                out.appended.insert(append_idx as usize, similarity);
            }
        } else {
            eprintln!("  semantic: append embedding sidecar provider/model mismatch — ignored");
        }
    }

    out
}

/// Collect and rank the common CLI/MCP candidate set.
///
/// The two surfaces previously differed in lexical admission and candidate
/// traversal even though they used the same final score formula.  This
/// function owns the deterministic part of retrieval: lexical prefilter,
/// semantic union, append-log admission and hybrid distance.  Surface state
/// such as predictive-cache and thought-graph boosts is applied afterwards.
// The argument list mirrors the production call sites (CLI and MCP) on
// purpose: the two surfaces must not be able to drift apart again.
#[allow(clippy::too_many_arguments)]
pub fn rank_candidates(
    config: &Config,
    reader: &MicroscopeReader,
    appended: &[crate::types::AppendEntry],
    query: &str,
    semantic: &SemanticCandidates,
    qx: f32,
    qy: f32,
    qz: f32,
    query_emotion: Option<[f32; 21]>,
    emotion_lookup: Option<&dyn Fn(usize) -> Option<[f32; 21]>>,
) -> Vec<RankedHit> {
    let relevance_query = crate::relevance::RelevanceQuery::new(query);
    let min_matches: u32 = std::env::var("MICROSCOPE_LEX_MIN_MATCHES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let lexical = reader
        .text_index
        .as_ref()
        .and_then(|index| {
            index.candidates_lexical_min_matches(relevance_query.tokens(), min_matches)
        })
        .unwrap_or_default();
    let candidates = crate::relevance::merge_candidates(
        lexical,
        semantic.main.keys().copied(),
        reader.block_count,
    );
    let emotional_recall_weight = config.search.emotional_bias_weight * 0.15;
    let mut out = Vec::new();

    for index in candidates {
        let text = reader.text(index);
        let lexical_score = relevance_query.lexical_score(text);
        let similarity = semantic.main_score(index);
        if lexical_score <= 0.0 && similarity.is_none() {
            continue;
        }
        let header = reader.header(index);
        let dx = header.x - qx;
        let dy = header.y - qy;
        let dz = header.z - qz;
        let spatial_dist = dx * dx + dy * dy + dz * dz;
        let emotional_boost = query_emotion
            .and_then(|emotion| {
                emotion_lookup
                    .and_then(|lookup| lookup(index))
                    .map(|stored| (emotion, stored))
            })
            .map(|(emotion, stored)| {
                crate::emotional_similarity(&emotion, &stored) * emotional_recall_weight
            })
            .unwrap_or(0.0);
        let distance = rank_candidate(
            &config.search,
            lexical_score,
            spatial_dist,
            header.importance,
            similarity,
            emotional_boost,
        );
        out.push(RankedHit {
            id: MemoryId::Main(index),
            distance,
            text: text.to_string(),
        });
    }

    for (index, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let spatial_dist = dx * dx + dy * dy + dz * dz;
        let lexical_score = relevance_query.lexical_score(&entry.text);
        let similarity = semantic.appended_score(index);
        let emotional_boost = query_emotion
            .map(|emotion| {
                crate::emotional_similarity(&emotion, &entry.emotion) * emotional_recall_weight
            })
            .unwrap_or(0.0);
        if spatial_dist >= 0.1
            && lexical_score <= 0.0
            && emotional_boost <= 0.0
            && similarity.is_none()
        {
            continue;
        }
        let distance = rank_candidate(
            &config.search,
            lexical_score,
            spatial_dist,
            entry.importance,
            similarity,
            emotional_boost,
        );
        out.push(RankedHit {
            id: MemoryId::Append(index),
            distance,
            text: entry.text.clone(),
        });
    }

    out.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.id.cmp(&b.id))
    });
    deduplicate_ranked_hits(out)
}

/// Fuse lexical, spatial, importance, semantic and emotional evidence into a
/// lower-is-better distance.
///
/// `semantic_rank_gain` is intentionally the only semantic ranking knob.  It
/// is not clamped to 1.0: a measured gain of 1.5 is materially different from
/// 1.0, and clamping made the benchmark sweep unmeasurable.
pub fn rank_candidate(
    search: &Search,
    lexical_score: f32,
    spatial_dist_sq: f32,
    importance: u8,
    semantic_similarity: Option<f32>,
    emotional_boost: f32,
) -> f32 {
    let base = crate::relevance::rank_distance_from_score(
        lexical_score,
        spatial_dist_sq,
        search.keyword_boost,
        importance,
    );
    let boosted = crate::relevance::apply_boost(base, emotional_boost);
    match semantic_similarity {
        Some(similarity) => boosted - similarity * search.semantic_rank_gain.clamp(0.0, 8.0),
        None => boosted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(gain: f32) -> Search {
        Search {
            semantic_rank_gain: gain,
            ..Search::default()
        }
    }

    #[test]
    fn semantic_gain_can_move_a_real_match_ahead() {
        let cfg = search(1.5);
        let semantic = rank_candidate(&cfg, 0.05, 0.10, 5, Some(0.75), 0.0);
        let lexical = rank_candidate(&cfg, 0.25, 0.10, 5, None, 0.0);
        assert!(semantic < lexical);
    }

    #[test]
    fn zero_gain_keeps_semantic_provider_effect_out_of_ranking() {
        let cfg = search(0.0);
        let semantic = rank_candidate(&cfg, 0.05, 0.10, 5, Some(0.99), 0.0);
        let lexical = rank_candidate(&cfg, 0.05, 0.10, 5, None, 0.0);
        assert_eq!(semantic, lexical);
    }

    #[test]
    fn emotional_boost_remains_a_positive_boost() {
        let cfg = search(1.5);
        let plain = rank_candidate(&cfg, 0.5, 0.1, 5, None, 0.0);
        let boosted = rank_candidate(&cfg, 0.5, 0.1, 5, None, 0.5);
        assert!(boosted < plain);
    }

    #[test]
    fn ranked_hit_ids_are_stable_and_ordered() {
        let mut hits = [
            RankedHit {
                id: MemoryId::Append(4),
                distance: 0.2,
                text: "append".to_string(),
            },
            RankedHit {
                id: MemoryId::Main(9),
                distance: 0.1,
                text: "main".to_string(),
            },
        ];
        hits.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| a.id.cmp(&b.id))
        });
        assert_eq!(hits[0].id, MemoryId::Main(9));
        assert_eq!(hits[1].id, MemoryId::Append(4));
    }

    #[test]
    fn cli_and_mcp_adapters_preserve_identical_top_k_ids() {
        let hits = vec![
            RankedHit {
                id: MemoryId::Main(3),
                distance: 0.1,
                text: "main".to_string(),
            },
            RankedHit {
                id: MemoryId::Append(2),
                distance: 0.2,
                text: "append".to_string(),
            },
        ];
        let cli_ids: Vec<MemoryId> = ranked_hits_to_results(hits.clone())
            .into_iter()
            .take(2)
            .map(|(_, index, is_main)| {
                if is_main {
                    MemoryId::Main(index)
                } else {
                    MemoryId::Append(index - 1_000_000)
                }
            })
            .collect();
        let mcp_ids = ranked_ids(&hits, 2);
        assert_eq!(cli_ids, mcp_ids);
    }

    #[test]
    fn duplicate_hierarchical_text_is_collapsed() {
        let hits = vec![
            RankedHit {
                id: MemoryId::Main(10),
                distance: 0.2,
                text: "A projekt nyelve Rust.".to_string(),
            },
            RankedHit {
                id: MemoryId::Main(11),
                distance: 0.1,
                text: "  A projekt   nyelve Rust.  ".to_string(),
            },
            RankedHit {
                id: MemoryId::Main(12),
                distance: 0.3,
                text: "A projekt nyelve Python.".to_string(),
            },
        ];
        let unique = deduplicate_ranked_hits(hits);
        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].id, MemoryId::Main(11));
        assert_eq!(unique[1].id, MemoryId::Main(12));
    }
}
