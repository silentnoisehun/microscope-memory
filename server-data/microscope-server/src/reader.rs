//! MicroscopeReader — high-performance memory-mapped reader for the binary index.

use colored::Colorize;
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::{
    auto_depth, content_coords_blended, layer_to_id, safe_truncate, BLOCK_DATA_SIZE,
    DEPTH_ENTRY_SIZE, HEADER_SIZE, LAYER_NAMES, META_HEADER_SIZE,
};

#[cfg(feature = "stealth")]
use crate::syscaller::nt_query_virtual_memory;

#[cfg(windows)]
use windows_sys::Win32::System::Memory::{MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS};

#[cfg(windows)]
#[cfg(not(feature = "stealth"))]
use windows_sys::Win32::System::Memory::VirtualQuery;

#[cfg(feature = "stealth")]
#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

/// Block header: 32 bytes, packed, mmap-ready.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct BlockHeader {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zoom: f32,
    pub depth: u8,
    pub layer_id: u8,
    pub data_offset: u32,
    pub data_len: u16,
    pub parent_idx: u32,
    pub child_count: u16,
    pub crc16: [u8; 2],
}

// Meta header: 48 bytes at start of meta.bin
#[repr(C, packed)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct MetaHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub block_count: u32,
    pub depth_count: u32,
}

pub fn layer_color(id: u8) -> &'static str {
    match id {
        0 => "white",
        1 => "blue",
        2 => "cyan",
        3 => "green",
        4 => "red",
        5 => "yellow",
        6 => "magenta",
        7 => "orange",
        8 => "lime",
        9 => "purple",
        _ => "white",
    }
}

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[inline(always)]
fn l2_dist_sq_simd(h: &BlockHeader, x: f32, y: f32, z: f32, qz: f32, zw: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let h_vals = _mm_loadu_ps(h as *const BlockHeader as *const f32);
        let q_vals = _mm_set_ps(qz, z, y, x);
        let diff = _mm_sub_ps(h_vals, q_vals);
        let weights = _mm_set_ps(zw, 1.0, 1.0, 1.0);
        let weighted_diff = _mm_mul_ps(diff, weights);
        let sq = _mm_mul_ps(weighted_diff, weighted_diff);
        let res = _mm_hadd_ps(sq, sq);
        let res2 = _mm_hadd_ps(res, res);
        let mut dist = 0.0f32;
        _mm_store_ss(&mut dist, res2);
        dist
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let dx = h.x - x;
        let dy = h.y - y;
        let dz = h.z - z;
        let dw = (h.zoom - qz) * zw;
        dx * dx + dy * dy + dz * dz + dw * dw
    }
}

/// Backing store for block data — either memory-mapped or decompressed in-memory.
pub enum DataStore {
    /// Normal mmap path (uncompressed data.bin)
    Mmap(memmap2::Mmap),
    /// Decompressed data held in memory (from data.bin.zst)
    #[cfg(feature = "compression")]
    InMemory(Vec<u8>),
}

impl std::ops::Deref for DataStore {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            DataStore::Mmap(m) => m,
            #[cfg(feature = "compression")]
            DataStore::InMemory(v) => v,
        }
    }
}

/// High-performance memory-mapped reader for the Microscope index.
pub struct MicroscopeReader {
    pub headers: memmap2::Mmap,
    pub data: DataStore,
    pub block_count: usize,
    pub depth_ranges: [(u32, u32); 9],
    pub ghost_mode: bool,
}

