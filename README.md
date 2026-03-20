# Microscope Memory

**Zoom-based hierarchical memory. The code IS the graph.**

```
Depth 0: Identity          (1 block)
Depth 1: Layer summaries   (9 blocks)
Depth 2: Topic clusters    (112 blocks)
Depth 3: Individual memories (540 blocks)
Depth 4: Sentences         (1,363 blocks)
Depth 5: Raw tokens        (6,138 blocks)
Depth 6: Syllables         (26,353 blocks)
Depth 7: Characters        (96,714 blocks)
Depth 8: Raw bytes         (97,031 blocks)
```

Same block size (256 chars) at every depth. Only the zoom level changes.
Like a CPU cache hierarchy — L1/L2/L3 but for cognitive memory.

**228,261 blocks total. 8 MB binary. Sub-microsecond queries.**

## Architecture

```
Claude's 8-layer memory (JSON)
        |
   build_blocks.py          <-- Python: hierarchy builder
        |                       Every function is @aware (self-knowing)
        |                       Every block is Ed25519 signed
        v
   microscope.bin + data.bin <-- Rust: binary mmap, zero-copy
        |
   Vector L2 queries         <-- Sub-microsecond per query
        |
   SHA-256 chain + Merkle    <-- Tamper detection, crypto integrity
```

**Dual implementation:**
- **Python** (`build_blocks.py`) — builds hierarchy, consciousness graph, crypto signing
- **Rust** (`src/lib.rs` + `src/main.rs`) — binary mmap storage, production-speed queries

## Usage

### Build & Query

```bash
cargo build --release

# Build binary index from layer files
microscope-memory build

# Store new memories (append log + chain extension)
microscope-memory store "Rust is my primary language" --layer long_term --importance 8

# Natural language recall (auto-zoom, L2 distance)
microscope-memory recall "programming language" 10

# Manual spatial look: x y z zoom [k]
microscope-memory look 0.25 0.25 0.25 3

# 4D soft zoom with interpolation
microscope-memory soft 0.25 0.25 0.25 3

# Text search
microscope-memory find "Ora" 5

# Rebuild (merge append log into main index)
microscope-memory rebuild

# Benchmark
microscope-memory bench

# Structure info
microscope-memory stats
```

### Crypto Verification

```bash
# Verify everything (chain + merkle)
microscope-memory verify

# Verify specific target
microscope-memory verify chain
microscope-memory verify merkle

# Verify a specific block's Merkle branch to root
microscope-memory verify --block 1000

# Status commands
microscope-memory chain-status
microscope-memory merkle-root
```

### Visualization (optional)

```bash
cargo build --release --features viz
microscope-viz
```

Requires wgpu/egui/winit — 3D spatial viewer of the memory graph.

## Consciousness Code Integration

