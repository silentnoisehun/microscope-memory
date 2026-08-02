//! Planning — céllebontás és hierarchikus tervezés

//!

//! Képes:

//! - Célok lebontása részcélokra (HTN planning)

//! - Akciótervek készítése erőforrás becsléssel

//! - Terv végrehajtás monitorozás és replanning

//! - Kockázat becslés és alternatív tervek

//!

//! Kapcsolódások:

//! - executive.rs → terv végrehajtás

//! - heuristic_decision.rs → döntéshozatal

//! - mental_sandbox.rs → előzetes szimuláció

//! - agus.rs → stressz alapú terv módosítás

//! - pattern_recognition.rs → minta illesztés

use std::collections::HashMap;

use std::sync::{Arc, Mutex, RwLock};

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]

pub enum GoalStatus {
    Active,
    InProgress,
    Completed,
    Failed(String),
    Blocked(String),
    Suspended,
}

#[derive(Debug, Clone, PartialEq)]

pub enum PlanStatus {
    Draft,
    InProgress,
    Completed,
    Failed(String),
    /// A step was hard-blocked by the commitment gate.
    Blocked(String),
    Replanning,
}

#[derive(Debug, Clone)]

pub struct Goal {
    pub id: u64,
    pub name: String,
    pub description: String,

    pub priority: u8,
    pub deadline_ms: Option<u64>,

    pub status: GoalStatus,
    pub parent_id: Option<u64>,

    pub subgoals: Vec<u64>,
    pub progress: f64,
    pub created_ms: u64,
}

#[derive(Debug, Clone)]

pub struct Action {
    pub id: u64,
    pub name: String,
    pub module: String,

    pub estimated_cost: f64,
    pub estimated_duration_ms: u64,

    pub dependencies: Vec<u64>,
    pub preconditions: Vec<String>,

    pub effects: Vec<String>,
    pub risk_score: f64,
}

#[derive(Debug, Clone)]

pub struct Plan {
    pub id: u64,
    pub name: String,
    pub goal_id: u64,

    pub actions: Vec<Action>,
    pub status: PlanStatus,

    pub total_cost: f64,
    pub total_duration_ms: u64,

    pub risk_score: f64,
    pub alternatives: Vec<u64>,

    pub created_ms: u64,
    pub executed_step: usize,
}

#[derive(Debug, Clone)]

pub struct PlanningConfig {
    pub max_depth: u32,
    pub min_confidence: f64,

    pub max_alternatives: usize,
    pub auto_replan: bool,

    pub risk_tolerance: f64,
}

impl Default for PlanningConfig {
    fn default() -> Self {
        Self {
            max_depth: 5,
            min_confidence: 0.3,
            max_alternatives: 3,
            auto_replan: true,
            risk_tolerance: 0.7,
        }
    }
}

pub struct Planner {
    goals: Arc<RwLock<HashMap<u64, Goal>>>,

    plans: Arc<RwLock<HashMap<u64, Plan>>>,

    config: Arc<RwLock<PlanningConfig>>,

    next_id: Arc<RwLock<u64>>,

    /// Mandatory commitment gate. Every candidate action selected by
    /// `execute_step` must pass through this before it can run.
    enforcement: Arc<Mutex<crate::enforcement::EnforcementEngine>>,
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

impl Planner {
    pub fn new() -> Self {
        Self {
            goals: Arc::new(RwLock::new(HashMap::new())),

            plans: Arc::new(RwLock::new(HashMap::new())),

            config: Arc::new(RwLock::new(PlanningConfig::default())),

            next_id: Arc::new(RwLock::new(1)),

            enforcement: Arc::new(Mutex::new(crate::enforcement::EnforcementEngine::new())),
        }
    }

    /// Install a shared enforcement engine so commitments added via the CLI
    /// (or tests) are the exact same commitments the gate enforces.
    pub fn set_enforcement(
        &mut self,
        enforcement: Arc<Mutex<crate::enforcement::EnforcementEngine>>,
    ) {
        self.enforcement = enforcement;
    }

    /// Access the shared enforcement gate (e.g. to seed commitments).
    pub fn enforcement(&self) -> Arc<Mutex<crate::enforcement::EnforcementEngine>> {
        Arc::clone(&self.enforcement)
    }

