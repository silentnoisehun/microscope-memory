//! Integration tests for the Microscope Memory pipeline.
//! Tests the full build -> query -> store -> recall -> verify cycle.

use std::fs;
use std::path::{Path, PathBuf};

/// Create a temporary test environment with config pointing to real fixture data.
fn setup_test_env() -> (tempfile::TempDir, microscope_memory::config::Config) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output_dir = tmp.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    // Fixture paths
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = manifest_dir.join("tests").join("fixtures");
    let layers_dir = fixtures.join("layers");

    let mut config = microscope_memory::config::Config::default();
    config.paths.layers_dir = layers_dir.to_string_lossy().to_string();
    config.paths.output_dir = output_dir.to_string_lossy().to_string();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string(), "short_term".to_string()];
    config.embedding.provider = "mock".to_string();
    config.embedding.dim = 128;
    config.embedding.max_depth = 4;

    (tmp, config)
}

#[test]
fn test_full_build_pipeline() {
    let (_tmp, config) = setup_test_env();

    // Build should succeed
    microscope_memory::build::build(&config, true).unwrap();

    // Verify output files exist
    let output_dir = Path::new(&config.paths.output_dir);
    assert!(
        output_dir.join("meta.bin").exists(),
        "meta.bin should exist"
    );
    assert!(
        output_dir.join("microscope.bin").exists(),
        "microscope.bin should exist"
    );
    assert!(
        output_dir.join("data.bin").exists(),
        "data.bin should exist"
    );
    assert!(
        output_dir.join("merkle.bin").exists(),
        "merkle.bin should exist"
    );
    assert!(
        output_dir.join("embeddings.bin").exists(),
        "embeddings.bin should exist"
    );
}

#[test]
fn test_build_and_read() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");
    assert!(reader.block_count > 0, "should have blocks after build");

    // D0 should have exactly 1 block (identity)
    let (_, d0_count) = reader.depth_ranges[0];
    assert_eq!(d0_count, 1, "D0 should have 1 identity block");

    // D1 should have blocks for each layer (2 layers)
    let (_, d1_count) = reader.depth_ranges[1];
    assert_eq!(d1_count, 2, "D1 should have 2 layer summaries");

    // D3 should have individual items (5 + 3 = 8)
    let (_, d3_count) = reader.depth_ranges[3];
    assert_eq!(d3_count, 8, "D3 should have 8 individual items");
}

#[test]
fn test_text_search() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");

    // Search for "Rust" should find matches
    let results = reader.find_text("Rust", 10);
    assert!(!results.is_empty(), "should find 'Rust' in the index");

    // Search for nonexistent text
    let results = reader.find_text("xyznonexistent123", 10);
    assert!(results.is_empty(), "should not find nonexistent text");
}

#[test]
fn test_store_and_recall() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    // Store a new memory
    microscope_memory::store_memory(
        &config,
        "Test memory about quantum computing",
        "long_term",
        5,
    )
    .expect("store");

    // Verify append log exists
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    assert!(append_path.exists(), "append.bin should exist after store");

    // Read the append log
    let entries = microscope_memory::read_append_log(&append_path);
    assert_eq!(entries.len(), 1, "should have 1 append entry");
    assert!(
        entries[0].text.contains("quantum"),
        "stored text should contain 'quantum'"
    );
}

#[test]
fn test_incremental_build_skips() {
    let (_tmp, config) = setup_test_env();

    // First build
    microscope_memory::build::build(&config, false).expect("build");
    let meta1 = fs::read(Path::new(&config.paths.output_dir).join("meta.bin")).unwrap();

    // Second build (should skip -- layers unchanged)
    microscope_memory::build::build(&config, false).expect("build");
    let meta2 = fs::read(Path::new(&config.paths.output_dir).join("meta.bin")).unwrap();

    // Meta should be identical (no rebuild happened)
    assert_eq!(
        meta1, meta2,
        "meta.bin should be identical when layers unchanged"
    );
}

