//! Synaptic Plasticity — Long-term potentiation, long-term depression, spike-timing-dependent plasticity
//! Core mechanisms of learning at synaptic level: activity-dependent weight changes

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Synapse {
    pub pre_block: u32,
    pub post_block: u32,
    pub weight: f32,                   // 0.0-1.0
    pub last_pre_spike_ms: u64,
    pub last_post_spike_ms: u64,
    pub spike_history: Vec<i64>,       // relative timing of spikes (ms)
    pub ltp_count: u32,                // LTP events
    pub ltd_count: u32,                // LTD events
}

pub struct SynapticPlasticity {
    pub synapses: HashMap<(u32, u32), Synapse>,
    pub ltp_threshold_ms: i64,         // spike timing window for LTP (+ms)
    pub ltd_threshold_ms: i64,         // spike timing window for LTD (-ms)
    pub learning_rate: f32,            // 0.0-1.0
    pub total_updates: u32,
}

impl SynapticPlasticity {
    pub fn new() -> Self {
        Self {
            synapses: HashMap::new(),
            ltp_threshold_ms: 20,          // post before pre: LTP
            ltd_threshold_ms: -20,         // pre before post: LTD
            learning_rate: 0.1,
            total_updates: 0,
        }
    }

    /// Long-Term Potentiation: strengthen synapse
    pub fn ltp(&mut self, pre_block: u32, post_block: u32) -> f32 {
        let key = (pre_block, post_block);
        let now = Self::now_ms();

        let synapse = self.synapses.entry(key)
            .or_insert_with(|| Synapse {
                pre_block,
                post_block,
                weight: 0.5,
                last_pre_spike_ms: now,
                last_post_spike_ms: now,
                spike_history: Vec::new(),
                ltp_count: 0,
                ltd_count: 0,
            });

        // Consolidate: weight increase
        let increment = self.learning_rate * (1.0 - synapse.weight);
        synapse.weight = (synapse.weight + increment).min(1.0);
        synapse.ltp_count += 1;
        synapse.last_pre_spike_ms = now;
        self.total_updates += 1;

        synapse.weight
    }

    /// Long-Term Depression: weaken synapse
    pub fn ltd(&mut self, pre_block: u32, post_block: u32) -> f32 {
        let key = (pre_block, post_block);
        let now = Self::now_ms();

        let synapse = self.synapses.entry(key)
            .or_insert_with(|| Synapse {
                pre_block,
                post_block,
                weight: 0.5,
                last_pre_spike_ms: now,
                last_post_spike_ms: now,
                spike_history: Vec::new(),
                ltp_count: 0,
                ltd_count: 0,
            });

        // Weaken: weight decrease
        let decrement = self.learning_rate * synapse.weight;
        synapse.weight = (synapse.weight - decrement).max(0.0);
        synapse.ltd_count += 1;
        synapse.last_post_spike_ms = now;
        self.total_updates += 1;

        synapse.weight
    }

    /// Spike-Timing-Dependent Plasticity (STDP)
    /// Pre spike then post spike (causal): LTP
    /// Post spike then pre spike (acausal): LTD
    pub fn stdp(&mut self, pre_block: u32, post_block: u32, pre_time_ms: i64, post_time_ms: i64) -> f32 {
        let key = (pre_block, post_block);
        let timing_diff = post_time_ms - pre_time_ms; // positive = post after pre

        let synapse = self.synapses.entry(key)
            .or_insert_with(|| Synapse {
                pre_block,
                post_block,
                weight: 0.5,
                last_pre_spike_ms: Self::now_ms(),
                last_post_spike_ms: Self::now_ms(),
                spike_history: Vec::new(),
                ltp_count: 0,
                ltd_count: 0,
            });

        synapse.spike_history.push(timing_diff);
        if synapse.spike_history.len() > 50 {
            synapse.spike_history.remove(0);
        }

        // Causal: pre before post → LTP
        if timing_diff > 0 && timing_diff <= self.ltp_threshold_ms {
            let strength = 1.0 - (timing_diff as f32 / self.ltp_threshold_ms as f32);
            let increment = self.learning_rate * strength * (1.0 - synapse.weight);
            synapse.weight = (synapse.weight + increment).min(1.0);
            synapse.ltp_count += 1;
        }
        // Anti-causal: post before pre → LTD
        else if timing_diff < 0 && timing_diff >= self.ltd_threshold_ms {
            let strength = 1.0 - (-timing_diff as f32 / -self.ltd_threshold_ms as f32);
            let decrement = self.learning_rate * strength * synapse.weight;
            synapse.weight = (synapse.weight - decrement).max(0.0);
            synapse.ltd_count += 1;
        }

        self.total_updates += 1;
        synapse.weight
    }

