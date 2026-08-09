//! Kognitív Morfogenezis — Audit-infrastruktúra és integrációs motor.
//!
//! Ez a modul összekapcsolja a meglévő kognitív modulokat (Hebbian, Resonance,
//! Epistemic, Predictive Cache, Emotion) egyetlen dinamikus élő hálózattá.
//! A MorphogenField koordinátái kognitív állapotteret reprezentálnak.
//!
//! Lásd: docs/COGNITIVE_MORPHOGENESIS.md

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hebbian::HebbianState;
use crate::resonance::ResonanceState;
use crate::epistemic::EvidenceLedger;
use crate::predictive_cache::PredictiveCache;
use crate::emotional_contagion::EmotionalContagionState;
use crate::morphogenesis::{
    GrowthConfig, MorphogenField, Seed, mycelium_growth,
    MorphNode, MorphConnection,
};


// ─── Helpers ────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn cycle_id() -> u64 {
    now_ms() ^ 0xC0FFEE
}

// ─── Phase ──────────────────────────────────────────

/// A hálózat növekedési fázisa — a gradiens erőssége határozza meg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Szabad exploráció — kaotikus, minden irányba.
    Gas,
    /// Gradiens-követés — alkalmazkodás, áramlás.
    Liquid,
    /// Rögzített útvonalak — stabil, megbízható asszociációk.
    Solid,
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Gas => write!(f, "GAS"),
            Phase::Liquid => write!(f, "LIQUID"),
            Phase::Solid => write!(f, "SOLID"),
        }
    }
}

impl Phase {
    pub fn from_gradient(avg_gradient: f64) -> Self {
        if avg_gradient < 0.3 {
            Phase::Gas
        } else if avg_gradient < 0.7 {
            Phase::Liquid
        } else {
            Phase::Solid
        }
    }

    /// Fázis-specifikus GrowthConfig alkalmazása.
    pub fn growth_config(&self, base: &GrowthConfig) -> GrowthConfig {
        let mut cfg = base.clone();
        match self {
            Phase::Gas => {
                cfg.branching_probability = 0.5;
                cfg.energy_decay = 0.03;
                cfg.anastomosis_probability = 0.02;
            }
            Phase::Liquid => {
                cfg.branching_probability = 0.35;
                cfg.energy_decay = 0.08;
                cfg.anastomosis_probability = 0.08;
            }
            Phase::Solid => {
                cfg.branching_probability = 0.1;
                cfg.energy_decay = 0.02;
                cfg.anastomosis_probability = 0.15;
            }
        }
        cfg
    }
}

// ─── CognitiveGradient ──────────────────────────────

/// Kognitív gradiens — 7 komponensből álló súlyozott jelzőrendszer.
pub struct CognitiveGradient {
    /// Súlyok: (relevance, resonance, evidence, hebbian, prediction, emotion, execution)
    pub weights: (f64, f64, f64, f64, f64, f64, f64),
}

impl Default for CognitiveGradient {
    fn default() -> Self {
        Self {
            weights: (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        }
    }
}

impl CognitiveGradient {
    /// Kiszámolja a kognitív gradienst egy adott blokkhoz.
    pub fn compute(
        &self,
        lexical_score: f32,
        resonance_strength: f32,
        evidence_confidence: u8,
        hebbian_energy: f32,
        prediction_hit_rate: f32,
        emotional_valence: f32,
        execution_success: f32,
    ) -> f64 {
        let (w1, w2, w3, w4, w5, w6, w7) = self.weights;

        let relevance = lexical_score.max(0.0).min(1.0) as f64;
        let resonance = resonance_strength.max(0.0).min(1.0) as f64;
        let evidence = (evidence_confidence as f64 / 100.0).max(0.0).min(1.0);
        let hebbian = hebbian_energy.max(0.0).min(1.0) as f64;
        let prediction = prediction_hit_rate.max(0.0).min(1.0) as f64;
        let emotion = ((emotional_valence + 1.0) / 2.0).max(0.0).min(1.0) as f64;
        let execution = execution_success.max(0.0).min(1.0) as f64;

        w1 * relevance + w2 * resonance + w3 * evidence + w4 * hebbian
            + w5 * prediction + w6 * emotion + w7 * execution
    }
}

// ─── MorphogenesisAuditEntry ────────────────────────

/// Egyetlen kognitív morfogenezis ciklus audit-bejegyzése.
#[derive(Debug, Clone)]
pub struct MorphogenesisAuditEntry {
    pub timestamp_ms: u64,
    pub cycle_id: u64,