impl MicroscopeReader {
    pub fn open(config: &Config) -> Result<Self, String> {
        let output_dir = Path::new(&config.paths.output_dir);
        let meta_path = output_dir.join("meta.bin");
        let hdr_path = output_dir.join("microscope.bin");
        let dat_path = output_dir.join("data.bin");

        let meta = fs::read(&meta_path)
            .map_err(|e| format!("open meta.bin — run 'build' first: {}", e))?;
        if meta.len() < 12 {
            return Err("meta.bin too small".to_string());
        }
        let magic = &meta[0..4];
        if magic != b"MSCM" && magic != b"MSC2" && magic != b"MSC3" {
            return Err("invalid magic: expected MSCM, MSC2 or MSC3".to_string());
        }
        let block_count = u32::from_le_bytes(
            meta[8..12]
                .try_into()
                .map_err(|_| "meta.bin: bad block_count bytes")?,
        ) as usize;
        let mut depth_ranges = [(0u32, 0u32); 9];
        for (d, range) in depth_ranges.iter_mut().enumerate() {
            let off = META_HEADER_SIZE + d * DEPTH_ENTRY_SIZE;
            if off + 8 > meta.len() {
                return Err(format!("meta.bin truncated at depth {}", d));
            }
            let start = u32::from_le_bytes(
                meta[off..off + 4]
                    .try_into()
                    .map_err(|_| "meta.bin: bad depth range bytes")?,
            );
            let count = u32::from_le_bytes(
                meta[off + 4..off + 8]
                    .try_into()
                    .map_err(|_| "meta.bin: bad depth range bytes")?,
            );
            *range = (start, count);
        }

        let hdr_file =
            fs::File::open(&hdr_path).map_err(|e| format!("open microscope.bin: {}", e))?;
        // Safety: microscope.bin is read-only and will remain valid for the lifetime of MicroscopeReader
        let headers =
            unsafe { memmap2::Mmap::map(&hdr_file).map_err(|e| format!("mmap headers: {}", e))? };

        // Red Audit: Stability check for headers mmap
        #[cfg(windows)]
        if let Err(e) = Self::verify_mmap_protection(headers.as_ptr(), headers.len()) {
            return Err(format!("Stability check failed (headers): {}", e));
        }

        #[cfg(feature = "compression")]
        let data = {
            let zst_path = output_dir.join("data.bin.zst");
            if zst_path.exists()
                && (!dat_path.exists()
                    || fs::metadata(&zst_path)
                        .and_then(|zm| {
                            fs::metadata(&dat_path).map(|dm| {
                                zm.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                                    > dm.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                            })
                        })
                        .unwrap_or(false))
            {
                let compressed =
                    fs::read(&zst_path).map_err(|e| format!("read data.bin.zst: {}", e))?;
                let decompressed = zstd::decode_all(std::io::Cursor::new(&compressed))
                    .map_err(|e| format!("zstd decompress: {}", e))?;
                DataStore::InMemory(decompressed)
            } else {
                let dat_file =
                    fs::File::open(&dat_path).map_err(|e| format!("open data.bin: {}", e))?;
                // Safety: data.bin is read-only and will remain valid for the lifetime of MicroscopeReader
                DataStore::Mmap(unsafe {
                    memmap2::Mmap::map(&dat_file).map_err(|e| format!("mmap data.bin: {}", e))?
                })
            }
        };

        #[cfg(not(feature = "compression"))]
        let data = {
            let dat_file =
                fs::File::open(&dat_path).map_err(|e| format!("open data.bin: {}", e))?;
            // Safety: data.bin is read-only and will remain valid for the lifetime of MicroscopeReader
            DataStore::Mmap(unsafe {
                memmap2::Mmap::map(&dat_file).map_err(|e| format!("mmap data.bin: {}", e))?
            })
        };

        Ok(MicroscopeReader {
            headers,
            data,
            block_count,
            depth_ranges,
            #[cfg(feature = "stealth")]
            ghost_mode: crate::antidebug::is_sandbox(),
            #[cfg(not(feature = "stealth"))]
            ghost_mode: false,
        })
    }

