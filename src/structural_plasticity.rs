//! Structural Neuroplasticity — physical network reorganization
//! Dendritic growth, synaptic pruning, neurogenesis (new neuron-like blocks)

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Dendrite {
    pub block_id: u32,
    pub branches: Vec<u32>,        // connected blocks
    pub branch_count: u32,
    pub total_length: f32,         // accumulated connection strength
    pub growth_rate: f32,          // 0.0-1.0
    pub pruning_state: u8,         // 0=active, 1=marked_for_pruning, 2=pruned
}

#[derive(Clone, Debug)]
pub struct NeuronLike {
    pub id: u64,
    pub blocks: Vec<u32>,
    pub dendrite: Dendrite,
    pub axon_terminals: Vec<u32>,
    pub activation_history: Vec<f32>,  // recent activations
    pub specialization: String,
}

pub struct StructuralPlasticity {
    pub neurons: HashMap<u64, NeuronLike>,
    pub dendritic_growth_rate: f32,
    pub pruning_threshold: f32,
    pub neurogenesis_events: u32,
}

impl StructuralPlasticity {
    pub fn new() -> Self {
        Self {
            neurons: HashMap::new(),
            dendritic_growth_rate: 0.05,
            pruning_threshold: 0.1,
            neurogenesis_events: 0,
        }
    }

    /// Grow dendrite: add new branches (dendritic growth)
    pub fn grow_dendrite(&mut self, neuron_id: u64, new_block: u32) -> bool {
        if let Some(neuron) = self.neurons.get_mut(&neuron_id) {
            // Check if dendrite can grow
            if neuron.dendrite.pruning_state == 0 {
                neuron.dendrite.branches.push(new_block);
                neuron.dendrite.branch_count += 1;
                neuron.dendrite.total_length += self.dendritic_growth_rate;
                return true;
            }
        }
        false
    }

    /// Mark synapse for pruning (use it or lose it)
    pub fn mark_for_pruning(&mut self, neuron_id: u64, block_to_remove: u32) {
        if let Some(neuron) = self.neurons.get_mut(&neuron_id) {
            if let Some(pos) = neuron.dendrite.branches.iter().position(|&b| b == block_to_remove) {
                neuron.dendrite.branches.remove(pos);
                neuron.dendrite.branch_count = neuron.dendrite.branch_count.saturating_sub(1);
                neuron.dendrite.total_length -= 0.05;
            }
        }
    }

    /// Prune inactive branches
    pub fn prune_inactive_branches(&mut self, neuron_id: u64) -> u32 {
        let mut pruned = 0;
        
        if let Some(neuron) = self.neurons.get_mut(&neuron_id) {
            let avg_activation = if neuron.activation_history.is_empty() {
                0.0
            } else {
                neuron.activation_history.iter().sum::<f32>() / neuron.activation_history.len() as f32
            };

            neuron.dendrite.branches.retain(|_| avg_activation > self.pruning_threshold);
            let new_count = neuron.dendrite.branches.len() as u32;
            pruned = neuron.dendrite.branch_count.saturating_sub(new_count);
            neuron.dendrite.branch_count = new_count;
        }

        pruned
    }

    /// Neurogenesis: create new neuron-like structure
    pub fn neurogenesis(&mut self, seed_blocks: Vec<u32>, specialization: &str) -> u64 {
        let neuron_id = Self::neuron_hash(&seed_blocks);
        
        let dendrite = Dendrite {
            block_id: seed_blocks[0],
            branches: seed_blocks.clone(),
            branch_count: seed_blocks.len() as u32,
            total_length: seed_blocks.len() as f32 * 0.1,
            growth_rate: 0.05,
            pruning_state: 0,
        };

        let neuron = NeuronLike {
            id: neuron_id,
            blocks: seed_blocks,
            dendrite,
            axon_terminals: Vec::new(),
            activation_history: Vec::new(),
            specialization: specialization.to_string(),
        };

        self.neurons.insert(neuron_id, neuron);
        self.neurogenesis_events += 1;
        neuron_id
    }

    /// Record activation (for pruning decisions)
    pub fn record_activation(&mut self, neuron_id: u64, strength: f32) {
        if let Some(neuron) = self.neurons.get_mut(&neuron_id) {
            neuron.activation_history.push(strength);
            if neuron.activation_history.len() > 100 {
                neuron.activation_history.remove(0); // sliding window
            }
        }
    }

    /// Calculate dendritic complexity
    pub fn dendritic_complexity(&self, neuron_id: u64) -> f32 {
        if let Some(neuron) = self.neurons.get(&neuron_id) {
            (neuron.dendrite.branch_count as f32 * neuron.dendrite.total_length).min(1.0)
        } else {
            0.0
        }
    }

