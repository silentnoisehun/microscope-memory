//! Advanced Cognition — Predictive Processing, Intuition, Unconscious Problem-Solving

use std::collections::HashMap;

// ═══ PREDICTIVE PROCESSING ═══════════════════════════════════════════
#[derive(Clone, Debug)]
pub struct Prediction {
    pub pattern: Vec<u32>,
    pub confidence: f32,
    pub error: f32, // prediction error
    pub timestamp_ms: u64,
}

pub struct PredictiveProcessor {
    pub predictions: Vec<Prediction>,
    pub error_history: Vec<f32>,
    pub model_accuracy: f32, // 0.0-1.0
}

impl PredictiveProcessor {
    pub fn new() -> Self {
        Self {
            predictions: Vec::new(),
            error_history: Vec::new(),
            model_accuracy: 0.5,
        }
    }

    /// Generate prediction from pattern
    pub fn predict(&mut self, pattern: &[u32]) -> (Vec<u32>, f32) {
        let confidence = self.model_accuracy;
        let mut predicted = pattern.to_vec();

        // Extend pattern with predicted next elements
        if pattern.len() > 0 {
            let last = pattern[pattern.len() - 1];
            predicted.push(last.wrapping_add(1));
            predicted.push(last.wrapping_add(2));
        }

        self.predictions.push(Prediction {
            pattern: predicted.clone(),
            confidence,
            error: 0.0,
            timestamp_ms: Self::now_ms(),
        });

        (predicted, confidence)
    }

    /// Update prediction error
    pub fn update_error(&mut self, actual: &[u32]) {
        if let Some(pred) = self.predictions.last_mut() {
            let error = Self::calculate_error(&pred.pattern, actual);
            pred.error = error;
            self.error_history.push(error);

            // Update model accuracy
            if self.error_history.len() > 20 {
                self.error_history.remove(0);
            }
            let avg_error: f32 =
                self.error_history.iter().sum::<f32>() / self.error_history.len() as f32;
            self.model_accuracy = (1.0 - avg_error).max(0.1);
        }
    }

    fn calculate_error(predicted: &[u32], actual: &[u32]) -> f32 {
        let len = predicted.len().min(actual.len());
        if len == 0 {
            return 1.0;
        }

        let mut diff = 0u32;
        for i in 0..len {
            diff = diff.wrapping_add((predicted[i] ^ actual[i]) as u32);
        }
        ((diff as f32) / (len as f32 * 256.0)).min(1.0)
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// ═══ INTUITION ════════════════════════════════════════════════════════
#[derive(Clone, Debug)]
pub struct IntuitionPattern {
    pub pattern_hash: u64,
    pub confidence: f32,
    pub decision: String,
    pub success_rate: f32,
    pub frequency: u32,
}

pub struct IntuitionSystem {
    pub patterns: HashMap<u64, IntuitionPattern>,
    pub fast_decisions: Vec<(String, f32)>, // (decision, speed_ms)
}

impl IntuitionSystem {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            fast_decisions: Vec::new(),
        }
    }

    /// Learn pattern for future intuitive decisions
    pub fn learn_pattern(&mut self, pattern: &[u32], decision: &str, successful: bool) {
        let hash = Self::pattern_hash(pattern);

        let pattern_entry = self
            .patterns
            .entry(hash)
            .or_insert_with(|| IntuitionPattern {
                pattern_hash: hash,
                confidence: 0.5,
                decision: decision.to_string(),
                success_rate: 0.0,
                frequency: 0,
            });

        pattern_entry.frequency += 1;
        if successful {
            pattern_entry.success_rate = (pattern_entry.success_rate * 0.8 + 1.0 * 0.2).min(1.0);
            pattern_entry.confidence = (pattern_entry.confidence + 0.1).min(1.0);
        } else {
            pattern_entry.success_rate = (pattern_entry.success_rate * 0.9).max(0.0);
            pattern_entry.confidence = (pattern_entry.confidence - 0.05).max(0.0);
        }
    }

    /// Make intuitive decision (fast, pattern-based)
    pub fn intuitive_decision(&mut self, pattern: &[u32]) -> Option<(String, f32)> {
        let hash = Self::pattern_hash(pattern);

        if let Some(entry) = self.patterns.get(&hash) {
            if entry.confidence > 0.6 {
                // High confidence threshold
                self.fast_decisions.push((entry.decision.clone(), 5.0)); // 5ms decision time
                return Some((entry.decision.clone(), entry.confidence));
            }
        }
        None
    }

