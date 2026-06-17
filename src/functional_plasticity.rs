//! Functional Neuroplasticity — adaptive reorganization of functional areas
//! Sensorimotor remapping, cross-modal plasticity, functional compensation

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct FunctionalArea {
    pub id: u64,
    pub name: String,                  // "vision", "language", "motor", etc.
    pub blocks: Vec<u32>,              // blocks assigned to this area
    pub primary_domain: String,
    pub cross_modal_connections: Vec<u64>, // connected to other areas
    pub plasticity_index: f32,         // 0.0-1.0, how adaptive this area is
    pub compensation_level: f32,       // how much it compensates for damage
}

#[derive(Clone, Debug)]
pub struct SensoriMotorMap {
    pub input_block: u32,
    pub output_blocks: Vec<u32>,
    pub strength: f32,
    pub remapping_count: u32,
    pub efficiency: f32,
}

pub struct FunctionalPlasticity {
    pub areas: HashMap<u64, FunctionalArea>,
    pub sensorimotor_maps: Vec<SensoriMotorMap>,
    pub crossmodal_connections: HashMap<(u64, u64), f32>, // (area1, area2) -> strength
    pub damage_compensation: HashMap<u64, f32>, // area_id -> compensation strength
}

impl FunctionalPlasticity {
    pub fn new() -> Self {
        Self {
            areas: HashMap::new(),
            sensorimotor_maps: Vec::new(),
            crossmodal_connections: HashMap::new(),
            damage_compensation: HashMap::new(),
        }
    }

    /// Create functional area (e.g., visual cortex)
    pub fn create_area(&mut self, name: &str, domain: &str, seed_blocks: Vec<u32>) -> u64 {
        let area_id = Self::area_hash(name, domain);

        let area = FunctionalArea {
            id: area_id,
            name: name.to_string(),
            blocks: seed_blocks,
            primary_domain: domain.to_string(),
            cross_modal_connections: Vec::new(),
            plasticity_index: 0.5,
            compensation_level: 0.0,
        };

        self.areas.insert(area_id, area);
        area_id
    }

    /// Create sensorimotor mapping (input -> output)
    pub fn map_sensorimotor(&mut self, input: u32, outputs: Vec<u32>) -> f32 {
        let strength = (outputs.len() as f32 / 10.0).min(1.0);

        self.sensorimotor_maps.push(SensoriMotorMap {
            input_block: input,
            output_blocks: outputs,
            strength,
            remapping_count: 0,
            efficiency: 0.5,
        });

        strength
    }

    /// Remap sensorimotor pathway (plasticity in action)
    pub fn remap_pathway(&mut self, old_input: u32, new_input: u32) -> bool {
        if let Some(map) = self.sensorimotor_maps.iter_mut().find(|m| m.input_block == old_input) {
            map.input_block = new_input;
            map.remapping_count += 1;
            map.efficiency = (map.efficiency + 0.05).min(1.0);
            return true;
        }
        false
    }

    /// Create cross-modal connection (area1 <-> area2)
    pub fn connect_areas(&mut self, area1_id: u64, area2_id: u64) -> bool {
        if self.areas.contains_key(&area1_id) && self.areas.contains_key(&area2_id) {
            let strength = 0.3;
            self.crossmodal_connections.insert((area1_id, area2_id), strength);
            self.crossmodal_connections.insert((area2_id, area1_id), strength);

            if let Some(a1) = self.areas.get_mut(&area1_id) {
                a1.cross_modal_connections.push(area2_id);
            }
            if let Some(a2) = self.areas.get_mut(&area2_id) {
                a2.cross_modal_connections.push(area1_id);
            }

            return true;
        }
        false
    }

    /// Strengthen cross-modal connection
    pub fn strengthen_crossmodal(&mut self, area1_id: u64, area2_id: u64, use_success: bool) {
        let key = (area1_id, area2_id);
        if let Some(strength) = self.crossmodal_connections.get_mut(&key) {
            let delta = if use_success { 0.05 } else { -0.02 };
            *strength = (*strength + delta).clamp(0.0, 1.0);
        }
    }

    /// Simulate area damage and trigger compensation
    pub fn damage_area(&mut self, area_id: u64, severity: f32) {
        let connected: Vec<u64> = if let Some(area) = self.areas.get_mut(&area_id) {
            area.plasticity_index = (area.plasticity_index - severity).max(0.0);
            area.cross_modal_connections.clone()
        } else {
            Vec::new()
        };
        
        // Trigger compensation from connected areas
        for connected_id in connected {
            let comp_strength = (severity * 0.7).min(1.0);
            self.damage_compensation.insert(connected_id, comp_strength);

            if let Some(conn_area) = self.areas.get_mut(&connected_id) {
                conn_area.compensation_level = (conn_area.compensation_level + comp_strength).min(1.0);
                conn_area.plasticity_index = (conn_area.plasticity_index + 0.1).min(1.0);
            }
        }
    }

    /// Recover compensation (gradual adaptation)
    pub fn recover_compensation(&mut self, area_id: u64, recovery_rate: f32) -> f32 {
        if let Some(comp) = self.damage_compensation.get_mut(&area_id) {
            *comp = (*comp - recovery_rate).max(0.0);
            return *comp;
        }
        0.0
    }

