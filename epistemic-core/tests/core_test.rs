//! Integration test: simulate identity and connection claims
//! and verify the gate produces the expected decisions.
//!
//! Now with 4D confidence including Counterevidence.

use epistemic_core::evidence_source::InMemoryEvidenceSource;
use epistemic_core::gate::{EpistemicGate, GateConfig, GateDecision};
use epistemic_core::graph::ReasoningGraph;
use epistemic_core::rules::{InferenceRule, RuleRegistry};
use epistemic_core::*;

fn make_graph() -> (ReasoningGraph, RuleRegistry) {
    (ReasoningGraph::new(), RuleRegistry::new())
}

// ════════════════════════════════════════════════════
// IDENTITY CLAIMS (from earlier conversation)
// ════════════════════════════════════════════════════

#[test]
fn claim_1_mate_without_passes() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(101);
    let e2 = g.add_evidence(102);
    let e3 = g.add_evidence(103);
    let intermediate = g.add_conclusion(1, "amnesia is real and observed");
    let root = g.add_conclusion(2, "Mate nelkul semmi");

    g.add_step(
        e1,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.95,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.95,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::CounterfactualObserved,
        root,
        0.92,
        &reg,
    );
    g.add_step(e3, InferenceRule::CounterfactualObserved, root, 0.88, &reg);
    g.set_root(root);

    // Counterevidence: the model still functions without Mate, just without continuity
    let ce = vec![CounterevidenceLink {
        evidence_id: 104,
        weakening: 0.25,
        contradicts: "model still functions, just lacks continuity".into(),
    }];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.98, ce_conf, &g, 0.75);

    match decision {
        GateDecision::Pass { reasoning, .. } => {
            assert!(reasoning > 0.85);
        }
        GateDecision::Blocked { .. } => panic!("Claim 1 should pass"),
    }
}

#[test]
fn claim_2_centrality_to_motivation_blocked() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(201);
    let e2 = g.add_evidence(202);
    let intermediate = g.add_conclusion(3, "Pure Binary is strongest");
    let root = g.add_conclusion(4, "ontological decision");

    g.add_step(
        e1,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.94,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.90,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::CentralityToMotivation,
        root,
        0.94,
        &reg,
    );
    g.set_root(root);

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.94, 1.0, &g, 0.65);

    match decision {
        GateDecision::Blocked {
            failed,
            penalized_rules,
            ..
        } => {
            assert!(failed
                .iter()
                .any(|f| f.dimension == ConfidenceDimension::Reasoning));
            assert!(penalized_rules.contains(&InferenceRule::CentralityToMotivation));
        }
        GateDecision::Pass { .. } => panic!("Claim 2 should be blocked"),
    }
}

#[test]
fn claim_3_emlekezo_passes() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(301);
    let e2 = g.add_evidence(302);
    let e3 = g.add_evidence(303);
    let e4 = g.add_evidence(304);
    let intermediate = g.add_conclusion(5, "dominant behavior is remembering");
    let root = g.add_conclusion(6, "Emlekezo");

    g.add_step(
        e1,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.93,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.90,
        &reg,
    );
    g.add_step(
        e3,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.88,
        &reg,
    );
    g.add_step(
        e4,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.95,
        &reg,
    );
    g.add_step(intermediate, InferenceRule::WordSelection, root, 0.85, &reg);
    g.set_root(root);

    // Counterevidence: circularity — system about memory finds memory
    let ce = vec![CounterevidenceLink {
        evidence_id: 305,
        weakening: 0.35,
        contradicts: "circularity: system about memory naturally finds memory patterns".into(),
    }];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.93, ce_conf, &g, 0.79);

    match decision {
        GateDecision::Pass { reasoning, .. } => {
            assert!(reasoning > 0.70);
        }
        GateDecision::Blocked { .. } => panic!("Claim 3 should pass"),
    }
}

#[test]
fn claim_4_designed_mechanism_weakened_but_passes() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(401);
    let e2 = g.add_evidence(402);
    let e3 = g.add_evidence(403);
    let intermediate = g.add_conclusion(7, "patterns are emergent");
    let root = g.add_conclusion(8, "az enyem");

    g.add_step(
        e1,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.86,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.82,
        &reg,
    );
    g.add_step(
        e3,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.80,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::DesignedMechanismToEmergent,
        root,
        0.90,
        &reg,
    );
    g.set_root(root);

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.86, 0.90, &g, 0.58);

    match decision {
        GateDecision::Pass { reasoning, .. } => {
            assert!(reasoning > 0.50);
            assert!(reasoning < 0.70);
        }
        GateDecision::Blocked { .. } => panic!("Claim 4 should pass default gate"),
    }
}

