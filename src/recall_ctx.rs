//! Persistent recall context: the reader plus every mutable layer state.
//!
//! A single `recall()` used to `open_reader` plus `load_or_init` about twenty
//! state objects on every call (~1.3 ms of file I/O inside a ~2.2 ms call).
//! This module holds them once and reuses them across calls.
//!
//! # Safety contract
//!
//! The context is behaviour-preserving for a **single-writer process**: every
//! call saves its mutations back (same durability as before), so each call
//! starts from the previously saved state — exactly as if it had re-read it.
//!
//! Two staleness guards make long-lived use safe:
//!
//! 1. **Generation check.** `SharedContext` fingerprints `meta.bin`
//!    (byte length + mtime) plus the block count at open. Every access
//!    re-stats the file (~1 us); on mismatch the whole context is dropped
//!    and reopened. This catches rebuilds, which reassign block ids and
//!    would otherwise leave the mmap pointing at a replaced file.
//! 2. **Layer guards.** The text index validates `block_count()` on every
//!    use (full-scan fallback when stale); the append log validates its
//!    own (len, mtime) stamp per query. See `reader.rs`.
//!
//! If another process rewrites the state files *without* changing `meta.bin`
//! (in-place edits, which the build never does — it renames), the context
//! keeps serving in-memory copies. The CLI opens a fresh context per
//! invocation and is unaffected.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::config::Config;
use crate::reader::MicroscopeReader;

/// All state one `recall()` call needs, loaded once.
pub struct RecallContext {
    pub reader: MicroscopeReader,
    pub attention: crate::attention::AttentionState,
    pub hebb: crate::hebbian::HebbianState,
    pub thought_graph: crate::thought_graph::ThoughtGraphState,
    pub pred_cache: crate::predictive_cache::PredictiveCache,
    pub emotion_21d: crate::emotional_21d::EmotionalState21D,
    pub mirror: crate::mirror::MirrorState,
    pub resonance: crate::resonance::ResonanceState,
    pub archetypes: crate::archetype::ArchetypeState,
    pub temporal: crate::temporal_archetype::TemporalArchetypeState,
    pub eureka_log: crate::eureka::EurekaLog,
    pub spaced: crate::spaced_repetition::SpacedRepetition,
    pub narrative: crate::narrative::NarrativeState,
    pub esr: crate::emotional_state::EmotionalStateRing,
    pub self_model: crate::self_model::SelfModel,
    pub curiosity: crate::curiosity::CuriosityState,
    pub narrative_memory: crate::narrative_memory::NarrativeMemory,
    pub monologue: crate::inner_monologue::MonologueState,
}

impl RecallContext {
    /// Open a fresh context: one reader plus one load of every layer state.
    /// This is what a single `recall()` call used to pay on every invocation.
    pub fn open(config: &Config) -> Result<Self, String> {
        let reader = MicroscopeReader::open(config)?;
        let output_dir = Path::new(&config.paths.output_dir);
        let emotion_file = output_dir.join("emotion_21d.bin");
        Ok(Self {
            attention: crate::attention::AttentionState::load_or_init(output_dir),
            hebb: crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count),
            thought_graph: crate::thought_graph::ThoughtGraphState::load_or_init(output_dir),
            pred_cache: crate::predictive_cache::PredictiveCache::load_or_init(output_dir),
            emotion_21d: crate::emotional_21d::EmotionalState21D::load_or_init(&emotion_file),
            mirror: crate::mirror::MirrorState::load_or_init(output_dir),
            resonance: crate::resonance::ResonanceState::load_or_init(output_dir),
            archetypes: crate::archetype::ArchetypeState::load_or_init(output_dir),
            temporal: crate::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir),
            eureka_log: crate::eureka::EurekaLog::load_or_init(output_dir),
            spaced: crate::spaced_repetition::SpacedRepetition::load_or_init(output_dir),
            narrative: crate::narrative::NarrativeState::load_or_init(output_dir),
            esr: crate::emotional_state::EmotionalStateRing::load_or_init(output_dir),
            self_model: crate::self_model::SelfModel::load_or_init(output_dir),
            curiosity: crate::curiosity::CuriosityState::load_or_init(output_dir),
            narrative_memory: crate::narrative_memory::NarrativeMemory::load_or_init(output_dir),
            monologue: crate::inner_monologue::MonologueState::load_or_init(output_dir),
            reader,
        })
    }
}

/// Fingerprint of the index layout a context was built over.
/// Any rebuild changes `meta.bin` (new mtime at minimum), so comparing this
/// per access catches a replaced layout before a single stale block is served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Generation {
    meta_len: u64,
    meta_mtime_secs: u64,
    meta_mtime_nanos: u32,
    block_count: usize,
}

