// WebAssembly interface module
// Allows running Microscope Memory in browsers

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys::console;

use crate::embeddings::{MockEmbeddingProvider, EmbeddingProvider, cosine_similarity_simd};

/// JavaScript-accessible block structure
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmBlock {
    text: String,
    x: f32,
    y: f32,
    z: f32,
    depth: u8,
    similarity: f32,
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
    pub fn similarity(&self) -> f32 {
        self.similarity
    }
}

/// Main WASM interface for Microscope Memory
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct MicroscopeWasm {
    blocks: Vec<InternalBlock>,
    embeddings: Vec<Vec<f32>>,
    provider: MockEmbeddingProvider,
}

struct InternalBlock {
    text: String,
    x: f32,
    y: f32,
    z: f32,
    depth: u8,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl MicroscopeWasm {
    /// Create new instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console::log_1(&"Microscope Memory WASM initialized".into());

        Self {
            blocks: Vec::new(),
            embeddings: Vec::new(),
            provider: MockEmbeddingProvider::new(128),
        }
    }

    /// Add a block
    #[wasm_bindgen]
    pub fn add_block(&mut self, text: &str, x: f32, y: f32, z: f32, depth: u8) {
        let block = InternalBlock {
            text: text.to_string(),
            x, y, z, depth,
        };

        // Generate embedding
        if let Ok(embedding) = self.provider.embed(text) {
            self.embeddings.push(embedding);
        } else {
            self.embeddings.push(vec![0.0; 128]);
        }

        self.blocks.push(block);

        console::log_1(&format!("Added block at ({},{},{})", x, y, z).into());
    }

    /// Semantic search
    #[wasm_bindgen]
    pub fn semantic_search(&self, query: &str, k: usize) -> Vec<WasmBlock> {
        console::log_1(&format!("Searching for: {}", query).into());

        let query_embedding = match self.provider.embed(query) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();

        for (i, block) in self.blocks.iter().enumerate() {
            if i < self.embeddings.len() {
                let similarity = cosine_similarity_simd(&query_embedding, &self.embeddings[i]);
                results.push((similarity, i));
            }
        }

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.truncate(k);

        results.into_iter().map(|(sim, idx)| {
            let block = &self.blocks[idx];
            WasmBlock {
                text: block.text.clone(),
                x: block.x,
                y: block.y,
                z: block.z,
                depth: block.depth,
                similarity: sim,
            }
        }).collect()
    }

    /// Spatial search (L2 distance)
    #[wasm_bindgen]
    pub fn spatial_search(&self, x: f32, y: f32, z: f32, radius: f32) -> Vec<WasmBlock> {
        let mut results = Vec::new();

        for block in &self.blocks {
            let dx = block.x - x;
            let dy = block.y - y;
            let dz = block.z - z;
            let dist = (dx*dx + dy*dy + dz*dz).sqrt();

            if dist <= radius {
                results.push(WasmBlock {
                    text: block.text.clone(),
                    x: block.x,
                    y: block.y,
                    z: block.z,
                    depth: block.depth,
                    similarity: 1.0 - (dist / radius),
                });
            }
        }

        results
    }

    /// Get block count
    #[wasm_bindgen]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Clear all blocks
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.embeddings.clear();
        console::log_1(&"Cleared all blocks".into());
    }

    /// Load from JSON
    #[wasm_bindgen]
    pub fn load_json(&mut self, json: &str) -> Result<(), JsValue> {
        // Parse JSON and load blocks
        // This would integrate with serde_json in real implementation
        console::log_1(&format!("Loading JSON data: {} bytes", json.len()).into());
        Ok(())
    }

    /// Export to JSON
    #[wasm_bindgen]
    pub fn export_json(&self) -> String {
        // Export blocks to JSON
        // This would use serde_json in real implementation
        format!("{{\"blocks\": {}}}", self.blocks.len())
    }
}

/// Initialize WASM module
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    console::log_1(&"Microscope Memory WASM module loaded".into());
}

/// Panic hook for better error messages
#[cfg(target_arch = "wasm32")]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}