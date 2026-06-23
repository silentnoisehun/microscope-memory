//! Daydreaming — asszociatív drift külső prompt nélkül.
//!
//! Amikor a rendszer tétlen, nem áll le. A legutóbbi narratív blokkból
//! indulva asszociatív láncot indít: seed → recall → legközelebbi blokk
//! → ESR frissítés → új narratíva → ismétlés.
//!
//! Bináris formátum: no separate file, uses narrative.bin and append log.

use crate::config::Config;
use crate::reader::MicroscopeReader;
use std::path::Path;

// ─── Constants ──────────────────────────────────────

/// Hány blokkot hozzunk fel minden drift lépésben.
/// Minimális távolság a seed és az asszociált blokk között (hogy ne ugyanazt hozza).
const MIN_ASSOC_DIST: f32 = 0.01;

// ─── DaydreamResult ────────────────────────────────

/// Egy daydream ciklus eredménye.
pub struct DaydreamResult {
    pub steps: Vec<DaydreamStep>,
    pub final_narrative: String,
    pub total_emotion_shift: f32,
}

/// Egyetlen asszociatív lépés.
pub struct DaydreamStep {
    pub step: usize,
    pub seed_text: String,
    pub associated_block: u32,
    pub associated_text: String,
    pub spatial_dist: f32,
    pub emotion_shift: f32,
}

/// Daydreaming: asszociatív drift végrehajtása.
///
/// 1. Seed: a legutóbbi narratíva (vagy egy query)
/// 2. Recall a seed-re → top K eredmény
/// 3. Válasszuk ki a legközelebbi asszociatív blokkot (ami nem a seed)
/// 4. ESR frissítés a blokk emotion-jával
/// 5. Új narratíva generálás
/// 6. Ismétlés steps-szer
pub fn daydream(config: &Config, seed: &str, steps: usize) -> Result<DaydreamResult, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let reader = MicroscopeReader::open(config).map_err(|e| format!("open reader: {}", e))?;
    let sw = config.search.semantic_weight;

    let mut current_seed = seed.to_string();
    let mut visited = std::collections::HashSet::new();
    let mut result = DaydreamResult {
        steps: Vec::new(),
        final_narrative: String::new(),
        total_emotion_shift: 0.0,
    };

    let mut ring = crate::emotional_state::EmotionalStateRing::load_or_init(output_dir);
    let mut narrative_state = crate::narrative::NarrativeState::load_or_init(output_dir);

    for step in 0..steps {
        // 1. Recall the seed
        let (qx, qy, qz) = crate::content_coords_blended(&current_seed, "long_term", sw);
        let (zoom_lo, zoom_hi) = (2, 4);

        let mut best_dist = f32::MAX;
        let mut best_idx = u32::MAX;
        let mut best_text = String::new();

        for zoom in zoom_lo..=zoom_hi {
            let (start, count) = reader.depth_ranges[zoom as usize];
            if count == 0 {
                continue;
            }

            // Linear scan (acceptable for daydreaming — not latency-critical)
            for bi in 0..count {
                let i = start + bi;
                let hdr = reader.header(i as usize);
                let dx = hdr.x - qx;
                let dy = hdr.y - qy;
                let dz = hdr.z - qz;
                let dist = dx * dx + dy * dy + dz * dz;
                if dist < MIN_ASSOC_DIST {
                    continue;
                } // skip same
                if visited.contains(&i) {
                    continue;
                } // skip already visited
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = i;
                    best_text = reader.text(i as usize).to_string();
                }
            }
        }

        if best_idx == u32::MAX {
            break; // nothing new to associate
        }
        visited.insert(best_idx);

        // 2. Load emotions.bin for this block
        let emotion_lookup = crate::reader::load_emotion_lookup(output_dir);
        let block_emotion = emotion_lookup
            .as_ref()
            .and_then(|lookup| lookup(best_idx as usize))
            .unwrap_or([0.0f32; 21]);

        let emo_intensity: f32 = block_emotion.iter().map(|x| x * x).sum::<f32>().sqrt();
        let shift_before = ring.intensity();

        // 3. Update ESR with the associated block's emotion
        if emo_intensity > 0.1 {
            ring.update(&block_emotion, 5);
            let _ = ring.save(output_dir);
        }

        let shift_after = ring.intensity();
        let emotion_shift = (shift_after - shift_before).abs();

        // 4. Generate new narrative
        let _ = narrative_state.update(
            output_dir,
            Some(&ring),
            None,
            None,
            None,
            Some(&format!("daydream: {}", safe_truncate(&best_text, 40))),
        );

        result.steps.push(DaydreamStep {
            step,
            seed_text: safe_truncate(&current_seed, 40),
            associated_block: best_idx,
            associated_text: safe_truncate(&best_text, 60),
            spatial_dist: best_dist.sqrt(),
            emotion_shift,
        });

        result.total_emotion_shift += emotion_shift;

        // 5. The associated text becomes the next seed
        current_seed = best_text;
    }

    result.final_narrative = narrative_state.narrative.clone();
    Ok(result)
}

fn safe_truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

/// DaydreamResult formázása CLI kimenethez.
pub fn format_daydream(result: &DaydreamResult, verbose: bool) -> String {
    let mut out = format!(
        "{} ({} steps)\n",
        "DAYDREAM".cyan().bold(),
        result.steps.len()
    );
    out.push_str(&format!(
        "  Total emotion shift: {:.3}\n",
        result.total_emotion_shift
    ));
    out.push_str(&format!(
        "  Final narrative: \"{}\"\n",
        result.final_narrative
    ));

    if verbose {
        for step in &result.steps {
            out.push_str(&format!(
                "  [{}/{}] \"{}\" → block {} dist={:.3} emo_shift={:.3}\n             \"{}\"\n",
                step.step + 1,
                result.steps.len(),
                step.seed_text,
                step.associated_block,
                step.spatial_dist,
                step.emotion_shift,
                step.associated_text,
            ));
        }
    }

    out
}

use colored::Colorize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_truncate_short() {
        assert_eq!(safe_truncate("hello", 10), "hello");
    }

    #[test]
    fn test_safe_truncate_long() {
        let s = safe_truncate("hello world this is a long string", 10);
        assert!(s.len() <= 13); // 10 + "..."
        assert!(s.ends_with("..."));
    }

    #[test]
    fn test_daydream_empty_steps() {
        // Just validate that the result type works
        let result = DaydreamResult {
            steps: vec![],
            final_narrative: "I am silent.".to_string(),
            total_emotion_shift: 0.0,
        };
        assert_eq!(result.steps.len(), 0);
    }
}
