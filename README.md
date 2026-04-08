# Microscope Memory

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/microscope-memory.svg)](https://crates.io/crates/microscope-memory)
[![Zero-JSON](https://img.shields.io/badge/Architecture-Zero--JSON-green.svg)](#core-pillars)
[![LLM Bridge](https://img.shields.io/badge/LLM-Bridge%20API-purple.svg)](#-spine-bridge-api--llm-integration)

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

## 🤖 Spine Bridge API — LLM Integration

Microscope Memory includes a **REST API bridge** that connects AI models (ChatGPT, Claude, any OpenAI-compatible agent) directly to the cognitive memory engine.

```bash
# Start the Bridge API (default port 6060)
./target/release/microscope-mem bridge

# Custom host/port
./target/release/microscope-mem bridge --host 0.0.0.0 --port 8888
```

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/status` | Engine health & stats |
| `GET` | `/recall?q=...&k=10` | Recall by natural language query |
| `POST` | `/remember` | Store a new memory |
| `GET` | `/openapi.json` | OpenAPI spec for ChatGPT/Claude import |
| `GET` | `/` | Interactive API documentation |

### Connect ChatGPT / Claude

1. Start the bridge: `microscope-mem bridge`
2. **ChatGPT Custom GPT** → Actions → Import from URL: `http://YOUR_IP:6060/openapi.json`
3. **Claude Projects** → Integrations → Add tool → paste the same URL

```bash
# Quick test
curl "http://localhost:6060/recall?q=What+is+Hebbian+learning&k=3"
curl -X POST http://localhost:6060/remember \
  -H "Content-Type: application/json" \
  -d '{"text": "Hebbian learning: neurons that fire together wire together", "layer": "long_term", "importance": 9}'
```

## 🐳 Docker Support

Run Microscope Memory in a container:

```bash
docker build -t microscope-mem .
docker run -it microscope-mem init-demo
docker run -it microscope-mem build
docker run -p 6060:6060 microscope-mem bridge
```

## 📂 Integration

### Python API (PyO3)
Microscope Memory provides high-performance Python bindings.

```python
import microscope_memory as mm

# Initialize in-memory engine
engine = mm.PyMicroscope()
engine.add_block("Memory text", x=0.1, y=0.2, z=0.3, depth=3, layer_id=1)

# Hybrid search (semantic + spatial)
results = engine.hybrid_search("query", x=0.1, y=0.2, z=0.3, 
                               semantic_weight=0.5, spatial_weight=0.5, k=5)
```
*Note: The Python API is currently synchronous. Large index operations should be handled in background threads to avoid GIL contention.*

### WebAssembly (WASM)
Run the cognitive engine directly in the browser. 

- **mmap Fallback**: Since browsers do not support `mmap`, the WASM module falls back to in-memory `Vec<u8>` buffers (ArrayBuffer).
- **Efficiency**: Near-native performance for 3D spatial queries on mobile and desktop browsers.

```javascript
import init, { MicroscopeWasm } from './pkg/microscope_memory.js';

async function run() {
    await init();
    const mm = new MicroscopeWasm();
    // Load binary buffers directly from URL or IndexedDB
    mm.load_binary(metaBuf, microscopeBuf, dataBuf);
    const results = mm.recall("What is the Spine?", 5);
}
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
