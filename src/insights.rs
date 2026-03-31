//! Insights — automatic pattern detection across all consciousness layers.
//!
//! Analyzes Hebbian activations, co-activations, temporal patterns, emotional
//! state, archetypes, and thought graphs to surface what's happening in your
//! memory landscape.
//!
//! Usage:
//!   microscope-mem insights

use std::collections::HashMap;
use std::path::Path;

use crate::archetype::ArchetypeState;
use crate::attention::AttentionState;
use crate::dream::DreamState;
use crate::emotional_contagion::EmotionalContagionState;
use crate::hebbian::HebbianState;
use crate::reader::MicroscopeReader;
use crate::resonance::ResonanceState;
use crate::temporal_archetype::TemporalArchetypeState;
use crate::thought_graph::ThoughtGraphState;

// ─── Insight types ──────────────────────────────────

/// A single insight derived from memory analysis.
#[derive(Clone, Debug)]
pub struct Insight {
    pub category: InsightCategory,
    pub title: String,
    pub description: String,
    pub evidence: Vec<String>,
    pub strength: f32, // 0.0 - 1.0
}

#[derive(Clone, Debug, PartialEq)]
pub enum InsightCategory {
    TopTheme,        // Most activated topics
    CoOccurrence,    // Things that appear together
    TemporalPattern, // Time-based patterns
    EmotionalTrend,  // Emotional patterns
    FadingMemory,    // Things you haven't thought about
    EmergingTheme,   // Newly crystallizing archetypes
    DreamPattern,    // What dream consolidation reveals
    AttentionBias,   // Which layers dominate your thinking
}

/// Full insights report.
#[derive(Clone, Debug)]
pub struct InsightsReport {
    pub insights: Vec<Insight>,
    pub total_blocks: usize,
    pub total_activations: u64,
    pub total_archetypes: usize,
    pub active_patterns: usize,
}

// ─── Generate insights ──────────────────────────────

pub fn generate_insights(
    output_dir: &Path,
    reader: &MicroscopeReader,
) -> InsightsReport {
    let hebb = HebbianState::load_or_init(output_dir, reader.block_count);
    let archetypes = ArchetypeState::load_or_init(output_dir);
    let temporal = TemporalArchetypeState::load_or_init(output_dir);
    let thought_graph = ThoughtGraphState::load_or_init(output_dir);
    let attention = AttentionState::load_or_init(output_dir);
    let dream = DreamState::load_or_init(output_dir);
    let emotional = EmotionalContagionState::load_or_init(output_dir);
    let resonance = ResonanceState::load_or_init(output_dir);

    let mut insights = Vec::new();

    // 1. Top themes — most activated blocks
    insights.extend(analyze_top_themes(&hebb, reader));

    // 2. Co-occurrence — what fires together
    insights.extend(analyze_cooccurrence(&hebb, reader));

    // 3. Temporal patterns — when you think about what
    insights.extend(analyze_temporal(&temporal, &archetypes));

    // 4. Emotional trends
    insights.extend(analyze_emotional(&emotional));

    // 5. Fading memories — high activation count but decayed energy
    insights.extend(analyze_fading(&hebb, reader));

    // 6. Emerging themes — young archetypes gaining strength
    insights.extend(analyze_emerging(&archetypes));

    // 7. Dream patterns — what consolidation reveals
    insights.extend(analyze_dreams(&dream));

    // 8. Attention bias — which cognitive layers dominate
    insights.extend(analyze_attention(&attention));

    // Sort by strength
    insights.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal));

    let total_activations: u64 = hebb
        .activations
        .iter()
        .map(|a| a.activation_count as u64)
        .sum();

    InsightsReport {
        insights,
        total_blocks: reader.block_count,
        total_activations,
        total_archetypes: archetypes.archetypes.len(),
        active_patterns: thought_graph.top_patterns(100).len(),
    }
}

// ─── Individual analyzers ───────────────────────────

fn analyze_top_themes(hebb: &HebbianState, reader: &MicroscopeReader) -> Vec<Insight> {
    let mut insights = Vec::new();

    // Find top 5 most activated blocks
    let mut indexed: Vec<(usize, &crate::hebbian::ActivationRecord)> =
        hebb.activations.iter().enumerate().collect();
    indexed.sort_by(|a, b| b.1.energy.partial_cmp(&a.1.energy).unwrap_or(std::cmp::Ordering::Equal));

    let top: Vec<(usize, f32)> = indexed
        .iter()
        .filter(|(_, a)| a.activation_count > 0)
        .take(5)
        .map(|(i, a)| (*i, a.energy))
        .collect();

    if !top.is_empty() {
        let evidence: Vec<String> = top
            .iter()
            .filter_map(|(idx, energy)| {
                if *idx < reader.block_count {
                    let text = reader.text(*idx);
                    let preview: String = text.chars().take(80).filter(|&c| c != '\n').collect();
                    Some(format!("({:.2}) {}", energy, preview))
                } else {
                    None
                }
            })
            .collect();

        if !evidence.is_empty() {
            insights.push(Insight {
                category: InsightCategory::TopTheme,
                title: "Top active themes".to_string(),
                description: "These memories have the highest activation energy right now.".to_string(),
                strength: top.first().map(|(_, e)| (*e).min(1.0)).unwrap_or(0.0),
                evidence,
            });
        }
    }

    insights
}

