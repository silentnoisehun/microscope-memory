# AGENTS.md - Microscope Memory Agent Guidelines

This document provides comprehensive guidelines for agentic coding assistants working on the Microscope Memory codebase. Follow these conventions to maintain code quality, consistency, and the project's unique architecture.

## Build/Lint/Test Commands

### Core Build Commands
```bash
# Standard release build
cargo build --release

# Development build
cargo build

# Build with specific features
cargo build --release --features embeddings
cargo build --release --features "gpu compression"
```

### Testing Commands
```bash
# Run all tests
cargo test

# Run tests with verbose output
cargo test --verbose

# Run a specific test
cargo test test_full_build_pipeline
cargo test test_text_search
cargo test -- --test test_build_and_read

# Run integration tests only
cargo test --test integration

# Run tests with specific features
cargo test --features compression

# Run tests in release mode (faster)
cargo test --release
```

### Lint and Format Commands
```bash
# Check formatting
cargo fmt --all -- --check

# Apply formatting
cargo fmt --all

# Run Clippy linter
cargo clippy --all-targets -- -D warnings

# Run Clippy with features
cargo clippy --all-targets --features compression -- -D warnings
```

### Benchmark Commands
```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench microscope_bench

# GPU vs CPU benchmark (requires gpu feature)
cargo bench --bench microscope_bench --features gpu
```

### Additional Commands
```bash
# Check documentation
cargo doc --open

# Generate documentation without opening
cargo doc

# Clean build artifacts
cargo clean

# Update dependencies
cargo update
```

## Code Style Guidelines

### General Principles
- **Follow Rust 2021 edition standards** and idiomatic patterns
- **Keep functions small and focused** - aim for single responsibility
- **Use meaningful variable names** - avoid abbreviations except in well-established contexts
- **Document public APIs** with comprehensive doc comments
- **Add comments for complex logic** and non-obvious algorithms
- **Prefer explicit over implicit** - be clear about intent
- **Zero unsafe code** unless absolutely necessary for performance

### Naming Conventions
```rust
// Structs and enums: PascalCase
pub struct MicroscopeReader { ... }
pub enum ConsciousnessLayer { ... }

// Functions and methods: snake_case
pub fn build_index(config: &Config) -> Result<(), Error> { ... }
pub fn calculate_distance(a: &[f32], b: &[f32]) -> f32 { ... }

// Variables: snake_case, descriptive names
let block_coordinates = calculate_coords(text, layer);
let activation_count = reader.activation_count(block_idx);

// Constants: SCREAMING_SNAKE_CASE
pub const BLOCK_DATA_SIZE: usize = 256;
pub const HEADER_SIZE: usize = 32;
```

### Module Organization
- **Group related functionality** into modules
- **Use clear module names** that reflect their purpose
- **Keep module files focused** - prefer splitting large modules
- **Re-export commonly used items** in lib.rs

```rust
// lib.rs - Good module organization
pub mod reader;      // Core reading functionality
pub mod build;       // Index construction
pub mod query;       // Query parsing and execution
pub mod embeddings;  // Embedding providers
pub mod hebbian;     // Hebbian learning layer
pub mod mirror;      // Mirror neuron layer
// ... etc
```

### Import Style
```rust
// Group imports by source
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

// External crates (alphabetized)
use clap::Parser;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// Local modules (group related)
use crate::{
    config::Config,
    reader::{BlockHeader, MicroscopeReader},
    query::Query,
};
```

### Error Handling
```rust
// Use Result<T, E> for fallible operations
pub fn process_data(input: &str) -> Result<ProcessedData, Error> {
    // Implementation
    Ok(processed)
}

// Use custom error types with thiserror
#[derive(thiserror::Error, Debug)]
pub enum MemoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid block header at index {index}")]
    InvalidHeader { index: usize },

    #[error("Query parsing failed: {reason}")]
    QueryParse { reason: String },
}
```

### Memory Management
- **Use memory mapping** for large files (mmap2 crate)
- **Minimize allocations** in hot paths
- **Use stack allocation** for small, fixed-size data
- **Consider zero-copy** operations where possible

```rust
// Good: memory-mapped access
pub struct DataStore {
    mmap: memmap2::Mmap,
    data_offset: usize,
}

impl DataStore {
    pub fn get_block(&self, offset: usize, len: usize) -> &[u8] {
        &self.mmap[offset..offset + len]
    }
}
```

### Performance Considerations
- **Use SIMD** for vector operations (target_arch = "x86_64")
- **Leverage parallelism** with rayon for CPU-intensive tasks
- **Profile before optimizing** - use cargo bench to measure
- **Consider memory layout** for cache efficiency

```rust
// SIMD-accelerated L2 distance calculation
#[cfg(target_arch = "x86_64")]
pub fn l2_distance_simd(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::x86_64::*;
    // SIMD implementation
}

// Parallel processing with rayon
pub fn process_blocks_parallel(blocks: &[Block]) -> Vec<ProcessedBlock> {
    blocks.par_iter()
        .map(|block| process_single_block(block))
        .collect()
}
```