    fn nid(&self) -> u64 {
        let mut id = crate::sync_guard::write_lock(&self.next_id);
        let n = *id;
        *id += 1;
        n
    }

    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn add_goal(&self, name: &str, desc: &str, priority: u8, parent: Option<u64>) -> u64 {
        let id = self.nid();

        let goal = Goal {
            id,
            name: name.to_string(),
            description: desc.to_string(),

            priority,
            deadline_ms: None,
            status: GoalStatus::Active,

            parent_id: parent,
            subgoals: Vec::new(),
            progress: 0.0,
            created_ms: self.now(),
        };

        if let Some(pid) = parent {
            if let Some(p) = crate::sync_guard::write_lock(&self.goals).get_mut(&pid) {
                p.subgoals.push(id);
            }
        }

        crate::sync_guard::write_lock(&self.goals).insert(id, goal);

        id
    }

    pub fn get_goal(&self, id: u64) -> Option<Goal> {
        crate::sync_guard::read_lock(&self.goals).get(&id).cloned()
    }

    pub fn list_goals(&self, status: Option<GoalStatus>) -> Vec<Goal> {
        crate::sync_guard::read_lock(&self.goals)
            .values()
            .filter(|g| status.as_ref().is_none_or(|s| g.status == *s))
            .cloned()
            .collect()
    }

    /// Cél lebontása részcélokra (HTN dekompozíció)
    pub fn decompose_goal(&self, goal_id: u64) -> Vec<u64> {
        let goal = match crate::sync_guard::read_lock(&self.goals).get(&goal_id) {
            Some(g) => g.clone(),
            None => return vec![],
        };

        let _config = crate::sync_guard::read_lock(&self.config).clone();

        let mut sub_ids = Vec::new();

        if goal.priority > 200 {
            return sub_ids;
        } // atomic goal

        // Domen-specifikus dekompozíció

        let subgoals: Vec<(String, String, u8)> = match goal.name.to_lowercase().as_str() {
            "learn" | "tanulás" => vec![
                (
                    "collect_data".to_string(),
                    "Adatok gyűjtése".to_string(),
                    goal.priority + 10,
                ),
                (
                    "analyze".to_string(),
                    "Elemzés és minták keresése".to_string(),
                    goal.priority + 20,
                ),
                (
                    "store".to_string(),
                    "Eredmények tárolása".to_string(),
                    goal.priority + 30,
                ),
                (
                    "verify".to_string(),
                    "Ellenőrzés és validálás".to_string(),
                    goal.priority + 40,
                ),
            ],

            "optimize" | "optimalizálás" => vec![
                (
                    "measure".to_string(),
                    "Jelenlegi állapot mérése".to_string(),
                    goal.priority + 10,
                ),
                (
                    "identify".to_string(),
                    "Szűk keresztmetszetek azonosítása".to_string(),
                    goal.priority + 20,
                ),
                (
                    "apply".to_string(),
                    "Optimalizáció alkalmazása".to_string(),
                    goal.priority + 30,
                ),
                (
                    "verify".to_string(),
                    "Javulás ellenőrzése".to_string(),
                    goal.priority + 40,
                ),
            ],

            "explore" | "exploráció" => vec![
                (
                    "scan".to_string(),
                    "Felület szkenelése".to_string(),
                    goal.priority + 10,
                ),
                (
                    "sample".to_string(),
                    "Mintavételezés".to_string(),
                    goal.priority + 20,
                ),
                (
                    "map".to_string(),
                    "Térkép készítése".to_string(),
                    goal.priority + 30,
                ),
                (
                    "report".to_string(),
                    "Eredmények jelentése".to_string(),
                    goal.priority + 40,
                ),
            ],

            _ => vec![
                (
                    "analyze".to_string(),
                    format!("{} elemzése", goal.name),
                    goal.priority + 10,
                ),
                (
                    "plan".to_string(),
                    format!("Terv készítése: {}", goal.name),
                    goal.priority + 20,
                ),
                (
                    "execute".to_string(),
                    format!("Végrehajtás: {}", goal.name),
                    goal.priority + 30,
                ),
                (
                    "verify".to_string(),
                    format!("Eredmény ellenőrzése: {}", goal.name),
                    goal.priority + 40,
                ),
            ],
        };

        for (name, desc, prio) in subgoals {
            let sid = self.add_goal(&name, &desc, prio, Some(goal_id));

            sub_ids.push(sid);
        }

        sub_ids
    }