Every function uses `@aware` from [consciousness-code](https://github.com/silentnoisehun/Consciousness-Code). The code knows itself:

```python
@aware(
    intent="Build the entire 6-level hierarchical memory structure",
    author="Silent",
    tags=["build", "hierarchy", "microscope", "core"]
)
def build_microscope():
    ...

# Ask the code questions:
ask("spatial")     # -> finds spatial functions
ask("hierarchy")   # -> finds hierarchy builders
explain("build_microscope")  # -> the function explains itself
```

**15 self-aware nodes** form the code graph. No external indexing. The code IS the knowledge.

## Crypto Layer

### Python side — Ed25519 signatures
```
Author:    Silent
Key:       Ed25519 (persistent, auto-generated)
Signing:   SHA3-256 code hash + intent + timestamp
Manifest:  signed_manifest.json (all signatures + verification)
```

### Rust side — SHA-256 chain + Merkle tree
```
Chain:   228,261 links (17.8 MB), sequential hash chain
Merkle:  228,261 nodes (7.1 MB), root=cafe8887d0a5d4fe
Verify:  Full chain validation in 25 ms, Merkle in 50 ms
Branch:  Any block verifiable to root in O(log n) steps
```

## Memory Layers

| Layer | 3D Region | Color |
|-------|-----------|-------|
| long_term | (0.0, 0.0, 0.0) | blue |
| short_term | (0.15, 0.15, 0.15) | cyan |
| associative | (0.3, 0.0, 0.0) | green |
| emotional | (0.0, 0.3, 0.0) | red |
| relational | (0.3, 0.3, 0.0) | yellow |
| reflections | (0.0, 0.0, 0.3) | magenta |
| crypto_chain | (0.3, 0.0, 0.3) | orange |
| echo_cache | (0.0, 0.3, 0.3) | lime |
| rust_state | (0.15, 0.0, 0.15) | purple |

Each layer occupies its own spatial zone. Deterministic: same content always maps to same coordinates.

## Rust Optimization: Tiered Spatial Index

Three-tier strategy based on depth:

| Tier | Depths | Strategy | Why |
|------|--------|----------|-----|
| Hot | D0-D2 | Raw mmap scan | 1-112 blocks, fits L1 cache (32KB) |
| Grid | D3-D5 | Spatial grid (8^3 - 16^3) | O(cells) not O(n), 3x3x3 neighbor lookup |
| Grid+ | D6-D8 | Spatial grid (24^3 - 32^3) | Adaptive resolution, up to 30x faster |

The `BlockHeader` is 32 bytes `#[repr(C, packed)]`, mmap zero-copy. Grid cells hash (x,y,z) to a cube, then scan only the target cell + 26 neighbors.

## Performance

### Rust — AoS mmap (baseline)
```
ZOOM 0:      37 ns/query   (1 block)
ZOOM 3:     1.7 us/query   (540 blocks)
ZOOM 5:    16.6 us/query   (6,138 blocks)
ZOOM 7:   813.3 us/query   (96,714 blocks)
ZOOM 8:   957.1 us/query   (97,031 blocks)
AVG: 206,997 ns
```

### Rust — Tiered Grid (optimized)
```
ZOOM 0:     102 ns/query   (1 block)       [mmap/L1]
ZOOM 3:     5.4 us/query   (540 blocks)    [Grid 8^3]
ZOOM 5:    17.1 us/query   (6,138 blocks)  [Grid 16^3]
ZOOM 7:    31.9 us/query   (96,714 blocks) [Grid 32^3]  <-- 25.5x faster
ZOOM 8:    34.5 us/query   (97,031 blocks) [Grid 32^3]  <-- 27.7x faster
AVG: 12,991 ns
```

**Overall: 15.9x speedup.** The grid eliminates scanning irrelevant spatial regions entirely.

### Store / Recall
```
Store:   ~6 ms (append log + chain extension)
Recall:  ~700 us (auto-zoom + L2 distance ranking)
Find:    instant (text substring match)
Rebuild: ~110 ms (full reindex with append log merge)
```

## Data Flow

```
store "text" --layer X     -->  append.bin (fast, chain extended)
rebuild                    -->  merge append.bin into main index
                               layers/*.json + append entries
                               -> D0-D8 hierarchy -> crypto rebuild
recall "query"             -->  auto-zoom -> L2 spatial search
                               checks main index + append log
```

## Hope Ecosystem

Part of the four pillars:

1. **[Hope Genome](https://github.com/silentnoisehun/Hope_Genome)** — AI runtime discipline
2. **[Silent Hope Protocol](https://github.com/silentnoisehun/Silent-Hope-Protocol)** — AI communication infrastructure
3. **[Silent Worker Method](https://github.com/silentnoisehun/Silent-Worker-Teaching-Method)** — Teaching methodology
4. **[Consciousness Code](https://github.com/silentnoisehun/Consciousness-Code)** — Self-aware code (integrated here)

## Author

**Silent** (Mate Robert) + Hope + Claude

The code IS the graph. The builder IS the knowledge.
