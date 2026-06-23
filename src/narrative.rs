//! Belső Narratíva — a rendszer saját magának mesél.
//!
//! Minden interakció után a rendszer összegyűjti a jelenlegi állapotát
//! (emotion, fókusz, esedékes ismétlések, gondolati útvonalak) és
//! egy mondatban "elmeséli magának", hogy mi történt.
//!
//! Ez a mondat a következő interakció priming-jaként szolgál:
//! a rendszer nem csak egy memória adatbázis, hanem van egy
//! folyamatos, önreflexív "belső hangja".
//!
//! Binary format: NAR1
//!   magic: "NAR1" (4 bytes)
//!   timestamp_ms: u64 (8 bytes)
//!   session_count: u64 (8 bytes)
//!   emotion: [f32; 21] (84 bytes)
//!   narrative_len: u16 (2 bytes) + narrative bytes (UTF-8)
//!
//! Minden interakció után frissül. Minimum példányszám: 1.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::reader::EMOTION_DIMS;

// ─── Constants ──────────────────────────────────────

/// Minimum emotional intensity to register as "felt".
const FEELING_THRESHOLD: f32 = 0.1;
/// Max fókusz téma hossza a narratívában.
const MAX_FOCUS_LEN: usize = 40;

// ─── NarrativeState ────────────────────────────────

/// A rendszer belső narratívája — egy snapshot arról, "ki most ő".
pub struct NarrativeState {
    /// Mikor frissült utoljára.
    pub last_update_ms: u64,
    /// Hány interakciója volt életében.
    pub session_count: u64,
    /// Aktuális érzelmi állapot.
    pub emotion: [f32; 21],
    /// A legutóbbi narratíva mondat.
    pub narrative: String,
}

impl NarrativeState {
    /// Betöltés NAR1 file-ból, vagy friss generálás a jelenlegi állapotból.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("narrative.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 102 && &data[0..4] == b"NAR1" {
                let ts = u64::from_le_bytes(data[4..12].try_into().unwrap_or([0; 8]));
                let count = u64::from_le_bytes(data[12..20].try_into().unwrap_or([0; 8]));
                let mut emotion = [0.0f32; 21];
                for (i, e) in emotion.iter_mut().enumerate() {
                    let off = 20 + i * 4;
                    *e = f32::from_le_bytes(data[off..off + 4].try_into().unwrap_or([0u8; 4]));
                }
                let nlen = u16::from_le_bytes(data[104..106].try_into().unwrap_or([0; 2])) as usize;
                let narrative = if 106 + nlen <= data.len() {
                    String::from_utf8_lossy(&data[106..106 + nlen]).to_string()
                } else {
                    String::new()
                };
                return NarrativeState {
                    last_update_ms: ts,
                    session_count: count,
                    emotion,
                    narrative,
                };
            }
        }
        // Empty narrative: the system hasn't "spoken" yet.
        NarrativeState {
            last_update_ms: 0,
            session_count: 0,
            emotion: [0.0f32; 21],
            narrative: String::new(),
        }
    }

    /// Mentés NAR1 formátumba.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("narrative.bin");
        let nbytes = self.narrative.as_bytes();
        let nlen = nbytes.len().min(4096) as u16;
        let mut buf = Vec::with_capacity(106 + nlen as usize);
        buf.extend_from_slice(b"NAR1");
        buf.extend_from_slice(&self.last_update_ms.to_le_bytes());
        buf.extend_from_slice(&self.session_count.to_le_bytes());
        for v in &self.emotion {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&nlen.to_le_bytes());
        buf.extend_from_slice(&nbytes[..nlen as usize]);
        let tmp_path = output_dir.join("narrative.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write narrative.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename narrative.bin: {}", e))
    }

    /// Frissíti a narratívát a jelenlegi rendszerállapotból.
    /// Ezt kell minden interakció után hívni.
    pub fn update(
        &mut self,
        output_dir: &Path,
        esr: Option<&crate::emotional_state::EmotionalStateRing>,
        wm_items: Option<&[String]>,
        due_count: Option<usize>,
        thought_count: Option<usize>,
        query: Option<&str>,
    ) -> Result<(), String> {
        self.last_update_ms = now_ms();
        self.session_count += 1;

        // 1. Érzelem: ha van ESR, vedd át
        if let Some(ring) = esr {
            if ring.is_active() {
                self.emotion = ring.current;
            }
        }

        // 2. Generate the narrative sentence
        self.narrative = build_narrative(&self.emotion, wm_items, due_count, thought_count, query);

        self.save(output_dir)
    }
}

/// Meta-kognitív rekonszolidáció: a narratíva visszaírása a memóriába.
/// Minden generált narratív mondatot elmentünk store_memory-val,
/// [MetaCognitive] prefix-szel, hogy a rendszer később emlékezzen rá,
/// miről gondolkodott.
pub fn metacognitive_store(output_dir: &Path, narrative: &str, emotion: &[f32; 21]) {
    // Nem tároljuk a csendes vagy üres narratívákat
    if narrative.is_empty() || narrative == "I am silent." {
        return;
    }
    // A store_memory-hoz config kell, de itt nincs. Használjuk az emotion_log-ot
    // és a persist_to_layer_file-t, hogy a rebuild túlélje.
    let layer = "meta_cognitive";
    let layer_path = output_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("layers").join(format!("{}.txt", layer)))
        .unwrap_or_else(|| {
            output_dir
                .join("..")
                .join("layers")
                .join(format!("{}.txt", layer))
        });

    // Write directly to layer file (best-effort)
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&layer_path)
    {
        use std::io::Write;
        let ts = now_ms();
        let line = format!("[MetaCognitive] [{}] {}\n", ts, narrative);
        let _ = file.write_all(line.as_bytes());
    }

    // Also write emotion to emotion_log for rebuild survival
    let _ = crate::reader::append_emotion_log(
        output_dir,
        &format!("[MetaCognitive] {}", narrative),
        emotion,
    );
}

