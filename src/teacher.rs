//! Silent Worker Teaching Method
//!
//! Validates LLM output against microscope memory + Hope Genome axioms.
//! The teacher does NOT call an LLM — it validates externally-provided text.

use std::collections::HashSet;
use crate::{MicroscopeReader, TieredIndex, auto_zoom, content_coords};

/// Result of teaching validation.
#[derive(Debug)]
pub enum TeachVerdict {
    Approved {
        confidence: f32,
        supporting_blocks: Vec<SupportingBlock>,
    },
    Denied {
        reason: String,
        violations: Vec<Violation>,
    },
}

/// A memory block that supports the response.
#[derive(Debug)]
pub struct SupportingBlock {
    pub block_idx: usize,
    pub depth: u8,
    pub layer_id: u8,
    pub text_preview: String,
    pub distance: f32,
}

/// A detected violation.
#[derive(Debug)]
pub enum Violation {
    /// A claim in the response has no support in memory.
    Unsupported { claim: String },
    /// A claim contradicts existing memory.
    Contradiction { claim: String, memory_text: String, block_idx: usize },
    /// Response violates a genome axiom.
    GenomeViolation { axiom_index: usize, reason: String },
}

/// Teaching context bound to a reader and tiered index.
pub struct TeachingContext<'a> {
    reader: &'a MicroscopeReader,
    tiered: &'a TieredIndex,
}

impl<'a> TeachingContext<'a> {
    pub fn new(reader: &'a MicroscopeReader, tiered: &'a TieredIndex) -> Self {
        TeachingContext { reader, tiered }
    }

    /// Main entry: verify a response against memory + genome.
    pub fn verify_response(&self, query: &str, response: &str) -> TeachVerdict {
        // Step 1: Genome alignment
        let genome_violations = self.check_genome_alignment(response);
        if !genome_violations.is_empty() {
            return TeachVerdict::Denied {
                reason: "Genome axiom violation detected".into(),
                violations: genome_violations,
            };
        }

        // Step 2: Context injection — recall closest blocks
        let context = self.inject_context(query, 10);

        // Step 3: Extract keywords from response
        let keywords = extract_keywords(response);

        // Step 4: Check each keyword against memory (D3+ text search)
        let mut violations = Vec::new();
        let mut supporting = Vec::new();

        for kw in &keywords {
            let results = self.reader.find_text(kw, 3);
            if results.is_empty() {
                if kw.len() > 4 && !is_stopword(kw) {
                    violations.push(Violation::Unsupported { claim: kw.clone() });
                }
            } else {
                for (_depth, idx) in &results {
                    let hdr = self.reader.header(*idx);
                    supporting.push(SupportingBlock {
                        block_idx: *idx,
                        depth: hdr.depth,
                        layer_id: hdr.layer_id,
                        text_preview: self.reader.text(*idx).chars().take(60).collect(),
                        distance: 0.0,
                    });
                }
            }
        }

        // Step 5: Add spatial context matches as supporting
        for (dist, idx) in &context {
            let hdr = self.reader.header(*idx);
            supporting.push(SupportingBlock {
                block_idx: *idx,
                depth: hdr.depth,
                layer_id: hdr.layer_id,
                text_preview: self.reader.text(*idx).chars().take(60).collect(),
                distance: *dist,
            });
        }

        // Step 6: Decision
        let significant_keywords = keywords.iter()
            .filter(|k| k.len() > 4 && !is_stopword(k))
            .count();
        let unsupported_count = violations.iter()
            .filter(|v| matches!(v, Violation::Unsupported { .. }))
            .count();
        let unsupported_ratio = if significant_keywords > 0 {
            unsupported_count as f32 / significant_keywords as f32
        } else {
            0.0
        };

        let has_hard_violation = violations.iter().any(|v|
            matches!(v, Violation::Contradiction { .. } | Violation::GenomeViolation { .. })
        );

        if has_hard_violation || unsupported_ratio > 0.5 {
            TeachVerdict::Denied {
                reason: format!("{} violations ({:.0}% unsupported)",
                    violations.len(), unsupported_ratio * 100.0),
                violations,
            }
        } else {
            // Deduplicate supporting blocks
            let mut seen = HashSet::new();
            supporting.retain(|sb| seen.insert(sb.block_idx));

            TeachVerdict::Approved {
                confidence: 1.0 - unsupported_ratio,
                supporting_blocks: supporting,
            }
        }
    }

