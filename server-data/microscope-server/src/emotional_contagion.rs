//! Emotional Contagion for Microscope Memory.
//!
//! Propagates emotional state across federated indices, creating shared
//! emotional context between instances. Each instance captures its emotional
//! field as a snapshot (centroid, energy, valence) and exchanges these with
//! peers. The blended emotional centroid influences the search space warp.
//!
//! Binary format: emotional_field.bin (EMO1)

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hebbian::HebbianState;
use crate::reader::MicroscopeReader;

// ─── Constants ──────────────────────────────────────

/// Wire format magic for snapshot exchange.
const WIRE_MAGIC: &[u8; 4] = b"EXS1";

/// Snapshots older than this are ignored in blending.
const SNAPSHOT_EXPIRY_MS: u64 = 172_800_000; // 48h

/// Max remote snapshots stored.
const MAX_REMOTE_SNAPSHOTS: usize = 50;

/// Emotional layer ID (index 4 in LAYER_NAMES).
const EMOTIONAL_LAYER_ID: u8 = 4;

/// Snapshot record size in bytes.
const SNAPSHOT_BYTES: usize = 40; // 8+8+4+4+4+4+4+4

// ─── Positive / negative word lists for valence ─────

const POSITIVE_WORDS: &[&str] = &[
    "good",
    "great",
    "happy",
    "love",
    "joy",
    "wonderful",
    "excellent",
    "amazing",
    "beautiful",
    "success",
    "hope",
    "peace",
    "calm",
    "warm",
    "bright",
    "kind",
    "jó",
    "szép",
    "boldog",
    "öröm",
    "szeretet",
    "csodás",
    "remek",
    "siker",
];

const NEGATIVE_WORDS: &[&str] = &[
    "bad",
    "sad",
    "angry",
    "hate",
    "fear",
    "terrible",
    "awful",
    "pain",
    "dark",
    "fail",
    "loss",
    "cold",
    "broken",
    "wrong",
    "ugly",
    "hurt",
    "rossz",
    "szomorú",
    "fájdalom",
    "harag",
    "félelem",
    "kudarc",
    "sötét",
];

// ─── Types ──────────────────────────────────────────

/// Compact emotional field summary shared between instances.
#[derive(Clone, Debug)]
pub struct EmotionalSnapshot {
    pub timestamp_ms: u64,
    pub source_id: u64,
    pub centroid: (f32, f32, f32),
    pub total_energy: f32,
    pub active_blocks: u32,
    pub valence: f32, // -1.0 (negative) to +1.0 (positive)
}

/// Persistent emotional contagion state.
pub struct EmotionalContagionState {
    pub instance_id: u64,
    pub local_snapshot: Option<EmotionalSnapshot>,
    pub remote_snapshots: Vec<EmotionalSnapshot>,
}

pub struct EmotionalContagionStats {
    pub instance_id: u64,
    pub has_local: bool,
    pub remote_count: usize,
    pub local_energy: f32,
    pub local_valence: f32,
    pub blended_valence: f32,
}

// ─── EmotionalContagionState ────────────────────────