// ─── Narrative generation ──────────────────────────

/// Megépíti a "belső monológ" mondatot a jelenlegi állapotból.
fn build_narrative(
    emotion: &[f32; 21],
    wm_items: Option<&[String]>,
    due_count: Option<usize>,
    thought_count: Option<usize>,
    query: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Érzelem rész
    let felt = felt_emotions(emotion, 2);
    if !felt.is_empty() {
        parts.push(format!("I feel {}", felt));
    }

    // Fókusz rész (working memory)
    if let Some(items) = wm_items {
        if !items.is_empty() {
            let foci: Vec<&str> = items
                .iter()
                .take(2)
                .map(|s| {
                    let s = s.trim();
                    if s.len() > MAX_FOCUS_LEN {
                        &s[..s.floor_char_boundary(MAX_FOCUS_LEN)]
                    } else {
                        s
                    }
                })
                .collect();
            parts.push(format!("focused on {}", foci.join(" and ")));
        }
    }

    // Esedékes ismétlések
    if let Some(due) = due_count {
        if due > 0 {
            parts.push(format!("{} memories need review", due));
        }
    }

    // Gondolati aktivitás
    if let Some(tc) = thought_count {
        if tc > 0 {
            parts.push(format!("{} recall paths active", tc));
        }
    }

    // Aktuális query (ha van)
    if let Some(q) = query {
        let q = q.trim();
        if !q.is_empty() {
            let short = crate::safe_truncate(q, 30);
            parts.push(format!("exploring '{}'", short));
        }
    }

    if parts.is_empty() {
        "I am silent.".to_string()
    } else {
        parts.join(". ") + "."
    }
}

/// Legdominánsabb érzelmi dimenziók szövegesen.
fn felt_emotions(emotion: &[f32; 21], max: usize) -> String {
    let mut pairs: Vec<(usize, f32)> = emotion.iter().enumerate().map(|(i, &v)| (i, v)).collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let felt: Vec<String> = pairs
        .into_iter()
        .take(max)
        .filter(|(_, v)| *v > FEELING_THRESHOLD)
        .map(|(i, v)| {
            let name = EMOTION_DIMS.get(i).unwrap_or(&"?");
            let intensity = if v > 0.6 {
                "strongly "
            } else if v > 0.3 {
                ""
            } else {
                "slightly "
            };
            format!("{}{}", intensity, name)
        })
        .collect();

    felt.join(" and ")
}

/// A narrative használata recall priming-hoz.
/// Visszaadja a narrative emotion-ját (használható boost-ként a recall-ban).
pub fn narrative_prime(state: &NarrativeState) -> Option<[f32; 21]> {
    if state.session_count == 0 {
        return None;
    }
    let intensity: f32 = state.emotion.iter().map(|x| x * x).sum::<f32>().sqrt();
    if intensity < FEELING_THRESHOLD {
        return None;
    }
    Some(state.emotion)
}

/// A narrative szöveges reprezentációja.
pub fn narrative_display(state: &NarrativeState) -> String {
    if state.session_count == 0 {
        "I have not yet spoken.".to_string()
    } else {
        format!(
            "{} [{}] {}",
            "NARRATIVE".cyan().bold(),
            state.session_count,
            state.narrative
        )
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

use colored::Colorize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_narrative() {
        let state = NarrativeState::load_or_init(Path::new("."));
        assert_eq!(state.session_count, 0);
        assert!(state.narrative.is_empty());
    }

    #[test]
    fn test_build_narrative_no_state() {
        let emotion = [0.0f32; 21];
        let s = build_narrative(&emotion, None, None, None, None);
        assert_eq!(s, "I am silent.");
    }

    #[test]
    fn test_build_narrative_with_emotion() {
        let mut emotion = [0.0f32; 21];
        emotion[0] = 0.9; // joy
        emotion[7] = 0.6; // anticipation
        let s = build_narrative(
            &emotion,
            Some(["hello".to_string()].as_slice()),
            Some(3),
            Some(5),
            Some("test query"),
        );
        assert!(s.contains("joy"));
        assert!(s.contains("hello"));
        assert!(s.contains("test"));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir();
        let state = NarrativeState {
            last_update_ms: 1000,
            session_count: 42,
            emotion: [0.5f32; 21],
            narrative: "I feel curiosity, exploring the world.".to_string(),
        };
        state.save(&dir).unwrap();

        let loaded = NarrativeState::load_or_init(&dir);
        assert_eq!(loaded.session_count, 42);
        assert_eq!(loaded.narrative, "I feel curiosity, exploring the world.");

        let _ = std::fs::remove_file(dir.join("narrative.bin"));
    }

    #[test]
    fn test_narrative_prime_returns_emotion_when_active() {
        let state = NarrativeState {
            last_update_ms: 1000,
            session_count: 10,
            emotion: [0.5f32; 21],
            narrative: "test".to_string(),
        };
        let prime = narrative_prime(&state);
        assert!(prime.is_some());
    }

    #[test]
    fn test_narrative_prime_returns_none_when_silent() {
        let state = NarrativeState::load_or_init(Path::new("."));
        let prime = narrative_prime(&state);
        assert!(prime.is_none());
    }
}
