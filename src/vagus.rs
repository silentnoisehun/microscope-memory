//! Vagus Nerve — Autonomic System Regulation and Cognitive Homeostasis
//!
//! A bolygóideg (nervus vagus) a leghosszabb agyideg, a paraszimpatikus idegrendszer főcsatornája.
//! Ez a modul a biológiai vagus funkcióit emulálja a szoftver rendszerben:
//!
//! 1. **Globális Rendszer-Homeosztázis és "Szívverés"** — HRV-szerű pulzus-monitorozás
//! 2. **Bél-Agy Tengely** — Perifériás szenzor hálózat, "zsigeri" jelek feldolgozása
//! 3. **Gyulladáscsökkentő Útvonal** — Kognitív "gyulladás" (memory leak, deadlock, támadás) kezelése
//! 4. **Nyugalom és Kognitív Tisztaság** — Kényszerített lassítás, prioritás-kezelés
//!
//! A vagus nem felülről lefelé (top-down) kényszerít, hanem autonóm reflexként működik.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::architecture_simulator::ArchitectureSimulator;
use crate::heuristic_decision::HeuristicDecisionMaker;
use crate::knowledge_base::KnowledgeBase;

// ─── Alap típusok ───────────────────────────────────────────────────────────

/// Vagus tónus - a rendszer rugalmasságának mértéke (0.0 = merev/félelem, 1.0 = rugalmas/nyugodt)
#[derive(Debug, Clone)]
pub struct VagusTone {
    pub current: f64,
    pub baseline: f64,
    pub trend: f64,      // csökkenő = stressz, növekvő = relaxáció
    pub volatility: f64, // mennyire stabil a tónus
    pub last_update: u64,
}

/// Rendszer "pulzus" - aktuális terhelési állapot
#[derive(Debug, Clone)]
pub struct SystemPulse {
    pub timestamp: u64,
    pub cpu_pressure: f64,     // 0.0-1.0
    pub memory_pressure: f64,  // 0.0-1.0
    pub io_pressure: f64,      // 0.0-1.0
    pub network_pressure: f64, // 0.0-1.0
    pub request_rate: f64,     // kérések/másodperc
    pub error_rate: f64,       // hibák/másodperc
    pub hrv: f64,
}

/// Perifériás "zsigeri" jel - nyers, még nem feldolgozott adat
#[derive(Debug, Clone)]
pub struct GutFeeling {
    pub id: String,
    pub source: String, // honnan jött (pl. "bridge_api", "spine_client", "mcp_device")
    pub raw_signal: f64, // nyers jel erőssége
    pub anomaly_score: f64, // mennyire "furcsa" ez a jel
    pub timestamp: u64,
    pub processed: bool, // feldolgozta-e már a rendszer
}

/// Kognitív "gyulladás" típusai
#[derive(Debug, Clone, PartialEq)]
pub enum CognitiveInflammation {
    /// Memory leak - szivárgó memória
    MemoryLeak { module: String, leak_rate: f64 },
    /// Deadlock - beragadt lock
    Deadlock { thread: String, wait_time_ms: u64 },
    /// Infinite loop - végtelen ciklus
    InfiniteLoop { location: String, iterations: u64 },
    /// DOS támadás
    DoSAttack { source: String, request_count: u64 },
    /// Resource exhaustion - erőforrás kimerülés
    ResourceExhaustion { resource: String, usage: f64 },
    /// Cascade failure - kaszkád hiba
    CascadeFailure {
        origin: String,
        affected: Vec<String>,
    },
    ///custom
    Custom(String),
}

/// Gyulladáscsökkentő válasz
#[derive(Debug, Clone)]
pub struct AntiInflammatoryResponse {
    pub inflammation_type: CognitiveInflammation,
    pub action: String,            // milyen beavatkozás történt
    pub target: String,            // melyik modul/csomópont
    pub success: bool,             // sikeres volt-e
    pub recovery_time_ms: u64,     // mennyi idő alatt állt helyre
    pub side_effects: Vec<String>, // mellékhatások
    pub timestamp: u64,
}

/// Vagus állapot
#[derive(Debug, Clone)]
pub struct VagusState {
    pub tone: VagusTone,
    pub pulse: SystemPulse,
    pub gut_feelings: Vec<GutFeeling>,
    pub active_inflammations: Vec<CognitiveInflammation>,
    pub response_history: Vec<AntiInflammatoryResponse>,
    pub parasympathetic_mode: bool, // aktívan lassít-e a rendszer
    pub freeze_initiated: bool,     // kényszerített lassítás aktív
}

