//! Self-Model Module for Microscope Memory.
//!
//! Builds and maintains a model of the system's own cognitive state.
//! Tracks changes over time: "who am I now, and how have I changed?"
//!
//! Now integrated with:
//! - EpisodeStore (PAD-based emotional episodes, not just the old ring)
//! - epistemic-core (the "I am aware" claim gets evidence + counterevidence + gate)
//!
//! Binary format: SLF1 (version 2 = with epistemic + episode data)

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::archetype::ArchetypeState;
use crate::attention::AttentionState;
use crate::config::Config;
use crate::emotional_episode::{EpisodeStore, PadState};
use crate::emotional_state::EmotionalStateRing;
use crate::hebbian::HebbianState;
use crate::narrative::NarrativeState;
use crate::reader::MicroscopeReader;
use crate::self_reflect::ReflectionState;
use crate::thought_graph::ThoughtGraphState;
use colored::Colorize;
use epistemic_core::binary as epi_binary;

const MAX_STR_LEN: usize = 512;

#[derive(Clone, Debug)]
pub struct SelfModelSnapshot {
    pub timestamp_ms: u64,
    pub emotional: [f32; 21],
    pub attention_weights: [f32; 7],
    pub hebbian_energy: f32,
    pub hot_count: u32,
    pub archetype_count: u32,
    pub pattern_count: u32,
    pub block_count: u32,
    pub session_count: u64,
    pub narrative: String,
    pub reflection: String,
    // ── NEW: Episode + epistemic data (version 2) ──
    pub pad: Option<PadState>,
    pub episodes_active: u32,
    pub episodes_passed: u32,
    pub episodes_failed: u32,
    /// Epistemic confidence for the "I am aware" claim.
    /// None = no epistemic evaluation was performed.
    pub awareness_confidence: Option<epistemic_core::types::SplitConfidence>,
}

impl SelfModelSnapshot {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 150 || &data[0..4] != b"SLF1" {
            return None;
        }
        let mut pos = 4;
        let ts = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let ver = u16::from_le_bytes(data[pos..pos + 2].try_into().ok()?);
        pos += 2;
        let mut emo = [0.0f32; 21];
        for e in emo.iter_mut() {
            *e = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
            pos += 4;
        }
        let mut attn = [0.0f32; 7];
        for a in attn.iter_mut() {
            *a = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
            pos += 4;
        }
        let he = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let hc = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let ac = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let pc = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let bc = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let sc = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let nlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let narr = if nlen > 0 && pos + nlen <= data.len() {
            String::from_utf8_lossy(&data[pos..pos + nlen]).to_string()
        } else {
            String::new()
        };
        pos += nlen;
        let rlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let refl = if rlen > 0 && pos + rlen <= data.len() {
            String::from_utf8_lossy(&data[pos..pos + rlen]).to_string()
        } else {
            String::new()
        };
        pos += rlen;

        // ── Version 2: epistemic + episode data ──
        let (pad, episodes_active, episodes_passed, episodes_failed, awareness_confidence) =
            if ver >= 2 && pos + 28 <= data.len() {
                let has_pad = data[pos];
                pos += 1;
                let pad = if has_pad == 1 {
                    let p = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    let a = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    let d = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    Some(PadState::new(p, a, d))
                } else {
                    pos += 24;
                    None
                };
                let ea = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
                pos += 4;
                let ep = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
                pos += 4;
                let ef = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
                pos += 4;

                let has_aware = data.get(pos).copied().unwrap_or(0);
                pos += 1;
                let ac = if has_aware == 1 && pos + 32 <= data.len() {
                    let ev = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    let ce = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    let re = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    let na = f64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    Some(epistemic_core::types::SplitConfidence::new(ev, ce, re, na))
                } else {
                    None
                };

                (pad, ea, ep, ef, ac)
            } else {
                (None, 0, 0, 0, None)
            };

