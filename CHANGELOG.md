# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-10-13

### Added
- `Spine::begin_publish()` returns an exclusive `Publish` batch with typed
  read/write methods, `tick_stream`, and explicit commit (drop also commits).
  The consciousness stream publishes its scalar tick and all due aggregate
  headers in one window, retaining the existing ten-cycle aggregate interval.
- `Spine::read_store_snapshot()` validates all eleven layer headers and the
  superblock together. Prefixes are 128 bytes so the 108-byte Emotion header
  fits; compile-time bounds cover every encoded header. The summary uses this
  snapshot and reports failure instead of falling back to mixed reads.
- Whole-store tearing regressions cover shared-handle threads, independent
  handles and a child process, and require observed publish progress. Unit
  tests cover empty/read-only batches, drop/unwind, short mappings, byte-64+
  Emotion fields and interrupted counters. `bench_spine_publish` measures
  batching separately from end-to-end stream execution.

### Fixed
- Every standalone header write now participates in the seqlock. The mapping
  mutex is acquired before the publisher lock: `flock` is tied to the open
  file description, so a second thread on one handle must not acquire/unlock
  that descriptor while the first thread publishes.
- Republishing after an interrupted odd counter no longer exposes a transient
  even value. Windows' shared-writer-mark fallback no longer references the
  Unix-only `as_raw_fd` API; cross-handle exclusion remains Unix/advisory only.
- Batches do not promise rollback or durability: unwinding publishes the
  already-written subset, and crash recovery cannot restore the old state.

### Fixed
- **CI never ran the tearing regression.** `build-and-test` invokes `cargo test --lib`
  and two named integration targets, so `tests/spine_torn_read.rs` compiled
  (`Build all targets`) and was never executed. It has its own step now. Same
  shape as the `Mojibake` gate sitting behind `Formatting` and never getting a
  turn: a check nobody runs is decoration.
- **Two files were committed mojibaked and nobody noticed, because the gate that
  catches it could never run.** `examples/bench_recall.rs` (497 marker hits) and
  `tests/append_cache_liveness.rs` (32) carry UTF-8 read through CP1250 and
  re-saved, so `//! FuttatĂ„â€šĂ‹â€ˇs` instead of `//! Futtatás`. The <!-- mojibake-ok: a serult alakot szandekosan mutatjuk -->
  `standards` job runs `Mojibake` *after* `Formatting`, and formatting had been
  red on this tree long enough that the mojibake check never got a turn — the
  first commit to fix `cargo fmt` is also the first commit that fails on the
  real state of the text. Neither file has a clean revision in history: they were
  born damaged. Repaired by re-encoding through the CP1250 decode map, iterated
  per file until the marker set empties, and accepted only on the condition that
  the ASCII skeleton of the file is byte-identical before and after — so the
  change cannot touch code, string escapes or delimiters, only the accented
  text. `python3 scripts/check_mojibake.py --worktree` reports 360 files clean.

### Added
- `examples/bench_spine.rs` gained a **contended** section (n publishers × m readers over one `spine.bin`): throughput per configuration plus a tearing count judged from inside the page, with unpopulated pages counted separately. Measured: 2.6 M snapshots per sweep, three sweeps, zero torn reads. The single-threaded sections are now labelled as such, and the bench warns when `data/` is missing instead of silently benchmarking empty directories.

### Fixed
- **`HebbianHeader::decay_event_count` is no longer corrupted** (`src/consciousness_stream.rs`). `publish_spine` did `+= s.cycle` on every cycle, so the field grew as a sum of cycle values while nothing counted decay events — `HebbianState::apply_drift` decays energies without keeping a tally. The write is gone; the field and `ArchetypeHeader::decay_events` now say in their docs that they have no producer and no consumer, which is the honest state of a fixed-size page layout that cannot be shrunk without a version bump.
- **`UNSAFE.md` covers the spine publish window.** The new `unsafe`
  surface — `AtomicU64::from_ptr` over the mapping, the raw `flock(2)`
  declaration, and the `map_mut` of a resized file — is inventoried with what
  a reviewer has to check, and `seq_atomic` now asserts its length bound in
  release too, so a future caller that forgets its guard panics rather than
  reading outside the mapping. `UNSAFE.md`'s counts were re-derived and the
  drift against the previous pass is stated instead of hidden.
