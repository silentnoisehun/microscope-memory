//! Emotional State Ring — persistent running 21D emotional state.
//!
//! Automatically tracks the emotional context of stored memories
//! and provides emotional priming for recall — the system's innate
//! "mood" that biases which memories surface.
//!
//! Binary format: emotional_state.bin (ESR1)
//! - magic: [0x45, 0x53, 0x52, 0x31] = "ESR1" (4 bytes)
//! - current_state: [f32; 21] (84 bytes)
//! - ring_head: u32 (4 bytes)
//! - ring_count: u32 (4 bytes)
//! - ring_buffer: [[f32; 21]; 10] (840 bytes)
//! - last_update: u64 (8 bytes)
//! - total_updates: u64 (8 bytes)
//!
//! Total: 952 bytes

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

const RING_MAGIC: &[u8; 4] = b"ESR1";
const RING_SIZE: usize = 10;
const STATE_BYTES: usize = 952; // 4 + 84 + 4 + 4 + 840 + 8 + 8

/// How much of the new emotion bleeds into the running state (0.0–1.0).
const BLEND_FACTOR: f32 = 0.3;

/// How fast the emotional state decays toward neutral when no new data arrives (per day).
const DECAY_PER_DAY: f32 = 0.15;

/// If emotional intensity is above this, the state is "active" and primes recall.
const PRIMING_THRESHOLD: f32 = 0.15;

// ─── EmotionalStateRing ─────────────────────────────

/// Persistent running emotional state with ring buffer history.
pub struct EmotionalStateRing {
    /// Current blended 21D emotional state vector.
    pub current: [f32; 21],
    /// Ring buffer of recent raw emotion vectors.
    pub ring: Vec<[f32; 21]>,
    /// Head index in ring buffer.
    head: usize,
    /// Number of valid entries in ring buffer.
    count: usize,
    /// Unix ms of last update.
    pub last_update: u64,
    /// Total lifetime updates.
    pub total_updates: u64,
}

impl EmotionalStateRing {
    /// Load from disk or return a fresh neutral state.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("emotional_state.bin");
        if path.exists() {
            if let Ok(data) = fs::read(&path) {
                if data.len() >= STATE_BYTES && &data[0..4] == RING_MAGIC {
                    return Self::from_bytes(&data);
                }
            }
        }
        Self::default()
    }

    /// Save to disk.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("emotional_state.bin");
        let tmp_path = output_dir.join("emotional_state.bin.tmp");
        fs::write(&tmp_path, self.to_bytes())
            .map_err(|e| format!("save emotional_state.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename emotional_state.bin: {}", e))
    }

    /// Update the emotional state with a new incoming emotion vector.
    /// Importance scales the blend: higher importance = stronger influence.
    pub fn update(&mut self, emotion: &[f32; 21], importance: u8) {
        let imp_factor = 0.5 + (importance as f32 / 10.0) * 0.5; // 0.55–1.0
        let blend = (BLEND_FACTOR * imp_factor).min(1.0);

        // Blend: current * (1 - blend) + incoming * blend
        for (cur, &inc) in self.current.iter_mut().zip(emotion.iter()) {
            *cur = *cur * (1.0 - blend) + inc * blend;
        }

        // Push into ring buffer
        self.ring[self.head] = *emotion;
        self.head = (self.head + 1) % RING_SIZE;
        if self.count < RING_SIZE {
            self.count += 1;
        }

        self.last_update = now_epoch_ms();
        self.total_updates += 1;
    }

    /// Apply time-based decay toward neutral. Call once per session or periodically.
    pub fn decay(&mut self) {
        let age_ms = now_epoch_ms().saturating_sub(self.last_update);
        let age_days = age_ms as f32 / 86_400_000.0;
        let decay = (DECAY_PER_DAY * age_days).min(1.0);
        if decay > 0.0 {
            for i in 0..21 {
                self.current[i] *= 1.0 - decay;
            }
        }
    }

    /// The magnitude (L2 norm) of the current emotional state.
    pub fn intensity(&self) -> f32 {
        let sum: f32 = self.current.iter().map(|x| x * x).sum();
        sum.sqrt()
    }

    /// Returns true if the current state is strong enough to prime recall.
    pub fn is_active(&self) -> bool {
        self.intensity() > PRIMING_THRESHOLD
    }

    /// Get the dominant emotion dimension name and value.
    pub fn dominant(&self) -> Option<(&'static str, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, &v) in self.current.iter().enumerate() {
            if v > 0.05 && best.is_none_or(|(_, b)| v > b) {
                best = Some((i, v));
            }
        }
        best.map(|(i, v)| {
            let name = crate::EMOTION_DIMS.get(i).copied().unwrap_or("?");
            (name, v)
        })
    }

    /// Compute the average emotion over the ring buffer (recent trend).
    pub fn recent_trend(&self) -> [f32; 21] {
        if self.count == 0 {
            return [0.0f32; 21];
        }
        let mut avg = [0.0f32; 21];
        for i in 0..self.count {
            let idx = if i < self.count {
                (self.head + RING_SIZE - 1 - i) % RING_SIZE
            } else {
                0
            };
            for (j, val) in avg.iter_mut().enumerate() {
                *val += self.ring[idx][j];
            }
        }
        let n = self.count as f32;
        for val in avg.iter_mut() {
            *val /= n;
        }
        avg
    }

    // ─── Binary serialization ──────────────────────

    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(STATE_BYTES);
        buf.extend_from_slice(RING_MAGIC);
        for v in &self.current {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&(self.head as u32).to_le_bytes());
        buf.extend_from_slice(&(self.count as u32).to_le_bytes());
        for i in 0..RING_SIZE {
            for j in 0..21 {
                buf.extend_from_slice(&self.ring[i][j].to_le_bytes());
            }
        }
        buf.extend_from_slice(&self.last_update.to_le_bytes());
        buf.extend_from_slice(&self.total_updates.to_le_bytes());
        buf
    }

    fn from_bytes(data: &[u8]) -> Self {
        let mut pos = 4; // skip magic
        let mut current = [0.0f32; 21];
        for c in current.iter_mut() {
            *c = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0u8; 4]));
            pos += 4;
        }
        let head = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0u8; 4])) as usize;
        pos += 4;
        let count = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0u8; 4])) as usize;
        pos += 4;
        let mut ring = vec![[0.0f32; 21]; RING_SIZE];
        for slot in ring.iter_mut() {
            for v in slot.iter_mut() {
                *v = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap_or([0u8; 4]));
                pos += 4;
            }
        }
        let last_update = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap_or([0u8; 8]));
        pos += 8;
        let total_updates = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap_or([0u8; 8]));

        EmotionalStateRing {
            current,
            ring,
            head,
            count,
            last_update,
            total_updates,
        }
    }
}