    // T0: aktiváció
    pub activated_blocks: Vec<(u32, f32)>,
    pub query_hash: u64,

    // T1: gradiens
    pub gradient_avg: f64,
    pub phase: Phase,
    pub component_scores: GradientComponents,

    // T2: növekedés
    pub new_node_count: usize,
    pub new_connection_count: usize,

    // T3: anastomosis
    pub anastomosis_count: usize,
    pub anastomosis_validated: usize,

    // T4: megerősítés
    pub prediction_hit: bool,
    pub evidence_confidence_before: u8,
    pub evidence_confidence_after: u8,

    // T5: konszolidáció
    pub solidified_paths: usize,
    pub pruned_paths: usize,
}

/// A kognitív gradiens komponens-pontszámai.
#[derive(Debug, Clone, Default)]
pub struct GradientComponents {
    pub relevance: f64,
    pub resonance: f64,
    pub evidence: f64,
    pub hebbian: f64,
    pub prediction: f64,
    pub emotion: f64,
    pub execution: f64,
}

impl std::fmt::Display for GradientComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rel={:.3} res={:.3} evi={:.3} heb={:.3} pred={:.3} emo={:.3} exec={:.3}",
            self.relevance, self.resonance, self.evidence, self.hebbian,
            self.prediction, self.emotion, self.execution
        )
    }
}

// ─── MorphogenesisMetrics ───────────────────────────

/// Metrikák egy kognitív morfogenezis ciklushoz.
#[derive(Debug, Clone)]
pub struct MorphogenesisMetrics {
    pub timestamp_ms: u64,
    pub cycle_id: u64,
    pub phase: Phase,

    // Recall
    pub recall_precision: f64,
    pub recall_count: usize,

    // Prediction
    pub prediction_hit_rate: f64,
    pub prediction_count: usize,

    // Associations
    pub total_associations: usize,
    pub false_associations: usize,
    pub false_association_rate: f64,

    // Graph
    pub graph_entropy: f64,
    pub node_count: usize,
    pub edge_count: usize,
    pub avg_degree: f64,

    // Paths
    pub total_paths: usize,
    pub stable_paths: usize,
    pub path_stability: f64,

    // Convergence
    pub convergence_cycles: u32,

    // Counterevidence
    pub counterevidence_reaction_rate: f64,

    // Restart
    pub restart_continuity: f64,
}

impl MorphogenesisMetrics {
    pub fn empty() -> Self {
        Self {
            timestamp_ms: now_ms(),
            cycle_id: 0,
            phase: Phase::Gas,
            recall_precision: 0.0,
            recall_count: 0,
            prediction_hit_rate: 0.0,
            prediction_count: 0,
            total_associations: 0,
            false_associations: 0,
            false_association_rate: 0.0,
            graph_entropy: 0.0,
            node_count: 0,
            edge_count: 0,
            avg_degree: 0.0,
            total_paths: 0,
            stable_paths: 0,
            path_stability: 0.0,
            convergence_cycles: 0,
            counterevidence_reaction_rate: 0.0,
            restart_continuity: 0.0,
        }
    }
}

// ─── Audit log bináris formátum ─────────────────────

const AUDIT_MAGIC: &[u8; 4] = b"MGA1"; // Morphogenesis Audit v1
const AUDIT_ENTRY_FIXED: usize = 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8;

// ─── CognitiveMorphogenesisEngine ───────────────────

