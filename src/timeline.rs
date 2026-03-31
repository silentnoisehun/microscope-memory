//! Timeline — thought archaeology for any topic.
//!
//! Shows how your thinking about a specific topic evolved over time.
//! Tracks emotional shifts, related concepts, and turning points.
//!
//! Usage:
//!   microscope-mem timeline "career"
//!   microscope-mem timeline "memory" --limit 20

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::emotional_contagion::EmotionalContagionState;
use crate::hebbian::HebbianState;
use crate::reader::MicroscopeReader;
use crate::{read_append_log, AppendEntry};

// ─── Types ──────────────────────────────────────────

/// A single point in a topic's timeline.
#[derive(Clone, Debug)]
pub struct TimelinePoint {
    pub timestamp_ms: u64,
    pub text: String,
    pub block_idx: usize,
    pub is_append: bool,
    pub activation_count: u32,
    pub energy: f32,
    pub depth: u8,
    pub layer: String,
}

/// Emotional shift detected in the timeline.
#[derive(Clone, Debug)]
pub struct EmotionalShift {
    pub from_valence: f32,
    pub to_valence: f32,
    pub timestamp_ms: u64,
    pub description: String,
}

/// Full timeline report for a topic.
#[derive(Clone, Debug)]
pub struct TimelineReport {
    pub topic: String,
    pub points: Vec<TimelinePoint>,
    pub first_mention_ms: Option<u64>,
    pub last_mention_ms: Option<u64>,
    pub total_mentions: usize,
    pub emotional_shifts: Vec<EmotionalShift>,
    pub related_topics: Vec<(String, usize)>, // (topic, co-occurrence count)
    pub turning_points: Vec<TurningPoint>,
}

/// A significant change in how the topic appears in memory.
#[derive(Clone, Debug)]
pub struct TurningPoint {
    pub timestamp_ms: u64,
    pub description: String,
    pub before_text: String,
    pub after_text: String,
}

// ─── Generate timeline ──────────────────────────────

pub fn generate_timeline(
    output_dir: &Path,
    reader: &MicroscopeReader,
    config: &crate::config::Config,
    topic: &str,
    limit: usize,
) -> TimelineReport {
    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);

    let topic_lower = topic.to_lowercase();
    let keywords: Vec<&str> = topic_lower.split_whitespace().collect();

    let mut points = Vec::new();

    // Search main index
    for idx in 0..reader.block_count {
        let text = reader.text(idx);
        let text_lower = text.to_lowercase();

        let matches = keywords
            .iter()
            .all(|kw| text_lower.contains(kw));

        if matches {
            let h = reader.header(idx);
            let act = if idx < hebb.activations.len() {
                &hebb.activations[idx]
            } else {
                &crate::hebbian::ActivationRecord::default()
            };

            let layer = crate::LAYER_NAMES
                .get(h.layer_id as usize)
                .unwrap_or(&"?")
                .to_string();

            points.push(TimelinePoint {
                timestamp_ms: act.last_activated_ms,
                text: text.to_string(),
                block_idx: idx,
                is_append: false,
                activation_count: act.activation_count,
                energy: act.energy,
                depth: h.depth,
                layer,
            });
        }
    }

    // Search append log
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (i, entry) in appended.iter().enumerate() {
        let text_lower = entry.text.to_lowercase();
        let matches = keywords.iter().all(|kw| text_lower.contains(kw));
        if matches {
            points.push(TimelinePoint {
                timestamp_ms: 0,
                text: entry.text.clone(),
                block_idx: i + 1_000_000,
                is_append: true,
                activation_count: 0,
                energy: 0.0,
                depth: entry.depth,
                layer: crate::LAYER_NAMES
                    .get(entry.layer_id as usize)
                    .unwrap_or(&"?")
                    .to_string(),
            });
        }
    }

    // Sort by timestamp
    points.sort_by_key(|p| p.timestamp_ms);

    // Limit
    if points.len() > limit {
        points = points[points.len() - limit..].to_vec();
    }

    let first_mention = points.first().map(|p| p.timestamp_ms);
    let last_mention = points.last().map(|p| p.timestamp_ms);
    let total_mentions = points.len();

    // Find related topics — words that frequently appear alongside the topic
    let related_topics = find_related_topics(&points, &keywords);

    // Detect turning points — significant changes in frequency or energy
    let turning_points = detect_turning_points(&points);

    // Emotional shifts (simplified — based on energy changes)
    let emotional_shifts = detect_emotional_shifts(&points);

    TimelineReport {
        topic: topic.to_string(),
        points,
        first_mention_ms: first_mention,
        last_mention_ms: last_mention,
        total_mentions,
        emotional_shifts,
        related_topics,
        turning_points,
    }
}

