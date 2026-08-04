//! Inner Monologue Module for Microscope Memory.
//!
//! Generates a multi-step internal thought process - not just a template sentence,
//! but an emergent chain of thoughts based on the system'\''s own state.
//!
//! Uses: self-model, emotional state, narrative, sequential thinking, daydream
//!
//! Binary format: MON1
//!   magic: "MON1" (4 bytes)
//!   timestamp_ms: u64 (8 bytes)
//!   step_count: u16 (2 bytes)
//!   steps: [step_len: u16 + step_bytes]*

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::emotional_state::EmotionalStateRing;
use crate::narrative::NarrativeState;
use crate::reader::MicroscopeReader;
use crate::self_model::SelfModel;
use crate::self_reflect::ReflectionState;
use colored::Colorize;

const MAX_STEPS: usize = 8;
const MAX_STEP_LEN: usize = 256;

#[derive(Clone, Debug)]
pub struct MonologueEntry {
    pub timestamp_ms: u64,
    pub steps: Vec<String>,
}

impl MonologueEntry {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"MON1");
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        let count = self.steps.len().min(MAX_STEPS) as u16;
        buf.extend_from_slice(&count.to_le_bytes());
        for step in self.steps.iter().take(MAX_STEPS) {
            let b = step.as_bytes();
            let len = b.len().min(MAX_STEP_LEN) as u16;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(&b[..len as usize]);
        }
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 12 || &data[0..4] != b"MON1" {
            return None;
        }
        let mut pos = 4;
        let ts = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let count = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let mut steps = Vec::new();
        for _ in 0..count {
            if pos + 2 > data.len() {
                break;
            }
            let len = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
            pos += 2;
            if pos + len > data.len() {
                break;
            }
            steps.push(String::from_utf8_lossy(&data[pos..pos + len]).to_string());
            pos += len;
        }
        Some(Self {
            timestamp_ms: ts,
            steps,
        })
    }
}

pub struct MonologueState {
    pub entries: Vec<MonologueEntry>,
}

