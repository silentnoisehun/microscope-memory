//! Attention Mechanism — dynamic weighting of consciousness layers.
//!
//! Computes an attention vector over 7 layers based on query context:
//! query length, emotional energy, session depth, pattern confidence,
//! cache hit rate, archetype match score.
//!
//! Tracks which attention distributions lead to good results (user stops
//! searching) vs bad results (user immediately re-queries). Over time,
//! learns which layer weights work best.
//!
//! Binary format: attention.bin (ATT1)

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

/// Layer indices: Hebbian, Mirror, Resonance, Archetype, Emotional, ThoughtGraph, PredictiveCache
pub const NUM_LAYERS: usize = 7;
pub const LAYER_NAMES: [&str; NUM_LAYERS] = [
    "Hebbian",
    "Mirror",
    "Resonance",
    "Archetype",
    "Emotional",
    "ThoughtGraph",
    "PredictiveCache",
];

const MAX_HISTORY: usize = 200;
const LEARN_RATE: f32 = 0.05;
const QUALITY_GOOD_THRESHOLD: f32 = 0.7;
const QUALITY_BAD_THRESHOLD: f32 = 0.3;

/// Time thresholds for quality inference (ms).
const SATISFIED_MS: u64 = 60_000; // >60s gap = satisfied
const UNSATISFIED_MS: u64 = 5_000; // <5s gap = unsatisfied

// ─── AttentionSignals ───────────────────────────────

/// Input signals for computing attention weights.
pub struct AttentionSignals {
    pub query_length: usize,
    pub emotional_energy: f32,
    pub session_depth: usize,
    pub pattern_confidence: f32,
    pub cache_hit_rate: f32,
    pub archetype_match_score: f32,
}

// ─── AttentionVector ────────────────────────────────

/// Computed attention weights for a single recall.
#[derive(Clone, Debug)]
pub struct AttentionVector {
    pub weights: [f32; NUM_LAYERS],
}

impl AttentionVector {
    /// Get weight for a specific layer.
    pub fn weight(&self, layer: usize) -> f32 {
        if layer < NUM_LAYERS {
            self.weights[layer]
        } else {
            1.0
        }
    }
}

// ─── AttentionOutcome ───────────────────────────────

/// Recorded outcome for learning.
#[derive(Clone, Debug)]
pub struct AttentionOutcome {
    pub weights: [f32; NUM_LAYERS],
    pub timestamp_ms: u64,
    pub quality: f32,
}

const OUTCOME_BYTES: usize = NUM_LAYERS * 4 + 8 + 4; // 28 + 8 + 4 = 40

// ─── AttentionState ─────────────────────────────────

pub struct AttentionState {
    /// Learned optimal weights (running average from good outcomes).
    pub learned_weights: [f32; NUM_LAYERS],
    /// Recent outcome history.
    pub history: Vec<AttentionOutcome>,
    /// Last recall timestamp for quality inference.
    pub last_recall_ms: u64,
    /// Total recalls tracked.
    pub total_recalls: u32,
}

