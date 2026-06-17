//! Knowledge Base — a memóriában tárolt architektúrákból, szimulációs eredményekből
//! és döntési mintákból épített kereshető tudásbázis.
//!
//! Ez a modul folyamatosan tanul a szimulációkból, döntésekből és eureka pillanatokból,
//! és strukturált tudást biztosít a heurisztikus döntéshozó számára.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::architecture_simulator::{Architecture, SimulationMetrics};
use crate::heuristic_decision::{DecisionLogEntry, HeuristicPattern};

// ─── Alap típusok ───────────────────────────────────────────────────────────

/// Egy tudásbázis bejegyzés típusa
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KnowledgeEntryType {
    /// Architektúra minta
    ArchitecturePattern,
    /// Szimulációs eredmény
    SimulationResult,
    /// Döntési minta
    DecisionPattern,
    /// Eureka/Insight
    Insight,
    /// Heurisztikus szabály
    HeuristicRule,
    /// Legjobb gyakorlat
    BestPractice,
    /// Ismert hiba / buktató
    KnownPitfall,
    /// Egyedi
    Custom(String),
}

/// Egy tudásbázis bejegyzés
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub id: String,
    pub entry_type: KnowledgeEntryType,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    /// Kapcsolódó bejegyzések ID-i
    pub related_entries: Vec<String>,
    /// Megbízhatósági pontszám (0.0 - 1.0)
    pub confidence: f64,
    /// Hányszor volt hasznos
    pub usefulness: u64,
    /// Létrehozás időpontja
    pub created_at: u64,
    /// Utolsó módosítás
    pub updated_at: u64,
    /// Forrás (pl. melyik architektúrából, döntésből származik)
    pub source: String,
    /// Kontextus (milyen körülmények között érvényes)
    pub context: String,
}

/// Keresési eredmény
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: KnowledgeEntry,
    pub relevance_score: f64,
    pub matched_tags: Vec<String>,
}

/// Tudásbázis statisztikák
#[derive(Debug, Clone)]
pub struct KnowledgeBaseStats {
    pub total_entries: u64,
    pub architecture_patterns: u64,
    pub simulation_results: u64,
    pub decision_patterns: u64,
    pub insights: u64,
    pub heuristic_rules: u64,
    pub best_practices: u64,
    pub known_pitfalls: u64,
    pub avg_confidence: f64,
    pub total_usefulness: u64,
}

// ─── Tudásbázis ─────────────────────────────────────────────────────────────

/// A fő tudásbázis rendszer
pub struct KnowledgeBase {
    entries: Arc<RwLock<HashMap<String, KnowledgeEntry>>>,
    tag_index: Arc<RwLock<HashMap<String, Vec<String>>>>, // tag -> entry_ids
    type_index: Arc<RwLock<HashMap<KnowledgeEntryType, Vec<String>>>>, // type -> entry_ids
    /// Tanult asszociációk (entry_id -> related_entry_id -> weight)
    associations: Arc<RwLock<HashMap<String, HashMap<String, f64>>>>,
    next_id: Arc<RwLock<u64>>,
}

impl KnowledgeBase {
    /// Létrehoz egy új tudásbázist
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            tag_index: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
            associations: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Hozzáad egy új tudásbázis bejegyzést
    pub fn add_entry(&self, entry: KnowledgeEntry) -> String {
        let mut entries = self.entries.write().unwrap();
        let mut tag_index = self.tag_index.write().unwrap();
        let mut type_index = self.type_index.write().unwrap();

        let id = entry.id.clone();
        entries.insert(id.clone(), entry.clone());

        // Indexelés tagek szerint
        for tag in &entry.tags {
            tag_index.entry(tag.clone())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        // Indexelés típus szerint
        type_index.entry(entry.entry_type.clone())
            .or_insert_with(Vec::new)
            .push(id.clone());

        id
    }

    /// Lekér egy bejegyzést ID alapján
    pub fn get_entry(&self, id: &str) -> Option<KnowledgeEntry> {
        let entries = self.entries.read().unwrap();
        entries.get(id).cloned()
    }

    /// Keresés a tudásbázisban
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let entries = self.entries.read().unwrap();
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(f64, &KnowledgeEntry, Vec<String>)> = entries.values()
            .map(|entry| {
                let mut score = 0.0;

                // Cím egyezés
                if entry.title.to_lowercase().contains(&query_lower) {
                    score += 3.0;
                }

                // Leírás egyezés
                if entry.description.to_lowercase().contains(&query_lower) {
                    score += 2.0;
                }

                // Tag egyezés
                let matched_tags: Vec<String> = entry.tags.iter()
                    .filter(|tag| query_words.iter().any(|w| tag.to_lowercase().contains(w)))
                    .cloned()
                    .collect();
                score += matched_tags.len() as f64 * 1.5;

                // Kontextus egyezés
                if entry.context.to_lowercase().contains(&query_lower) {
                    score += 1.0;
                }

                // Hasznosság bónusz
                score += (entry.usefulness as f64).ln_1p() * 0.5;

                // Megbízhatóság bónusz
                score += entry.confidence * 0.5;

                (score, entry, matched_tags)
            })
            .filter(|(score, _, _)| *score > 0.0)
            .collect();

        // Rendezés pontszám szerint
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        scored.into_iter()
            .take(max_results)
            .map(|(score, entry, matched_tags)| SearchResult {
                entry: entry.clone(),
                relevance_score: score,
                matched_tags,
            })
            .collect()
    }

