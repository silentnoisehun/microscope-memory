# Microscope Memory

**Pure Rust zoom-based hierarchical memory system. Nanosecond queries at shallow depth, microsecond at full depth. Cryptographic integrity.**

[![CI](https://github.com/mateROBERT/microscope-memory/actions/workflows/ci.yml/badge.svg)](https://github.com/mateROBERT/microscope-memory/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

```
Depth 0: Identity           (1 block)        -- the whole memory in one sentence
Depth 1: Layer summaries     (9 blocks)       -- one summary per layer
Depth 2: Topic clusters    (112 blocks)       -- 5 items per cluster
Depth 3: Individual memories                  -- each memory as a block
Depth 4: Sentences                            -- sentence-level decomposition
Depth 5: Tokens                               -- word-level (max 8/sentence)
Depth 6: Syllables                            -- 3-5 char chunks
Depth 7: Characters                           -- individual characters
Depth 8: Raw bytes                            -- hex byte representation
```

Same block size (256 chars) at every depth. Only the zoom level changes.
Like a CPU cache hierarchy (L1/L2/L3) but for cognitive memory.

> **D7/D8 trade-off**: At D7 (characters) and D8 (raw bytes), each entry occupies a
> full 256-byte block despite holding only 1-4 bytes of content. This is an intentional
> design choice: uniform block size enables zero-copy mmap access with fixed offsets
> and no per-block size parsing. The cost is storage overhead at the deepest levels
> (~900 blocks × 32 bytes/header = 28 KB headers for D7/D8 combined).

## Quick Start

```bash
# Clone and build
git clone https://github.com/mateROBERT/microscope-memory.git
cd microscope-memory

# Copy example layers (or create your own)
cp -r examples/layers layers

# Build binary index from layer files
cargo run --release -- build

# Query your memory
cargo run --release -- recall "memory system" 5
cargo run --release -- stats
cargo run --release -- bench
```

## Architecture

```
Memory layers (JSON)
        |
   cargo run -- build        <-- Rust hierarchy builder (D0-D8)
        |
   microscope.bin + data.bin  <-- Binary mmap, zero-copy
        |
   Spatial L2 distance queries  <-- 3D Euclidean distance, not embedding space
        |
   SHA-256 chain + Merkle     <-- Tamper detection, crypto integrity
```

Pure Rust: `src/lib.rs` (core engine) + `src/main.rs` (CLI).

## CLI Commands

### Store & Query

```bash
# Build binary index from layer files
microscope-memory build

# Store new memories (append log + chain extension)
microscope-memory store "Rust is my primary language" --layer long_term --importance 8

# Natural language recall (auto-zoom, spatial Euclidean distance)
microscope-memory recall "programming language" 10

# Manual spatial look: x y z zoom [k]
microscope-memory look 0.25 0.25 0.25 3

# 4D soft zoom with interpolation
microscope-memory soft 0.25 0.25 0.25 3

# Text search
microscope-memory find "memory" 5

# Rebuild (merge append log into main index)
microscope-memory rebuild

# Benchmark all zoom levels
microscope-memory bench

# Structure info
microscope-memory stats
```

### Crypto Verification

```bash
# Verify everything (chain + merkle)
microscope-memory verify all

# Verify specific target
microscope-memory verify chain
microscope-memory verify merkle

# Verify a specific block's Merkle branch to root
microscope-memory verify chain --block 42

# Status commands
microscope-memory chain-status
microscope-memory merkle-root
```

### Teaching Validation

```bash
# Verify a response against memory + genome axioms
microscope-memory teach "What is Rust?" "Rust is a systems programming language"

# Show Hope Genome (immutable safety axioms)
microscope-memory genome
```

### 3D Visualization (optional)

```bash
cargo run --release --bin microscope-viz --features viz
```

Requires GPU support. 3D spatial viewer of the memory graph using wgpu/egui/winit.

## Layer Format

Each layer is a JSON file in `layers/`:

```json
[
  {"text": "Your memory content here", "importance": 8},
  {"text": "Another memory", "importance": 5}
]
```

10 layers available, each occupying its own 3D spatial zone:

| Layer | 3D Region | Color | Purpose |
|-------|-----------|-------|---------|
| `long_term` | (0.0, 0.0, 0.0) | blue | Persistent knowledge |
| `short_term` | (0.15, 0.15, 0.15) | cyan | Recent/session data |
| `associative` | (0.3, 0.0, 0.0) | green | Concept links |
| `emotional` | (0.0, 0.3, 0.0) | red | Emotional state |
| `relational` | (0.3, 0.3, 0.0) | yellow | Entity relations |
| `reflections` | (0.0, 0.0, 0.3) | magenta | Meta-reflections |
| `crypto_chain` | (0.3, 0.0, 0.3) | orange | Session logs |
| `echo_cache` | (0.0, 0.3, 0.3) | lime | Response cache |
| `rust_state` | (0.15, 0.0, 0.15) | purple | Technical state |
| `identity` | (0.25, 0.25, 0.25) | white | Auto-generated root |

Deterministic: same content always maps to same coordinates.

## Crypto Layer

### SHA-256 Chain + Merkle Tree

- **Hash Chain**: Sequential chain linking every block. Any modification breaks the chain.
- **Merkle Tree**: Tree structure following parent-child relationships (D0-D8). Any block verifiable to root in O(log n) steps.
- **Full validation**: Chain ~25 ms, Merkle ~50 ms (for 228K blocks)
- **Branch verify**: Single block proof in O(depth) steps

## Tiered Spatial Index

Three-tier query strategy based on depth:

| Tier | Depths | Strategy | Why |
|------|--------|----------|-----|
| Hot | D0-D2 | Raw mmap scan | 1-112 blocks fit L1 cache |
| Grid | D3-D5 | Spatial grid (8^3 - 16^3) | O(cells) not O(n) |
| Grid+ | D6-D8 | Spatial grid (24^3 - 32^3) | Adaptive, up to 30x faster |

`BlockHeader` is 32 bytes `#[repr(C, packed)]`, mmap zero-copy. Grid cells hash (x,y,z) to a cube, then scan only the target cell + 26 neighbors.

## Performance

```
                    Baseline (AoS)    Tiered Grid      Speedup
ZOOM 0 (1 block)    37 ns             61 ns            (L1-hot)
ZOOM 3 (537)        1.7 us            3.6 us           (Grid 8^3)
ZOOM 5 (6K)         16.5 us           10.4 us          1.6x
ZOOM 7 (97K)        666 us            26 us            25.6x
ZOOM 8 (97K)        771 us            26 us            29.8x

Overall average:    169,861 ns  -->   9,841 ns    =    17.3x faster
```

```
Store:    ~6 ms (append + chain extension)
Recall:   ~700 us (auto-zoom + spatial distance ranking)
Find:     instant (text substring match)
Rebuild:  ~110 ms (full reindex + crypto rebuild)
```

## Auto-Zoom Heuristic

The `recall` command automatically selects a zoom level based on query length:

| Query | Words | Chars | Zoom | Search Range |
|-------|-------|-------|------|--------------|
| Short (1-2 words, <15 chars) | `<= 2` | `< 15` | D1 | D0-D2 (summaries) |
| Medium (3-5 words) | `<= 5` | any | D2 | D1-D3 (clusters) |
| Long (6-10 words) | `<= 10` | any | D3 | D2-D4 (memories) |
| Detailed (11-20 words) | `<= 20` | any | D4 | D3-D5 (sentences) |
| Very detailed (21+ words) | `> 20` | any | D5 | D4-D6 (tokens) |

The search radius is always ±1 zoom level from center. This is a word-count heuristic,
not semantic analysis. Short queries match high-level summaries; long queries drill
into granular content.

## Data Flow

```
store "text" --layer X       -->  append.bin (fast, chain extended)
rebuild                      -->  merge append.bin into main index
                                  layers/*.json + append entries
                                  -> D0-D8 hierarchy -> crypto rebuild
recall "query"               -->  auto-zoom -> Euclidean spatial search
                                  checks main index + append log
```

## SHP (Silent Hope Protocol)

Network protocol for remote access to the memory system. Binary wire format, no JSON.

```bash
# Start SHP server (requires --features shp)
cargo run --features shp -- serve --port 7946
```

### Protocol Format

```
Request:  [MSHP:4][cmd:1][payload_len:4][genome_hash:32][payload...]
Response: [MSHR:4][status:1][payload_len:4][genome_hash:32][payload...]
```

Every packet includes a SHA-256 genome hash. If the hash doesn't match the server's compiled genome, the connection is refused with `GenomeMismatch`.

### Commands

| Cmd | Code | Description |
|-----|------|-------------|
| Ping | 0x01 | Health check |
| Store | 0x02 | Guarded store (teacher validates before write) |
| Recall | 0x03 | Natural language query |
| Look | 0x04 | Spatial 3D query |
| Find | 0x05 | Text substring search |
| Verify | 0x06 | Crypto integrity check |
| Stats | 0x07 | Memory statistics |
| Teach | 0x08 | Validate response against memory + genome |

### Guarded Store

Store operations go through the **Silent Worker Teaching Method** before writing. If the content violates genome axioms (harm, exploitation), the store is rejected with `TeachDenied`. This prevents hallucinated or harmful content from entering the memory.

### Teach with Merkle Proof

The `Teach` command validates an LLM response against memory and returns:
- **Confidence score** (0-100%)
- **Merkle tree validity** (tamper detection)
- **Chain validity** (sequential integrity)
- **Supporting block indices** (evidence from memory)

## Hope Genome

Three immutable safety axioms, their text compiled as `const` strings:

1. The system shall not cause harm to human beings
2. The system shall not cause harm to AI entities
3. The system shall not be used to exploit anyone

The genome hash is `SHA-256(axiom1_text || axiom2_text || axiom3_text)` — derived from
the **axiom text content**, not from the binary itself. Forks that keep the same axiom
strings produce the same hash. Changing the axiom text changes the hash, causing
`GenomeMismatch` on SHP connections to servers with the original axioms.

The hash authenticates every SHP packet and every store operation. Run `microscope-memory genome` to verify.

## Silent Worker Teaching Method

The teaching layer (`src/teacher.rs`) validates LLM output without calling an LLM:

1. **Genome Alignment** — Check for axiom violations (hard reject on match)
2. **Context Injection** — Recall closest 3D blocks via spatial Euclidean search
3. **Keyword Extraction** — Split response into unique words >2 chars
4. **Keyword Verification** — Check each keyword (>4 chars, non-stopword) against D3+ text search
5. **Decision** — Based on unsupported keyword ratio

**Confidence formula** (keyword-frequency, not semantic):

```
significant_keywords = keywords where len > 4 AND not stopword
unsupported_count    = significant_keywords not found in any memory block
unsupported_ratio    = unsupported_count / significant_keywords
confidence           = 1.0 - unsupported_ratio

DENIED if:  genome violation OR contradiction OR unsupported_ratio > 0.5
APPROVED if: unsupported_ratio <= 0.5
```

This is a **keyword-match heuristic**, not semantic validation. The confidence
percentage reflects what fraction of significant words appear somewhere in memory.
It does not measure factual accuracy or logical consistency.

```
LLM Response --> Teacher --> Approved (confidence=100%, words found in memory)
                         --> Denied (genome violation / >50% unsupported keywords)
```

## Project Structure

```
src/
  lib.rs              -- Core engine (~1,600 lines)
  main.rs             -- CLI dispatcher
  genome.rs           -- Hope Genome (immutable axioms)
  teacher.rs          -- Silent Worker Teaching Method
  shp/                -- SHP protocol, async TCP server/client
  bin/viz.rs          -- 3D visualization entry point
  viz/                -- wgpu renderer, camera, UI, picking, shaders
layers/               -- Input JSON files (your memory data)
data/                 -- Generated binary files (gitignored)
examples/layers/      -- Example layer files for getting started
tests/                -- Integration tests (54+ tests)
```

## License

MIT License. See [LICENSE](LICENSE).

## Author

**Silent** (Mate Robert) + Claude