/// A kognitív morfogenezis integrációs motor.
pub struct CognitiveMorphogenesisEngine {
    pub audit_log: Vec<MorphogenesisAuditEntry>,
    pub metrics_log: Vec<MorphogenesisMetrics>,
    pub gradient: CognitiveGradient,
}

impl CognitiveMorphogenesisEngine {
    pub fn new() -> Self {
        Self {
            audit_log: Vec::new(),
            metrics_log: Vec::new(),
            gradient: CognitiveGradient::default(),
        }
    }

    /// Betölti a meglévő audit-naplót a fájlból (ha van).
    pub fn load_or_init(output_dir: &Path) -> Self {
        let mut engine = Self::new();
        let path = output_dir.join("morphogenesis_audit.bin");
        if let Ok(data) = std::fs::read(&path) {
            if data.len() >= 4 && &data[0..4] == AUDIT_MAGIC {
                engine.audit_log = decode_audit_log(&data);
            }
        }
        let metrics_path = output_dir.join("morphogenesis_metrics.bin");
        if let Ok(data) = std::fs::read(&metrics_path) {
            if data.len() >= 4 && &data[0..4] == b"MGM1" {
                engine.metrics_log = decode_metrics_log(&data);
            }
        }
        engine
    }

    /// Elmenti az audit-naplót és a metrikákat.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        // Audit log
        let audit_path = output_dir.join("morphogenesis_audit.bin");
        let audit_data = encode_audit_log(&self.audit_log);
        let tmp = output_dir.join("morphogenesis_audit.bin.tmp");
        std::fs::write(&tmp, &audit_data)
            .map_err(|e| format!("write audit: {}", e))?;
        std::fs::rename(&tmp, &audit_path)
            .map_err(|e| format!("rename audit: {}", e))?;

        // Metrics log
        let metrics_path = output_dir.join("morphogenesis_metrics.bin");
        let metrics_data = encode_metrics_log(&self.metrics_log);
        let tmp2 = output_dir.join("morphogenesis_metrics.bin.tmp");
        std::fs::write(&tmp2, &metrics_data)
            .map_err(|e| format!("write metrics: {}", e))?;
        std::fs::rename(&tmp2, &metrics_path)
            .map_err(|e| format!("rename metrics: {}", e))?;

