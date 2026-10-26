//! Regression tests for the meta.bin writer in `dream.rs`.
//!
//! `dream.rs` had a second, independent implementation of the index writer
//! that sat next to `build.rs`. It wrote a 12-byte meta header (magic,
//! version, block_count -- no `depth_count`) where every reader assumes 16,
//! read `depth` from byte 14 instead of byte 16, and copied the old
//! `data_offset` and `crc16` into the compacted index while writing a
//! freshly packed `data.bin`.
//!
//! The visible damage on the canonical C: index was three-fold:
//!
//! * the four-byte shift moved the depth table four bytes left, so the reader
//!   read the Merkle root as the D8 count -- 3,595,294,311 -- and
//!   `look()`'s `Vec::with_capacity(count)` asked for 57 GB and aborted the
//!   process;
//! * reading `depth` from byte 14 (the low byte of the `zoom` f32) put
//!   almost every block in D0, so the depth table described an index that
//!   did not exist;
//! * the stale `data_offset`/`crc16` left 96.6% of the blocks failing their
//!   CRC check.
//!
//! These cases drive the compaction and then read the result back with the
//! real reader, so the assertions are on the observable behaviour rather
//! than on the writer's internals.

use std::fs;
use std::path::Path;

use microscope_memory::config::Config;
use microscope_memory::reader::MicroscopeReader;

/// A packed, variable-length index: the depth is written at byte 16, the
/// data span is recorded in the header rather than derived from a grid.
struct PackedIndex {
    headers: Vec<u8>,
    data: Vec<u8>,
}

/// Build a small index by hand.
///
/// `build.rs` sorts the blocks by depth, and the depth table describes one
/// contiguous run per depth, so the fixture keeps the same layout: a D0
/// block, then a D5 block, then a D8 block.
fn packed_index() -> PackedIndex {
    const HEADER_SIZE: usize = 50;
    let mut headers = Vec::new();
    let mut data = Vec::new();

    let blocks: [(&[u8], u8, u8); 3] = [
        (b"root thought", 0, 9),
        (b"middle thought", 5, 7),
        (b"leaf thought", 8, 4),
    ];

    for (text, depth, importance) in blocks {
        let offset = data.len() as u32;
        let len = text.len() as u16;
        data.extend_from_slice(text);

        let mut h = [0u8; HEADER_SIZE];
        h[16] = depth;
        h[17] = 0; // layer_id
        h[18..22].copy_from_slice(&offset.to_le_bytes());
        h[22..24].copy_from_slice(&len.to_le_bytes());
        h[24..28].copy_from_slice(&u32::MAX.to_le_bytes()); // no parent
        h[30..32].copy_from_slice(&microscope_memory::crc16_ccitt(text).to_le_bytes());
        h[48] = importance;
        headers.extend_from_slice(&h);
    }

    PackedIndex { headers, data }
}

/// Write a meta.bin in the canonical 16-byte-header layout.
///
/// The depth table always has nine entries, whether or not a level holds
/// blocks, so the fixture has to fill all nine.
fn canonical_meta(block_count: u32, depth_ranges: &[(u32, u32)]) -> Vec<u8> {
    let mut meta = Vec::new();
    meta.extend_from_slice(b"MSC4");
    meta.extend_from_slice(&4u32.to_le_bytes());
    meta.extend_from_slice(&block_count.to_le_bytes());
    meta.extend_from_slice(&9u32.to_le_bytes());
    for d in 0..9 {
        let (start, count) = depth_ranges.get(d).copied().unwrap_or((0, 0));
        meta.extend_from_slice(&start.to_le_bytes());
        meta.extend_from_slice(&count.to_le_bytes());
    }
    meta.extend_from_slice(&[0u8; 32]); // merkle root
    meta.extend_from_slice(&[0u8; 32]); // layers hash
    meta
}

