//! Hebbian learning layer for Microscope Memory.
//!
//! "Neurons that fire together wire together."
//!
//! Tracks block activations and co-activations from queries.
//! Over time, frequently co-activated blocks drift their coordinates closer.
//! Energy decays exponentially — recently active blocks are "hot".
//!
//! Binary formats:
//!   activations.bin — per-block activation state (HEB2, sparse: only records
//!     that differ from the default are stored, plus block_count)
//!   activations_delta.bin — append-only, CRC-checked journal of the records a
//!     recall touched (AEM2); folded into the base on a full save or at
//!     JOURNAL_MAX_RECORDS
//!   coactivations.bin — sparse co-activation pairs (COA1)
//!   fingerprints.bin — activation fingerprints for mirror neurons (FPR1)

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

const ACTIVATION_RECORD_BYTES: usize = 32; // manual serialization, not sizeof
/// Upper bound on how far `record_activation` will extend the activation
/// vector. It exists only to reject the `u32::MAX` "dropped block" sentinel
/// the retention remap uses -- not to police real block indices, which cannot
/// reach it. See the call site.
const ACTIVATION_GROWTH_LIMIT: usize = 4_000_000;
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
#[derive(Clone)]
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

        // Update per-block activation records with saturation protection
        for &(block_idx, _score) in results {
            let idx = block_idx as usize;
            if idx < self.activations.len() {
                let rec = &mut self.activations[idx];
                rec.activation_count = rec.activation_count.saturating_add(1);
                rec.last_activated_ms = now_ms;
                rec.energy = 1.0; // fresh activation = max energy (already saturated)
            } else if idx < ACTIVATION_GROWTH_LIMIT {
                // Grow rather than drop. The vector is a sparse prefix, so an
                // index past its end is an untouched block, not an invalid one,
                // and skipping it would lose that block's very first
                // activation -- silently, with no error anywhere.
                //
                // The limit rejects the `u32::MAX` "dropped block" sentinel the
                // retention remap uses, which here would ask for a 128 GB
                // allocation. It is four times the largest index this project has
                // shipped (967,587), so no real block index can reach it.
                self.activations
                    .resize(idx + 1, ActivationRecord::default());
                let rec = &mut self.activations[idx];
                rec.activation_count = 1;
                rec.last_activated_ms = now_ms;
                rec.energy = 1.0;
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
                pair.count = pair.count.saturating_add(1);
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

        // First: decay all energies (exponential half-life decay)
        // Correct formula: energy *= exp(-elapsed * ln(2) / half_life)
        // The previous version multiplied energy by a negative factor then took exp(),
        // which collapsed all energies toward 0 regardless of their initial value.
        for rec in &mut self.activations {
            if rec.energy > 0.0 && rec.last_activated_ms > 0 {
                let elapsed_ms = (now_ms - rec.last_activated_ms) as f64;
                if elapsed_ms < 0.0 {
                    // Clock skew or future timestamp — skip to avoid panics
                    continue;
                }
                let decay =
                    (-(elapsed_ms / ENERGY_HALF_LIFE_MS) * std::f64::consts::LN_2).exp() as f32;
                rec.energy *= decay;
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

    /// Block indices whose decayed energy is at or above `threshold`, ascending.
    ///
    /// The energy lives in memory, so the candidate set can be built before any
    /// block header is touched. A consumer that would otherwise scan all
    /// 699,110 headers to find the handful of hot blocks reads only those
    /// headers. The result is exactly `{i : energy(i) >= threshold}`, in
    /// ascending order, so a full-scan formulation and this one produce
    /// bit-identical sums.
    pub fn hot_indices(&self, threshold: f32) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, rec) in self.activations.iter().enumerate() {
            // energy() is 0.0 in exactly these two cases; skip the exp() for
            // them, which is nearly every record on a fresh index.
            if rec.energy <= 0.0 || rec.last_activated_ms == 0 {
                continue;
            }
            if self.energy(i) >= threshold {
                out.push(i);
            }
        }
        out
    }

    /// Save all Hebbian state to binary files. Writes the activation base in
    /// full, which is the correct thing after a rebuild or a remap.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_activations(output_dir, &self.activations)?;
        save_coactivations(output_dir, &self.coactivations)?;
        save_fingerprints(output_dir, &self.fingerprints)?;
        Ok(())
    }

    /// Save after a recall: journal only the blocks that were actually
    /// activated, and rewrite the base only when the journal has reached its
    /// bound or the base is missing.
    ///
    /// The caller passes the dirty indices because it is the one that knows
    /// them -- recall activates exactly the top-k blocks it returned. Nothing is
    /// deferred to process exit on purpose: a CLI invocation is a whole
    /// process, so "write it later" would lose the learning entirely.
    pub fn save_dirty(&self, output_dir: &Path, dirty: &[u32]) -> Result<(), String> {
        let base_missing = !output_dir.join("activations.bin").exists();
        if base_missing || delta_journal_records(output_dir) >= JOURNAL_MAX_RECORDS {
            save_activations(output_dir, &self.activations)?;
        } else {
            append_activation_deltas(output_dir, &self.activations, dirty)?;
        }
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
        blocks.sort_by(|a, b| b.1.total_cmp(&a.1));
        blocks.truncate(n);
        blocks
    }

    /// Get top-N strongest co-activation pairs.
    pub fn strongest_pairs(&self, n: usize) -> Vec<&CoactivationPair> {
        let mut pairs: Vec<&CoactivationPair> = self.coactivations.values().collect();
        pairs.sort_by_key(|b| std::cmp::Reverse(b.count));
        pairs.truncate(n);
        pairs
    }

    /// Remap all block indexes after a rebuild reorders blocks.
    ///
    /// `old_to_new[old_idx] = new_idx`. Blocks that were not in the old
    /// layout get `u32::MAX` and are dropped.
    pub fn remap_indexes(&mut self, old_to_new: &[u32]) {
        let old_len = self.activations.len();
        // u32::MAX marks "dropped" — it must not size the target vector, or a
        // single unmatched block would allocate a 2^32-element array.
        let new_len = old_to_new
            .iter()
            .copied()
            .filter(|&m| m != u32::MAX)
            .max()
            .map_or(0, |m| m as usize + 1);

        // 1. Remap activations vector
        let mut new_activations = vec![ActivationRecord::default(); new_len];
        for (old_idx, &new_idx) in old_to_new
            .iter()
            .enumerate()
            .take(old_len.min(old_to_new.len()))
        {
            if new_idx != u32::MAX && (new_idx as usize) < new_len {
                new_activations[new_idx as usize] = self.activations[old_idx];
            }
        }
        self.activations = new_activations;

        // 2. Remap coactivation pair keys
        let mut new_coactivations = HashMap::new();
        for ((a, b), mut pair) in self.coactivations.drain() {
            let na = if (a as usize) < old_to_new.len() {
                old_to_new[a as usize]
            } else {
                u32::MAX
            };
            let nb = if (b as usize) < old_to_new.len() {
                old_to_new[b as usize]
            } else {
                u32::MAX
            };
            if na != u32::MAX && nb != u32::MAX && na != nb {
                let key = (na.min(nb), na.max(nb));
                pair.block_a = key.0;
                pair.block_b = key.1;
                new_coactivations
                    .entry(key)
                    .and_modify(|e: &mut CoactivationPair| {
                        e.count = e.count.saturating_add(pair.count);
                    })
                    .or_insert(pair);
            }
        }
        self.coactivations = new_coactivations;

        // 3. Remap fingerprint block indexes
        for fp in &mut self.fingerprints {
            for (idx, _) in &mut fp.activations {
                if (*idx as usize) < old_to_new.len() {
                    *idx = old_to_new[*idx as usize];
                } else {
                    *idx = u32::MAX;
                }
            }
            fp.activations.retain(|(idx, _)| *idx != u32::MAX);
        }
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

/// CRC32 (IEEE, bitwise). Validates delta-journal records: a torn tail must be
/// detected and dropped, never applied.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Journal record: 4 (index) + 32 (record) + 4 (crc).
const DELTA_RECORD_BYTES: usize = 4 + ACTIVATION_RECORD_BYTES + 4;
const DELTA_MAGIC: &[u8; 4] = b"AEM2";
const DELTA_FILE: &str = "activations_delta.bin";
/// Compact the base once the journal holds this many records (~160 KB), so the
/// load path stays bounded however long the process keeps learning.
const JOURNAL_MAX_RECORDS: usize = 4096;

fn decode_activation_record(data: &[u8], off: usize) -> ActivationRecord {
    ActivationRecord {
        activation_count: read_u32(data, off),
        last_activated_ms: read_u64(data, off + 4),
        drift_x: read_f32(data, off + 12),
        drift_y: read_f32(data, off + 16),
        drift_z: read_f32(data, off + 20),
        energy: read_f32(data, off + 24),
        _pad: read_u32(data, off + 28),
    }
}

fn encode_activation_record(buf: &mut Vec<u8>, rec: &ActivationRecord) {
    buf.extend_from_slice(&rec.activation_count.to_le_bytes());
    buf.extend_from_slice(&rec.last_activated_ms.to_le_bytes());
    buf.extend_from_slice(&rec.drift_x.to_le_bytes());
    buf.extend_from_slice(&rec.drift_y.to_le_bytes());
    buf.extend_from_slice(&rec.drift_z.to_le_bytes());
    buf.extend_from_slice(&rec.energy.to_le_bytes());
    buf.extend_from_slice(&rec._pad.to_le_bytes());
}

/// True when a record carries no information, i.e. it equals what a freshly
/// resized vector already holds. On a clean 699k-block index that is all of
/// them, and none of them is ever written.
fn is_default_record(rec: &ActivationRecord) -> bool {
    rec.activation_count == 0
        && rec.last_activated_ms == 0
        && rec.drift_x == 0.0
        && rec.drift_y == 0.0
        && rec.drift_z == 0.0
        && rec.energy == 0.0
        && rec._pad == 0
}

/// Apply the append-only delta journal on top of a loaded base.
///
/// A crash can leave a partial record. Records are CRC-checked and the first
/// bad one ends the scan: a torn tail is dropped, never applied, and the file
/// is truncated to the last good record so the next append lands on a valid
/// boundary.
fn apply_activation_deltas(
    output_dir: &Path,
    records: &mut Vec<ActivationRecord>,
    block_count: usize,
) {
    let path = output_dir.join(DELTA_FILE);
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(_) => return,
    };
    if data.len() < 8 || &data[0..4] != DELTA_MAGIC {
        // Too short, or not ours (a future format): ignore rather than
        // misinterpret. Never delete a file we do not understand.
        return;
    }
    let mut pos = 8usize;
    while pos + DELTA_RECORD_BYTES <= data.len() {
        let body = &data[pos..pos + 4 + ACTIVATION_RECORD_BYTES];
        if crc32(body) != read_u32(&data, pos + 4 + ACTIVATION_RECORD_BYTES) {
            break;
        }
        let idx = read_u32(&data, pos) as usize;
        if idx >= block_count {
            // The record names a block this corpus no longer has. Stop: a
            // rebuild re-creates the file, and resurrecting a block the index
            // does not have would be wrong.
            break;
        }
        if idx >= records.len() {
            // Legitimate, and past the base file's last stored index. Grow.
            // Bounding this by `records.len()` instead of `block_count` -- which
            // is what the old corpus-sized vector made indistinguishable --
            // truncates the whole journal at the first record not yet folded
            // into the base, silently losing every record after it.
            records.resize(idx + 1, ActivationRecord::default());
        }
        records[idx] = decode_activation_record(&data, pos + 4);
        pos += DELTA_RECORD_BYTES;
    }
    if pos != data.len() {
        // Trim the unusable tail so the next append cannot be misread as part
        // of a torn record.
        if let Ok(f) = fs::OpenOptions::new().write(true).open(&path) {
            let _ = f.set_len(pos as u64);
        }
    }
}

fn load_activations(output_dir: &Path, block_count: usize) -> Vec<ActivationRecord> {
    let path = output_dir.join("activations.bin");
    // Sized from the stored records, not from `block_count`. Allocating the
    // full block count zeroed 30,962,784 bytes on every recall to hold the 197
    // records in a 7,104-byte file: 6.8 ms of a 37.4 ms query, scaling with the
    // corpus rather than with what had actually been learned.
    //
    // The vector is a sparse prefix: `len()` is the highest stored index plus
    // one, and an index at or past it has never been activated. For the
    // evaluation index the highest stored index is 12,625, so 0.4 MB replaces
    // 31 MB.
    let mut records: Vec<ActivationRecord> = Vec::new();
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 12 && &data[0..4] == b"HEB2" {
            // [HEB2][u32 block_count][u32 stored][(u32 idx, record)...]
            let _stored = read_u32(&data, 4) as usize;
            let count = read_u32(&data, 8) as usize;
            let stride = 4 + ACTIVATION_RECORD_BYTES;
            // Size from the record indices only.
            //
            // The field at offset 4 is named `stored` here but it is the
            // corpus block_count, not a record count -- the header layout is
            // [HEB2][u32 block_count][u32 count]. Folding it into `needed` is
            // what made a first attempt at this change a silent no-op: the
            // vector stayed at 967,587 entries and the load time did not move
            // at all.
            let mut needed = 0usize;
            for i in 0..count {
                let off = 12 + i * stride;
                if off + stride > data.len() {
                    break;
                }
                let idx = read_u32(&data, off) as usize;
                if idx < block_count {
                    needed = needed.max(idx + 1);
                }
            }
            records.resize(needed, ActivationRecord::default());
            for i in 0..count {
                let off = 12 + i * stride;
                if off + stride > data.len() {
                    break;
                }
                let idx = read_u32(&data, off) as usize;
                if idx < records.len() {
                    records[idx] = decode_activation_record(&data, off + 4);
                }
            }
        } else if data.len() >= 8 && &data[0..4] == b"HEB1" {
            // Legacy dense layout, still written by older builds. Read it, and
            // the next save converts the file to the sparse form.
            let stored_count = read_u32(&data, 4) as usize;
            let expected_size = 8 + stored_count * ACTIVATION_RECORD_BYTES;
            if data.len() >= expected_size {
                records.clear();
                records.reserve(block_count.max(stored_count));
                for i in 0..stored_count {
                    records.push(decode_activation_record(
                        &data,
                        8 + i * ACTIVATION_RECORD_BYTES,
                    ));
                }
                records.resize(block_count.max(stored_count), ActivationRecord::default());
            }
        }
    }
    apply_activation_deltas(output_dir, &mut records, block_count);
    records
}

