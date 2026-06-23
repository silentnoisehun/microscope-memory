//! Hippocampus — episodic binding, context binding, and consolidation orchestration
//! Mimics biological hippocampus: indexes new memories, creates context-event bindings,
//! coordinates transfer from short-term (WM) to long-term (cortex-like) storage

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ContextBinding {
    pub query_hash: u64,
    pub context: String,         // spatial/temporal context
    pub block_indices: Vec<u32>, // blocks retrieved in this context
    pub binding_strength: f32,   // 0.0-1.0
    pub timestamp_ms: u64,
    pub retrieval_count: u32,
}

#[derive(Clone, Debug)]
pub struct EpisodicIndex {
    pub episode_id: u64,
    pub blocks: Vec<u32>,
    pub context_binding: ContextBinding,
    pub consolidation_state: u8, // 0=fresh, 1=processing, 2=consolidated
    pub replay_count: u32,
}

pub struct Hippocampus {
    pub context_bindings: HashMap<u64, ContextBinding>,
    pub episodes: Vec<EpisodicIndex>,
    pub consolidation_queue: Vec<u64>, // episode IDs to consolidate
    pub binding_threshold: f32,
}

impl Default for Hippocampus {
    fn default() -> Self {
        Self::new()
    }
}

impl Hippocampus {
    pub fn new() -> Self {
        Self {
            context_bindings: HashMap::new(),
            episodes: Vec::new(),
            consolidation_queue: Vec::new(),
            binding_threshold: 0.3,
        }
    }

    /// Create context-event binding: link blocks to their retrieval context
    pub fn create_binding(&mut self, query_hash: u64, context: &str, blocks: Vec<u32>) -> u64 {
        let binding_strength = (blocks.len() as f32 / 100.0).min(1.0);

        let binding = ContextBinding {
            query_hash,
            context: context.to_string(),
            block_indices: blocks.clone(),
            binding_strength,
            timestamp_ms: Self::now_ms(),
            retrieval_count: 1,
        };

        self.context_bindings.insert(query_hash, binding);

        // Create episode
        let episode_id = Self::episode_hash(&blocks);
        let episode = EpisodicIndex {
            episode_id,
            blocks,
            context_binding: self.context_bindings[&query_hash].clone(),
            consolidation_state: 0, // fresh
            replay_count: 0,
        };

        self.episodes.push(episode);
        self.consolidation_queue.push(episode_id);

        episode_id
    }

    /// Strengthen context-event binding through repetition
    pub fn reinforce_binding(&mut self, query_hash: u64, success: bool) {
        if let Some(binding) = self.context_bindings.get_mut(&query_hash) {
            binding.retrieval_count += 1;
            if success {
                binding.binding_strength = (binding.binding_strength + 0.1).min(1.0);
            } else {
                binding.binding_strength = (binding.binding_strength - 0.05).max(0.0);
            }
        }
    }

    /// Get consolidation candidates (fresh episodes with strong bindings)
    pub fn get_consolidation_candidates(&self, k: usize) -> Vec<EpisodicIndex> {
        let mut candidates: Vec<_> = self
            .episodes
            .iter()
            .filter(|e| {
                e.consolidation_state == 0
                    && e.context_binding.binding_strength > self.binding_threshold
            })
            .cloned()
            .collect();

        candidates.sort_by(|a, b| {
            b.context_binding
                .binding_strength
                .partial_cmp(&a.context_binding.binding_strength)
                .unwrap()
        });

        candidates.into_iter().take(k).collect()
    }

    /// Mark episode as consolidating
    pub fn mark_consolidating(&mut self, episode_id: u64) {
        if let Some(ep) = self
            .episodes
            .iter_mut()
            .find(|e| e.episode_id == episode_id)
        {
            ep.consolidation_state = 1;
        }
    }

    /// Mark episode as consolidated
    pub fn mark_consolidated(&mut self, episode_id: u64) {
        if let Some(ep) = self
            .episodes
            .iter_mut()
            .find(|e| e.episode_id == episode_id)
        {
            ep.consolidation_state = 2;
            ep.replay_count += 1;
        }
    }