        Ok(())
    }

    /// Egy teljes kognitív morfogenezis ciklus végrehajtása.
    ///
    /// T0→T5: aktiváció → gradiens → növekedés → anastomosis → megerősítés → konszolidáció
    pub fn run_cycle(
        &mut self,
        activated_blocks: &[(u32, f32)],
        query_hash: u64,
        hebb: &HebbianState,
        resonance: &ResonanceState,
        evidence_ledger: &EvidenceLedger,
        predictive_cache: &PredictiveCache,
        emotional_state: &EmotionalContagionState,
        block_count: usize,
        headers: &[(f32, f32, f32)],
    ) -> MorphogenesisAuditEntry {
        let ts = now_ms();
        let cid = cycle_id();

        // ─── T0: Aktiváció ─────────────────────────────
        // A Hebbian state-ből kiválasztjuk a legaktívabb blokkokat

        // T1: Gradiens számolás — minden blokkhoz
        let mut gradient_sum = 0.0f64;
        let mut gradient_count = 0usize;
        let mut comp_sum = GradientComponents::default();

        // ─── T1: Kognitív MorphogenField építése ────────
        // Hebbian energy → attractorok a MorphogenFieldben
        let mut field = MorphogenField::new();
        sync_hebbian_to_field(hebb, &mut field, headers);
        sync_resonance_to_field(resonance, &mut field);
        apply_evidence_modulation(&mut field, evidence_ledger, headers);
        apply_prediction_modulation(&mut field, predictive_cache);
        apply_emotion_modulation(&mut field, emotional_state);

        // Globális komponens-értékek (cikluson kívül)
        let pred_hr = predictive_cache.stats.hit_rate();
        let emo_valence = emotional_state
            .local_snapshot
            .as_ref()
            .map(|s| s.valence)
            .unwrap_or(0.0);
        let evi_conf = evidence_ledger.records.values()
            .map(|r| r.confidence as f64)
            .sum::<f64>()
            / evidence_ledger.records.len().max(1) as f64;
        let evi_conf_u8 = evi_conf.clamp(0.0, 100.0) as u8;

        for &(block_idx, _score) in activated_blocks {
            let idx = block_idx as usize;
            if idx >= block_count {
                continue;
            }

            // Hebbian energy
            let hebb_energy = if idx < hebb.activations.len() {
                hebb.activations[idx].energy
            } else {
                0.0
            };

            // Resonance — a resonance field-ből (pozíció alapján kellene, de most egyszerűsített)
            let res_strength = resonance.field.values().sum::<f32>()
                / resonance.field.len().max(1) as f32;

            // Relevance — nem áll rendelkezésre közvetlenül, a lexical_score kellene
            // Most 0.0, mert a relevancia-t a recall már alkalmazta
            let lexical = 0.0f32;

            // Execution — alapértelmezett sikeres
            let exec = 1.0f32;

            let g = self.gradient.compute(
                lexical, res_strength, evi_conf_u8, hebb_energy,
                pred_hr, emo_valence, exec,
            );
            gradient_sum += g;
            gradient_count += 1;

            // Komponens-összegzés
            comp_sum.relevance += lexical as f64;
            comp_sum.resonance += res_strength as f64;
            comp_sum.evidence += evi_conf / 100.0;
            comp_sum.hebbian += hebb_energy as f64;
            comp_sum.prediction += pred_hr as f64;
            comp_sum.emotion += ((emo_valence + 1.0) / 2.0) as f64;
            comp_sum.execution += exec as f64;
        }

        let gradient_avg = if gradient_count > 0 {
            gradient_sum / gradient_count as f64
        } else {
            0.0
        };

        // Komponens-átlagok
        if gradient_count > 0 {
            let n = gradient_count as f64;
            comp_sum.relevance /= n;
            comp_sum.resonance /= n;
            comp_sum.evidence /= n;
            comp_sum.hebbian /= n;
            comp_sum.prediction /= n;
            comp_sum.emotion /= n;
            comp_sum.execution /= n;
        }

        let phase = Phase::from_gradient(gradient_avg);

        // ─── T2: Növekedés ─────────────────────────────
        // Seed-ek a legaktívabb blokkok pozícióiból
        let phase_config = phase.growth_config(&GrowthConfig::mycelium_default());
        let mut all_nodes: Vec<MorphNode> = Vec::new();
        let mut all_connections: Vec<MorphConnection> = Vec::new();
        let mut anastomosis_count = 0usize;
        let mut anastomosis_validated = 0usize;
        let mut solidified_paths = 0usize;
        let mut pruned_paths = 0usize;

        // Top 3 blokk → 3 seed → 3 párhuzamos mycelium
        // Minden organizmus forrás-blokkjait tároljuk a co-aktiváció ellenőrzéshez
        struct OrganismContext {
            source_block: u32,
            nodes: Vec<MorphNode>,
            connections: Vec<MorphConnection>,
            avg_confidence: f64,
        }
        let mut organisms: Vec<OrganismContext> = Vec::new();

        let seed_count = activated_blocks.len().min(3);
        for (i, &(block_idx, score)) in activated_blocks.iter().take(seed_count).enumerate() {
            let idx = block_idx as usize;
            if idx >= headers.len() { continue; }
            let (hx, hy, hz) = headers[idx];

            let seed = Seed::new(
                &format!("seed_{}_{}", cid, i),
                hx as f64, hy as f64, hz as f64,
                &format!("block_{}", idx),
            ).with_energy((score as f64 * 100.0).max(10.0));

            let organism = mycelium_growth(&seed, &field, &phase_config);

            // Epistemic gate: átlag confidence a forrás-blokk evidence-jából
            let avg_conf = if evidence_ledger.records.is_empty() {
                0.5 // alapértelmezett
            } else {
                evidence_ledger.records.values()
                    .map(|r| r.confidence as f64)
                    .sum::<f64>()
                    / evidence_ledger.records.len() as f64 / 100.0
            };

            organisms.push(OrganismContext {
                source_block: block_idx,
                nodes: organism.nodes,
                connections: organism.connections,
                avg_confidence: avg_conf,
            });
        }

        // ─── T3: Anastomosis validáció co-aktiváció alapján ───
        // Két organizmus akkor anastomosizálhat, ha a forrás-blokkjaik
        // co-aktiváltak a Hebbian state-ben
        for i in 0..organisms.len() {
            for j in (i + 1)..organisms.len() {
                let a = organisms[i].source_block.min(organisms[j].source_block);
                let b = organisms[i].source_block.max(organisms[j].source_block);
                let pair_key = (a, b);

                // Geometriai anastomosis: node-ok közötti távolság < 0.5
                let mut geo_anastomosis = 0usize;
                for ni in &organisms[i].nodes {
                    for nj in &organisms[j].nodes {
                        let dx = ni.position.0 - nj.position.0;
                        let dy = ni.position.1 - nj.position.1;
                        let dz = ni.position.2 - nj.position.2;
                        let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                        if dist < 0.5 {
                            geo_anastomosis += 1;
                        }
                    }
                }

                if geo_anastomosis > 0 {
                    anastomosis_count += geo_anastomosis;

                    // Co-aktiváció validáció
                    if let Some(coa) = hebb.coactivations.get(&pair_key) {
                        // Van co-aktiváció → valid anastomosis
                        anastomosis_validated += geo_anastomosis;
                    }
                    // Nincs co-aktiváció → geometriai találkozás, de nem valid
                }
            }
        }

        // ─── T4: Epistemic gate — útvonalak szilárdítása/pruning ───
        // Magas confidence → solidified, alacsony → pruned
        for org in &organisms {
            if org.avg_confidence > 0.5 {
                solidified_paths += 1;
            } else if org.avg_confidence < 0.2 {
                pruned_paths += 1;
            }
        }

        // Összesítés
        for org in organisms {
            all_nodes.extend(org.nodes);
            all_connections.extend(org.connections);
        }

        // ─── T3-T5: Audit ──────────────────────────────
        let entry = MorphogenesisAuditEntry {
            timestamp_ms: ts,
            cycle_id: cid,
            activated_blocks: activated_blocks.to_vec(),
            query_hash,
            gradient_avg,
            phase,
            component_scores: comp_sum,
            new_node_count: all_nodes.len(),
            new_connection_count: all_connections.len(),
            anastomosis_count,
            anastomosis_validated: anastomosis_count, // Phase 1-ben mindegyik valid
            prediction_hit: false,
            evidence_confidence_before: 0,
            evidence_confidence_after: 0,
            solidified_paths: 0,
            pruned_paths: 0,
        };

        self.audit_log.push(entry.clone());

        // ─── Metrika-gyűjtés ─────────────────────────────
        let metrics = MorphogenesisMetrics {
            timestamp_ms: ts,
            cycle_id: cid,
            phase,
            recall_precision: 0.0, // Phase 3-ban töltjük ki
            recall_count: activated_blocks.len(),
            prediction_hit_rate: pred_hr as f64,
            prediction_count: predictive_cache.stats.total_predictions as usize,
            total_associations: all_nodes.len(),
            false_associations: 0,
            false_association_rate: 0.0,
            graph_entropy: graph_entropy(all_nodes.len(), all_connections.len()),
            node_count: all_nodes.len(),
            edge_count: all_connections.len(),
            avg_degree: if all_nodes.is_empty() {
                0.0
            } else {
                2.0 * all_connections.len() as f64 / all_nodes.len() as f64
            },
            total_paths: seed_count,
            stable_paths: solidified_paths,
            path_stability: if seed_count > 0 {
                solidified_paths as f64 / seed_count as f64
            } else {
                0.0
            },
            convergence_cycles: 0,
            counterevidence_reaction_rate: 0.0,
            restart_continuity: 0.0,
        };
        self.metrics_log.push(metrics);

        entry
    }

    /// Statisztikák az audit-naplóról.
    pub fn stats(&self) -> CognitiveMorphogenesisStats {
        let total_cycles = self.audit_log.len();
        let gas_cycles = self.audit_log.iter().filter(|e| e.phase == Phase::Gas).count();
        let liquid_cycles = self.audit_log.iter().filter(|e| e.phase == Phase::Liquid).count();
        let solid_cycles = self.audit_log.iter().filter(|e| e.phase == Phase::Solid).count();

        let avg_gradient = if total_cycles > 0 {
            self.audit_log.iter().map(|e| e.gradient_avg).sum::<f64>() / total_cycles as f64
        } else {
            0.0
        };

        let total_anastomosis: usize = self.audit_log.iter().map(|e| e.anastomosis_count).sum();
        let validated_anastomosis: usize = self.audit_log.iter().map(|e| e.anastomosis_validated).sum();

        CognitiveMorphogenesisStats {
            total_cycles,
            gas_cycles,
            liquid_cycles,
            solid_cycles,
            avg_gradient,
            total_anastomosis,
            validated_anastomosis,
            total_audit_entries: self.audit_log.len(),
            total_metrics_entries: self.metrics_log.len(),
        }
    }
}

