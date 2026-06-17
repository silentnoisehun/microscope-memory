# Contributing to Microscope Memory

Thank you for your interest in contributing!

## How to Contribute

### Reporting Issues
- Check if the issue already exists
- Include OS and Rust version (`rustc --version`)
- Provide minimal reproduction steps

### Submitting Pull Requests
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/name`)
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass: `cargo test && cargo test --test integration`
6. Run clippy: `cargo clippy -- -D warnings`
7. Format: `cargo fmt`
8. Commit with descriptive messages, push, open a PR

### Code Style
- Rust 2021 edition, idiomatic patterns
- `PascalCase` for types, `snake_case` for functions
- Document public APIs with doc comments
- `Arc<RwLock<T>>` for shared state
- `Result<T, E>` for fallible operations

### Testing
- Unit tests in each module's `#[cfg(test)] mod tests {}`
- Integration tests in `tests/integration.rs`
- New modules must have both unit and integration tests

### Areas of Interest (v0.8.0)
- **Growth algorithms**: new biological patterns for morphogenesis
- **Pattern detection**: sequence mining, graph motif discovery
- **Code memory**: deeper IDE integration, symbol graph
- **Autopoiesis**: WASM hot-swap, self-compilation loop
- **ChatGPT import**: improved emotional context extraction
- **PWA**: better mobile UX, push notifications
- **MCP tools**: more coding agent integrations
- **Performance**: SIMD, parallel growth, GPU morphogenesis

## Development Setup
```bash
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory
cargo build
cargo test
```
