//! Architecture Generator — a memóriában tárolt minták alapján új architektúrák generálása.
//!
//! Ez a modul a tudásbázisban tárolt architektúra mintákból, szimulációs eredményekből
//! és legjobb gyakorlatokból kiindulva automatikusan generál új architektúra javaslatokat.
//! Képes kombinálni a bevált mintákat, elkerülni az ismert buktatókat, és optimalizálni
//! a teljesítményt a tanult heurisztikák alapján.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::architecture_simulator::{
    Architecture, ArchitectureSimulator, Component, ComponentType, Connection, SimulationConfig,
    SimulationMetrics,
};
use crate::knowledge_base::{KnowledgeBase, KnowledgeEntryType};

// ─── Alap típusok ───────────────────────────────────────────────────────────

/// Generálási stratégia
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationStrategy {
    /// Kombinálja a legjobb mintákat
    Hybrid,
    /// Másol egy meglévő architektúrát optimalizációkkal
    Optimize,
    /// Új architektúra a semmiből
    Novel,
    /// Véletlenszerű variációk
    Evolutionary,
}

/// Generálási paraméterek
#[derive(Debug, Clone)]
pub struct GenerationParams {
    /// Hány komponens legyen
    pub min_components: usize,
    pub max_components: usize,
    /// Hány kapcsolat legyen
    pub min_connections: usize,
    pub max_connections: usize,
    /// Cél késleltetés (ms)
    pub target_latency_ms: f64,
    /// Cél áteresztőképesség (req/s)
    pub target_throughput: f64,
    /// Megbízhatósági követelmény (0.0 - 1.0)
    pub reliability_requirement: f64,
    /// Generálási stratégia
    pub strategy: GenerationStrategy,
    /// Generációk száma (evolúciós stratégiánál)
    pub generations: u32,
    /// Mutációs ráta (0.0 - 1.0)
    pub mutation_rate: f64,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            min_components: 3,
            max_components: 10,
            min_connections: 2,
            max_connections: 15,
            target_latency_ms: 50.0,
            target_throughput: 1000.0,
            reliability_requirement: 0.95,
            strategy: GenerationStrategy::Hybrid,
            generations: 5,
            mutation_rate: 0.1,
        }
    }
}

/// Generált architektúra javaslat
#[derive(Debug, Clone)]
pub struct ArchitectureProposal {
    pub architecture: Architecture,
    pub generation_score: f64,
    pub inspiration_sources: Vec<String>,
    pub improvements: Vec<String>,
    pub predicted_metrics: Option<SimulationMetrics>,
}

// ─── Architektúra Generátor ─────────────────────────────────────────────────

/// A fő architektúra generátor
pub struct ArchitectureGenerator {
    knowledge_base: Arc<KnowledgeBase>,
    simulator: Arc<ArchitectureSimulator>,
    params: Arc<RwLock<GenerationParams>>,
    /// Generált architektúrák története
    generation_history: Arc<RwLock<Vec<ArchitectureProposal>>>,
}

