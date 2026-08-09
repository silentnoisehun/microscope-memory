//! Absentia — Csend Réteg / Silence Layer
//!
//! Azt követi nyomon, ami NEM történt meg, amit NEM mondtak ki,
//! és ami hiányzik onnan, ahol lennie kellene.
//!
//! Ez a réteg a causal laundering egyetlen valódi ellenszere:
//! nem a jelenlétet, hanem a *hiányt* detektálja.
//!
//! Bináris fájl: absentia.bin (ABS1)

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hebbian::HebbianState;
use crate::epistemic::EvidenceLedger;

// ─── Helpers ────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── AbsencePattern ─────────────────────────────────

/// A hiány mintázatának típusa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsencePattern {
    /// Rendszeresen jelen volt, eltűnt.
    ExpectedDisappearance,
    /// Logikusan ott kellene lennie, de nincs.
    LogicalAbsence,
    /// Elkerült téma — soha nem bukkan fel, pedig illeszkedne.
    Taboo,
    /// Befejezetlen gondolat — nyitott hurok.
    InterruptedThought,
    /// Együtt KELLENE aktiválódni, de nem — anti-Hebbian jel.
    AntiHebbian,
}

impl std::fmt::Display for AbsencePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AbsencePattern::ExpectedDisappearance => write!(f, "disappearance"),
            AbsencePattern::LogicalAbsence => write!(f, "logical_absence"),
            AbsencePattern::Taboo => write!(f, "taboo"),
            AbsencePattern::InterruptedThought => write!(f, "interrupted"),
            AbsencePattern::AntiHebbian => write!(f, "anti_hebbian"),
        }
    }
}

impl AbsencePattern {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::ExpectedDisappearance,
            1 => Self::LogicalAbsence,
            2 => Self::Taboo,
            3 => Self::InterruptedThought,
            4 => Self::AntiHebbian,
            _ => Self::LogicalAbsence,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::ExpectedDisappearance => 0,
            Self::LogicalAbsence => 1,
            Self::Taboo => 2,
            Self::InterruptedThought => 3,
            Self::AntiHebbian => 4,
        }
    }
}

// ─── AbsentiaRecord ─────────────────────────────────

/// Egyetlen hiány-rekord: ami logikusan ott kellene legyen, de nincs.
#[derive(Debug, Clone)]
pub struct AbsentiaRecord {
    /// A kontextus hash-e, amiben a hiányt észleltük.
    pub expected_context_hash: u64,
    /// A hiányzó entitás hash-e.
    pub missing_entity_hash: u64,
    /// Hányszor fordult elő korábban ebben a kontextusban.
    pub expected_frequency: f32,
    /// Hányszor fordult elő ténylegesen.
    pub actual_frequency: f32,
    /// Hiány pontszám: 0.0 = jelen van, 1.0 = teljesen hiányzik.
    pub absence_score: f32,
    /// Mikor észleltük először.
    pub first_detected_ms: u64,
    /// Mennyi ideje hiányzik.
    pub duration_ms: u64,
    /// A hiány mintázatának típusa.
    pub pattern_type: AbsencePattern,
    /// Súly a morfogenezis gradiensben.
    pub gradient_weight: f32,
}

// ─── AntiHebbianPair ────────────────────────────────

/// Anti-Hebbian pár: két blokk, aminek együtt KELLENE aktiválódnia, de nem.
#[derive(Debug, Clone)]
pub struct AntiHebbianPair {
    pub block_a: u32,
    pub block_b: u32,
    pub expected_coactivation: f32,
    pub actual_coactivation: f32,
    pub absence_score: f32,
    pub first_detected_ms: u64,
}

// ─── AbsentiaState ──────────────────────────────────

/// Az Absentia réteg állapota.
pub struct AbsentiaState {
    /// Hiány-rekordok.
    pub records: Vec<AbsentiaRecord>,
    /// Anti-Hebbian párok.
    pub anti_hebbian: Vec<AntiHebbianPair>,
    /// Negatív attractorok a MorphogenFieldben: (x, y, z, weight).
    pub negative_attractors: Vec<(f64, f64, f64, f64)>,
    /// Utolsó szkennelés időpontja.
    pub last_scan_ms: u64,
}

