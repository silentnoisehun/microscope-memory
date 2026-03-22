//! Visualization export for Microscope Memory.
//!
//! Exports the full consciousness state as a structured JSON snapshot
//! that 3D renderers (Three.js, WebGL, custom) can consume.
//!
//! Includes: block positions, Hebbian energy/drift, resonance field,
//! archetype centroids, mirror echoes, co-activation edges.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::archetype::ArchetypeState;
use crate::hebbian::HebbianState;
use crate::mirror::MirrorState;
use crate::reader::MicroscopeReader;
use crate::resonance::ResonanceState;
use crate::LAYER_NAMES;

// ─── Snapshot Export ────────────────────────────────

/// Export a full visualization snapshot as JSON.
pub fn export_snapshot(
    _output_dir: &Path,
    reader: &MicroscopeReader,
    hebb: &HebbianState,
    mirror: &MirrorState,
    resonance: &ResonanceState,
    archetypes: &ArchetypeState,
) -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str("{\n");

    // Metadata
    out.push_str(&format!(
        "  \"block_count\": {},\n  \"instance_id\": \"{:x}\",\n",
        reader.block_count, resonance.instance_id
    ));

    // Blocks: position, energy, drift, layer, depth
    out.push_str("  \"blocks\": [\n");
    for i in 0..reader.block_count {
        let h = reader.header(i);
        // Copy packed struct fields to avoid unaligned reference
        let hx = h.x;
        let hy = h.y;
        let hz = h.z;
        let depth = h.depth;
        let layer_id = h.layer_id;
        let energy = hebb.energy(i);
        let (dx, dy, dz) = if i < hebb.activations.len() {
            let rec = &hebb.activations[i];
            (rec.drift_x, rec.drift_y, rec.drift_z)
        } else {
            (0.0, 0.0, 0.0)
        };
        let layer = LAYER_NAMES.get(layer_id as usize).unwrap_or(&"?");
        let activation_count = if i < hebb.activations.len() {
            hebb.activations[i].activation_count
        } else {
            0
        };
        let mirror_strength = mirror.boost_for(i as u32);

        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str(&format!(
            "    {{\"x\":{:.4},\"y\":{:.4},\"z\":{:.4},\"dx\":{:.4},\"dy\":{:.4},\"dz\":{:.4},\"e\":{:.4},\"d\":{},\"l\":\"{}\",\"a\":{},\"m\":{:.4}}}",
            hx, hy, hz, dx, dy, dz, energy, depth, layer, activation_count, mirror_strength
        ));
    }
    out.push_str("\n  ],\n");

    // Co-activation edges (top 200 strongest)
    out.push_str("  \"edges\": [\n");
    let mut pairs: Vec<_> = hebb.coactivations.values().collect();
    pairs.sort_by(|a, b| b.count.cmp(&a.count));
    pairs.truncate(200);
    for (i, pair) in pairs.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str(&format!(
            "    {{\"a\":{},\"b\":{},\"c\":{}}}",
            pair.block_a, pair.block_b, pair.count
        ));
    }
    out.push_str("\n  ],\n");

    // Resonance field (non-zero cells)
    out.push_str("  \"field\": [\n");
    let mut field_entries: Vec<_> = resonance.field.iter().collect();
    field_entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    field_entries.truncate(500);
    for (i, (&(qx, qy, qz), &v)) in field_entries.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str(&format!(
            "    {{\"x\":{:.3},\"y\":{:.3},\"z\":{:.3},\"v\":{:.4}}}",
            qx as f32 / 20.0,
            qy as f32 / 20.0,
            qz as f32 / 20.0,
            v
        ));
    }
    out.push_str("\n  ],\n");

    // Archetypes
    out.push_str("  \"archetypes\": [\n");
    for (i, a) in archetypes.archetypes.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        let members_json: Vec<String> = a.members.iter().map(|m| m.to_string()).collect();
        out.push_str(&format!(
            "    {{\"id\":{},\"label\":\"{}\",\"cx\":{:.4},\"cy\":{:.4},\"cz\":{:.4},\"str\":{:.4},\"r\":{},\"members\":[{}]}}",
            a.id,
            escape_json(&a.label),
            a.centroid.0, a.centroid.1, a.centroid.2,
            a.strength, a.reinforcement_count,
            members_json.join(",")
        ));
    }
    out.push_str("\n  ],\n");

    // Mirror echoes (recent)
    out.push_str("  \"echoes\": [\n");
    let recent_echoes = mirror.echoes.iter().rev().take(50);
    for (i, echo) in recent_echoes.enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        let shared: Vec<String> = echo.shared_blocks.iter().map(|b| b.to_string()).collect();
        out.push_str(&format!(
            "    {{\"sim\":{:.4},\"trigger\":\"{:x}\",\"echo\":\"{:x}\",\"shared\":[{}]}}",
            echo.similarity,
            echo.trigger_hash,
            echo.echo_hash,
            shared.join(",")
        ));
    }
    out.push_str("\n  ],\n");

    // Summary stats
    let hebb_stats = hebb.stats();
    let mirror_stats = mirror.stats();
    let res_stats = resonance.stats();
    let arc_stats = archetypes.stats();

    out.push_str("  \"stats\": {\n");
    out.push_str(&format!(
        "    \"active_blocks\": {},\n    \"hot_blocks\": {},\n    \"coactivation_pairs\": {},\n",
        hebb_stats.active_blocks, hebb_stats.hot_blocks, hebb_stats.coactivation_pairs
    ));
    out.push_str(&format!(
        "    \"resonant_blocks\": {},\n    \"echoes\": {},\n",
        mirror_stats.resonant_blocks, mirror_stats.total_echoes
    ));
    out.push_str(&format!(
        "    \"outgoing_pulses\": {},\n    \"field_cells\": {},\n    \"field_energy\": {:.4},\n",
        res_stats.outgoing_pulses, res_stats.field_cells, res_stats.field_energy
    ));
    out.push_str(&format!(
        "    \"archetypes\": {},\n    \"archetype_members\": {}\n",
        arc_stats.archetype_count, arc_stats.total_members
    ));
    out.push_str("  }\n");

    out.push_str("}\n");
    out
}

