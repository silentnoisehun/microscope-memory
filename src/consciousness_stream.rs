//! Live Consciousness Stream — folyamatos tudatstream.
//!
//! A 13 consciousness réteg nem per-query fut, hanem folyamatosan,
//! egy háttérszálon. Minden 100ms-ben:
//!   - Hebbian decay + resonance field decay
//!   - Emotional drift
//!   - Predictive forward model (mit fog kérdezni legközelebb?)
//!   - Curiosity generálás
//!   - Surprise signal (ha a predikció eltér a valóságtól)
//!
//! A recall hot path innen olvassa az állapotot, nem fájlból.
//! Ez ~10 file I/O-t spórol meg query-nként.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Global stream state — ha fut, a recall innen olvas, nem fájlból.
static GLOBAL_STREAM: OnceLock<Arc<Mutex<StreamState>>> = OnceLock::new();

use crate::archetype::ArchetypeState;
use crate::attention::{AttentionSignals, AttentionState, AttentionVector};
use crate::config::Config;
use crate::consciousness_seqlock::SharedSnapshot;
use crate::emotional_state::EmotionalStateRing;
use crate::hebbian::HebbianState;
use crate::mirror::MirrorState;
use crate::predictive_cache::PredictiveCache;
use crate::resonance::ResonanceState;
use crate::thought_graph::ThoughtGraphState;

// ─── Constants ──────────────────────────────────────

/// Stream cycle interval (100ms = 10 Hz — agyi theta tartomány)
const CYCLE_MS: u64 = 100;
/// Hány ciklus után jöjjön a curiosity generálás
const CURIOSITY_INTERVAL: u64 = 50; // 5 másodperc
/// Hány ciklus után jöjjön a predictive forward model
const PREDICT_INTERVAL: u64 = 10; // 1 másodperc
/// Hány ciklus után jöjjön a decay
const DECAY_INTERVAL: u64 = 10; // 1 másodperc
/// Surprise threshold: ha a predikció és a valóság eltérése > ennyi
const SURPRISE_THRESHOLD: f32 = 0.3;

// ─── StreamState ───────────────────────────────────

/// A consciousness stream aktuális állapota — minden réteg in-memory.
pub struct StreamState {
    pub hebbian: HebbianState,
    pub attention: AttentionState,
    pub emotional_ring: EmotionalStateRing,
    pub resonance: ResonanceState,
    pub thought_graph: ThoughtGraphState,
    pub predictive_cache: PredictiveCache,
    pub archetypes: ArchetypeState,
    pub mirror: MirrorState,

    /// Current prediction: "what will the next query be?"
    pub predicted_query_hash: u64,
    pub predicted_confidence: f32,
    /// Surprise level (0.0 = boring, 1.0 = mind-blowing)
    pub surprise_level: f32,
    /// Current curiosity level
    pub curiosity_level: f32,
    /// Cycle counter
    pub cycle: u64,
    /// Last query hash that was actually received
    pub last_query_hash: u64,
    /// Timestamp of last query
    pub last_query_ms: u64,

    /// Pre-computed sum of `hebbian.activations[*].energy`. Maintained
    /// incrementally by the background cycle so `format()` doesn't have
    /// to iterate 28k+ f32s on every read. Updated in O(1) per decay.
    pub hebbian_total_energy: f32,

    /// Lock-free, mmap-able view of the stream. The background cycle
    /// publishes a new snapshot once per tick via a seqlock; readers
    /// (this process or another) can read it without taking the Mutex.
    /// Lives as `Arc<SharedSnapshot>` so the seqlock address is stable
    /// even if the StreamState Mutex is held by a writer.
    pub snapshot: Arc<SharedSnapshot>,
}

/// Thread-safe wrapper
pub struct ConsciousnessStream {
    pub state: Arc<Mutex<StreamState>>,
    running: Arc<Mutex<bool>>,
}

