//! Implicit memory — procedural & habit learning without conscious recall
//! Skills, patterns, and conditioned responses from repeated exposure

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ImplicitPattern {
    pub pattern: Vec<u32>,      // Sequence of block hashes
    pub strength: f32,          // 0.0-1.0, reinforcement level
    pub frequency: u32,         // How many times seen
    pub last_activation_ms: u64,
    pub performance_metric: f32, // Success rate for procedural skills
}

#[derive(Clone, Debug)]
pub struct SkillNode {
    pub name: String,
    pub mastery_level: f32,     // 0.0-1.0
    pub error_rate: f32,        // Recent error frequency
    pub last_practiced_ms: u64,
    pub practice_count: u32,
}

pub struct ImplicitMemory {
    pub patterns: HashMap<u64, ImplicitPattern>, // Pattern hash -> pattern
    pub skills: HashMap<String, SkillNode>,
    pub habits: Vec<(String, f32)>,              // (trigger, strength) pairs
    pub conditioning: HashMap<String, f32>,      // stimulus -> response strength
}

impl ImplicitMemory {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            skills: HashMap::new(),
            habits: Vec::new(),
            conditioning: HashMap::new(),
        }
    }

    /// Learn a procedural pattern from repeated activation
    pub fn learn_pattern(&mut self, pattern: Vec<u32>, success: bool) {
        let hash = Self::hash_pattern(&pattern);
        let strength_delta = if success { 0.1 } else { -0.05 };

        self.patterns.entry(hash)
            .and_modify(|p| {
                p.strength = (p.strength + strength_delta).clamp(0.0, 1.0);
                p.frequency += 1;
                p.performance_metric = p.performance_metric * 0.8 + if success { 1.0 } else { 0.0 } * 0.2;
                p.last_activation_ms = Self::now_ms();
            })
            .or_insert_with(|| ImplicitPattern {
                pattern: pattern.clone(),
                strength: 0.5,
                frequency: 1,
                last_activation_ms: Self::now_ms(),
                performance_metric: if success { 1.0 } else { 0.0 },
            });
    }

    /// Develop or improve a skill
    pub fn practice_skill(&mut self, skill: &str, error_occurred: bool) {
        self.skills.entry(skill.to_string())
            .and_modify(|s| {
                s.practice_count += 1;
                let improvement = 0.02;
                if !error_occurred {
                    s.mastery_level = (s.mastery_level + improvement).min(1.0);
                    s.error_rate = (s.error_rate * 0.95).max(0.0);
                } else {
                    s.error_rate = (s.error_rate + 0.05).min(1.0);
                }
                s.last_practiced_ms = Self::now_ms();
            })
            .or_insert_with(|| SkillNode {
                name: skill.to_string(),
                mastery_level: if !error_occurred { 0.2 } else { 0.05 },
                error_rate: if error_occurred { 0.5 } else { 0.1 },
                last_practiced_ms: Self::now_ms(),
                practice_count: 1,
            });
    }

    /// Form or reinforce a habit
    pub fn form_habit(&mut self, trigger: &str, strength_delta: f32) {
        let pos = self.habits.iter().position(|(t, _)| t == trigger);
        if let Some(idx) = pos {
            let new_strength = (self.habits[idx].1 + strength_delta).clamp(0.0, 1.0);
            self.habits[idx].1 = new_strength;
        } else {
            self.habits.push((trigger.to_string(), strength_delta.clamp(0.0, 1.0)));
        }
    }

    /// Classical conditioning: stimulus -> response
    pub fn condition_response(&mut self, stimulus: &str, response_strength: f32) {
        self.conditioning.entry(stimulus.to_string())
            .and_modify(|s| *s = (*s * 0.7 + response_strength * 0.3).clamp(0.0, 1.0))
            .or_insert(response_strength.clamp(0.0, 1.0));
    }

    /// Get strongest patterns
    pub fn strongest_patterns(&self, k: usize) -> Vec<(u64, ImplicitPattern)> {
        let mut sorted: Vec<_> = self.patterns.iter()
            .map(|(h, p)| (*h, p.clone()))
            .collect();
        sorted.sort_by(|a, b| b.1.strength.partial_cmp(&a.1.strength).unwrap());
        sorted.into_iter().take(k).collect()
    }

    /// Get most practiced skills
    pub fn skill_ranking(&self) -> Vec<(String, SkillNode)> {
        let mut sorted: Vec<_> = self.skills.iter()
            .map(|(n, s)| (n.clone(), s.clone()))
            .collect();
        sorted.sort_by(|a, b| b.1.mastery_level.partial_cmp(&a.1.mastery_level).unwrap());
        sorted
    }

    /// Decay: periodic forgetting of weak patterns/skills
    pub fn decay(&mut self) {
        let now = Self::now_ms();
        const DECAY_THRESHOLD_MS: u64 = 604_800_000; // 7 days

        // Decay patterns
        self.patterns.retain(|_, p| {
            let age = now.saturating_sub(p.last_activation_ms);
            if age > DECAY_THRESHOLD_MS {
                p.strength *= 0.5; // Decay strength
            }
            p.strength > 0.05 // Remove near-forgotten patterns
        });

        // Decay skills
        self.skills.retain(|_, s| {
            let age = now.saturating_sub(s.last_practiced_ms);
            if age > DECAY_THRESHOLD_MS {
                s.mastery_level *= 0.7;
            }
            s.mastery_level > 0.05
        });

        // Decay habits
        self.habits.retain(|(_, strength)| *strength > 0.05);
    }

    fn hash_pattern(pattern: &[u32]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &val in pattern {
            hash = hash.wrapping_mul(0x100000001b3) ^ (val as u64);
        }
        hash
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("implicit_memory.bin");
        let mut data = Vec::new();

        // Magic + version
        data.extend_from_slice(b"IMPL");
        data.push(1);

        // Patterns
        let pattern_count = self.patterns.len() as u32;
        data.extend_from_slice(&pattern_count.to_le_bytes());
        for (hash, pattern) in &self.patterns {
            data.extend_from_slice(&hash.to_le_bytes());
            data.extend_from_slice(&pattern.strength.to_le_bytes());
            data.extend_from_slice(&pattern.frequency.to_le_bytes());
            data.extend_from_slice(&pattern.last_activation_ms.to_le_bytes());
            data.extend_from_slice(&pattern.performance_metric.to_le_bytes());
            
            let seq_len = pattern.pattern.len() as u16;
            data.extend_from_slice(&seq_len.to_le_bytes());
            for &val in &pattern.pattern {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        // Skills
        let skill_count = self.skills.len() as u32;
        data.extend_from_slice(&skill_count.to_le_bytes());
        for (name, skill) in &self.skills {
            let name_bytes = name.as_bytes();
            data.push(name_bytes.len() as u8);
            data.extend_from_slice(name_bytes);
            data.extend_from_slice(&skill.mastery_level.to_le_bytes());
            data.extend_from_slice(&skill.error_rate.to_le_bytes());
            data.extend_from_slice(&skill.last_practiced_ms.to_le_bytes());
            data.extend_from_slice(&skill.practice_count.to_le_bytes());
        }

        let tmp_path = dir.join("implicit_memory.bin.tmp");
        fs::write(&tmp_path, data).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("implicit_memory.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 9 || &data[0..4] != b"IMPL" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let mut patterns = HashMap::new();
        let mut skills = HashMap::new();

        // Read patterns
        if idx + 4 <= data.len() {
            let pattern_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..pattern_count {
                if idx + 32 > data.len() { break; }
                
                let hash = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                             data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                let strength = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let frequency = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let last_activation_ms = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                           data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                let performance_metric = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                if idx + 2 <= data.len() {
                    let seq_len = u16::from_le_bytes([data[idx], data[idx+1]]) as usize;
                    idx += 2;

                    let mut pattern = Vec::new();
                    for _ in 0..seq_len {
                        if idx + 4 > data.len() { break; }
                        let val = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                        pattern.push(val);
                        idx += 4;
                    }

                    patterns.insert(hash, ImplicitPattern {
                        pattern,
                        strength,
                        frequency,
                        last_activation_ms,
                        performance_metric,
                    });
                }
            }
        }

        // Read skills
        if idx + 4 <= data.len() {
            let skill_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..skill_count {
                if idx >= data.len() { break; }
                let name_len = data[idx] as usize;
                idx += 1;

                if idx + name_len > data.len() { break; }
                let name = String::from_utf8_lossy(&data[idx..idx+name_len]).to_string();
                idx += name_len;

                if idx + 20 > data.len() { break; }
                let mastery_level = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let error_rate = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let last_practiced_ms = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                          data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                let practice_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                skills.insert(name.clone(), SkillNode {
                    name,
                    mastery_level,
                    error_rate,
                    last_practiced_ms,
                    practice_count,
                });
            }
        }

        Ok(Self {
            patterns,
            skills,
            habits: Vec::new(),
            conditioning: HashMap::new(),
        })
    }

    pub fn load_or_init(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_else(|_| Self::new())
    }
}