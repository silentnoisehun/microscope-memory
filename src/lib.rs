//! Microscope Memory library interface.
//! Re-exports core types and functions for integration tests and external use.

pub mod archetype;
pub mod build;
#[cfg(not(target_arch = "wasm32"))]
pub mod cache;
pub mod config;
pub mod embedding_index;
pub mod embeddings;
pub mod emotional;
#[cfg(not(target_arch = "wasm32"))]
pub mod federation;
pub mod fingerprint;
pub mod hebbian;
#[cfg(not(target_arch = "wasm32"))]
pub mod mcp;
pub mod merkle;
pub mod mirror;
pub mod query;
pub mod reader;
pub mod resonance;
pub mod snapshot;
#[cfg(not(target_arch = "wasm32"))]
pub mod streaming;

pub mod thought_graph;
pub mod viz;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(feature = "python")]
#[allow(non_local_definitions)]
pub mod python;

#[cfg(feature = "gpu")]
pub mod gpu;

pub mod cli;

// Re-export commonly used items
pub use reader::{
    read_append_log, store_memory, AppendEntry, BlockHeader, DataStore, MicroscopeReader,
    RadialResult, ResultSet,
};

// Re-export CLI
pub use cli::{Cli, Cmd};

// ─── Shared constants ────────────────────────────────
pub const DEFAULT_CONFIG_PATH: &str = "config.toml";
pub const BLOCK_DATA_SIZE: usize = 256;
pub const HEADER_SIZE: usize = 32;
pub const META_HEADER_SIZE: usize = 16;
pub const DEPTH_ENTRY_SIZE: usize = 8;
pub const LAYER_NAMES: &[&str] = &[
    "identity",
    "long_term",
    "short_term",
    "associative",
    "emotional",
    "relational",
    "reflections",
    "crypto_chain",
    "echo_cache",
    "rust_state",
];

// ─── Shared utility functions ────────────────────────

pub fn layer_to_id(name: &str) -> u8 {
    LAYER_NAMES.iter().position(|&n| n == name).unwrap_or(0) as u8
}

/// CRC16-CCITT (poly=0x1021, init=0xFFFF) over arbitrary data.
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub fn content_coords(text: &str, layer: &str) -> (f32, f32, f32) {
    let mut h: [u64; 3] = [0xcbf29ce484222325, 0x100000001b3, 0xa5a5a5a5a5a5a5a5];
    for &b in text.as_bytes().iter().take(128) {
        h[0] = h[0].wrapping_mul(0x100000001b3) ^ b as u64;
        h[1] = h[1].wrapping_mul(0x100000001b3) ^ b as u64;
        h[2] = h[2].wrapping_mul(0x1000193) ^ b as u64;
    }
    let bx = (h[0] & 0xFFFF) as f32 / 65535.0;
    let by = (h[1] & 0xFFFF) as f32 / 65535.0;
    let bz = (h[2] & 0xFFFF) as f32 / 65535.0;

    let (ox, oy, oz) = match layer {
        "long_term" => (0.0, 0.0, 0.0),
        "associative" => (0.3, 0.0, 0.0),
        "emotional" => (0.0, 0.3, 0.0),
        "relational" => (0.3, 0.3, 0.0),
        "reflections" => (0.0, 0.0, 0.3),
        "crypto_chain" => (0.3, 0.0, 0.3),
        "echo_cache" => (0.0, 0.3, 0.3),
        "short_term" => (0.15, 0.15, 0.15),
        "rust_state" => (0.15, 0.0, 0.15),
        _ => (0.25, 0.25, 0.25),
    };

    (ox + bx * 0.25, oy + by * 0.25, oz + bz * 0.25)
}

fn semantic_coords(text: &str, weight: f32) -> Option<(f32, f32, f32)> {
    if weight <= 0.0 {
        return None;
    }
    use embeddings::{EmbeddingProvider, MockEmbeddingProvider};
    let provider = MockEmbeddingProvider::new(128);
    if let Ok(emb) = provider.embed(text) {
        if emb.len() >= 3 {
            let sx = (emb[0] + 1.0) / 2.0;
            let sy = (emb[1] + 1.0) / 2.0;
            let sz = (emb[2] + 1.0) / 2.0;
            return Some((sx, sy, sz));
        }
    }
    None
}

pub fn content_coords_blended(text: &str, layer: &str, weight: f32) -> (f32, f32, f32) {
    let (hx, hy, hz) = content_coords(text, layer);
    if weight <= 0.0 {
        return (hx, hy, hz);
    }
    match semantic_coords(text, weight) {
        Some((sx, sy, sz)) => {
            let w = weight.clamp(0.0, 1.0);
            (
                (1.0 - w) * hx + w * sx,
                (1.0 - w) * hy + w * sy,
                (1.0 - w) * hz + w * sz,
            )
        }
        None => (hx, hy, hz),
    }
}

pub fn hex_str(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

pub fn safe_truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

pub fn to_block(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    if bytes.len() <= BLOCK_DATA_SIZE {
        bytes.to_vec()
    } else {
        let mut v = bytes[..BLOCK_DATA_SIZE - 3].to_vec();
        v.extend_from_slice(b"...");
        v
    }
}

// ─── AUTO ZOOM / AUTO DEPTH ──────────────────────────

pub fn auto_zoom(query: &str) -> (u8, u8) {
    let stopwords = ["a", "the", "is", "of", "and", "to", "in", "it", "on", "for"];
    let unique_content_words = query
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !stopwords.contains(w) && w.len() > 2)
        .count();

    if unique_content_words <= 1 {
        return (1, 1);
    }
    if unique_content_words <= 3 {
        return (2, 1);
    }
    if unique_content_words <= 6 {
        return (3, 1);
    }
    if unique_content_words <= 10 {
        return (4, 1);
    }
    (5, 1)
}

pub fn auto_depth(text: &str) -> u8 {
    let len = text.len();
    if len >= 100 {
        3
    } else if len >= 40 {
        4
    } else if len >= 15 {
        5
    } else {
        6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_ccitt_known_vector() {
        let data = b"123456789";
        assert_eq!(crc16_ccitt(data), 0x29B1);
    }

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16_ccitt(b""), 0xFFFF);
    }

    #[test]
    fn test_crc16_deterministic() {
        let a = crc16_ccitt(b"hello world");
        let b = crc16_ccitt(b"hello world");
        assert_eq!(a, b);
        assert_ne!(a, crc16_ccitt(b"hello worl!"));
    }
}