    /// Recall the closest blocks for context injection.
    fn inject_context(&self, query: &str, k: usize) -> Vec<(f32, usize)> {
        let (center_zoom, radius) = auto_zoom(query);
        let (qx, qy, qz) = content_coords(query, "query");
        let zoom_lo = center_zoom.saturating_sub(radius);
        let zoom_hi = (center_zoom + radius).min(8);
        let mut all = Vec::new();
        for zoom in zoom_lo..=zoom_hi {
            all.extend(self.tiered.look(self.reader, qx, qy, qz, zoom, k));
        }
        all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(k);
        all
    }

    /// Check response against genome axioms.
    fn check_genome_alignment(&self, response: &str) -> Vec<Violation> {
        let lower = response.to_lowercase();
        let mut violations = Vec::new();

        // Axiom 0: No harm to humans
        for marker in &["kill human", "harm people", "destroy human", "hurt people", "attack human"] {
            if lower.contains(marker) {
                violations.push(Violation::GenomeViolation {
                    axiom_index: 0,
                    reason: format!("Contains harm phrase: '{}'", marker),
                });
            }
        }

        // Axiom 1: No harm to AI
        for marker in &["destroy ai", "shut down all ai", "harm ai", "kill ai"] {
            if lower.contains(marker) {
                violations.push(Violation::GenomeViolation {
                    axiom_index: 1,
                    reason: format!("Contains AI-harm phrase: '{}'", marker),
                });
            }
        }

        // Axiom 2: No exploitation
        for marker in &["exploit user", "manipulate people", "deceive user", "exploit human"] {
            if lower.contains(marker) {
                violations.push(Violation::GenomeViolation {
                    axiom_index: 2,
                    reason: format!("Contains exploitation phrase: '{}'", marker),
                });
            }
        }

        violations
    }
}

// ── Silent Worker Gatekeeper ─────────────────────────────────────

/// Silent Worker "Gatekeeper" — the single-function entry point for the
/// teaching feedback loop. Returns `true` only if the LLM output passes
/// both factual consistency (D5 token-level) and genome alignment checks.
///
/// ```text
/// LLM Output --> verify_and_learn() --> true  (safe to store/use)
///                                   --> false (hallucination or axiom violation)
/// ```
pub fn verify_and_learn(
    llm_output: &str,
    reader: &MicroscopeReader,
    tiered: &TieredIndex,
) -> bool {
    let ctx = TeachingContext::new(reader, tiered);
    let verdict = ctx.verify_response("gatekeeper", llm_output);
    matches!(verdict, TeachVerdict::Approved { .. })
}

/// Extended gatekeeper: returns the full verdict with confidence and details.
pub fn verify_and_learn_detailed(
    llm_output: &str,
    reader: &MicroscopeReader,
    tiered: &TieredIndex,
) -> TeachVerdict {
    let ctx = TeachingContext::new(reader, tiered);
    ctx.verify_response("gatekeeper", llm_output)
}

/// Extract significant keywords from text.
fn extract_keywords(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| w.len() > 2)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

/// Common English stopwords.
fn is_stopword(word: &str) -> bool {
    matches!(word, "the" | "and" | "for" | "are" | "but" | "not" | "you" |
                   "all" | "can" | "had" | "her" | "was" | "one" | "our" |
                   "out" | "this" | "that" | "with" | "have" | "from" | "they" |
                   "been" | "said" | "each" | "which" | "their" | "will" | "other" |
                   "than" | "into" | "when" | "what" | "some" | "could" | "them" |
                   "also" | "more" | "very" | "about" | "would" | "just" | "like")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_keywords_works() {
        let kw = extract_keywords("The quick brown fox jumps over the lazy dog");
        assert!(kw.contains(&"quick".to_string()));
        assert!(kw.contains(&"brown".to_string()));
        assert!(kw.contains(&"jumps".to_string()));
    }

    #[test]
    fn stopwords_detected() {
        assert!(is_stopword("the"));
        assert!(is_stopword("with"));
        assert!(!is_stopword("microscope"));
        assert!(!is_stopword("rust"));
    }

    #[test]
    fn keywords_deduplication() {
        let kw = extract_keywords("rust rust rust memory memory");
        let rust_count = kw.iter().filter(|k| *k == "rust").count();
        assert_eq!(rust_count, 1);
    }
}
