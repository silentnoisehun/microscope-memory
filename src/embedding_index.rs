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
        search_with_floor(query_emb, k, self.dim, &self.data, HEADER_SIZE, self.embedded_count, &self.block_ids())
    }

    /// Every stored block id, in ascending order.
    ///
    /// Exposed for diagnostics that reason about the whole embedded set (a depth
    /// histogram, a per-block score) rather than a top-k slice of it.
    pub fn all_block_ids(&self) -> &[u32] {
        self.block_ids()
    }

    /// Cosine similarity of one stored block against a query embedding.
    pub fn similarity_of(&self, block_idx: usize, query_emb: &[f32]) -> Option<f32> {
        if query_emb.len() != self.dim {
            return None;
        }
        let ids = self.block_ids();
        let pos = ids.binary_search(&(block_idx as u32)).ok()?;
        let offset = HEADER_SIZE + self.embedded_count * 4 + pos * self.dim * 4;
        let ptr = self.data[offset..].as_ptr() as *const f32;
        // Safety: same invariants as search(); pos < embedded_count.
        let emb = unsafe { std::slice::from_raw_parts(ptr, self.dim) };
        Some(cosine_similarity_simd(query_emb, emb))
    }

    /// Rank of a stored block within the *full* untruncated cosine ordering.
    ///
    /// Diagnostics only. This separates "the embedding is bad" from "the top-k
    /// is crowded out": a block with a high score at a high rank is being
    /// drowned by redundant neighbours, not poorly represented.
    pub fn full_rank_of(&self, block_idx: usize, query_emb: &[f32]) -> Option<(usize, f32)> {
        if query_emb.len() != self.dim {
            return None;
        }
        let ids = self.block_ids();
        let target = ids.binary_search(&(block_idx as u32)).ok()?;
        let target_off = HEADER_SIZE + self.embedded_count * 4 + target * self.dim * 4;
        let tptr = self.data[target_off..].as_ptr() as *const f32;
        let target_emb = unsafe { std::slice::from_raw_parts(tptr, self.dim) };
        let target_sim = cosine_similarity_simd(query_emb, target_emb);
        let mut higher = 0usize;
        for i in 0..self.embedded_count {
            let offset = HEADER_SIZE + self.embedded_count * 4 + i * self.dim * 4;
            let ptr = self.data[offset..].as_ptr() as *const f32;
            // Safety: same invariants as search(); i < embedded_count.
            let emb = unsafe { std::slice::from_raw_parts(ptr, self.dim) };
            if cosine_similarity_simd(query_emb, emb) > target_sim {
                higher += 1;
            }
        }
        Some((higher, target_sim))
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

/// Minimum text length, in characters, for a block to be embedded. Overridable
/// with `MICROSCOPE_MIN_EMBED_CHARS`; 24 is the measured floor, not a guess --
/// at 17 the 17..23 character band re-introduces the crowding the gate exists
/// to remove and hit@10 fell 81.7% -> 78.3%.
pub fn min_embed_chars() -> usize {
    std::env::var("MICROSCOPE_MIN_EMBED_CHARS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(24)
}

/// Why a text is not worth embedding. Deliberately about what the text *is*,
/// never about how it scores: a cosine-threshold dedup would merge
/// contradictory facts -- of 40,000 sampled high-cosine pairs, 1,666 differ in
/// numbers and 245 differ in negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedVerdict {
    Keep,
    /// Below the character floor.
    Short,
    /// The reader's sentinel for bytes it could not decode.
    Unencodable,
    /// UTF-8 bytes shown as cp1252 characters.
    Mojibake,
}

/// The embedding quality gate, shared by the index build and the store path so
/// a freshly stored memory is admitted under exactly the policy that built the
/// index -- a second, looser admission rule would re-introduce what this
/// removes.
pub fn quality_gate(text: &str, min_chars: usize) -> EmbedVerdict {
    // `text()` returns these sentinels for bytes it could not decode, so the
    // block carries no retrievable content at all.
    if text == "<bin>" || text == "[out of bounds]" {
        return EmbedVerdict::Unencodable;
    }
    let total = text.chars().count();
    if total < min_chars {
        return EmbedVerdict::Short;
    }
    // Mojibake: if more than a quarter of the characters sit in the
    // U+0080..U+02FF band, this is not natural language.
    let suspicious = text
        .chars()
        .filter(|c| ('\u{80}'..='\u{2ff}').contains(c))
        .count();
    if total > 0 && suspicious * 4 > total {
        return EmbedVerdict::Mojibake;
    }
    EmbedVerdict::Keep
}

/// Sidecar holding vectors for memories that are still in the append log.
///
/// A stored memory is invisible to the semantic path until the next full
/// rebuild: `embeddings.bin` is built from the consolidated index, and the
/// append log is addressed by position (`1_000_000 + ai`). This closes that
/// gap -- the store path embeds the text once and records the vector under its
/// append index, and recall scores it next to the main index.
///
/// Layout:
///   [4 bytes "AEM1"][u32 dim][u32 record_count]
///   [u32 append_index][f32 x dim] x record_count
///
/// Two properties keep it from desynchronising into wrong answers. It is
/// removed wherever `append.bin` is removed (rebuild, doctor repair), and a
/// record whose append index no longer exists is simply never looked up, so an
/// external edit of the log cannot resurrect a vector for a different memory.
pub const APPEND_EMBEDDINGS_FILE: &str = "append_embeddings.bin";
const APPEND_EMBEDDINGS_MAGIC: &[u8; 4] = b"AEM1";
const APPEND_EMBEDDINGS_HEADER: usize = 12;

pub struct AppendEmbeddings {
    pub dim: usize,
    /// (append index, vector), in write order.
    pub entries: Vec<(u32, Vec<f32>)>,
}

impl AppendEmbeddings {
    pub fn new(dim: usize) -> Self {
        AppendEmbeddings {
            dim,
            entries: Vec::new(),
        }
    }

    /// Open an existing sidecar, or None when it is absent, malformed, or was
    /// written at a different vector width. A width mismatch is ignored rather
    /// than coerced: scoring 384-dim vectors against a 768-dim query is not a
    /// degraded answer, it is a wrong one.
    pub fn open(path: &Path, dim: usize) -> Option<Self> {
        let data = fs::read(path).ok()?;
        if data.len() < APPEND_EMBEDDINGS_HEADER || &data[0..4] != APPEND_EMBEDDINGS_MAGIC {
            return None;
        }
        let file_dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
        if dim == 0 || file_dim != dim {
            return None;
        }
        let count = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
        let stride = 4 + dim * 4;
        let mut entries = Vec::with_capacity(count.min(65_536));
        for i in 0..count {
            let off = APPEND_EMBEDDINGS_HEADER + i * stride;
            if off + stride > data.len() {
                // A crash can leave a partial tail; keep the valid prefix.
                break;
            }
            let index = u32::from_le_bytes(data[off..off + 4].try_into().ok()?);
            let mut v = Vec::with_capacity(dim);
            for j in 0..dim {
                let p = off + 4 + j * 4;
                v.push(f32::from_le_bytes(data[p..p + 4].try_into().ok()?));
            }
            entries.push((index, v));
        }
        Some(AppendEmbeddings { dim, entries })
    }

    pub fn push(&mut self, append_index: u32, vector: Vec<f32>) {
        self.entries.push((append_index, vector));
    }
}

impl AppendEmbeddings {
    /// Write atomically (temp file + rename), like the main index.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let stride = 4 + self.dim * 4;
        let mut buf = Vec::with_capacity(APPEND_EMBEDDINGS_HEADER + self.entries.len() * stride);
        buf.extend_from_slice(APPEND_EMBEDDINGS_MAGIC);
        buf.extend_from_slice(&(self.dim as u32).to_le_bytes());
        buf.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (index, v) in &self.entries {
            if v.len() != self.dim {
                return Err(format!(
                    "append embedding width {} does not match header dim {}",
                    v.len(),
                    self.dim
                ));
            }
            buf.extend_from_slice(&index.to_le_bytes());
            for f in v {
                buf.extend_from_slice(&f.to_le_bytes());
            }
        }
        let tmp = path.with_extension("bin.tmp");
        fs::write(&tmp, &buf).map_err(|e| format!("write {}: {}", APPEND_EMBEDDINGS_FILE, e))?;
        fs::rename(&tmp, path).map_err(|e| format!("rename {}: {}", APPEND_EMBEDDINGS_FILE, e))?;
        Ok(())
    }

    /// Top-k by cosine as (similarity, append index), with the same floor the
    /// main index uses so the two candidate sources stay comparable.
    pub fn search(&self, query_emb: &[f32], k: usize) -> Vec<(f32, u32)> {
        if query_emb.len() != self.dim {
            return vec![];
        }
        let mut out: Vec<(f32, u32)> = self
            .entries
            .iter()
            .filter_map(|(i, v)| {
                let sim = cosine_similarity_simd(query_emb, v);
                (sim > similarity_floor()).then_some((sim, *i))
            })
            .collect();
        out.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.truncate(k);
        out
    }
}

