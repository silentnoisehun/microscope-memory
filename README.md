# Microscope Memory

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Zero-JSON](https://img.shields.io/badge/Architecture-Zero--JSON-green.svg)](#core-pillars)

**Microscope Memory** is a high-performance, hierarchical cognitive memory engine built for low-latency AI architectures. It operates on a "Zero-JSON" principle, utilizing memory-mapped binary blocks for sub-microsecond retrieval and associative learning.

## Core Pillars

- ⚡ **Sub-microsecond Latency**: Built on `memmap2`, achieving ~1.2ns raw read speeds and ~1.7µs complex hierarchical queries.
- 🧊 **Zero-JSON Architecture**: Strict prohibition of text-based parsers in the critical path. Data structures are packed into fixed 256-byte binary blocks.
- 🧠 **Hebbian Learning System**: Implements associative memory drift, allowing the hierarchy to reorganize based on activation patterns.
- 🏗️ **9-Depth Hierarchy**: Multi-scale data organization from Identity (D0) down to Raw Bytes (D8), enabling semantic "zooming".
- 🔐 **Merkle Integrity**: Integrated Merkle tree verification for deterministic hierarchy state validation.

## Performance Benchmarks

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Binary Block Read | 1.207 ns | 800M+ ops/s |
| Atomic Spine Write| 1.397 ns | 700M+ ops/s |
| Hierarchical Query| 1.742 µs | 500k+ ops/s |
| Neural Flow Tick  | 3.935 ns | 250M+ ops/s |

## 🚀 Quickstart (30 Seconds)

The fastest way to experience Microscope Memory is using the `init-demo` command:

```bash
# 1. Initialize demo dataset
./target/release/microscope-mem init-demo

# 2. Build the binary index
./target/release/microscope-mem build

# 3. Think and explore
./target/release/microscope-mem think "What is Hebbian feedback?"
```

## 🛠️ Installation

### Prerequisites
- Rust 1.75+
- LLVM/Clang (for SIMD optimizations)

### From Source
```bash
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory
cargo build --release
```

## 🎯 Use Cases

- 🧠 **Autonomous AI Agent Memory**: Persistent long-term storage for LLM agents that improves over time via Hebbian drift.
- ⚡ **High-Speed RAG Caching**: Sub-microsecond semantic retrieval for high-traffic RAG pipelines.
- 🔗 **Personal Knowledge Management (PKM)**: Associative note-taking and knowledge graph discovery.
- 🌐 **Federated Knowledge Networks**: Synchronized cognitive states across distributed edge nodes using the Resonance Protocol.

## 🐳 Docker Support

Run Microscope Memory in a container:

```bash
docker build -t microscope-mem .
docker run -it microscope-mem init-demo
docker run -it microscope-mem build
docker run -p 6060:6060 microscope-mem spine
```

## 📂 Examples

Explore the `examples/` directory for integration patterns:
- `python_quickstart.py`: Connect to the Binary Spine API using Python.

## Internal Architecture

The engine organizes data into a 9-depth fractal hierarchy:
- **D0**: System Identity / Global State
- **D1**: Layer Aggregates
- **D2**: Topic Clusters
- **D3-D5**: Associative Memories & Sentences
- **D6-D8**: Tokens, Characters, and Raw Bytes

Each block is a C-represented struct ensuring zero-copy alignment with the CPU cache lines.

## License
Distributed under the MIT License. See `LICENSE` for more information.

---
*Developed by [Máté Róbert](https://github.com/silentnoisehun) — Part of the autonomous cognitive research series.*