- **The tearing regression proves what it claims** (`tests/spine_torn_read.rs`). Its header said every test runs an unbracketed
  control read; `torn_read_two_writers` did not have one, and the other two
  stopped their publisher after a fixed number of writes, so a writer that
  finished before the readers turned the test green and empty. Publishers now
  run until the readers are done, the two-publisher test asserts publish
  progress (its half-page control measured 0/120 000 once a second reader
  shared the handle, so it was not assertable), and the header states what
  each of the four counters measures — `open_window` is a property of the
  read path, not of the writer.
- **Spine publish is now one coherent snapshot, and it is a seqlock**
  (`src/spine.rs`). The previous commit's message claimed "1 StreamPage write
  + 1 epoch publish" with "Release/Acquire" ordering; what the code did was
  take the mapping mutex twice — once for the stream page, once for a
  millisecond timestamp in the superblock that nothing read back — and call
  the second `drop()` a release barrier. Two handles or two processes each own
  a `Mutex<MmapMut>`, so the two locks exclude nothing and there is no
  happens-before between a writer in one process and a reader in another: a
  reader could pair a new stream header with an older record, and two ticks
  inside the same millisecond were indistinguishable.
  - `tick()` now takes `flock(LOCK_EX)` on a new `spine.publish` sidecar (the
    piece that serialises publishers across processes), then the mapping lock
    once, and brackets the stream page *and* the superblock payload in a single
    seqlock window: an odd `AtomicU64` at `SEQ_SLOT` (superblock byte 56)
    while writing, the next even value stored with `Release`.
  - `Superblock` gains a monotonic `tick_seq` (superblock format version 2);
    `epoch_ms` stays as a wall-clock field, not a sequence number.
  - `Spine::read_snapshot` is the matching read: copy under the counter,
    retry if it moved, so a torn pairing is not returned. `format_summary` and
    `examples/spine_live.rs` now read through it, and `Spine::open_ro` is a
    handle that cannot mark itself a writer or resize the file underneath one.
  - `write_slot` on the superblock slot is truncated to the payload so a page
    write can never reset the counter; a window left open by a killed writer
    is repairable via `Spine::recover_stalled_publish` (`read_snapshot` calls
    it when its retry budget is exhausted rather than staying wedged).
- **`tests/spine_torn_read.rs`**: torn-read regression for the three setups the
  old code failed — two handles in one process, two publishers, two processes.
  Each asserts zero mixed/torn/window-open reads *and* runs an unbracketed
  control read on the same file that is required to still show the mixing, so
  the suite cannot pass by quietly measuring nothing. Measured on Linux with
  the dev profile: bracketed reads 0 violations in all three setups; the
  control read reported the mixing.
- `cargo fmt --all -- --check` passes again (13 files were failing, 64 hunks in
  `src/spine.rs`), which was keeping `cargo fmt` red in CI.
- Removed the unused optional `winapi` dependency: nothing referenced it and no
  feature selected it (it never reached Cargo.lock, so no lockfile churn).


## [1.0.0] - 2026-10-13

First 1.x release. The headline is recall and startup latency, measured
in-process (see BENCHMARKS.md), with the index policy and the breaking API
change stated honestly below.

### Added
- **Spine (`src/spine.rs`)**: the consciousness stream's whole scalar layer
  state (cycle, surprise, curiosity, prediction, Hebbian totals plus ten
  per-layer headers) in a single `spine.bin` mmap, published by the 10 Hz
  background tick. Startup: 19.2 ms of per-layer `load_or_init` file reads
  becomes a 1.08 ms mmap open. New deps: `byteorder`, `parking_lot`.
- **Inverted text index wired into `find_text`/`find_text_ranked`**: the TIX1
  index (`text_index.rs`, writer/reader/parser already in-tree, `build` already
  emitting it) is now actually consulted. `MicroscopeReader::candidate_blocks`
  prefilters to the index's candidate set with an exact substring predicate
  still run on every candidate, so results are bit-identical to a full scan
  (asserted per-query in `examples/bench_recall_ab.rs`).
- **`refresh_append_log` + self-validating append cache**: `find_text_all_ranked`
  no longer re-reads and re-parses `append.bin` per query; the reader caches it
  at open and re-validates against the file's (len, mtime) on every call
  (~1 us metadata check vs ~230 us re-read), so a reader held across stores
  still sees new appends. Regression tests: `tests/append_cache_liveness.rs`.
