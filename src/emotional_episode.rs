//! Emotional Episode — event-bound emotional memory with epistemic backing.
//!
//! An emotional episode is NOT just a valence number. It is a structured
//! memory of *why* the system felt something:
//!
//! - What event triggered it (trigger_evidence_id)
//! - What PAD state it produced (pleasure, arousal, dominance)
//! - What 21D emotion vector it generated
//! - **Epistemic claim**: "this event triggered emotion X because of Y"
//! - **Counterevidence**: "word match could be pattern, not genuine emotion"
//! - Decay rate: how fast this episode fades
//! - Resonance links: which other memories this episode connects to
//!
//! Binary format: emotional_episodes.bin (EEP1)

use epistemic_core::gate::{EpistemicGate, GateConfig, GateDecision};
use epistemic_core::graph::ReasoningGraph;
use epistemic_core::rules::{InferenceRule, RuleRegistry};
use epistemic_core::types::{CounterevidenceLink, SplitConfidence};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

const EPISODE_MAGIC: &[u8; 4] = b"EEP1";
const MAX_EPISODES: usize = 500;

// ─── PAD Model ──────────────────────────────────────

/// Pleasure-Arousal-Dominance emotional model.
///
/// - Pleasure: -1.0 (negative) to +1.0 (positive) — valence
/// - Arousal: 0.0 (calm) to 1.0 (highly activated) — energy
/// - Dominance: 0.0 (submissive) to 1.0 (dominant) — control
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadState {
    pub pleasure: f64,
    pub arousal: f64,
    pub dominance: f64,
}

impl PadState {
    pub fn new(pleasure: f64, arousal: f64, dominance: f64) -> Self {
        Self {
            pleasure: pleasure.clamp(-1.0, 1.0),
            arousal: arousal.clamp(0.0, 1.0),
            dominance: dominance.clamp(0.0, 1.0),
        }
    }

    pub fn neutral() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Intensity = arousal * (1 - |1 - 2*dominance|) — dominance modulates
    pub fn intensity(&self) -> f64 {
        self.arousal * (1.0 - (1.0 - 2.0 * self.dominance).abs())
    }

    /// Convert PAD to a 21D emotion vector (compatible with existing EMOTION_DIMS).
    /// Maps PAD to Plutchik-like dimensions using known PAD→emotion correspondences.
    pub fn to_21d(&self) -> [f32; 21] {
        let p = self.pleasure as f32;
        let a = self.arousal as f32;
        let d = self.dominance as f32;

        // PAD → 21D mapping based on Mehrabian's correspondences
        [
            p * a,                           // 0: joy
            -p * a,                          // 1: sadness
            -p * a * (1.0 - d),              // 2: anger
            -p * a * d,                      // 3: fear
            a * (1.0 - d) * 0.5,             // 4: surprise
            -p * (1.0 - d),                  // 5: disgust
            p * d,                           // 6: trust
            a * d * 0.5,                     // 7: anticipation
            p * a * d,                       // 8: love
            p * d * 0.8,                     // 9: gratitude
            a * d,                           // 10: curiosity
            -d * a * 0.5,                    // 11: confusion
            p * a * d * 0.7,                 // 12: pride
            -p * a * (1.0 - d) * 0.7,        // 13: shame
            -p * a * (1.0 - d),              // 14: anxiety
            p * (1.0 - a) * d,               // 15: calm
            p * a * d,                       // 16: excitement
            (-p).max(0.0) * (1.0 - a) * 0.3, // 17: boredom
            p * a * d * 0.6,                 // 18: hope
            -p * a * (1.0 - d) * 0.6,        // 19: regret
            p * a * d * 0.5,                 // 20: empathy
        ]
    }
}

// ─── Emotional Episode ──────────────────────────────

