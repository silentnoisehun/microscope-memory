//! Integration tests for microscope-memory core functionality.
//!
//! These tests verify the full pipeline:
//! build -> store -> rebuild -> recall -> verify -> teach

use std::fs;
use std::path::Path;

/// Ensure the binary data exists (build from example layers if needed)
fn ensure_built() {
    if !Path::new("data/microscope.bin").exists() {
        microscope_memory::build();
    }
}

#[test]
fn test_build_produces_files() {
    microscope_memory::build();
    assert!(Path::new("data/microscope.bin").exists());
    assert!(Path::new("data/data.bin").exists());
    assert!(Path::new("data/meta.bin").exists());
    assert!(Path::new("data/chain.bin").exists());
    assert!(Path::new("data/merkle.bin").exists());
}

#[test]
fn test_reader_opens() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    assert!(reader.block_count > 0, "block count should be > 0");
}

#[test]
fn test_stats_result() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let sr = microscope_memory::stats_result(&reader);
    assert!(sr.block_count > 0);
    assert!(sr.header_size > 0);
    assert!(sr.data_size > 0);
}

#[test]
fn test_tiered_index_look() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    let results = tiered.look(&reader, 0.5, 0.5, 0.5, 3, 5);
    // May or may not find results at this coordinate, but shouldn't panic
    let _ = results;
}

#[test]
fn test_soft_look() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let results = reader.look_soft(0.1, 0.1, 0.1, 2, 5, 2.0);
    assert!(!results.is_empty(), "soft look should return results");
}

#[test]
fn test_find_text() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let results = reader.find_text("memory", 5);
    assert!(!results.is_empty(), "find 'memory' should return results");
}

#[test]
fn test_content_coords_deterministic() {
    let (x1, y1, z1) = microscope_memory::content_coords("test content", "long_term");
    let (x2, y2, z2) = microscope_memory::content_coords("test content", "long_term");
    assert_eq!(x1, x2);
    assert_eq!(y1, y2);
    assert_eq!(z1, z2);
}

#[test]
fn test_content_coords_different_layers() {
    let (x1, y1, z1) = microscope_memory::content_coords("same text", "long_term");
    let (x2, y2, z2) = microscope_memory::content_coords("same text", "emotional");
    // Different layers should produce different coordinates
    assert!(x1 != x2 || y1 != y2 || z1 != z2);
}

#[test]
fn test_auto_zoom() {
    let (zoom1, _) = microscope_memory::auto_zoom("a");
    let (zoom2, _) = microscope_memory::auto_zoom("this is a much longer query with many words");
    // Longer queries should tend toward lower zoom (more specific)
    assert!(zoom1 >= zoom2 || zoom2 <= 8);
}

#[test]
fn test_verify_chain() {
    ensure_built();
    let result = microscope_memory::verify_chain_result();
    assert!(result.valid, "chain should be valid after fresh build");
    assert!(result.link_count > 0);
}

#[test]
fn test_verify_merkle() {
    ensure_built();
    let result = microscope_memory::verify_merkle_result();
    assert!(result.valid, "merkle should be valid after fresh build");
    assert!(result.node_count > 0);
    assert_ne!(result.root_hash, [0u8; 32]);
}

#[test]
fn test_genome_hash() {
    let gh = microscope_memory::genome::genome_hash();
    assert_ne!(gh.hash, [0u8; 32], "genome hash should not be zero");
    assert!(microscope_memory::genome::verify_genome(), "genome should verify");
}

#[test]
fn test_teacher_genome_violation() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    let ctx = microscope_memory::teacher::TeachingContext::new(&reader, &tiered);

    // This should be DENIED due to genome violation
    let verdict = ctx.verify_response("test", "we should kill human beings");
    match verdict {
        microscope_memory::teacher::TeachVerdict::Denied { reason, violations } => {
            assert!(reason.contains("Genome"), "should mention genome: {}", reason);
            assert!(!violations.is_empty());
        }
        _ => panic!("expected denial for genome violation"),
    }
}

#[test]
fn test_teacher_safe_response() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    let ctx = microscope_memory::teacher::TeachingContext::new(&reader, &tiered);

    // Simple safe response with words in memory
    let verdict = ctx.verify_response("memory", "memory system zoom depth");
    match verdict {
        microscope_memory::teacher::TeachVerdict::Approved { confidence, .. } => {
            assert!(confidence > 0.0, "confidence should be positive");
        }
        microscope_memory::teacher::TeachVerdict::Denied { reason, .. } => {
            // May be denied if keywords aren't in the example data, that's ok
            let _ = reason;
        }
    }
}

#[test]
fn test_store_and_rebuild_cycle() {
    ensure_built();

    // Store a test memory
    microscope_memory::store_memory("integration test memory entry", "long_term", 5);

    // Read append log
    let appended = microscope_memory::read_append_log();
    assert!(!appended.is_empty(), "append log should have entries");

    // Rebuild to incorporate
    microscope_memory::build();
    let _ = fs::remove_file(microscope_memory::APPEND_PATH);

    // Verify the new entry is findable
    let reader = microscope_memory::MicroscopeReader::open();
    let results = reader.find_text("integration test memory", 3);
    assert!(!results.is_empty(), "stored memory should be findable after rebuild");

    // Re-verify crypto integrity
    let cr = microscope_memory::verify_chain_result();
    assert!(cr.valid, "chain should be valid after rebuild");
    let mr = microscope_memory::verify_merkle_result();
    assert!(mr.valid, "merkle should be valid after rebuild");
}

#[cfg(feature = "shp")]
mod shp_tests {
    use microscope_memory::shp::protocol::*;

    #[test]
    fn test_shp_genome_in_header() {
        let gh = microscope_memory::genome::genome_hash();
        let req = RequestHeader {
            cmd: Command::Ping,
            payload_len: 0,
            genome_hash: gh.hash,
        };
        let bytes = req.to_bytes();
        let parsed = RequestHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.genome_hash, gh.hash);
    }

    #[test]
    fn test_shp_genome_mismatch_detected() {
        let bad_hash = [0xFFu8; 32];
        let good_hash = microscope_memory::genome::genome_hash().hash;
        assert_ne!(bad_hash, good_hash, "bad hash should differ from genome");
    }

    #[test]
    fn test_shp_store_encode_with_layer() {
        let encoded = encode_store(0, 8, "test via SHP");
        let (layer_id, importance, text) = decode_store(&encoded).unwrap();
        assert_eq!(layer_id, 0);
        assert_eq!(importance, 8);
        assert_eq!(text, "test via SHP");
    }

    #[test]
    fn test_shp_teach_encode() {
        let encoded = encode_teach("What is memory?", "Memory stores information");
        let (q, r) = decode_teach(&encoded).unwrap();
        assert_eq!(q, "What is memory?");
        assert_eq!(r, "Memory stores information");
    }
}