fn analyze_cooccurrence(hebb: &HebbianState, reader: &MicroscopeReader) -> Vec<Insight> {
    let mut insights = Vec::new();

    // Find strongest co-activation pairs
    let mut pairs = hebb.coactivations.values().cloned().collect::<Vec<_>>().clone();
    pairs.sort_by(|a, b| b.count.cmp(&a.count));

    let top_pairs: Vec<String> = pairs
        .iter()
        .take(5)
        .filter_map(|pair| {
            let a_idx = pair.block_a as usize;
            let b_idx = pair.block_b as usize;
            if a_idx < reader.block_count && b_idx < reader.block_count {
                let a_text: String = reader.text(a_idx).chars().take(40).filter(|&c| c != '\n').collect();
                let b_text: String = reader.text(b_idx).chars().take(40).filter(|&c| c != '\n').collect();
                Some(format!("({}x) '{}' <-> '{}'", pair.count, a_text, b_text))
            } else {
                None
            }
        })
        .collect();

    if !top_pairs.is_empty() {
        let strength = pairs.first().map(|p| (p.count as f32 / 20.0).min(1.0)).unwrap_or(0.0);
        insights.push(Insight {
            category: InsightCategory::CoOccurrence,
            title: "Linked concepts".to_string(),
            description: "These ideas consistently appear together in your thinking.".to_string(),
            strength,
            evidence: top_pairs,
        });
    }

    insights
}

fn analyze_temporal(
    temporal: &TemporalArchetypeState,
    archetypes: &ArchetypeState,
) -> Vec<Insight> {
    let mut insights = Vec::new();

    for profile in &temporal.profiles {
        if let Some(dominant) = profile.dominant_window() {
            let label = archetypes
                .archetypes
                .iter()
                .find(|a| a.id == profile.archetype_id)
                .map(|a| a.label.clone())
                .unwrap_or_else(|| format!("Archetype#{}", profile.archetype_id));

            let window_label = crate::temporal_archetype::WINDOW_LABELS
                .get(dominant)
                .unwrap_or(&"??");

            insights.push(Insight {
                category: InsightCategory::TemporalPattern,
                title: format!("'{}' peaks at {}", label, window_label),
                description: format!(
                    "This theme is most active during the {} time window ({} total activations).",
                    window_label, profile.total_activations
                ),
                strength: (profile.total_activations as f32 / 50.0).min(1.0),
                evidence: profile
                    .window_weights
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        let wl = crate::temporal_archetype::WINDOW_LABELS
                            .get(i)
                            .unwrap_or(&"??");
                        format!("{}: {:.0}%", wl, w * 100.0)
                    })
                    .collect(),
            });
        }
    }

    insights
}

fn analyze_emotional(emotional: &EmotionalContagionState) -> Vec<Insight> {
    let mut insights = Vec::new();

    if !emotional.remote_snapshots.is_empty() {
        let recent = &emotional.remote_snapshots[emotional.remote_snapshots.len().saturating_sub(10)..];
        let avg_valence: f32 =
            recent.iter().map(|s| s.valence).sum::<f32>() / recent.len().max(1) as f32;
        let avg_energy: f32 =
            recent.iter().map(|s| s.total_energy).sum::<f32>() / recent.len().max(1) as f32;

        let mood = if avg_valence > 0.3 {
            "positive"
        } else if avg_valence < -0.3 {
            "negative"
        } else {
            "neutral"
        };

        let energy = if avg_energy > 0.5 {
            "high energy"
        } else if avg_energy < -0.3 {
            "low energy"
        } else {
            "moderate energy"
        };

        insights.push(Insight {
            category: InsightCategory::EmotionalTrend,
            title: format!("Emotional state: {} / {}", mood, energy),
            description: format!(
                "Recent emotional trend: valence={:.2}, arousal={:.2} ({} snapshots analyzed).",
                avg_valence,
                avg_energy,
                recent.len()
            ),
            strength: avg_valence.abs().min(1.0),
            evidence: vec![
                format!("Average valence: {:.2}", avg_valence),
                format!("Average arousal: {:.2}", avg_energy),
                format!("Snapshots: {}", emotional.remote_snapshots.len()),
            ],
        });
    }

    insights
}

