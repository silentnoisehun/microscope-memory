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
pub fn dream_consolidate(
    output_dir: &Path,
    block_count: usize,
    max_blocks: usize,
    protect_min_importance: u8,
) -> Result<DreamCycle, String> {
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
    let mut forgotten = forget_old_thoughts(output_dir, block_count)?;

    // Step 8b: Size-bounded eviction — drop lowest-scoring blocks when the
    // index exceeds max_blocks. Blocks with importance >= protect_min_importance
    // are never evicted.
    let evicted = evict_over_capacity(output_dir, max_blocks, protect_min_importance)?;
    if evicted > 0 {
        println!(
            "  [EVICT] {} blokk eltávolítva (plafon: {}, maradt: {})",
            evicted,
            max_blocks,
            block_count.saturating_sub(evicted as usize)
        );
    }
    forgotten += evicted;

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
#[allow(dead_code)]
const FORGET_AGE_MS: u64 = 86_400_000; // 24 hours
const FORGET_INTERNAL_LAYERS: &[u8] = &[2, 3, 6, 11];
const FORGET_MIN_IMPORTANCE: u8 = 5;

pub fn forget_old_thoughts(output_dir: &Path, _block_count: usize) -> Result<u32, String> {
    use crate::{BLOCK_DATA_SIZE, DEPTH_ENTRY_SIZE, HEADER_SIZE, META_HEADER_SIZE};
    use std::fs;

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

    let _t0 = now_ms();
    let mut keep_indices: Vec<usize> = Vec::with_capacity(actual_blocks);
    let mut forgotten = 0u32;

    for i in 0..actual_blocks {
        let off = i * HEADER_SIZE;
        if off + HEADER_SIZE > headers.len() {
            break;
        }

        // Read layer_id (byte 17 in MSC4 header: depth(16) then layer_id(17))
        let layer_id = headers[off + 17];
        // Read importance (byte 48 in MSC4 header: after project_id)
        let importance = headers[off + 48];

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
        new_meta.extend_from_slice(b"MSC4   ");
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
    for &count in &depth_counts[..9] {
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

/// Evict the lowest-scoring blocks when the index exceeds `max_blocks`.
///
/// Approved policy: eviction score = importance × 10 + recall energy − age
/// penalty, where age is the block position normalized to [0, 1]. Blocks with
/// importance >= `protect_min_importance` are never evicted; if the cap cannot
/// be reached without touching protected memory, nothing is evicted. The
/// embeddings index is rewritten together with the block index so semantic
/// rows stay aligned.
pub fn evict_over_capacity(
    output_dir: &Path,
    max_blocks: usize,
    protect_min_importance: u8,
) -> Result<u32, String> {
    use crate::{BLOCK_DATA_SIZE, DEPTH_ENTRY_SIZE, HEADER_SIZE, META_HEADER_SIZE};

    let hdr_path = output_dir.join("microscope.bin");
    let dat_path = output_dir.join("data.bin");
    let meta_path = output_dir.join("meta.bin");
    if !hdr_path.exists() || !dat_path.exists() || !meta_path.exists() {
        return Ok(0);
    }
    let headers = fs::read(&hdr_path).map_err(|e| format!("read microscope.bin: {e}"))?;
    let data = fs::read(&dat_path).map_err(|e| format!("read data.bin: {e}"))?;
    let meta = fs::read(&meta_path).map_err(|e| format!("read meta.bin: {e}"))?;

    let actual_blocks = headers.len() / HEADER_SIZE;
    if actual_blocks == 0 || max_blocks == 0 || actual_blocks <= max_blocks {
        return Ok(0);
    }
    let target = actual_blocks - max_blocks;

    // Recall signal: per-block energy from the Hebbian state.
    let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, actual_blocks);

    let mut scored: Vec<(usize, f32)> = Vec::with_capacity(actual_blocks);
    for i in 0..actual_blocks {
        let off = i * HEADER_SIZE;
        let importance = headers[off + 48]; // byte 48: importance (MSC4 layout)
        if importance >= protect_min_importance {
            continue; // protected core memory is never evicted
        }
        let energy = hebb.activations.get(i).map(|r| r.energy).unwrap_or(0.0);
        let age = i as f32 / actual_blocks as f32;
        scored.push((i, importance as f32 * 10.0 + energy - age * 5.0));
    }
    if scored.len() <= target {
        // Reaching the cap would require evicting protected memory.
        return Ok(0);
    }

    scored.sort_by(|a, b| a.1.total_cmp(&b.1)); // lowest score first
    let victims: std::collections::HashSet<usize> =
        scored.iter().take(target).map(|(idx, _)| *idx).collect();
    let keep_indices: Vec<usize> = (0..actual_blocks)
        .filter(|i| !victims.contains(i))
        .collect();

    // Rewrite the binary index, keeping only the surviving blocks.
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
    fs::write(&hdr_tmp, &new_headers).map_err(|e| format!("write microscope.bin: {e}"))?;
    fs::write(&dat_tmp, &new_data).map_err(|e| format!("write data.bin: {e}"))?;
    fs::rename(&hdr_tmp, &hdr_path).map_err(|e| format!("rename microscope.bin: {e}"))?;
    fs::rename(&dat_tmp, &dat_path).map_err(|e| format!("rename data.bin: {e}"))?;

    // Rebuild meta.bin with the new block count and depth ranges.
    let n = keep_indices.len();
    let mut new_meta = Vec::with_capacity(META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE);
    if meta.len() >= 8 {
        new_meta.extend_from_slice(&meta[..8]);
    } else {
        new_meta.extend_from_slice(b"MSC4\x04\0\0\0");
    }
    new_meta.extend_from_slice(&(n as u32).to_le_bytes());
    let mut depth_counts = [0u32; 9];
    for &idx in &keep_indices {
        let off = idx * HEADER_SIZE;
        let depth = headers[off + 14]; // byte 14: depth
        if (depth as usize) < 9 {
            depth_counts[depth as usize] += 1;
        }
    }
    let mut running_start = 0u32;
    for &count in &depth_counts[..9] {
        new_meta.extend_from_slice(&running_start.to_le_bytes());
        new_meta.extend_from_slice(&count.to_le_bytes());
        running_start += count;
    }
    let meta_tail_start = META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE;
    if meta_tail_start < meta.len() {
        new_meta.extend_from_slice(&meta[meta_tail_start..]);
    }
    let meta_tmp = output_dir.join("meta.bin.tmp");
    fs::write(&meta_tmp, &new_meta).map_err(|e| format!("write meta.bin: {e}"))?;
    fs::rename(&meta_tmp, &meta_path).map_err(|e| format!("rename meta.bin: {e}"))?;

    trim_embeddings(output_dir, &keep_indices)?;

    Ok(target as u32)
}

/// Drop the evicted rows from embeddings.bin so semantic rows stay aligned
/// with the compacted block index. A mismatched or missing file is left alone
/// (the next embedding rebuild regenerates it).
fn trim_embeddings(output_dir: &Path, keep_indices: &[usize]) -> Result<(), String> {
    let emb_path = output_dir.join("embeddings.bin");
    if !emb_path.exists() {
        return Ok(());
    }
    let emb = fs::read(&emb_path).map_err(|e| format!("read embeddings.bin: {e}"))?;
    if emb.len() < 12 {
        return Ok(());
    }
    let block_count = u32::from_le_bytes(emb[0..4].try_into().unwrap()) as usize;
    let dim = u32::from_le_bytes(emb[4..8].try_into().unwrap()) as usize;
    let expected = 12 + block_count * dim * 4;
    if dim == 0 || block_count == 0 || emb.len() < expected {
        return Ok(());
    }
    let mut out = Vec::with_capacity(12 + keep_indices.len() * dim * 4);
    out.extend_from_slice(&(keep_indices.len() as u32).to_le_bytes());
    out.extend_from_slice(&(dim as u32).to_le_bytes());
    out.extend_from_slice(&emb[8..12]); // max_depth
    for &idx in keep_indices {
        let off = 12 + idx * dim * 4;
        out.extend_from_slice(&emb[off..off + dim * 4]);
    }
    let tmp = output_dir.join("embeddings.bin.tmp");
    fs::write(&tmp, &out).map_err(|e| format!("write embeddings.bin: {e}"))?;
    fs::rename(&tmp, &emb_path).map_err(|e| format!("rename embeddings.bin: {e}"))?;
    Ok(())
}

/// Automatic importance promotion for frequently recalled blocks.
///
/// Blocks whose Hebbian recall energy is at least `promote_energy` and which
/// have been activated at least once get their importance bumped by one,
/// capped at `protect_min_importance` (the eviction protection floor). The
/// bump is written into the binary headers and mirrored back into the layer
/// source entries (via the `(imp=N)` marker) so it survives rebuilds.
pub fn promote_recalled_blocks(
    output_dir: &Path,
    layers_dir: &Path,
    block_count: usize,
    promote_energy: f32,
    protect_min_importance: u8,
) -> Result<u32, String> {
    use crate::{BLOCK_DATA_SIZE, HEADER_SIZE};

    let hdr_path = output_dir.join("microscope.bin");
    if !hdr_path.exists() {
        return Ok(0);
    }
    let headers = fs::read(&hdr_path).map_err(|e| format!("read microscope.bin: {e}"))?;
    let actual = headers.len() / HEADER_SIZE;
    if actual == 0 || actual != block_count {
        return Ok(0);
    }
    let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, actual);

    let mut bumps: Vec<(usize, u8)> = Vec::new(); // (block index, new importance)

    // Load the evidence ledger for the epistemic gate (C4).
    let epistemic_gate = output_dir.exists();
    let ledger = if epistemic_gate {
        crate::epistemic::EvidenceLedger::load_or_init(output_dir)
    } else {
        crate::epistemic::EvidenceLedger { records: std::collections::HashMap::new() }
    };

    for i in 0..actual {
        let imp = headers[i * HEADER_SIZE + 48]; // byte 48: importance (MSC4)
        if imp >= protect_min_importance {
            continue; // already at or above the protection floor
        }
        let rec = match hebb.activations.get(i) {
            Some(r) => r,
            None => continue,
        };
        if rec.energy >= promote_energy && rec.activation_count > 0 {
            // Epistemic gate (C4): Inference/Hypothesis without evidence cannot promote.
            let flags = headers[i * HEADER_SIZE + 49]; // byte 48+1: flags
            let class = crate::epistemic::class_of_flags(flags);
            let block_hash = {
                // Read the block text from data.bin to compute its content hash.
                let data_path = output_dir.join("data.bin");
                if let Ok(data) = fs::read(&data_path) {
                    let start = i * crate::BLOCK_DATA_SIZE;
                    if start + crate::BLOCK_DATA_SIZE <= data.len() {
                        let block = &data[start..start + crate::BLOCK_DATA_SIZE];
                        let end = block.iter().position(|&b| b == 0).unwrap_or(block.len());
                        crate::epistemic::content_hash(String::from_utf8_lossy(&block[..end]).trim())
                    } else { 0 }
                } else { 0 }
            };
            if let Err(_reason) = crate::epistemic::check_promotion_gate(&ledger, class, block_hash) {
                // Gate blocked: log but do not promote.
                continue;
            }
            bumps.push((i, imp.saturating_add(1).min(protect_min_importance)));
        }
    }
    if bumps.is_empty() {
        return Ok(0);
    }

    // Rewrite headers with the bumped importance values (atomic tmp+rename).
    let mut new_headers = headers.clone();
    for &(idx, new_imp) in &bumps {
        new_headers[idx * HEADER_SIZE + 48] = new_imp; // byte 48: importance (MSC4)
    }
    let hdr_tmp = output_dir.join("microscope.bin.tmp");
    fs::write(&hdr_tmp, &new_headers).map_err(|e| format!("write microscope.bin: {e}"))?;
    fs::rename(&hdr_tmp, &hdr_path).map_err(|e| format!("rename microscope.bin: {e}"))?;

    // Mirror the bumps into the layer source entries so rebuilds keep them.
    let dat_path = output_dir.join("data.bin");
    let data = fs::read(&dat_path).map_err(|e| format!("read data.bin: {e}"))?;
    let mut updated_files: std::collections::HashMap<std::path::PathBuf, String> =
        std::collections::HashMap::new();
    for &(idx, new_imp) in &bumps {
        let start = idx * BLOCK_DATA_SIZE;
        if start >= data.len() {
            continue;
        }
        let block = &data[start..(start + BLOCK_DATA_SIZE).min(data.len())];
        let end = block.iter().position(|&b| b == 0).unwrap_or(block.len());
        let block_text = String::from_utf8_lossy(&block[..end]).trim().to_string();
        if block_text.len() < 8 {
            continue;
        }
        if let Ok(entries) = fs::read_dir(layers_dir) {
            for file in entries.filter_map(|e| e.ok()) {
                let path = file.path();
                if path.extension().and_then(|e| e.to_str()) != Some("txt") {
                    continue;
                }
                let content = updated_files
                    .entry(path.clone())
                    .or_insert_with(|| fs::read_to_string(&path).unwrap_or_default());
                if let Some(next) = bump_entry_by_text_hash(content, &block_text, new_imp) {
                    *content = next;
                    break;
                }
            }
        }
    }
    for (path, content) in &updated_files {
        let tmp = path.with_extension("txt.tmp");
        fs::write(&tmp, content).map_err(|e| format!("write layer file: {e}"))?;
        fs::rename(&tmp, path).map_err(|e| format!("rename layer file: {e}"))?;
    }

    Ok(bumps.len() as u32)
}

/// Replace the `(imp=N)` marker of the layer entry whose text hash exactly
/// matches the promoted block's text.
///
/// Matching is exact hash equality (FNV-1a over the marker-stripped, trimmed
/// text), never substring matching, so overlapping or duplicated entries cannot
/// be modified by mistake. At most one entry changes: the first exact match.
fn bump_entry_by_text_hash(content: &str, block_text: &str, new_imp: u8) -> Option<String> {
    let needle_hash = crate::fingerprint::fnv1a_hash(block_text.trim().as_bytes());
    let mut entries: Vec<String> = content.split("\n\n").map(|s| s.to_string()).collect();
    for entry in &mut entries {
        let (stripped, _imp) = crate::reader::parse_imp_marker(entry);
        if crate::fingerprint::fnv1a_hash(stripped.trim().as_bytes()) == needle_hash
            && entry.starts_with("(imp=")
        {
            if let Some(end) = entry.find(')') {
                *entry = format!("(imp={}){}", new_imp, &entry[end + 1..]);
                return Some(entries.join("\n\n"));
            }
        }
    }
    None
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
                    forgotten_blocks: 0,
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
                    forgotten_blocks: 0,
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
    fn eviction_respects_importance_protection() {
        use crate::{BLOCK_DATA_SIZE, HEADER_SIZE};
        use std::fs;

        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let n = 10usize;
        let mut headers = Vec::new();
        let mut data = Vec::new();
        for i in 0..n {
            let mut h = vec![0u8; HEADER_SIZE];
            h[17] = 1; // layer_id (byte 17 in MSC4)
            h[48] = if i < 2 { 8 } else { 5 }; // importance (byte 48 in MSC4)
            h[16] = (i % 9) as u8; // depth (byte 16 in MSC4)
            headers.extend_from_slice(&h);
            data.extend_from_slice(&[b'a'; BLOCK_DATA_SIZE]);
        }
        let mut meta = Vec::new();
        meta.extend_from_slice(b"MSC4\x04\0\0\0");
        meta.extend_from_slice(&(n as u32).to_le_bytes());
        for _ in 0..9 {
            meta.extend_from_slice(&0u32.to_le_bytes());
            meta.extend_from_slice(&0u32.to_le_bytes());
        }
        fs::write(dir.join("microscope.bin"), &headers).unwrap();
        fs::write(dir.join("data.bin"), &data).unwrap();
        fs::write(dir.join("meta.bin"), &meta).unwrap();

        let evicted = evict_over_capacity(dir, 4, 8).unwrap();
        assert_eq!(evicted, 6);

        let new_headers = fs::read(dir.join("microscope.bin")).unwrap();
        assert_eq!(new_headers.len() / HEADER_SIZE, 4);
        let mut protected_survivors = 0;
        let mut min_imp = u8::MAX;
        for i in 0..4 {
            let imp = new_headers[i * HEADER_SIZE + 48]; // byte 48: importance (MSC4)
            if imp >= 8 {
                protected_survivors += 1;
            }
            min_imp = min_imp.min(imp);
        }
        assert_eq!(protected_survivors, 2, "both protected blocks survive");
        assert!(min_imp >= 5, "no protected block is evicted");
    }

    #[test]
    fn eviction_is_noop_below_cap_or_without_candidates() {
        use crate::{BLOCK_DATA_SIZE, HEADER_SIZE};
        use std::fs;

        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let n = 6usize;
        let mut headers = Vec::new();
        let mut data = Vec::new();
        for _ in 0..n {
            let mut h = vec![0u8; HEADER_SIZE];
            h[48] = 8; // everything protected (byte 48 in MSC4)
            headers.extend_from_slice(&h);
            data.extend_from_slice(&[b'a'; BLOCK_DATA_SIZE]);
        }
        fs::write(dir.join("microscope.bin"), &headers).unwrap();
        fs::write(dir.join("data.bin"), &data).unwrap();
        fs::write(dir.join("meta.bin"), b"MSC4\x04\0\0\0").unwrap();

        // Below the cap: no-op.
        assert_eq!(evict_over_capacity(dir, 100, 8).unwrap(), 0);
        // Cap cannot be reached without evicting protected memory: no-op.
        assert_eq!(evict_over_capacity(dir, 2, 8).unwrap(), 0);
        let after = fs::read(dir.join("microscope.bin")).unwrap();
        assert_eq!(after.len() / HEADER_SIZE, n);
    }

    #[test]
    fn promotion_bumps_recalled_blocks_and_mirrors_layer_source() {
        use crate::hebbian::{ActivationRecord, HebbianState};
        use crate::{BLOCK_DATA_SIZE, HEADER_SIZE};
        use std::fs;

        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let layer_dir = dir.join("layers");
        fs::create_dir_all(&layer_dir).unwrap();
        let n = 4usize;

        let mut headers = Vec::new();
        let mut data = Vec::new();
        for i in 0..n {
            let mut h = vec![0u8; HEADER_SIZE];
            h[17] = 0; // layer_id (byte 17 in MSC4)
            h[48] = 5; // importance (byte 48 in MSC4)
            headers.extend_from_slice(&h);
            let text = format!("emlék szöveg blokk {}", i);
            let mut block = text.as_bytes().to_vec();
            block.resize(BLOCK_DATA_SIZE, 0);
            data.extend_from_slice(&block);
        }
        fs::write(dir.join("microscope.bin"), &headers).unwrap();
        fs::write(dir.join("data.bin"), &data).unwrap();
        fs::write(dir.join("meta.bin"), b"MSC4\x04\0\0\0").unwrap();
        fs::write(
            layer_dir.join("long_term.txt"),
            "(imp=5) emlék szöveg blokk 0\n\n(imp=5) emlék szöveg blokk 1\n\n\
             (imp=5) emlék szöveg blokk 2\n\n(imp=5) emlék szöveg blokk 3",
        )
        .unwrap();

        let mut hebb = HebbianState {
            activations: vec![ActivationRecord::default(); n],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };
        hebb.activations[1].energy = 0.8;
        hebb.activations[1].activation_count = 3;
        hebb.activations[2].energy = 0.9; // high energy but never activated
        hebb.save(dir).unwrap();

        let promoted = promote_recalled_blocks(dir, &layer_dir, n, 0.35, 8).unwrap();
        assert_eq!(promoted, 1);

        let new_headers = fs::read(dir.join("microscope.bin")).unwrap();
        assert_eq!(new_headers[HEADER_SIZE + 48], 6);
        assert_eq!(
            new_headers[2 * HEADER_SIZE + 48],
            5,
            "no activation -> no promotion"
        );
        assert_eq!(new_headers[48], 5);

        let layer = fs::read_to_string(layer_dir.join("long_term.txt")).unwrap();
        assert!(
            layer.contains("(imp=6) emlék szöveg blokk 1"),
            "layer source must mirror the promotion"
        );
        assert!(layer.contains("(imp=5) emlék szöveg blokk 2"));
    }

    #[test]
    fn bump_entry_matches_exact_text_not_substring() {
        let content = "(imp=5) fontos dolog\n\n(imp=5) ez fontos dolog";
        let result = bump_entry_by_text_hash(content, "fontos dolog", 7).unwrap();
        assert!(result.starts_with("(imp=7) fontos dolog"));
        assert!(
            result.contains("\n\n(imp=5) ez fontos dolog"),
            "overlapping entry must stay untouched"
        );
    }

    #[test]
    fn bump_entry_changes_at_most_one_entry() {
        let content = "(imp=5) ugyanaz\n\n(imp=5) ugyanaz";
        let result = bump_entry_by_text_hash(content, "ugyanaz", 7).unwrap();
        assert_eq!(result.matches("(imp=7) ugyanaz").count(), 1);
        assert_eq!(result.matches("(imp=5) ugyanaz").count(), 1);
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

        let cycle = dream_consolidate(tmp.path(), 10, 0, 8).unwrap();
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

        let cycle = dream_consolidate(tmp.path(), 5, 0, 8).unwrap();
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

        let cycle = dream_consolidate(tmp.path(), 5, 0, 8).unwrap();
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

        let cycle = dream_consolidate(tmp.path(), 5, 0, 8).unwrap();
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
                    forgotten_blocks: 0,
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
                    forgotten_blocks: 0,
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
