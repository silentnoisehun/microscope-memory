//! Rekonszolidáció — minden recall ÁTÍRJA a memóriát.
//!
//! Az emberi agyban minden felidézés rekonszolidáció: a memória az aktuális
//! kontextusba ágyazva íródik újra. Itt ugyanez:
//! - Emotion blend: a blokk érzelmi vektora közelít a query emotion-hoz
//! - Spatial drift: a blokk koordinátái húzódnak a query térhez
//!
//! Ez teszi a rendszert PATHDEPENDENS-sé: két azonos store → különböző
//! recall útvonalak → különböző memóriaállapotok. Mint egy igzi agy.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::reader::{emotional_similarity, load_emotion_lookup, write_emotion};
use crate::{content_coords_blended, MicroscopeReader};

// ─── Constants ──────────────────────────────────────

/// Emotion blend rate: mennyit mozdul a blokk emotion vektora a query felé.
const EMOTION_BLEND_RATE: f32 = 0.15;
/// Spatial drift rate: mennyit mozdul a blokk koordinátája a query felé.
const SPATIAL_DRIFT_RATE: f32 = 0.02;
/// Ha a query emotion és a blokk emotion hasonlósága e felett van, nem blend-elünk (már hasonló).
const MAX_EMO_SIM_FOR_RECONSOLIDATION: f32 = 0.85;
/// Minimum recall quality a reconsolidation triggeréhez.
const MIN_QUALITY: u8 = 3;
/// Ha ennyi idő (ms) óta nem volt aktiválva a blokk, nagyobb a drift.
const STALE_THRESHOLD_MS: u64 = 86_400_000 * 3; // 3 days

// ─── Reconsolidation ───────────────────────────────

/// Reconsolidate emotion vectors of recalled blocks.
/// Minden activated blokk emotion vektorát blend-eli a query emotion-nal,
/// ha a hasonlóság még nem túl magas.
pub fn reconsolidate_emotions(
    output_dir: &Path,
    reader: &MicroscopeReader,
    _query: &str,
    query_emotion: Option<&[f32; 21]>,
    activated: &[(u32, f32)],
) -> u32 {
    let qe = match query_emotion {
        Some(e) => e,
        None => {
            // Ha nincs explicit query emotion, próbáljuk az EmotionalStateRing-et
            let ring = crate::emotional_state::EmotionalStateRing::load_or_init(output_dir);
            if ring.is_active() {
                // Use a small allocation to hold the reference... just use the ring.current
                let current: [f32; 21] = ring.current;
                return reconsolidate_emotions_inner(output_dir, reader, &current, activated);
            }
            return 0;
        }
    };
    reconsolidate_emotions_inner(output_dir, reader, qe, activated)
}

fn reconsolidate_emotions_inner(
    output_dir: &Path,
    reader: &MicroscopeReader,
    query_emotion: &[f32; 21],
    activated: &[(u32, f32)],
) -> u32 {
    let lookup = match load_emotion_lookup(output_dir) {
        Some(l) => l,
        None => return 0,
    };
    let emotions_path = output_dir.join("emotions.bin");
    let mut blended = 0u32;

    // Csak a main index blokkokat reconsolidáljuk (append log entry-ket nem)
    let block_count = reader.block_count;

    for &(idx, _) in activated {
        let idx_usize = idx as usize;
        if idx_usize >= block_count {
            continue;
        }

        // Olvasd ki a jelenlegi emotion vektort
        let current_emo = lookup(idx_usize).unwrap_or([0.0f32; 21]);

        // Ha már nagyon hasonló, hagyd
        let sim = emotional_similarity(query_emotion, &current_emo);
        if sim > MAX_EMO_SIM_FOR_RECONSOLIDATION {
            continue;
        }

        // Blend: current * (1 - rate) + query * rate
        let rate = EMOTION_BLEND_RATE * (1.0 - sim); // minél kevésbé hasonló, annál többet blend-el
        let mut new_emo = [0.0f32; 21];
        for i in 0..21 {
            new_emo[i] = current_emo[i] * (1.0 - rate) + query_emotion[i] * rate;
        }

        // Normalizáljuk: max érték ne haladja meg az 1.0-t
        let max_val = new_emo.iter().cloned().fold(0.0f32, f32::max);
        if max_val > 1.0 {
            for v in &mut new_emo {
                *v /= max_val;
            }
        }

        if write_emotion(&emotions_path, idx_usize, &new_emo).is_err() {
            continue;
        }
        blended += 1;
    }

    blended
}

