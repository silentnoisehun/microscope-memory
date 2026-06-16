# AGENTS.md - Microscope Memory Agent Guidelines

Build and code style guidelines for the Microscope Memory codebase. Follow these conventions to maintain code quality and consistency.

## Build/Lint/Test Commands

### Build Commands
```bash
# Release build
cargo build --release

# Dev build
cargo build

# With features
cargo build --release --features embeddings
cargo build --release --features "gpu compression"
```

### Test Commands
```bash
# All tests
cargo test

# Verbose output
cargo test --verbose

# Single test (exact name)
cargo test test_full_build_pipeline

# Run integration tests only
cargo test --test integration

# With features
cargo test --features compression

# Release mode (faster)
cargo test --release
```

### Lint and Format
```bash
# Check formatting
cargo fmt --all -- --check

# Apply formatting
cargo fmt --all

# Clippy linter
cargo clippy --all-targets -- -D warnings
```

### Benchmarks
```bash
cargo bench --bench microscope_bench
cargo bench --bench microscope_bench --features gpu
```

## Code Style Guidelines

### General Principles
- Use **Rust 2021 edition** idiomatic patterns
- Keep functions small and focused (single responsibility)
- Use meaningful variable names; avoid abbreviations
- Document public APIs with doc comments
- Prefer explicit over implicit
- Avoid unsafe code unless performance-critical

### Naming Conventions
```rust
// Structs/enums: PascalCase
pub struct MicroscopeReader { }
pub enum ConsciousnessLayer { }

// Functions: snake_case
pub fn build_index(config: &Config) -> Result<(), Error> { }

// Variables: snake_case
let block_coordinates = calculate_coords(text, layer);

// Constants: SCREAMING_SNAKE_CASE
pub const BLOCK_DATA_SIZE: usize = 256;
```

### Import Style
```rust
// Group by source: std, external, local
use std::{fs, path::Path, sync::Arc};
use clap::Parser;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{config::Config, reader::MicroscopeReader};
```

### Error Handling
```rust
// Use Result<T, E> with thiserror
#[derive(thiserror::Error, Debug)]
pub enum MemoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid block header at {index}")]
    InvalidHeader { index: usize },
}
```

### Memory Management
- Use **mmap** for large files (memmap2 crate)
- Minimize allocations in hot paths
- Use stack for fixed-size data
- Prefer zero-copy operations

### Testing Patterns
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_distance() {
        assert!(result.is_ok());
    }
}
```

## Project Architecture

### Consciousness Layers (13 modules)
- **hebbian.rs** - Learning through use
- **mirror.rs** - Activation pattern similarity
- **resonance.rs** - Spatial field interference
- **emotional.rs**, **attention.rs**, **dream.rs**, etc.

### Memory Block Structure (32-byte header)
```rust
#[repr(C)]
pub struct BlockHeader {
    pub x: f32, pub y: f32, pub z: f32,
    pub zoom: f32, pub depth: u8, pub layer_id: u8,
    pub data_offset: u32, pub data_len: u16,
    pub parent_idx: u32, pub child_count: u16,
    pub crc16: [u8; 2],
}
```

### Depth Levels (D0-D8)
- D0: Identity, D1: Layer summaries, D2: Topic clusters
- D3: Memories, D4: Sentences, D5: Tokens
- D6: Syllables, D7: Characters, D8: Raw bytes

## Feature Flags
```rust
default = []
wasm = ["wasm-bindgen", "web-sys"]
python = ["pyo3"]
gpu = ["wgpu", "bytemuck", "pollster"]
embeddings = ["candle-core", "candle-nn"]
compression = ["zstd"]
```

## Before Committing
1. `cargo test`
2. `cargo fmt --all -- --check`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo build --release`

## Common Pitfalls
- Avoid allocations in hot loops
- Validate input bounds
- Use checked arithmetic for offsets
- Test edge cases and error conditions