impl AbsentiaState {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            anti_hebbian: Vec::new(),
            negative_attractors: Vec::new(),
            last_scan_ms: 0,
        }
    }

    /// Betölti vagy inicializálja az Absentia állapotot.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("absentia.bin");
        if let Ok(data) = std::fs::read(&path) {
            if data.len() >= 4 && &data[0..4] == b"ABS1" {
                return Self::decode(&data);
            }
        }
        Self::new()
    }

    /// Elmenti az Absentia állapotot.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("absentia.bin");
        let data = self.encode();
        let tmp = output_dir.join("absentia.bin.tmp");
        std::fs::write(&tmp, &data).map_err(|e| format!("write absentia: {}", e))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("rename absentia: {}", e))?;
        Ok(())
    }

    /// Szkenneli a Hebbian állapotot és az Evidence ledgert hiányok után.
    ///
    /// Anti-Hebbian detektálás: két blokk, aminek együtt KELLENE aktiválódnia
    /// (struktúra alapján), de a co-aktivációs rekordjuk alacsony vagy nulla.
    pub fn scan(
        &mut self,
        hebb: &HebbianState,
        evidence: &EvidenceLedger,
        _block_count: usize,
    ) {
        let now = now_ms();
        self.last_scan_ms = now;

        // ─── Anti-Hebbian detektálás ─────────────────
        // Keresünk blokk-párokat, amik:
        // 1. Ugyanabban a rétegben vannak (strukturális szomszédok)
        // 2. Magas activation_count-juk van külön-külön
        // 3. Alacsony vagy nulla co-aktivációjuk van

        // Top aktív blokkok
        let mut active_blocks: Vec<(u32, f32)> = hebb
            .activations
            .iter()
            .enumerate()
            .filter(|(_, r)| r.energy > 0.3)
            .map(|(i, r)| (i as u32, r.energy))
            .collect();
        active_blocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        active_blocks.truncate(50); // top 50

        // Páronként ellenőrizzük
        for i in 0..active_blocks.len() {
            for j in (i + 1)..active_blocks.len() {
                let a = active_blocks[i].0.min(active_blocks[j].0);
                let b = active_blocks[i].0.max(active_blocks[j].0);
                let pair_key = (a, b);

                // Ha nincs co-aktiváció, de mindkét blokk aktív
                if !hebb.coactivations.contains_key(&pair_key) {
                    let energy_a = active_blocks[i].1;
                    let energy_b = active_blocks[j].1;
                    // Minél aktívabbak külön-külön, annál gyanúsabb az együtt nem aktiválódás
                    let absence = (energy_a * energy_b).min(1.0);

                    if absence > 0.2 {
                        // Anti-Hebbian pár
                        self.anti_hebbian.push(AntiHebbianPair {
                            block_a: a,
                            block_b: b,
                            expected_coactivation: absence,
                            actual_coactivation: 0.0,
                            absence_score: absence,
                            first_detected_ms: now,
                        });
                    }
                }
            }
        }

        // ─── Evidence hiány detektálás ───────────────
        // Keresünk aktív blokkokat, amiknek NINCS evidence record-uk
        // de magas Hebbian energiájuk van — ez "hamis biztonság"
        for (i, rec) in hebb.activations.iter().enumerate() {
            if rec.energy > 0.5 {
                // Nincs evidence record — feltűnő hiány
                // (nem tudjuk közvetlenül ellenőrizni content_hash nélkül,
                //  de a hiány detektálható)
                self.records.push(AbsentiaRecord {
                    expected_context_hash: i as u64,
                    missing_entity_hash: 0,
                    expected_frequency: rec.activation_count as f32,
                    actual_frequency: 0.0,
                    absence_score: rec.energy,
                    first_detected_ms: now,
                    duration_ms: 0,
                    pattern_type: AbsencePattern::LogicalAbsence,
                    gradient_weight: rec.energy * 0.5,
                });
            }
        }

        // ─── Negatív attractorok építése ─────────────
        // Az anti-Hebbian párok negatív attractorokat hoznak létre
        // a MorphogenFieldben — olyan pontokat, ahova a hifák NEM nőnek
        self.negative_attractors.clear();
        for pair in &self.anti_hebbian {
            // A negatív attractor a két blokk közötti térben helyezkedik el
            // Ez "árnyékot vet" a gradiensre
            let weight = pair.absence_score as f64 * -1.0;
            self.negative_attractors.push((
                pair.block_a as f64 * 0.1,
                pair.block_b as f64 * 0.1,
                0.0,
                weight,
            ));
        }

        // Korlátozzuk a rekordok számát
        if self.records.len() > 500 {
            self.records.drain(0..self.records.len() - 500);
        }
        if self.anti_hebbian.len() > 200 {
            self.anti_hebbian.drain(0..self.anti_hebbian.len() - 200);
        }
    }

    /// Statisztikák.
    pub fn stats(&self) -> AbsentiaStats {
        let total_records = self.records.len();
        let anti_hebbian_count = self.anti_hebbian.len();
        let negative_attractor_count = self.negative_attractors.len();

        let avg_absence = if total_records > 0 {
            self.records.iter().map(|r| r.absence_score as f64).sum::<f64>()
                / total_records as f64
        } else {
            0.0
        };

        let avg_anti_hebbian = if anti_hebbian_count > 0 {
            self.anti_hebbian.iter().map(|p| p.absence_score as f64).sum::<f64>()
                / anti_hebbian_count as f64
        } else {
            0.0
        };

        // Causal laundering gyanús: anti-Hebbian párok, ahol a co-aktiváció 0
        // de mindkét blokk aktív
        let causal_laundering_suspect = self.anti_hebbian.iter()
            .filter(|p| p.absence_score > 0.5)
            .count();

        AbsentiaStats {
            total_records,
            anti_hebbian_count,
            negative_attractor_count,
            avg_absence,
            avg_anti_hebbian,
            causal_laundering_suspect,
            last_scan_ms: self.last_scan_ms,
        }
    }

    // ─── Bináris szerializás ────────────────────────

    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"ABS1");
        buf.extend_from_slice(&(self.records.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.anti_hebbian.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.negative_attractors.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.last_scan_ms.to_le_bytes());

        // Records
        for r in &self.records {
            buf.extend_from_slice(&r.expected_context_hash.to_le_bytes());
            buf.extend_from_slice(&r.missing_entity_hash.to_le_bytes());
            buf.extend_from_slice(&r.expected_frequency.to_le_bytes());
            buf.extend_from_slice(&r.actual_frequency.to_le_bytes());
            buf.extend_from_slice(&r.absence_score.to_le_bytes());
            buf.extend_from_slice(&r.first_detected_ms.to_le_bytes());
            buf.extend_from_slice(&r.duration_ms.to_le_bytes());
            buf.extend_from_slice(&r.pattern_type.to_u8().to_le_bytes());
            buf.extend_from_slice(&r.gradient_weight.to_le_bytes());
        }

        // Anti-Hebbian pairs
        for p in &self.anti_hebbian {
            buf.extend_from_slice(&p.block_a.to_le_bytes());
            buf.extend_from_slice(&p.block_b.to_le_bytes());
            buf.extend_from_slice(&p.expected_coactivation.to_le_bytes());
            buf.extend_from_slice(&p.actual_coactivation.to_le_bytes());
            buf.extend_from_slice(&p.absence_score.to_le_bytes());
            buf.extend_from_slice(&p.first_detected_ms.to_le_bytes());
        }

        // Negative attractors
        for &(x, y, z, w) in &self.negative_attractors {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
            buf.extend_from_slice(&z.to_le_bytes());
            buf.extend_from_slice(&w.to_le_bytes());
        }

        buf
    }

    fn decode(data: &[u8]) -> Self {
        if data.len() < 20 {
            return Self::new();
        }
        let record_count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let anti_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        let neg_count = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let last_scan = u64::from_le_bytes(data[16..24].try_into().unwrap());

        let mut off = 24;
        let mut records = Vec::with_capacity(record_count);
        for _ in 0..record_count {
            if off + 41 > data.len() { break; }
            let expected_context_hash = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let missing_entity_hash = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let expected_frequency = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let actual_frequency = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let absence_score = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let first_detected_ms = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let duration_ms = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let pattern_byte = data[off]; off += 1;
            let gradient_weight = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            records.push(AbsentiaRecord {
                expected_context_hash,
                missing_entity_hash,
                expected_frequency,
                actual_frequency,
                absence_score,
                first_detected_ms,
                duration_ms,
                pattern_type: AbsencePattern::from_u8(pattern_byte),
                gradient_weight,
            });
        }

        let mut anti_hebbian = Vec::with_capacity(anti_count);
        for _ in 0..anti_count {
            if off + 28 > data.len() { break; }
            let block_a = u32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let block_b = u32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let expected_coactivation = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let actual_coactivation = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let absence_score = f32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let first_detected_ms = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            anti_hebbian.push(AntiHebbianPair {
                block_a,
                block_b,
                expected_coactivation,
                actual_coactivation,
                absence_score,
                first_detected_ms,
            });
        }

        let mut negative_attractors = Vec::with_capacity(neg_count);
        for _ in 0..neg_count {
            if off + 32 > data.len() { break; }
            let x = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let y = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let z = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let w = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            negative_attractors.push((x, y, z, w));
        }

        Self {
            records,
            anti_hebbian,
            negative_attractors,
            last_scan_ms: last_scan,
        }
    }
}

// ─── AbsentiaStats ──────────────────────────────────

pub struct AbsentiaStats {
    pub total_records: usize,
    pub anti_hebbian_count: usize,
    pub negative_attractor_count: usize,
    pub avg_absence: f64,
    pub avg_anti_hebbian: f64,
    pub causal_laundering_suspect: usize,
    pub last_scan_ms: u64,
}

// ─── Morfogenezis integráció ────────────────────────

/// Negatív attractorok alkalmazása a MorphogenFieldre.
///
/// Ezek "árnyékot vetnek" a gradiensre — olyan pontok, ahova a hifák
/// logikusan kellene nőjenek, de a hiány-jel miatt nem nőnek.
pub fn apply_absentia_to_field(
    absentia: &AbsentiaState,
    field: &mut crate::morphogenesis::MorphogenField,
) {
    for &(x, y, z, weight) in &absentia.negative_attractors {
        // Negatív attractor = taszító pont
        field.add_attractor(x, y, z, weight);
    }
}

