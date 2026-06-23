//! Self-Reflection Module for Microscope Memory.
//!
//! Periodically introspects the system'\''s own cognitive state and generates
//! a self-reflective narrative - the system "thinking about itself."

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::archetype::ArchetypeState;
use crate::attention::AttentionState;
use crate::config::Config;
use crate::emotional_state::EmotionalStateRing;
use crate::hebbian::HebbianState;
use crate::narrative::NarrativeState;
use crate::reader::MicroscopeReader;
use crate::spaced_repetition::SpacedRepetition;
use crate::thought_graph::ThoughtGraphState;
use colored::Colorize;

pub const AUTO_REFLECT_INTERVAL: usize = 5;
const MAX_REFLECTION_BYTES: usize = 2048;

pub struct ReflectionState {
    pub last_reflection_ms: u64,
    pub total_reflections: u64,
    pub last_reflection_text: String,
}

impl ReflectionState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("self_reflect.bin");
        if path.exists() {
            if let Ok(data) = fs::read(&path) {
                if data.len() >= 14 && &data[0..4] == b"SELF" {
                    let ts = u64::from_le_bytes(data[4..12].try_into().unwrap_or([0; 8]));
                    let count = u64::from_le_bytes(data[12..20].try_into().unwrap_or([0; 8]));
                    let rlen =
                        u16::from_le_bytes(data[20..22].try_into().unwrap_or([0; 2])) as usize;
                    let text = if rlen > 0 && 22 + rlen <= data.len() {
                        String::from_utf8_lossy(&data[22..22 + rlen]).to_string()
                    } else {
                        String::new()
                    };
                    return Self {
                        last_reflection_ms: ts,
                        total_reflections: count,
                        last_reflection_text: text,
                    };
                }
            }
        }
        Self {
            last_reflection_ms: 0,
            total_reflections: 0,
            last_reflection_text: String::new(),
        }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("self_reflect.bin");
        let text_bytes = self.last_reflection_text.as_bytes();
        let rlen = text_bytes.len().min(MAX_REFLECTION_BYTES) as u16;
        let mut buf = Vec::with_capacity(22 + rlen as usize);
        buf.extend_from_slice(b"SELF");
        buf.extend_from_slice(&self.last_reflection_ms.to_le_bytes());
        buf.extend_from_slice(&self.total_reflections.to_le_bytes());
        buf.extend_from_slice(&rlen.to_le_bytes());
        buf.extend_from_slice(&text_bytes[..rlen as usize]);
        let tmp_path = output_dir.join("self_reflect.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write self_reflect.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename self_reflect.bin: {}", e))
    }
}

pub fn introspect(config: &Config, reader: &MicroscopeReader, output_dir: &Path) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
    let attention = AttentionState::load_or_init(output_dir);
    let archetypes = ArchetypeState::load_or_init(output_dir);
    let narrative = NarrativeState::load_or_init(output_dir);
    let esr = EmotionalStateRing::load_or_init(output_dir);
    let spaced = SpacedRepetition::load_or_init(output_dir);
    let thought_graph = ThoughtGraphState::load_or_init(output_dir);
    let reflection = ReflectionState::load_or_init(output_dir);

    let mut parts: Vec<String> = Vec::new();

    // 1. Emotional state
    if esr.is_active() {
        let labels = crate::reader::EMOTION_DIMS;
        let mut emotions: Vec<(usize, f32)> = esr
            .current
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v))
            .collect();
        emotions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top: Vec<String> = emotions
            .iter()
            .take(3)
            .filter(|(_, v)| *v > 0.1)
            .map(|(i, v)| {
                let intensity = if *v > 0.6 {
                    "strongly "
                } else if *v > 0.3 {
                    ""
                } else {
                    "slightly "
                };
                format!("{}{}", intensity, labels.get(*i).unwrap_or(&"?"))
            })
            .collect();
        if !top.is_empty() {
            parts.push(format!("I feel {}", top.join(" and ")));
        }
    }

    // 2. Attention focus
    let layer_names = [
        "Hebbian",
        "Mirror",
        "Resonance",
        "Archetype",
        "Emotional",
        "ThoughtGraph",
        "PredictiveCache",
    ];
    let mut layers: Vec<(usize, f32)> = attention
        .learned_weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w))
        .collect();
    layers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((best_idx, best_w)) = layers.first() {
        let name = layer_names.get(*best_idx).unwrap_or(&"?");
        parts.push(format!(
            "my {} layer is most active ({:.0}%)",
            name,
            best_w * 100.0
        ));
    }

    // 3. Hebbian activity
    let hot_count = hebb.activations.iter().filter(|a| a.energy > 0.1).count();
    let total_energy: f32 = hebb.activations.iter().map(|a| a.energy).sum();
    if hot_count > 0 {
        parts.push(format!(
            "{} hot memories (energy={:.1})",
            hot_count, total_energy
        ));
    }

    // 4. Archetypes
    if !archetypes.archetypes.is_empty() {
        let arch_labels: Vec<&str> = archetypes
            .archetypes
            .iter()
            .take(3)
            .map(|a| a.label.as_str())
            .collect();
        parts.push(format!(
            "{} archetypes: {}",
            archetypes.archetypes.len(),
            arch_labels.join(", ")
        ));
    }

    // 5. Spaced repetition
    let due = spaced.due_count();
    let mastered = spaced.mastered_count();
    if due > 0 || mastered > 0 {
        parts.push(format!("{} memories due, {} mastered", due, mastered));
    }

    // 6. Thought patterns
    let patterns = thought_graph.crystallized_count();
    if patterns > 0 {
        parts.push(format!("{} thought patterns crystallized", patterns));
    }

    // 7. Memory stats
    parts.push(format!("{} total blocks", reader.block_count));

    // 8. Previous reflection context
    if reflection.total_reflections > 0 && !reflection.last_reflection_text.is_empty() {
        let prev = crate::safe_truncate(&reflection.last_reflection_text, 60);
        parts.push(format!("previously I reflected: \"{}\"", prev));
    }

    // 9. Session count
    if narrative.session_count > 0 {
        parts.push(format!(
            "this is my {}th interaction",
            narrative.session_count
        ));
    }

    let reflection_text = if parts.is_empty() {
        "I am aware of myself but have no distinct feelings right now.".to_string()
    } else {
        parts.join(". ") + "."
    };

    let mut state = ReflectionState::load_or_init(output_dir);
    state.last_reflection_ms = now_ms;
    state.total_reflections += 1;
    state.last_reflection_text = reflection_text.clone();
    let _ = state.save(output_dir);

    reflection_text
}

pub fn format_reflection(text: &str) -> String {
    format!("  {} {}", "SELF:".cyan().bold(), text)
}
