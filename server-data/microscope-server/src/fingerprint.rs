//! Structural fingerprinting for Microscope Memory.
//!
//! Each block gets a structural fingerprint at index time:
//! Shannon entropy + byte distribution hash. Blocks with similar
//! fingerprints are linked in `links.bin` — creating "wormholes"
//! between structurally similar but spatially distant blocks.
//!
//! Binary format: fingerprints.idx (FGP1), links.bin (LNK1)

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ─── Constants ──────────────────────────────────────

/// Similarity threshold for creating a structural link (0..1).
const LINK_THRESHOLD: f32 = 0.85;
/// Maximum links per block.
const MAX_LINKS_PER_BLOCK: usize = 8;
/// Number of byte histogram buckets (compressed from 256).
const HIST_BUCKETS: usize = 16;
/// Fingerprint binary size: 4 (entropy f32) + 16 (histogram) + 8 (hash u64) = 28 bytes
const FINGERPRINT_BYTES: usize = 28;

// ─── Types ──────────────────────────────────────────

/// Structural fingerprint for a single block.
#[derive(Clone, Debug)]
pub struct BlockFingerprint {
    /// Shannon entropy of the block content (0.0 = uniform, ~8.0 = max random).
    pub entropy: f32,
    /// Compressed byte distribution (16 buckets, normalized to 0..255).
    pub histogram: [u8; HIST_BUCKETS],
    /// FNV-1a hash of the histogram for fast comparison.
    pub hash: u64,
}

/// A structural link between two blocks.
#[derive(Clone, Debug)]
pub struct StructuralLink {
    pub block_a: u32,
    pub block_b: u32,
    pub similarity: f32,
}

/// Link table — mmap-friendly storage of structural links.
pub struct LinkTable {
    pub fingerprints: Vec<BlockFingerprint>,
    pub links: Vec<StructuralLink>,
}

impl LinkTable {
    /// Build fingerprints and links from block data.
    pub fn build(block_texts: &[&str]) -> Self {
        let fingerprints: Vec<BlockFingerprint> = block_texts
            .iter()
            .map(|text| compute_fingerprint(text.as_bytes()))
            .collect();

        let links = find_links(&fingerprints);

        LinkTable {
            fingerprints,
            links,
        }
    }

    /// Get structural links for a specific block.
    pub fn links_for(&self, block_idx: u32) -> Vec<&StructuralLink> {
        self.links
            .iter()
            .filter(|l| l.block_a == block_idx || l.block_b == block_idx)
            .collect()
    }