    /// Get functional reorganization stats
    pub fn stats(&self) -> (u32, u32, usize, f32) {
        let total_areas = self.areas.len() as u32;
        let total_blocks: u32 = self.areas.values().map(|a| a.blocks.len() as u32).sum();
        let total_maps = self.sensorimotor_maps.len();
        let avg_plasticity = if !self.areas.is_empty() {
            self.areas.values().map(|a| a.plasticity_index).sum::<f32>() / self.areas.len() as f32
        } else {
            0.0
        };

        (total_areas, total_blocks, total_maps, avg_plasticity)
    }

    /// Get areas by domain
    pub fn areas_by_domain(&self, domain: &str) -> Vec<&FunctionalArea> {
        self.areas.values()
            .filter(|a| a.primary_domain == domain)
            .collect()
    }

    /// Get most plastic areas
    pub fn most_plastic(&self, k: usize) -> Vec<(&str, f32)> {
        let mut areas: Vec<_> = self.areas.values()
            .map(|a| (a.name.as_str(), a.plasticity_index))
            .collect();
        areas.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        areas.into_iter().take(k).collect()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("functional_plasticity.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"FNPL");
        data.push(1);

        // Areas
        let area_count = self.areas.len() as u32;
        data.extend_from_slice(&area_count.to_le_bytes());
        for (_, area) in &self.areas {
            data.extend_from_slice(&area.id.to_le_bytes());
            
            let name_bytes = area.name.as_bytes();
            data.push(name_bytes.len() as u8);
            data.extend_from_slice(name_bytes);

            let domain_bytes = area.primary_domain.as_bytes();
            data.push(domain_bytes.len() as u8);
            data.extend_from_slice(domain_bytes);

            data.extend_from_slice(&area.plasticity_index.to_le_bytes());
            data.extend_from_slice(&area.compensation_level.to_le_bytes());

            let block_count = area.blocks.len() as u16;
            data.extend_from_slice(&block_count.to_le_bytes());
            for &block in &area.blocks {
                data.extend_from_slice(&block.to_le_bytes());
            }
        }

        // Sensorimotor maps
        let map_count = self.sensorimotor_maps.len() as u32;
        data.extend_from_slice(&map_count.to_le_bytes());
        for map in &self.sensorimotor_maps {
            data.extend_from_slice(&map.input_block.to_le_bytes());
            data.extend_from_slice(&map.strength.to_le_bytes());
            data.extend_from_slice(&map.efficiency.to_le_bytes());
            data.extend_from_slice(&map.remapping_count.to_le_bytes());

            let out_count = map.output_blocks.len() as u16;
            data.extend_from_slice(&out_count.to_le_bytes());
            for &out in &map.output_blocks {
                data.extend_from_slice(&out.to_le_bytes());
            }
        }

        fs::write(&path, data).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("functional_plasticity.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..4] != b"FNPL" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let mut areas = HashMap::new();
        let mut sensorimotor_maps = Vec::new();

        // Read areas
        if idx + 4 <= data.len() {
            let area_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..area_count {
                if idx + 18 > data.len() { break; }

                let id = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                           data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                if idx >= data.len() { break; }
                let name_len = data[idx] as usize;
                idx += 1;

                if idx + name_len > data.len() { break; }
                let name = String::from_utf8_lossy(&data[idx..idx+name_len]).to_string();
                idx += name_len;

                if idx >= data.len() { break; }
                let domain_len = data[idx] as usize;
                idx += 1;

                if idx + domain_len > data.len() { break; }
                let primary_domain = String::from_utf8_lossy(&data[idx..idx+domain_len]).to_string();
                idx += domain_len;

                if idx + 12 > data.len() { break; }
                let plasticity_index = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;
                let compensation_level = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let block_count = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                let mut blocks = Vec::new();
                for _ in 0..block_count {
                    if idx + 4 > data.len() { break; }
                    let block = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    blocks.push(block);
                    idx += 4;
                }

                areas.insert(id, FunctionalArea {
                    id,
                    name,
                    blocks,
                    primary_domain,
                    cross_modal_connections: Vec::new(),
                    plasticity_index,
                    compensation_level,
                });
            }
        }

        // Read sensorimotor maps
        if idx + 4 <= data.len() {
            let map_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..map_count {
                if idx + 20 > data.len() { break; }

                let input_block = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let strength = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let efficiency = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let remapping_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let out_count = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                let mut output_blocks = Vec::new();
                for _ in 0..out_count {
                    if idx + 4 > data.len() { break; }
                    let out = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    output_blocks.push(out);
                    idx += 4;
                }

                sensorimotor_maps.push(SensoriMotorMap {
                    input_block,
                    output_blocks,
                    strength,
                    remapping_count,
                    efficiency,
                });
            }
        }

        Ok(Self {
            areas,
            sensorimotor_maps,
            crossmodal_connections: HashMap::new(),
            damage_compensation: HashMap::new(),
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    fn area_hash(name: &str, domain: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in (name.to_string() + domain).as_bytes().iter().take(16) {
            hash = hash.wrapping_mul(0x100000001b3) ^ (b as u64);
        }
        hash
    }
}