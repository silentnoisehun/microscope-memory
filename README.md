# 🔬 Microscope Memory

**Zoom-based hierarchical memory system with sub-microsecond queries**

A revolutionary approach to memory indexing that treats data like looking through a microscope — the zoom level determines what you see. Pure binary, zero JSON, powered by mmap and L2 vector search.

## ✨ Features

- **⚡ Sub-microsecond queries**: 37ns to 500μs depending on depth
- **🎯 9-level hierarchy**: From identity (D0) to raw bytes (D8)
- **📍 3D spatial indexing**: Content-based deterministic positioning
- **💾 Zero-copy mmap**: Direct memory access, no serialization overhead
- **🔍 Hybrid search**: L2 distance + keyword matching
- **📦 Fixed viewport**: 256 chars per block at every zoom level

## 🚀 Performance

Benchmarked on 227,168 blocks (10,000 queries per depth):

| Depth | Blocks | Query Time | Description |
|-------|--------|------------|-------------|
| D0 | 1 | **37 ns** | Identity |
| D1 | 9 | **92 ns** | Layer summaries |
| D2 | 108 | **506 ns** | Topic clusters |
| D3 | 523 | **1.7 μs** | Individual memories |
| D4 | 1,349 | **3.9 μs** | Sentences |
| D5 | 6,070 | **18 μs** | Tokens |
| D6 | 26,198 | **72 μs** | Syllables |
| D7 | 96,297 | **505 μs** | Characters |
| D8 | 96,613 | **492 μs** | Raw bytes |

## 📖 Philosophy

This project explores a unique memory model inspired by microscopy:

- **Fixed viewport size**: Every block is exactly 256 characters
- **Zoom determines detail**: Not what data exists, but what you can see
- **Spatial coherence**: Similar content clusters in 3D space
- **Atomic boundary**: Below D8 (bytes), data corrupts — a philosophical limit

## 🛠️ Installation

### Prerequisites

- Rust 1.70+ (for the core engine)
- Python 3.8+ with NumPy (optional, for Python implementation)

### Build from source

```bash
# Clone the repository
git clone https://github.com/yourusername/microscope-memory.git
cd microscope-memory

# Build the Rust binary
cargo build --release

# Run tests
cargo test
```

## 📚 Usage

### Build the index

```bash
# Build binary index from memory layers
cargo run --release -- build
```

### Query with natural language

```bash
# Auto-zoom based on query complexity
cargo run --release -- recall "What is Ora?" 10

# Returns top 10 results with automatic depth selection
```

### Manual microscope control

```bash
# Look at specific coordinates (x, y, z) at zoom level 3
cargo run --release -- look 0.25 0.25 0.25 3

# 4D soft zoom (zoom as weighted dimension)
cargo run --release -- soft 0.15 0.15 0.15 4
```

### Store new memories

```bash
# Add to long-term memory with importance 8
cargo run --release -- store "Important insight about the project" long_term 8
```

### Performance benchmark

```bash
cargo run --release -- bench
```

## 🏗️ Architecture

### Binary Structure

```
microscope.bin  — Headers (7.1 MB, mmap'd)
├── BlockHeader[0..n]: 32 bytes each
│   ├── x, y, z: f32 (3D position)
│   ├── zoom: f32 (normalized depth)
│   ├── depth: u8 (0-8)
│   └── data_offset: u32 → data.bin

data.bin       — Text content (887 KB)
└── Raw UTF-8 text blocks

meta.bin       — Metadata (88 bytes)
└── Depth ranges for fast lookup
```

### Memory Layers

The system integrates 9 cognitive layers:

- `long_term` — Persistent knowledge
- `short_term` — Working memory
- `associative` — Concept connections
- `emotional` — Affective associations
- `relational` — Entity relationships
- `reflections` — Meta-thoughts
- `crypto_chain` — Cryptographic memories
- `echo_cache` — Response history
- `rust_state` — System state

## 🔬 How It Works

1. **Content → Position**: Text is hashed to deterministic 3D coordinates
2. **Hierarchical decomposition**: Each memory breaks down into sentences → tokens → syllables → characters → bytes
3. **Spatial queries**: L2 distance in 3D space + zoom level filtering
4. **mmap access**: Zero-copy reads directly from mapped memory
5. **Hybrid ranking**: Vector distance + keyword boosting

## 🐍 Python Implementation

Alternative implementation in `build_blocks.py`:

```python
# Build with Python (NumPy-based)
python build_blocks.py

# Creates JSON export for analysis
# Supports visualization and debugging
```

## 📊 Benchmarks

```bash
# Full benchmark suite
cargo bench

# Results on typical hardware:
# - Index build: ~2 seconds for 500+ memories
# - Query latency: 37ns - 500μs
# - Memory usage: ~8 MB total
# - Cache efficiency: L1d/L2 for depths 0-4
```

## 🧪 Examples

### Find memories about a topic
```rust
// Searches across all depths with auto-zoom
microscope-memory recall "quantum physics" 5
```

### Explore at different zoom levels
```rust
// Zoom 0: See entire identity
// Zoom 3: See individual memories
// Zoom 8: See raw bytes
microscope-memory look 0.5 0.5 0.5 [0-8]
```

### Build custom memory layers
```rust
// Add your own JSON files to layers/
// Supports any structure with "content" fields
microscope-memory build
```

## 🤝 Contributing

Contributions are welcome! Areas of interest:

- [ ] GPU acceleration for vector search
- [ ] Distributed index sharding
- [ ] Real-time index updates
- [ ] Visualization tools
- [ ] Alternative distance metrics
- [ ] Compression algorithms

## 📄 License

MIT License - See [LICENSE](LICENSE) file for details

## 🙏 Acknowledgments

- Inspired by hierarchical data structures and vector databases
- Built with Rust for performance and safety
- Uses memory-mapped I/O for zero-copy access
- Explores the boundary between structure and chaos

## 📮 Contact

Máté Róbert (Silent) - [GitHub](https://github.com/yourusername)

Project Link: [https://github.com/yourusername/microscope-memory](https://github.com/yourusername/microscope-memory)

---

*"Below the byte level, only corruption exists — the atomic boundary of information."*