//! Inverted text index (TIX1) for fast `find_text` / recall lexical prefilter.
//!
//! The hot search paths scan every block (`find` ~51 ms, `recall` ~255 ms on
//! ~1M blocks). This index maps lowercased words to the blocks that contain
//! them, so a query only touches the blocks that can possibly match, then the
//! caller re-runs its exact predicate on those candidates (identical results,
//! far fewer text scans).
//!
//! Binary layout (mmap, zero heap):
//! ```text
//!   [0..4)   magic "TIX1"
//!   [4..8)   word_count  u32
//!   [8..12)  total_postings u32
//!   [12..16) block_count u32
//!   [16..20) postings_offset u32 (absolute byte offset; 4-aligned)
//!   [20..)   dict offsets: word_count × u32 (absolute byte offset of a DictEntry)
//!   DictEntry (variable):
//!     u16 word_len + word bytes + u32 posting_start + u32 posting_len
//!   padding to 4-byte alignment
//!   postings: total_postings × u32 block ids (each list ascending)
//! ```
//!
//! Tokenization is lowercased alphanumeric runs of at least 2 chars. Postings
//! are block ids in ascending order (blocks are visited ascending at build
//! time), so membership checks use binary search.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

const MAGIC: &[u8; 4] = b"TIX1";
const HEADER_SIZE: usize = 20; // magic(4) + word_count(4) + total_postings(4) + block_count(4) + postings_offset(4)

/// Mmap-backed inverted text index.
pub struct TextIndex {
    data: memmap2::Mmap,
    word_count: usize,
    total_postings: usize,
    block_count: usize,
    postings_start: usize,
}

struct DictEntry<'a> {
    word: &'a str,
    posting_start: usize,
    posting_len: usize,
}

/// Split text into lowercased alphanumeric words (length >= 2).
/// `find_text` semantics are substring-based, so query tokens are treated the
/// same way here and candidates are later verified with the exact predicate.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else {
            if cur.chars().count() >= 2 {
                words.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if cur.chars().count() >= 2 {
        words.push(cur);
    }
    words
}