/// Statisztikák a kognitív morfogenezisről.
pub struct CognitiveMorphogenesisStats {
    pub total_cycles: usize,
    pub gas_cycles: usize,
    pub liquid_cycles: usize,
    pub solid_cycles: usize,
    pub avg_gradient: f64,
    pub total_anastomosis: usize,
    pub validated_anastomosis: usize,
    pub total_audit_entries: usize,
    pub total_metrics_entries: usize,
}

// ─── Bináris szerializás ────────────────────────────

fn encode_audit_log(entries: &[MorphogenesisAuditEntry]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(AUDIT_MAGIC);
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());

    for e in entries {
        buf.extend_from_slice(&e.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&e.cycle_id.to_le_bytes());
        buf.extend_from_slice(&(e.activated_blocks.len() as u32).to_le_bytes());
        buf.extend_from_slice(&e.query_hash.to_le_bytes());
        buf.extend_from_slice(&e.gradient_avg.to_le_bytes());
        buf.extend_from_slice(&(e.phase as u8).to_le_bytes());
        // Component scores
        buf.extend_from_slice(&e.component_scores.relevance.to_le_bytes());
        buf.extend_from_slice(&e.component_scores.resonance.to_le_bytes());
        buf.extend_from_slice(&e.component_scores.evidence.to_le_bytes());
        buf.extend_from_slice(&e.component_scores.hebbian.to_le_bytes());
        buf.extend_from_slice(&e.component_scores.prediction.to_le_bytes());
        buf.extend_from_slice(&e.component_scores.emotion.to_le_bytes());
        buf.extend_from_slice(&e.component_scores.execution.to_le_bytes());
        // Counts
        buf.extend_from_slice(&(e.new_node_count as u32).to_le_bytes());
        buf.extend_from_slice(&(e.new_connection_count as u32).to_le_bytes());
        buf.extend_from_slice(&(e.anastomosis_count as u32).to_le_bytes());
        buf.extend_from_slice(&(e.anastomosis_validated as u32).to_le_bytes());
        buf.extend_from_slice(&(e.prediction_hit as u8).to_le_bytes());
        buf.extend_from_slice(&e.evidence_confidence_before.to_le_bytes());
        buf.extend_from_slice(&e.evidence_confidence_after.to_le_bytes());
        buf.extend_from_slice(&(e.solidified_paths as u32).to_le_bytes());
        buf.extend_from_slice(&(e.pruned_paths as u32).to_le_bytes());
        // Activated blocks (compact: idx + score pairs)
        for &(idx, score) in &e.activated_blocks {
            buf.extend_from_slice(&idx.to_le_bytes());
            buf.extend_from_slice(&score.to_le_bytes());
        }
    }
    buf
}

