//! Spaced Repetition — Ebbinghaus felejtési görbe management.
//!
//! SM-2 alapú: minden recall után nő az intervallum, a gyakran felidézett
//! blokkok egyre könnyebben előjönnek. Túléli a rebuild-et (block_idx alapú).
//!
//! Binary format: SPR1
//!   [S,P,R,1] magic (4 bytes)
//!   count: u32 (4 bytes)
//!   records[]:
//!     block_idx: u32 | recall_count: u16 | last_recall_ms: u64
//!     interval_days: f32 | ease_factor: f32 | importance: u8
//!     = 23 bytes/record

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

/// SM-2 kezdő intervallum (nap).
const INITIAL_INTERVAL_DAYS: f32 = 1.0;
/// Minimum ease factor (nehéz → gyorsabban felejt).
const MIN_EASE: f32 = 1.3;
/// Maximum ease factor (könnyű → lassan felejt).
const MAX_EASE: f32 = 5.0;
/// Default ease factor (SM-2 standard).
const DEFAULT_EASE: f32 = 2.5;
/// Ha ennyiszer felidézték, "megtanultnak" tekintjük.
const MASTERED_THRESHOLD: u16 = 15;
/// Meddig számít "még frissnek" (ms). 7 nap.
const FRESH_WINDOW_MS: u64 = 7 * 86_400_000;
/// Boost mértéke: mennyit vonjunk le a distance-ből due blokknál.
const SPACING_BOOST_FACTOR: f32 = 0.05;

// ─── Types ──────────────────────────────────────────

/// Egy blokk spaced repetition állapota.
#[derive(Clone, Debug)]
pub struct SpacedBlock {
    pub block_idx: u32,
    pub recall_count: u16,
    pub last_recall_ms: u64,
    pub interval_days: f32,
    pub ease_factor: f32,
    pub importance: u8,
}

impl SpacedBlock {
    /// SM-2 szerint esedékes?
    pub fn is_due(&self, now_ms: u64) -> bool {
        let elapsed_days = (now_ms.saturating_sub(self.last_recall_ms)) as f32 / 86_400_000.0;
        elapsed_days >= self.interval_days
    }

    /// Mennyivel késett (nap). Negatív = még nem esedékes.
    pub fn overdue_days(&self, now_ms: u64) -> f32 {
        let elapsed_days = (now_ms.saturating_sub(self.last_recall_ms)) as f32 / 86_400_000.0;
        elapsed_days - self.interval_days
    }

    /// SM-2 minősítés alapján új ease_factor + interval.
    /// quality: 0 (legrosszabb) – 5 (legjobb)
    pub fn record_recall(&mut self, quality: u8, now_ms: u64) {
        let q = quality.min(5) as f32;
        self.recall_count += 1;
        self.last_recall_ms = now_ms;

        if q < 3.0 {
            // recall failed → reset
            self.interval_days = 1.0;
            self.ease_factor = (self.ease_factor - 0.2).max(MIN_EASE);
        } else {
            // SM-2: update ease factor
            self.ease_factor = (self.ease_factor + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02)))
                .max(MIN_EASE)
                .min(MAX_EASE);

            if self.recall_count <= 1 {
                self.interval_days = 1.0;
            } else if self.recall_count == 2 {
                self.interval_days = 6.0;
            } else {
                self.interval_days = (self.interval_days * self.ease_factor).min(365.0 * 5.0); // max 5 year
            }
        }
    }

    /// Spacing boost érték a recall scoring-hoz.
    /// 0.0 ha nem esedékes, pozitív ha due vagy túlkésett.
    pub fn spacing_boost(&self, now_ms: u64) -> f32 {
        if !self.is_due(now_ms) {
            return 0.0;
        }
        let overdue = self.overdue_days(now_ms).max(0.0);
        // Alap boost + extra minél régebb óta esedékes
        let base = SPACING_BOOST_FACTOR;
        let extra = (overdue * 0.01).min(SPACING_BOOST_FACTOR);
        base + extra
    }
}