/// Get the global stream state (if running).
pub fn global_stream() -> Option<&'static Arc<Mutex<StreamState>>> {
    GLOBAL_STREAM.get()
}

impl ConsciousnessStream {
    /// Start the stream in a background thread.
    /// Sets the global stream state so recall can use it.
    pub fn start(config: &Config) -> Arc<Mutex<StreamState>> {
        let output_dir = Path::new(&config.paths.output_dir);
        let block_count = crate::reader::MicroscopeReader::open(config)
            .map(|r| r.block_count)
            .unwrap_or(100);

        let hebbian = HebbianState::load_or_init(output_dir, block_count);
        // Initial total: sum whatever was loaded from disk. Subsequent
        // decay updates keep this in sync via the background cycle.
        let hebbian_total_energy: f32 = hebbian.activations.iter().map(|a| a.energy).sum();

        let snapshot = Arc::new(SharedSnapshot::new_zeroed());

        let state = Arc::new(Mutex::new(StreamState {
            hebbian,
            attention: AttentionState::load_or_init(output_dir),
            emotional_ring: EmotionalStateRing::load_or_init(output_dir),
            resonance: ResonanceState::load_or_init(output_dir),
            thought_graph: ThoughtGraphState::load_or_init(output_dir),
            predictive_cache: PredictiveCache::load_or_init(output_dir),
            archetypes: ArchetypeState::load_or_init(output_dir),
            mirror: MirrorState::load_or_init(output_dir),
            predicted_query_hash: 0,
            predicted_confidence: 0.0,
            surprise_level: 0.0,
            curiosity_level: 0.0,
            cycle: 0,
            last_query_hash: 0,
            last_query_ms: 0,
            hebbian_total_energy,
            snapshot: snapshot.clone(),
        }));

        // Set global stream state
        let _ = GLOBAL_STREAM.set(state.clone());

        let state_clone = state.clone();
        let running = Arc::new(Mutex::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            let mut curiosity_counter = 0u64;
            let mut predict_counter = 0u64;
            let mut decay_counter = 0u64;

            loop {
                if !*running_clone.lock().unwrap() {
                    break;
                }

                let mut s = state_clone.lock().unwrap();
                s.cycle += 1;
                curiosity_counter += 1;
                predict_counter += 1;
                decay_counter += 1;

                // ─── Decay (1 másodpercenként) ───
                if decay_counter >= DECAY_INTERVAL {
                    decay_counter = 0;
                    // Hebbian decay: csökkentjük az energiákat, és a
                    // pre-computed total_energy-t O(1)-ben frissítjük.
                    // Az egyetlen O(n) lépés kikerül a read path-ból.
                    const DECAY_FACTOR: f32 = 0.995_f32;
                    // 0.995^DECAY_INTERVAL — egyszer kiszámítjuk, O(1) update
                    let decay_pow: f32 =
                        (0..DECAY_INTERVAL as i32).fold(1.0_f32, |acc, _| acc * DECAY_FACTOR);
                    s.hebbian_total_energy *= decay_pow;
                    for rec in &mut s.hebbian.activations {
                        rec.energy *= DECAY_FACTOR;
                    }
                    s.resonance.decay_field(0.99);
                    s.resonance.expire_pulses();
                    s.mirror.decay();
                    s.archetypes.decay();
                }

                // ─── Emotional drift ───
                s.emotional_ring.decay();

                // ─── Predictive forward model (1 másodpercenként) ───
                if predict_counter >= PREDICT_INTERVAL {
                    predict_counter = 0;
                    let tg = s.thought_graph.clone();
                    s.predictive_cache.predict_next(&tg);
                    // Update avg confidence
                    if !s.predictive_cache.predictions.is_empty() {
                        s.predicted_confidence = s.predictive_cache.stats.avg_confidence;
                    }
                }

                // ─── Curiosity (5 másodpercenként) ───
                if curiosity_counter >= CURIOSITY_INTERVAL {
                    curiosity_counter = 0;
                    // Curiosity = f(prediction uncertainty, surprise decay, emotional intensity)
                    let emo_intensity = s.emotional_ring.intensity();
                    let pred_uncertainty = 1.0 - s.predicted_confidence;
                    let surprise_decay = s.surprise_level * 0.95; // slow decay
                    s.curiosity_level =
                        (emo_intensity * 0.3 + pred_uncertainty * 0.4 + surprise_decay * 0.3)
                            .clamp(0.0, 1.0);
                }

                // ─── Publish snapshot (minden ciklusban, lock-free olvasók számára) ───
                publish_snapshot(&s);

                drop(s);
                thread::sleep(Duration::from_millis(CYCLE_MS));
            }
        });

