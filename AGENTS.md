# AGENTS.md - Microscope Memory Agent Guidelines

Build and code style guidelines for the Microscope Memory codebase.

## Build / Test / Lint

```bash
# Release build
cargo build --release

# All tests
cargo test
cargo test --test integration   # Integration tests only
cargo test --lib                # Library tests only

# Benchmarks
cargo bench

# Lint
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# With features
cargo build --release --features "gpu embeddings"
```

## Code Style

- Rust 2021 edition, idiomatic patterns
- `PascalCase` for structs/enums, `snake_case` for functions/variables
- `SCREAMING_SNAKE_CASE` for constants
- `Result<T, E>` with `thiserror` for error handling
- `Arc<RwLock<T>>` for shared state
- `HashMap` for collections, `f64` for metrics

## Project Architecture (v0.8.1)

### Cognitive Modules
| Module | Description |
|--------|-------------|
| `morphogenesis.rs` | Biological growth algorithms (mycelium, capillary, slime, fractal L-system) |
| `pattern_recognition.rs` | Sequence, temporal, structural, cluster, cross-domain patterns |
| `executive.rs` | Cognitive conductor — scheduling, homeostasis, resource allocation |
| `planning.rs` | HTN goal decomposition, action plans, replanning |
| `autopoiesis.rs` | Template-based code generation, versioned mutations, rollback |
| `code_memory.rs` | Code-specific memory for coding agents (symbols, errors, project structure) |
| `chatgpt.rs` | ChatGPT export parser and import |

### Memory Stack
- **13 layers**: identity, long_term, short_term, associative, emotional, relational, reflections, crypto_chain, echo_cache, rust_state, code, session
- **9 depths** (D0-D8): hierarchical zoom-based indexing
- **Binary mmap**: zero-JSON hot path, sub-microsecond recall

## Before Committing
1. `cargo test`
2. `cargo fmt --all -- --check`
3. `cargo test --test integration`
4. `cargo build --release`
