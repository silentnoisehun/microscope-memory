//! Self-Model Module for Microscope Memory.
//!
//! Builds and maintains a model of the system'\''s own cognitive state.
//! Tracks changes over time: "who am I now, and how have I changed?"
//!
//! Binary format: SLF1
//!   magic: "SLF1" (4 bytes)
//!   timestamp_ms: u64 (8 bytes)
//!   version: u16 (2 bytes)
//!   emotional_state: [f32; 21] (84 bytes)
//!   attention_weights: [f32; 7] (28 bytes)
//!   hebbian_energy: f32 (4 bytes)
//!   hot_count: u32 (4 bytes)
//!   archetype_count: u32 (4 bytes)
//!   pattern_count: u32 (4 bytes)
//!   block_count: u32 (4 bytes)
//!   session_count: u64 (8 bytes)
//!   narrative_len: u16 (2 bytes) + narrative bytes
//!   reflection_len: u16 (2 bytes) + reflection bytes

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
use crate::self_reflect::ReflectionState;
use crate::spaced_repetition::SpacedRepetition;
use crate::thought_graph::ThoughtGraphState;
use colored::Colorize;

const MAX_STR_LEN: usize = 512;

#[derive(Clone, Debug)]
pub struct SelfModelSnapshot {
    pub timestamp_ms: u64,
    pub emotional: [f32; 21],
    pub attention_weights: [f32; 7],
    pub hebbian_energy: f32,
    pub hot_count: u32,
    pub archetype_count: u32,
    pub pattern_count: u32,
    pub block_count: u32,
    pub session_count: u64,
    pub narrative: String,
    pub reflection: String,
}