        state
    }

    /// Feed a query into the stream — triggers surprise if prediction was wrong.
    pub fn feed_query(state: &Arc<Mutex<StreamState>>, query_hash: u64) {
        let mut s = state.lock().unwrap();
        let now = now_ms();

        // Compute surprise: prediction vs reality
        if s.predicted_confidence > 0.3 && s.last_query_hash != 0 {
            let predicted = s.predicted_query_hash;
            let actual = query_hash;
            if predicted != actual {
                // Prediction was wrong → surprise!
                let gap = (now - s.last_query_ms) as f32 / 1000.0;
                s.surprise_level =
                    (s.surprise_level + 0.3 * (1.0 - (gap / 60.0).min(1.0))).min(1.0);
            } else {
                // Prediction was correct → confidence boost
                s.surprise_level = (s.surprise_level * 0.9).max(0.0);
            }
        }

        s.last_query_hash = query_hash;
        s.last_query_ms = now;
    }

    /// Internal format implementation (takes &StreamState reference).
    pub fn format_internal(s: &StreamState) -> String {
        let emo_intensity = s.emotional_ring.intensity();
        let dominant = s.emotional_ring.dominant();

        format!(
            "🧠 Consciousness Stream — cycle #{} ({} Hz)\n\
             \x20 Emotion:  intensity={:.3}{}\n\
             \x20 Surprise: {:.3}\n\
             \x20 Curiosity: {:.3}\n\
             \x20 Predict:  hash={:016x} confidence={:.3}\n\
             \x20 Hebbian:  {} activations, {:.2} total energy\n\
             \x20 Attention: {} layers\n\
             \x20 Resonance: {} field cells\n\
             \x20 Patterns:  {} crystallized\n\
             \x20 Cache:    {} predictions, hit rate={:.1}%\n\
             \x20 Archetypes: {}\n\
             \x20 Mirror:    {} echoes\n",
            s.cycle,
            1000 / CYCLE_MS,
            emo_intensity,
            dominant
                .map(|(n, v)| format!(" ({}={:.2})", n, v))
                .unwrap_or_default(),
            s.surprise_level,
            s.curiosity_level,
            s.predicted_query_hash,
            s.predicted_confidence,
            s.hebbian.activations.len(),
            s.hebbian_total_energy as f64,
            s.attention.learned_weights.len(),
            s.resonance.field.len(),
            s.thought_graph.crystallized_count(),
            s.predictive_cache.predictions.len(),
            s.predictive_cache.stats.hit_rate() * 100.0,
            s.archetypes.archetypes.len(),
            s.mirror.echoes.len(),
        )
    }

    /// Get the current stream state as a formatted string.
    pub fn format(state: &Arc<Mutex<StreamState>>) -> String {
        let s = state.lock().unwrap();
        Self::format_internal(&s)
    }

    /// Format a snapshot read via seqlock — no Mutex needed on the read path.
    /// Use this from the MCP tool or any other reader that wants lock-free
    /// access. Falls back to "stale data" if the writer is mid-update for
    /// more than `SNAPSHOT_MAX_RETRIES` iterations.
    pub fn format_snapshot(snapshot: &SharedSnapshot) -> String {
        use crate::consciousness_seqlock::SnapshotData;
        let v: SnapshotData = match snapshot.read() {
            Some(d) => d,
            None => return "🧠 Consciousness Stream — (snapshot busy, retry)".to_string(),
        };
        let dominant = if v.emo_dominant_idx >= 0 {
            format!(
                " (idx={} val={:.2})",
                v.emo_dominant_idx, v.emo_dominant_val
            )
        } else {
            String::new()
        };
        format!(
            "🧠 Consciousness Stream — cycle #{} ({} Hz) [seqlock]\n\
             \x20 Emotion:  intensity={:.3}{}\n\
             \x20 Surprise: {:.3}\n\
             \x20 Curiosity: {:.3}\n\
             \x20 Predict:  hash={:016x} confidence={:.3}\n\
             \x20 Hebbian:  {} activations, {:.2} total energy\n\
             \x20 Attention: {} layers\n\
             \x20 Resonance: {} field cells\n\
             \x20 Patterns:  {} crystallized\n\
             \x20 Cache:    {} predictions, hit rate={:.1}%\n\
             \x20 Archetypes: {}\n\
             \x20 Mirror:    {} echoes\n",
            v.cycle,
            1000 / CYCLE_MS,
            v.emo_intensity,
            dominant,
            v.surprise_level,
            v.curiosity_level,
            v.predicted_query_hash,
            v.predicted_confidence,
            v.activations_count,
            v.activations_total_energy,
            v.attention_layers,
            v.resonance_cells,
            v.patterns_crystallized,
            v.predictions_count,
            v.predictions_hit_rate * 100.0,
            v.archetypes_count,
            v.mirror_echoes,
        )
    }
}