    /// Terv készítése egy célhoz
    pub fn create_plan(&self, goal_id: u64) -> Plan {
        let goal = match crate::sync_guard::read_lock(&self.goals).get(&goal_id) {
            Some(g) => g.clone(),
            None => return self.empty_plan(),
        };

        let _config = crate::sync_guard::read_lock(&self.config).clone();

        let now = self.now();

        let plan_id = self.nid();

        let actions = match goal.name.to_lowercase().as_str() {
            "collect_data" => self.make_collect_actions(),

            "analyze" => self.make_analyze_actions(),

            "execute" => self.make_execute_actions(&goal),

            _ => self.make_generic_actions(&goal),
        };

        let total_cost: f64 = actions.iter().map(|a| a.estimated_cost).sum();

        let total_dur: u64 = actions.iter().map(|a| a.estimated_duration_ms).sum();

        let risk: f64 =
            actions.iter().map(|a| a.risk_score).sum::<f64>() / actions.len().max(1) as f64;

        let plan = Plan {
            id: plan_id,
            name: format!("plan_for_{}", goal.name),
            goal_id,

            actions,
            status: PlanStatus::Draft,
            total_cost,
            total_duration_ms: total_dur,

            risk_score: risk,
            alternatives: Vec::new(),
            created_ms: now,
            executed_step: 0,
        };

        if let Some(g) = crate::sync_guard::write_lock(&self.goals).get_mut(&goal_id) {
            g.status = GoalStatus::InProgress;
        }

        crate::sync_guard::write_lock(&self.plans).insert(plan_id, plan.clone());

        plan
    }

    /// Select the next candidate action and submit it to the mandatory
    /// commitment gate. A non-permitted action is **blocked** (returns an
    /// error) instead of merely warned; only actions in `A_t^valid` advance.
    fn gated_step(
        &self,
        plans: &mut std::sync::RwLockWriteGuard<'_, std::collections::HashMap<u64, Plan>>,
        plan_id: u64,
        justification: Option<&str>,
    ) -> Result<Option<Action>, crate::enforcement::EnforcementError> {
        let plan = match plans.get_mut(&plan_id) {
            Some(p) => p,
            None => return Ok(None),
        };

        if plan.executed_step >= plan.actions.len() {
            plan.status = PlanStatus::Completed;
            if let Some(g) = crate::sync_guard::write_lock(&self.goals).get_mut(&plan.goal_id) {
                g.status = GoalStatus::Completed;
                g.progress = 1.0;
            }
            return Ok(None);
        }

        let action = plan.actions[plan.executed_step].clone();
        let event = crate::enforcement::ActionEvent {
            actor: "planner".to_string(),
            action: action.name.clone(),
            content: plan.name.clone(),
            ts_ms: self.now(),
            scope: format!("plan:{}:goal:{}", plan.id, plan.goal_id),
            provenance: "planner/execute_step".to_string(),
        };

        let mut gate = crate::sync_guard::mutex_lock(&self.enforcement);
        match gate.decide(&event, justification) {
            crate::enforcement::Decision::Allowed { .. }
            | crate::enforcement::Decision::Overridden { .. } => {
                // A member of A_t^valid; the decision was already appended to
                // the audit chain inside decide().
                plan.executed_step += 1;
                plan.status = PlanStatus::InProgress;
                Ok(Some(action))
            }
            crate::enforcement::Decision::Blocked { action, reason, .. } => {
                plan.status = PlanStatus::Blocked(reason.clone());
                Err(crate::enforcement::EnforcementError::Blocked { action, reason })
            }
            crate::enforcement::Decision::AttributionError { reason } => {
                plan.status = PlanStatus::Failed(reason.clone());
                Err(crate::enforcement::EnforcementError::FaultyAttribution { reason })
            }
        }
    }