/// The cosine below which a stored vector is not offered as a candidate.
///
/// This was a literal `0.3` in two places. It is read from
/// `MICROSCOPE_SIM_FLOOR` so the value can be measured instead of guessed:
/// on the 60-fact benchmark the default excludes two correct answers that are
/// present and embedded -- "diet" scores 0.089 and "email domain" 0.073 -- and
/// whether a lower floor recovers them is a ranking question, so it is
/// measured before it is changed. The default is unchanged.
pub fn similarity_floor() -> f32 {
    static FLOOR: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *FLOOR.get_or_init(|| {
        std::env::var("MICROSCOPE_SIM_FLOOR")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.3)
    })
}

/// The shared scan behind `EmbeddingIndex::search`, with the floor passed in.
///
/// Split out so a test can measure a different floor without going through the
/// process-wide environment read, which happens once per process.
fn search_with_floor(
    query_emb: &[f32],
    k: usize,
    dim: usize,
    data: &[u8],
    header: usize,
    count: usize,
    ids: &[u32],
) -> Vec<(f32, usize)> {
    if query_emb.len() != dim {
        return vec![];
    }
    let floor = similarity_floor();
    let mut results: Vec<(f32, usize)> = (0..count)
        .into_par_iter()
        .filter_map(|i| {
            let offset = header + count * 4 + i * dim * 4;
            let ptr = data[offset..].as_ptr() as *const f32;
            // Safety: validated in open(); i < count.
            let emb = unsafe { std::slice::from_raw_parts(ptr, dim) };
            let sim = cosine_similarity_simd(query_emb, emb);
            (sim > floor).then_some((sim, ids[i] as usize))
        })
        .collect();

    results.sort_by(|a, b| b.0.total_cmp(&a.0));
    results.truncate(k);
    results
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
    let total_blocks = reader.block_count;

    // Pass 1: count blocks that qualify (depth <= max_depth, text worth embedding).
    // The rule itself is `quality_gate`, shared with the store path; its
    // rationale and the measurements behind it are documented there.
    let min_chars = min_embed_chars();
    let mut skipped_short = 0usize;
    let mut skipped_unencodable = 0usize;
    let mut skipped_mojibake = 0usize;
    let mut qualifying = Vec::new();
    for i in 0..total_blocks {
        let h = reader.header(i);
        if h.depth > max_depth {
            continue;
        }
        match quality_gate(reader.text(i), min_chars) {
            EmbedVerdict::Keep => qualifying.push(i),
            EmbedVerdict::Short => skipped_short += 1,
            EmbedVerdict::Unencodable => skipped_unencodable += 1,
            EmbedVerdict::Mojibake => skipped_mojibake += 1,
        }
    }
    if skipped_short + skipped_unencodable + skipped_mojibake > 0 {
        println!(
            "  Embedding quality gate: -{} short (<{} chars), -{} unencodable, -{} mojibake",
            skipped_short, min_chars, skipped_unencodable, skipped_mojibake
        );
    }

    println!(
        "  Embedding up to {} blocks (D0-D{}, dim={})...",
        qualifying.len(),
        max_depth,
        dim
    );

    let mut block_ids: Vec<u32> = Vec::with_capacity(qualifying.len());
    let mut vectors: Vec<f32> = Vec::with_capacity(qualifying.len() * dim);
    let mut failures = 0usize;
    let mut first_error: Option<String> = None;

    for (n, &i) in qualifying.iter().enumerate() {
        let text = reader.text(i);
        match provider.embed(text) {
            Ok(emb) if emb.len() == dim && emb.iter().any(|&v| v != 0.0) => {
                block_ids.push(i as u32);
                vectors.extend_from_slice(&emb);
            }
            Ok(emb) => {
                // Length mismatch or all-zero vector: record it instead of
                // silently dropping, so a dimension bug cannot look like a
                // successful build.
                failures += 1;
                if first_error.is_none() {
                    first_error = Some(format!(
                        "block {} produced {} dims, expected {} ({})",
                        i,
                        emb.len(),
                        dim,
                        if emb.iter().all(|&v| v == 0.0) {
                            "all-zero vector"
                        } else {
                            "length mismatch"
                        }
                    ));
                }
            }
            Err(e) => {
                failures += 1;
                if first_error.is_none() {
                    first_error = Some(format!("block {}: {}", i, e));
                }
            }
        }
        if n.is_multiple_of(1000) {
            eprint!("\r  Embedded {}/{}", n, qualifying.len());
        }
    }
    eprintln!("\r  Embedded {}/{}", qualifying.len(), qualifying.len());

    // A build that stored nothing is a failure, not a success.
    if qualifying.is_empty() {
        return Err("no blocks qualified for embedding".into());
    }
    if block_ids.is_empty() {
        return Err(format!(
            "all {} blocks failed to embed; first failure: {}",
            qualifying.len(),
            first_error.unwrap_or_else(|| "unknown".into())
        ));
    }
    if failures > 0 {
        eprintln!(
            "  WARN: {} of {} blocks were skipped",
            failures,
            qualifying.len()
        );
    }

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
    fn quality_gate_admits_only_retrievable_text() {
        // A real memory, well above the floor, no mojibake band.
        assert_eq!(
            quality_gate("The user has a pine nut allergy.", 24),
            EmbedVerdict::Keep
        );
        // Accented natural language is not mojibake: one character in the band
        // out of 37 must not disqualify the text.
        assert_eq!(
            quality_gate("The user drinks café lattes every morning.", 24),
            EmbedVerdict::Keep
        );
        // Below the character floor: the fragments that used to crowd the index.
        assert_eq!(quality_gate("LLM", 24), EmbedVerdict::Short);
        assert_eq!(
            quality_gate("The user is vegetarian.", 24),
            EmbedVerdict::Short,
            "23 characters is under the measured 24 floor"
        );
        // The reader's sentinels for undecodable bytes carry no content at all.
        assert_eq!(quality_gate("<bin>", 24), EmbedVerdict::Unencodable);
        assert_eq!(
            quality_gate("[out of bounds]", 24),
            EmbedVerdict::Unencodable
        );
        // Mojibake: long enough to clear the floor, every character in the band.
        assert_eq!(
            quality_gate("đź§đź§đź§đź§đź§đź§đź§đź§", 24),
            EmbedVerdict::Mojibake
        );
        // The floor is a parameter, not a constant baked into the rule.
        assert_eq!(
            quality_gate("The user is vegetarian.", 20),
            EmbedVerdict::Keep,
            "MICROSCOPE_MIN_EMBED_CHARS=20 would admit a 23-character fact"
        );
    }

    #[test]
    fn min_embed_chars_default_is_the_measured_floor() {
        if std::env::var_os("MICROSCOPE_MIN_EMBED_CHARS").is_none() {
            assert_eq!(min_embed_chars(), 24);
        }
    }

    #[test]
    fn append_embeddings_roundtrip_and_guards() {
        let dir = std::env::temp_dir().join("mscope_append_emb_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(APPEND_EMBEDDINGS_FILE);

        let mut side = AppendEmbeddings::new(4);
        side.push(0, vec![1.0, 0.0, 0.0, 0.0]);
        side.push(1, vec![0.0, 1.0, 0.0, 0.0]);
        side.push(7, vec![0.0, 0.0, 0.0, 1.0]);
        side.save(&path).expect("save");

        let opened = AppendEmbeddings::open(&path, 4).expect("open");
        assert_eq!(opened.dim, 4);
        assert_eq!(opened.entries.len(), 3);
        assert_eq!(opened.entries[2].0, 7, "append index survives the round trip");

        // A query equal to the first stored vector ranks it first.
        let hits = opened.search(&[1.0, 0.0, 0.0, 0.0], 10);
        assert_eq!(hits[0].1, 0);
        assert!(hits[0].0 > 0.99, "identical vectors must score ~1.0");
        assert!(hits.iter().all(|&(sim, _)| sim > 0.3), "0.3 floor applies");

        // A width mismatch must be ignored, not coerced: scoring 4-dim vectors
        // against an 8-dim query would be a wrong answer, not a degraded one.
        assert!(AppendEmbeddings::open(&path, 8).is_none());
        // A foreign file is not a sidecar.
        let junk = dir.join("junk.bin");
        fs::write(&junk, b"NOPE\x04\x00\x00\x00\x00\x00\x00\x00").unwrap();
        assert!(AppendEmbeddings::open(&junk, 4).is_none());

        // A crash mid-write leaves a partial tail; the valid prefix must load.
        let mut data = fs::read(&path).unwrap();
        data.extend_from_slice(&[0xAB, 0xCD, 0xEF]);
        fs::write(&path, &data).unwrap();
        let partial = AppendEmbeddings::open(&path, 4).expect("valid prefix survives");
        assert_eq!(partial.entries.len(), 3);

        // A record of the wrong width is refused at save time, not on read.
        let mut bad = AppendEmbeddings::new(4);
        bad.push(0, vec![1.0, 0.0]);
        assert!(bad.save(&path).is_err());

        // A width-mismatched query never scores.
        assert!(partial.search(&[1.0, 0.0], 10).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

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
