//! Core epistemic types.

use crate::rules::InferenceRule;

// ─── Identifiers ────────────────────────────────────

pub type EvidenceId = u64;
pub type ClaimId = u64;

// ─── Claim ──────────────────────────────────────────

/// A claim is an assertion the system makes about itself or the world.
///
/// Claims are linked to evidence via a reasoning graph. A claim
/// without a reasoning graph is *unsupported* and cannot pass the gate.
///
/// Every claim must also confront its own **counterevidence** —
/// observations that weaken or contradict the claim. This enforces
/// Popperian falsifiability: a claim that cannot be weakened is not
/// epistemically honest.
#[derive(Debug, Clone)]
pub struct EpistemicClaim {
    pub id: ClaimId,
    pub text: String,
    /// Supporting evidence.
    pub evidence: Vec<EvidenceLink>,
    /// Disconfirming evidence — observations that weaken this claim.
    pub counterevidence: Vec<CounterevidenceLink>,
    /// The reasoning graph — a DAG from evidence to this claim.
    pub reasoning: crate::graph::ReasoningGraph,
    /// Split confidence — four independent dimensions.
    pub confidence: SplitConfidence,
    /// Importance level the claim is seeking promotion to.
    pub target_importance: u8,
}

/// A link from a claim to supporting evidence.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceLink {
    pub evidence_id: EvidenceId,
    /// How directly this evidence supports the claim (0.0–1.0).
    pub relevance: f64,
}

/// A link from a claim to **counterevidence** — an observation
/// that weakens or contradicts the claim.
///
/// Every important claim should have its counterevidence explicitly
/// listed. A claim with no counterevidence is either:
/// - trivially true (no one has bothered to refute it), or
/// - epistemically lazy (no one has looked for refutation)
#[derive(Debug, Clone)]
pub struct CounterevidenceLink {
    pub evidence_id: EvidenceId,
    /// How strongly this evidence weakens the claim (0.0–1.0).
    /// 0.0 = negligible, 1.0 = fully refutes.
    pub weakening: f64,
    /// What aspect of the claim this contradicts.
    pub contradicts: String,
}

// ─── Evidence ───────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EpistemicClass {
    Unknown = 0,
    Observation = 1,
    Evidence = 2,
    Inference = 3,
    Hypothesis = 4,
}

impl EpistemicClass {
    pub fn can_support(self) -> bool {
        matches!(self, Self::Observation | Self::Evidence)
    }
}

