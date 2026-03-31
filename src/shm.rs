//! Shared Memory integration — zero-copy IPC with external processes.
//!
//! Uses a 2048-byte mmap'd binary region for lock-free inter-process
//! communication. Any process can attach and read/write cognitive state
//! with zero latency.
//!
//! Layout:
//!   [464-512]   MicroscopeSlot (48 bytes) — cognitive state
//!   [1536-2048] MicroscopeRing (512 bytes) — recall result ring
//!
//! Usage:
//!   microscope-mem shm-write   # write current state to SHM
//!   microscope-mem shm-read    # read SHM state
//!   microscope-mem shm-daemon  # continuous sync loop

use memmap2::MmapMut;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

/// Default SHM file path
pub const DEFAULT_SHM_PATH: &str = "microscope-shm.bin";

/// SHM total size
const SHM_SIZE: usize = 2048;

/// Magic number for validation
const SHM_MAGIC: u64 = 0x4D53435053484D30; // "MSCPSHM0"

/// Microscope slot offset (48 bytes, offset 464)
const MICROSCOPE_OFFSET: usize = 464;

/// Microscope slot size
const MICROSCOPE_SLOT_SIZE: usize = 48;

/// Microscope ring buffer offset (512 bytes, offset 1536)
const RING_OFFSET: usize = 1536;

/// Ring buffer data size
const RING_SIZE: usize = 512;

/// Ring entry size (64 bytes: 4 idx + 4 layer + 4 depth + 4 energy + 48 text)
const RING_ENTRY_SIZE: usize = 64;

/// Ring capacity
const RING_CAPACITY: usize = RING_SIZE / RING_ENTRY_SIZE; // 8 entries

// ─── MicroscopeSlot (48 bytes) ──────────────────────

/// Microscope cognitive state in shared memory.
/// Written after every recall/store/insights operation.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MicroscopeSlot {
    /// Last query hash
    pub last_query_hash: u64,           // 8 bytes  [464-472]
    /// Last recall best block index
    pub last_recall_block: u32,         // 4 bytes  [472-476]
    /// Active archetype ID
    pub active_archetype_id: u32,       // 4 bytes  [476-480]
    /// Attention weights (top 4 layers)
    pub attention_weights: [f32; 4],    // 16 bytes [480-496]
    /// Emotional valence from last recall
    pub emotional_valence: f32,         // 4 bytes  [496-500]
    /// Total blocks in index
    pub total_blocks: u32,              // 4 bytes  [500-504]
    /// Write sequence (monotonic counter)
    pub write_seq: u32,                 // 4 bytes  [504-508]
    /// Timestamp of last update
    pub timestamp_ms: u32,              // 4 bytes  [508-512]
}

impl Default for MicroscopeSlot {
    fn default() -> Self {
        Self {
            last_query_hash: 0,
            last_recall_block: 0,
            active_archetype_id: 0,
            attention_weights: [0.0; 4],
            emotional_valence: 0.0,
            total_blocks: 0,
            write_seq: 0,
            timestamp_ms: 0,
        }
    }
}

/// A ring buffer entry for recall results (64 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct RingEntry {
    pub block_idx: u32,     // 4
    pub layer_id: u8,       // 1
    pub depth: u8,          // 1
    pub _pad: u16,          // 2
    pub distance: f32,      // 4
    pub energy: f32,        // 4
    pub text: [u8; 48],     // 48
}

impl Default for RingEntry {
    fn default() -> Self {
        Self {
            block_idx: 0,
            layer_id: 0,
            depth: 0,
            _pad: 0,
            distance: 0.0,
            energy: 0.0,
            text: [0; 48],
        }
    }
}

// ─── ShmBridge ──────────────────────────────────────

/// Bridge between Microscope Memory and SHM.
pub struct ShmBridge {
    _file: File,
    mmap: MmapMut,
}

