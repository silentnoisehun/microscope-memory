//! Narrative Memory Module for Microscope Memory.
//!
//! Automatically links sequences of recalls into coherent narratives -
//! story arcs that emerge from thought patterns and can be replayed
//! as structured episodes.
//!
//! Binary format: NVM1
//!   magic: "NVM1" (4 bytes)
//!   timestamp_ms: u64 (8 bytes)
//!   episode_id: u32 (4 bytes)
//!   title_len: u16 (2 bytes) + title bytes
//!   summary_len: u16 (2 bytes) + summary bytes
//!   block_count: u16 (2 bytes) + block_indices (u32 each)
//!   emotional_arc: [f32; 21] (84 bytes)

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::reader::MicroscopeReader;
use crate::thought_graph::ThoughtGraphState;
use colored::Colorize;

const MAX_STR_LEN: usize = 512;
const MAX_EPISODES: usize = 50;

#[derive(Clone, Debug)]
pub struct NarrativeEpisode {
    pub timestamp_ms: u64,
    pub episode_id: u32,
    pub title: String,
    pub summary: String,
    pub block_indices: Vec<u32>,
    pub emotional_arc: [f32; 21],
}

impl NarrativeEpisode {
    pub fn to_bytes(&self) -> Vec<u8> {
        let title_b = self.title.as_bytes();
        let summary_b = self.summary.as_bytes();
        let tlen = title_b.len().min(MAX_STR_LEN) as u16;
        let slen = summary_b.len().min(MAX_STR_LEN) as u16;
        let bcount = self.block_indices.len().min(50) as u16;
        let mut buf = Vec::with_capacity(120 + tlen as usize + slen as usize + bcount as usize * 4);
        buf.extend_from_slice(b"NVM1");
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&self.episode_id.to_le_bytes());
        buf.extend_from_slice(&tlen.to_le_bytes());
        buf.extend_from_slice(&title_b[..tlen as usize]);
        buf.extend_from_slice(&slen.to_le_bytes());
        buf.extend_from_slice(&summary_b[..slen as usize]);
        buf.extend_from_slice(&bcount.to_le_bytes());
        for &idx in self.block_indices.iter().take(50) {
            buf.extend_from_slice(&idx.to_le_bytes());
        }
        for v in self.emotional_arc {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 120 || &data[0..4] != b"NVM1" {
            return None;
        }
        let mut pos = 4;
        let ts = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let eid = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        pos += 4;
        let tlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let title = if tlen > 0 {
            String::from_utf8_lossy(&data[pos..pos + tlen]).to_string()
        } else {
            String::new()
        };
        pos += tlen;
        let slen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let summary = if slen > 0 {
            String::from_utf8_lossy(&data[pos..pos + slen]).to_string()
        } else {
            String::new()
        };
        pos += slen;
        let bcount = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let mut indices = Vec::new();
        for _ in 0..bcount {
            let idx = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
            pos += 4;
            indices.push(idx);
        }
        let mut emo = [0.0f32; 21];
        for i in 0..21 {
            emo[i] = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
            pos += 4;
        }
        Some(Self {
            timestamp_ms: ts,
            episode_id: eid,
            title,
            summary,
            block_indices: indices,
            emotional_arc: emo,
        })
    }
}

pub struct NarrativeMemory {
    pub episodes: Vec<NarrativeEpisode>,
    pub next_id: u32,
}

impl NarrativeMemory {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("narrative_memory.bin");
        let mut episodes = Vec::new();
        let mut next_id = 1u32;
        if let Ok(data) = fs::read(&path) {
            let mut pos = 0;
            while pos + 120 <= data.len() {
                if let Some(ep) = NarrativeEpisode::from_bytes(&data[pos..]) {
                    let size = 4
                        + 8
                        + 4
                        + 2
                        + ep.title.len().min(MAX_STR_LEN)
                        + 2
                        + ep.summary.len().min(MAX_STR_LEN)
                        + 2
                        + ep.block_indices.len().min(50) * 4
                        + 84;
                    pos += size;
                    if ep.episode_id >= next_id {
                        next_id = ep.episode_id + 1;
                    }
                    episodes.push(ep);
                } else {
                    pos += 1;
                }
            }
        }
        Self { episodes, next_id }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("narrative_memory.bin");
        let mut buf = Vec::new();
        for ep in self.episodes.iter().rev().take(MAX_EPISODES).rev() {
            buf.extend_from_slice(&ep.to_bytes());
        }
        let tmp_path = output_dir.join("narrative_memory.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write narrative_memory.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename narrative_memory.bin: {}", e))
    }

    pub fn build_episode(
        &mut self,
        config: &Config,
        reader: &MicroscopeReader,
        output_dir: &Path,
        query: &str,
        results: &[(f32, usize, bool)],
    ) -> Option<NarrativeEpisode> {
        if results.is_empty() {
            return None;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Collect main index results
        let indices: Vec<u32> = results
            .iter()
            .filter(|(_, _, is_main)| *is_main)
            .take(10)
            .map(|(_, idx, _)| *idx as u32)
            .collect();
        if indices.is_empty() {
            return None;
        }

        // Build title from query
        let title = format!("Episode: {}", crate::safe_truncate(query, 60));

        // Build summary from result texts
        let texts: Vec<String> = indices
            .iter()
            .take(5)
            .map(|&idx| {
                let text = reader.text(idx as usize);
                crate::safe_truncate(text, 80)
            })
            .collect();
        let summary = if texts.len() == 1 {
            format!("Recalled: {}", texts[0])
        } else {
            format!("Connected {} memories: {}", texts.len(), texts.join(" -> "))
        };

        // Get emotional arc from emotional state
        let esr = crate::emotional_state::EmotionalStateRing::load_or_init(output_dir);
        let emotional_arc = if esr.is_active() {
            esr.current
        } else {
            [0.0f32; 21]
        };

        let episode = NarrativeEpisode {
            timestamp_ms: now_ms,
            episode_id: self.next_id,
            title,
            summary,
            block_indices: indices,
            emotional_arc,
        };
        self.next_id += 1;
        self.episodes.push(episode.clone());
        let _ = self.save(output_dir);
        Some(episode)
    }

    pub fn recent_episodes(&self, n: usize) -> Vec<&NarrativeEpisode> {
        self.episodes.iter().rev().take(n).collect()
    }
}

pub fn format_episode(ep: &NarrativeEpisode) -> String {
    let labels = crate::reader::EMOTION_DIMS;
    let mut emos: Vec<(usize, f32)> = ep
        .emotional_arc
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    emos.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_emo: Vec<String> = emos
        .iter()
        .take(2)
        .filter(|(_, v)| *v > 0.1)
        .map(|(i, v)| format!("{}={:.2}", labels[*i], v))
        .collect();

    format!(
        "  {} [#{}] {}\n\
         \x20       {}\n\
         \x20       {} blocks, emotion: {}\n\
         \x20       {}",
        "STORY:".cyan().bold(),
        ep.episode_id,
        ep.title,
        ep.summary,
        ep.block_indices.len(),
        if top_emo.is_empty() {
            "neutral".to_string()
        } else {
            top_emo.join(", ")
        },
        chrono_str(ep.timestamp_ms),
    )
}

fn chrono_str(ts_ms: u64) -> String {
    let secs = ts_ms / 1000;
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!("{}d {}h {}m ago", days, hours, mins)
}