    /// Get synapse weight
    pub fn get_weight(&self, pre_block: u32, post_block: u32) -> f32 {
        self.synapses.get(&(pre_block, post_block))
            .map(|s| s.weight)
            .unwrap_or(0.0)
    }

    /// Get strongest synapses
    pub fn strongest_synapses(&self, k: usize) -> Vec<(&(u32, u32), &Synapse)> {
        let mut synapses: Vec<_> = self.synapses.iter().collect();
        synapses.sort_by(|a, b| b.1.weight.partial_cmp(&a.1.weight).unwrap());
        synapses.into_iter().take(k).collect()
    }

    /// Get synapses by plasticity type
    pub fn ltp_dominant(&self) -> Vec<&Synapse> {
        self.synapses.values()
            .filter(|s| s.ltp_count > s.ltd_count)
            .collect()
    }

    pub fn ltd_dominant(&self) -> Vec<&Synapse> {
        self.synapses.values()
            .filter(|s| s.ltd_count > s.ltp_count)
            .collect()
    }

    /// Calculate STDP curve value
    pub fn stdp_curve(&self, timing_diff_ms: i64) -> f32 {
        if timing_diff_ms > 0 && timing_diff_ms <= self.ltp_threshold_ms {
            // LTP: exponential decay
            (1.0 - (timing_diff_ms as f32 / self.ltp_threshold_ms as f32).abs()).max(0.0)
        } else if timing_diff_ms < 0 && timing_diff_ms >= self.ltd_threshold_ms {
            // LTD: negative decay
            -((timing_diff_ms as f32 / self.ltd_threshold_ms as f32).abs()).min(1.0)
        } else {
            0.0
        }
    }