    /// Red Audit: Verifies that the mmap'ed memory is indeed readable and not guarded.
    #[cfg(windows)]
    fn verify_mmap_protection(ptr: *const u8, _len: usize) -> Result<(), String> {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        let mut _return_len: usize = 0;

        #[cfg(feature = "stealth")]
        let status = unsafe {
            nt_query_virtual_memory(
                -1isize as HANDLE, // Current process
                ptr as *const _,
                0, // MemoryBasicInformation
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                &mut _return_len,
            )
        };
        #[cfg(feature = "stealth")]
        if status != 0 {
            return Err(format!(
                "NtQueryVirtualMemory failed with status 0x{:08X}",
                status
            ));
        }

        #[cfg(not(feature = "stealth"))]
        {
            let res = unsafe {
                VirtualQuery(
                    ptr as *const _,
                    &mut info as *mut _ as *mut _,
                    std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            };
            if res == 0 {
                return Err("VirtualQuery failed".to_string());
            }
        }

        if info.Protect == PAGE_NOACCESS || (info.Protect & PAGE_GUARD) != 0 {
            return Err("Memory protection violation: Page is NOACCESS or GUARD".to_string());
        }

        Ok(())
    }

    #[inline(always)]
    pub fn header(&self, i: usize) -> &BlockHeader {
        debug_assert!(i < self.block_count);
        unsafe { &*(self.headers.as_ptr().add(i * HEADER_SIZE) as *const BlockHeader) }
    }

    #[inline(always)]
    pub fn text(&self, i: usize) -> &str {
        if self.ghost_mode {
            // Red Audit Phase 3: Ghost Mode protection.
            // In highly certain sandbox, we could mask data here.
        }
        let h = self.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;

        // Red Audit: Basic bounds and null-check sanitization
        if end > self.data.len() || start >= end {
            return "[out of bounds]";
        }

        let raw = &self.data[start..end];

        // Anti-Analysis: Ensure no suspicious control characters
        std::str::from_utf8(raw).unwrap_or("<bin>")
    }

    /// The MICROSCOPE: exact depth + spatial L2 search.
    pub fn look(
        &self,
        config: &Config,
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        k: usize,
    ) -> Vec<(f32, usize, bool)> {
        let (start, count) = self.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);

        let mut results: Vec<(f32, usize, bool)> = Vec::with_capacity(count + 10);
        if count > 0 {
            for i in start..(start + count) {
                let h = self.header(i);
                let dx = h.x - x;
                let dy = h.y - y;
                let dz = h.z - z;
                results.push((dx * dx + dy * dy + dz * dz, i, true));
            }
        }

        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            if entry.depth != zoom {
                continue;
            }
            let dx = entry.x - x;
            let dy = entry.y - y;
            let dz = entry.z - z;
            results.push((dx * dx + dy * dy + dz * dz, ai + 1_000_000, false));
        }

        let k = k.min(results.len());
        if k == 0 {
            return vec![];
        }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    /// 4D soft zoom search with SIMD.
    #[allow(clippy::too_many_arguments)]
    pub fn look_soft(
        &self,
        config: &Config,
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        k: usize,
        zw: f32,
    ) -> Vec<(f32, usize, bool)> {
        let qz = zoom as f32 / 8.0;
        let mut results: Vec<(f32, usize, bool)> = (0..self.block_count)
            .into_par_iter()
            .map(|i| {
                let h = self.header(i);
                (l2_dist_sq_simd(h, x, y, z, qz, zw), i, true)
            })
            .collect();

        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            let dx = entry.x - x;
            let dy = entry.y - y;
            let dz = entry.z - z;
            let entry_zoom = entry.depth as f32 / 8.0;
            let dw = (entry_zoom - qz) * zw;
            results.push((dx * dx + dy * dy + dz * dz + dw * dw, ai + 1_000_000, false));
        }