/// Spatial reconsolidation: drift blocks toward query coordinates.
/// Minden activated blokk koordinátáit húzza a query térbeli pozíciója felé,
/// a HebbianState drift mezőin keresztül (a drift a következő build-nél
/// beégetődik a header-ekbe).
pub fn reconsolidate_spatial(
    output_dir: &Path,
    reader: &MicroscopeReader,
    query: &str,
    config: &crate::config::Config,
    activated: &[(u32, f32)],
) -> u32 {
    let sw = config.search.semantic_weight;
    let (qx, qy, qz) = content_coords_blended(query, "long_term", sw);
    let block_count = reader.block_count;

    // Load Hebbian state
    let mut hebb = crate::hebbian::HebbianState::load_or_init(output_dir, block_count);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut drifted = 0u32;

    for &(idx, _) in activated {
        let idx_usize = idx as usize;
        if idx_usize >= block_count {
            continue;
        }

        let hdr = reader.header(idx_usize);
        let dx = qx - hdr.x;
        let dy = qy - hdr.y;
        let dz = qz - hdr.z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        // Ha túl közel van, nem kell drift
        if dist < 0.001 {
            continue;
        }

        // Minél távolabbi, annál nagyobb a drift
        let recency_factor = if idx_usize < hebb.activations.len() {
            let age = now_ms.saturating_sub(hebb.activations[idx_usize].last_activated_ms);
            if age > STALE_THRESHOLD_MS {
                2.0 // régi memória → nagyobb drift
            } else {
                1.0
            }
        } else {
            1.0
        };

        let rate = SPATIAL_DRIFT_RATE * dist * recency_factor;
        let rate = rate.min(0.1); // max 10% egy alkalommal (nem akarjuk destabilizálni)

        if idx_usize < hebb.activations.len() {
            hebb.activations[idx_usize].drift_x += dx * rate;
            hebb.activations[idx_usize].drift_y += dy * rate;
            hebb.activations[idx_usize].drift_z += dz * rate;
            drifted += 1;
        }
    }

    if drifted > 0 {
        let _ = hebb.save(output_dir);
    }

    drifted
}

/// Full reconsolidation: emotion + spatial, minden activated blokkra.
/// Visszaadja a rekonszolidált blokkok számát (emotion + spatial együtt).
pub fn reconsolidate(
    output_dir: &Path,
    reader: &MicroscopeReader,
    query: &str,
    query_emotion: Option<&[f32; 21]>,
    config: &crate::config::Config,
    quality: u8,
    activated: &[(u32, f32)],
) -> (u32, u32) {
    if quality < MIN_QUALITY || activated.is_empty() {
        return (0, 0);
    }

    let emo_count = reconsolidate_emotions(output_dir, reader, query, query_emotion, activated);
    let spatial_count = reconsolidate_spatial(output_dir, reader, query, config, activated);

    (emo_count, spatial_count)
}

/// Formázás CLI kimenethez (színek nélkül, main.rs-ben lesz színezve).
pub fn format_reconsolidation(emo: u32, spatial: u32) -> String {
    if emo == 0 && spatial == 0 {
        String::new()
    } else {
        format!("  RECONSOLIDATED emotion={} spatial={}", emo, spatial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emotion_blend_basic() {
        let qe = [
            0.9f32, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
        ];
        let ce = [
            0.1f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.8, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
        ]; // anxiety=0.8
        let sim = emotional_similarity(&qe, &ce);
        assert!(sim < MAX_EMO_SIM_FOR_RECONSOLIDATION); // joy vs anxiety → low sim

        let rate = EMOTION_BLEND_RATE * (1.0 - sim);
        let mut blended = [0.0f32; 21];
        for i in 0..21 {
            blended[i] = ce[i] * (1.0 - rate) + qe[i] * rate;
        }
        // blended[0] should be > 0.1 (joy blended in)
        assert!(blended[0] > 0.1);
        // blended[13] (anxiety) should be < 0.8 (diluted)
        assert!(blended[13] < ce[13]);
    }

    #[test]
    fn test_high_similarity_skips_blend() {
        let qe = [0.8f32; 21];
        let ce = [0.78f32; 21];
        let sim = emotional_similarity(&qe, &ce);
        assert!(sim > MAX_EMO_SIM_FOR_RECONSOLIDATION); // should be very close
    }
}
