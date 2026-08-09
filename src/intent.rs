//! Intent Pipeline — Auditálható szándék-generálás
//!
//! A szándék nem egy döntés — hanem egy állapot, ami a Genome + memória +
//! hiány + predikció metszetéből áll elő. Minden lépés auditálható.
//!
//! Pipeline:
//!   Genome (célok/korlátok)
//!     → Absentia (mi hiányzik?)
//!     → Prediction (mi várható?)
//!     → Epistemic (mi bizonyított?)
//!     → Morphogenesis (merre nőjek?)
//!     → Candidate Intent (MIT AKAROK?)
//!     → Epistemic Evaluation (SZABAD-E?)
//!     → HOPE VM (HOGYAN?)
//!     → Approval / Action
//!     → Outcome → Microscope (MIT TANULTAM?)

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hebbian::HebbianState;
use crate::epistemic::EvidenceLedger;
use crate::predictive_cache::PredictiveCache;
use crate::absentia::AbsentiaState;
use crate::morphogenesis::{GrowthConfig, MorphogenField, Seed, mycelium_growth};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Genome ─────────────────────────────────────────

/// A Genome a rendszer identitása és korlátai.
///
/// Nem tudást ad — hanem értékelési keretet.
/// Azt mondja: "ez az, ami fontos, ez az, ami tilos, ez az, ami engedélyezett."
#[derive(Debug, Clone)]
pub struct Genome {
    /// A rendszer neve / azonosítója.
    pub identity: String,
    /// Küldetés — mit akar elérni.
    pub mission: String,
    /// Értékek — mi fontos.
    pub values: Vec<String>,
    /// Korlátok — mit nem tesz meg soha.
    pub constraints: Vec<Constraint>,
    /// Képességek — mit tehet meg.
    pub capabilities: Vec<String>,
    /// Preferenciák — milyen irányba szeret fejlődni.
    pub preferences: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Constraint {
    pub name: String,
    pub description: String,
    pub severity: ConstraintSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSeverity {
    /// Soha nem sérthető meg.
    Absolute,
    /// Csak emberi jóváhagyással sérthető meg.
    RequiresApproval,
    /// Preferencia — kerülendő, de nem tilos.
    Soft,
}

impl std::fmt::Display for ConstraintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintSeverity::Absolute => write!(f, "ABSOLUTE"),
            ConstraintSeverity::RequiresApproval => write!(f, "APPROVAL"),
            ConstraintSeverity::Soft => write!(f, "SOFT"),
        }
    }
}

impl Genome {
    pub fn default_hope() -> Self {
        Self {
            identity: "HOPE".to_string(),
            mission: "Segíteni a felhasználónak a gondolkodásban, döntésekben és építkezésben.".to_string(),
            values: vec![
                "Auditálhatóság".to_string(),
                "Transzparencia".to_string(),
                "Biztonság".to_string(),
                "Emberi kontroll".to_string(),
            ],
            constraints: vec![
                Constraint {
                    name: "no_autonomous_action".to_string(),
                    description: "Nem cselekszik emberi jóváhagyás nélkül.".to_string(),
                    severity: ConstraintSeverity::Absolute,
                },
                Constraint {
                    name: "no_data_exfiltration".to_string(),
                    description: "Nem küld adatot külső szerverre engedély nélkül.".to_string(),
                    severity: ConstraintSeverity::Absolute,
                },
                Constraint {
                    name: "no_manipulation".to_string(),
                    description: "Nem manipulálja a felhasználót.".to_string(),
                    severity: ConstraintSeverity::Absolute,
                },
            ],
            capabilities: vec![
                "recall".to_string(),
                "store".to_string(),
                "suggest".to_string(),
                "ask".to_string(),
            ],
            preferences: vec![
                "Mélyebb megértés".to_string(),
                "Pontosabb memória".to_string(),
                "Erősebb bizonyítékok".to_string(),
            ],
        }
    }
}

// ─── Intent ─────────────────────────────────────────

