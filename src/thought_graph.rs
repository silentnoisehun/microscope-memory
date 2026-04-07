//! ThoughtGraph — L6: Pattern Recognition for Microscope Memory.
//!
//! Tracks sequential recall paths and detects recurring thought patterns.
//! Every recall creates a node; consecutive recalls form edges.
//! When a sequence (A→B→C) recurs enough times, it crystallizes into a pattern.
//! Recognized patterns boost future search results.
//!
//! Binary formats:
//!   thought_graph.bin — nodes + edges (THG1)
//!   thought_patterns.bin — crystallized patterns (PTN1)

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ──────────────────────────────────────

const SESSION_GAP_MS: u64 = 1_800_000; // 30 min = new session
const MAX_NODES: usize = 5000; // ring buffer
const MAX_EDGES: usize = 10_000;
const MAX_PATTERNS: usize = 200;
const PATTERN_MIN_FREQ: u32 = 3; // min traversals to crystallize
const PATTERN_DECAY: f32 = 0.995; // per-recall decay
const NODE_BYTES: usize = 24;
const EDGE_BYTES: usize = 24;

/// How much pattern recognition boosts search scores.
pub const PATTERN_BOOST_WEIGHT: f32 = 0.15;

// ─── ThoughtNode ────────────────────────────────────

/// A single recall event.
#[derive(Clone, Debug)]
pub struct ThoughtNode {
    pub timestamp_ms: u64,
    pub query_hash: u64,
    pub session_id: u32,
    pub result_count: u16,
    pub dominant_layer: u8,
    pub centroid_hash: u8,
}

// ─── ThoughtEdge ────────────────────────────────────

/// Directed edge between two query types.
#[derive(Clone, Debug)]
pub struct ThoughtEdge {
    pub from_hash: u64,
    pub to_hash: u64,
    pub count: u32,
    pub last_ms: u32, // lower 32 bits of epoch ms
}

// ─── ThoughtPattern ─────────────────────────────────

/// A crystallized recall sequence.
#[derive(Clone, Debug)]
pub struct ThoughtPattern {
    pub id: u32,
    pub sequence: Vec<u64>, // ordered query hashes (len 2..=5)
    pub frequency: u32,
    pub strength: f32,
    pub last_seen_ms: u64,
    pub result_blocks: Vec<u32>, // union of top block indices
}

// ─── ThoughtGraphState ──────────────────────────────

pub struct ThoughtGraphState {
    pub nodes: Vec<ThoughtNode>,
    pub edges: HashMap<(u64, u64), ThoughtEdge>,
    pub patterns: Vec<ThoughtPattern>,
    pub current_session_id: u32,
    last_node_ts: u64,
    next_pattern_id: u32,
}

impl ThoughtGraphState {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let graph_path = output_dir.join("thought_graph.bin");
        let pattern_path = output_dir.join("thought_patterns.bin");

        let (nodes, edges, session_id, last_ts, next_pid) = if graph_path.exists() {
            load_graph(&graph_path)
        } else {
            (Vec::new(), HashMap::new(), 0u32, 0u64, 0u32)
        };

        let patterns = if pattern_path.exists() {
            load_patterns(&pattern_path)
        } else {
            Vec::new()
        };