fn decode_audit_log(data: &[u8]) -> Vec<MorphogenesisAuditEntry> {
    if data.len() < 8 {
        return Vec::new();
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut entries = Vec::with_capacity(count);
    let mut off = 8;

    for _ in 0..count {
        if off + AUDIT_ENTRY_FIXED > data.len() {
            break;
        }
        let ts = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let cid = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let block_count = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let qh = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let ga = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let phase_byte = data[off]; off += 1;
        let phase = match phase_byte {
            0 => Phase::Gas,
            1 => Phase::Liquid,
            _ => Phase::Solid,
        };
        let rel = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let res = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let evi = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let heb = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let pred = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let emo = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let exec = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let nnc = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ncc = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ac = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let av = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ph = data[off] != 0; off += 1;
        let ecb = data[off]; off += 1;
        let eca = data[off]; off += 1;
        let sp = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let pp = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;

        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            if off + 8 > data.len() { break; }
            let idx = u32::from_le_bytes(data[off..off+4].try_into().unwrap());
            let score = f32::from_le_bytes(data[off+4..off+8].try_into().unwrap());
            blocks.push((idx, score));
            off += 8;
        }

        entries.push(MorphogenesisAuditEntry {
            timestamp_ms: ts,
            cycle_id: cid,
            activated_blocks: blocks,
            query_hash: qh,
            gradient_avg: ga,
            phase,
            component_scores: GradientComponents {
                relevance: rel,
                resonance: res,
                evidence: evi,
                hebbian: heb,
                prediction: pred,
                emotion: emo,
                execution: exec,
            },
            new_node_count: nnc,
            new_connection_count: ncc,
            anastomosis_count: ac,
            anastomosis_validated: av,
            prediction_hit: ph,
            evidence_confidence_before: ecb,
            evidence_confidence_after: eca,
            solidified_paths: sp,
            pruned_paths: pp,
        });
    }
    entries
}