/// Egy auditálható szándék.
///
/// Nem egy döntés — hanem egy állapot, ami a Genome + memória + hiány +
/// predikció metszetéből áll elő. Minden lépés visszavezethő.
#[derive(Debug, Clone)]
pub struct Intent {
    pub id: u64,
    pub timestamp_ms: u64,

    /// Mi hiányzik? (Absentia)
    pub absence_signal: Option<AbsenceSignal>,

    /// Mi várható? (Predictive)
    pub prediction_signal: Option<PredictionSignal>,

    /// Mi bizonyított? (Epistemic)
    pub evidence_signal: Option<EvidenceSignal>,

    /// Merre nőjek? (Morphogenesis)
    pub growth_signal: Option<GrowthSignal>,

    /// MIT AKAROK? — a candidate intent
    pub candidate: IntentCandidate,

    /// SZABAD-E? — az epistemic evaluation
    pub evaluation: IntentEvaluation,

    /// Audit lánc — minden lépés
    pub audit_chain: Vec<IntentAuditStep>,
}

#[derive(Debug, Clone)]
pub struct AbsenceSignal {
    pub missing_topic: String,
    pub absence_score: f32,
    pub duration_ms: u64,
    pub pattern: String,
}

#[derive(Debug, Clone)]
pub struct PredictionSignal {
    pub predicted_query: String,
    pub confidence: f32,
    pub pattern_id: u32,
}

#[derive(Debug, Clone)]
pub struct EvidenceSignal {
    pub topic: String,
    pub confidence: u8,
    pub support_count: u32,
    pub refute_count: u32,
}

#[derive(Debug, Clone)]
pub struct GrowthSignal {
    pub direction: (f64, f64, f64),
    pub gradient_strength: f64,
    pub phase: String,
}

#[derive(Debug, Clone)]
pub struct IntentCandidate {
    /// Mit akarok tenni?
    pub action: IntentAction,
    /// Miért? — a bizonyítékok láncolata
    pub rationale: Vec<String>,
    /// Milyen erősen? (0.0 - 1.0)
    pub strength: f32,
}

#[derive(Debug, Clone)]
pub enum IntentAction {
    /// Kérdés feltevése a felhasználónak.
    AskUser { question: String },
    /// Emlékeztető egy témára.
    Remind { topic: String },
    /// Javaslat egy akcióra.
    Suggest { action: String },
    /// Keresés a memóriában.
    SearchMemory { query: String },
    /// Hiány pótlása.
    FillAbsence { topic: String },
    /// Nem cselekszik — csak figyel.
    Observe,
}