    /// Replay episode for consolidation (simulates sleep consolidation)
    pub fn replay_episode(&mut self, episode_id: u64) -> Option<Vec<u32>> {
        self.episodes
            .iter_mut()
            .find(|e| e.episode_id == episode_id)
            .map(|e| {
                e.replay_count += 1;
                e.context_binding.binding_strength =
                    (e.context_binding.binding_strength + 0.05).min(1.0);
                e.blocks.clone()
            })
    }

    /// Get related episodes (same context)
    pub fn get_related_episodes(&self, episode_id: u64) -> Vec<EpisodicIndex> {
        let context = self
            .episodes
            .iter()
            .find(|e| e.episode_id == episode_id)
            .map(|e| e.context_binding.context.clone());

        if let Some(ctx) = context {
            return self
                .episodes
                .iter()
                .filter(|e| e.context_binding.context == ctx && e.episode_id != episode_id)
                .cloned()
                .collect();
        }
        Vec::new()
    }

    /// Get statistics
    pub fn stats(&self) -> (usize, usize, usize, f32) {
        let total_bindings = self.context_bindings.len();
        let total_episodes = self.episodes.len();
        let consolidated = self
            .episodes
            .iter()
            .filter(|e| e.consolidation_state == 2)
            .count();
        let avg_binding_strength = if !self.context_bindings.is_empty() {
            self.context_bindings
                .values()
                .map(|b| b.binding_strength)
                .sum::<f32>()
                / self.context_bindings.len() as f32
        } else {
            0.0
        };

        (
            total_bindings,
            total_episodes,
            consolidated,
            avg_binding_strength,
        )
    }

