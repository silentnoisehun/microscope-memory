//! Architecture Simulator — valós idejű architektúra szimuláció és stressztesztelés.
//!
//! Ez a modul képes a memóriában tárolt architektúrák (szoftver, hardver, hálózati)
//! valós idejű szimulációjára, stressztesztelésére és teljesítményértékelésére.
//! A szimulációs eredmények visszakerülnek a kognitív memóriába, hogy a rendszer
//! tanulhasson belőlük.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

// ─── Alap típusok ───────────────────────────────────────────────────────────

/// Egy architektúra komponens típusa
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComponentType {
    /// Szoftver komponens (mikroszolgáltatás, modul, library)
    Software,
    /// Hardver komponens (CPU, GPU, memória, hálózati eszköz)
    Hardware,
    /// Hálózati protokoll vagy kapcsolat
    Network,
    /// Adattároló (adatbázis, cache, fájlrendszer)
    Storage,
    /// Egyéb
    Custom(String),
}

/// Egy architektúra komponens
#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub component_type: ComponentType,
    /// Kapacitás paraméterek (pl. "cpu_cores": 8, "memory_mb": 16384)
    pub capacity: HashMap<String, f64>,
    /// Aktuális terhelés (0.0 - 1.0)
    pub load: f64,
    /// Hibaarány (0.0 - 1.0)
    pub error_rate: f64,
    /// Késleltetés ms-ben
    pub latency_ms: f64,
}

/// Kapcsolat két komponens között
#[derive(Debug, Clone)]
pub struct Connection {
    pub from: String,
    pub to: String,
    /// Sávszélesség (Mbps)
    pub bandwidth: f64,
    /// Protokoll típusa
    pub protocol: String,
    /// Aktuális késleltetés
    pub latency_ms: f64,
    /// Csomagvesztési arány
    pub packet_loss: f64,
}

/// Egy teljes architektúra modell
#[derive(Debug, Clone)]
pub struct Architecture {
    pub id: String,
    pub name: String,
    pub description: String,
    pub components: HashMap<String, Component>,
    pub connections: Vec<Connection>,
    /// Verzió / generáció
    pub version: u32,
    /// Belső kohéziós pontszám (0.0 - 1.0)
    pub cohesion_score: f64,
}

/// Szimulációs konfiguráció
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Szimuláció időtartama másodpercben
    pub duration_secs: f64,
    /// Időlépés mérete ms-ben
    pub time_step_ms: f64,
    /// Maximális egyidejű kérések száma
    pub max_concurrent_requests: u32,
    /// Terhelési minta (pl. "linear", "spike", "sine", "random")
    pub load_pattern: String,
    /// Csúcs terhelés (0.0 - 1.0)
    pub peak_load: f64,
    /// Hibainjektálás engedélyezése
    pub enable_fault_injection: bool,
    /// Hibaarány (0.0 - 1.0)
    pub fault_rate: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            duration_secs: 60.0,
            time_step_ms: 100.0,
            max_concurrent_requests: 100,
            load_pattern: "linear".to_string(),
            peak_load: 0.8,
            enable_fault_injection: false,
            fault_rate: 0.01,
        }
    }
}

/// Szimulációs metrikák
#[derive(Debug, Clone)]
pub struct SimulationMetrics {
    pub architecture_id: String,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub throughput_req_per_sec: f64,
    pub error_rate: f64,
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub network_utilization: f64,
    pub bottleneck_components: Vec<String>,
    pub stability_score: f64,
    pub resilience_score: f64,
    pub total_requests: u64,
    pub failed_requests: u64,
    pub simulation_time_secs: f64,
}

