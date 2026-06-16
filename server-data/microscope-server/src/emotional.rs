//! Emotional bias warp for Microscope Memory.
//!
//! Bends search space coordinates based on the emotional layer's active clusters.
//! Does NOT override the search — warps the query point toward emotional attractors.
//! The weight is configurable in config.toml (`search.emotional_bias_weight`).

use crate::hebbian::HebbianState;
use crate::reader::MicroscopeReader;
use crate::LAYER_NAMES;

/// The emotional layer ID (index 4 in LAYER_NAMES: "emotional").
const EMOTIONAL_LAYER_ID: u8 = 4;

/// Compute the emotional bias warp for a query point.
/// Returns warped (x, y, z) coordinates.
///
/// The warp pulls the query point toward the centroid of hot emotional blocks,
/// weighted by their Hebbian energy. With weight=0, the original coords are returned.
pub fn apply_emotional_bias(
    qx: f32,
    qy: f32,
    qz: f32,
    weight: f32,
    reader: &MicroscopeReader,
    hebb: &HebbianState,
) -> (f32, f32, f32) {
    if weight <= 0.0 {
        return (qx, qy, qz);
    }

    let weight = weight.clamp(0.0, 1.0);

    // Find active emotional blocks and their weighted centroid
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut sum_z = 0.0f32;
    let mut total_energy = 0.0f32;

    for i in 0..reader.block_count {
        let h = reader.header(i);
        if h.layer_id != EMOTIONAL_LAYER_ID {
            continue;
        }

        let energy = hebb.energy(i);
        if energy < 0.01 {
            continue;
        }

        // Copy packed struct fields
        let hx = h.x;
        let hy = h.y;
        let hz = h.z;

        sum_x += hx * energy;
        sum_y += hy * energy;
        sum_z += hz * energy;
        total_energy += energy;
    }

    if total_energy < 0.01 {
        return (qx, qy, qz); // No active emotional blocks
    }

    // Emotional centroid
    let cx = sum_x / total_energy;
    let cy = sum_y / total_energy;
    let cz = sum_z / total_energy;

    // Warp: blend query point toward emotional centroid
    let warped_x = qx + (cx - qx) * weight;
    let warped_y = qy + (cy - qy) * weight;
    let warped_z = qz + (cz - qz) * weight;

    (warped_x, warped_y, warped_z)
}

/// Get the current emotional field summary:
/// centroid of active emotional blocks, total energy, and count.
pub fn emotional_field(reader: &MicroscopeReader, hebb: &HebbianState) -> Option<EmotionalField> {
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut sum_z = 0.0f32;
    let mut total_energy = 0.0f32;
    let mut active_count = 0usize;
    let mut hottest_idx: Option<(usize, f32)> = None;

    for i in 0..reader.block_count {
        let h = reader.header(i);
        if h.layer_id != EMOTIONAL_LAYER_ID {
            continue;
        }

        let energy = hebb.energy(i);
        if energy < 0.01 {
            continue;
        }

        let hx = h.x;
        let hy = h.y;
        let hz = h.z;

        sum_x += hx * energy;
        sum_y += hy * energy;
        sum_z += hz * energy;
        total_energy += energy;
        active_count += 1;

        if hottest_idx.is_none() || energy > hottest_idx.unwrap().1 {
            hottest_idx = Some((i, energy));
        }
    }

    if active_count == 0 {
        return None;
    }

    Some(EmotionalField {
        centroid: (
            sum_x / total_energy,
            sum_y / total_energy,
            sum_z / total_energy,
        ),
        total_energy,
        active_blocks: active_count,
        hottest_block: hottest_idx,
    })
}

pub struct EmotionalField {
    pub centroid: (f32, f32, f32),
    pub total_energy: f32,
    pub active_blocks: usize,
    pub hottest_block: Option<(usize, f32)>,
}

/// Verify the emotional layer ID matches our constant.
pub fn emotional_layer_name() -> &'static str {
    LAYER_NAMES
        .get(EMOTIONAL_LAYER_ID as usize)
        .unwrap_or(&"emotional")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_no_warp_at_zero_weight() {
        let (x, _y, _z) = apply_emotional_bias_pure(0.5, 0.5, 0.5, 0.0, None);
        assert!((x - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_warp_with_centroid() {
        // Test the pure math: warp toward centroid
        let centroid = Some((0.2, 0.3, 0.4));
        let (x, y, z) = apply_emotional_bias_pure(0.5, 0.5, 0.5, 0.5, centroid);
        // Should move halfway toward centroid
        assert!((x - 0.35).abs() < 0.001);
        assert!((y - 0.4).abs() < 0.001);
        assert!((z - 0.45).abs() < 0.001);
    }

    #[test]
    fn test_warp_full_weight() {
        let centroid = Some((0.2, 0.3, 0.4));
        let (x, y, z) = apply_emotional_bias_pure(0.5, 0.5, 0.5, 1.0, centroid);
        // Should move fully to centroid
        assert!((x - 0.2).abs() < 0.001);
        assert!((y - 0.3).abs() < 0.001);
        assert!((z - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_no_centroid() {
        let (x, _y, _z) = apply_emotional_bias_pure(0.5, 0.5, 0.5, 1.0, None);
        // No centroid → no warp
        assert!((x - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_emotional_layer_name() {
        assert_eq!(emotional_layer_name(), "emotional");
    }

    /// Pure math version for unit testing without MicroscopeReader.
    fn apply_emotional_bias_pure(
        qx: f32,
        qy: f32,
        qz: f32,
        weight: f32,
        centroid: Option<(f32, f32, f32)>,
    ) -> (f32, f32, f32) {
        if weight <= 0.0 {
            return (qx, qy, qz);
        }
        match centroid {
            None => (qx, qy, qz),
            Some((cx, cy, cz)) => {
                let w = weight.clamp(0.0, 1.0);
                (qx + (cx - qx) * w, qy + (cy - qy) * w, qz + (cz - qz) * w)
            }
        }
    }
}
