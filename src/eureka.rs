//! Eureka/Insight — váratlan összefüggések detektálása.
//!
//! Amikor a recall olyan blokkot hoz fel, ami térben távol van a query-től, de
//! emocionálisan vagy tematikusan erősen releváns, az egy "eureka pillanat" —
//! potenciálisan új, nem triviális összefüggés a memóriában.
//!
//! Binary format: EUR1
//!   magic: "EUR1" (4 bytes)
//!   count: u32 (4 bytes)
//!   events[count]:
//!     id: u64 | ts: u64 | qlen: u16 | query[] | tlen: u16 | text[]
//!     surprise: f32 | curiosity: f32 | spatial_dist: f32 | emotional_sim: f32
//!     layer_id: u8 | block_index: u32

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::reader::{emotional_similarity, load_emotion_lookup, EMOTION_DIMS};
use crate::{safe_truncate, MicroscopeReader};

// ─── Constants ──────────────────────────────────────

/// Minimum spatial distance (squared) to qualify as "distant" → potential eureka.
const EUREKA_MIN_SPATIAL_DIST: f32 = 0.05;
/// Minimum emotional similarity to qualify as eureka.
const EUREKA_MIN_EMO_SIM: f32 = 0.3;
/// Insight score threshold: surprise * curiosity * emo_sim / spatial_dist.
const EUREKA_MIN_INSIGHT_SCORE: f32 = 0.5;

// ─── Types ──────────────────────────────────────────

/// A single eureka/insight event.
#[derive(Clone, Debug)]
pub struct EurekaEvent {
    pub id: u64,
    pub timestamp_ms: u64,
    pub query: String,
    pub text: String,
    pub surprise_score: f32,
    pub curiosity_score: f32,
    pub spatial_dist: f32,
    pub emotional_sim: f32,
    pub layer_id: u8,
    pub block_index: u32,
}

impl EurekaEvent {
    /// Compute the overall insight score.
    pub fn insight_score(&self) -> f32 {
        let base = self.surprise_score * self.curiosity_score * self.emotional_sim;
        if self.spatial_dist > 0.01 {
            base / self.spatial_dist
        } else {
            base * 10.0 // very close spatial → still interesting if emotion-high
        }
    }
}

/// Persistent eureka log.
pub struct EurekaLog {
    pub events: Vec<EurekaEvent>,
    pub next_id: u64,
}