### Testing Patterns
```rust
// Unit tests alongside implementation
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_distance() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let distance = calculate_distance(&a, &b);
        assert!((distance - 5.196152).abs() < 0.001);
    }

    #[test]
    fn test_edge_cases() {
        // Test empty inputs, boundary conditions, etc.
    }
}

// Integration tests in tests/ directory
#[test]
fn test_full_pipeline() {
    let config = setup_test_config();
    build::build(&config, true).unwrap();

    let reader = MicroscopeReader::open(&config).unwrap();
    assert!(reader.block_count > 0);
}
```

### Documentation Style
```rust
/// Processes a block of memory through the consciousness layers.
///
/// This function applies the complete recall pipeline:
/// 1. Attention weighting
/// 2. Predictive cache check
/// 3. Emotional warp application
/// 4. Spatial search execution
///
/// # Arguments
/// * `query` - The search query string
/// * `reader` - Reference to the memory index
/// * `config` - System configuration
///
/// # Returns
/// A ranked list of memory blocks matching the query
///
/// # Examples
/// ```
/// let results = recall("What is Rust?", &reader, &config);
/// assert!(!results.is_empty());
/// ```
pub fn recall(query: &str, reader: &MicroscopeReader, config: &Config) -> Vec<MemoryResult> {
    // Implementation
}
```

### Feature Flags
- **Use Cargo features** for optional functionality
- **Document feature requirements** in code comments
- **Test with and without features** in CI

```rust
// Conditional compilation based on features
#[cfg(feature = "gpu")]
pub mod gpu_accelerator {
    // GPU-specific code
}

#[cfg(feature = "embeddings")]
pub fn embed_text(text: &str) -> Result<Vec<f32>, Error> {
    // Embedding implementation
}
```

## Project-Specific Patterns

### Consciousness Layer Architecture
The system implements 13 consciousness layers as separate modules:
- **hebbian.rs** - Learning through use (blocks strengthen on access)
- **mirror.rs** - Activation pattern similarity detection
- **resonance.rs** - Spatial field interference patterns
- **archetype.rs** - Crystallized activation patterns
- **emotional.rs** - Affective bias in search space
- **thought_graph.rs** - Sequential recall path tracking
- **predictive_cache.rs** - Pre-fetch based on patterns
- **temporal_archetype.rs** - Time-windowed activation profiles
- **attention.rs** - Dynamic layer weighting
- **dream.rs** - Offline memory consolidation
- **emotional_contagion.rs** - Federated emotional state sharing
- **multimodal.rs** - Cross-modal memory integration

### Memory Block Structure
```rust
// Fixed 32-byte header format
#[repr(C)]
pub struct BlockHeader {
    pub x: f32,           // 3D spatial coordinate
    pub y: f32,
    pub z: f32,
    pub zoom: f32,        // Normalized depth (depth/8.0)
    pub depth: u8,        // Hierarchical level (0-8)
    pub layer_id: u8,     // Memory layer index
    pub data_offset: u32, // Byte offset in data.bin
    pub data_len: u16,    // Actual text length (≤ 256)
    pub parent_idx: u32,  // Parent block index
    pub child_count: u16, // Number of child blocks
    pub crc16: [u8; 2],   // Integrity check
}
```

### Hierarchical Decomposition
Memory is decomposed into 9 depth levels:
- **D0**: Identity (1 block - system summary)
- **D1**: Layer summaries (9 blocks)
- **D2**: Topic clusters
- **D3**: Individual memories
- **D4**: Sentences
- **D5**: Tokens/words
- **D6**: Syllables
- **D7**: Characters
- **D8**: Raw bytes (atomic boundary)

### Configuration Management
```rust
// TOML-based configuration
#[derive(Deserialize)]
pub struct Config {
    pub paths: Paths,
    pub index: Index,
    pub search: Search,
    pub memory_layers: MemoryLayers,
    pub embedding: Embedding,
    pub server: Server,
    pub performance: Performance,
}
```

## Development Workflow

### Before Committing
1. **Run tests**: `cargo test`
2. **Check formatting**: `cargo fmt --all -- --check`
3. **Run linter**: `cargo clippy --all-targets -- -D warnings`
4. **Verify build**: `cargo build --release`

### CI Requirements
- **Tests pass** on Ubuntu, Windows, macOS
- **Clippy clean** with no warnings
- **Formatted code** (rustfmt)
- **No unsafe code** without justification

### Performance Expectations
- **Sub-microsecond queries** (37ns - 500μs depending on depth)
- **Memory efficient** - mmap-based access
- **SIMD acceleration** on x86_64
- **Parallel processing** with rayon

## Common Pitfalls to Avoid

### Memory Safety
- Avoid unsafe code unless performance-critical
- Validate all input data bounds
- Use checked arithmetic for offsets and indices

### Performance Issues
- Don't allocate in hot loops
- Prefer stack over heap for small data
- Consider cache locality in data structures

### API Design
- Keep APIs simple and composable
- Provide both high-level and low-level interfaces
- Document all assumptions and limitations

### Testing
- Test edge cases and error conditions
- Use realistic test data
- Verify both success and failure paths

This document should be updated as the codebase evolves. Always refer to recent commits and discussions for the latest patterns and conventions.