impl Default for EmotionalStateRing {
    fn default() -> Self {
        Self {
            current: [0.0f32; 21],
            ring: vec![[0.0f32; 21]; RING_SIZE],
            head: 0,
            count: 0,
            last_update: 0,
            total_updates: 0,
        }
    }
}

/// Get the emotional priming weight for use in recall scoring.
/// Returns 0.0 if the state is inactive (below threshold).
pub fn emotional_prime_weight(state: &EmotionalStateRing) -> f32 {
    if !state.is_active() {
        return 0.0;
    }
    // Scale: intensity above threshold → linear 0.0–1.0, then square for diminishing tail
    let raw = (state.intensity() - PRIMING_THRESHOLD) / (1.0 - PRIMING_THRESHOLD);
    (raw * 2.0).min(1.0) // amplify so even moderate intensity primes well
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_neutral() {
        let ring = EmotionalStateRing::default();
        assert_eq!(ring.intensity(), 0.0);
        assert!(!ring.is_active());
    }

    #[test]
    fn test_update_changes_state() {
        let mut ring = EmotionalStateRing::default();
        let happy = [
            0.8f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.3, 0.0,
            0.0, 0.0, 0.0, 0.4,
        ];
        ring.update(&happy, 8);
        assert!(ring.intensity() > 0.0);
        assert!(ring.is_active());
        assert_eq!(ring.count, 1);
        assert_eq!(ring.total_updates, 1);
    }

    #[test]
    fn test_multiple_updates_blend() {
        let mut ring = EmotionalStateRing::default();
        let v1 = [1.0f32; 21];
        let v2 = [0.0f32; 21];
        ring.update(&v1, 10);
        let intense = ring.intensity();
        ring.update(&v2, 1);
        // Should have moved toward zero but not all the way
        assert!(ring.intensity() < intense);
        assert!(ring.intensity() > 0.0);
    }

    #[test]
    fn test_ring_buffer_rotation() {
        let mut ring = EmotionalStateRing::default();
        for i in 0..15 {
            let emo = [(i as f32) / 10.0; 21];
            ring.update(&emo, 5);
        }
        assert_eq!(ring.count, RING_SIZE);
        assert_eq!(ring.total_updates, 15);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let mut ring = EmotionalStateRing::default();
        let happy = [0.9f32; 21];
        ring.update(&happy, 10);
        ring.save(&dir).unwrap();

        let loaded = EmotionalStateRing::load_or_init(&dir);
        assert_eq!(loaded.total_updates, 1);
        assert!((loaded.intensity() - ring.intensity()).abs() < 0.001);

        let _ = std::fs::remove_file(dir.join("emotional_state.bin"));
    }

    #[test]
    fn test_emotional_prime_weight_inactive() {
        let ring = EmotionalStateRing::default();
        assert_eq!(emotional_prime_weight(&ring), 0.0);
    }

    #[test]
    fn test_emotional_prime_weight_active() {
        let mut ring = EmotionalStateRing::default();
        let strong = [0.8f32; 21];
        ring.update(&strong, 10);
        assert!(emotional_prime_weight(&ring) > 0.0);
    }

    #[test]
    fn test_dominant_emotion() {
        let mut ring = EmotionalStateRing::default();
        let mut v = [0.0f32; 21];
        v[0] = 0.9; // joy
        v[6] = 0.7; // trust
        ring.update(&v, 10);
        let dom = ring.dominant();
        assert!(dom.is_some());
        assert_eq!(dom.unwrap().0, "joy");
        // After blend (BLEND_FACTOR=0.3, importance=10 → imp_factor=1.0 → blend=0.3): 0.0*0.7 + 0.9*0.3 = 0.27
        assert!((dom.unwrap().1 - 0.27).abs() < 0.01);
    }
}
