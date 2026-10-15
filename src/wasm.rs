//! WebAssembly interface module — runs Microscope Memory in browsers.
//!
//! Uses the real binary format (meta.bin + microscope.bin + data.bin) via
//! ArrayBuffer → &[u8] slices. No file I/O, no mmap — pure owned Vec<u8>.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys::console;

use crate::{
    content_coords_blended, BlockHeader, DEPTH_ENTRY_SIZE, HEADER_SIZE,
    LAYER_NAMES, META_HEADER_SIZE,
};

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
    layer_id: u8,
    importance: u8,
    distance: f32,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmBlock {
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn x(&self) -> f32 {
        self.x
    }
    #[wasm_bindgen(getter)]
    pub fn y(&self) -> f32 {
        self.y
    }
    #[wasm_bindgen(getter)]
    pub fn z(&self) -> f32 {
        self.z
    }
    #[wasm_bindgen(getter)]
    pub fn depth(&self) -> u8 {
        self.depth
    }
    #[wasm_bindgen(getter)]
    pub fn layer(&self) -> String {
        self.layer.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn layer_id(&self) -> u8 {
        self.layer_id
    }
    #[wasm_bindgen(getter)]
    pub fn importance(&self) -> u8 {
        self.importance
    }
    #[wasm_bindgen(getter)]
    pub fn distance(&self) -> f32 {
        self.distance
    }
}

/// Internal reader that owns the binary data (no mmap).
struct WasmReader {
    headers: Vec<u8>,
    data: Vec<u8>,
    block_count: usize,
    depth_ranges: [(u32, u32); 9],
    /// Precomputed lowercase copy of every block's text, built once at load
    /// time. Search hot paths (`find_text` / `recall`) read this instead of
    /// calling `to_lowercase()` on each block on every query.
    search_texts: Vec<String>,
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
            return Err(format!(
                "headers too small: {} < {}",
                headers.len(),
                expected_hdr_size
            ));
        }

        let mut depth_ranges = [(0u32, 0u32); 9];
        for d in 0..9 {
            let off = META_HEADER_SIZE + d * DEPTH_ENTRY_SIZE;
            let start = u32::from_le_bytes(meta[off..off + 4].try_into().unwrap());
            let count = u32::from_le_bytes(meta[off + 4..off + 8].try_into().unwrap());
            depth_ranges[d] = (start, count);
        }

        let mut reader = WasmReader {
            headers,
            data,
            block_count,
            depth_ranges,
            search_texts: Vec::new(),
        };

        // Pre-normalize every block's text to lowercase ONCE, at load time.
        // This is the single biggest search hot-path win: `find_text`/`recall`
        // previously called `to_lowercase()` on each block for every query,
        // which dominated latency (benchmark showed WASM 3.3–3.6× slower than
        // the JS scan). Reusing `text(i)` keeps semantics identical (including
        // the "<oob>"/"<bin>" sentinels for malformed ranges) while the
        // lowercased copy is read once per query thereafter.
        reader.search_texts = (0..reader.block_count)
            .map(|i| reader.text(i).to_lowercase())
            .collect();

        Ok(reader)
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

    /// Lowercased, precomputed copy of block `i`'s text. O(1) lookup — the
    /// lowercasing happened once in `from_buffers`.
    #[inline(always)]
    fn search_text(&self, i: usize) -> &str {
        self.search_texts.get(i).map(|s| s.as_str()).unwrap_or("")
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
            results.push((dx * dx + dy * dy + dz * dz, i));
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

    fn look_soft(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize, zw: f32) -> Vec<(f32, usize)> {
        let qz = zoom as f32 / 8.0;
        let mut results: Vec<(f32, usize)> = (0..self.block_count)
            .map(|i| {
                let h = self.header(i);
                let dx = h.x - x;
                let dy = h.y - y;
                let dz = h.z - z;
                let dw = (h.zoom - qz) * zw;
                (dx * dx + dy * dy + dz * dz + dw * dw, i)
            })
            .collect();
        let k = k.min(results.len());
        if k == 0 {
            return vec![];
        }
        results.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
        results.truncate(k);
        results.sort_by(|a, b| a.0.total_cmp(&b.0));
        results
    }

    fn find_text(&self, query: &str, k: usize) -> Vec<(u8, usize)> {
        let q = query.to_lowercase();
        let mut results: Vec<(u8, usize)> = (0..self.block_count)
            .filter_map(|i| {
                if self.search_text(i).contains(&q) {
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
        let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

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
                let st = self.search_text(i);
                let keyword_hits = keywords.iter().filter(|&&kw| st.contains(kw)).count();
                if keyword_hits > 0 {
                    let h = self.header(i);
                    let dx = h.x - qx;
                    let dy = h.y - qy;
                    let dz = h.z - qz;
                    let spatial = dx * dx + dy * dy + dz * dz;
                    let boost = keyword_hits as f32 * 0.1;
                    results.push(((spatial - boost).max(0.0), i));
                }
            }
        }

        results.sort_by(|a, b| a.0.total_cmp(&b.0));
        results.truncate(k);
        results
    }

    /// Count substring matches across all blocks (uncapped by `k`). Used by
    /// `MicroscopeWasm::find_count` to avoid allocating `WasmBlock`s.
    fn find_text_count(&self, query: &str) -> usize {
        let q = query.to_lowercase();
        (0..self.block_count)
            .filter(|&i| self.search_text(i).contains(&q))
            .count()
    }

    /// Count keyword-match recall hits within the zoom-range window.
    fn recall_count(&self, query: &str) -> usize {
        let q_lower = query.to_lowercase();
        let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();
        if keywords.is_empty() {
            return 0;
        }
        let zoom_range = match query.len() {
            0..=10 => (0u8, 3u8),
            11..=40 => (3, 6),
            _ => (6, 8),
        };
        let mut hits = 0usize;
        for zoom in zoom_range.0..=zoom_range.1 {
            let (start, count) = self.depth_ranges[zoom as usize];
            let (start, count) = (start as usize, count as usize);
            for i in start..(start + count) {
                let st = self.search_text(i);
                if keywords.iter().any(|kw| st.contains(*kw)) {
                    hits += 1;
                }
            }
        }
        hits
    }

    fn to_wasm_block(&self, i: usize, dist: f32) -> WasmBlock {
        let h = self.header(i);
        let text = self.text(i);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        WasmBlock {
            text: text.to_string(),
            x: h.x,
            y: h.y,
            z: h.z,
            depth: h.depth,
            layer: layer.to_string(),
            layer_id: h.layer_id,
            importance: h.importance,
            distance: dist,
        }
    }
}

/// Append log entry for WASM (in-memory, no file I/O)
struct WasmAppendEntry {
    text: String,
    /// Lowercased copy of `text`, populated in `store`/`load_append` and used by
    /// `find_append`/`recall_append`/`find_append_count`/`recall_append_count`.
    search_text: String,
    layer_id: u8,
    importance: u8,
    depth: u8,
    x: f32,
    y: f32,
    z: f32,
}

impl WasmAppendEntry {
    /// Borrow the lowercased searchable text (cheap, no allocation).
    #[inline(always)]
    fn search_text(&self) -> &str {
        &self.search_text
    }
}

/// Bridge from an in-memory append entry to a JS-visible `WasmBlock`.
#[cfg(target_arch = "wasm32")]
impl WasmAppendEntry {
    fn to_wasm_block(&self, dist: f32) -> WasmBlock {
        let layer = LAYER_NAMES.get(self.layer_id as usize).unwrap_or(&"?");
        WasmBlock {
            text: self.text.clone(),
            x: self.x,
            y: self.y,
            z: self.z,
            depth: self.depth,
            layer: layer.to_string(),
            layer_id: self.layer_id,
            importance: self.importance,
            distance: dist,
        }
    }
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

    /// Exact depth search (k-nearest at zoom level) — searches both the main
    /// index blocks AND in-memory append entries.
    #[wasm_bindgen]
    pub fn look(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<WasmBlock> {
        if k == 0 {
            return vec![];
        }
        // Collect candidate (distance, block) pairs from both sources.
        let mut top: Vec<(f32, WasmBlock)> = Vec::with_capacity(k * 2);
        if let Some(reader) = &self.reader {
            for (dist, idx) in reader.look(x, y, z, zoom, k) {
                top.push((dist, reader.to_wasm_block(idx, dist)));
            }
        }
        if !self.append_entries.is_empty() {
            for entry in &self.append_entries {
                let dx = entry.x - x;
                let dy = entry.y - y;
                let dz = entry.z - z;
                let dist = dx * dx + dy * dy + dz * dz;
                top.push((dist, entry.to_wasm_block(dist)));
            }
        }
        if top.is_empty() {
            return vec![];
        }
        let k = k.min(top.len());
        if k < top.len() {
            top.select_nth_unstable_by(k, |a, b| a.0.total_cmp(&b.0));
        }
        top.truncate(k);
        top.sort_by(|a, b| a.0.total_cmp(&b.0));
        top.into_iter().map(|(_, b)| b).collect()
    }

    /// 4D soft zoom search (all blocks, zoom as weighted dimension) —
    /// searches both the main index AND in-memory append entries.
    #[wasm_bindgen]
    pub fn look_soft(&self, x: f32, y: f32, z: f32, zoom: u8, k: usize) -> Vec<WasmBlock> {
        if k == 0 {
            return vec![];
        }
        let qz = zoom.min(8) as f32 / 8.0;
        let mut top: Vec<(f32, WasmBlock)> = Vec::with_capacity(k * 2);
        if let Some(reader) = &self.reader {
            for (dist, idx) in reader.look_soft(x, y, z, zoom, k, 2.0) {
                top.push((dist, reader.to_wasm_block(idx, dist)));
            }
        }
        if !self.append_entries.is_empty() {
            for entry in &self.append_entries {
                let dx = entry.x - x;
                let dy = entry.y - y;
                let dz = entry.z - z;
                let dw = (entry.depth as f32 / 8.0 - qz) * 2.0;
                let dist = dx * dx + dy * dy + dz * dz + dw * dw;
                top.push((dist, entry.to_wasm_block(dist)));
            }
        }
        if top.is_empty() {
            return vec![];
        }
        let k = k.min(top.len());
        if k < top.len() {
            top.select_nth_unstable_by(k, |a, b| a.0.total_cmp(&b.0));
        }
        top.truncate(k);
        top.sort_by(|a, b| a.0.total_cmp(&b.0));
        top.into_iter().map(|(_, b)| b).collect()
    }

    /// Natural language recall (keyword + spatial) — searches both the main
    /// index blocks AND in-memory append entries.
    #[wasm_bindgen]
    pub fn recall(&self, query: &str, k: usize) -> Vec<WasmBlock> {
        if k == 0 {
            return vec![];
        }
        let mut top: Vec<(f32, WasmBlock)> = Vec::new();
        if let Some(reader) = &self.reader {
            for (dist, idx) in reader.recall(query, k) {
                top.push((dist, reader.to_wasm_block(idx, dist)));
            }
        }
        for (dist, idx) in self.recall_append(query, k) {
            top.push((dist, self.append_entries[idx].to_wasm_block(dist)));
        }
        if top.is_empty() {
            return vec![];
        }
        let k = k.min(top.len());
        if k < top.len() {
            top.select_nth_unstable_by(k, |a, b| a.0.total_cmp(&b.0));
        }
        top.truncate(k);
        top.sort_by(|a, b| a.0.total_cmp(&b.0));
        top.into_iter().map(|(_, b)| b).collect()
    }

    /// Text search (substring match) — searches both the main index blocks
    /// and in-memory append entries. Results are sorted by depth (shallow
    /// first) and truncated to `k`.
    #[wasm_bindgen]
    pub fn find(&self, query: &str, k: usize) -> Vec<WasmBlock> {
        if k == 0 {
            return vec![];
        }
        let mut blocks: Vec<WasmBlock> = Vec::with_capacity(k * 2);
        if let Some(reader) = &self.reader {
            for (_d, idx) in reader.find_text(query, k) {
                blocks.push(reader.to_wasm_block(idx, 0.0));
            }
        }
        for idx in self.find_append(query, k) {
            blocks.push(self.append_entries[idx].to_wasm_block(0.0));
        }
        if blocks.is_empty() {
            return vec![];
        }
        blocks.sort_by_key(|b| b.depth);
        blocks.truncate(k);
        blocks
    }

    /// Count substring matches across the main index + append — the TRUE total
    /// match count, uncapped by `k` (a count must never hide hits behind a
    /// result-page limit). Allocates no `WasmBlock`s. Used by the search UI to
    /// show an accurate hit count before deciding whether to materialize results
    /// via `find`, which (deliberately) truncates to `k`.
    #[wasm_bindgen]
    pub fn find_count(&self, query: &str, _k: usize) -> usize {
        let mut total = 0usize;
        if let Some(reader) = &self.reader {
            total += reader.find_text_count(query);
        }
        total += self.find_append_count(query);
        total
    }

    /// Count recall (keyword) matches across the main index + append — the
    /// TRUE total, uncapped by `k`. Allocates no `WasmBlock`s.
    #[wasm_bindgen]
    pub fn recall_count(&self, query: &str, _k: usize) -> usize {
        let mut total = 0usize;
        if let Some(reader) = &self.reader {
            total += reader.recall_count(query);
        }
        total += self.recall_append_count(query);
        total
    }

    /// Paginated substring search. Returns matches ranked by depth
    /// (shallow-first), the slice `[offset, offset + limit)` of the full
    /// match ranking. Only `offset + limit` candidates are materialized per
    /// source, so the UI can page through large result sets without marshaling
    /// the full list back into JS (`find_count` still returns the uncapped total).
    #[wasm_bindgen]
    pub fn find_slice(&self, query: &str, offset: usize, limit: usize) -> Vec<WasmBlock> {
        if limit == 0 {
            return vec![];
        }
        let need = offset + limit;
        let mut blocks: Vec<WasmBlock> = Vec::with_capacity(need * 2);
        if let Some(reader) = &self.reader {
            for (_d, idx) in reader.find_text(query, need) {
                blocks.push(reader.to_wasm_block(idx, 0.0));
            }
        }
        for idx in self.find_append(query, need) {
            blocks.push(self.append_entries[idx].to_wasm_block(0.0));
        }
        if blocks.is_empty() {
            return vec![];
        }
        blocks.sort_by_key(|b| b.depth);
        let start = offset.min(blocks.len());
        let end = (start + limit).min(blocks.len());
        if start >= end {
            return vec![];
        }
        // `WasmBlock` is move-only (no `Clone`): drain moves only the kept
        // slice out; the matching candidates are freed when `blocks` drops.
        blocks.drain(start..end).collect()
    }

    /// Paginated keyword + spatial recall. Returns matches ranked by inverse
    /// distance (nearest-first), the slice `[offset, offset + limit)`, using a
    /// partial selection so only `offset + limit` matches are materialized.
    #[wasm_bindgen]
    pub fn recall_slice(&self, query: &str, offset: usize, limit: usize) -> Vec<WasmBlock> {
        if limit == 0 {
            return vec![];
        }
        let need = offset + limit;
        let mut top: Vec<(f32, WasmBlock)> = Vec::with_capacity(need * 2);
        if let Some(reader) = &self.reader {
            for (dist, idx) in reader.recall(query, need) {
                top.push((dist, reader.to_wasm_block(idx, dist)));
            }
        }
        for (dist, idx) in self.recall_append(query, need) {
            top.push((dist, self.append_entries[idx].to_wasm_block(dist)));
        }
        if top.is_empty() {
            return vec![];
        }
        let k = need.min(top.len());
        if k < top.len() {
            top.select_nth_unstable_by(k, |a, b| a.0.total_cmp(&b.0));
        }
        top.truncate(k);
        top.sort_by(|a, b| a.0.total_cmp(&b.0));
        let start = offset.min(top.len());
        let end = (start + limit).min(top.len());
        if start >= end {
            return vec![];
        }
        top.drain(start..end).map(|(_, b)| b).collect()
    }

    /// Keyword + spatial search over in-memory append entries. Returns up to
    /// `k` (distance, index) pairs sorted nearest-first.
    fn recall_append(&self, query: &str, k: usize) -> Vec<(f32, usize)> {
        if k == 0 || self.append_entries.is_empty() {
            return Vec::new();
        }
        let (qx, qy, qz) = content_coords_blended(query, "long_term", 0.0);
        let q_lower = query.to_lowercase();
        let keywords: Vec<&str> = q_lower
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .collect();
        if keywords.is_empty() {
            return Vec::new();
        }
        let mut results: Vec<(f32, usize)> = Vec::new();
        for (index, entry) in self.append_entries.iter().enumerate() {
            let keyword_hits = keywords
                .iter()
                .filter(|&&word| entry.search_text().contains(word))
                .count();
            if keyword_hits == 0 {
                continue;
            }
            let dx = entry.x - qx;
            let dy = entry.y - qy;
            let dz = entry.z - qz;
            let spatial = dx * dx + dy * dy + dz * dz;
            let boost = keyword_hits as f32 * 0.1;
            results.push(((spatial - boost).max(0.0), index));
        }
        if results.len() > k {
            results.select_nth_unstable_by(k, |a, b| {
                a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1))
            });
            results.truncate(k);
        }
        results.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        results
    }

    /// Count recall (keyword) hits over in-memory append entries.
    fn recall_append_count(&self, query: &str) -> usize {
        if self.append_entries.is_empty() {
            return 0;
        }
        let q_lower = query.to_lowercase();
        let keywords: Vec<&str> = q_lower
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .collect();
        if keywords.is_empty() {
            return 0;
        }
        self.append_entries
            .iter()
            .filter(|entry| keywords.iter().any(|kw| entry.search_text().contains(*kw)))
            .count()
    }

    /// Substring match over in-memory append entries. Returns up to `k` indices
    /// sorted by depth (shallow first, then insertion order).
    fn find_append(&self, query: &str, k: usize) -> Vec<usize> {
        let q = query.to_lowercase();
        let mut results: Vec<usize> = self
            .append_entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.search_text().contains(&q).then_some(index))
            .collect();
        if results.len() > k {
            results.select_nth_unstable_by_key(k, |&index| {
                (self.append_entries[index].depth, index)
            });
            results.truncate(k);
        }
        results.sort_unstable_by_key(|&index| (self.append_entries[index].depth, index));
        results
    }

    /// Count substring matches over in-memory append entries.
    fn find_append_count(&self, query: &str) -> usize {
        if self.append_entries.is_empty() {
            return 0;
        }
        let q = query.to_lowercase();
        self.append_entries
            .iter()
            .filter(|entry| entry.search_text().contains(&q))
            .count()
    }

    /// Store new memory entry (in-memory append, no file I/O)
    #[wasm_bindgen]
    pub fn store(&mut self, text: &str, layer: &str, importance: u8) {
        let (x, y, z) = content_coords_blended(text, layer, 0.0);
        let layer_id = crate::layer_to_id(layer);
        let depth = if text.len() >= 100 {
            3
        } else if text.len() >= 40 {
            4
        } else if text.len() >= 15 {
            5
        } else {
            6
        };
        self.append_entries.push(WasmAppendEntry {
            text: text.to_string(),
            search_text: text.to_lowercase(),
            layer_id,
            importance,
            depth,
            x,
            y,
            z,
        });
    }

    /// Load append log from binary (APv2 format)
    #[wasm_bindgen]
    pub fn load_append(&mut self, data: &[u8]) {
        if data.len() < 4 {
            return;
        }
        let is_v2 = &data[0..4] == b"APv2";
        let mut pos = if is_v2 { 4 } else { 0 };
        let hdr_size = if is_v2 { 19 } else { 18 };

        while pos + hdr_size <= data.len() {
            let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            let lid = data[pos + 4];
            let imp = data[pos + 5];
            let (depth, coords_start) = if is_v2 {
                (data[pos + 6], pos + 7)
            } else {
                (4u8, pos + 6)
            };
            let x = f32::from_le_bytes(data[coords_start..coords_start + 4].try_into().unwrap());
            let y =
                f32::from_le_bytes(data[coords_start + 4..coords_start + 8].try_into().unwrap());
            let z = f32::from_le_bytes(
                data[coords_start + 8..coords_start + 12]
                    .try_into()
                    .unwrap(),
            );
            pos += hdr_size;
            if pos + len > data.len() {
                break;
            }
            let text = String::from_utf8_lossy(&data[pos..pos + len]).to_string();
            let search_text = text.to_lowercase();
            pos += len;
            self.append_entries.push(WasmAppendEntry {
                text,
                search_text,
                layer_id: lid,
                importance: imp,
                depth,
                x,
                y,
                z,
            });
        }
    }

    /// Export append entries as APv2 binary
    #[wasm_bindgen]
    pub fn export_append(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"APv2");
        for entry in &self.append_entries {
            let text_bytes = entry.text.as_bytes();
            let len = text_bytes.len();
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

// ═══════════════════════════════════════════════════════
//  COGNITIVE CLI — WASM parancsok
// ═══════════════════════════════════════════════════════

/// Virtual CLI executor — executes cognitive commands in WASM.
///
/// Commands: status, recall <query> [k], store <text>, stats, doctor, intent, absentia
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct HopeCli {
    blocks: Vec<(String, String, u8, u8)>, // (text, layer, importance, depth)
    hebbian_pairs: Vec<(u32, u32, f32)>,   // (block_a, block_b, strength)
    audit_log: Vec<String>,
    intent_history: Vec<String>,
    absentia_records: Vec<String>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl HopeCli {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console::log_1(&"HOPE CLI WASM initialized".into());
        Self {
            blocks: Vec::new(),
            hebbian_pairs: Vec::new(),
            audit_log: Vec::new(),
            intent_history: Vec::new(),
            absentia_records: Vec::new(),
        }
    }

    /// Execute a virtual CLI command and return JSON result
    #[wasm_bindgen]
    pub fn exec(&mut self, command: &str) -> String {
        let parts: Vec<&str> = command.splitn(3, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            String::new()
        };

        let result = match cmd.as_str() {
            "status" => self.cmd_status(),
            "stats" => self.cmd_stats(),
            "recall" | "r" => self.cmd_recall(&args),
            "store" | "s" => self.cmd_store(&args),
            "find" | "f" => self.cmd_find(&args),
            "doctor" => self.cmd_doctor(),
            "intent" => self.cmd_intent(&args),
            "absentia" => self.cmd_absentia(&args),
            "hebbian" => self.cmd_hebbian(),
            "audit" => self.cmd_audit(&args),
            "genome" => self.cmd_genome(),
            "permissions" => self.cmd_permissions(),
            "help" | "?" => self.cmd_help(),
            _ => format!("{{\"ok\":false,\"error\":\"Unknown command: {}\"}}", cmd),
        };

        self.audit_log.push(format!(
            "{} → {}",
            command,
            &result[..result.len().min(100)]
        ));
        if self.audit_log.len() > 500 {
            self.audit_log.drain(0..self.audit_log.len() - 500);
        }

        result
    }

    /// Load blocks from binary data (for integration with MicroscopeWasm)
    #[wasm_bindgen]
    pub fn load_blocks(&mut self, texts: &str) {
        // texts format: "text1\0layer1\0importance1\0text2\0layer2\0importance2\0..."
        let parts: Vec<&str> = texts.split('\0').collect();
        let mut i = 0;
        while i + 2 < parts.len() {
            let text = parts[i].to_string();
            let layer = parts[i + 1].to_string();
            let importance: u8 = parts[i + 2].parse().unwrap_or(5);
            let depth = 3; // default
            self.blocks.push((text, layer, importance, depth));
            i += 3;
        }
    }

    fn cmd_status(&self) -> String {
        format!(
            "{{\"ok\":true,\"status\":{{\"blocks\":{},\"hebbian_pairs\":{},\"audit_entries\":{},\"intent_history\":{},\"absentia_records\":{}}}}}",
            self.blocks.len(),
            self.hebbian_pairs.len(),
            self.audit_log.len(),
            self.intent_history.len(),
            self.absentia_records.len(),
        )
    }

    fn cmd_stats(&self) -> String {
        format!(
            "{{\"ok\":true,\"stats\":{{\"blocks\":{},\"domain\":\"wasm\",\"wasm\":true,\"opfs\":false,\"hebbian_pairs\":{},\"audit_entries\":{}}}}}",
            self.blocks.len(),
            self.hebbian_pairs.len(),
            self.audit_log.len(),
        )
    }

    fn cmd_recall(&self, args: &str) -> String {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let query = parts[0].to_lowercase();
        let k: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);

        if query.is_empty() {
            return "{\"ok\":false,\"error\":\"Usage: recall <query> [k]\"}".to_string();
        }

        let mut scored: Vec<(usize, f32)> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, (text, _, _, _))| {
                let t = text.to_lowercase();
                let mut s = 0.0f32;
                if t.contains(&query) {
                    s += 3.0;
                }
                for word in query.split_whitespace() {
                    if word.len() > 2 && t.contains(word) {
                        s += 1.0;
                    }
                }
                (i, s)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(k);

        let results: Vec<String> = scored
            .iter()
            .map(|(i, s)| {
                let (text, layer, imp, _) = &self.blocks[*i];
                format!(
                    "{{\"text\":\"{}\",\"layer\":\"{}\",\"importance\":{},\"score\":{}}}",
                    text.chars()
                        .take(200)
                        .collect::<String>()
                        .replace('"', "\\\""),
                    layer,
                    imp,
                    s
                )
            })
            .collect();

        format!(
            "{{\"ok\":true,\"query\":\"{}\",\"count\":{},\"results\":[{}]}}",
            query,
            results.len(),
            results.join(",")
        )
    }

    fn cmd_store(&mut self, args: &str) -> String {
        let parts: Vec<&str> = args.splitn(3, ' ').collect();
        if parts.is_empty() || parts[0].is_empty() {
            return "{\"ok\":false,\"error\":\"Usage: store <text> [layer] [importance]\"}"
                .to_string();
        }
        let text = parts[0].to_string();
        let layer = parts.get(1).unwrap_or(&"session").to_string();
        let importance: u8 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
        let depth = 3u8;

        // Hebbian: associate with recent blocks
        let new_id = self.blocks.len() as u32;
        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        for (i, (existing_text, _, _, _)) in self.blocks.iter().enumerate() {
            let existing_lower = existing_text.to_lowercase();
            for word in &words {
                if existing_lower.contains(word) {
                    self.hebbian_pairs.push((new_id, i as u32, 0.15));
                    break;
                }
            }
        }

        self.blocks
            .push((text.clone(), layer.clone(), importance, depth));
        format!(
            "{{\"ok\":true,\"id\":\"m{}\",\"text\":\"{}\",\"layer\":\"{}\",\"importance\":{}}}",
            new_id,
            text.chars()
                .take(100)
                .collect::<String>()
                .replace('"', "\\\""),
            layer,
            importance
        )
    }

    fn cmd_find(&self, args: &str) -> String {
        self.cmd_recall(args)
    }

    fn cmd_doctor(&self) -> String {
        let checks = vec![
            format!(
                "{{\"name\":\"memory\",\"ok\":true,\"detail\":\"{} blocks\"}}",
                self.blocks.len()
            ),
            format!(
                "{{\"name\":\"hebbian\",\"ok\":true,\"detail\":\"{} pairs\"}}",
                self.hebbian_pairs.len()
            ),
            format!(
                "{{\"name\":\"audit\",\"ok\":true,\"detail\":\"{} entries\"}}",
                self.audit_log.len()
            ),
            format!("{{\"name\":\"wasm\",\"ok\":true,\"detail\":\"running\"}}"),
        ];
        format!(
            "{{\"ok\":true,\"doctor\":{{\"healthy\":true,\"checks\":[{}]}}}}",
            checks.join(",")
        )
    }

    fn cmd_intent(&mut self, args: &str) -> String {
        let action = args.split_whitespace().next().unwrap_or("generate");
        match action {
            "generate" => {
                // Simple intent based on absence
                let has_blocks = !self.blocks.is_empty();
                let (intent_action, strength) = if !has_blocks {
                    ("ASK", 0.8)
                } else {
                    ("OBSERVE", 0.1)
                };
                let intent = format!(
                    "{{\"action\":\"{}\",\"strength\":{},\"allowed\":true,\"requires_approval\":false,\"audit_steps\":3,\"rationale\":[\"WASM cognitive runtime\"]}}",
                    intent_action, strength
                );
                self.intent_history.push(intent.clone());
                format!("{{\"ok\":true,\"intent\":{}}}", intent)
            }
            "audit" => {
                let entries: Vec<String> = self
                    .intent_history
                    .iter()
                    .rev()
                    .take(10)
                    .map(|e| e.clone())
                    .collect();
                format!("{{\"ok\":true,\"audit\":[{}]}}", entries.join(","))
            }
            _ => "{\"ok\":false,\"error\":\"Usage: intent [generate|audit]\"}".to_string(),
        }
    }

    fn cmd_absentia(&mut self, args: &str) -> String {
        let action = args.split_whitespace().next().unwrap_or("status");
        match action {
            "status" => {
                format!("{{\"ok\":true,\"absentia\":{{\"blocks\":{},\"hebbian_pairs\":{},\"audit_entries\":{}}}}}",
                    self.blocks.len(), self.hebbian_pairs.len(), self.audit_log.len())
            }
            "scan" => {
                // Find blocks older than 24h (simplified)
                let count = self.blocks.len();
                format!("{{\"ok\":true,\"scan\":{{\"block_count\":{},\"hebbian_pairs\":{},\"absence_records\":0}}}}", count, self.hebbian_pairs.len())
            }
            _ => "{\"ok\":false,\"error\":\"Usage: absentia [status|scan]\"}".to_string(),
        }
    }

    fn cmd_hebbian(&self) -> String {
        let pairs: Vec<String> = self
            .hebbian_pairs
            .iter()
            .rev()
            .take(20)
            .map(|(a, b, s)| format!("{{\"from\":{},\"to\":{},\"strength\":{}}}", a, b, s))
            .collect();
        format!(
            "{{\"ok\":true,\"total_pairs\":{},\"top_pairs\":[{}]}}",
            self.hebbian_pairs.len(),
            pairs.join(",")
        )
    }

    fn cmd_audit(&self, args: &str) -> String {
        let n: usize = args
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        let entries: Vec<String> = self
            .audit_log
            .iter()
            .rev()
            .take(n)
            .map(|e| format!("\"{}\"", e.replace('"', "\\\"")))
            .collect();
        format!("{{\"ok\":true,\"audit\":[{}]}}", entries.join(","))
    }

    fn cmd_genome(&self) -> String {
        "{\"ok\":true,\"genome\":{\"identity\":\"HOPE\",\"mission\":\"Segíteni a felhasználónak a gondolkodásban.\",\"values\":[\"Auditálhatóság\",\"Transzparencia\",\"Biztonság\",\"Emberi kontroll\"],\"constraints\":[{\"name\":\"no_autonomous_action\",\"severity\":\"ABSOLUTE\"}]}}".to_string()
    }

    fn cmd_permissions(&self) -> String {
        "{\"ok\":true,\"permissions\":{\"capabilities\":[\"recall\",\"store\",\"suggest\",\"ask\"],\"constraints\":[{\"name\":\"no_autonomous_action\",\"severity\":\"ABSOLUTE\"}]}}".to_string()
    }

    fn cmd_help(&self) -> String {
        "{\"ok\":true,\"commands\":{\"Core\":[\"status\",\"stats\",\"doctor\",\"help\"],\"Memory\":[\"recall <query> [k]\",\"store <text> [layer] [importance]\",\"find <pattern>\"],\"Intent\":[\"intent generate\",\"intent audit\"],\"Absentia\":[\"absentia status\",\"absentia scan\"],\"Hebbian\":[\"hebbian\"],\"Audit\":[\"audit [n]\"],\"Genome\":[\"genome\"],\"Permissions\":[\"permissions\"]}}".to_string()
    }
}
