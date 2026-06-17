# Microscope Memory

**A zoom-based hierarchical memory engine for AI agents. Pure binary, mmap-backed, sub-microsecond reads.**

Most AI memory systems store conversation history as flat text or JSON blobs and re-embed everything on every query. Microscope Memory takes a different approach: every piece of text is decomposed once into a 9-level hierarchy — from a single-sentence identity summary down to raw bytes — and stored as fixed-size binary blocks that are memory-mapped directly off disk. There is no parsing step at query time. There is no JSON. The query's "zoom level" determines how much detail comes back, and retrieval at any level is a direct memory read.

```
microscope-mem build              # layers/*.txt -> binary index
microscope-mem recall "what is Ora?" 10
microscope-mem bridge --port 6060  # Spine Bridge REST API
microscope-mem serve  --port 8080  # 3D viewer
microscope-mem mcp                 # MCP server for Claude Desktop
```

---

## Why this exists

Conversational AI systems lose context the moment a session ends, or they pay the cost of re-embedding and re-ranking large text blobs on every single turn. This project was built to solve a narrower, concrete problem: **give an AI agent a memory store that persists across restarts, returns results in microseconds rather than milliseconds, and doesn't require a vector database, a JSON parser, or a network call to function.**

It's a single static binary. It runs on a Raspberry Pi as easily as a server.

## How it works

### The core idea: fixed-size blocks at every depth

Every block in the index — regardless of what depth it represents — is exactly 256 bytes of data behind a 32-byte header. This is the foundational design constraint. It means the reader never has to guess how much to read; it computes an offset and reads a constant span. There's no variable-length parsing, no delimiter scanning, no schema validation on the hot path.

```
microscope.bin   — Block headers (32 bytes each, mmap'd)
├── x, y, z: f32       3D spatial position (FNV-hashed from content)
├── zoom: f32          normalized depth (depth / 8.0)
├── depth: u8          0–8
├── layer_id: u8        which memory layer this belongs to
├── data_offset: u32    byte offset into data.bin
├── data_len: u16       actual text length (<= 256)
├── parent_idx: u32     index of parent block
├── child_count: u16
└── crc16: [u8; 2]      per-block integrity check

data.bin         — Raw UTF-8 text (optionally zstd-compressed)
meta.bin         — MSC3 format: magic, version, block/depth counts, Merkle root, source hash
merkle.bin       — Full SHA-256 Merkle tree over all blocks
embeddings.bin   — mmap'd f32 vectors (Candle BERT or mock hash-based)
append.bin       — Append-only log for new memories written between rebuilds
```

### The 9-level hierarchy (D0–D8)

Text is decomposed once, at build time, into nine levels of granularity:

| Depth | What it represents | Example block count* |
|---|---|---|
| D0 | Whole-system identity summary | 1 |
| D1 | Per-layer summaries | 9 |
| D2 | Topic clusters | ~100 |
| D3 | Individual memories (raw entries) | ~500 |
| D4 | Sentence-level splits | ~1,300 |
| D5 | Word-level tokens (max 8/parent) | ~6,000 |
| D6 | Syllable-like morpheme chunks | ~26,000 |
| D7 | Individual characters | ~96,000 |
| D8 | Raw byte representation | ~96,000 |

*\*Counts from a benchmark run against ~227k total blocks; they scale with corpus size.*

A query's "zoom level" picks which depth to search. A two-word query doesn't need to scan sentence-level blocks; a detailed question benefits from finer granularity. Auto-zoom heuristics map query length to a sensible depth range, but you can also query a specific depth or range directly via MQL (see below).

### Spatial indexing without an index

Each block's (x, y, z) position is computed by FNV-hashing its text content, offset by a fixed per-layer origin. This is deterministic — the same text always lands in the same place — and it means semantically or lexically similar content tends to cluster in 3D space without needing a separate spatial index structure to maintain. Nearest-neighbor search is L2 distance in this space, optionally SIMD-accelerated (SSE4/AVX2) and blended with keyword matching and BERT embedding similarity for hybrid ranking.

### Memory layers

Content is partitioned into 10 cognitive layers, each with its own region of 3D space:

`identity` · `long_term` · `short_term` · `associative` · `emotional` · `relational` · `reflections` · `crypto_chain` · `echo_cache` · `rust_state`

These are organizational categories, not separate storage backends — a single binary index holds all of them, distinguished by `layer_id` in the block header.

### Incremental builds and the append log

