//! Archetype emergence layer for Microscope Memory.
//!
//! Archetypes are recurring activation patterns that crystallize from
//! repeated Hebbian/mirror/resonance activity. When the same spatial
//! regions fire together often enough, an archetype "emerges" —
//! a stable attractor in the memory landscape.
//!
//! Archetypes are visible at depth 0 (D0) — the highest zoom level —
//! as named clusters that represent concepts the system has learned.
//!
//! Binary format: archetypes.bin (ARC1)

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::hebbian;
use crate::resonance::ResonanceState;

// ─── Constants ──────────────────────────────────────

/// Minimum resonance field energy to seed an archetype.
const SEED_THRESHOLD: f32 = 2.0;
/// Minimum number of member blocks for a valid archetype.
const MIN_MEMBERS: usize = 3;
/// Maximum number of archetypes.
const MAX_ARCHETYPES: usize = 100;
/// Spatial radius for clustering blocks into an archetype.
const CLUSTER_RADIUS: f32 = 0.15;
/// Archetype strength decay per cycle.
const ARCHETYPE_DECAY: f32 = 0.98;

// ─── Types ──────────────────────────────────────────

/// An archetype — an emergent concept crystallized from activation patterns.
#[derive(Clone, Debug)]
pub struct Archetype {
    /// Unique archetype ID.
    pub id: u32,
    /// Centroid in 3D space (average of member block coordinates).
    pub centroid: (f32, f32, f32),
    /// Member block indices.
    pub members: Vec<u32>,
    /// Archetype strength (accumulated from resonance + Hebbian energy).
    pub strength: f32,
    /// Number of times this archetype has been reinforced.
    pub reinforcement_count: u32,
    /// Timestamp of first emergence.
    pub emerged_ms: u64,
    /// Timestamp of last reinforcement.
    pub last_reinforced_ms: u64,
    /// Auto-generated label (derived from member block content).
    pub label: String,
}

/// Archetype system state.
pub struct ArchetypeState {
    pub archetypes: Vec<Archetype>,
    next_id: u32,
}

impl ArchetypeState {
    /// Load or initialize archetype state.
    pub fn load_or_init(output_dir: &Path) -> Self {
        load_archetypes(output_dir).unwrap_or_else(|| Self {
            archetypes: Vec::new(),
            next_id: 1,
        })
    }