impl ArchitectureGenerator {
    /// Létrehoz egy új generátort
    pub fn new(knowledge_base: Arc<KnowledgeBase>, simulator: Arc<ArchitectureSimulator>) -> Self {
        Self {
            knowledge_base,
            simulator,
            params: Arc::new(RwLock::new(GenerationParams::default())),
            generation_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Generálási paraméterek beállítása
    pub fn set_params(&self, params: GenerationParams) {
        *self.params.write().unwrap() = params;
    }

    /// Architektúra generálása a megadott követelmények alapján
    pub fn generate(&self, requirements: &str) -> Vec<ArchitectureProposal> {
        let params = self.params.read().unwrap().clone();

        match params.strategy {
            GenerationStrategy::Hybrid => self.generate_hybrid(requirements, &params),
            GenerationStrategy::Optimize => self.generate_optimized(requirements, &params),
            GenerationStrategy::Novel => self.generate_novel(requirements, &params),
            GenerationStrategy::Evolutionary => self.generate_evolutionary(requirements, &params),
        }
    }

    /// Hibrid generálás: kombinálja a legjobb mintákat a tudásbázisból
    fn generate_hybrid(
        &self,
        requirements: &str,
        params: &GenerationParams,
    ) -> Vec<ArchitectureProposal> {
        let mut proposals = Vec::new();

        // 1. Keressünk releváns mintákat a tudásbázisban
        let relevant_entries = self.knowledge_base.search(requirements, 20);

        // 2. Azonosítsuk a bevált komponens típusokat
        let mut component_type_weights: HashMap<ComponentType, f64> = HashMap::new();
        let mut connection_patterns: Vec<(String, String, f64)> = Vec::new();

        for result in &relevant_entries {
            let entry = &result.entry;
            match entry.entry_type {
                KnowledgeEntryType::BestPractice => {
                    // Best practice -> magas súly
                    if entry.tags.contains(&"microservices".to_string()) {
                        *component_type_weights
                            .entry(ComponentType::Software)
                            .or_insert(0.0) += 0.3;
                    }
                    if entry.tags.contains(&"scalability".to_string()) {
                        *component_type_weights
                            .entry(ComponentType::Network)
                            .or_insert(0.0) += 0.2;
                    }
                }
                KnowledgeEntryType::SimulationResult => {
                    // Magas stabilitású architektúrák komponensei
                    if entry.confidence > 0.8 {
                        *component_type_weights
                            .entry(ComponentType::Software)
                            .or_insert(0.0) += 0.2;
                        *component_type_weights
                            .entry(ComponentType::Storage)
                            .or_insert(0.0) += 0.1;
                    }
                }
                KnowledgeEntryType::KnownPitfall => {
                    // Buktatók -> csökkentjük a súlyt
                    if entry.tags.contains(&"bottleneck".to_string()) {
                        *component_type_weights
                            .entry(ComponentType::Storage)
                            .or_insert(0.0) -= 0.1;
                    }
                }
                _ => {}
            }
        }

        // 3. Generáljunk 3 javaslatot különböző konfigurációkkal
        for variant in 0..3 {
            let comp_count = params.min_components + variant * 2;
            let conn_count = params.min_connections + variant;

            let mut components = HashMap::new();
            let mut connections = Vec::new();

            // Komponensek generálása súlyozott választással
            for i in 0..comp_count.min(params.max_components) {
                let comp_type = self.select_weighted_type(&component_type_weights, i);
                let comp_id = format!("comp_{}", i);
                let latency = 5.0 + (i as f64 * 2.0) + rand::random::<f64>() * 5.0;
                let error_rate = 0.005 + rand::random::<f64>() * 0.02;

                components.insert(
                    comp_id.clone(),
                    Component {
                        id: comp_id,
                        name: format!("{}_{}", Self::type_name(&comp_type), i),
                        component_type: comp_type,
                        capacity: HashMap::new(),
                        load: 0.0,
                        error_rate,
                        latency_ms: latency,
                    },
                );
            }

            // Kapcsolatok generálása
            let comp_ids: Vec<String> = components.keys().cloned().collect();
            for i in 0..conn_count
                .min(params.max_connections)
                .min(comp_ids.len().saturating_sub(1))
            {
                let from = &comp_ids[i % comp_ids.len()];
                let to = &comp_ids[(i + 1) % comp_ids.len()];
                let bandwidth = 1000.0 + rand::random::<f64>() * 9000.0;
                let protocol = if i % 2 == 0 { "HTTP/2" } else { "gRPC" };

                connections.push(Connection {
                    from: from.clone(),
                    to: to.clone(),
                    bandwidth,
                    protocol: protocol.to_string(),
                    latency_ms: 1.0 + rand::random::<f64>() * 2.0,
                    packet_loss: 0.001,
                });
            }

            let arch = Architecture {
                id: format!("generated_{}", rand::random::<u32>()),
                name: format!("Hybrid Architecture v{}", variant + 1),
                description: format!(
                    "Generated from knowledge base patterns. Requirements: {}",
                    requirements
                ),
                components,
                connections,
                version: 1,
                cohesion_score: 0.5 + rand::random::<f64>() * 0.3,
            };

            // Szimuláció futtatása
            let config = SimulationConfig {
                duration_secs: 30.0,
                time_step_ms: 100.0,
                max_concurrent_requests: 500,
                load_pattern: "sine".to_string(),
                peak_load: 0.7,
                enable_fault_injection: true,
                fault_rate: 0.01,
                ..SimulationConfig::default()
            };

            let sim_result = self.simulator.run_simulation(&arch.id, &config);

            // Pontszám számítása
            let score = self.calculate_proposal_score(&arch, &sim_result, params);

            // Inspirációs források
            let sources: Vec<String> = relevant_entries
                .iter()
                .take(3)
                .map(|r| r.entry.title.clone())
                .collect();

            // Javasolt fejlesztések
            let improvements = self.suggest_improvements(&arch, &sim_result);

            proposals.push(ArchitectureProposal {
                architecture: arch,
                generation_score: score,
                inspiration_sources: sources,
                improvements,
                predicted_metrics: sim_result,
            });
        }

        // Rendezés pontszám szerint
        proposals.sort_by(|a, b| b.generation_score.partial_cmp(&a.generation_score).unwrap());

        // Tárolás a történetben
        self.generation_history
            .write()
            .unwrap()
            .extend(proposals.clone());

        proposals
    }

    /// Optimalizált generálás: meglévő architektúra javítása
    fn generate_optimized(
        &self,
        requirements: &str,
        params: &GenerationParams,
    ) -> Vec<ArchitectureProposal> {
        let mut proposals = Vec::new();

        // Keressünk egy meglévő architektúrát a szimulátorban
        let existing = self.simulator.list_architectures();
        if existing.is_empty() {
            return self.generate_hybrid(requirements, params);
        }

        for arch in existing.iter().take(2) {
            let mut optimized = arch.clone();
            optimized.id = format!("optimized_{}", rand::random::<u32>());
            optimized.name = format!("Optimized {}", arch.name);
            optimized.version = arch.version + 1;

            // Optimalizációk:
            // 1. Csökkentsük a magas késleltetésű komponensek számát
            let mut to_remove = Vec::new();
            for (comp_id, comp) in &optimized.components {
                if comp.latency_ms > params.target_latency_ms * 2.0 {
                    to_remove.push(comp_id.clone());
                }
            }
            for comp_id in &to_remove {
                optimized.components.remove(comp_id);
                optimized
                    .connections
                    .retain(|c| c.from != *comp_id && c.to != *comp_id);
            }

            // 2. Adjunk hozzá cache réteget a teljesítmény javításához
            if !optimized
                .components
                .values()
                .any(|c| c.component_type == ComponentType::Storage)
            {
                let cache_id = "cache_0".to_string();
                optimized.components.insert(
                    cache_id.clone(),
                    Component {
                        id: cache_id.clone(),
                        name: "Cache Layer".to_string(),
                        component_type: ComponentType::Storage,
                        capacity: {
                            let mut cap = HashMap::new();
                            cap.insert("memory_mb".to_string(), 8192.0);
                            cap
                        },
                        load: 0.0,
                        error_rate: 0.001,
                        latency_ms: 1.0,
                    },
                );

                // Kapcsoljuk a cache-t az első komponenshez
                if let Some(first_comp) = optimized.components.keys().next() {
                    optimized.connections.push(Connection {
                        from: first_comp.clone(),
                        to: cache_id,
                        bandwidth: 10000.0,
                        protocol: "Redis".to_string(),
                        latency_ms: 0.5,
                        packet_loss: 0.0001,
                    });
                }
            }

            // Szimuláció
            let config = SimulationConfig {
                duration_secs: 30.0,
                time_step_ms: 100.0,
                max_concurrent_requests: 500,
                load_pattern: "sine".to_string(),
                peak_load: 0.7,
                enable_fault_injection: true,
                fault_rate: 0.01,
                ..SimulationConfig::default()
            };

            let sim_result = self.simulator.run_simulation(&optimized.id, &config);
            let score = self.calculate_proposal_score(&optimized, &sim_result, params);

            proposals.push(ArchitectureProposal {
                architecture: optimized,
                generation_score: score,
                inspiration_sources: vec![format!("Optimized from: {}", arch.name)],
                improvements: vec![
                    "Added cache layer for reduced latency".to_string(),
                    format!("Removed {} high-latency components", to_remove.len()),
                ],
                predicted_metrics: sim_result,
            });
        }

        proposals.sort_by(|a, b| b.generation_score.partial_cmp(&a.generation_score).unwrap());
        self.generation_history
            .write()
            .unwrap()
            .extend(proposals.clone());
        proposals
    }

    /// Új architektúra generálása a semmiből
    fn generate_novel(
        &self,
        requirements: &str,
        params: &GenerationParams,
    ) -> Vec<ArchitectureProposal> {
        let mut proposals = Vec::new();

        for variant in 0..3 {
            let comp_count = params.min_components + variant;
            let mut components = HashMap::new();

            // Változatos komponens típusok
            let types = [
                ComponentType::Software,
                ComponentType::Storage,
                ComponentType::Network,
                ComponentType::Hardware,
            ];

            for i in 0..comp_count {
                let comp_type = types[i % types.len()].clone();
                let comp_id = format!("novel_comp_{}", i);
                let latency = 1.0 + rand::random::<f64>() * 20.0;
                let error_rate = rand::random::<f64>() * 0.01;

                components.insert(
                    comp_id.clone(),
                    Component {
                        id: comp_id,
                        name: format!("Novel_{}_{}", Self::type_name(&comp_type), i),
                        component_type: comp_type,
                        capacity: HashMap::new(),
                        load: 0.0,
                        error_rate,
                        latency_ms: latency,
                    },
                );
            }

            // Mesh hálózat (mindenki mindenkivel)
            let comp_ids: Vec<String> = components.keys().cloned().collect();
            let mut connections = Vec::new();
            for i in 0..comp_ids.len() {
                for j in (i + 1)..comp_ids.len() {
                    if rand::random::<f64>() < 0.3 {
                        // 30% kapcsolódási valószínűség
                        connections.push(Connection {
                            from: comp_ids[i].clone(),
                            to: comp_ids[j].clone(),
                            bandwidth: 1000.0 + rand::random::<f64>() * 9000.0,
                            protocol: if rand::random() {
                                "HTTP/2".to_string()
                            } else {
                                "gRPC".to_string()
                            },
                            latency_ms: rand::random::<f64>() * 5.0,
                            packet_loss: rand::random::<f64>() * 0.01,
                        });
                    }
                }
            }

            let arch = Architecture {
                id: format!("novel_{}", rand::random::<u32>()),
                name: format!("Novel Architecture v{}", variant + 1),
                description: format!("Novel architecture for: {}", requirements),
                components,
                connections,
                version: 1,
                cohesion_score: rand::random::<f64>() * 0.5 + 0.3,
            };

            let config = SimulationConfig {
                duration_secs: 30.0,
                time_step_ms: 100.0,
                max_concurrent_requests: 500,
                load_pattern: "sine".to_string(),
                peak_load: 0.7,
                enable_fault_injection: true,
                fault_rate: 0.01,
                ..SimulationConfig::default()
            };

            let sim_result = self.simulator.run_simulation(&arch.id, &config);
            let score = self.calculate_proposal_score(&arch, &sim_result, params);

            proposals.push(ArchitectureProposal {
                architecture: arch,
                generation_score: score,
                inspiration_sources: vec!["Novel generation from scratch".to_string()],
                improvements: vec!["Novel architecture — no prior patterns used".to_string()],
                predicted_metrics: sim_result,
            });
        }

        proposals.sort_by(|a, b| b.generation_score.partial_cmp(&a.generation_score).unwrap());
        self.generation_history
            .write()
            .unwrap()
            .extend(proposals.clone());
        proposals
    }

    /// Evolúciós generálás: mutáció és szelekció
    fn generate_evolutionary(
        &self,
        requirements: &str,
        params: &GenerationParams,
    ) -> Vec<ArchitectureProposal> {
        let mut population = Vec::new();

        // Kezdeti populáció létrehozása
        for _ in 0..10 {
            let comp_count = params.min_components
                + (rand::random::<usize>() % (params.max_components - params.min_components + 1));
            let mut components = HashMap::new();

            for i in 0..comp_count {
                let comp_type = match rand::random::<u8>() % 4 {
                    0 => ComponentType::Software,
                    1 => ComponentType::Storage,
                    2 => ComponentType::Network,
                    _ => ComponentType::Hardware,
                };
                let comp_id = format!("evo_comp_{}", i);
                components.insert(
                    comp_id.clone(),
                    Component {
                        id: comp_id,
                        name: format!("Evo_{}_{}", Self::type_name(&comp_type), i),
                        component_type: comp_type,
                        capacity: HashMap::new(),
                        load: 0.0,
                        error_rate: rand::random::<f64>() * 0.02,
                        latency_ms: rand::random::<f64>() * 30.0 + 1.0,
                    },
                );
            }

            let comp_ids: Vec<String> = components.keys().cloned().collect();
            let mut connections = Vec::new();
            for i in 0..comp_ids.len().saturating_sub(1) {
                if rand::random::<f64>() < 0.5 {
                    connections.push(Connection {
                        from: comp_ids[i].clone(),
                        to: comp_ids[i + 1].clone(),
                        bandwidth: 1000.0 + rand::random::<f64>() * 9000.0,
                        protocol: if rand::random() {
                            "HTTP/2".to_string()
                        } else {
                            "gRPC".to_string()
                        },
                        latency_ms: rand::random::<f64>() * 3.0,
                        packet_loss: rand::random::<f64>() * 0.01,
                    });
                }
            }

            let arch = Architecture {
                id: format!("evo_{}", rand::random::<u32>()),
                name: format!("Evolutionary v{}", population.len() + 1),
                description: format!("Evolutionary architecture for: {}", requirements),
                components,
                connections,
                version: 1,
                cohesion_score: rand::random::<f64>() * 0.5,
            };

            self.simulator.register_architecture(arch.clone());
            population.push(arch);
        }

        // Evolúciós ciklusok
        for gen in 0..params.generations {
            let mut new_population = Vec::new();

            for arch in &population {
                // Szimuláció
                let config = SimulationConfig {
                    duration_secs: 20.0,
                    time_step_ms: 100.0,
                    max_concurrent_requests: 500,
                    load_pattern: "sine".to_string(),
                    peak_load: 0.7,
                    enable_fault_injection: true,
                    fault_rate: 0.01,
                    ..SimulationConfig::default()
                };

                let sim_result = self.simulator.run_simulation(&arch.id, &config);
                let score = self.calculate_proposal_score(arch, &sim_result, params);

                // Szelekció: csak a legjobbak maradnak
                if score > 0.5 {
                    new_population.push(arch.clone());

                    // Mutáció
                    if rand::random::<f64>() < params.mutation_rate {
                        let mut mutated = arch.clone();
                        mutated.id = format!("evo_mut_{}_{}", gen, rand::random::<u32>());
                        mutated.name = format!("Mutated Gen{}", gen + 1);
                        mutated.version = gen + 1;

                        // Véletlenszerű módosítások
                        if !mutated.components.is_empty() {
                            let keys: Vec<String> = mutated.components.keys().cloned().collect();
                            if let Some(key) = keys.get(rand::random::<usize>() % keys.len()) {
                                if let Some(comp) = mutated.components.get_mut(key) {
                                    comp.latency_ms *= 0.5 + rand::random::<f64>(); // 50-150% változás
                                    comp.error_rate *= 0.5 + rand::random::<f64>();
                                }
                            }
                        }

                        self.simulator.register_architecture(mutated.clone());
                        new_population.push(mutated);
                    }
                }
            }

            population = new_population;
            if population.is_empty() {
                break;
            }
        }

        // Végeredmény: a legjobbak
        let mut proposals = Vec::new();
        for arch in population.iter().take(3) {
            let config = SimulationConfig {
                duration_secs: 30.0,
                time_step_ms: 100.0,
                max_concurrent_requests: 500,
                load_pattern: "sine".to_string(),
                peak_load: 0.7,
                enable_fault_injection: true,
                fault_rate: 0.01,
                ..SimulationConfig::default()
            };

            let sim_result = self.simulator.run_simulation(&arch.id, &config);
            let score = self.calculate_proposal_score(arch, &sim_result, params);

            proposals.push(ArchitectureProposal {
                architecture: arch.clone(),
                generation_score: score,
                inspiration_sources: vec![format!(
                    "Evolutionary generation ({} generations)",
                    params.generations
                )],
                improvements: vec!["Evolved through natural selection".to_string()],
                predicted_metrics: sim_result,
            });
        }

        proposals.sort_by(|a, b| b.generation_score.partial_cmp(&a.generation_score).unwrap());
        self.generation_history
            .write()
            .unwrap()
            .extend(proposals.clone());
        proposals
    }

    /// Javaslat pontszámának kiszámítása
    fn calculate_proposal_score(
        &self,
        arch: &Architecture,
        sim_result: &Option<SimulationMetrics>,
        params: &GenerationParams,
    ) -> f64 {
        let mut score = 0.0;

        // Alap pontszám a kohézióból
        score += arch.cohesion_score * 0.2;

        if let Some(ref metrics) = sim_result {
            // Késleltetés pontszám
            let latency_score = if metrics.avg_latency_ms <= params.target_latency_ms {
                1.0
            } else {
                (params.target_latency_ms / metrics.avg_latency_ms).min(1.0)
            };
            score += latency_score * 0.2;

            // Áteresztőképesség pontszám
            let throughput_score =
                (metrics.throughput_req_per_sec / params.target_throughput).min(1.0);
            score += throughput_score * 0.15;

            // Stabilitás pontszám
            score += metrics.stability_score * 0.15;

            // Reziliencia pontszám
            score += metrics.resilience_score * 0.1;

            // Hibaarány büntetés
            score -= metrics.error_rate * 0.1;

            // Bottleneck büntetés
            score -= metrics.bottleneck_components.len() as f64 * 0.05;
        }

        // Komponensek számának optimalitása
        let comp_ratio = arch.components.len() as f64 / params.max_components as f64;
        if comp_ratio > 0.3 && comp_ratio < 0.8 {
            score += 0.1; // Bónusz az optimális mérethez
        }

        // Kapcsolatok sűrűsége
        let max_connections = arch.components.len() * (arch.components.len() - 1) / 2;
        if max_connections > 0 {
            let density = arch.connections.len() as f64 / max_connections as f64;
            if density > 0.2 && density < 0.6 {
                score += 0.05; // Bónusz a jó kapcsolódási sűrűséghez
            }
        }

        score.max(0.0).min(1.0)
    }

    /// Javasolt fejlesztések generálása
    fn suggest_improvements(
        &self,
        arch: &Architecture,
        sim_result: &Option<SimulationMetrics>,
    ) -> Vec<String> {
        let mut improvements = Vec::new();

        if let Some(ref metrics) = sim_result {
            if metrics.avg_latency_ms > 100.0 {
                improvements.push("Consider adding a cache layer to reduce latency".to_string());
            }
            if metrics.error_rate > 0.05 {
                improvements.push(
                    "Implement retry logic and circuit breakers for error resilience".to_string(),
                );
            }
            if !metrics.bottleneck_components.is_empty() {
                improvements.push(format!(
                    "Scale or optimize bottleneck components: {}",
                    metrics.bottleneck_components.join(", ")
                ));
            }
            if metrics.stability_score < 0.7 {
                improvements
                    .push("Add load balancing and auto-scaling for better stability".to_string());
            }
            if metrics.resilience_score < 0.7 {
                improvements
                    .push("Implement bulkhead isolation to prevent cascade failures".to_string());
            }
        }

        if arch.connections.len() < arch.components.len() {
            improvements
                .push("Consider adding redundant connections for fault tolerance".to_string());
        }

        improvements
    }

    /// Súlyozott komponens típus választás
    fn select_weighted_type(
        &self,
        weights: &HashMap<ComponentType, f64>,
        index: usize,
    ) -> ComponentType {
        if weights.is_empty() {
            return match index % 4 {
                0 => ComponentType::Software,
                1 => ComponentType::Storage,
                2 => ComponentType::Network,
                _ => ComponentType::Hardware,
            };
        }

        let total: f64 = weights.values().sum();
        if total <= 0.0 {
            return ComponentType::Software;
        }

        let mut r = rand::random::<f64>() * total;
        for (comp_type, weight) in weights {
            r -= weight.max(0.0);
            if r <= 0.0 {
                return comp_type.clone();
            }
        }

        ComponentType::Software
    }

    /// Komponens típus neve
    fn type_name(comp_type: &ComponentType) -> String {
        match comp_type {
            ComponentType::Software => "Service".to_string(),
            ComponentType::Hardware => "Hardware".to_string(),
            ComponentType::Network => "Network".to_string(),
            ComponentType::Storage => "Storage".to_string(),
            ComponentType::Custom(s) => s.clone(),
        }
    }

    /// Generálási történet lekérése
    pub fn get_history(&self) -> Vec<ArchitectureProposal> {
        self.generation_history.read().unwrap().clone()
    }

    /// Történet törlése
    pub fn clear_history(&self) {
        self.generation_history.write().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_generator() -> ArchitectureGenerator {
        let kb = Arc::new(KnowledgeBase::new());
        let sim = Arc::new(ArchitectureSimulator::new());
        ArchitectureGenerator::new(kb, sim)
    }

    #[test]
    fn test_generate_hybrid() {
        let gen = create_test_generator();
        let proposals = gen.generate("scalable microservices");
        assert!(!proposals.is_empty());
        assert!(proposals[0].generation_score > 0.0);
    }

    #[test]
    fn test_generate_novel() {
        let gen = create_test_generator();
        gen.set_params(GenerationParams {
            strategy: GenerationStrategy::Novel,
            ..GenerationParams::default()
        });
        let proposals = gen.generate("test requirements");
        assert!(!proposals.is_empty());
    }

    #[test]
    fn test_generate_evolutionary() {
        let gen = create_test_generator();
        gen.set_params(GenerationParams {
            strategy: GenerationStrategy::Evolutionary,
            generations: 3,
            min_components: 3,
            max_components: 5,
            mutation_rate: 1.0,
            target_latency_ms: 1000.0,
            target_throughput: 1.0,
            ..GenerationParams::default()
        });
        let proposals = gen.generate("test");
        assert!(!proposals.is_empty());
    }
}