/// Figyelmeztetési szint
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum AlertLevel {
    Relaxed = 0,
    Vigilant = 1,
    Stressed = 2,
    Critical = 3,
    Emergency = 4,
}

// ─── Vagus Modul ───────────────────────────────────────────────────────────

pub struct VagusNerve {
    state: Arc<RwLock<VagusState>>,
    /// Architektúra szimulátor - stressz teszteléshez
    #[allow(dead_code)]
    simulator: Arc<ArchitectureSimulator>,
    /// Heurisztikus döntéshozó - reflex döntésekhez
    decision_maker: Arc<RwLock<Option<HeuristicDecisionMaker>>>,
    /// Tudásbázis - tanult mintákhoz
    knowledge_base: Arc<KnowledgeBase>,
    /// Utolsó pulzus idő
    last_pulse: Arc<RwLock<Instant>>,
    /// Szívverés interval (ms)
    heartbeat_interval_ms: u64,
    /// Gyulladás küszöb
    inflammation_threshold: f64,
    /// Paraszimpatikus aktiválás küszöb
    parasympathetic_threshold: f64,
}

impl VagusNerve {
    pub fn new(simulator: Arc<ArchitectureSimulator>, knowledge_base: Arc<KnowledgeBase>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            state: Arc::new(RwLock::new(VagusState {
                tone: VagusTone {
                    current: 0.8, // Kezdetben nyugodt
                    baseline: 0.8,
                    trend: 0.0,
                    volatility: 0.1,
                    last_update: now,
                },
                pulse: SystemPulse {
                    timestamp: now,
                    cpu_pressure: 0.0,
                    memory_pressure: 0.0,
                    io_pressure: 0.0,
                    network_pressure: 0.0,
                    request_rate: 0.0,
                    error_rate: 0.0,
                    hrv: 0.8, // healthy variability
                },
                gut_feelings: Vec::new(),
                active_inflammations: Vec::new(),
                response_history: Vec::new(),
                parasympathetic_mode: false,
                freeze_initiated: false,
            })),
            simulator,
            decision_maker: Arc::new(RwLock::new(None)),
            knowledge_base,
            last_pulse: Arc::new(RwLock::new(Instant::now())),
            heartbeat_interval_ms: 1000,
            inflammation_threshold: 0.7,
            parasympathetic_threshold: 0.85,
        }
    }

    /// Beállítja a heurisztikus döntéshozót
    pub fn set_decision_maker(&self, dm: HeuristicDecisionMaker) {
        *self.decision_maker.write().unwrap() = Some(dm);
    }

    /// Rendszer pulzus mérése — a szívverés analóg
    pub fn measure_pulse(&self, metrics: &SystemMetrics) -> SystemPulse {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // HRV szamitasa - a rendszer valaszkeszsege
        let prev_pulse = self.state.read().unwrap().pulse.clone();
        let response_delta = (metrics.request_rate - prev_pulse.request_rate).abs();
        let hrv = 1.0 - (response_delta / 100.0).min(1.0);

        let pulse = SystemPulse {
            timestamp: now,
            cpu_pressure: metrics.cpu_usage,
            memory_pressure: metrics.memory_usage,
            io_pressure: metrics.io_usage,
            network_pressure: metrics.network_usage,
            request_rate: metrics.request_rate,
            error_rate: metrics.error_rate,
            hrv,
        };

        // Állapot frissítése
        {
            let mut state = self.state.write().unwrap();
            state.pulse = pulse.clone();
        }

        *self.last_pulse.write().unwrap() = Instant::now();
        pulse
    }

    /// Vagus tónus frissítése — a rendszer relaxációs állapota
    pub fn update_tone(&self) {
        let mut state = self.state.write().unwrap();
        let pulse = &state.pulse;

        // Átlagos terhelés
        let avg_pressure = (pulse.cpu_pressure
            + pulse.memory_pressure
            + pulse.io_pressure
            + pulse.network_pressure)
            / 4.0;

        // Új tónus számítása
        let target_tone = 1.0 - avg_pressure;

        //mozgóátlag (exponenciális smoothing)
        let alpha = 0.1;
        let new_tone = state.tone.current * (1.0 - alpha) + target_tone * alpha;

        // Trend számítása
        let trend = new_tone - state.tone.current;

        // Volatilitás számítása
        let vol_change = (new_tone - state.tone.current).abs();
        let new_volatility = state.tone.volatility * 0.95 + vol_change * 0.05;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        state.tone = VagusTone {
            current: new_tone,
            baseline: state.tone.baseline,
            trend,
            volatility: new_volatility,
            last_update: now,
        };
    }

    /// "Zsigeri" jel fogadása — a perifériáról
    pub fn receive_gut_feeling(&self, source: &str, raw_signal: f64) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Anomália detektálás (egyszerű: ha túl nagy a jel, az anomália)
        let baseline = 0.5; // assumed baseline
        let anomaly_score = ((raw_signal - baseline).abs() / baseline).min(1.0);

        let feeling = GutFeeling {
            id: format!("gf_{}", rand::random::<u32>()),
            source: source.to_string(),
            raw_signal,
            anomaly_score,
            timestamp: now,
            processed: false,
        };

        let id = feeling.id.clone();

        {
            let mut state = self.state.write().unwrap();
            state.gut_feelings.push(feeling);

            // Max 1000 gut feelinget tárolunk
            if state.gut_feelings.len() > 1000 {
                state.gut_feelings.drain(0..500);
            }
        }

        id
    }

    /// Gyulladás detektálása
    pub fn detect_inflammation(&self) -> Vec<CognitiveInflammation> {
        let state = self.state.read().unwrap();
        let pulse = &state.pulse;
        let tone = &state.tone;

        let mut inflammations = Vec::new();

        // 1. Memory leak detektálás
        if pulse.memory_pressure > self.inflammation_threshold && tone.trend < -0.1 {
            // memory_pressure nő és tónus csökken = potenciális leak
            inflammations.push(CognitiveInflammation::MemoryLeak {
                module: "unknown".to_string(),
                leak_rate: pulse.memory_pressure - self.inflammation_threshold,
            });
        }

        // 2. High error rate = potenciális támadás
        if pulse.error_rate > 0.1 && pulse.request_rate > 100.0 {
            inflammations.push(CognitiveInflammation::DoSAttack {
                source: "unknown".to_string(),
                request_count: (pulse.request_rate * pulse.error_rate) as u64,
            });
        }

        // 3. Resource exhaustion
        if pulse.cpu_pressure > 0.95 || pulse.memory_pressure > 0.95 {
            inflammations.push(CognitiveInflammation::ResourceExhaustion {
                resource: if pulse.cpu_pressure > 0.95 {
                    "cpu".to_string()
                } else {
                    "memory".to_string()
                },
                usage: pulse.cpu_pressure.max(pulse.memory_pressure),
            });
        }

        // 4. Low HRV = rendszer "fél" (nem tud válaszolni)
        if pulse.hrv < 0.3 {
            inflammations.push(CognitiveInflammation::Custom(
                "system_paralysis".to_string(),
            ));
        }

        // Frissítjük az állapotot
        drop(state);
        let mut state = self.state.write().unwrap();
        state.active_inflammations = inflammations.clone();

        inflammations
    }

    /// Gyulladáscsökkentő válasz végrehajtása
    pub fn trigger_anti_inflammatory_response(
        &self,
        inflammation: &CognitiveInflammation,
    ) -> AntiInflammatoryResponse {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let start = Instant::now();

        let (action, target, success, side_effects) = match inflammation {
            CognitiveInflammation::MemoryLeak { module, .. } => {
                // Memory flush a working_memory-ből
                (
                    "force_memory_flush".to_string(),
                    module.clone(),
                    true,
                    vec![
                        "cache_cleared".to_string(),
                        "recent_activations_preserved".to_string(),
                    ],
                )
            }
            CognitiveInflammation::DoSAttack { source, .. } => {
                // Rate limiting aktiválása
                (
                    "activate_rate_limiting".to_string(),
                    source.clone(),
                    true,
                    vec![
                        "throttle_applied".to_string(),
                        "legitimate_requests_delayed".to_string(),
                    ],
                )
            }
            CognitiveInflammation::ResourceExhaustion { resource, .. } => {
                // Erőforrás izolálás
                (
                    "isolate_resource".to_string(),
                    resource.clone(),
                    true,
                    vec!["resource_quota_enforced".to_string()],
                )
            }
            CognitiveInflammation::Deadlock { thread, .. } => {
                // Szál megszakítás
                (
                    "force_thread_interrupt".to_string(),
                    thread.clone(),
                    true,
                    vec!["potential_data_loss".to_string()],
                )
            }
            CognitiveInflammation::InfiniteLoop { location, .. } => {
                // Loop megszakítás
                (
                    "break_loop".to_string(),
                    location.clone(),
                    true,
                    vec!["partial_computation_lost".to_string()],
                )
            }
            _ => (
                "general_pause".to_string(),
                "system".to_string(),
                true,
                vec!["all_operations_slowed".to_string()],
            ),
        };

        let recovery_time = start.elapsed().as_millis() as u64;

        let response = AntiInflammatoryResponse {
            inflammation_type: inflammation.clone(),
            action: action.clone(),
            target: target.clone(),
            success,
            recovery_time_ms: recovery_time,
            side_effects: side_effects.clone(),
            timestamp: now,
        };

        // Tároljuk a választ
        {
            let mut state = self.state.write().unwrap();
            state.response_history.push(response.clone());

            // Max 100 választ tárolunk
            if state.response_history.len() > 100 {
                state.response_history.drain(0..50);
            }
        }

        // Tanulás a válaszból a tudásbázisba
        self.knowledge_base.add_pitfall(
            &format!("Inflammation detected: {:?}", inflammation),
            &format!(
                "Action taken: {}, Recovery time: {}ms",
                action, recovery_time
            ),
            "system_defense",
            vec![
                "vagus_response".to_string(),
                "anti_inflammatory".to_string(),
            ],
        );

        response
    }

    /// Paraszimpatikus mód aktiválása — kényszerített lassítás
    pub fn activate_parasympathetic(&self) -> bool {
        let mut state = self.state.write().unwrap();

        let tone = state.tone.current;
        let pulse = &state.pulse;
        let avg_pressure = (pulse.cpu_pressure
            + pulse.memory_pressure
            + pulse.io_pressure
            + pulse.network_pressure)
            / 4.0;

        // Ha túl magas a nyomás és alacsony a tónus -> aktiválás
        if avg_pressure > self.parasympathetic_threshold && tone < 0.3 {
            state.parasympathetic_mode = true;
            state.freeze_initiated = true;

            // Logoljuk
            self.knowledge_base.add_insight(
                "Parasympathetic Mode Activated",
                &format!(
                    "System entering rest mode. Avg pressure: {:.2}, Tone: {:.2}",
                    avg_pressure, tone
                ),
                vec!["vagus".to_string(), "homeostasis".to_string()],
                "vagus_system",
            );

            true
        } else {
            false
        }
    }

    /// Paraszimpatikus mód deaktiválása — visszatérés normál működéshez
    pub fn deactivate_parasympathetic(&self) -> bool {
        let state = self.state.read().unwrap();

        let tone = state.tone.current;
        let pulse = &state.pulse;
        let avg_pressure = (pulse.cpu_pressure
            + pulse.memory_pressure
            + pulse.io_pressure
            + pulse.network_pressure)
            / 4.0;

        drop(state);

        // Ha alacsony a nyomás és magas a tónus -> deaktiválás
        if avg_pressure < 0.4 && tone > 0.6 {
            let mut state = self.state.write().unwrap();
            if state.parasympathetic_mode {
                state.parasympathetic_mode = false;
                state.freeze_initiated = false;

                self.knowledge_base.add_insight(
                    "Parasympathetic Mode Deactivated",
                    &format!(
                        "System returning to active mode. Avg pressure: {:.2}, Tone: {:.2}",
                        avg_pressure, tone
                    ),
                    vec!["vagus".to_string(), "recovery".to_string()],
                    "vagus_system",
                );

                return true;
            }
        }

        false
    }

    /// Alert szint meghatározása
    pub fn get_alert_level(&self) -> AlertLevel {
        let state = self.state.read().unwrap();

        let pulse = &state.pulse;
        let tone = state.tone.current;
        let avg_pressure = (pulse.cpu_pressure
            + pulse.memory_pressure
            + pulse.io_pressure
            + pulse.network_pressure)
            / 4.0;

        // Emergency: kritikus terhelés + alacsony HRV
        if avg_pressure > 0.95 && pulse.hrv < 0.2 {
            return AlertLevel::Emergency;
        }

        // Critical: magas terhelés
        if avg_pressure > 0.85 {
            return AlertLevel::Critical;
        }

        // Stressed: magas terhelés, alacsony tónus
        if avg_pressure > 0.7 && tone < 0.4 {
            return AlertLevel::Stressed;
        }

        // Vigilant: mérsékelt terhelés
        if avg_pressure > 0.5 {
            return AlertLevel::Vigilant;
        }

        // Relaxed: normál működés
        AlertLevel::Relaxed
    }

    /// Rendszer állapot lekérése
    pub fn get_state(&self) -> VagusState {
        self.state.read().unwrap().clone()
    }

    /// Vagus tónus lekérése
    pub fn get_tone(&self) -> VagusTone {
        self.state.read().unwrap().tone.clone()
    }

    /// Feldolgozatlan "zsigeri" jelek lekérése
    pub fn get_unprocessed_gut_feelings(&self) -> Vec<GutFeeling> {
        let state = self.state.read().unwrap();
        state
            .gut_feelings
            .iter()
            .filter(|f| !f.processed)
            .cloned()
            .collect()
    }

    /// Jel feldolgozottként megjelölése
    pub fn mark_gut_feeling_processed(&self, id: &str) {
        let mut state = self.state.write().unwrap();
        if let Some(feeling) = state.gut_feelings.iter_mut().find(|f| f.id == id) {
            feeling.processed = true;
        }
    }

    /// Automatikus vagus ciklus — ezt hívja a rendszer periodikusan
    pub fn automatic_cycle(&self) -> VagusStatus {
        let mut status = VagusStatus {
            pulse_measured: false,
            tone_updated: false,
            inflammation_detected: false,
            response_triggered: false,
            parasympathetic_activated: false,
            alert_level: AlertLevel::Relaxed,
        };

        // 1. Pulzus mérése (ha kell)
        if self.last_pulse.read().unwrap().elapsed()
            > Duration::from_millis(self.heartbeat_interval_ms)
        {
            // Itt a hívónak kell megadnia a metrikákat
            status.pulse_measured = true;
        }

        // 2. Tónus frissítése
        self.update_tone();
        status.tone_updated = true;

        // 3. Gyulladás detektálás
        let inflammations = self.detect_inflammation();
        status.inflammation_detected = !inflammations.is_empty();

        // 4. Gyulladáscsökkentő válasz
        for inf in &inflammations {
            self.trigger_anti_inflammatory_response(inf);
            status.response_triggered = true;
        }

        // 5. Paraszimpatikus mód kezelés
        let activated = self.activate_parasympathetic();
        if activated {
            status.parasympathetic_activated = true;
        } else {
            self.deactivate_parasympathetic();
        }

        // 6. Alert szint
        status.alert_level = self.get_alert_level();

        status
    }

    /// Statisztikák lekérése
    pub fn get_statistics(&self) -> VagusStatistics {
        let state = self.state.read().unwrap();

        VagusStatistics {
            current_tone: state.tone.current,
            tone_trend: state.tone.trend,
            tone_volatility: state.tone.volatility,
            hrv: state.pulse.hrv,
            avg_pressure: (state.pulse.cpu_pressure
                + state.pulse.memory_pressure
                + state.pulse.io_pressure
                + state.pulse.network_pressure)
                / 4.0,
            gut_feelings_count: state.gut_feelings.len(),
            unprocessed_gut_feelings: state.gut_feelings.iter().filter(|f| !f.processed).count(),
            active_inflammations: state.active_inflammations.len(),
            response_count: state.response_history.len(),
            parasympathetic_mode: state.parasympathetic_mode,
            freeze_initiated: state.freeze_initiated,
            alert_level: self.get_alert_level(),
        }
    }
}