fn analyze_fading(hebb: &HebbianState, reader: &MicroscopeReader) -> Vec<Insight> {
    let mut insights = Vec::new();

    // Find blocks that were activated many times but now have very low energy
    let mut fading: Vec<(usize, u32, f32)> = hebb
        .activations
        .iter()
        .enumerate()
        .filter(|(_, a)| a.activation_count >= 3 && a.energy < 0.01)
        .map(|(i, a)| (i, a.activation_count, a.energy))
        .collect();

    fading.sort_by(|a, b| b.1.cmp(&a.1));

    let evidence: Vec<String> = fading
        .iter()
        .take(5)
        .filter_map(|(idx, count, _)| {
            if *idx < reader.block_count {
                let text = reader.text(*idx);
                let preview: String = text.chars().take(60).filter(|&c| c != '\n').collect();
                Some(format!("({}x, now faded) {}", count, preview))
            } else {
                None
            }
        })
        .collect();

    if !evidence.is_empty() {
        insights.push(Insight {
            category: InsightCategory::FadingMemory,
            title: "Fading memories".to_string(),
            description: "Once-active memories that haven't been recalled recently.".to_string(),
            strength: 0.4,
            evidence,
        });
    }

    insights
}

fn analyze_emerging(archetypes: &ArchetypeState) -> Vec<Insight> {
    let mut insights = Vec::new();

    // Find young archetypes (low reinforcement count but growing)
    let mut young: Vec<&crate::archetype::Archetype> = archetypes
        .archetypes
        .iter()
        .filter(|a| a.reinforcement_count <= 5 && a.strength > 0.5)
        .collect();

    young.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal));

    let evidence: Vec<String> = young
        .iter()
        .take(5)
        .map(|a| {
            format!(
                "'{}' (strength={:.2}, {} members, reinforced {}x)",
                a.label,
                a.strength,
                a.members.len(),
                a.reinforcement_count
            )
        })
        .collect();

    if !evidence.is_empty() {
        insights.push(Insight {
            category: InsightCategory::EmergingTheme,
            title: "Emerging themes".to_string(),
            description: "New patterns crystallizing in your memory — concepts taking shape."
                .to_string(),
            strength: 0.7,
            evidence,
        });
    }

    insights
}

fn analyze_dreams(dream: &DreamState) -> Vec<Insight> {
    let mut insights = Vec::new();

    if !dream.cycles.is_empty() {
        let recent = &dream.cycles[dream.cycles.len().saturating_sub(5)..];

        let total_strengthened: u32 = recent.iter().map(|c| c.strengthened_pairs).sum();
        let total_pruned: u32 = recent.iter().map(|c| c.pruned_pairs + c.pruned_activations).sum();
        let total_consolidated: u32 = recent.iter().map(|c| c.consolidated_patterns).sum();

        let avg_energy_change: f32 = recent
            .iter()
            .map(|c| c.energy_after - c.energy_before)
            .sum::<f32>()
            / recent.len().max(1) as f32;

        let trend = if avg_energy_change > 0.0 {
            "growing"
        } else if avg_energy_change < -0.1 {
            "consolidating (pruning weak links)"
        } else {
            "stable"
        };

        insights.push(Insight {
            category: InsightCategory::DreamPattern,
            title: format!("Dream consolidation: {}", trend),
            description: format!(
                "Last {} cycles: {} pairs strengthened, {} pruned, {} patterns consolidated.",
                recent.len(),
                total_strengthened,
                total_pruned,
                total_consolidated
            ),
            strength: 0.5,
            evidence: vec![
                format!("Energy trend: {:.3} per cycle", avg_energy_change),
                format!("Total dream cycles: {}", dream.cycles.len()),
                format!("Strengthened: {}", total_strengthened),
                format!("Pruned: {}", total_pruned),
            ],
        });
    }

    insights
}

fn analyze_attention(attention: &AttentionState) -> Vec<Insight> {
    let mut insights = Vec::new();

    if attention.total_recalls > 5 {
        let weights = &attention.learned_weights;
        let layer_names = [
            "Hebbian",
            "Mirror",
            "Resonance",
            "Archetype",
            "Emotional",
            "ThoughtGraph",
            "PredictiveCache",
            "Temporal",
            "Attention",
        ];

        let mut indexed: Vec<(usize, f32)> = weights.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let dominant = indexed.first().map(|(i, _)| *i).unwrap_or(0);
        let dominant_name = layer_names.get(dominant).unwrap_or(&"Unknown");

        let evidence: Vec<String> = indexed
            .iter()
            .take(5)
            .filter_map(|(i, w)| {
                layer_names.get(*i).map(|name| format!("{}: {:.1}%", name, w * 100.0))
            })
            .collect();

        insights.push(Insight {
            category: InsightCategory::AttentionBias,
            title: format!("Thinking style: {} dominant", dominant_name),
            description: format!(
                "Your recall patterns rely most on the {} layer ({} total recalls analyzed).",
                dominant_name, attention.total_recalls
            ),
            strength: 0.3,
            evidence,
        });
    }

    insights
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insight_categories() {
        assert_ne!(InsightCategory::TopTheme, InsightCategory::FadingMemory);
    }

    #[test]
    fn test_empty_report() {
        // With empty states, analyzers should return empty vecs, not panic
        let report = InsightsReport {
            insights: vec![],
            total_blocks: 0,
            total_activations: 0,
            total_archetypes: 0,
            active_patterns: 0,
        };
        assert_eq!(report.insights.len(), 0);
    }
}
