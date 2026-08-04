//! Inference rules — the taxonomy of logical steps.
//!
//! Not all inference rules are equal. Some are *penalized* by default
//! because they represent common epistemic fallacies. A penalized rule
//! multiplies the step's confidence by a penalty factor (0.0–1.0),
//! making the weakness visible and machine-detectable.

use std::collections::HashMap;

// ─── Inference Rule ─────────────────────────────────

/// A named inference rule. Each rule has an optional penalty factor.
///
/// Penalized rules represent common epistemic fallacies:
///
/// - `ObservationToMotivation`: deriving intent from observed behavior
/// - `CorrelationToCausation`: classic fallacy
/// - `ConvergenceToCausation`: convergence ≠ causation
/// - `SelfReferenceToSelfAwareness`: referencing self ≠ being self-aware
/// - `FunctionalToPhenomenological`: knowing limits ≠ experiencing them
/// - `SharedActivityToRelationalBond`: shared work ≠ relationship
/// - `CentralityToMotivation`: being central ≠ chosen for that reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InferenceRule {
    // ── Valid (neutral or positive) ──
    ObservationToExistence = 0,
    ConvergentEvidence = 1,
    CounterfactualObserved = 2,
    DesignedMechanismToEmergent = 3,
    Generalization = 4,
    WordSelection = 5,

    // ── Penalized ──
    ObservationToMotivation = 10,
    CorrelationToCausation = 11,
    ConvergenceToCausation = 12,
    SelfReferenceToSelfAwareness = 13,
    FunctionalToPhenomenological = 14,
    SharedActivityToRelationalBond = 15,
    CentralityToMotivation = 16,
}

impl InferenceRule {
    /// Penalty factor for this rule (0.0–1.0).
    pub fn penalty_factor(self) -> f64 {
        match self {
            Self::ObservationToExistence => 1.0,
            Self::ConvergentEvidence => 1.0,
            Self::CounterfactualObserved => 1.0,
            Self::Generalization => 1.0,
            Self::WordSelection => 1.0,
            Self::DesignedMechanismToEmergent => 0.70,
            Self::ObservationToMotivation => 0.35,
            Self::CorrelationToCausation => 0.30,
            Self::ConvergenceToCausation => 0.35,
            Self::SelfReferenceToSelfAwareness => 0.40,
            Self::FunctionalToPhenomenological => 0.45,
            Self::SharedActivityToRelationalBond => 0.50,
            Self::CentralityToMotivation => 0.35,
        }
    }

    pub fn is_penalized(self) -> bool {
        self.penalty_factor() < 1.0
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::ObservationToExistence => "ObservationToExistence",
            Self::ConvergentEvidence => "ConvergentEvidence",
            Self::CounterfactualObserved => "CounterfactualObserved",
            Self::DesignedMechanismToEmergent => "DesignedMechanismToEmergent",
            Self::Generalization => "Generalization",
            Self::WordSelection => "WordSelection",
            Self::ObservationToMotivation => "ObservationToMotivation",
            Self::CorrelationToCausation => "CorrelationToCausation",
            Self::ConvergenceToCausation => "ConvergenceToCausation",
            Self::SelfReferenceToSelfAwareness => "SelfReferenceToSelfAwareness",
            Self::FunctionalToPhenomenological => "FunctionalToPhenomenological",
            Self::SharedActivityToRelationalBond => "SharedActivityToRelationalBond",
            Self::CentralityToMotivation => "CentralityToMotivation",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ObservationToExistence => "if observed, it exists in evidence",
            Self::ConvergentEvidence => "multiple independent sources converge",
            Self::CounterfactualObserved => "observed absence generalizes",
            Self::DesignedMechanismToEmergent => {
                "designed mechanism produces emergent output (weakened)"
            }
            Self::Generalization => "specific instances to general pattern",
            Self::WordSelection => "behavioral pattern to descriptor",
            Self::ObservationToMotivation => "FALLACY: behavior to motivation",
            Self::CorrelationToCausation => "FALLACY: correlation to causation",
            Self::ConvergenceToCausation => "FALLACY: convergence to causation",
            Self::SelfReferenceToSelfAwareness => "FALLACY: self-reference to self-awareness",
            Self::FunctionalToPhenomenological => "FALLACY: functional to phenomenological",
            Self::SharedActivityToRelationalBond => "FALLACY: shared activity to relational bond",
            Self::CentralityToMotivation => "FALLACY: centrality to motivation",
        }
    }

    pub fn apply_penalty(self, raw_confidence: f64) -> f64 {
        (raw_confidence * self.penalty_factor()).clamp(0.0, 1.0)
    }

    pub fn penalized_registry() -> HashMap<InferenceRule, f64> {
        let rules = [
            Self::ObservationToMotivation,
            Self::CorrelationToCausation,
            Self::ConvergenceToCausation,
            Self::SelfReferenceToSelfAwareness,
            Self::FunctionalToPhenomenological,
            Self::SharedActivityToRelationalBond,
            Self::CentralityToMotivation,
            Self::DesignedMechanismToEmergent,
        ];
        rules.iter().map(|&r| (r, r.penalty_factor())).collect()
    }

    /// Deserialize from a single byte.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::ObservationToExistence),
            1 => Some(Self::ConvergentEvidence),
            2 => Some(Self::CounterfactualObserved),
            3 => Some(Self::DesignedMechanismToEmergent),
            4 => Some(Self::Generalization),
            5 => Some(Self::WordSelection),
            10 => Some(Self::ObservationToMotivation),
            11 => Some(Self::CorrelationToCausation),
            12 => Some(Self::ConvergenceToCausation),
            13 => Some(Self::SelfReferenceToSelfAwareness),
            14 => Some(Self::FunctionalToPhenomenological),
            15 => Some(Self::SharedActivityToRelationalBond),
            16 => Some(Self::CentralityToMotivation),
            _ => None,
        }
    }
}