impl TextIndex {
    /// Open an existing text_index.bin (sparse validation; corrupt files return
    /// `None` so callers fall back to a full scan).
    pub fn open(path: &Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }
        let file = fs::File::open(path).ok()?;
        let data = unsafe { memmap2::Mmap::map(&file).ok()? };
        if data.len() < HEADER_SIZE || &data[0..4] != MAGIC {
            return None;
        }
        let word_count = u32_at(&data, 4)? as usize;
        let total_postings = u32_at(&data, 8)? as usize;
        let block_count = u32_at(&data, 12)? as usize;
        let postings_start = u32_at(&data, 16)? as usize;
        let offsets_end = HEADER_SIZE.checked_add(word_count.checked_mul(4)?)?;
        let postings_bytes = total_postings.checked_mul(4)?;
        if postings_start < offsets_end
            || data.len() < postings_start.checked_add(postings_bytes)?
            || postings_start % 4 != 0
        {
            return None;
        }
        Some(TextIndex {
            data,
            word_count,
            total_postings,
            block_count,
            postings_start,
        })
    }

    /// Number of blocks the index was built over.
    pub fn block_count(&self) -> usize {
        self.block_count
    }

    fn u32_at(&self, off: usize) -> Option<u32> {
        self.data
            .get(off..off + 4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    }

    fn u16_at(&self, off: usize) -> Option<u16> {
        self.data
            .get(off..off + 2)
            .map(|s| u16::from_le_bytes(s.try_into().unwrap()))
    }

    /// Read the i-th dictionary entry.
    fn entry(&self, i: usize) -> Option<DictEntry<'_>> {
        if i >= self.word_count {
            return None;
        }
        let entry_off = self.u32_at(HEADER_SIZE + i * 4)? as usize;
        let wlen = self.u16_at(entry_off)? as usize;
        let start = entry_off.checked_add(2)?;
        let end = start.checked_add(wlen)?;
        let word = std::str::from_utf8(self.data.get(start..end)?).ok()?;
        let ps = self.u32_at(end)? as usize;
        let pl = self.u32_at(end + 4)? as usize;
        if ps.checked_add(pl)? > self.total_postings {
            return None;
        }
        Some(DictEntry {
            word,
            posting_start: ps,
            posting_len: pl,
        })
    }

    /// Posting slice for a dictionary entry (zero-copy mmap).
    fn postings(&self, e: &DictEntry<'_>) -> &[u32] {
        let start = self.postings_start + e.posting_start * 4;
        let ptr = self.data[start..].as_ptr() as *const u32;
        // Safety: bounds validated in entry() against total_postings; the
        // postings region is a multiple of 4 bytes past a 4-aligned base.
        unsafe { std::slice::from_raw_parts(ptr, e.posting_len) }
    }

    /// Binary search the sorted word dictionary.
    fn find_word(&self, target: &str) -> Option<usize> {
        let mut lo = 0usize;
        let mut hi = self.word_count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.entry(mid)?.word.cmp(target) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    /// Range of dictionary entries whose word starts with `prefix` (sorted →
    /// contiguous). Returns [lo, hi) dict indices.
    fn range_words_starting_with(&self, prefix: &str) -> (usize, usize) {
        let mut lo = 0usize;
        let mut hi = self.word_count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if let Some(e) = self.entry(mid) {
                if e.word < prefix {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            } else {
                break;
            }
        }
        let start = lo;
        hi = self.word_count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if let Some(e) = self.entry(mid) {
                if e.word.starts_with(prefix) {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            } else {
                break;
            }
        }
        (start, lo)
    }

    /// Blocks whose text contains `token` as a substring of some word.
    ///
    /// A pure-alphanumeric token appears in a block iff some word of that block
    /// contains it, so the union of postings of every dictionary word containing
    /// the token is exactly the matching block set. (This is what makes
    /// `find_text`'s substring semantics exact even for fragments like "rus"
    /// inside "rust".)
    fn blocks_containing_token(&self, token: &str) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for i in 0..self.word_count {
            let Some(e) = self.entry(i) else { break };
            if e.word.contains(token) {
                out.extend_from_slice(self.postings(&e));
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Exact superset of blocks whose text contains `query` as a substring.
    ///
    /// Returns `None` when the query cannot be routed through the index (too
    /// short / punctuation-only) — caller must fall back to a full scan.
    /// Returns `Some(ids)` with ids sorted ascending; the caller verifies the
    /// exact substring predicate on the candidates.
    pub fn candidates_substring(&self, query_lower: &str) -> Option<Vec<u32>> {
        let tokens = tokenize(query_lower);
        if tokens.is_empty() {
            return None;
        }
        let mut sets: Vec<Vec<u32>> = Vec::with_capacity(tokens.len());
        for t in &tokens {
            let set = self.blocks_containing_token(t);
            if set.is_empty() {
                return Some(Vec::new());
            }
            sets.push(set);
        }
        // Intersection (a block must contain every token for the phrase).
        sets.sort_by_key(|v| v.len());
        let mut result = std::mem::take(&mut sets[0]);
        for other in &sets[1..] {
            if result.is_empty() {
                break;
            }
            result.retain(|&id| other.binary_search(&id).is_ok());
        }
        Some(result)
    }

    /// Blocks matching any token under `token_similarity` semantics:
    /// `t == q` for short tokens (3-4 bytes), or sharing the first 5 chars
    /// for tokens of length >= 5. Used as a recall prefilter; `None` → full
    /// scan fallback.
    pub fn candidates_lexical(&self, tokens: &[String]) -> Option<Vec<u32>> {
        if tokens.is_empty() {
            return None;
        }
        let mut union: Vec<u32> = Vec::new();
        for t in tokens {
            let chars: Vec<char> = t.chars().collect();
            if chars.len() < 3 {
                continue;
            }
            if chars.len() < 5 {
                // Short token: only exact word equality matches.
                if let Some(i) = self.find_word(t) {
                    if let Some(e) = self.entry(i) {
                        union.extend_from_slice(self.postings(&e));
                    }
                }
                continue;
            }
            // Token length >= 5: every dictionary word sharing the first 5
            // chars can produce a lexical match (exact, prefix-extension, and
            // divergence-after-5 all covered by the contiguous range).
            let prefix: String = chars[..5].iter().collect();
            let (lo, hi) = self.range_words_starting_with(&prefix);
            for i in lo..hi {
                if let Some(e) = self.entry(i) {
                    union.extend_from_slice(self.postings(&e));
                }
            }
        }
        union.sort_unstable();
        union.dedup();
        Some(union)
    }
}

fn u32_at(data: &[u8], off: usize) -> Option<u32> {
    data.get(off..off + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// Build a TIX1 index from block texts. `block_count` is the total number of
/// blocks (used for staleness checks at open time).
pub fn build_text_index<'a, I>(
    texts: I,
    block_count: usize,
    output_path: &Path,
) -> Result<(), String>
where
    I: IntoIterator<Item = &'a str>,
{
    // Word → block ids. Blocks are visited in ascending order, so each
    // posting list is naturally ascending.
    let mut map: HashMap<String, Vec<u32>> = HashMap::new();
    let mut total_postings = 0usize;
    for (i, text) in texts.into_iter().enumerate() {
        let mut words = tokenize(text);
        words.sort_unstable();
        words.dedup();
        for w in words {
            total_postings += 1;
            map.entry(w).or_default().push(i as u32);
        }
    }

    let mut words: Vec<(&String, &Vec<u32>)> = map.iter().collect();
    words.sort_by(|a, b| a.0.cmp(b.0));

    let mut buf = Vec::with_capacity(HEADER_SIZE + words.len() * 4 + total_postings * 4);
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&(words.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(total_postings as u32).to_le_bytes());
    buf.extend_from_slice(&(block_count as u32).to_le_bytes());
    // postings_offset placeholder; patched after the entries are laid out and
    // the buffer is padded to 4-byte alignment (postings are read as f32-slice
    // aliases, so they must be 4-aligned relative to the mmap base).
    buf.extend_from_slice(&0u32.to_le_bytes());

    let offsets_pos = buf.len();
    buf.resize(buf.len() + words.len() * 4, 0);

    let mut posting_cursor = 0usize;
    let mut postings_buf: Vec<u8> = Vec::with_capacity(total_postings * 4);
    for (wi, (w, list)) in words.iter().enumerate() {
        let entry_off = buf.len();
        let off = offsets_pos + wi * 4;
        buf[off..off + 4].copy_from_slice(&(entry_off as u32).to_le_bytes());
        let wb = w.as_bytes();
        buf.extend_from_slice(&(wb.len() as u16).to_le_bytes());
        buf.extend_from_slice(wb);
        buf.extend_from_slice(&(posting_cursor as u32).to_le_bytes());
        buf.extend_from_slice(&(list.len() as u32).to_le_bytes());
        for &id in list.iter() {
            postings_buf.extend_from_slice(&id.to_le_bytes());
        }
        posting_cursor += list.len();
    }
    // Pad so the postings region starts on a 4-byte boundary.
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
    let postings_offset = buf.len() as u32;
    buf[16..20].copy_from_slice(&postings_offset.to_le_bytes());
    buf.extend_from_slice(&postings_buf);

    let tmp_path = output_path.with_extension("bin.tmp");
    fs::write(&tmp_path, &buf).map_err(|e| format!("write text_index.bin: {e}"))?;
    fs::rename(&tmp_path, output_path).map_err(|e| format!("rename text_index.bin: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_from(texts: &[&str]) -> (tempfile::TempDir, TextIndex) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_index.bin");
        build_text_index(texts.iter().copied(), texts.len(), &path).unwrap();
        let idx = TextIndex::open(&path).unwrap();
        (dir, idx)
    }

    #[test]
    fn tokenize_basic() {
        assert_eq!(tokenize("Rust is fast!"), vec!["rust", "is", "fast"]);
        assert_eq!(tokenize("a b"), Vec::<String>::new());
        assert_eq!(tokenize("rust-mmap"), vec!["rust", "mmap"]);
        assert_eq!(tokenize(""), Vec::<String>::new());
        // Hungarian unicode lowercase.
        assert_eq!(tokenize("ÁRvíZ"), vec!["árvíz"]);
    }

    #[test]
    fn roundtrip_and_substring_candidates() {
        let (_dir, idx) = build_from(&[
            "Rust is fast",
            "Memory is managed by the runtime",
            "rust zooms across depths",
        ]);
        assert_eq!(idx.block_count(), 3);

        // Exact word.
        let c = idx.candidates_substring("rust").unwrap();
        assert_eq!(c, vec![0, 2]);

        // Word in two blocks, intersection with another word.
        let c = idx.candidates_substring("rust is").unwrap();
        assert_eq!(c, vec![0]);

        // Fragment inside a longer word (substring semantics).
        let c = idx.candidates_substring("rus").unwrap();
        assert_eq!(c, vec![0, 2]);

        // Missing word → empty, not fallback.
        assert_eq!(
            idx.candidates_substring("xyzzy").unwrap(),
            Vec::<u32>::new()
        );

        // Too short → not indexable.
        assert!(idx.candidates_substring("a").is_none());
    }

    #[test]
    fn lexical_candidates_match_token_similarity() {
        use crate::relevance::RelevanceQuery;
        let texts = [
            "rusting is a memory effect",
            "rusti prefix of rusting",
            "rust never sleeps",
            "quick brown fox",
        ];
        let (_dir, idx) = build_from(&texts);

        // Query "rusting": matches exact token (0), shared-5-prefix "rusti" (1).
        // "rust" (block 2) is only 4 chars — token_similarity requires the
        // shorter side to have length >= 5, so it does NOT match "rusting".
        let q = RelevanceQuery::new("rusting");
        let cands = idx.candidates_lexical(q.tokens()).unwrap();
        assert_eq!(cands, vec![0, 1]);

        // Short token (len 3-4): only exact equality.
        let q = RelevanceQuery::new("rust");
        let cands = idx.candidates_lexical(q.tokens()).unwrap();
        assert_eq!(cands, vec![2]);
    }

    #[test]
    fn missing_or_empty_lexical() {
        let (_dir, idx) = build_from(&["one two three"]);
        let cands = idx.candidates_lexical(&["zzz".to_string()]).unwrap();
        assert!(cands.is_empty());
        assert!(idx.candidates_lexical(&[]).is_none());
    }

    #[test]
    fn truncated_or_corrupt_file_fails_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_index.bin");
        build_text_index(["hello world"].iter().copied(), 1, &path).unwrap();
        let bytes = fs::read(&path).unwrap();
        // Truncate in the middle of postings.
        fs::write(&path, &bytes[..bytes.len() - 3]).unwrap();
        assert!(TextIndex::open(&path).is_none());
        // Bad magic.
        fs::write(&path, b"NOPE").unwrap();
        assert!(TextIndex::open(&path).is_none());
    }

    #[test]
    fn scale_search_is_selective() {
        // 50k synthetic blocks, one unique term each. The index must return
        // only the matching block(s), not scan everything.
        let mut texts: Vec<String> = Vec::with_capacity(50_000);
        for i in 0..50_000u32 {
            texts.push(format!("unique_term_{i} common filler text"));
        }
        let owned: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_index.bin");
        build_text_index(owned.iter().copied(), owned.len(), &path).unwrap();
        let idx = TextIndex::open(&path).unwrap();

        // 49999 is the maximum id, so no other token contains it as a
        // substring — the candidate set is exactly one block.
        let c = idx.candidates_substring("unique_term_49999").unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0], 49999);
        // Substring semantics: "4242" also appears inside 14242/24242/34242/
        // 44242 and 42420..42429, but candidates stay tiny vs 50k blocks.
        let c = idx.candidates_substring("unique_term_4242").unwrap();
        assert!(c.contains(&4242), "matching block must be a candidate");
        assert!(
            c.len() <= 15,
            "candidates must be selective, got {}",
            c.len()
        );
        // Common word hits all blocks.
        let c = idx.candidates_substring("common").unwrap();
        assert_eq!(c.len(), 50_000);
    }
}
