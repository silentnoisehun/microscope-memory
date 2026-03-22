# Microscope Memory

[![CI](https://github.com/silentnoisehun/microscope-memory/actions/workflows/ci.yml/badge.svg)](https://github.com/silentnoisehun/microscope-memory/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Zoom-based hierarchical memory system with sub-microsecond queries**

A memory indexing system that treats data like looking through a microscope — the zoom level determines what you see. Pure binary, zero JSON, powered by mmap, SIMD vector search, and rayon parallelism.

## Features

- **Sub-microsecond queries**: 37ns to 500us depending on depth
- **9-level hierarchy**: From identity (D0) to raw bytes (D8)
- **3D spatial indexing**: Content-based deterministic positioning
- **Zero-copy mmap**: Direct memory access, no serialization overhead
- **Hybrid search**: L2 distance + keyword matching + semantic embeddings
- **Real embeddings**: Candle BERT models (all-MiniLM-L6-v2) with mmap-backed embedding index
- **Query language (MQL)**: Structured queries with layer/depth/spatial filters and boolean operators
- **HTTP server**: tiny_http-powered REST API with thread pool
- **Snapshot archives**: `.mscope` format for backup, restore, and diff
- **Merkle integrity**: SHA-256 tree with per-block verification and proofs
- **Fixed viewport**: 256 bytes per block at every zoom level
- **Parallel build**: Rayon-based multi-threaded index construction
- **SSE4/AVX2 SIMD**: Hardware-accelerated L2 distance and cosine similarity
- **Zstd compression**: Optional data.bin compression with transparent decompression
- **Incremental build**: SHA-256 content hash skips rebuild when layers unchanged
- **GPU compute**: Optional wgpu-based acceleration
- **WASM support**: Compiles to WebAssembly
- **Python bindings**: PyO3-based Python integration

## Performance

Benchmarked on 227,168 blocks (10,000 queries per depth):

| Depth | Blocks | Query Time | Description |
|-------|--------|------------|-------------|
| D0 | 1 | **37 ns** | Identity |
| D1 | 9 | **92 ns** | Layer summaries |
| D2 | 108 | **506 ns** | Topic clusters |
| D3 | 523 | **1.7 us** | Individual memories |
| D4 | 1,349 | **3.9 us** | Sentences |
| D5 | 6,070 | **18 us** | Tokens |
| D6 | 26,198 | **72 us** | Syllables |
| D7 | 96,297 | **505 us** | Characters |
| D8 | 96,613 | **492 us** | Raw bytes |

## Philosophy

- **Fixed viewport size**: Every block is exactly 256 bytes
- **Zoom determines detail**: Not what data exists, but what you can see
- **Spatial coherence**: Similar content clusters in 3D space
- **Atomic boundary**: Below D8 (bytes), data corrupts — a philosophical limit

## Installation

### Prerequisites

- Rust 1.70+

### Build from source

```bash
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory

cargo build --release
```

### Configuration

Copy the example config and adjust your paths:
```bash
cp config.example.toml config.toml
```
Edit `config.toml` to set your `layers_dir` and `output_dir`.

## Usage

### Build the index

```bash
# Build binary index from memory layer JSON files
microscope-memory build

# Force rebuild even if layers are unchanged
microscope-memory build --force

# Rebuild — merges append log into main index
microscope-memory rebuild
```

Builds are **incremental** — if the layer source files haven't changed (verified via SHA-256 content hash in MSC3 meta format), the build is skipped. Use `--force` to override.

### Recall — natural language query

```bash
# Auto-zoom based on query complexity
microscope-memory recall "What is Ora?" 10
```

The auto-zoom selects depth based on query length:
- 1-2 words -> D0-D2 (identity/summaries)
- 3-5 words -> D1-D3 (topic clusters)
- 6-10 words -> D2-D4 (individual memories)
- 11-20 words -> D3-D5 (sentences)
- 20+ words -> D4-D6 (tokens)

### MQL — Microscope Query Language

```bash
# Filter by layer and depth range
microscope-memory query 'layer:long_term depth:2..5 "Ora"'

# Boolean operators
microscope-memory query '"memory" AND "Rust"'
microscope-memory query '"emotional" OR "relational"'

# Spatial filter (x,y,z,radius)
microscope-memory query 'near:0.2,0.3,0.1,0.05 "pattern"'

# Override result limit
microscope-memory query 'limit:20 layer:associative "concept"'
```

MQL supports:
| Filter | Syntax | Example |
|--------|--------|---------|
| Layer | `layer:NAME` | `layer:long_term` |
| Depth | `depth:N` or `depth:N..M` | `depth:3`, `depth:2..5` |
| Spatial | `near:X,Y,Z[,R]` | `near:0.2,0.3,0.1,0.05` |
| Keyword | `"quoted"` or `bare` | `"Ora"`, `memory` |
| Boolean | `AND`, `OR` | `"foo" AND "bar"` |
| Limit | `limit:N` | `limit:20` |

### Manual microscope control

```bash
# Look at specific coordinates (x, y, z) at zoom level 3
microscope-memory look 0.25 0.25 0.25 3

# 4D soft zoom (zoom as weighted dimension, searches all blocks)
microscope-memory soft 0.15 0.15 0.15 4
```

### Text search

```bash
# Brute-force text search across all depths
microscope-memory find "Ora" 5
```

### Store new memories

```bash
# Add to long-term memory (default) with importance 5 (default)
microscope-memory store "Important insight about the project"

# Specify layer and importance
microscope-memory store "Feeling good about progress" --layer emotional --importance 8
```

### Semantic search (embeddings)

```bash
# Cosine similarity search using pre-built embedding index
microscope-memory embed "quantum physics" 10

# Alternative metrics: l2, dot
microscope-memory embed "quantum physics" 10 --metric l2
```

When built with `--features embeddings`, uses a real Candle BERT model (all-MiniLM-L6-v2, 384 dimensions). Otherwise falls back to a mock hash-based embedding provider.

The embedding index (`embeddings.bin`) is built automatically during `build` and `rebuild`, providing mmap-backed zero-copy semantic search.

### HTTP Server

```bash
# Start server (default port 6060)
microscope-memory serve

# Custom port
microscope-memory serve --port 8080
```

Endpoints:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/stats` | Index statistics (block count, depths, append count) |
| `GET` | `/find?q=...&k=N` | Text search |
| `POST` | `/store` | Store memory: `{"text":"...", "layer":"...", "importance": N}` |
| `POST` | `/recall` | Recall query: `{"query":"...", "k": N}` |
| `POST` | `/query` | MQL query: `{"mql":"layer:long_term \"Ora\""}` |

Examples:
```bash
curl http://localhost:6060/stats
curl http://localhost:6060/find?q=Ora&k=5
curl -X POST http://localhost:6060/store -d '{"text":"test memory","layer":"long_term"}'
curl -X POST http://localhost:6060/recall -d '{"query":"What is Ora?","k":5}'
curl -X POST http://localhost:6060/query -d '{"mql":"layer:long_term depth:2..5 \"Ora\""}'
```

### Snapshot — backup, restore, diff

```bash
# Export entire index to a single .mscope archive
microscope-memory export backup.mscope

# Import archive into a directory
microscope-memory import backup.mscope --output-dir ./restored

# Compare two archives (Merkle roots, file sizes, block counts)
microscope-memory diff v1.mscope v2.mscope
```

The `.mscope` format bundles all index files (`meta.bin`, `microscope.bin`, `data.bin`, `merkle.bin`, `append.bin`, `embeddings.bin`) into a single portable archive.

### Integrity verification

```bash
# CRC16 checksum verification of all blocks
microscope-memory verify

# Merkle tree verification (SHA-256)
microscope-memory verify-merkle

# Generate Merkle proof for a specific block
microscope-memory proof 42
```

### Stats and benchmark

```bash
microscope-memory stats
microscope-memory bench
microscope-memory gpu-bench  # requires --features gpu
```

## Architecture

### Binary Structure

```
microscope.bin  — Block headers (32 bytes each, mmap'd)
├── x, y, z: f32          (3D spatial position)
├── zoom: f32              (normalized depth: depth/8.0)
├── depth: u8              (0-8)
├── layer_id: u8           (memory layer index)
├── data_offset: u32       (byte offset into data.bin)
├── data_len: u16          (actual text bytes, <= 256)
├── parent_idx: u32        (parent block index)
├── child_count: u16       (number of children)
└── crc16: [u8; 2]         (CRC16-CCITT integrity check)

data.bin        — Raw UTF-8 text content
data.bin.zst    — Zstd-compressed data (optional, --features compression)

meta.bin        — Index metadata (MSC3 format)
├── magic: "MSC3"          (4 bytes)
├── version: u32
├── block_count: u32
├── depth_count: u32
├── depth_ranges: 9 x (start: u32, count: u32)
├── merkle_root: [u8; 32]  (SHA-256 root hash)
└── layers_hash: [u8; 32]  (SHA-256 of source layer files — for incremental build)

merkle.bin      — Full Merkle tree (SHA-256)

embeddings.bin  — Pre-computed embedding vectors (mmap'd)
├── block_count: u32
├── dim: u32
├── max_depth: u32
└── vectors: f32 x dim x block_count

append.bin      — Hot memory append log (APv2 format)
```

### Memory Layers

The system integrates 9 cognitive layers:

| Layer | Description | 3D Region |
|-------|-------------|-----------|
| `identity` | System identity | (root) |
| `long_term` | Persistent knowledge | (0.0, 0.0, 0.0) |
| `short_term` | Working memory | (0.15, 0.15, 0.15) |
| `associative` | Concept connections | (0.3, 0.0, 0.0) |
| `emotional` | Affective associations | (0.0, 0.3, 0.0) |
| `relational` | Entity relationships | (0.3, 0.3, 0.0) |
| `reflections` | Meta-thoughts | (0.0, 0.0, 0.3) |
| `crypto_chain` | Cryptographic memories | (0.3, 0.0, 0.3) |
| `echo_cache` | Response history | (0.0, 0.3, 0.3) |
| `rust_state` | System state | (0.15, 0.0, 0.15) |

### Hierarchical Decomposition (D0-D8)

```
D0: Identity          — 1 block (entire system summary)
D1: Layer summaries   — 9 blocks (one per layer)
D2: Clusters          — groups of 5 items
D3: Individual items  — raw memories from JSON
D4: Sentences         — sentence-level splits
D5: Tokens            — word-level (max 8 per parent)
D6: Syllables         — 3-5 char morpheme chunks
D7: Characters        — individual characters
D8: Raw bytes         — hex representation (atomic limit)
```

## How It Works

1. **Content -> Position**: Text is FNV-hashed to deterministic 3D coordinates, offset by layer
2. **Hierarchical decomposition**: Each memory decomposes into sentences -> tokens -> syllables -> characters -> bytes
3. **Parallel build**: Depths 4-8 are constructed with rayon parallel iterators
4. **Embedding index**: Build-time embedding generation (mock or Candle BERT) into mmap-backed binary
5. **Spatial queries**: L2 distance in 3D space + zoom level filtering
6. **mmap access**: Zero-copy reads directly from memory-mapped binary files
7. **Hybrid ranking**: Vector distance + keyword boosting + semantic similarity
8. **Append log**: New memories stored instantly via binary append, merged on rebuild
9. **Merkle integrity**: SHA-256 tree for tamper detection and per-block proofs
10. **Incremental build**: SHA-256 hash of layer sources stored in MSC3 meta — skips rebuild when unchanged
11. **Optional compression**: Zstd-compressed `data.bin.zst` with transparent decompression at read time

## Source Structure

```
src/
├── main.rs              — Core engine: build, query, mmap reader, CLI
├── config.rs            — Configuration system (TOML-based)
├── embeddings.rs        — Embedding providers (Mock + Candle BERT)
├── embedding_index.rs   — Mmap-backed pre-computed embedding index
├── query.rs             — MQL parser and executor
├── streaming.rs         — HTTP server (tiny_http, thread pool)
├── snapshot.rs          — .mscope archive: export, import, diff
├── merkle.rs            — SHA-256 Merkle tree with proofs
├── gpu.rs               — Optional wgpu GPU acceleration
├── wasm.rs              — WASM target support
└── python.rs            — PyO3 Python bindings
```

## Optional Features

```bash
# Default build (no optional features)
cargo build --release

# Real BERT embeddings (downloads model from HuggingFace)
cargo build --release --features embeddings

# Zstd compression (compresses data.bin → data.bin.zst during build)
cargo build --release --features compression

# GPU acceleration (wgpu compute shaders)
cargo build --release --features gpu

# WASM build
cargo build --release --features wasm --target wasm32-unknown-unknown

# Python bindings
cargo build --release --features python

# All features
cargo build --release --features "embeddings compression gpu"
```

## Configuration

Example `config.toml`:

```toml
[paths]
layers_dir = "layers"
output_dir = "output"
temp_dir = "tmp"

[index]
block_size = 256
max_depth = 8
header_size = 32

[search]
default_k = 10
zoom_weight = 2.0
keyword_boost = 0.1
semantic_weight = 0.0

[memory_layers]
layers = ["long_term", "short_term", "associative", "echo_cache"]

[performance]
use_mmap = true
cache_size = 64
build_workers = 4
use_gpu = false
compression = false              # Enable zstd compression (requires --features compression)

[embedding]
provider = "mock"           # "mock" or "candle"
model = "sentence-transformers/all-MiniLM-L6-v2"
dim = 384
max_depth = 4               # Only embed blocks D0-D4

[server]
port = 6060
cors_origin = "*"

[logging]
level = "info"
file = "microscope.log"
```

## CLI Reference

```
microscope-memory <COMMAND>

Commands:
  build          Build binary index from raw layer files [--force]
  rebuild        Rebuild index (merges append log)
  store          Store a new memory
  recall         Natural language query with auto-zoom
  query          MQL query (Microscope Query Language)
  look           Manual look: x y z zoom [k]
  soft           4D soft zoom: x y z zoom [k]
  find           Text search
  embed          Semantic search using embeddings
  stats          Index statistics
  bench          Performance benchmark
  gpu-bench      GPU vs CPU benchmark
  verify         CRC16 integrity check
  verify-merkle  Merkle tree verification
  proof          Merkle proof for a specific block
  serve          Start HTTP server
  export         Export index to .mscope archive
  import         Import .mscope archive
  diff           Compare two .mscope archives
```

## License

MIT License - See [LICENSE](LICENSE) file for details.

For a deeper technical overview, see the [Whitepaper](WHITEPAPER.md).

---

*"Below the byte level, only corruption exists — the atomic boundary of information."*
