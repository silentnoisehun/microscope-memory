//! 21D Emotional State  wave-based emotional bias for search coordinates.
//!
//! Provides a persistent 21D emotional state vector and a bias function
//! that warps search coordinates based on the current emotional state.

use std::fs;
use std::path::Path;

const MAGIC: &[u8; 4] = b"E21D";
const STATE_SIZE: usize = 4 + 21 * 4; // magic + 21 f32 values

/// Persistent 21D emotional state.
pub struct EmotionalState21D {
    pub vector: [f32; 21],
}

impl EmotionalState21D {
    /// Load from file or create default (neutral).
    pub fn load_or_init(path: &Path) -> Self {
        if path.exists() {
            if let Ok(data) = fs::read(path) {
                if data.len() >= STATE_SIZE && &data[0..4] == MAGIC {
                    let mut vector = [0.0f32; 21];
                    for i in 0..21 {
                        let off = 4 + i * 4;
                        vector[i] = f32::from_le_bytes(
                            data[off..off + 4].try_into().unwrap_or([0u8; 4]),
                        );
                    }
                    return Self { vector };
                }
            }
        }
        Self { vector: [0.0f32; 21] }
    }

    /// Save to file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut buf = Vec::with_capacity(STATE_SIZE);
        buf.extend_from_slice(MAGIC);
        for &v in &self.vector {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        fs::write(path, &buf).map_err(|e| format!("write emotion_21d.bin: {}", e))
    }
}

/// Compute a 3D bias vector from the 21D emotional state.
/// The first 3 components of the emotion vector are used as the bias direction,
/// scaled by the overall emotional intensity.
pub fn emotion_21d_bias(state: &EmotionalState21D) -> (f32, f32, f32) {
    let intensity: f32 = state.vector.iter().map(|x| x * x).sum::<f32>().sqrt().min(1.0);
    let dx = state.vector[0].clamp(-1.0, 1.0) * intensity;
    let dy = state.vector[1].clamp(-1.0, 1.0) * intensity;
    let dz = state.vector[2].clamp(-1.0, 1.0) * intensity;
    (dx * 0.1, dy * 0.1, dz * 0.1)
}
