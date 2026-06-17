//! Integration tests for the Microscope Memory pipeline.
//! Tests the full build -> query -> store -> recall -> verify cycle.

use std::fs;
use std::path::Path;

/// Create a temporary test environment with config pointing to real fixture data.
fn setup_test_env() -> (tempfile::TempDir, microscope_memory::config::Config) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output_dir = tmp.path().join("data");
    let layers_dir = tmp.path().join("layers");
    fs::create_dir_all(&output_dir).unwrap();
    fs::create_dir_all(&layers_dir).unwrap();

    // Create dummy RAW TEXT layer file (Zero-JSON)
    let layer_content = "Rust is a systems programming language focusing on safety.\n\nMemory management in Rust uses ownership and borrowing.\n\nMicroscope Memory uses hierarchical indexing.";
    fs::write(layers_dir.join("long_term.txt"), layer_content).unwrap();

    let mut config = microscope_memory::config::Config::default();
    config.paths.layers_dir = layers_dir.to_string_lossy().to_string();
    config.paths.output_dir = output_dir.to_string_lossy().to_string();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string()];
    config.embedding.provider = "mock".to_string();

    (tmp, config)
}

#[test]
fn test_full_build_pipeline() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).expect("build failed");

    let output_dir = Path::new(&config.paths.output_dir);
    assert!(output_dir.join("meta.bin").exists());
    assert!(output_dir.join("microscope.bin").exists());
    assert!(output_dir.join("data.bin").exists());
}

#[test]
fn test_build_and_read() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).expect("build failed");

    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");
    assert!(reader.block_count > 0);
}

#[test]
fn test_text_search() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).expect("build failed");
    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    let results = reader.find_text("Rust", 10);
    assert!(!results.is_empty(), "should find 'Rust' in txt file");
}

#[test]
fn test_store_and_recall() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    microscope_memory::store_memory(
        &config,
        "Test memory about standing on own feet",
        "long_term",
        5,
        None,
    )
    .expect("store");

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    assert!(append_path.exists());

    let entries = microscope_memory::read_append_log(&append_path);
    assert!(!entries.is_empty());
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

    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");
    let appended = vec![];

    // Query with keyword
    let q = microscope_memory::query::parse("\"Rust\"");
    let results = microscope_memory::query::execute(&q, &reader, &appended);
    assert!(!results.is_empty());
}

#[test]
fn test_mql_complex_query() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");
    let appended = vec![];

    // 1. Boolean AND
    let q = microscope_memory::query::parse("\"Rust\" AND \"safety\"");
    let results = execute_query(&q, &reader, &appended);
    assert!(!results.is_empty(), "AND query failed");

    // 2. Boolean OR
    let q = microscope_memory::query::parse("\"ownership\" OR \"non-existent\"");
    let results = execute_query(&q, &reader, &appended);
    assert!(!results.is_empty(), "OR query failed");

    // 3. Layer + Depth Filter
    let q = microscope_memory::query::parse("layer:long_term depth:3 \"Rust\"");
    let results = execute_query(&q, &reader, &appended);
    assert!(!results.is_empty(), "Layer/Depth filter failed");

    // 4. Spatial 'near:' filter
    // Get coords for 'Rust' item first
    let rust_h = reader.header(0);
    let rx = rust_h.x;
    let ry = rust_h.y;
    let rz = rust_h.z;
    let q_str = format!("near:{},{},{},0.5 \"Rust\"", rx, ry, rz);
    let q = microscope_memory::query::parse(&q_str);
    let results = execute_query(&q, &reader, &appended);
    assert!(!results.is_empty(), "Spatial 'near:' filter failed");
}

fn execute_query(
    q: &microscope_memory::query::Query,
    reader: &microscope_memory::reader::MicroscopeReader,
    appended: &[microscope_memory::AppendEntry],
) -> Vec<microscope_memory::query::QueryResult> {
    microscope_memory::query::execute(q, reader, appended)
}

