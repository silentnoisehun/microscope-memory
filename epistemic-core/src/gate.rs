//! Epistemic gate — the promotion gate with four-dimensional thresholds.
//!
//! The gate decides whether a claim can be promoted to a given importance
//! level. It checks all four confidence dimensions independently:
//!
//! - If evidence_confidence < min_evidence → BLOCKED (weak facts)
//! - If counterevidence_confidence < min_counterevidence → BLOCKED (refuted)
//! - If reasoning_confidence < min_reasoning → BLOCKED (broken logic)
//! - If narrative_confidence < min_narrative → BLOCKED (too speculative)
//!
//! The gate also flags penalized rules used in the reasoning graph,
//! even if the claim passes — the audit trail records them.

use crate::graph::ReasoningGraph;
use crate::rules::{InferenceRule, RuleRegistry};
use crate::types::*;

// ─── Gate Decision ──────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    Pass {
        evidence: f64,
        counterevidence: f64,
        reasoning: f64,
        narrative: f64,
        aggregate: f64,
    },
    Blocked {
        evidence: f64,
        counterevidence: f64,
        reasoning: f64,
        narrative: f64,
        failed: Vec<GateFailure>,
        penalized_rules: Vec<InferenceRule>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GateFailure {
    pub dimension: ConfidenceDimension,
    pub value: f64,
    pub threshold: f64,
}

impl std::fmt::Display for GateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {:.2} < {:.2}",
            self.dimension, self.value, self.threshold
        )
    }
}

impl std::fmt::Display for GateDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass {
                evidence,
                counterevidence,
                reasoning,
                narrative,
                aggregate,
            } => {
                write!(
                    f,
                    "PASS  evidence={evidence:.2}  counter={counterevidence:.2}  reasoning={reasoning:.2}  narrative={narrative:.2}  agg={aggregate:.2}"
                )
            }
            Self::Blocked {
                evidence,
                counterevidence,
                reasoning,
                narrative,
                failed,
                penalized_rules,
            } => {
                write!(
                    f,
                    "BLOCKED  evidence={evidence:.2}  counter={counterevidence:.2}  reasoning={reasoning:.2}  narrative={narrative:.2}"
                )?;
                if !failed.is_empty() {
                    write!(f, "  failed=[")?;
                    for (i, fail) in failed.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{fail}")?;
                    }
                    write!(f, "]")?;
                }
                if !penalized_rules.is_empty() {
                    write!(f, "  penalized=[")?;
                    for (i, r) in penalized_rules.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{r}")?;
                    }
                    write!(f, "]")?;
                }
                Ok(())
            }
        }
    }
}

// ─── Gate Configuration ─────────────────────────────

#[derive(Debug, Clone)]
pub struct GateConfig {
    pub min_evidence: f64,
    pub min_counterevidence: f64,
    pub min_reasoning: f64,
    pub min_narrative: f64,
    pub registry: RuleRegistry,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            min_evidence: 0.50,
            min_counterevidence: 0.50,
            min_reasoning: 0.50,
            min_narrative: 0.40,
            registry: RuleRegistry::new(),
        }
    }
}

impl GateConfig {
    pub fn new(
        min_evidence: f64,
        min_counterevidence: f64,
        min_reasoning: f64,
        min_narrative: f64,
    ) -> Self {
        Self {
            min_evidence,
            min_counterevidence,
            min_reasoning,
            min_narrative,
            registry: RuleRegistry::new(),
        }
    }

    /// Stricter thresholds for high-importance claims.
    pub fn strict() -> Self {
        Self::new(0.70, 0.65, 0.60, 0.50)
    }

    /// Very strict for identity-level (imp>=7) claims.
    pub fn identity() -> Self {
        Self::new(0.75, 0.70, 0.65, 0.55)
    }
}

// ─── Epistemic Gate ─────────────────────────────────

pub struct EpistemicGate {
    config: GateConfig,
}

impl EpistemicGate {
    pub fn new(config: GateConfig) -> Self {
        Self { config }
    }

    pub fn with_default() -> Self {
        Self::new(GateConfig::default())
    }

    /// Evaluate a claim against the gate with all four dimensions.
    pub fn evaluate(
        &self,
        evidence_confidence: f64,
        counterevidence_confidence: f64,
        reasoning_graph: &ReasoningGraph,
        narrative_confidence: f64,
    ) -> GateDecision {
        let reasoning_confidence = reasoning_graph.reasoning_confidence();
        let penalized_rules: Vec<InferenceRule> = reasoning_graph
            .penalized_steps()
            .iter()
            .map(|(step, _)| step.rule)
            .collect();

        let mut failures = Vec::new();

        if evidence_confidence < self.config.min_evidence {
            failures.push(GateFailure {
                dimension: ConfidenceDimension::Evidence,
                value: evidence_confidence,
                threshold: self.config.min_evidence,
            });
        }

        if counterevidence_confidence < self.config.min_counterevidence {
            failures.push(GateFailure {
                dimension: ConfidenceDimension::Counterevidence,
                value: counterevidence_confidence,
                threshold: self.config.min_counterevidence,
            });
        }

        if reasoning_confidence < self.config.min_reasoning {
            failures.push(GateFailure {
                dimension: ConfidenceDimension::Reasoning,
                value: reasoning_confidence,
                threshold: self.config.min_reasoning,
            });
        }

        if narrative_confidence < self.config.min_narrative {
            failures.push(GateFailure {
                dimension: ConfidenceDimension::Narrative,
                value: narrative_confidence,
                threshold: self.config.min_narrative,
            });
        }

        if failures.is_empty() {
            let split = SplitConfidence::new(
                evidence_confidence,
                counterevidence_confidence,
                reasoning_confidence,
                narrative_confidence,
            );
            GateDecision::Pass {
                evidence: evidence_confidence,
                counterevidence: counterevidence_confidence,
                reasoning: reasoning_confidence,
                narrative: narrative_confidence,
                aggregate: split.aggregate(),
            }
        } else {
            GateDecision::Blocked {
                evidence: evidence_confidence,
                counterevidence: counterevidence_confidence,
                reasoning: reasoning_confidence,
                narrative: narrative_confidence,
                failed: failures,
                penalized_rules,
            }
        }
    }

