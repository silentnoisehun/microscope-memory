//! Embedding index: mmap-backed pre-computed embedding vectors.
//!
//! Sparse format — only blocks that actually received an embedding are stored
//! (depth <= max_depth, non-trivial text, non-zero vector). Previously every
//! block held a full vector with a NaN sentinel for the ~99% un-embedded ones,
//! which blew the file up to gigabytes for no benefit. Search now scans only
//! the embedded subset.
//!
//! Layout:
//!   [u32 embedded_count][u32 dim][u32 max_depth]
//!   [u32 block_idx × embedded_count]      (ascending, for binary search)
//!   [f32 × dim × embedded_count]
//!
//! `embedding(block_idx)` returns None for blocks without a stored vector.

use std::fs;
use std::path::Path;

use rayon::prelude::*;

use crate::embeddings::{cosine_similarity_simd, EmbeddingProvider};

/// Mmap-backed embedding index for fast semantic lookup.
#[allow(dead_code)]
pub struct EmbeddingIndex {
    data: memmap2::Mmap,
    embedded_count: usize,
    dim: usize,
    max_depth: u32,
}

const HEADER_SIZE: usize = 12; // 3 × u32

impl EmbeddingIndex {
    /// Open an existing embeddings.bin file (sparse format).
    pub fn open(path: &Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }
        let file = fs::File::open(path).ok()?;
        let data = unsafe { memmap2::Mmap::map(&file).ok()? };
        if data.len() < HEADER_SIZE {
            return None;
        }

        let embedded_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let dim = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let max_depth = u32::from_le_bytes(data[8..12].try_into().unwrap());

        let expected = HEADER_SIZE
            + embedded_count
                .checked_mul(4)
                .and_then(|v| v.checked_add(embedded_count.checked_mul(dim * 4)?))
                .unwrap_or(usize::MAX);
        if expected > data.len() {
            return None;
        }

        Some(EmbeddingIndex {
            data,
            embedded_count,
            dim,
            max_depth,
        })
    }

    /// Number of stored (embedded) blocks.
    pub fn block_count(&self) -> usize {
        self.embedded_count
    }

    /// Get the embedding for a block index (zero-copy mmap access).
    pub fn embedding(&self, block_idx: usize) -> Option<&[f32]> {
        let ids = self.block_ids();
        let pos = ids.binary_search(&(block_idx as u32)).ok()?;
        let offset = HEADER_SIZE + self.embedded_count * 4 + pos * self.dim * 4;
        let ptr = self.data[offset..].as_ptr() as *const f32;
        // Safety: the vectors region is a multiple of 4 bytes past a 4-aligned
        // base, and the size was validated in open().
        Some(unsafe { std::slice::from_raw_parts(ptr, self.dim) })
    }

    /// Embedding dimension.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Max depth that was embedded.
    #[allow(dead_code)]
    pub fn max_depth(&self) -> u8 {
        self.max_depth as u8
    }

    /// Search for top-k most similar blocks to query embedding.
    /// Returns Vec<(similarity, block_index)> sorted descending.
    pub fn search(&self, query_emb: &[f32], k: usize) -> Vec<(f32, usize)> {
        if query_emb.len() != self.dim {
            return vec![];
        }

        let ids = self.block_ids();
        let mut results: Vec<(f32, usize)> = (0..self.embedded_count)
            .into_par_iter()
            .filter_map(|i| {
                let offset = HEADER_SIZE + self.embedded_count * 4 + i * self.dim * 4;
                let ptr = self.data[offset..].as_ptr() as *const f32;
                // Safety: validated in open(); i < embedded_count.
                let emb = unsafe { std::slice::from_raw_parts(ptr, self.dim) };
                let sim = cosine_similarity_simd(query_emb, emb);
                (sim > 0.3).then_some((sim, ids[i] as usize))
            })
            .collect();

        results.sort_by(|a, b| b.0.total_cmp(&a.0));
        results.truncate(k);
        results
    }

    /// Block-id lookup array (ascending u32 indices).
    fn block_ids(&self) -> &[u32] {
        let start = HEADER_SIZE;
        let end = start + self.embedded_count * 4;
        let ptr = self.data[start..end].as_ptr() as *const u32;
        // Safety: start is 4-aligned and the region size was validated.
        unsafe { std::slice::from_raw_parts(ptr, self.embedded_count) }
    }
}

