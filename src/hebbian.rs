//! Hebbian learning layer for Microscope Memory.
//!
//! "Neurons that fire together wire together."
//!
//! Tracks block activations and co-activations from queries.
//! Over time, frequently co-activated blocks drift their coordinates closer.
//! Energy decays exponentially — recently active blocks are "hot".
//!
//! Binary formats:
//!   activations.bin — per-block activation state (HEB1)
//!   coactivations.bin — sparse co-activation pairs (COA1)
//!   fingerprints.bin — activation fingerprints for mirror neurons (FPR1)

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

const ACTIVATION_RECORD_BYTES: usize = 32; // manual serialization, not sizeof
const COACTIVATION_RECORD_BYTES: usize = 20; // manual serialization, not sizeof
const ENERGY_HALF_LIFE_MS: f64 = 86_400_000.0; // 24 hours
const DRIFT_RATE: f32 = 0.01; // how fast coordinates move per Hebbian step
const DRIFT_MAX: f32 = 0.1; // maximum drift from original position

// ─── Activation state per block ─────────────────────

/// Per-block activation record: 32 bytes, stored in activations.bin.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ActivationRecord {
    pub activation_count: u32,
    pub last_activated_ms: u64,
    pub drift_x: f32,
    pub drift_y: f32,
    pub drift_z: f32,
    pub energy: f32,
    pub _pad: u32,
}

impl Default for ActivationRecord {
    fn default() -> Self {
        Self {
            activation_count: 0,
            last_activated_ms: 0,
            drift_x: 0.0,
            drift_y: 0.0,
            drift_z: 0.0,
            energy: 0.0,
            _pad: 0,
        }
    }
}

/// Co-activation pair: 20 bytes, stored in coactivations.bin.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CoactivationPair {
    pub block_a: u32,
    pub block_b: u32,
    pub count: u32,
    pub last_ts_ms: u64,
}

/// Activation fingerprint — snapshot of a query's activation pattern.
/// Used for mirror neuron resonance (future).
#[derive(Clone, Debug)]
pub struct ActivationFingerprint {
    pub timestamp_ms: u64,
    pub query_hash: u64,
    pub activations: Vec<(u32, f32)>, // (block_idx, score)
}

// ─── HebbianState ───────────────────────────────────

/// In-memory Hebbian state, loaded from binary files.
pub struct HebbianState {
    pub activations: Vec<ActivationRecord>,
    pub coactivations: HashMap<(u32, u32), CoactivationPair>,
    pub fingerprints: Vec<ActivationFingerprint>,
}

impl HebbianState {
    /// Load or initialize Hebbian state for a given block count.
    pub fn load_or_init(output_dir: &Path, block_count: usize) -> Self {
        let activations = load_activations(output_dir, block_count);
        let coactivations = load_coactivations(output_dir);
        let fingerprints = load_fingerprints(output_dir);

        Self {
            activations,
            coactivations,
            fingerprints,
        }
    }