    /// Decay old episodes (biological forgetting)
    pub fn decay(&mut self) {
        let now = Self::now_ms();
        const DECAY_THRESHOLD_MS: u64 = 2_592_000_000; // 30 days

        self.episodes.retain(|e| {
            let age = now.saturating_sub(e.context_binding.timestamp_ms);
            if age > DECAY_THRESHOLD_MS && e.consolidation_state != 2 {
                return false; // Remove unconsolidated old episodes
            }
            true
        });

        self.context_bindings.retain(|_, b| {
            let age = now.saturating_sub(b.timestamp_ms);
            age <= DECAY_THRESHOLD_MS || b.retrieval_count >= 5
        });
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("hippocampus.bin");
        let tmp_path = dir.join("hippocampus.bin.tmp");
        let mut data = Vec::new();

        data.extend_from_slice(b"HIPP");
        data.push(1);

        // Bindings
        let binding_count = self.context_bindings.len() as u32;
        data.extend_from_slice(&binding_count.to_le_bytes());
        for (hash, binding) in &self.context_bindings {
            data.extend_from_slice(&hash.to_le_bytes());
            data.extend_from_slice(&binding.binding_strength.to_le_bytes());
            data.extend_from_slice(&binding.timestamp_ms.to_le_bytes());
            data.extend_from_slice(&binding.retrieval_count.to_le_bytes());

            let ctx_bytes = binding.context.as_bytes();
            data.extend_from_slice(&(ctx_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(ctx_bytes);

            let block_count = binding.block_indices.len() as u16;
            data.extend_from_slice(&block_count.to_le_bytes());
            for &idx in &binding.block_indices {
                data.extend_from_slice(&idx.to_le_bytes());
            }
        }

        // Episodes
        let episode_count = self.episodes.len() as u32;
        data.extend_from_slice(&episode_count.to_le_bytes());
        for episode in &self.episodes {
            data.extend_from_slice(&episode.episode_id.to_le_bytes());
            data.push(episode.consolidation_state);
            data.extend_from_slice(&episode.replay_count.to_le_bytes());

            let block_count = episode.blocks.len() as u16;
            data.extend_from_slice(&block_count.to_le_bytes());
            for &idx in &episode.blocks {
                data.extend_from_slice(&idx.to_le_bytes());
            }
        }

        fs::write(&tmp_path, &data).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename hippocampus.bin: {}", e))
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("hippocampus.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..4] != b"HIPP" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let mut context_bindings = HashMap::new();
        let mut episodes = Vec::new();

        // Read bindings
        if idx + 4 <= data.len() {
            let binding_count =
                u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]])
                    as usize;
            idx += 4;

            for _ in 0..binding_count {
                if idx + 32 > data.len() {
                    break;
                }

                let hash = u64::from_le_bytes([
                    data[idx],
                    data[idx + 1],
                    data[idx + 2],
                    data[idx + 3],
                    data[idx + 4],
                    data[idx + 5],
                    data[idx + 6],
                    data[idx + 7],
                ]);
                idx += 8;

                let binding_strength =
                    f32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;

                let timestamp_ms = u64::from_le_bytes([
                    data[idx],
                    data[idx + 1],
                    data[idx + 2],
                    data[idx + 3],
                    data[idx + 4],
                    data[idx + 5],
                    data[idx + 6],
                    data[idx + 7],
                ]);
                idx += 8;

                let retrieval_count =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;

                if idx + 2 > data.len() {
                    break;
                }
                let ctx_len = u16::from_le_bytes([data[idx], data[idx + 1]]) as usize;
                idx += 2;

                if idx + ctx_len > data.len() {
                    break;
                }
                let context = String::from_utf8_lossy(&data[idx..idx + ctx_len]).to_string();
                idx += ctx_len;

                if idx + 2 > data.len() {
                    break;
                }
                let block_count = u16::from_le_bytes([data[idx], data[idx + 1]]) as usize;
                idx += 2;

                let mut block_indices = Vec::new();
                for _ in 0..block_count {
                    if idx + 4 > data.len() {
                        break;
                    }
                    let block_idx = u32::from_le_bytes([
                        data[idx],
                        data[idx + 1],
                        data[idx + 2],
                        data[idx + 3],
                    ]);
                    block_indices.push(block_idx);
                    idx += 4;
                }

                context_bindings.insert(
                    hash,
                    ContextBinding {
                        query_hash: hash,
                        context,
                        block_indices,
                        binding_strength,
                        timestamp_ms,
                        retrieval_count,
                    },
                );
            }
        }

        // Read episodes
        if idx + 4 <= data.len() {
            let episode_count =
                u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]])
                    as usize;
            idx += 4;

            for _ in 0..episode_count {
                if idx + 15 > data.len() {
                    break;
                }

                let episode_id = u64::from_le_bytes([
                    data[idx],
                    data[idx + 1],
                    data[idx + 2],
                    data[idx + 3],
                    data[idx + 4],
                    data[idx + 5],
                    data[idx + 6],
                    data[idx + 7],
                ]);
                idx += 8;

                let consolidation_state = data[idx];
                idx += 1;

                let replay_count =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;

                let block_count = u16::from_le_bytes([data[idx], data[idx + 1]]) as usize;
                idx += 2;

                let mut blocks = Vec::new();
                for _ in 0..block_count {
                    if idx + 4 > data.len() {
                        break;
                    }
                    let block_idx = u32::from_le_bytes([
                        data[idx],
                        data[idx + 1],
                        data[idx + 2],
                        data[idx + 3],
                    ]);
                    blocks.push(block_idx);
                    idx += 4;
                }

                // Create dummy binding for loading
                let binding = ContextBinding {
                    query_hash: episode_id,
                    context: String::new(),
                    block_indices: blocks.clone(),
                    binding_strength: 0.5,
                    timestamp_ms: Self::now_ms(),
                    retrieval_count: 0,
                };

                episodes.push(EpisodicIndex {
                    episode_id,
                    blocks,
                    context_binding: binding,
                    consolidation_state,
                    replay_count,
                });
            }
        }

        Ok(Self {
            context_bindings,
            episodes,
            consolidation_queue: Vec::new(),
            binding_threshold: 0.3,
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    fn episode_hash(blocks: &[u32]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &block in blocks.iter().take(10) {
            hash = hash.wrapping_mul(0x100000001b3) ^ (block as u64);
        }
        hash
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