    /// Get the other end of a link from a given block.
    pub fn linked_blocks(&self, block_idx: u32) -> Vec<(u32, f32)> {
        self.links
            .iter()
            .filter_map(|l| {
                if l.block_a == block_idx {
                    Some((l.block_b, l.similarity))
                } else if l.block_b == block_idx {
                    Some((l.block_a, l.similarity))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find the most structurally similar blocks to a query text.
    pub fn find_similar(&self, text: &str, k: usize) -> Vec<(u32, f32)> {
        let query_fp = compute_fingerprint(text.as_bytes());
        let mut results: Vec<(u32, f32)> = self
            .fingerprints
            .iter()
            .enumerate()
            .map(|(i, fp)| (i as u32, fingerprint_similarity(&query_fp, fp)))
            .filter(|(_, sim)| *sim > 0.5)
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(k);
        results
    }

    /// Statistics.
    pub fn stats(&self) -> FingerprintStats {
        let avg_entropy = if self.fingerprints.is_empty() {
            0.0
        } else {
            self.fingerprints.iter().map(|f| f.entropy).sum::<f32>()
                / self.fingerprints.len() as f32
        };

        // Count unique hash buckets
        let mut hash_counts: HashMap<u64, usize> = HashMap::new();
        for fp in &self.fingerprints {
            *hash_counts.entry(fp.hash).or_insert(0) += 1;
        }
        let unique_hashes = hash_counts.len();
        let largest_cluster = hash_counts.values().max().copied().unwrap_or(0);

        FingerprintStats {
            block_count: self.fingerprints.len(),
            link_count: self.links.len(),
            avg_entropy,
            unique_hashes,
            largest_cluster,
        }
    }

    /// Save to disk.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        save_fingerprints(output_dir, &self.fingerprints)?;
        save_links(output_dir, &self.links)?;
        Ok(())
    }

    /// Load from disk.
    pub fn load(output_dir: &Path) -> Option<Self> {
        let fingerprints = load_fingerprints(output_dir)?;
        let links = load_links(output_dir).unwrap_or_default();
        Some(LinkTable {
            fingerprints,
            links,
        })
    }
}

pub struct FingerprintStats {
    pub block_count: usize,
    pub link_count: usize,
    pub avg_entropy: f32,
    pub unique_hashes: usize,
    pub largest_cluster: usize,
}

// ─── Core algorithms ────────────────────────────────

/// Compute the structural fingerprint of a block's content.
pub fn compute_fingerprint(data: &[u8]) -> BlockFingerprint {
    // Shannon entropy
    let mut byte_counts = [0u32; 256];
    for &b in data {
        byte_counts[b as usize] += 1;
    }
    let len = data.len().max(1) as f32;

    let entropy: f32 = byte_counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f32 / len;
            -p * p.log2()
        })
        .sum();

    // Compressed histogram: 256 → 16 buckets
    let mut histogram = [0u8; HIST_BUCKETS];
    for (i, bucket) in histogram.iter_mut().enumerate() {
        let start = i * 16;
        let end = start + 16;
        let sum: u32 = byte_counts[start..end].iter().sum();
        // Normalize to 0..255
        *bucket = ((sum as f32 / len) * 255.0).min(255.0) as u8;
    }

    // FNV-1a hash of histogram
    let hash = fnv1a_hash(&histogram);

    BlockFingerprint {
        entropy,
        histogram,
        hash,
    }
}

/// Compute similarity between two fingerprints (0.0..1.0).
pub fn fingerprint_similarity(a: &BlockFingerprint, b: &BlockFingerprint) -> f32 {
    // Fast path: if hashes differ wildly, skip detailed comparison
    if a.hash != b.hash {
        // Histogram cosine similarity
        let mut dot = 0u32;
        let mut norm_a = 0u32;
        let mut norm_b = 0u32;
        for i in 0..HIST_BUCKETS {
            let va = a.histogram[i] as u32;
            let vb = b.histogram[i] as u32;
            dot += va * vb;
            norm_a += va * va;
            norm_b += vb * vb;
        }

        let denom = ((norm_a as f64).sqrt() * (norm_b as f64).sqrt()) as f32;
        if denom < 1.0 {
            return 0.0;
        }
        let cosine = dot as f32 / denom;

        // Entropy similarity (penalize large entropy differences)
        let entropy_diff = (a.entropy - b.entropy).abs();
        let entropy_sim = 1.0 - (entropy_diff / 8.0).min(1.0);

        cosine * 0.7 + entropy_sim * 0.3
    } else {
        // Same hash → very similar histograms, refine with entropy
        let entropy_diff = (a.entropy - b.entropy).abs();
        let entropy_sim = 1.0 - (entropy_diff / 8.0).min(1.0);
        0.85 + entropy_sim * 0.15
    }
}

/// Find structural links between fingerprints above threshold.
fn find_links(fingerprints: &[BlockFingerprint]) -> Vec<StructuralLink> {
    let mut links = Vec::new();
    let n = fingerprints.len();

    // Group by hash for O(n*k) instead of O(n²)
    let mut hash_groups: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, fp) in fingerprints.iter().enumerate() {
        hash_groups.entry(fp.hash).or_default().push(i);
    }

    // Within each hash group, all pairs are candidates
    for group in hash_groups.values() {
        if group.len() < 2 {
            continue;
        }
        for i in 0..group.len() {
            let mut block_links = 0;
            for j in (i + 1)..group.len() {
                if block_links >= MAX_LINKS_PER_BLOCK {
                    break;
                }
                let a = group[i];
                let b = group[j];
                let sim = fingerprint_similarity(&fingerprints[a], &fingerprints[b]);
                if sim >= LINK_THRESHOLD {
                    links.push(StructuralLink {
                        block_a: a as u32,
                        block_b: b as u32,
                        similarity: sim,
                    });
                    block_links += 1;
                }
            }
        }
    }