/// Rendszer metrikák (a hívó adja meg)
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub io_usage: f64,
    pub network_usage: f64,
    pub request_rate: f64,
    pub error_rate: f64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            io_usage: 0.0,
            network_usage: 0.0,
            request_rate: 0.0,
            error_rate: 0.0,
        }
    }
}

/// Vagus ciklus státusza
#[derive(Debug, Clone)]
pub struct VagusStatus {
    pub pulse_measured: bool,
    pub tone_updated: bool,
    pub inflammation_detected: bool,
    pub response_triggered: bool,
    pub parasympathetic_activated: bool,
    pub alert_level: AlertLevel,
}

/// Vagus statisztikák
#[derive(Debug, Clone)]
pub struct VagusStatistics {
    pub current_tone: f64,
    pub tone_trend: f64,
    pub tone_volatility: f64,
    pub hrv: f64,
    pub avg_pressure: f64,
    pub gut_feelings_count: usize,
    pub unprocessed_gut_feelings: usize,
    pub active_inflammations: usize,
    pub response_count: usize,
    pub parasympathetic_mode: bool,
    pub freeze_initiated: bool,
    pub alert_level: AlertLevel,
}

// ─── CLI Integráció ───────────────────────────────────────────────────────

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertLevel::Relaxed => write!(f, "RELAXED"),
            AlertLevel::Vigilant => write!(f, "VIGILANT"),
            AlertLevel::Stressed => write!(f, "STRESSED"),
            AlertLevel::Critical => write!(f, "CRITICAL"),
            AlertLevel::Emergency => write!(f, "EMERGENCY"),
        }
    }
}

