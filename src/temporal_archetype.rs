//! Temporal Archetypes — time-windowed activation patterns.
//!
//! Tracks which time-of-day windows activate which archetypes.
//! Detects temporal patterns: "archetype X always activates between 08-12h."
//! Provides time-aware recall boosting — archetypes active in the current
//! time window get a higher boost.
//!
//! Binary format: temporal_archetypes.bin (TAR1)

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

/// 6 time windows: 00-04, 04-08, 08-12, 12-16, 16-20, 20-24
const TIME_WINDOWS: usize = 6;
const WINDOW_HOURS: u64 = 4;
const TEMPORAL_DECAY: f32 = 0.99;
const MIN_ACTIVATIONS: u32 = 5;
const PROFILE_BYTES: usize = 56; // 4 + 24 + 24 + 4

/// Time window labels.
pub const WINDOW_LABELS: [&str; TIME_WINDOWS] = [
    "00-04", "04-08", "08-12", "12-16", "16-20", "20-24",
];

// ─── TemporalProfile ────────────────────────────────

/// Per-archetype temporal activation profile.
#[derive(Clone, Debug)]
pub struct TemporalProfile {
    pub archetype_id: u32,
    pub window_counts: [u32; TIME_WINDOWS],
    pub window_weights: [f32; TIME_WINDOWS],
    pub total_activations: u32,
}

impl TemporalProfile {
    fn new(archetype_id: u32) -> Self {
        Self {
            archetype_id,
            window_counts: [0; TIME_WINDOWS],
            window_weights: [0.0; TIME_WINDOWS],
            total_activations: 0,
        }
    }

    /// Dominant time window (highest weight), if enough activations.
    pub fn dominant_window(&self) -> Option<usize> {
        if self.total_activations < MIN_ACTIVATIONS {
            return None;
        }
        self.window_weights
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
    }

    /// Temporal boost for current time window: ratio of current window weight to peak.
    pub fn temporal_boost(&self, current_window: usize) -> f32 {
        if self.total_activations < MIN_ACTIVATIONS {
            return 1.0; // neutral boost
        }
        let max_weight = self
            .window_weights
            .iter()
            .cloned()
            .fold(0.0f32, f32::max);
        if max_weight < 0.01 {
            return 1.0;
        }
        (self.window_weights[current_window] / max_weight).max(0.1)
    }
}

// ─── TemporalArchetypeState ─────────────────────────

pub struct TemporalArchetypeState {
    pub profiles: Vec<TemporalProfile>,
}

impl TemporalArchetypeState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("temporal_archetypes.bin");
        if path.exists() {
            load_temporal(&path)
        } else {
            Self {
                profiles: Vec::new(),
            }
        }
    }

    /// Record that an archetype was activated at the given timestamp.
    pub fn record_activation(&mut self, archetype_id: u32, timestamp_ms: u64) {
        let window = time_window(timestamp_ms);

        let profile = match self
            .profiles
            .iter_mut()
            .find(|p| p.archetype_id == archetype_id)
        {
            Some(p) => p,
            None => {
                self.profiles.push(TemporalProfile::new(archetype_id));
                self.profiles.last_mut().unwrap()
            }
        };

        profile.window_counts[window] += 1;
        profile.window_weights[window] += 1.0;
        profile.total_activations += 1;
    }

    /// Decay all weights.
    pub fn decay(&mut self) {
        for profile in &mut self.profiles {
            for w in &mut profile.window_weights {
                *w *= TEMPORAL_DECAY;
            }
        }
    }

    /// Get temporal boost for a specific archetype at current time.
    pub fn boost(&self, archetype_id: u32) -> f32 {
        let window = current_time_window();
        self.profiles
            .iter()
            .find(|p| p.archetype_id == archetype_id)
            .map(|p| p.temporal_boost(window))
            .unwrap_or(1.0)
    }

    /// Save to binary.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_temporal(
            &output_dir.join("temporal_archetypes.bin"),
            &self.profiles,
        )
    }
}

// ─── Time utilities ─────────────────────────────────

/// Current UTC time window index (0..5).
pub fn current_time_window() -> usize {
    let ms = now_epoch_ms();
    time_window(ms)
}