Rebuilding the full hierarchy for a large corpus isn't free, so the build pipeline hashes the source layer files (SHA-256) and skips the rebuild entirely if nothing changed. New memories written between rebuilds go into `append.bin`, an append-only log that's queried alongside the main index and merged in on the next `rebuild`. This means writes are cheap (a single append) and reads stay fast (mmap'd binary, no JSON parsing) even as new data accumulates.

### Integrity

Every block carries a CRC16-CCITT checksum. The full index is also covered by a SHA-256 Merkle tree, so you can generate and verify a proof for any individual block, or detect tampering across the whole structure with `verify-merkle`. This is useful if the memory store is shared, exported, or needs to be audited — the `.mscope` export format bundles everything (`meta.bin`, `microscope.bin`, `data.bin`, `merkle.bin`, `append.bin`, `embeddings.bin`) into one portable archive with reproducible hashes.

### Beyond simple lookup: usage-pattern tracking

A few modules go beyond pure storage-and-retrieve:

- **Thought graph** (`thought_graph.rs`) — logs each recall as a node and consecutive recalls as edges. When a sequence of queries (A→B→C) recurs often enough, it crystallizes into a recognized pattern, which then gets a small boost in future ranking. This is closer to query-log analysis than to "learning" in the ML sense, but it does mean repeated usage patterns affect future results.
- **Hebbian-style weighting** (`hebbian.rs`) — blocks that are retrieved together have their association strength adjusted over time, influencing future co-retrieval.
- **Spaced repetition / reconsolidation** — modules that adjust block salience over time based on retrieval frequency and recency, loosely modeled on memory-consolidation research.

These are real, working mechanisms that change retrieval behavior based on usage history — worth being precise about what they are: heuristic, file-backed feedback loops, not claims about cognition or awareness.

## Performance

Benchmarked on 227,168 blocks, 10,000 queries per depth, single machine:

| Depth | Blocks | Query time |
|---|---|---|
| D0 | 1 | 37 ns |
| D1 | 9 | 92 ns |
| D2 | 108 | 506 ns |
| D3 | 523 | 1.7 µs |
| D4 | 1,349 | 3.9 µs |
| D5 | 6,070 | 18 µs |
| D6 | 26,198 | 72 µs |
| D7 | 96,297 | 505 µs |
| D8 | 96,613 | 492 µs |

These numbers come from `microscope-mem bench` against the project's own test corpus on one machine — they aren't an independent third-party benchmark, and there's no published apples-to-apples comparison yet against vector databases under identical hardware and corpus conditions. Treat the relative ordering (lower depths are faster) as solid; treat absolute cross-tool comparisons with appropriate skepticism until run independently.

## Installation

```bash
git clone https://github.com/silentnoisehun/microscope-memory.git
cd microscope-memory
cargo build --release
cp config.example.toml config.toml
# edit config.toml: set layers_dir and output_dir
```

Requires Rust 1.70+.

## Usage

### Build and query

```bash
microscope-mem build                          # build binary index from layers/*.txt
microscope-mem build --force                   # force full rebuild
microscope-mem rebuild                          # merge append log into main index

microscope-mem recall "what is Ora?" 10         # natural-language query, auto-zoom
microscope-mem find "Ora" 5                     # brute-force text search
microscope-mem embed "quantum physics" 10       # semantic search via embeddings
microscope-mem look 0.25 0.25 0.25 3            # manual: x y z zoom
```

### MQL — Microscope Query Language

```bash
microscope-mem query 'layer:long_term depth:2..5 "Ora"'
microscope-mem query '"memory" AND "Rust"'
microscope-mem query 'near:0.2,0.3,0.1,0.05 "pattern"'
microscope-mem query 'limit:20 layer:associative "concept"'
```

| Filter | Syntax | Example |
|---|---|---|
| Layer | `layer:NAME` | `layer:long_term` |
| Depth | `depth:N` or `depth:N..M` | `depth:2..5` |
| Spatial | `near:X,Y,Z[,R]` | `near:0.2,0.3,0.1,0.05` |
| Keyword | `"quoted"` or bare | `"Ora"` |
| Boolean | `AND`, `OR` | `"foo" AND "bar"` |
| Limit | `limit:N` | `limit:20` |

### Storing new memories

```bash
microscope-mem store "Important insight about the project"
microscope-mem store "Feeling good about progress" --layer emotional --importance 8
```

### Spine Bridge API (REST)

```bash
microscope-mem bridge --port 6060
```

| Method | Path | Description |
|---|---|---|
| GET | `/status` | Engine health, block count, layers |
| GET | `/session` | Resolve user/backend/scope namespace |
| GET | `/recall?q=...&k=N` | Semantic recall |
| POST | `/remember` | `{"text":"...", "layer":"...", "importance": N}` |
| GET | `/find?q=...&k=N` | Keyword text search |
| GET | `/look?x=&y=&z=&zoom=&k=N` | Spatial coordinate lookup |
| GET | `/mql?mql=...` | MQL query |
| POST | `/build` | Rebuild index (`{"force": true}`) |
| GET | `/session_log?n=50` | Last N session layer entries |
| POST | `/consolidate` | Consolidate sessions into long-term memory |
| POST | `/dream` | Run dream consolidation cycle |
| POST | `/mobile/recall` | POST-based recall for mobile clients |
| POST | `/mobile/remember` | User-scoped memory store |
| POST | `/mobile/chat` | Provider-agnostic chat (Ollama / OpenAI / Gemini) |
| GET | `/openapi.json` | Full OpenAPI spec |

All endpoints also available under `/v1/` prefix. Most accept optional `user_id`, `memory_backend` (local\|cloud), and `memory_scope` (personal\|shared\|both) for multi-user isolation.

### MCP server (Claude Desktop / Cline)

```bash
microscope-mem mcp
```

Starts a JSON-RPC 2.0 server over stdio, compatible with the Model Context Protocol. Register it in Claude Desktop's MCP config to give Claude direct access to the memory store.

### 3D Viewer

```bash
microscope-mem serve --port 8080
# open http://localhost:8080  (viewer.html)
```

Visual exploration of the memory index in 3D space.

### PWA Chat

A mobile-friendly chat interface is available in `pwa/chat.html`. It connects to the Spine Bridge on `:6060` and Ollama on `:11434` for a fully local AI chat with persistent memory.

### Backup, restore, integrity

```bash
microscope-mem export backup.mscope
microscope-mem import backup.mscope --output-dir ./restored
microscope-mem diff v1.mscope v2.mscope

microscope-mem verify            # CRC16 check
microscope-mem verify-merkle     # Merkle tree verification
microscope-mem proof 42          # Merkle proof for block 42
```

## Optional features

```bash
cargo build --release --features embeddings    # real Candle BERT (all-MiniLM-L6-v2, 384-dim)
cargo build --release --features compression    # zstd-compressed data.bin
cargo build --release --features gpu            # wgpu compute acceleration
cargo build --release --features python         # PyO3 bindings
cargo build --release --features wasm --target wasm32-unknown-unknown
```

Without `--features embeddings`, semantic search falls back to a deterministic mock hash-based provider — useful for testing the pipeline, not for real semantic similarity.

## Source layout

```
src/
├── lib.rs / main.rs / cli.rs    — library interface, entry point, CLI definitions
├── build.rs                      — layers/ -> binary decomposition (D0–D8), rayon-parallel
├── reader.rs                     — MicroscopeReader, BlockHeader, DataStore, append log
├── query.rs                      — MQL parser and executor
├── embeddings.rs / embedding_index.rs — embedding providers + mmap'd vector index
├── merkle.rs                     — SHA-256 Merkle tree with proof generation
├── snapshot.rs                   — .mscope archive export/import/diff
├── bridge.rs                     — Spine Bridge REST API (Axum, all 14 endpoints)
├── mcp.rs                        — Model Context Protocol server (JSON-RPC 2.0)
├── thought_graph.rs               — recall-sequence pattern tracking
├── hebbian.rs                     — co-retrieval association weighting
└── gpu.rs / wasm.rs / python.rs   — optional acceleration / platform targets
```

## Current state

This is a v0.x project. It's built and maintained by one developer, tested on the project's own fixtures and benchmark corpus, and has not yet had independent third-party adoption or review. The core retrieval path (build → mmap → query → verify) is solid and covered by integration tests; some of the more experimental modules (pattern crystallization, Hebbian weighting, GPU offload) are newer and less battle-tested. If you try it and find sharp edges, that's expected at this stage — issues and PRs are welcome.

## License

MIT — see [LICENSE](LICENSE).

For a deeper technical write-up, see [WHITEPAPER.md](WHITEPAPER.md).

---

*Built by [silentnoisehun](https://github.com/silentnoisehun) (Máté Róbert), Győrújfalu, Hungary.*