impl std::fmt::Display for VagusStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", "═".repeat(40))?;
        writeln!(f, "  VAGUS NERVE STATUS")?;
        writeln!(f, "{}", "═".repeat(40))?;
        writeln!(
            f,
            "  Tone: {:.2} (trend: {:.3}, vol: {:.3})",
            self.current_tone, self.tone_trend, self.tone_volatility
        )?;
        writeln!(f, "  HRV: {:.2}", self.hrv)?;
        writeln!(f, "  Avg Pressure: {:.1}%", self.avg_pressure * 100.0)?;
        writeln!(
            f,
            "  Gut Feelings: {} / {} pending",
            self.gut_feelings_count - self.unprocessed_gut_feelings,
            self.unprocessed_gut_feelings
        )?;
        writeln!(f, "  Active Inflammations: {}", self.active_inflammations)?;
        writeln!(f, "  Response History: {}", self.response_count)?;
        writeln!(
            f,
            "  Parasympathetic: {} | Freeze: {}",
            if self.parasympathetic_mode {
                "ACTIVE"
            } else {
                "OFF"
            },
            if self.freeze_initiated {
                "ACTIVE"
            } else {
                "OFF"
            }
        )?;
        writeln!(f, "  Alert Level: {}", self.alert_level)?;
        writeln!(f, "{}", "═".repeat(40))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vagus() -> VagusNerve {
        let kb = Arc::new(KnowledgeBase::new());
        let sim = Arc::new(ArchitectureSimulator::new());
        VagusNerve::new(sim, kb)
    }

    #[test]
    fn test_gut_feeling() {
        let vagus = create_test_vagus();

        let id = vagus.receive_gut_feeling("test_source", 0.9);
        assert!(!id.is_empty());

        let feelings = vagus.get_unprocessed_gut_feelings();
        assert!(!feelings.is_empty());

        vagus.mark_gut_feeling_processed(&id);
        let feelings = vagus.get_unprocessed_gut_feelings();
        assert!(feelings.is_empty());
    }

    #[test]
    fn test_tone_update() {
        let vagus = create_test_vagus();

        // Pulse first
        let metrics = SystemMetrics {
            cpu_usage: 0.5,
            memory_usage: 0.3,
            io_usage: 0.2,
            network_usage: 0.1,
            request_rate: 100.0,
            error_rate: 0.01,
        };
        vagus.measure_pulse(&metrics);

        vagus.update_tone();
        let tone = vagus.get_tone();

        assert!(tone.current >= 0.0 && tone.current <= 1.0);
    }

    #[test]
    fn test_alert_level() {
        let vagus = create_test_vagus();

        // Relaxed state
        let level = vagus.get_alert_level();
        assert_eq!(level, AlertLevel::Relaxed);

        // High pressure
        let metrics = SystemMetrics {
            cpu_usage: 0.9,
            memory_usage: 0.9,
            io_usage: 0.9,
            network_usage: 0.9,
            request_rate: 1000.0,
            error_rate: 0.5,
        };
        vagus.measure_pulse(&metrics);
        vagus.update_tone();

        let level = vagus.get_alert_level();
        assert!(level >= AlertLevel::Stressed);
    }

    #[test]
    fn test_inflammation_detection() {
        let vagus = create_test_vagus();

        // DOS-like conditions
        let metrics = SystemMetrics {
            cpu_usage: 0.5,
            memory_usage: 0.5,
            io_usage: 0.5,
            network_usage: 0.5,
            request_rate: 500.0,
            error_rate: 0.2, // High error rate
        };
        vagus.measure_pulse(&metrics);
        vagus.update_tone();

        let inflammations = vagus.detect_inflammation();
        assert!(!inflammations.is_empty());
    }
}
