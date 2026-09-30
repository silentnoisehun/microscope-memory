//! Shared lexical-spatial ranking for every natural-language recall surface.
//!
//! The legacy ranker subtracted a fixed keyword boost from squared spatial
//! distance and clamped the result at zero.  On real memories this collapsed
//! many distinct candidates to the same score, making traversal order decide
//! the final ranking.  This module keeps the score continuous and normalizes
//! lexical evidence by query coverage.

/// Parsed query reused while scoring all candidate memories.
#[derive(Clone, Debug)]
pub struct RelevanceQuery {
    normalized: String,
    tokens: Vec<String>,
}

impl RelevanceQuery {
    pub fn new(query: &str) -> Self {
        let normalized = normalize(query);
        let mut tokens = Vec::new();
        for token in normalized
            .split_whitespace()
            .filter(|token| token.len() > 2)
        {
            if !tokens.iter().any(|existing| existing == token) {
                tokens.push(token.to_string());
            }
        }
        Self { normalized, tokens }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The normalized query tokens (length > 2, deduplicated). Used by the
    /// inverted text index to prefilter blocks before lexical scoring.
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// Lexical relevance in `[0, 1]`.
    ///
    /// Exact query-token coverage is dominant.  A conservative prefix match
    /// helps inflected English and Hungarian words without allowing short,
    /// noisy fragments to dominate.  Contiguous phrase matches break ties.
    pub fn lexical_score(&self, text: &str) -> f32 {
        if self.tokens.is_empty() {
            return 0.0;
        }

        let lowercase_text = text.to_lowercase();
        if lowercase_text.is_empty() {
            return 0.0;
        }

        // The word list is built once per block, not once per query token.
        //
        // The original was `for query_token { text.split(..).map(similarity)
        // .fold(max) }`, which re-walked and re-split the whole block for every
        // query token. Blocks hold a whole document (16 KiB after the
        // BLOCK_DATA_SIZE change) and the lexical prefilter admits ~4,300 of
        // them, so an 8-token query spent its time re-tokenising 69 MB of text
        // eight times over: roughly 86 million `token_similarity` calls and
        // eight full scans per block.
        //
        // Tokenising once and then folding the query tokens over the same words
        // visits exactly the same (query_token, word) pairs in the same order,
        // so the maximum per token is unchanged. `split_once_per_block` is
        // asserted against the original formulation in the tests below rather
        // than assumed.
        let words: Vec<&str> = lowercase_text
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|token| !token.is_empty())
            .collect();

        let mut matched = 0.0f32;
        for query_token in &self.tokens {
            let best = words
                .iter()
                .map(|text_token| token_similarity(query_token, text_token))
                .fold(0.0f32, f32::max);
            matched += best;
        }

        let coverage = matched / self.tokens.len() as f32;
        let phrase =
            (!self.normalized.is_empty() && lowercase_text.contains(&self.normalized)) as u8 as f32;
        (coverage * 0.95 + phrase * 0.05).clamp(0.0, 1.0)
    }

    /// Continuous lower-is-better rank distance.
    ///
    /// Lexical coverage owns most of the rank, spatial distance resolves
    /// semantically comparable candidates, and importance is deliberately a
    /// small prior so it cannot rescue an unrelated memory.
    pub fn rank_distance(
        &self,
        text: &str,
        spatial_dist_sq: f32,
        keyword_boost: f32,
        importance: u8,
    ) -> f32 {
        let lexical = self.lexical_score(text);
        rank_distance_from_score(lexical, spatial_dist_sq, keyword_boost, importance)
    }
}