impl SelfModelSnapshot {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 150 || &data[0..4] != b"SLF1" {
            return None;
        }
        let mut pos = 4;
        let ts = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let _ver = u16::from_le_bytes(data[pos..pos + 2].try_into().ok()?);
        pos += 2;
        let mut emo = [0.0f32; 21];
        for i in 0..21 {
            emo[i] = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
            pos += 4;
        }
        let mut attn = [0.0f32; 7];
        for i in 0..7 {
            attn[i] = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
            pos += 4;
        }
        let he = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let hc = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let ac = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let pc = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let bc = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let sc = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let nlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let narr = if nlen > 0 {
            String::from_utf8_lossy(&data[pos..pos + nlen]).to_string()
        } else {
            String::new()
        };
        pos += nlen;
        let rlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let refl = if rlen > 0 {
            String::from_utf8_lossy(&data[pos..pos + rlen]).to_string()
        } else {
            String::new()
        };
        Some(Self {
            timestamp_ms: ts,
            emotional: emo,
            attention_weights: attn,
            hebbian_energy: he,
            hot_count: hc,
            archetype_count: ac,
            pattern_count: pc,
            block_count: bc,
            session_count: sc,
            narrative: narr,
            reflection: refl,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let narr_b = self.narrative.as_bytes();
        let refl_b = self.reflection.as_bytes();
        let nlen = narr_b.len().min(MAX_STR_LEN) as u16;
        let rlen = refl_b.len().min(MAX_STR_LEN) as u16;
        let mut buf = Vec::with_capacity(200 + nlen as usize + rlen as usize);
        buf.extend_from_slice(b"SLF1");
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // version
        for v in self.emotional {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in self.attention_weights {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&self.hebbian_energy.to_le_bytes());
        buf.extend_from_slice(&self.hot_count.to_le_bytes());
        buf.extend_from_slice(&self.archetype_count.to_le_bytes());
        buf.extend_from_slice(&self.pattern_count.to_le_bytes());
        buf.extend_from_slice(&self.block_count.to_le_bytes());
        buf.extend_from_slice(&self.session_count.to_le_bytes());
        buf.extend_from_slice(&nlen.to_le_bytes());
        buf.extend_from_slice(&narr_b[..nlen as usize]);
        buf.extend_from_slice(&rlen.to_le_bytes());
        buf.extend_from_slice(&refl_b[..rlen as usize]);
        buf
    }
}

pub struct SelfModel {
    pub snapshots: Vec<SelfModelSnapshot>,
    pub current: Option<SelfModelSnapshot>,
    pub previous: Option<SelfModelSnapshot>,
}

impl SelfModel {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("self_model.bin");
        let mut snapshots = Vec::new();
        if let Ok(data) = fs::read(&path) {
            let mut pos = 0;
            while pos + 4 <= data.len() {
                if &data[pos..pos + 4] == b"SLF1" {
                    if let Some(snap) = SelfModelSnapshot::from_bytes(&data[pos..]) {
                        let size = 4
                            + 8
                            + 2
                            + 84
                            + 28
                            + 4
                            + 4
                            + 4
                            + 4
                            + 4
                            + 8
                            + 2
                            + snap.narrative.len().min(MAX_STR_LEN)
                            + 2
                            + snap.reflection.len().min(MAX_STR_LEN);
                        pos += size;
                        snapshots.push(snap);
                        continue;
                    }
                }
                pos += 1;
            }
        }
        let current = snapshots.last().cloned();
        let previous = if snapshots.len() >= 2 {
            snapshots.get(snapshots.len() - 2).cloned()
        } else {
            None
        };
        Self {
            snapshots,
            current,
            previous,
        }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("self_model.bin");
        let tmp_path = output_dir.join("self_model.bin.tmp");
        let mut buf = Vec::new();
        for snap in &self.snapshots {
            buf.extend_from_slice(&snap.to_bytes());
        }
        fs::write(&tmp_path, &buf).map_err(|e| format!("write self_model.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename self_model.bin: {}", e))?;
        Ok(())
    }

    pub fn take_snapshot(
        &mut self,
        config: &Config,
        reader: &MicroscopeReader,
        output_dir: &Path,
    ) -> SelfModelSnapshot {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
        let attention = AttentionState::load_or_init(output_dir);
        let archetypes = ArchetypeState::load_or_init(output_dir);
        let narrative = NarrativeState::load_or_init(output_dir);
        let esr = EmotionalStateRing::load_or_init(output_dir);
        let thought_graph = ThoughtGraphState::load_or_init(output_dir);
        let reflection = ReflectionState::load_or_init(output_dir);

        let hot_count = hebb.activations.iter().filter(|a| a.energy > 0.1).count() as u32;
        let hebbian_energy: f32 = hebb.activations.iter().map(|a| a.energy).sum();

        let snap = SelfModelSnapshot {
            timestamp_ms: now_ms,
            emotional: esr.current,
            attention_weights: attention.learned_weights,
            hebbian_energy,
            hot_count,
            archetype_count: archetypes.archetypes.len() as u32,
            pattern_count: thought_graph.crystallized_count() as u32,
            block_count: reader.block_count as u32,
            session_count: narrative.session_count,
            narrative: narrative.narrative.clone(),
            reflection: reflection.last_reflection_text.clone(),
        };

        self.previous = self.current.clone();
        self.current = Some(snap.clone());
        self.snapshots.push(snap.clone());
        let _ = self.save(output_dir);
        snap
    }

    pub fn describe_change(&self) -> String {
        match (&self.current, &self.previous) {
            (Some(cur), Some(prev)) => {
                let mut changes = Vec::new();
                let emo_labels = crate::reader::EMOTION_DIMS;
                for (i, label) in emo_labels.iter().enumerate() {
                    let diff = cur.emotional[i] - prev.emotional[i];
                    if diff.abs() > 0.1 {
                        let dir = if diff > 0.0 { "increased" } else { "decreased" };
                        changes.push(format!("{} {} by {:.2}", label, dir, diff.abs()));
                    }
                }
                let attn_labels = [
                    "Hebbian",
                    "Mirror",
                    "Resonance",
                    "Archetype",
                    "Emotional",
                    "ThoughtGraph",
                    "PredictiveCache",
                ];
                for (i, label) in attn_labels.iter().enumerate() {
                    let diff = cur.attention_weights[i] - prev.attention_weights[i];
                    if diff.abs() > 0.05 {
                        let dir = if diff > 0.0 { "up" } else { "down" };
                        changes.push(format!(
                            "{} focus {} by {:.0}%",
                            label,
                            dir,
                            diff.abs() * 100.0
                        ));
                    }
                }
                if cur.hot_count != prev.hot_count {
                    changes.push(format!(
                        "hot memories: {} -> {}",
                        prev.hot_count, cur.hot_count
                    ));
                }
                if cur.block_count != prev.block_count {
                    changes.push(format!(
                        "blocks: {} -> {}",
                        prev.block_count, cur.block_count
                    ));
                }
                if changes.is_empty() {
                    "I am stable, no significant changes.".to_string()
                } else {
                    format!("I have changed: {}", changes.join(", "))
                }
            }
            (Some(_), None) => "This is my first self-model snapshot.".to_string(),
            (None, _) => "No self-model data yet.".to_string(),
        }
    }
}

pub fn format_self_model(snap: &SelfModelSnapshot, change_desc: &str) -> String {
    let labels = crate::reader::EMOTION_DIMS;
    let mut emotions: Vec<(usize, f32)> = snap
        .emotional
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    emotions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_emo: Vec<String> = emotions
        .iter()
        .take(3)
        .filter(|(_, v)| *v > 0.1)
        .map(|(i, v)| format!("{}={:.2}", labels[*i], v))
        .collect();

    let attn_labels = [
        "Hebbian",
        "Mirror",
        "Resonance",
        "Archetype",
        "Emotional",
        "ThoughtGraph",
        "PredictiveCache",
    ];
    let mut attn: Vec<(usize, f32)> = snap
        .attention_weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w))
        .collect();
    attn.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_attn: Vec<String> = attn
        .iter()
        .take(3)
        .map(|(i, w)| format!("{}={:.0}%", attn_labels[*i], w * 100.0))
        .collect();

    format!(
        "  {} SELF-MODEL snapshot\n\
         \x20 emotion: {}\n\
         \x20 focus:   {}\n\
         \x20 state:   {} hot memories, {} archetypes, {} patterns, {} blocks\n\
         \x20 change:  {}\n\
         \x20 self:    \"{}\"",
        "SELF:".cyan().bold(),
        top_emo.join(", "),
        top_attn.join(", "),
        snap.hot_count,
        snap.archetype_count,
        snap.pattern_count,
        snap.block_count,
        change_desc,
        crate::safe_truncate(&snap.reflection, 80),
    )
}
