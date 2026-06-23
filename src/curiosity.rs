//! Proactive Curiosity Module for Microscope Memory.
//!
//! The system autonomously identifies interesting blocks and generates queries.
//! Uses: Eureka surprise, Hebbian energy, spaced repetition due/overdue,
//! emotional charge, and low familiarity to find "curious" signals.
//!
//! Binary format: CUR1
//!   magic: "CUR1" (4 bytes)
//!   timestamp_ms: u64 (8 bytes)
//!   query_len: u16 (2 bytes) + query bytes
//!   reason_len: u16 (2 bytes) + reason bytes
//!   score: f32 (4 bytes)

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::hebbian::HebbianState;
use crate::reader::MicroscopeReader;
use crate::spaced_repetition::SpacedRepetition;
use colored::Colorize;

const MAX_CURIOSITY_QUERIES: usize = 20;
const MAX_STR_LEN: usize = 256;

#[derive(Clone, Debug)]
pub struct CuriosityQuery {
    pub timestamp_ms: u64,
    pub query: String,
    pub reason: String,
    pub score: f32,
}

impl CuriosityQuery {
    pub fn to_bytes(&self) -> Vec<u8> {
        let qb = self.query.as_bytes();
        let rb = self.reason.as_bytes();
        let qlen = qb.len().min(MAX_STR_LEN) as u16;
        let rlen = rb.len().min(MAX_STR_LEN) as u16;
        let mut buf = Vec::with_capacity(20 + qlen as usize + rlen as usize);
        buf.extend_from_slice(b"CUR1");
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&qlen.to_le_bytes());
        buf.extend_from_slice(&qb[..qlen as usize]);
        buf.extend_from_slice(&rlen.to_le_bytes());
        buf.extend_from_slice(&rb[..rlen as usize]);
        buf.extend_from_slice(&self.score.to_le_bytes());
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 20 || &data[0..4] != b"CUR1" {
            return None;
        }
        let mut pos = 4;
        let ts = u64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        pos += 8;
        let qlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let query = if qlen > 0 {
            String::from_utf8_lossy(&data[pos..pos + qlen]).to_string()
        } else {
            String::new()
        };
        pos += qlen;
        let rlen = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap_or([0; 2])) as usize;
        pos += 2;
        let reason = if rlen > 0 {
            String::from_utf8_lossy(&data[pos..pos + rlen]).to_string()
        } else {
            String::new()
        };
        pos += rlen;
        let score = f32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        Some(Self {
            timestamp_ms: ts,
            query,
            reason,
            score,
        })
    }
}

pub struct CuriosityState {
    pub queries: Vec<CuriosityQuery>,
}

impl CuriosityState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("curiosity.bin");
        let mut queries = Vec::new();
        if let Ok(data) = fs::read(&path) {
            let mut pos = 0;
            while pos + 20 <= data.len() {
                if let Some(q) = CuriosityQuery::from_bytes(&data[pos..]) {
                    let size = 4
                        + 8
                        + 2
                        + q.query.len().min(MAX_STR_LEN)
                        + 2
                        + q.reason.len().min(MAX_STR_LEN)
                        + 4;
                    pos += size;
                    queries.push(q);
                } else {
                    pos += 1;
                }
            }
        }
        Self { queries }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("curiosity.bin");
        let mut buf = Vec::new();
        for q in self.queries.iter().rev().take(MAX_CURIOSITY_QUERIES).rev() {
            buf.extend_from_slice(&q.to_bytes());
        }
        let tmp_path = output_dir.join("curiosity.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write curiosity.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename curiosity.bin: {}", e))
    }

    pub fn generate_queries(
        &mut self,
        config: &Config,
        reader: &MicroscopeReader,
        output_dir: &Path,
    ) -> Vec<CuriosityQuery> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
        let spaced = SpacedRepetition::load_or_init(output_dir);
        let mut new_queries = Vec::new();

        // 1. Check due blocks - memories that need review
        let due = spaced.due_blocks();
        if !due.is_empty() {
            let texts: Vec<String> = due
                .iter()
                .take(3)
                .map(|&idx| {
                    let text = reader.text(idx as usize);
                    crate::safe_truncate(text, 60)
                })
                .collect();
            if !texts.is_empty() {
                new_queries.push(CuriosityQuery {
                    timestamp_ms: now_ms,
                    query: format!("Why have I not recalled {} recently?", texts.join(", ")),
                    reason: format!(
                        "{} memories are due for spaced repetition review",
                        due.len()
                    ),
                    score: 0.5 + (due.len() as f32 * 0.05).min(0.5),
                });
            }
        }

        // 2. Find hot blocks with high energy but low familiarity
        let mut hot_blocks: Vec<(u32, f32)> = hebb
            .activations
            .iter()
            .enumerate()
            .filter(|(_, a)| a.energy > 0.2)
            .map(|(i, a)| (i as u32, a.energy))
            .collect();
        hot_blocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for &(idx, energy) in hot_blocks.iter().take(2) {
            let text = reader.text(idx as usize);
            let preview = crate::safe_truncate(text, 50);
            new_queries.push(CuriosityQuery {
                timestamp_ms: now_ms,
                query: format!("What makes '{}' so active?", preview),
                reason: format!(
                    "block {} has energy {:.2} - highest in the system",
                    idx, energy
                ),
                score: energy * 0.8,
            });
        }

        // 3. Check archetypes - emerging patterns
        let archetypes = crate::archetype::ArchetypeState::load_or_init(output_dir);
        if !archetypes.archetypes.is_empty() {
            for arch in archetypes.archetypes.iter().take(2) {
                new_queries.push(CuriosityQuery {
                    timestamp_ms: now_ms,
                    query: format!(
                        "What is the '{}' archetype and why is it forming?",
                        arch.label
                    ),
                    reason: format!(
                        "archetype '{}' has {} members and strength {:.3}",
                        arch.label,
                        arch.members.len(),
                        arch.strength
                    ),
                    score: arch.strength * 0.7,
                });
            }
        }

        // 4. Check thought patterns
        let thought_graph = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
        let patterns = thought_graph.top_patterns(2);
        for p in patterns {
            new_queries.push(CuriosityQuery {
                timestamp_ms: now_ms,
                query: format!("What connects the blocks in this thought pattern?"),
                reason: format!(
                    "thought pattern with {} blocks, frequency {}",
                    p.result_blocks.len(),
                    p.frequency
                ),
                score: (p.frequency as f32) * 0.1,
            });
        }

        // 5. General curiosity about system state
        new_queries.push(CuriosityQuery {
            timestamp_ms: now_ms,
            query: format!("How am I changing over time?"),
            reason: format!(
                "{} total blocks, {} hot memories, {} due for review",
                reader.block_count,
                hot_blocks.len(),
                due.len()
            ),
            score: 0.3,
        });

        self.queries.extend(new_queries.clone());
        if self.queries.len() > MAX_CURIOSITY_QUERIES * 2 {
            self.queries = self.queries[self.queries.len() - MAX_CURIOSITY_QUERIES..].to_vec();
        }
        let _ = self.save(output_dir);
        new_queries
    }
}

pub fn format_curiosity(queries: &[CuriosityQuery]) -> String {
    let mut out = format!("  {} I am curious about:\n", "CURIOUS:".yellow().bold());
    for q in queries.iter().take(5) {
        out.push_str(&format!(
            "    \x20 [{:.2}] {} ({})\n",
            q.score, q.query, q.reason
        ));
    }
    out
}