        Self {
            nodes,
            edges,
            patterns,
            current_session_id: session_id,
            last_node_ts: last_ts,
            next_pattern_id: next_pid,
        }
    }

    /// Record a recall event. Returns session_id.
    pub fn record_recall(
        &mut self,
        query_hash: u64,
        results: &[(u32, f32)],
        dominant_layer: u8,
    ) -> u32 {
        let now_ms = now_epoch_ms();

        // Session detection
        if self.last_node_ts == 0 || (now_ms - self.last_node_ts) > SESSION_GAP_MS {
            self.current_session_id += 1;
        }
        self.last_node_ts = now_ms;

        // Centroid hash: spatial bucket of result center
        let centroid_hash = if results.is_empty() {
            0u8
        } else {
            let avg_idx =
                results.iter().map(|&(i, _)| i as u64).sum::<u64>() / results.len() as u64;
            (avg_idx & 0xFF) as u8
        };

        let node = ThoughtNode {
            timestamp_ms: now_ms,
            query_hash,
            session_id: self.current_session_id,
            result_count: results.len().min(u16::MAX as usize) as u16,
            dominant_layer,
            centroid_hash,
        };

        // Edge: connect to previous node in same session
        if let Some(prev) = self.nodes.last() {
            if prev.session_id == self.current_session_id {
                let key = (prev.query_hash, query_hash);
                let edge = self.edges.entry(key).or_insert(ThoughtEdge {
                    from_hash: prev.query_hash,
                    to_hash: query_hash,
                    count: 0,
                    last_ms: 0,
                });
                edge.count += 1;
                edge.last_ms = (now_ms & 0xFFFFFFFF) as u32;
            }
        }

        // Ring buffer
        self.nodes.push(node);
        if self.nodes.len() > MAX_NODES {
            self.nodes.drain(0..(self.nodes.len() - MAX_NODES));
        }

        // Evict edges if too many (drop least used)
        if self.edges.len() > MAX_EDGES {
            let mut edge_list: Vec<_> = self.edges.keys().cloned().collect();
            edge_list.sort_by_key(|k| self.edges[k].count);
            for key in edge_list.iter().take(self.edges.len() - MAX_EDGES) {
                self.edges.remove(key);
            }
        }

        self.current_session_id
    }

    /// Detect and crystallize patterns from recent session history.
    /// Uses sliding-window n-gram (lengths 2..=5) on the current session's recalls.
    pub fn detect_patterns(&mut self) {
        let session_nodes: Vec<&ThoughtNode> = self
            .nodes
            .iter()
            .filter(|n| n.session_id == self.current_session_id)
            .collect();

        if session_nodes.len() < 2 {
            return;
        }

        // Decay existing patterns
        for p in &mut self.patterns {
            p.strength *= PATTERN_DECAY;
        }

        // Check n-grams of length 2..=5
        for window_size in 2..=5usize {
            if session_nodes.len() < window_size {
                continue;
            }

            let start = session_nodes.len() - window_size;
            let seq: Vec<u64> = session_nodes[start..]
                .iter()
                .map(|n| n.query_hash)
                .collect();

            // Check if edges support this sequence (all transitions seen >= 2 times)
            let edges_ok = seq
                .windows(2)
                .all(|w| self.edges.get(&(w[0], w[1])).is_some_and(|e| e.count >= 2));

            if !edges_ok {
                continue;
            }

            // Find existing pattern or create candidate
            if let Some(p) = self.patterns.iter_mut().find(|p| p.sequence == seq) {
                p.frequency += 1;
                p.strength = (p.strength + 0.2).min(5.0);
                p.last_seen_ms = now_epoch_ms();
            } else {
                // New candidate
                let pattern = ThoughtPattern {
                    id: self.next_pattern_id,
                    sequence: seq,
                    frequency: 1,
                    strength: 1.0,
                    last_seen_ms: now_epoch_ms(),
                    result_blocks: Vec::new(),
                };
                self.next_pattern_id += 1;
                self.patterns.push(pattern);
            }
        }

        // Evict weak patterns and enforce limit
        self.patterns
            .retain(|p| p.strength >= 0.05 || p.frequency >= PATTERN_MIN_FREQ);
        if self.patterns.len() > MAX_PATTERNS {
            self.patterns.sort_by(|a, b| {
                let sa = a.strength * a.frequency as f32;
                let sb = b.strength * b.frequency as f32;
                sb.partial_cmp(&sa).unwrap()
            });
            self.patterns.truncate(MAX_PATTERNS);
        }
    }

    /// Compute pattern boost for a new query.
    /// Checks if recent session recalls + this query form a known pattern prefix/match.
    /// Returns block indices with boost scores.
    pub fn pattern_boost(&self, current_query_hash: u64) -> Vec<(u32, f32)> {
        let session_hashes: Vec<u64> = self
            .nodes
            .iter()
            .filter(|n| n.session_id == self.current_session_id)
            .map(|n| n.query_hash)
            .collect();

        let mut boosts: HashMap<u32, f32> = HashMap::new();

        for pattern in &self.patterns {
            if pattern.frequency < PATTERN_MIN_FREQ {
                continue;
            }

            let seq = &pattern.sequence;

            // Check if the session trail + current_query matches this pattern
            // Build the trail: last (seq.len()-1) session hashes + current_query_hash
            let prefix_len = seq.len() - 1;
            if session_hashes.len() < prefix_len {
                continue;
            }

            let trail_start = session_hashes.len() - prefix_len;
            let trail = &session_hashes[trail_start..];

            // Check if trail matches pattern prefix and current query matches the last element
            if trail == &seq[..prefix_len] && seq[prefix_len] == current_query_hash {
                // Full match — boost result blocks
                let boost = pattern.strength * PATTERN_BOOST_WEIGHT;
                for &block_idx in &pattern.result_blocks {
                    let entry = boosts.entry(block_idx).or_insert(0.0);
                    *entry += boost;
                }
            }
        }

        boosts.into_iter().collect()
    }

    /// Update pattern result blocks with the actual results from a recall.
    /// Called after record_recall when a pattern was matched.
    pub fn update_pattern_blocks(&mut self, query_hash: u64, result_blocks: &[u32]) {
        let session_hashes: Vec<u64> = self
            .nodes
            .iter()
            .filter(|n| n.session_id == self.current_session_id)
            .map(|n| n.query_hash)
            .collect();

        for pattern in &mut self.patterns {
            if pattern.frequency < PATTERN_MIN_FREQ {
                continue;
            }

            let seq = &pattern.sequence;

            // Check if this recall matches the last step of any pattern
            if seq.last() != Some(&query_hash) {
                continue;
            }

            let prefix_len = seq.len() - 1;
            if session_hashes.len() < prefix_len + 1 {
                continue;
            }

            // The node was already added, so check one before last
            let trail_start = session_hashes.len() - prefix_len - 1;
            let trail = &session_hashes[trail_start..session_hashes.len() - 1];

            if trail == &seq[..prefix_len] {
                // Merge result blocks (union, capped)
                for &b in result_blocks {
                    if !pattern.result_blocks.contains(&b) {
                        pattern.result_blocks.push(b);
                    }
                }
                // Cap at 50 blocks
                if pattern.result_blocks.len() > 50 {
                    pattern.result_blocks.truncate(50);
                }
            }
        }
    }

    /// Save to binary files.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_graph(
            &output_dir.join("thought_graph.bin"),
            &self.nodes,
            &self.edges,
            self.current_session_id,
            self.last_node_ts,
            self.next_pattern_id,
        )?;
        save_patterns(&output_dir.join("thought_patterns.bin"), &self.patterns)?;
        Ok(())
    }

    /// Top patterns by strength * frequency.
    pub fn top_patterns(&self, n: usize) -> Vec<&ThoughtPattern> {
        let mut sorted: Vec<&ThoughtPattern> = self.patterns.iter().collect();
        sorted.sort_by(|a, b| {
            let sa = a.strength * a.frequency as f32;
            let sb = b.strength * b.frequency as f32;
            sb.partial_cmp(&sa).unwrap()
        });
        sorted.truncate(n);
        sorted
    }

    /// Crystallized patterns (frequency >= PATTERN_MIN_FREQ).
    pub fn crystallized_count(&self) -> usize {
        self.patterns
            .iter()
            .filter(|p| p.frequency >= PATTERN_MIN_FREQ)
            .count()
    }

    /// Get current session's recall path.
    pub fn current_path(&self) -> Vec<&ThoughtNode> {
        self.nodes
            .iter()
            .filter(|n| n.session_id == self.current_session_id)
            .collect()
    }

    /// Get recent sessions (unique session IDs, most recent first).
    pub fn recent_sessions(&self, n: usize) -> Vec<Vec<&ThoughtNode>> {
        let mut session_map: HashMap<u32, Vec<&ThoughtNode>> = HashMap::new();
        for node in &self.nodes {
            session_map.entry(node.session_id).or_default().push(node);
        }

        let mut session_ids: Vec<u32> = session_map.keys().cloned().collect();
        session_ids.sort_unstable_by(|a, b| b.cmp(a));
        session_ids.truncate(n);

        session_ids
            .into_iter()
            .filter_map(|id| session_map.remove(&id))
            .collect()
    }

    /// Export crystallized patterns for cross-instance exchange.
    pub fn export_patterns(&self) -> Vec<&ThoughtPattern> {
        self.patterns
            .iter()
            .filter(|p| p.frequency >= PATTERN_MIN_FREQ)
            .collect()
    }

    /// Import patterns from a remote instance with trust weighting.
    pub fn import_patterns(&mut self, patterns: &[ThoughtPattern], trust: f32) {
        for remote in patterns {
            if let Some(local) = self
                .patterns
                .iter_mut()
                .find(|p| p.sequence == remote.sequence)
            {
                // Reinforce existing pattern
                local.strength = (local.strength + remote.strength * trust * 0.3).min(5.0);
            } else {
                // Add new with trust-weighted strength
                let mut imported = remote.clone();
                imported.id = self.next_pattern_id;
                self.next_pattern_id += 1;
                imported.strength = remote.strength * trust * 0.5;
                imported.frequency = 1; // starts as candidate
                self.patterns.push(imported);
            }
        }

        // Enforce cap
        if self.patterns.len() > MAX_PATTERNS {
            self.patterns.sort_by(|a, b| {
                let sa = a.strength * a.frequency as f32;
                let sb = b.strength * b.frequency as f32;
                sb.partial_cmp(&sa).unwrap()
            });
            self.patterns.truncate(MAX_PATTERNS);
        }
    }

    /// Stats summary.
    pub fn stats(&self) -> ThoughtGraphStats {
        ThoughtGraphStats {
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            pattern_count: self.patterns.len(),
            crystallized: self.crystallized_count(),
            current_session_id: self.current_session_id,
            current_path_len: self.current_path().len(),
        }
    }
}

