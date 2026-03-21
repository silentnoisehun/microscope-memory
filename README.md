# Microscope Memory

**Zoom-based hierarchical memory system with sub-microsecond queries**

A memory indexing system that treats data like looking through a microscope — the zoom level determines what you see. Pure binary, zero JSON, powered by mmap, L2 vector search, and rayon parallelism.

## Features

- **Sub-microsecond queries**: 37ns to 500us depending on depth
- **9-level hierarchy**: From identity (D0) to raw bytes (D8)
- **3D spatial indexing**: Content-based deterministic positioning
- **Zero-copy mmap**: Direct memory access, no serialization overhead
- **Hybrid search**: L2 distance + keyword matching + semantic embeddings
- **Fixed viewport**: 256 bytes per block at every zoom level
- **Parallel build**: Rayon-based multi-threaded index construction
- **SSE4 SIMD**: Hardware-accelerated L2 distance on x86_64

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
- Python 3.8+ with NumPy (optional, for Python implementation)

### Build from source

```bash
git clone https://github.com/mater-robert/microscope-memory.git
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
# Build binary index from memory layer JSON files (configured in config.toml)
microscope-memory build
```

### Recall — natural language query

```bash
# Auto-zoom based on query complexity
microscope-memory recall "What is Ora?" 10
```

The auto-zoom selects depth based on query length:
- 1-2 words → D0-D2 (identity/summaries)
- 3-5 words → D1-D3 (topic clusters)
- 6-10 words → D2-D4 (individual memories)
- 11-20 words → D3-D5 (sentences)
- 20+ words → D4-D6 (tokens)

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
# Cosine similarity search (mock embeddings for now)
microscope-memory embed "quantum physics" 10

# Alternative metrics: l2, dot
microscope-memory embed "quantum physics" 10 --metric l2
```

### Rebuild index

```bash
# Rebuild from layers + merge append log
microscope-memory rebuild
```

### Stats and benchmark

```bash
microscope-memory stats
microscope-memory bench
```

## Architecture

### Binary Structure

```
microscope.bin  — Block headers (mmap'd)
├── BlockHeader[0..n]: 32 bytes each
│   ├── x, y, z: f32        (3D spatial position)
│   ├── zoom: f32            (normalized depth: depth/8.0)
│   ├── depth: u8            (0-8)
│   ├── layer_id: u8         (memory layer index)
│   ├── data_offset: u32     (byte offset into data.bin)
│   ├── data_len: u16        (actual text bytes, <= 256)
│   ├── parent_idx: u32      (parent block index)
│   ├── child_count: u16     (number of children)
│   └── _pad: [u8; 2]        (alignment to 32 bytes)

data.bin        — Raw UTF-8 text content

meta.bin        — Index metadata (88 bytes)
├── magic: "MSCM"
├── version: u32
├── block_count: u32
├── depth_count: u32
└── depth_ranges: 9 x (start: u32, count: u32)
```

### Memory Layers

The system integrates 9 cognitive layers:

| Layer | Description | 3D Region |
|-------|-------------|-----------|
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

1. **Content → Position**: Text is FNV-hashed to deterministic 3D coordinates, offset by layer
2. **Hierarchical decomposition**: Each memory decomposes into sentences → tokens → syllables → characters → bytes
3. **Parallel build**: Depths 4-8 are constructed with rayon parallel iterators
4. **Spatial queries**: L2 distance in 3D space + zoom level filtering
5. **mmap access**: Zero-copy reads directly from memory-mapped binary files
6. **Hybrid ranking**: Vector distance + keyword boosting
7. **Append log**: New memories stored instantly via binary append, merged on rebuild

## Source Structure

```
src/
├── main.rs          — Core engine: build, query, mmap reader, CLI
├── streaming.rs     — Real-time streaming update server
├── embeddings.rs    — Semantic vector search (SIMD cosine similarity)
├── gpu.rs           — wgpu GPU acceleration (optional)
├── python.rs        — PyO3 Python bindings (optional)
├── wasm.rs          — WASM target (optional)
└── shaders/         — GPU compute shaders
```

## Optional Features

```bash
# WASM build
cargo build --release --features wasm --target wasm32-unknown-unknown

# Python bindings
cargo build --release --features python

# GPU acceleration
cargo build --release --features gpu
```

## Python Implementation

Alternative implementation in `build_blocks.py`:

```bash
python build_blocks.py
```

## Contributing

Contributions welcome. Areas of interest:

- [ ] GPU acceleration for vector search
- [ ] Distributed index sharding
- [ ] Real-time index updates
- [ ] Visualization tools
- [ ] Alternative distance metrics
- [ ] Compression algorithms
- [ ] Real embedding providers (OpenAI, HuggingFace)

## License

MIT License - See [LICENSE](LICENSE) file for details.

---

*"Below the byte level, only corruption exists — the atomic boundary of information."*