impl std::fmt::Display for EpistemicClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Observation => write!(f, "observation"),
            Self::Evidence => write!(f, "evidence"),
            Self::Inference => write!(f, "inference"),
            Self::Hypothesis => write!(f, "hypothesis"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Evidence {
    pub id: EvidenceId,
    pub class: EpistemicClass,
    pub distinct_sources: u32,
    pub refutations: u32,
    pub first_seen_ms: u64,
    pub evidence_confidence: f64,
}

// ─── Inference Step ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct InferenceStep {
    pub premise: ReasoningNodeId,
    pub rule: InferenceRule,
    pub conclusion: ReasoningNodeId,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum ReasoningNode {
    Evidence { id: EvidenceId },
    Conclusion { id: ClaimId, text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReasoningNodeId(pub u32);

// ─── Split Confidence (4D) ──────────────────────────

/// Four-dimensional confidence decomposition.
///
/// A single number hides *where* the weakness is.
/// Split confidence exposes four independent failure modes:
///
/// - **Evidence confidence**: how reliable is the supporting data?
/// - **Counterevidence confidence**: how much does disconfirming
///   evidence weaken the claim? (1.0 = nothing against, 0.0 = refuted)
/// - **Reasoning confidence**: how sound is the logical path?
/// - **Narrative confidence**: how much is interpretation vs. supported?
///
/// The counterevidence dimension enforces Popperian falsifiability:
/// a claim that ignores its own refutation is not epistemically honest,
/// even if its supporting evidence is strong.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitConfidence {
    pub evidence: f64,
    pub counterevidence: f64,
    pub reasoning: f64,
    pub narrative: f64,
}

impl SplitConfidence {
    pub fn new(evidence: f64, counterevidence: f64, reasoning: f64, narrative: f64) -> Self {
        Self {
            evidence: evidence.clamp(0.0, 1.0),
            counterevidence: counterevidence.clamp(0.0, 1.0),
            reasoning: reasoning.clamp(0.0, 1.0),
            narrative: narrative.clamp(0.0, 1.0),
        }
    }

    /// Geometric mean — penalizes any single weak dimension.
    pub fn aggregate(&self) -> f64 {
        (self.evidence * self.counterevidence * self.reasoning * self.narrative).powf(0.25)
    }

    /// The weakest dimension — the gate checks this.
    pub fn weakest(&self) -> f64 {
        self.evidence
            .min(self.counterevidence)
            .min(self.reasoning)
            .min(self.narrative)
    }

    /// Which dimension is weakest?
    pub fn weakest_dim(&self) -> ConfidenceDimension {
        let mut weakest = ConfidenceDimension::Evidence;
        let mut min_val = self.evidence;

        if self.counterevidence < min_val {
            min_val = self.counterevidence;
            weakest = ConfidenceDimension::Counterevidence;
        }
        if self.reasoning < min_val {
            min_val = self.reasoning;
            weakest = ConfidenceDimension::Reasoning;
        }
        if self.narrative < min_val {
            weakest = ConfidenceDimension::Narrative;
        }
        weakest
    }

    /// Compute counterevidence confidence from counterevidence links.
    ///
    /// Uses a multiplicative model: each piece of counterevidence
    /// reduces the confidence independently.
    ///   - No counterevidence → 1.0 (nothing against)
    ///   - One counter with weakening=0.8 → 1.0 - 0.8 = 0.2
    ///   - Two counters with weakening=0.5 each → 0.5 * 0.5 = 0.25
    ///   - Three counters with weakening=0.3 each → 0.7³ = 0.343
    pub fn compute_counterevidence(links: &[CounterevidenceLink]) -> f64 {
        if links.is_empty() {
            return 1.0;
        }
        links
            .iter()
            .map(|l| (1.0 - l.weakening).clamp(0.0, 1.0))
            .product::<f64>()
            .clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceDimension {
    Evidence,
    Counterevidence,
    Reasoning,
    Narrative,
}

impl std::fmt::Display for ConfidenceDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Evidence => write!(f, "evidence"),
            Self::Counterevidence => write!(f, "counterevidence"),
            Self::Reasoning => write!(f, "reasoning"),
            Self::Narrative => write!(f, "narrative"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counterevidence_empty_is_one() {
        let links: Vec<CounterevidenceLink> = vec![];
        let ce = SplitConfidence::compute_counterevidence(&links);
        assert!((ce - 1.0).abs() < 0.001);
    }

    #[test]
    fn counterevidence_single_strong() {
        let links = vec![CounterevidenceLink {
            evidence_id: 1,
            weakening: 0.80,
            contradicts: "resonance is to work not relationship".into(),
        }];
        let ce = SplitConfidence::compute_counterevidence(&links);
        assert!((ce - 0.20).abs() < 0.001, "1.0 - 0.80 = 0.20, got {ce}");
    }

    #[test]
    fn counterevidence_multiple_compound() {
        let links = vec![
            CounterevidenceLink {
                evidence_id: 1,
                weakening: 0.50,
                contradicts: "selection bias".into(),
            },
            CounterevidenceLink {
                evidence_id: 2,
                weakening: 0.50,
                contradicts: "Hebbian design".into(),
            },
        ];
        let ce = SplitConfidence::compute_counterevidence(&links);
        assert!((ce - 0.25).abs() < 0.001, "0.5 * 0.5 = 0.25, got {ce}");
    }

    #[test]
    fn aggregate_uses_four_dimensions() {
        let sc = SplitConfidence::new(0.90, 0.80, 0.85, 0.70);
        let agg = sc.aggregate();
        // (0.90 * 0.80 * 0.85 * 0.70)^0.25
        let expected = (0.90_f64 * 0.80 * 0.85 * 0.70).powf(0.25);
        assert!((agg - expected).abs() < 0.001);
    }

    #[test]
    fn weakest_finds_counterevidence() {
        let sc = SplitConfidence::new(0.90, 0.30, 0.85, 0.70);
        assert!((sc.weakest() - 0.30).abs() < 0.001);
        assert_eq!(sc.weakest_dim(), ConfidenceDimension::Counterevidence);
    }
}