    /// Record that a set of blocks were activated together by a query.
    /// This is the core Hebbian learning signal.
    pub fn record_activation(&mut self, results: &[(u32, f32)], query_hash: u64) {
        let now_ms = now_epoch_ms();

        // Update per-block activation records
        for &(block_idx, _score) in results {
            let idx = block_idx as usize;
            if idx < self.activations.len() {
                let rec = &mut self.activations[idx];
                rec.activation_count += 1;
                rec.last_activated_ms = now_ms;
                rec.energy = 1.0; // fresh activation = max energy
            }
        }

        // Record co-activations for all pairs
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let a = results[i].0.min(results[j].0);
                let b = results[i].0.max(results[j].0);
                let pair = self
                    .coactivations
                    .entry((a, b))
                    .or_insert(CoactivationPair {
                        block_a: a,
                        block_b: b,
                        count: 0,
                        last_ts_ms: 0,
                    });
                pair.count += 1;
                pair.last_ts_ms = now_ms;
            }
        }

        // Store activation fingerprint (for mirror neurons)
        self.fingerprints.push(ActivationFingerprint {
            timestamp_ms: now_ms,
            query_hash,
            activations: results.to_vec(),
        });

        // Keep fingerprints bounded (last 1000)
        if self.fingerprints.len() > 1000 {
            self.fingerprints.drain(0..self.fingerprints.len() - 1000);
        }
    }

    /// Apply Hebbian drift: co-activated blocks pull each other's coordinates closer.
    /// Call this during rebuild or periodically.
    pub fn apply_drift(&mut self, headers: &[(f32, f32, f32)]) {
        let now_ms = now_epoch_ms();

        // First: decay all energies
        for rec in &mut self.activations {
            if rec.energy > 0.0 && rec.last_activated_ms > 0 {
                let elapsed_ms = (now_ms - rec.last_activated_ms) as f64;
                rec.energy *= (-(elapsed_ms / ENERGY_HALF_LIFE_MS) * std::f64::consts::LN_2) as f32;
                rec.energy = rec.energy.exp();
            }
        }

        // Apply Hebbian drift for co-activated pairs
        for pair in self.coactivations.values() {
            let a = pair.block_a as usize;
            let b = pair.block_b as usize;

            if a >= headers.len() || b >= headers.len() {
                continue;
            }

            // Strength proportional to co-activation count, capped
            let strength = (pair.count as f32).ln().min(5.0) * DRIFT_RATE;
            if strength < 0.001 {
                continue;
            }

            let (ax, ay, az) = headers[a];
            let (bx, by, bz) = headers[b];

            // Vector from A to B
            let dx = bx + self.activations[b].drift_x - (ax + self.activations[a].drift_x);
            let dy = by + self.activations[b].drift_y - (ay + self.activations[a].drift_y);
            let dz = bz + self.activations[b].drift_z - (az + self.activations[a].drift_z);

            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < 0.001 {
                continue;
            }

            // Move A toward B, and B toward A
            let nx = dx / dist * strength;
            let ny = dy / dist * strength;
            let nz = dz / dist * strength;

            self.activations[a].drift_x = clamp_drift(self.activations[a].drift_x + nx);
            self.activations[a].drift_y = clamp_drift(self.activations[a].drift_y + ny);
            self.activations[a].drift_z = clamp_drift(self.activations[a].drift_z + nz);

            self.activations[b].drift_x = clamp_drift(self.activations[b].drift_x - nx);
            self.activations[b].drift_y = clamp_drift(self.activations[b].drift_y - ny);
            self.activations[b].drift_z = clamp_drift(self.activations[b].drift_z - nz);
        }
    }

    /// Get effective coordinates for a block (original + Hebbian drift).
    pub fn effective_coords(&self, block_idx: usize, original: (f32, f32, f32)) -> (f32, f32, f32) {
        if block_idx < self.activations.len() {
            let rec = &self.activations[block_idx];
            (
                original.0 + rec.drift_x,
                original.1 + rec.drift_y,
                original.2 + rec.drift_z,
            )
        } else {
            original
        }
    }

    /// Get the energy (heat) of a block. 1.0 = just activated, decays toward 0.
    pub fn energy(&self, block_idx: usize) -> f32 {
        if block_idx < self.activations.len() {
            let rec = &self.activations[block_idx];
            if rec.energy > 0.0 && rec.last_activated_ms > 0 {
                let elapsed_ms = (now_epoch_ms() - rec.last_activated_ms) as f64;
                let decay = (-(elapsed_ms / ENERGY_HALF_LIFE_MS) * std::f64::consts::LN_2).exp();
                decay as f32
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Save all Hebbian state to binary files.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_activations(output_dir, &self.activations)?;
        save_coactivations(output_dir, &self.coactivations)?;
        save_fingerprints(output_dir, &self.fingerprints)?;
        Ok(())
    }

    /// Get statistics about the Hebbian state.
    pub fn stats(&self) -> HebbianStats {
        let active_blocks = self
            .activations
            .iter()
            .filter(|r| r.activation_count > 0)
            .count();
        let total_activations: u64 = self
            .activations
            .iter()
            .map(|r| r.activation_count as u64)
            .sum();
        let hot_blocks = self
            .activations
            .iter()
            .enumerate()
            .filter(|(i, _)| self.energy(*i) > 0.1)
            .count();
        let drifted_blocks = self
            .activations
            .iter()
            .filter(|r| {
                r.drift_x.abs() > 0.001 || r.drift_y.abs() > 0.001 || r.drift_z.abs() > 0.001
            })
            .count();

        HebbianStats {
            block_count: self.activations.len(),
            active_blocks,
            total_activations,
            hot_blocks,
            coactivation_pairs: self.coactivations.len(),
            fingerprint_count: self.fingerprints.len(),
            drifted_blocks,
        }
    }

    /// Get the latest activation fingerprint (for mirror neuron sharing).
    pub fn latest_fingerprint(&self) -> Option<&ActivationFingerprint> {
        self.fingerprints.last()
    }

    /// Get top-N most activated blocks.
    pub fn hottest_blocks(&self, n: usize) -> Vec<(usize, f32)> {
        let mut blocks: Vec<(usize, f32)> = (0..self.activations.len())
            .map(|i| (i, self.energy(i)))
            .filter(|(_, e)| *e > 0.01)
            .collect();
        blocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        blocks.truncate(n);
        blocks
    }

    /// Get top-N strongest co-activation pairs.
    pub fn strongest_pairs(&self, n: usize) -> Vec<&CoactivationPair> {
        let mut pairs: Vec<&CoactivationPair> = self.coactivations.values().collect();
        pairs.sort_by(|a, b| b.count.cmp(&a.count));
        pairs.truncate(n);
        pairs
    }
}

pub struct HebbianStats {
    pub block_count: usize,
    pub active_blocks: usize,
    pub total_activations: u64,
    pub hot_blocks: usize,
    pub coactivation_pairs: usize,
    pub fingerprint_count: usize,
    pub drifted_blocks: usize,
}

// ─── Binary I/O ─────────────────────────────────────

fn read_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn read_u64(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
fn read_f32(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn load_activations(output_dir: &Path, block_count: usize) -> Vec<ActivationRecord> {
    let path = output_dir.join("activations.bin");
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 8 && &data[0..4] == b"HEB1" {
            let stored_count = read_u32(&data, 4) as usize;
            let expected_size = 8 + stored_count * ACTIVATION_RECORD_BYTES;
            if data.len() >= expected_size {
                let mut records = Vec::with_capacity(block_count.max(stored_count));
                for i in 0..stored_count {
                    let off = 8 + i * ACTIVATION_RECORD_BYTES;
                    records.push(ActivationRecord {
                        activation_count: read_u32(&data, off),
                        last_activated_ms: read_u64(&data, off + 4),
                        drift_x: read_f32(&data, off + 12),
                        drift_y: read_f32(&data, off + 16),
                        drift_z: read_f32(&data, off + 20),
                        energy: read_f32(&data, off + 24),
                        _pad: read_u32(&data, off + 28),
                    });
                }
                records.resize(block_count.max(stored_count), ActivationRecord::default());
                return records;
            }
        }
    }
    vec![ActivationRecord::default(); block_count]
}

fn save_activations(output_dir: &Path, records: &[ActivationRecord]) -> Result<(), String> {
    let path = output_dir.join("activations.bin");
    let mut buf = Vec::with_capacity(8 + records.len() * ACTIVATION_RECORD_BYTES);
    buf.extend_from_slice(b"HEB1");
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for rec in records {
        buf.extend_from_slice(&rec.activation_count.to_le_bytes());
        buf.extend_from_slice(&rec.last_activated_ms.to_le_bytes());
        buf.extend_from_slice(&rec.drift_x.to_le_bytes());
        buf.extend_from_slice(&rec.drift_y.to_le_bytes());
        buf.extend_from_slice(&rec.drift_z.to_le_bytes());
        buf.extend_from_slice(&rec.energy.to_le_bytes());
        buf.extend_from_slice(&rec._pad.to_le_bytes());
    }
    fs::write(&path, &buf).map_err(|e| format!("write activations.bin: {}", e))
}

fn load_coactivations(output_dir: &Path) -> HashMap<(u32, u32), CoactivationPair> {
    let path = output_dir.join("coactivations.bin");
    let mut map = HashMap::new();
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 8 && &data[0..4] == b"COA1" {
            let pair_count = read_u32(&data, 4) as usize;
            for i in 0..pair_count {
                let off = 8 + i * COACTIVATION_RECORD_BYTES;
                if off + COACTIVATION_RECORD_BYTES > data.len() {
                    break;
                }
                let pair = CoactivationPair {
                    block_a: read_u32(&data, off),
                    block_b: read_u32(&data, off + 4),
                    count: read_u32(&data, off + 8),
                    last_ts_ms: read_u64(&data, off + 12),
                };
                map.insert((pair.block_a, pair.block_b), pair);
            }
        }
    }
    map
}

fn save_coactivations(
    output_dir: &Path,
    pairs: &HashMap<(u32, u32), CoactivationPair>,
) -> Result<(), String> {
    let path = output_dir.join("coactivations.bin");
    let mut buf = Vec::with_capacity(8 + pairs.len() * COACTIVATION_RECORD_BYTES);
    buf.extend_from_slice(b"COA1");
    buf.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for pair in pairs.values() {
        buf.extend_from_slice(&pair.block_a.to_le_bytes());
        buf.extend_from_slice(&pair.block_b.to_le_bytes());
        buf.extend_from_slice(&pair.count.to_le_bytes());
        buf.extend_from_slice(&pair.last_ts_ms.to_le_bytes());
    }
    fs::write(&path, &buf).map_err(|e| format!("write coactivations.bin: {}", e))
}

fn load_fingerprints(output_dir: &Path) -> Vec<ActivationFingerprint> {
    let path = output_dir.join("fingerprints.bin");
    let mut fingerprints = Vec::new();
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 8 && &data[0..4] == b"FPR1" {
            let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
            let mut pos = 8;
            for _ in 0..count {
                if pos + 18 > data.len() {
                    break;
                }
                let timestamp_ms = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                let query_hash = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().unwrap());
                let activated_count =
                    u16::from_le_bytes(data[pos + 16..pos + 18].try_into().unwrap()) as usize;
                pos += 18;

                let mut activations = Vec::with_capacity(activated_count);
                for _ in 0..activated_count {
                    if pos + 8 > data.len() {
                        break;
                    }
                    let block_idx = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                    let score = f32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
                    activations.push((block_idx, score));
                    pos += 8;
                }

                fingerprints.push(ActivationFingerprint {
                    timestamp_ms,
                    query_hash,
                    activations,
                });
            }
        }
    }
    fingerprints
}