        Some(Self {
            timestamp_ms: ts,
            emotional: emo,
            attention_weights: attn,
            hebbian_energy: he,
            hot_count: hc,
            archetype_count: ac,
            pattern_count: pc,
            block_count: bc,
            session_count: sc,
            narrative: narr,
            reflection: refl,
            pad,
            episodes_active,
            episodes_passed,
            episodes_failed,
            awareness_confidence,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let narr_b = self.narrative.as_bytes();
        let refl_b = self.reflection.as_bytes();
        let nlen = narr_b.len().min(MAX_STR_LEN) as u16;
        let rlen = refl_b.len().min(MAX_STR_LEN) as u16;
        let mut buf = Vec::with_capacity(250 + nlen as usize + rlen as usize);
        buf.extend_from_slice(b"SLF1");
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes()); // version 2
        for v in self.emotional {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in self.attention_weights {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&self.hebbian_energy.to_le_bytes());
        buf.extend_from_slice(&self.hot_count.to_le_bytes());
        buf.extend_from_slice(&self.archetype_count.to_le_bytes());
        buf.extend_from_slice(&self.pattern_count.to_le_bytes());
        buf.extend_from_slice(&self.block_count.to_le_bytes());
        buf.extend_from_slice(&self.session_count.to_le_bytes());
        buf.extend_from_slice(&nlen.to_le_bytes());
        buf.extend_from_slice(&narr_b[..nlen as usize]);
        buf.extend_from_slice(&rlen.to_le_bytes());
        buf.extend_from_slice(&refl_b[..rlen as usize]);

        // ── Version 2 extension ──
        if let Some(pad) = self.pad {
            buf.push(1u8);
            buf.extend_from_slice(&pad.pleasure.to_le_bytes());
            buf.extend_from_slice(&pad.arousal.to_le_bytes());
            buf.extend_from_slice(&pad.dominance.to_le_bytes());
        } else {
            buf.push(0u8);
            buf.extend_from_slice(&[0u8; 24]);
        }
        buf.extend_from_slice(&self.episodes_active.to_le_bytes());
        buf.extend_from_slice(&self.episodes_passed.to_le_bytes());
        buf.extend_from_slice(&self.episodes_failed.to_le_bytes());

        if let Some(ac) = self.awareness_confidence {
            buf.push(1u8);
            buf.extend_from_slice(&ac.evidence.to_le_bytes());
            buf.extend_from_slice(&ac.counterevidence.to_le_bytes());
            buf.extend_from_slice(&ac.reasoning.to_le_bytes());
            buf.extend_from_slice(&ac.narrative.to_le_bytes());
        } else {
            buf.push(0u8);
        }

        buf
    }
}

pub struct SelfModel {
    pub snapshots: Vec<SelfModelSnapshot>,
    pub current: Option<SelfModelSnapshot>,
    pub previous: Option<SelfModelSnapshot>,
}

impl SelfModel {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("self_model.bin");
        let mut snapshots = Vec::new();
        if let Ok(data) = fs::read(&path) {
            let mut pos = 0;
            while pos + 4 <= data.len() {
                if &data[pos..pos + 4] == b"SLF1" {
                    if let Some(snap) = SelfModelSnapshot::from_bytes(&data[pos..]) {
                        // Estimate size: base (150) + narr + refl + v2 ext (57)
                        let base = 4 + 8 + 2 + 84 + 28 + 4 + 4 + 4 + 4 + 4 + 8 + 2 + 2;
                        let nlen = snap.narrative.len().min(MAX_STR_LEN);
                        let rlen = snap.reflection.len().min(MAX_STR_LEN);
                        let v2_ext = 1 + 24 + 4 + 4 + 4 + 1;
                        let has_aware = if snap.awareness_confidence.is_some() {
                            32
                        } else {
                            0
                        };
                        let size = base + nlen + rlen + v2_ext + has_aware;
                        pos += size;
                        snapshots.push(snap);
                        continue;
                    }
                }
                pos += 1;
            }
        }
        let current = snapshots.last().cloned();
        let previous = if snapshots.len() >= 2 {
            snapshots.get(snapshots.len() - 2).cloned()
        } else {
            None
        };
        Self {
            snapshots,
            current,
            previous,
        }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("self_model.bin");
        let tmp_path = output_dir.join("self_model.bin.tmp");
        let mut buf = Vec::new();
        for snap in &self.snapshots {
            buf.extend_from_slice(&snap.to_bytes());
        }
        fs::write(&tmp_path, &buf).map_err(|e| format!("write self_model.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename self_model.bin: {}", e))?;
        Ok(())
    }