    /// Bejegyzés hasznosságának növelése
    pub fn mark_useful(&self, id: &str) {
        let mut entries = self.entries.write().unwrap();
        if let Some(entry) = entries.get_mut(id) {
            entry.usefulness += 1;
            entry.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }

    /// Asszociáció tanulása két bejegyzés között
    pub fn learn_association(&self, from_id: &str, to_id: &str, weight: f64) {
        let mut associations = self.associations.write().unwrap();
        associations
            .entry(from_id.to_string())
            .or_insert_with(HashMap::new)
            .entry(to_id.to_string())
            .and_modify(|w| *w = (*w + weight) / 2.0) // mozgóátlag
            .or_insert(weight);
    }

    /// Kapcsolódó bejegyzések lekérése
    pub fn get_related(&self, id: &str, max_results: usize) -> Vec<KnowledgeEntry> {
        let associations = self.associations.read().unwrap();
        let entries = self.entries.read().unwrap();

        if let Some(related) = associations.get(id) {
            let mut scored: Vec<(f64, &KnowledgeEntry)> = related.iter()
                .filter_map(|(rel_id, weight)| {
                    entries.get(rel_id).map(|entry| (*weight, entry))
                })
                .collect();

            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

            scored.into_iter()
                .take(max_results)
                .map(|(_, entry)| entry.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Architektúra minta hozzáadása a szimulációs eredményből
    pub fn add_from_simulation(&self, arch: &Architecture, metrics: &SimulationMetrics) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut tags = vec![
            "architecture".to_string(),
            arch.name.clone(),
            format!("components_{}", arch.components.len()),
            format!("connections_{}", arch.connections.len()),
        ];

        if !metrics.bottleneck_components.is_empty() {
            tags.push("bottleneck".to_string());
            for b in &metrics.bottleneck_components {
                tags.push(format!("bottleneck_{}", b));
            }
        }

        if metrics.stability_score > 0.8 {
            tags.push("high_stability".to_string());
        }
        if metrics.resilience_score > 0.8 {
            tags.push("high_resilience".to_string());
        }

        let entry = KnowledgeEntry {
            id: format!("sim_{}", rand::random::<u32>()),
            entry_type: KnowledgeEntryType::SimulationResult,
            title: format!("Simulation: {} (stability={:.2}, resilience={:.2})",
                arch.name, metrics.stability_score, metrics.resilience_score),
            description: format!(
                "Architecture '{}' simulated: avg_latency={:.2}ms, throughput={:.0}req/s, error_rate={:.2}%, bottlenecks={}",
                arch.name, metrics.avg_latency_ms, metrics.throughput_req_per_sec,
                metrics.error_rate * 100.0, metrics.bottleneck_components.len()
            ),
            tags,
            related_entries: Vec::new(),
            confidence: metrics.stability_score.min(metrics.resilience_score),
            usefulness: 0,
            created_at: now,
            updated_at: now,
            source: format!("simulation_{}", arch.id),
            context: format!("load_pattern=standard, duration={}s", metrics.simulation_time_secs),
        };

        self.add_entry(entry)
    }

    /// Döntési minta hozzáadása
    pub fn add_from_decision(&self, decision: &DecisionLogEntry, pattern: &HeuristicPattern) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = KnowledgeEntry {
            id: format!("decision_{}", rand::random::<u32>()),
            entry_type: KnowledgeEntryType::DecisionPattern,
            title: format!("Decision pattern: {} (success={:.1}%)",
                pattern.name, pattern.success_rate * 100.0),
            description: format!(
                "Decision type: {:?}, selected: {}, outcome_score: {:.2}, reflection: {}",
                decision.decision_type, decision.selected_option, decision.outcome_score, decision.reflection
            ),
            tags: vec![
                "decision".to_string(),
                format!("{:?}", decision.decision_type),
                format!("success_{:.0}", pattern.success_rate * 100.0),
            ],
            related_entries: Vec::new(),
            confidence: pattern.success_rate,
            usefulness: pattern.usage_count,
            created_at: now,
            updated_at: now,
            source: format!("decision_{}", decision.decision_id),
            context: pattern.context.clone(),
        };

        self.add_entry(entry)
    }

    /// Eureka/Insight hozzáadása
    pub fn add_insight(&self, title: &str, description: &str, tags: Vec<String>, source: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = KnowledgeEntry {
            id: format!("insight_{}", rand::random::<u32>()),
            entry_type: KnowledgeEntryType::Insight,
            title: title.to_string(),
            description: description.to_string(),
            tags: {
                let mut t = vec!["insight".to_string(), "eureka".to_string()];
                t.extend(tags);
                t
            },
            related_entries: Vec::new(),
            confidence: 0.5, // insights start with medium confidence
            usefulness: 0,
            created_at: now,
            updated_at: now,
            source: source.to_string(),
            context: "discovery".to_string(),
        };

        self.add_entry(entry)
    }

    /// Legjobb gyakorlat hozzáadása
    pub fn add_best_practice(&self, title: &str, description: &str, context: &str, tags: Vec<String>) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = KnowledgeEntry {
            id: format!("practice_{}", rand::random::<u32>()),
            entry_type: KnowledgeEntryType::BestPractice,
            title: title.to_string(),
            description: description.to_string(),
            tags: {
                let mut t = vec!["best_practice".to_string(), "recommendation".to_string()];
                t.extend(tags);
                t
            },
            related_entries: Vec::new(),
            confidence: 0.7,
            usefulness: 0,
            created_at: now,
            updated_at: now,
            source: "knowledge_base".to_string(),
            context: context.to_string(),
        };

        self.add_entry(entry)
    }

    /// Ismert hiba/buktátó hozzáadása
    pub fn add_pitfall(&self, title: &str, description: &str, context: &str, tags: Vec<String>) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = KnowledgeEntry {
            id: format!("pitfall_{}", rand::random::<u32>()),
            entry_type: KnowledgeEntryType::KnownPitfall,
            title: title.to_string(),
            description: description.to_string(),
            tags: {
                let mut t = vec!["pitfall".to_string(), "warning".to_string(), "anti_pattern".to_string()];
                t.extend(tags);
                t
            },
            related_entries: Vec::new(),
            confidence: 0.8,
            usefulness: 0,
            created_at: now,
            updated_at: now,
            source: "knowledge_base".to_string(),
            context: context.to_string(),
        };

        self.add_entry(entry)
    }

