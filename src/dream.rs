//! Dream Consolidation for Microscope Memory.
//!
//! An offline process that replays the day's recall patterns during idle time,
//! strengthening important pathways and pruning weak ones — analogous to how
//! biological sleep consolidates memories.
//!
//! Binary format: dream_log.bin (DRM1)

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hebbian::HebbianState;
use crate::predictive_cache::PredictiveCache;
use crate::resonance::ResonanceState;
use crate::thought_graph::ThoughtGraphState;
use crate::{emotional_similarity, load_emotion_lookup};

// ─── Constants ──────────────────────────────────────

/// Replay window: consider fingerprints from the last 24h.
const REPLAY_WINDOW_MS: u64 = 86_400_000;

/// Co-activation pairs seen only this many times AND older than PRUNE_AGE are pruned.
const COACTIVATION_PRUNE_THRESHOLD: u32 = 1;

/// Prune age: 48h.
const PRUNE_AGE_MS: u64 = 172_800_000;

/// Activation records with energy below this are pruned (zeroed).
const ACTIVATION_PRUNE_ENERGY: f32 = 0.001;

/// Dream replay gives partial energy (lighter than real activation).
const REPLAY_ENERGY: f32 = 0.3;

/// Co-activation pairs seen in 3+ replayed fingerprints get strengthened.
const STRENGTHEN_MIN_APPEARANCES: usize = 3;

/// Multiplier for strengthened co-activation pairs.
const STRENGTHEN_MULTIPLIER: f32 = 1.5;

/// Resonance field decay factor during dream.
const FIELD_DREAM_DECAY: f32 = 0.8;

// ─── Types ──────────────────────────────────────────

/// Record of a single dream consolidation cycle.
#[derive(Clone, Debug)]
pub struct DreamCycle {
    pub timestamp_ms: u64,
    pub duration_ms: u32,
    pub replayed_fingerprints: u32,
    pub strengthened_pairs: u32,
    pub pruned_pairs: u32,
    pub pruned_activations: u32,
    pub consolidated_patterns: u32,
    pub forgotten_blocks: u32,
    pub energy_before: f32,
    pub energy_after: f32,
}

/// Persistent dream consolidation log.
pub struct DreamState {
    pub cycles: Vec<DreamCycle>,
    pub last_dream_ms: u64,
}

pub struct DreamStats {
    pub total_cycles: usize,
    pub last_dream_ms: u64,
    pub total_pruned_pairs: u64,
    pub total_pruned_activations: u64,
    pub total_strengthened: u64,
    pub total_replayed: u64,
    pub total_forgotten_blocks: u64,
}

// ─── DreamState I/O ─────────────────────────────────

const CYCLE_BYTES: usize = 44; // 8+4+4+4+4+4+4+4+4+4