    fn pattern_hash(pattern: &[u32]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &val in pattern.iter().take(10) {
            hash = hash.wrapping_mul(0x100000001b3) ^ (val as u64);
        }
        hash
    }
}

// ═══ UNCONSCIOUS PROBLEM-SOLVING ════════════════════════════════════
#[derive(Clone, Debug)]
pub struct IncubatedProblem {
    pub problem_id: u64,
    pub problem_desc: String,
    pub incubation_start_ms: u64,
    pub background_processing_count: u32,
    pub solution_found: bool,
    pub solution: Option<String>,
    pub insight_strength: f32,
}

pub struct UnconsciousSolver {
    pub incubating: Vec<IncubatedProblem>,
    pub solutions: HashMap<u64, String>,
    pub incubation_time_ms: u64, // how long to let problems simmer
}

impl UnconsciousSolver {
    pub fn new() -> Self {
        Self {
            incubating: Vec::new(),
            solutions: HashMap::new(),
            incubation_time_ms: 5000, // 5 seconds default
        }
    }

    /// Start incubating a problem (send to background)
    pub fn incubate(&mut self, problem_desc: &str) -> u64 {
        let problem_id = Self::problem_hash(problem_desc);

        self.incubating.push(IncubatedProblem {
            problem_id,
            problem_desc: problem_desc.to_string(),
            incubation_start_ms: Self::now_ms(),
            background_processing_count: 0,
            solution_found: false,
            solution: None,
            insight_strength: 0.0,
        });

        problem_id
    }

    /// Background processing (happens passively)
    pub fn background_process(&mut self) {
        let now = Self::now_ms();

        for problem in &mut self.incubating {
            if !problem.solution_found {
                problem.background_processing_count += 1;

                let incubation_time = now.saturating_sub(problem.incubation_start_ms);

                // Solution emerges after incubation period
                if incubation_time > self.incubation_time_ms
                    && problem.background_processing_count > 3
                {
                    // Generate insight (simple heuristic)
                    problem.solution = Some(format!(
                        "Insight: Approach {} differently",
                        problem.background_processing_count
                    ));
                    problem.solution_found = true;
                    problem.insight_strength =
                        0.5 + (problem.background_processing_count as f32 * 0.05).min(0.4);
                }
            }
        }
    }

    /// Retrieve solution from background processing
    pub fn retrieve_solution(&mut self, problem_id: u64) -> Option<(String, f32)> {
        if let Some(pos) = self
            .incubating
            .iter()
            .position(|p| p.problem_id == problem_id)
        {
            let problem = self.incubating.remove(pos);

            if problem.solution_found {
                if let Some(solution) = problem.solution {
                    self.solutions.insert(problem_id, solution.clone());
                    return Some((solution, problem.insight_strength));
                }
            }
        }
        None
    }

    /// Check if solution is ready
    pub fn is_ready(&self, problem_id: u64) -> bool {
        self.incubating
            .iter()
            .find(|p| p.problem_id == problem_id)
            .map(|p| p.solution_found)
            .unwrap_or(false)
    }

    fn problem_hash(desc: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in desc.as_bytes().iter().take(32) {
            hash = hash.wrapping_mul(0x100000001b3) ^ (b as u64);
        }
        hash
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// ═══ INTEGRATED SYSTEM ═════════════════════════════════════════════════
pub struct AdvancedCognition {
    pub predictor: PredictiveProcessor,
    pub intuition: IntuitionSystem,
    pub solver: UnconsciousSolver,
}

impl AdvancedCognition {
    pub fn new() -> Self {
        Self {
            predictor: PredictiveProcessor::new(),
            intuition: IntuitionSystem::new(),
            solver: UnconsciousSolver::new(),
        }
    }

    /// Process in integrated way
    pub fn process_cycle(&mut self) {
        self.solver.background_process();

        // Let intuition contribute to predictions
        self.predictor.model_accuracy = (self.predictor.model_accuracy * 0.9
            + self.intuition.patterns.len() as f32 * 0.001)
            .min(1.0);
    }
}
