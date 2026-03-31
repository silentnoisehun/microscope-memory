//! Morning Brief — daily intelligence report from your memory.
//!
//! Combines temporal archetypes, spaced repetition, emotional patterns,
//! and emerging themes into a single daily summary.
//!
//! Usage:
//!   microscope-mem morning

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::archetype::ArchetypeState;
use crate::dream::DreamState;
use crate::emotional_contagion::EmotionalContagionState;
use crate::hebbian::HebbianState;
use crate::reader::MicroscopeReader;
use crate::temporal_archetype::{TemporalArchetypeState, WINDOW_LABELS};

// ─── Constants ──────────────────────────────────────

const DAY_MS: u64 = 86_400_000;
const WEEK_MS: u64 = 7 * DAY_MS;
const MONTH_MS: u64 = 30 * DAY_MS;
const YEAR_MS: u64 = 365 * DAY_MS;

/// Spaced repetition intervals
const INTERVALS: &[(u64, &str)] = &[
    (DAY_MS, "1 day ago"),
    (WEEK_MS, "1 week ago"),
    (MONTH_MS, "1 month ago"),
    (YEAR_MS, "1 year ago"),
];

/// Tolerance for finding memories near a time point
const TIME_TOLERANCE_MS: u64 = DAY_MS / 2; // +/- 12 hours

// ─── Morning brief types ────────────────────────────

