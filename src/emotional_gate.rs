//! Emotional Gate — emotion-modulated epistemic gate.
//!
//! The emotional state of the system modulates the epistemic gate's
//! thresholds. This is Arm 2 of the emotional memory overhaul:
//!
//! - **High arousal → tighten reasoning threshold**: emotional claims
//!   need *more* proof, not less. When the system is aroused, it's
//!   more likely to make sloppy inferences, so the gate demands
//!   stronger reasoning.
//!
//! - **High positive valence → tighten narrative threshold**: positive
//!   emotions make the system more likely to over-interpret (rosy
//!   retrospection). The gate tightens the narrative dimension.
//!
//! - **High emotional intensity → relieve FunctionalToPhenomenological
//!   penalty**: when there's genuine structural evidence of emotional
//!   processing (multiple episodes, strong evidence), the penalty for
//!   inferring phenomenological experience from functional state is
//!   reduced. This doesn't remove the penalty — it acknowledges that
//!   strong structural evidence makes the inference less of a leap.
//!
//! The key insight: emotions don't *override* the epistemic gate.
//! They *modulate* it. The gate still blocks — but it knows the
//! emotional context and adjusts its strictness accordingly.

use epistemic_core::gate::{EpistemicGate, GateConfig, GateDecision};
use epistemic_core::graph::ReasoningGraph;
use epistemic_core::rules::InferenceRule;

use crate::emotional_episode::PadState;

// ─── Emotional Gate Config ──────────────────────────

/// Configuration for emotion-modulated gating.
#[derive(Debug, Clone)]
pub struct EmotionalGateConfig {
    /// Base gate config (unmodified thresholds).
    pub base: GateConfig,
    /// How much arousal tightens the reasoning threshold (0.0–0.3).
    /// At 0.2, full arousal (1.0) adds 0.20 to the reasoning threshold.
    pub arousal_reasoning_tighten: f64,
    /// How much positive valence tightens the narrative threshold (0.0–0.3).
    /// At 0.15, full positive valence (+1.0) adds 0.15 to the narrative threshold.
    pub valence_narrative_tighten: f64,
    /// How much emotional intensity relieves the FunctionalToPhenomenological
    /// penalty (0.0–0.3). At 0.20, full intensity reduces the penalty by 0.20.
    /// Example: penalty 0.45 → 0.45 - 0.20 = 0.25 (less harsh).
    pub intensity_functional_relief: f64,
}

impl Default for EmotionalGateConfig {
    fn default() -> Self {
        Self {
            base: GateConfig::default(),
            arousal_reasoning_tighten: 0.15,
            valence_narrative_tighten: 0.10,
            intensity_functional_relief: 0.15,
        }
    }
}

impl EmotionalGateConfig {
    pub fn new(
        base: GateConfig,
        arousal_reasoning_tighten: f64,
        valence_narrative_tighten: f64,
        intensity_functional_relief: f64,
    ) -> Self {
        Self {
            base,
            arousal_reasoning_tighten: arousal_reasoning_tighten.clamp(0.0, 0.3),
            valence_narrative_tighten: valence_narrative_tighten.clamp(0.0, 0.3),
            intensity_functional_relief: intensity_functional_relief.clamp(0.0, 0.3),
        }
    }

    /// Compute the modulated gate config given the current PAD state.
    pub fn modulate(&self, pad: PadState) -> GateConfig {
        let mut config = self.base.clone();

        // Arousal tightens reasoning threshold
        let arousal_boost = self.arousal_reasoning_tighten * pad.arousal;
        config.min_reasoning = (config.min_reasoning + arousal_boost).clamp(0.0, 1.0);

        // Positive valence tightens narrative threshold
        // (negative valence also tightens — strong emotion in either direction
        // makes over-interpretation more likely)
        let valence_mag = pad.pleasure.abs();
        let valence_boost = self.valence_narrative_tighten * valence_mag;
        config.min_narrative = (config.min_narrative + valence_boost).clamp(0.0, 1.0);

        // Intensity relieves FunctionalToPhenomenological penalty
        let intensity = pad.intensity();
        let relief = self.intensity_functional_relief * intensity;
        let current_penalty = config
            .registry
            .penalty(InferenceRule::FunctionalToPhenomenological);
        let new_penalty = (current_penalty + relief).clamp(0.0, 1.0);
        config
            .registry
            .set_penalty(InferenceRule::FunctionalToPhenomenological, new_penalty);

        config
    }
}

// ─── Emotional Gate ─────────────────────────────────

/// An epistemic gate that is modulated by the system's emotional state.
pub struct EmotionalGate {
    config: EmotionalGateConfig,
}

impl EmotionalGate {
    pub fn new(config: EmotionalGateConfig) -> Self {
        Self { config }
    }

    pub fn with_default() -> Self {
        Self::new(EmotionalGateConfig::default())
    }