/// An emotional episode — an event-bound emotional memory with epistemic backing.
///
/// Every episode carries:
/// 1. **Trigger**: what evidence (event) caused this emotion
/// 2. **PAD state**: the pleasure-arousal-dominance values
/// 3. **21D vector**: the Plutchik-compatible emotion vector
/// 4. **Epistemic claim**: why this is a genuine emotion, not just pattern-match
/// 5. **Counterevidence**: what could weaken this claim
/// 6. **Gate decision**: did this episode pass the epistemic gate?
/// 7. **Decay**: how fast this episode fades from active state
#[derive(Debug, Clone)]
pub struct EmotionalEpisode {
    pub episode_id: u64,
    pub timestamp_ms: u64,
    /// What evidence (event/memory) triggered this emotion.
    pub trigger_evidence_id: u64,
    /// The PAD emotional state.
    pub pad: PadState,
    /// The 21D emotion vector (derived from PAD + context).
    pub emotion_21d: [f32; 21],
    /// The dominant emotion label index (0-20).
    pub dominant_emotion: usize,
    /// Epistemic claim: "this event triggered emotion X because of Y"
    pub claim_text: String,
    /// Evidence confidence (how strong is the triggering evidence).
    pub evidence_confidence: f64,
    /// Counterevidence: what weakens this emotional claim.
    pub counterevidence: Vec<CounterevidenceLink>,
    /// Gate decision: did this episode pass?
    pub gate_passed: bool,
    /// The split confidence at gate time.
    pub split_confidence: Option<SplitConfidence>,
    /// Decay rate per day (how fast this episode fades).
    pub decay_per_day: f64,
    /// Connected memory block IDs (resonance links).
    pub resonance_links: Vec<u64>,
}

impl EmotionalEpisode {
    /// Create a new emotional episode with epistemic backing.
    ///
    /// This constructs the reasoning graph, counterevidence, and runs
    /// the gate automatically.
    pub fn new(
        episode_id: u64,
        trigger_evidence_id: u64,
        pad: PadState,
        claim_text: impl Into<String>,
        evidence_confidence: f64,
        trigger_is_structural: bool,
        counterevidence: Vec<CounterevidenceLink>,
        gate_config: &GateConfig,
    ) -> Self {
        let claim_text = claim_text.into();
        let emotion_21d = pad.to_21d();
        let dominant_emotion = emotion_21d
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Build reasoning graph
        let mut graph = ReasoningGraph::new();
        let reg = RuleRegistry::new();

        let ev_node = graph.add_evidence(trigger_evidence_id);
        let root = graph.add_conclusion(episode_id, &claim_text);

        // If the trigger is structural (not just word match), use ObservationToExistence.
        // If it's word-list-based, use DesignedMechanismToEmergent (weakened).
        let rule = if trigger_is_structural {
            InferenceRule::ObservationToExistence
        } else {
            InferenceRule::DesignedMechanismToEmergent
        };

        graph.add_step(ev_node, rule, root, evidence_confidence, &reg);
        graph.set_root(root);

        // Compute counterevidence confidence
        let ce_conf = SplitConfidence::compute_counterevidence(&counterevidence);

        // Narrative confidence: structural triggers get higher narrative,
        // word-list triggers get lower (more likely to be pattern-match)
        let narrative = if trigger_is_structural { 0.75 } else { 0.45 };

        // Run the gate
        let gate = EpistemicGate::new(gate_config.clone());
        let decision = gate.evaluate(evidence_confidence, ce_conf, &graph, narrative);

        let (gate_passed, split_confidence) = match decision {
            GateDecision::Pass {
                evidence,
                counterevidence,
                reasoning,
                narrative,
                ..
            } => (
                true,
                Some(SplitConfidence::new(
                    evidence,
                    counterevidence,
                    reasoning,
                    narrative,
                )),
            ),
            GateDecision::Blocked {
                evidence,
                counterevidence,
                reasoning,
                narrative,
                ..
            } => (
                false,
                Some(SplitConfidence::new(
                    evidence,
                    counterevidence,
                    reasoning,
                    narrative,
                )),
            ),
        };

        Self {
            episode_id,
            timestamp_ms: now_epoch_ms(),
            trigger_evidence_id,
            pad,
            emotion_21d,
            dominant_emotion,
            claim_text,
            evidence_confidence,
            counterevidence,
            gate_passed,
            split_confidence,
            decay_per_day: 0.15,
            resonance_links: Vec::new(),
        }
    }