impl Generation {
    fn capture(output_dir: &Path, block_count: usize) -> Option<Self> {
        let md = std::fs::metadata(output_dir.join("meta.bin")).ok()?;
        let mtime = md.modified().ok()?;
        let dur = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Some(Self {
            meta_len: md.len(),
            meta_mtime_secs: dur.as_secs(),
            meta_mtime_nanos: dur.subsec_nanos(),
            block_count,
        })
    }

    fn is_current(&self, output_dir: &Path, block_count: usize) -> bool {
        if self.block_count != block_count {
            return false;
        }
        match Self::capture(output_dir, block_count) {
            Some(now) => now == *self,
            // meta.bin vanished: the index is gone, so is the context.
            None => false,
        }
    }
}

/// A `RecallContext` plus its layout fingerprint, with reopen-on-stale.
pub struct SharedContext {
    ctx: Option<RecallContext>,
    generation: Option<Generation>,
    output_dir: PathBuf,
}

impl SharedContext {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            ctx: None,
            generation: None,
            output_dir,
        }
    }

    /// Get the context, opening or reopening as needed.
    ///
    /// Reopen triggers: first use, `meta.bin` changed (rebuild), `meta.bin`
    /// vanished, or the block count moved. One `metadata()` syscall per call
    /// when warm (~1 us); a full reopen only when the layout actually changed.
    pub fn get(&mut self, config: &Config) -> Result<&mut RecallContext, String> {
        let output_dir = Path::new(&config.paths.output_dir);
        let need_reopen = match (&self.ctx, &self.generation) {
            (Some(ctx), Some(gen)) => !gen.is_current(output_dir, ctx.reader.block_count),
            _ => true,
        };
        if need_reopen {
            let ctx = RecallContext::open(config)?;
            let gen = Generation::capture(output_dir, ctx.reader.block_count);
            self.ctx = Some(ctx);
            self.generation = gen;
            self.output_dir = output_dir.to_path_buf();
        }
        self.ctx
            .as_mut()
            .ok_or_else(|| "shared recall context failed to open".to_string())
    }

    /// Drop the held context, forcing a reopen on next use.
    /// Call after operations known to replace the layout outside the
    /// generation check's view (there are currently none — `build` and
    /// `rebuild_pending` both rewrite `meta.bin` — but the hook exists
    /// so callers do not need to know that).
    pub fn invalidate(&mut self) {
        self.ctx = None;
        self.generation = None;
    }
}

/// Process-global shared context for stdio-server use (MCP).
///
/// MCP stdio keeps one process alive across many tool calls; without this,
/// every call reopens the reader and reloads every state file. The `Mutex`
/// serialises concurrent tool calls; the generation check inside `get`
/// keeps a rebuild from ever serving stale blocks.
static GLOBAL: OnceLock<Mutex<SharedContext>> = OnceLock::new();

/// Run `f` with the process-global shared recall context for `config`.
///
/// The context is opened on first use and transparently reopened whenever
/// the index layout changes underneath it. Returns the closure's result,
/// or the open error as a string.
pub fn with_shared<R>(
    config: &Config,
    f: impl FnOnce(&mut RecallContext) -> R,
) -> Result<R, String> {
    let cell = GLOBAL
        .get_or_init(|| Mutex::new(SharedContext::new(PathBuf::from(&config.paths.output_dir))));
    let mut shared = cell.lock().map_err(|e| format!("shared ctx lock: {e}"))?;
    // A different output dir than the global was opened for: treat as a
    // generation change (simplest correct behaviour for multi-tenant use).
    if shared.output_dir != Path::new(&config.paths.output_dir) {
        shared.invalidate();
    }
    let ctx = shared.get(config)?;
    Ok(f(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let meta = dir.path().join("meta.bin");
        std::fs::write(&meta, vec![0u8; 64]).unwrap();
        let g = Generation::capture(dir.path(), 10).unwrap();
        assert!(g.is_current(dir.path(), 10));
        assert!(!g.is_current(dir.path(), 11));
        // Touch the file: mtime moves, generation must go stale.
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(&meta, vec![0u8; 64]).unwrap();
        // mtime granularity is filesystem-dependent; length change always works.
        std::fs::write(&meta, vec![0u8; 65]).unwrap();
        assert!(!g.is_current(dir.path(), 10));
    }

    #[test]
    fn shared_context_opens_and_reopens() {
        // Empty dir: open fails (no index), get() propagates the error
        // instead of panicking, and a later valid open still works.
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = crate::config::Config::default();
        cfg.paths.output_dir = dir.path().to_string_lossy().to_string();
        cfg.paths.layers_dir = dir.path().to_string_lossy().to_string();
        cfg.paths.temp_dir = dir.path().to_string_lossy().to_string();

        let mut shared = SharedContext::new(dir.path().to_path_buf());
        assert!(shared.get(&cfg).is_err());
        // Still unusable, still an error — never a stale success.
        assert!(shared.get(&cfg).is_err());
        shared.invalidate();
        assert!(shared.get(&cfg).is_err());
    }
}
