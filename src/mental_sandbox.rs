//! Mental sandbox for pre-action scenario simulation.
//! Provides a safe environment to test different action paths before committing.

use std::sync::{Arc, RwLock};
use std::collections::HashMap;

/// Represents a simulated scenario with its outcome
#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: String,
    pub description: String,
    pub actions: Vec<String>,
    pub outcome_probability: f32,
    pub risk_score: f32,
    pub reward_potential: f32,
}

/// Maximum scenarios kept in memory before automatic purge of oldest.
const MAX_SCENARIOS: usize = 100;

/// Mental sandbox that simulates multiple scenarios
pub struct MentalSandbox {
    scenarios: Arc<RwLock<HashMap<String, Scenario>>>,
    current_simulation: Arc<RwLock<Option<String>>>,
    long_term_goals: Vec<String>,
}

impl MentalSandbox {
    /// Create a new mental sandbox
    pub fn new() -> Self {
        Self {
            scenarios: Arc::new(RwLock::new(HashMap::new())),
            current_simulation: Arc::new(RwLock::new(None)),
            long_term_goals: Vec::new(),
        }
    }

    /// Add a long-term goal for scenario evaluation
    pub fn add_goal(&mut self, goal: &str) {
        self.long_term_goals.push(goal.to_string());
    }

    /// Simulate a scenario before real action
    pub fn simulate_scenario(&self, description: &str, actions: Vec<&str>) -> Scenario {
        let id = format!("scenario_{}", rand::random::<u32>());
        
        // Simple heuristic scoring based on goal alignment
        let goal_alignment = self.calculate_goal_alignment(description);
        let complexity_factor = actions.len() as f32 * 0.1;
        
        let scenario = Scenario {
            id: id.clone(),
            description: description.to_string(),
            actions: actions.iter().map(|s| s.to_string()).collect(),
            outcome_probability: 0.7, // base probability
            risk_score: (1.0 - goal_alignment) * complexity_factor,
            reward_potential: goal_alignment * (1.0 - complexity_factor.min(1.0)),
        };

        let mut scenarios = self.scenarios.write().unwrap();
        scenarios.insert(id.clone(), scenario.clone());

        // Automatic purge: if over MAX_SCENARIOS, remove oldest entries
        if scenarios.len() > MAX_SCENARIOS {
            let excess = scenarios.len() - MAX_SCENARIOS;
            let keys: Vec<String> = scenarios.keys().take(excess).cloned().collect();
            for k in keys {
                scenarios.remove(&k);
            }
        }

        let mut current = self.current_simulation.write().unwrap();
        *current = Some(id);

        scenario
    }

    /// Calculate how well a scenario aligns with long-term goals
    fn calculate_goal_alignment(&self, description: &str) -> f32 {
        if self.long_term_goals.is_empty() {
            return 0.5; // neutral alignment
        }

        let keywords = description.to_lowercase();
        let mut matches = 0.0;
        
        for goal in &self.long_term_goals {
            let goal_lower = goal.to_lowercase();
            if keywords.contains(&goal_lower) {
                matches += 1.0;
            }
        }

        matches / self.long_term_goals.len() as f32
    }

    /// Get the best scenario based on risk/reward ratio
    pub fn get_best_scenario(&self) -> Option<Scenario> {
        let scenarios = self.scenarios.read().unwrap();
        
        scenarios.values()
            .max_by(|a, b| {
                let a_score = a.reward_potential / (a.risk_score + 0.01);
                let b_score = b.reward_potential / (b.risk_score + 0.01);
                a_score.partial_cmp(&b_score).unwrap()
            })
            .cloned()
    }

    /// Clear all simulated scenarios
    pub fn clear(&mut self) {
        self.scenarios.write().unwrap().clear();
        *self.current_simulation.write().unwrap() = None;
    }
}

/// Run multiple scenario simulations in parallel
pub fn run_parallel_simulations(sandbox: &MentalSandbox, scenario_descriptions: &[(&str, Vec<&str>)]) -> Vec<Scenario> {
    scenario_descriptions.iter()
        .map(|(desc, actions)| sandbox.simulate_scenario(desc, actions.clone()))
        .collect()
}