    /// Heurisztikus szabály hozzáadása
    pub fn add_heuristic_rule(&self, title: &str, description: &str, context: &str, confidence: f64, tags: Vec<String>) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let entry = KnowledgeEntry {
            id: format!("rule_{}", rand::random::<u32>()),
            entry_type: KnowledgeEntryType::HeuristicRule,
            title: title.to_string(),
            description: description.to_string(),
            tags: {
                let mut t = vec!["heuristic".to_string(), "rule".to_string()];
                t.extend(tags);
                t
            },
            related_entries: Vec::new(),
            confidence,
            usefulness: 0,
            created_at: now,
            updated_at: now,
            source: "knowledge_base".to_string(),
            context: context.to_string(),
        };

        self.add_entry(entry)
    }

    /// Típus szerinti lekérdezés
    pub fn get_by_type(&self, entry_type: &KnowledgeEntryType) -> Vec<KnowledgeEntry> {
        let type_index = self.type_index.read().unwrap();
        let entries = self.entries.read().unwrap();

        type_index.get(entry_type)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| entries.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Tag szerinti lekérdezés
    pub fn get_by_tag(&self, tag: &str) -> Vec<KnowledgeEntry> {
        let tag_index = self.tag_index.read().unwrap();
        let entries = self.entries.read().unwrap();

        tag_index.get(tag)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| entries.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Statisztikák lekérése
    pub fn get_stats(&self) -> KnowledgeBaseStats {
        let entries = self.entries.read().unwrap();
        let type_index = self.type_index.read().unwrap();

        let total = entries.len() as u64;
        let arch_count = type_index.get(&KnowledgeEntryType::ArchitecturePattern).map(|v| v.len() as u64).unwrap_or(0);
        let sim_count = type_index.get(&KnowledgeEntryType::SimulationResult).map(|v| v.len() as u64).unwrap_or(0);
        let dec_count = type_index.get(&KnowledgeEntryType::DecisionPattern).map(|v| v.len() as u64).unwrap_or(0);
        let ins_count = type_index.get(&KnowledgeEntryType::Insight).map(|v| v.len() as u64).unwrap_or(0);
        let rule_count = type_index.get(&KnowledgeEntryType::HeuristicRule).map(|v| v.len() as u64).unwrap_or(0);
        let prac_count = type_index.get(&KnowledgeEntryType::BestPractice).map(|v| v.len() as u64).unwrap_or(0);
        let pit_count = type_index.get(&KnowledgeEntryType::KnownPitfall).map(|v| v.len() as u64).unwrap_or(0);

        let avg_conf = if total > 0 {
            entries.values().map(|e| e.confidence).sum::<f64>() / total as f64
        } else {
            0.0
        };

        let total_use = entries.values().map(|e| e.usefulness).sum();

        KnowledgeBaseStats {
            total_entries: total,
            architecture_patterns: arch_count,
            simulation_results: sim_count,
            decision_patterns: dec_count,
            insights: ins_count,
            heuristic_rules: rule_count,
            best_practices: prac_count,
            known_pitfalls: pit_count,
            avg_confidence: avg_conf,
            total_usefulness: total_use,
        }
    }

    /// Tudásbázis exportálása (minden bejegyzés)
    pub fn export_all(&self) -> Vec<KnowledgeEntry> {
        let entries = self.entries.read().unwrap();
        entries.values().cloned().collect()
    }

    /// Tudásbázis törlése
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
        self.tag_index.write().unwrap().clear();
        self.type_index.write().unwrap().clear();
        self.associations.write().unwrap().clear();
    }
}