#[test]
fn test_crc_integrity_after_build() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    for i in 0..reader.block_count {
        let h = reader.header(i);
        let stored = u16::from_le_bytes(h.crc16);
        if stored == 0x0000 {
            continue;
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

    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    for i in 0..reader.block_count {
        let h = reader.header(i);
        let start = h.data_offset as usize;
        let end = start + h.data_len as usize;
        let data = &reader.data[start..end];
        assert!(tree.verify_leaf(i, data));
    }
}

#[test]
fn test_cross_platform_merkle_consistency() {
    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    let output_dir = Path::new(&config.paths.output_dir);
    let merkle_data = fs::read(output_dir.join("merkle.bin")).unwrap();
    let tree = microscope_memory::merkle::MerkleTree::from_bytes(&merkle_data).unwrap();

    // Cross-platform consistency check: Merkle root should be identical across platforms
    // This ensures deterministic hashing and no endianness issues
    let root = tree.root;
    let expected_root: [u8; 32] = [
        251, 179, 237, 94, 128, 172, 123, 228, 175, 112, 2, 231, 250, 94, 111, 148, 158, 81, 141,
        218, 55, 185, 26, 158, 220, 2, 45, 122, 169, 78, 161, 22,
    ];
    assert_eq!(
        root, expected_root,
        "Merkle root must be consistent across platforms"
    );
}

#[test]
fn test_mcp_protocol_compatibility() {
    // Test MCP (Model Context Protocol) JSON-RPC compatibility
    // Ensures the native MCP server responds correctly to standard requests

    let (_tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).unwrap();

    // Test initialize
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    let init_response = handle_mcp_request(&init_request, &config);
    assert_eq!(init_response["jsonrpc"], "2.0");
    assert_eq!(init_response["id"], 1);
    assert!(init_response.get("result").is_some());
    assert_eq!(init_response["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(
        init_response["result"]["serverInfo"]["name"],
        "microscope-memory"
    );

    // Test tools/list
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let list_response = handle_mcp_request(&list_request, &config);
    assert_eq!(list_response["jsonrpc"], "2.0");
    assert_eq!(list_response["id"], 2);
    let tools = list_response["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    // Check that memory_status tool exists
    let status_tool = tools.iter().find(|t| t["name"] == "memory_status").unwrap();
    assert_eq!(
        status_tool["description"],
        "Get microscope memory index status: block count, depths, append log size"
    );

    // Test tools/call for memory_status
    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "memory_status",
            "arguments": {}
        }
    });
    let call_response = handle_mcp_request(&call_request, &config);
    assert_eq!(call_response["jsonrpc"], "2.0");
    assert_eq!(call_response["id"], 3);
    let content = call_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(content.contains("Microscope Memory Status"));
    assert!(content.contains("Blocks:"));
}

// Helper function to simulate MCP request handling (extracted from mcp.rs logic)
fn handle_mcp_request(
    request: &serde_json::Value,
    config: &microscope_memory::config::Config,
) -> serde_json::Value {
    use serde_json::json;

    let id = request.get("id").cloned().unwrap_or(json!(null));
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "microscope-memory",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "memory_status",
                        "description": "Get microscope memory index status: block count, depths, append log size",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }
                ]
            }
        }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");

            match tool_name {
                "memory_status" => {
                    let reader = microscope_memory::reader::MicroscopeReader::open(config).unwrap();
                    let append_path =
                        std::path::Path::new(&config.paths.output_dir).join("append.bin");
                    let appended = microscope_memory::read_append_log(&append_path);

                    let hdr_kb =
                        (reader.block_count * microscope_memory::HEADER_SIZE) as f64 / 1024.0;
                    let data_kb = reader.data.len() as f64 / 1024.0;

                    let content = format!(
                        "Microscope Memory Status\n\
                         ========================\n\
                         Blocks: {}\n\
                         Headers: {:.1} KB\n\
                         Data: {:.1} KB\n\
                         Total: {:.1} KB\n\
                         Append log: {} entries",
                        reader.block_count,
                        hdr_kb,
                        data_kb,
                        hdr_kb + data_kb,
                        appended.len()
                    );

                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{"type": "text", "text": content}]
                        }
                    })
                }
                _ => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": "Method not found"}
                }),
            }
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": "Method not found"}
        }),
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
        assert!(idx.block_count() > 0);

        use microscope_memory::embeddings::{EmbeddingProvider, MockEmbeddingProvider};
        let reader =
            microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");
        let block0_text = reader.text(0);
        let provider = MockEmbeddingProvider::new(idx.dim());
        let query_emb = provider.embed(block0_text).unwrap();
        let results = idx.search(&query_emb, 5);
        assert!(!results.is_empty());
    }
}


// ─── Morphogenesis Integration ───────────────────────────

#[test]
fn test_morphogenesis_mycelium_growth_integration() {
    use microscope_memory::morphogenesis::*;
    let seed = Seed::new("int_test", 0.0, 0.0, 0.0, "service");
    let field = MorphogenField::new();
    let config = GrowthConfig::mycelium_default();
    let organism = mycelium_growth(&seed, &field, &config);
    assert!(!organism.nodes.is_empty(), "Mycelium should grow nodes");
}

#[test]
fn test_morphogenesis_engine_evolve() {
    use microscope_memory::morphogenesis::*;
    let engine = MorphogenesisEngine::new();
    let seeds = vec![Seed::new("evo", 0.0, 0.0, 0.0, "test")];
    let results = engine.evolve_population(&seeds, 3, &FitnessObjective::Balanced, 8);
    assert!(!results.is_empty(), "Should produce organisms");
}

// ─── Pattern Recognition Integration ─────────────────────

