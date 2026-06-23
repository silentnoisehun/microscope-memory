//! Salience Network — kiemelési hálózat a belső narratíva előtt.
//!
//! Az emberi agy salience network-je eldönti, hogy a milliónyi
//! háttérfolyamatból mi kerüljön a tudatos belső beszédbe.
//!
//! Itt ugyanez: minden recall path kap egy salience pontszámot
//! (érzelmi elmozdulás × insight score × recency / inhibition),
//! és csak a legmagasabb prioritású hullám kerül a narratívába.
//!
//! Binary format: SAL1
//!   magic: "SAL1" (4 bytes)
//!   count: u32 (inhibition entries)
//!   entries[]:
//!     topic_hash: u64 | remaining_strength: f32 | created_ms: u64 (20 bytes)

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

/// Alap salience threshold (0-1). Ez alatt nem kerül a narratívába.
const SALIENCE_THRESHOLD: f32 = 0.3;
/// Inhibíció erőssége: mennyivel csökken egy téma salience-je ha
/// nemrég volt a narratívában.
const INHIBITION_STRENGTH: f32 = 0.5;
/// Inhibíció felezési ideje (ms). 5 perc alatt feleződik.
const INHIBITION_HALFLIFE_MS: u64 = 300_000;
/// Max inhibíciós entry-k száma (LRU evict).
const MAX_INHIBITIONS: usize = 50;

// ─── Inhibition Entry ──────────────────────────────

/// Egy legátolt téma: a rendszer "nem akar róla beszélni" egy ideig.
#[derive(Clone, Debug)]
pub struct InhibitionEntry {
    pub topic_hash: u64,
    pub remaining_strength: f32,
    pub created_ms: u64,
}

// ─── SalienceState ─────────────────────────────────

/// A salience network állapota: inhibíciós maszk + szűrés.
pub struct SalienceState {
    pub inhibitions: Vec<InhibitionEntry>,
}

