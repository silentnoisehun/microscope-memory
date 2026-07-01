//! Heuristic Decision Layer — gyors, heurisztikus döntéshozatal a kognitív modulok integrációjával.
//!
//! Ez a réteg egyesíti a salience (kiemelés), eureka (betekintés), meta_supervision (meta-felügyelet)
//! és architecture_simulator modulokat egy egységes döntéshozó rendszerré.
//! Képes mintázatfelismerésre, gyors heurisztikus döntésekre és folyamatos tanulásra.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::architecture_simulator::{ArchitectureSimulator, SimulationConfig, SimulationMetrics};
use crate::eureka::EurekaLog;
use crate::knowledge_base::KnowledgeBase;
use crate::meta_supervision::MetaSupervisor;
use crate::salience::SalienceState;

// ─── Alap típusok ───────────────────────────────────────────────────────────

/// Döntés típusa
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DecisionType {
    /// Architektúra választás
    ArchitectureSelection,
    /// Erőforrás allokáció
    ResourceAllocation,
    /// Optimalizációs stratégia
    OptimizationStrategy,
    /// Kockázatkezelés
    RiskManagement,
    /// Tanulási stratégia
    LearningStrategy,
    /// Egyedi
    Custom(String),
}

/// Döntési lehetőség
#[derive(Debug, Clone)]
pub struct DecisionOption {
    pub id: String,
    pub description: String,
    pub decision_type: DecisionType,
    /// Várható haszon (0.0 - 1.0)
    pub expected_utility: f64,
    /// Kockázati szint (0.0 - 1.0)
    pub risk_level: f64,
    /// Végrehajtási költség (0.0 - 1.0)
    pub execution_cost: f64,
    /// Bizonyossági szint (0.0 - 1.0)
    pub confidence: f64,
    /// Salience pontszám (a kiemelési hálózatból)
    pub salience_score: f64,
    /// Eureka pontszám (váratlan összefüggések)
    pub eureka_score: f64,
    /// Meta-felügyeleti pontszám
    pub meta_score: f64,
    /// Szimulációs előrejelzés (ha van)
    pub simulation_prediction: Option<SimulationMetrics>,
}

/// Döntési eredmény
#[derive(Debug, Clone)]
pub struct Decision {
    pub id: String,
    pub timestamp: u64,
    pub selected_option: DecisionOption,
    pub alternatives: Vec<DecisionOption>,
    pub reasoning: Vec<String>,
    pub confidence_level: f64,
    pub expected_outcome: String,
}

/// Heurisztikus minta
#[derive(Debug, Clone)]
pub struct HeuristicPattern {
    pub id: String,
    pub name: String,
    pub pattern_type: String,
    /// Milyen kontextusban érvényes
    pub context: String,
    /// Milyen gyakran hozott helyes döntést (0.0 - 1.0)
    pub success_rate: f64,
    /// Hányszor használtuk
    pub usage_count: u64,
    /// Utolsó használat időpontja
    pub last_used: u64,
    /// A minta súlya a döntéshozatalban
    pub weight: f64,
}

/// Döntési napló bejegyzés
#[derive(Debug, Clone)]
pub struct DecisionLogEntry {
    pub timestamp: u64,
    pub decision_id: String,
    pub decision_type: DecisionType,
    pub selected_option: String,
    pub outcome_score: f64,
    pub reflection: String,
}

// ─── Heurisztikus Döntéshozó ─────────────────────────────────────────────────

/// A fő heurisztikus döntéshozó rendszer
pub struct HeuristicDecisionMaker {
    /// Salience hálózat — kiemelés
    salience: Arc<RwLock<SalienceState>>,
    /// Eureka detektor — váratlan összefüggések
    eureka: Arc<RwLock<EurekaLog>>,
    /// Meta-felügyelet — teljesítményfigyelés
    meta_supervisor: Arc<RwLock<MetaSupervisor>>,
    /// Architektúra szimulátor
    simulator: Arc<ArchitectureSimulator>,
    /// Tudásbázis
    knowledge_base: Arc<KnowledgeBase>,
    /// Tanult heurisztikus minták
    patterns: Arc<RwLock<HashMap<String, HeuristicPattern>>>,
    /// Döntési napló
    decision_log: Arc<RwLock<Vec<DecisionLogEntry>>>,
    /// Döntési preferenciák (pl. "risk_averse", "aggressive", "balanced")
    preference: String,
    /// Tanulási ráta (0.0 - 1.0)
    learning_rate: f64,
}

