# Microscope Memory v0.8.0 "Cognitive Evolution"

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![API v1](https://img.shields.io/badge/API-v1.0-cyan.svg)](#-spine-bridge-api-v1)

```
    ◇
  ◇    ◇      a semmiből teremtődik a mindenség
◇          ◇
  ◇    ◇
    ◇
```

**Microscope Memory** is a hierarchical cognitive memory engine designed for AI agents.  
Sub-microsecond binary memory retrieval with a living ecosystem of cognitive modules.

---

## New in v0.8.0 — Cognitive Modules

### 🧬 Morphogenesis
Biological pattern-inspired architecture generation:
- **Mycelium** — fungal network growth for P2P topologies
- **Capillary** — fractal branching for hierarchical cache/dataflow
- **Slime Mold** — Physarum-inspired optimal route finding
- **Fractal L-System** — self-similar structure cultivation
- **Evolutionary engine** — genetic algorithm over growth parameters

```bash
microscope-mem morph --grow "api" --pattern mycelium
microscope-mem morph --evolve 10 --objective latency
microscope-mem morph --daemon --interval 5 --threshold 0.5
```

### 🔍 Pattern Recognition
Multi-domain pattern detection:
- **Sequence** — recurring thought/recall pathways
- **Temporal** — daily/weekly activity rhythms
- **Structural** — graph motif detection in architectures
- **Cluster** — DBSCAN-based spatial grouping in memory space
- **Cross-domain** — pattern correlation across layers

### 🧠 Executive
Cognitive conductor — module scheduling, resource allocation, homeostasis.

### 🎯 Planning
Goal decomposition (HTN) and action planning:
- Cél → részcélok → akcióterv
- Erőforrás becslés és kockázat számítás
- Replanning változott körülmények esetén

### 🔄 Autopoiesis
Self-modifying code system:
- Template-based code generation
- Versioned mutations with rollback
- Integration with planning → automated fixes

### 💬 ChatGPT Import + PWA
Import conversations from ChatGPT exports:
```bash
microscope-mem import-chat-gpt conversations.json --dry-run
microscope-mem import-chat-gpt --gdrive <shared-url>
microscope-mem import-chat-gpt --gdrive-folder <folder-url>
```

**PWA Chat** — installable web app:
```bash
microscope-mem serve --port 8080
# Open http://localhost:8080/chat.html
# Access from phone on same WiFi
```

---

## ⚡ Core Engine

- **Sub-microsecond retrieval** — direct `mmap` binary frames, zero-JSON hot path
- **9-depth cognitive hierarchy** (D0-D8): identity, long-term, short-term, associative, emotional, relational, reflections, crypto-chain, echo cache
- **21D emotion vectors** — every memory carries emotional context
- **Merkle Tree integrity** + CRC16 per-block validation
- **Atomic append-log** — crash-proof persistence

---

## 🔬 Cognitive Features

| Feature | Module | Description |
|---------|--------|-------------|
| Hebbian Learning | `hebbian.rs` | Co-activation → spatial drift |
| Neuroplasticity | `neuroplasticity.rs` | Synaptic strengthening/pruning |
| Hippocampus | `hippocampus.rs` | Episodic binding and consolidation |
| Working Memory | `working_memory.rs` | 7±2 buffer with temporal decay |
| Attention | `attention.rs` | Layer weight modulation |
| Dream | `dream.rs` | Offline memory replay and pruning |
| Emotional Contagion | `emotional_contagion.rs` | Cross-instance emotion sharing |
| Resonance | `resonance.rs` | Federated pulse synchronization |
| Hyperfocus | `hyperfocus.rs` | Deep focus mode |
| Mental Sandbox | `mental_sandbox.rs` | Pre-action scenario simulation |

---

## 📊 Benchmarks

| System | Query Type | Avg Latency | Index Size |
|--------|-----------|-------------|------------|
| **Microscope Memory** | Exact spatial recall | **87 µs** | 722 KB |
| FAISS (flat IP) | Approximate k-NN | ~1-5 ms | ~10-50 MB |
| Pinecone | Approximate vector search | ~5-20 ms | hosted |
| ChromaDB | Approximate vector search | ~5-50 ms | ~10-100 MB |

Microscope uses hierarchical spatial indexing (D0-D8), not approximate vector search.  
It trades semantic fuzziness for deterministic, sub-millisecond exact recall.  
See [BENCHMARKS.md](BENCHMARKS.md) for full data.

---

## 🤖 Spine Bridge API v1

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/v1/status` | Engine health & stats |
| `GET` | `/v1/recall?q=...&k=10` | Semantic/spatial recall |
| `POST` | `/v1/remember` | Store a memory |
| `POST` | `/v1/mobile/chat` | User-scoped mobile chat |

```python
import requests
res = requests.get("http://localhost:6060/v1/recall", params={"q": "User preference", "k": 3})
print(res.json())
```

---

## 🏗️ Quick Start

```bash
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory
cargo build --release

# Launch HTTP server with PWA chat
./target/release/microscope-mem serve --port 8080

# Or start the Bridge API
./target/release/microscope-mem bridge --port 6060
```

---

## License

MIT — see [LICENSE](LICENSE)

---

```
    ◇
  ◇    ◇      a semmiből teremtődik a mindenség
◇          ◇
  ◇    ◇
    ◇
```

*Designed by Máté Róbert*  
*The Silent Noise Research Series*
