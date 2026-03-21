//! WebAssembly interface module — runs Microscope Memory in browsers.
//!
//! Uses the real binary format (meta.bin + microscope.bin + data.bin) via
//! ArrayBuffer → &[u8] slices. No file I/O, no mmap — pure owned Vec<u8>.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys::console;

use crate::{BlockHeader, HEADER_SIZE, META_HEADER_SIZE, DEPTH_ENTRY_SIZE, LAYER_NAMES,
            content_coords_blended, BLOCK_DATA_SIZE};

/// JavaScript-accessible block result
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmBlock {
    text: String,
    x: f32,
    y: f32,
    z: f32,
    depth: u8,
    layer: String,
    distance: f32,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmBlock {
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String { self.text.clone() }
    #[wasm_bindgen(getter)]
    pub fn x(&self) -> f32 { self.x }
    #[wasm_bindgen(getter)]
    pub fn y(&self) -> f32 { self.y }
    #[wasm_bindgen(getter)]
    pub fn z(&self) -> f32 { self.z }
    #[wasm_bindgen(getter)]
    pub fn depth(&self) -> u8 { self.depth }
    #[wasm_bindgen(getter)]
    pub fn layer(&self) -> String { self.layer.clone() }
    #[wasm_bindgen(getter)]
    pub fn distance(&self) -> f32 { self.distance }
}

/// Internal reader that owns the binary data (no mmap).
struct WasmReader {
    headers: Vec<u8>,
    data: Vec<u8>,
    block_count: usize,
    depth_ranges: [(u32, u32); 9],
}

impl WasmReader {
    fn from_buffers(meta: &[u8], headers: Vec<u8>, data: Vec<u8>) -> Result<Self, String> {
        if meta.len() < META_HEADER_SIZE + 9 * DEPTH_ENTRY_SIZE {
            return Err("meta.bin too small".into());
        }
        let magic = &meta[0..4];
        if magic != b"MSCM" && magic != b"MSC2" {
            return Err(format!("invalid magic: {:?}", &meta[0..4]));
        }
        let block_count = u32::from_le_bytes(meta[8..12].try_into().unwrap()) as usize;

        let expected_hdr_size = block_count * HEADER_SIZE;
        if headers.len() < expected_hdr_size {
            return Err(format!("headers too small: {} < {}", headers.len(), expected_hdr_size));
        }

        let mut depth_ranges = [(0u32, 0u32); 9];
        for d in 0..9 {
            let off = META_HEADER_SIZE + d * DEPTH_ENTRY_SIZE;
            let start = u32::from_le_bytes(meta[off..off+4].try_into().unwrap());
            let count = u32::from_le_bytes(meta[off+4..off+8].try_into().unwrap());
            depth_ranges[d] = (start, count);
        }

        Ok(WasmReader { headers, data, block_count, depth_ranges })
    }

    #[inline(always)]
    fn header(&self, i: usize) -> &BlockHeader {
        debug_assert!(i < self.block_count);
        unsafe { &*(self.headers.as_ptr().add(i * HEADER_SIZE) as *const BlockHeader) }
    }

    #[inline(always)]
    fn text(&self, i: usize) -> &str {
        let h = self.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        if end <= self.data.len() {
            std::str::from_utf8(&self.data[start..end]).unwrap_or("<bin>")
        } else {
            "<oob>"
        }
    }

    fn look(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<(f32, usize)> {
        let (start, count) = self.depth_ranges[zoom.min(8) as usize];
        let (start, count) = (start as usize, count as usize);
        let mut results: Vec<(f32, usize)> = Vec::with_capacity(count);
        for i in start..(start + count) {
            let h = self.header(i);
            let dx = h.x - x;
            let dy = h.y - y;
            let dz = h.z - z;
            results.push((dx*dx + dy*dy + dz*dz, i));
        }
        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    fn look_soft(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize, zw: f32) -> Vec<(f32, usize)> {
        let qz = zoom as f32 / 8.0;
        let mut results: Vec<(f32, usize)> = (0..self.block_count)
            .map(|i| {
                let h = self.header(i);
                let dx = h.x - x;
                let dy = h.y - y;
                let dz = h.z - z;
                let dw = (h.zoom - qz) * zw;
                (dx*dx + dy*dy + dz*dz + dw*dw, i)
            })
            .collect();
        let k = k.min(results.len());
        if k == 0 { return vec![]; }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results
    }

    fn find_text(&self, query: &str, k: usize) -> Vec<(u8, usize)> {
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize)> = (0..self.block_count)
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

    fn recall(&self, query: &str, k: usize) -> Vec<(f32, usize)> {
        let (qx, qy, qz) = content_coords_blended(query, "long_term", 0.0);
        let q_lower = query.to_lowercase();
        let keywords: Vec<&str> = q_lower.split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        let zoom_range = match query.len() {
            0..=10 => (0u8, 3u8),
            11..=40 => (3, 6),
            _ => (6, 8),
        };

        let mut results: Vec<(f32, usize)> = Vec::new();
        for zoom in zoom_range.0..=zoom_range.1 {
            let (start, count) = self.depth_ranges[zoom as usize];
            let (start, count) = (start as usize, count as usize);
            for i in start..(start + count) {
                let text = self.text(i).to_lowercase();
                let keyword_hits = keywords.iter().filter(|&&kw| text.contains(kw)).count();
                if keyword_hits > 0 {
                    let h = self.header(i);
                    let dx = h.x - qx;
                    let dy = h.y - qy;
                    let dz = h.z - qz;
                    let spatial = dx*dx + dy*dy + dz*dz;
                    let boost = keyword_hits as f32 * 0.1;
                    results.push(((spatial - boost).max(0.0), i));
                }
            }
        }

        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        results.truncate(k);
        results
    }

    fn to_wasm_block(&self, i: usize, dist: f32) -> WasmBlock {
        let h = self.header(i);
        let text = self.text(i);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        WasmBlock {
            text: text.to_string(),
            x: h.x, y: h.y, z: h.z,
            depth: h.depth,
            layer: layer.to_string(),
            distance: dist,
        }
    }
}

/// Append log entry for WASM (in-memory, no file I/O)
struct WasmAppendEntry {
    text: String,
    layer_id: u8,
    importance: u8,
    depth: u8,
    x: f32, y: f32, z: f32,
}

/// Main WASM interface for Microscope Memory
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct MicroscopeWasm {
    reader: Option<WasmReader>,
    append_entries: Vec<WasmAppendEntry>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl MicroscopeWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console::log_1(&"Microscope Memory WASM v0.2.0 initialized".into());
        Self {
            reader: None,
            append_entries: Vec::new(),
        }
    }

    /// Load from binary buffers (meta.bin, microscope.bin, data.bin as ArrayBuffers)
    #[wasm_bindgen]
    pub fn load_binary(&mut self, meta: &[u8], headers: &[u8], data: &[u8]) -> Result<(), JsValue> {
        match WasmReader::from_buffers(meta, headers.to_vec(), data.to_vec()) {
            Ok(reader) => {
                console::log_1(&format!("Loaded {} blocks", reader.block_count).into());
                self.reader = Some(reader);
                Ok(())
            }
            Err(e) => Err(JsValue::from_str(&e)),
        }
    }

    /// Exact depth search (k-nearest at zoom level)
    #[wasm_bindgen]
    pub fn look(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<WasmBlock> {
        let reader = match &self.reader {
            Some(r) => r,
            None => return vec![],
        };
        reader.look(x, y, z, zoom, k)
            .into_iter()
            .map(|(dist, idx)| reader.to_wasm_block(idx, dist))
            .collect()
    }

    /// 4D soft zoom search (all blocks, zoom as weighted dimension)
    #[wasm_bindgen]
    pub fn look_soft(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<WasmBlock> {
        let reader = match &self.reader {
            Some(r) => r,
            None => return vec![],
        };
        reader.look_soft(x, y, z, zoom, k, 2.0)
            .into_iter()
            .map(|(dist, idx)| reader.to_wasm_block(idx, dist))
            .collect()
    }

    /// Natural language recall (keyword + spatial)
    #[wasm_bindgen]
    pub fn recall(&self, query: &str, k: usize) -> Vec<WasmBlock> {
        let reader = match &self.reader {
            Some(r) => r,
            None => return vec![],
        };
        reader.recall(query, k)
            .into_iter()
            .map(|(dist, idx)| reader.to_wasm_block(idx, dist))
            .collect()
    }

    /// Text search (substring match)
    #[wasm_bindgen]
    pub fn find(&self, query: &str, k: usize) -> Vec<WasmBlock> {
        let reader = match &self.reader {
            Some(r) => r,
            None => return vec![],
        };
        reader.find_text(query, k)
            .into_iter()
            .map(|(_d, idx)| reader.to_wasm_block(idx, 0.0))
            .collect()
    }

    /// Store new memory entry (in-memory append, no file I/O)
    #[wasm_bindgen]
    pub fn store(&mut self, text: &str, layer: &str, importance: u8) {
        let (x, y, z) = content_coords_blended(text, layer, 0.0);
        let layer_id = crate::layer_to_id(layer);
        let depth = if text.len() >= 100 { 3 } else if text.len() >= 40 { 4 } else if text.len() >= 15 { 5 } else { 6 };
        self.append_entries.push(WasmAppendEntry {
            text: text.to_string(),
            layer_id, importance, depth, x, y, z,
        });
    }

    /// Load append log from binary (APv2 format)
    #[wasm_bindgen]
    pub fn load_append(&mut self, data: &[u8]) {
        if data.len() < 4 { return; }
        let is_v2 = &data[0..4] == b"APv2";
        let mut pos = if is_v2 { 4 } else { 0 };
        let hdr_size = if is_v2 { 19 } else { 18 };

        while pos + hdr_size <= data.len() {
            let len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
            let lid = data[pos+4];
            let imp = data[pos+5];
            let (depth, coords_start) = if is_v2 {
                (data[pos+6], pos + 7)
            } else {
                (4u8, pos + 6)
            };
            let x = f32::from_le_bytes(data[coords_start..coords_start+4].try_into().unwrap());
            let y = f32::from_le_bytes(data[coords_start+4..coords_start+8].try_into().unwrap());
            let z = f32::from_le_bytes(data[coords_start+8..coords_start+12].try_into().unwrap());
            pos += hdr_size;
            if pos + len > data.len() { break; }
            let text = String::from_utf8_lossy(&data[pos..pos+len]).to_string();
            pos += len;
            self.append_entries.push(WasmAppendEntry { text, layer_id: lid, importance: imp, depth, x, y, z });
        }
    }

    /// Export append entries as APv2 binary
    #[wasm_bindgen]
    pub fn export_append(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"APv2");
        for entry in &self.append_entries {
            let text_bytes = entry.text.as_bytes();
            let len = text_bytes.len().min(BLOCK_DATA_SIZE);
            buf.extend_from_slice(&(len as u32).to_le_bytes());
            buf.push(entry.layer_id);
            buf.push(entry.importance);
            buf.push(entry.depth);
            buf.extend_from_slice(&entry.x.to_le_bytes());
            buf.extend_from_slice(&entry.y.to_le_bytes());
            buf.extend_from_slice(&entry.z.to_le_bytes());
            buf.extend_from_slice(&text_bytes[..len]);
        }
        buf
    }

    /// Get block count (main index + append)
    #[wasm_bindgen]
    pub fn block_count(&self) -> usize {
        let main = self.reader.as_ref().map(|r| r.block_count).unwrap_or(0);
        main + self.append_entries.len()
    }

    /// Check if binary data is loaded
    #[wasm_bindgen]
    pub fn is_loaded(&self) -> bool {
        self.reader.is_some()
    }
}

/// Initialize WASM module
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    console::log_1(&"Microscope Memory WASM module loaded".into());
}
