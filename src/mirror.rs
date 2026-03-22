//! Mirror neuron layer for Microscope Memory.
//!
//! "When one pattern fires, similar past patterns resonate."
//!
//! Compares incoming activation fingerprints against stored ones.
//! High-resonance matches boost recall — the system "recognizes" familiar
//! activation shapes and strengthens related pathways.
//!
//! Binary format: resonance.bin (RES1)

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::hebbian::{ActivationFingerprint, HebbianState};

// ─── Constants ──────────────────────────────────────

/// Minimum cosine similarity to count as resonance.
const RESONANCE_THRESHOLD: f32 = 0.3;
/// Maximum number of resonance echoes to store.
const MAX_ECHOES: usize = 500;
/// Decay factor for older echoes (per-step multiplier).
const ECHO_DECAY: f32 = 0.95;

// ─── Types ──────────────────────────────────────────

/// A resonance echo — records when a new query matched a past fingerprint.
#[derive(Clone, Debug)]
pub struct ResonanceEcho {
    /// Timestamp of the resonance event.
    pub timestamp_ms: u64,
    /// The new query's fingerprint hash.
    pub trigger_hash: u64,
    /// The matched past fingerprint hash.
    pub echo_hash: u64,
    /// Cosine similarity between the two activation patterns.
    pub similarity: f32,
    /// Which blocks were in the intersection (amplified blocks).
    pub shared_blocks: Vec<u32>,
}

/// Per-block resonance accumulator.
#[derive(Clone, Debug, Default)]
pub struct BlockResonance {
    /// How many times this block appeared in resonance echoes.
    pub echo_count: u32,
    /// Accumulated resonance strength (decays over time).
    pub strength: f32,
}

/// Mirror neuron state — loaded from resonance.bin.
pub struct MirrorState {
    pub echoes: Vec<ResonanceEcho>,
    pub block_resonance: HashMap<u32, BlockResonance>,
}

impl MirrorState {
    /// Load or initialize mirror state.
    pub fn load_or_init(output_dir: &Path) -> Self {
        load_mirror_state(output_dir).unwrap_or_else(|| Self {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        })
    }

    /// Detect resonance between a new activation and all stored fingerprints.
    /// Returns the resonance boost per block (block_idx → boost_score).
    pub fn detect_resonance(
        &mut self,
        new_activations: &[(u32, f32)],
        new_hash: u64,
        fingerprints: &[ActivationFingerprint],
    ) -> HashMap<u32, f32> {
        let mut boosts: HashMap<u32, f32> = HashMap::new();

        if new_activations.is_empty() || fingerprints.is_empty() {
            return boosts;
        }

        // Build sparse vector for the new activation
        let new_vec = activation_to_sparse(new_activations);

        // Compare against all stored fingerprints (skip self — same hash)
        for fp in fingerprints {
            if fp.query_hash == new_hash {
                continue;
            }

            let past_vec = activation_to_sparse(&fp.activations);
            let sim = sparse_cosine(&new_vec, &past_vec);

            if sim >= RESONANCE_THRESHOLD {
                // Find shared blocks
                let shared: Vec<u32> = new_vec
                    .keys()
                    .filter(|k| past_vec.contains_key(k))
                    .copied()
                    .collect();

                // Boost shared blocks proportional to similarity
                for &block_idx in &shared {
                    let entry = boosts.entry(block_idx).or_insert(0.0);
                    *entry += sim * 0.1; // resonance boost factor

                    let res = self
                        .block_resonance
                        .entry(block_idx)
                        .or_default();
                    res.echo_count += 1;
                    res.strength += sim;
                }

                // Record the echo
                let now_ms = crate::hebbian::now_epoch_ms_pub();
                self.echoes.push(ResonanceEcho {
                    timestamp_ms: now_ms,
                    trigger_hash: new_hash,
                    echo_hash: fp.query_hash,
                    similarity: sim,
                    shared_blocks: shared,
                });
            }
        }

        // Trim echoes
        if self.echoes.len() > MAX_ECHOES {
            self.echoes.drain(0..self.echoes.len() - MAX_ECHOES);
        }

        boosts
    }

