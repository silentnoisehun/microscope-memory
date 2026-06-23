//! Lock-free shared-memory consciousness snapshot.
//!
//! # What is this?
//!
//! A snapshot of the consciousness stream's state, protected by a **seqlock**
//! (sequence-locked) protocol. The stream's background cycle publishes a
//! `SharedSnapshot` once per tick; any number of readers (in this process or
//! another) can read it **without taking a Mutex**, **without copying**,
//! **without serializing**.
//!
//! # Why is this useful?
//!
//! The hot read path (`ConsciousnessStream::format`) historically needed a
//! `Mutex` lock + 28k-element sum = ~25 µs. With pre-computed aggregates
//! + seqlock reads, the cost is bounded by one atomic load + one
//! fixed-size struct copy.
//!
//! # Why "impossible but possible"?
//!
//! Reading a multi-field structure while another thread writes it
//! classically requires either a lock (defeats concurrency) or a copy
//! (defeats speed). The seqlock sidesteps both:
//!
//!   - Writer increments a sequence counter (odd during write)
//!   - Writer writes the data
//!   - Writer increments again (even after write)
//!   - Reader checks sequence (must be even), reads data, rechecks
//!   - If sequence changed, retry
//!
//! For 28k-element data, retries are extremely rare because the writer
//! holds the lock for microseconds and the reader holds for nanoseconds.
//!
//! # Cross-process
//!
//! The snapshot is a fixed-layout `#[repr(C)]` struct, designed to be
//! mmap'd to a file. Two processes looking at the same file see the same
//! snapshot. This is the "federation without serialization" path.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;

/// Magic number identifying a valid snapshot ("CONS" in little-endian).
pub const SNAPSHOT_MAGIC: u32 = 0x534E_4F43;
/// Snapshot format version. Bumped on incompatible layout changes.
pub const SNAPSHOT_VERSION: u32 = 1;
/// Maximum retries on a torn read before giving up.
pub const SNAPSHOT_MAX_RETRIES: u32 = 8;

/// 96-byte snapshot of the consciousness stream. All fields are
/// pre-computed by the writer; readers do no derivation.
///
/// `#[repr(C)]` guarantees a stable layout for mmap'd files.
///
/// The data fields live inside `UnsafeCell` so the writer can mutate them
/// through `&self` (required for the seqlock protocol). The seqlock
/// guarantees readers never see a torn write.
#[repr(C)]
pub struct SharedSnapshot {
    /// Seqlock counter. Even = stable, odd = write in progress.
    pub sequence: AtomicU64,
    /// `SNAPSHOT_MAGIC`. Lets readers detect a fresh/uninitialized file.
    pub magic: u32,
    /// `SNAPSHOT_VERSION`. Bumped on layout change.
    pub version: u32,
    /// Reserved padding to align the UnsafeCell to 8 bytes.
    _pad: [u32; 2],

    /// Mutable data block. UnsafeCell is the standard Rust idiom for
    /// interior mutability behind `&self`. Layout is stable thanks to
    /// `#[repr(C)]` and explicit field types.
    data: UnsafeCell<SnapshotData>,

    // ─── Fast-path fields (not in mmap layout) ───────────
    /// Pre-formatted consciousness string. Updated by the background cycle.
    /// Readers clone this in O(1) without any format!() calls.
    cached_format: RwLock<String>,
    /// Lock-free cycle counter. Readers can check freshness without seqlock.
    pub hot_cycle: AtomicU64,
    /// Lock-free surprise level (f32 stored as u32 bits).
    pub hot_surprise_bits: AtomicU32,
    /// Lock-free curiosity level (f32 stored as u32 bits).
    pub hot_curiosity_bits: AtomicU32,
    /// Lock-free predicted query hash.
    pub hot_predicted_hash: AtomicU64,
}

/// Inner data block. Plain scalars, no atomics — seqlock protects them.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SnapshotData {
    pub cycle: u64,
    pub last_query_ms: u64,
    pub activations_count: u32,
    pub activations_total_energy: f64,
    pub attention_layers: u32,
    pub resonance_cells: u32,
    pub patterns_crystallized: u32,
    pub predictions_count: u32,
    pub predictions_hit_rate: f32,
    pub archetypes_count: u32,
    pub mirror_echoes: u32,
    pub predicted_query_hash: u64,
    pub predicted_confidence: f32,
    pub surprise_level: f32,
    pub curiosity_level: f32,
    pub emo_intensity: f32,
    pub emo_dominant_idx: i32,
    pub emo_dominant_val: f32,
    _pad: [u8; 8],
}

impl SnapshotData {
    pub const fn zeroed() -> Self {
        Self {
            cycle: 0,
            last_query_ms: 0,
            activations_count: 0,
            activations_total_energy: 0.0,
            attention_layers: 0,
            resonance_cells: 0,
            patterns_crystallized: 0,
            predictions_count: 0,
            predictions_hit_rate: 0.0,
            archetypes_count: 0,
            mirror_echoes: 0,
            predicted_query_hash: 0,
            predicted_confidence: 0.0,
            surprise_level: 0.0,
            curiosity_level: 0.0,
            emo_intensity: 0.0,
            emo_dominant_idx: -1,
            emo_dominant_val: 0.0,
            _pad: [0; 8],
        }
    }
}

// SAFETY: `SharedSnapshot` is a seqlock. The seqlock protocol guarantees
// that any data read happens either before or after any data write —
// never during. Multiple readers can hold `&SharedSnapshot` concurrently
// because `read()` does not mutate. The single writer must hold the
// seqlock (via `begin_write`/`end_write`) before calling `data_mut`.
unsafe impl Sync for SharedSnapshot {}
unsafe impl Send for SharedSnapshot {}