// ─── Helpers ────────────────────────────────────────

fn find_related_topics(points: &[TimelinePoint], exclude_keywords: &[&str]) -> Vec<(String, usize)> {
    let mut word_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    // Common stop words to exclude
    let stop_words: std::collections::HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "shall", "can", "and", "or", "but", "if",
        "then", "else", "when", "at", "by", "for", "with", "about", "against",
        "between", "through", "during", "before", "after", "above", "below",
        "to", "from", "up", "down", "in", "out", "on", "off", "over", "under",
        "again", "further", "then", "once", "here", "there", "all", "each",
        "every", "both", "few", "more", "most", "other", "some", "such", "no",
        "not", "only", "own", "same", "so", "than", "too", "very", "just",
        "that", "this", "these", "those", "it", "its", "of", "as",
    ]
    .iter()
    .copied()
    .collect();

    for point in points {
        let words: Vec<String> = point
            .text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3)
            .filter(|w| !stop_words.contains(w))
            .filter(|w| !exclude_keywords.contains(w))
            .map(|w| w.to_string())
            .collect();

        for word in words {
            *word_counts.entry(word).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<(String, usize)> = word_counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(10);
    sorted
}

fn detect_turning_points(points: &[TimelinePoint]) -> Vec<TurningPoint> {
    let mut turning_points = Vec::new();

    if points.len() < 4 {
        return turning_points;
    }

    // Sliding window: detect energy spikes
    let window_size = (points.len() / 4).max(2);

    for i in window_size..points.len() {
        let before_avg: f32 = points[i - window_size..i]
            .iter()
            .map(|p| p.energy)
            .sum::<f32>()
            / window_size as f32;

        let after_avg: f32 = points[i..std::cmp::min(i + window_size, points.len())]
            .iter()
            .map(|p| p.energy)
            .sum::<f32>()
            / std::cmp::min(window_size, points.len() - i).max(1) as f32;

        let change = after_avg - before_avg;
        if change.abs() > 0.2 {
            let before_preview: String = points[i - 1]
                .text
                .chars()
                .take(60)
                .filter(|&c| c != '\n')
                .collect();
            let after_preview: String = points[i]
                .text
                .chars()
                .take(60)
                .filter(|&c| c != '\n')
                .collect();

            let direction = if change > 0.0 {
                "intensifying"
            } else {
                "fading"
            };

            turning_points.push(TurningPoint {
                timestamp_ms: points[i].timestamp_ms,
                description: format!(
                    "Topic {} ({:+.2} energy shift)",
                    direction, change
                ),
                before_text: before_preview,
                after_text: after_preview,
            });
        }
    }

    // Keep only the most significant turning points
    turning_points.sort_by(|a, b| {
        let a_change: f32 = a.description.split('(').nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let b_change: f32 = b.description.split('(').nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        b_change.abs().partial_cmp(&a_change.abs()).unwrap_or(std::cmp::Ordering::Equal)
    });
    turning_points.truncate(5);
    turning_points
}

fn detect_emotional_shifts(points: &[TimelinePoint]) -> Vec<EmotionalShift> {
    let mut shifts = Vec::new();

    if points.len() < 4 {
        return shifts;
    }

    // Use energy as a proxy for emotional engagement
    let window = (points.len() / 3).max(2);

    let early_energy: f32 = points[..window]
        .iter()
        .map(|p| p.energy)
        .sum::<f32>()
        / window as f32;

    let late_energy: f32 = points[points.len() - window..]
        .iter()
        .map(|p| p.energy)
        .sum::<f32>()
        / window as f32;

    let change = late_energy - early_energy;
    if change.abs() > 0.1 {
        let description = if change > 0.0 {
            "Increasing engagement with this topic"
        } else {
            "Decreasing engagement with this topic"
        };

        shifts.push(EmotionalShift {
            from_valence: early_energy,
            to_valence: late_energy,
            timestamp_ms: points.last().map(|p| p.timestamp_ms).unwrap_or(0),
            description: description.to_string(),
        });
    }

    shifts
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_related_empty() {
        let related = find_related_topics(&[], &["test"]);
        assert!(related.is_empty());
    }

    #[test]
    fn test_detect_turning_points_short() {
        let points = vec![
            TimelinePoint {
                timestamp_ms: 1000,
                text: "hello".to_string(),
                block_idx: 0,
                is_append: false,
                activation_count: 1,
                energy: 0.5,
                depth: 0,
                layer: "long_term".to_string(),
            },
        ];
        let tp = detect_turning_points(&points);
        assert!(tp.is_empty()); // too few points
    }
}