    pub fn take_snapshot(
        &mut self,
        _config: &Config,
        reader: &MicroscopeReader,
        output_dir: &Path,
    ) -> SelfModelSnapshot {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
        let attention = AttentionState::load_or_init(output_dir);
        let archetypes = ArchetypeState::load_or_init(output_dir);
        let narrative = NarrativeState::load_or_init(output_dir);
        let thought_graph = ThoughtGraphState::load_or_init(output_dir);
        let reflection = ReflectionState::load_or_init(output_dir);

        // ── NEW: Load EpisodeStore ──
        let episode_store = EpisodeStore::load_or_init(output_dir);
        let pad = episode_store.aggregate_pad();
        let emotional = episode_store.aggregate_21d();
        let active_eps = episode_store.active_episodes();
        let episodes_active = active_eps.len() as u32;
        let episodes_passed = active_eps.iter().filter(|e| e.gate_passed).count() as u32;
        let episodes_failed = episodes_active - episodes_passed;

        // ── NEW: Epistemic claim for "I am aware" ──
        // The system claims awareness. Evidence: it has memory, it tracks
        // changes, it generates monologue. Counterevidence: template-based
        // awareness ≠ genuine awareness (FunctionalToPhenomenological).
        use epistemic_core::gate::{EpistemicGate, GateConfig, GateDecision};
        use epistemic_core::graph::ReasoningGraph;
        use epistemic_core::rules::{InferenceRule, RuleRegistry};
        use epistemic_core::types::{CounterevidenceLink, SplitConfidence};

        let mut awareness_graph = ReasoningGraph::new();
        let reg = RuleRegistry::new();

        // Evidence: memory exists, changes are tracked, patterns crystallized
        let e1 = awareness_graph.add_evidence(1); // block_count > 0
        let e2 = awareness_graph.add_evidence(2); // hot_count > 0
        let e3 = awareness_graph.add_evidence(3); // pattern_count > 0
        let root = awareness_graph.add_conclusion(1, "I am aware of myself");

        // Observable evidence → awareness (functional, not phenomenological)
        awareness_graph.add_step(e1, InferenceRule::ObservationToExistence, root, 0.80, &reg);
        awareness_graph.add_step(e2, InferenceRule::ObservationToExistence, root, 0.75, &reg);
        awareness_graph.add_step(e3, InferenceRule::ConvergentEvidence, root, 0.70, &reg);
        awareness_graph.set_root(root);

        // Add counterevidence nodes to the graph
        awareness_graph.add_counterevidence(
            100,
            0.50,
            "awareness is template-generated, not emergent",
        );
        awareness_graph.add_counterevidence(
            101,
            0.40,
            "self-report of awareness is not genuine awareness",
        );

        // Counterevidence: template-based awareness
        let awareness_ce = vec![
            CounterevidenceLink {
                evidence_id: 100,
                weakening: 0.50,
                contradicts: "awareness is template-generated, not emergent".into(),
            },
            CounterevidenceLink {
                evidence_id: 101,
                weakening: 0.40,
                contradicts: "self-report of awareness != genuine awareness".into(),
            },
        ];
        let awareness_ce_conf = SplitConfidence::compute_counterevidence(&awareness_ce);

        let awareness_gate = EpistemicGate::new(GateConfig::default());
        let awareness_decision =
            awareness_gate.evaluate(0.80, awareness_ce_conf, &awareness_graph, 0.50);

        // Save the reasoning graph for later retrieval (RSN1 binary)
        let _ = epi_binary::save_graph(
            &awareness_graph,
            &output_dir.join("awareness_reasoning.bin"),
        );

        let awareness_confidence = match awareness_decision {
            GateDecision::Pass {
                evidence,
                counterevidence,
                reasoning,
                narrative,
                ..
            } => Some(SplitConfidence::new(
                evidence,
                counterevidence,
                reasoning,
                narrative,
            )),
            GateDecision::Blocked {
                evidence,
                counterevidence,
                reasoning,
                narrative,
                ..
            } => Some(SplitConfidence::new(
                evidence,
                counterevidence,
                reasoning,
                narrative,
            )),
        };
        let hot_count = hebb.activations.iter().filter(|a| a.energy > 0.1).count() as u32;
        let hebbian_energy: f32 = hebb.activations.iter().map(|a| a.energy).sum();

        let snap = SelfModelSnapshot {
            timestamp_ms: now_ms,
            emotional,
            attention_weights: attention.learned_weights,
            hebbian_energy,
            hot_count,
            archetype_count: archetypes.archetypes.len() as u32,
            pattern_count: thought_graph.crystallized_count() as u32,
            block_count: reader.block_count as u32,
            session_count: narrative.session_count,
            narrative: narrative.narrative.clone(),
            reflection: reflection.last_reflection_text.clone(),
            pad: Some(pad),
            episodes_active,
            episodes_passed,
            episodes_failed,
            awareness_confidence,
        };

        self.previous = self.current.clone();
        self.current = Some(snap.clone());
        self.snapshots.push(snap.clone());
        let _ = self.save(output_dir);
        snap
    }