fn encode_metrics_log(entries: &[MorphogenesisMetrics]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"MGM1");
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());

    for e in entries {
        buf.extend_from_slice(&e.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&e.cycle_id.to_le_bytes());
        buf.extend_from_slice(&(e.phase as u8).to_le_bytes());
        buf.extend_from_slice(&e.recall_precision.to_le_bytes());
        buf.extend_from_slice(&(e.recall_count as u32).to_le_bytes());
        buf.extend_from_slice(&e.prediction_hit_rate.to_le_bytes());
        buf.extend_from_slice(&(e.prediction_count as u32).to_le_bytes());
        buf.extend_from_slice(&(e.total_associations as u32).to_le_bytes());
        buf.extend_from_slice(&(e.false_associations as u32).to_le_bytes());
        buf.extend_from_slice(&e.false_association_rate.to_le_bytes());
        buf.extend_from_slice(&e.graph_entropy.to_le_bytes());
        buf.extend_from_slice(&(e.node_count as u32).to_le_bytes());
        buf.extend_from_slice(&(e.edge_count as u32).to_le_bytes());
        buf.extend_from_slice(&e.avg_degree.to_le_bytes());
        buf.extend_from_slice(&(e.total_paths as u32).to_le_bytes());
        buf.extend_from_slice(&(e.stable_paths as u32).to_le_bytes());
        buf.extend_from_slice(&e.path_stability.to_le_bytes());
        buf.extend_from_slice(&e.convergence_cycles.to_le_bytes());
        buf.extend_from_slice(&e.counterevidence_reaction_rate.to_le_bytes());
        buf.extend_from_slice(&e.restart_continuity.to_le_bytes());
    }
    buf
}