impl AttentionState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("attention.bin");
        if path.exists() {
            load_attention(&path)
        } else {
            Self {
                learned_weights: [1.0; NUM_LAYERS],
                history: Vec::new(),
                last_recall_ms: 0,
                total_recalls: 0,
            }
        }
    }

    /// Compute attention vector from context signals.
    pub fn compute_attention(&self, signals: &AttentionSignals) -> AttentionVector {
        let mut raw = [1.0f32; NUM_LAYERS];

        // Factor 1: query length
        if signals.query_length <= 10 {
            raw[0] *= 1.5; // Hebbian: short = familiar territory
            raw[3] *= 1.3; // Archetype: short = concept lookup
        } else {
            raw[5] *= 1.3; // ThoughtGraph: long = complex reasoning chain
        }

        // Factor 2: emotional energy
        raw[4] *= 1.0 + signals.emotional_energy.min(2.0);

        // Factor 3: session depth
        let depth_factor = (signals.session_depth as f32 / 5.0).min(1.0);
        raw[5] *= 1.0 + depth_factor * 0.5; // ThoughtGraph benefits from deep sessions
        raw[6] *= 1.0 + depth_factor * 0.3; // PredictiveCache needs session context

        // Factor 4: pattern confidence
        raw[5] *= 1.0 + signals.pattern_confidence;

        // Factor 5: cache hit rate
        raw[6] *= 1.0 + signals.cache_hit_rate;

        // Factor 6: archetype match
        raw[3] *= 1.0 + signals.archetype_match_score.min(2.0);

        // Blend with learned weights (80% computed, 20% learned)
        for (i, w) in raw.iter_mut().enumerate() {
            *w = *w * 0.8 + self.learned_weights[i] * 0.2;
        }

        // Normalize so average weight = 1.0
        let sum: f32 = raw.iter().sum();
        if sum > 0.0 {
            let scale = NUM_LAYERS as f32 / sum;
            for w in &mut raw {
                *w *= scale;
            }
        }

        AttentionVector { weights: raw }
    }

    /// Infer quality of last recall from time gap.
    /// Returns quality score (0.0 = bad, 1.0 = good).
    pub fn infer_quality(&self) -> f32 {
        if self.last_recall_ms == 0 {
            return 0.5; // no data
        }
        let now = now_epoch_ms();
        let gap = now.saturating_sub(self.last_recall_ms);

        if gap >= SATISFIED_MS {
            1.0
        } else if gap <= UNSATISFIED_MS {
            0.2
        } else {
            // Linear interpolation
            let t = (gap - UNSATISFIED_MS) as f32 / (SATISFIED_MS - UNSATISFIED_MS) as f32;
            0.2 + t * 0.8
        }
    }

    /// Record the outcome of a recall (the quality applies to the PREVIOUS recall).
    pub fn record_outcome(&mut self, quality: f32, weights: &[f32; NUM_LAYERS]) {
        self.history.push(AttentionOutcome {
            weights: *weights,
            timestamp_ms: now_epoch_ms(),
            quality,
        });

        if self.history.len() > MAX_HISTORY {
            self.history.drain(0..(self.history.len() - MAX_HISTORY));
        }

        self.update_learned_weights();
    }

    /// Mark that a recall just happened (for next quality inference).
    pub fn mark_recall(&mut self) {
        self.last_recall_ms = now_epoch_ms();
        self.total_recalls += 1;
    }

    /// Update learned weights from outcome history using EMA.
    fn update_learned_weights(&mut self) {
        let good: Vec<&AttentionOutcome> = self
            .history
            .iter()
            .filter(|o| o.quality >= QUALITY_GOOD_THRESHOLD)
            .collect();
        let bad: Vec<&AttentionOutcome> = self
            .history
            .iter()
            .filter(|o| o.quality <= QUALITY_BAD_THRESHOLD)
            .collect();

        if good.is_empty() && bad.is_empty() {
            return;
        }

        for i in 0..NUM_LAYERS {
            let good_avg = if good.is_empty() {
                self.learned_weights[i]
            } else {
                good.iter().map(|o| o.weights[i]).sum::<f32>() / good.len() as f32
            };
            let bad_avg = if bad.is_empty() {
                self.learned_weights[i]
            } else {
                bad.iter().map(|o| o.weights[i]).sum::<f32>() / bad.len() as f32
            };

            let delta = good_avg - bad_avg;
            self.learned_weights[i] += delta * LEARN_RATE;
            self.learned_weights[i] = self.learned_weights[i].clamp(0.1, 3.0);
        }
    }

    /// Save to binary.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_attention(&output_dir.join("attention.bin"), self)
    }
}