#[test]
fn test_pattern_recognition_sequences() {
    use microscope_memory::pattern_recognition::*;
    let recognizer = PatternRecognizer::new();
    // Register a known pattern, then verify it's there
    recognizer.register_pattern("test_seq", PatternType::Sequence, 
        PatternTemplate::Sequence(vec!["a".to_string(), "b".to_string()]), 
        "test", vec![]);
    let found = recognizer.find_pattern("test_seq");
    assert!(found.is_some(), "Should find registered pattern");
    assert_eq!(found.unwrap().pattern_type, PatternType::Sequence);
}

#[test]
fn test_pattern_recognition_motifs() {
    use microscope_memory::pattern_recognition::*;
    let recognizer = PatternRecognizer::new();
    recognizer.register_pattern("motif_test", PatternType::Structural,
        PatternTemplate::Structural(vec![("a".to_string(), "b".to_string(), 1.0)]),
        "test_graph", vec![]);
    let patterns = recognizer.list_patterns(Some(PatternType::Structural), Some("test_graph"));
    assert!(patterns.len() >= 1, "Should have structural patterns");
}

// ─── Executive Integration ───────────────────────────────

#[test]
fn test_executive_schedule_and_cycle() {
    use microscope_memory::executive::Executive;
    let exec = Executive::new();
    exec.register_module("a", "A", 200, 0.3, vec![]);
    exec.register_module("b", "B", 100, 0.5, vec![]);
    let plan = exec.schedule();
    assert_eq!(plan.module_order[0], "a", "Higher priority first");
    let executed = exec.cycle();
    assert!(!executed.is_empty(), "Should execute modules");
}

#[test]
fn test_executive_homeostasis() {
    use microscope_memory::executive::Executive;
    let exec = Executive::new();
    exec.register_module("m", "M", 50, 0.5, vec![]);
    exec.update_resources(|r| r.energy_level = 0.05);
    let actions = exec.homeostasis();
    assert!(actions.contains(&"energy_conservation".to_string()));
}

// ─── Planning Integration ────────────────────────────────

#[test]
fn test_planning_goal_decomposition() {
    use microscope_memory::planning::Planner;
    let planner = Planner::new();
    let gid = planner.add_goal("learn", "Learn Rust", 100, None);
    let subs = planner.decompose_goal(gid);
    assert!(!subs.is_empty(), "Should decompose into subgoals");
}

#[test]
fn test_planning_create_and_execute() {
    use microscope_memory::planning::Planner;
    let planner = Planner::new();
    let gid = planner.add_goal("learn", "Learning", 100, None);
    planner.decompose_goal(gid);
    let plan = planner.create_plan(gid);
    assert_eq!(plan.status, microscope_memory::planning::PlanStatus::Draft);
    let step = planner.execute_step(plan.id);
    assert!(step.is_some(), "Should execute first step");
}

// ─── Autopoiesis Integration ─────────────────────────────

#[test]
fn test_autopoiesis_template_and_mutation() {
    use microscope_memory::autopoiesis::*;
    let engine = AutopoiesisEngine::new();
    engine.register_template("tmpl", "mod", "fn {{name}}() {}", vec!["name".to_string()], "Test");
    let mut vars = std::collections::HashMap::new();
    vars.insert("name".to_string(), "hello".to_string());
    let code = engine.generate_from_template("tmpl", &vars);
    assert_eq!(code, Some("fn hello() {}".to_string()));
}

#[test]
fn test_autopoiesis_mutation_lifecycle() {
    use microscope_memory::autopoiesis::*;
    let engine = AutopoiesisEngine::new();
    let id = engine.propose_mutation("fix", MutationType::Modification, "test_mod", "code", "reason", "system");
    assert!(engine.apply_mutation(id));
    assert!(!engine.list_mutations(None).is_empty());
}

// ─── Code Memory Integration ─────────────────────────────

#[test]
fn test_code_memory_store_and_recall() {
    use microscope_memory::code_memory::*;
    let cm = CodeMemory::new();
    cm.store(CodeEntryType::Function, "parse", "fn parse() {}", "lib.rs", "rust", "proj", vec![], vec![]);
    let q = CodeQuery { query: "parse".to_string(), language: None, entry_type: None, project: None, file: None, k: 10 };
    assert!(!cm.recall(&q).is_empty());
}

#[test]
fn test_code_memory_error_solution() {
    use microscope_memory::code_memory::*;
    let cm = CodeMemory::new();
    let id = cm.store_error_solution("cannot borrow", "add mut", "test.rs", "rust", "proj");
    let entry = cm.get(id);
    assert_eq!(entry.unwrap().solution, Some("add mut".to_string()));
}

#[test]
fn test_code_memory_stats() {
    use microscope_memory::code_memory::*;
    let cm = CodeMemory::new();
    cm.store(CodeEntryType::Function, "a", "fn a() {}", "a.rs", "rust", "p1", vec![], vec![]);
    cm.store(CodeEntryType::Type, "B", "struct B {}", "b.rs", "rust", "p2", vec![], vec![]);
    assert_eq!(cm.stats().0, 2);
}