    /// Get statistics
    pub fn stats(&self) -> (u32, u32, u32, f32, f32) {
        let total_synapses = self.synapses.len() as u32;
        let total_ltp: u32 = self.synapses.values().map(|s| s.ltp_count).sum();
        let total_ltd: u32 = self.synapses.values().map(|s| s.ltd_count).sum();
        
        let avg_weight = if !self.synapses.is_empty() {
            self.synapses.values().map(|s| s.weight).sum::<f32>() / self.synapses.len() as f32
        } else {
            0.0
        };

        let ltp_ratio = if total_ltp + total_ltd > 0 {
            total_ltp as f32 / (total_ltp + total_ltd) as f32
        } else {
            0.0
        };

        (total_synapses, total_ltp, total_ltd, avg_weight, ltp_ratio)
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("synaptic_plasticity.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"SYPLS");
        data.push(1);

        let synapse_count = self.synapses.len() as u32;
        data.extend_from_slice(&synapse_count.to_le_bytes());

        for ((pre, post), synapse) in &self.synapses {
            data.extend_from_slice(&pre.to_le_bytes());
            data.extend_from_slice(&post.to_le_bytes());
            data.extend_from_slice(&synapse.weight.to_le_bytes());
            data.extend_from_slice(&synapse.last_pre_spike_ms.to_le_bytes());
            data.extend_from_slice(&synapse.last_post_spike_ms.to_le_bytes());
            data.extend_from_slice(&synapse.ltp_count.to_le_bytes());
            data.extend_from_slice(&synapse.ltd_count.to_le_bytes());

            let hist_count = synapse.spike_history.len() as u16;
            data.extend_from_slice(&hist_count.to_le_bytes());
            for &timing in &synapse.spike_history {
                data.extend_from_slice(&timing.to_le_bytes());
            }
        }

        fs::write(&path, data).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("synaptic_plasticity.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..5] != b"SYPLS" {
            return Ok(Self::new());
        }

        let mut idx = 6;
        let mut synapses = HashMap::new();

        if idx + 4 <= data.len() {
            let synapse_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..synapse_count {
                if idx + 40 > data.len() { break; }

                let pre = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;
                let post = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;
                let weight = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;
                let last_pre = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                 data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;
                let last_post = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                  data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;
                let ltp_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;
                let ltd_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let hist_count = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                let mut spike_history = Vec::new();
                for _ in 0..hist_count {
                    if idx + 8 > data.len() { break; }
                    let timing = i64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                   data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                    spike_history.push(timing);
                    idx += 8;
                }

                synapses.insert((pre, post), Synapse {
                    pre_block: pre,
                    post_block: post,
                    weight,
                    last_pre_spike_ms: last_pre,
                    last_post_spike_ms: last_post,
                    spike_history,
                    ltp_count,
                    ltd_count,
                });
            }
        }

        Ok(Self {
            synapses,
            ltp_threshold_ms: 20,
            ltd_threshold_ms: -20,
            learning_rate: 0.1,
            total_updates: 0,
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    /// Heterosynaptic Depression — weaken other synapses when one is active
    pub fn heterosynaptic_depression(&mut self, active_synapse: (u32, u32), neighborhood_radius: u32) {
        if let Some(active) = self.synapses.get(&active_synapse) {
            let pre = active.pre_block;
            let depression_strength = self.learning_rate * 0.5;

            // Weaken other synapses on same post-neuron
            let post = active_synapse.1;
            let keys_to_weaken: Vec<_> = self.synapses.keys()
                .filter(|(p_pre, p_post)| *p_post == post && self.distance(pre, *p_pre) <= neighborhood_radius)
                .cloned()
                .collect();

            for key in keys_to_weaken {
                if key != active_synapse {
                    if let Some(synapse) = self.synapses.get_mut(&key) {
                        synapse.weight = (synapse.weight - depression_strength).max(0.0);
                        synapse.ltd_count += 1;
                    }
                }
            }
        }
    }

    /// Calculate simple distance between blocks
    fn distance(&self, a: u32, b: u32) -> u32 {
        if a > b { a - b } else { b - a }
    }

    /// Time-dependent plasticity: high at start, decreases with practice, increases with new strategy
    pub fn time_dependent_plasticity(&mut self, synapse_key: (u32, u32), practice_count: u32, strategy_age_ms: u64) -> f32 {
        let now = Self::now_ms();
        
        if let Some(synapse) = self.synapses.get_mut(&synapse_key) {
            // Phase 1: Early learning (high plasticity, 0-10 practices)
            let early_phase_rate = if practice_count < 10 {
                0.2 + (10 - practice_count) as f32 * 0.01  // 0.2-0.3
            } else {
                0.05
            };

            // Phase 2: Consolidation (low plasticity, 10-50 practices)
            let consolidation_decay = if practice_count >= 10 && practice_count < 50 {
                0.05 * (50 - practice_count) as f32 / 40.0  // 0.0625 → 0.05
            } else if practice_count >= 50 {
                0.02  // Very low
            } else {
                early_phase_rate
            };

            // Phase 3: New strategy detection (high plasticity boost if strategy is new)
            let strategy_boost = if strategy_age_ms < 300_000 {  // 5 minutes
                0.1 * (1.0 - (strategy_age_ms as f32 / 300_000.0))  // 0.1 → 0.0
            } else {
                0.0
            };

            self.learning_rate = (consolidation_decay + strategy_boost).min(0.3);
            synapse.last_pre_spike_ms = now;
            
            self.learning_rate
        } else {
            0.2  // Default high plasticity for new synapses
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}