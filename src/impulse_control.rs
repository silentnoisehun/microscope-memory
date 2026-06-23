//! Impulse control system for filtering incoming stimuli and suppressing irrelevant thoughts.
//! Implements a gating mechanism based on attention priority and long-term goals.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents an incoming stimulus that needs filtering
#[derive(Debug, Clone)]
pub struct Stimulus {
    pub id: String,
    pub content: String,
    pub source: String,
    pub urgency: f32,   // 0.0-1.0
    pub relevance: f32, // 0.0-1.0
    pub timestamp: u64,
    pub suppressed: bool,
}

/// Impulse control filter based on attention gates
pub struct ImpulseControl {
    #[allow(dead_code)]
    attention_gates: Arc<RwLock<Vec<String>>>,
    suppression_patterns: Arc<RwLock<HashSet<String>>>,
    long_term_goals: Vec<String>,
    attention_budget: f32, // 0.0-1.0, current available attention
    suppression_threshold: f32,
}

impl ImpulseControl {
    /// Create a new impulse control system
    pub fn new() -> Self {
        Self {
            attention_gates: Arc::new(RwLock::new(vec![
                "urgent".to_string(),
                "important".to_string(),
                "goal_aligned".to_string(),
            ])),
            suppression_patterns: Arc::new(RwLock::new(HashSet::new())),
            long_term_goals: Vec::new(),
            attention_budget: 0.8, // start with 80% attention available
            suppression_threshold: 0.3,
        }
    }

    /// Add a long-term goal for relevance filtering
    pub fn add_goal(&mut self, goal: &str) {
        self.long_term_goals.push(goal.to_string());
    }

    /// Add a suppression pattern (keywords to automatically suppress)
    pub fn add_suppression_pattern(&mut self, pattern: &str) {
        self.suppression_patterns
            .write()
            .unwrap()
            .insert(pattern.to_lowercase());
    }

    /// Filter a stimulus based on attention gates and goals
    pub fn filter_stimulus(&mut self, content: &str, source: &str, urgency: f32) -> Stimulus {
        let id = format!("stimulus_{}", rand::random::<u32>());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut suppressed = false;
        let mut relevance = self.calculate_relevance(content);

        // Check against suppression patterns
        let content_lower = content.to_lowercase();
        let patterns = self.suppression_patterns.read().unwrap();
        for pattern in patterns.iter() {
            if content_lower.contains(pattern) {
                suppressed = true;
                relevance = 0.0; // Completely irrelevant if suppressed
                break;
            }
        }

        // Apply attention budget constraint
        if !suppressed && self.attention_budget < 0.2 {
            // Low attention budget, suppress less relevant stimuli
            if relevance < self.suppression_threshold * 2.0 {
                suppressed = true;
            }
        }

        let stimulus = Stimulus {
            id,
            content: content.to_string(),
            source: source.to_string(),
            urgency,
            relevance,
            timestamp,
            suppressed,
        };

        // Update attention budget
        if !stimulus.suppressed && stimulus.relevance > 0.5 {
            self.attention_budget -= 0.1; // Consume attention
            self.attention_budget = self.attention_budget.max(0.0);
        }

        stimulus
    }

    /// Calculate relevance to long-term goals
    fn calculate_relevance(&self, content: &str) -> f32 {
        if self.long_term_goals.is_empty() {
            return 0.5; // neutral relevance
        }

        let content_lower = content.to_lowercase();
        let mut score = 0.0;

        for goal in &self.long_term_goals {
            let goal_lower = goal.to_lowercase();
            if content_lower.contains(&goal_lower) {
                score += 1.0;
            }
        }

        // Also check for goal-related keywords
        let goal_keywords = ["achieve", "progress", "improve", "develop", "complete"];
        for keyword in &goal_keywords {
            if content_lower.contains(keyword) {
                score += 0.5;
            }
        }

        (score / (self.long_term_goals.len() as f32 + goal_keywords.len() as f32)).min(1.0)
    }

    /// Restore attention budget (call periodically)
    pub fn restore_attention(&mut self, amount: f32) {
        self.attention_budget += amount;
        self.attention_budget = self.attention_budget.min(1.0);
    }

    /// Get statistics about filtering
    pub fn get_stats(&self) -> (f32, usize) {
        let patterns = self.suppression_patterns.read().unwrap();
        (self.attention_budget, patterns.len())
    }

    /// Clear all suppression patterns
    pub fn clear_patterns(&mut self) {
        self.suppression_patterns.write().unwrap().clear();
    }
}

/// Batch filter multiple stimuli
pub fn batch_filter_stimuli(
    control: &mut ImpulseControl,
    stimuli: &[(&str, &str, f32)],
) -> Vec<Stimulus> {
    stimuli
        .iter()
        .map(|(content, source, urgency)| control.filter_stimulus(content, source, *urgency))
        .collect()
}