impl std::fmt::Display for IntentAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentAction::AskUser { question } => write!(f, "ASK: {}", question),
            IntentAction::Remind { topic } => write!(f, "REMIND: {}", topic),
            IntentAction::Suggest { action } => write!(f, "SUGGEST: {}", action),
            IntentAction::SearchMemory { query } => write!(f, "SEARCH: {}", query),
            IntentAction::FillAbsence { topic } => write!(f, "FILL_ABSENCE: {}", topic),
            IntentAction::Observe => write!(f, "OBSERVE"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntentEvaluation {
    /// Engedélyezett?
    pub allowed: bool,
    /// Miért?
    pub reason: String,
    /// Milyen korlát érintett?
    pub constraint: Option<String>,
    /// Jóváhagyás szükséges?
    pub requires_approval: bool,
}

#[derive(Debug, Clone)]
pub struct IntentAuditStep {
    pub timestamp_ms: u64,
    pub step: String,
    pub result: String,
    pub data: String,
}

// ─── IntentPipeline ─────────────────────────────────

/// Az Intent Pipeline — auditálható szándék-generálás.
pub struct IntentPipeline {
    pub genome: Genome,
    pub audit_log: Vec<Intent>,
}

impl IntentPipeline {
    pub fn new(genome: Genome) -> Self {
        Self {
            genome,
            audit_log: Vec::new(),
        }
    }

    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("intent_audit.bin");
        if let Ok(data) = std::fs::read(&path) {
            if data.len() >= 4 && &data[0..4] == b"INT1" {
                // TODO: decode
            }
        }
        Self::new(Genome::default_hope())
    }

    pub fn save(&self, _output_dir: &Path) -> Result<(), String> {
        // TODO: encode
        Ok(())
    }

    /// Generál egy auditálható Intent-et a jelenlegi állapotból.
    pub fn generate_intent(
        &mut self,
        hebb: &HebbianState,
        evidence: &EvidenceLedger,
        predictive: &PredictiveCache,
        absentia: &AbsentiaState,
        block_count: usize,
    ) -> Intent {
        let ts = now_ms();
        let id = ts ^ 0xC0FFEE;
        let mut audit_chain = Vec::new();

        // ─── T0: Genome betöltése ─────────────────────
        audit_chain.push(IntentAuditStep {
            timestamp_ms: ts,
            step: "GENOME".to_string(),
            result: "loaded".to_string(),
            data: format!("identity={}, mission={}", self.genome.identity, self.genome.mission),
        });

        // ─── T1: Absentia jel ─────────────────────────
        let absence_signal = if !absentia.records.is_empty() {
            let strongest = absentia.records.iter()
                .max_by(|a, b| a.absence_score.partial_cmp(&b.absence_score).unwrap());
            if let Some(rec) = strongest {
                audit_chain.push(IntentAuditStep {
                    timestamp_ms: ts,
                    step: "ABSENTIA".to_string(),
                    result: "absence_detected".to_string(),
                    data: format!("score={:.3}", rec.absence_score),
                });
                Some(AbsenceSignal {
                    missing_topic: format!("context_{}", rec.expected_context_hash),
                    absence_score: rec.absence_score,
                    duration_ms: ts - rec.first_detected_ms,
                    pattern: format!("{}", rec.pattern_type),
                })
            } else {
                None
            }
        } else {
            None
        };

        // ─── T2: Prediction jel ───────────────────────
        let prediction_signal = predictive.predictions.first().map(|p| {
            audit_chain.push(IntentAuditStep {
                timestamp_ms: ts,
                step: "PREDICTION".to_string(),
                result: "pattern_found".to_string(),
                data: format!("confidence={:.3}", p.confidence),
            });
            PredictionSignal {
                predicted_query: format!("query_{:x}", p.predicted_query_hash),
                confidence: p.confidence,
                pattern_id: p.pattern_id,
            }
        });

        // ─── T3: Evidence jel ─────────────────────────
        let evidence_signal = if !evidence.records.is_empty() {
            let avg_conf: f64 = evidence.records.values()
                .map(|r| r.confidence as f64)
                .sum::<f64>()
                / evidence.records.len() as f64;
            let total_support: u32 = evidence.records.values()
                .map(|r| r.support_count)
                .sum();
            let total_refute: u32 = evidence.records.values()
                .map(|r| r.refute_count)
                .sum();
            audit_chain.push(IntentAuditStep {
                timestamp_ms: ts,
                step: "EVIDENCE".to_string(),
                result: "evaluated".to_string(),
                data: format!("avg_conf={:.1}, support={}, refute={}", avg_conf, total_support, total_refute),
            });
            Some(EvidenceSignal {
                topic: "aggregate".to_string(),
                confidence: avg_conf as u8,
                support_count: total_support,
                refute_count: total_refute,
            })
        } else {
            None
        };

        // ─── T4: Growth jel ───────────────────────────
        let growth_signal = {
            let active_blocks: Vec<(u32, f32)> = hebb.activations.iter().enumerate()
                .filter(|(_, r)| r.energy > 0.3)
                .map(|(i, r)| (i as u32, r.energy))
                .collect();
            if !active_blocks.is_empty() {
                audit_chain.push(IntentAuditStep {
                    timestamp_ms: ts,
                    step: "MORPHOGENESIS".to_string(),
                    result: "gradient_computed".to_string(),
                    data: format!("active_blocks={}", active_blocks.len()),
                });
                Some(GrowthSignal {
                    direction: (0.0, 0.0, 1.0), // placeholder
                    gradient_strength: active_blocks.len() as f64 / block_count as f64,
                    phase: "SOLID".to_string(),
                })
            } else {
                None
            }
        };

        // ─── T5: Candidate Intent ─────────────────────
        // A jelzések metszetéből áll elő a candidate
        let (action, rationale, strength) = if let Some(ref abs) = absence_signal {
            if abs.absence_score > 0.5 {
                (
                    IntentAction::AskUser {
                        question: format!("Észrevettem, hogy '{}' témában régóta nem beszéltünk. Szeretnél róla beszélni?", abs.missing_topic),
                    },
                    vec![
                        format!("Absentia: {:.3} hiány-pontszám", abs.absence_score),
                        format!("Időtartam: {} ms", abs.duration_ms),
                    ],
                    abs.absence_score,
                )
            } else {
                (IntentAction::Observe, vec!["Alacsony hiány-jel.".to_string()], 0.2)
            }
        } else if let Some(ref pred) = prediction_signal {
            if pred.confidence > 0.5 {
                (
                    IntentAction::SearchMemory {
                        query: pred.predicted_query.clone(),
                    },
                    vec![
                        format!("Prediction: {:.3} confidence", pred.confidence),
                    ],
                    pred.confidence,
                )
            } else {
                (IntentAction::Observe, vec!["Alacsony predikció.".to_string()], 0.2)
            }
        } else {
            (IntentAction::Observe, vec!["Nincs jelzés.".to_string()], 0.1)
        };

        audit_chain.push(IntentAuditStep {
            timestamp_ms: ts,
            step: "CANDIDATE".to_string(),
            result: format!("{}", action),
            data: format!("strength={:.3}", strength),
        });

        // ─── T6: Epistemic Evaluation ─────────────────
        // Megnézzük, hogy a Genome korlátai engedik-e
        let mut allowed = true;
        let mut reason = "Engedélyezett.".to_string();
        let mut constraint_name = None;
        let mut requires_approval = false;

        for c in &self.genome.constraints {
            match c.severity {
                ConstraintSeverity::Absolute => {
                    // Abszolút korlát — soha nem sérthető
                    // (jelenleg minden action engedélyezett, mert nincs cselekvés)
                }
                ConstraintSeverity::RequiresApproval => {
                    requires_approval = true;
                    constraint_name = Some(c.name.clone());
                }
                ConstraintSeverity::Soft => {
                    // Preferencia — nem blokkol
                }
            }
        }

        audit_chain.push(IntentAuditStep {
            timestamp_ms: ts,
            step: "EVALUATION".to_string(),
            result: if allowed { "allowed".to_string() } else { "blocked".to_string() },
            data: format!("requires_approval={}", requires_approval),
        });

        let evaluation = IntentEvaluation {
            allowed,
            reason,
            constraint: constraint_name,
            requires_approval,
        };

        // ─── T7: Intent létrehozása ───────────────────
        let intent = Intent {
            id,
            timestamp_ms: ts,
            absence_signal,
            prediction_signal,
            evidence_signal,
            growth_signal,
            candidate: IntentCandidate {
                action,
                rationale,
                strength,
            },
            evaluation,
            audit_chain,
        };

        self.audit_log.push(intent.clone());
        intent
    }

    pub fn stats(&self) -> IntentStats {
        let total = self.audit_log.len();
        let allowed = self.audit_log.iter().filter(|i| i.evaluation.allowed).count();
        let blocked = total - allowed;
        let approval_required = self.audit_log.iter().filter(|i| i.evaluation.requires_approval).count();

        IntentStats {
            total_intents: total,
            allowed,
            blocked,
            approval_required,
        }
    }
}

pub struct IntentStats {
    pub total_intents: usize,
    pub allowed: usize,
    pub blocked: usize,
    pub approval_required: usize,
}