// ─── Automatikus tudásépítés ────────────────────────────────────────────────

/// Automatikusan építi a tudásbázist a szimulációs eredményekből
pub fn auto_build_from_simulations(
    kb: &KnowledgeBase,
    architectures: &[Architecture],
    simulation_results: &HashMap<String, Vec<SimulationMetrics>>,
) {
    for arch in architectures {
        if let Some(metrics_list) = simulation_results.get(&arch.id) {
            for metrics in metrics_list {
                let entry_id = kb.add_from_simulation(arch, metrics);

                // Bottleneck detektálás -> pitfall
                if !metrics.bottleneck_components.is_empty() {
                    kb.add_pitfall(
                        &format!("Bottleneck in {}: {}", arch.name, metrics.bottleneck_components.join(", ")),
                        &format!("Components {} became bottlenecks at load. Consider scaling or optimizing.",
                            metrics.bottleneck_components.join(", ")),
                        &format!("architecture={}, load_pattern=standard", arch.name),
                        vec![
                            "bottleneck".to_string(),
                            arch.name.clone(),
                            "performance".to_string(),
                        ],
                    );
                }

                // Magas stabilitás -> best practice
                if metrics.stability_score > 0.9 {
                    kb.add_best_practice(
                        &format!("High stability architecture: {}", arch.name),
                        &format!("Architecture '{}' achieved {:.1}% stability under test conditions.",
                            arch.name, metrics.stability_score * 100.0),
                        &format!("architecture={}, stability>0.9", arch.name),
                        vec![
                            "stability".to_string(),
                            arch.name.clone(),
                            "reference_architecture".to_string(),
                        ],
                    );
                }
            }
        }
    }
}