    /// Apply time-based decay. Returns the decayed intensity (0.0–1.0).
    pub fn decayed_intensity(&self) -> f64 {
        let age_ms = now_epoch_ms().saturating_sub(self.timestamp_ms);
        let age_days = age_ms as f64 / 86_400_000.0;
        let decay = (self.decay_per_day * age_days).min(1.0);
        self.pad.intensity() * (1.0 - decay)
    }

    /// Is this episode still active (above threshold)?
    pub fn is_active(&self) -> bool {
        self.decayed_intensity() > 0.05
    }

    /// Get the decayed 21D vector (scaled by decay factor).
    pub fn decayed_21d(&self) -> [f32; 21] {
        let intensity = self.decayed_intensity() as f32;
        let base_intensity = self.pad.intensity() as f32;
        if base_intensity < 1e-6 {
            return [0.0; 21];
        }
        let scale = intensity / base_intensity;
        self.emotion_21d.map(|v| v * scale)
    }
}

// ─── Episode Store ──────────────────────────────────

/// Stores emotional episodes with EEP1 binary persistence.
pub struct EpisodeStore {
    pub episodes: Vec<EmotionalEpisode>,
    pub next_id: u64,
}

impl EpisodeStore {
    pub fn new() -> Self {
        Self {
            episodes: Vec::new(),
            next_id: 1,
        }
    }