#[derive(Clone, Debug)]
pub struct MorningBrief {
    pub greeting: String,
    pub current_window: String,
    pub relevant_now: Vec<RelevantItem>,
    pub spaced_repetition: Vec<SpacedItem>,
    pub emotional_forecast: Option<EmotionalForecast>,
    pub emerging_themes: Vec<String>,
    pub dream_summary: Option<String>,
    pub action_items: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RelevantItem {
    pub archetype_label: String,
    pub temporal_boost: f32,
    pub member_count: usize,
}

#[derive(Clone, Debug)]
pub struct SpacedItem {
    pub interval_label: String,
    pub text_preview: String,
    pub block_idx: usize,
}

#[derive(Clone, Debug)]
pub struct EmotionalForecast {
    pub predicted_mood: String,
    pub valence: f32,
    pub energy: f32,
    pub basis: String,
}

// ─── Generate morning brief ─────────────────────────

pub fn generate_morning(output_dir: &Path, reader: &MicroscopeReader) -> MorningBrief {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
    let archetypes = ArchetypeState::load_or_init(output_dir);
    let temporal = TemporalArchetypeState::load_or_init(output_dir);
    let dream = DreamState::load_or_init(output_dir);
    let emotional = EmotionalContagionState::load_or_init(output_dir);

    // Current time window
    let hour = ((now_ms / 3_600_000) % 24) as usize;
    let current_window_idx = hour / 4;
    let current_window = WINDOW_LABELS
        .get(current_window_idx)
        .unwrap_or(&"??")
        .to_string();

    // Greeting based on time
    let greeting = match hour {
        0..=5 => "Good night".to_string(),
        6..=11 => "Good morning".to_string(),
        12..=17 => "Good afternoon".to_string(),
        _ => "Good evening".to_string(),
    };

    // 1. What's relevant NOW — archetypes with high temporal boost for this window
    let relevant_now = find_relevant_now(&archetypes, &temporal, current_window_idx);

    // 2. Spaced repetition — memories from 1d/1w/1m/1y ago
    let spaced_repetition = find_spaced_items(&hebb, reader, now_ms);

    // 3. Emotional forecast — based on temporal emotional patterns
    let emotional_forecast = predict_emotion(&emotional, &temporal, current_window_idx);

    // 4. Emerging themes
    let emerging_themes: Vec<String> = archetypes
        .archetypes
        .iter()
        .filter(|a| a.reinforcement_count <= 3 && a.strength > 0.5)
        .take(3)
        .map(|a| format!("{} (strength: {:.1})", a.label, a.strength))
        .collect();

    // 5. Dream summary
    let dream_summary = if let Some(last) = dream.cycles.last() {
        Some(format!(
            "Last dream cycle: {} pairs strengthened, {} pruned, energy {:.2} -> {:.2}",
            last.strengthened_pairs, last.pruned_pairs, last.energy_before, last.energy_after
        ))
    } else {
        None
    };

    // 6. Action items
    let mut action_items = Vec::new();

    if dream.cycles.is_empty() {
        action_items.push("Run 'microscope-mem dream' to consolidate memories".to_string());
    } else if let Some(last) = dream.cycles.last() {
        if now_ms - last.timestamp_ms > DAY_MS {
            action_items.push("Dream consolidation overdue — run 'microscope-mem dream'".to_string());
        }
    }

    if archetypes.archetypes.is_empty() {
        action_items.push("Run 'microscope-mem emerge' to detect emerging patterns".to_string());
    }

    let fading_count = hebb
        .activations
        .iter()
        .filter(|a| a.activation_count >= 5 && a.energy < 0.01)
        .count();
    if fading_count > 10 {
        action_items.push(format!(
            "{} important memories are fading — recall them to strengthen",
            fading_count
        ));
    }

    MorningBrief {
        greeting,
        current_window,
        relevant_now,
        spaced_repetition,
        emotional_forecast,
        emerging_themes,
        dream_summary,
        action_items,
    }
}

// ─── Helpers ────────────────────────────────────────

fn find_relevant_now(
    archetypes: &ArchetypeState,
    temporal: &TemporalArchetypeState,
    current_window: usize,
) -> Vec<RelevantItem> {
    let mut items = Vec::new();

    for archetype in &archetypes.archetypes {
        if let Some(profile) = temporal
            .profiles
            .iter()
            .find(|p| p.archetype_id == archetype.id)
        {
            let boost = profile.temporal_boost(current_window);
            if boost > 0.5 {
                items.push(RelevantItem {
                    archetype_label: archetype.label.clone(),
                    temporal_boost: boost,
                    member_count: archetype.members.len(),
                });
            }
        }
    }

    items.sort_by(|a, b| b.temporal_boost.partial_cmp(&a.temporal_boost).unwrap_or(std::cmp::Ordering::Equal));
    items.truncate(5);
    items
}

fn find_spaced_items(
    hebb: &HebbianState,
    reader: &MicroscopeReader,
    now_ms: u64,
) -> Vec<SpacedItem> {
    let mut items = Vec::new();

    for &(interval_ms, label) in INTERVALS {
        let target_ms = now_ms.saturating_sub(interval_ms);

        // Find the block activated closest to this time point
        let mut best: Option<(usize, u64)> = None;
        for (idx, act) in hebb.activations.iter().enumerate() {
            if act.activation_count == 0 || act.last_activated_ms == 0 {
                continue;
            }
            let diff = (act.last_activated_ms as i64 - target_ms as i64).unsigned_abs();
            if diff < TIME_TOLERANCE_MS {
                match best {
                    None => best = Some((idx, diff)),
                    Some((_, best_diff)) if diff < best_diff => best = Some((idx, diff)),
                    _ => {}
                }
            }
        }

        if let Some((idx, _)) = best {
            if idx < reader.block_count {
                let text = reader.text(idx);
                let preview: String = text.chars().take(100).filter(|&c| c != '\n').collect();
                items.push(SpacedItem {
                    interval_label: label.to_string(),
                    text_preview: preview,
                    block_idx: idx,
                });
            }
        }
    }

    items
}

fn predict_emotion(
    emotional: &EmotionalContagionState,
    temporal: &TemporalArchetypeState,
    current_window: usize,
) -> Option<EmotionalForecast> {
    if emotional.remote_snapshots.is_empty() {
        return None;
    }

    // Use recent snapshots from the same time window
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let window_snapshots: Vec<&crate::emotional_contagion::EmotionalSnapshot> = emotional
        .remote_snapshots
        .iter()
        .filter(|s| {
            let hour = ((s.timestamp_ms / 3_600_000) % 24) as usize;
            hour / 4 == current_window
        })
        .collect();

    if window_snapshots.is_empty() {
        return None;
    }

    let avg_valence: f32 =
        window_snapshots.iter().map(|s| s.valence).sum::<f32>() / window_snapshots.len() as f32;
    let avg_energy: f32 =
        window_snapshots.iter().map(|s| s.total_energy).sum::<f32>() / window_snapshots.len() as f32;

    let mood = if avg_valence > 0.3 {
        "positive"
    } else if avg_valence < -0.3 {
        "negative"
    } else {
        "neutral"
    };

    let energy_level = if avg_energy > 0.5 {
        "high"
    } else if avg_energy < -0.3 {
        "low"
    } else {
        "moderate"
    };

    Some(EmotionalForecast {
        predicted_mood: format!("{} mood, {} energy", mood, energy_level),
        valence: avg_valence,
        energy: avg_energy,
        basis: format!(
            "Based on {} previous sessions in the {} window",
            window_snapshots.len(),
            WINDOW_LABELS.get(current_window).unwrap_or(&"??")
        ),
    })
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intervals() {
        assert_eq!(INTERVALS.len(), 4);
        assert!(INTERVALS[0].0 < INTERVALS[1].0);
    }

    #[test]
    fn test_time_tolerance() {
        assert_eq!(TIME_TOLERANCE_MS, DAY_MS / 2);
    }
}