/// Time window from epoch ms.
pub fn time_window(epoch_ms: u64) -> usize {
    let hour = (epoch_ms / 3_600_000) % 24;
    (hour / WINDOW_HOURS) as usize
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Binary I/O ─────────────────────────────────────

fn save_temporal(path: &Path, profiles: &[TemporalProfile]) -> Result<(), String> {
    let mut buf = Vec::with_capacity(8 + profiles.len() * PROFILE_BYTES);

    buf.write_all(b"TAR1").map_err(|e| e.to_string())?;
    buf.write_all(&(profiles.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;

    for p in profiles {
        buf.write_all(&p.archetype_id.to_le_bytes())
            .map_err(|e| e.to_string())?;
        for &c in &p.window_counts {
            buf.write_all(&c.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        for &w in &p.window_weights {
            buf.write_all(&w.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        buf.write_all(&p.total_activations.to_le_bytes())
            .map_err(|e| e.to_string())?;
    }

    fs::write(path, &buf).map_err(|e| e.to_string())
}

fn load_temporal(path: &Path) -> TemporalArchetypeState {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            return TemporalArchetypeState {
                profiles: Vec::new(),
            }
        }
    };

    if data.len() < 8 || &data[0..4] != b"TAR1" {
        return TemporalArchetypeState {
            profiles: Vec::new(),
        };
    }

    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut offset = 8;
    let mut profiles = Vec::with_capacity(count);

    for _ in 0..count {
        if offset + PROFILE_BYTES > data.len() {
            break;
        }

        let archetype_id = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let mut window_counts = [0u32; TIME_WINDOWS];
        for c in &mut window_counts {
            *c = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let mut window_weights = [0.0f32; TIME_WINDOWS];
        for w in &mut window_weights {
            *w = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        let total_activations = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        profiles.push(TemporalProfile {
            archetype_id,
            window_counts,
            window_weights,
            total_activations,
        });
    }

    TemporalArchetypeState { profiles }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_window() {
        // 08:30 UTC in ms from epoch: use a known offset
        let h8_30 = 8 * 3_600_000 + 30 * 60_000; // just hour component
        assert_eq!(time_window(h8_30), 2); // 08-12 window

        let h0 = 0u64;
        assert_eq!(time_window(h0), 0); // 00-04

        let h23 = 23 * 3_600_000;
        assert_eq!(time_window(h23), 5); // 20-24
    }

    #[test]
    fn test_record_activation() {
        let mut state = TemporalArchetypeState {
            profiles: Vec::new(),
        };
        let ts = 10 * 3_600_000u64; // 10:00 → window 2
        state.record_activation(42, ts);

        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.profiles[0].archetype_id, 42);
        assert_eq!(state.profiles[0].window_counts[2], 1);
        assert_eq!(state.profiles[0].total_activations, 1);
    }

    #[test]
    fn test_temporal_boost_peak() {
        let mut profile = TemporalProfile::new(1);
        profile.total_activations = 10;
        profile.window_weights = [0.0, 0.0, 5.0, 0.0, 0.0, 0.0]; // peak at window 2

        let boost = profile.temporal_boost(2); // current = peak
        assert!((boost - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_temporal_boost_off_peak() {
        let mut profile = TemporalProfile::new(1);
        profile.total_activations = 10;
        profile.window_weights = [0.0, 0.0, 5.0, 1.0, 0.0, 0.0];

        let boost = profile.temporal_boost(3); // off-peak
        assert!((boost - 0.2).abs() < 0.001); // 1.0/5.0

        let boost_empty = profile.temporal_boost(0); // zero window
        assert!((boost_empty - 0.1).abs() < 0.001); // clamped to 0.1
    }

    #[test]
    fn test_decay() {
        let mut state = TemporalArchetypeState {
            profiles: vec![TemporalProfile {
                archetype_id: 1,
                window_counts: [0; TIME_WINDOWS],
                window_weights: [1.0; TIME_WINDOWS],
                total_activations: 10,
            }],
        };
        state.decay();
        assert!((state.profiles[0].window_weights[0] - TEMPORAL_DECAY).abs() < 0.001);
    }

    #[test]
    fn test_dominant_window() {
        let mut profile = TemporalProfile::new(1);
        profile.total_activations = 10;
        profile.window_weights = [0.5, 0.1, 3.0, 0.2, 0.1, 0.0];

        assert_eq!(profile.dominant_window(), Some(2));
    }

    #[test]
    fn test_dominant_window_insufficient_data() {
        let profile = TemporalProfile::new(1); // total_activations = 0
        assert_eq!(profile.dominant_window(), None);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let state = TemporalArchetypeState {
            profiles: vec![
                TemporalProfile {
                    archetype_id: 1,
                    window_counts: [5, 0, 10, 3, 0, 1],
                    window_weights: [2.0, 0.0, 5.0, 1.5, 0.0, 0.5],
                    total_activations: 19,
                },
                TemporalProfile {
                    archetype_id: 7,
                    window_counts: [0, 0, 0, 0, 8, 2],
                    window_weights: [0.0, 0.0, 0.0, 0.0, 4.0, 1.0],
                    total_activations: 10,
                },
            ],
        };

        state.save(dir.path()).unwrap();
        let loaded = TemporalArchetypeState::load_or_init(dir.path());

        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.profiles[0].archetype_id, 1);
        assert_eq!(loaded.profiles[0].window_counts[2], 10);
        assert!((loaded.profiles[0].window_weights[2] - 5.0).abs() < 0.001);
        assert_eq!(loaded.profiles[1].archetype_id, 7);
        assert_eq!(loaded.profiles[1].total_activations, 10);
    }
}