/// Write a meta.bin the way the old `dream.rs` writer did: a 12-byte header
/// and the depth table starting four bytes earlier.
fn short_header_meta(block_count: u32, depth_ranges: &[(u32, u32)]) -> Vec<u8> {
    let mut meta = Vec::new();
    meta.extend_from_slice(b"MSC4");
    meta.extend_from_slice(&4u32.to_le_bytes());
    meta.extend_from_slice(&block_count.to_le_bytes());
    for d in 0..9 {
        let (start, count) = depth_ranges.get(d).copied().unwrap_or((0, 0));
        meta.extend_from_slice(&start.to_le_bytes());
        meta.extend_from_slice(&count.to_le_bytes());
    }
    meta.extend_from_slice(&[0u8; 32]);
    meta.extend_from_slice(&[0u8; 32]);
    meta
}

fn write_index(dir: &Path, idx: &PackedIndex, meta: &[u8]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("microscope.bin"), &idx.headers).unwrap();
    fs::write(dir.join("data.bin"), &idx.data).unwrap();
    fs::write(dir.join("meta.bin"), meta).unwrap();
}

fn config_for(dir: &Path) -> Config {
    let mut config = Config::default();
    config.paths.output_dir = dir.to_string_lossy().to_string();
    config.paths.layers_dir = dir.join("layers").to_string_lossy().to_string();
    config.paths.temp_dir = dir.join("tmp").to_string_lossy().to_string();
    config
}

/// The meta.bin the compaction wrote must be the canonical 152-byte layout.
///
/// The old writer emitted 148 bytes: a 12-byte header plus the depth table
/// plus the two 32-byte tails. Every reader in the codebase assumes the
/// 16-byte header, so the 148-byte file shifted the whole depth table four
/// bytes left.
#[test]
fn compaction_writes_a_canonical_meta() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    // The D0 block has importance 9, the D5 block 7 and the D8 block 4.
    // FORGET_INTERNAL_LAYERS covers layer 0 only, so nothing is dropped by
    // layer; the low-importance D8 block is dropped by the eviction path
    // instead, which is exercised separately below.
    let forgotten = microscope_memory::dream::forget_old_thoughts(&dir, 3).unwrap();
    assert_eq!(forgotten, 0, "nothing should have been forgotten");

    // Nothing was forgotten, so the meta is untouched -- but it must still
    // be readable. This asserts the fixture, not the writer.
    let meta = fs::read(dir.join("meta.bin")).unwrap();
    assert_eq!(meta.len(), 152, "the fixture meta is not canonical");
}

/// The depth table has to describe the *actual* depths of the blocks.
///
/// The old writer read `depth` from byte 14 -- the low byte of the `zoom`
/// f32 -- so every block landed in D0 and the table claimed 3 blocks at D0
/// and 0 at every other level.
#[test]
fn depth_table_matches_the_actual_block_depths() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    // Drop the D8 block by giving it importance below the floor and putting
    // it on an internal layer, so `forget_old_thoughts` removes it and
    // rewrites the index.
    let mut headers = idx.headers.clone();
    // The D8 block is the third one; its layer_id (byte 17) becomes an
    // internal layer (session = 11) and its importance drops below 5.
    headers[2 * 50 + 17] = 11;
    headers[2 * 50 + 48] = 2;
    let idx = PackedIndex {
        headers,
        data: idx.data,
    };
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    let forgotten = microscope_memory::dream::forget_old_thoughts(&dir, 3).unwrap();
    assert_eq!(
        forgotten, 1,
        "the low-importance session block should be forgotten"
    );

    let meta = fs::read(dir.join("meta.bin")).unwrap();
    assert_eq!(
        meta.len(),
        152,
        "the compaction wrote a {} byte meta.bin; the canonical layout is 152 bytes \
         (16 header + 9*8 depth table + 32 merkle + 32 layers hash)",
        meta.len()
    );

    let block_count = u32::from_le_bytes(meta[8..12].try_into().unwrap());
    assert_eq!(
        block_count, 2,
        "the meta must claim the surviving block count"
    );

    // The surviving blocks are D0 and D5: one run each, in depth order.
    let ranges: Vec<(u32, u32)> = (0..9)
        .map(|d| {
            let off = 16 + d * 8;
            (
                u32::from_le_bytes(meta[off..off + 4].try_into().unwrap()),
                u32::from_le_bytes(meta[off + 4..off + 8].try_into().unwrap()),
            )
        })
        .collect();
    assert_eq!(ranges[0], (0, 1), "D0 must hold exactly the one D0 block");
    assert_eq!(ranges[5], (1, 1), "D5 must hold exactly the one D5 block");
    assert_eq!(
        ranges[8],
        (0, 0),
        "D8 must be empty after the D8 block was forgotten"
    );
}