    /// Load from disk or initialize empty.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("emotional_episodes.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 8 && &data[0..4] == EPISODE_MAGIC {
                return Self::from_bytes(&data);
            }
        }
        Self::new()
    }

    /// Add a new episode. Returns the episode ID.
    pub fn add(&mut self, mut episode: EmotionalEpisode) -> u64 {
        let id = self.next_id;
        episode.episode_id = id;
        self.next_id += 1;
        self.episodes.push(episode);

        // Trim to max
        if self.episodes.len() > MAX_EPISODES {
            self.episodes.drain(0..(self.episodes.len() - MAX_EPISODES));
        }

        id
    }

    /// Get all active (non-decayed) episodes.
    pub fn active_episodes(&self) -> Vec<&EmotionalEpisode> {
        self.episodes.iter().filter(|e| e.is_active()).collect()
    }

    /// Aggregate the 21D state from all active episodes.
    /// Each episode contributes its decayed 21D vector, weighted by
    /// intensity and gate pass status (failed episodes contribute less).
    pub fn aggregate_21d(&self) -> [f32; 21] {
        let mut result = [0.0f32; 21];
        let mut total_weight = 0.0f32;

        for episode in &self.episodes {
            if !episode.is_active() {
                continue;
            }

            let intensity = episode.decayed_intensity() as f32;
            // Gate-passed episodes contribute fully.
            // Gate-failed episodes contribute at 30% (suspicious emotion).
            let gate_weight = if episode.gate_passed { 1.0 } else { 0.3 };
            let weight = intensity * gate_weight;

            let decayed = episode.decayed_21d();
            for (i, &v) in decayed.iter().enumerate() {
                result[i] += v * weight;
            }
            total_weight += weight;
        }

        if total_weight > 1e-6 {
            // Normalize by episode count (not weight) so failed episodes
            // contribute less in absolute magnitude
            let n = self.episodes.iter().filter(|e| e.is_active()).count() as f32;
            for v in &mut result {
                *v /= n;
            }
        }

        result
    }

    /// Aggregate PAD state from active episodes.
    pub fn aggregate_pad(&self) -> PadState {
        let active: Vec<_> = self.episodes.iter().filter(|e| e.is_active()).collect();
        if active.is_empty() {
            return PadState::neutral();
        }

        let mut p = 0.0f64;
        let mut a = 0.0f64;
        let mut d = 0.0f64;
        let mut total_weight = 0.0f64;

        for episode in &active {
            let intensity = episode.decayed_intensity();
            let gate_weight = if episode.gate_passed { 1.0 } else { 0.3 };
            let weight = intensity * gate_weight;

            p += episode.pad.pleasure * weight;
            a += episode.pad.arousal * weight;
            d += episode.pad.dominance * weight;
            total_weight += weight;
        }

        if total_weight > 1e-6 {
            PadState::new(p / total_weight, a / total_weight, d / total_weight)
        } else {
            PadState::neutral()
        }
    }

    /// Save to disk (EEP1 format).
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("emotional_episodes.bin");
        let tmp = output_dir.join("emotional_episodes.bin.tmp");
        fs::write(&tmp, self.to_bytes())
            .map_err(|e| format!("write emotional_episodes.bin: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename emotional_episodes.bin: {e}"))
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1024);
        buf.extend_from_slice(EPISODE_MAGIC);
        buf.extend_from_slice(&(self.episodes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.next_id.to_le_bytes());

        for ep in &self.episodes {
            // Fixed header
            buf.extend_from_slice(&ep.episode_id.to_le_bytes()); // 8
            buf.extend_from_slice(&ep.timestamp_ms.to_le_bytes()); // 8
            buf.extend_from_slice(&ep.trigger_evidence_id.to_le_bytes()); // 8
            buf.extend_from_slice(&ep.pad.pleasure.to_le_bytes()); // 8
            buf.extend_from_slice(&ep.pad.arousal.to_le_bytes()); // 8
            buf.extend_from_slice(&ep.pad.dominance.to_le_bytes()); // 8
            for &v in &ep.emotion_21d {
                buf.extend_from_slice(&v.to_le_bytes()); // 21 * 4
            }
            buf.extend_from_slice(&(ep.dominant_emotion as u32).to_le_bytes()); // 4
            buf.extend_from_slice(&ep.evidence_confidence.to_le_bytes()); // 8
            buf.push(if ep.gate_passed { 1u8 } else { 0u8 }); // 1

            // Split confidence (if present)
            if let Some(sc) = ep.split_confidence {
                buf.push(1u8); // has confidence
                buf.extend_from_slice(&sc.evidence.to_le_bytes()); // 8
                buf.extend_from_slice(&sc.counterevidence.to_le_bytes()); // 8
                buf.extend_from_slice(&sc.reasoning.to_le_bytes()); // 8
                buf.extend_from_slice(&sc.narrative.to_le_bytes()); // 8
            } else {
                buf.push(0u8);
            }

            // Claim text
            let text_bytes = ep.claim_text.as_bytes();
            buf.extend_from_slice(&(text_bytes.len() as u32).to_le_bytes()); // 4
            buf.extend_from_slice(text_bytes);

            // Decay rate
            buf.extend_from_slice(&ep.decay_per_day.to_le_bytes()); // 8

            // Resonance links
            buf.extend_from_slice(&(ep.resonance_links.len() as u32).to_le_bytes()); // 4
            for &link in &ep.resonance_links {
                buf.extend_from_slice(&link.to_le_bytes()); // 8 each
            }

            // Counterevidence count + links
            buf.extend_from_slice(&(ep.counterevidence.len() as u32).to_le_bytes()); // 4
            for ce in &ep.counterevidence {
                buf.extend_from_slice(&ce.evidence_id.to_le_bytes()); // 8
                buf.extend_from_slice(&ce.weakening.to_le_bytes()); // 8
                let ct_bytes = ce.contradicts.as_bytes();
                buf.extend_from_slice(&(ct_bytes.len() as u32).to_le_bytes()); // 4
                buf.extend_from_slice(ct_bytes);
            }
        }

        buf
    }

    fn from_bytes(data: &[u8]) -> Self {
        let mut off = 4; // skip magic
        let count = u32::from_le_bytes(data[off..off + 4].try_into().unwrap_or([0; 4])) as usize;
        off += 4;
        let next_id = u64::from_le_bytes(data[off..off + 8].try_into().unwrap_or([0; 8]));
        off += 8;

        let mut episodes = Vec::with_capacity(count);

        for _ in 0..count {
            if off + 8 + 8 + 8 + 8 + 8 + 8 + 84 + 4 + 8 + 1 > data.len() {
                break;
            }

            let episode_id = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;
            let timestamp_ms = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;
            let trigger_evidence_id = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;
            let pleasure = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;
            let arousal = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;
            let dominance = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;

            let mut emotion_21d = [0.0f32; 21];
            for v in &mut emotion_21d {
                *v = f32::from_le_bytes(data[off..off + 4].try_into().unwrap());
                off += 4;
            }

            let dominant_emotion =
                u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let evidence_confidence = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;
            let gate_passed = data[off] == 1;
            off += 1;

            let has_confidence = data[off] == 1;
            off += 1;
            let split_confidence = if has_confidence {
                let ev = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                off += 8;
                let ce = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                off += 8;
                let re = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                off += 8;
                let na = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                off += 8;
                Some(SplitConfidence::new(ev, ce, re, na))
            } else {
                None
            };

            let text_len = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let claim_text = String::from_utf8_lossy(&data[off..off + text_len]).into_owned();
            off += text_len;

            let decay_per_day = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
            off += 8;

            let link_count = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let mut resonance_links = Vec::with_capacity(link_count);
            for _ in 0..link_count {
                if off + 8 > data.len() {
                    break;
                }
                resonance_links.push(u64::from_le_bytes(data[off..off + 8].try_into().unwrap()));
                off += 8;
            }

            let ce_count = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            let mut counterevidence = Vec::with_capacity(ce_count);
            for _ in 0..ce_count {
                if off + 8 + 8 + 4 > data.len() {
                    break;
                }
                let ce_id = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                off += 8;
                let weakening = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                off += 8;
                let ct_len = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
                off += 4;
                let contradicts = String::from_utf8_lossy(&data[off..off + ct_len]).into_owned();
                off += ct_len;
                counterevidence.push(CounterevidenceLink {
                    evidence_id: ce_id,
                    weakening,
                    contradicts,
                });
            }

            episodes.push(EmotionalEpisode {
                episode_id,
                timestamp_ms,
                trigger_evidence_id,
                pad: PadState::new(pleasure, arousal, dominance),
                emotion_21d,
                dominant_emotion,
                claim_text,
                evidence_confidence,
                counterevidence,
                gate_passed,
                split_confidence,
                decay_per_day,
                resonance_links,
            });
        }

        Self { episodes, next_id }
    }
}