fn save_fingerprints(
    output_dir: &Path,
    fingerprints: &[ActivationFingerprint],
) -> Result<(), String> {
    let path = output_dir.join("fingerprints.bin");
    let mut file =
        fs::File::create(&path).map_err(|e| format!("create fingerprints.bin: {}", e))?;
    file.write_all(b"FPR1")
        .map_err(|e| format!("write magic: {}", e))?;
    file.write_all(&(fingerprints.len() as u32).to_le_bytes())
        .map_err(|e| format!("write count: {}", e))?;
    for fp in fingerprints {
        file.write_all(&fp.timestamp_ms.to_le_bytes())
            .map_err(|e| format!("write ts: {}", e))?;
        file.write_all(&fp.query_hash.to_le_bytes())
            .map_err(|e| format!("write hash: {}", e))?;
        file.write_all(&(fp.activations.len() as u16).to_le_bytes())
            .map_err(|e| format!("write count: {}", e))?;
        for &(block_idx, score) in &fp.activations {
            file.write_all(&block_idx.to_le_bytes())
                .map_err(|e| format!("write idx: {}", e))?;
            file.write_all(&score.to_le_bytes())
                .map_err(|e| format!("write score: {}", e))?;
        }
    }
    Ok(())
}

// ─── Utilities ──────────────────────────────────────

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Public accessor for mirror neuron module.
pub fn now_epoch_ms_pub() -> u64 {
    now_epoch_ms()
}