- **`examples/index_text.rs`**: regenerates `text_index.bin` for an existing
  data dir without a full rebuild.
- **`examples/bench_recall_ab.rs`**: interleaved A/B benchmark (indexed vs
  full scan, same process, alternating rounds, warm-up discarded, results
  asserted equal).
- **`RecallContext` (`src/main.rs`)**: the reader plus all seventeen layer
  states opened once and reused across `recall()` calls, instead of ~20
  whole-file reads per call. `recall()` itself opens a fresh context per
  invocation (behaviour identical to before); the bench loop and future
  server paths hold one context. Saves are unchanged, so durability is
  unchanged. Steady-state recall: 2.3 ms → 0.3 ms (7x).
- **`recall_ctx` library module + MCP shared reader**: `RecallContext` moved
  to `src/recall_ctx.rs` with a generation-guarded `SharedContext`
  (reopens transparently when `meta.bin` changes, i.e. after any rebuild)
  and a process-global `with_shared()` accessor. The `memory_find` and
  `memory_recall` MCP tools now serve from the shared reader instead of
  reopening per call; mutable layer states stay per-call by design (their
  stream-vs-disk precedence and save schedule were not safe to guess —
  documented in the wiring).
- **`pub fn contains_ci` + `tests/contains_ci.rs`**: allocation-free
  case-insensitive search, kept as a tested utility. See Performance notes.

### Changed
- **BREAKING: `find_text_all_ranked` takes `&mut self`** (the cached append
  log may refresh). External callers need `let mut reader`.
- Constructor and unit-test fixture struct literals gain the new
  `append_log` / `append_log_stamp` fields.
- `Cargo.toml`: `byteorder 1.5`, `parking_lot 0.12` added as direct deps.

### Performance
- Startup (24 per-layer state files → 1 mmap): **17.8x** cold, **2.9x** warm.
  - *Labelled 2026-10-13:* this figure is `bench_spine`'s A-vs-B comparison, whose "before" side loads the layer states from a `data/` directory that is not in this repository, and whose "24 files" label counts the files the layer set opens rather than the eight the benchmark calls. It was not re-measured here (the crate cannot be built without crates.io access); the spine-side numbers that can be measured were: `Spine::open` 531 µs cold / 10.9 µs warm, `tick()` 666 ns, and `read_snapshot` 103 ns against the 805 ns two-read pattern it replaces. See the "Spine publish protocol" section of BENCHMARKS.md.
- Recall, in-process, 23,774 blocks, 19 interleaved rounds:
  **901 us → 93 us per query (9.7x)**, bit-identical results.
- The existing end-to-end finding (§"Inverted text index", 2026-09-27,
  695k blocks, ~91 ms both paths) stands: at that scale process start-up
  dominates, so the user-visible `find` is unchanged. The 9.7x is the
  in-process text path, where the loop-bound services (bridge, MCP) live.
- Attempted and **rejected**: replacing the per-block `to_lowercase()` with
  a hand-rolled allocation-free scan (`contains_ci`). Measured in-process at
  **5.4x slower** (913 ns vs 4.9 us per block-compare); std `str::contains`
  is a SIMD two-way scan and cannot be beaten by a byte loop. Documented in
  the function's doc comment and its test file so nobody re-attempts it.

### Notes for operators
- After a **light rebuild** the text index is **regenerated** from the final
  layout at the end of `build()` (cheap, content-derived), so the indexed
  recall path survives rebuilds. The stale mid-build file is still dropped
  first and never served — `test_light_rebuild_regenerates_text_index` pins
  that the regenerated file matches the new block count and returns results
  identical to a full scan.
- `cargo test --release` is unsupported by this crate's own profile
  (`[profile.release]` sets `panic = "abort"`; tests need `unwind`). The
  release gate is `cargo build --release` plus debug tests plus CI.

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

### Fixed
- **Precise layer promotion**: the dream's importance mirror now matches the
  layer entry by exact FNV-1a hash equality of the marker-stripped text instead
  of substring matching, so overlapping entries can never receive a wrong bump
  and at most one entry changes per promoted block.
