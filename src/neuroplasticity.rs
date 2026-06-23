//! Neuroplasticity — adaptive reorganization and learning through experience
//! Implements Hebbian learning, structural reorganization, and adaptive pathway strengthening

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct SynapticConnection {
    pub from_block: u32,
    pub to_block: u32,
    pub weight: f32,    // 0.0-1.0, synaptic strength
    pub frequency: u32, // co-activation count
    pub last_activated_ms: u64,
    pub plasticity_score: f32, // capacity to change
}

#[derive(Clone, Debug)]
pub struct NeuralPathway {
    pub id: u64,
    pub nodes: Vec<u32>,         // blocks in sequence
    pub strength: f32,           // pathway activation strength
    pub efficiency: f32,         // speed of activation (inverse latency)
    pub usage_count: u32,        // how often used
    pub specialized_for: String, // domain/context
}

pub struct Neuroplasticity {
    pub synapses: HashMap<(u32, u32), SynapticConnection>,
    pub pathways: HashMap<u64, NeuralPathway>,
    pub reorganization_threshold: f32,
    pub plasticity_window_ms: u64,
}

impl Neuroplasticity {
    pub fn new() -> Self {
        Self {
            synapses: HashMap::new(),
            pathways: HashMap::new(),
            reorganization_threshold: 0.5,
            plasticity_window_ms: 86_400_000, // 24 hours
        }
    }

    /// Strengthen or create synaptic connection (Hebbian learning)
    pub fn strengthen_synapse(&mut self, from: u32, to: u32, success: bool) {
        let key = (from, to);
        let now = Self::now_ms();

        self.synapses
            .entry(key)
            .and_modify(|s| {
                s.frequency += 1;
                let delta = if success { 0.05 } else { -0.02 };
                s.weight = (s.weight + delta).clamp(0.0, 1.0);
                s.last_activated_ms = now;
            })
            .or_insert_with(|| SynapticConnection {
                from_block: from,
                to_block: to,
                weight: if success { 0.6 } else { 0.3 },
                frequency: 1,
                last_activated_ms: now,
                plasticity_score: 0.8,
            });
    }

    /// Create or strengthen neural pathway
    pub fn strengthen_pathway(&mut self, nodes: Vec<u32>, domain: &str) -> u64 {
        let pathway_id = Self::pathway_hash(&nodes);

        self.pathways
            .entry(pathway_id)
            .and_modify(|p| {
                p.usage_count += 1;
                p.strength = (p.strength + 0.1).min(1.0);
                p.efficiency = (p.efficiency + 0.05).min(1.0);
            })
            .or_insert_with(|| NeuralPathway {
                id: pathway_id,
                nodes: nodes.clone(),
                strength: 0.5,
                efficiency: 0.4,
                usage_count: 1,
                specialized_for: domain.to_string(),
            });

        pathway_id
    }

    /// Prune weak connections (use it or lose it)
    pub fn prune_weak_synapses(&mut self, strength_threshold: f32) -> usize {
        let before = self.synapses.len();
        self.synapses.retain(|_, s| s.weight > strength_threshold);
        before - self.synapses.len()
    }

    /// Reorganize pathways (merge/split based on usage)
    pub fn reorganize_pathways(&mut self) -> usize {
        let mut reorganized = 0;
        let now = Self::now_ms();

        // Strengthen frequently used pathways
        for pathway in self.pathways.values_mut() {
            if pathway.usage_count >= 10 {
                pathway.strength = (pathway.strength + 0.02).min(1.0);
                reorganized += 1;
            }
        }

        // Prune unused pathways
        let unused_before = self.pathways.len();
        self.pathways.retain(|_, p| {
            let age = now.saturating_sub(p.id as u64);
            p.usage_count > 0 || age < self.plasticity_window_ms
        });

        reorganized + (unused_before - self.pathways.len())
    }

    /// Calculate network plasticity (capacity to change)
    pub fn calculate_plasticity(&self) -> f32 {
        if self.synapses.is_empty() {
            return 0.0;
        }

        let weak_synapses = self.synapses.values().filter(|s| s.weight < 0.3).count() as f32;
        let total = self.synapses.len() as f32;

        (weak_synapses / total).min(1.0)
    }

    /// Find alternative pathways (network rewiring)
    pub fn find_alternative_pathways(&self, source: u32, target: u32) -> Vec<Vec<u32>> {
        let mut alternatives = Vec::new();

        // Simple: find two-hop paths
        for (key, synapse) in &self.synapses {
            if key.0 == source {
                for (key2, synapse2) in &self.synapses {
                    if key2.0 == key.1 && key2.1 == target {
                        let combined_weight = synapse.weight * synapse2.weight;
                        if combined_weight > 0.2 {
                            alternatives.push(vec![source, key.1, target]);
                        }
                    }
                }
            }
        }

        alternatives
    }

