//! Embedding index: mmap-backed pre-computed embedding vectors.
//!
//! Format: [u32 block_count][u32 dim][u32 max_depth][f32 × dim × embedded_count]
//! Only blocks at depth 0..max_depth are embedded.

use std::fs;
use std::path::Path;

use rayon::prelude::*;

use crate::embeddings::{cosine_similarity_simd, EmbeddingProvider};

/// Mmap-backed embedding index for fast semantic lookup.
#[allow(dead_code)]
pub struct EmbeddingIndex {
    data: memmap2::Mmap,
    block_count: u32,
    dim: u32,
    max_depth: u32,
}

const HEADER_SIZE: usize = 12; // 3 × u32

impl EmbeddingIndex {
    /// Open an existing embeddings.bin file.
    pub fn open(path: &Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }
        let file = fs::File::open(path).ok()?;
        let data = unsafe { memmap2::Mmap::map(&file).ok()? };
        if data.len() < HEADER_SIZE {
            return None;
        }

        let block_count = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let dim = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let max_depth = u32::from_le_bytes(data[8..12].try_into().unwrap());

        let expected = HEADER_SIZE + block_count as usize * dim as usize * 4;
        if data.len() < expected {
            return None;
        }

        Some(EmbeddingIndex {
            data,
            block_count,
            dim,
            max_depth,
        })
    }

    /// Get embedding for block at index (zero-copy mmap access).
    pub fn embedding(&self, block_idx: usize) -> Option<&[f32]> {
        if block_idx >= self.block_count as usize {
            return None;
        }
        let offset = HEADER_SIZE + block_idx * self.dim as usize * 4;
        let end = offset + self.dim as usize * 4;
        if end > self.data.len() {
            return None;
        }
        // Safety: data is aligned to f32 by construction during build
        let ptr = self.data[offset..end].as_ptr() as *const f32;
        Some(unsafe { std::slice::from_raw_parts(ptr, self.dim as usize) })
    }

    /// Number of embedded blocks.
    pub fn block_count(&self) -> usize {
        self.block_count as usize
    }

    /// Embedding dimension.
    pub fn dim(&self) -> usize {
        self.dim as usize
    }

    /// Max depth that was embedded.
    #[allow(dead_code)]
    pub fn max_depth(&self) -> u8 {
        self.max_depth as u8
    }

    /// Search for top-k most similar blocks to query embedding.
    /// Returns Vec<(similarity, block_index)> sorted descending.
    pub fn search(&self, query_emb: &[f32], k: usize) -> Vec<(f32, usize)> {
        if query_emb.len() != self.dim as usize {
            return vec![];
        }

        let mut results: Vec<(f32, usize)> = (0..self.block_count as usize)
            .into_par_iter()
            .filter_map(|i| {
                let emb = self.embedding(i)?;
                // Check for zero embedding (unembedded block placeholder)
                let is_zero = emb.iter().all(|&v| v == 0.0);
                if is_zero {
                    return None;
                }
                let sim = cosine_similarity_simd(query_emb, emb);
                if sim > 0.3 {
                    Some((sim, i))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.truncate(k);
        results
    }
}

/// Build embedding index file from a provider and reader.
/// Only embeds blocks at depth 0..=max_depth.
pub fn build_embedding_index(
    provider: &dyn EmbeddingProvider,
    reader: &crate::MicroscopeReader,
    max_depth: u8,
    output_path: &Path,
) -> Result<(), String> {
    let dim = provider.dimension();

    // Count blocks to embed (depth 0..=max_depth)
    let mut embed_count = 0usize;
    for d in 0..=max_depth as usize {
        if d < reader.depth_ranges.len() {
            embed_count += reader.depth_ranges[d].1 as usize;
        }
    }

    println!(
        "  Embedding {} blocks (D0-D{}, dim={})...",
        embed_count, max_depth, dim
    );

    // Build embeddings buffer: header + flat f32 vectors
    // Blocks outside max_depth get zero vectors
    let total_blocks = reader.block_count;
    let mut buf = Vec::with_capacity(HEADER_SIZE + total_blocks * dim * 4);

    // Header
    buf.extend_from_slice(&(total_blocks as u32).to_le_bytes());
    buf.extend_from_slice(&(dim as u32).to_le_bytes());
    buf.extend_from_slice(&(max_depth as u32).to_le_bytes());

    // Embed blocks
    let zero_vec = vec![0.0f32; dim];
    let mut embedded = 0usize;

    for i in 0..total_blocks {
        let h = reader.header(i);
        if h.depth <= max_depth {
            let text = reader.text(i);
            match provider.embed(text) {
                Ok(emb) => {
                    for &v in &emb {
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
                    embedded += 1;
                    if embedded.is_multiple_of(1000) {
                        eprint!("\r  Embedded {}/{}", embedded, embed_count);
                    }
                }
                Err(_) => {
                    for &v in &zero_vec {
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
        } else {
            for &v in &zero_vec {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    eprintln!("\r  Embedded {}/{}", embedded, embed_count);

    fs::write(output_path, &buf).map_err(|e| format!("write embeddings.bin: {}", e))?;
    let size_kb = buf.len() as f64 / 1024.0;
    println!(
        "  embeddings.bin: {:.1} KB ({} blocks, {} dim)",
        size_kb, total_blocks, dim
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_embedding_index_roundtrip() {
        let dir = std::env::temp_dir().join("mscope_emb_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("embeddings.bin");

        // Build a small test file: 3 blocks, dim=4
        let mut buf = Vec::new();
        buf.extend_from_slice(&3u32.to_le_bytes()); // block_count
        buf.extend_from_slice(&4u32.to_le_bytes()); // dim
        buf.extend_from_slice(&2u32.to_le_bytes()); // max_depth

        // Block 0: [1, 0, 0, 0]
        for &v in &[1.0f32, 0.0, 0.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        // Block 1: [0, 1, 0, 0]
        for &v in &[0.0f32, 1.0, 0.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        // Block 2: zero (not embedded)
        for &v in &[0.0f32, 0.0, 0.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }

        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&buf).unwrap();

        let idx = EmbeddingIndex::open(&path).unwrap();
        assert_eq!(idx.block_count(), 3);
        assert_eq!(idx.dim(), 4);
        assert_eq!(idx.max_depth(), 2);

        let emb0 = idx.embedding(0).unwrap();
        assert_eq!(emb0, &[1.0, 0.0, 0.0, 0.0]);

        // Search with query [1, 0, 0, 0] should find block 0
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].1, 0); // block 0 should be most similar

        let _ = fs::remove_dir_all(&dir);
    }
}