impl EurekaLog {
    /// Load from EUR1 file or return empty.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("eureka.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 8 && &data[0..4] == b"EUR1" {
                let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
                let mut events = Vec::with_capacity(count);
                let mut off = 8;
                let mut max_id = 0u64;
                for _ in 0..count {
                    if off + 16 > data.len() { break; }
                    let id = u64::from_le_bytes(data[off..off+8].try_into().unwrap());
                    let ts = u64::from_le_bytes(data[off+8..off+16].try_into().unwrap());
                    off += 16;
                    if off + 2 > data.len() { break; }
                    let qlen = u16::from_le_bytes(data[off..off+2].try_into().unwrap()) as usize;
                    off += 2;
                    if off + qlen > data.len() { break; }
                    let query = String::from_utf8_lossy(&data[off..off+qlen]).to_string();
                    off += qlen;
                    if off + 2 > data.len() { break; }
                    let tlen = u16::from_le_bytes(data[off..off+2].try_into().unwrap()) as usize;
                    off += 2;
                    let text = if off + tlen <= data.len() {
                        String::from_utf8_lossy(&data[off..off+tlen]).to_string()
                    } else { String::new() };
                    off += tlen;
                    if off + 20 > data.len() { break; }
                    let surprise = f32::from_le_bytes(data[off..off+4].try_into().unwrap());
                    let curiosity = f32::from_le_bytes(data[off+4..off+8].try_into().unwrap());
                    let sd = f32::from_le_bytes(data[off+8..off+12].try_into().unwrap());
                    let es = f32::from_le_bytes(data[off+12..off+16].try_into().unwrap());
                    let lid = data[off+16];
                    let bi = u32::from_le_bytes(data[off+17..off+21].try_into().unwrap());
                    off += 21;
                    if id > max_id { max_id = id; }
                    events.push(EurekaEvent {
                        id, timestamp_ms: ts, query, text,
                        surprise_score: surprise, curiosity_score: curiosity,
                        spatial_dist: sd, emotional_sim: es,
                        layer_id: lid, block_index: bi,
                    });
                }
                return EurekaLog { events, next_id: max_id + 1 };
            }
        }
        EurekaLog { events: Vec::new(), next_id: 1 }
    }

    /// Save to EUR1 file.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("eureka.bin");
        let mut buf = Vec::with_capacity(8 + self.events.len() * 128);
        buf.extend_from_slice(b"EUR1");
        buf.extend_from_slice(&(self.events.len() as u32).to_le_bytes());
        for ev in &self.events {
            buf.extend_from_slice(&ev.id.to_le_bytes());
            buf.extend_from_slice(&ev.timestamp_ms.to_le_bytes());
            let qbytes = ev.query.as_bytes();
            buf.extend_from_slice(&(qbytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(qbytes);
            let tbytes = ev.text.as_bytes();
            buf.extend_from_slice(&(tbytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(tbytes);
            buf.extend_from_slice(&ev.surprise_score.to_le_bytes());
            buf.extend_from_slice(&ev.curiosity_score.to_le_bytes());
            buf.extend_from_slice(&ev.spatial_dist.to_le_bytes());
            buf.extend_from_slice(&ev.emotional_sim.to_le_bytes());
            buf.push(ev.layer_id);
            buf.extend_from_slice(&ev.block_index.to_le_bytes());
        }
        let tmp_path = output_dir.join("eureka.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write eureka.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename eureka.bin: {}", e))
    }

    /// Add a new event and save.
    pub fn record(&mut self, output_dir: &Path, event: EurekaEvent) -> Result<(), String> {
        self.events.push(event);
        self.next_id += 1;
        self.save(output_dir)
    }
}

// ─── Detection ──────────────────────────────────────

/// Detect eureka moments from recall results.
///
/// For each result that is spatially distant from the query but emotionally
/// similar, compute an insight score. Events above threshold get recorded
/// and stored as an insight memory.
pub fn detect_eureka(
    config: &crate::config::Config,
    reader: &MicroscopeReader,
    query: &str,
    emotion: Option<&[f32; 21]>,
    results: &[(f32, usize, bool)],
) -> Vec<EurekaEvent> {
    let output_dir = Path::new(&config.paths.output_dir);
    let emotion_lookup = load_emotion_lookup(output_dir);
    let (qx, qy, qz) = crate::content_coords_blended(query, "long_term", config.search.semantic_weight);
    let mut eureka_events = Vec::new();

    // Extract surprise and curiosity from the query emotion
    let surprise_idx = EMOTION_DIMS.iter().position(|&d| d == "surprise").unwrap_or(4);
    let curiosity_idx = EMOTION_DIMS.iter().position(|&d| d == "curiosity").unwrap_or(10);
    let query_surprise = emotion.map(|e| e[surprise_idx]).unwrap_or(0.0);
    let query_curiosity = emotion.map(|e| e[curiosity_idx]).unwrap_or(0.0);

    for &(_score, idx, is_main) in results {
        if !is_main { continue; }
        if idx >= reader.block_count { continue; }

        let hdr = reader.header(idx);
        let dx = hdr.x - qx;
        let dy = hdr.y - qy;
        let dz = hdr.z - qz;
        let spatial_dist = dx * dx + dy * dy + dz * dz;

        if spatial_dist < EUREKA_MIN_SPATIAL_DIST { continue; }

        // Emotional similarity between query emotion and block emotion
        let emotional_sim = match (emotion, emotion_lookup.as_ref()) {
            (Some(qe), Some(lookup)) => {
                lookup(idx).map(|be| emotional_similarity(qe, &be)).unwrap_or(0.0)
            }
            _ => 0.0,
        };

        if emotional_sim < EUREKA_MIN_EMO_SIM { continue; }

        // Use query surprise/curiosity, or default to emotional_sim * 0.5
        let surprise = query_surprise.max(emotional_sim * 0.3);
        let curiosity = query_curiosity.max(emotional_sim * 0.4);

        let event = EurekaEvent {
            id: 0, // assigned by EurekaLog::record
            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            query: safe_truncate(query, 80),
            text: safe_truncate(reader.text(idx), 120),
            surprise_score: surprise,
            curiosity_score: curiosity,
            spatial_dist: spatial_dist.sqrt(),
            emotional_sim,
            layer_id: hdr.layer_id,
            block_index: idx as u32,
        };

        if event.insight_score() >= EUREKA_MIN_INSIGHT_SCORE {
            eureka_events.push(event);
        }
    }

    eureka_events
}

// ─── CLI display ────────────────────────────────────

/// Format a single eureka event for CLI output.
pub fn format_eureka(event: &EurekaEvent) -> String {
    format!(
        "  [#{}] 🔍 {} | surprise={:.2} curiosity={:.2} dist={:.3} emo_sim={:.2} | score={:.1}\n         Q: {}\n         R: {}",
        event.id,
        event.timestamp_ms,
        event.surprise_score,
        event.curiosity_score,
        event.spatial_dist,
        event.emotional_sim,
        event.insight_score(),
        safe_truncate(&event.query, 60),
        safe_truncate(&event.text, 60),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_log() {
        let log = EurekaLog::load_or_init(Path::new("."));
        assert!(log.events.is_empty());
        assert_eq!(log.next_id, 1);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let mut log = EurekaLog::load_or_init(&dir);
        let ev = EurekaEvent {
            id: 1,
            timestamp_ms: 1000,
            query: "test query".into(),
            text: "test result".into(),
            surprise_score: 0.8,
            curiosity_score: 0.7,
            spatial_dist: 0.3,
            emotional_sim: 0.6,
            layer_id: 3,
            block_index: 42,
        };
        log.record(&dir, ev).unwrap();

        let loaded = EurekaLog::load_or_init(&dir);
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].query, "test query");
        assert!((loaded.events[0].surprise_score - 0.8).abs() < 0.01);

        let _ = std::fs::remove_file(dir.join("eureka.bin"));
    }

    #[test]
    fn test_insight_score_formula() {
        let ev = EurekaEvent {
            id: 1, timestamp_ms: 0,
            query: "q".into(), text: "r".into(),
            surprise_score: 0.8, curiosity_score: 0.7,
            spatial_dist: 0.2, emotional_sim: 0.6,
            layer_id: 0, block_index: 0,
        };
        let score = ev.insight_score();
        // 0.8 * 0.7 * 0.6 / 0.2 = 1.68
        assert!((score - 1.68).abs() < 0.01);
    }
}