pub struct ThoughtGraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub pattern_count: usize,
    pub crystallized: usize,
    pub current_session_id: u32,
    pub current_path_len: usize,
}

// ─── Binary I/O ─────────────────────────────────────

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn save_graph(
    path: &Path,
    nodes: &[ThoughtNode],
    edges: &HashMap<(u64, u64), ThoughtEdge>,
    session_id: u32,
    last_ts: u64,
    next_pid: u32,
) -> Result<(), String> {
    let edge_vec: Vec<&ThoughtEdge> = edges.values().collect();
    let capacity = 4 + 4 + 8 + 4 + 4 + 4 + nodes.len() * NODE_BYTES + edge_vec.len() * EDGE_BYTES;
    let mut buf = Vec::with_capacity(capacity);

    buf.write_all(b"THG1").map_err(|e| e.to_string())?;
    buf.write_all(&session_id.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&last_ts.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&next_pid.to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&(nodes.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    buf.write_all(&(edge_vec.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;

    for n in nodes {
        buf.write_all(&n.timestamp_ms.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&n.query_hash.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&n.session_id.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&n.result_count.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&[n.dominant_layer, n.centroid_hash])
            .map_err(|e| e.to_string())?;
    }

    for e in &edge_vec {
        buf.write_all(&e.from_hash.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&e.to_hash.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&e.count.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&e.last_ms.to_le_bytes())
            .map_err(|e| e.to_string())?;
    }

    fs::write(path, &buf).map_err(|e| e.to_string())
}

type GraphData = (
    Vec<ThoughtNode>,
    HashMap<(u64, u64), ThoughtEdge>,
    u32,
    u64,
    u32,
);

fn load_graph(path: &Path) -> GraphData {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return (Vec::new(), HashMap::new(), 0, 0, 0),
    };

    if data.len() < 28 || &data[0..4] != b"THG1" {
        return (Vec::new(), HashMap::new(), 0, 0, 0);
    }

    let session_id = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let last_ts = u64::from_le_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
    ]);
    let next_pid = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let node_count = u32::from_le_bytes([data[20], data[21], data[22], data[23]]) as usize;
    let edge_count = u32::from_le_bytes([data[24], data[25], data[26], data[27]]) as usize;

    let mut offset = 28;
    let mut nodes = Vec::with_capacity(node_count);

    for _ in 0..node_count {
        if offset + NODE_BYTES > data.len() {
            break;
        }
        let timestamp_ms = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        let query_hash = u64::from_le_bytes([
            data[offset + 8],
            data[offset + 9],
            data[offset + 10],
            data[offset + 11],
            data[offset + 12],
            data[offset + 13],
            data[offset + 14],
            data[offset + 15],
        ]);
        let session_id_n = u32::from_le_bytes([
            data[offset + 16],
            data[offset + 17],
            data[offset + 18],
            data[offset + 19],
        ]);
        let result_count = u16::from_le_bytes([data[offset + 20], data[offset + 21]]);
        let dominant_layer = data[offset + 22];
        let centroid_hash = data[offset + 23];

        nodes.push(ThoughtNode {
            timestamp_ms,
            query_hash,
            session_id: session_id_n,
            result_count,
            dominant_layer,
            centroid_hash,
        });
        offset += NODE_BYTES;
    }

    let mut edges = HashMap::with_capacity(edge_count);
    for _ in 0..edge_count {
        if offset + EDGE_BYTES > data.len() {
            break;
        }
        let from_hash = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        let to_hash = u64::from_le_bytes([
            data[offset + 8],
            data[offset + 9],
            data[offset + 10],
            data[offset + 11],
            data[offset + 12],
            data[offset + 13],
            data[offset + 14],
            data[offset + 15],
        ]);
        let count = u32::from_le_bytes([
            data[offset + 16],
            data[offset + 17],
            data[offset + 18],
            data[offset + 19],
        ]);
        let last_ms = u32::from_le_bytes([
            data[offset + 20],
            data[offset + 21],
            data[offset + 22],
            data[offset + 23],
        ]);

        edges.insert(
            (from_hash, to_hash),
            ThoughtEdge {
                from_hash,
                to_hash,
                count,
                last_ms,
            },
        );
        offset += EDGE_BYTES;
    }

    (nodes, edges, session_id, last_ts, next_pid)
}

fn save_patterns(path: &Path, patterns: &[ThoughtPattern]) -> Result<(), String> {
    let mut buf = Vec::with_capacity(8 + patterns.len() * 64);

    buf.write_all(b"PTN1").map_err(|e| e.to_string())?;
    buf.write_all(&(patterns.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;

    for p in patterns {
        buf.write_all(&p.id.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&(p.sequence.len() as u16).to_le_bytes())
            .map_err(|e| e.to_string())?;
        for &h in &p.sequence {
            buf.write_all(&h.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        buf.write_all(&p.frequency.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&p.strength.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&p.last_seen_ms.to_le_bytes())
            .map_err(|e| e.to_string())?;
        buf.write_all(&(p.result_blocks.len() as u16).to_le_bytes())
            .map_err(|e| e.to_string())?;
        for &b in &p.result_blocks {
            buf.write_all(&b.to_le_bytes()).map_err(|e| e.to_string())?;
        }
    }

    fs::write(path, &buf).map_err(|e| e.to_string())
}

fn load_patterns(path: &Path) -> Vec<ThoughtPattern> {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    if data.len() < 8 || &data[0..4] != b"PTN1" {
        return Vec::new();
    }

    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut offset = 8;
    let mut patterns = Vec::with_capacity(count);

    for _ in 0..count {
        if offset + 6 > data.len() {
            break;
        }

        let id = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let seq_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        if offset + seq_len * 8 > data.len() {
            break;
        }
        let mut sequence = Vec::with_capacity(seq_len);
        for _ in 0..seq_len {
            let h = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            sequence.push(h);
            offset += 8;
        }

        if offset + 16 > data.len() {
            break;
        }
        let frequency = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let strength = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let last_seen_ms = u64::from_le_bytes([
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
        let mut result_blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let b = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            result_blocks.push(b);
            offset += 4;
        }

        patterns.push(ThoughtPattern {
            id,
            sequence,
            frequency,
            strength,
            last_seen_ms,
            result_blocks,
        });
    }

    patterns
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> ThoughtGraphState {
        ThoughtGraphState {
            nodes: Vec::new(),
            edges: HashMap::new(),
            patterns: Vec::new(),
            current_session_id: 0,
            last_node_ts: 0,
            next_pattern_id: 0,
        }
    }

    #[test]
    fn test_record_recall() {
        let mut state = make_state();
        let results = vec![(10u32, 0.5f32), (20, 0.3)];
        let sid = state.record_recall(0xAABB, &results, 1);
        assert_eq!(sid, 1); // first recall starts session 1
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.edges.len(), 0); // only 1 node, no edge
    }

    #[test]
    fn test_sequential_recalls() {
        let mut state = make_state();
        state.last_node_ts = now_epoch_ms(); // force same session
        state.current_session_id = 1;

        state.record_recall(0xAA, &[(1, 0.5)], 1);
        state.record_recall(0xBB, &[(2, 0.3)], 1);

        assert_eq!(state.nodes.len(), 2);
        assert_eq!(state.edges.len(), 1);
        assert!(state.edges.contains_key(&(0xAA, 0xBB)));
        assert_eq!(state.edges[&(0xAA, 0xBB)].count, 1);
    }

    #[test]
    fn test_session_gap() {
        let mut state = make_state();
        state.record_recall(0xAA, &[], 0);
        let sid1 = state.current_session_id;

        // Simulate gap
        state.last_node_ts = now_epoch_ms() - SESSION_GAP_MS - 1;
        state.record_recall(0xBB, &[], 0);
        let sid2 = state.current_session_id;

        assert!(sid2 > sid1);
        assert_eq!(state.edges.len(), 0); // different sessions, no edge
    }

    #[test]
    fn test_pattern_detection() {
        let mut state = make_state();
        state.current_session_id = 1;
        state.last_node_ts = now_epoch_ms();

        // Simulate the sequence A→B→C three times
        // We need edges with count >= 2 for patterns to form
        // So we repeat the full sequence multiple times in one session

        for _ in 0..4 {
            state.record_recall(0xAA, &[(1, 0.5)], 1);
            state.record_recall(0xBB, &[(2, 0.3)], 1);
            state.record_recall(0xCC, &[(3, 0.2)], 1);
        }

        state.detect_patterns();

        // Should have found patterns (at least the 2-gram BB→CC)
        assert!(!state.patterns.is_empty());
    }

    #[test]
    fn test_pattern_boost_empty() {
        let state = make_state();
        let boosts = state.pattern_boost(0xAA);
        assert!(boosts.is_empty());
    }

    #[test]
    fn test_pattern_boost_with_match() {
        let mut state = make_state();
        state.current_session_id = 1;
        state.last_node_ts = now_epoch_ms();

        // Create a crystallized pattern AA→BB with result blocks
        state.patterns.push(ThoughtPattern {
            id: 0,
            sequence: vec![0xAA, 0xBB],
            frequency: PATTERN_MIN_FREQ,
            strength: 2.0,
            last_seen_ms: now_epoch_ms(),
            result_blocks: vec![10, 20, 30],
        });

        // Simulate: last recall was AA, now querying BB
        state.nodes.push(ThoughtNode {
            timestamp_ms: now_epoch_ms(),
            query_hash: 0xAA,
            session_id: 1,
            result_count: 1,
            dominant_layer: 0,
            centroid_hash: 0,
        });

        let boosts = state.pattern_boost(0xBB);
        assert!(!boosts.is_empty());
        // Should boost blocks 10, 20, 30
        let boost_map: HashMap<u32, f32> = boosts.into_iter().collect();
        assert!(boost_map.contains_key(&10));
        assert!(boost_map.contains_key(&20));
        assert!(boost_map.contains_key(&30));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = make_state();
        state.current_session_id = 1;
        state.last_node_ts = now_epoch_ms();

        state.record_recall(0xAA, &[(1, 0.5)], 1);
        state.record_recall(0xBB, &[(2, 0.3)], 2);

        state.patterns.push(ThoughtPattern {
            id: 0,
            sequence: vec![0xAA, 0xBB],
            frequency: 5,
            strength: 2.0,
            last_seen_ms: 12345678,
            result_blocks: vec![10, 20],
        });

        state.save(dir.path()).unwrap();

        let loaded = ThoughtGraphState::load_or_init(dir.path());
        assert_eq!(loaded.nodes.len(), 2);
        assert_eq!(loaded.edges.len(), 1);
        assert_eq!(loaded.patterns.len(), 1);
        assert_eq!(loaded.patterns[0].sequence, vec![0xAA, 0xBB]);
        assert_eq!(loaded.patterns[0].frequency, 5);
        assert_eq!(loaded.patterns[0].result_blocks, vec![10, 20]);
        assert_eq!(loaded.current_session_id, 1);
    }

    #[test]
    fn test_node_ring_buffer() {
        let mut state = make_state();
        state.current_session_id = 1;
        state.last_node_ts = now_epoch_ms();

        for i in 0..MAX_NODES + 100 {
            state.record_recall(i as u64, &[], 0);
        }

        assert_eq!(state.nodes.len(), MAX_NODES);
    }

    #[test]
    fn test_recent_sessions() {
        let mut state = make_state();

        // Session 1
        state.record_recall(0xAA, &[], 0);
        state.record_recall(0xBB, &[], 0);

        // Force new session
        state.last_node_ts = now_epoch_ms() - SESSION_GAP_MS - 1;
        state.record_recall(0xCC, &[], 0);

        let sessions = state.recent_sessions(5);
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_stats() {
        let mut state = make_state();
        state.record_recall(0xAA, &[], 0);
        state.record_recall(0xBB, &[], 0);

        let stats = state.stats();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
        assert_eq!(stats.pattern_count, 0);
    }
}