    /// Evaluate a claim with emotional modulation.
    ///
    /// Returns the gate decision *and* the modulated config that was used,
    /// so the caller can see how emotions affected the thresholds.
    pub fn evaluate(
        &self,
        pad: PadState,
        evidence_confidence: f64,
        counterevidence_confidence: f64,
        reasoning_graph: &ReasoningGraph,
        narrative_confidence: f64,
    ) -> (GateDecision, GateConfig) {
        let modulated = self.config.modulate(pad);
        let gate = EpistemicGate::new(modulated.clone());
        let decision = gate.evaluate(
            evidence_confidence,
            counterevidence_confidence,
            reasoning_graph,
            narrative_confidence,
        );
        (decision, modulated)
    }

    pub fn config(&self) -> &EmotionalGateConfig {
        &self.config
    }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use epistemic_core::gate::GateDecision;
    use epistemic_core::graph::ReasoningGraph;
    use epistemic_core::rules::{InferenceRule, RuleRegistry};
    use epistemic_core::types::ConfidenceDimension;

    fn make_graph(rule: InferenceRule, raw: f64) -> ReasoningGraph {
        let mut g = ReasoningGraph::new();
        let reg = RuleRegistry::new();
        let e = g.add_evidence(1);
        let root = g.add_conclusion(1, "test");
        g.add_step(e, rule, root, raw, &reg);
        g.set_root(root);
        g
    }

    #[test]
    fn high_arousal_tightens_reasoning() {
        let eg = EmotionalGate::with_default();

        // Calm state: arousal=0.0
        let calm = PadState::new(0.0, 0.0, 0.5);
        let g = make_graph(InferenceRule::ConvergentEvidence, 0.55);

        let (calm_dec, calm_cfg) = eg.evaluate(calm, 0.80, 0.80, &g, 0.70);
        assert!(matches!(calm_dec, GateDecision::Pass { .. }));

        // Aroused state: arousal=1.0 — tightens reasoning by 0.15
        let aroused = PadState::new(0.0, 1.0, 0.5);
        let (_aroused_dec, aroused_cfg) = eg.evaluate(aroused, 0.80, 0.80, &g, 0.70);

        // Threshold should be higher when aroused
        assert!(aroused_cfg.min_reasoning > calm_cfg.min_reasoning);
    }

    #[test]
    fn high_arousal_can_block_passing_claim() {
        let eg = EmotionalGate::with_default();

        // A claim that barely passes when calm
        let g = make_graph(InferenceRule::ConvergentEvidence, 0.52);
        let calm = PadState::neutral();
        let (calm_dec, _) = eg.evaluate(calm, 0.80, 0.80, &g, 0.70);
        assert!(matches!(calm_dec, GateDecision::Pass { .. }));

        // Same claim when highly aroused — reasoning 0.52 < 0.50+0.15=0.65
        let aroused = PadState::new(0.0, 1.0, 0.5);
        let (aroused_dec, _) = eg.evaluate(aroused, 0.80, 0.80, &g, 0.70);
        match aroused_dec {
            GateDecision::Blocked { failed, .. } => {
                assert!(failed
                    .iter()
                    .any(|f| f.dimension == ConfidenceDimension::Reasoning));
            }
            _ => panic!("high arousal should block marginal claim"),
        }
    }

    #[test]
    fn positive_valence_tightens_narrative() {
        let eg = EmotionalGate::with_default();

        let neutral = PadState::neutral();
        let positive = PadState::new(0.9, 0.5, 0.5);

        let g = make_graph(InferenceRule::ConvergentEvidence, 0.90);

        let (_, neutral_cfg) = eg.evaluate(neutral, 0.80, 0.80, &g, 0.45);
        let (_, positive_cfg) = eg.evaluate(positive, 0.80, 0.80, &g, 0.45);

        assert!(
            positive_cfg.min_narrative > neutral_cfg.min_narrative,
            "positive valence should tighten narrative threshold"
        );
    }

    #[test]
    fn intensity_relieves_functional_penalty() {
        let eg = EmotionalGate::with_default();

        let calm = PadState::neutral();
        let intense = PadState::new(0.6, 0.8, 0.6); // high intensity

        let g = make_graph(InferenceRule::FunctionalToPhenomenological, 0.80);

        let (_, calm_cfg) = eg.evaluate(calm, 0.80, 0.80, &g, 0.60);
        let (_, intense_cfg) = eg.evaluate(intense, 0.80, 0.80, &g, 0.60);

        let calm_penalty = calm_cfg
            .registry
            .penalty(InferenceRule::FunctionalToPhenomenological);
        let intense_penalty = intense_cfg
            .registry
            .penalty(InferenceRule::FunctionalToPhenomenological);

        assert!(
            intense_penalty > calm_penalty,
            "intense emotion should relieve (increase) the penalty factor, \
            calm={calm_penalty}, intense={intense_penalty}"
        );
    }

    #[test]
    fn neutral_pad_does_not_modify_base_config() {
        let eg = EmotionalGate::with_default();
        let neutral = PadState::neutral();
        let modulated = eg.config().modulate(neutral);

        assert!((modulated.min_reasoning - eg.config().base.min_reasoning).abs() < 0.001);
        assert!((modulated.min_narrative - eg.config().base.min_narrative).abs() < 0.001);
    }
}