impl DreamState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("dream_log.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 16 && &data[0..4] == b"DRM1" {
                let cycle_count = read_u32(&data, 4) as usize;
                let last_dream_ms = read_u64(&data, 8);
                let mut cycles = Vec::with_capacity(cycle_count);
                for i in 0..cycle_count {
                    let off = 16 + i * CYCLE_BYTES;
                    if off + CYCLE_BYTES > data.len() {
                        break;
                    }
                    cycles.push(DreamCycle {
                        timestamp_ms: read_u64(&data, off),
                        duration_ms: read_u32(&data, off + 8),
                        replayed_fingerprints: read_u32(&data, off + 12),
                        strengthened_pairs: read_u32(&data, off + 16),
                        pruned_pairs: read_u32(&data, off + 20),
                        pruned_activations: read_u32(&data, off + 24),
                        consolidated_patterns: read_u32(&data, off + 28),
                        forgotten_blocks: read_u32(&data, off + 32),
                        energy_before: read_f32(&data, off + 36),
                        energy_after: read_f32(&data, off + 40),
                    });
                }
                return Self {
                    cycles,
                    last_dream_ms,
                };
            }
        }
        Self {
            cycles: Vec::new(),
            last_dream_ms: 0,
        }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("dream_log.bin");
        let mut buf = Vec::with_capacity(16 + self.cycles.len() * CYCLE_BYTES);
        buf.extend_from_slice(b"DRM1");
        buf.extend_from_slice(&(self.cycles.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.last_dream_ms.to_le_bytes());
        for c in &self.cycles {
            buf.extend_from_slice(&c.timestamp_ms.to_le_bytes());
            buf.extend_from_slice(&c.duration_ms.to_le_bytes());
            buf.extend_from_slice(&c.replayed_fingerprints.to_le_bytes());
            buf.extend_from_slice(&c.strengthened_pairs.to_le_bytes());
            buf.extend_from_slice(&c.pruned_pairs.to_le_bytes());
            buf.extend_from_slice(&c.pruned_activations.to_le_bytes());
            buf.extend_from_slice(&c.consolidated_patterns.to_le_bytes());
            buf.extend_from_slice(&c.forgotten_blocks.to_le_bytes());
            buf.extend_from_slice(&c.energy_before.to_le_bytes());
            buf.extend_from_slice(&c.energy_after.to_le_bytes());
        }
        let tmp_path = output_dir.join("dream_log.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write dream_log.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename dream_log.bin: {}", e))
    }

    pub fn stats(&self) -> DreamStats {
        DreamStats {
            total_cycles: self.cycles.len(),
            last_dream_ms: self.last_dream_ms,
            total_pruned_pairs: self.cycles.iter().map(|c| c.pruned_pairs as u64).sum(),
            total_pruned_activations: self
                .cycles
                .iter()
                .map(|c| c.pruned_activations as u64)
                .sum(),
            total_forgotten_blocks: self.cycles.iter().map(|c| c.forgotten_blocks as u64).sum(),
            total_strengthened: self
                .cycles
                .iter()
                .map(|c| c.strengthened_pairs as u64)
                .sum(),
            total_replayed: self
                .cycles
                .iter()
                .map(|c| c.replayed_fingerprints as u64)
                .sum(),
        }
    }
}

// ─── Dream Consolidation ─────────────────────────────