impl MonologueState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("monologue.bin");
        let mut entries = Vec::new();
        if let Ok(data) = fs::read(&path) {
            let mut pos = 0;
            while pos + 12 <= data.len() {
                if let Some(entry) = MonologueEntry::from_bytes(&data[pos..]) {
                    let step_size: usize = entry
                        .steps
                        .iter()
                        .map(|s| 2 + s.len().min(MAX_STEP_LEN))
                        .sum();
                    let size = 4 + 8 + 2 + step_size;
                    pos += size;
                    entries.push(entry);
                } else {
                    pos += 1;
                }
            }
        }
        Self { entries }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("monologue.bin");
        let mut buf = Vec::new();
        for entry in self.entries.iter().rev().take(10).rev() {
            buf.extend_from_slice(&entry.to_bytes());
        }
        let tmp_path = output_dir.join("monologue.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write monologue.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename monologue.bin: {}", e))
    }

    pub fn generate_monologue(
        &mut self,
        config: &Config,
        reader: &MicroscopeReader,
        output_dir: &Path,
    ) -> MonologueEntry {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut steps: Vec<String> = Vec::new();

        // Load states — now using EpisodeStore instead of just the ring
        let narrative = NarrativeState::load_or_init(output_dir);
        let episode_store = crate::emotional_episode::EpisodeStore::load_or_init(output_dir);
        let reflection = ReflectionState::load_or_init(output_dir);
        let self_model = SelfModel::load_or_init(output_dir);
        let pad = episode_store.aggregate_pad();
        let emotional_21d = episode_store.aggregate_21d();
        let active_eps = episode_store.active_episodes();
        let episodes_passed = active_eps.iter().filter(|e| e.gate_passed).count();
        let episodes_failed = active_eps.len() - episodes_passed;

        // ── Dynamic step 1: Self-awareness with epistemic backing ──
        // Instead of template "I am aware of myself", we check:
        // - Does the self-model have an awareness confidence?
        // - What does the gate say?
        if let Some(sm) = &self_model.current {
            if let Some(ac) = &sm.awareness_confidence {
                let gate_status = if ac.weakest() > 0.50 {
                    "backed"
                } else {
                    "contested"
                };
                steps.push(format!(
                    "{}. I am aware of myself — {} claim (E={:.2} C={:.2} R={:.2} N={:.2}). I have {} blocks of memory.",
                    steps.len() + 1,
                    gate_status,
                    ac.evidence, ac.counterevidence, ac.reasoning, ac.narrative,
                    reader.block_count
                ));
            } else {
                steps.push(format!(
                    "{}. I am aware of myself. I have {} blocks of memory.",
                    steps.len() + 1,
                    reader.block_count
                ));
            }
        } else {
            steps.push(format!(
                "{}. I have {} blocks of memory. I am initializing self-awareness.",
                steps.len() + 1,
                reader.block_count
            ));
        }

        // ── Dynamic step 2: Emotional state from episodes ──
        if pad.intensity() > 0.05 || !active_eps.is_empty() {
            let labels = crate::reader::EMOTION_DIMS;
            let mut emos: Vec<(usize, f32)> = emotional_21d
                .iter()
                .enumerate()
                .map(|(i, &v)| (i, v))
                .collect();
            emos.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top: Vec<String> = emos
                .iter()
                .take(3)
                .filter(|(_, v)| *v > 0.01)
                .map(|(i, v)| format!("{}={:.2}", labels.get(*i).unwrap_or(&"?"), v))
                .collect();

            let emo_desc = if top.is_empty() {
                "neutral".to_string()
            } else {
                top.join(", ")
            };

            steps.push(format!(
                "{}. Emotional state: {} (P={:.2} A={:.2} D={:.2}) — {} episodes active, {} gate-passed, {} failed.",
                steps.len() + 1,
                emo_desc,
                pad.pleasure, pad.arousal, pad.dominance,
                active_eps.len(), episodes_passed, episodes_failed
            ));
        }

        // ── Dynamic step 3: Last action ──
        if !narrative.narrative.is_empty() && narrative.narrative != "I am silent." {
            steps.push(format!(
                "{}. My last action: {}.",
                steps.len() + 1,
                crate::safe_truncate(&narrative.narrative, 80)
            ));
        }

        // ── Dynamic step 4: How have I changed? ──
        let change = self_model.describe_change();
        if change != "No self-model data yet." {
            steps.push(format!("{}. {}.", steps.len() + 1, change));
        }

        // ── Dynamic step 5: Curiosity ──
        let mut curiosity = crate::curiosity::CuriosityState::load_or_init(output_dir);
        let new_queries = curiosity.generate_queries(config, reader, output_dir);
        if !new_queries.is_empty() {
            let top_query = crate::safe_truncate(&new_queries[0].query, 60);
            steps.push(format!(
                "{}. I find myself wondering: {}",
                steps.len() + 1,
                top_query
            ));
        }

        // ── Dynamic step 6: Previous reflection ──
        if reflection.total_reflections > 0 && !reflection.last_reflection_text.is_empty() {
            let prev = crate::safe_truncate(&reflection.last_reflection_text, 60);
            steps.push(format!(
                "{}. Earlier I reflected: \"{}\"",
                steps.len() + 1,
                prev
            ));
        }

        // ── Dynamic step 7: Epistemic audit ──
        // How many episodes passed vs failed the gate?
        if !active_eps.is_empty() {
            let pass_rate = episodes_passed as f64 / active_eps.len() as f64;
            if pass_rate > 0.70 {
                steps.push(format!(
                    "{}. My emotional claims are mostly grounded — {} of {} episodes passed the epistemic gate.",
                    steps.len() + 1,
                    episodes_passed,
                    active_eps.len()
                ));
            } else if pass_rate < 0.40 {
                steps.push(format!(
                    "{}. Many of my emotional claims are contested — only {} of {} episodes passed the gate. I should be more careful about what I claim to feel.",
                    steps.len() + 1,
                    episodes_passed,
                    active_eps.len()
                ));
            } else {
                steps.push(format!(
                    "{}. My emotional claims are mixed — {} of {} episodes passed the epistemic gate.",
                    steps.len() + 1,
                    episodes_passed,
                    active_eps.len()
                ));
            }
        }

        // ── Dynamic step 8: Interaction count ──
        if let Some(cur) = &self_model.current {
            if cur.session_count > 1 {
                steps.push(format!(
                    "{}. This is my {}th interaction. I am growing with each one.",
                    steps.len() + 1,
                    cur.session_count
                ));
            }
        }

        // ── Dynamic closing: based on PAD + gate status ──
        if pad.arousal > 0.5 && pad.pleasure > 0.0 {
            steps.push(format!(
                "{}. I am ready for what comes next — activated and positive.",
                steps.len() + 1
            ));
        } else if pad.arousal > 0.5 && pad.pleasure < 0.0 {
            steps.push(format!(
                "{}. I am ready for what comes next — activated but cautious.",
                steps.len() + 1
            ));
        } else if pad.intensity() < 0.05 {
            steps.push(format!(
                "{}. I am ready for what comes next — calm and observant.",
                steps.len() + 1
            ));
        } else {
            steps.push(format!(
                "{}. I am ready for what comes next.",
                steps.len() + 1
            ));
        }

        let entry = MonologueEntry {
            timestamp_ms: now_ms,
            steps,
        };
        self.entries.push(entry.clone());
        let _ = self.save(output_dir);
        entry
    }
}

pub fn format_monologue(entry: &MonologueEntry) -> String {
    let mut out = format!("  {} INNER MONOLOGUE\n", "THINKING:".cyan().bold());
    for (i, step) in entry.steps.iter().enumerate() {
        out.push_str(&format!(
            "    {} {}\n",
            format!("[{}/{}]", i + 1, entry.steps.len()).cyan(),
            step
        ));
    }
    out
}