    /// Decay all block resonance strengths (call periodically).
    pub fn decay(&mut self) {
        self.block_resonance.retain(|_, res| {
            res.strength *= ECHO_DECAY;
            res.strength > 0.01
        });
    }

    /// Get resonance boost for a specific block.
    pub fn boost_for(&self, block_idx: u32) -> f32 {
        self.block_resonance
            .get(&block_idx)
            .map(|r| r.strength.min(0.5)) // cap boost at 0.5
            .unwrap_or(0.0)
    }

    /// Get statistics.
    pub fn stats(&self) -> MirrorStats {
        let total_echoes = self.echoes.len();
        let resonant_blocks = self.block_resonance.len();
        let avg_similarity = if self.echoes.is_empty() {
            0.0
        } else {
            self.echoes.iter().map(|e| e.similarity).sum::<f32>() / self.echoes.len() as f32
        };
        let strongest = self
            .block_resonance
            .iter()
            .max_by(|a, b| a.1.strength.partial_cmp(&b.1.strength).unwrap())
            .map(|(&idx, res)| (idx, res.strength));

        MirrorStats {
            total_echoes,
            resonant_blocks,
            avg_similarity,
            strongest_block: strongest,
        }
    }

    /// Get top-N most resonant blocks.
    pub fn most_resonant(&self, n: usize) -> Vec<(u32, &BlockResonance)> {
        let mut blocks: Vec<(u32, &BlockResonance)> =
            self.block_resonance.iter().map(|(&k, v)| (k, v)).collect();
        blocks.sort_by(|a, b| b.1.strength.partial_cmp(&a.1.strength).unwrap());
        blocks.truncate(n);
        blocks
    }

    /// Save mirror state to disk.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_mirror_state(output_dir, self)
    }
}

pub struct MirrorStats {
    pub total_echoes: usize,
    pub resonant_blocks: usize,
    pub avg_similarity: f32,
    pub strongest_block: Option<(u32, f32)>,
}

// ─── Sparse vector operations ───────────────────────

fn activation_to_sparse(activations: &[(u32, f32)]) -> HashMap<u32, f32> {
    activations.iter().copied().collect()
}

fn sparse_cosine(a: &HashMap<u32, f32>, b: &HashMap<u32, f32>) -> f32 {
    let (smaller, larger) = if a.len() <= b.len() { (a, b) } else { (b, a) };

    let mut dot = 0.0f32;
    for (k, va) in smaller {
        if let Some(vb) = larger.get(k) {
            dot += va * vb;
        }
    }

    let norm_a: f32 = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let norm_b: f32 = b.values().map(|v| v * v).sum::<f32>().sqrt();

    if norm_a < 1e-9 || norm_b < 1e-9 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

// ─── Binary I/O ─────────────────────────────────────
//
// resonance.bin format:
//   magic: b"RES1" (4 bytes)
//   echo_count: u32 (4 bytes)
//   block_count: u32 (4 bytes)
//   echoes: [echo_count × variable-length echo records]
//   block_resonance: [block_count × 12 bytes (u32 idx + u32 count + f32 strength)]

fn load_mirror_state(output_dir: &Path) -> Option<MirrorState> {
    let path = output_dir.join("resonance.bin");
    let data = fs::read(&path).ok()?;
    if data.len() < 12 || &data[0..4] != b"RES1" {
        return None;
    }

    let echo_count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let block_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

    let mut pos = 12;
    let mut echoes = Vec::with_capacity(echo_count);
    for _ in 0..echo_count {
        if pos + 28 > data.len() {
            break;
        }
        let timestamp_ms = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        let trigger_hash = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().unwrap());
        let echo_hash = u64::from_le_bytes(data[pos + 16..pos + 24].try_into().unwrap());
        let similarity = f32::from_le_bytes(data[pos + 24..pos + 28].try_into().unwrap());
        let shared_count =
            u16::from_le_bytes(data[pos + 28..pos + 30].try_into().unwrap()) as usize;
        pos += 30;

        let mut shared_blocks = Vec::with_capacity(shared_count);
        for _ in 0..shared_count {
            if pos + 4 > data.len() {
                break;
            }
            shared_blocks.push(u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()));
            pos += 4;
        }

        echoes.push(ResonanceEcho {
            timestamp_ms,
            trigger_hash,
            echo_hash,
            similarity,
            shared_blocks,
        });
    }

    let mut block_resonance = HashMap::with_capacity(block_count);
    for _ in 0..block_count {
        if pos + 12 > data.len() {
            break;
        }
        let idx = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        let echo_count = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
        let strength = f32::from_le_bytes(data[pos + 8..pos + 12].try_into().unwrap());
        pos += 12;
        block_resonance.insert(
            idx,
            BlockResonance {
                echo_count,
                strength,
            },
        );
    }

    Some(MirrorState {
        echoes,
        block_resonance,
    })
}