/// Build a sparse embedding index file from a provider and reader.
/// Only blocks at depth 0..=max_depth with non-trivial text get embedded;
/// failed or zero embeddings are omitted (search treats them as absent).
pub fn build_embedding_index(
    provider: &dyn EmbeddingProvider,
    reader: &crate::MicroscopeReader,
    max_depth: u8,
    output_path: &Path,
) -> Result<(), String> {
    let dim = provider.dimension();

    // Pass 1: count blocks that qualify (depth <= max_depth, non-trivial text).
    let total_blocks = reader.block_count;
    let mut qualifying = Vec::new();
    for i in 0..total_blocks {
        let h = reader.header(i);
        if h.depth <= max_depth && reader.text(i).len() >= 3 {
            qualifying.push(i);
        }
    }

    println!(
        "  Embedding up to {} blocks (D0-D{}, dim={})...",
        qualifying.len(),
        max_depth,
        dim
    );

    let mut block_ids: Vec<u32> = Vec::with_capacity(qualifying.len());
    let mut vectors: Vec<f32> = Vec::new();

    for (n, &i) in qualifying.iter().enumerate() {
        let text = reader.text(i);
        match provider.embed(text) {
            Ok(emb) if emb.len() == dim && emb.iter().any(|&v| v != 0.0) => {
                block_ids.push(i as u32);
                vectors.extend_from_slice(&emb);
            }
            _ => {} // failed or zero embedding → omit (equivalent to current
                    // NaN/zero filtering in search)
        }
        if n.is_multiple_of(1000) {
            eprint!("\r  Embedded {}/{}", n, qualifying.len());
        }
    }
    eprintln!("\r  Embedded {}/{}", qualifying.len(), qualifying.len());

    let mut buf = Vec::with_capacity(HEADER_SIZE + block_ids.len() * 4 + vectors.len() * 4);
    buf.extend_from_slice(&(block_ids.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(dim as u32).to_le_bytes());
    buf.extend_from_slice(&(max_depth as u32).to_le_bytes());
    for &id in &block_ids {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    for &v in &vectors {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    let tmp_path = output_path.with_extension("bin.tmp");
    fs::write(&tmp_path, &buf).map_err(|e| format!("write embeddings.bin: {}", e))?;
    fs::rename(&tmp_path, output_path).map_err(|e| format!("rename embeddings.bin: {}", e))?;
    println!(
        "  embeddings.bin: {:.1} KB ({} stored vectors of {} blocks, dim {})",
        buf.len() as f64 / 1024.0,
        block_ids.len(),
        total_blocks,
        dim
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_embedding_index_sparse_roundtrip() {
        let dir = std::env::temp_dir().join("mscope_emb_sparse_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("embeddings.bin");

        // Sparse file: 3 stored vectors, dim=4, max_depth=2.
        // Stored blocks: 0, 2, 5. Block 7 absent.
        let mut buf = Vec::new();
        buf.extend_from_slice(&3u32.to_le_bytes()); // embedded_count
        buf.extend_from_slice(&4u32.to_le_bytes()); // dim
        buf.extend_from_slice(&2u32.to_le_bytes()); // max_depth
        for &id in &[0u32, 2, 5] {
            buf.extend_from_slice(&id.to_le_bytes());
        }
        for &v in &[1.0f32, 0.0, 0.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for &v in &[0.0f32, 1.0, 0.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for &v in &[0.0f32, 0.0, 1.0, 0.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }

        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&buf).unwrap();

        let idx = EmbeddingIndex::open(&path).unwrap();
        assert_eq!(idx.block_count(), 3);
        assert_eq!(idx.dim(), 4);
        assert_eq!(idx.max_depth(), 2);

        assert_eq!(idx.embedding(0).unwrap(), &[1.0, 0.0, 0.0, 0.0]);
        assert_eq!(idx.embedding(2).unwrap(), &[0.0, 1.0, 0.0, 0.0]);
        assert_eq!(idx.embedding(5).unwrap(), &[0.0, 0.0, 1.0, 0.0]);
        assert!(idx.embedding(1).is_none());
        assert!(idx.embedding(7).is_none());

        // Query [1,0,0,0] should return block 0 first and never block 7.
        let results = idx.search(&[1.0, 0.0, 0.0, 0.0], 10);
        assert_eq!(results[0].1, 0);
        assert!(!results.iter().any(|&(_, i)| i == 7));
        assert!(!results.iter().any(|&(_, i)| i == 1));

        // Truncated file must fail open (never panic on short mmap).
        let trunc = dir.join("truncated.bin");
        fs::write(&trunc, &buf[..buf.len() - 3]).unwrap();
        assert!(EmbeddingIndex::open(&trunc).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_empty_index_open_fails_gracefully() {
        let dir = std::env::temp_dir().join("mscope_emb_empty_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("embeddings.bin");
        fs::write(&path, [0u8; 4]).unwrap();
        assert!(EmbeddingIndex::open(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