impl HeuristicDecisionMaker {
    /// Létrehoz egy új döntéshozót
    pub fn new(
        salience: Arc<RwLock<SalienceState>>,
        eureka: Arc<RwLock<EurekaLog>>,
        meta_supervisor: Arc<RwLock<MetaSupervisor>>,
        simulator: Arc<ArchitectureSimulator>,
        knowledge_base: Arc<KnowledgeBase>,
    ) -> Self {
        Self {
            salience,
            eureka,
            meta_supervisor,
            simulator,
            knowledge_base,
            patterns: Arc::new(RwLock::new(HashMap::new())),
            decision_log: Arc::new(RwLock::new(Vec::new())),
            preference: "balanced".to_string(),
            learning_rate: 0.1,
        }
    }

    /// Beállítja a döntési preferenciát
    pub fn set_preference(&mut self, preference: &str) {
        self.preference = preference.to_string();
    }

    /// Beállítja a tanulási rátát
    pub fn set_learning_rate(&mut self, rate: f64) {
        self.learning_rate = rate.clamp(0.0, 1.0);
    }

    /// Döntési lehetőségek értékelése és rangsorolása
    pub fn evaluate_options(&self, options: Vec<DecisionOption>) -> Vec<DecisionOption> {
        let mut scored_options: Vec<(f64, DecisionOption)> = options
            .into_iter()
            .map(|opt| {
                let score = self.calculate_option_score(&opt);
                (score, opt)
            })
            .collect();

        // Rendezés pontszám szerint csökkenő sorrendben
        scored_options.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        scored_options.into_iter().map(|(_, opt)| opt).collect()
    }

    /// Kiszámítja egy opció összpontszámát
    fn calculate_option_score(&self, option: &DecisionOption) -> f64 {
        let mut score = 0.0;

        // 1. Várható haszon
        score += option.expected_utility * 0.25;

        // 2. Kockázat kezelése preferencia szerint
        let risk_factor = match self.preference.as_str() {
            "risk_averse" => 1.0 - option.risk_level,
            "aggressive" => option.risk_level,
            _ => 0.5 + (0.5 - option.risk_level), // balanced
        };
        score += risk_factor * 0.15;

        // 3. Végrehajtási költség (minél alacsonyabb, annál jobb)
        score += (1.0 - option.execution_cost) * 0.1;

        // 4. Bizonyosság
        score += option.confidence * 0.1;

        // 5. Salience pontszám (a kiemelési hálózatból)
        // Valós idejű salience számítás a leírás alapján
        let salience_score = {
            let salience = self.salience.read().unwrap();
            let hash = SalienceState::topic_hash(&option.description);
            let computed = salience.compute_salience(0.5f32, 0.5f32, 0.5f32, hash);
            computed as f64
        };
        score += salience_score;

        // 6. Eureka pontszám (váratlan összefüggések)
        // Az események insight_score-jának átlaga
        let eureka_score = {
            let eureka = self.eureka.read().unwrap();
            let events = &eureka.events;
            if events.is_empty() {
                0.0
            } else {
                let total: f32 = events.iter().map(|e| e.insight_score()).sum();
                (total / events.len() as f32) as f64
            }
        };
        score += eureka_score * 0.1;

        // 7. Meta-felügyeleti pontszám
        let meta_score = {
            let meta = self.meta_supervisor.read().unwrap();
            let (current, _trend, _volatility) = meta.get_summary();
            current as f64
        };
        score += meta_score * 0.1;

        // 8. Szimulációs előrejelzés (ha van)
        if let Some(ref sim) = option.simulation_prediction {
            let stability_bonus = sim.stability_score * 0.05;
            let resilience_bonus = sim.resilience_score * 0.05;
            score += stability_bonus + resilience_bonus;
        }

        // 9. Tanult minták alapján korrekció
        let pattern_correction = self.apply_learned_patterns(option);
        score += pattern_correction * 0.1;

        // 10. Tudásbázisból származó megerősítés
        let kb_boost = {
            let results = self.knowledge_base.search(&option.description, 3);
            let boost: f64 = results
                .iter()
                .map(|r| r.entry.confidence * r.relevance_score.min(1.0) / 10.0)
                .sum();
            boost.min(0.2)
        };
        score += kb_boost;

        score
    }

