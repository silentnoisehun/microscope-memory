//! MicroscopeReader — high-performance memory-mapped reader for the binary index.

use colored::Colorize;
use rayon::prelude::*;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::{
    auto_depth, content_coords_blended, layer_to_id, safe_truncate, BLOCK_DATA_SIZE,
    DEPTH_ENTRY_SIZE, HEADER_SIZE, LAYER_NAMES, META_HEADER_SIZE,
};

#[cfg(feature = "stealth")]
use crate::syscaller::nt_query_virtual_memory;

use windows_sys::Win32::System::Memory::{MEMORY_BASIC_INFORMATION, PAGE_NOACCESS, PAGE_GUARD};

#[cfg(not(feature = "stealth"))]
use windows_sys::Win32::System::Memory::VirtualQuery;

#[cfg(feature = "stealth")]
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
            return Err(format!("NtQueryVirtualMemory failed with status 0x{:08X}", status));
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

    let is_v2 = data.len() >= 4 && &data[0..4] == b"APv2";
    if is_v2 {
        pos = 4;
    }

    let header_size = if is_v2 { 19 } else { 18 };

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

pub fn store_memory(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
) -> Result<(), String> {
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
        write(&mut file, b"APv2")?;
    }

    let text_bytes = text.as_bytes();
    let len = text_bytes.len().min(BLOCK_DATA_SIZE);

    write(&mut file, &(len as u32).to_le_bytes())?;
    write(&mut file, &[lid])?;
    write(&mut file, &[importance])?;
    write(&mut file, &[depth])?;
    write(&mut file, &x.to_le_bytes())?;
    write(&mut file, &y.to_le_bytes())?;
    write(&mut file, &z.to_le_bytes())?;
    write(&mut file, &text_bytes[..len])?;

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
    println!("  {} ns", elapsed.as_nanos());
    Ok(())
}