fn save_mirror_state(output_dir: &Path, state: &MirrorState) -> Result<(), String> {
    let path = output_dir.join("resonance.bin");
    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(b"RES1");
    buf.extend_from_slice(&(state.echoes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(state.block_resonance.len() as u32).to_le_bytes());

    // Echoes
    for echo in &state.echoes {
        buf.extend_from_slice(&echo.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&echo.trigger_hash.to_le_bytes());
        buf.extend_from_slice(&echo.echo_hash.to_le_bytes());
        buf.extend_from_slice(&echo.similarity.to_le_bytes());
        buf.extend_from_slice(&(echo.shared_blocks.len() as u16).to_le_bytes());
        for &block_idx in &echo.shared_blocks {
            buf.extend_from_slice(&block_idx.to_le_bytes());
        }
    }

    // Block resonance
    for (&idx, res) in &state.block_resonance {
        buf.extend_from_slice(&idx.to_le_bytes());
        buf.extend_from_slice(&res.echo_count.to_le_bytes());
        buf.extend_from_slice(&res.strength.to_le_bytes());
    }

    fs::write(&path, &buf).map_err(|e| format!("write resonance.bin: {}", e))
}

// ─── Integration helper ─────────────────────────────

/// Full mirror neuron cycle: detect resonance and return boosted scores.
/// Call this after Hebbian activation recording.
pub fn mirror_boost(
    hebb: &HebbianState,
    mirror: &mut MirrorState,
    new_activations: &[(u32, f32)],
    query_hash: u64,
) -> Vec<(u32, f32)> {
    let boosts = mirror.detect_resonance(new_activations, query_hash, &hebb.fingerprints);

    // Return sorted by boost strength
    let mut result: Vec<(u32, f32)> = boosts.into_iter().collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_cosine_identical() {
        let a: HashMap<u32, f32> = [(0, 1.0), (1, 0.5), (2, 0.3)].into();
        let sim = sparse_cosine(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sparse_cosine_orthogonal() {
        let a: HashMap<u32, f32> = [(0, 1.0), (1, 0.5)].into();
        let b: HashMap<u32, f32> = [(2, 1.0), (3, 0.5)].into();
        let sim = sparse_cosine(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_sparse_cosine_partial_overlap() {
        let a: HashMap<u32, f32> = [(0, 1.0), (1, 0.5), (2, 0.3)].into();
        let b: HashMap<u32, f32> = [(1, 0.8), (2, 0.6), (3, 0.4)].into();
        let sim = sparse_cosine(&a, &b);
        assert!(sim > 0.0);
        assert!(sim < 1.0);
    }

    #[test]
    fn test_detect_resonance_no_fingerprints() {
        let mut mirror = MirrorState {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        };
        let boosts = mirror.detect_resonance(&[(0, 1.0), (1, 0.5)], 42, &[]);
        assert!(boosts.is_empty());
    }

    #[test]
    fn test_detect_resonance_with_match() {
        let mut mirror = MirrorState {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        };

        // Past fingerprint: blocks 0, 1, 2 activated
        let past = ActivationFingerprint {
            timestamp_ms: 1000,
            query_hash: 100,
            activations: vec![(0, 0.9), (1, 0.7), (2, 0.5)],
        };

        // New activation: blocks 0, 1, 3 — overlaps on 0, 1
        let new_act = vec![(0, 0.8), (1, 0.6), (3, 0.4)];
        let boosts = mirror.detect_resonance(&new_act, 200, &[past]);

        // Should have resonance on shared blocks (0, 1)
        assert!(!boosts.is_empty());
        assert!(boosts.contains_key(&0));
        assert!(boosts.contains_key(&1));
        assert!(!boosts.contains_key(&3)); // not shared

        // Echo should be recorded
        assert_eq!(mirror.echoes.len(), 1);
        assert_eq!(mirror.echoes[0].trigger_hash, 200);
        assert_eq!(mirror.echoes[0].echo_hash, 100);
    }

    #[test]
    fn test_decay() {
        let mut mirror = MirrorState {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        };

        mirror.block_resonance.insert(
            0,
            BlockResonance {
                echo_count: 5,
                strength: 1.0,
            },
        );
        mirror.block_resonance.insert(
            1,
            BlockResonance {
                echo_count: 1,
                strength: 0.005,
            },
        );

        mirror.decay();

        // Block 0 should still exist (0.95)
        assert!(mirror.block_resonance.contains_key(&0));
        // Block 1 should be removed (below 0.01)
        assert!(!mirror.block_resonance.contains_key(&1));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let dir = tmp.path();

        let mut mirror = MirrorState {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        };

        mirror.echoes.push(ResonanceEcho {
            timestamp_ms: 12345,
            trigger_hash: 100,
            echo_hash: 200,
            similarity: 0.75,
            shared_blocks: vec![1, 5, 10],
        });
        mirror.block_resonance.insert(
            1,
            BlockResonance {
                echo_count: 3,
                strength: 0.8,
            },
        );
        mirror.block_resonance.insert(
            5,
            BlockResonance {
                echo_count: 1,
                strength: 0.3,
            },
        );

        mirror.save(dir).expect("save");

        let loaded = MirrorState::load_or_init(dir);
        assert_eq!(loaded.echoes.len(), 1);
        assert_eq!(loaded.echoes[0].trigger_hash, 100);
        assert_eq!(loaded.echoes[0].shared_blocks, vec![1, 5, 10]);
        assert_eq!(loaded.block_resonance.len(), 2);
        assert_eq!(loaded.block_resonance[&1].echo_count, 3);
        assert!((loaded.block_resonance[&5].strength - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_most_resonant() {
        let mut mirror = MirrorState {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        };

        mirror.block_resonance.insert(
            0,
            BlockResonance {
                echo_count: 1,
                strength: 0.1,
            },
        );
        mirror.block_resonance.insert(
            1,
            BlockResonance {
                echo_count: 5,
                strength: 0.9,
            },
        );
        mirror.block_resonance.insert(
            2,
            BlockResonance {
                echo_count: 3,
                strength: 0.5,
            },
        );

        let top = mirror.most_resonant(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, 1); // strongest
        assert_eq!(top[1].0, 2);
    }

    #[test]
    fn test_mirror_boost_integration() {
        let hebb = HebbianState {
            activations: vec![crate::hebbian::ActivationRecord::default(); 5],
            coactivations: HashMap::new(),
            fingerprints: vec![ActivationFingerprint {
                timestamp_ms: 1000,
                query_hash: 100,
                activations: vec![(0, 0.9), (1, 0.7), (2, 0.5)],
            }],
        };

        let mut mirror = MirrorState {
            echoes: Vec::new(),
            block_resonance: HashMap::new(),
        };

        let new_act = vec![(0, 0.8), (1, 0.6), (3, 0.4)];
        let result = mirror_boost(&hebb, &mut mirror, &new_act, 200);

        // Should return boosted blocks that were shared
        assert!(!result.is_empty());
    }
}