fn clamp_drift(v: f32) -> f32 {
    v.clamp(-DRIFT_MAX, DRIFT_MAX)
}

/// Hash a query string to u64 (for fingerprint tracking).
pub fn query_hash(query: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in query.as_bytes() {
        h = h.wrapping_mul(0x100000001b3) ^ b as u64;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization_sizes() {
        // Manual serialization sizes (not struct sizes — repr(C) may add padding)
        assert_eq!(ACTIVATION_RECORD_BYTES, 32); // 4+8+4+4+4+4+4
        assert_eq!(COACTIVATION_RECORD_BYTES, 20); // 4+4+4+8
    }

    #[test]
    fn test_record_activation() {
        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 10],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        state.record_activation(&[(0, 0.5), (3, 0.3), (7, 0.1)], 12345);

        assert_eq!(state.activations[0].activation_count, 1);
        assert_eq!(state.activations[3].activation_count, 1);
        assert_eq!(state.activations[7].activation_count, 1);
        assert_eq!(state.activations[1].activation_count, 0);

        // 3 pairs: (0,3), (0,7), (3,7)
        assert_eq!(state.coactivations.len(), 3);
        assert!(state.coactivations.contains_key(&(0, 3)));
        assert!(state.coactivations.contains_key(&(0, 7)));
        assert!(state.coactivations.contains_key(&(3, 7)));

        // Fingerprint stored
        assert_eq!(state.fingerprints.len(), 1);
        assert_eq!(state.fingerprints[0].query_hash, 12345);
        assert_eq!(state.fingerprints[0].activations.len(), 3);
    }

    #[test]
    fn test_repeated_coactivation() {
        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 5],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        state.record_activation(&[(1, 0.5), (2, 0.3)], 100);
        state.record_activation(&[(1, 0.4), (2, 0.6)], 200);
        state.record_activation(&[(1, 0.3), (2, 0.2)], 300);

        assert_eq!(state.activations[1].activation_count, 3);
        assert_eq!(state.coactivations[&(1, 2)].count, 3);
    }

    #[test]
    fn test_drift_application() {
        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 3],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        // Simulate strong co-activation between blocks 0 and 2
        for _ in 0..20 {
            state.record_activation(&[(0, 0.5), (2, 0.5)], 42);
        }

        let headers = vec![(0.0, 0.0, 0.0), (0.5, 0.5, 0.5), (1.0, 1.0, 1.0)];
        state.apply_drift(&headers);

        // Block 0 should drift toward (1,1,1) and block 2 toward (0,0,0)
        assert!(state.activations[0].drift_x > 0.0);
        assert!(state.activations[0].drift_y > 0.0);
        assert!(state.activations[0].drift_z > 0.0);
        assert!(state.activations[2].drift_x < 0.0);
        assert!(state.activations[2].drift_y < 0.0);
        assert!(state.activations[2].drift_z < 0.0);
    }

    #[test]
    fn test_effective_coords() {
        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 2],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        state.activations[0].drift_x = 0.05;
        state.activations[0].drift_y = -0.03;
        state.activations[0].drift_z = 0.01;

        let (x, y, z) = state.effective_coords(0, (0.2, 0.3, 0.4));
        assert!((x - 0.25).abs() < 0.001);
        assert!((y - 0.27).abs() < 0.001);
        assert!((z - 0.41).abs() < 0.001);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let dir = tmp.path();

        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 5],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        state.record_activation(&[(0, 0.5), (2, 0.3), (4, 0.1)], 999);
        state.record_activation(&[(1, 0.8), (3, 0.2)], 888);

        state.save(dir).expect("save");

        let loaded = HebbianState::load_or_init(dir, 5);
        assert_eq!(loaded.activations[0].activation_count, 1);
        assert_eq!(loaded.activations[1].activation_count, 1);
        assert_eq!(loaded.coactivations.len(), 4); // (0,2), (0,4), (2,4), (1,3)
        assert_eq!(loaded.fingerprints.len(), 2);
        assert_eq!(loaded.fingerprints[0].query_hash, 999);
        assert_eq!(loaded.fingerprints[1].query_hash, 888);
    }

    #[test]
    fn test_clamp_drift() {
        assert_eq!(clamp_drift(0.05), 0.05);
        assert_eq!(clamp_drift(0.2), DRIFT_MAX);
        assert_eq!(clamp_drift(-0.2), -DRIFT_MAX);
    }

    #[test]
    fn test_query_hash_deterministic() {
        assert_eq!(query_hash("hello"), query_hash("hello"));
        assert_ne!(query_hash("hello"), query_hash("world"));
    }

    #[test]
    fn test_hottest_blocks() {
        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 5],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        state.record_activation(&[(0, 1.0), (2, 0.5)], 1);

        let hot = state.hottest_blocks(10);
        assert!(!hot.is_empty());
        // Block 0 and 2 should be hot
        assert!(hot.iter().any(|(idx, _)| *idx == 0));
        assert!(hot.iter().any(|(idx, _)| *idx == 2));
    }

    #[test]
    fn test_stats() {
        let mut state = HebbianState {
            activations: vec![ActivationRecord::default(); 10],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        state.record_activation(&[(0, 1.0), (5, 0.5)], 42);

        let stats = state.stats();
        assert_eq!(stats.block_count, 10);
        assert_eq!(stats.active_blocks, 2);
        assert_eq!(stats.total_activations, 2);
        assert_eq!(stats.coactivation_pairs, 1);
        assert_eq!(stats.fingerprint_count, 1);
    }
}