/// Spaced repetition állapot — az összes tracked blokk.
pub struct SpacedRepetition {
    pub blocks: Vec<SpacedBlock>,
}

impl SpacedRepetition {
    /// Betöltés SPR1 file-ból, vagy üres init.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("spaced.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 8 && &data[0..4] == b"SPR1" {
                let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
                let rec_size = 23;
                let mut blocks = Vec::with_capacity(count);
                let mut off = 8;
                for _ in 0..count {
                    if off + rec_size > data.len() { break; }
                    let block_idx = u32::from_le_bytes(data[off..off+4].try_into().unwrap());
                    let recall_count = u16::from_le_bytes(data[off+4..off+6].try_into().unwrap());
                    let last_recall_ms = u64::from_le_bytes(data[off+6..off+14].try_into().unwrap());
                    let interval_days = f32::from_le_bytes(data[off+14..off+18].try_into().unwrap());
                    let ease_factor = f32::from_le_bytes(data[off+18..off+22].try_into().unwrap());
                    let importance = data[off+22];
                    off += rec_size;
                    blocks.push(SpacedBlock {
                        block_idx, recall_count, last_recall_ms,
                        interval_days, ease_factor, importance,
                    });
                }
                return SpacedRepetition { blocks };
            }
        }
        SpacedRepetition { blocks: Vec::new() }
    }

    /// Mentés SPR1 formátumba.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("spaced.bin");
        let rec_size = 23;
        let mut buf = Vec::with_capacity(8 + self.blocks.len() * rec_size);
        buf.extend_from_slice(b"SPR1");
        buf.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes());
        for b in &self.blocks {
            buf.extend_from_slice(&b.block_idx.to_le_bytes());
            buf.extend_from_slice(&b.recall_count.to_le_bytes());
            buf.extend_from_slice(&b.last_recall_ms.to_le_bytes());
            buf.extend_from_slice(&b.interval_days.to_le_bytes());
            buf.extend_from_slice(&b.ease_factor.to_le_bytes());
            buf.push(b.importance);
        }
        fs::write(&path, &buf).map_err(|e| format!("write spaced.bin: {}", e))
    }

    /// Keress egy blokkot block_idx alapján.
    pub fn find(&self, block_idx: u32) -> Option<&SpacedBlock> {
        self.blocks.iter().find(|b| b.block_idx == block_idx)
    }

    pub fn find_mut(&mut self, block_idx: u32) -> Option<&mut SpacedBlock> {
        self.blocks.iter_mut().find(|b| b.block_idx == block_idx)
    }

    /// Hozz létre vagy frissíts egy blokkot a recall után.
    pub fn record_recall(&mut self, block_idx: u32, importance: u8, quality: u8) {
        let now = now_ms();
        if let Some(block) = self.find_mut(block_idx) {
            block.record_recall(quality, now);
            block.importance = importance;
        } else {
            // Új blokk
            let mut block = SpacedBlock {
                block_idx,
                recall_count: 0,
                last_recall_ms: now,
                interval_days: INITIAL_INTERVAL_DAYS,
                ease_factor: DEFAULT_EASE,
                importance,
            };
            block.record_recall(quality, now);
            self.blocks.push(block);
        }
    }

    /// Spacing boost adott blokkhoz a recall scoring-hoz.
    pub fn spacing_boost(&self, block_idx: u32) -> f32 {
        let now = now_ms();
        self.find(block_idx).map(|b| b.spacing_boost(now)).unwrap_or(0.0)
    }

    /// Hány due blokk van?
    pub fn due_count(&self) -> usize {
        let now = now_ms();
        self.blocks.iter().filter(|b| b.is_due(now)).count()
    }

    /// Hány "mastered" blokk (recall_count >= 15)?
    pub fn mastered_count(&self) -> usize {
        self.blocks.iter().filter(|b| b.recall_count >= MASTERED_THRESHOLD).count()
    }

    /// Statisztikák.
    pub fn stats(&self) -> SpacedStats {
        let now = now_ms();
        SpacedStats {
            total_blocks: self.blocks.len(),
            due: self.blocks.iter().filter(|b| b.is_due(now)).count(),
            mastered: self.blocks.iter().filter(|b| b.recall_count >= MASTERED_THRESHOLD).count(),
            fresh: self.blocks.iter().filter(|b| now.saturating_sub(b.last_recall_ms) < FRESH_WINDOW_MS).count(),
            avg_ease: self.blocks.iter().map(|b| b.ease_factor).sum::<f32>() / self.blocks.len().max(1) as f32,
            avg_interval: self.blocks.iter().map(|b| b.interval_days).sum::<f32>() / self.blocks.len().max(1) as f32,
        }
    }

    /// Listázd a due blokkokat block_idx szerint.
    pub fn due_blocks(&self) -> Vec<u32> {
        let now = now_ms();
        let mut due: Vec<u32> = self.blocks.iter()
            .filter(|b| b.is_due(now))
            .map(|b| b.block_idx)
            .collect();
        due.sort();
        due
    }
}

