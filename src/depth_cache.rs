//! Per-depth mmap cache — instant recall from shared memory.
//!
//! After every recall, the top results for each depth level are written
//! to a dedicated mmap'd file. Next read = single mmap access = ~1 ns.
//!
//! Layout: microscope-cache.bin (4096 bytes, mmap'd)
//!
//!   [0-8]      Magic: "MSCPCACH" (0x4D53435043414348)
//!   [8-16]     Write sequence (u64, monotonic)
//!   [16-24]    Last query hash (u64)
//!   [24-32]    Timestamp ms (u64)
//!   [32-64]    Reserved
//!
//!   [64-4032]  9 × DepthSlot (440 bytes each)
//!              Each DepthSlot holds top-5 results for that depth
//!
//!   [4032-4096] Footer / checksum
//!
//! Each DepthSlot (440 bytes):
//!   [0-8]    depth_id (u8) + result_count (u8) + padding (6)
//!   [8-440]  5 × CachedResult (86 bytes each = 430) + 2 pad
//!
//! Each CachedResult (86 bytes):
//!   [0-4]    block_idx (u32)
//!   [4-8]    distance (f32)
//!   [8-12]   energy (f32)
//!   [12-13]  layer_id (u8)
//!   [13-14]  depth (u8)
//!   [14-16]  padding
//!   [16-80]  text preview (64 bytes UTF-8)
//!   [80-84]  x coordinate (f32)
//!   [84-86]  padding (2 bytes)
//!
//! Total per depth: 8 + 5*86 + 2 = 440 bytes
//! Total 9 depths: 9 * 440 = 3960 bytes
//! With header (64) + footer (64) = 4088 → fits in 4096

use memmap2::MmapMut;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

pub const CACHE_MAGIC: u64 = 0x4D53435043414348; // "MSCPCACH"
pub const CACHE_SIZE: usize = 4096;
pub const CACHE_FILE: &str = "microscope-cache.bin";

const HEADER_SIZE: usize = 64;
const DEPTH_COUNT: usize = 9;
const RESULTS_PER_DEPTH: usize = 5;
const TEXT_SIZE: usize = 64;

const RESULT_SIZE: usize = 86;   // 4+4+4+1+1+2+64+4+2
const DEPTH_SLOT_SIZE: usize = 440; // 8 + 5*86 + 2
const DEPTHS_OFFSET: usize = HEADER_SIZE; // 64

// ─── Cached result ──────────────────────────────────

#[derive(Clone, Debug)]
pub struct CachedResult {
    pub block_idx: u32,
    pub distance: f32,
    pub energy: f32,
    pub layer_id: u8,
    pub depth: u8,
    pub text: String,
    pub x: f32,
}

impl Default for CachedResult {
    fn default() -> Self {
        Self {
            block_idx: 0,
            distance: f32::MAX,
            energy: 0.0,
            layer_id: 0,
            depth: 0,
            text: String::new(),
            x: 0.0,
        }
    }
}

/// Per-depth cache with top-K results.
#[derive(Clone, Debug)]
pub struct DepthResults {
    pub depth: u8,
    pub results: Vec<CachedResult>,
}

/// Cache header info.
#[derive(Clone, Debug)]
pub struct CacheHeader {
    pub write_seq: u64,
    pub last_query_hash: u64,
    pub timestamp_ms: u64,
}

// ─── DepthCache ─────────────────────────────────────

/// Memory-mapped per-depth recall cache.
pub struct DepthCache {
    _file: File,
    mmap: MmapMut,
}

impl DepthCache {
    /// Open or create the cache file.
    pub fn open(dir: &Path) -> io::Result<Self> {
        let path = dir.join(CACHE_FILE);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let metadata = file.metadata()?;
        if metadata.len() < CACHE_SIZE as u64 {
            file.set_len(CACHE_SIZE as u64)?;
        }

        let mut mmap = unsafe { MmapMut::map_mut(&file)? };

        // Initialize if new
        if metadata.len() < CACHE_SIZE as u64 {
            mmap[0..8].copy_from_slice(&CACHE_MAGIC.to_le_bytes());
            mmap[8..16].copy_from_slice(&0u64.to_le_bytes());
            mmap.flush()?;
        }

        Ok(Self { _file: file, mmap })
    }

    /// Check if cache is valid.
    pub fn is_valid(&self) -> bool {
        let magic = u64::from_le_bytes(self.mmap[0..8].try_into().unwrap_or([0; 8]));
        magic == CACHE_MAGIC
    }

    /// Read cache header.
    pub fn header(&self) -> CacheHeader {
        CacheHeader {
            write_seq: self.read_u64(8),
            last_query_hash: self.read_u64(16),
            timestamp_ms: self.read_u64(24),
        }
    }

    /// Read cached results for a specific depth (0-8).
    /// This is the HOT PATH — single mmap read, no search.
    pub fn read_depth(&self, depth: u8) -> DepthResults {
        if depth > 8 {
            return DepthResults { depth, results: vec![] };
        }

        let slot_offset = DEPTHS_OFFSET + (depth as usize) * DEPTH_SLOT_SIZE;
        let result_count = self.mmap[slot_offset + 1] as usize;
        let count = result_count.min(RESULTS_PER_DEPTH);

        let mut results = Vec::with_capacity(count);
        for i in 0..count {
            let r_offset = slot_offset + 8 + i * RESULT_SIZE;
            results.push(self.read_result(r_offset));
        }

        DepthResults { depth, results }
    }

    /// Read all depths.
    pub fn read_all(&self) -> Vec<DepthResults> {
        (0..DEPTH_COUNT as u8)
            .map(|d| self.read_depth(d))
            .collect()
    }