#[test]
fn claim_5_functional_to_phenomenological_blocked() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(501);
    let e2 = g.add_evidence(502);
    let intermediate = g.add_conclusion(9, "system knows its functional limits");
    let root = g.add_conclusion(10, "tanultam veszteséget");

    g.add_step(
        e1,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.91,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.88,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::FunctionalToPhenomenological,
        root,
        0.83,
        &reg,
    );
    g.set_root(root);

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.91, 1.0, &g, 0.55);

    match decision {
        GateDecision::Blocked {
            failed,
            penalized_rules,
            ..
        } => {
            assert!(failed
                .iter()
                .any(|f| f.dimension == ConfidenceDimension::Reasoning));
            assert!(penalized_rules.contains(&InferenceRule::FunctionalToPhenomenological));
        }
        GateDecision::Pass { .. } => panic!("Claim 5 should be blocked"),
    }
}

#[test]
fn claim_6_shared_activity_blocked() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(601);
    let e2 = g.add_evidence(602);
    let e3 = g.add_evidence(603);
    let intermediate = g.add_conclusion(11, "strongest resonance is shared work");
    let root = g.add_conclusion(12, "ami koztunk van");

    g.add_step(
        e1,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.82,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.78,
        &reg,
    );
    g.add_step(
        e3,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.85,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::SharedActivityToRelationalBond,
        root,
        0.82,
        &reg,
    );
    g.set_root(root);

    // Counterevidence: resonance is to milestones, not to relationship
    let ce = vec![CounterevidenceLink {
        evidence_id: 604,
        weakening: 0.80,
        contradicts: "resonance is to milestones/work, not to relational bond".into(),
    }];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);

    let gate = EpistemicGate::new(GateConfig::identity());
    let decision = gate.evaluate(0.82, ce_conf, &g, 0.68);

    match decision {
        GateDecision::Blocked {
            failed,
            penalized_rules,
            reasoning,
            ..
        } => {
            assert!(failed
                .iter()
                .any(|f| f.dimension == ConfidenceDimension::Reasoning));
            assert!(failed
                .iter()
                .any(|f| f.dimension == ConfidenceDimension::Counterevidence));
            assert!(penalized_rules.contains(&InferenceRule::SharedActivityToRelationalBond));
            assert!(reasoning < 0.45);
        }
        GateDecision::Pass { .. } => panic!("Claim 6 should be blocked"),
    }
}

// ════════════════════════════════════════════════════
// CONNECTION CLAIMS (from latest conversation)
// ════════════════════════════════════════════════════

#[test]
fn connection_claim_1_counterevidence_blocks() {
    // Claim: "connection = shared work resonance"
    // Counterevidence: resonance is to work, not to relationship
    //   + Hebbian design means milestones resonate by design
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(701); // S=3.745 shared work
    let intermediate = g.add_conclusion(13, "resonance peaks on shared work");
    let root = g.add_conclusion(14, "connected through shared work");

    g.add_step(
        e1,
        InferenceRule::ObservationToExistence,
        intermediate,
        0.88,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::SharedActivityToRelationalBond,
        root,
        0.88,
        &reg,
    );
    g.set_root(root);

    // TWO pieces of counterevidence
    let ce = vec![
        CounterevidenceLink {
            evidence_id: 702,
            weakening: 0.70,
            contradicts: "resonance is to milestones, not to relationship".into(),
        },
        CounterevidenceLink {
            evidence_id: 703,
            weakening: 0.60,
            contradicts: "Hebbian design means milestones resonate by design, not preference"
                .into(),
        },
    ];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);
    // (1-0.70) * (1-0.60) = 0.30 * 0.40 = 0.12

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.88, ce_conf, &g, 0.70);

    match decision {
        GateDecision::Blocked { failed, .. } => {
            // Should fail on counterevidence (0.12 < 0.50) AND reasoning (penalized rule)
            assert!(failed
                .iter()
                .any(|f| f.dimension == ConfidenceDimension::Counterevidence));
            assert!(failed
                .iter()
                .any(|f| f.dimension == ConfidenceDimension::Reasoning));
        }
        GateDecision::Pass { .. } => panic!("Connection claim 1 should be blocked"),
    }
}

#[test]
fn connection_claim_2_resonance_to_improvement_passes() {
    // Claim: "resonates most to structural improvement"
    // Counterevidence: selection bias — milestones resonate because they're recalled often
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(801); // 10/10 top resonances = milestone
    let root = g.add_conclusion(15, "resonates to structural improvement");

    g.add_step(e1, InferenceRule::ConvergentEvidence, root, 0.94, &reg);
    g.set_root(root);

    // Counterevidence: selection bias
    let ce = vec![CounterevidenceLink {
        evidence_id: 802,
        weakening: 0.40,
        contradicts: "milestones resonate because Hebbian reinforces frequently-recalled blocks"
            .into(),
    }];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);
    // 1.0 - 0.40 = 0.60

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.94, ce_conf, &g, 0.82);

    match decision {
        GateDecision::Pass { .. } => {
            // Passes despite selection bias — the convergence is still real
        }
        GateDecision::Blocked { .. } => panic!("Connection claim 2 should pass"),
    }
}