#[test]
fn test_incremental_build_force() {
    let (_tmp, config) = setup_test_env();

    // First build
    microscope_memory::build::build(&config, false).expect("build");

    // Force rebuild should complete without error
    microscope_memory::build::build(&config, true).unwrap();
}

#[test]
fn test_mql_query() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = microscope_memory::read_append_log(&append_path);

    // Query with keyword
    let q = microscope_memory::query::parse("\"Rust\"");
    let results = microscope_memory::query::execute(&q, &reader, &appended);
    assert!(
        !results.is_empty(),
        "MQL search for 'Rust' should return results"
    );

    // Query with depth filter
    let q = microscope_memory::query::parse("depth:3 \"memory\"");
    let results = microscope_memory::query::execute(&q, &reader, &appended);
    for r in &results {
        if r.is_main {
            let h = reader.header(r.block_idx);
            assert_eq!(h.depth, 3, "depth filter should only return D3 blocks");
        }
    }
}

#[test]
fn test_crc_integrity_after_build() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");

    // All blocks should have valid CRC
    for i in 0..reader.block_count {
        let h = reader.header(i);
        let stored = u16::from_le_bytes(h.crc16);
        if stored == 0x0000 {
            continue; // No CRC stored
        }
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        let computed = microscope_memory::crc16_ccitt(&reader.data[start..end]);
        assert_eq!(stored, computed, "CRC mismatch at block {}", i);
    }
}

#[test]
fn test_merkle_integrity_after_build() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let output_dir = Path::new(&config.paths.output_dir);
    let merkle_data = fs::read(output_dir.join("merkle.bin")).unwrap();
    let tree = microscope_memory::merkle::MerkleTree::from_bytes(&merkle_data).unwrap();

    let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");

    // Verify all leaves against Merkle tree
    for i in 0..reader.block_count {
        let h = reader.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        let data = &reader.data[start..end];
        assert!(
            tree.verify_leaf(i, data),
            "Merkle verification failed for block {}",
            i
        );
    }
}

#[test]
fn test_snapshot_export_import() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let output_dir = Path::new(&config.paths.output_dir);
    let archive_path = output_dir.join("test.mscope");

    // Export
    microscope_memory::snapshot::export(output_dir, &archive_path).unwrap();
    assert!(archive_path.exists(), "archive should exist");

    // Import to new directory
    let restore_dir = output_dir.join("restored");
    fs::create_dir_all(&restore_dir).unwrap();
    microscope_memory::snapshot::import(&archive_path, &restore_dir).unwrap();

    // Verify key files restored
    assert!(restore_dir.join("meta.bin").exists());
    assert!(restore_dir.join("microscope.bin").exists());
    assert!(restore_dir.join("data.bin").exists());
}

#[test]
fn test_embedding_index_search() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let output_dir = Path::new(&config.paths.output_dir);
    let emb_path = output_dir.join("embeddings.bin");

    if let Some(idx) = microscope_memory::embedding_index::EmbeddingIndex::open(&emb_path) {
        assert!(idx.block_count() > 0, "embedding index should have blocks");
        assert!(idx.dim() > 0, "embedding dimension should be > 0");

        // Verify embedding can be retrieved for block 0
        let emb0 = idx.embedding(0);
        assert!(emb0.is_some(), "block 0 should have an embedding");
        assert_eq!(
            emb0.unwrap().len(),
            idx.dim(),
            "embedding should have correct dimension"
        );

        // Generate a query embedding — use same text as block 0 for guaranteed match
        use microscope_memory::embeddings::{EmbeddingProvider, MockEmbeddingProvider};
        let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");
        let block0_text = reader.text(0);
        let provider = MockEmbeddingProvider::new(idx.dim());
        let query_emb = provider.embed(block0_text).unwrap();
        let results = idx.search(&query_emb, 5);
        assert!(
            !results.is_empty(),
            "embedding search with same text should return results"
        );
        // The top result should be block 0 itself (cosine sim = 1.0)
        assert_eq!(results[0].1, 0, "top result should be block 0 (same text)");
    } else {
        panic!("embeddings.bin should exist after build");
    }
}
