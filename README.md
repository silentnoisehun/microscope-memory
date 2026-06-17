# ▓▒░ MICROSCOPE MEMORY ░▒▓
## ░▒▓ v0.8.0 — "Cognitive Evolution" ▓▒░

```
╔══════════════════════════════════════════════════════════════════════════════════╗
║                                                                                  ║
║   ████████╗██╗   ██╗███████╗███████╗██████╗ ██╗   ██╗ ██████╗  ██████╗ ███████╗  ║
║   ╚══██╔══╝╚██╗ ██╔╝██╔════╝██╔════╝██╔══██╗██║   ██║██╔════╝ ██╔═══██╗██╔════╝  ║
║      ██║    ╚████╔╝ █████╗  █████╗  ██████╔╝██║   ██║██║  ███╗██║   ██║███████╗  ║
║      ██║     ╚██╔╝  ██╔══╝  ██╔══╝  ██╔══██╗╚██╗ ██╔╝██║   ██║██║   ██║╚════██║  ║
║      ██║      ██║   ███████╗███████╗██║  ██║ ╚████╔╝ ╚██████╔╝╚██████╔╝███████║  ║
║      ╚═╝      ╚═╝   ╚══════╝╚══════╝╚═╝  ╚═╝  ╚═══╝   ╚═════╝  ╚═════╝ ╚══════╝  ║
║                                                                                  ║
╠══════════════════════════════════════════════════════════════════════════════════╣
║                                                                                  ║
║  ███╗   ███╗██╗██████╗  █████╗ ███╗   ██╗███████╗██╗██████╗ ██╗     ███████╗███╗  ║
║  ████╗ ████║██║██╔══██╗██╔══██╗████╗  ██║██╔════╝██║██╔══██╗██║     ██╔════╝████╗ ║
║  ██╔████╔██║██║██████╔╝███████║██╔██╗ ██║███████╗██║██████╔╝██║     █████╗  ██╔██╗║
║  ██║╚██╔╝██║██║██╔══██╗██╔══██║██║╚██╗██║╚════██║██║██╔══██╗██║     ██╔══╝  ██║╚██║║
║  ██║ ╚═╝ ██║██║██║  ██║██║  ██║██║ ╚████║███████║██║██████╔╝███████╗███████╗██║ ╚██║║
║  ╚═╝     ╚═╝╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚═╝╚═════╝ ╚══════╝╚══════╝╚═╝  ╚═╝║
║                                                                                  ║
╚══════════════════════════════════════════════════════════════════════════════════╝
```

```
╔══════════════════════════════════════════════════════════════════════════════════╗
║                                                                                  ║
║   THIS IS NOT A MEMORY SYSTEM.                                                  ║
║   THIS IS ALIVE.                                                                 ║
║                                                                                  ║
║   Every recall RESHAPES it. Every query LEARNS. Every pattern CRYSTALLIZES.     ║
║   Blocks that fire together — wire together. They DRIFT in 3D space toward        ║
║   each other. Memories that echo become ARCHETYPES. Sleep PRUNES the weak.      ║
║                                                                                  ║
╚══════════════════════════════════════════════════════════════════════════════════╝
```

