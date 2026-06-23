//! Executive — kognitív végrehajtó rendszer
//!
//! Kapcsolódások: vagus, attention, meta_supervision, planning, impulse_control, hyperfocus

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleState {
    Idle,
    Running,
    Blocked(String),
    Error(String),
    Exhausted,
}
#[derive(Debug, Clone)]
pub struct CognitiveModule {
    pub id: String,
    pub name: String,
    pub priority: u8,
    pub energy_cost: f64,
    pub state: ModuleState,
    pub last_run_ms: u64,
    pub run_count: u64,
    pub avg_duration_ms: f64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CognitiveResources {
    pub attention_budget: f64,
    pub energy_level: f64,
    pub time_budget_ms: u64,
    pub working_memory_load: f64,
    pub context_switches: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub module_order: Vec<String>,
    pub total_energy: f64,
    pub estimated_duration_ms: u64,
    pub priority: u8,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionLogEntry {
    pub timestamp_ms: u64,
    pub module_id: String,
    pub action: String,
    pub duration_ms: u64,
    pub energy_used: f64,
    pub success: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct ExecutiveConfig {
    pub max_modules_per_cycle: usize,
    pub min_energy_for_execution: f64,
    pub energy_regen_rate: f64,
    pub critical_energy_threshold: f64,
    pub attention_decay_rate: f64,
    pub log_retention: usize,
}

impl Default for ExecutiveConfig {
    fn default() -> Self {
        Self {
            max_modules_per_cycle: 5,
            min_energy_for_execution: 0.1,
            energy_regen_rate: 0.02,
            critical_energy_threshold: 0.15,
            attention_decay_rate: 0.05,
            log_retention: 500,
        }
    }
}
pub struct Executive {
    modules: Arc<RwLock<HashMap<String, CognitiveModule>>>,
    resources: Arc<RwLock<CognitiveResources>>,
    log: Arc<RwLock<VecDeque<ExecutionLogEntry>>>,
    config: Arc<RwLock<ExecutiveConfig>>,
    current_plan: Arc<RwLock<Option<ExecutionPlan>>>,
    module_queue: Arc<RwLock<VecDeque<String>>>,
}

impl Executive {
    pub fn new() -> Self {
        Self {
            modules: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(CognitiveResources {
                attention_budget: 1.0,
                energy_level: 1.0,
                time_budget_ms: 1000,
                working_memory_load: 0.0,
                context_switches: 0,
            })),
            log: Arc::new(RwLock::new(VecDeque::with_capacity(500))),
            config: Arc::new(RwLock::new(ExecutiveConfig::default())),
            current_plan: Arc::new(RwLock::new(None)),
            module_queue: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn register_module(
        &self,
        id: &str,
        name: &str,
        priority: u8,
        energy_cost: f64,
        tags: Vec<String>,
    ) {
        self.modules.write().unwrap().insert(
            id.to_string(),
            CognitiveModule {
                id: id.to_string(),
                name: name.to_string(),
                priority,
                energy_cost,
                state: ModuleState::Idle,
                last_run_ms: 0,
                run_count: 0,
                avg_duration_ms: 0.0,
                tags,
            },
        );
    }

    pub fn module_state(&self, id: &str) -> Option<ModuleState> {
        self.modules
            .read()
            .unwrap()
            .get(id)
            .map(|m| m.state.clone())
    }

    pub fn set_module_state(&self, id: &str, state: ModuleState) -> bool {
        self.modules
            .write()
            .unwrap()
            .get_mut(id)
            .map(|m| {
                m.state = state;
                true
            })
            .unwrap_or(false)
    }

    pub fn list_modules(&self) -> Vec<CognitiveModule> {
        self.modules.read().unwrap().values().cloned().collect()
    }

    pub fn set_priority(&self, id: &str, priority: u8) -> bool {
        self.modules
            .write()
            .unwrap()
            .get_mut(id)
            .map(|m| {
                m.priority = priority;
                true
            })
            .unwrap_or(false)
    }

    pub fn resources(&self) -> CognitiveResources {
        self.resources.read().unwrap().clone()
    }

    pub fn update_resources(&self, f: impl Fn(&mut CognitiveResources)) {
        f(&mut *self.resources.write().unwrap());
    }

    pub fn schedule(&self) -> ExecutionPlan {
        let modules = self.modules.read().unwrap();
        let mut sorted: Vec<&CognitiveModule> = modules
            .values()
            .filter(|m| m.state == ModuleState::Idle || m.state == ModuleState::Running)
            .collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        let order: Vec<String> = sorted.iter().map(|m| m.id.clone()).collect();
        let energy: f64 = sorted.iter().map(|m| m.energy_cost).sum();
        let plan = ExecutionPlan {
            module_order: order.clone(),
            total_energy: energy,
            estimated_duration_ms: sorted.len() as u64 * 100,
            priority: sorted.first().map(|m| m.priority).unwrap_or(0),
            reason: format!("{} modules, {:.2} energy", sorted.len(), energy),
        };
        let mut queue = self.module_queue.write().unwrap();
        queue.clear();
        for id in &order {
            queue.push_back(id.clone());
        }
        *self.current_plan.write().unwrap() = Some(plan.clone());
        plan
    }

    pub fn execute_next(&self) -> Option<String> {
        let next = self.module_queue.write().unwrap().pop_front()?;
        {
            let mut res = self.resources.write().unwrap();
            if let Some(m) = self.modules.read().unwrap().get(&next) {
                res.energy_level = (res.energy_level - m.energy_cost * 0.01).max(0.0);
                res.context_switches += 1;
            }
        }
        let now = self.now_ms();
        self.log.write().unwrap().push_back(ExecutionLogEntry {
            timestamp_ms: now,
            module_id: next.clone(),
            action: "execute".to_string(),
            duration_ms: 100,
            energy_used: 0.01,
            success: true,
            detail: "Scheduled by executive".to_string(),
        });
        if let Some(m) = self.modules.write().unwrap().get_mut(&next) {
            m.last_run_ms = now;
            m.run_count += 1;
            m.state = ModuleState::Running;
        }
        Some(next)
    }
}
impl Executive {
    pub fn cycle(&self) -> Vec<String> {
        let config = self.config.read().unwrap().clone();
        let mut executed = Vec::new();
        self.schedule();
        if self.resources.read().unwrap().energy_level < config.min_energy_for_execution {
            return vec!["__low_energy__".to_string()];
        }
        for _ in 0..config.max_modules_per_cycle {
            if let Some(id) = self.execute_next() {
                executed.push(id);
            } else {
                break;
            }
        }
        self.update_resources(|r| {
            r.energy_level = (r.energy_level + config.energy_regen_rate).min(1.0);
            if r.energy_level > 0.9 {
                r.attention_budget = 1.0;
            }
        });
        executed
    }

    pub fn homeostasis(&self) -> Vec<String> {
        let mut actions = Vec::new();
        let res = self.resources.read().unwrap().clone();
        let config = self.config.read().unwrap().clone();
        if res.energy_level < config.critical_energy_threshold {
            let mut modules = self.modules.write().unwrap();
            for m in modules.values_mut() {
                if m.priority < 100 && m.state == ModuleState::Running {
                    m.state = ModuleState::Exhausted;
                    actions.push(format!("suspended_{}", m.id));
                }
            }
            actions.push("energy_conservation".to_string());
        }
        if res.context_switches > 1000 {
            self.update_resources(|r| r.context_switches = 0);
            actions.push("context_switch_throttle".to_string());
        }
        actions
    }

    pub fn stats(&self) -> (usize, usize, f64, f64) {
        let m = self.modules.read().unwrap();
        let r = self.resources.read().unwrap();
        (
            m.len(),
            m.values()
                .filter(|m| m.state == ModuleState::Running)
                .count(),
            r.energy_level,
            r.attention_budget,
        )
    }

    pub fn recent_log(&self, k: usize) -> Vec<ExecutionLogEntry> {
        self.log
            .read()
            .unwrap()
            .iter()
            .rev()
            .take(k)
            .cloned()
            .collect()
    }

    pub fn recommend_module(&self, vagus_tone: Option<f64>) -> Option<String> {
        let modules = self.modules.read().unwrap();
        let res = self.resources.read().unwrap();
        let mut candidates: Vec<&CognitiveModule> = modules
            .values()
            .filter(|m| m.state == ModuleState::Idle && m.energy_cost <= res.energy_level)
            .collect();
        if candidates.is_empty() {
            return None;
        }
        if let Some(tone) = vagus_tone {
            if tone < 0.4 {
                candidates.sort_by(|a, b| {
                    let a_stress = a.tags.contains(&"stress".to_string()) as u8;
                    let b_stress = b.tags.contains(&"stress".to_string()) as u8;
                    b_stress
                        .cmp(&a_stress)
                        .then_with(|| b.priority.cmp(&a.priority))
                });
            } else {
                candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
            }
        } else {
            candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
        }
        Some(candidates[0].id.clone())
    }
}