    /// Get network statistics
    pub fn stats(&self) -> (usize, usize, f32, f32, usize) {
        let synapse_count = self.synapses.len();
        let pathway_count = self.pathways.len();
        let avg_weight = if !self.synapses.is_empty() {
            self.synapses.values().map(|s| s.weight).sum::<f32>() / synapse_count as f32
        } else {
            0.0
        };
        let plasticity = self.calculate_plasticity();
        let strong_pathways = self.pathways.values().filter(|p| p.strength > 0.7).count();

        (
            synapse_count,
            pathway_count,
            avg_weight,
            plasticity,
            strong_pathways,
        )
    }

    /// Get strongest pathways
    pub fn strongest_pathways(&self, k: usize) -> Vec<&NeuralPathway> {
        let mut sorted: Vec<_> = self.pathways.values().collect();
        sorted.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap());
        sorted.into_iter().take(k).collect()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("neuroplasticity.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"NPLS");
        data.push(1);

        // Synapses
        let synapse_count = self.synapses.len() as u32;
        data.extend_from_slice(&synapse_count.to_le_bytes());
        for ((from, to), synapse) in &self.synapses {
            data.extend_from_slice(&from.to_le_bytes());
            data.extend_from_slice(&to.to_le_bytes());
            data.extend_from_slice(&synapse.weight.to_le_bytes());
            data.extend_from_slice(&synapse.frequency.to_le_bytes());
            data.extend_from_slice(&synapse.last_activated_ms.to_le_bytes());
            data.extend_from_slice(&synapse.plasticity_score.to_le_bytes());
        }

        // Pathways
        let pathway_count = self.pathways.len() as u32;
        data.extend_from_slice(&pathway_count.to_le_bytes());
        for (_, pathway) in &self.pathways {
            data.extend_from_slice(&pathway.id.to_le_bytes());
            data.extend_from_slice(&pathway.strength.to_le_bytes());
            data.extend_from_slice(&pathway.efficiency.to_le_bytes());
            data.extend_from_slice(&pathway.usage_count.to_le_bytes());

            let spec_bytes = pathway.specialized_for.as_bytes();
            data.push(spec_bytes.len() as u8);
            data.extend_from_slice(spec_bytes);

            let node_count = pathway.nodes.len() as u16;
            data.extend_from_slice(&node_count.to_le_bytes());
            for &node in &pathway.nodes {
                data.extend_from_slice(&node.to_le_bytes());
            }
        }

        let tmp_path = dir.join("neuroplasticity.bin.tmp");
        fs::write(&tmp_path, data).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("neuroplasticity.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..4] != b"NPLS" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let mut synapses = HashMap::new();
        let mut pathways = HashMap::new();

        // Read synapses
        if idx + 4 <= data.len() {
            let synapse_count =
                u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]])
                    as usize;
            idx += 4;

            for _ in 0..synapse_count {
                if idx + 36 > data.len() {
                    break;
                }

                let from =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;
                let to =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;
                let weight =
                    f32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;
                let frequency =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;
                let last_activated_ms = u64::from_le_bytes([
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
                let plasticity_score =
                    f32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;

                synapses.insert(
                    (from, to),
                    SynapticConnection {
                        from_block: from,
                        to_block: to,
                        weight,
                        frequency,
                        last_activated_ms,
                        plasticity_score,
                    },
                );
            }
        }

        // Read pathways
        if idx + 4 <= data.len() {
            let pathway_count =
                u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]])
                    as usize;
            idx += 4;

            for _ in 0..pathway_count {
                if idx + 30 > data.len() {
                    break;
                }

                let id = u64::from_le_bytes([
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

                let strength =
                    f32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;
                let efficiency =
                    f32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;
                let usage_count =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                idx += 4;

                if idx >= data.len() {
                    break;
                }
                let spec_len = data[idx] as usize;
                idx += 1;

                if idx + spec_len > data.len() {
                    break;
                }
                let specialized_for =
                    String::from_utf8_lossy(&data[idx..idx + spec_len]).to_string();
                idx += spec_len;

                if idx + 2 > data.len() {
                    break;
                }
                let node_count = u16::from_le_bytes([data[idx], data[idx + 1]]) as usize;
                idx += 2;

                let mut nodes = Vec::new();
                for _ in 0..node_count {
                    if idx + 4 > data.len() {
                        break;
                    }
                    let node = u32::from_le_bytes([
                        data[idx],
                        data[idx + 1],
                        data[idx + 2],
                        data[idx + 3],
                    ]);
                    nodes.push(node);
                    idx += 4;
                }

                pathways.insert(
                    id,
                    NeuralPathway {
                        id,
                        nodes,
                        strength,
                        efficiency,
                        usage_count,
                        specialized_for,
                    },
                );
            }
        }

        Ok(Self {
            synapses,
            pathways,
            reorganization_threshold: 0.5,
            plasticity_window_ms: 86_400_000,
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    fn pathway_hash(nodes: &[u32]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &node in nodes.iter().take(10) {
            hash = hash.wrapping_mul(0x100000001b3) ^ (node as u64);
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