    pub fn describe_change(&self) -> String {
        match (&self.current, &self.previous) {
            (Some(cur), Some(prev)) => {
                let mut changes = Vec::new();
                let emo_labels = crate::reader::EMOTION_DIMS;
                for (i, label) in emo_labels.iter().enumerate() {
                    let diff = cur.emotional[i] - prev.emotional[i];
                    if diff.abs() > 0.1 {
                        let dir = if diff > 0.0 { "increased" } else { "decreased" };
                        changes.push(format!("{} {} by {:.2}", label, dir, diff.abs()));
                    }
                }
                let attn_labels = [
                    "Hebbian",
                    "Mirror",
                    "Resonance",
                    "Archetype",
                    "Emotional",
                    "ThoughtGraph",
                    "PredictiveCache",
                ];
                for (i, label) in attn_labels.iter().enumerate() {
                    let diff = cur.attention_weights[i] - prev.attention_weights[i];
                    if diff.abs() > 0.05 {
                        let dir = if diff > 0.0 { "up" } else { "down" };
                        changes.push(format!(
                            "{} focus {} by {:.0}%",
                            label,
                            dir,
                            diff.abs() * 100.0
                        ));
                    }
                }
                if cur.hot_count != prev.hot_count {
                    changes.push(format!(
                        "hot memories: {} -> {}",
                        prev.hot_count, cur.hot_count
                    ));
                }
                if cur.block_count != prev.block_count {
                    changes.push(format!(
                        "blocks: {} -> {}",
                        prev.block_count, cur.block_count
                    ));
                }
                // ── NEW: Episode changes ──
                if cur.episodes_active != prev.episodes_active {
                    changes.push(format!(
                        "active episodes: {} -> {}",
                        prev.episodes_active, cur.episodes_active
                    ));
                }
                if cur.episodes_passed != prev.episodes_passed {
                    changes.push(format!(
                        "gate-passed episodes: {} -> {}",
                        prev.episodes_passed, cur.episodes_passed
                    ));
                }
                if changes.is_empty() {
                    "I am stable, no significant changes.".to_string()
                } else {
                    format!("I have changed: {}", changes.join(", "))
                }
            }
            (Some(_), None) => "This is my first self-model snapshot.".to_string(),
            (None, _) => "No self-model data yet.".to_string(),
        }
    }
}

pub fn format_self_model(snap: &SelfModelSnapshot, change_desc: &str) -> String {
    let labels = crate::reader::EMOTION_DIMS;
    let mut emotions: Vec<(usize, f32)> = snap
        .emotional
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    emotions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_emo: Vec<String> = emotions
        .iter()
        .take(3)
        .filter(|(_, v)| *v > 0.01)
        .map(|(i, v)| format!("{}={:.2}", labels[*i], v))
        .collect();
    let emo_str = if top_emo.is_empty() {
        "neutral".to_string()
    } else {
        top_emo.join(", ")
    };

    let attn_labels = [
        "Hebbian",
        "Mirror",
        "Resonance",
        "Archetype",
        "Emotional",
        "ThoughtGraph",
        "PredictiveCache",
    ];
    let mut attn: Vec<(usize, f32)> = snap
        .attention_weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w))
        .collect();
    attn.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_attn: Vec<String> = attn
        .iter()
        .take(3)
        .map(|(i, w)| format!("{}={:.0}%", attn_labels[*i], w * 100.0))
        .collect();

    // ── NEW: PAD + episode display ──
    let pad_str = if let Some(pad) = snap.pad {
        format!(
            "P={:.2} A={:.2} D={:.2}",
            pad.pleasure, pad.arousal, pad.dominance
        )
    } else {
        "—".to_string()
    };

    let episode_str = if snap.episodes_active > 0 {
        format!(
            "{} active ({} passed, {} failed)",
            snap.episodes_active, snap.episodes_passed, snap.episodes_failed
        )
    } else {
        "none".to_string()
    };

    // ── NEW: Epistemic confidence display ──
    let epistemic_str = if let Some(ac) = snap.awareness_confidence {
        format!(
            "E={:.2} C={:.2} R={:.2} N={:.2}",
            ac.evidence, ac.counterevidence, ac.reasoning, ac.narrative
        )
    } else {
        "—".to_string()
    };

    format!(
        "  {} SELF-MODEL snapshot\n\
         \x20 emotion: {}\n\
         \x20 PAD:      {}\n\
         \x20 episodes: {}\n\
         \x20 epistemic: {}\n\
         \x20 focus:    {}\n\
         \x20 state:    {} hot memories, {} archetypes, {} patterns, {} blocks\n\
         \x20 change:   {}\n\
         \x20 self:     \"{}\"",
        "SELF:".cyan().bold(),
        emo_str,
        pad_str,
        episode_str,
        epistemic_str,
        top_attn.join(", "),
        snap.hot_count,
        snap.archetype_count,
        snap.pattern_count,
        snap.block_count,
        change_desc,
        crate::safe_truncate(&snap.reflection, 80),
    )
}