/// Every block's CRC must verify against the bytes it points at.
///
/// The old writer copied the old `data_offset` and `crc16` into the
/// compacted headers while packing a new `data.bin`, so the header pointed
/// at the old position and the CRC described bytes that were no longer
/// there -- 96.6% of the blocks failed the check.
#[test]
fn compacted_blocks_pass_their_crc_check() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();

    let mut headers = idx.headers.clone();
    // Make the middle block forgettable: internal layer, low importance.
    headers[50 + 17] = 11;
    headers[50 + 48] = 1;
    let idx = PackedIndex {
        headers,
        data: idx.data,
    };
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    let forgotten = microscope_memory::dream::forget_old_thoughts(&dir, 3).unwrap();
    assert_eq!(forgotten, 1);

    // Read the compacted index back through the real reader and verify every
    // block's CRC against the bytes its header points at.
    let config = config_for(&dir);
    let reader = MicroscopeReader::open(&config).unwrap();
    assert_eq!(reader.block_count, 2);

    let data = fs::read(dir.join("data.bin")).unwrap();
    let mut verified = 0;
    for i in 0..reader.block_count {
        let h = reader.header(i);
        let start = h.data_offset as usize;
        let data_len = h.data_len as usize;
        let end = start + data_len;
        assert!(
            end <= data.len(),
            "block {i} points past the end of data.bin ({start}+{data_len} > {})",
            data.len()
        );
        let computed = microscope_memory::crc16_ccitt(&data[start..end]);
        let stored = u16::from_le_bytes(h.crc16);
        assert_eq!(
            computed, stored,
            "block {i} fails its CRC: the header points at offset {start} but the \
             compaction did not rewrite data_offset/crc16"
        );
        verified += 1;
    }
    assert_eq!(verified, 2);
}

/// The blocks must still hold their own text after the compaction.
///
/// A compaction that re-packs `data.bin` has to move each block's bytes to
/// the position its header names, and the header has to follow.
#[test]
fn compacted_blocks_keep_their_text() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();

    let mut headers = idx.headers.clone();
    headers[50 + 17] = 11;
    headers[50 + 48] = 1;
    let idx = PackedIndex {
        headers,
        data: idx.data,
    };
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    microscope_memory::dream::forget_old_thoughts(&dir, 3).unwrap();

    let config = config_for(&dir);
    let reader = MicroscopeReader::open(&config).unwrap();
    let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();
    assert!(
        texts.iter().any(|t| t.contains("root thought")),
        "the surviving D0 block lost its text: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("leaf thought")),
        "the surviving D8 block lost its text: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.contains("middle thought")),
        "the forgotten block is still in the index: {texts:?}"
    );
}

/// A meta.bin written with the old 12-byte header must be refused, not
/// turned into a 57 GB allocation.
///
/// This is the exact damage found on the canonical index: the reader assumed
/// a 16-byte header, read the depth table four bytes early, and the last
/// entry picked up the Merkle root -- 3,595,294,311 as its count.
#[test]
fn a_short_header_meta_is_refused_instead_of_allocating() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();

    // The short-header writer, with a Merkle root whose low bytes become a
    // plausible-looking but enormous count when read four bytes early.
    let mut meta = short_header_meta(3, &[(0, 1), (1, 1), (2, 1)]);
    assert_eq!(meta.len(), 148, "the short-header layout is 148 bytes");
    // Plant the root so the shifted read yields the historical count.
    meta[84..88].copy_from_slice(&3_595_294_311u32.to_le_bytes());
    write_index(&dir, &idx, &meta);

    let config = config_for(&dir);
    let err = MicroscopeReader::open(&config);
    assert!(
        err.is_err(),
        "a meta.bin with a 12-byte header was accepted; the depth table it \
         describes does not fit the index"
    );

    // The refusal must name the cause, not just fail.
    let msg = match err {
        Err(m) => m,
        Ok(_) => panic!("a meta.bin with a 12-byte header was accepted"),
    };
    assert!(
        msg.contains("corrupt") || msg.contains("depth"),
        "the error does not explain the damage: {msg}"
    );
}