    /// Get network statistics
    pub fn stats(&self) -> (u32, u32, f32, u32) {
        let total_neurons = self.neurons.len() as u32;
        let total_branches: u32 = self.neurons.values().map(|n| n.dendrite.branch_count).sum();
        let avg_dendrite_length = if self.neurons.is_empty() {
            0.0
        } else {
            self.neurons.values().map(|n| n.dendrite.total_length).sum::<f32>() / self.neurons.len() as f32
        };

        (total_neurons, total_branches, avg_dendrite_length, self.neurogenesis_events)
    }

    /// Get most specialized neurons
    pub fn specialized_neurons(&self) -> Vec<(u64, &str, u32)> {
        let mut specialized: Vec<_> = self.neurons.iter()
            .map(|(id, n)| (*id, n.specialization.as_str(), n.dendrite.branch_count))
            .collect();
        specialized.sort_by(|a, b| b.2.cmp(&a.2));
        specialized
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("structural_plasticity.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"STPLS");
        data.push(1);

        // Neurons
        let neuron_count = self.neurons.len() as u32;
        data.extend_from_slice(&neuron_count.to_le_bytes());
        for (_, neuron) in &self.neurons {
            data.extend_from_slice(&neuron.id.to_le_bytes());
            
            let blocks_count = neuron.blocks.len() as u16;
            data.extend_from_slice(&blocks_count.to_le_bytes());
            for &block in &neuron.blocks {
                data.extend_from_slice(&block.to_le_bytes());
            }

            // Dendrite
            data.extend_from_slice(&neuron.dendrite.block_id.to_le_bytes());
            data.extend_from_slice(&neuron.dendrite.growth_rate.to_le_bytes());
            data.push(neuron.dendrite.pruning_state);
            
            let branch_count = neuron.dendrite.branches.len() as u16;
            data.extend_from_slice(&branch_count.to_le_bytes());
            for &branch in &neuron.dendrite.branches {
                data.extend_from_slice(&branch.to_le_bytes());
            }

            // Specialization
            let spec_bytes = neuron.specialization.as_bytes();
            data.push(spec_bytes.len() as u8);
            data.extend_from_slice(spec_bytes);
        }

        data.extend_from_slice(&self.neurogenesis_events.to_le_bytes());

        let tmp_path = dir.join("structural_plasticity.bin.tmp");
        fs::write(&tmp_path, data).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("structural_plasticity.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..5] != b"STPLS" {
            return Ok(Self::new());
        }

        let mut idx = 6;
        let mut neurons = HashMap::new();

        // Read neurons
        if idx + 4 <= data.len() {
            let neuron_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..neuron_count {
                if idx + 10 > data.len() { break; }

                let neuron_id = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                  data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                let blocks_count = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                let mut blocks = Vec::new();
                for _ in 0..blocks_count {
                    if idx + 4 > data.len() { break; }
                    let block = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    blocks.push(block);
                    idx += 4;
                }

                if idx + 13 > data.len() { break; }
                let dendrite_block_id = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let growth_rate = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let pruning_state = data[idx];
                idx += 1;

                let branch_count = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                let mut branches = Vec::new();
                for _ in 0..branch_count {
                    if idx + 4 > data.len() { break; }
                    let branch = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    branches.push(branch);
                    idx += 4;
                }

                if idx >= data.len() { break; }
                let spec_len = data[idx] as usize;
                idx += 1;

                if idx + spec_len > data.len() { break; }
                let specialization = String::from_utf8_lossy(&data[idx..idx+spec_len]).to_string();
                idx += spec_len;

                let dendrite = Dendrite {
                    block_id: dendrite_block_id,
                    branches,
                    branch_count: branch_count as u32,
                    total_length: branch_count as f32 * 0.1,
                    growth_rate,
                    pruning_state,
                };

                neurons.insert(neuron_id, NeuronLike {
                    id: neuron_id,
                    blocks,
                    dendrite,
                    axon_terminals: Vec::new(),
                    activation_history: Vec::new(),
                    specialization,
                });
            }
        }

        let mut neurogenesis_events = 0;
        if idx + 4 <= data.len() {
            neurogenesis_events = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
        }

        Ok(Self {
            neurons,
            dendritic_growth_rate: 0.05,
            pruning_threshold: 0.1,
            neurogenesis_events,
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    fn neuron_hash(blocks: &[u32]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &block in blocks.iter().take(10) {
            hash = hash.wrapping_mul(0x100000001b3) ^ (block as u64);
        }
        hash
    }
}