/// Automatikusan építi a tudásbázist a döntési mintákból
pub fn auto_build_from_decisions(
    kb: &KnowledgeBase,
    decision_log: &[DecisionLogEntry],
    patterns: &[HeuristicPattern],
) {
    for pattern in patterns {
        // Csak a releváns döntéseket keressük
        let relevant_decisions: Vec<&DecisionLogEntry> = decision_log.iter()
            .filter(|d| format!("{:?}", d.decision_type) == pattern.pattern_type)
            .collect();

        if let Some(latest) = relevant_decisions.last() {
            kb.add_from_decision(latest, pattern);
        }

        // Magas sikerrátájú minták -> best practice
        if pattern.success_rate > 0.8 && pattern.usage_count > 3 {
            kb.add_best_practice(
                &format!("Proven decision pattern: {}", pattern.name),
                &format!("Pattern '{}' has {:.1}% success rate after {} uses.",
                    pattern.name, pattern.success_rate * 100.0, pattern.usage_count),
                &format!("decision_type={}", pattern.pattern_type),
                vec![
                    "decision".to_string(),
                    pattern.pattern_type.clone(),
                    "proven".to_string(),
                ],
            );
        }

        // Alacsony sikerrátájú minták -> pitfall
        if pattern.success_rate < 0.3 && pattern.usage_count > 2 {
            kb.add_pitfall(
                &format!("Unreliable decision pattern: {}", pattern.name),
                &format!("Pattern '{}' has only {:.1}% success rate. Consider alternative approaches.",
                    pattern.name, pattern.success_rate * 100.0),
                &format!("decision_type={}", pattern.pattern_type),
                vec![
                    "decision".to_string(),
                    pattern.pattern_type.clone(),
                    "unreliable".to_string(),
                ],
            );
        }
    }
}

/// Automatikus asszociáció tanulás a tudásbázisban
pub fn auto_learn_associations(kb: &KnowledgeBase) {
    let entries = kb.export_all();

    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let a = &entries[i];
            let b = &entries[j];

            // Közös tagek alapján asszociáció
            let common_tags: Vec<&String> = a.tags.iter()
                .filter(|t| b.tags.contains(t))
                .collect();

            if !common_tags.is_empty() {
                let weight = common_tags.len() as f64 / a.tags.len().max(1).min(b.tags.len().max(1)) as f64;
                if weight > 0.3 {
                    kb.learn_association(&a.id, &b.id, weight);
                    kb.learn_association(&b.id, &a.id, weight);
                }
            }

            // Típus alapján asszociáció (pl. simulation -> pitfall)
            if a.entry_type == KnowledgeEntryType::SimulationResult &&
               b.entry_type == KnowledgeEntryType::KnownPitfall {
                if a.description.contains(&b.title[..b.title.len().min(20)]) {
                    kb.learn_association(&a.id, &b.id, 0.8);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_search() {
        let kb = KnowledgeBase::new();

        kb.add_best_practice(
            "Use microservices for scalability",
            "Microservices allow independent scaling of components",
            "high_load_systems",
            vec!["microservices".to_string(), "scalability".to_string()],
        );

        kb.add_pitfall(
            "Avoid tight coupling in distributed systems",
            "Tight coupling reduces fault tolerance",
            "distributed_systems",
            vec!["coupling".to_string(), "distributed".to_string()],
        );

        let results = kb.search("microservices", 10);
        assert!(!results.is_empty());
        assert!(results[0].relevance_score > 0.0);
    }

    #[test]
    fn test_get_stats() {
        let kb = KnowledgeBase::new();
        let stats = kb.get_stats();
        assert_eq!(stats.total_entries, 0);

        kb.add_best_practice("Test", "Test description", "test", vec![]);
        let stats = kb.get_stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.best_practices, 1);
    }

    #[test]
    fn test_auto_associations() {
        let kb = KnowledgeBase::new();

        let id_a = kb.add_best_practice(
            "Test Practice",
            "A test best practice",
            "test",
            vec!["test".to_string(), "practice".to_string()],
        );

        let id_b = kb.add_pitfall(
            "Test Pitfall",
            "A test pitfall",
            "test",
            vec!["test".to_string(), "pitfall".to_string()],
        );

        auto_learn_associations(&kb);
        kb.learn_association(&id_a, &id_b, 0.5);

        let related = kb.get_related(&id_a, 5);
        assert!(!related.is_empty());
    }
}