/// Write the sparse base atomically and retire the journal: after this a load
/// needs the base only. This is the checkpoint a rebuild or a remap relies on.
fn save_activations(output_dir: &Path, records: &[ActivationRecord]) -> Result<(), String> {
    let path = output_dir.join("activations.bin");
    let idxs: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(_, rec)| !is_default_record(rec))
        .map(|(i, _)| i)
        .collect();
    let mut buf = Vec::with_capacity(12 + idxs.len() * (4 + ACTIVATION_RECORD_BYTES));
    buf.extend_from_slice(b"HEB2");
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(idxs.len() as u32).to_le_bytes());
    for &i in &idxs {
        buf.extend_from_slice(&(i as u32).to_le_bytes());
        encode_activation_record(&mut buf, &records[i]);
    }
    let tmp_path = output_dir.join("activations.bin.tmp");
    fs::write(&tmp_path, &buf).map_err(|e| format!("write activations.bin: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("rename activations.bin: {}", e))?;
    // The base now contains every delta that was journalled.
    let _ = fs::remove_file(output_dir.join(DELTA_FILE));
    Ok(())
}

/// How many records the journal currently holds.
fn delta_journal_records(output_dir: &Path) -> usize {
    match fs::metadata(output_dir.join(DELTA_FILE)) {
        Ok(m) if m.len() > 8 => ((m.len() - 8) / DELTA_RECORD_BYTES as u64) as usize,
        _ => 0,
    }
}

/// Append the given block indices' records to the journal.
///
/// The whole batch goes out with a single append, so a concurrent reader sees
/// either none of it or all of it. Records that still hold the default value
/// are skipped: a block that was decayed back to nothing costs nothing to keep.
fn append_activation_deltas(
    output_dir: &Path,
    records: &[ActivationRecord],
    dirty: &[u32],
) -> Result<(), String> {
    let path = output_dir.join(DELTA_FILE);
    let existing = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut buf: Vec<u8> = Vec::with_capacity(8 + dirty.len() * DELTA_RECORD_BYTES);
    if existing == 0 {
        buf.extend_from_slice(DELTA_MAGIC);
        buf.extend_from_slice(&0u32.to_le_bytes()); // record count, advisory
    }
    let mut written = 0usize;
    for &i in dirty {
        let idx = i as usize;
        if idx >= records.len() {
            continue;
        }
        let rec = &records[idx];
        if is_default_record(rec) {
            continue;
        }
        let start = buf.len();
        buf.extend_from_slice(&i.to_le_bytes());
        encode_activation_record(&mut buf, rec);
        let crc = crc32(&buf[start..]);
        buf.extend_from_slice(&crc.to_le_bytes());
        written += 1;
    }
    if written == 0 && existing > 0 {
        return Ok(());
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open activations journal: {}", e))?;
    f.write_all(&buf)
        .map_err(|e| format!("write activations journal: {}", e))?;
    f.flush()
        .map_err(|e| format!("flush activations journal: {}", e))?;
    Ok(())
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
    let tmp_path = output_dir.join("coactivations.bin.tmp");
    fs::write(&tmp_path, &buf).map_err(|e| format!("write coactivations.bin: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("rename coactivations.bin: {}", e))
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
    let mut buf: Vec<u8> = Vec::with_capacity(8 + fingerprints.len() * 32);
    buf.extend_from_slice(b"FPR1");
    buf.extend_from_slice(&(fingerprints.len() as u32).to_le_bytes());
    for fp in fingerprints {
        buf.extend_from_slice(&fp.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&fp.query_hash.to_le_bytes());
        buf.extend_from_slice(&(fp.activations.len() as u16).to_le_bytes());
        for &(block_idx, score) in &fp.activations {
            buf.extend_from_slice(&block_idx.to_le_bytes());
            buf.extend_from_slice(&score.to_le_bytes());
        }
    }
    // Atomic, like every other state file here. `File::create` truncated in
    // place, so a crash mid-write left a half-written mirror-neuron history
    // that the next load would silently accept as the whole history.
    let tmp_path = output_dir.join("fingerprints.bin.tmp");
    fs::write(&tmp_path, &buf).map_err(|e| format!("write fingerprints.bin: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("rename fingerprints.bin: {}", e))
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
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// The activation vector used to be allocated to the full block count on
    /// every load: 30,962,784 bytes for the 967,587-block evaluation index, to
    /// hold the 197 records in a 7,104-byte file. It is now a sparse prefix,
    /// which changes what a missing entry means and is easy to get wrong.
    ///
    /// The trap this pins: `ActivationRecord::default()` is all zeros, so
    /// "absent" and "present but untouched" are indistinguishable by value --
    /// but not by `Option`, and the vector is read with two fallbacks:
    ///
    ///   commands/recall.rs:402  .get(idx).map(|a| a.energy).unwrap_or(0.0)
    ///   dream.rs:588            .get(i).map(|r| r.energy).unwrap_or(0.0)
    ///
    /// Both must stay at 0.0. recall.rs used 0.5, which was unreachable while
    /// the vector was corpus-sized; under the sparse prefix it would have given
    /// every never-recalled block a neutral salience, ranking the whole
    /// untouched corpus above blocks that were activated and then decayed.
    #[test]
    fn untouched_activation_is_absent_and_reads_as_zero() {
        let dir = tmp_dir("hebb_absent_vs_zero");
        let mut hebb = HebbianState::load_or_init(&dir, 4);

        // A fresh index stores nothing, so nothing is allocated per block.
        assert!(
            hebb.activations.is_empty(),
            "a fresh index must not allocate a record per block, got {}",
            hebb.activations.len()
        );

        // Index 3 was never activated: absent, not a zeroed record.
        let rec = hebb.activations.get(3);
        assert!(
            rec.is_none(),
            "an unstored index must read as None under the sparse prefix"
        );
        assert_eq!(rec.map(|a| a.energy).unwrap_or(0.0), 0.0);
        assert_eq!(rec.map(|r| r.energy).unwrap_or(0.0), 0.0);

        // A stored record is still Some, with its energy intact.
        hebb.activations.resize(4, ActivationRecord::default());
        hebb.activations[3].energy = 0.4;
        assert_eq!(hebb.activations.get(3).map(|a| a.energy), Some(0.4));
    }

    /// Activating a block for the first time must create its record, not drop
    /// the activation because the index is past the end of a sparse vector.
    /// This is what the growth branch in `record_activation` exists to prevent,
    /// and the failure would be silent: no error, just a block that never learns.
    #[test]
    fn first_activation_of_a_high_index_grows_the_vector() {
        let dir = tmp_dir("hebb_first_activation");
        let mut hebb = HebbianState::load_or_init(&dir, 10_000);
        assert!(hebb.activations.is_empty(), "a fresh index starts empty");

        hebb.record_activation(&[(9_000, 0.9)], 0xdead_beef);

        assert!(
            hebb.activations.len() > 9_000,
            "the record must be created, len is {}",
            hebb.activations.len()
        );
        assert_eq!(hebb.activations[9_000].activation_count, 1);
        assert_eq!(hebb.activations[9_000].energy, 1.0);
    }

    /// A record stored at a high index must survive a load, and the vector must
    /// still cover it. Guards the other half: sizing the vector to the number of
    /// stored records would drop index 9,000 in a corpus of 10,000.
    #[test]
    fn high_index_record_round_trips() {
        let dir = tmp_dir("hebb_high_index");
        let mut hebb = HebbianState::load_or_init(&dir, 10_000);
        // Three activations, so the round trip has a count to preserve.
        for _ in 0..3 {
            hebb.record_activation(&[(9_000, 0.77)], 1);
        }
        hebb.activations[9_000].energy = 0.77;
        hebb.save(&dir).unwrap();

        let back = HebbianState::load_or_init(&dir, 10_000);
        assert_eq!(back.activations[9_000].energy, 0.77);
        assert_eq!(back.activations[9_000].activation_count, 3);
        // Untouched neighbours are still addressable.
        assert_eq!(back.activations[8_999].energy, 0.0);
    }

    #[test]
    fn sparse_base_stores_only_learned_records() {
        let dir = tmp_dir("mscope_hebb_sparse");
        let n = 1000;
        let mut recs = vec![ActivationRecord::default(); n];
        recs[7].activation_count = 3;
        recs[7].energy = 1.0;
        recs[900].activation_count = 1;
        recs[900].drift_x = 0.05;
        save_activations(&dir, &recs).unwrap();

        let file = fs::read(dir.join("activations.bin")).unwrap();
        assert_eq!(&file[0..4], b"HEB2");
        assert!(
            file.len() < 200,
            "sparse base should be a few records, got {} bytes",
            file.len()
        );
        assert!(file.len() < 8 + n * ACTIVATION_RECORD_BYTES);

        let back = load_activations(&dir, n);
        // Sized to the highest stored index, not to the block count: 901
        // entries cover this 1,000-block corpus, which is the whole point of
        // the change. The base file was already sparse on disk; now memory
        // matches it.
        assert_eq!(back.len(), 901, "sized to the highest stored index, not n");
        assert_eq!(back[7].activation_count, 3);
        assert_eq!(back[7].energy, 1.0);
        assert_eq!(back[900].drift_x, 0.05);
        assert!(is_default_record(&back[11]));
        // `back[901]` no longer exists, and that is the change: the vector
        // stops at the highest stored index rather than at the block count.
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_dense_base_still_loads() {
        let dir = tmp_dir("mscope_hebb_legacy");
        let n = 64;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"HEB1");
        buf.extend_from_slice(&(n as u32).to_le_bytes());
        for i in 0..n {
            let mut rec = ActivationRecord::default();
            if i == 5 {
                rec.activation_count = 9;
                rec.energy = 0.75;
            }
            encode_activation_record(&mut buf, &rec);
        }
        fs::write(dir.join("activations.bin"), &buf).unwrap();

        let back = load_activations(&dir, n);
        assert_eq!(back.len(), n);
        assert_eq!(back[5].activation_count, 9);
        assert_eq!(back[5].energy, 0.75);
        assert!(is_default_record(&back[6]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn journaled_update_is_visible_to_the_next_process() {
        // The second load must see the first process's learning. This is why the
        // write is a journal rather than a buffer flushed at exit: a CLI
        // invocation is a whole process and there is no later.
        let dir = tmp_dir("mscope_hebb_journal");
        let n = 500;
        let mut first = HebbianState::load_or_init(&dir, n);
        first.record_activation(&[(3, 0.9)], 1);
        first.save_dirty(&dir, &[3]).unwrap(); // bootstraps the base

        let mut second = HebbianState::load_or_init(&dir, n);
        assert_eq!(second.activations[3].activation_count, 1);
        second.record_activation(&[(3, 0.8), (4, 0.4)], 2);
        second.save_dirty(&dir, &[3, 4]).unwrap(); // journals
        assert!(
            dir.join(DELTA_FILE).exists(),
            "an incremental save must not rewrite the whole base"
        );

        let third = HebbianState::load_or_init(&dir, n);
        assert_eq!(third.activations[3].activation_count, 2);
        assert_eq!(third.activations[4].activation_count, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn torn_journal_tail_is_dropped_and_the_file_is_truncated() {
        let dir = tmp_dir("mscope_hebb_torn");
        let n = 200;
        let mut first = HebbianState::load_or_init(&dir, n);
        first.record_activation(&[(1, 1.0)], 1);
        first.save_dirty(&dir, &[1]).unwrap();

        let mut second = HebbianState::load_or_init(&dir, n);
        second.record_activation(&[(2, 1.0)], 2);
        second.save_dirty(&dir, &[2]).unwrap();

        // A crash mid-append leaves a partial record at the tail.
        let jp = dir.join(DELTA_FILE);
        let mut data = fs::read(&jp).unwrap();
        let good_len = data.len();
        data.extend_from_slice(&[0x01, 0x02, 0x03]);
        fs::write(&jp, &data).unwrap();

        let loaded = HebbianState::load_or_init(&dir, n);
        assert_eq!(loaded.activations[1].activation_count, 1);
        assert_eq!(
            loaded.activations[2].activation_count, 1,
            "the record before the tear must be applied"
        );
        assert_eq!(
            fs::metadata(&jp).unwrap().len(),
            good_len as u64,
            "the unusable tail is truncated so the next append is readable"
        );

        // A later append must still be seen: the tear cost one record, not the
        // journal.
        let mut third = loaded.clone();
        third.record_activation(&[(3, 1.0)], 3);
        third.save_dirty(&dir, &[3]).unwrap();
        let after = HebbianState::load_or_init(&dir, n);
        assert_eq!(after.activations[3].activation_count, 1);
        assert_eq!(after.activations[2].activation_count, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn journal_is_folded_into_the_base_at_the_threshold() {
        let dir = tmp_dir("mscope_hebb_compact");
        let n = 64;
        // Pre-fill the journal past its bound with valid records.
        let mut buf = Vec::new();
        buf.extend_from_slice(DELTA_MAGIC);
        buf.extend_from_slice(&0u32.to_le_bytes());
        for i in 0..JOURNAL_MAX_RECORDS {
            let rec = ActivationRecord {
                activation_count: 1,
                energy: 1.0,
                ..Default::default()
            };
            let start = buf.len();
            buf.extend_from_slice(&((i % n) as u32).to_le_bytes());
            encode_activation_record(&mut buf, &rec);
            let crc = crc32(&buf[start..]);
            buf.extend_from_slice(&crc.to_le_bytes());
        }
        fs::write(dir.join(DELTA_FILE), &buf).unwrap();
        assert!(delta_journal_records(&dir) >= JOURNAL_MAX_RECORDS);

        // A load folds the journal into memory; the next save must checkpoint.
        let state = HebbianState::load_or_init(&dir, n);
        state.save_dirty(&dir, &[1]).unwrap();
        assert_eq!(
            delta_journal_records(&dir),
            0,
            "the journal is retired when the base is rewritten"
        );
        let data = fs::read(dir.join("activations.bin")).unwrap();
        assert_eq!(&data[0..4], b"HEB2");
        let back = load_activations(&dir, n);
        // The journal is last-write-wins per index, so 4096 records over 64
        // indices must leave exactly those 64 at their latest value -- not 4096
        // activations. Compaction must not lose an index.
        let learned = back.iter().filter(|r| r.activation_count > 0).count();
        assert_eq!(learned, n, "every journalled index must survive compaction");
        assert!(back.iter().all(|r| r.activation_count == 1));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remap_keeps_activations_across_a_rebuild() {
        let dir = tmp_dir("mscope_hebb_remap");
        let n = 10;
        let mut state = HebbianState::load_or_init(&dir, n);
        state.record_activation(&[(2, 1.0), (7, 1.0)], 42);
        state.save(&dir).unwrap(); // a rebuild checkpoints in full

        // Reverse the layout, as a rebuild that reorders blocks would.
        let map: Vec<u32> = (0..n).map(|i| (n - 1 - i) as u32).collect();
        let mut state = HebbianState::load_or_init(&dir, n);
        state.remap_indexes(&map);
        state.save(&dir).unwrap();

        let back = HebbianState::load_or_init(&dir, n);
        assert_eq!(back.activations[n - 1 - 2].activation_count, 1);
        assert_eq!(back.activations[n - 1 - 7].activation_count, 1);
        let _ = fs::remove_dir_all(&dir);
    }

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
// ── MM-001: remap_indexes tests ────────────────────

#[test]
fn mm001_remap_preserves_activation_energy() {
    let mut hebb = HebbianState {
        activations: vec![ActivationRecord::default(); 5],
        coactivations: HashMap::new(),
        fingerprints: Vec::new(),
    };
    // Block 2 has high energy
    hebb.activations[2].energy = 0.95;
    hebb.activations[2].activation_count = 42;

    // Rebuild maps: old[0]->new[3], old[1]->new[0], old[2]->new[4], old[3]->new[1], old[4]->new[2]
    let old_to_new = vec![3u32, 0, 4, 1, 2];
    hebb.remap_indexes(&old_to_new);

    // Block 2's energy should now be at index 4
    assert_eq!(hebb.activations[4].energy, 0.95);
    assert_eq!(hebb.activations[4].activation_count, 42);
    // Old index 2 should now be empty (default)
    assert_eq!(hebb.activations[2].energy, 0.0);
}

#[test]
fn mm001_remap_coactivations_merge_duplicates() {
    let mut hebb = HebbianState {
        activations: vec![ActivationRecord::default(); 4],
        coactivations: HashMap::new(),
        fingerprints: Vec::new(),
    };
    // Two pairs that will merge after remap
    hebb.coactivations.insert(
        (0, 1),
        CoactivationPair {
            block_a: 0,
            block_b: 1,
            count: 3,
            last_ts_ms: 100,
        },
    );
    hebb.coactivations.insert(
        (2, 3),
        CoactivationPair {
            block_a: 2,
            block_b: 3,
            count: 5,
            last_ts_ms: 200,
        },
    );

    // old[0]->new[0], old[1]->new[2], old[2]->new[0], old[3]->new[2]
    // Both pairs map to (0, 2) — should merge counts
    let old_to_new = vec![0u32, 2, 0, 2];
    hebb.remap_indexes(&old_to_new);

    assert_eq!(hebb.coactivations.len(), 1, "pairs should merge");
    let merged = hebb.coactivations.values().next().unwrap();
    assert_eq!(merged.count, 8, "counts should be summed");
}

#[test]
fn mm001_remap_drops_out_of_range() {
    let mut hebb = HebbianState {
        activations: vec![ActivationRecord::default(); 3],
        coactivations: HashMap::new(),
        fingerprints: Vec::new(),
    };
    hebb.activations[1].energy = 0.5;

    // Remap to a smaller array — old index 2 is out of range
    let old_to_new = vec![0u32, 1];
    hebb.remap_indexes(&old_to_new);

    assert_eq!(hebb.activations.len(), 2);
    assert_eq!(hebb.activations[1].energy, 0.5);
}

#[test]
fn mm001_remap_ignores_drop_markers_when_sizing() {
    let mut hebb = HebbianState {
        activations: vec![ActivationRecord::default(); 3],
        coactivations: HashMap::new(),
        fingerprints: Vec::new(),
    };
    hebb.activations[1].energy = 0.5;

    // A dropped block (u32::MAX) must NOT size the target vector: the old
    // code computed new_len = max(map) + 1 and tried to allocate a
    // 2^32-element array whenever any block was dropped.
    let old_to_new = vec![0u32, 1, u32::MAX];
    hebb.remap_indexes(&old_to_new);

    assert_eq!(hebb.activations.len(), 2);
    assert_eq!(hebb.activations[1].energy, 0.5);
}
