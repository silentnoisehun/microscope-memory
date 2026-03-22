//! Predictive Cache — pre-fetches blocks based on ThoughtGraph patterns.
//!
//! After each recall, the cache predicts what the *next* query will be
//! based on recognized thought patterns. It pre-loads the expected result
//! blocks. If the prediction hits, the pattern gets a positive reward;
//! misses decay the prediction confidence.
//!
//! This is a feedback loop: good patterns → accurate predictions → rewards → stronger patterns.
//!
//! Binary format: predictive_cache.bin (PRC1)

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::thought_graph::{ThoughtGraphState, PATTERN_BOOST_WEIGHT};

// ─── Constants ──────────────────────────────────────

const MAX_PREDICTIONS: usize = 50; // max cached predictions
const MAX_BLOCKS_PER_PREDICTION: usize = 30;
const HIT_REWARD: f32 = 0.3; // strength reward on cache hit
const MISS_PENALTY: f32 = 0.05; // strength penalty on miss
const PREDICTION_DECAY: f32 = 0.98; // per-recall confidence decay
const MIN_CONFIDENCE: f32 = 0.1; // below this, evict prediction
const MIN_PATTERN_FREQ: u32 = 3; // only predict from crystallized patterns

// ─── Prediction ─────────────────────────────────────

/// A single prediction: "if next query hash is X, these blocks are likely results."
#[derive(Clone, Debug)]
pub struct Prediction {
    pub predicted_query_hash: u64,
    pub blocks: Vec<u32>,
    pub confidence: f32,
    pub pattern_id: u32,
    pub created_ms: u64,
}

// ─── CacheStats ─────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    pub total_predictions: u32,
    pub total_hits: u32,
    pub total_misses: u32,
    pub total_partial_hits: u32,
    pub current_predictions: usize,
    pub avg_confidence: f32,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f32 {
        let total = self.total_hits + self.total_misses + self.total_partial_hits;
        if total == 0 {
            return 0.0;
        }
        (self.total_hits as f32 + self.total_partial_hits as f32 * 0.5) / total as f32
    }
}

// ─── PredictiveCache ────────────────────────────────

pub struct PredictiveCache {
    pub predictions: Vec<Prediction>,
    pub stats: CacheStats,
}

