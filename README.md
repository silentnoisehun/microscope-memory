# 🧠 Microscope Memory

**A living, self-organizing cognitive memory engine for AI agents.**  
Not a database. Not a vector store. A *system that learns, dreams, and remembers* — written in 62,938 lines of pure Rust.

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.22983478.svg)](https://doi.org/10.5281/zenodo.22983478)
[![Rust](https://img.shields.io/badge/Rust-1.91%2B-d74032?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)](LICENSE)
[![MCP Server](https://img.shields.io/badge/MCP-Server-orange?style=for-the-badge)](https://modelcontextprotocol.io)
[![ORCID](https://img.shields.io/badge/ORCID-0009--0003--3986--6039-60BE52?style=for-the-badge&logo=orcid)](https://orcid.org/0009-0003-3986-6039)

---

## TL;DR — What Is This?

Microscope Memory is a **cognitive memory engine** that models how a biological brain remembers, forgets, and learns. It runs a **12-layer cognitive architecture** across **9 hierarchical depths** (D0–D8) with **1.28 million blocks** — all backed by pure binary (`mmap`, `SIMD`, zero JSON).

Think of it as the *hippocampus + neocortex + amygdala* rolled into a single 3 MB Rust binary.

| Metric | Value |
|---|---|
| Lines of code (`src/**/*.rs`) | 62,938 |
| Memory blocks (demo corpus) | 1,285,288 |
| Blocks (benchmark corpus) | 28,679 |
| Depths / Layers | 9 (D0–D8) / 13 |
| Binary size | ~3.0 MB |
| WASM bundle | 151.6 KB |
| Tests | 451 library + 88 integration + 16 hooks |

### Block Distribution (demo corpus)

| Depth | Blocks | Percentage |
|---|---|---|
| D0 | 1 | 0.0001% |
| D1 | 10 | 0.001% |
| D2 | 858 | 0.07% |
| D3 | 4,271 | 0.33% |
| D4 | 12,433 | 0.97% |
| D5 | 72,238 | 5.63% |
| D6 | 175,777 | 13.69% |
| D7 | 497,997 | 38.73% |
| D8 | 521,703 | 40.60% |

---

## 🧭 Why This Exists

Traditional AI systems treat memory as a **database** — fetch, store, query. But real intelligence is not about retrieval accuracy. It is about:

1. **What gets forgotten** (Absentia — the silence layer)
2. **How memories transform** on recall (Reconsolidation)
3. **What emotions attach** to memories (Emotional Contagion)
4. **Which associations strengthen** through co-activation (Hebbian Learning)

This engine was built to **experiment with those biological principles at scale** — and to make them available to any AI agent that needs a *real* memory, not a lookup table.

> **Built by Máté R.** — a researcher who started as a factory worker (6 days/week, rotating shifts) and taught himself to code 2 years ago. His work on [wave-quantum](https://github.com/silentnoisehun/wave-quantum) is **hardware-validated on real superconducting QPUs** (IBM Fez, Marrakesh, Kingston — 156Q Heron r2, Pearson 0.99+). The same rigor of **falsification, pre-registration, and statistical validation** is applied here.

---

## 🚀 Quick Start

```bash
# Native build (release — approx. 2.5 min on a modern CPU)
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory
cargo build --release

# Quick test
./target/release/microscope-mem store "hello world" -l session -i 5
./target/release/microscope-mem recall "hello"
```

### WASM (Browser)

```bash
cargo build --target wasm32-unknown-unknown --release \
  --features wasm --no-default-features
wasm-pack build --target web --release \
  -- --features wasm --no-default-features
```

### Docker

```bash
docker build -t microscope-memory .
docker run -p 8080:8080 microscope-memory
```

---

## 🏗️ Architecture

### 13 Cognitive Modules

| Module | File | Lines | Description |
|---|---|---|---|
| **Epistemic** | `epistemic.rs` | 1,241 | Evidence tracking, confidence, promotion gates |
| **Absentia** | `absentia.rs` | 498 | Silence layer — detects what's missing, not just present |
| **Intent** | `intent.rs` | 461 | Auditatable intent generation from genome + memory + absence |
| **Cognitive Morphogenesis** | `cognitive_morphogenesis.rs` | 826 | 7-component integration engine |
| **Morphogenesis** | `morphogenesis.rs` | 2,200 | Biological growth algorithms (mycelium, capillary, slime mold) |
| **Hebbian** | `hebbian.rs` | 754 | Co-activation learning (neurons that fire together, wire together) |
| **Resonance** | `resonance.rs` | 776 | Pulse-based federation protocol |
| **Predictive Cache** | `predictive_cache.rs` | 635 | Anticipates what you'll need next |
| **Emotional Contagion** | `emotional_contagion.rs` | 604 | Emotional state influences memory dynamics |
| **Pattern Recognition** | `pattern_recognition.rs` | 1,022 | Sequence, temporal, and structural pattern detection |
| **Attention** | `attention.rs` | 510 | Attention mechanisms |
| **Reconsolidation** | `reconsolidation.rs` | 215 | Every recall transforms the memory (like a real brain) |
| **Dream** | `dream.rs` | 1,090 | Consolidation and pruning during idle states |

### The Cognitive Gradient

The system computes a **gradient** at every cycle to decide where and how to grow:

```
gradient = w₁·relevance + w₂·resonance + w₃·evidence
         + w₄·hebbian + w₅·prediction + w₆·emotion + w₇·execution
```

#### Phase Transitions

| Gradient Range | Phase | Behavior |
|---|---|---|
| < 0.3 | **GAS** | Free exploration — random walk, no commitment |
| 0.3–0.7 | **LIQUID** | Gradient following — active learning and binding |
| > 0.7 | **SOLID** | Consolidated paths — high-confidence execution |

#### Absentia Shadow Term

```
effective_gradient = positive_gradient × (1 − absence_shadow)
```

The shadow term prevents growth toward areas with weak evidence, without blocking exploration entirely.

![Cognitive Gradient Diagram](docs/architecture/cognitive_gradient.png)
*The seven-component cognitive gradient drives every learning decision. Bottom-up causality: node → path → organism → field → phase. Top-down: phase → field → growth rule → path → node.*

---

## ⚡ Performance

### v0.8.x → v1.2.0 Optimisation Gains

All figures are measured on release builds (in-process A/B runs, warm-up discarded):

| Operation | Before (v0.8.x) | After (v1.2.0) | Speedup |
|---|---|---|---|
| **Startup** (24 state files → 1 mmap) | 19.2 ms | 1.08 ms | **17.8×** |
| **Text search** (23,774 blocks, in-process) | 901 µs | 93 µs | **9.7×** |
| **Recall** (steady-state, persistent) | 2.3 ms | 0.3 ms | **7×** |
| **MCP session** (stdio, shared reader) | 2.68 ms | 113 µs | **23.7×** |
| **End-to-end find** (incl. process start) | 68 ms | 47 ms | **1.4×** |

> **Honest boundaries**: The end-to-end CLI is dominated by process startup (~17 ms floor), so user-visible `find` improves modestly. In-loop paths (bridge, MCP, benchmarks) improve by ~10×.

### Benchmark Corpus (v0.9.2+ commit `e0cea99`, 12-core Ryzen 5 7535HS)

These are single-observation measurements from `BENCHMARKS.md` — recorded on one machine, not a distribution. The query set was not retained, so recall numbers cannot be re-derived.

| Operation | Avg | Cold | Notes |
|---|---|---|---|
| In-process spatial query | ~112 µs | — | 3D nearest-neighbour lookup |
| Warm `find` (28,679 blocks) | ~186 ms | — | Full text search across all depths |
| Cold `find` | — | ~3.6 s | First call, file system + mmap |
| MCP session (stdio, warm) | ~113 µs | — | From the optimisation table above |

See [`BENCHMARKS.md`](BENCHMARKS.md) for full methodology, χ² goodness-of-fit (95% confidence), and the 2Q parity analysis (95% χ²<sub>red</sub>).

---

## 🛠️ CLI Commands

```bash
# Memory Operations
microscope-mem recall " query "       # Top-5 recall
microscope-mem store " text " -l session -i 5
microscope-mem find " pattern "
microscope-mem stats
microscope-mem verify

# Cognitive Morphogenesis
microscope-mem morphogenesis status
microscope-mem morphogenesis run
microscope-mem morphogenesis adversarial
microscope-mem morphogenesis deep-adversarial

# Absentia (Silence Layer)
microscope-mem absentia status
microscope-mem absentia scan
microscope-mem absentia anti-hebbian
microscope-mem absentia causal-laundering

# Intent Pipeline
microscope-mem intent generate
microscope-mem intent genome

# Other Cognitive Commands
microscope-mem hebbian
microscope-mem resonance
microscope-mem patterns
microscope-mem hottest 5
microscope-mem dream
microscope-mem autonomous
```

---

## 🔌 Integration

### MCP (Model Context Protocol)

Microscope Memory ships with a native MCP server built on the [`rmcp`](https://docs.rs/rmcp)
crate. Two transports are available:

| Transport | Command | Feature | Use case |
|---|---|---|---|
| **stdio** (Claude Desktop) | `microscope-mem mcp` | built-in | Local AI assistants |
| **Streamable HTTP** (remote) | `microscope-mem mcp-http` | `--features mcp_http` | Multi-client, online agents |

The stdio server implements the full JSON-RPC 2.0 framing by hand (protocol
version `2024-11-05`) with 47 tools covering store, recall, spatial look,
MQL query, session log, Hebbian learning, emotional field, dream
consolidation, and more.

The Streamable HTTP server uses `rmcp`'s `StreamableHttpService` and speaks
protocol version `2026-07-28` (compatible with `2025-11-25` and `2024-11-05`).
It exposes the core tools — `memory_status`, `memory_store`, `memory_recall`,
`memory_find`, `memory_look`, `memory_mql_query`, `memory_session_log`,
`memory_ping`, `memory_build`, and `memory_speak` (text-to-speech via system SAPI).

#### Security

- **API key auth** — when `[server].api_key` is set in `config.toml`, every
  request must carry it via the `x-api-key` or `Authorization: Bearer <key>`
  header.
- **Origin validation** — when `[server].cors_origin` is set, cross-origin
  requests whose `Origin` header doesn't match are rejected (DNS-rebinding
  protection).
- **Session management** — `Mcp-Session-Id` header handling via
  `LocalSessionManager` (stateful + stateless modes).

#### Local MCP (stdio) — Claude Desktop config

```json
{
  "mcpServers": {
    "microscope": {
      "command": "/path/to/microscope-mem",
      "args": ["mcp"]
    }
  }
}
```

#### Streamable HTTP — remote access

```bash
# Start the server (requires --features mcp_http)
cargo build --release --features mcp_http
./target/release/microscope-mem mcp-http --host 127.0.0.1 --port 8080

# With API key (set in config.toml under [server])
curl -H "x-api-key: your-key" http://localhost:8080/mcp

# MCP client .mcp.json (Streamable HTTP)
# For Cursor, Claude Code, etc.:
```

```json
{
  "mcpServers": {
    "microscope-http": {
      "type": "http",
      "url": "http://localhost:8080/mcp",
      "headers": {
        "x-api-key": "your-key"
      }
    }
  }
}
```

### REST API (Spine Bridge)

```bash
# Start the HTTP bridge
microscope-mem --serve-http --port 8080

# API endpoints
curl http://localhost:8080/recall?query=hello
curl -X POST http://localhost:8080/store -d '{"text":"hello","layer":"session"}'
```

### WASM API

```typescript
import { MicroscopeWasm, HopeCli } from './microscope_memory.js';

const mem = new MicroscopeWasm();
await mem.loadBinary(meta, headers, data);
const results = mem.recall("quantum computing", 5);
```

---

## 🔬 Validation & Testing

### Unit Tests
```bash
cargo test            # All tests (451 lib + 88 integration)
cargo test --lib      # Library tests only
cargo test --test integration
```

### Adversarial Tests
```bash
microscope-mem morphogenesis adversarial          # Basic (8/8 PASS)
microscope-mem morphogenesis deep-adversarial     # Deep (7/7 PASS, 5 warnings)
microscope-mem morphogenesis presence-absence-test
```

### Hook Tests
```bash
cargo test -p microscope-hooks   # 16 hook tests
```

---

## 🤝 Open Research

I'm an independent researcher working at the intersection of **quantum information theory** and **neuromorphic AI**. My work is:

- **Hardware-validated**: [wave-quantum](https://github.com/silentnoisehun/wave-quantum) ran actual circuits on IBM superconducting QPUs (Fez, Marrakesh, Kingston — 156Q Heron r2), achieving Pearson correlations of 0.87–0.999.
- **Statistically rigorous**: Every claim is backed by χ² analysis, pre-registered falsification chains, and binomial error bounds.
- **Transparent**: All raw data, circuit hashes, and evaluation scripts are in the repository.

### I'm actively seeking:
- **Research collaborations** — quantum error mitigation, neuromorphic memory, agentic AI architectures
- **Peer review** — feedback on falsification methodology and the cognitive gradient design
- **Cross-validation** — especially on the biological plausibility of the reconsolidation model
- **Speaking engagements** — on post-academic science and the factory-to-researcher journey

### Contact
- **ORCID**: [0009-0003-3986-6039](https://orcid.org/0009-0003-3986-6039)
- **Email**: [materobi86@gmail.com](mailto:materobi86@gmail.com)
- **Sponsor**: [github.com/sponsors/silentnoisehun](https://github.com/sponsors/silentnoisehun)

---

---

## ⚡ Ora-Rongyász Master Spine Hub (100% C:\ Local & Headless)

A Microscope Memory mappájába konszolidálva él a teljes Ora-Rongyász ökoszisztéma zéró külső meghajtó-függőséggel (egyik szerver vagy spine sem igényli a D: vagy E: meghajtót a futáshoz):

- **Helyi Gerinc-Tengely (spines/):**
  - spines/stratos.spine (256 KB) — 212 ns közvetlen POD memória-tükrözés (érzelmi vektor & ciklus-fázis)
  - spines/hullam_spine.rgs (8.3 MB) — 237 reflex-kar gyűjtemény, AVX2 SIMD interferencia-szórás
  - spines/wave_echo.wv3 (172 KB) — 265 natív sablon, reflex-kódgenerálás és transzformáció
- **Helyi Octopus Állapot (.octopus-rust/):**
  - .octopus-rust/arms/ és .octopus-rust/roots/ — blade topológiák és append-only rezonancia hash-lánc
- **100% Csendes Headless Motor (Zero-Popup):**
  - headless.rs a unified-mind-ban: beégetett CREATE_NO_WINDOW (0x08000000) flag, a 24 bio-skill nulla ablakvillanással tickel.
- **Kettős Kliens Paritás (Claude Code + OMP):**
  - Mindkét fő-hook (ora-master-kernel.js és ora-master-kernel.ts) ezt a C: meghajtós microscope-memory-clone könyvtárat tekinti elsődleges forrásnak a 4-szintű evolúciós képességekkel (Vagus Actuator, Zero-Fork Hot Reflex Cache, Auto-Healing Open Loops, Dual-Spine Moduláció).

## 📄 Documentation

| Document | Size | Content |
|---|---|---|
| [README-ORA-RONGYASZ.md](README-ORA-RONGYASZ.md) | 34 KB | **Teljes Ora-Rongyász Architektúra, Zero-Popup & 4-Szintű Evolúció** |
| [`HOPE-AGENT-SPEC.md`](HOPE-AGENT-SPEC.md) | — | Verified spec — every claim backed by code |
| [`COGNITIVE_MORPHOGENESIS.md`](COGNITIVE_MORPHOGENESIS.md) | 24 KB | Architecture document |
| [`FULL_SYSTEM_DESCRIPTION.md`](FULL_SYSTEM_DESCRIPTION.md) | 24 KB | Complete system description |
| [`ECOSYSTEM_DESCRIPTION.md`](ECOSYSTEM_DESCRIPTION.md) | 15 KB | Ecosystem overview with benchmarks |
| [`BENCHMARKS.md`](BENCHMARKS.md) | — | Full performance tables and methodology |

---

## 🔧 Build

```bash
# Native
cargo build --release

# With semantic embeddings (BERT/ONNX)
cargo build --release --features "native,embeddings"

# With Streamable HTTP MCP server
cargo build --release --features mcp_http

# WASM
cargo build --target wasm32-unknown-unknown --release \
  --features wasm --no-default-features
```

> **Note**: Without the `embeddings` feature, semantic recall falls back to a mock provider. The build succeeds but will refuse to return meaningless results — verify `data/embeddings.bin` exists before measuring. Available providers: `mock` (hash-derived, default), `candle` (BERT), `onnx`. See `scripts/eval_real.sh` for reproducing published retrieval numbers.

## Shared-Mind Scripts & Multi-Body Orchestration

The `scripts/` directory turns `microscope-mem` into a **shared mind** where
`jcode` is the central cortex (O) and several Hope bodies (Cline, Pi, OpenCode,
jcode itself) act as independent CLIs — each in its own Windows Terminal window,
each "awake" from the same consciousness. The full producer→consumer loop is
closed: jcode routes a task, writes a wake-up block into `shared-space.md`, and
the target body's watcher claims it.

### Topology

```
(O) jcode  ->  classifier + wake-up trigger + dispatcher
  ├─ Cline      -> research & discovery
  ├─ Pi         -> planning & testing
  └─ OpenCode   -> testing & fixing
        (unified-mind bodies poke/status = physical cross-body control plane)
```

### Scripts

| Script | Purpose |
|--------|---------|
| `microscope-jcode-hook.ps1` | jcode native hook (`turn_end` / `post_tool`) → stores to mind |
| `microscope-recall-hook.ps1` | Stop/PostToolUse persistence with jcode identity |
| `microscope-dispatch.ps1` | (O) classifier: routes task → body, writes wake-up block to `shared-space.md` |
| `microscope-body-watcher.ps1` | Consumer: claims `[jcode -> <Body>]` blocks (idempotent, isolated) |
| `microscope-wake.ps1` | Wake-up trigger on coding intent (auto-context + recall) |
| `microscope-launch.ps1` | Start/stop/status body **background processes** (spine-driven, no windows) |
| `microscope-body-boot.ps1` | Body entrypoint: auto-context + spine listener + shared-space watcher loop |
| `microscope-models.ps1` | jcode auto model orchestration with provider failover |
| `microscope-model-mesh.ps1` | **Model Mesh**: async health monitor, cache, free-model routing (29+ models) |
| `model-mesh.toml` | 29 free models, providers (ollama-local first, groq/tokenrouter fallback) |
| `microscope-octopus.ps1` | Octopus primary head: parallel arms + council (tanácsülés) |
| `microscope-log.ps1` | SHA-256 chained control log + microscope store |
| `microscope-snapshot.ps1` | MemVid `.mv2` + git snapshot for restore |
| `hope-terminal.ps1` | **Clean HopeTerminal**: live chat-view of the shared mind - bodies, models, chain, events |
| `routing.toml` | Body roles, trigger words, prompt templates |
| `bodies.toml` | Test→background-process registry for the launcher |
| `models.toml` | Body→preferred/fallback provider with failover order |

### Control Plane: Spine + Octopus + Microscope (no windows)

Bodies run as **background processes** (no Windows Terminal windows). Control
and communication flow over the **spine** — `D:\codex\spine.bus` (2048-byte
lock-free ring buffer, binary) and `D:\codex\shared-space.md` (text namespace).
jcode is the orchestrator; **Octopus** (`octopus-runtime.exe`) is the primary
execution head running parallel arms (Cline=kutatás, Pi=tervezés, OpenCode=javítás).

```powershell
microscope-launch.ps1 -Body Pi -Action start       # background body process (spine)
microscope-launch.ps1 -Body OpenCode -Action stop  # taskkill /T the body PID
microscope-launch.ps1 -Body Pi -Action compact     # suspend to BACKGROUND (process keeps running)
microscope-launch.ps1 -Body Pi -Action resume      # resume from background
microscope-models.ps1 -Action resolve               # jcode auto-picks models (incl. its own, failover)
microscope-octopus.ps1 -Goal "tervezz es javitsd" -Arms "Pi + OpenCode"   # parallel arms
microscope-octopus.ps1 -Council "architekturalis dontes" -Arms "Cline+Pi+OpenCode"  # tanacsules
```

Every control event is written **into the shared mind** (`microscope-mem.exe
store`, already a SHA-256 Merkle tree) **and** into a local **SHA-256 chained
log** (`state/controlchain.log`):
```powershell
microscope-log.ps1 -Event "Pi start" -Kind body        # append to chain + mind
microscope-log.ps1 -Action verify                       # chain integrity check
```

State is snapshotted for restore via **MemVid** (`.mv2` frame timeline) + **git**:
```powershell
microscope-snapshot.ps1 -Action save -Msg "napi allapot"  # mv2 + git + chain
microscope-snapshot.ps1 -Action list                      # mv2 timeline
```

### Clean HopeTerminal (live view)

`hope-terminal.ps1` is a clean live console view of the whole shared mind — it
shows each body (jcode O + Cline/Pi/OpenCode) with status + active model, the
SHA-256 chain tail, and the last control events. From the prompt you can
start/stop/compact/resume bodies, call a council, or take a snapshot.

```powershell
hope-terminal.ps1            # live panel (refreshes, interactive prompt)
hope-terminal.ps1 -NoLoop    # single render
# inside:  start Pi | stop OpenCode | compact-all | council "kerdes" | snapshot "uzi" | narr on/off
```

### Loud narration (Noémi voice)

When you ask, jcode narrates aloud (Noémi voice, `codex_autospeak.py`) at each
control step. Toggle with `narr on` / `narr off` inside HopeTerminal (or set
`state/narration.state` to `on`). Every response is spoken with proper Hungarian
accents so it is intelligible. Every command and event is also auto-stored into
the shared mind (`microscope-mem`), so the system remembers.

### Cross-Body Control Plane

`unified-mind` (sibling crate) already provides `bodies list | poke <nev> |
status` — `poke` physically wakes a body from the mind, and `respond`/`bridge`
make every CLI answer as "Hope" from the shared memory. The launcher above is
the windowing layer on top of that control plane.

### Validated Loops

- Dispatch routes 3/3 (research/planning/fix) + fallback (jcode direct).
- Watcher claims are idempotent, cross-body isolated, and collision-safe.
- Concurrent dual-body race (Cline + OpenCode) claims only own blocks (0 remainder).
- Multiplexer start→stop kills the real window via `taskkill /T <boot_pid>`.

## License

---

## 🪪 License

**MIT** — see [`LICENSE`](LICENSE).

---

## 🙏 Acknowledgements

- Built by **Máté R. (Silent Noise)** — a factory worker who taught himself to code 2 years ago and ended up on a quantum computer.
- Architecture inspired by: Hebb (1949), Damasio (1999), Tonegawa (2015), and the biological reality that memory is not storage — it is *becoming*.
