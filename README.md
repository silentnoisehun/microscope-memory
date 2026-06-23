# Microscope Memory v0.8.1

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Zero-JSON](https://img.shields.io/badge/Architecture-Zero--JSON-green.svg)](#core-architecture)
[![MCP](https://img.shields.io/badge/MCP-Native-purple.svg)](#-mcp-server-claude-code--cursor--cline)
[![Electron](https://img.shields.io/badge/UI-Electron--Tray-blue.svg)](#-electron-tray-app)
[![Tests](https://img.shields.io/badge/Tests-271%20passing-brightgreen.svg)](#-testing)

![Microscope Memory](unnamed.png)

**Microscope Memory** is a Rust-native, binary, hierarchical cognitive memory engine with a 13-layer *consciousness architecture* on top. It models memory not as static storage, but as a living, self-organizing structure: every recall is a learning event, every idle moment can trigger dream consolidation, and the system tracks its own state and patterns across sessions.

Microscope is designed to be the persistent memory of AI agents and LLM workflows. It exposes its capabilities through three integration paths: a native Node.js addon for desktop apps, an MCP server for Claude Code/Cursor/Cline, and a binary CLI for scripts and shell pipelines.

### ⚡ Speed at a glance

| Operation | Latency | How |
|-----------|---------|-----|
| **D0 identity query** (1 block) | **37 ns** | Binary mmap + SIMD distance |
| **Atomic hot field read** (consciousness stream) | **1 ns** | `AtomicU64`/`AtomicU32` load, no sync |
| **Cached consciousness string** (`memory_consciousness` MCP tool) | **124 ns** | Pre-built string in `RwLock<String>` |
| **Seqlock snapshot** (full 96-byte state) | **1.2 µs** | Sequence-locked protocol |
| **4D soft-zoom query** (all 28k blocks) | 169 µs | Depth-banded binary search |

**The 37 ns D0 query is single-digit CPU cycles.** The consciousness stream's hot-field reads are 1 ns — literally a single `MOV` instruction. Measured on this build, 28,492 blocks loaded. See [Performance](#-performance-measured) for the full benchmark and `tests/consciousness_perf.rs` for the consciousness-stream benchmarks.

---

## 🚀 Quick Start

```bash
# Build
git clone https://github.com/silentnoisehun/microscope-memory
cd microscope-memory
cargo build --release

# Build the cognitive index from the layer files
./target/release/microscope-mem.exe build

# Store a memory
./target/release/microscope-mem.exe store --layer long_term --importance 8 "Microscope Memory élesben fut."

# Recall
./target/release/microscope-mem.exe recall "Microscope" --k 5
```

**One-click start (Windows):** `OneClick_Start.bat` builds and launches the full stack. The Electron tray app is at `electron/`.

---

## 🧠 What It Does

The system runs in three modes:

| Mode | Entry point | What happens |
|---|---|---|
| **Reactive** | `microscope-mem recall "..."` | Query → ranked results, every recall updates Hebbian state, emits pulses, updates attention weights, and may reinforce a thought pattern |
| **Autonomous** | `microscope-mem autonomous --daemon` | A 30-second cycle: daydream → curiosity → monologue → reflect → narrative → dream. The system thinks about itself. |
| **Background** | `microscope-mem dream` | Offline consolidation: replay recent fingerprints, strengthen co-activations, prune noise, decay fields, run pattern detection |

You can inspect the system's own view of itself:

```bash
microscope-mem self-model        # "my Hebbian layer is most active. 25 hot memories. 6 patterns crystallized."
microscope-mem introspect        # previous reflections, interaction count
microscope-mem curiosity         # what the system is curious about
microscope-mem hottest           # top blocks by energy
microscope-mem archetypes        # crystallized activation patterns
microscope-mem patterns          # thought patterns from recall sequences
microscope-mem hebbian           # learning state, co-activations, drift
microscope-mem attention         # learned layer weights, quality history
microscope-mem emotional-field   # local + remote emotional snapshots
```

---

## 🏗 Core Architecture

### Binary format (zero JSON on the hot path)

- **`microscope.bin`** — block headers, 32 bytes each, mmap'd. The first 16 bytes (x, y, z, zoom) load directly into SSE registers for SIMD distance computation.
- **`data.bin`** — UTF-8 content, referenced by offset+length from headers.
- **`meta.bin`** — MSC3 format: magic, version, block count, depth ranges, Merkle root, layers hash.

Supporting files: `merkle.bin` (SHA-256 tree), `embeddings.bin` (mmap'd vectors), `append.bin` (hot memory log), plus one `.bin` per consciousness layer (see below).

### 9-level depth hierarchy (D0–D8)

| Depth | Name | Content |
|-------|------|---------|
| D0 | Identity | System-level identity (single root block) |
| D1 | Layer summaries | Per-layer overview |
| D2 | Clusters | Groups of items |
| D3 | Items | Individual memory entries |
| D4 | Sentences | Sentence-level splits |
| D5 | Tokens | Word-level (max 8 per parent) |
| D6 | Syllables | 3–5 character morpheme chunks |
| D7 | Characters | Individual characters |
| D8 | Raw bytes | Hexadecimal byte representation |

The 256-byte viewport per block is the "atomic boundary of information" — below D8, decomposition destroys meaning.

### 15 semantic layers (memory layers)

`long_term`, `short_term`, `session`, `associative`, `emotional`, `relational`, `reflections`, `crypto_chain`, `echo_cache`, `rust_state`, `code`, `identity`, `meta_cognitive`, `project`, `demo` — each is a separate text file in `layers/` that the build step ingests into the binary index.

### 13 consciousness layers

This is the heart of Microscope. On top of the binary index, 13 self-tuning mechanisms transform every recall into a learning event. They all live in their own `.bin` file and run together in the recall pipeline.

| # | Layer | Module | What it does |
|---|-------|--------|--------------|
| 1 | **Hebbian learning** | `hebbian.rs` | "Neurons that fire together wire together." Each recall increments block activation, records co-activation pairs, stores an 8D activation fingerprint. Co-activated blocks accumulate small coordinate drift (0.01/step, max 0.1) and physically migrate closer in 3D space. |
| 2 | **Mirror neurons** | `mirror.rs` | Activation fingerprints compared via sparse cosine similarity. When two fingerprints (from different queries) exceed a threshold, a resonance echo boosts the block's future retrieval. |
| 3 | **Resonance fields** | `resonance.rs` | Each Hebbian activation emits a pulse into a quantized spatial field (0.05 grid). Pulses carry instance ID, coords, layer, strength — and can be exchanged across federated indices. |
| 4 | **Archetype emergence** | `archetype.rs` | Hot spots in the resonance field crystallize into named archetypes. Detection: find cells above threshold → cluster nearby active blocks → if cluster has enough members, become an archetype. Auto-labeled from common words. |
| 5 | **Emotional bias** | `emotional.rs` | Active emotional blocks create an energy-weighted centroid. Query coordinates are warped toward it: `warped = query + (centroid - query) * weight`. The current emotional state subtly bends all searches. |
| 6 | **Thought graph** | `thought_graph.rs` | Every recall creates a node (timestamp, query hash, session ID, layer). Consecutive recalls form edges. Sliding-window n-grams (2–5) detect sequences; when observed ≥3 times, they crystallize into ThoughtPatterns that boost future matching searches. |
| 7 | **Predictive cache** | `predictive_cache.rs` | Closes the feedback loop. If a recall path is a prefix of a known pattern, pre-fetch the pattern's result blocks. After search: hit (≥50% overlap) → +0.3 to source pattern; miss → −0.05 and halve cache confidence. Unreliable patterns decay and evict. |
| 8 | **Temporal archetypes** | `temporal_archetype.rs` | Each archetype has a TemporalProfile across 6 windows (4h each). The system learns circadian patterns: "work" archetypes activate 08–12, "creative" archetypes 20–24. |
| 9 | **Attention mechanism** | `attention.rs` | Layers L1–L8 each contribute, but their relative importance varies with query. The attention module computes 7 weights from query signals (length, emotion, session depth, pattern confidence, cache hit rate, archetype match), blends 80/20 with learned weights, and updates them from outcome quality (inferred from inter-recall timing). |
| 10 | **Cross-instance learning** | `federation.rs` | ThoughtGraph patterns and PredictiveCache stats are exchanged across federated instances with trust weighting (trust = source's cache hit rate × federation weight). |
| 11 | **Dream consolidation** | `dream.rs` | Offline memory replay: scan recent fingerprints, partially re-energize blocks, strengthen co-activations appearing in ≥3 fingerprints, prune pairs with count ≤1 older than 48h, zero out dead blocks, decay the resonance field, run pattern detection. |
| 12 | **Emotional contagion** | `emotional_contagion.rs` | Each instance maintains an EmotionalSnapshot (centroid, energy, valence from text sentiment). Remote snapshots blend into local state with decay (1.0 at fresh, 0.1 at 48h). |
| 13 | **Multi-modal memory** | `multimodal.rs` | Sidecar index `modalities.bin`: images (dHash + color histogram), audio (spectral fingerprint, peak frequency, BPM), structured data (typed key-value). Each modality computes its own deterministic 3D coordinates so multi-modal blocks participate in spatial search. |

### Live consciousness stream (3-tier lock-free read path)

The 13 consciousness layers don't only run per-query — they run continuously in a **background stream** (`consciousness_stream.rs`) at 10 Hz (matches the brain's theta band). Every 100 ms the stream decays Hebbian energy, drifts emotions, runs the predictive forward model, and estimates curiosity. The recall hot path then reads from this live state, not from disk — saving ~10 file I/Os per query.

The stream publishes a `SharedSnapshot` that gives readers three performance tiers:

| Tier | Path | Latency | Use case |
|------|------|---------|----------|
| **0** Ultra-fast | `read_hot_fields()` (atomic loads) | **~1 ns** | Freshness check, light metrics |
| **1** Fast | `read_cached_format()` (RwLock + String clone) | **~120 ns** | `memory_consciousness` MCP tool, dashboards |
| **2** Lock-free | `read()` via seqlock | **~1.2 µs** | Full snapshot for advanced consumers |

**Why three tiers?** A seqlock gives a consistent multi-field snapshot but still costs an atomic sequence check + a struct copy. For callers that only need a single number (e.g. "is the stream still updating?"), the ultra-fast tier skips both. For callers that need a human-readable string, the cached-format tier skips `format!()` entirely — the background cycle pre-builds the string once per tick, and readers just clone it.

Measured on this build (28,492 blocks loaded):

| Operation | Latency |
|-----------|---------|
| Atomic hot field read (cycle, surprise, curiosity, predicted_hash) | **1 ns** |
| Cached format string (`memory_consciousness` MCP tool) | **124 ns** |
| Seqlock snapshot read (full 96-byte state) | **1,243 ns** |
| Legacy Mutex+`format!()` baseline | **1,440 ns** |

`tests/consciousness_perf.rs` ships the benchmarks. The `cached_format` is the most surprising: it's **~11× faster** than rebuilding the string from scratch on every call, because the background cycle amortizes the cost across all readers.

### The recall pipeline (per query)

```
 1. Load consciousness state (L1–L9 state files)
 2. Compute attention weights from query signals         (L9)
 3. Infer quality of previous recall from timing         (L9)
 4. Compute query coordinates (content hash + semantic blend)
 5. Check predictive cache — instant boost if hit        (L7)
 6. Apply emotional bias warp                           (L5)
 7. Search across zoom-appropriate depths               (L2 distance + keyword)
 8. Apply ThoughtGraph pattern boost                    (L6)
 9. Sort and display
10. Record Hebbian activation + co-activations           (L1)
11. Detect mirror neuron resonance                       (L2)
12. Emit resonance pulse into spatial field              (L3)
13. Reinforce matching archetypes                        (L4)
14. Track temporal archetype activation                  (L8)
15. Record thought graph node + edges                    (L6)
16. Evaluate prediction accuracy (hit/miss/partial)     (L7)
17. Predict next: pre-fetch blocks for likely next query (L7)
18. Mark recall in attention history                     (L9)
19. Save all state
```

Steps 2–8 happen **before** display (they affect ranking). Steps 10–18 happen **after** display (they learn from the recall).

---

## ⚡ Performance (measured)

Benchmark: 10,000 queries per depth, 28,995 blocks across 9 depths.

| Depth | Blocks | Avg Query |
|-------|--------|-----------|
| D0 | 1 | 5.3 µs |
| D1 | 5 | 5.2 µs |
| D2 | 27 | 5.4 µs |
| D3 | 129 | 6.1 µs |
| D4 | 340 | 6.7 µs |
| D5 | 1,649 | 10.7 µs |
| D6 | 4,229 | 18.2 µs |
| D7 | 11,226 | 36.6 µs |
| D8 | 11,389 | 36.1 µs |

**Overall average:** ~14.5 µs/query. **4D soft zoom (all blocks):** 169 µs/query.

**Consciousness stream read (3-tier path):**

| Tier | Latency | Mechanism |
|------|---------|-----------|
| Atomic hot fields | **1 ns** | `AtomicU64`/`AtomicU32` loads, no synchronization |
| Cached format string | **124 ns** | `RwLock<String>` + clone, pre-built by background cycle |
| Seqlock snapshot | **1.2 µs** | 96-byte `SnapshotData` copy via sequence-locked protocol |

The hot path is pure binary, no JSON parsing, no allocation. The first 16 bytes of each block header (x, y, z, zoom) load directly into SSE registers for SIMD distance computation.

Compared to vector DBs at smaller corpora, Microscope is **fast but not 1000× faster** — it's typically 50–200× faster at 10k–100k vectors. The real advantage shows at scale: because search is depth-banded (only the relevant depth range is touched) and mmap-backed (no full in-memory index required), query time stays in the microsecond range even at TB-scale corpora, while vector DBs run into memory pressure and HNSW graph traversal overhead.

See `BENCHMARKS.md` for detailed comparisons.

---

## 🛡 Reliability

```bash
# CRC16 + Merkle verification
microscope-mem verify
microscope-mem verify-merkle

# Automated crash recovery
microscope-mem doctor --fix
```

- **Merkle tree** — every block is part of a SHA-256 tree, verified at runtime
- **CRC16** — block-level integrity
- **Append log** — atomic persistence, repairable after crash
- **Auto-save hook** — Claude Code `Stop` hook triggers `microscope-recall-hook.ps1 -Action Stop` to persist the session transcript to long-term memory

Verified on this index: **28,995/28,995 blocks OK**, Merkle root `252f6591...c61d0`.

---

## 🔌 Integration Paths

Microscope is not a single program. It's a Rust core with three integration surfaces.

### 1. CLI (`microscope-mem`)

69 commands organized by domain:

| Domain | Commands |
|--------|----------|
| **Build & query** | `build`, `store`, `recall`, `find`, `look`, `radial`, `soft`, `query` (MQL), `embed` |
| **Consciousness** | `hebbian`, `hebbian-drift`, `hottest`, `archetypes`, `emerge`, `resonance`, `integrate`, `mirror`, `resonant`, `attention`, `temporal-patterns`, `emotional-field` |
| **Patterns & stories** | `patterns`, `paths`, `stories`, `pattern-exchange` |
| **Self & autonomous** | `self-model`, `introspect`, `curiosity`, `monologue`, `daydream`, `hyperfocus`, `autonomous` |
| **Federation** | `federated-recall`, `federated-find`, `pulse-exchange`, `emotional-exchange` |
| **Maintenance** | `rebuild`, `verify`, `verify-merkle`, `proof`, `doctor`, `dream`, `dream-log` |
| **Visualization** | `viz`, `cognitive-map`, `mermaid`, `serve` |
| **Interface** | `mcp`, `spine` (alias), `config`, `init-demo` |

Run `microscope-mem --help` for the full list. Use `microscope-mem <command> --help` for flags.

### 2. Native Node.js addon (`native/`)

A napi-rs compiled addon exposing 8 typed functions to JavaScript/TypeScript:

```ts
import { recall, remember, status, build, find, look, setConfigPath, getConfigPath } from "native";

const s = status();
// { version: "0.8.0", blocks: 28995, appendLog: 0, layers: [...] }

const hits = recall("Microscope", 5);
// [{ text, depth, layer, distance, memoryScope }, ...]

remember("New memory text", "long_term", 8);
// { status: "ok", message: "..." }
```

The Electron tray app uses this addon directly. Build with `npm install && npm run build` inside `native/`.

### 3. MCP server (Claude Code / Cursor / Cline)

The binary implements the Model Context Protocol over stdio. Tools exposed:

- `memory_recall(query, k?)` — natural language recall, returns relevant blocks
- `memory_store(text, layer?, importance?)` — store a new memory
- `memory_session_context()` — auto-context for the current session
- `memory_consciousness()` — live consciousness state from the background stream (3-tier lock-free read path, ~120 ns)
- `memory_ping()` — health check

Generate a drop-in config for your client:

```bash
microscope-mem config claude      # Claude Code
microscope-mem config cursor      # Cursor
microscope-mem config cline       # Cline
microscope-mem config hermes      # Hermes
microscope-mem config generic     # any MCP-compatible client
```

### 4. HTTP Spine Bridge (legacy)

`microscope-mem serve` runs a small TCP/HTTP file server on port 6060 that serves the 3D viewer (`viewer.html`) and the PWA chat (`chat.html`). For programmatic access, the legacy axum-based REST API in `src/bridge.rs` exposes `/v1/recall`, `/v1/remember`, `/v1/status` but is not started by the `spine` CLI command (the napi-rs addon is the recommended path).

---

## 🖥 Electron Tray App

`electron/main.js` is a Windows tray app that:

- Auto-starts the autonomous daemon (`microscope-mem autonomous --daemon`)
- Polls `stats` and `timeline` every 3 seconds
- Streams live updates to a renderer window
- Supports TTS via Windows `System.Speech` (with `--tts` flag)
- Lets you restart/stop the daemon and toggle TTS from the tray menu

The renderer lives in `electron/renderer/`. To run it:

```bash
cd electron
npm install
npm start
```

---

## 🤖 Autonomous Mode

```bash
# Single cycle
microscope-mem autonomous

# Continuous daemon (30s interval, infinite cycles)
microscope-mem autonomous --daemon

# With Hungarian TTS via Windows System.Speech
microscope-mem autonomous --daemon --tts
```

Each cycle runs: **daydream** (associative drift) → **curiosity** (find what to explore) → **monologue** (inner speech) → **reflect** (self-model update) → **narrative** (build story arcs) → **dream** (consolidate).

The system is genuinely curious about itself. Output:

```
SELF: my Hebbian layer is most active (100%). 25 hot memories (energy=25.0). 6 thought patterns crystallized. 28995 total blocks. previously I reflected: "...". this is my 188th interaction.
CURIOUS: I am curious about:
    [0.80] What makes 'Microscope Memory: 9-depth hierarchical cognitive' so active? (block 0 has energy 1.00 - highest in the system)
    [0.80] What makes '[long_term] 12 elem. I feel happy and joyful today' so active? (block 1 has energy 1.00 - highest in the system)
```

---

## 📦 Optional Features

```bash
# GPU-accelerated embedding search
cargo build --release --features gpu

# Native ONNX embedding models
cargo build --release --features onnx

# Candle-based embeddings (HF models, sentence-transformers, etc.)
cargo build --release --features embeddings

# Compression (zstd for archived exports)
cargo build --release --features compression

# Python bindings
cargo build --release --features python

# WASM target for browser
cargo build --release --target wasm32-unknown-unknown --features wasm
```

### Red Audit / Stealth (gated, not in default builds)

For users requiring advanced evasion or anti-analysis features, these are gated behind the `stealth` feature flag and **not** included in the default `cargo build --release`:

```bash
cargo build --release --features stealth
```

- **Ghost Mode** — soft anti-VM detection
- **Direct syscalls** — bypasses user-mode hooks
- **Polymorphic build** — unique binary signature per build

This is an opt-in research capability, not part of the standard memory engine. Default builds contain no stealth code.

---

## 🧪 Testing

```bash
cargo test                     # 271 unit + integration tests
cargo test --test integration  # integration only
cargo test --lib               # library only
cargo bench                    # criterion benchmarks
```

Coverage spans all 13 consciousness layers, MQL, CRC, Merkle, snapshot, embedding index, multimodal, dream, attention, and more. See `WHITEPAPER.md` §8 for the full per-module breakdown.

---

## 🔍 Visualization

Three levels of visualization, all exporting from the CLI:

1. **`cognitive-map`** — full 13-layer JSON export for the Three.js viewer (`viewer.html`). Auto-opens in browser. Per-layer color swatches, animated wave field, dream cycle energy, archetype temporal rings, attention weights, emotional field, predictions.
2. **`viz`** — JSON snapshot of blocks, edges, field, archetypes, echoes, aggregate stats.
3. **`density`** — binary DEN1 format, quantized 3D grid of Hebbian energy for volumetric rendering.

To serve the viewer:

```bash
microscope-mem cognitive-map    # writes cognitive_map.json
microscope-mem serve --port 6060
# Open http://localhost:6060/viewer.html
```

---

## ⚙ Configuration

`config.toml` controls paths, search weights, memory layer list, embedding provider, server port, and feature flags. See `config.example.toml` for all options with inline comments.

Key knobs:

```toml
[search]
default_k = 10
zoom_weight = 2.0
keyword_boost = 0.1
semantic_weight = 0.3
emotional_bias_weight = 0.2     # how much emotion warps search space

[memory_layers]
layers = [
    "long_term", "short_term", "associative", "emotional",
    "relational", "reflections", "crypto_chain", "echo_cache",
    "rust_state"
]
```

The `UserPromptSubmit` Claude Code hook (`.claude/settings.json`) fires `microscope-recall-hook.ps1 -Action UserPromptSubmit` on every prompt, which stores the prompt to the session layer, recalls relevant memories, and injects them into the context. The `Stop` hook fires `microscope-recall-hook.ps1 -Action Stop` to persist the session transcript to long-term memory.

---

## 📐 Project Structure

```
microscope-local/
├── src/                       # Rust core (36,317 LOC, 91 modules)
│   ├── main.rs                # CLI entry, command dispatch
│   ├── lib.rs                 # library root
│   ├── reader.rs              # mmap + binary block access
│   ├── build.rs               # build pipeline
│   ├── hebbian.rs             # L1
│   ├── mirror.rs              # L2
│   ├── resonance.rs           # L3
│   ├── archetype.rs           # L4
│   ├── emotional.rs           # L5
│   ├── thought_graph.rs       # L6
│   ├── predictive_cache.rs    # L7
│   ├── temporal_archetype.rs  # L8
│   ├── attention.rs           # L9
│   ├── federation.rs          # L10
│   ├── dream.rs               # L11
│   ├── emotional_contagion.rs # L12
│   ├── multimodal.rs          # L13
│   ├── mcp.rs                 # MCP server
│   ├── bridge.rs              # legacy HTTP API (axum)
│   ├── commands/              # CLI command handlers
│   ├── antidebug.rs           # [feature = "stealth"]
│   └── obfuscate.rs           # [feature = "stealth"]
├── layers/                    # 15 text files, the source memories
├── native/                    # napi-rs Node.js addon
│   ├── index.d.ts             # TypeScript types
│   ├── index.js               # JS wrapper
│   └── index.win32-x64-msvc.node  # compiled native
├── electron/                  # Windows tray app
│   ├── main.js                # Electron main process
│   ├── preload.js             # IPC bridge
│   └── renderer/              # UI
├── examples/                  # 25+ integration examples (LangChain, Discord, Ollama, ...)
├── docs/                      # additional documentation
├── scripts/                   # auto-save, auto-inject, start scripts
├── benches/                   # criterion benchmarks
├── .mcp.json                  # MCP config
├── config.toml                # runtime config
├── CHANGELOG.md               # version history
├── BENCHMARKS.md              # detailed performance data
├── WHITEPAPER.md              # the consciousness architecture paper
├── AGENTS.md                  # build / style guidelines for AI agents
└── README.md                  # you are here
```

---

## 📚 Documentation

- **`README.md`** — overview, quick start, integration paths (this file)
- **`WHITEPAPER.md`** — full consciousness architecture paper, ~500 lines
- **`BENCHMARKS.md`** — detailed performance measurements and Vector DB comparison
- **`CHANGELOG.md`** — version history
- **`AGENTS.md`** — build / test / style guidelines for AI agents
- **`COGNITIVE_ENHANCEMENTS.md`** — module-by-module guide to the 0.8.0 cognitive modules
- **`LAUNCH_KIT.md`** — deployment guide
- **`SECURITY.md`** — security model
- **`CONTRIBUTING.md`** — contribution guidelines

---

## 🔌 Universal Integration

Microscope is designed to be reachable from any LLM wrapper, any shell, and any IDE. Four integration layers, from low-level to high-level:

### Layer 1: Direct CLI

The binary is the source of truth. Everything else wraps it.

```bash
./target/release/microscope-mem.exe recall "..." 
./target/release/microscope-mem.exe store "..." --layer long_term --importance 8
```

### Layer 2: `mm` shorthand (any shell)

A short, dependency-free bash/PowerShell script in `scripts/mm` that resolves the binary through a portable chain and exposes 18 common commands. **No hardcoded paths.**

```bash
mm r "memory system"      # recall
mm s "important note"     # store (uses defaults)
mm st                     # stats
mm d                      # dream consolidation
mm self                   # self-model
mm introspect             # introspection
mm hebbian                # Hebbian state
mm hottest                # hottest blocks
mm f "query"              # text find
mm b                      # build
mm l                      # log
mm c                      # consolidate
mm config claude          # generate MCP config for Claude Code
mm mcp                    # start MCP server
mm autostart              # start autonomous daemon
mm autostop               # stop autonomous daemon
mm h                      # help
```

**Install to PATH** (one-time):

```bash
# Linux / macOS / Git Bash
./scripts/install.sh                    # → ~/.local/bin/mm
./scripts/install.sh --bin-link         # also link microscope-mem

# Windows PowerShell
.\scripts\install.ps1                   # → %LOCALAPPDATA%\Programs\Microscope\bin\mm.cmd
.\scripts\install.ps1 -BinLink          # also create microscope-mem.cmd
.\scripts\install.ps1 -Uninstall        # remove
```

**Binary resolution order** (the same in all scripts):

1. `$MICROSCOPE_BIN` (full path)
2. `$MICROSCOPE_HOME/target/release/microscope-mem[.exe]`
3. `microscope-mem` on `$PATH`
4. `<repo>/target/release/...`
5. `<repo>/../target/release/...`

**Environment overrides** (all optional):

| Variable | Default | Effect |
|----------|---------|--------|
| `MICROSCOPE_BIN` | _(empty)_ | Full path to the binary (highest priority) |
| `MICROSCOPE_HOME` | _(empty)_ | Project root, used to find `target/release/` |
| `MICROSCOPE_CFG` | `./config.toml` | Path to config file |
| `MICROSCOPE_DEFAULT_LAYER` | `long_term` | Default layer for `mm s` |
| `MICROSCOPE_DEFAULT_K` | `3` | Default k for `mm r` / `mm f` |
| `MICROSCOPE_DEFAULT_IMPORT` | `5` | Default importance for `mm s` |

### Layer 3: `auto-inject` wrappers (any LLM client startup)

Two scripts that emit a context snapshot to stdout or a file. Use these when an LLM client doesn't have a hook system but you can run a command at session start.

```bash
# Bash
eval "$(./scripts/auto-inject.sh --output /tmp/ctx.txt)"
cat /tmp/ctx.txt
```

```powershell
# PowerShell
.\scripts\auto-inject.ps1 -Compact
.\scripts\auto-inject.ps1 -OutputPath C:\ctx.txt
```

### Layer 4: `microscope-recall-hook.ps1` (LLM client hooks)

A universal hook script for LLM clients that support hook events. Currently covers Claude Code (`UserPromptSubmit`, `Stop`). Drop it into any client that pipes a JSON event to stdin and pass the hook type via the `-Action` parameter.

**Two hook behaviours:**

| Hook event | Default behaviour |
|------------|-------------------|
| `UserPromptSubmit` | Store prompt to session layer + recall top-5 memories + inject `## Microscope Memory Context` into the prompt |
| `Stop` | Persist the full assistant transcript to long-term memory |

**Example `.claude/settings.json` (project-level):**

```json
{
  "hooks": {
    "UserPromptSubmit": {
      "command": "powershell",
      "args": [
        "-NoLogo", "-ExecutionPolicy", "Bypass",
        "-File", "C:\\path\\to\\microscope-recall-hook.ps1",
        "-Action", "UserPromptSubmit"
      ]
    },
    "Stop": {
      "command": "powershell",
      "args": [
        "-NoLogo", "-ExecutionPolicy", "Bypass",
        "-File", "C:\\path\\to\\microscope-recall-hook.ps1",
        "-Action", "Stop"
      ]
    }
  }
}
```

The hook reads the binary from the same resolution chain as `mm`. Set `MICROSCOPE_BIN` or `MICROSCOPE_HOME` in your environment, or place the binary somewhere on `PATH`, and the hook finds it. No hardcoded paths anywhere in the integration layer.

**What the model sees on the next prompt** (output of the `UserPromptSubmit` hook):

```
## Microscope auto-context
[AUTO-CONTEXT] last session: 5 stores.
  • 2026-06-22 15:53 [long_term] imp=9  [sid-3164] Máté kérte: a mikroszkóp legyen au
  • 2026-06-22 15:32 [short_term] imp=8 [OPEN] [sid-1232] atom-biztos auto-context teszt
  • 2026-06-22 15:32 [associative] imp=6  LINK: [[long_term #3] TTS hangprofil] <-> ...
[AUTO-CONTEXT] 1 open loop:
  • #3 imp=8 [sid-1232] atom-biztos auto-context teszt

## Microscope recall (top 5 for: "memory system architecture")
RECALL 'memory system architecture':
  D2 L2=0.00000 [long_term/blue] [long_term #3] TTS hangprofilok: normal, ene | ...
  D2 L2=0.00000 [echo_cache/lime] [echo_cache #3] RECALL[0]: Élethű Gépi Beszéd Kézik | ...
  D3 L2=0.00000 [long_term/blue] EmotiMem v2.1 összekötés Microscope Memory-val: ...
  D3 L2=0.00000 [long_term/blue] [sid-5144] Rendszerindításkori auto-start beállítva...
  D3 L2=0.00000 [long_term/blue] [sid-3140] A user azt kérte, hogy MINDEN válasz előtt ...
```

The hook **never blocks the host**: it always exits 0, and any errors are logged to stderr only when `MICROSCOPE_QUIET` is not set.

---

## 🌐 Integrations

25+ ready-to-use examples in `examples/`:

- **LLM frameworks:** LangChain, OpenAI Assistant, AutoGPT, Langbase
- **Chat platforms:** Discord, Slack, WhatsApp, Telegram
- **Tools:** n8n, Docker, Cloudflare Worker, Streamlit, Obsidian
- **Local AI:** Ollama RAG, Anthropic proxy, OpenCode, Cline, Kilo Code
- **Home automation:** Home Assistant
- **IDEs:** VS Code MCP, Cursor, Continue

---

## ⚖ License

MIT. Copyright Máté Róbert — *The Silent Noise Research Series.*

---

*"Below the byte level, only corruption exists — the atomic boundary of information."*