    /// Detect new archetypes from the resonance field and Hebbian state.
    /// Returns the number of new archetypes emerged.
    pub fn detect(
        &mut self,
        resonance: &ResonanceState,
        hebb: &hebbian::HebbianState,
        headers: &[(f32, f32, f32)],
        block_texts: &[&str],
    ) -> usize {
        let now_ms = hebbian::now_epoch_ms_pub();
        let mut new_count = 0;

        // Find hot spots in the resonance field above threshold
        let hot_cells: Vec<((i16, i16, i16), f32)> = resonance
            .field
            .iter()
            .filter(|(_, &v)| v >= SEED_THRESHOLD)
            .map(|(&k, &v)| (k, v))
            .collect();

        for ((qx, qy, qz), field_energy) in hot_cells {
            // Convert quantized coords back to float
            let cx = qx as f32 / 20.0;
            let cy = qy as f32 / 20.0;
            let cz = qz as f32 / 20.0;

            // Check if an existing archetype already covers this region
            if self
                .archetypes
                .iter()
                .any(|a| spatial_dist(a.centroid, (cx, cy, cz)) < CLUSTER_RADIUS)
            {
                // Reinforce existing archetype instead
                if let Some(a) = self.archetypes.iter_mut().min_by(|a, b| {
                    spatial_dist(a.centroid, (cx, cy, cz))
                        .partial_cmp(&spatial_dist(b.centroid, (cx, cy, cz)))
                        .unwrap()
                }) {
                    a.strength += field_energy * 0.1;
                    a.reinforcement_count += 1;
                    a.last_reinforced_ms = now_ms;
                }
                continue;
            }

            // Find nearby blocks to form the archetype
            let mut members = Vec::new();
            for (idx, (bx, by, bz)) in headers.iter().enumerate() {
                let dx = cx - bx;
                let dy = cy - by;
                let dz = cz - bz;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if dist < CLUSTER_RADIUS {
                    // Extra requirement: block must have some Hebbian activity
                    if idx < hebb.activations.len() && hebb.activations[idx].activation_count > 0 {
                        members.push(idx as u32);
                    }
                }
            }

            if members.len() < MIN_MEMBERS {
                continue;
            }

            // Compute centroid from actual member positions
            let (sum_x, sum_y, sum_z) = members.iter().fold((0.0f32, 0.0f32, 0.0f32), |acc, &m| {
                let (x, y, z) = headers[m as usize];
                (acc.0 + x, acc.1 + y, acc.2 + z)
            });
            let n = members.len() as f32;
            let centroid = (sum_x / n, sum_y / n, sum_z / n);

            // Generate label from most common words in member blocks
            let label = generate_label(&members, block_texts);

            let archetype = Archetype {
                id: self.next_id,
                centroid,
                members,
                strength: field_energy,
                reinforcement_count: 1,
                emerged_ms: now_ms,
                last_reinforced_ms: now_ms,
                label,
            };

            self.archetypes.push(archetype);
            self.next_id += 1;
            new_count += 1;

            if self.archetypes.len() >= MAX_ARCHETYPES {
                break;
            }
        }

        new_count
    }

    /// Reinforce archetypes based on a new activation pattern.
    /// If activated blocks overlap with an archetype's members, strengthen it.
    pub fn reinforce(&mut self, activated_blocks: &[(u32, f32)]) {
        let now_ms = hebbian::now_epoch_ms_pub();
        let activated_set: HashMap<u32, f32> = activated_blocks.iter().copied().collect();

        for archetype in &mut self.archetypes {
            let overlap: f32 = archetype
                .members
                .iter()
                .filter_map(|m| activated_set.get(m))
                .sum();

            if overlap > 0.0 {
                archetype.strength += overlap * 0.05;
                archetype.reinforcement_count += 1;
                archetype.last_reinforced_ms = now_ms;
            }
        }
    }

    /// Decay archetype strengths. Remove dead archetypes.
    pub fn decay(&mut self) {
        for a in &mut self.archetypes {
            a.strength *= ARCHETYPE_DECAY;
        }
        // Remove archetypes that have decayed below threshold and have few reinforcements
        self.archetypes
            .retain(|a| a.strength > 0.1 || a.reinforcement_count > 5);
    }

    /// Find which archetype (if any) a query activation best matches.
    pub fn match_archetype(&self, activated_blocks: &[(u32, f32)]) -> Option<(usize, f32)> {
        let activated_set: HashMap<u32, f32> = activated_blocks.iter().copied().collect();

        let mut best: Option<(usize, f32)> = None;
        for (i, archetype) in self.archetypes.iter().enumerate() {
            let overlap: f32 = archetype
                .members
                .iter()
                .filter_map(|m| activated_set.get(m))
                .sum();

            let coverage = if archetype.members.is_empty() {
                0.0
            } else {
                let matching = archetype
                    .members
                    .iter()
                    .filter(|m| activated_set.contains_key(m))
                    .count();
                matching as f32 / archetype.members.len() as f32
            };

            let score = overlap * coverage * archetype.strength;
            if score > 0.0 && (best.is_none() || score > best.unwrap().1) {
                best = Some((i, score));
            }
        }
        best
    }

