//! Mental Stimulation — continuous activity requirement to prevent cognitive decay
//! Tracks activity, triggers novelty seeking when dormant, maintains engagement level

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ActivityRecord {
    pub timestamp_ms: u64,
    pub activity_type: String,  // "recall", "learn", "consolidate", etc.
    pub intensity: f32,         // 0.0-1.0
    pub engagement_gain: f32,
}

pub struct MentalStimulation {
    pub activity_history: Vec<ActivityRecord>,
    pub engagement_level: f32,  // 0.0-1.0, current mental engagement
    pub decay_rate: f32,        // how fast engagement decays without activity
    pub last_activity_ms: u64,
    pub stimulation_need: f32,  // 0.0-1.0, how much stimulation is needed
    pub novelty_threshold: f32, // trigger new experiences when engagement drops below this
}

impl MentalStimulation {
    pub fn new() -> Self {
        Self {
            activity_history: Vec::new(),
            engagement_level: 0.8,
            decay_rate: 0.001,       // 0.1% per second
            last_activity_ms: Self::now_ms(),
            stimulation_need: 0.0,
            novelty_threshold: 0.3,
        }
    }

    /// Record activity (recall, learning, etc.)
    pub fn record_activity(&mut self, activity_type: &str, intensity: f32) {
        let now = Self::now_ms();
        
        // Apply decay since last activity
        self.apply_decay(now);
        
        // Boost engagement from activity
        let engagement_gain = intensity * 0.2;  // up to 0.2 per activity
        self.engagement_level = (self.engagement_level + engagement_gain).min(1.0);
        
        self.activity_history.push(ActivityRecord {
            timestamp_ms: now,
            activity_type: activity_type.to_string(),
            intensity,
            engagement_gain,
        });

        if self.activity_history.len() > 1000 {
            self.activity_history.remove(0);
        }

        self.last_activity_ms = now;
        self.stimulation_need = 0.0;
    }

    /// Apply engagement decay over time
    pub fn apply_decay(&mut self, current_ms: u64) {
        let time_since_activity_ms = current_ms.saturating_sub(self.last_activity_ms);
        let decay_amount = self.decay_rate * (time_since_activity_ms as f32 / 1000.0);
        
        self.engagement_level = (self.engagement_level - decay_amount).max(0.0);
        self.stimulation_need = (1.0 - self.engagement_level).max(0.0);
    }

    /// Check if system needs stimulation (novelty seeking)
    pub fn needs_stimulation(&mut self) -> bool {
        let now = Self::now_ms();
        self.apply_decay(now);
        self.engagement_level < self.novelty_threshold
    }

    /// Get recommended stimulation activities
    pub fn get_stimulation_activities(&self) -> Vec<&'static str> {
        let mut activities = Vec::new();
        
        if self.engagement_level < 0.2 {
            // Critical: major novelty needed
            activities.push("explore_new_patterns");
            activities.push("cross_modal_learning");
        } else if self.engagement_level < 0.4 {
            // Low: moderate novelty
            activities.push("challenge_existing_pathways");
            activities.push("learn_new_domain");
        } else if self.engagement_level < 0.6 {
            // Moderate: light stimulation
            activities.push("recall_with_variations");
            activities.push("make_new_associations");
        }
        
        activities
    }

    /// Get activity statistics
    pub fn stats(&self) -> (f32, u64, usize, f32) {
        let now = Self::now_ms();
        let time_since_last = now.saturating_sub(self.last_activity_ms);
        let activity_count = self.activity_history.len();
        
        let recent_avg_intensity = if !self.activity_history.is_empty() {
            self.activity_history.iter()
                .rev()
                .take(10)
                .map(|a| a.intensity)
                .sum::<f32>() / self.activity_history.len().min(10) as f32
        } else {
            0.0
        };

        (self.engagement_level, time_since_last, activity_count, recent_avg_intensity)
    }

    /// Get activity diversity (prevents repetitive patterns)
    pub fn activity_diversity(&self) -> f32 {
        if self.activity_history.is_empty() {
            return 0.0;
        }

        let mut type_counts = HashMap::new();
        for activity in &self.activity_history {
            *type_counts.entry(activity.activity_type.clone()).or_insert(0) += 1;
        }

        let unique_types = type_counts.len() as f32;
        let total_activities = self.activity_history.len() as f32;
        
        (unique_types / total_activities).min(1.0)
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("mental_stimulation.bin");
        let mut data = Vec::new();

        data.extend_from_slice(b"STIM");
        data.push(1);

        data.extend_from_slice(&self.engagement_level.to_le_bytes());
        data.extend_from_slice(&self.last_activity_ms.to_le_bytes());
        data.extend_from_slice(&self.stimulation_need.to_le_bytes());

        let activity_count = self.activity_history.len() as u32;
        data.extend_from_slice(&activity_count.to_le_bytes());

        for activity in &self.activity_history {
            data.extend_from_slice(&activity.timestamp_ms.to_le_bytes());
            data.extend_from_slice(&activity.intensity.to_le_bytes());
            data.extend_from_slice(&activity.engagement_gain.to_le_bytes());

            let type_bytes = activity.activity_type.as_bytes();
            data.push(type_bytes.len() as u8);
            data.extend_from_slice(type_bytes);
        }

        let tmp_path = dir.join("mental_stimulation.bin.tmp");
        fs::write(&tmp_path, data).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("mental_stimulation.bin");
        if !path.exists() {
            return Ok(Self::new());
        }

        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() < 5 || &data[0..4] != b"STIM" {
            return Ok(Self::new());
        }

        let mut idx = 5;
        let engagement_level = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
        idx += 4;

        let last_activity_ms = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                 data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
        idx += 8;

        let stimulation_need = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
        idx += 4;

        let mut activity_history = Vec::new();
        
        if idx + 4 <= data.len() {
            let activity_count = u32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]) as usize;
            idx += 4;

            for _ in 0..activity_count {
                if idx + 20 > data.len() { break; }

                let timestamp_ms = u64::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3],
                                                     data[idx+4], data[idx+5], data[idx+6], data[idx+7]]);
                idx += 8;

                let intensity = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                let engagement_gain = f32::from_le_bytes([data[idx], data[idx+1], data[idx+2], data[idx+3]]);
                idx += 4;

                if idx >= data.len() { break; }
                let type_len = data[idx] as usize;
                idx += 1;

                if idx + type_len > data.len() { break; }
                let activity_type = String::from_utf8_lossy(&data[idx..idx+type_len]).to_string();
                idx += type_len;

                activity_history.push(ActivityRecord {
                    timestamp_ms,
                    activity_type,
                    intensity,
                    engagement_gain,
                });
            }
        }

        Ok(Self {
            activity_history,
            engagement_level,
            decay_rate: 0.001,
            last_activity_ms,
            stimulation_need,
            novelty_threshold: 0.3,
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