impl ShmBridge {
    /// Open or create the shared memory file.
    pub fn open(path: &str) -> io::Result<Self> {
        let shm_path = Path::new(path);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(shm_path)?;

        let metadata = file.metadata()?;
        if metadata.len() < SHM_SIZE as u64 {
            file.set_len(SHM_SIZE as u64)?;
        }

        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(Self { _file: file, mmap })
    }

    /// Open with default path.
    pub fn open_default() -> io::Result<Self> {
        Self::open(DEFAULT_SHM_PATH)
    }

    /// Check if this is a valid SHM region.
    pub fn is_valid(&self) -> bool {
        if self.mmap.len() < SHM_SIZE {
            return false;
        }
        let magic = u64::from_le_bytes(self.mmap[0..8].try_into().unwrap_or([0; 8]));
        magic == SHM_MAGIC
    }

    // ─── Read ───────────────────────────────────────

    /// Read the Microscope slot from SHM.
    pub fn read_slot(&self) -> MicroscopeSlot {
        if self.mmap.len() < MICROSCOPE_OFFSET + MICROSCOPE_SLOT_SIZE {
            return MicroscopeSlot::default();
        }
        let bytes = &self.mmap[MICROSCOPE_OFFSET..MICROSCOPE_OFFSET + MICROSCOPE_SLOT_SIZE];
        unsafe { *(bytes.as_ptr() as *const MicroscopeSlot) }
    }

    /// Read a ring entry by index.
    pub fn read_ring_entry(&self, index: usize) -> Option<RingEntry> {
        if index >= RING_CAPACITY {
            return None;
        }
        let offset = RING_OFFSET + index * RING_ENTRY_SIZE;
        if self.mmap.len() < offset + RING_ENTRY_SIZE {
            return None;
        }
        let bytes = &self.mmap[offset..offset + RING_ENTRY_SIZE];
        Some(unsafe { *(bytes.as_ptr() as *const RingEntry) })
    }

