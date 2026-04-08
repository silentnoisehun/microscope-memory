# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive MQL integration tests (Boolean AND/OR, Spatial `near:`, Layer/Depth filters)
- Python API documentation in README
- WASM browser integration documentation in README
- Fixed 3D coordinate distribution for emotional memory clusters

### Changed
- Refined CLI command descriptions for `build` and `rebuild`
- Improved `config.example.toml` comments on semantic search and embedding depth

## [0.1.0] - 2026-03-21

### Added
- Initial release of Microscope Memory
- 9-level hierarchical depth system (D0-D8)
- Pure binary storage with mmap support
- Sub-microsecond query performance
- 3D spatial indexing with L2 distance search
- Natural language recall with auto-zoom
- Hybrid search combining vector distance and keyword matching
- Support for 9 cognitive memory layers
- Python alternative implementation with NumPy
- Comprehensive benchmark suite
- Fixed 256-character viewport blocks
- Append log for incremental updates
- Store and recall CLI commands
- GitHub Actions CI/CD pipeline

### Technical Details
- Zero JSON, pure binary format
- Memory-mapped I/O for zero-copy access
- Deterministic content-based positioning
- Cache-optimized data structures (L1d/L2 for shallow depths)
- Rust implementation with safety guarantees
- Cross-platform support (Linux, Windows, macOS)

### Performance
- D0-D1: 37-92 nanoseconds per query
- D2-D4: 0.5-4 microseconds per query
- D5-D6: 18-72 microseconds per query
- D7-D8: ~500 microseconds per query
- Build time: ~2 seconds for 500+ memories
- Total index size: ~8 MB for 227,168 blocks

[0.1.0]: https://github.com/silentnoisehun/microscope-memory/releases/tag/v0.1.0