// ─── Binary I/O ─────────────────────────────────────

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn save_attention(path: &Path, state: &AttentionState) -> Result<(), String> {
    let mut buf = Vec::with_capacity(48 + state.history.len() * OUTCOME_BYTES);

    // Header
    buf.write_all(b"ATT1").map_err(|e| e.to_string())?;
    buf.write_all(&state.total_recalls.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&state.last_recall_ms.to_le_bytes())
        .map_err(|e| e.to_string())?;

    // Learned weights
    for &w in &state.learned_weights {
        buf.write_all(&w.to_le_bytes()).map_err(|e| e.to_string())?;
    }

    // History
    buf.write_all(&(state.history.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;

    for outcome in &state.history {
        for &w in &outcome.weights {
            buf.write_all(&w.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        buf.write_all(&outcome.timestamp_ms.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&outcome.quality.to_le_bytes())
            .map_err(|e| e.to_string())?;
    }

    fs::write(path, &buf).map_err(|e| e.to_string())
}

fn load_attention(path: &Path) -> AttentionState {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            return AttentionState {
                learned_weights: [1.0; NUM_LAYERS],
                history: Vec::new(),
                last_recall_ms: 0,
                total_recalls: 0,
            }
        }
    };

    // Header: 4 (magic) + 4 (total_recalls) + 8 (last_recall_ms) + 28 (learned_weights) + 4 (history_count) = 48
    if data.len() < 48 || &data[0..4] != b"ATT1" {
        return AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
    }

    let total_recalls = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let last_recall_ms = u64::from_le_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
    ]);

    let mut learned_weights = [0.0f32; NUM_LAYERS];
    let mut offset = 16;
    for w in &mut learned_weights {
        *w = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
    }

    let history_count = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;

    let mut history = Vec::with_capacity(history_count);
    for _ in 0..history_count {
        if offset + OUTCOME_BYTES > data.len() {
            break;
        }

        let mut weights = [0.0f32; NUM_LAYERS];
        for w in &mut weights {
            *w = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let timestamp_ms = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        let quality = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        history.push(AttentionOutcome {
            weights,
            timestamp_ms,
            quality,
        });
    }

    AttentionState {
        learned_weights,
        history,
        last_recall_ms,
        total_recalls,
    }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_signals() -> AttentionSignals {
        AttentionSignals {
            query_length: 20,
            emotional_energy: 0.0,
            session_depth: 0,
            pattern_confidence: 0.0,
            cache_hit_rate: 0.0,
            archetype_match_score: 0.0,
        }
    }

    #[test]
    fn test_compute_attention_short_query() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
        let mut signals = default_signals();
        signals.query_length = 5; // short

        let attn = state.compute_attention(&signals);
        // Hebbian (0) and Archetype (3) should be elevated
        assert!(attn.weights[0] > attn.weights[1]); // Hebbian > Mirror
        assert!(attn.weights[3] > attn.weights[1]); // Archetype > Mirror
    }

    #[test]
    fn test_compute_attention_long_query() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
        let mut signals = default_signals();
        signals.query_length = 50; // long

        let attn = state.compute_attention(&signals);
        // ThoughtGraph (5) should be elevated
        assert!(attn.weights[5] > attn.weights[1]); // ThoughtGraph > Mirror
    }

    #[test]
    fn test_compute_attention_high_emotion() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
        let mut signals = default_signals();
        signals.emotional_energy = 2.0;

        let attn = state.compute_attention(&signals);
        // Emotional (4) should be highest
        assert!(attn.weights[4] > attn.weights[0]);
    }

    #[test]
    fn test_compute_attention_deep_session() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
        let mut signals = default_signals();
        signals.session_depth = 10;

        let attn = state.compute_attention(&signals);
        // ThoughtGraph and PredictiveCache should be elevated
        assert!(attn.weights[5] > attn.weights[1]);
        assert!(attn.weights[6] > attn.weights[1]);
    }

    #[test]
    fn test_normalization() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
        let signals = default_signals();
        let attn = state.compute_attention(&signals);

        let sum: f32 = attn.weights.iter().sum();
        assert!((sum - NUM_LAYERS as f32).abs() < 0.01);
    }

    #[test]
    fn test_quality_inference_fast_requery() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: now_epoch_ms() - 2_000, // 2s ago
            total_recalls: 1,
        };
        let q = state.infer_quality();
        assert!(q < 0.3); // unsatisfied
    }

    #[test]
    fn test_quality_inference_satisfied() {
        let state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: now_epoch_ms() - 120_000, // 2 min ago
            total_recalls: 1,
        };
        let q = state.infer_quality();
        assert!((q - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_learned_weights_update() {
        let mut state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };

        // Record some good outcomes with high Hebbian weight
        let mut good_weights = [1.0f32; NUM_LAYERS];
        good_weights[0] = 2.0; // Hebbian high
        for _ in 0..5 {
            state.record_outcome(0.9, &good_weights);
        }

        // Record some bad outcomes with high Mirror weight
        let mut bad_weights = [1.0f32; NUM_LAYERS];
        bad_weights[1] = 2.0; // Mirror high
        for _ in 0..5 {
            state.record_outcome(0.1, &bad_weights);
        }

        // Learned weights should now favor Hebbian over Mirror
        assert!(state.learned_weights[0] > state.learned_weights[1]);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AttentionState {
            learned_weights: [1.1, 0.9, 1.0, 1.3, 0.8, 1.2, 0.7],
            history: Vec::new(),
            last_recall_ms: 12345678,
            total_recalls: 42,
        };
        state.history.push(AttentionOutcome {
            weights: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            timestamp_ms: 999,
            quality: 0.75,
        });

        state.save(dir.path()).unwrap();
        let loaded = AttentionState::load_or_init(dir.path());

        assert_eq!(loaded.total_recalls, 42);
        assert_eq!(loaded.last_recall_ms, 12345678);
        assert!((loaded.learned_weights[0] - 1.1).abs() < 0.001);
        assert!((loaded.learned_weights[6] - 0.7).abs() < 0.001);
        assert_eq!(loaded.history.len(), 1);
        assert!((loaded.history[0].quality - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_history_cap() {
        let mut state = AttentionState {
            learned_weights: [1.0; NUM_LAYERS],
            history: Vec::new(),
            last_recall_ms: 0,
            total_recalls: 0,
        };
        let weights = [1.0; NUM_LAYERS];
        for _ in 0..MAX_HISTORY + 50 {
            state.record_outcome(0.5, &weights);
        }
        assert!(state.history.len() <= MAX_HISTORY);
    }
}
