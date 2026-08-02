//! Lock-free consciousness snapshot, protected by a **seqlock** protocol.
//!
//! # What is this?
//!
//! A snapshot of the consciousness stream's state. The stream's background
//! cycle publishes a `SharedSnapshot` once per tick; any number of readers in
//! the **same process** can read it **without taking a Mutex**, **without
//! copying**, **without serializing**.
//!
//! # Why is this useful?
//!
//! The hot read path (`ConsciousnessStream::format`) historically needed a
//! `Mutex` lock plus 28k-element sum = ~25 µs. With pre-computed aggregates
//! plus seqlock reads, the cost is bounded by one atomic load plus one
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
//! # Cross-process (mmap) layer
//!
//! The in-process path (`SharedSnapshot` behind `Arc`) is used by
//! `ConsciousnessStream`. For cross-process federation, the file-backed mmap
//! layer below (`MappedSnapshot` / `MappedSnapshotWriter`) maps the seqlock
//! header + `SnapshotData` block — the `#[repr(C)]` prefix that is
//! byte-identical to `SharedSnapshot`'s layout — so two processes looking at
//! the same file see the same snapshot with the same seqlock protocol. The
//! heap-backed fast-path fields (`RwLock<String>`, hot atomics) stay
//! in-process only.

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
/// `#[repr(C)]` keeps the seqlock header and data block layout stable
/// (relevant if a future file-mmap layer is ever built).
/// The data fields live inside `UnsafeCell` so the writer can mutate them
/// through `&self` (required for the seqlock protocol). The seqlock
/// guarantees readers never see a torn write.
#[repr(C)]
pub struct SharedSnapshot {
    /// Seqlock counter. Even = stable, odd = write in progress.
    pub sequence: AtomicU64,
    /// `SNAPSHOT_MAGIC`. Lets readers detect a fresh/uninitialized snapshot.
    pub magic: u32,
    /// `SNAPSHOT_VERSION`. Bumped on layout change.
    pub version: u32,
    /// Reserved padding to align the UnsafeCell to 8 bytes.
    _pad: [u32; 2],

    /// Mutable data block. UnsafeCell is the standard Rust idiom for
    /// interior mutability behind `&self`. Layout is stable thanks to
    /// `#[repr(C)]` and explicit field types.
    data: UnsafeCell<SnapshotData>,

    // ─── Fast-path fields (heap-backed, in-process only) ───────────
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
    /// # Safety
    ///
    /// Caller must hold the seqlock by having called `begin_write` and
    /// not yet called `end_write`. Only one writer at a time.
    #[allow(clippy::mut_from_ref)]
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
        self.hot_surprise_bits
            .store(surprise.to_bits(), Ordering::Relaxed);
        self.hot_curiosity_bits
            .store(curiosity.to_bits(), Ordering::Relaxed);
        self.hot_predicted_hash
            .store(predicted_hash, Ordering::Relaxed);
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
            let data = unsafe { *self.data.get() };
            std::sync::atomic::fence(Ordering::Acquire);
            let s2 = self.sequence.load(Ordering::Acquire);
            if s1 == s2 && self.magic == SNAPSHOT_MAGIC {
                return Some(data);
            }
        }
        None
    }
}

/// File-backed, cross-process seqlock snapshot.
///
/// Only the seqlock header and the `SnapshotData` block are shared; the layout
/// is the `#[repr(C)]` prefix of `SharedSnapshot`, so the two are
/// byte-compatible. The seqlock protocol (odd/even sequence, retry on torn
/// reads) is identical across processes — the OS guarantees coherence of
/// `MAP_SHARED` pages.
#[cfg(not(target_arch = "wasm32"))]
#[repr(C)]
pub struct MmapSnapshot {
    sequence: AtomicU64,
    magic: u32,
    version: u32,
    _pad: [u32; 2],
    data: UnsafeCell<SnapshotData>,
}

// SAFETY: `MmapSnapshot` is a seqlock. The protocol guarantees readers never
// observe a torn write, and the single writer must hold the seqlock before
// touching `data`. This mirrors the `SharedSnapshot` safety argument.
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Sync for MmapSnapshot {}
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Send for MmapSnapshot {}

#[cfg(not(target_arch = "wasm32"))]
impl MmapSnapshot {
    /// Required file size in bytes for a shared snapshot mapping.
    pub const FILE_SIZE: usize = size_of::<MmapSnapshot>();

    /// Validate the magic/version header.
    fn header_valid(&self) -> bool {
        self.magic == SNAPSHOT_MAGIC && self.version == SNAPSHOT_VERSION
    }

