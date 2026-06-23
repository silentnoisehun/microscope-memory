# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.1] - 2026-06-23

### Fixed
- **Crash safety**: 50 files — all `fs::write()` calls replaced with temp file + atomic rename pattern
- **NaN propagation**: emotional_21d, attention, and 3 other files — NaN/inf sanitization on load
- **Data loss**: emotion vector now persisted in `store_memory_with_emotion`
- **Planning**: added `fail_step()` rollback method
- **Memory leak**: mental_sandbox capped at `MAX_SCENARIOS=100`
- **Redundant I/O**: removed duplicate `HebbianState::load_or_init` in hot recall path

### Changed
- **Hook script**: `scripts/microscope-recall-hook.ps1` — now uses `-Action` parameter instead of `$env:CLAUDE_HOOK_TYPE`; UserPromptSubmit does store + recall + inject; Stop stores to long_term
- **README**: corrected layer list (15 layers, deduplicated), module count (84), LOC (36,317)
- **Version**: bumped to 0.8.1

## [0.8.0] - 2026-06-17

### Added
- **morphogenesis.rs** — 4 biological growth algorithms (mycelium, capillary, slime mold, fractal L-system) + evolutionary engine
- **pattern_recognition.rs** — sequence, temporal, structural & cluster pattern detection with cross-domain correlation
- **executive.rs** — cognitive conductor: module scheduling, resource allocation, homeostasis
- **planning.rs** — HTN goal decomposition, action planning, replanning on changing conditions
- **autopoiesis.rs** — template-based code generation, versioned mutations, rollback mechanism
- **code_memory.rs** — dedicated memory layer for coding agents (code snippets, symbols, error↔solution pairs)
- **chatgpt.rs** — ChatGPT export parser with Google Drive import support (--gdrive, --gdrive-folder)
- **PWA chat** — Progressive Web App with manifest, service worker, installable on mobile
- **MCP integration** — Model Context Protocol server for Claude Code, Cline, Kilo Code, OpenCode
- **25 integration examples** — LangChain, OpenAI Assistant, Ollama RAG, Discord, Slack, WhatsApp, n8n, Docker, Home Assistant, Streamlit, Obsidian, AutoGPT, Cloudflare Worker

### Changed
- CLI: added `morph`, `code`, `import-chat-gpt` commands
- Serve: binds to 0.0.0.0, serves PWA on /chat.html, displays local IP for phone access
- Scripts: updated binary name, ports, and removed obsolete TTS references
- Layers: added missing layer files (identity, emotional, relational, reflections, crypto_chain, rust_state, code)
- README: full rewrite with v0.8.0 features and Vector DB comparison benchmarks
- BENCHMARKS.md: added comparison table (FAISS, Pinecone, ChromaDB, Qdrant)

### Removed
- demo.html (replaced by PWA chat)
- website/ directory (landing page)
- server-data/ (obsolete server duplicate, 606MB)
- backup/ directory
- tools/edge_tts_server.py (obsolete, replaced by voice-mcp)
- examples/index.html

### Performance
- Overall query: 87 µs avg across 9 depths (20323 blocks)
- 4D soft zoom: 249 µs/query
- 265 tests, all passing

## [0.7.0] - 2026-04-08

### Added
- Comprehensive MQL integration tests
- Python API documentation
- WASM browser integration documentation

### Changed
- Refined CLI command descriptions
- Improved config.example.toml comments

## [0.1.0] - 2026-03-21

### Added
- Initial release: 9-level hierarchical depth system (D0-D8)
- Pure binary storage with mmap
- Sub-microsecond query performance
- 3D spatial indexing with L2 distance search
- Natural language recall with auto-zoom
- 9 cognitive memory layers
- GitHub Actions CI/CD pipeline