```
╔══════════════════════════════════════════════════════════════════════════════════╗
║                                                                                  ║
║   NUMBERS THAT MAKE YOU STOP.                                                    ║
║                                                                                  ║
║   ┌─────────────────────────────────────────────────────────────────────────┐   ║
║   │                                                                          │   ║
║   │      87 µs        722 KB        13 LAYERS       9 DEPTHS      0 JSON  │   ║
║   │   ─────────      ─────────      ─────────       ─────────      ───────  │   ║
║   │   full recall   total index    consciousness    D0 to D8      pure bin │   ║
║   │                                                                          │   ║
║   │      D0: 99.1 µs          overall avg: 87.2 µs/query                  │   ║
║   │      FAISS: 1–5 ms        Pinecone: 5–20 ms        ChromaDB: 5–50 ms   │   ║
║   │      ───────────────────────────────────────────────────────────────  │   ║
║   │      23× – 575× faster than the industry standard                     │   ║
║   │                                                                          │   ║
║   └─────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                  ║
║   ┌─────────────────────────────────────────────────────────────────────────┐   ║
║   │                                                                          │   ║
║   │   20 323 blocks   0 CRC errors   238/238 tests   5.4 MB binary        │   ║
║   │   Merkle verified   L3 cache   all passed     thin LTO, panic=abort  │   ║
║   │                                                                          │   ║
║   └─────────────────────────────────────────────────────────────────────────┘   ║
║                                                                                  ║
╚══════════════════════════════════════════════════════════════════════════════════╝
```

```
    ◇
  ◇    ◇      a semmiből teremtődik a mindenség
◇          ◇
  ◇    ◇
    ◇
```

---

