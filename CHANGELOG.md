# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Shared hybrid lexical-spatial relevance ranker** (`relevance.rs`): query-token
  coverage, phrase evidence, spatial distance and a small importance prior replace
  the legacy clamped keyword-distance formula. Deterministic regression gate:
  Recall@3 = 1.000, MRR = 1.000 on the fixture corpus (`tests/relevance_benchmark.rs`).
- **`stats` data-footprint report**: per-file sizes of the binary index
  (`embeddings.bin`, `merkle.bin`, `links.bin`, ...), the stale `index-history/`
  snapshots, `layers/*.txt` total and the configured retention bound, so
  unbounded growth is visible from one command.

### Changed
- **Size-bounded memory eviction**: `max_blocks` (0 = unbounded) and
  `protect_min_importance` (default 8) under `[index]`. When the index exceeds
  the cap, the dream cycle evicts the lowest-scoring blocks — score =
  importance × 10 + recall energy − age penalty — and rewrites `embeddings.bin`
  in sync. Blocks with importance >= `protect_min_importance` are never evicted.
- **Durable importance**: layer entries now carry a leading `(imp=N)` marker and
  the build reads it instead of flattening every layer block to importance 5, so
  rebuilds no longer erase stored importance. The dream cycle automatically
  promotes frequently recalled blocks (`promote_energy_threshold`, default
  0.35): energy >= threshold bumps importance by one, capped at the protection
  floor, mirrored back into the layer source.

### Changed
- **Bounded layer retention**: `layer_retention_entries` default is now 2000
  (was 0 / unlimited). Each layer file keeps at most 2000 newest entries and
  trims the oldest past that cap, so `layers/*.txt` cannot grow without bound.

## [0.8.2] - 2026-07-17

### Added
- **auto-context CORE MEMORIES section**: `auto_context.rs` now embeds full text (up to 500 chars) of the top 3 highest-importance memories (imp >= 7) directly into every auto-context snapshot. LLMs receive actual memory content, not just metadata — eliminating hallucination on foundation memories.

### Changed
- **Recall truncation limit**: `blade-microscope-memory` recall output increased from 150 to 4000 chars per block, ensuring full context reaches the LLM via MCP/tool-server.

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