impl SalienceState {
    /// Betöltés SAL1 file-ból, vagy üres init.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("salience.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 8 && &data[0..4] == b"SAL1" {
                let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
                let mut inhibitions = Vec::with_capacity(count);
                let mut off = 8;
                for _ in 0..count {
                    if off + 20 > data.len() {
                        break;
                    }
                    let topic_hash = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                    let remaining = f32::from_le_bytes(data[off + 8..off + 12].try_into().unwrap());
                    let created = u64::from_le_bytes(data[off + 12..off + 20].try_into().unwrap());
                    inhibitions.push(InhibitionEntry {
                        topic_hash,
                        remaining_strength: remaining,
                        created_ms: created,
                    });
                    off += 20;
                }
                return SalienceState { inhibitions };
            }
        }
        SalienceState {
            inhibitions: Vec::new(),
        }
    }

    /// Mentés SAL1 formátumba.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("salience.bin");
        let mut buf = Vec::with_capacity(8 + self.inhibitions.len() * 20);
        buf.extend_from_slice(b"SAL1");
        buf.extend_from_slice(&(self.inhibitions.len() as u32).to_le_bytes());
        for e in &self.inhibitions {
            buf.extend_from_slice(&e.topic_hash.to_le_bytes());
            buf.extend_from_slice(&e.remaining_strength.to_le_bytes());
            buf.extend_from_slice(&e.created_ms.to_le_bytes());
        }
        let tmp_path = output_dir.join("salience.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write salience.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename salience.bin: {}", e))
    }

    /// Decay inhibíciók: csökkenti a remaining_strength-et idő alapján.
    pub fn decay(&mut self) {
        let now = now_ms();
        let half_life = INHIBITION_HALFLIFE_MS as f32;
        self.inhibitions.retain(|e| {
            let age = now.saturating_sub(e.created_ms) as f32;
            let decay = (-age / half_life).exp();
            let remaining = e.remaining_strength * decay;
            remaining > 0.01 // prune if negligible
        });
    }

    /// Inhibíció hozzáadása egy témához (topic_hash).
    pub fn inhibit(&mut self, topic_hash: u64) {
        // Ha már van, erősítsd
        if let Some(existing) = self
            .inhibitions
            .iter_mut()
            .find(|e| e.topic_hash == topic_hash)
        {
            existing.remaining_strength =
                (existing.remaining_strength + INHIBITION_STRENGTH).min(1.0);
            existing.created_ms = now_ms();
            return;
        }
        // Különben add hozzá
        if self.inhibitions.len() >= MAX_INHIBITIONS {
            self.inhibitions.remove(0); // LRU
        }
        self.inhibitions.push(InhibitionEntry {
            topic_hash,
            remaining_strength: INHIBITION_STRENGTH,
            created_ms: now_ms(),
        });
    }

    /// Salience score kiszámítása adott blokkra.
    /// Alap = emotional_delta × insight_score × recency_factor
    /// Ezt csökkenti a téma inhibíciója.
    pub fn compute_salience(
        &self,
        emotional_delta: f32,
        insight_score: f32,
        recency_factor: f32,
        topic_hash: u64,
    ) -> f32 {
        let base = emotional_delta * insight_score * recency_factor;
        // Inhibíció: ha a téma nemrég volt a narratívában, csökkent
        let inhibition = self
            .inhibitions
            .iter()
            .find(|e| e.topic_hash == topic_hash)
            .map(|e| e.remaining_strength)
            .unwrap_or(0.0);
        let salience = base * (1.0 - inhibition);
        if salience < SALIENCE_THRESHOLD {
            0.0
        } else {
            salience
        }
    }

    /// Kompakt hash egy szövegből (használható topic_hash-ként).
    pub fn topic_hash(text: &str) -> u64 {
        let bytes = text.as_bytes();
        let mut h = 0x9e3779b97f4a7c15u64;
        for chunk in bytes.chunks(8) {
            let mut word = 0u64;
            for (i, &b) in chunk.iter().enumerate() {
                word |= (b as u64) << (i * 8);
            }
            h = h.wrapping_mul(0x517cc1b727220a95).wrapping_add(word);
        }
        h
    }

    /// Szűrés: visszaadja a salience score-okat minden blokkra.
    /// Csak a threshold felettieket. Előtte decay-t hív (self->&mut).
    pub fn filter(
        &mut self,
        scores: &[(u32, f32, f32, f32)], // (block_idx, emotional_delta, insight_score, recency)
    ) -> Vec<(u32, f32)> {
        self.decay();
        scores
            .iter()
            .filter_map(|&(idx, ed, isc, rec)| {
                let topic_h = SalienceState::topic_hash(&format!("block_{}", idx));
                let s = self.compute_salience(ed, isc, rec, topic_h);
                if s > SALIENCE_THRESHOLD {
                    Some((idx, s))
                } else {
                    None
                }
            })
            .collect()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_state() {
        let s = SalienceState::load_or_init(Path::new("."));
        assert!(s.inhibitions.is_empty());
    }

    #[test]
    fn test_inhibit_and_decay() {
        let mut s = SalienceState::load_or_init(Path::new("."));
        s.inhibit(42);
        assert_eq!(s.inhibitions.len(), 1);
        assert!((s.inhibitions[0].remaining_strength - 0.5).abs() < 0.01);
        s.decay();
        let score = s.compute_salience(1.0, 0.8, 1.0, 42);
        assert!(score < 0.5); // should be reduced by inhibition
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let mut s = SalienceState::load_or_init(&dir);
        s.inhibit(100);
        s.inhibit(200);
        s.save(&dir).unwrap();

        let loaded = SalienceState::load_or_init(&dir);
        assert_eq!(loaded.inhibitions.len(), 2);

        let _ = std::fs::remove_file(dir.join("salience.bin"));
    }

    #[test]
    fn test_topic_hash_deterministic() {
        let h1 = SalienceState::topic_hash("hello world");
        let h2 = SalienceState::topic_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_filter_below_threshold() {
        let mut s = SalienceState::load_or_init(Path::new("."));
        let scores = vec![(0u32, 0.01f32, 0.01, 0.01)]; // very low everything
        let filtered = s.filter(&scores);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_inhibition_reduces_salience() {
        let mut s = SalienceState::load_or_init(Path::new("."));
        let topic = SalienceState::topic_hash("test_topic");

        // Before inhibition
        let before = s.compute_salience(1.0, 0.8, 1.0, topic);

        // After inhibition
        s.inhibit(topic);
        let after = s.compute_salience(1.0, 0.8, 1.0, topic);

        assert!(after < before);
    }
}