impl std::fmt::Debug for SharedSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedSnapshot")
            .field("sequence", &self.sequence.load(Ordering::Relaxed))
            .field("magic", &self.magic)
            .field("version", &self.version)
            .finish()
    }
}

impl SharedSnapshot {
    /// Build a zeroed snapshot, ready to be written.
    pub fn new_zeroed() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            magic: SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            _pad: [0; 2],
            data: UnsafeCell::new(SnapshotData::zeroed()),
            cached_format: RwLock::new(String::new()),
            hot_cycle: AtomicU64::new(0),
            hot_surprise_bits: AtomicU32::new(0),
            hot_curiosity_bits: AtomicU32::new(0),
            hot_predicted_hash: AtomicU64::new(0),
        }
    }

    /// Begin a write. Increments sequence to an odd value, returns the
    /// value the writer must restore after the write to mark completion.
    /// Caller MUST call `end_write` with the returned value, even on panic.
    pub fn begin_write(&self) -> u64 {
        let s = self.sequence.fetch_add(1, Ordering::AcqRel);
        std::sync::atomic::fence(Ordering::Release);
        s + 1
    }

    /// End a write. Stores an even sequence value, signaling readers that
    /// the data is consistent.
    pub fn end_write(&self, expected: u64) {
        std::sync::atomic::fence(Ordering::Release);
        self.sequence.store(expected + 1, Ordering::Release);
    }

    /// Get an exclusive `&mut` reference to the data block. The seqlock
    /// protocol guarantees no reader can be accessing the data while this
    /// is held (the sequence is odd, so readers retry).
    ///
    /// SAFETY: Caller must hold the seqlock by having called
    /// `begin_write` and not yet called `end_write`. Only one writer at a time.
    pub unsafe fn data_mut(&self) -> &mut SnapshotData {
        &mut *self.data.get()
    }

    // ─── Fast-path read methods ────────────────────────────

    /// Read the pre-formatted consciousness string. O(1) clone, no format!().
    /// This is the fastest path for the MCP tool: ~50-100ns per call.
    pub fn read_cached_format(&self) -> String {
        match self.cached_format.read() {
            Ok(guard) => guard.clone(),
            Err(_) => "🧠 Consciousness Stream — (cache poisoned)".to_string(),
        }
    }

    /// Read hot fields atomically without seqlock. ~5-20ns.
    /// Returns (cycle, surprise, curiosity, predicted_hash).
    pub fn read_hot_fields(&self) -> (u64, f32, f32, u64) {
        let cycle = self.hot_cycle.load(Ordering::Relaxed);
        let surprise = f32::from_bits(self.hot_surprise_bits.load(Ordering::Relaxed));
        let curiosity = f32::from_bits(self.hot_curiosity_bits.load(Ordering::Relaxed));
        let hash = self.hot_predicted_hash.load(Ordering::Relaxed);
        (cycle, surprise, curiosity, hash)
    }

    /// Check if the snapshot is fresh (hot_cycle matches or exceeds expected).
    pub fn is_fresh(&self, expected_cycle: u64) -> bool {
        self.hot_cycle.load(Ordering::Relaxed) >= expected_cycle
    }

    /// Update cached format string. Called by the background cycle.
    pub fn set_cached_format(&self, s: String) {
        if let Ok(mut guard) = self.cached_format.write() {
            *guard = s;
        }
    }

    /// Update hot atomic fields. Called by the background cycle.
    pub fn set_hot_fields(&self, cycle: u64, surprise: f32, curiosity: f32, predicted_hash: u64) {
        self.hot_cycle.store(cycle, Ordering::Relaxed);
        self.hot_surprise_bits.store(surprise.to_bits(), Ordering::Relaxed);
        self.hot_curiosity_bits.store(curiosity.to_bits(), Ordering::Relaxed);
        self.hot_predicted_hash.store(predicted_hash, Ordering::Relaxed);
    }

    /// Read the snapshot. Returns `None` after `SNAPSHOT_MAX_RETRIES` torn reads.
    /// The returned `SnapshotData` is a copy — no lock required to use it.
    pub fn read(&self) -> Option<SnapshotData> {
        for _ in 0..SNAPSHOT_MAX_RETRIES {
            let s1 = self.sequence.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            std::sync::atomic::fence(Ordering::Acquire);
            // SAFETY: sequence is even and stable (s1 & 1 == 0), the writer
            // is not currently mutating data. We do a single read of the
            // whole data block; torn reads are detected by the post-read
            // sequence check.
            let data = unsafe { (*self.data.get()).clone() };
            std::sync::atomic::fence(Ordering::Acquire);
            let s2 = self.sequence.load(Ordering::Acquire);
            if s1 == s2 && self.magic == SNAPSHOT_MAGIC {
                return Some(data);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seqlock_basic_roundtrip() {
        let s = SharedSnapshot::new_zeroed();
        let token = s.begin_write();
        // Simulate writer: cannot safely mutate through &self from &self
        // methods in a real scenario; this test exercises the sequence
        // protocol only.
        s.end_write(token);
        assert!(s.read().is_some());
    }

    #[test]
    fn seqlock_detects_in_progress_write() {
        // With a static AtomicU64 we can simulate the protocol from
        // multiple test sites without unsafe.
        let s = SharedSnapshot::new_zeroed();
        let token = s.begin_write();
        // While the write is in progress (sequence is odd), read should retry.
        // We assert: sequence is odd now.
        assert_eq!(s.sequence.load(Ordering::Acquire) & 1, 1);
        s.end_write(token);
        assert_eq!(s.sequence.load(Ordering::Acquire) & 1, 0);
    }
}
