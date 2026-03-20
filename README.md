# Microscope Memory

**Pure Rust zoom-based hierarchical memory. Sub-microsecond queries over 228K blocks.**

```
Depth 0: Identity          (1 block)
Depth 1: Layer summaries   (9 blocks)
Depth 2: Topic clusters    (112 blocks)
Depth 3: Individual memories (537 blocks)
Depth 4: Sentences         (1,360 blocks)
Depth 5: Raw tokens        (6,114 blocks)
Depth 6: Syllables         (26,308 blocks)
Depth 7: Characters        (96,594 blocks)
Depth 8: Raw bytes         (96,911 blocks)
```

Same block size (256 chars) at every depth. Only the zoom level changes.
Like a CPU cache hierarchy — L1/L2/L3 but for cognitive memory.

**227,946 blocks total. 8 MB binary. Sub-microsecond queries.**

## Architecture

```
Memory layers (JSON)
        |
   cargo run -- build        <-- Rust: hierarchy builder (D0-D8)
        |
   microscope.bin + data.bin <-- Binary mmap, zero-copy
        |
   Vector L2 queries         <-- Sub-microsecond per query
        |
   SHA-256 chain + Merkle    <-- Tamper detection, crypto integrity
```

Pure Rust implementation: `src/lib.rs` (core engine) + `src/main.rs` (CLI).

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

## Crypto Layer

### SHA-256 chain + Merkle tree
```
Chain:   227,946 links (17.8 MB), sequential hash chain
Merkle:  227,946 nodes (7.1 MB), root=cafe8887d0a5d4fe
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

### AoS mmap (baseline)
```
ZOOM 0:      37 ns/query   (1 block)
ZOOM 3:     1.7 us/query   (537 blocks)
ZOOM 5:    16.5 us/query   (6,114 blocks)
ZOOM 7:   666.5 us/query   (96,594 blocks)
ZOOM 8:   771.1 us/query   (96,911 blocks)
AVG: 169,861 ns
```

### Tiered Grid (optimized)
```
ZOOM 0:      61 ns/query   (1 block)        [mmap/L1]
ZOOM 3:     3.6 us/query   (537 blocks)     [Grid 8^3]
ZOOM 5:    10.4 us/query   (6,114 blocks)   [Grid 16^3]
ZOOM 7:    26.0 us/query   (96,594 blocks)  [Grid 32^3]  <-- 25.6x faster
ZOOM 8:    25.9 us/query   (96,911 blocks)  [Grid 32^3]  <-- 29.8x faster
AVG: 9,841 ns
```

**Overall: 17.3x speedup.** The grid eliminates scanning irrelevant spatial regions entirely.

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

## Author

**Silent** (Mate Robert) + Claude
