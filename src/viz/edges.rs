use bytemuck::{Pod, Zeroable};
use crate::MicroscopeReader;
use crate::viz::scene::{PointInstance, layer_color_rgba};
use std::collections::HashSet;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EdgeVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Build edge vertices for parent-child lines among visible blocks.
pub fn build_edges(
    reader: &MicroscopeReader,
    instances: &[PointInstance],
    block_indices: &[usize],
) -> Vec<EdgeVertex> {
    let visible: HashSet<usize> = block_indices.iter().copied().collect();
    let mut edges = Vec::new();

    for (i, &bi) in block_indices.iter().enumerate() {
        let h = reader.header(bi);
        let parent = h.parent_idx as usize;
        if parent == u32::MAX as usize || !visible.contains(&parent) {
            continue;
        }

        let ph = reader.header(parent);
        let color = layer_color_rgba(h.layer_id);
        let parent_color = layer_color_rgba(ph.layer_id);

        // Start vertex (child)
        edges.push(EdgeVertex {
            position: instances[i].position,
            color,
        });

        // End vertex (parent) — find parent's instance index
        if let Some(pi) = block_indices.iter().position(|&x| x == parent) {
            edges.push(EdgeVertex {
                position: instances[pi].position,
                color: parent_color,
            });
        } else {
            // Parent not in visible set — use raw coords
            edges.push(EdgeVertex {
                position: [ph.x, ph.y, ph.z],
                color: parent_color,
            });
        }
    }

    edges
}