    // Also check near-miss hashes (different hash but potentially similar)
    // Only for small block counts to avoid O(n²)
    if n < 5000 {
        for i in 0..n {
            let mut block_links = links
                .iter()
                .filter(|l| l.block_a == i as u32 || l.block_b == i as u32)
                .count();
            if block_links >= MAX_LINKS_PER_BLOCK {
                continue;
            }

            for j in (i + 1)..n {
                if fingerprints[i].hash == fingerprints[j].hash {
                    continue; // already checked
                }
                let sim = fingerprint_similarity(&fingerprints[i], &fingerprints[j]);
                if sim >= LINK_THRESHOLD {
                    links.push(StructuralLink {
                        block_a: i as u32,
                        block_b: j as u32,
                        similarity: sim,
                    });
                    block_links += 1;
                    if block_links >= MAX_LINKS_PER_BLOCK {
                        break;
                    }
                }
            }
        }
    }

    links
}

fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ─── Binary I/O ─────────────────────────────────────

fn save_fingerprints(output_dir: &Path, fps: &[BlockFingerprint]) -> Result<(), String> {
    let path = output_dir.join("fingerprints.idx");
    let mut buf = Vec::with_capacity(8 + fps.len() * FINGERPRINT_BYTES);
    buf.extend_from_slice(b"FGP1");
    buf.extend_from_slice(&(fps.len() as u32).to_le_bytes());
    for fp in fps {
        buf.extend_from_slice(&fp.entropy.to_le_bytes());
        buf.extend_from_slice(&fp.histogram);
        buf.extend_from_slice(&fp.hash.to_le_bytes());
    }
    fs::write(&path, &buf).map_err(|e| format!("write fingerprints.idx: {}", e))
}