impl EmotionalContagionState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("emotional_field.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 16 && &data[0..4] == b"EMO1" {
                let instance_id = read_u64(&data, 4);
                let snapshot_count = read_u32(&data, 12) as usize;
                let mut pos = 16;

                // First snapshot is local (if present)
                let local_snapshot = if snapshot_count > 0 && pos + SNAPSHOT_BYTES <= data.len() {
                    let snap = decode_snapshot_at(&data, pos);
                    pos += SNAPSHOT_BYTES;
                    Some(snap)
                } else {
                    None
                };

                let mut remote_snapshots = Vec::new();
                for _ in 1..snapshot_count {
                    if pos + SNAPSHOT_BYTES > data.len() {
                        break;
                    }
                    remote_snapshots.push(decode_snapshot_at(&data, pos));
                    pos += SNAPSHOT_BYTES;
                }

                return Self {
                    instance_id,
                    local_snapshot,
                    remote_snapshots,
                };
            }
        }

        // Generate instance ID from output_dir hash
        let instance_id = hash_path(output_dir);
        Self {
            instance_id,
            local_snapshot: None,
            remote_snapshots: Vec::new(),
        }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("emotional_field.bin");
        let total = if self.local_snapshot.is_some() {
            1 + self.remote_snapshots.len()
        } else {
            0
        };
        let mut buf = Vec::with_capacity(16 + total * SNAPSHOT_BYTES);
        buf.extend_from_slice(b"EMO1");
        buf.extend_from_slice(&self.instance_id.to_le_bytes());
        buf.extend_from_slice(&(total as u32).to_le_bytes());

        if let Some(ref snap) = self.local_snapshot {
            encode_snapshot_into(snap, &mut buf);
        }
        for snap in &self.remote_snapshots {
            encode_snapshot_into(snap, &mut buf);
        }

        fs::write(&path, &buf).map_err(|e| format!("write emotional_field.bin: {}", e))
    }

    /// Capture the current local emotional field as a snapshot.
    pub fn capture_local(&mut self, reader: &MicroscopeReader, hebb: &HebbianState) {
        let now = now_ms();
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut sum_z = 0.0f32;
        let mut total_energy = 0.0f32;
        let mut active_count = 0u32;
        let mut text_samples: Vec<String> = Vec::new();

        for i in 0..reader.block_count {
            let h = reader.header(i);
            if h.layer_id != EMOTIONAL_LAYER_ID {
                continue;
            }
            let energy = hebb.energy(i);
            if energy < 0.01 {
                continue;
            }

            sum_x += h.x * energy;
            sum_y += h.y * energy;
            sum_z += h.z * energy;
            total_energy += energy;
            active_count += 1;

            if text_samples.len() < 20 {
                text_samples.push(reader.text(i).to_lowercase());
            }
        }

        if active_count == 0 {
            self.local_snapshot = None;
            return;
        }

        let centroid = (
            sum_x / total_energy,
            sum_y / total_energy,
            sum_z / total_energy,
        );

        let valence = compute_valence(&text_samples);

        self.local_snapshot = Some(EmotionalSnapshot {
            timestamp_ms: now,
            source_id: self.instance_id,
            centroid,
            total_energy,
            active_blocks: active_count,
            valence,
        });
    }

    /// Export local snapshot as wire-format bytes.
    pub fn export_snapshot(&self) -> Vec<u8> {
        match &self.local_snapshot {
            Some(snap) => {
                let mut buf = Vec::with_capacity(4 + SNAPSHOT_BYTES);
                buf.extend_from_slice(WIRE_MAGIC);
                encode_snapshot_into(snap, &mut buf);
                buf
            }
            None => Vec::new(),
        }
    }

    /// Import a remote snapshot from wire-format bytes.
    pub fn import_snapshot(data: &[u8]) -> Option<EmotionalSnapshot> {
        if data.len() < 4 + SNAPSHOT_BYTES {
            return None;
        }
        if &data[0..4] != WIRE_MAGIC {
            return None;
        }
        Some(decode_snapshot_at(data, 4))
    }

    /// Receive a remote snapshot and store it.
    pub fn receive_remote(&mut self, snap: EmotionalSnapshot) {
        // Don't import our own snapshots
        if snap.source_id == self.instance_id {
            return;
        }
        // Replace existing from same source
        self.remote_snapshots
            .retain(|s| s.source_id != snap.source_id);
        self.remote_snapshots.push(snap);
        // Cap
        if self.remote_snapshots.len() > MAX_REMOTE_SNAPSHOTS {
            self.remote_snapshots
                .drain(0..self.remote_snapshots.len() - MAX_REMOTE_SNAPSHOTS);
        }
    }

    /// Compute blended emotional centroid from local + remote snapshots.
    /// local_weight controls how much local vs remote influences the result.
    /// Returns None if no active emotional state exists.
    pub fn blended_centroid(&self, local_weight: f32) -> Option<(f32, f32, f32)> {
        let now = now_ms();
        let local_w = local_weight.clamp(0.0, 1.0);

        let mut total_weight = 0.0f32;
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut cz = 0.0f32;

        if let Some(ref snap) = self.local_snapshot {
            let w = snap.total_energy * local_w;
            cx += snap.centroid.0 * w;
            cy += snap.centroid.1 * w;
            cz += snap.centroid.2 * w;
            total_weight += w;
        }

        let remote_w = 1.0 - local_w;
        for snap in &self.remote_snapshots {
            // Skip expired
            if now.saturating_sub(snap.timestamp_ms) > SNAPSHOT_EXPIRY_MS {
                continue;
            }
            // Recency decay: linear from 1.0 (fresh) to 0.1 (48h old)
            let age = (now - snap.timestamp_ms) as f32 / SNAPSHOT_EXPIRY_MS as f32;
            let recency = 1.0 - 0.9 * age;
            let w = snap.total_energy * remote_w * recency;

            cx += snap.centroid.0 * w;
            cy += snap.centroid.1 * w;
            cz += snap.centroid.2 * w;
            total_weight += w;
        }

        if total_weight < 0.001 {
            return None;
        }

        Some((cx / total_weight, cy / total_weight, cz / total_weight))
    }

    /// Apply contagion: warp query coords toward blended emotional centroid.
    pub fn apply_contagion(
        &self,
        qx: f32,
        qy: f32,
        qz: f32,
        weight: f32,
        local_weight: f32,
    ) -> (f32, f32, f32) {
        if weight <= 0.0 {
            return (qx, qy, qz);
        }
        match self.blended_centroid(local_weight) {
            Some((cx, cy, cz)) => {
                let w = weight.clamp(0.0, 1.0);
                (qx + (cx - qx) * w, qy + (cy - qy) * w, qz + (cz - qz) * w)
            }
            None => (qx, qy, qz),
        }
    }

    pub fn stats(&self) -> EmotionalContagionStats {
        let local_energy = self
            .local_snapshot
            .as_ref()
            .map(|s| s.total_energy)
            .unwrap_or(0.0);
        let local_valence = self
            .local_snapshot
            .as_ref()
            .map(|s| s.valence)
            .unwrap_or(0.0);

        // Compute blended valence
        let mut total_w = 0.0f32;
        let mut val_sum = 0.0f32;
        if let Some(ref snap) = self.local_snapshot {
            total_w += snap.total_energy;
            val_sum += snap.valence * snap.total_energy;
        }
        for snap in &self.remote_snapshots {
            total_w += snap.total_energy;
            val_sum += snap.valence * snap.total_energy;
        }
        let blended_valence = if total_w > 0.0 {
            val_sum / total_w
        } else {
            0.0
        };

        EmotionalContagionStats {
            instance_id: self.instance_id,
            has_local: self.local_snapshot.is_some(),
            remote_count: self.remote_snapshots.len(),
            local_energy,
            local_valence,
            blended_valence,
        }
    }
}