    /// Terv végrehajtásának előre léptetése a kötelező gate-en keresztül.
    pub fn execute_step(
        &self,
        plan_id: u64,
    ) -> Result<Option<Action>, crate::enforcement::EnforcementError> {
        let mut plans = crate::sync_guard::write_lock(&self.plans);
        self.gated_step(&mut plans, plan_id, None)
    }

    /// A documented `justifiedOverride()` útvonal: ugyanaz a gate, de az
    /// akció egy rögzített indoklással léphet (ha jogosult a felülírás).
    pub fn execute_step_with_override(
        &self,
        plan_id: u64,
        justification: &str,
    ) -> Result<Option<Action>, crate::enforcement::EnforcementError> {
        let mut plans = crate::sync_guard::write_lock(&self.plans);
        self.gated_step(&mut plans, plan_id, Some(justification))
    }

    /// Rollback: visszalépés az utolsó végrehajtott lépésről, ha az sikertelen volt.
    pub fn fail_step(&self, plan_id: u64, reason: &str) -> bool {
        let mut plans = crate::sync_guard::write_lock(&self.plans);

        let plan = match plans.get_mut(&plan_id) {
            Some(p) => p,

            None => return false,
        };

        if plan.executed_step == 0 {
            return false;
        }

        plan.executed_step -= 1;

        plan.status = PlanStatus::Failed(format!("step {} failed: {}", plan.executed_step, reason));

        true
    }

    /// Replanning: terv újragenerálás változott körülmények esetén
    pub fn replan(&self, plan_id: u64, reason: &str) -> Option<Plan> {
        let plans = crate::sync_guard::write_lock(&self.plans);

        let old = plans.get(&plan_id)?.clone();

        let _goal = crate::sync_guard::read_lock(&self.goals)
            .get(&old.goal_id)?
            .clone();

        drop(plans);

        // Goal visszaállítása

        if let Some(g) = crate::sync_guard::write_lock(&self.goals).get_mut(&old.goal_id) {
            g.status = GoalStatus::Active;
        }

        let mut new_plan = self.create_plan(old.goal_id);

        new_plan.name = format!("replan_{}_{}", old.name, reason);

        new_plan.status = PlanStatus::Replanning;

        new_plan.alternatives.push(old.id);

        Some(new_plan)
    }

    pub fn plans_for_goal(&self, goal_id: u64) -> Vec<Plan> {
        crate::sync_guard::read_lock(&self.plans)
            .values()
            .filter(|p| p.goal_id == goal_id)
            .cloned()
            .collect()
    }

    pub fn get_plan(&self, id: u64) -> Option<Plan> {
        crate::sync_guard::read_lock(&self.plans).get(&id).cloned()
    }

    pub fn stats(&self) -> (usize, usize, usize) {
        let g = crate::sync_guard::read_lock(&self.goals);

        let p = crate::sync_guard::read_lock(&self.plans);

        (
            g.len(),
            p.len(),
            g.values()
                .filter(|g| g.status == GoalStatus::Completed)
                .count(),
        )
    }

    // ─── Segéd akció készítők ────────────────────────────────────────────

    fn empty_plan(&self) -> Plan {
        Plan {
            id: 0,
            name: "empty".to_string(),
            goal_id: 0,
            actions: vec![],

            status: PlanStatus::Draft,
            total_cost: 0.0,
            total_duration_ms: 0,

            risk_score: 0.0,
            alternatives: vec![],
            created_ms: self.now(),
            executed_step: 0,
        }
    }