        let k = k.min(results.len());
        if k == 0 {
            return vec![];
        }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    /// Radial search: find all blocks within `radius` of (x, y, z) at a specific depth.
    /// Returns a ResultSet with the closest match as primary and neighbors distance-weighted.
    #[allow(clippy::too_many_arguments)]
    pub fn radial_search(
        &self,
        config: &Config,
        x: f32,
        y: f32,
        z: f32,
        depth: u8,
        radius: f32,
        k: usize,
    ) -> ResultSet {
        let radius_sq = radius * radius;
        let (start, count) = self.depth_ranges[depth as usize];
        let (start, count) = (start as usize, count as usize);

        // SIMD-accelerated radial scan within depth band
        let mut candidates: Vec<(f32, usize, bool)> = if count > 0 {
            (start..(start + count))
                .into_par_iter()
                .filter_map(|i| {
                    let h = self.header(i);
                    let qz = depth as f32 / 8.0;
                    let dist_sq = l2_dist_sq_simd(h, x, y, z, qz, 0.0); // no zoom weight for radial
                    if dist_sq <= radius_sq {
                        Some((dist_sq, i, true))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        // Include append log entries at the same depth
        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            if entry.depth != depth {
                continue;
            }
            let dx = entry.x - x;
            let dy = entry.y - y;
            let dz = entry.z - z;
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq <= radius_sq {
                candidates.push((dist_sq, ai + 1_000_000, false));
            }
        }

        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Build ResultSet
        let primary = candidates
            .first()
            .map(|&(dist, idx, is_main)| RadialResult {
                block_idx: idx,
                dist_sq: dist,
                weight: 1.0,
                is_main,
            });

        let neighbors: Vec<RadialResult> = candidates
            .iter()
            .skip(1)
            .take(k.saturating_sub(1))
            .map(|&(dist_sq, idx, is_main)| {
                // Weight: inverse distance, normalized so closest neighbor = 1.0
                let weight = if dist_sq > 0.0001 {
                    (radius_sq - dist_sq) / radius_sq
                } else {
                    1.0
                };
                RadialResult {
                    block_idx: idx,
                    dist_sq,
                    weight,
                    is_main,
                }
            })
            .collect();

        let total_within_radius = candidates.len();

        ResultSet {
            primary,
            neighbors,
            center: (x, y, z),
            depth,
            radius,
            total_within_radius,
        }
    }

    /// Text search
    pub fn find_text(&self, query: &str, k: usize) -> Vec<(u8, usize)> {
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize)> = (0..self.block_count)
            .into_par_iter()
            .filter_map(|i| {
                if self.text(i).to_lowercase().contains(&q) {
                    Some((self.header(i).depth, i))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by_key(|&(d, _)| d);
        results.truncate(k);
        results
    }

    pub fn print_result(&self, i: usize, dist: f32) {
        let h = self.header(i);
        let text = self.text(i);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        let preview: String = text.chars().take(70).filter(|&c| c != '\n').collect();
        println!(
            "  {} {} {} {}",
            format!("D{}", h.depth).cyan(),
            format!("L2={:.5}", dist).yellow(),
            format!("[{}/{}]", layer, layer_color(h.layer_id)).green(),
            preview
        );
    }
}

// ─── 21D Emotion Vector ─────────────────────────────
/// The 21 named dimensions of the emotion vector.
pub const EMOTION_DIMS: &[&str] = &[
    "joy", "sadness", "anger", "fear", "surprise",
    "disgust", "trust", "anticipation", "love", "gratitude",
    "curiosity", "awe", "confusion", "anxiety", "serenity",
    "hope", "pride", "shame", "guilt", "empathy",
    "excitement",
];
/// Size of the emotion vector in bytes (21 × f32 = 84)
pub const EMOTION_VECTOR_SIZE: usize = 21 * 4;

// ─── APPEND LOG ──────────────────────────────────────

#[allow(dead_code)]
pub struct AppendEntry {
    pub text: String,
    pub layer_id: u8,
    pub importance: u8,
    pub depth: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// 21D emotion vector (zero-initialized if not stored)
    pub emotion: [f32; 21],
}

impl Default for AppendEntry {
    fn default() -> Self {
        Self {
            text: String::new(),
            layer_id: 0,
            importance: 5,
            depth: 4,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            emotion: [0.0; 21],
        }
    }
}

pub fn read_append_log(path: &Path) -> Vec<AppendEntry> {
    if !path.exists() {
        return vec![];
    }
    let data = fs::read(path).unwrap_or_default();
    if data.is_empty() {
        return vec![];
    }

    let mut entries = Vec::new();
    let mut pos = 0;

    let is_v2 = data.len() >= 4 && &data[0..2] == b"AP";
    let has_emotion = data.len() >= 4 && &data[0..4] == b"APv3";

    if is_v2 {
        pos = 4;
    }

    // APv1: 18 bytes (no depth field), APv2: 19 bytes, APv3: 103 bytes (with emotion)
    let header_size = if has_emotion {
        103
    } else if is_v2 {
        19
    } else {
        18
    };

    while pos + header_size <= data.len() {
        let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let lid = data[pos + 4];
        let imp = data[pos + 5];

        let (depth, coords_start) = if is_v2 {
            (data[pos + 6], pos + 7)
        } else {
            (4u8, pos + 6)
        };

        let x = f32::from_le_bytes(data[coords_start..coords_start + 4].try_into().unwrap());
        let y = f32::from_le_bytes(data[coords_start + 4..coords_start + 8].try_into().unwrap());
        let z = f32::from_le_bytes(
            data[coords_start + 8..coords_start + 12]
                .try_into()
                .unwrap(),
        );

        // Read 21D emotion vector (v3 only, zero for older formats)
        let mut emotion = [0.0f32; 21];
        if has_emotion {
            let emo_start = pos + 19;
            for i in 0..21 {
                let off = emo_start + i * 4;
                emotion[i] = f32::from_le_bytes(
                    data[off..off + 4].try_into().unwrap_or([0u8; 4]),
                );
            }
        }

        pos += header_size;
        if pos + len > data.len() {
            break;
        }
        let text = String::from_utf8_lossy(&data[pos..pos + len]).to_string();
        pos += len;
        entries.push(AppendEntry {
            text,
            layer_id: lid,
            importance: imp,
            depth,
            x,
            y,
            z,
            emotion,
        });
    }
    entries
}

/// Display a single append-log result entry.
pub fn print_append_result(appended: &[AppendEntry], idx: usize, dist: f32) {
    let ai = idx - 1_000_000;
    if ai < appended.len() {
        let e = &appended[ai];
        let layer = LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
        println!(
            "  {} {} {} {}",
            format!("D{}", e.depth).cyan(),
            format!("L2={:.5}", dist).yellow(),
            format!("[{}/new]", layer).green(),
            safe_truncate(&e.text, 70)
        );
    }
}

// ─── EMOTIONS.BIN (parallel fixed-record file) ───────

/// Path to the emotions.bin file relative to output_dir.
pub fn emotions_path(output_dir: &Path) -> PathBuf {
    output_dir.join("emotions.bin")
}

/// Read the entire emotions.bin file into a Vec of [f32; 21].
/// Returns an empty vec if the file doesn't exist.
/// Each record is EMOTION_VECTOR_SIZE (84) bytes.
pub fn read_emotions(path: &Path) -> Vec<[f32; 21]> {
    if !path.exists() {
        return vec![];
    }
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    let record_size = EMOTION_VECTOR_SIZE;
    let count = data.len() / record_size;
    let mut emotions = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * record_size;
        let mut emo = [0.0f32; 21];
        for j in 0..21 {
            let boff = off + j * 4;
            if boff + 4 <= data.len() {
                emo[j] = f32::from_le_bytes(
                    data[boff..boff + 4].try_into().unwrap_or([0u8; 4]),
                );
            }
        }
        emotions.push(emo);
    }
    emotions
}

/// Write a single emotion vector to emotions.bin at `block_idx` position.
/// Creates or grows the file as needed. Zero-fills any gap.
pub fn write_emotion(path: &Path, block_idx: usize, emotion: &[f32; 21]) -> Result<(), String> {
    let record_size = EMOTION_VECTOR_SIZE;
    let needed_size = (block_idx + 1) * record_size;

    let mut data = if path.exists() {
        fs::read(path).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Extend file with zeros if needed
    if data.len() < needed_size {
        data.resize(needed_size, 0u8);
    }

    let off = block_idx * record_size;
    for j in 0..21 {
        let boff = off + j * 4;
        let bytes = emotion[j].to_le_bytes();
        data[boff..boff + 4].copy_from_slice(&bytes);
    }

    fs::write(path, &data).map_err(|e| format!("write emotions.bin: {}", e))?;
    Ok(())
}

/// Format an emotion vector as a human-readable string showing the top-K dimensions.
pub fn format_emotion(emotion: &[f32; 21], top_k: usize) -> String {
    let mut pairs: Vec<(usize, &f32)> = emotion.iter().enumerate().collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    let top: Vec<String> = pairs
        .iter()
        .take(top_k.min(21))
        .filter(|(_, &v)| v > 0.05)
        .map(|(i, v)| {
            let name = EMOTION_DIMS.get(*i).unwrap_or(&"?");
            format!("{}:{:.2}", name, v)
        })
        .collect();
    if top.is_empty() {
        "neutral".to_string()
    } else {
        top.join(" ")
    }
}

/// Cosine similarity between two 21D emotion vectors.
/// Returns 0.0 if either vector is zero (neutral).
/// Range: 0.0 (unrelated) to 1.0 (identical direction).
pub fn emotional_similarity(a: &[f32; 21], b: &[f32; 21]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a < 1e-8 || mag_b < 1e-8 {
        return 0.0;
    }
    (dot / (mag_a * mag_b)).max(0.0) // clamp negative to 0
}

/// Load emotions.bin and return a slice-lookup function:
/// `|block_idx| -> Option<[f32; 21]>` that returns the emotion vector for a given block index.
/// Returns None if the file doesn't exist or is empty.
pub fn load_emotion_lookup(output_dir: &Path) -> Option<Box<dyn Fn(usize) -> Option<[f32; 21]>>> {
    let path = output_dir.join("emotions.bin");
    let data = fs::read(&path).ok()?;
    let record_size = EMOTION_VECTOR_SIZE;
    let count = data.len() / record_size;
    if count == 0 {
        return None;
    }
    Some(Box::new(move |block_idx: usize| {
        if block_idx >= count {
            return None;
        }
        let off = block_idx * record_size;
        let mut emo = [0.0f32; 21];
        for j in 0..21 {
            let boff = off + j * 4;
            if boff + 4 <= data.len() {
                emo[j] = f32::from_le_bytes(data[boff..boff + 4].try_into().unwrap_or([0u8; 4]));
            }
        }
        Some(emo)
    }))
}

// ─── RADIAL SEARCH TYPES ─────────────────────────────

/// A single result from radial search.
#[derive(Debug, Clone)]
pub struct RadialResult {
    pub block_idx: usize,
    pub dist_sq: f32,
    pub weight: f32, // 1.0 = primary, decays with distance for neighbors
    pub is_main: bool,
}

/// ResultSet from radial search: primary hit + distance-weighted neighbors.
#[derive(Debug)]
pub struct ResultSet {
    pub primary: Option<RadialResult>,
    pub neighbors: Vec<RadialResult>,
    pub center: (f32, f32, f32),
    pub depth: u8,
    pub radius: f32,
    pub total_within_radius: usize,
}

impl ResultSet {
    /// All results (primary + neighbors) as a flat list.
    pub fn all(&self) -> Vec<&RadialResult> {
        let mut v = Vec::with_capacity(1 + self.neighbors.len());
        if let Some(ref p) = self.primary {
            v.push(p);
        }
        v.extend(self.neighbors.iter());
        v
    }

    /// Block indices of all results (for Hebbian co-activation).
    pub fn block_indices(&self) -> Vec<(u32, f32)> {
        self.all()
            .iter()
            .map(|r| (r.block_idx as u32, r.weight))
            .collect()
    }
}

const LAYER_ROLLING_WINDOW: usize = 50;

struct FileLock {
    path: PathBuf,
}

impl FileLock {
    fn acquire(config: &Config) -> Result<Self, String> {
        let lock_path = Path::new(&config.paths.output_dir).join("microscope.lock");
        loop {
            match fs::OpenOptions::new().write(true).create_new(true).open(&lock_path) {
                Ok(_f) => return Ok(FileLock { path: lock_path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) => return Err(format!("lock acquire: {}", e)),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn persist_to_layer_file(config: &Config, text: &str, layer: &str) -> Result<(), String> {
    let layers_dir = Path::new(&config.paths.layers_dir);
    let file_path = layers_dir.join(format!("{}.txt", layer));
    let mut content = String::new();
    if file_path.exists() {
        content = fs::read_to_string(&file_path)
            .map_err(|e| format!("read layer file: {}", e))?;
    }
    let stamped: String;
    let entry_text: &str = if layer == "session" {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let datetime = chrono_stamp(ts);
        stamped = format!("[{}] {}", datetime, text);
        &stamped
    } else {
        text
    };
    let mut entries: Vec<&str> = content.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
    entries.push(entry_text);
    if entries.len() > LAYER_ROLLING_WINDOW {
        let start = entries.len() - LAYER_ROLLING_WINDOW;
        entries.drain(..start);
    }
    let result = entries.join("\n\n");
    fs::write(&file_path, &result).map_err(|e| format!("write layer file: {}", e))?;
    Ok(())
}

fn chrono_stamp(epoch_secs: u64) -> String {
    let total_days = epoch_secs / 86400;
    let mut y = 1970u64;
    let mut remaining = total_days;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if remaining < diy { break; }
        remaining -= diy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u64;
    for &md in &mdays {
        if remaining < md as u64 { break; }
        remaining -= md as u64;
        mo += 1;
    }
    let day = remaining + 1;
    let secs_in_day = epoch_secs % 86400;
    let h = secs_in_day / 3600;
    let m = (secs_in_day % 3600) / 60;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, day, h, m)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn store_memory(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
    emotion: Option<[f32; 21]>,
) -> Result<(), String> {
    let _lock = FileLock::acquire(config)?;
    let t0 = std::time::Instant::now();
    let (x, y, z) = content_coords_blended(text, layer, config.search.semantic_weight);
    let lid = layer_to_id(layer);
    let depth = auto_depth(text);

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");

    let needs_magic = !append_path.exists()
        || fs::metadata(&append_path)
            .map(|m| m.len() == 0)
            .unwrap_or(true);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&append_path)
        .map_err(|e| format!("open append log: {}", e))?;

    let write = |f: &mut fs::File, data: &[u8]| -> Result<(), String> {
        f.write_all(data)
            .map_err(|e| format!("write append log: {}", e))
    };

    if needs_magic {
        write(&mut file, b"APv3")?;
    }

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    // Write header: len(4) + lid(1) + imp(1) + depth(1) + xyz(12) + emotion(84) = 103
    write(&mut file, &(len as u32).to_le_bytes())?;
    write(&mut file, &[lid])?;
    write(&mut file, &[importance])?;
    write(&mut file, &[depth])?;
    write(&mut file, &x.to_le_bytes())?;
    write(&mut file, &y.to_le_bytes())?;
    write(&mut file, &z.to_le_bytes())?;

    // Write 21D emotion vector (zero if not provided)
    let emo = emotion.unwrap_or([0.0f32; 21]);
    for val in &emo {
        write(&mut file, &val.to_le_bytes())?;
    }

    write(&mut file, &text_bytes[..len])?;

    // Also write to parallel emotions.bin for main-index lookups
    let emotions_path = Path::new(&config.paths.output_dir).join("emotions.bin");
    let _ = write_emotion(&emotions_path, usize::MAX, &emo); // placeholder entry

    if let Err(e) = persist_to_layer_file(config, text, layer) {
        eprintln!("  {} persist to layer file: {}", "WARN".yellow(), e);
    }

    let elapsed = t0.elapsed();
    print!(
        "  {} D{} [{}/{}] ({:.3},{:.3},{:.3})",
        "STORED".green().bold(),
        depth,
        layer,
        layer_color(lid),
        x,
        y,
        z,
    );
    // Show top-3 emotion dimensions if non-zero
    if emo.iter().any(|&v| v != 0.0) {
        let emo_str = format_emotion(&emo, 3);
        print!(" [{}]", emo_str.cyan());
    }
    println!(" {}", safe_truncate(text, 60));
    println!("  {} ns", elapsed.as_nanos());
    Ok(())
}