    /// Get statistics.
    pub fn stats(&self) -> ArchetypeStats {
        let total_members: usize = self.archetypes.iter().map(|a| a.members.len()).sum();
        let strongest = self
            .archetypes
            .iter()
            .max_by(|a, b| a.strength.partial_cmp(&b.strength).unwrap());

        ArchetypeStats {
            archetype_count: self.archetypes.len(),
            total_members,
            strongest_label: strongest.map(|a| a.label.clone()),
            strongest_strength: strongest.map(|a| a.strength),
        }
    }

    /// Save to disk.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_archetypes(output_dir, self)
    }
}

pub struct ArchetypeStats {
    pub archetype_count: usize,
    pub total_members: usize,
    pub strongest_label: Option<String>,
    pub strongest_strength: Option<f32>,
}

// ─── Helpers ────────────────────────────────────────

fn spatial_dist(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    let dz = a.2 - b.2;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Generate a label from the most common meaningful words in member blocks.
fn generate_label(members: &[u32], block_texts: &[&str]) -> String {
    let mut word_counts: HashMap<&str, usize> = HashMap::new();
    let stopwords = [
        "a", "the", "is", "of", "and", "to", "in", "it", "on", "for", "that", "this", "with",
        "was", "are", "be", "has", "had", "not", "but", "from", "or", "an", "at", "by",
    ];

    for &idx in members {
        let i = idx as usize;
        if i < block_texts.len() {
            for word in block_texts[i].split_whitespace() {
                let w = word.trim_matches(|c: char| !c.is_alphanumeric());
                if w.len() > 2 && !stopwords.contains(&w.to_lowercase().as_str()) {
                    *word_counts.entry(w).or_insert(0) += 1;
                }
            }
        }
    }

    let mut words: Vec<(&&str, &usize)> = word_counts.iter().collect();
    words.sort_by(|a, b| b.1.cmp(a.1));

    words
        .iter()
        .take(3)
        .map(|(w, _)| **w)
        .collect::<Vec<&str>>()
        .join("-")
}

// ─── Binary I/O ─────────────────────────────────────
//
// archetypes.bin format:
//   magic: b"ARC1" (4 bytes)
//   next_id: u32 (4 bytes)
//   count: u32 (4 bytes)
//   per archetype:
//     id: u32, cx/cy/cz: f32×3, strength: f32
//     reinforcement_count: u32, emerged_ms: u64, last_reinforced_ms: u64
//     member_count: u16, members: [member_count × u32]
//     label_len: u16, label_bytes: [label_len]

fn load_archetypes(output_dir: &Path) -> Option<ArchetypeState> {
    let path = output_dir.join("archetypes.bin");
    let data = fs::read(&path).ok()?;
    if data.len() < 12 || &data[0..4] != b"ARC1" {
        return None;
    }

    let next_id = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

    let mut pos = 12;
    let mut archetypes = Vec::with_capacity(count);

    for _ in 0..count {
        if pos + 40 > data.len() {
            break;
        }

        let id = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        let cx = f32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
        let cy = f32::from_le_bytes(data[pos + 8..pos + 12].try_into().unwrap());
        let cz = f32::from_le_bytes(data[pos + 12..pos + 16].try_into().unwrap());
        let strength = f32::from_le_bytes(data[pos + 16..pos + 20].try_into().unwrap());
        let reinforcement_count = u32::from_le_bytes(data[pos + 20..pos + 24].try_into().unwrap());
        let emerged_ms = u64::from_le_bytes(data[pos + 24..pos + 32].try_into().unwrap());
        let last_reinforced_ms = u64::from_le_bytes(data[pos + 32..pos + 40].try_into().unwrap());
        pos += 40;

        if pos + 2 > data.len() {
            break;
        }
        let member_count = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;

        let mut members = Vec::with_capacity(member_count);
        for _ in 0..member_count {
            if pos + 4 > data.len() {
                break;
            }
            members.push(u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()));
            pos += 4;
        }

        if pos + 2 > data.len() {
            break;
        }
        let label_len = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;

        let label = if pos + label_len <= data.len() {
            let s = String::from_utf8_lossy(&data[pos..pos + label_len]).to_string();
            pos += label_len;
            s
        } else {
            String::new()
        };

        archetypes.push(Archetype {
            id,
            centroid: (cx, cy, cz),
            members,
            strength,
            reinforcement_count,
            emerged_ms,
            last_reinforced_ms,
            label,
        });
    }

    Some(ArchetypeState {
        archetypes,
        next_id,
    })
}