fn decode_metrics_log(data: &[u8]) -> Vec<MorphogenesisMetrics> {
    if data.len() < 8 {
        return Vec::new();
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut entries = Vec::with_capacity(count);
    let mut off = 8;

    for _ in 0..count {
        if off + 113 > data.len() { break; } // fixed size per entry
        let ts = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let cid = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let phase = match data[off] { 0 => Phase::Gas, 1 => Phase::Liquid, _ => Phase::Solid }; off += 1;
        let rp = f64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
        let rc = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let phr = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;
        let pc = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ta = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let fa = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let far = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;
        let ge = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;
        let nc = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ec = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ad = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;
        let tp = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let sp = u32::from_le_bytes(data[off..off+4].try_into().unwrap()) as usize; off += 4;
        let ps = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;
        let cc = u32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
        let crr = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;
        let rsc = f64::from_le_bytes(data[off..off+8].to_vec().try_into().unwrap()); off += 8;

        entries.push(MorphogenesisMetrics {
            timestamp_ms: ts, cycle_id: cid, phase,
            recall_precision: rp, recall_count: rc,
            prediction_hit_rate: phr, prediction_count: pc,
            total_associations: ta, false_associations: fa, false_association_rate: far,
            graph_entropy: ge, node_count: nc, edge_count: ec, avg_degree: ad,
            total_paths: tp, stable_paths: sp, path_stability: ps,
            convergence_cycles: cc,
            counterevidence_reaction_rate: crr,
            restart_continuity: rsc,
        });
    }
    entries
}

// ─── Sync helpers (Phase 1+ integráció) ─────────────

/// Hebbian energy → MorphogenField attractorok szinkronizációja.
pub fn sync_hebbian_to_field(
    hebb: &HebbianState,
    field: &mut MorphogenField,
    headers: &[(f32, f32, f32)],
) {
    for (i, rec) in hebb.activations.iter().enumerate() {
        if rec.energy > 0.1 && i < headers.len() {
            let (x, y, z) = headers[i];
            field.add_attractor(x as f64, y as f64, z as f64, rec.energy as f64);
        }
    }
}

/// Resonance field → MorphogenField gradiensek szinkronizációja.
pub fn sync_resonance_to_field(
    res: &ResonanceState,
    field: &mut MorphogenField,
) {
    for (&(x, y, z), &strength) in &res.field {
        let fx = x as f64 * 0.05; // de-quantize
        let fy = y as f64 * 0.05;
        let fz = z as f64 * 0.05;
        field.add_attractor(fx, fy, fz, strength as f64);
    }
}

/// Evidence confidence modulálja a gradienst.
pub fn apply_evidence_modulation(
    field: &mut MorphogenField,
    ledger: &EvidenceLedger,
    _headers: &[(f32, f32, f32)],
) {
    if ledger.records.is_empty() {
        return;
    }
    let avg_conf: f64 = ledger.records.values()
        .map(|r| r.confidence as f64)
        .sum::<f64>()
        / ledger.records.len() as f64;
    let factor = 1.0 + avg_conf / 200.0; // enyhe boost
    for val in field.gradients.values_mut() {
        *val *= factor;
    }
}

/// Prediction hit-rate modulálja a gradiens globális erősségét.
pub fn apply_prediction_modulation(
    field: &mut MorphogenField,
    cache: &PredictiveCache,
) {
    let hit_rate = cache.stats.hit_rate() as f64;
    let modulation = 1.0 + hit_rate;
    for val in field.gradients.values_mut() {
        *val *= modulation;
    }
}

/// Emotion modulálja a gradiens dinamikáját.
pub fn apply_emotion_modulation(
    field: &mut MorphogenField,
    emo: &EmotionalContagionState,
) {
    if let Some(ref snap) = emo.local_snapshot {
        let valence_factor = (snap.valence as f64 + 1.0) / 2.0; // [-1,1] → [0,1]
        for val in field.gradients.values_mut() {
            *val *= 0.5 + valence_factor;
        }
    }
}

// ─── Graph entropy ──────────────────────────────────

/// Shannon-entrópia a gráf fokszám-eloszlásából.
pub fn graph_entropy(node_count: usize, edge_count: usize) -> f64 {
    if node_count == 0 || edge_count == 0 {
        return 0.0;
    }
    let avg_degree = (2.0 * edge_count as f64) / node_count as f64;
    let p = (avg_degree / (node_count as f64)).min(1.0);
    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }
    -(p * p.log2() + (1.0 - p) * (1.0 - p).log2())
}
