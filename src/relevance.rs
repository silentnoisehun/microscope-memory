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

        let mut matched = 0.0f32;
        for query_token in &self.tokens {
            let best = lowercase_text
                .split(|ch: char| !ch.is_alphanumeric())
                .filter(|token| !token.is_empty())
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