fn save_archetypes(output_dir: &Path, state: &ArchetypeState) -> Result<(), String> {
    let path = output_dir.join("archetypes.bin");
    let mut buf = Vec::new();

    buf.extend_from_slice(b"ARC1");
    buf.extend_from_slice(&state.next_id.to_le_bytes());
    buf.extend_from_slice(&(state.archetypes.len() as u32).to_le_bytes());

    for a in &state.archetypes {
        buf.extend_from_slice(&a.id.to_le_bytes());
        buf.extend_from_slice(&a.centroid.0.to_le_bytes());
        buf.extend_from_slice(&a.centroid.1.to_le_bytes());
        buf.extend_from_slice(&a.centroid.2.to_le_bytes());
        buf.extend_from_slice(&a.strength.to_le_bytes());
        buf.extend_from_slice(&a.reinforcement_count.to_le_bytes());
        buf.extend_from_slice(&a.emerged_ms.to_le_bytes());
        buf.extend_from_slice(&a.last_reinforced_ms.to_le_bytes());

        buf.extend_from_slice(&(a.members.len() as u16).to_le_bytes());
        for &m in &a.members {
            buf.extend_from_slice(&m.to_le_bytes());
        }

        let label_bytes = a.label.as_bytes();
        buf.extend_from_slice(&(label_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(label_bytes);
    }

    fs::write(&path, &buf).map_err(|e| format!("write archetypes.bin: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_dist() {
        assert!((spatial_dist((0.0, 0.0, 0.0), (1.0, 0.0, 0.0)) - 1.0).abs() < 0.001);
        assert!(spatial_dist((0.1, 0.2, 0.3), (0.1, 0.2, 0.3)) < 0.001);
    }

    #[test]
    fn test_generate_label() {
        let texts = [
            "hello world Rust",
            "Rust memory system",
            "Rust binary format",
        ];
        let members = vec![0, 1, 2];
        let label = generate_label(&members, &texts);
        assert!(label.contains("Rust"));
    }

    #[test]
    fn test_reinforce() {
        let mut state = ArchetypeState {
            archetypes: vec![Archetype {
                id: 1,
                centroid: (0.1, 0.2, 0.3),
                members: vec![0, 1, 2],
                strength: 1.0,
                reinforcement_count: 1,
                emerged_ms: 1000,
                last_reinforced_ms: 1000,
                label: "test".to_string(),
            }],
            next_id: 2,
        };

        state.reinforce(&[(0, 0.9), (1, 0.5), (5, 0.3)]);

        assert!(state.archetypes[0].strength > 1.0); // reinforced
        assert_eq!(state.archetypes[0].reinforcement_count, 2);
    }

    #[test]
    fn test_match_archetype() {
        let state = ArchetypeState {
            archetypes: vec![
                Archetype {
                    id: 1,
                    centroid: (0.1, 0.2, 0.3),
                    members: vec![0, 1, 2],
                    strength: 2.0,
                    reinforcement_count: 5,
                    emerged_ms: 1000,
                    last_reinforced_ms: 2000,
                    label: "alpha".to_string(),
                },
                Archetype {
                    id: 2,
                    centroid: (0.5, 0.5, 0.5),
                    members: vec![10, 11, 12],
                    strength: 1.0,
                    reinforcement_count: 2,
                    emerged_ms: 1500,
                    last_reinforced_ms: 1800,
                    label: "beta".to_string(),
                },
            ],
            next_id: 3,
        };

        // Activate blocks that overlap with archetype "alpha"
        let result = state.match_archetype(&[(0, 0.9), (1, 0.7), (2, 0.5)]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, 0); // matched "alpha"

        // No overlap
        let result2 = state.match_archetype(&[(99, 0.9)]);
        assert!(result2.is_none());
    }

    #[test]
    fn test_decay() {
        let mut state = ArchetypeState {
            archetypes: vec![
                Archetype {
                    id: 1,
                    centroid: (0.0, 0.0, 0.0),
                    members: vec![0, 1, 2],
                    strength: 5.0,
                    reinforcement_count: 10,
                    emerged_ms: 1000,
                    last_reinforced_ms: 2000,
                    label: "strong".to_string(),
                },
                Archetype {
                    id: 2,
                    centroid: (0.5, 0.5, 0.5),
                    members: vec![3, 4, 5],
                    strength: 0.05, // weak, few reinforcements
                    reinforcement_count: 1,
                    emerged_ms: 1000,
                    last_reinforced_ms: 1000,
                    label: "weak".to_string(),
                },
            ],
            next_id: 3,
        };

        state.decay();

        // Strong archetype survives
        assert_eq!(state.archetypes.len(), 1);
        assert_eq!(state.archetypes[0].id, 1);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let dir = tmp.path();

        let mut state = ArchetypeState {
            archetypes: Vec::new(),
            next_id: 1,
        };

        state.archetypes.push(Archetype {
            id: 1,
            centroid: (0.1, 0.2, 0.3),
            members: vec![0, 5, 10],
            strength: 2.5,
            reinforcement_count: 7,
            emerged_ms: 12345,
            last_reinforced_ms: 67890,
            label: "test-archetype".to_string(),
        });
        state.next_id = 2;

        state.save(dir).expect("save");

        let loaded = ArchetypeState::load_or_init(dir);
        assert_eq!(loaded.archetypes.len(), 1);
        assert_eq!(loaded.archetypes[0].id, 1);
        assert!((loaded.archetypes[0].centroid.0 - 0.1).abs() < 0.001);
        assert_eq!(loaded.archetypes[0].members, vec![0, 5, 10]);
        assert_eq!(loaded.archetypes[0].label, "test-archetype");
        assert_eq!(loaded.next_id, 2);
    }

    #[test]
    fn test_detect_no_field() {
        let resonance = ResonanceState {
            instance_id: 42,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            field: HashMap::new(), // empty field
        };

        let hebb = hebbian::HebbianState {
            activations: vec![hebbian::ActivationRecord::default(); 5],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        let headers = vec![(0.1, 0.2, 0.3); 5];
        let texts: Vec<&str> = vec!["a"; 5];

        let mut state = ArchetypeState {
            archetypes: Vec::new(),
            next_id: 1,
        };

        let emerged = state.detect(&resonance, &hebb, &headers, &texts);
        assert_eq!(emerged, 0);
    }

    #[test]
    fn test_stats() {
        let state = ArchetypeState {
            archetypes: vec![
                Archetype {
                    id: 1,
                    centroid: (0.0, 0.0, 0.0),
                    members: vec![0, 1, 2],
                    strength: 3.0,
                    reinforcement_count: 5,
                    emerged_ms: 1000,
                    last_reinforced_ms: 2000,
                    label: "alpha".to_string(),
                },
                Archetype {
                    id: 2,
                    centroid: (0.5, 0.5, 0.5),
                    members: vec![3, 4],
                    strength: 1.0,
                    reinforcement_count: 2,
                    emerged_ms: 1500,
                    last_reinforced_ms: 1800,
                    label: "beta".to_string(),
                },
            ],
            next_id: 3,
        };

        let stats = state.stats();
        assert_eq!(stats.archetype_count, 2);
        assert_eq!(stats.total_members, 5);
        assert_eq!(stats.strongest_label.unwrap(), "alpha");
    }
}