    /// Evaluate a full epistemic claim (with pre-computed split confidence).
    pub fn evaluate_claim(&self, claim: &EpistemicClaim) -> GateDecision {
        self.evaluate(
            claim.confidence.evidence,
            claim.confidence.counterevidence,
            &claim.reasoning,
            claim.confidence.narrative,
        )
    }

    pub fn config(&self) -> &GateConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ReasoningGraph;
    use crate::rules::{InferenceRule, RuleRegistry};

    fn make_graph(rule: InferenceRule, raw_confidence: f64) -> ReasoningGraph {
        let mut g = ReasoningGraph::new();
        let reg = RuleRegistry::new();
        let e = g.add_evidence(1);
        let root = g.add_conclusion(1, "test claim");
        g.add_step(e, rule, root, raw_confidence, &reg);
        g.set_root(root);
        g
    }

    #[test]
    fn pass_when_all_four_dimensions_above_threshold() {
        let gate = EpistemicGate::with_default();
        let g = make_graph(InferenceRule::ConvergentEvidence, 0.90);

        let decision = gate.evaluate(0.90, 0.85, &g, 0.80);
        assert!(matches!(decision, GateDecision::Pass { .. }));
    }

    #[test]
    fn block_when_counterevidence_below_threshold() {
        let gate = EpistemicGate::with_default();
        let g = make_graph(InferenceRule::ConvergentEvidence, 0.90);

        // counterevidence = 0.30 < 0.50 threshold
        let decision = gate.evaluate(0.90, 0.30, &g, 0.80);
        match decision {
            GateDecision::Blocked { failed, .. } => {
                assert!(failed
                    .iter()
                    .any(|f| f.dimension == ConfidenceDimension::Counterevidence));
            }
            _ => panic!("should be blocked on counterevidence"),
        }
    }

    #[test]
    fn block_when_reasoning_below_threshold() {
        let gate = EpistemicGate::with_default();
        let g = make_graph(InferenceRule::CentralityToMotivation, 0.94);

        let decision = gate.evaluate(0.94, 0.80, &g, 0.80);
        match decision {
            GateDecision::Blocked { failed, .. } => {
                assert!(failed
                    .iter()
                    .any(|f| f.dimension == ConfidenceDimension::Reasoning));
            }
            _ => panic!("should be blocked"),
        }
    }

    #[test]
    fn block_when_evidence_below_threshold() {
        let gate = EpistemicGate::with_default();
        let g = make_graph(InferenceRule::ConvergentEvidence, 0.90);

        let decision = gate.evaluate(0.30, 0.80, &g, 0.80);
        match decision {
            GateDecision::Blocked { failed, .. } => {
                assert!(failed
                    .iter()
                    .any(|f| f.dimension == ConfidenceDimension::Evidence));
            }
            _ => panic!("should be blocked"),
        }
    }

    #[test]
    fn claim_6_counterevidence_crushes_confidence() {
        // Simulating Claim 6: SharedActivityToRelationalBond
        // + counterevidence: resonance is to work, not relationship
        let gate = EpistemicGate::new(GateConfig::identity());
        let g = make_graph(InferenceRule::SharedActivityToRelationalBond, 0.82);

        // Counterevidence: weakening=0.80 (resonance is to milestones, not bond)
        let ce_links = vec![CounterevidenceLink {
            evidence_id: 601,
            weakening: 0.80,
            contradicts: "resonance is to milestones/work, not to relationship".into(),
        }];
        let ce_conf = SplitConfidence::compute_counterevidence(&ce_links);
        // 1.0 - 0.80 = 0.20

        let decision = gate.evaluate(0.82, ce_conf, &g, 0.68);
        match decision {
            GateDecision::Blocked {
                failed,
                penalized_rules,
                ..
            } => {
                // Should fail on BOTH reasoning AND counterevidence
                assert!(failed
                    .iter()
                    .any(|f| f.dimension == ConfidenceDimension::Reasoning));
                assert!(failed
                    .iter()
                    .any(|f| f.dimension == ConfidenceDimension::Counterevidence));
                assert!(penalized_rules.contains(&InferenceRule::SharedActivityToRelationalBond));
            }
            _ => panic!("Claim 6 should be blocked on multiple dimensions"),
        }
    }

    #[test]
    fn strict_gate_rejects_more() {
        let gate = EpistemicGate::new(GateConfig::strict());
        let g = make_graph(InferenceRule::ConvergentEvidence, 0.55);

        let decision = gate.evaluate(0.55, 0.50, &g, 0.60);
        assert!(matches!(decision, GateDecision::Blocked { .. }));
    }
}
