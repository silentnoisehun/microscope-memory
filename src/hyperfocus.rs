//! Hyperfocus — concentrated attention on single objective
//! Redirects all system resources to one focus point: planning, problem-solving, creative depth, research

use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct HyperfocusState {
    pub target: String,
    pub target_type: String,  // "planning", "problem_solving", "creative", "research"
    pub intensity: f32,       // 0.0-1.0, how concentrated
    pub start_time_ms: u64,
    pub depth_level: f32,     // how deep in the topic (0.0-1.0)
    pub efficiency: f32,      // data processing efficiency
    pub blocks_processed: u32,
}

pub struct Hyperfocus {
    pub current_focus: Option<HyperfocusState>,
    pub focus_history: Vec<HyperfocusState>,
    pub attention_multiplier: f32,  // base 1.0, up to 3.0 in hyperfocus
    pub resource_concentration: f32, // how much goes to focus (0.5-1.0)
}

impl Hyperfocus {
    pub fn new() -> Self {
        Self {
            current_focus: None,
            focus_history: Vec::new(),
            attention_multiplier: 1.0,
            resource_concentration: 0.5,
        }
    }

    /// Enter hyperfocus state on a specific target
    pub fn enter_hyperfocus(&mut self, target: &str, focus_type: &str) -> f32 {
        let now = Self::now_ms();
        
        // Ramp up attention and resources
        self.attention_multiplier = 3.0;  // 3x normal
        self.resource_concentration = 0.95; // 95% to this task
        
        let state = HyperfocusState {
            target: target.to_string(),
            target_type: focus_type.to_string(),
            intensity: 1.0,
            start_time_ms: now,
            depth_level: 0.0,
            efficiency: 0.8,
            blocks_processed: 0,
        };
        
        self.current_focus = Some(state);
        self.attention_multiplier
    }

    /// Process data during hyperfocus (deepen understanding)
    pub fn process_data(&mut self, blocks_count: u32, complexity: f32) {
        if let Some(focus) = &mut self.current_focus {
            // Efficiency increases with focus type
            let type_bonus = match focus.target_type.as_str() {
                "research" => 1.2,      // best for data processing
                "problem_solving" => 1.1,
                "creative" => 0.9,
                "planning" => 1.0,
                _ => 1.0,
            };
            
            focus.efficiency = (focus.efficiency + 0.02 * type_bonus).min(1.0);
            focus.depth_level = (focus.depth_level + 0.05 * complexity).min(1.0);
            focus.blocks_processed += blocks_count;
            
            // Intensity sustained at high level
            focus.intensity = 0.95 + (blocks_count as f32 * 0.001).min(0.05);
        }
    }

    /// Exit hyperfocus (save state and insights)
    pub fn exit_hyperfocus(&mut self) -> Option<HyperfocusState> {
        if let Some(focus) = self.current_focus.take() {
            // Save to history
            self.focus_history.push(focus.clone());
            if self.focus_history.len() > 100 {
                self.focus_history.remove(0);
            }
            
            // Return to normal
            self.attention_multiplier = 1.0;
            self.resource_concentration = 0.5;
            
            return Some(focus);
        }
        None
    }

    /// Get hyperfocus insights (what was learned)
    pub fn get_insights(&self) -> Vec<String> {
        let mut insights = Vec::new();
        
        if let Some(focus) = &self.current_focus {
            let mut insight = format!("Hyperfocus on '{}' ({})", focus.target, focus.target_type);
            
            if focus.blocks_processed > 100 {
                insight.push_str(&format!(" - Processed {} blocks", focus.blocks_processed));
            }
            
            if focus.depth_level > 0.7 {
                insight.push_str(" - Deep understanding achieved");
            } else if focus.depth_level > 0.4 {
                insight.push_str(" - Moderate depth");
            }
            
            if focus.efficiency > 0.9 {
                insight.push_str(" - High efficiency");
            }
            
            insights.push(insight);
        }
        
        insights
    }

    /// Check if hyperfocus is still active and productive
    pub fn is_productive(&self) -> bool {
        if let Some(focus) = &self.current_focus {
            focus.intensity > 0.7 && focus.efficiency > 0.5
        } else {
            false
        }
    }

    /// Get resource allocation (how much goes to different tasks)
    pub fn get_resource_allocation(&self) -> (f32, f32) {
        // (to_focus, to_background)
        if self.current_focus.is_some() {
            (self.resource_concentration, 1.0 - self.resource_concentration)
        } else {
            (0.5, 0.5)
        }
    }

    /// Get stats
    pub fn stats(&self) -> (bool, f32, f32, u32) {
        if let Some(focus) = &self.current_focus {
            (
                true,
                focus.intensity,
                focus.depth_level,
                focus.blocks_processed,
            )
        } else {
            (false, 0.0, 0.0, 0)
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("hyperfocus.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"HYPF");
        data.push(1);

        if let Some(focus) = &self.current_focus {
            data.push(1); // Active
            
            let target_bytes = focus.target.as_bytes();
            data.extend_from_slice(&(target_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(target_bytes);
            
            let type_bytes = focus.target_type.as_bytes();
            data.push(type_bytes.len() as u8);
            data.extend_from_slice(type_bytes);
            
            data.extend_from_slice(&focus.intensity.to_le_bytes());
            data.extend_from_slice(&focus.start_time_ms.to_le_bytes());
            data.extend_from_slice(&focus.depth_level.to_le_bytes());
            data.extend_from_slice(&focus.efficiency.to_le_bytes());
            data.extend_from_slice(&focus.blocks_processed.to_le_bytes());
        } else {
            data.push(0); // Inactive
        }

        fs::write(&path, data).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("hyperfocus.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..4] != b"HYPF" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let mut current_focus = None;

        if idx < data.len() && data[idx] == 1 {
            idx += 1;

            if idx + 2 <= data.len() {
                let target_len = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                idx += 2;

                if idx + target_len > data.len() { return Ok(Self::new()); }
                let target = String::from_utf8_lossy(&data[idx..idx+target_len]).to_string();
                idx += target_len;

                if idx >= data.len() { return Ok(Self::new()); }
                let type_len = data[idx] as usize;
                idx += 1;

                if idx + type_len > data.len() { return Ok(Self::new()); }
                let target_type = String::from_utf8_lossy(&data[idx..idx+type_len]).to_string();
                idx += type_len;

                if idx + 24 <= data.len() {
                    let intensity = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    idx += 4;
                    let start_time_ms = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                          data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                    idx += 8;
                    let depth_level = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    idx += 4;
                    let efficiency = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                    idx += 4;
                    let blocks_processed = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);

                    current_focus = Some(HyperfocusState {
                        target,
                        target_type,
                        intensity,
                        start_time_ms,
                        depth_level,
                        efficiency,
                        blocks_processed,
                    });
                }
            }
        }

        let is_active = current_focus.is_some();
        Ok(Self {
            current_focus,
            focus_history: Vec::new(),
            attention_multiplier: if is_active { 3.0 } else { 1.0 },
            resource_concentration: if is_active { 0.95 } else { 0.5 },
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}