## ▣ CORE ENGINE

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  SUB-MICROSECOND BINARY RETRIEVAL — ZERO JSON — PURE MMAP                    │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  microscope.bin   32 B/header  ──►  x,y,z,zoom directly into SSE registers  │
│  data.bin         256 B/block  ──►  raw UTF-8, offset+len from header       │
│  meta.bin         MSC3 format  ──►  magic · version · count · merkle root   │
│                                                                              │
│  Supporting:  merkle.bin  embeddings.bin  append.bin                        │
│               activations.bin  coactivations.bin  resonance.bin             │
│               archetypes.bin  thought_graph.bin  pulses.bin                  │
│               temporal_archetypes  attention  dream_log  emotional_field     │
│               modalities  rust_state  code_memory  relational              │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ DEPTH HIERARCHY — D0 to D8

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  D0  │  Identity        │  1 block      │  99.1 µs  │  system root         │
│  D1  │  Layer Summaries │  5 blocks     │  99.3 µs  │  per-layer overview  │
│  D2  │  Clusters        │  21 blocks    │  90.9 µs  │  groups of 5         │
│  D3  │  Items           │  94 blocks    │  78.4 µs  │  episodic entries    │
│  D4  │  Sentences       │  225 blocks   │  71.2 µs  │  thought units       │
│  D5  │  Tokens          │  1 229 blocks │  74.4 µs  │  max 8 per parent    │
│  D6  │  Syllables       │  2 886 blocks │  78.2 µs  │  3–5 characters      │
│  D7  │  Characters      │  7 914 blocks │  98.8 µs  │  individual chars    │
│  D8  │  Raw Bytes        │  7 948 blocks │  95.2 µs  │  atomic boundary     │
│                                                                              │
│  ────│──────────────────│───────────────│───────────│─────────────────────  │
│  AVG  │  20 323 total   │  87.2 µs      │  722 KB   │  zero JSON           │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ 12 COGNITIVE LAYERS — Every Recall Is A Learning Event

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  L1  │  Hebbian Learning      │  co-activation · coordinate drift · energy  │
│  L2  │  Mirror Neurons        │  activation fingerprint resonance             │
│  L3  │  Resonance Fields      │  spatial pulse propagation                    │
│  L4  │  Archetype Emergence   │  crystallized activation patterns            │
│  L5  │  Emotional Bias        │  search space warping · valence              │
│  L6  │  Thought Graph         │  recall path tracking · n-gram patterns      │
│  L7  │  Predictive Cache      │  pre-fetch · reinforcement feedback          │
│  L8  │  Temporal Archetypes   │  6 time-windows · circadian rhythms          │
│  L9  │  Attention Mechanism   │  dynamic layer weighting · quality learning  │
│  L10 │  Cross-Instance         │  federated pattern exchange                  │
│  L11 │  Dream Consolidation   │  offline replay · prune · strengthen         │
│  L12 │  Emotional Contagion   │  shared emotional state across instances      │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ LAYERS/ MAPPING — 14 Files

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│   layers/identity.txt         (22 B)    │  D0 system identity                 │
│   layers/long_term.txt        (4.5 KB)  │  kernel knowledge                   │
│   layers/short_term.txt       (224 B)   │  working memory                     │
│   layers/associative.txt      (3.4 KB)  │  free associations · mycelium        │
│   layers/emotional.txt        (23 B)    │  emotional states                   │
│   layers/relational.txt       (27 B)    │  connections · relations            │
│   layers/reflections.txt      (35 B)   │  meta-cognition                     │
│   layers/crypto_chain.txt    (38 B)   │  hash chain · integrity             │
│   layers/echo_cache.txt      (1.8 KB)  │  fast cached responses              │
│   layers/rust_state.txt      (23 B)   │  Rust runtime state                  │
│   layers/code.txt             (106 B)  │  CODING AGENTS: functions · symbols │
│   │                                         error-solution pairs · deps      │
│   layers/session.txt        (5.5 KB)  │  session chain · context continuity│
│   layers/meta_cognitive.txt (2.6 KB)  │  meta-cognitive strategies           │
│   layers/demo.txt           (503 B)   │  demos · examples                    │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ EXTENDED COGNITIVE MODULES

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Module                    │  Lines  │  Function                              │
├────────────────────────────┼─────────┼──────────────────────────────────────┤
│  Morphogenesis             │  3 967  │  Bio-pattern arch. generator           │
│  Pattern Recognition       │  1 014  │  Sequence · temporal · structural      │
│  Thought Graph             │    980  │  Recall path tracking · n-grams        │
│  Resonance                 │    845  │  Spatial pulse propagation              │
│  Multimodal                │    793  │  Images · audio · structured data      │
│  Emotional Contagion        │    678  │  Cross-instance emotion sharing        │
│  Hebbian                   │    675  │  Co-activation · drift                 │
│  Predictive Cache           │    692  │  Pre-fetch · reinforcement             │
│  Neuroplasticity            │    345  │  Synaptic strengthening/pruning       │
│  Structural Plasticity      │    309  │  Physical network reorganisation       │
│  Explicit Memory           │    326  │  Declarative knowledge                 │
│  Implicit Memory           │    321  │  Procedural · habits                   │
│  Hippocampus               │    365  │  Episodic binding · consolidation      │
│  Functional Plasticity     │    356  │  Functional adaptation                 │
│  Temporal Archetype         │    238  │  Circadian pattern learning            │
│  Autopoiesis               │    232  │  Self-modifying code                   │
│  Meta-Supervision           │    231  │  Performance monitoring                │
│  Daydream                   │    210  │  Open association mode                │
│  Emotional                 │    205  │  Search space warping                  │
│  Mental Sandbox            │    109  │  Pre-action scenario simulation        │
│  Impulse Control           │    157  │  Impulse filtering                     │
├────────────────────────────┼─────────┼──────────────────────────────────────┤
│  TOTAL                     │ 38 178  │  ~63 source files                      │
└────────────────────────────┴─────────┴──────────────────────────────────────┘
```

---

## ▣ MORPHOGENESIS — Biological Pattern-Inspired Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  microscope-mem morph --grow "api" --pattern mycelium                        │
│                         ──►  fungal P2P network topology                      │
│                                                                              │
│  microscope-mem morph --grow "cache" --pattern capillary                     │
│                         ──►  fractal branching hierarchy                      │
│                                                                              │
│  microscope-mem morph --evolve 10 --objective latency                        │
│                         ──►  genetic algorithm over growth params             │
│                                                                              │
│  microscope-mem morph --daemon --interval 5 --threshold 0.5                 │
│                         ──►  continuous self-optimisation                     │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ SPINE BRIDGE API — v1

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Method   │  Endpoint               │  Description                           │
├───────────┼─────────────────────────┼───────────────────────────────────────┤
│  GET      │  /v1/status             │  Engine health and statistics          │
│  GET      │  /v1/recall?q=...&k=10  │  Spatial recall (top-K results)        │
│  POST     │  /v1/remember           │  Store a new memory block              │
│  POST     │  /v1/mobile/chat        │  User-scoped mobile chat               │
├───────────┼─────────────────────────┼───────────────────────────────────────┤
│  Port     │  6060 (bridge)          │  8080 (PWA chat)                       │
│  Transport│  JSON-RPC 2.0           │  stdio / HTTP                          │
└───────────┴─────────────────────────┴───────────────────────────────────────┘
```