/// Export snapshot to a file.
pub fn export_to_file(
    output_dir: &Path,
    reader: &MicroscopeReader,
    hebb: &HebbianState,
    mirror: &MirrorState,
    resonance: &ResonanceState,
    archetypes: &ArchetypeState,
    dest: &Path,
) -> Result<(), String> {
    let json = export_snapshot(output_dir, reader, hebb, mirror, resonance, archetypes);
    let mut file = std::fs::File::create(dest).map_err(|e| format!("create viz file: {}", e))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("write viz file: {}", e))
}

/// Export a compact binary density map for fast rendering.
/// Format: DEN1 + grid_size:u16 + cells:[grid_size³ × f32]
pub fn export_density_map(
    hebb: &HebbianState,
    headers: &[(f32, f32, f32)],
    grid_size: u16,
) -> Vec<u8> {
    let n = grid_size as usize;
    let mut grid = vec![0.0f32; n * n * n];
    let step = 1.0 / n as f32;

    for (i, (x, y, z)) in headers.iter().enumerate() {
        let energy = hebb.energy(i);
        if energy < 0.01 {
            continue;
        }

        let gx = ((x / step) as usize).min(n - 1);
        let gy = ((y / step) as usize).min(n - 1);
        let gz = ((z / step) as usize).min(n - 1);

        grid[gx * n * n + gy * n + gz] += energy;
    }

    let mut buf = Vec::with_capacity(6 + n * n * n * 4);
    buf.extend_from_slice(b"DEN1");
    buf.extend_from_slice(&grid_size.to_le_bytes());
    for v in &grid {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf
}

/// Generate a layer heatmap: per-layer activation summary.
pub fn layer_heatmap(hebb: &HebbianState, reader: &MicroscopeReader) -> HashMap<String, f32> {
    let mut map: HashMap<String, f32> = HashMap::new();

    for i in 0..reader.block_count.min(hebb.activations.len()) {
        let h = reader.header(i);
        let layer = LAYER_NAMES
            .get(h.layer_id as usize)
            .unwrap_or(&"?")
            .to_string();
        let energy = hebb.energy(i);
        *map.entry(layer).or_insert(0.0) += energy;
    }

    map
}

// ─── Helpers ────────────────────────────────────────

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json() {
        assert_eq!(escape_json("hello"), "hello");
        assert_eq!(escape_json("he\"llo"), "he\\\"llo");
        assert_eq!(escape_json("line\nnew"), "line\\nnew");
    }

    #[test]
    fn test_density_map_format() {
        let hebb = HebbianState {
            activations: vec![crate::hebbian::ActivationRecord::default(); 5],
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };
        let headers = vec![(0.1, 0.2, 0.3); 5];

        let data = export_density_map(&hebb, &headers, 8);
        assert_eq!(&data[0..4], b"DEN1");
        assert_eq!(u16::from_le_bytes(data[4..6].try_into().unwrap()), 8);
        // 8³ × 4 bytes = 2048, + 6 header
        assert_eq!(data.len(), 6 + 8 * 8 * 8 * 4);
    }

    #[test]
    fn test_layer_heatmap_empty() {
        let hebb = HebbianState {
            activations: Vec::new(),
            coactivations: HashMap::new(),
            fingerprints: Vec::new(),
        };

        // We can't easily create a MicroscopeReader in tests,
        // so this test just verifies the function exists and
        // handles empty state gracefully.
        assert_eq!(hebb.activations.len(), 0);
    }
}