    /// Tanult minták alkalmazása egy opcióra
    fn apply_learned_patterns(&self, option: &DecisionOption) -> f64 {
        let patterns = self.patterns.read().unwrap();
        let mut correction = 0.0;

        for pattern in patterns.values() {
            // Csak releváns mintákat alkalmazunk
            if option.description.contains(&pattern.context)
                || option
                    .decision_type
                    .to_string()
                    .contains(&pattern.pattern_type)
            {
                let recency_factor = if pattern.last_used > 0 {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let hours_since = (now - pattern.last_used) as f64 / 3600.0;
                    (-hours_since / 24.0).exp() // exponenciális felejtés
                } else {
                    0.0
                };

                correction += pattern.success_rate * pattern.weight * recency_factor;
            }
        }

        correction
    }

    /// Döntés meghozatala a legjobb opció kiválasztásával
    pub fn make_decision(&self, options: Vec<DecisionOption>) -> Option<Decision> {
        if options.is_empty() {
            return None;
        }

        let ranked = self.evaluate_options(options);
        let best = ranked.first().cloned()?;
        let alternatives: Vec<DecisionOption> = ranked.into_iter().skip(1).collect();

        // Indoklás generálása
        let reasoning = self.generate_reasoning(&best);

        let decision = Decision {
            id: format!("decision_{}", rand::random::<u32>()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            selected_option: best.clone(),
            alternatives,
            reasoning,
            confidence_level: best.confidence,
            expected_outcome: format!(
                "Expected utility: {:.2}, Risk: {:.2}, Confidence: {:.2}",
                best.expected_utility, best.risk_level, best.confidence
            ),
        };

        // Döntés naplózása
        self.log_decision(&decision);

        Some(decision)
    }

    /// Indoklás generálása a döntéshez
    fn generate_reasoning(&self, option: &DecisionOption) -> Vec<String> {
        let mut reasoning = Vec::new();

        reasoning.push(format!(
            "Selected option: {} (type: {:?})",
            option.description, option.decision_type
        ));

        if option.salience_score > 0.7 {
            reasoning.push(format!(
                "High salience detected ({:.2}) — option stands out from alternatives",
                option.salience_score
            ));
        }

        if option.eureka_score > 0.5 {
            reasoning.push(format!(
                "Eureka insight detected ({:.2}) — unexpected connection found",
                option.eureka_score
            ));
        }

        if let Some(ref sim) = option.simulation_prediction {
            reasoning.push(format!(
                "Simulation predicts: stability={:.2}, resilience={:.2}, bottleneck={}",
                sim.stability_score,
                sim.resilience_score,
                sim.bottleneck_components.len()
            ));
        }

        reasoning.push(format!(
            "Risk/reward ratio: {:.2}",
            option.expected_utility / (option.risk_level + 0.01)
        ));

        reasoning
    }

    /// Döntés naplózása
    fn log_decision(&self, decision: &Decision) {
        let mut log = self.decision_log.write().unwrap();
        log.push(DecisionLogEntry {
            timestamp: decision.timestamp,
            decision_id: decision.id.clone(),
            decision_type: decision.selected_option.decision_type.clone(),
            selected_option: decision.selected_option.description.clone(),
            outcome_score: 0.0, // kezdetben ismeretlen
            reflection: String::new(),
        });
    }

    /// Döntés utólagos értékelése (tanulás)
    pub fn evaluate_decision_outcome(
        &self,
        decision_id: &str,
        outcome_score: f64,
        reflection: &str,
    ) {
        let mut log = self.decision_log.write().unwrap();

        if let Some(entry) = log.iter_mut().find(|e| e.decision_id == decision_id) {
            entry.outcome_score = outcome_score;
            entry.reflection = reflection.to_string();

            // Minta tanulása
            self.learn_from_outcome(entry, outcome_score);
        }
    }

    /// Tanulás a döntés kimeneteléből
    fn learn_from_outcome(&self, entry: &DecisionLogEntry, outcome_score: f64) {
        let mut patterns = self.patterns.write().unwrap();
        let pattern_id = format!("pattern_{}", entry.decision_type);

        let pattern = patterns.entry(pattern_id).or_insert(HeuristicPattern {
            id: format!("pattern_{}", rand::random::<u32>()),
            name: format!("{}_pattern", entry.decision_type),
            pattern_type: entry.decision_type.to_string(),
            context: entry.selected_option.clone(),
            success_rate: 0.5,
            usage_count: 0,
            last_used: entry.timestamp,
            weight: 0.5,
        });

        pattern.usage_count += 1;
        pattern.last_used = entry.timestamp;

        // Sikerráta frissítése (exponenciális mozgóátlag)
        let learning = self.learning_rate;
        pattern.success_rate = pattern.success_rate * (1.0 - learning) + outcome_score * learning;

        // Súly frissítése a konzisztencia alapján
        if outcome_score > 0.7 {
            pattern.weight = (pattern.weight + learning).min(1.0);
        } else if outcome_score < 0.3 {
            pattern.weight = (pattern.weight - learning).max(0.0);
        }
    }

    /// Architektúra ajánlás a szimulátor segítségével
    pub fn recommend_architecture(&self, requirements: &str) -> Option<Decision> {
        let architectures = self.simulator.list_architectures();

        if architectures.is_empty() {
            return None;
        }

        let options: Vec<DecisionOption> = architectures
            .iter()
            .map(|arch| {
                // Szimuláció futtatása
                let config = SimulationConfig {
                    duration_secs: 30.0,
                    time_step_ms: 100.0,
                    max_concurrent_requests: 500,
                    load_pattern: "sine".to_string(),
                    peak_load: 0.7,
                    enable_fault_injection: true,
                    fault_rate: 0.01,
                };

                let sim_result = self.simulator.run_simulation(&arch.id, &config);

                // Salience pontszám (valós salience számítás)
                let salience_score = {
                    let salience = self.salience.read().unwrap();
                    let hash =
                        SalienceState::topic_hash(&format!("{} {}", arch.name, requirements));
                    salience.compute_salience(0.5f32, 0.5f32, 0.5f32, hash) as f64
                };

                // Eureka pontszám (insight score alapján)
                let eureka_score = {
                    let eureka = self.eureka.read().unwrap();
                    let events = &eureka.events;
                    if events.is_empty() {
                        0.0
                    } else {
                        let total: f32 = events.iter().map(|e| e.insight_score()).sum();
                        (total / events.len() as f32) as f64
                    }
                };

                // Meta-felügyeleti pontszám
                let meta_score = {
                    let meta = self.meta_supervisor.read().unwrap();
                    let (current, _trend, _volatility) = meta.get_summary();
                    current as f64
                };

                DecisionOption {
                    id: arch.id.clone(),
                    description: format!("{}: {}", arch.name, arch.description),
                    decision_type: DecisionType::ArchitectureSelection,
                    expected_utility: arch.cohesion_score,
                    risk_level: 1.0 - arch.cohesion_score,
                    execution_cost: 0.3,
                    confidence: 0.7,
                    salience_score,
                    eureka_score,
                    meta_score,
                    simulation_prediction: sim_result,
                }
            })
            .collect();

        self.make_decision(options)
    }

    /// Gyors heurisztikus döntés (időkorlátos)
    pub fn quick_decision(
        &self,
        options: Vec<DecisionOption>,
        time_budget_ms: u64,
    ) -> Option<Decision> {
        let start = std::time::Instant::now();

        // Csak a legfontosabb tényezőket értékeljük
        let scored: Vec<(f64, DecisionOption)> = options
            .into_iter()
            .map(|opt| {
                let elapsed = start.elapsed().as_millis() as u64;
                if elapsed > time_budget_ms {
                    // Időkeret túllépve — csak a várható hasznot nézzük
                    return (opt.expected_utility, opt);
                }

                // Gyorsított pontszámítás (kevesebb tényező)
                let score = opt.expected_utility * 0.4
                    + (1.0 - opt.risk_level) * 0.3
                    + opt.confidence * 0.3;
                (score, opt)
            })
            .collect();

        let best = scored
            .into_iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())?;

        Some(Decision {
            id: format!("quick_decision_{}", rand::random::<u32>()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            selected_option: best.1,
            alternatives: Vec::new(),
            reasoning: vec!["Quick decision — limited evaluation".to_string()],
            confidence_level: 0.5,
            expected_outcome: "Quick heuristic assessment".to_string(),
        })
    }

    /// Mintázat felismerés a döntési naplóban
    pub fn recognize_patterns(&self) -> Vec<HeuristicPattern> {
        let log = self.decision_log.read().unwrap();
        let mut pattern_map: HashMap<String, Vec<f64>> = HashMap::new();

        for entry in log.iter() {
            let key = format!("{:?}", entry.decision_type);
            pattern_map
                .entry(key)
                .or_default()
                .push(entry.outcome_score);
        }

        pattern_map
            .into_iter()
            .map(|(key, scores)| {
                let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
                HeuristicPattern {
                    id: format!("recognized_{}", rand::random::<u32>()),
                    name: format!("{}_pattern", key),
                    pattern_type: key.clone(),
                    context: "general".to_string(),
                    success_rate: avg_score,
                    usage_count: scores.len() as u64,
                    last_used: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    weight: avg_score,
                }
            })
            .collect()
    }

    /// Döntési statisztikák lekérdezése
    pub fn get_statistics(&self) -> DecisionStatistics {
        let log = self.decision_log.read().unwrap();
        let patterns = self.patterns.read().unwrap();

        let total_decisions = log.len() as u64;
        let successful = log.iter().filter(|e| e.outcome_score > 0.7).count() as u64;
        let failed = log.iter().filter(|e| e.outcome_score < 0.3).count() as u64;

        DecisionStatistics {
            total_decisions,
            successful_decisions: successful,
            failed_decisions: failed,
            success_rate: if total_decisions > 0 {
                successful as f64 / total_decisions as f64
            } else {
                0.0
            },
            learned_patterns: patterns.len() as u64,
            current_preference: self.preference.clone(),
            learning_rate: self.learning_rate,
        }
    }

    /// Döntési napló exportálása
    pub fn export_decision_log(&self) -> Vec<DecisionLogEntry> {
        let log = self.decision_log.read().unwrap();
        log.clone()
    }

    /// Tanult minták exportálása
    pub fn export_patterns(&self) -> Vec<HeuristicPattern> {
        let patterns = self.patterns.read().unwrap();
        patterns.values().cloned().collect()
    }
}

/// Döntési statisztikák
#[derive(Debug, Clone)]
pub struct DecisionStatistics {
    pub total_decisions: u64,
    pub successful_decisions: u64,
    pub failed_decisions: u64,
    pub success_rate: f64,
    pub learned_patterns: u64,
    pub current_preference: String,
    pub learning_rate: f64,
}

// ─── Segédfüggvények ────────────────────────────────────────────────────────

/// Létrehoz egy egyszerű döntési lehetőséget
pub fn create_option(
    description: &str,
    decision_type: DecisionType,
    expected_utility: f64,
    risk_level: f64,
) -> DecisionOption {
    DecisionOption {
        id: format!("opt_{}", rand::random::<u32>()),
        description: description.to_string(),
        decision_type,
        expected_utility,
        risk_level,
        execution_cost: 0.5,
        confidence: 0.7,
        salience_score: 0.5,
        eureka_score: 0.0,
        meta_score: 0.5,
        simulation_prediction: None,
    }
}

/// Döntési típus stringgé alakítása
impl std::fmt::Display for DecisionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionType::ArchitectureSelection => write!(f, "architecture_selection"),
            DecisionType::ResourceAllocation => write!(f, "resource_allocation"),
            DecisionType::OptimizationStrategy => write!(f, "optimization_strategy"),
            DecisionType::RiskManagement => write!(f, "risk_management"),
            DecisionType::LearningStrategy => write!(f, "learning_strategy"),
            DecisionType::Custom(s) => write!(f, "custom_{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture_simulator::ArchitectureSimulator;
    use crate::eureka::EurekaLog;
    use crate::knowledge_base::KnowledgeBase;
    use crate::meta_supervision::MetaSupervisor;
    use crate::salience::SalienceState;
    use std::path::Path;

    fn create_test_decision_maker() -> HeuristicDecisionMaker {
        let data_dir = Path::new("data");
        let salience = Arc::new(RwLock::new(SalienceState::load_or_init(data_dir)));
        let eureka = Arc::new(RwLock::new(EurekaLog::load_or_init(data_dir)));
        let meta = Arc::new(RwLock::new(MetaSupervisor::new()));
        let simulator = Arc::new(ArchitectureSimulator::new());
        let kb = Arc::new(KnowledgeBase::new());

        HeuristicDecisionMaker::new(salience, eureka, meta, simulator, kb)
    }

    #[test]
    fn test_make_decision() {
        let dm = create_test_decision_maker();

        let options = vec![
            create_option(
                "Use microservices architecture",
                DecisionType::ArchitectureSelection,
                0.8,
                0.3,
            ),
            create_option(
                "Use monolithic architecture",
                DecisionType::ArchitectureSelection,
                0.5,
                0.1,
            ),
            create_option(
                "Use serverless architecture",
                DecisionType::ArchitectureSelection,
                0.6,
                0.5,
            ),
        ];

        let decision = dm.make_decision(options);
        assert!(decision.is_some());
        let decision = decision.unwrap();
        assert!(!decision.reasoning.is_empty());
        assert!(decision.confidence_level > 0.0);
    }

    #[test]
    fn test_quick_decision() {
        let dm = create_test_decision_maker();

        let options = vec![
            create_option("Option A", DecisionType::ResourceAllocation, 0.9, 0.2),
            create_option("Option B", DecisionType::ResourceAllocation, 0.6, 0.4),
        ];

        let decision = dm.quick_decision(options, 100);
        assert!(decision.is_some());
    }

    #[test]
    fn test_learn_from_outcome() {
        let dm = create_test_decision_maker();

        let options = vec![create_option(
            "Test option",
            DecisionType::LearningStrategy,
            0.7,
            0.3,
        )];

        let decision = dm.make_decision(options).unwrap();
        dm.evaluate_decision_outcome(&decision.id, 0.9, "Good decision");

        let stats = dm.get_statistics();
        assert!(stats.total_decisions > 0);
        assert!(stats.success_rate > 0.0);
    }

    #[test]
    fn test_pattern_recognition() {
        let dm = create_test_decision_maker();

        // Több döntés szimulálása
        for i in 0..5 {
            let options = vec![create_option(
                &format!("Pattern option {}", i),
                DecisionType::ArchitectureSelection,
                0.7,
                0.3,
            )];
            if let Some(decision) = dm.make_decision(options) {
                dm.evaluate_decision_outcome(
                    &decision.id,
                    0.8 + (i as f64 * 0.02),
                    "Consistent pattern",
                );
            }
        }

        let patterns = dm.recognize_patterns();
        assert!(!patterns.is_empty());
    }
}
