use bytemuck::{Pod, Zeroable};
use crate::MicroscopeReader;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PointInstance {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
}

pub fn layer_color_rgba(layer_id: u8) -> [f32; 4] {
    match layer_id {
        0 => [1.0, 1.0, 1.0, 1.0],
        1 => [0.2, 0.4, 1.0, 1.0],
        2 => [0.0, 1.0, 1.0, 1.0],
        3 => [0.0, 1.0, 0.0, 1.0],
        4 => [1.0, 0.2, 0.2, 1.0],
        5 => [1.0, 1.0, 0.0, 1.0],
        6 => [1.0, 0.0, 1.0, 1.0],
        7 => [1.0, 0.5, 0.0, 1.0],
        8 => [0.5, 1.0, 0.0, 1.0],
        9 => [0.6, 0.2, 1.0, 1.0],
        _ => [0.5, 0.5, 0.5, 1.0],
    }
}

pub fn depth_to_size(depth: u8) -> f32 {
    match depth {
        0 => 0.025,
        1 => 0.018,
        2 => 0.012,
        3 => 0.008,
        4 => 0.005,
        5 => 0.003,
        6 => 0.002,
        7 => 0.0012,
        8 => 0.0008,
        _ => 0.003,
    }
}

pub struct SceneData {
    pub instances: Vec<PointInstance>,
    pub block_indices: Vec<usize>,
}

impl SceneData {
    pub fn from_reader(reader: &MicroscopeReader, visible_depths: &[bool; 9], visible_layers: &[bool; 10]) -> Self {
        let mut instances = Vec::new();
        let mut block_indices = Vec::new();

        for depth in 0..9u8 {
            if !visible_depths[depth as usize] { continue; }

            let (start, count) = reader.depth_ranges[depth as usize];
            let (start, count) = (start as usize, count as usize);

            for i in start..(start + count) {
                let h = reader.header(i);
                let lid = h.layer_id;
                if lid < 10 && !visible_layers[lid as usize] { continue; }

                instances.push(PointInstance {
                    position: [h.x, h.y, h.z],
                    size: depth_to_size(h.depth),
                    color: layer_color_rgba(lid),
                });
                block_indices.push(i);
            }
        }

        SceneData { instances, block_indices }
    }
}