// ─── Statisztikák ───────────────────────────────────

pub struct SpacedStats {
    pub total_blocks: usize,
    pub due: usize,
    pub mastered: usize,
    pub fresh: usize,
    pub avg_ease: f32,
    pub avg_interval: f32,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Tesztek ────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let sr = SpacedRepetition::load_or_init(Path::new("."));
        assert_eq!(sr.stats().total_blocks, 0);
    }

    #[test]
    fn test_sm2_interval_growth() {
        let _sr = SpacedRepetition { blocks: vec![] };
        let mut block = SpacedBlock {
            block_idx: 0, recall_count: 0, last_recall_ms: 1000,
            interval_days: 1.0, ease_factor: 2.5, importance: 5,
        };
        // 1st recall (quality=4)
        block.record_recall(4, 2000);
        assert!(block.interval_days >= 1.0);
        let after_first = block.interval_days;

        // 2nd recall (quality=5)
        block.record_recall(5, 3000);
        assert!(block.interval_days > after_first);

        // 3rd recall — SM-2 multiplies by ease_factor
        block.record_recall(5, 4000);
        assert!(block.interval_days > after_first);
    }

    #[test]
    fn test_poor_quality_resets() {
        let mut block = SpacedBlock {
            block_idx: 0, recall_count: 2, last_recall_ms: 1000,
            interval_days: 10.0, ease_factor: 2.5, importance: 5,
        };
        block.record_recall(1, 2000); // poor quality → reset
        assert!((block.interval_days - 1.0).abs() < 0.01);
        assert!(block.ease_factor < 2.5);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let mut sr = SpacedRepetition::load_or_init(&dir);
        sr.record_recall(0, 7, 4);
        sr.record_recall(1, 5, 5);
        sr.save(&dir).unwrap();

        let loaded = SpacedRepetition::load_or_init(&dir);
        assert_eq!(loaded.blocks.len(), 2);
        assert_eq!(loaded.blocks[0].block_idx, 0);
        assert_eq!(loaded.blocks[1].block_idx, 1);

        let _ = std::fs::remove_file(dir.join("spaced.bin"));
    }

    #[test]
    fn test_due_detection() {
        let sr = SpacedRepetition {
            blocks: vec![SpacedBlock {
                block_idx: 0, recall_count: 1,
                last_recall_ms: 1000,  // régi
                interval_days: 0.001,  // nagyon rövid intervallum → due
                ease_factor: 2.5, importance: 5,
            }],
        };
        assert!(sr.due_count() > 0);
        assert!(sr.spacing_boost(0) > 0.0);
    }

    #[test]
    fn test_stats() {
        let mut sr = SpacedRepetition { blocks: vec![] };
        sr.record_recall(0, 5, 5);
        sr.record_recall(1, 3, 3);
        let stats = sr.stats();
        assert_eq!(stats.total_blocks, 2);
        assert!(stats.avg_ease > 0.0);
    }
}