    /// Publish a full `SnapshotData` under the seqlock.
    pub fn write(&self, data: &SnapshotData) -> Result<(), String> {
        if !self.header_valid() {
            return Err("snapshot file is not initialized (wrong magic/version)".to_string());
        }
        let s = self.sequence.fetch_add(1, Ordering::AcqRel); // odd: write in progress
        std::sync::atomic::fence(Ordering::Release);
        // SAFETY: sequence is odd, so readers retry; only one writer exists.
        unsafe {
            *self.data.get() = *data;
        }
        std::sync::atomic::fence(Ordering::Release);
        self.sequence.store(s + 2, Ordering::Release); // even: stable
        Ok(())
    }

    /// Begin an in-place write; returns the token for `end_write`.
    pub fn begin_write(&self) -> Result<u64, String> {
        if !self.header_valid() {
            return Err("snapshot file is not initialized (wrong magic/version)".to_string());
        }
        let s = self.sequence.fetch_add(1, Ordering::AcqRel);
        std::sync::atomic::fence(Ordering::Release);
        Ok(s + 1)
    }

    /// End an in-place write started with `begin_write`.
    pub fn end_write(&self, expected: u64) {
        std::sync::atomic::fence(Ordering::Release);
        self.sequence.store(expected + 1, Ordering::Release);
    }

    /// Exclusive `&mut` access to the data block while the seqlock is held.
    ///
    /// # Safety
    /// Caller must hold the seqlock (`begin_write`, not yet `end_write`).
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn data_mut(&self) -> &mut SnapshotData {
        &mut *self.data.get()
    }

    /// Read the snapshot; `None` after `SNAPSHOT_MAX_RETRIES` torn reads or
    /// while the file is not initialized.
    pub fn read(&self) -> Option<SnapshotData> {
        if !self.header_valid() {
            return None;
        }
        for _ in 0..SNAPSHOT_MAX_RETRIES {
            let s1 = self.sequence.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            std::sync::atomic::fence(Ordering::Acquire);
            // SAFETY: sequence is even and stable; torn reads are detected by
            // the post-read sequence check.
            let data = unsafe { *self.data.get() };
            std::sync::atomic::fence(Ordering::Acquire);
            let s2 = self.sequence.load(Ordering::Acquire);
            if s1 == s2 {
                return Some(data);
            }
        }
        None
    }

    fn from_raw(ptr: *mut u8) -> &'static MmapSnapshot {
        // SAFETY: callers guarantee the mapping is at least `FILE_SIZE` bytes
        // and page-aligned (memmap2 mappings are). The seqlock protocol
        // protects every access to `data`.
        unsafe { &*(ptr as *const MmapSnapshot) }
    }
}

/// Read-only file mapping of a shared consciousness snapshot.
#[cfg(not(target_arch = "wasm32"))]
pub struct MappedSnapshot {
    _file: std::fs::File,
    _mmap: memmap2::Mmap,
    shared: &'static MmapSnapshot,
}

#[cfg(not(target_arch = "wasm32"))]
impl MappedSnapshot {
    /// Map an existing snapshot file for reading.
    pub fn open(path: &std::path::Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("open snapshot file: {e}"))?;
        if file.metadata().map_err(|e| e.to_string())?.len() < MmapSnapshot::FILE_SIZE as u64 {
            return Err(format!(
                "snapshot file too small ({} bytes, need {})",
                file.metadata().map_err(|e| e.to_string())?.len(),
                MmapSnapshot::FILE_SIZE
            ));
        }
        let mmap = unsafe { memmap2::Mmap::map(&file) }
            .map_err(|e| format!("mmap snapshot file: {e}"))?;
        let shared = MmapSnapshot::from_raw(mmap.as_ptr() as *mut u8);
        Ok(Self {
            _file: file,
            _mmap: mmap,
            shared,
        })
    }

    /// Read the latest published snapshot.
    pub fn read(&self) -> Option<SnapshotData> {
        self.shared.read()
    }

    /// True once a writer has initialized the file.
    pub fn is_initialized(&self) -> bool {
        self.shared.header_valid()
    }
}

/// Writable file mapping of a shared consciousness snapshot (single writer).
#[cfg(not(target_arch = "wasm32"))]
pub struct MappedSnapshotWriter {
    _file: std::fs::File,
    _mmap: memmap2::MmapMut,
    shared: &'static MmapSnapshot,
}