impl PredictiveCache {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("predictive_cache.bin");
        if path.exists() {
            load_cache(&path)
        } else {
            Self {
                predictions: Vec::new(),
                stats: CacheStats::default(),
            }
        }
    }

    /// Check if we have a prediction for this query. Returns cached blocks + confidence.
    /// This is called BEFORE the actual search, so the results can be used immediately.
    pub fn check(&self, query_hash: u64) -> Option<(Vec<u32>, f32)> {
        self.predictions
            .iter()
            .find(|p| p.predicted_query_hash == query_hash && p.confidence >= MIN_CONFIDENCE)
            .map(|p| (p.blocks.clone(), p.confidence))
    }

    /// Evaluate prediction accuracy after a recall completes.
    /// Compares predicted blocks against actual results.
    /// Returns: (hit_type, overlap_count) where hit_type is "hit", "partial", or "miss".
    pub fn evaluate(
        &mut self,
        query_hash: u64,
        actual_results: &[u32],
        thought_graph: &mut ThoughtGraphState,
    ) -> (&'static str, usize) {
        let prediction = self
            .predictions
            .iter()
            .find(|p| p.predicted_query_hash == query_hash);

        let prediction = match prediction {
            Some(p) => p.clone(),
            None => return ("none", 0),
        };

        // Count overlap
        let overlap = prediction
            .blocks
            .iter()
            .filter(|b| actual_results.contains(b))
            .count();

        let hit_type = if overlap == 0 {
            // Miss
            self.stats.total_misses += 1;
            // Penalize the pattern
            if let Some(pattern) = thought_graph
                .patterns
                .iter_mut()
                .find(|p| p.id == prediction.pattern_id)
            {
                pattern.strength = (pattern.strength - MISS_PENALTY).max(0.0);
            }
            // Decay prediction confidence
            if let Some(pred) = self
                .predictions
                .iter_mut()
                .find(|p| p.predicted_query_hash == query_hash)
            {
                pred.confidence *= 0.5; // harsh decay on miss
            }
            "miss"
        } else if overlap >= prediction.blocks.len() / 2 || overlap >= 3 {
            // Hit — majority of predicted blocks were correct
            self.stats.total_hits += 1;
            // Reward the pattern
            if let Some(pattern) = thought_graph
                .patterns
                .iter_mut()
                .find(|p| p.id == prediction.pattern_id)
            {
                pattern.strength = (pattern.strength + HIT_REWARD).min(5.0);
            }
            // Boost prediction confidence
            if let Some(pred) = self
                .predictions
                .iter_mut()
                .find(|p| p.predicted_query_hash == query_hash)
            {
                pred.confidence = (pred.confidence + 0.2).min(1.0);
            }
            "hit"
        } else {
            // Partial hit
            self.stats.total_partial_hits += 1;
            let reward = HIT_REWARD * (overlap as f32 / prediction.blocks.len() as f32);
            if let Some(pattern) = thought_graph
                .patterns
                .iter_mut()
                .find(|p| p.id == prediction.pattern_id)
            {
                pattern.strength = (pattern.strength + reward).min(5.0);
            }
            "partial"
        };

        (hit_type, overlap)
    }

    /// Generate predictions for the next likely query based on current session state.
    /// Called after each recall to pre-load the cache.
    pub fn predict_next(&mut self, thought_graph: &ThoughtGraphState) {
        // Decay all existing predictions
        for pred in &mut self.predictions {
            pred.confidence *= PREDICTION_DECAY;
        }
        self.predictions.retain(|p| p.confidence >= MIN_CONFIDENCE);

        let session_hashes: Vec<u64> = thought_graph
            .nodes
            .iter()
            .filter(|n| n.session_id == thought_graph.current_session_id)
            .map(|n| n.query_hash)
            .collect();

        if session_hashes.is_empty() {
            return;
        }

        let now_ms = now_epoch_ms();

        for pattern in &thought_graph.patterns {
            if pattern.frequency < MIN_PATTERN_FREQ {
                continue;
            }
            if pattern.result_blocks.is_empty() {
                continue;
            }

            let seq = &pattern.sequence;

            // Check if session trail matches any prefix of this pattern
            // If trail ends with seq[0..n], predict seq[n] with its blocks
            for prefix_len in 1..seq.len() {
                if session_hashes.len() < prefix_len {
                    continue;
                }

                let trail_start = session_hashes.len() - prefix_len;
                let trail = &session_hashes[trail_start..];

                if trail == &seq[..prefix_len] {
                    let predicted_hash = seq[prefix_len];

                    // Don't duplicate predictions for same hash
                    if self
                        .predictions
                        .iter()
                        .any(|p| p.predicted_query_hash == predicted_hash)
                    {
                        continue;
                    }

                    let confidence = pattern.strength * PATTERN_BOOST_WEIGHT
                        * (prefix_len as f32 / seq.len() as f32);

                    let blocks: Vec<u32> = pattern
                        .result_blocks
                        .iter()
                        .take(MAX_BLOCKS_PER_PREDICTION)
                        .copied()
                        .collect();

                    self.predictions.push(Prediction {
                        predicted_query_hash: predicted_hash,
                        blocks,
                        confidence: confidence.min(1.0),
                        pattern_id: pattern.id,
                        created_ms: now_ms,
                    });

                    self.stats.total_predictions += 1;
                }
            }
        }

        // Cap predictions
        if self.predictions.len() > MAX_PREDICTIONS {
            self.predictions
                .sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
            self.predictions.truncate(MAX_PREDICTIONS);
        }

        // Update avg confidence
        if !self.predictions.is_empty() {
            self.stats.avg_confidence = self.predictions.iter().map(|p| p.confidence).sum::<f32>()
                / self.predictions.len() as f32;
        }
        self.stats.current_predictions = self.predictions.len();
    }

    /// Save to binary.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_cache(&output_dir.join("predictive_cache.bin"), self)
    }
}