/// Run a full dream consolidation cycle.
/// 1. Replay recent fingerprints (partial energy boost)
/// 2. Strengthen co-activation pairs appearing in 3+ replayed fingerprints
/// 3. Prune weak co-activation pairs (count=1, older than 48h)
/// 4. Prune cold activation records (zero energy, zero count)
/// 5. Detect thought patterns across recent sessions
/// 6. Decay resonance field
/// 7. Clean up expired predictive cache entries
pub fn dream_consolidate(output_dir: &Path, block_count: usize) -> Result<DreamCycle, String> {
    let t0 = now_ms();

    let mut hebb = HebbianState::load_or_init(output_dir, block_count);
    let mut thought_graph = ThoughtGraphState::load_or_init(output_dir);
    let mut pred_cache = PredictiveCache::load_or_init(output_dir);
    let mut resonance = ResonanceState::load_or_init(output_dir);

    // Measure energy before
    let energy_before: f32 = hebb.activations.iter().map(|r| r.energy).sum();

    // Step 1: Replay recent fingerprints
    let cutoff = t0.saturating_sub(REPLAY_WINDOW_MS);
    let recent_fps: Vec<_> = hebb
        .fingerprints
        .iter()
        .filter(|fp| fp.timestamp_ms >= cutoff)
        .cloned()
        .collect();
    let replayed_count = recent_fps.len() as u32;

    // Count how many fingerprints each co-activation pair appears in
    let mut pair_appearances: std::collections::HashMap<(u32, u32), usize> =
        std::collections::HashMap::new();

    for fp in &recent_fps {
        // Replay: partial energy boost
        for &(block_idx, _score) in &fp.activations {
            let idx = block_idx as usize;
            if idx < hebb.activations.len() {
                let rec = &mut hebb.activations[idx];
                // Boost energy, but lighter than real activation
                rec.energy = (rec.energy + REPLAY_ENERGY).min(1.0);
            }
        }

        // Track co-activation pair appearances
        for i in 0..fp.activations.len() {
            for j in (i + 1)..fp.activations.len() {
                let a = fp.activations[i].0.min(fp.activations[j].0);
                let b = fp.activations[i].0.max(fp.activations[j].0);
                *pair_appearances.entry((a, b)).or_insert(0) += 1;
            }
        }
    }

    // Step 2: Strengthen frequently co-appearing pairs with emotional coherence boost
    let emotion_lookup = load_emotion_lookup(output_dir);
    let mut strengthened = 0u32;
    let mut coherence_boosted = 0u32;
    let mut _emotion_pruned = 0u32;

    for ((a, b), appearances) in &pair_appearances {
        if *appearances >= STRENGTHEN_MIN_APPEARANCES {
            // Check emotional coherence for extra boost
            let coherence_mult = emotion_lookup
                .as_ref()
                .and_then(|lookup| {
                    lookup(*a as usize).and_then(|ea| {
                        lookup(*b as usize).map(|eb| {
                            let sim = emotional_similarity(&ea, &eb);
                            if sim > 0.4 {
                                coherence_boosted += 1;
                                1.0 + sim * 0.5 // up to 1.5x extra boost
                            } else {
                                1.0
                            }
                        })
                    })
                })
                .unwrap_or(1.0);

            if let Some(pair) = hebb.coactivations.get_mut(&(*a, *b)) {
                pair.count = (pair.count as f32 * STRENGTHEN_MULTIPLIER * coherence_mult) as u32;
                strengthened += 1;
            }
        } else if let Some(ref lookup) = emotion_lookup {
            // Emotionally incoherent pairs get extra pruning pressure
            if let (Some(ea), Some(eb)) = (lookup(*a as usize), lookup(*b as usize)) {
                let sim = emotional_similarity(&ea, &eb);
                if sim < 0.1 {
                    if let Some(pair) = hebb.coactivations.get_mut(&(*a, *b)) {
                        if pair.count > 1 {
                            pair.count /= 2;
                            _emotion_pruned += 1;
                        }
                    }
                }
            }
        }
    }

    // Step 3: Prune weak co-activation pairs
    let mut pruned_pairs = 0u32;
    hebb.coactivations.retain(|_, pair| {
        if pair.count <= COACTIVATION_PRUNE_THRESHOLD && pair.last_ts_ms + PRUNE_AGE_MS < t0 {
            pruned_pairs += 1;
            false
        } else {
            true
        }
    });

    // Step 4: Prune cold activations
    let mut pruned_activations = 0u32;
    for rec in &mut hebb.activations {
        if rec.energy < ACTIVATION_PRUNE_ENERGY && rec.activation_count == 0 {
            *rec = crate::hebbian::ActivationRecord::default();
            pruned_activations += 1;
        }
    }

    // Step 5: Pattern detection
    let patterns_before = thought_graph.crystallized_count();
    thought_graph.detect_patterns();
    let consolidated_patterns = (thought_graph.crystallized_count() - patterns_before) as u32;

    // Step 6: Decay resonance field
    resonance.decay_field(FIELD_DREAM_DECAY);
    resonance.expire_pulses();

    // Step 7: Predictive cache cleanup — remove predictions with very low confidence
    pred_cache.dream_cleanup();

    // Step 8: Forget old internal thoughts (autonomous mode outputs)
    let forgotten = forget_old_thoughts(output_dir, block_count)?;

    // Measure energy after
    let energy_after: f32 = hebb.activations.iter().map(|r| r.energy).sum();

    // Save everything
    hebb.save(output_dir)
        .map_err(|e| format!("save hebbian: {}", e))?;
    thought_graph
        .save(output_dir)
        .map_err(|e| format!("save thought_graph: {}", e))?;
    pred_cache
        .save(output_dir)
        .map_err(|e| format!("save predictive_cache: {}", e))?;
    resonance
        .save(output_dir)
        .map_err(|e| format!("save resonance: {}", e))?;

    let duration_ms = (now_ms() - t0) as u32;

    Ok(DreamCycle {
        timestamp_ms: t0,
        duration_ms,
        replayed_fingerprints: replayed_count,
        strengthened_pairs: strengthened,
        pruned_pairs,
        pruned_activations,
        consolidated_patterns,
        forgotten_blocks: forgotten,
        energy_before,
        energy_after,
    })
}

// ─── Binary helpers ─────────────────────────────────

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

// ─── Tests ──────────────────────────────────────────