/// The reader must refuse a depth entry that runs past the block count,
/// even when the file is otherwise well formed.
#[test]
fn a_depth_count_past_the_block_count_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();
    let mut meta = canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]);
    // D8 claims 3,595,294,311 blocks out of an index that holds three.
    meta[16 + 8 * 8 + 4..16 + 8 * 8 + 8].copy_from_slice(&3_595_294_311u32.to_le_bytes());
    write_index(&dir, &idx, &meta);

    let config = config_for(&dir);
    assert!(
        MicroscopeReader::open(&config).is_err(),
        "a depth entry claiming 3.6 billion blocks in a three-block index was accepted"
    );
}

/// The eviction path writes the same canonical meta as the forgetting path.
#[test]
fn eviction_writes_a_canonical_meta() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    // Cap the index at two blocks. The D8 block has importance 4 and the
    // protection floor is 8, so it is the one that goes.
    let evicted = microscope_memory::dream::evict_over_capacity(&dir, 2, 8).unwrap();
    assert_eq!(evicted, 1, "one block should have been evicted");

    let meta = fs::read(dir.join("meta.bin")).unwrap();
    assert_eq!(
        meta.len(),
        152,
        "the eviction wrote a {} byte meta.bin; the canonical layout is 152 bytes",
        meta.len()
    );
    assert_eq!(
        u32::from_le_bytes(meta[8..12].try_into().unwrap()),
        2,
        "the meta must claim the surviving block count"
    );

    let config = config_for(&dir);
    let reader = MicroscopeReader::open(&config).unwrap();
    assert_eq!(reader.block_count, 2);

    // The D0 block survives and the D5 block (importance 7 < 8) went too --
    // whichever way the score fell, the index must still verify.
    let data = fs::read(dir.join("data.bin")).unwrap();
    for i in 0..reader.block_count {
        let h = reader.header(i);
        let start = h.data_offset as usize;
        let data_len = h.data_len as usize;
        let end = start + data_len;
        assert!(
            end <= data.len(),
            "block {i} points past the end of data.bin"
        );
        assert_eq!(
            microscope_memory::crc16_ccitt(&data[start..end]),
            u16::from_le_bytes(h.crc16),
            "block {i} fails its CRC after eviction"
        );
    }
}

/// The compacted index must still be readable by the real reader, with the
/// depth table pointing at the right runs.
#[test]
fn the_compacted_index_opens_and_answers_lookups() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("output");
    let idx = packed_index();

    let mut headers = idx.headers.clone();
    headers[50 + 17] = 11;
    headers[50 + 48] = 1;
    let idx = PackedIndex {
        headers,
        data: idx.data,
    };
    write_index(&dir, &idx, &canonical_meta(3, &[(0, 1), (1, 1), (2, 1)]));

    microscope_memory::dream::forget_old_thoughts(&dir, 3).unwrap();

    let config = config_for(&dir);
    let reader = MicroscopeReader::open(&config).unwrap();
    let d0 = reader.depth_ranges[0];
    let d5 = reader.depth_ranges[5];
    let d8 = reader.depth_ranges[8];
    assert_eq!(d0, (0, 1), "D0 must hold the one D0 block");
    assert_eq!(d5, (0, 0), "D5 must be empty");
    assert_eq!(d8, (1, 1), "D8 must hold the one D8 block");
}