// ─── Valence computation ────────────────────────────

/// Compute valence from text samples using keyword sentiment.
/// Returns -1.0 to +1.0.
pub fn compute_valence(texts: &[String]) -> f32 {
    let mut positive = 0u32;
    let mut negative = 0u32;

    for text in texts {
        let words: Vec<&str> = text.split_whitespace().collect();
        for word in &words {
            let w = word.trim_matches(|c: char| !c.is_alphanumeric());
            if POSITIVE_WORDS.contains(&w) {
                positive += 1;
            }
            if NEGATIVE_WORDS.contains(&w) {
                negative += 1;
            }
        }
    }

    let total = positive + negative;
    if total == 0 {
        return 0.0; // neutral
    }

    // Map to -1..+1: all positive = +1, all negative = -1
    (positive as f32 - negative as f32) / total as f32
}

// ─── Binary helpers ─────────────────────────────────

fn encode_snapshot_into(snap: &EmotionalSnapshot, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&snap.timestamp_ms.to_le_bytes());
    buf.extend_from_slice(&snap.source_id.to_le_bytes());
    buf.extend_from_slice(&snap.centroid.0.to_le_bytes());
    buf.extend_from_slice(&snap.centroid.1.to_le_bytes());
    buf.extend_from_slice(&snap.centroid.2.to_le_bytes());
    buf.extend_from_slice(&snap.total_energy.to_le_bytes());
    buf.extend_from_slice(&snap.active_blocks.to_le_bytes());
    buf.extend_from_slice(&snap.valence.to_le_bytes());
}

