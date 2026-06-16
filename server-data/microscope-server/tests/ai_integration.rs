//! AI Integration Tests
//!
//! Tests the AI Adapter functionality for binary protocol communication.

use microscope_memory::ai_adapter::{AIAdapter, AICommand};
use microscope_memory::config::Config;

fn setup_test_memory() -> (tempfile::TempDir, Config) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output_dir = tmp.path().join("data");
    let layers_dir = tmp.path().join("layers");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::create_dir_all(&layers_dir).unwrap();

    // Create test layer file
    let layer_content = "Rust programming language for systems development.\n\nMemory management with ownership and borrowing.\n\nMicroscope Memory: hierarchical cognitive storage.";
    std::fs::write(layers_dir.join("long_term.txt"), layer_content).unwrap();

    let mut config = Config::default();
    config.paths.layers_dir = layers_dir.to_string_lossy().to_string();
    config.paths.output_dir = output_dir.to_string_lossy().to_string();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string()];
    config.embedding.provider = "mock".to_string();

    // Build the memory
    microscope_memory::build::build(&config, true).expect("Failed to build test memory");

    (tmp, config)
}

#[test]
fn test_ai_adapter_read_operations() {
    let (_tmp, config) = setup_test_memory();
    let mut adapter = AIAdapter::new(config).expect("Failed to create AI adapter");

    // Test reading block 0
    let cmd = AICommand::read(1, 0);
    let result = adapter.process_command(cmd);
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.op_code, 0); // Read response
    assert_eq!(response.block_id, 0);

    // The response should contain valid data
    assert!(response.payload.len() == 242); // Should be full payload size
}

#[test]
fn test_ai_adapter_write_operations() {
    let (_tmp, config) = setup_test_memory();
    let mut adapter = AIAdapter::new(config).expect("Failed to create AI adapter");

    // Test write command (currently returns error, but marks as dirty)
    let cmd = AICommand::write(1, 0, b"Test data");
    let result = adapter.process_command(cmd);
    assert!(result.is_err()); // Not implemented yet
    assert!(result.unwrap_err().contains("not yet implemented"));
}

#[test]
fn test_ai_adapter_learning_operations() {
    let (_tmp, config) = setup_test_memory();
    let mut adapter = AIAdapter::new(config).expect("Failed to create AI adapter");

    // Test learning command (currently returns error, but marks as dirty)
    let cmd = AICommand::learn(0, 0.1);
    let result = adapter.process_command(cmd);
    assert!(result.is_err()); // Not implemented yet
    assert!(result.unwrap_err().contains("not yet implemented"));
}

#[test]
fn test_ai_adapter_merkle_integrity() {
    let (_tmp, config) = setup_test_memory();
    let adapter = AIAdapter::new(config).expect("Failed to create AI adapter");

    // Test that we can get the current Merkle root
    let root = adapter.current_merkle_root();
    assert!(root.is_ok());
    let root_bytes = root.unwrap();
    assert_eq!(root_bytes.len(), 32); // SHA-256 is 32 bytes
}

#[test]
fn test_lazy_merkle_updates() {
    let (_tmp, config) = setup_test_memory();
    let mut adapter = AIAdapter::new(config).expect("Failed to create AI adapter");

    // Send multiple commands to trigger lazy updates
    for i in 0..150 {
        let cmd = AICommand::read(1, (i % 3) as u64);
        let _ = adapter.process_command(cmd);
        // Every 100 commands, Merkle update should be attempted
    }

    // Adapter should still function normally
    let cmd = AICommand::read(1, 0);
    let result = adapter.process_command(cmd);
    assert!(result.is_ok());
}