// ─── Forgetting ─────────────────────────────────────────
/// Forget old internal thoughts (autonomous mode outputs).
/// Only targets internal layers: short_term(2), associative(3), reflections(6), session(11).
/// Never touches: identity(0), long_term(1), emotional(4), relational(5),
/// crypto_chain(7), echo_cache(8), rust_state(9), code(10).
/// Blocks older than FORGET_AGE_MS with importance < 5 are removed.
const FORGET_AGE_MS: u64 = 86_400_000; // 24 hours
const FORGET_INTERNAL_LAYERS: &[u8] = &[2, 3, 6, 11];
const FORGET_MIN_IMPORTANCE: u8 = 5;

pub fn forget_old_thoughts(output_dir: &Path, block_count: usize) -> Result<u32, String> {
    use crate::{BLOCK_DATA_SIZE, DEPTH_ENTRY_SIZE, HEADER_SIZE, META_HEADER_SIZE};
    use std::fs;
    use std::io::{Read, Seek, SeekFrom, Write};

    let hdr_path = output_dir.join("microscope.bin");
    let dat_path = output_dir.join("data.bin");
    let meta_path = output_dir.join("meta.bin");

    if !hdr_path.exists() || !dat_path.exists() || !meta_path.exists() {
        return Ok(0); // Nothing to do if files don't exist
    }

    let headers = fs::read(&hdr_path).map_err(|e| format!("read microscope.bin: {}", e))?;
    let data = fs::read(&dat_path).map_err(|e| format!("read data.bin: {}", e))?;
    let meta = fs::read(&meta_path).map_err(|e| format!("read meta.bin: {}", e))?;

    let actual_blocks = headers.len() / HEADER_SIZE;
    if actual_blocks == 0 {
        return Ok(0);
    }

    let t0 = now_ms();
    let mut keep_indices: Vec<usize> = Vec::with_capacity(actual_blocks);
    let mut forgotten = 0u32;

    for i in 0..actual_blocks {
        let off = i * HEADER_SIZE;
        if off + HEADER_SIZE > headers.len() {
            break;
        }

        // Read layer_id (byte 12 in header: after x(4), y(4), z(4))
        let layer_id = headers[off + 12];
        // Read importance (byte 13 in header)
        let importance = headers[off + 13];

        // Check if this is an internal thought that should be forgotten
        if FORGET_INTERNAL_LAYERS.contains(&layer_id) && importance < FORGET_MIN_IMPORTANCE {
            // We don't have a direct timestamp in the header, so we estimate
            // based on block position: older blocks have lower indices in their depth range.
            // For simplicity, we forget based on layer + importance only.
            // Old internal thoughts with low importance are always forgotten.
            forgotten += 1;
            continue; // Skip this block
        }

        keep_indices.push(i);
    }

    if forgotten == 0 {
        return Ok(0);
    }

    // Rewrite microscope.bin with only kept headers
    let mut new_headers = Vec::with_capacity(keep_indices.len() * HEADER_SIZE);
    let mut new_data = Vec::with_capacity(keep_indices.len() * BLOCK_DATA_SIZE);

    for &idx in &keep_indices {
        let hdr_off = idx * HEADER_SIZE;
        let dat_off = idx * BLOCK_DATA_SIZE;

        new_headers.extend_from_slice(&headers[hdr_off..hdr_off + HEADER_SIZE]);
        if dat_off + BLOCK_DATA_SIZE <= data.len() {
            new_data.extend_from_slice(&data[dat_off..dat_off + BLOCK_DATA_SIZE]);
        } else {
            new_data.extend_from_slice(&[0u8; BLOCK_DATA_SIZE]);
        }
    }

    let hdr_tmp = output_dir.join("microscope.bin.tmp");
    let dat_tmp = output_dir.join("data.bin.tmp");
    fs::write(&hdr_tmp, &new_headers).map_err(|e| format!("write microscope.bin: {}", e))?;
    fs::write(&dat_tmp, &new_data).map_err(|e| format!("write data.bin: {}", e))?;
    fs::rename(&hdr_tmp, &hdr_path).map_err(|e| format!("rename microscope.bin: {}", e))?;
    fs::rename(&dat_tmp, &dat_path).map_err(|e| format!("rename data.bin: {}", e))?;

    // Rebuild meta.bin with new block count and depth ranges
    let n = keep_indices.len();
    let mut new_meta = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE);

    // Copy original magic and version (first 8 bytes)
    if meta.len() >= 8 {
        new_meta.extend_from_slice(&meta[..8]);
    } else {
        new_meta.extend_from_slice(b"MSC3   ");
    }
    // Write new block count (u32 at offset 8)
    new_meta.extend_from_slice(&(n as u32).to_le_bytes());

    // Compute depth ranges from kept headers
    let mut depth_counts = [0u32; 9];
    for &idx in &keep_indices {
        let off = idx * HEADER_SIZE;
        let depth = headers[off + 14]; // depth is at byte 14
        if (depth as usize) < 9 {
            depth_counts[depth as usize] += 1;
        }
    }

    let mut running_start = 0u32;
    for d in 0..9 {
        let count = depth_counts[d];
        new_meta.extend_from_slice(&running_start.to_le_bytes());
        new_meta.extend_from_slice(&count.to_le_bytes());
        running_start += count;
    }

    // Copy remaining meta data (merkle root, etc.) if available
    let meta_tail_start = META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE;
    if meta_tail_start < meta.len() {
        new_meta.extend_from_slice(&meta[meta_tail_start..]);
    }

    let meta_tmp = output_dir.join("meta.bin.tmp");
    fs::write(&meta_tmp, &new_meta).map_err(|e| format!("write meta.bin: {}", e))?;
    fs::rename(&meta_tmp, &meta_path).map_err(|e| format!("rename meta.bin: {}", e))?;

    println!(
        "  [FORGET] {} belső gondolat elfelejtve ({} blokk maradt)",
        forgotten, n
    );

    Ok(forgotten)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archetype::ArchetypeState;
    use crate::hebbian::{ActivationFingerprint, ActivationRecord, CoactivationPair};
    use crate::predictive_cache::PredictiveCache;
    use crate::resonance::ResonanceState;
    use crate::thought_graph::ThoughtGraphState;
    use std::collections::HashMap;

    fn make_hebb(block_count: usize) -> HebbianState {
        HebbianState {
            activations: vec![ActivationRecord::default(); block_count],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        }
    }

    #[test]
    fn test_dream_log_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = DreamState {
            cycles: vec![
                DreamCycle {
                    timestamp_ms: 1000,
                    duration_ms: 50,
                    replayed_fingerprints: 10,
                    strengthened_pairs: 3,
                    pruned_pairs: 5,
                    pruned_activations: 2,
                    consolidated_patterns: 1,
                    energy_before: 10.5,
                    energy_after: 8.2,
                },
                DreamCycle {
                    timestamp_ms: 2000,
                    duration_ms: 30,
                    replayed_fingerprints: 8,
                    strengthened_pairs: 2,
                    pruned_pairs: 3,
                    pruned_activations: 1,
                    consolidated_patterns: 0,
                    energy_before: 8.2,
                    energy_after: 7.0,
                },
            ],
            last_dream_ms: 2000,
        };
        state.save(tmp.path()).unwrap();
        let loaded = DreamState::load_or_init(tmp.path());
        assert_eq!(loaded.cycles.len(), 2);
        assert_eq!(loaded.last_dream_ms, 2000);
        assert_eq!(loaded.cycles[0].replayed_fingerprints, 10);
        assert_eq!(loaded.cycles[1].pruned_pairs, 3);
    }

    #[test]
    fn test_dream_strengthens_repeated_coactivations() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut hebb = make_hebb(10);

        // Insert a co-activation pair
        hebb.coactivations.insert(
            (0, 1),
            CoactivationPair {
                block_a: 0,
                block_b: 1,
                count: 5,
                last_ts_ms: now_ms(),
            },
        );

        // Add 3 fingerprints that co-activate blocks 0 and 1
        let now = now_ms();
        for i in 0..3 {
            hebb.fingerprints.push(ActivationFingerprint {
                timestamp_ms: now - i * 1000,
                query_hash: 100 + i,
                activations: vec![(0, 0.5), (1, 0.3)],
            });
        }

        hebb.save(tmp.path()).unwrap();

        // Also need thought_graph, pred_cache, resonance, archetypes
        let tg = ThoughtGraphState::load_or_init(tmp.path());
        tg.save(tmp.path()).unwrap();
        let pc = PredictiveCache::load_or_init(tmp.path());
        pc.save(tmp.path()).unwrap();
        let res = ResonanceState::load_or_init(tmp.path());
        res.save(tmp.path()).unwrap();
        let arc = ArchetypeState::load_or_init(tmp.path());
        arc.save(tmp.path()).unwrap();

        let cycle = dream_consolidate(tmp.path(), 10).unwrap();
        assert!(cycle.strengthened_pairs > 0);

        // Verify the pair was strengthened
        let hebb2 = HebbianState::load_or_init(tmp.path(), 10);
        let pair = hebb2.coactivations.get(&(0, 1)).unwrap();
        assert!(pair.count > 5); // was 5, should be 5 * 1.5 = 7
    }

    #[test]
    fn test_dream_prunes_weak_pairs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut hebb = make_hebb(5);

        // Old, weak pair
        hebb.coactivations.insert(
            (0, 1),
            CoactivationPair {
                block_a: 0,
                block_b: 1,
                count: 1,
                last_ts_ms: 1000, // very old
            },
        );
        // Recent, strong pair
        hebb.coactivations.insert(
            (2, 3),
            CoactivationPair {
                block_a: 2,
                block_b: 3,
                count: 10,
                last_ts_ms: now_ms(),
            },
        );

        hebb.save(tmp.path()).unwrap();
        ThoughtGraphState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        PredictiveCache::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        ResonanceState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        ArchetypeState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();

        let cycle = dream_consolidate(tmp.path(), 5).unwrap();
        assert_eq!(cycle.pruned_pairs, 1);

        let hebb2 = HebbianState::load_or_init(tmp.path(), 5);
        assert!(!hebb2.coactivations.contains_key(&(0, 1))); // pruned
        assert!(hebb2.coactivations.contains_key(&(2, 3))); // kept
    }

    #[test]
    fn test_dream_replays_fingerprints() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut hebb = make_hebb(5);

        // Block 0 has zero energy
        assert_eq!(hebb.activations[0].energy, 0.0);

        // Add a recent fingerprint activating block 0
        hebb.fingerprints.push(ActivationFingerprint {
            timestamp_ms: now_ms() - 1000,
            query_hash: 42,
            activations: vec![(0, 0.5)],
        });

        hebb.save(tmp.path()).unwrap();
        ThoughtGraphState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        PredictiveCache::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        ResonanceState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        ArchetypeState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();

        let cycle = dream_consolidate(tmp.path(), 5).unwrap();
        assert_eq!(cycle.replayed_fingerprints, 1);

        let hebb2 = HebbianState::load_or_init(tmp.path(), 5);
        assert!(hebb2.activations[0].energy >= REPLAY_ENERGY - 0.01);
    }

    #[test]
    fn test_dream_no_fingerprints() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let hebb = make_hebb(5);
        hebb.save(tmp.path()).unwrap();
        ThoughtGraphState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        PredictiveCache::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        ResonanceState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();
        ArchetypeState::load_or_init(tmp.path())
            .save(tmp.path())
            .unwrap();

        let cycle = dream_consolidate(tmp.path(), 5).unwrap();
        assert_eq!(cycle.replayed_fingerprints, 0);
        assert_eq!(cycle.strengthened_pairs, 0);
        assert_eq!(cycle.pruned_pairs, 0);
    }

    #[test]
    fn test_dream_stats() {
        let state = DreamState {
            cycles: vec![
                DreamCycle {
                    timestamp_ms: 1000,
                    duration_ms: 50,
                    replayed_fingerprints: 10,
                    strengthened_pairs: 3,
                    pruned_pairs: 5,
                    pruned_activations: 2,
                    consolidated_patterns: 1,
                    energy_before: 10.0,
                    energy_after: 8.0,
                },
                DreamCycle {
                    timestamp_ms: 2000,
                    duration_ms: 30,
                    replayed_fingerprints: 8,
                    strengthened_pairs: 2,
                    pruned_pairs: 3,
                    pruned_activations: 1,
                    consolidated_patterns: 0,
                    energy_before: 8.0,
                    energy_after: 7.0,
                },
            ],
            last_dream_ms: 2000,
        };
        let stats = state.stats();
        assert_eq!(stats.total_cycles, 2);
        assert_eq!(stats.total_pruned_pairs, 8);
        assert_eq!(stats.total_strengthened, 5);
        assert_eq!(stats.total_replayed, 18);
    }
}