impl Default for EpisodeStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Helpers ────────────────────────────────────────

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_to_21d_joy_dominant() {
        let pad = PadState::new(0.8, 0.7, 0.6);
        let v = pad.to_21d();
        // joy (index 0) should be high: p * a = 0.8 * 0.7 = 0.56
        assert!(v[0] > 0.4, "joy should be high, got {}", v[0]);
        // sadness (index 1) should be low/negative
        assert!(v[1] < 0.0, "sadness should be negative");
    }

    #[test]
    fn pad_neutral_is_zero() {
        let pad = PadState::neutral();
        let v = pad.to_21d();
        for &val in &v {
            assert!(val.abs() < 0.01, "neutral should be near-zero");
        }
    }

    #[test]
    fn episode_with_structural_trigger_passes_gate() {
        let gate_config = GateConfig::default();
        let ce = vec![CounterevidenceLink {
            evidence_id: 999,
            weakening: 0.20,
            contradicts: "could be pattern-match not genuine emotion".into(),
        }];

        let ep = EmotionalEpisode::new(
            1,
            100,
            PadState::new(0.7, 0.6, 0.5),
            "this milestone triggered joy because of shared work completion",
            0.85,
            true, // structural trigger
            ce,
            &gate_config,
        );

        assert!(
            ep.gate_passed,
            "structural trigger with good evidence should pass"
        );
    }

    #[test]
    fn episode_with_word_list_trigger_weakened() {
        let gate_config = GateConfig::default();
        let ce = vec![CounterevidenceLink {
            evidence_id: 998,
            weakening: 0.60,
            contradicts: "word-list valence is pattern-match, not structural evidence".into(),
        }];

        let ep = EmotionalEpisode::new(
            2,
            200,
            PadState::new(0.5, 0.4, 0.5),
            "text contains 'happy' so emotion is joy",
            0.50,
            false, // word-list trigger (not structural)
            ce,
            &gate_config,
        );

        // With word-list trigger: DesignedMechanismToEmergent ×0.70
        // + counterevidence 0.60 weakening: (1-0.60) = 0.40
        // + narrative 0.45 (low for word-list)
        // Should be blocked or barely pass
        let sc = ep.split_confidence.unwrap();
        // reasoning should be weakened by DesignedMechanismToEmergent
        assert!(
            sc.reasoning < 0.70,
            "word-list trigger should weaken reasoning, got {}",
            sc.reasoning
        );
    }

    #[test]
    fn episode_store_aggregate_21d() {
        let mut store = EpisodeStore::new();
        let gate_config = GateConfig::default();

        let ep1 = EmotionalEpisode::new(
            1,
            100,
            PadState::new(0.8, 0.7, 0.6),
            "milestone completed",
            0.90,
            true,
            vec![],
            &gate_config,
        );
        let ep2 = EmotionalEpisode::new(
            2,
            200,
            PadState::new(-0.3, 0.5, 0.4),
            "bridge timeout error",
            0.80,
            true,
            vec![],
            &gate_config,
        );

        store.add(ep1);
        store.add(ep2);

        let agg = store.aggregate_21d();
        // Should not be all zeros
        let sum: f32 = agg.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(sum > 0.01, "aggregate should be non-zero");
    }

    #[test]
    fn episode_store_aggregate_pad() {
        let mut store = EpisodeStore::new();
        let gate_config = GateConfig::default();

        store.add(EmotionalEpisode::new(
            1,
            100,
            PadState::new(0.8, 0.7, 0.6),
            "milestone",
            0.90,
            true,
            vec![],
            &gate_config,
        ));

        let pad = store.aggregate_pad();
        // Should be positive pleasure (only positive episode)
        assert!(pad.pleasure > 0.0, "pleasure should be positive");
    }

    #[test]
    fn episode_store_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = EpisodeStore::new();
        let gate_config = GateConfig::default();

        let ce = vec![CounterevidenceLink {
            evidence_id: 999,
            weakening: 0.30,
            contradicts: "test counterevidence".into(),
        }];

        let ep = EmotionalEpisode::new(
            1,
            100,
            PadState::new(0.7, 0.6, 0.5),
            "test episode",
            0.85,
            true,
            ce,
            &gate_config,
        );
        store.add(ep);
        store.save(dir.path()).unwrap();

        let loaded = EpisodeStore::load_or_init(dir.path());
        assert_eq!(loaded.episodes.len(), 1);
        assert_eq!(loaded.episodes[0].claim_text, "test episode");
        assert!(loaded.episodes[0].gate_passed);
        assert_eq!(loaded.episodes[0].counterevidence.len(), 1);
    }

    #[test]
    fn failed_gate_episodes_contribute_less() {
        let mut store = EpisodeStore::new();
        let gate_config = GateConfig::default();

        // Episode that will fail the gate (weak evidence + strong counterevidence)
        let ce = vec![CounterevidenceLink {
            evidence_id: 998,
            weakening: 0.90,
            contradicts: "strong refutation".into(),
        }];

        let ep = EmotionalEpisode::new(
            1,
            100,
            PadState::new(0.8, 0.7, 0.6),
            "weak emotional claim",
            0.30,
            false,
            ce,
            &gate_config,
        );
        assert!(!ep.gate_passed, "should fail gate");

        store.add(ep);
        let agg = store.aggregate_21d();
        // Failed episode contributes at 30% weight, so aggregate should be small
        let sum: f32 = agg.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            sum < 0.5,
            "failed episode should contribute weakly, got sum={sum}"
        );
    }
}
