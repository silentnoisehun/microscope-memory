//! Multi-index federation for Microscope Memory.
//!
//! Query multiple microscope indices in parallel and merge results.
//! Enables cross-project memory search.

use crate::config::Config;
use crate::reader::MicroscopeReader;
use crate::{content_coords_blended, read_append_log, LAYER_NAMES};
use std::path::Path;

/// A single result from a federated search, tagged with its source index.
#[derive(Clone)]
pub struct FederatedResult {
    pub text: String,
    pub depth: u8,
    pub layer: String,
    pub score: f32,
    pub source_index: String,
    pub is_append: bool,
}

/// Federated search across multiple microscope indices.
pub struct FederatedSearch {
    /// (name, config, weight) for each index
    indices: Vec<(String, Config, f32)>,
}

impl FederatedSearch {
    /// Create from the main config's federation section.
    pub fn from_config(config: &Config) -> Result<Self, String> {
        let mut indices = Vec::new();

        for entry in &config.federation.indices {
            let idx_config = Config::load(&entry.config_path).map_err(|e| {
                format!(
                    "Failed to load federated index '{}' from '{}': {}",
                    entry.name, entry.config_path, e
                )
            })?;
            indices.push((entry.name.clone(), idx_config, entry.weight));
        }

        if indices.is_empty() {
            return Err("No federated indices configured".to_string());
        }

        Ok(Self { indices })
    }

    /// Recall query across all federated indices in parallel.
    pub fn recall(&self, query: &str, k: usize) -> Vec<FederatedResult> {
        let results: Vec<Vec<FederatedResult>> = std::thread::scope(|s| {
            let handles: Vec<_> = self
                .indices
                .iter()
                .map(|(name, config, weight)| {
                    let name = name.clone();
                    let weight = *weight;
                    s.spawn(move || recall_single(&name, config, query, k, weight))
                })
                .collect();

            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        merge_results(results, k)
    }

    /// Text search across all federated indices in parallel.
    pub fn find_text(&self, query: &str, k: usize) -> Vec<FederatedResult> {
        let results: Vec<Vec<FederatedResult>> = std::thread::scope(|s| {
            let handles: Vec<_> = self
                .indices
                .iter()
                .map(|(name, config, weight)| {
                    let name = name.clone();
                    let weight = *weight;
                    s.spawn(move || find_single(&name, config, query, k, weight))
                })
                .collect();

            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        merge_results(results, k)
    }

    /// MQL query across all federated indices in parallel.
    pub fn mql_query(&self, mql: &str, k: usize) -> Vec<FederatedResult> {
        let results: Vec<Vec<FederatedResult>> = std::thread::scope(|s| {
            let handles: Vec<_> = self
                .indices
                .iter()
                .map(|(name, config, weight)| {
                    let name = name.clone();
                    let weight = *weight;
                    s.spawn(move || mql_single(&name, config, mql, k, weight))
                })
                .collect();

            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        merge_results(results, k)
    }

    /// Get names and status of all federated indices.
    pub fn status(&self) -> Vec<(String, Result<usize, String>)> {
        self.indices
            .iter()
            .map(|(name, config, _)| {
                let result = MicroscopeReader::open(config).map(|r| r.block_count);
                (name.clone(), result)
            })
            .collect()
    }
}

/// Recall from a single index.
fn recall_single(
    name: &str,
    config: &Config,
    query: &str,
    k: usize,
    weight: f32,
) -> Vec<FederatedResult> {
    let reader = match MicroscopeReader::open(config) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);
    let q_lower = query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

    let (zoom_lo, zoom_hi) = match query.len() {
        0..=10 => (0u8, 3u8),
        11..=40 => (3, 6),
        _ => (6, 8),
    };

    let mut results: Vec<(f32, FederatedResult)> = Vec::new();

    for zoom in zoom_lo..=zoom_hi {
        let (start, count) = reader.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        for i in start..(start + count) {
            let text = reader.text(i);
            let text_lower = text.to_lowercase();
            let hits = keywords
                .iter()
                .filter(|&&kw| text_lower.contains(kw))
                .count();
            if hits > 0 {
                let h = reader.header(i);
                let dx = h.x - qx;
                let dy = h.y - qy;
                let dz = h.z - qz;
                let dist = dx * dx + dy * dy + dz * dz;
                let boost = hits as f32 * 0.1;
                let score = (dist - boost).max(0.0) / weight; // lower weight = better score
                results.push((
                    score,
                    FederatedResult {
                        text: text.to_string(),
                        depth: h.depth,
                        layer: LAYER_NAMES
                            .get(h.layer_id as usize)
                            .unwrap_or(&"?")
                            .to_string(),
                        score,
                        source_index: name.to_string(),
                        is_append: false,
                    },
                ));
            }
        }
    }

    // Also search append log
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for entry in &appended {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx * dx + dy * dy + dz * dz;
        let text_lower = entry.text.to_lowercase();
        let hits = keywords
            .iter()
            .filter(|&&kw| text_lower.contains(kw))
            .count();
        if dist < 0.1 || hits > 0 {
            let boost = hits as f32 * 0.1;
            let score = (dist - boost).max(0.0) / weight;
            results.push((
                score,
                FederatedResult {
                    text: entry.text.clone(),
                    depth: entry.depth,
                    layer: LAYER_NAMES
                        .get(entry.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score,
                    source_index: name.to_string(),
                    is_append: true,
                },
            ));
        }
    }

    results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    results.truncate(k);
    results.into_iter().map(|(_, r)| r).collect()
}

/// Text search from a single index.
fn find_single(
    name: &str,
    config: &Config,
    query: &str,
    k: usize,
    weight: f32,
) -> Vec<FederatedResult> {
    let reader = match MicroscopeReader::open(config) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let results = reader.find_text(query, k);
    results
        .iter()
        .enumerate()
        .map(|(rank, &(_, idx))| {
            let h = reader.header(idx);
            FederatedResult {
                text: reader.text(idx).to_string(),
                depth: h.depth,
                layer: LAYER_NAMES
                    .get(h.layer_id as usize)
                    .unwrap_or(&"?")
                    .to_string(),
                score: rank as f32 / weight,
                source_index: name.to_string(),
                is_append: false,
            }
        })
        .collect()
}

/// MQL query from a single index.
fn mql_single(
    name: &str,
    config: &Config,
    mql: &str,
    k: usize,
    weight: f32,
) -> Vec<FederatedResult> {
    let reader = match MicroscopeReader::open(config) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    let q = crate::query::parse(mql);
    let mut results = crate::query::execute(&q, &reader, &appended);
    results.truncate(k);

    results
        .iter()
        .map(|r| {
            if r.is_main {
                let h = reader.header(r.block_idx);
                FederatedResult {
                    text: reader.text(r.block_idx).to_string(),
                    depth: h.depth,
                    layer: LAYER_NAMES
                        .get(h.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score: r.score / weight,
                    source_index: name.to_string(),
                    is_append: false,
                }
            } else {
                let entry = &appended[r.block_idx];
                FederatedResult {
                    text: entry.text.clone(),
                    depth: entry.depth,
                    layer: LAYER_NAMES
                        .get(entry.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score: r.score / weight,
                    source_index: name.to_string(),
                    is_append: true,
                }
            }
        })
        .collect()
}

/// Merge results from multiple indices, sort by score, truncate to k.
fn merge_results(all: Vec<Vec<FederatedResult>>, k: usize) -> Vec<FederatedResult> {
    let mut merged: Vec<FederatedResult> = all.into_iter().flatten().collect();
    merged.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
    merged.truncate(k);
    merged
}