// ─── Binary I/O ─────────────────────────────────────

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn save_cache(path: &Path, cache: &PredictiveCache) -> Result<(), String> {
    let mut buf = Vec::with_capacity(256);

    // Header
    buf.write_all(b"PRC1").map_err(|e| e.to_string())?;
    buf.write_all(&(cache.predictions.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;

    // Stats
    buf.write_all(&cache.stats.total_predictions.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&cache.stats.total_hits.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&cache.stats.total_misses.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&cache.stats.total_partial_hits.to_le_bytes())
        .map_err(|e| e.to_string())?;

    // Predictions (variable length)
    for p in &cache.predictions {
        buf.write_all(&p.predicted_query_hash.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&p.confidence.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&p.pattern_id.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&p.created_ms.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&(p.blocks.len() as u16).to_le_bytes())
            .map_err(|e| e.to_string())?;
        for &b in &p.blocks {
            buf.write_all(&b.to_le_bytes()).map_err(|e| e.to_string())?;
        }
    }

    fs::write(path, &buf).map_err(|e| e.to_string())
}

fn load_cache(path: &Path) -> PredictiveCache {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => {
            return PredictiveCache {
                predictions: Vec::new(),
                stats: CacheStats::default(),
            }
        }
    };

    if data.len() < 24 || &data[0..4] != b"PRC1" {
        return PredictiveCache {
            predictions: Vec::new(),
            stats: CacheStats::default(),
        };
    }

    let pred_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    let total_predictions = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let total_hits = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let total_misses = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let total_partial_hits = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);

    let mut offset = 24;
    let mut predictions = Vec::with_capacity(pred_count);

    for _ in 0..pred_count {
        if offset + 22 > data.len() {
            break;
        }

        let predicted_query_hash = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        let confidence = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let pattern_id = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let created_ms = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        if offset + 2 > data.len() {
            break;
        }
        let block_count = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        if offset + block_count * 4 > data.len() {
            break;
        }
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let b = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            blocks.push(b);
            offset += 4;
        }

        predictions.push(Prediction {
            predicted_query_hash,
            blocks,
            confidence,
            pattern_id,
            created_ms,
        });
    }

    let current_predictions = predictions.len();
    let avg_confidence = if predictions.is_empty() {
        0.0
    } else {
        predictions.iter().map(|p| p.confidence).sum::<f32>() / predictions.len() as f32
    };

    PredictiveCache {
        predictions,
        stats: CacheStats {
            total_predictions,
            total_hits,
            total_misses,
            total_partial_hits,
            current_predictions,
            avg_confidence,
        },
    }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thought_graph::{ThoughtGraphState, ThoughtPattern};

    fn make_tg() -> ThoughtGraphState {
        ThoughtGraphState::load_or_init(Path::new("/nonexistent"))
    }

    #[test]
    fn test_check_empty() {
        let cache = PredictiveCache {
            predictions: Vec::new(),
            stats: CacheStats::default(),
        };
        assert!(cache.check(0xAA).is_none());
    }

    #[test]
    fn test_check_hit() {
        let cache = PredictiveCache {
            predictions: vec![Prediction {
                predicted_query_hash: 0xAA,
                blocks: vec![10, 20, 30],
                confidence: 0.8,
                pattern_id: 0,
                created_ms: 0,
            }],
            stats: CacheStats::default(),
        };
        let result = cache.check(0xAA);
        assert!(result.is_some());
        let (blocks, conf) = result.unwrap();
        assert_eq!(blocks, vec![10, 20, 30]);
        assert!((conf - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_evaluate_hit() {
        let mut cache = PredictiveCache {
            predictions: vec![Prediction {
                predicted_query_hash: 0xAA,
                blocks: vec![10, 20, 30],
                confidence: 0.5,
                pattern_id: 0,
                created_ms: 0,
            }],
            stats: CacheStats::default(),
        };
        let mut tg = make_tg();
        tg.patterns.push(ThoughtPattern {
            id: 0,
            sequence: vec![0xBB, 0xAA],
            frequency: 5,
            strength: 1.0,
            last_seen_ms: 0,
            result_blocks: vec![10, 20, 30],
        });

        let actual = vec![10u32, 20, 30, 40];
        let (hit_type, overlap) = cache.evaluate(0xAA, &actual, &mut tg);
        assert_eq!(hit_type, "hit");
        assert_eq!(overlap, 3);
        assert_eq!(cache.stats.total_hits, 1);
        // Pattern should be rewarded
        assert!(tg.patterns[0].strength > 1.0);
    }

    #[test]
    fn test_evaluate_miss() {
        let mut cache = PredictiveCache {
            predictions: vec![Prediction {
                predicted_query_hash: 0xAA,
                blocks: vec![10, 20, 30],
                confidence: 0.5,
                pattern_id: 0,
                created_ms: 0,
            }],
            stats: CacheStats::default(),
        };
        let mut tg = make_tg();
        tg.patterns.push(ThoughtPattern {
            id: 0,
            sequence: vec![0xBB, 0xAA],
            frequency: 5,
            strength: 1.0,
            last_seen_ms: 0,
            result_blocks: vec![10, 20, 30],
        });

        let actual = vec![100u32, 200, 300]; // no overlap
        let (hit_type, overlap) = cache.evaluate(0xAA, &actual, &mut tg);
        assert_eq!(hit_type, "miss");
        assert_eq!(overlap, 0);
        assert_eq!(cache.stats.total_misses, 1);
        // Pattern should be penalized
        assert!(tg.patterns[0].strength < 1.0);
    }

    #[test]
    fn test_evaluate_no_prediction() {
        let mut cache = PredictiveCache {
            predictions: Vec::new(),
            stats: CacheStats::default(),
        };
        let mut tg = make_tg();
        let (hit_type, _) = cache.evaluate(0xAA, &[10, 20], &mut tg);
        assert_eq!(hit_type, "none");
    }

    #[test]
    fn test_predict_next() {
        let mut cache = PredictiveCache {
            predictions: Vec::new(),
            stats: CacheStats::default(),
        };
        let mut tg = make_tg();

        // Set up: session with one recall (hash=0xAA), pattern AA→BB with blocks
        tg.current_session_id = 1;
        tg.nodes.push(crate::thought_graph::ThoughtNode {
            timestamp_ms: 1000,
            query_hash: 0xAA,
            session_id: 1,
            result_count: 3,
            dominant_layer: 1,
            centroid_hash: 0,
        });
        tg.patterns.push(ThoughtPattern {
            id: 0,
            sequence: vec![0xAA, 0xBB],
            frequency: 5,
            strength: 2.0,
            last_seen_ms: 1000,
            result_blocks: vec![10, 20, 30],
        });

        cache.predict_next(&tg);

        assert_eq!(cache.predictions.len(), 1);
        assert_eq!(cache.predictions[0].predicted_query_hash, 0xBB);
        assert_eq!(cache.predictions[0].blocks, vec![10, 20, 30]);
        assert!(cache.predictions[0].confidence > 0.0);
    }

    #[test]
    fn test_predict_decay() {
        let mut cache = PredictiveCache {
            predictions: vec![Prediction {
                predicted_query_hash: 0xAA,
                blocks: vec![10],
                confidence: MIN_CONFIDENCE + 0.01,
                pattern_id: 0,
                created_ms: 0,
            }],
            stats: CacheStats::default(),
        };
        let tg = make_tg();

        // Multiple predict_next calls should decay confidence below threshold
        for _ in 0..20 {
            cache.predict_next(&tg);
        }
        assert!(cache.predictions.is_empty());
    }

    #[test]
    fn test_hit_rate() {
        let mut stats = CacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);

        stats.total_hits = 7;
        stats.total_misses = 3;
        assert!((stats.hit_rate() - 0.7).abs() < 0.001);

        stats.total_partial_hits = 2;
        // (7 + 2*0.5) / (7+3+2) = 8/12 = 0.667
        assert!((stats.hit_rate() - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();

        let cache = PredictiveCache {
            predictions: vec![
                Prediction {
                    predicted_query_hash: 0xAA,
                    blocks: vec![10, 20],
                    confidence: 0.75,
                    pattern_id: 1,
                    created_ms: 12345,
                },
                Prediction {
                    predicted_query_hash: 0xBB,
                    blocks: vec![30, 40, 50],
                    confidence: 0.5,
                    pattern_id: 2,
                    created_ms: 67890,
                },
            ],
            stats: CacheStats {
                total_predictions: 10,
                total_hits: 5,
                total_misses: 3,
                total_partial_hits: 2,
                current_predictions: 2,
                avg_confidence: 0.625,
            },
        };

        cache.save(dir.path()).unwrap();
        let loaded = PredictiveCache::load_or_init(dir.path());

        assert_eq!(loaded.predictions.len(), 2);
        assert_eq!(loaded.predictions[0].predicted_query_hash, 0xAA);
        assert_eq!(loaded.predictions[0].blocks, vec![10, 20]);
        assert!((loaded.predictions[0].confidence - 0.75).abs() < 0.001);
        assert_eq!(loaded.predictions[1].blocks, vec![30, 40, 50]);
        assert_eq!(loaded.stats.total_hits, 5);
        assert_eq!(loaded.stats.total_misses, 3);
        assert_eq!(loaded.stats.total_partial_hits, 2);
    }
}