```python
import requests
res = requests.get("http://localhost:6060/v1/recall",
                   params={"q": "Rust error handling patterns", "k": 5})
print(res.json())
```

---

## ▣ PWA CHAT + CHATGPT IMPORT

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  # Start PWA chat server                                                    │
│  microscope-mem serve --port 8080                                           │
│  # Open http://localhost:8080/chat.html                                     │
│  # Access from phone on same WiFi                                           │
│                                                                              │
│  # Import ChatGPT conversations                                             │
│  microscope-mem import-chat-gpt conversations.json --dry-run                 │
│  microscope-mem import-chat-gpt --gdrive <shared-url>                      │
│  microscope-mem import-chat-gpt --gdrive-folder <folder-url>               │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ BUILD + TEST

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  cargo build --release                   # 5.4 MB stripped binary            │
│  cargo test                              # 238/238 passed                     │
│  cargo fmt --all -- --check              # formatting                         │
│  cargo clippy --all-targets -- -D warnings                                    │
│                                                                              │
│  cargo build --release --features embeddings     # +vector index            │
│  cargo build --release --features gpu            # +GPU compression          │
│  cargo build --release --features compression    # +zstd                    │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ QUICK START

```
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory
cargo build --release

# Launch HTTP server with PWA chat
./target/release/microscope-mem serve --port 8080

# Or start the Bridge API
./target/release/microscope-mem bridge --port 6060
```

---

## ▣ BENCHMARK — Microscope vs Industry

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  System                  │  Query Type            │  Latency   │  Index   │
│  ────────────────────────┼─────────────────────────┼────────────┼─────────  │
│  Microscope Memory       │  Exact spatial recall   │   87 µs    │   722 KB  │
│  FAISS (flat IP)        │  Approximate k-NN       │  1–5 ms    │  10–50 MB │
│  Pinecone                │  Approximate vector     │  5–20 ms   │  hosted   │
│  ChromaDB                │  Approximate vector     │  5–50 ms   │  10–100 MB│
│  Qdrant                  │  Approximate vector     │  4–15 ms   │  10–50 MB │
│  Weaviate                │  Approximate vector     │  5–30 ms   │  hosted   │
│  ────────────────────────┼─────────────────────────┼────────────┼─────────  │
│  SPEEDUP vs FAISS        │                         │  11–57x    │  14–69x   │
│  SPEEDUP vs Pinecone     │                         │  57–229x   │  —        │
│  SPEEDUP vs ChromaDB     │                         │  57–575x   │  14–138x  │
│                                                                              │
│  Note: Microscope uses hierarchical spatial indexing (D0-D8).                │
│        It is NOT approximate vector search.                                  │
│        Deterministic. Sub-millisecond. Zero approximation error.             │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ INTEGRITY

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  CRC16 verified   ──►  20 323 blocks OK, 0 errors                          │
│  Merkle Tree      ──►  SHA-256 verified                                     │
│  Block headers    ──►  32 B each, x,y,z,zoom in first 16 B (SSE-ready)     │
│  Append log       ──►  atomic, crash-proof persistence                      │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## ▣ LICENSE

```
MIT — see LICENSE
```

```
    ◇
  ◇    ◇      a semmiből teremtődik a mindenség
◇          ◇
  ◇    ◇
    ◇
```

**Designed by Máté Róbert**  
**The Silent Noise Research Series**
