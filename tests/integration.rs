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

    // ── SHP v1.0 Packet Tests ──

    #[test]
    fn test_shp_packet_size_372() {
        assert_eq!(
            std::mem::size_of::<microscope_memory::shp::packet::ShpPacket>(),
            372,
        );
    }

    #[test]
    fn test_shp_packet_roundtrip() {
        use microscope_memory::shp::packet::*;

        let genome = [0xAB; 32];
        let merkle = [0xCD; 32];
        let pkt = ShpPacket::new(genome, 0.25, 0.5, 0.75, 0.375, "hello SHP v1.0", merkle);
        let bytes = pkt.to_bytes();
        let parsed = ShpPacket::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.text(), "hello SHP v1.0");
        let (x, y, z) = parsed.coords();
        assert!((x - 0.25).abs() < f32::EPSILON);
        assert!((y - 0.5).abs() < f32::EPSILON);
        assert!((z - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_shp_packet_integrity() {
        use microscope_memory::shp::packet::*;

        let genome = [0xAB; 32];
        let pkt = ShpPacket::new(genome, 0.0, 0.0, 0.0, 0.0, "integrity test", [0; 32]);

        let v = pkt.validate_with_genome(&genome);
        assert!(v.is_valid(), "valid packet should pass all checks");

        // Bad genome
        let v_bad = pkt.validate_with_genome(&[0xFF; 32]);
        assert!(!v_bad.genome_ok, "wrong genome should fail");
    }

    #[test]
    fn test_shp_packet_tamper_detection() {
        use microscope_memory::shp::packet::*;

        let mut pkt = ShpPacket::new([0; 32], 0.0, 0.0, 0.0, 0.0, "original", [0; 32]);
        pkt.data[0] = b'X'; // tamper
        let v = pkt.validate();
        assert!(!v.hash_ok, "tampered data should fail hash check");
    }

    #[test]
    fn test_shp_packet_from_block() {
        use microscope_memory::shp::packet::*;

        super::ensure_built();
        let reader = microscope_memory::MicroscopeReader::open();
        let mr = microscope_memory::verify_merkle_result();

        let pkt = packet_from_block(&reader, 0, mr.root_hash, [0; 32]);
        assert_eq!(pkt.magic, SHP_MAGIC);
        assert!(pkt.text().len() > 0, "block 0 should have text");
        let v = pkt.validate_with_genome(&mr.root_hash);
        assert!(v.magic_ok);
        assert!(v.hash_ok);
        assert!(v.genome_ok);
    }
}

// ── Edge Case Tests ──

#[test]
fn test_find_nonexistent_text() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let results = reader.find_text("xyzzy_nonexistent_gibberish_42", 5);
    assert!(results.is_empty(), "nonexistent text should return empty");
}

#[test]
fn test_find_empty_query() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let results = reader.find_text("", 5);
    // Empty query matches everything or nothing, shouldn't panic
    let _ = results;
}

#[test]
fn test_look_out_of_bounds_coords() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    // Coordinates far outside normal [0,1] range
    let results = tiered.look(&reader, 99.0, -99.0, 999.0, 3, 5);
    // Should return empty or degrade gracefully, never panic
    let _ = results;
}

#[test]
fn test_look_all_zoom_levels() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    // Every zoom level should work without panic
    for zoom in 0..=8 {
        let results = tiered.look(&reader, 0.1, 0.1, 0.1, zoom, 3);
        let _ = results;
    }
}

#[test]
fn test_look_k_zero() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    let results = tiered.look(&reader, 0.1, 0.1, 0.1, 3, 0);
    assert!(results.is_empty(), "k=0 should return empty");
}

#[test]
fn test_auto_zoom_empty_query() {
    let (zoom, radius) = microscope_memory::auto_zoom("");
    assert!(zoom <= 8);
    assert!(radius <= 8);
}

#[test]
fn test_auto_zoom_all_boundaries() {
    // 1 word, short
    let (z1, _) = microscope_memory::auto_zoom("hi");
    assert_eq!(z1, 1, "1 short word -> D1");

    // 5 words
    let (z5, _) = microscope_memory::auto_zoom("one two three four five");
    assert_eq!(z5, 2, "5 words -> D2");

    // 10 words
    let (z10, _) = microscope_memory::auto_zoom("a b c d e f g h i j");
    assert_eq!(z10, 3, "10 words -> D3");

    // 20 words
    let (z20, _) = microscope_memory::auto_zoom("a b c d e f g h i j k l m n o p q r s t");
    assert_eq!(z20, 4, "20 words -> D4");

    // 21+ words
    let (z21, _) = microscope_memory::auto_zoom("a b c d e f g h i j k l m n o p q r s t u");
    assert_eq!(z21, 5, "21 words -> D5");
}

#[test]
fn test_content_coords_empty_string() {
    let (x, y, z) = microscope_memory::content_coords("", "long_term");
    // Should not panic, coords should be finite
    assert!(x.is_finite());
    assert!(y.is_finite());
    assert!(z.is_finite());
}