- **Release-safe header reads**: `MicroscopeReader::header` is now bounds
  checked with `assert!` in all builds (previously `debug_assert!`, which is
  compiled out of release). The hot whole-index loops use the explicitly
  `unsafe header_unchecked`, so an out-of-range index can never read outside
  the mmap region again.
- **Bounded MCP framing**: the stdio MCP server rejects `Content-Length` values
  above 16 MiB before allocating the body buffer (previously an unbounded
  `vec![0u8; len]`), and caps header lines at 64 KiB so a client that never
  terminates its headers cannot grow memory without bound.
- **Dependency audit**: cleared all seven `cargo audit` vulnerabilities —
  pyo3 0.20 → 0.29 (also adds Python 3.14 support and fixes the Cargo.toml
  target-section bug that made the optional native deps unresolvable on native
  builds), crossbeam-epoch 0.9.20, quinn-proto 0.11.16, rustls-webpki 0.103.13;
  anyhow/memmap2/rand updated to clear the unsound advisories.
- **NaN-safe ranking**: 64 `partial_cmp(&...).unwrap()` f32 comparators across
  the recall/rank/sort paths replaced with `total_cmp`, so a NaN score can
  never panic a hot path.
- **Bridge auth**: optional `[server] api_key` — when set, every inbound bridge
  request must carry `X-API-Key: <key>` (checked before the handlers, after
  CORS so preflight keeps working). The bridge refuses to start on a
  non-loopback host without a key, and the single-user localhost default is
  documented in the README.
- **Per-user bridge tokens**: `microscope-mem token <user_id>` generates an
  HMAC-SHA256-signed bearer token (`user_id.<hmac>`, constant-time verified).
  The middleware binds the memory scope to the verified `user_id`, so users
  are separated from each other, not just from outsiders; the shared
  `X-API-Key` mode remains as the trusted-team fallback.
- **Poison-safe locks**: 200 `RwLock`/`Mutex` `.read()/.write()/.lock().unwrap()`
  sites across the codebase now go through `sync_guard::{read_lock, write_lock,
  mutex_lock}`, which recover from a poisoned lock instead of panicking forever.
  One stray panic while holding a lock can no longer permanently break a
  subsystem for the rest of the process lifetime.
- **Self-defensive multimodal decoding**: `decode_image_meta` and
  `decode_audio_meta` now return `Option` and reject short input up front
  instead of relying on every caller to bounds-check first.
- **Cross-process snapshot mmap**: `consciousness_seqlock` now ships a real
  file-backed mmap layer (`MappedSnapshot` / `MappedSnapshotWriter`) mapping
  the `#[repr(C)]` seqlock header + `SnapshotData` block via `memmap2`, so two
  processes sharing a file see the same snapshot with the same seqlock
  protocol (odd/even sequence, retry on torn reads). The heap-backed fast-path
  fields remain in-process only. The `ConsciousnessStream` background cycle now
  publishes to `<output_dir>/consciousness_snapshot.bin` every tick, so another
  process can map the file and read the live consciousness snapshot. Covered by
  roundtrip, uninitialized-file, no-torn-reads-under-concurrency and
  stream-publishes-to-mmap tests.
- **Honored `[server] cors_origin`**: the bridge previously hardcoded
  `allow_origin(Any)` and silently ignored the config value. It now uses the
  configured origin (exact match; the requester's origin is never reflected),
  with `"*"`/unset keeping the open default. Covered by a CORS test.
- **Deterministic morphogenesis**: the growth functions derive their RNG from
  `Seed::rng()` (FNV hash of the seed id + position), so the same seed always
  produces the same organism — the previously flaky, RNG-driven tests are now
  fully deterministic. The evolution loop's mutation RNG is now seeded from
  the generation number too, so a whole `evolve_population` run is
  reproducible for the same initial seeds (verified byte-identical).
- **Poison-safe locks completed**: the earlier sweep missed 34 multi-line
  `RwLock`/`Mutex` `.read()/.write()/.lock().unwrap()` sites (receiver and
  call on separate lines); all now go through `sync_guard` as well.
- **Debug CLI stack overflow fixed**: `main` now runs the async runtime on a
  16 MiB thread, so debug builds no longer overflow the 1 MiB OS main-thread
  stack at startup (`cargo run` works again).

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