/// Stresszteszt eredmény
#[derive(Debug, Clone)]
pub struct StressTestResult {
    pub architecture_id: String,
    pub breaking_point_load: f64,
    pub recovery_time_ms: f64,
    pub graceful_degradation: bool,
    pub cascade_failures: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Szimulációs esemény (naplózáshoz)
#[derive(Debug, Clone)]
pub struct SimulationEvent {
    pub time_ms: f64,
    pub event_type: String,
    pub component_id: String,
    pub description: String,
    pub severity: u8, // 0=info, 1=warning, 2=error, 3=critical
}

// ─── Architektúra Szimulátor ─────────────────────────────────────────────────

/// A fő szimulátor motor
pub struct ArchitectureSimulator {
    architectures: Arc<RwLock<HashMap<String, Architecture>>>,
    simulation_results: Arc<RwLock<HashMap<String, Vec<SimulationMetrics>>>>,
    stress_test_results: Arc<RwLock<HashMap<String, Vec<StressTestResult>>>>,
    events: Arc<RwLock<Vec<SimulationEvent>>>,
    /// Tanult minták a korábbi szimulációkból
    learned_patterns: Arc<RwLock<HashMap<String, f64>>>,
}

impl ArchitectureSimulator {
    /// Létrehoz egy új szimulátort
    pub fn new() -> Self {
        Self {
            architectures: Arc::new(RwLock::new(HashMap::new())),
            simulation_results: Arc::new(RwLock::new(HashMap::new())),
            stress_test_results: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            learned_patterns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Regisztrál egy architektúrát a szimulátorban
    pub fn register_architecture(&self, arch: Architecture) {
        let mut architectures = self.architectures.write().unwrap();
        architectures.insert(arch.id.clone(), arch);
    }

    /// Lekér egy architektúrát ID alapján
    pub fn get_architecture(&self, id: &str) -> Option<Architecture> {
        let architectures = self.architectures.read().unwrap();
        architectures.get(id).cloned()
    }

    /// Listázza az összes regisztrált architektúrát
    pub fn list_architectures(&self) -> Vec<Architecture> {
        let architectures = self.architectures.read().unwrap();
        architectures.values().cloned().collect()
    }

    /// Futtat egy szimulációt egy architektúrán
    pub fn run_simulation(&self, arch_id: &str, config: &SimulationConfig) -> Option<SimulationMetrics> {
        let arch = {
            let architectures = self.architectures.read().unwrap();
            architectures.get(arch_id).cloned()?
        };

        let mut events = self.events.write().unwrap();
        let _start_time = Instant::now();
        let mut metrics = SimulationMetrics {
            architecture_id: arch_id.to_string(),
            avg_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            throughput_req_per_sec: 0.0,
            error_rate: 0.0,
            cpu_utilization: 0.0,
            memory_utilization: 0.0,
            network_utilization: 0.0,
            bottleneck_components: Vec::new(),
            stability_score: 1.0,
            resilience_score: 1.0,
            total_requests: 0,
            failed_requests: 0,
            simulation_time_secs: config.duration_secs,
        };

        let steps = (config.duration_secs * 1000.0 / config.time_step_ms) as u64;
        let mut latencies: Vec<f64> = Vec::new();
        let mut current_load = 0.0_f64;
        let mut component_loads: HashMap<String, Vec<f64>> = HashMap::new();

        // Inicializáljuk a komponens terheléseket
        for comp_id in arch.components.keys() {
            component_loads.insert(comp_id.clone(), Vec::new());
        }

        for step in 0..steps {
            let current_time = step as f64 * config.time_step_ms;

            // Terhelési minta generálása
            current_load = match config.load_pattern.as_str() {
                "linear" => (current_time / (config.duration_secs * 1000.0)) * config.peak_load,
                "spike" => {
                    if current_time > config.duration_secs * 500.0 && current_time < config.duration_secs * 600.0 {
                        config.peak_load
                    } else {
                        config.peak_load * 0.3
                    }
                }
                "sine" => {
                    let phase = (current_time / (config.duration_secs * 1000.0)) * std::f64::consts::PI * 4.0;
                    (phase.sin() * 0.5 + 0.5) * config.peak_load
                }
                "random" => {
                    rand::random::<f64>() * config.peak_load
                }
                _ => config.peak_load * 0.5,
            };

            // Kérések generálása
            let requests_this_step = (current_load * config.max_concurrent_requests as f64) as u64;
            metrics.total_requests += requests_this_step;

            // Komponensek terhelésének szimulálása
            for (comp_id, comp) in &arch.components {
                let base_latency = comp.latency_ms;
                let load_factor = current_load * (1.0 + comp.error_rate);
                let effective_latency = base_latency * (1.0 + load_factor * 2.0);
                
                latencies.push(effective_latency);
                component_loads.get_mut(comp_id).unwrap().push(load_factor);

                // Hibainjektálás
                if config.enable_fault_injection && rand::random::<f64>() < config.fault_rate {
                    metrics.failed_requests += 1;
                    events.push(SimulationEvent {
                        time_ms: current_time,
                        event_type: "fault".to_string(),
                        component_id: comp_id.clone(),
                        description: format!("Fault injected in {} at load {:.2}", comp.name, current_load),
                        severity: 2,
                    });
                }

                // Kapcsolatok szimulálása
                for conn in &arch.connections {
                    if conn.from == *comp_id || conn.to == *comp_id {
                        let network_delay = conn.latency_ms * (1.0 + current_load);
                        latencies.push(network_delay);

                        if rand::random::<f64>() < conn.packet_loss {
                            metrics.failed_requests += 1;
                        }
                    }
                }
            }

            // Stabilitás számítása
            if current_load > 0.9 {
                metrics.stability_score *= 0.99;
            }
            if metrics.failed_requests as f64 / metrics.total_requests.max(1) as f64 > 0.1 {
                metrics.resilience_score *= 0.95;
            }
        }

        // Metrikák számítása
        if !latencies.is_empty() {
            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = latencies.len();
            metrics.avg_latency_ms = latencies.iter().sum::<f64>() / len as f64;
            metrics.p95_latency_ms = latencies[(len as f64 * 0.95) as usize];
            metrics.p99_latency_ms = latencies[(len as f64 * 0.99) as usize];
        }

        metrics.throughput_req_per_sec = metrics.total_requests as f64 / config.duration_secs;
        metrics.error_rate = metrics.failed_requests as f64 / metrics.total_requests.max(1) as f64;

        // CPU, memória, hálózat kihasználtság
        let avg_loads: Vec<f64> = component_loads.values()
            .map(|loads| loads.iter().sum::<f64>() / loads.len() as f64)
            .collect();
        
        if avg_loads.len() >= 3 {
            metrics.cpu_utilization = avg_loads[0];
            metrics.memory_utilization = avg_loads[1 % avg_loads.len()];
            metrics.network_utilization = avg_loads[2 % avg_loads.len()];
        }

        // Szűk keresztmetszetek azonosítása
        for (comp_id, loads) in &component_loads {
            let avg_load = loads.iter().sum::<f64>() / loads.len() as f64;
            if avg_load > 0.8 {
                metrics.bottleneck_components.push(comp_id.clone());
            }
        }

        // Eredmények tárolása
        let mut results = self.simulation_results.write().unwrap();
        results.entry(arch_id.to_string())
            .or_insert_with(Vec::new)
            .push(metrics.clone());

        // Tanulás a szimulációból
        self.learn_from_simulation(&metrics);

        Some(metrics)
    }

    /// Stresszteszt: fokozatosan növeli a terhelést a töréspont megtalálásáig
    pub fn run_stress_test(&self, arch_id: &str) -> Option<StressTestResult> {
        let _arch = {
            let architectures = self.architectures.read().unwrap();
            architectures.get(arch_id).cloned()?
        };

        let mut result = StressTestResult {
            architecture_id: arch_id.to_string(),
            breaking_point_load: 0.0,
            recovery_time_ms: 0.0,
            graceful_degradation: true,
            cascade_failures: Vec::new(),
            recommendations: Vec::new(),
        };

        let mut load = 0.1;
        let mut previous_error_rate = 0.0;
        let mut cascade_started = false;

        while load <= 1.0 {
            let config = SimulationConfig {
                duration_secs: 10.0,
                time_step_ms: 50.0,
                max_concurrent_requests: 1000,
                load_pattern: "linear".to_string(),
                peak_load: load,
                enable_fault_injection: true,
                fault_rate: 0.001,
            };

            if let Some(metrics) = self.run_simulation(arch_id, &config) {
                // Töréspont detektálása
                if metrics.error_rate > 0.5 && result.breaking_point_load == 0.0 {
                    result.breaking_point_load = load;
                    
                    // Kaszkád hibák detektálása
                    if metrics.error_rate > previous_error_rate * 2.0 {
                        cascade_started = true;
                        result.cascade_failures.push(format!(
                            "Cascade failure at load {:.2}: error rate jumped from {:.2} to {:.2}",
                            load, previous_error_rate, metrics.error_rate
                        ));
                    }
                }

                // Graceful degradation ellenőrzése
                if metrics.error_rate > 0.8 {
                    result.graceful_degradation = false;
                }

                previous_error_rate = metrics.error_rate;
            }

            load += 0.1;
        }

        // Ha nem találtunk töréspontot, a maximális terhelésnél sincs probléma
        if result.breaking_point_load == 0.0 {
            result.breaking_point_load = 1.0;
            result.recommendations.push("Architecture is stable up to maximum load".to_string());
        } else {
            result.recommendations.push(format!(
                "Breaking point at {:.0}% load — consider scaling or optimization",
                result.breaking_point_load * 100.0
            ));
        }

        if !result.graceful_degradation {
            result.recommendations.push(
                "Implement circuit breaker pattern for graceful degradation".to_string()
            );
        }

        if cascade_started {
            result.recommendations.push(
                "Add bulkhead isolation to prevent cascade failures".to_string()
            );
        }

        // Eredmények tárolása
        let mut stress_results = self.stress_test_results.write().unwrap();
        stress_results.entry(arch_id.to_string())
            .or_insert_with(Vec::new)
            .push(result.clone());

        Some(result)
    }

    /// Összehasonlít két architektúrát
    pub fn compare_architectures(&self, arch_id_a: &str, arch_id_b: &str) -> Option<ComparisonResult> {
        let results = self.simulation_results.read().unwrap();
        let results_a = results.get(arch_id_a)?;
        let results_b = results.get(arch_id_b)?;

        let latest_a = results_a.last()?;
        let latest_b = results_b.last()?;

        Some(ComparisonResult {
            architecture_a: arch_id_a.to_string(),
            architecture_b: arch_id_b.to_string(),
            latency_winner: if latest_a.avg_latency_ms < latest_b.avg_latency_ms { arch_id_a.to_string() } else { arch_id_b.to_string() },
            throughput_winner: if latest_a.throughput_req_per_sec > latest_b.throughput_req_per_sec { arch_id_a.to_string() } else { arch_id_b.to_string() },
            stability_winner: if latest_a.stability_score > latest_b.stability_score { arch_id_a.to_string() } else { arch_id_b.to_string() },
            resilience_winner: if latest_a.resilience_score > latest_b.resilience_score { arch_id_a.to_string() } else { arch_id_b.to_string() },
            recommendations: self.generate_recommendations(latest_a, latest_b),
        })
    }

    /// Tanulás a szimulációs eredményekből
    fn learn_from_simulation(&self, metrics: &SimulationMetrics) {
        let mut patterns = self.learned_patterns.write().unwrap();
        
        // Magas hibaarány -> negatív minta
        if metrics.error_rate > 0.1 {
            let key = format!("high_error_rate_{:.2}", metrics.error_rate);
            *patterns.entry(key).or_insert(0.0) -= 0.1;
        }

        // Alacsony késleltetés -> pozitív minta
        if metrics.avg_latency_ms < 10.0 {
            let key = format!("low_latency_{:.2}", metrics.avg_latency_ms);
            *patterns.entry(key).or_insert(0.0) += 0.1;
        }

        // Szűk keresztmetszetek -> figyelmeztető minta
        for bottleneck in &metrics.bottleneck_components {
            let key = format!("bottleneck_{}", bottleneck);
            *patterns.entry(key).or_insert(0.0) -= 0.05;
        }
    }

    /// Generál ajánlásokat két architektúra összehasonlításából
    fn generate_recommendations(&self, a: &SimulationMetrics, b: &SimulationMetrics) -> Vec<String> {
        let mut recs = Vec::new();

        if a.avg_latency_ms < b.avg_latency_ms * 0.8 {
            recs.push(format!(
                "Architecture A has {:.0}% better latency — consider adopting its component layout",
                (1.0 - a.avg_latency_ms / b.avg_latency_ms) * 100.0
            ));
        }

        if a.throughput_req_per_sec > b.throughput_req_per_sec * 1.2 {
            recs.push(format!(
                "Architecture A handles {:.0}% more throughput — analyze its parallelization strategy",
                (a.throughput_req_per_sec / b.throughput_req_per_sec - 1.0) * 100.0
            ));
        }

        if a.stability_score > b.stability_score {
            recs.push("Architecture A is more stable under load".to_string());
        }

        recs
    }

    /// Lekérdezi a tanult mintákat
    pub fn get_learned_patterns(&self) -> HashMap<String, f64> {
        let patterns = self.learned_patterns.read().unwrap();
        patterns.clone()
    }

    /// Lekérdezi a szimulációs eseményeket
    pub fn get_events(&self) -> Vec<SimulationEvent> {
        let events = self.events.read().unwrap();
        events.clone()
    }

    /// Törli az összes szimulációs eredményt
    pub fn clear_results(&self) {
        self.simulation_results.write().unwrap().clear();
        self.stress_test_results.write().unwrap().clear();
        self.events.write().unwrap().clear();
    }
}

/// Összehasonlítás eredménye
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub architecture_a: String,
    pub architecture_b: String,
    pub latency_winner: String,
    pub throughput_winner: String,
    pub stability_winner: String,
    pub resilience_winner: String,
    pub recommendations: Vec<String>,
}

// ─── Segédfüggvények ────────────────────────────────────────────────────────

/// Létrehoz egy egyszerű architektúra modellt a megadott paraméterekből
pub fn create_architecture(
    name: &str,
    description: &str,
    components: Vec<(&str, ComponentType, f64, f64)>, // (name, type, latency_ms, error_rate)
    connections: Vec<(&str, &str, f64, &str)>, // (from, to, bandwidth, protocol)
) -> Architecture {
    let id = format!("arch_{}", rand::random::<u32>());
    let mut comp_map = HashMap::new();

    for (i, (name, comp_type, latency, error_rate)) in components.iter().enumerate() {
        let comp_id = format!("comp_{}", i);
        comp_map.insert(comp_id.clone(), Component {
            id: comp_id,
            name: name.to_string(),
            component_type: comp_type.clone(),
            capacity: HashMap::new(),
            load: 0.0,
            error_rate: *error_rate,
            latency_ms: *latency,
        });
    }

    let conns: Vec<Connection> = connections.iter().enumerate().map(|(i, (_from, _to, bw, proto))| {
        Connection {
            from: format!("comp_{}", i),
            to: format!("comp_{}", i + 1),
            bandwidth: *bw,
            protocol: proto.to_string(),
            latency_ms: 1.0,
            packet_loss: 0.001,
        }
    }).collect();

    Architecture {
        id,
        name: name.to_string(),
        description: description.to_string(),
        components: comp_map,
        connections: conns,
        version: 1,
        cohesion_score: 0.7,
    }
}

/// Párhuzamos szimuláció futtatása több architektúrán
pub fn run_parallel_simulations(
    simulator: &ArchitectureSimulator,
    arch_ids: &[&str],
    config: &SimulationConfig,
) -> Vec<(String, Option<SimulationMetrics>)> {
    arch_ids.iter()
        .map(|id| {
            let result = simulator.run_simulation(id, config);
            (id.to_string(), result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_simulate() {
        let simulator = ArchitectureSimulator::new();
        
        let arch = create_architecture(
            "Test Microservice",
            "A simple test architecture",
            vec![
                ("API Gateway", ComponentType::Software, 5.0, 0.01),
                ("Auth Service", ComponentType::Software, 10.0, 0.02),
                ("Database", ComponentType::Storage, 20.0, 0.005),
            ],
            vec![
                ("API Gateway", "Auth Service", 1000.0, "HTTP/2"),
                ("Auth Service", "Database", 5000.0, "TCP"),
            ],
        );

        simulator.register_architecture(arch);
        
        let config = SimulationConfig::default();
        let result = simulator.run_simulation("arch_0", &config);
        
        assert!(result.is_some());
        let metrics = result.unwrap();
        assert!(metrics.total_requests > 0);
        assert!(metrics.avg_latency_ms > 0.0);
    }

    #[test]
    fn test_stress_test() {
        let simulator = ArchitectureSimulator::new();
        
        let arch = create_architecture(
            "Stress Test Target",
            "Architecture for stress testing",
            vec![
                ("Load Balancer", ComponentType::Software, 2.0, 0.001),
                ("Web Server", ComponentType::Software, 15.0, 0.01),
                ("Cache", ComponentType::Storage, 1.0, 0.001),
            ],
            vec![
                ("Load Balancer", "Web Server", 10000.0, "HTTP/1.1"),
                ("Web Server", "Cache", 20000.0, "Redis"),
            ],
        );

        simulator.register_architecture(arch);
        let result = simulator.run_stress_test("arch_0");
        
        assert!(result.is_some());
        let stress = result.unwrap();
        assert!(stress.breaking_point_load > 0.0);
    }
}