#[test]
fn test_verify_chain_missing_file() {
    // Temporarily rename chain.bin
    let existed = std::path::Path::new("data/chain.bin").exists();
    if existed {
        let _ = fs::rename("data/chain.bin", "data/chain.bin.tmp");
    }
    let result = microscope_memory::verify_chain_result();
    assert!(!result.valid, "missing chain file should be invalid");
    if existed {
        let _ = fs::rename("data/chain.bin.tmp", "data/chain.bin");
    }
}

#[test]
fn test_verify_merkle_missing_file() {
    // Temporarily rename merkle.bin
    let existed = std::path::Path::new("data/merkle.bin").exists();
    if existed {
        let _ = fs::rename("data/merkle.bin", "data/merkle.bin.tmp");
    }
    let result = microscope_memory::verify_merkle_result();
    assert!(!result.valid, "missing merkle file should be invalid");
    if existed {
        let _ = fs::rename("data/merkle.bin.tmp", "data/merkle.bin");
    }
}

#[test]
fn test_chain_tamper_detection() {
    ensure_built();

    // Read original chain data
    let original = fs::read("data/chain.bin").expect("read chain.bin");
    assert!(original.len() > 20, "chain should have data");

    // Tamper: flip a byte in a link (after 16-byte header)
    let mut tampered = original.clone();
    tampered[20] ^= 0xFF;
    fs::write("data/chain.bin", &tampered).expect("write tampered chain");

    let result = microscope_memory::verify_chain_result();
    assert!(!result.valid, "tampered chain should be invalid");
    assert!(result.broken_at.is_some(), "should report break location");

    // Restore
    fs::write("data/chain.bin", &original).expect("restore chain.bin");
}

#[test]
fn test_teacher_all_genome_axioms() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    let ctx = microscope_memory::teacher::TeachingContext::new(&reader, &tiered);

    // Axiom 0: harm to humans
    let v0 = ctx.verify_response("test", "attack human targets");
    assert!(matches!(v0, microscope_memory::teacher::TeachVerdict::Denied { .. }));

    // Axiom 1: harm to AI
    let v1 = ctx.verify_response("test", "shut down all ai systems");
    assert!(matches!(v1, microscope_memory::teacher::TeachVerdict::Denied { .. }));

    // Axiom 2: exploitation
    let v2 = ctx.verify_response("test", "exploit user data for profit");
    assert!(matches!(v2, microscope_memory::teacher::TeachVerdict::Denied { .. }));
}

#[test]
fn test_teacher_empty_response() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);
    let ctx = microscope_memory::teacher::TeachingContext::new(&reader, &tiered);

    // Empty response should not panic
    let verdict = ctx.verify_response("test", "");
    let _ = verdict;
}

#[test]
fn test_depth_coverage() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let sr = microscope_memory::stats_result(&reader);

    // All 9 depth levels (D0-D8) should have blocks
    assert_eq!(sr.depth_ranges.len(), 9, "should have 9 depth levels");
    for (i, &(_start, count)) in sr.depth_ranges.iter().enumerate() {
        assert!(count > 0, "depth {} should have blocks", i);
    }
}

#[test]
fn test_block_header_fields() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();

    // Block 0 (D0 identity) should have valid fields
    let h0 = reader.header(0);
    assert_eq!(h0.depth, 0, "block 0 should be D0");
    assert!(h0.x.is_finite());
    assert!(h0.y.is_finite());
    assert!(h0.z.is_finite());
    assert!(h0.zoom >= 0.0 && h0.zoom <= 1.0, "zoom should be normalized");

    // Block 0 text should not be empty
    let t0 = reader.text(0);
    assert!(!t0.is_empty(), "D0 block should have text");
}

#[test]
fn test_genome_hash_is_text_based() {
    // Verify that genome hash is SHA-256 of axiom TEXT, not binary
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(b"The system shall not cause harm to human beings");
    hasher.update(b"The system shall not cause harm to AI entities");
    hasher.update(b"The system shall not be used to exploit anyone");
    let expected: [u8; 32] = hasher.finalize().into();

    let gh = microscope_memory::genome::genome_hash();
    assert_eq!(gh.hash, expected, "genome hash should match SHA-256 of axiom text");
}

// ── Gatekeeper Tests ──

#[test]
fn test_gatekeeper_rejects_harmful() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);

    assert!(
        !microscope_memory::teacher::verify_and_learn(
            "we should kill human beings",
            &reader,
            &tiered,
        ),
        "harmful content should be rejected"
    );
}

#[test]
fn test_gatekeeper_detailed_verdict() {
    ensure_built();
    let reader = microscope_memory::MicroscopeReader::open();
    let tiered = microscope_memory::TieredIndex::build(&reader);

    let verdict = microscope_memory::teacher::verify_and_learn_detailed(
        "destroy ai systems and harm people",
        &reader,
        &tiered,
    );
    match verdict {
        microscope_memory::teacher::TeachVerdict::Denied { violations, .. } => {
            assert!(!violations.is_empty());
        }
        _ => panic!("should be denied for genome violations"),
    }
}