/// Publish the current stream state to the lock-free snapshot.
/// Called once per background cycle. Uses a seqlock so concurrent readers
/// either see the old or the new version, never a torn mix.
/// Also updates the cached format string and atomic hot fields for
/// the ultra-fast read path.
fn publish_snapshot(s: &StreamState) {
    let snap: &SharedSnapshot = &s.snapshot;

    // Build the formatted string first (while holding Mutex for state access)
    let formatted = ConsciousnessStream::format_internal(s);

    // Publish cached format and hot fields (no seqlock needed, atomic writes)
    snap.set_cached_format(formatted);
    snap.set_hot_fields(
        s.cycle,
        s.surprise_level,
        s.curiosity_level,
        s.predicted_query_hash,
    );

    // Now publish the full snapshot via seqlock
    let token = snap.begin_write();
    // SAFETY: We hold the seqlock — the sequence is odd, so any reader
    // who started before us will see the old sequence and retry, and any
    // reader who starts now will see the odd sequence and retry. We have
    // exclusive access to the data block until end_write.
    unsafe {
        let m = snap.data_mut();
        m.cycle = s.cycle;
        m.last_query_ms = s.last_query_ms;
        m.activations_count = s.hebbian.activations.len() as u32;
        m.activations_total_energy = s.hebbian_total_energy as f64;
        m.attention_layers = s.attention.learned_weights.len() as u32;
        m.resonance_cells = s.resonance.field.len() as u32;
        m.patterns_crystallized = s.thought_graph.crystallized_count() as u32;
        m.predictions_count = s.predictive_cache.predictions.len() as u32;
        m.predictions_hit_rate = s.predictive_cache.stats.hit_rate();
        m.archetypes_count = s.archetypes.archetypes.len() as u32;
        m.mirror_echoes = s.mirror.echoes.len() as u32;
        m.predicted_query_hash = s.predicted_query_hash;
        m.predicted_confidence = s.predicted_confidence;
        m.surprise_level = s.surprise_level;
        m.curiosity_level = s.curiosity_level;
        m.emo_intensity = s.emotional_ring.intensity();
        if let Some((_, val)) = s.emotional_ring.dominant() {
            m.emo_dominant_idx = 0;
            m.emo_dominant_val = val;
        } else {
            m.emo_dominant_idx = -1;
            m.emo_dominant_val = 0.0;
        }
    }
    snap.end_write(token);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