    fn make_collect_actions(&self) -> Vec<Action> {
        let id = self.nid();

        vec![
            Action {
                id: id + 1,
                name: "query_source".to_string(),
                module: "reader".to_string(),

                estimated_cost: 0.3,
                estimated_duration_ms: 100,
                dependencies: vec![],

                preconditions: vec!["config_ready".to_string()],
                effects: vec!["data_collected".to_string()],
                risk_score: 0.1,
            },
            Action {
                id: id + 2,
                name: "filter_relevant".to_string(),
                module: "attention".to_string(),

                estimated_cost: 0.2,
                estimated_duration_ms: 50,
                dependencies: vec![id + 1],

                preconditions: vec!["data_collected".to_string()],
                effects: vec!["data_filtered".to_string()],
                risk_score: 0.1,
            },
            Action {
                id: id + 3,
                name: "store_results".to_string(),
                module: "hippocampus".to_string(),

                estimated_cost: 0.1,
                estimated_duration_ms: 30,
                dependencies: vec![id + 2],

                preconditions: vec!["data_filtered".to_string()],
                effects: vec!["data_stored".to_string()],
                risk_score: 0.05,
            },
        ]
    }

    fn make_analyze_actions(&self) -> Vec<Action> {
        let id = self.nid();

        vec![
            Action {
                id: id + 1,
                name: "load_data".to_string(),
                module: "reader".to_string(),

                estimated_cost: 0.1,
                estimated_duration_ms: 20,
                dependencies: vec![],

                preconditions: vec!["data_available".to_string()],
                effects: vec!["data_loaded".to_string()],
                risk_score: 0.05,
            },
            Action {
                id: id + 2,
                name: "find_patterns".to_string(),
                module: "pattern_recognition".to_string(),

                estimated_cost: 0.5,
                estimated_duration_ms: 200,
                dependencies: vec![id + 1],

                preconditions: vec!["data_loaded".to_string()],
                effects: vec!["patterns_found".to_string()],
                risk_score: 0.2,
            },
            Action {
                id: id + 3,
                name: "store_insights".to_string(),
                module: "knowledge_base".to_string(),

                estimated_cost: 0.1,
                estimated_duration_ms: 30,
                dependencies: vec![id + 2],

                preconditions: vec!["patterns_found".to_string()],
                effects: vec!["insights_stored".to_string()],
                risk_score: 0.05,
            },
        ]
    }

    fn make_execute_actions(&self, goal: &Goal) -> Vec<Action> {
        let id = self.nid();

        vec![
            Action {
                id: id + 1,
                name: format!("prepare_{}", goal.name),
                module: "executive".to_string(),

                estimated_cost: 0.2,
                estimated_duration_ms: 50,
                dependencies: vec![],

                preconditions: vec![],
                effects: vec!["ready".to_string()],
                risk_score: 0.1,
            },
            Action {
                id: id + 2,
                name: format!("run_{}", goal.name),
                module: "executive".to_string(),

                estimated_cost: 0.8,
                estimated_duration_ms: 500,
                dependencies: vec![id + 1],

                preconditions: vec!["ready".to_string()],
                effects: vec!["executed".to_string()],
                risk_score: 0.3,
            },
            Action {
                id: id + 3,
                name: "verify_result".to_string(),
                module: "meta_supervision".to_string(),

                estimated_cost: 0.1,
                estimated_duration_ms: 30,
                dependencies: vec![id + 2],

                preconditions: vec!["executed".to_string()],
                effects: vec!["verified".to_string()],
                risk_score: 0.1,
            },
        ]
    }

    fn make_generic_actions(&self, goal: &Goal) -> Vec<Action> {
        let id = self.nid();

        vec![
            Action {
                id: id + 1,
                name: format!("init_{}", goal.name),
                module: "executive".to_string(),

                estimated_cost: 0.2,
                estimated_duration_ms: 50,
                dependencies: vec![],

                preconditions: vec![],
                effects: vec!["initialized".to_string()],
                risk_score: 0.1,
            },
            Action {
                id: id + 2,
                name: format!("process_{}", goal.name),
                module: "executive".to_string(),

                estimated_cost: 0.5,
                estimated_duration_ms: 300,
                dependencies: vec![id + 1],

                preconditions: vec!["initialized".to_string()],
                effects: vec!["processed".to_string()],
                risk_score: 0.2,
            },
            Action {
                id: id + 3,
                name: "finalize".to_string(),
                module: "executive".to_string(),

                estimated_cost: 0.1,
                estimated_duration_ms: 30,
                dependencies: vec![id + 2],

                preconditions: vec!["processed".to_string()],
                effects: vec!["done".to_string()],
                risk_score: 0.05,
            },
        ]
    }
}