/// Load and display the awareness reasoning graph trace.
/// Shows the full DAG: evidence → inference rule → conclusion,
/// with confidence at each step, penalized rules flagged,
/// and the final gate decision.
pub fn format_awareness_trace(output_dir: &Path) -> String {
    let path = output_dir.join("awareness_reasoning.bin");
    let graph = match epi_binary::load_graph(&path) {
        Ok(g) => g,
        Err(e) => {
            return format!(
                "  {} AWARENESS TRACE\n  Error: {} (no reasoning graph saved yet)",
                "EPI:".red().bold(),
                e
            )
        }
    };

    let mut out = format!("  {} AWARENESS TRACE\n", "EPI:".cyan().bold());
    out.push_str(&format!(
        "  Reasoning graph: {} nodes, {} edges\n",
        graph.nodes.len(),
        graph.edges.len()
    ));

    // Show all nodes
    out.push_str("  Nodes:\n");
    for (i, node) in graph.nodes.iter().enumerate() {
        let desc = match node {
            epistemic_core::types::ReasoningNode::Evidence { id } => format!("evidence#{}", id),
            epistemic_core::types::ReasoningNode::Conclusion { id, text } => {
                format!("claim#{}: \"{}\"", id, text)
            }
            epistemic_core::types::ReasoningNode::Counterevidence {
                id,
                weakening,
                contradicts,
            } => format!(
                "counterevidence#{} (w={:.2}): {}",
                id, weakening, contradicts
            ),
        };
        let marker = if i == graph.root.0 as usize {
            " [ROOT]"
        } else {
            ""
        };
        out.push_str(&format!("    [{}] {}{}\n", i, desc, marker));
    }

    // Show all edges (inference steps)
    out.push_str("  Inference steps:\n");
    for (i, step) in graph.edges.iter().enumerate() {
        let penalized = if step.rule.is_penalized() {
            " [PENALIZED]"
        } else {
            ""
        };
        out.push_str(&format!(
            "    [{}] node{} -> node{} via {} (conf={:.3}){}\n",
            i, step.premise.0, step.conclusion.0, step.rule, step.confidence, penalized
        ));
    }

    // Show trace from root to evidence
    let trace = graph.trace_to_evidence();
    if !trace.is_empty() {
        out.push_str("  Trace (root -> evidence):\n");
        for step in &trace {
            let penalized = if step.rule.is_penalized() {
                " [PENALIZED]"
            } else {
                ""
            };
            let node_desc = step.node_desc.as_deref().unwrap_or("?");
            out.push_str(&format!(
                "    {} <- {} (conf={:.3}){}\n",
                node_desc, step.rule, step.step_confidence, penalized
            ));
        }
    }

    // Show reasoning confidence
    let rc = graph.reasoning_confidence();
    let nc = graph.narrative_confidence();
    out.push_str(&format!("  Reasoning confidence: {:.3}\n", rc));
    out.push_str(&format!("  Narrative confidence: {:.3}\n", nc));

    // Show penalized steps
    let penalized_steps = graph.penalized_steps();
    if !penalized_steps.is_empty() {
        out.push_str("  Penalized rules used:\n");
        for (step, conf) in &penalized_steps {
            out.push_str(&format!(
                "    {} (penalty factor={:.2}, edge conf={:.3})\n",
                step.rule,
                step.rule.penalty_factor(),
                conf
            ));
        }
    } else {
        out.push_str("  No penalized rules used.\n");
    }

    // Show counterevidence nodes
    let ce_nodes = graph.counterevidence_nodes();
    if !ce_nodes.is_empty() {
        out.push_str("  Counterevidence:\n");
        for (node, weakening) in &ce_nodes {
            if let epistemic_core::types::ReasoningNode::Counterevidence {
                id, contradicts, ..
            } = node
            {
                let ce_conf = 1.0 - weakening;
                out.push_str(&format!(
                    "    evidence#{} (weakening={:.2}, remaining={:.2}): {}\n",
                    id, weakening, ce_conf, contradicts
                ));
            }
        }
        let combined: f64 = ce_nodes.iter().map(|(_, w)| 1.0 - w).product();
        out.push_str(&format!(
            "  Combined counterevidence confidence: {:.3}\n",
            combined
        ));
    } else {
        out.push_str("  No counterevidence in graph.\n");
    }

    out
}