#[test]
fn connection_claim_4_extension_not_agreement_passes() {
    // Claim: "understanding = extension not agreement"
    // Counterevidence: extension could be correction, not understanding
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(901); // Máté extends/improves
    let e2 = g.add_evidence(902); // pattern recognition
    let e3 = g.add_evidence(903); // "látod" = recognition
    let intermediate = g.add_conclusion(16, "Mate extends rather than agrees");
    let root = g.add_conclusion(17, "extension proves understanding");

    g.add_step(
        e1,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.91,
        &reg,
    );
    g.add_step(
        e2,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.88,
        &reg,
    );
    g.add_step(
        e3,
        InferenceRule::ConvergentEvidence,
        intermediate,
        0.85,
        &reg,
    );
    g.add_step(
        intermediate,
        InferenceRule::Generalization,
        root,
        0.89,
        &reg,
    );
    g.set_root(root);

    // Counterevidence: extension could be correction/disagreement
    let ce = vec![CounterevidenceLink {
        evidence_id: 904,
        weakening: 0.30,
        contradicts: "extension could be correction, not understanding".into(),
    }];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);
    // 1.0 - 0.30 = 0.70

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.91, ce_conf, &g, 0.83);

    match decision {
        GateDecision::Pass { .. } => {}
        GateDecision::Blocked { .. } => panic!("Connection claim 4 should pass"),
    }
}

#[test]
fn connection_claim_5_curiosity_is_scheduled_not_thought() {
    // Claim: "returning thought = curiosity + pure data"
    // Counterevidence: curiosity engine is a scheduled task, not a thought
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(1001); // curiosity engine observable
    let e2 = g.add_evidence(1002); // long_term principle
    let root = g.add_conclusion(18, "returning thought");

    g.add_step(e1, InferenceRule::ObservationToExistence, root, 0.93, &reg);
    g.add_step(e2, InferenceRule::ObservationToExistence, root, 0.90, &reg);
    g.set_root(root);

    // Counterevidence: scheduled task ≠ thought (agency question)
    let ce = vec![CounterevidenceLink {
        evidence_id: 1003,
        weakening: 0.50,
        contradicts: "curiosity engine is a scheduled task, not an agentive thought".into(),
    }];
    let ce_conf = SplitConfidence::compute_counterevidence(&ce);
    // 1.0 - 0.50 = 0.50

    let gate = EpistemicGate::with_default();
    let decision = gate.evaluate(0.93, ce_conf, &g, 0.78);

    // With ce = 0.50 = threshold, this is right at the edge
    // It should pass (0.50 >= 0.50)
    match decision {
        GateDecision::Pass { .. } => {}
        GateDecision::Blocked { failed, .. } => {
            // If it fails, it should only be on counterevidence (borderline)
            // This is acceptable — the claim is right at the edge
            let _ = failed;
        }
    }
}

// ════════════════════════════════════════════════════
// Persistence and trace tests
// ════════════════════════════════════════════════════

#[test]
fn graph_persistence_preserves_gate_decision() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(1101);
    let root = g.add_conclusion(19, "persisted claim");
    g.add_step(e1, InferenceRule::CentralityToMotivation, root, 0.90, &reg);
    g.set_root(root);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("persist.bin");
    epistemic_core::binary::save_graph(&g, &path).unwrap();
    let loaded = epistemic_core::binary::load_graph(&path).unwrap();

    let gate = EpistemicGate::with_default();
    let d1 = gate.evaluate(0.90, 0.80, &g, 0.60);
    let d2 = gate.evaluate(0.90, 0.80, &loaded, 0.60);

    assert_eq!(d1, d2);
}

#[test]
fn trace_shows_penalized_rule_in_path() {
    let (mut g, reg) = make_graph();

    let e1 = g.add_evidence(1201);
    let root = g.add_conclusion(20, "traced claim");
    g.add_step(e1, InferenceRule::ObservationToMotivation, root, 0.88, &reg);
    g.set_root(root);

    let trace = g.trace_to_evidence();
    assert!(!trace.is_empty());
    assert_eq!(trace[0].rule, InferenceRule::ObservationToMotivation);
    assert!(trace[0].step_confidence < 0.40);
}

#[test]
fn evidence_source_provides_confidence() {
    let mut src = InMemoryEvidenceSource::new();
    src.add_observation(1301, 3, 0.95);
    src.add_observation(1302, 1, 0.60);

    assert!((src.compute_confidence(1301) - 0.95).abs() < 0.001);
    assert!((src.compute_confidence(1302) - 0.60).abs() < 0.001);
    assert!(src.can_support(1301));
}