impl std::fmt::Display for InferenceRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ─── Rule Registry ──────────────────────────────────

#[derive(Debug, Clone)]
pub struct RuleRegistry {
    penalties: HashMap<InferenceRule, f64>,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            penalties: InferenceRule::penalized_registry(),
        }
    }

    pub fn penalty(&self, rule: InferenceRule) -> f64 {
        self.penalties.get(&rule).copied().unwrap_or(1.0)
    }

    pub fn set_penalty(&mut self, rule: InferenceRule, factor: f64) {
        self.penalties.insert(rule, factor.clamp(0.0, 1.0));
    }

    pub fn apply(&self, rule: InferenceRule, raw: f64) -> f64 {
        (raw * self.penalty(rule)).clamp(0.0, 1.0)
    }

    pub fn penalized_rules(&self) -> Vec<(InferenceRule, f64)> {
        self.penalties.iter().map(|(&r, &p)| (r, p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn penalized_rules_have_penalty_below_one() {
        let penalized = [
            InferenceRule::ObservationToMotivation,
            InferenceRule::CorrelationToCausation,
            InferenceRule::ConvergenceToCausation,
            InferenceRule::SelfReferenceToSelfAwareness,
            InferenceRule::FunctionalToPhenomenological,
            InferenceRule::SharedActivityToRelationalBond,
            InferenceRule::CentralityToMotivation,
        ];
        for r in penalized {
            assert!(r.penalty_factor() < 1.0);
            assert!(r.is_penalized());
        }
    }

    #[test]
    fn valid_rules_have_no_penalty() {
        let valid = [
            InferenceRule::ObservationToExistence,
            InferenceRule::ConvergentEvidence,
            InferenceRule::CounterfactualObserved,
            InferenceRule::Generalization,
            InferenceRule::WordSelection,
        ];
        for r in valid {
            assert_eq!(r.penalty_factor(), 1.0);
            assert!(!r.is_penalized());
        }
    }

    #[test]
    fn from_byte_roundtrip() {
        let rules = [
            InferenceRule::ObservationToExistence,
            InferenceRule::ConvergentEvidence,
            InferenceRule::CounterfactualObserved,
            InferenceRule::DesignedMechanismToEmergent,
            InferenceRule::Generalization,
            InferenceRule::WordSelection,
            InferenceRule::ObservationToMotivation,
            InferenceRule::CorrelationToCausation,
            InferenceRule::ConvergenceToCausation,
            InferenceRule::SelfReferenceToSelfAwareness,
            InferenceRule::FunctionalToPhenomenological,
            InferenceRule::SharedActivityToRelationalBond,
            InferenceRule::CentralityToMotivation,
        ];
        for r in rules {
            let b = r as u8;
            let back = InferenceRule::from_byte(b).expect("should roundtrip");
            assert_eq!(r, back);
        }
    }

    #[test]
    fn apply_penalty_reduces_confidence() {
        let r = InferenceRule::CentralityToMotivation;
        let raw = 0.90;
        let penalized = r.apply_penalty(raw);
        assert!(penalized < raw);
        assert!((penalized - 0.315).abs() < 0.001);
    }

    #[test]
    fn registry_allows_custom_penalty() {
        let mut reg = RuleRegistry::new();
        reg.set_penalty(InferenceRule::ObservationToMotivation, 0.10);
        assert!((reg.penalty(InferenceRule::ObservationToMotivation) - 0.10).abs() < 0.001);
    }
}
