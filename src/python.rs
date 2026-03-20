// Python bindings module using PyO3
// Allows using Microscope Memory from Python

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
use pyo3::types::PyList;

use crate::embeddings::{MockEmbeddingProvider, EmbeddingProvider, cosine_similarity_simd};

#[cfg(feature = "python")]
#[pyclass]
pub struct PyMicroscope {
    blocks: Vec<PyBlock>,
    embeddings: Vec<Vec<f32>>,
    provider: MockEmbeddingProvider,
}

#[cfg(feature = "python")]
#[pyclass]
#[derive(Clone)]
pub struct PyBlock {
    #[pyo3(get, set)]
    pub text: String,
    #[pyo3(get, set)]
    pub x: f32,
    #[pyo3(get, set)]
    pub y: f32,
    #[pyo3(get, set)]
    pub z: f32,
    #[pyo3(get, set)]
    pub depth: u8,
    #[pyo3(get, set)]
    pub layer_id: u8,
    #[pyo3(get, set)]
    pub similarity: f32,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyMicroscope {
    #[new]
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            embeddings: Vec::new(),
            provider: MockEmbeddingProvider::new(128),
        }
    }

    /// Add a block to the index
    pub fn add_block(&mut self, text: String, x: f32, y: f32, z: f32, depth: u8, layer_id: u8) {
        let block = PyBlock {
            text: text.clone(),
            x, y, z, depth, layer_id,
            similarity: 0.0,
        };

        // Generate embedding
        if let Ok(embedding) = self.provider.embed(&text) {
            self.embeddings.push(embedding);
        } else {
            self.embeddings.push(vec![0.0; 128]);
        }

        self.blocks.push(block);
    }

    /// Semantic search using embeddings
    pub fn semantic_search(&self, query: String, k: usize, metric: Option<String>) -> PyResult<Vec<PyBlock>> {
        let metric = metric.unwrap_or_else(|| "cosine".to_string());

        let query_embedding = self.provider.embed(&query)
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to generate embedding"))?;

        let mut results = Vec::new();

        for (i, block) in self.blocks.iter().enumerate() {
            if i < self.embeddings.len() {
                let similarity = match metric.as_str() {
                    "cosine" => cosine_similarity_simd(&query_embedding, &self.embeddings[i]),
                    "dot" => query_embedding.iter()
                        .zip(self.embeddings[i].iter())
                        .map(|(a, b)| a * b)
                        .sum(),
                    "l2" => {
                        let dist: f32 = query_embedding.iter()
                            .zip(self.embeddings[i].iter())
                            .map(|(a, b)| (a - b).powi(2))
                            .sum::<f32>()
                            .sqrt();
                        1.0 / (1.0 + dist)
                    },
                    _ => cosine_similarity_simd(&query_embedding, &self.embeddings[i]),
                };

                if similarity > 0.3 {  // Threshold
                    let mut result_block = block.clone();
                    result_block.similarity = similarity;
                    results.push((similarity, result_block));
                }
            }
        }

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.truncate(k);

        Ok(results.into_iter().map(|(_, b)| b).collect())
    }

    /// Spatial search (L2 distance in 3D space)
    pub fn spatial_search(&self, x: f32, y: f32, z: f32, radius: f32, depth: Option<u8>) -> PyResult<Vec<PyBlock>> {
        let mut results = Vec::new();

        for block in &self.blocks {
            // Filter by depth if specified
            if let Some(d) = depth {
                if block.depth != d {
                    continue;
                }
            }

            let dx = block.x - x;
            let dy = block.y - y;
            let dz = block.z - z;
            let dist = (dx*dx + dy*dy + dz*dz).sqrt();

            if dist <= radius {
                let mut result_block = block.clone();
                result_block.similarity = 1.0 - (dist / radius);
                results.push(result_block);
            }
        }

        // Sort by distance (stored in similarity field)
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());

        Ok(results)
    }

    /// Hybrid search combining semantic and spatial
    pub fn hybrid_search(&self, query: String, x: f32, y: f32, z: f32,
                          semantic_weight: f32, spatial_weight: f32, k: usize) -> PyResult<Vec<PyBlock>> {

        let query_embedding = self.provider.embed(&query)
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to generate embedding"))?;

        let mut results = Vec::new();

        for (i, block) in self.blocks.iter().enumerate() {
            if i < self.embeddings.len() {
                // Semantic similarity
                let semantic_sim = cosine_similarity_simd(&query_embedding, &self.embeddings[i]);

                // Spatial similarity
                let dx = block.x - x;
                let dy = block.y - y;
                let dz = block.z - z;
                let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                let spatial_sim = 1.0 / (1.0 + dist);

                // Combined score
                let combined = semantic_weight * semantic_sim + spatial_weight * spatial_sim;

                if combined > 0.1 {
                    let mut result_block = block.clone();
                    result_block.similarity = combined;
                    results.push((combined, result_block));
                }
            }
        }

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.truncate(k);

        Ok(results.into_iter().map(|(_, b)| b).collect())
    }

    /// Get block count
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Clear all blocks
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.embeddings.clear();
    }

    /// Load blocks from list
    pub fn load_blocks(&mut self, blocks: &PyList) -> PyResult<()> {
        self.clear();

        for item in blocks.iter() {
            if let Ok(block) = item.extract::<(String, f32, f32, f32, u8, u8)>() {
                self.add_block(block.0, block.1, block.2, block.3, block.4, block.5);
            }
        }

        Ok(())
    }

    /// Export blocks as list
    pub fn export_blocks(&self) -> Vec<(String, f32, f32, f32, u8, u8)> {
        self.blocks.iter().map(|b| {
            (b.text.clone(), b.x, b.y, b.z, b.depth, b.layer_id)
        }).collect()
    }

    /// Get statistics
    pub fn stats(&self) -> PyResult<String> {
        Ok(format!(
            "Blocks: {}\nEmbedding dimension: 128\nMemory usage: ~{} KB",
            self.blocks.len(),
            (self.blocks.len() * (256 + 128 * 4)) / 1024
        ))
    }
}

/// Python module definition
#[cfg(feature = "python")]
#[pymodule]
fn microscope_memory(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyMicroscope>()?;
    m.add_class::<PyBlock>()?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}

#[cfg(feature = "python")]
/// Helper function to create NumPy arrays (future enhancement)
pub fn blocks_to_numpy(blocks: &[PyBlock]) -> Vec<Vec<f32>> {
    blocks.iter().map(|b| {
        vec![b.x, b.y, b.z, b.depth as f32]
    }).collect()
}