    /// Write results for a specific depth after recall.
    pub fn write_depth(&mut self, depth: u8, results: &[CachedResult]) -> io::Result<()> {
        if depth > 8 {
            return Ok(());
        }

        let slot_offset = DEPTHS_OFFSET + (depth as usize) * DEPTH_SLOT_SIZE;

        // Write depth header
        self.mmap[slot_offset] = depth;
        self.mmap[slot_offset + 1] = results.len().min(RESULTS_PER_DEPTH) as u8;

        // Write results
        for (i, result) in results.iter().take(RESULTS_PER_DEPTH).enumerate() {
            let r_offset = slot_offset + 8 + i * RESULT_SIZE;
            self.write_result(r_offset, result);
        }

        Ok(())
    }

    /// Write results for ALL depths after a full recall, and update header.
    pub fn write_recall(
        &mut self,
        query_hash: u64,
        depth_results: &[(u8, Vec<CachedResult>)],
    ) -> io::Result<()> {
        // Update header
        let seq = self.read_u64(8);
        self.write_u64(8, seq.wrapping_add(1));
        self.write_u64(16, query_hash);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.write_u64(24, now);

        // Write each depth
        for (depth, results) in depth_results {
            self.write_depth(*depth, results)?;
        }

        self.mmap.flush()
    }

    // ─── Low-level read/write ───────────────────────

    fn read_u64(&self, offset: usize) -> u64 {
        u64::from_le_bytes(self.mmap[offset..offset + 8].try_into().unwrap_or([0; 8]))
    }

    fn write_u64(&mut self, offset: usize, val: u64) {
        self.mmap[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
    }

    fn read_u32(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.mmap[offset..offset + 4].try_into().unwrap_or([0; 4]))
    }

    fn read_f32(&self, offset: usize) -> f32 {
        f32::from_le_bytes(self.mmap[offset..offset + 4].try_into().unwrap_or([0; 4]))
    }

    fn read_result(&self, offset: usize) -> CachedResult {
        let block_idx = self.read_u32(offset);
        let distance = self.read_f32(offset + 4);
        let energy = self.read_f32(offset + 8);
        let layer_id = self.mmap[offset + 12];
        let depth = self.mmap[offset + 13];

        // Read text (64 bytes, null-terminated)
        let text_start = offset + 16;
        let text_end = text_start + TEXT_SIZE;
        let text_bytes = &self.mmap[text_start..text_end];
        let text_len = text_bytes.iter().position(|&b| b == 0).unwrap_or(TEXT_SIZE);
        let text = std::str::from_utf8(&text_bytes[..text_len])
            .unwrap_or("")
            .to_string();

        let x = self.read_f32(offset + 80);

        CachedResult {
            block_idx,
            distance,
            energy,
            layer_id,
            depth,
            text,
            x,
        }
    }

    fn write_result(&mut self, offset: usize, result: &CachedResult) {
        self.mmap[offset..offset + 4].copy_from_slice(&result.block_idx.to_le_bytes());
        self.mmap[offset + 4..offset + 8].copy_from_slice(&result.distance.to_le_bytes());
        self.mmap[offset + 8..offset + 12].copy_from_slice(&result.energy.to_le_bytes());
        self.mmap[offset + 12] = result.layer_id;
        self.mmap[offset + 13] = result.depth;
        self.mmap[offset + 14] = 0;
        self.mmap[offset + 15] = 0;

        // Write text (truncate to 64 bytes, UTF-8 safe)
        let text_bytes = result.text.as_bytes();
        let mut end = text_bytes.len().min(TEXT_SIZE);
        while end > 0 && !result.text.is_char_boundary(end) {
            end -= 1;
        }
        let text_start = offset + 16;
        self.mmap[text_start..text_start + TEXT_SIZE].fill(0);
        self.mmap[text_start..text_start + end].copy_from_slice(&text_bytes[..end]);

        self.mmap[offset + 80..offset + 84].copy_from_slice(&result.x.to_le_bytes());
        self.mmap[offset + 84] = 0;
        self.mmap[offset + 85] = 0;
    }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cache_sizes() {
        assert_eq!(RESULT_SIZE, 86);
        assert_eq!(DEPTH_SLOT_SIZE, 440);
        assert_eq!(DEPTHS_OFFSET + DEPTH_COUNT * DEPTH_SLOT_SIZE, 64 + 3960);
        assert!(DEPTHS_OFFSET + DEPTH_COUNT * DEPTH_SLOT_SIZE <= CACHE_SIZE);
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = std::env::temp_dir().join("microscope_depth_cache_test");
        let _ = fs::create_dir_all(&dir);

        {
            let mut cache = DepthCache::open(&dir).unwrap();
            assert!(cache.is_valid());

            let results = vec![
                CachedResult {
                    block_idx: 42,
                    distance: 0.001,
                    energy: 0.95,
                    layer_id: 1,
                    depth: 3,
                    text: "hello world".to_string(),
                    x: 0.25,
                },
            ];

            cache.write_recall(12345, &[(3, results.clone())]).unwrap();

            let read = cache.read_depth(3);
            assert_eq!(read.results.len(), 1);
            assert_eq!(read.results[0].block_idx, 42);
            assert!((read.results[0].distance - 0.001).abs() < 0.0001);
            assert_eq!(read.results[0].text, "hello world");

            let header = cache.header();
            assert_eq!(header.write_seq, 1);
            assert_eq!(header.last_query_hash, 12345);
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_empty_depth() {
        let dir = std::env::temp_dir().join("microscope_depth_empty_test");
        let _ = fs::create_dir_all(&dir);

        {
            let cache = DepthCache::open(&dir).unwrap();
            let read = cache.read_depth(0);
            assert_eq!(read.results.len(), 0);
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
