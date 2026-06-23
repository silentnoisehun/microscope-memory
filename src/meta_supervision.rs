//! Meta-cognitive supervision system for continuous performance evaluation and strategy correction.
//! Monitors system behavior and adjusts operational parameters in real-time.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Performance metrics for system evaluation
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub timestamp: u64,
    pub response_time_ms: f32,
    pub memory_usage_mb: f32,
    pub attention_efficiency: f32, // 0.0-1.0
    pub goal_progress: f32,        // 0.0-1.0
    pub error_rate: f32,           // 0.0-1.0
    pub overall_score: f32,
}

/// Meta-cognitive supervisor for system monitoring
pub struct MetaSupervisor {
    metrics_history: Arc<RwLock<VecDeque<PerformanceMetrics>>>,
    correction_strategies: Vec<String>,
    performance_thresholds: (f32, f32, f32), // (warning, alert, critical)
    last_correction_time: u64,
    correction_cooldown_secs: u64,
}

impl MetaSupervisor {
    /// Create a new meta-supervisor
    pub fn new() -> Self {
        Self {
            metrics_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            correction_strategies: vec![
                "reduce_complexity".to_string(),
                "increase_attention_focus".to_string(),
                "optimize_memory_usage".to_string(),
                "adjust_suppression_threshold".to_string(),
                "reallocate_resources".to_string(),
            ],
            performance_thresholds: (0.7, 0.5, 0.3),
            last_correction_time: 0,
            correction_cooldown_secs: 60, // 1 minute cooldown
        }
    }

    /// Record new performance metrics
    pub fn record_metrics(
        &mut self,
        response_time_ms: f32,
        memory_usage_mb: f32,
        attention_efficiency: f32,
        goal_progress: f32,
        error_rate: f32,
    ) -> PerformanceMetrics {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Calculate overall score (weighted average)
        let overall_score = 0.3 * (1.0 - (response_time_ms / 1000.0).min(1.0))
            + 0.2 * (1.0 - (memory_usage_mb / 1000.0).min(1.0))
            + 0.2 * attention_efficiency
            + 0.2 * goal_progress
            + 0.1 * (1.0 - error_rate);

        let metrics = PerformanceMetrics {
            timestamp,
            response_time_ms,
            memory_usage_mb,
            attention_efficiency,
            goal_progress,
            error_rate,
            overall_score,
        };

        // Add to history
        let mut history = self.metrics_history.write().unwrap();
        if history.len() >= 100 {
            history.pop_front(); // Keep only last 100 entries
        }
        history.push_back(metrics.clone());

        metrics
    }

    /// Evaluate system performance and suggest corrections
    pub fn evaluate_and_correct(&mut self) -> Option<String> {
        let history = self.metrics_history.read().unwrap();

        if history.is_empty() {
            return None;
        }

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check cooldown
        if current_time - self.last_correction_time < self.correction_cooldown_secs {
            return None;
        }

        // Get average of last 10 metrics
        let recent: Vec<&PerformanceMetrics> = history.iter().rev().take(10).collect();
        let avg_score: f32 =
            recent.iter().map(|m| m.overall_score).sum::<f32>() / recent.len() as f32;

        let (warning_threshold, alert_threshold, critical_threshold) = self.performance_thresholds;

        if avg_score < critical_threshold {
            self.last_correction_time = current_time;
            Some("critical_performance_degradation".to_string())
        } else if avg_score < alert_threshold {
            self.last_correction_time = current_time;
            Some(self.select_correction_strategy("alert"))
        } else if avg_score < warning_threshold {
            self.last_correction_time = current_time;
            Some(self.select_correction_strategy("warning"))
        } else {
            None
        }
    }

    /// Select appropriate correction strategy based on severity
    fn select_correction_strategy(&self, severity: &str) -> String {
        match severity {
            "critical" => "reallocate_resources".to_string(),
            "alert" => {
                if self.correction_strategies.len() > 1 {
                    self.correction_strategies[1].clone()
                } else {
                    "increase_attention_focus".to_string()
                }
            }
            "warning" => {
                if !self.correction_strategies.is_empty() {
                    self.correction_strategies[0].clone()
                } else {
                    "reduce_complexity".to_string()
                }
            }
            _ => "adjust_parameters".to_string(),
        }
    }

    /// Analyze trends in performance metrics
    pub fn analyze_trends(&self) -> (f32, f32) {
        let history = self.metrics_history.read().unwrap();

        if history.len() < 2 {
            return (0.0, 0.0);
        }

        let first_half: Vec<&PerformanceMetrics> = history.iter().take(history.len() / 2).collect();
        let second_half: Vec<&PerformanceMetrics> =
            history.iter().skip(history.len() / 2).collect();

        if first_half.is_empty() || second_half.is_empty() {
            return (0.0, 0.0);
        }

        let first_avg: f32 =
            first_half.iter().map(|m| m.overall_score).sum::<f32>() / first_half.len() as f32;
        let second_avg: f32 =
            second_half.iter().map(|m| m.overall_score).sum::<f32>() / second_half.len() as f32;

        let trend = second_avg - first_avg;
        let volatility = self.calculate_volatility(&history);

        (trend, volatility)
    }

    /// Calculate volatility of performance metrics
    fn calculate_volatility(&self, history: &VecDeque<PerformanceMetrics>) -> f32 {
        if history.len() < 2 {
            return 0.0;
        }

        let scores: Vec<f32> = history.iter().map(|m| m.overall_score).collect();
        let mean = scores.iter().sum::<f32>() / scores.len() as f32;

        let variance = scores
            .iter()
            .map(|&score| (score - mean).powi(2))
            .sum::<f32>()
            / scores.len() as f32;

        variance.sqrt()
    }

    /// Get performance summary
    pub fn get_summary(&self) -> (f32, f32, f32) {
        let history = self.metrics_history.read().unwrap();

        if history.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let current = history.back().unwrap().overall_score;
        let (trend, volatility) = self.analyze_trends();

        (current, trend, volatility)
    }

    /// Add a custom correction strategy
    pub fn add_correction_strategy(&mut self, strategy: &str) {
        self.correction_strategies.push(strategy.to_string());
    }
}

/// Generate a performance report
pub fn generate_report(supervisor: &MetaSupervisor) -> String {
    let (current_score, trend, volatility) = supervisor.get_summary();
    let (warning, alert, critical) = supervisor.performance_thresholds;

    format!(
        "Performance Report:\n\
         Current Score: {:.2}\n\
         Trend: {:.2}\n\
         Volatility: {:.2}\n\
         Thresholds - Warning: {:.2}, Alert: {:.2}, Critical: {:.2}\n\
         Status: {}",
        current_score,
        trend,
        volatility,
        warning,
        alert,
        critical,
        if current_score >= warning {
            "Optimal"
        } else if current_score >= alert {
            "Warning"
        } else if current_score >= critical {
            "Alert"
        } else {
            "Critical"
        }
    )
}