fn load_fingerprints(output_dir: &Path) -> Option<Vec<BlockFingerprint>> {
    let path = output_dir.join("fingerprints.idx");
    let data = fs::read(&path).ok()?;
    if data.len() < 8 || &data[0..4] != b"FGP1" {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut fps = Vec::with_capacity(count);
    let mut pos = 8;
    for _ in 0..count {
        if pos + FINGERPRINT_BYTES > data.len() {
            break;
        }
        let entropy = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        let mut histogram = [0u8; HIST_BUCKETS];
        histogram.copy_from_slice(&data[pos + 4..pos + 4 + HIST_BUCKETS]);
        let hash = u64::from_le_bytes(data[pos + 20..pos + 28].try_into().unwrap());
        fps.push(BlockFingerprint {
            entropy,
            histogram,
            hash,
        });
        pos += FINGERPRINT_BYTES;
    }
    Some(fps)
}

fn save_links(output_dir: &Path, links: &[StructuralLink]) -> Result<(), String> {
    let path = output_dir.join("links.bin");
    let mut buf = Vec::with_capacity(8 + links.len() * 12);
    buf.extend_from_slice(b"LNK1");
    buf.extend_from_slice(&(links.len() as u32).to_le_bytes());
    for link in links {
        buf.extend_from_slice(&link.block_a.to_le_bytes());
        buf.extend_from_slice(&link.block_b.to_le_bytes());
        buf.extend_from_slice(&link.similarity.to_le_bytes());
    }
    fs::write(&path, &buf).map_err(|e| format!("write links.bin: {}", e))
}

fn load_links(output_dir: &Path) -> Option<Vec<StructuralLink>> {
    let path = output_dir.join("links.bin");
    let data = fs::read(&path).ok()?;
    if data.len() < 8 || &data[0..4] != b"LNK1" {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut links = Vec::with_capacity(count);
    let mut pos = 8;
    for _ in 0..count {
        if pos + 12 > data.len() {
            break;
        }
        let block_a = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        let block_b = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
        let similarity = f32::from_le_bytes(data[pos + 8..pos + 12].try_into().unwrap());
        links.push(StructuralLink {
            block_a,
            block_b,
            similarity,
        });
        pos += 12;
    }
    Some(links)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_uniform() {
        let fp = compute_fingerprint(b"aaaaaaaaaa");
        assert!(fp.entropy < 0.01); // single byte = 0 entropy
    }

    #[test]
    fn test_entropy_varied() {
        let data: Vec<u8> = (0..=255).collect();
        let fp = compute_fingerprint(&data);
        assert!(fp.entropy > 7.0); // max entropy ≈ 8.0
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let a = compute_fingerprint(b"hello world, this is a test of the fingerprint system");
        let b = compute_fingerprint(b"hello world, this is a test of the fingerprint system");
        assert_eq!(a.hash, b.hash);
        assert!((a.entropy - b.entropy).abs() < 0.001);
        assert_eq!(a.histogram, b.histogram);
    }

    #[test]
    fn test_similarity_identical() {
        let fp = compute_fingerprint(b"hello world test");
        let sim = fingerprint_similarity(&fp, &fp);
        assert!(sim > 0.99);
    }

    #[test]
    fn test_similarity_different() {
        let a = compute_fingerprint(b"aaaaaaaaaaaaaaaa");
        let data: Vec<u8> = (0..=255).collect();
        let b = compute_fingerprint(&data);
        let sim = fingerprint_similarity(&a, &b);
        assert!(sim < 0.5); // very different
    }

    #[test]
    fn test_similarity_similar_text() {
        let a = compute_fingerprint(b"the quick brown fox jumps over the lazy dog");
        let b = compute_fingerprint(b"the fast brown fox leaps over the tired dog");
        let sim = fingerprint_similarity(&a, &b);
        assert!(sim > 0.7); // similar letter distributions
    }

    #[test]
    fn test_find_links() {
        let texts = [
            "hello world this is a test",
            "hello world this is a test", // identical
            "completely different binary data 12345!@#$%",
            "hello world this is another test", // similar
        ];
        let fps: Vec<BlockFingerprint> = texts
            .iter()
            .map(|t| compute_fingerprint(t.as_bytes()))
            .collect();
        let links = find_links(&fps);

        // At minimum, blocks 0 and 1 should be linked (identical)
        assert!(links
            .iter()
            .any(|l| { (l.block_a == 0 && l.block_b == 1) || (l.block_a == 1 && l.block_b == 0) }));
    }

    #[test]
    fn test_link_table_build() {
        let texts = vec!["hello world", "hello world", "binary data xyz"];
        let table = LinkTable::build(&texts);
        assert_eq!(table.fingerprints.len(), 3);
        // At least one link between the two identical texts
        assert!(!table.links.is_empty());
    }

    #[test]
    fn test_find_similar() {
        let texts = vec![
            "hello world testing one two three",
            "hello world testing four five six",
            "completely random binary noise !!!",
        ];
        let table = LinkTable::build(&texts);
        let results = table.find_similar("hello world testing", 5);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let dir = tmp.path();

        let texts = vec!["hello world", "test data", "more content here"];
        let table = LinkTable::build(&texts);
        table.save(dir).expect("save");

        let loaded = LinkTable::load(dir).expect("load");
        assert_eq!(loaded.fingerprints.len(), 3);
        assert!((loaded.fingerprints[0].entropy - table.fingerprints[0].entropy).abs() < 0.001);
        assert_eq!(loaded.fingerprints[0].hash, table.fingerprints[0].hash);
        assert_eq!(loaded.links.len(), table.links.len());
    }

    #[test]
    fn test_linked_blocks() {
        let texts = vec!["abc abc abc", "abc abc abc", "xyz xyz xyz"];
        let table = LinkTable::build(&texts);
        let linked = table.linked_blocks(0);
        // Block 0 and 1 are identical, should be linked
        assert!(linked.iter().any(|(idx, _)| *idx == 1));
    }

    #[test]
    fn test_stats() {
        let texts = vec!["hello", "world", "hello"];
        let table = LinkTable::build(&texts);
        let stats = table.stats();
        assert_eq!(stats.block_count, 3);
    }

    #[test]
    fn test_fnv1a_deterministic() {
        assert_eq!(fnv1a_hash(b"hello"), fnv1a_hash(b"hello"));
        assert_ne!(fnv1a_hash(b"hello"), fnv1a_hash(b"world"));
    }
}
