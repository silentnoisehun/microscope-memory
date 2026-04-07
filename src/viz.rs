//! Visualization export for Microscope Memory.
//!
//! ZERO JSON. Pure binary. mmap-ready.
//! Format: VIZ1 + block_count:u32 + blocks:[VizBlock] + edge_count:u32 + edges:[u32;3]

use std::path::Path;
use std::io::Write;
use crate::hebbian::HebbianState;
use crate::mirror::MirrorState;
use crate::reader::MicroscopeReader;
use crate::thought_graph::ThoughtGraphState;

/// Packed visualization block (32 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct VizBlock {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub energy: f32,
    pub layer_id: u8,
    pub depth: u8,
    pub flags: u8,
    pub padding: u8,
}

/// Export the full state as a raw binary buffer (VIZ1).
pub fn export_binary_snapshot(
    reader: &MicroscopeReader,
    hebb: &HebbianState,
    mirror: &MirrorState,
    thought_graph: &ThoughtGraphState,
) -> Vec<u8> {
    let block_count = reader.block_count;
    let mut buf = Vec::with_capacity(12 + block_count * 32 + 200 * 12 + 5000);
    
    // Header (4 bytes magic + 4 bytes block_count)
    buf.extend_from_slice(b"VIZ1");
    buf.extend_from_slice(&(block_count as u32).to_le_bytes());

    // Blocks (Zero-copy loop)
    for i in 0..block_count {
        let h = reader.header(i);
        let energy = hebb.energy(i);
        let (dx, dy, dz) = if i < hebb.activations.len() {
            let rec = &hebb.activations[i];
            (rec.drift_x, rec.drift_y, rec.drift_z)
        } else {
            (0.0, 0.0, 0.0)
        };
        
        let mirror_boost = mirror.boost_for(i as u32);
        let flags = if mirror_boost > 0.5 { 1 } else { 0 };

        let vb = VizBlock {
            x: h.x, y: h.y, z: h.z,
            dx, dy, dz,
            energy,
            layer_id: h.layer_id,
            depth: h.depth,
            flags,
            padding: 0,
        };
        
        let bytes: [u8; 32] = unsafe { std::mem::transmute(vb) };
        buf.extend_from_slice(&bytes);
    }

    // ThoughtGraph Nodes (recent 100)
    let recent_nodes = if thought_graph.nodes.len() > 100 {
        &thought_graph.nodes[thought_graph.nodes.len() - 100..]
    } else {
        &thought_graph.nodes
    };
    buf.extend_from_slice(&(recent_nodes.len() as u32).to_le_bytes());
    for n in recent_nodes {
        buf.extend_from_slice(&n.query_hash.to_le_bytes());
        buf.extend_from_slice(&n.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&(n.result_count as u32).to_le_bytes());
        buf.push(n.dominant_layer);
        buf.push(n.centroid_hash);
        buf.extend_from_slice(&[0u8; 2]); // padding to 24 bytes
    }

    // Patterns (top 20)
    let top_patterns = thought_graph.top_patterns(20);
    buf.extend_from_slice(&(top_patterns.len() as u32).to_le_bytes());
    for p in top_patterns {
        buf.extend_from_slice(&(p.id as u32).to_le_bytes());
        buf.extend_from_slice(&(p.frequency as u32).to_le_bytes());
        buf.extend_from_slice(&p.strength.to_le_bytes());
        buf.extend_from_slice(&(p.sequence.len() as u8).to_le_bytes());
        for &h in p.sequence.iter().take(5) {
            buf.extend_from_slice(&h.to_le_bytes());
        }
        for _ in p.sequence.len()..5 {
            buf.extend_from_slice(&0u64.to_le_bytes());
        }
    }

    // Edges (top 200 strongest)
    let mut pairs: Vec<_> = hebb.coactivations.values().collect();
    pairs.sort_by(|a, b| b.count.cmp(&a.count));
    pairs.truncate(200);
    
    buf.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for p in pairs {
        buf.extend_from_slice(&p.block_a.to_le_bytes());
        buf.extend_from_slice(&p.block_b.to_le_bytes());
        buf.extend_from_slice(&p.count.to_le_bytes());
    }

    buf
}

pub fn export_to_file(
    _output_dir: &Path,
    reader: &MicroscopeReader,
    hebb: &HebbianState,
    mirror: &MirrorState,
    thought_graph: &ThoughtGraphState,
    dest: &Path,
) -> Result<(), String> {
    let buf = export_binary_snapshot(reader, hebb, mirror, thought_graph);
    let mut file = std::fs::File::create(dest).map_err(|e| format!("create viz file: {}", e))?;
    file.write_all(&buf).map_err(|e| format!("write viz file: {}", e))
}
pub fn export_density_map(_hebb: &HebbianState, _headers: &[(f32, f32, f32)], _grid: u16) -> Vec<u8> { vec![] }
pub fn layer_heatmap(_hebb: &HebbianState, _reader: &MicroscopeReader) -> std::collections::HashMap<String, f32> { std::collections::HashMap::new() }
