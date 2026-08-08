//! Canonical memory store pipeline.
//!
//! One unified path for ALL memory store entrypoints:
//!   - Microscope CLI
//!   - Hope CLI
//!   - Hope server
//!   - Bridge
//!   - Future agent calls
//!
//! Pipeline steps:
//!   1. Normalize input
//!   2. Structural emotion extraction
//!   3. Persistence (append log + layer file)
//!   4. Timeline log
//!   5. Emotion log
//!   6. Auto-rebuild check
//!
//! This eliminates the architectural drift where different entrypoints
//! had different store semantics (some with emotion, some without).

use crate::config::Config;
use crate::emotion_extraction::extract_emotion;
use crate::reader::store_memory_with_status;

/// Canonical store pipeline result.
#[derive(Debug, Clone)]
pub struct StoreResult {
    pub stored: bool,
    pub emotion_extracted: bool,
    pub emotion_event: String,
    pub message: String,
}

/// The single canonical memory store pipeline.
///
/// All entrypoints MUST use this function instead of calling
/// `store_memory` or `store_memory_with_emotion` directly.
///
/// This ensures:
/// - Emotion extraction always runs
/// - Persistence semantics are identical
/// - Timeline logging is consistent
/// - No silent omission of emotion data
pub fn store_memory_pipeline(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
) -> Result<StoreResult, String> {
    // 1. Normalize: trim, ensure non-empty
    let normalized = text.trim();
    if normalized.is_empty() {
        return Err("Cannot store empty text".to_string());
    }

    let imp = importance.clamp(1, 10);

    // 2. Structural emotion extraction
    let extraction = extract_emotion(normalized);
    let emotion_vector = extraction.pad.to_21d();
    let emotion_event = format!("{:?}", extraction.event);

    // 3-6. Persistence + timeline + emotion log + auto-rebuild
    store_memory_with_status(config, normalized, layer, imp, None, Some(emotion_vector))?;

    Ok(StoreResult {
        stored: true,
        emotion_extracted: true,
        emotion_event: emotion_event.clone(),
        message: format!(
            "STORED | layer: {} | importance: {} | emotion: {}",
            layer, imp, emotion_event
        ),
    })
}

/// Pipeline with status (for open loops, resolved, archived).
pub fn store_memory_pipeline_with_status(
    config: &Config,
    text: &str,
    layer: &str,
    importance: u8,
    status: Option<&str>,
) -> Result<StoreResult, String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return Err("Cannot store empty text".to_string());
    }

    let imp = importance.clamp(1, 10);

    let extraction = extract_emotion(normalized);
    let emotion_vector = extraction.pad.to_21d();
    let emotion_event = format!("{:?}", extraction.event);

    store_memory_with_status(config, normalized, layer, imp, status, Some(emotion_vector))?;

    Ok(StoreResult {
        stored: true,
        emotion_extracted: true,
        emotion_event: emotion_event.clone(),
        message: format!(
            "STORED | layer: {} | importance: {} | status: {} | emotion: {}",
            layer,
            imp,
            status.unwrap_or("normal"),
            emotion_event
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        let dir = std::env::temp_dir().join("hope-pipeline-test");
        std::fs::create_dir_all(&dir).ok();
        let mut cfg = Config::default();
        cfg.paths.output_dir = dir.to_string_lossy().to_string();
        cfg.paths.temp_dir = dir.join("tmp").to_string_lossy().to_string();
        cfg
    }

    #[test]
    fn test_pipeline_stores_with_emotion() {
        let cfg = test_config();
        let result = store_memory_pipeline(
            &cfg,
            "Sikeresen lefordult a projekt, minden teszt zöld!",
            "session",
            6,
        );
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.stored);
        assert!(r.emotion_extracted);
        assert!(!r.emotion_event.is_empty());
    }

    #[test]
    fn test_pipeline_rejects_empty() {
        let cfg = test_config();
        assert!(store_memory_pipeline(&cfg, "", "session", 5).is_err());
        assert!(store_memory_pipeline(&cfg, "   ", "session", 5).is_err());
    }

    #[test]
    fn test_pipeline_clamps_importance() {
        let cfg = test_config();
        let r = store_memory_pipeline(&cfg, "test text", "session", 0).unwrap();
        assert!(r.message.contains("importance: 1"));
        let r2 = store_memory_pipeline(&cfg, "test text 2", "session", 255).unwrap();
        assert!(r2.message.contains("importance: 10"));
    }
}
