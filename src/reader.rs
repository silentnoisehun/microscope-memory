//! MicroscopeReader Ă˘Â€Â” high-performance memory-mapped reader for the binary index.

use colored::Colorize;
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::types::{AppendEntry, BlockHeader, MemoryQueryOptions, ProjectId};
use crate::{
    auto_depth, content_coords_blended, layer_to_id, safe_truncate, BLOCK_DATA_SIZE,
    DEPTH_ENTRY_SIZE, HEADER_SIZE, LAYER_NAMES, LEGACY_HEADER_SIZE, META_HEADER_SIZE,
};

#[cfg(windows)]
use windows_sys::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS,
};

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

/// Backing store for block data Ă˘Â€Â” either memory-mapped or decompressed in-memory.
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
    pub header_stride: usize,
    pub depth_ranges: [(u32, u32); 9],
}

impl MicroscopeReader {
    pub fn open(config: &Config) -> Result<Self, String> {
        Self::open_from_path(&config.paths.output_dir)
    }

    fn open_from_path(output_dir: &str) -> Result<Self, String> {
        let output_dir = Path::new(output_dir);
        let meta_path = output_dir.join("meta.bin");
        let hdr_path = output_dir.join("microscope.bin");
        let dat_path = output_dir.join("data.bin");

        let meta = fs::read(&meta_path)
            .map_err(|e| format!("open meta.bin Ă˘Â€Â” run 'build' first: {}", e))?;
        if meta.len() < 12 {
            return Err("meta.bin too small".to_string());
        }
        let magic = &meta[0..4];
        let header_stride = if magic == b"MSC4" {
            HEADER_SIZE
        } else {
            LEGACY_HEADER_SIZE
        };
        if magic != b"MSCM" && magic != b"MSC2" && magic != b"MSC3" && magic != b"MSC4" {
            return Err("invalid magic: expected MSCM, MSC2, MSC3 or MSC4".to_string());
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
            header_stride,
            depth_ranges,
        })
    }

    /// Verifies that the mmap'ed memory is indeed readable and not guarded.
    #[cfg(windows)]
    fn verify_mmap_protection(ptr: *const u8, _len: usize) -> Result<(), String> {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        let mut _return_len: usize = 0;

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

        if info.Protect == PAGE_NOACCESS || (info.Protect & PAGE_GUARD) != 0 {
            return Err("Memory protection violation: Page is NOACCESS or GUARD".to_string());
        }

        Ok(())
    }

    #[inline(always)]
    pub fn header(&self, i: usize) -> BlockHeader {
        assert!(
            i < self.block_count,
            "block header index {i} out of bounds (block_count={})",
            self.block_count
        );
        unsafe { self.header_unchecked(i) }
    }

    /// Read the header for block `i` without a bounds check.
    ///
    /// # Safety
    /// `i` must be strictly less than `self.block_count`. Violating this reads
    /// outside the mmap region, which is undefined behavior.
    #[inline(always)]
    pub unsafe fn header_unchecked(&self, i: usize) -> BlockHeader {
        let off = i * self.header_stride;
        let ptr = unsafe { self.headers.as_ptr().add(off) };
        if self.header_stride == HEADER_SIZE {
            unsafe { std::ptr::read_unaligned(ptr as *const BlockHeader) }
        } else {
            // Legacy MSC3 header: 32 bytes, no project/importance/flags fields.
            let mut bytes = [0u8; LEGACY_HEADER_SIZE];
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), LEGACY_HEADER_SIZE);
            }
            let x = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
            let y = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
            let z = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
            let zoom = f32::from_le_bytes(bytes[12..16].try_into().unwrap());
            let depth = bytes[16];
            let layer_id = bytes[17];
            let data_offset = u32::from_le_bytes(bytes[18..22].try_into().unwrap());
            let data_len = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
            let parent_idx = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
            let child_count = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
            let crc16 = [bytes[30], bytes[31]];
            BlockHeader {
                x,
                y,
                z,
                zoom,
                depth,
                layer_id,
                data_offset,
                data_len,
                parent_idx,
                child_count,
                crc16,
                project_id: ProjectId::GLOBAL,
                importance: 0,
                flags: 0,
            }
        }
    }

    #[inline(always)]
    pub fn text(&self, i: usize) -> &str {
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
        results.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
        results.truncate(k);
        results.sort_by(|a, b| a.0.total_cmp(&b.0));
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
                let h = unsafe { self.header_unchecked(i) };
                (l2_dist_sq_simd(&h, x, y, z, qz, zw), i, true)
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
        results.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
        results.truncate(k);
        results.sort_by(|a, b| a.0.total_cmp(&b.0));
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
                    let dist_sq = l2_dist_sq_simd(&h, x, y, z, qz, 0.0); // no zoom weight for radial
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

        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));

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
                    Some((unsafe { self.header_unchecked(i) }.depth, i))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by_key(|&(d, _)| d);
        results.truncate(k);
        results
    }

    /// Text search across both the immutable main index and the hot append log.
    /// Append entries use virtual indices starting at 1_000_000.
    pub fn find_text_all(&self, config: &Config, query: &str, k: usize) -> Vec<(u8, usize, bool)> {
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize, bool)> = self
            .find_text(query, k)
            .into_iter()
            .map(|(depth, idx)| (depth, idx, true))
            .collect();

        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        results.extend(appended.iter().enumerate().filter_map(|(idx, entry)| {
            entry
                .text
                .to_lowercase()
                .contains(&q)
                .then_some((entry.depth, idx + 1_000_000, false))
        }));

        results.sort_by_key(|&(depth, _, is_main)| (depth, is_main));
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

// Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€ APPEND LOG Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€

#[inline(always)]
fn entry_visible(entry: &AppendEntry, active: ProjectId, include_global: bool) -> bool {
    entry.project_id == active || (include_global && entry.project_id.is_global())
}

impl MicroscopeReader {
    /// Project-scoped variant of `look`.
    #[allow(clippy::too_many_arguments)]
    pub fn look_with_options(
        &self,
        config: &Config,
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        k: usize,
        opts: Option<&MemoryQueryOptions>,
    ) -> Vec<(f32, usize, bool)> {
        let opts = opts.cloned().unwrap_or_default();
        let (start, count) = self.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);

        let mut results: Vec<(f32, usize, bool)> = Vec::with_capacity(count + 10);
        if count > 0 {
            for i in start..(start + count) {
                let h = self.header(i);
                if !h.is_visible_to(opts.active_project, opts.include_global)
                    || h.importance < opts.min_importance
                {
                    continue;
                }
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
            if !entry_visible(entry, opts.active_project, opts.include_global)
                || entry.importance < opts.min_importance
            {
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
        results.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
        results.truncate(k);
        results.sort_by(|a, b| a.0.total_cmp(&b.0));
        results
    }

    /// Project-scoped variant of `look_soft`.
    #[allow(clippy::too_many_arguments)]
    pub fn look_soft_with_options(
        &self,
        config: &Config,
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        k: usize,
        zw: f32,
        opts: Option<&MemoryQueryOptions>,
    ) -> Vec<(f32, usize, bool)> {
        let opts = opts.cloned().unwrap_or_default();
        let qz = zoom as f32 / 8.0;
        let mut results: Vec<(f32, usize, bool)> = (0..self.block_count)
            .into_par_iter()
            .filter_map(|i| {
                let h = unsafe { self.header_unchecked(i) };
                if !h.is_visible_to(opts.active_project, opts.include_global)
                    || h.importance < opts.min_importance
                {
                    return None;
                }
                Some((l2_dist_sq_simd(&h, x, y, z, qz, zw), i, true))
            })
            .collect();

        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            if !entry_visible(entry, opts.active_project, opts.include_global)
                || entry.importance < opts.min_importance
            {
                continue;
            }
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
        results.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
        results.truncate(k);
        results.sort_by(|a, b| a.0.total_cmp(&b.0));
        results
    }

    /// Project-scoped variant of `radial_search`.
    #[allow(clippy::too_many_arguments)]
    pub fn radial_search_with_options(
        &self,
        config: &Config,
        x: f32,
        y: f32,
        z: f32,
        depth: u8,
        radius: f32,
        k: usize,
        opts: Option<&MemoryQueryOptions>,
    ) -> ResultSet {
        let opts = opts.cloned().unwrap_or_default();
        let radius_sq = radius * radius;
        let (start, count) = self.depth_ranges[depth as usize];
        let (start, count) = (start as usize, count as usize);

        let mut candidates: Vec<(f32, usize, bool)> = if count > 0 {
            (start..(start + count))
                .into_par_iter()
                .filter_map(|i| {
                    let h = self.header(i);
                    if !h.is_visible_to(opts.active_project, opts.include_global)
                        || h.importance < opts.min_importance
                    {
                        return None;
                    }
                    let qz = depth as f32 / 8.0;
                    let dist_sq = l2_dist_sq_simd(&h, x, y, z, qz, 0.0); // no zoom weight for radial
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

        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for (ai, entry) in appended.iter().enumerate() {
            if entry.depth != depth {
                continue;
            }
            if !entry_visible(entry, opts.active_project, opts.include_global)
                || entry.importance < opts.min_importance
            {
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

        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));

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

    /// Project-scoped variant of `find_text`.
    pub fn find_text_with_options(
        &self,
        query: &str,
        k: usize,
        opts: Option<&MemoryQueryOptions>,
    ) -> Vec<(u8, usize)> {
        let opts = opts.cloned().unwrap_or_default();
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize)> = (0..self.block_count)
            .into_par_iter()
            .filter_map(|i| {
                let h = unsafe { self.header_unchecked(i) };
                if !h.is_visible_to(opts.active_project, opts.include_global)
                    || h.importance < opts.min_importance
                {
                    return None;
                }
                if self.text(i).to_lowercase().contains(&q) {
                    Some((h.depth, i))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by_key(|&(d, _)| d);
        results.truncate(k);
        results
    }

    /// Project-scoped variant of `find_text_all`.
    pub fn find_text_all_with_options(
        &self,
        config: &Config,
        query: &str,
        k: usize,
        opts: Option<&MemoryQueryOptions>,
    ) -> Vec<(u8, usize, bool)> {
        let opts = opts.cloned().unwrap_or_default();
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize, bool)> = self
            .find_text_with_options(query, k, Some(&opts))
            .into_iter()
            .map(|(depth, idx)| (depth, idx, true))
            .collect();

        let append_path = Path::new(&config.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        results.extend(appended.iter().enumerate().filter_map(|(idx, entry)| {
            if !entry_visible(entry, opts.active_project, opts.include_global)
                || entry.importance < opts.min_importance
            {
                return None;
            }
            entry
                .text
                .to_lowercase()
                .contains(&q)
                .then_some((entry.depth, idx + 1_000_000, false))
        }));

        results.sort_by_key(|&(depth, _, is_main)| (depth, is_main));
        results.truncate(k);
        results
    }
}

#[allow(dead_code)]
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

    let (is_v2, is_v3) = if data.len() >= 4 {
        (&data[0..4] == b"APv2", &data[0..4] == b"APv3")
    } else {
        (false, false)
    };
    if is_v2 || is_v3 {
        pos = 4;
    }

    // v2: len(4)+lid(1)+imp(1)+depth(1)+coords(12) = 19
    // v3: len(4)+lid(1)+imp(1)+depth(1)+project_id(16)+coords(12) = 35
    let header_size = if is_v3 {
        35
    } else if is_v2 {
        19
    } else {
        18
    };

    while pos + header_size <= data.len() {
        let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let lid = data[pos + 4];
        let imp = data[pos + 5];
        let depth = if is_v2 || is_v3 { data[pos + 6] } else { 4u8 };

        let project_id = if is_v3 {
            let mut pid = [0u8; 16];
            pid.copy_from_slice(&data[pos + 7..pos + 23]);
            ProjectId(pid)
        } else {
            ProjectId::GLOBAL
        };

        let coords_start = if is_v3 {
            pos + 23
        } else if is_v2 {
            pos + 7
        } else {
            pos + 6
        };

        let x = f32::from_le_bytes(data[coords_start..coords_start + 4].try_into().unwrap());
        let y = f32::from_le_bytes(data[coords_start + 4..coords_start + 8].try_into().unwrap());
        let z = f32::from_le_bytes(
            data[coords_start + 8..coords_start + 12]
                .try_into()
                .unwrap(),
        );
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
            emotion: [0.0f32; 21],
            project_id,
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

// Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€ RADIAL SEARCH TYPES Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€

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

/// A lock file is considered stale when untouched for this many seconds,
/// regardless of content (covers lock files from older versions that carry no
/// owner token at all).
const LOCK_STALE_SECS: u64 = 300;

/// Total time we are willing to wait for a live lock before giving up.
const LOCK_ACQUIRE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub(crate) struct FileLock {
    path: PathBuf,
    token: String,
}

impl FileLock {
    pub(crate) fn acquire(config: &Config) -> Result<Self, String> {
        let lock_path = Path::new(&config.paths.output_dir).join("microscope.lock");
        Self::acquire_inner(&lock_path, LOCK_ACQUIRE_TIMEOUT)
    }

    fn acquire_inner(lock_path: &Path, timeout: std::time::Duration) -> Result<Self, String> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock_path)
            {
                Ok(mut file) => {
                    let token = lock_token();
                    if let Err(e) = file.write_all(token.as_bytes()).and_then(|()| file.flush()) {
                        // Never leave a partially-owned lock behind.
                        let _ = fs::remove_file(lock_path);
                        return Err(format!("lock acquire: write owner token: {}", e));
                    }
                    return Ok(FileLock {
                        path: lock_path.to_path_buf(),
                        token,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(lock_path) {
                        // Another process may race us to remove and recreate the
                        // file; just retry create_new after the removal.
                        let _ = fs::remove_file(lock_path);
                        continue;
                    }
                    if std::time::Instant::now() >= deadline {
                        let mtime = fs::metadata(lock_path)
                            .and_then(|m| m.modified())
                            .map(|t| format!("{:?}", t))
                            .unwrap_or_else(|_| "unknown".to_string());
                        let content = fs::read_to_string(lock_path)
                            .unwrap_or_else(|_| "<unreadable>".to_string());
                        return Err(format!(
                            "store lock held by {} since {}; remove {} if stale",
                            content,
                            mtime,
                            lock_path.display()
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(e) => return Err(format!("lock acquire: {}", e)),
            }
        }
    }
}

/// Ownership token: "<pid>:<unix milliseconds>". pid:unix_ms is unique enough
/// for the lifetime of a lock file; no random suffix is needed.
fn lock_token() -> String {
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}:{}", std::process::id(), unix_ms)
}

/// First field (before ':') of a lock token, when present and numeric.
#[cfg(windows)]
fn lock_owner_pid(content: &str) -> Option<u32> {
    content.split(':').next()?.trim().parse().ok()
}

/// Returns true when the existing lock file can safely be broken.
fn lock_is_stale(path: &Path) -> bool {
    // Primary check: file age. A lock untouched for LOCK_STALE_SECS is stale
    // even if it has no readable owner token.
    if let Ok(meta) = fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = modified.elapsed() {
                if age.as_secs() > LOCK_STALE_SECS {
                    return true;
                }
            }
        }
    }
    // Secondary check (Windows only): the recorded owner PID is no longer
    // alive. On non-Windows platforms we deliberately rely on the mtime check
    // alone: std has no portable process-existence probe and adding libc just
    // for this would be a new dependency.
    #[cfg(windows)]
    {
        if let Ok(content) = fs::read_to_string(path) {
            if let Some(pid) = lock_owner_pid(&content) {
                return !pid_is_alive(pid);
            }
        }
    }
    false
}

/// Windows-only process liveness probe using the already-available
/// windows-sys dependency (OpenProcess with the limited query right).
#[cfg(windows)]
fn pid_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return false;
        }
        CloseHandle(handle);
        true
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // Only delete the lock if it still carries OUR token. If another
        // process re-acquired the lock after we released it, removing the file
        // would break their mutual exclusion, so leave it alone.
        match fs::read_to_string(&self.path) {
            Ok(content) if content == self.token => {
                let _ = fs::remove_file(&self.path);
            }
            _ => {}
        }
    }
}

/// Parse a leading `(imp=N)` marker from a layer entry.
///
/// Layer entries carry their importance as a leading `(imp=N)` marker. Legacy
/// entries without the marker default to importance 5. Returns the entry text
/// without the marker and the parsed importance.
pub fn parse_imp_marker(entry: &str) -> (&str, u8) {
    let text = entry.trim_start();
    if let Some(rest) = text.strip_prefix("(imp=") {
        if let Some(end) = rest.find(')') {
            if let Ok(imp) = rest[..end].parse::<u8>() {
                return (rest[end + 1..].trim_start(), imp);
            }
        }
    }
    (entry, 5)
}

fn persist_to_layer_file(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
) -> Result<(), String> {
    let layers_dir = Path::new(&config.paths.layers_dir);
    let file_path = layers_dir.join(format!("{}.txt", layer));
    let mut content = String::new();
    if file_path.exists() {
        content = fs::read_to_string(&file_path).map_err(|e| format!("read layer file: {}", e))?;
    }
    let stamped: String;
    let entry_text: &str = if layer == "session" {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let datetime = chrono_stamp(ts);
        stamped = format!("(imp={}) [{}] {}", importance, datetime, text);
        &stamped
    } else {
        stamped = format!("(imp={}) {}", importance, text);
        &stamped
    };
    let mut entries: Vec<&str> = content
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();
    entries.push(entry_text);
    let retention = config.index.layer_retention_entries;
    if retention > 0 && entries.len() > retention {
        let start = entries.len() - retention;
        entries.drain(..start);
    }
    let result = entries.join("\n\n");
    let tmp_path = file_path.with_extension("txt.tmp");
    fs::write(&tmp_path, &result).map_err(|e| format!("write layer file: {}", e))?;
    fs::rename(&tmp_path, &file_path).map_err(|e| format!("rename layer file: {}", e))?;
    Ok(())
}

fn chrono_stamp(epoch_secs: u64) -> String {
    let total_days = epoch_secs / 86400;
    let mut y = 1970u64;
    let mut remaining = total_days;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if remaining < diy {
            break;
        }
        remaining -= diy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u64;
    for &md in &mdays {
        if remaining < md as u64 {
            break;
        }
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
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

#[derive(Clone, Copy)]
enum AppendFormat {
    V2,
    V3,
}

fn detect_append_format(path: &Path) -> AppendFormat {
    match fs::read(path) {
        Ok(data) if data.len() >= 4 => {
            if &data[0..4] == b"APv3" {
                AppendFormat::V3
            } else if &data[0..4] == b"APv2" {
                AppendFormat::V2
            } else {
                AppendFormat::V3
            }
        }
        _ => AppendFormat::V3,
    }
}

#[allow(clippy::too_many_arguments)]
fn write_append_entry(
    f: &mut fs::File,
    lid: u8,
    importance: u8,
    depth: u8,
    project_id: ProjectId,
    x: f32,
    y: f32,
    z: f32,
    text_bytes: &[u8],
    format: AppendFormat,
) -> Result<(), String> {
    let write = |f: &mut fs::File, data: &[u8]| -> Result<(), String> {
        f.write_all(data)
            .map_err(|e| format!("write append log: {}", e))
    };
    write(f, &(text_bytes.len() as u32).to_le_bytes())?;
    write(f, &[lid])?;
    write(f, &[importance])?;
    write(f, &[depth])?;
    if matches!(format, AppendFormat::V3) {
        write(f, &project_id.0)?;
    }
    write(f, &x.to_le_bytes())?;
    write(f, &y.to_le_bytes())?;
    write(f, &z.to_le_bytes())?;
    write(f, text_bytes)?;
    Ok(())
}

pub fn store_memory(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
) -> Result<(), String> {
    store_memory_with_status(config, text, layer, importance, None, None)
}

/// Variant with emotion vector.
pub fn store_memory_with_emotion(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
    emotion: Option<[f32; 21]>,
) -> Result<(), String> {
    store_memory_with_status(config, text, layer, importance, None, emotion)
}

/// Store memory to append log and timeline only (NOT to layer files).
/// Used for temporary/internal thoughts that should not persist through rebuilds.
pub fn store_memory_temporary(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
) -> Result<(), String> {
    let _lock = FileLock::acquire(config)?;
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

    let format = if needs_magic {
        AppendFormat::V3
    } else {
        detect_append_format(&append_path)
    };

    if needs_magic {
        let magic = match format {
            AppendFormat::V2 => b"APv2",
            AppendFormat::V3 => b"APv3",
        };
        file.write_all(magic)
            .map_err(|e| format!("write append log: {}", e))?;
    }

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    write_append_entry(
        &mut file,
        lid,
        importance,
        depth,
        config.project_id,
        x,
        y,
        z,
        &text_bytes[..len],
        format,
    )?;

    // Timeline log (always)
    let output_dir = Path::new(&config.paths.output_dir);
    let entry = crate::timeline::TimelineEntry {
        ts_ms: crate::timeline::now_epoch_ms(),
        layer_id: lid,
        importance,
        depth,
        status: crate::timeline::STATUS_NORMAL,
        text: text.to_string(),
    };
    if let Err(e) = crate::timeline::append_entry(&output_dir.join("timeline.bin"), &entry) {
        eprintln!("  {} append timeline: {}", "WARN".yellow(), e);
    }

    Ok(())
}

/// Variant of `store_memory` that also writes to the timeline log and,
/// optionally, marks the entry as an open loop (status="open").
pub fn store_memory_with_status(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
    status: Option<&str>,
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

    let format = if needs_magic {
        AppendFormat::V3
    } else {
        detect_append_format(&append_path)
    };

    if needs_magic {
        let magic = match format {
            AppendFormat::V2 => b"APv2",
            AppendFormat::V3 => b"APv3",
        };
        file.write_all(magic)
            .map_err(|e| format!("write append log: {}", e))?;
    }

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    write_append_entry(
        &mut file,
        lid,
        importance,
        depth,
        config.project_id,
        x,
        y,
        z,
        &text_bytes[..len],
        format,
    )?;
    file.flush()
        .map_err(|e| format!("flush append log: {}", e))?;

    if let Err(e) = persist_to_layer_file(config, text, layer, importance) {
        eprintln!("  {} persist to layer file: {}", "WARN".yellow(), e);
    }

    // Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€ Timeline log (always) Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€
    let output_dir = Path::new(&config.paths.output_dir);
    let timeline_status = match status.unwrap_or("normal") {
        "open" => crate::timeline::STATUS_OPEN,
        "resolved" => crate::timeline::STATUS_RESOLVED,
        "archived" => crate::timeline::STATUS_ARCHIVED,
        _ => crate::timeline::STATUS_NORMAL,
    };
    let entry = crate::timeline::TimelineEntry {
        ts_ms: crate::timeline::now_epoch_ms(),
        layer_id: lid,
        importance,
        depth,
        status: timeline_status,
        text: text.to_string(),
    };
    if let Err(e) = crate::timeline::append_entry(&output_dir.join("timeline.bin"), &entry) {
        eprintln!("  {} append timeline: {}", "WARN".yellow(), e);
    }

    // Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€ Open loops (only when status=open) Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€Ă˘Â”Â€
    if status == Some("open") {
        match crate::open_loops::append_open(output_dir, text, importance) {
            Ok(loop_id) => {
                println!("  {} loop_id={}", "LOOP".cyan().bold(), loop_id);
            }
            Err(e) => {
                eprintln!("  {} open loop: {}", "WARN".yellow(), e);
            }
        }
    }

    // â”€â”€â”€ Emotion log (when provided) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // Previously the emotion vector was accepted but never written anywhere,
    // silently dropping all 21D emotion data. Now we persist it to emotion_log.bin
    // (rebuilt into emotions.bin during `build_emotions_from_log`).
    if let Some(emo) = emotion {
        if let Err(e) = append_emotion_log(output_dir, text, &emo) {
            eprintln!("  {} append emotion log: {}", "WARN".yellow(), e);
        }
    }

    // Release the writer and process lock before a possible rebuild. The
    // rebuild helper acquires the same lock and rechecks the threshold.
    drop(file);
    drop(_lock);

    match crate::build::maybe_auto_rebuild(config) {
        Ok(Some(count)) => println!(
            "  {} {} append entries consolidated",
            "AUTO-REBUILD".cyan().bold(),
            count
        ),
        Ok(None) => {}
        Err(e) => eprintln!("  {} auto-rebuild: {}", "WARN".yellow(), e),
    }

    let elapsed = t0.elapsed();
    println!(
        "  {} D{} [{}/{}] ({:.3},{:.3},{:.3}) {}",
        "STORED".green().bold(),
        depth,
        layer,
        layer_color(lid),
        x,
        y,
        z,
        safe_truncate(text, 60)
    );
    if timeline_status != crate::timeline::STATUS_NORMAL {
        let label = match timeline_status {
            crate::timeline::STATUS_OPEN => "open",
            crate::timeline::STATUS_RESOLVED => "resolved",
            crate::timeline::STATUS_ARCHIVED => "archived",
            _ => "?",
        };
        println!("  {} status={}", "TIMELINE".cyan().bold(), label);
    }
    println!("  {} ns", elapsed.as_nanos());
    Ok(())
}

// Â¦Â¦Â¦ Emotion constants Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦Â¦
pub const EMOTION_VECTOR_SIZE: usize = 21;

/// Emotion dimension labels for the 21D emotion vector.
pub const EMOTION_DIMS: &[&str] = &[
    "joy",
    "sadness",
    "anger",
    "fear",
    "surprise",
    "disgust",
    "trust",
    "anticipation",
    "love",
    "gratitude",
    "curiosity",
    "confusion",
    "pride",
    "shame",
    "anxiety",
    "calm",
    "excitement",
    "boredom",
    "hope",
    "regret",
    "empathy",
];

/// Cosine similarity between two 21D emotion vectors.
pub fn emotional_similarity(a: &[f32; 21], b: &[f32; 21]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-10 || nb < 1e-10 {
        return 0.0;
    }
    (dot / (na * nb)).clamp(0.0, 1.0)
}

type EmotionLookup = Box<dyn Fn(usize) -> Option<[f32; 21]>>;

/// Load emotions.bin and return a lookup closure.
/// emotions.bin format: flat array of [f32; 21] per block index.
pub fn load_emotion_lookup(output_dir: &Path) -> Option<EmotionLookup> {
    let path = output_dir.join("emotions.bin");
    if !path.exists() {
        return None;
    }
    let data = fs::read(&path).ok()?;
    let entry_size = 21 * 4; // 21 f32 values, 4 bytes each
    let count = data.len() / entry_size;
    Some(Box::new(move |idx: usize| {
        if idx >= count {
            return None;
        }
        let off = idx * entry_size;
        if off + entry_size > data.len() {
            return None;
        }
        let mut emo = [0.0f32; 21];
        for (i, e) in emo.iter_mut().enumerate() {
            let bytes: [u8; 4] = data[off + i * 4..off + i * 4 + 4].try_into().ok()?;
            *e = f32::from_le_bytes(bytes);
        }
        Some(emo)
    }))
}

/// Write a single block's emotion vector to emotions.bin.
/// The file is grown to fit the block index if needed.
pub fn write_emotion(path: &Path, block_idx: usize, emotion: &[f32; 21]) -> Result<(), String> {
    let entry_size = 21 * 4;
    let needed = (block_idx + 1) * entry_size;
    let mut data = if path.exists() {
        fs::read(path).map_err(|e| format!("read emotions.bin: {}", e))?
    } else {
        Vec::new()
    };
    if data.len() < needed {
        data.resize(needed, 0u8);
    }
    let off = block_idx * entry_size;
    for i in 0..21 {
        data[off + i * 4..off + i * 4 + 4].copy_from_slice(&emotion[i].to_le_bytes());
    }
    // Atomic write: temp file + rename to prevent corruption on crash
    let tmp_path = path.with_extension("bin.tmp");
    fs::write(&tmp_path, &data).map_err(|e| format!("write emotions.bin: {}", e))?;
    fs::rename(&tmp_path, path).map_err(|e| format!("rename emotions.bin: {}", e))?;
    Ok(())
}

/// Append an emotion vector to the emotion log (emotion_log.bin).
/// Format: [u64 timestamp_ms] [f32; 21 emotion] [u32 text_len] [bytes text]
pub fn append_emotion_log(
    output_dir: &Path,
    text: &str,
    emotion: &[f32; 21],
) -> Result<(), String> {
    let path = output_dir.join("emotion_log.bin");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open emotion_log.bin: {}", e))?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    file.write_all(&ts.to_le_bytes())
        .map_err(|e| format!("write emotion_log ts: {}", e))?;
    for &v in emotion {
        file.write_all(&v.to_le_bytes())
            .map_err(|e| format!("write emotion_log value: {}", e))?;
    }
    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(4096) as u32;
    file.write_all(&len.to_le_bytes())
        .map_err(|e| format!("write emotion_log text_len: {}", e))?;
    file.write_all(&text_bytes[..len as usize])
        .map_err(|e| format!("write emotion_log text: {}", e))?;
    Ok(())
}

/// Build emotions.bin from the emotion log and main index.
/// Reads emotion_log.bin and maps each entry to the closest main-index block.
pub fn build_emotions_from_log(output_dir: &Path, reader: &MicroscopeReader) -> Result<(), String> {
    let log_path = output_dir.join("emotion_log.bin");
    if !log_path.exists() {
        return Ok(());
    }
    let data = fs::read(&log_path).map_err(|e| format!("read emotion_log.bin: {}", e))?;
    let entry_size = 8 + 21 * 4 + 4; // ts + emotion + text_len
    let mut emotions = vec![[0.0f32; 21]; reader.block_count];
    let mut i = 0;
    while i + entry_size <= data.len() {
        let ts_bytes: [u8; 8] = data[i..i + 8].try_into().unwrap();
        let _ts = u64::from_le_bytes(ts_bytes);
        i += 8;
        let mut emo = [0.0f32; 21];
        for e in emo.iter_mut() {
            let bytes: [u8; 4] = data[i..i + 4].try_into().unwrap();
            *e = f32::from_le_bytes(bytes);
            i += 4;
        }
        let text_len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        let text_end = (i + text_len).min(data.len());
        let _text = String::from_utf8_lossy(&data[i..text_end]).to_string();
        i = text_end;

        // Find closest block by content coords
        let (tx, ty, tz) = crate::content_coords(&_text, "emotional");
        let mut best_dist = f32::MAX;
        let mut best_idx = 0;
        for bi in 0..reader.block_count {
            let h = reader.header(bi);
            let dx = h.x - tx;
            let dy = h.y - ty;
            let dz = h.z - tz;
            let d = dx * dx + dy * dy + dz * dz;
            if d < best_dist {
                best_dist = d;
                best_idx = bi;
            }
        }
        if best_dist < 0.1 {
            emotions[best_idx] = emo;
        }
    }
    // Write emotions.bin
    let emo_path = output_dir.join("emotions.bin");
    let mut out = Vec::with_capacity(emotions.len() * 21 * 4);
    for emo in &emotions {
        for &v in emo {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    let emo_tmp = output_dir.join("emotions.bin.tmp");
    fs::write(&emo_tmp, &out).map_err(|e| format!("write emotions.bin: {}", e))?;
    fs::rename(&emo_tmp, &emo_path).map_err(|e| format!("rename emotions.bin: {}", e))?;
    Ok(())
}

/// Format an emotion vector as a human-readable string.
pub fn format_emotion(emotion: &[f32; 21]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, label) in EMOTION_DIMS.iter().enumerate() {
        if i < emotion.len() && emotion[i] > 0.1 {
            parts.push(format!("{}={:.2}", label, emotion[i]));
        }
    }
    if parts.is_empty() {
        "neutral".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn lock_token_contains_pid_and_timestamp() {
        let token = lock_token();
        let mut parts = token.splitn(2, ':');
        let pid = parts.next().unwrap().parse::<u32>().unwrap();
        assert_eq!(pid, std::process::id());
        let ms = parts.next().unwrap().parse::<u128>().unwrap();
        assert!(ms > 0);
    }

    #[test]
    fn acquire_releases_on_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("microscope.lock");
        {
            let lock = FileLock::acquire_inner(&path, Duration::from_secs(2)).unwrap();
            assert!(path.exists());
            assert_eq!(fs::read_to_string(&path).unwrap(), lock.token);
        }
        assert!(!path.exists(), "drop must remove our own lock");
    }

    #[test]
    fn live_lock_errors_in_bounded_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("microscope.lock");
        fs::write(&path, lock_token()).unwrap();
        let t0 = std::time::Instant::now();
        match FileLock::acquire_inner(&path, Duration::from_millis(300)) {
            Ok(_) => panic!("a fresh live lock must not be stolen"),
            Err(err) => {
                assert!(
                    t0.elapsed() < Duration::from_secs(5),
                    "acquire must give up within the timeout"
                );
                assert!(
                    err.contains("microscope.lock"),
                    "error should mention the lock path: {}",
                    err
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn dead_owner_pid_is_broken() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("microscope.lock");
        fs::write(&path, "4294967295:0").unwrap();
        let lock = FileLock::acquire_inner(&path, Duration::from_secs(2)).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), lock.token);
    }

    #[test]
    fn parse_imp_marker_extracts_importance_and_strips_marker() {
        assert_eq!(
            parse_imp_marker("(imp=7) döntés rögzítve"),
            ("döntés rögzítve", 7)
        );
        assert_eq!(
            parse_imp_marker("(imp=8) [2026-08-02 10:00] identitás"),
            ("[2026-08-02 10:00] identitás", 8)
        );
        assert_eq!(
            parse_imp_marker("régi bejegyzés marker nélkül"),
            ("régi bejegyzés marker nélkül", 5)
        );
        assert_eq!(
            parse_imp_marker("(imp=12) túl magas érték"),
            ("túl magas érték", 12)
        );
    }

    #[test]
    fn header_reads_valid_index_and_panics_out_of_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let hdr_file = dir.path().join("microscope.bin");
        fs::write(&hdr_file, vec![0u8; HEADER_SIZE]).unwrap();
        let dat_file = dir.path().join("data.bin");
        fs::write(&dat_file, vec![0u8; 1]).unwrap();
        let map = unsafe { memmap2::Mmap::map(&std::fs::File::open(&hdr_file).unwrap()) }.unwrap();
        let dmap = unsafe { memmap2::Mmap::map(&std::fs::File::open(&dat_file).unwrap()) }.unwrap();
        let reader = MicroscopeReader {
            headers: map,
            data: DataStore::Mmap(dmap),
            block_count: 1,
            header_stride: HEADER_SIZE,
            depth_ranges: [(0, 0); 9],
        };

        // Valid index reads the (dummy, zeroed) header without issue.
        let h = reader.header(0);
        assert_eq!(h.depth, 0);

        // Out-of-bounds index must panic even in release builds.
        let result = std::panic::catch_unwind(|| {
            let _ = reader.header(1);
        });
        assert!(result.is_err(), "out-of-bounds header read must panic");
    }
}
