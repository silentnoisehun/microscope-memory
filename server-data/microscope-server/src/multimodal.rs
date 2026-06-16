//! Multi-Modal Memory for Microscope Memory.
//!
//! Extends beyond text to store and recall images (perceptual hashes),
//! audio (spectral fingerprints), and structured data (typed key-value pairs)
//! within the same spatial coordinate framework.
//!
//! Binary format: modalities.bin (MOD1)
//!
//! Modalities are stored as a sidecar index — the core BlockHeader (32B mmap'd)
//! is unchanged. Each entry maps a block_idx to its modality metadata.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ─── Constants ──────────────────────────────────────

const IMAGE_PAYLOAD_BYTES: usize = 32; // 2+2+8+12+4+4
const AUDIO_PAYLOAD_BYTES: usize = 32; // 4+2+16+4+4+2

// ─── Modality types ─────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Modality {
    Text,
    Image(ImageMeta),
    Audio(AudioMeta),
    Structured(StructuredMeta),
}

impl Modality {
    pub fn tag(&self) -> u8 {
        match self {
            Modality::Text => 0,
            Modality::Image(_) => 1,
            Modality::Audio(_) => 2,
            Modality::Structured(_) => 3,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Modality::Text => "text",
            Modality::Image(_) => "image",
            Modality::Audio(_) => "audio",
            Modality::Structured(_) => "structured",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageMeta {
    pub width: u16,
    pub height: u16,
    pub phash: [u8; 8],
    pub color_histogram: [u8; 12],
    pub content_hash: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioMeta {
    pub duration_ms: u32,
    pub sample_rate: u16,
    pub spectral_fingerprint: [u8; 16],
    pub peak_freq: f32,
    pub bpm_estimate: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructuredMeta {
    pub fields: Vec<(String, FieldValue)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl FieldValue {
    pub fn type_tag(&self) -> u8 {
        match self {
            FieldValue::Str(_) => 0,
            FieldValue::Int(_) => 1,
            FieldValue::Float(_) => 2,
            FieldValue::Bool(_) => 3,
        }
    }
}

// ─── ModalityIndex ──────────────────────────────────

/// Sidecar index mapping block indices to modality metadata.
pub struct ModalityIndex {
    pub entries: HashMap<u32, Modality>,
}

pub struct ModalityStats {
    pub total_entries: usize,
    pub text_count: usize,
    pub image_count: usize,
    pub audio_count: usize,
    pub structured_count: usize,
}

impl ModalityIndex {
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("modalities.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 8 && &data[0..4] == b"MOD1" {
                let entry_count = read_u32(&data, 4) as usize;
                let mut entries = HashMap::new();
                let mut pos = 8;

                for _ in 0..entry_count {
                    if pos + 7 > data.len() {
                        break;
                    }
                    let block_idx = read_u32(&data, pos);
                    let modality_tag = data[pos + 4];
                    let payload_len = read_u16(&data, pos + 5) as usize;
                    pos += 7;

                    if pos + payload_len > data.len() {
                        break;
                    }

                    let modality = match modality_tag {
                        0 => Modality::Text,
                        1 if payload_len >= IMAGE_PAYLOAD_BYTES => {
                            let m = decode_image_meta(&data[pos..pos + payload_len]);
                            Modality::Image(m)
                        }
                        2 if payload_len >= AUDIO_PAYLOAD_BYTES => {
                            let m = decode_audio_meta(&data[pos..pos + payload_len]);
                            Modality::Audio(m)
                        }
                        3 => {
                            if let Some(m) = decode_structured_meta(&data[pos..pos + payload_len]) {
                                Modality::Structured(m)
                            } else {
                                pos += payload_len;
                                continue;
                            }
                        }
                        _ => {
                            pos += payload_len;
                            continue;
                        }
                    };

                    entries.insert(block_idx, modality);
                    pos += payload_len;
                }

                return Self { entries };
            }
        }
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("modalities.bin");
        let mut buf = Vec::new();
        buf.extend_from_slice(b"MOD1");
        buf.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());

        for (&block_idx, modality) in &self.entries {
            buf.extend_from_slice(&block_idx.to_le_bytes());
            buf.push(modality.tag());

            match modality {
                Modality::Text => {
                    buf.extend_from_slice(&0u16.to_le_bytes());
                }
                Modality::Image(m) => {
                    buf.extend_from_slice(&(IMAGE_PAYLOAD_BYTES as u16).to_le_bytes());
                    encode_image_meta(m, &mut buf);
                }
                Modality::Audio(m) => {
                    buf.extend_from_slice(&(AUDIO_PAYLOAD_BYTES as u16).to_le_bytes());
                    encode_audio_meta(m, &mut buf);
                }
                Modality::Structured(m) => {
                    let payload = encode_structured_meta(m);
                    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
                    buf.extend_from_slice(&payload);
                }
            }
        }

        fs::write(&path, &buf).map_err(|e| format!("write modalities.bin: {}", e))
    }

    /// Register a block's modality.
    pub fn register(&mut self, block_idx: u32, modality: Modality) {
        self.entries.insert(block_idx, modality);
    }

    /// Get modality for a block.
    pub fn get(&self, block_idx: u32) -> Option<&Modality> {
        self.entries.get(&block_idx)
    }

    /// Search by perceptual hash similarity (hamming distance).
    /// Returns Vec of (block_idx, hamming_distance), sorted by distance.
    pub fn search_image_similar(&self, phash: &[u8; 8], max_distance: u32) -> Vec<(u32, u32)> {
        let mut results: Vec<(u32, u32)> = self
            .entries
            .iter()
            .filter_map(|(&idx, m)| {
                if let Modality::Image(img) = m {
                    let dist = hamming_distance(&img.phash, phash);
                    if dist <= max_distance {
                        Some((idx, dist))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        results.sort_by_key(|&(_, d)| d);
        results
    }

    /// Search by spectral fingerprint similarity.
    /// Returns Vec of (block_idx, similarity_score), sorted by score descending.
    pub fn search_audio_similar(&self, fingerprint: &[u8; 16], threshold: f32) -> Vec<(u32, f32)> {
        let mut results: Vec<(u32, f32)> = self
            .entries
            .iter()
            .filter_map(|(&idx, m)| {
                if let Modality::Audio(aud) = m {
                    let sim = spectral_similarity(&aud.spectral_fingerprint, fingerprint);
                    if sim >= threshold {
                        Some((idx, sim))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results
    }

    /// Search structured data by field name and value.
    pub fn search_structured(&self, field: &str, value: &FieldValue) -> Vec<u32> {
        self.entries
            .iter()
            .filter_map(|(&idx, m)| {
                if let Modality::Structured(s) = m {
                    if s.fields.iter().any(|(k, v)| k == field && v == value) {
                        Some(idx)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compute spatial coordinates for a modality.
    /// Images: from phash, Audio: from spectral features, Structured: from field hashing.
    pub fn modality_coords(modality: &Modality) -> (f32, f32, f32) {
        match modality {
            Modality::Text => (0.0, 0.0, 0.0), // handled by normal content_coords
            Modality::Image(m) => {
                // Derive from phash bytes
                let x = (m.phash[0] as f32 + m.phash[1] as f32) / 510.0 * 0.25;
                let y = (m.phash[2] as f32 + m.phash[3] as f32) / 510.0 * 0.25;
                let z = (m.phash[4] as f32 + m.phash[5] as f32) / 510.0 * 0.25;
                // Offset into associative region
                (0.3 + x, 0.0 + y, 0.0 + z)
            }
            Modality::Audio(m) => {
                // Derive from spectral features
                let x = (m.peak_freq / 20000.0).clamp(0.0, 1.0) * 0.25;
                let y = (m.bpm_estimate / 300.0).clamp(0.0, 1.0) * 0.25;
                let z = (m.duration_ms as f32 / 600_000.0).clamp(0.0, 1.0) * 0.25;
                // Offset into echo_cache region
                (0.0 + x, 0.3 + y, 0.3 + z)
            }
            Modality::Structured(m) => {
                // Hash field names for coordinates
                let mut h: u64 = 0xcbf29ce484222325;
                for (k, _) in &m.fields {
                    for &b in k.as_bytes() {
                        h = h.wrapping_mul(0x100000001b3) ^ b as u64;
                    }
                }
                let x = ((h & 0xFFFF) as f32 / 65535.0) * 0.25;
                let y = (((h >> 16) & 0xFFFF) as f32 / 65535.0) * 0.25;
                let z = (((h >> 32) & 0xFFFF) as f32 / 65535.0) * 0.25;
                // Offset into rust_state region
                (0.15 + x, 0.0 + y, 0.15 + z)
            }
        }
    }

    pub fn stats(&self) -> ModalityStats {
        let mut text = 0;
        let mut image = 0;
        let mut audio = 0;
        let mut structured = 0;
        for m in self.entries.values() {
            match m {
                Modality::Text => text += 1,
                Modality::Image(_) => image += 1,
                Modality::Audio(_) => audio += 1,
                Modality::Structured(_) => structured += 1,
            }
        }
        ModalityStats {
            total_entries: self.entries.len(),
            text_count: text,
            image_count: image,
            audio_count: audio,
            structured_count: structured,
        }
    }
}

// ─── Perceptual hashing ─────────────────────────────

/// Compute perceptual hash (dHash-like) from grayscale pixel data.
/// Expects row-major grayscale pixels.
pub fn compute_phash(pixels: &[u8], width: u32, height: u32) -> [u8; 8] {
    // Downsample to 9x8 grid and compute differences
    let mut hash = [0u8; 8];
    if width < 2 || height < 2 || pixels.len() < (width * height) as usize {
        return hash;
    }

    let mut bit_idx = 0;
    for row in 0..8u32 {
        let src_y = (row * height / 8).min(height - 1);
        for col in 0..8u32 {
            let src_x1 = (col * width / 8).min(width - 1);
            let src_x2 = ((col + 1) * width / 8).min(width - 1);
            let p1 = pixels[(src_y * width + src_x1) as usize];
            let p2 = pixels[(src_y * width + src_x2) as usize];
            if p1 > p2 {
                hash[bit_idx / 8] |= 1 << (bit_idx % 8);
            }
            bit_idx += 1;
        }
    }
    hash
}

/// Simple spectral fingerprint from audio samples.
/// Divides into 16 frequency bands and computes energy per band.
pub fn compute_spectral_fingerprint(samples: &[f32], _sample_rate: u32) -> [u8; 16] {
    let mut fingerprint = [0u8; 16];
    if samples.is_empty() {
        return fingerprint;
    }

    let band_size = (samples.len() / 16).max(1);
    for (i, fp_byte) in fingerprint.iter_mut().enumerate() {
        let start = i * band_size;
        let end = ((i + 1) * band_size).min(samples.len());
        let energy: f32 =
            samples[start..end].iter().map(|s| s * s).sum::<f32>() / (end - start) as f32;
        *fp_byte = (energy.sqrt() * 255.0).clamp(0.0, 255.0) as u8;
    }
    fingerprint
}

/// Hamming distance between two byte arrays.
pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x ^ y).count_ones())
        .sum()
}

/// Spectral similarity: normalized dot product of fingerprints.
fn spectral_similarity(a: &[u8; 16], b: &[u8; 16]) -> f32 {
    let dot: u32 = a
        .iter()
        .zip(b.iter())
        .map(|(&x, &y)| x as u32 * y as u32)
        .sum();
    let mag_a: f32 = a
        .iter()
        .map(|&x| (x as f32) * (x as f32))
        .sum::<f32>()
        .sqrt();
    let mag_b: f32 = b
        .iter()
        .map(|&x| (x as f32) * (x as f32))
        .sum::<f32>()
        .sqrt();
    if mag_a < 0.001 || mag_b < 0.001 {
        return 0.0;
    }
    dot as f32 / (mag_a * mag_b)
}

// ─── Binary encoding helpers ────────────────────────

fn encode_image_meta(m: &ImageMeta, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&m.width.to_le_bytes());
    buf.extend_from_slice(&m.height.to_le_bytes());
    buf.extend_from_slice(&m.phash);
    buf.extend_from_slice(&m.color_histogram);
    buf.extend_from_slice(&m.content_hash.to_le_bytes());
    // pad to IMAGE_PAYLOAD_BYTES
    let written = 2 + 2 + 8 + 12 + 4; // 28
    for _ in written..IMAGE_PAYLOAD_BYTES {
        buf.push(0);
    }
}

fn decode_image_meta(data: &[u8]) -> ImageMeta {
    let width = u16::from_le_bytes(data[0..2].try_into().unwrap());
    let height = u16::from_le_bytes(data[2..4].try_into().unwrap());
    let mut phash = [0u8; 8];
    phash.copy_from_slice(&data[4..12]);
    let mut color_histogram = [0u8; 12];
    color_histogram.copy_from_slice(&data[12..24]);
    let content_hash = u32::from_le_bytes(data[24..28].try_into().unwrap());
    ImageMeta {
        width,
        height,
        phash,
        color_histogram,
        content_hash,
    }
}

fn encode_audio_meta(m: &AudioMeta, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&m.duration_ms.to_le_bytes());
    buf.extend_from_slice(&m.sample_rate.to_le_bytes());
    buf.extend_from_slice(&m.spectral_fingerprint);
    buf.extend_from_slice(&m.peak_freq.to_le_bytes());
    buf.extend_from_slice(&m.bpm_estimate.to_le_bytes());
    // pad to AUDIO_PAYLOAD_BYTES
    let written = 4 + 2 + 16 + 4 + 4; // 30
    for _ in written..AUDIO_PAYLOAD_BYTES {
        buf.push(0);
    }
}

fn decode_audio_meta(data: &[u8]) -> AudioMeta {
    let duration_ms = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let sample_rate = u16::from_le_bytes(data[4..6].try_into().unwrap());
    let mut spectral_fingerprint = [0u8; 16];
    spectral_fingerprint.copy_from_slice(&data[6..22]);
    let peak_freq = f32::from_le_bytes(data[22..26].try_into().unwrap());
    let bpm_estimate = f32::from_le_bytes(data[26..30].try_into().unwrap());
    AudioMeta {
        duration_ms,
        sample_rate,
        spectral_fingerprint,
        peak_freq,
        bpm_estimate,
    }
}

fn encode_structured_meta(m: &StructuredMeta) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(m.fields.len() as u8);
    for (key, val) in &m.fields {
        let key_bytes = key.as_bytes();
        buf.push(key_bytes.len().min(255) as u8);
        buf.extend_from_slice(&key_bytes[..key_bytes.len().min(255)]);
        buf.push(val.type_tag());
        match val {
            FieldValue::Str(s) => {
                let vb = s.as_bytes();
                buf.push(vb.len().min(255) as u8);
                buf.extend_from_slice(&vb[..vb.len().min(255)]);
            }
            FieldValue::Int(i) => {
                buf.push(8);
                buf.extend_from_slice(&i.to_le_bytes());
            }
            FieldValue::Float(f) => {
                buf.push(8);
                buf.extend_from_slice(&f.to_le_bytes());
            }
            FieldValue::Bool(b) => {
                buf.push(1);
                buf.push(if *b { 1 } else { 0 });
            }
        }
    }
    buf
}

fn decode_structured_meta(data: &[u8]) -> Option<StructuredMeta> {
    if data.is_empty() {
        return None;
    }
    let field_count = data[0] as usize;
    let mut pos = 1;
    let mut fields = Vec::new();

    for _ in 0..field_count {
        if pos >= data.len() {
            break;
        }
        let key_len = data[pos] as usize;
        pos += 1;
        if pos + key_len >= data.len() {
            break;
        }
        let key = String::from_utf8_lossy(&data[pos..pos + key_len]).to_string();
        pos += key_len;

        if pos + 2 > data.len() {
            break;
        }
        let type_tag = data[pos];
        let val_len = data[pos + 1] as usize;
        pos += 2;

        if pos + val_len > data.len() {
            break;
        }

        let value = match type_tag {
            0 => FieldValue::Str(String::from_utf8_lossy(&data[pos..pos + val_len]).to_string()),
            1 if val_len == 8 => {
                FieldValue::Int(i64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()))
            }
            2 if val_len == 8 => {
                FieldValue::Float(f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()))
            }
            3 if val_len >= 1 => FieldValue::Bool(data[pos] != 0),
            _ => {
                pos += val_len;
                continue;
            }
        };

        fields.push((key, value));
        pos += val_len;
    }

    Some(StructuredMeta { fields })
}

fn read_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn read_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phash_deterministic() {
        let pixels = vec![128u8; 64 * 64];
        let h1 = compute_phash(&pixels, 64, 64);
        let h2 = compute_phash(&pixels, 64, 64);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_phash_different_images() {
        // Ascending gradient — many left<right transitions
        let mut pixels1 = vec![0u8; 64 * 64];
        for (i, p) in pixels1.iter_mut().enumerate() {
            *p = ((i % 64) * 4) as u8; // 0..252 across each row
        }
        // Descending gradient — many left>right transitions
        let mut pixels2 = vec![0u8; 64 * 64];
        for (i, p) in pixels2.iter_mut().enumerate() {
            *p = (255 - (i % 64) * 4) as u8;
        }
        let h1 = compute_phash(&pixels1, 64, 64);
        let h2 = compute_phash(&pixels2, 64, 64);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hamming_distance() {
        let a = [0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00];
        let b = [0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00];
        assert_eq!(hamming_distance(&a, &b), 0);

        let c = [0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF];
        assert_eq!(hamming_distance(&a, &c), 64); // all bits differ
    }

    #[test]
    fn test_spectral_fingerprint_deterministic() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin()).collect();
        let fp1 = compute_spectral_fingerprint(&samples, 44100);
        let fp2 = compute_spectral_fingerprint(&samples, 44100);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_modality_coords() {
        let text_coords = ModalityIndex::modality_coords(&Modality::Text);
        assert_eq!(text_coords, (0.0, 0.0, 0.0));

        let img = Modality::Image(ImageMeta {
            width: 100,
            height: 100,
            phash: [128, 64, 32, 16, 8, 4, 2, 1],
            color_histogram: [0; 12],
            content_hash: 42,
        });
        let img_coords = ModalityIndex::modality_coords(&img);
        // Should be in associative region (0.3+, ...)
        assert!(img_coords.0 >= 0.3);

        let aud = Modality::Audio(AudioMeta {
            duration_ms: 30000,
            sample_rate: 44100,
            spectral_fingerprint: [0; 16],
            peak_freq: 440.0,
            bpm_estimate: 120.0,
        });
        let aud_coords = ModalityIndex::modality_coords(&aud);
        // Should be in echo_cache region
        assert!(aud_coords.1 >= 0.3);
    }

    #[test]
    fn test_image_meta_roundtrip() {
        let meta = ImageMeta {
            width: 1920,
            height: 1080,
            phash: [1, 2, 3, 4, 5, 6, 7, 8],
            color_histogram: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
            content_hash: 0xDEADBEEF,
        };
        let mut buf = Vec::new();
        encode_image_meta(&meta, &mut buf);
        let decoded = decode_image_meta(&buf);
        assert_eq!(meta, decoded);
    }

    #[test]
    fn test_audio_meta_roundtrip() {
        let meta = AudioMeta {
            duration_ms: 180000,
            sample_rate: 44100,
            spectral_fingerprint: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            peak_freq: 440.0,
            bpm_estimate: 120.5,
        };
        let mut buf = Vec::new();
        encode_audio_meta(&meta, &mut buf);
        let decoded = decode_audio_meta(&buf);
        assert_eq!(meta, decoded);
    }

    #[test]
    fn test_structured_meta_roundtrip() {
        let meta = StructuredMeta {
            fields: vec![
                ("name".to_string(), FieldValue::Str("test".to_string())),
                ("count".to_string(), FieldValue::Int(42)),
                ("ratio".to_string(), FieldValue::Float(3.125)),
                ("active".to_string(), FieldValue::Bool(true)),
            ],
        };
        let encoded = encode_structured_meta(&meta);
        let decoded = decode_structured_meta(&encoded).unwrap();
        assert_eq!(meta, decoded);
    }

    #[test]
    fn test_search_image_similar() {
        let mut index = ModalityIndex {
            entries: HashMap::new(),
        };
        index.register(
            0,
            Modality::Image(ImageMeta {
                width: 100,
                height: 100,
                phash: [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                color_histogram: [0; 12],
                content_hash: 1,
            }),
        );
        index.register(
            1,
            Modality::Image(ImageMeta {
                width: 100,
                height: 100,
                phash: [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE], // 1 bit diff
                color_histogram: [0; 12],
                content_hash: 2,
            }),
        );
        index.register(
            2,
            Modality::Image(ImageMeta {
                width: 100,
                height: 100,
                phash: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // max diff
                color_histogram: [0; 12],
                content_hash: 3,
            }),
        );

        let target = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let results = index.search_image_similar(&target, 5);
        assert_eq!(results.len(), 2); // blocks 0 and 1
        assert_eq!(results[0].1, 0); // exact match first
        assert_eq!(results[1].1, 1); // 1 bit diff second
    }

    #[test]
    fn test_search_structured_by_field() {
        let mut index = ModalityIndex {
            entries: HashMap::new(),
        };
        index.register(
            0,
            Modality::Structured(StructuredMeta {
                fields: vec![("type".to_string(), FieldValue::Str("report".to_string()))],
            }),
        );
        index.register(
            1,
            Modality::Structured(StructuredMeta {
                fields: vec![("type".to_string(), FieldValue::Str("note".to_string()))],
            }),
        );

        let results = index.search_structured("type", &FieldValue::Str("report".to_string()));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 0);
    }

    #[test]
    fn test_modality_index_save_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut index = ModalityIndex {
            entries: HashMap::new(),
        };

        index.register(
            0,
            Modality::Image(ImageMeta {
                width: 640,
                height: 480,
                phash: [1, 2, 3, 4, 5, 6, 7, 8],
                color_histogram: [10; 12],
                content_hash: 42,
            }),
        );
        index.register(
            1,
            Modality::Audio(AudioMeta {
                duration_ms: 60000,
                sample_rate: 44100,
                spectral_fingerprint: [5; 16],
                peak_freq: 440.0,
                bpm_estimate: 120.0,
            }),
        );
        index.register(
            2,
            Modality::Structured(StructuredMeta {
                fields: vec![("key".to_string(), FieldValue::Int(99))],
            }),
        );

        index.save(tmp.path()).unwrap();
        let loaded = ModalityIndex::load_or_init(tmp.path());
        assert_eq!(loaded.entries.len(), 3);
        assert!(matches!(loaded.get(0), Some(Modality::Image(_))));
        assert!(matches!(loaded.get(1), Some(Modality::Audio(_))));
        assert!(matches!(loaded.get(2), Some(Modality::Structured(_))));
    }
}