    /// Read all non-empty ring entries.
    pub fn read_ring(&self) -> Vec<RingEntry> {
        (0..RING_CAPACITY)
            .filter_map(|i| {
                let entry = self.read_ring_entry(i)?;
                if entry.block_idx > 0 || entry.text[0] != 0 {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect()
    }

    // ─── Write ──────────────────────────────────────

    /// Write the Microscope slot to SHM.
    pub fn write_slot(&mut self, slot: &MicroscopeSlot) -> io::Result<()> {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                slot as *const MicroscopeSlot as *const u8,
                MICROSCOPE_SLOT_SIZE,
            )
        };
        self.mmap[MICROSCOPE_OFFSET..MICROSCOPE_OFFSET + MICROSCOPE_SLOT_SIZE]
            .copy_from_slice(bytes);

        // Increment write_seq to notify readers
        let seq_offset = 16;
        let seq = u64::from_le_bytes(
            self.mmap[seq_offset..seq_offset + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );
        self.mmap[seq_offset..seq_offset + 8]
            .copy_from_slice(&seq.wrapping_add(1).to_le_bytes());

        self.mmap.flush()
    }

    /// Push a recall result to the ring buffer.
    pub fn push_ring_entry(&mut self, entry: &RingEntry) -> io::Result<()> {
        // Simple rotating write — find next slot by write_seq
        let slot = self.read_slot();
        let index = slot.write_seq as usize % RING_CAPACITY;
        let offset = RING_OFFSET + index * RING_ENTRY_SIZE;

        let bytes = unsafe {
            std::slice::from_raw_parts(
                entry as *const RingEntry as *const u8,
                RING_ENTRY_SIZE,
            )
        };
        self.mmap[offset..offset + RING_ENTRY_SIZE].copy_from_slice(bytes);
        self.mmap.flush()
    }

    /// Write a full cognitive state update.
    pub fn update_cognitive_state(
        &mut self,
        query_hash: u64,
        best_block: u32,
        archetype_id: u32,
        attention: &[f32],
        valence: f32,
        total_blocks: u32,
    ) -> io::Result<()> {
        let mut slot = self.read_slot();
        slot.last_query_hash = query_hash;
        slot.last_recall_block = best_block;
        slot.active_archetype_id = archetype_id;

        // Copy up to 4 attention weights
        for i in 0..4.min(attention.len()) {
            slot.attention_weights[i] = attention[i];
        }

        slot.emotional_valence = valence;
        slot.total_blocks = total_blocks;
        slot.write_seq = slot.write_seq.wrapping_add(1);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u32;
        slot.timestamp_ms = now;

        self.write_slot(&slot)
    }

    /// Push a recall result text to the ring.
    pub fn push_recall_result(
        &mut self,
        block_idx: u32,
        layer_id: u8,
        depth: u8,
        distance: f32,
        energy: f32,
        text: &str,
    ) -> io::Result<()> {
        let mut entry = RingEntry::default();
        entry.block_idx = block_idx;
        entry.layer_id = layer_id;
        entry.depth = depth;
        entry.distance = distance;
        entry.energy = energy;

        // Copy text (truncate to 48 bytes, UTF-8 safe)
        let text_bytes = text.as_bytes();
        let copy_len = text_bytes.len().min(48);
        // Find UTF-8 boundary
        let mut end = copy_len;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        entry.text[..end].copy_from_slice(&text_bytes[..end]);

        self.push_ring_entry(&entry)
    }

    // ─── Status ─────────────────────────────────────

    /// Get SHM status for display.
    pub fn status(&self) -> ShmStatus {
        let valid = self.is_valid();
        let slot = self.read_slot();
        let ring_entries = self.read_ring().len();

        let ora_seq = if valid {
            u64::from_le_bytes(self.mmap[16..24].try_into().unwrap_or([0; 8]))
        } else {
            0
        };

        ShmStatus {
            valid,
            ora_write_seq: ora_seq,
            microscope_slot: slot,
            ring_entries,
        }
    }
}

/// SHM status report.
#[derive(Debug)]
pub struct ShmStatus {
    pub valid: bool,
    pub ora_write_seq: u64,
    pub microscope_slot: MicroscopeSlot,
    pub ring_entries: usize,
}

// ─── Helper: make RingEntry text readable ───────────

impl RingEntry {
    pub fn text_str(&self) -> &str {
        let end = self.text.iter().position(|&b| b == 0).unwrap_or(48);
        std::str::from_utf8(&self.text[..end]).unwrap_or("<bin>")
    }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_size() {
        assert_eq!(std::mem::size_of::<MicroscopeSlot>(), MICROSCOPE_SLOT_SIZE);
    }

    #[test]
    fn test_ring_entry_size() {
        assert_eq!(std::mem::size_of::<RingEntry>(), RING_ENTRY_SIZE);
    }

    #[test]
    fn test_ring_capacity() {
        assert_eq!(RING_CAPACITY, 8);
    }

    #[test]
    fn test_default_slot() {
        let slot = MicroscopeSlot::default();
        assert_eq!(slot.last_query_hash, 0);
        assert_eq!(slot.write_seq, 0);
    }

    #[test]
    fn test_ring_entry_text() {
        let mut entry = RingEntry::default();
        entry.text[0] = b'H';
        entry.text[1] = b'i';
        assert_eq!(entry.text_str(), "Hi");
    }

    #[test]
    fn test_offsets_no_overlap() {
        // MicroscopeSlot: 464-512
        // RingBuffer: 1536-2048
        assert!(MICROSCOPE_OFFSET + MICROSCOPE_SLOT_SIZE <= 512);
        assert!(RING_OFFSET + RING_SIZE <= SHM_SIZE);
        assert!(MICROSCOPE_OFFSET + MICROSCOPE_SLOT_SIZE <= RING_OFFSET);
    }
}