#[cfg(not(target_arch = "wasm32"))]
impl MappedSnapshotWriter {
    /// Create (or truncate) the snapshot file and initialize its header.
    pub fn create(path: &std::path::Path) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|e| format!("create snapshot file: {e}"))?;
        file.set_len(MmapSnapshot::FILE_SIZE as u64)
            .map_err(|e| format!("size snapshot file: {e}"))?;
        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file) }
            .map_err(|e| format!("mmap snapshot file: {e}"))?;
        let raw = mmap.as_mut_ptr() as *mut MmapSnapshot;
        // SAFETY: fresh file, single writer; header is written once before
        // any reader can observe a valid magic.
        unsafe {
            (*raw).magic = SNAPSHOT_MAGIC;
            (*raw).version = SNAPSHOT_VERSION;
            (*raw).sequence.store(0, Ordering::Release);
        }
        let shared = MmapSnapshot::from_raw(mmap.as_mut_ptr());
        Ok(Self {
            _file: file,
            _mmap: mmap,
            shared,
        })
    }

    /// Open an existing snapshot file for writing.
    pub fn open(path: &std::path::Path) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("open snapshot file: {e}"))?;
        if file.metadata().map_err(|e| e.to_string())?.len() < MmapSnapshot::FILE_SIZE as u64 {
            return Err("snapshot file too small for a shared snapshot".to_string());
        }
        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file) }
            .map_err(|e| format!("mmap snapshot file: {e}"))?;
        let shared = MmapSnapshot::from_raw(mmap.as_mut_ptr());
        if !shared.header_valid() {
            return Err("snapshot file is not initialized (wrong magic/version)".to_string());
        }
        Ok(Self {
            _file: file,
            _mmap: mmap,
            shared,
        })
    }

    /// Publish a full `SnapshotData` and flush the mapping.
    pub fn write(&self, data: &SnapshotData) -> Result<(), String> {
        self.shared.write(data)?;
        let _ = self._mmap.flush();
        Ok(())
    }

    /// Begin an in-place write (mirrors `SharedSnapshot::begin_write`).
    pub fn begin_write(&self) -> Result<u64, String> {
        self.shared.begin_write()
    }

    /// End an in-place write.
    pub fn end_write(&self, expected: u64) {
        self.shared.end_write(expected);
    }

    /// Exclusive data access while the seqlock is held.
    ///
    /// # Safety
    /// Caller must hold the seqlock (`begin_write`, not yet `end_write`).
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn data_mut(&self) -> &mut SnapshotData {
        self.shared.data_mut()
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn mmap_snapshot_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot.bin");

        let writer = MappedSnapshotWriter::create(&path).unwrap();
        let mut data = SnapshotData::zeroed();
        data.cycle = 42;
        data.activations_count = 7;
        data.surprise_level = 0.25;
        writer.write(&data).unwrap();

        let reader = MappedSnapshot::open(&path).unwrap();
        assert!(reader.is_initialized());
        let read = reader.read().expect("published snapshot must be readable");
        assert_eq!(read.cycle, 42);
        assert_eq!(read.activations_count, 7);
        assert_eq!(read.surprise_level, 0.25);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn mmap_snapshot_uninitialized_file_reads_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("empty.bin");
        std::fs::write(&path, vec![0u8; MmapSnapshot::FILE_SIZE]).unwrap();

        let reader = MappedSnapshot::open(&path).unwrap();
        assert!(!reader.is_initialized());
        assert!(reader.read().is_none(), "uninitialized file must not yield data");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn mmap_snapshot_no_torn_reads_under_concurrency() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("live.bin");
        let writer = MappedSnapshotWriter::create(&path).unwrap();

        let writer_handle = std::thread::spawn(move || {
            for cycle in 1..=200u64 {
                let mut data = SnapshotData::zeroed();
                data.cycle = cycle;
                // Distinct marker so a torn read would be detectable:
                data.activations_count = (cycle * 7) as u32;
                writer.write(&data).unwrap();
            }
        });

        let reader = MappedSnapshot::open(&path).unwrap();
        let mut last_seen = 0u64;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while last_seen < 200 && std::time::Instant::now() < deadline {
            if let Some(data) = reader.read() {
                // Every successful read must be internally consistent:
                assert_eq!(
                    data.activations_count,
                    (data.cycle * 7) as u32,
                    "torn read observed: cycle={} count={}",
                    data.cycle,
                    data.activations_count
                );
                last_seen = last_seen.max(data.cycle);
            }
        }
        writer_handle.join().unwrap();
        assert_eq!(last_seen, 200, "reader must observe the final snapshot");
    }
}