fn decode_snapshot_at(data: &[u8], off: usize) -> EmotionalSnapshot {
    EmotionalSnapshot {
        timestamp_ms: read_u64(data, off),
        source_id: read_u64(data, off + 8),
        centroid: (
            read_f32(data, off + 16),
            read_f32(data, off + 20),
            read_f32(data, off + 24),
        ),
        total_energy: read_f32(data, off + 28),
        active_blocks: read_u32(data, off + 32),
        valence: read_f32(data, off + 36),
    }
}

fn read_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn read_u64(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
fn read_f32(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn hash_path(path: &Path) -> u64 {
    let s = path.to_string_lossy();
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in s.as_bytes() {
        h = h.wrapping_mul(0x100000001b3) ^ b as u64;
    }
    h
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_contagion_at_zero_weight() {
        let state = EmotionalContagionState {
            instance_id: 1,
            local_snapshot: Some(EmotionalSnapshot {
                timestamp_ms: now_ms(),
                source_id: 1,
                centroid: (0.2, 0.3, 0.4),
                total_energy: 1.0,
                active_blocks: 5,
                valence: 0.5,
            }),
            remote_snapshots: Vec::new(),
        };
        let (x, y, z) = state.apply_contagion(0.5, 0.5, 0.5, 0.0, 0.7);
        assert!((x - 0.5).abs() < 0.001);
        assert!((y - 0.5).abs() < 0.001);
        assert!((z - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_local_only_contagion() {
        let state = EmotionalContagionState {
            instance_id: 1,
            local_snapshot: Some(EmotionalSnapshot {
                timestamp_ms: now_ms(),
                source_id: 1,
                centroid: (0.2, 0.3, 0.4),
                total_energy: 1.0,
                active_blocks: 5,
                valence: 0.5,
            }),
            remote_snapshots: Vec::new(),
        };

        let centroid = state.blended_centroid(0.7).unwrap();
        // Only local, so centroid should be local centroid
        assert!((centroid.0 - 0.2).abs() < 0.001);
        assert!((centroid.1 - 0.3).abs() < 0.001);
        assert!((centroid.2 - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_remote_blend() {
        let now = now_ms();
        let state = EmotionalContagionState {
            instance_id: 1,
            local_snapshot: Some(EmotionalSnapshot {
                timestamp_ms: now,
                source_id: 1,
                centroid: (0.0, 0.0, 0.0),
                total_energy: 1.0,
                active_blocks: 5,
                valence: 0.5,
            }),
            remote_snapshots: vec![EmotionalSnapshot {
                timestamp_ms: now,
                source_id: 2,
                centroid: (1.0, 1.0, 1.0),
                total_energy: 1.0,
                active_blocks: 5,
                valence: -0.5,
            }],
        };

        // With equal local/remote weight
        let centroid = state.blended_centroid(0.5).unwrap();
        // Should be somewhere between (0,0,0) and (1,1,1)
        assert!(centroid.0 > 0.1 && centroid.0 < 0.9);
        assert!(centroid.1 > 0.1 && centroid.1 < 0.9);
    }

    #[test]
    fn test_valence_computation() {
        let texts = vec![
            "I feel happy and great today".to_string(),
            "wonderful success with joy".to_string(),
        ];
        let v = compute_valence(&texts);
        assert!(v > 0.5); // strongly positive

        let texts2 = vec![
            "bad pain and dark fear".to_string(),
            "terrible loss and hurt".to_string(),
        ];
        let v2 = compute_valence(&texts2);
        assert!(v2 < -0.5); // strongly negative

        let empty: Vec<String> = Vec::new();
        assert_eq!(compute_valence(&empty), 0.0); // neutral
    }

    #[test]
    fn test_snapshot_wire_roundtrip() {
        let snap = EmotionalSnapshot {
            timestamp_ms: 12345678,
            source_id: 99,
            centroid: (0.1, 0.2, 0.3),
            total_energy: 2.5,
            active_blocks: 10,
            valence: -0.3,
        };

        let state = EmotionalContagionState {
            instance_id: 99,
            local_snapshot: Some(snap),
            remote_snapshots: Vec::new(),
        };

        let wire = state.export_snapshot();
        let decoded = EmotionalContagionState::import_snapshot(&wire).unwrap();
        assert_eq!(decoded.source_id, 99);
        assert!((decoded.centroid.0 - 0.1).abs() < 0.001);
        assert!((decoded.valence - (-0.3)).abs() < 0.001);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let now = now_ms();

        let state = EmotionalContagionState {
            instance_id: 42,
            local_snapshot: Some(EmotionalSnapshot {
                timestamp_ms: now,
                source_id: 42,
                centroid: (0.1, 0.2, 0.3),
                total_energy: 1.5,
                active_blocks: 7,
                valence: 0.4,
            }),
            remote_snapshots: vec![EmotionalSnapshot {
                timestamp_ms: now - 1000,
                source_id: 99,
                centroid: (0.4, 0.5, 0.6),
                total_energy: 0.8,
                active_blocks: 3,
                valence: -0.2,
            }],
        };

        state.save(tmp.path()).unwrap();
        let loaded = EmotionalContagionState::load_or_init(tmp.path());
        assert_eq!(loaded.instance_id, 42);
        assert!(loaded.local_snapshot.is_some());
        assert_eq!(loaded.remote_snapshots.len(), 1);
        assert_eq!(loaded.remote_snapshots[0].source_id, 99);
    }

    #[test]
    fn test_snapshot_expiry() {
        let state = EmotionalContagionState {
            instance_id: 1,
            local_snapshot: None,
            remote_snapshots: vec![EmotionalSnapshot {
                timestamp_ms: 1000, // very old
                source_id: 2,
                centroid: (0.5, 0.5, 0.5),
                total_energy: 1.0,
                active_blocks: 5,
                valence: 0.0,
            }],
        };

        // Old snapshot should be ignored
        let centroid = state.blended_centroid(0.5);
        assert!(centroid.is_none());
    }

    #[test]
    fn test_receive_remote_dedup() {
        let now = now_ms();
        let mut state = EmotionalContagionState {
            instance_id: 1,
            local_snapshot: None,
            remote_snapshots: Vec::new(),
        };

        let snap1 = EmotionalSnapshot {
            timestamp_ms: now,
            source_id: 2,
            centroid: (0.1, 0.1, 0.1),
            total_energy: 0.5,
            active_blocks: 3,
            valence: 0.3,
        };
        state.receive_remote(snap1);
        assert_eq!(state.remote_snapshots.len(), 1);

        // Same source, new snapshot replaces
        let snap2 = EmotionalSnapshot {
            timestamp_ms: now + 1000,
            source_id: 2,
            centroid: (0.2, 0.2, 0.2),
            total_energy: 0.8,
            active_blocks: 5,
            valence: 0.6,
        };
        state.receive_remote(snap2);
        assert_eq!(state.remote_snapshots.len(), 1);
        assert!((state.remote_snapshots[0].centroid.0 - 0.2).abs() < 0.001);
    }
}