/// Lower-is-better rank distance for a lexical score already computed during
/// candidate filtering.  Keeping this separate prevents normalizing every
/// candidate twice on the hot recall path.
#[inline]
pub fn rank_distance_from_score(
    lexical_score: f32,
    spatial_dist_sq: f32,
    keyword_boost: f32,
    importance: u8,
) -> f32 {
    let lexical_weight = 0.8 + keyword_boost.clamp(0.0, 2.0);
    let lexical_penalty = (1.0 - lexical_score.clamp(0.0, 1.0)) * lexical_weight;
    let spatial_component = spatial_dist_sq.max(0.0) * 0.20;
    let importance_prior = 1.0 + (importance.min(10) as f32 * 0.015);
    (lexical_penalty + spatial_component) / importance_prior
}

/// Apply a positive cognitive boost without collapsing distinct ranks to zero.
#[inline]
pub fn apply_boost(distance: f32, boost: f32) -> f32 {
    distance / (1.0 + boost.max(0.0))
}

/// Apply an evidence-confidence boost to the rank distance.
///
/// `confidence` is 0..100 (0 = no evidence). The boost is subtracted from
/// the divisor, so higher confidence means lower distance (better rank).
/// A confidence of 50 reduces the distance by ~5%; 100 by ~10%.
///
/// The boost is gentle — it cannot dominate lexical or spatial distance,
/// it only orders memories with the same content by how much evidence
/// stands behind them. C1: recall frequency never affects this.
#[inline]
pub fn apply_confidence_boost(distance: f32, confidence: u8) -> f32 {
    distance / (1.0 + (confidence as f32 * 0.001))
}

fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_was_space = true;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
            previous_was_space = false;
        } else if !previous_was_space {
            out.push(' ');
            previous_was_space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn token_similarity(query: &str, text: &str) -> f32 {
    if query == text {
        return 1.0;
    }
    let common_prefix = query
        .chars()
        .zip(text.chars())
        .take_while(|(left, right)| left == right)
        .count();
    let shorter = query.chars().count().min(text.chars().count());
    if shorter >= 5 && common_prefix >= 5 {
        common_prefix as f32 / query.chars().count().max(text.chars().count()) as f32 * 0.72
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original `lexical_score`, kept verbatim as a reference oracle.
    ///
    /// The rewrite tokenises a block once instead of once per query token. That
    /// is only legitimate if it produces the same number, and "it obviously
    /// does" is exactly the kind of claim that should not be believed on a hot
    /// path that decides ranking, so the old formulation lives here and the new
    /// one is checked against it.
    fn lexical_score_reference(q: &RelevanceQuery, text: &str) -> f32 {
        if q.tokens.is_empty() {
            return 0.0;
        }
        let lowercase_text = text.to_lowercase();
        if lowercase_text.is_empty() {
            return 0.0;
        }
        let mut matched = 0.0f32;
        for query_token in &q.tokens {
            let best = lowercase_text
                .split(|ch: char| !ch.is_alphanumeric())
                .filter(|token| !token.is_empty())
                .map(|text_token| token_similarity(query_token, text_token))
                .fold(0.0f32, f32::max);
            matched += best;
        }
        let coverage = matched / q.tokens.len() as f32;
        let phrase =
            (!q.normalized.is_empty() && lowercase_text.contains(&q.normalized)) as u8 as f32;
        (coverage * 0.95 + phrase * 0.05).clamp(0.0, 1.0)
    }

    #[test]
    fn split_once_per_block_matches_the_per_token_split() {
        // Deliberately awkward inputs: empty text, text with no word characters
        // at all, repeated punctuation, accented characters, a query token that
        // appears twice, and a phrase that does and does not occur.
        let texts = [
            "",
            "   ",
            "!!! ??? ---",
            "the user has a cat named Bella",
            "Bella, bella, BELLA; the-user/has\ta\tcat",
            "visszakeresési minőség",
            "A visszakeresési-minőség mérhető.",
            "aBc DeF 123 x9  456",
            "rebuild rollback rollback rollback",
            "Transactional rebuild uses a rollback snapshot",
        ];
        let queries = [
            "",
            "cat",
            "the user has a cat named Bella",
            "rebuild rollback",
            "visszakeresési minőség",
            "123 x9",
            "a",
        ];
        for q in &queries {
            let query = RelevanceQuery::new(q);
            for t in &texts {
                let got = query.lexical_score(t);
                let want = lexical_score_reference(&query, t);
                assert!(
                    (got - want).abs() < 1e-6,
                    "query {q:?} text {t:?}: got {got}, reference {want}"
                );
            }
        }
    }

    #[test]
    fn complete_query_coverage_beats_partial_match() {
        let query = RelevanceQuery::new("transactional rebuild rollback");
        let complete = query.lexical_score("Transactional rebuild uses a rollback snapshot");
        let partial = query.lexical_score("Rebuild completed successfully");
        assert!(
            complete > partial + 0.4,
            "{complete} should dominate {partial}"
        );
    }

    #[test]
    fn unicode_and_punctuation_are_normalized() {
        let query = RelevanceQuery::new("visszakeresési minőség");
        assert!(query.lexical_score("A visszakeresési-minőség mérhető.") >= 0.95);
    }

    #[test]
    fn continuous_distance_does_not_collapse_keyword_matches() {
        let query = RelevanceQuery::new("octopus snapshot lifecycle");
        let complete = query.rank_distance("Octopus snapshot lifecycle is durable", 0.5, 0.4, 5);
        let partial = query.rank_distance("Octopus runtime", 0.01, 0.4, 5);
        assert!(complete < partial, "complete={complete}, partial={partial}");
        assert_ne!(complete, 0.0);
        assert_ne!(partial, 0.0);
    }

    #[test]
    fn boosts_preserve_existing_order() {
        assert!(apply_boost(0.2, 0.5) < apply_boost(0.4, 0.5));
    }
}

/// Build the block set that recall ranks, from the two independent candidate
/// sources.
///
/// The inverted text index and the embedding index each produce a partial view:
/// tokens for one, nearest neighbours for the other. Neither is a gate on the
/// other. An earlier CLI implementation walked the depth ranges and *skipped*
/// any block absent from the lexical candidate list, which silently discarded
/// every vector hit that was not also a lexical hit -- and returned nothing at
/// all when the lexical list came back empty, i.e. for exactly the paraphrases
/// the embedding path is meant to answer.
///
/// Returns a sorted, deduplicated list; out-of-range ids are dropped so a stale
/// or corrupt index cannot cause an out-of-bounds read.
pub fn merge_candidates(
    lexical: impl IntoIterator<Item = u32>,
    semantic: impl IntoIterator<Item = usize>,
    block_count: usize,
) -> Vec<usize> {
    let mut out: Vec<usize> = lexical
        .into_iter()
        .map(|c| c as usize)
        .chain(semantic)
        .filter(|&i| i < block_count)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod candidate_tests {
    use super::merge_candidates;

    #[test]
    fn semantic_candidates_survive_without_any_lexical_overlap() {
        // The regression: a paraphrase sharing no token with the target block.
        // The lexical index contributes nothing; the vector hit must still rank.
        let merged = merge_candidates(Vec::<u32>::new(), [7usize], 16);
        assert_eq!(merged, vec![7]);
    }

    #[test]
    fn lexical_and_semantic_are_unioned_and_deduplicated() {
        let merged = merge_candidates([3u32, 1, 9], [9usize, 4, 1], 16);
        assert_eq!(merged, vec![1, 3, 4, 9]);
    }

    #[test]
    fn out_of_range_ids_are_dropped() {
        // A corrupt or stale index must not produce an out-of-bounds access.
        let merged = merge_candidates([2u32, 99], [50usize, 2], 16);
        assert_eq!(merged, vec![2]);
    }

    #[test]
    fn empty_sources_yield_no_candidates() {
        assert!(merge_candidates(Vec::<u32>::new(), Vec::<usize>::new(), 16).is_empty());
    }
}
