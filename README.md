# Microscope Memory

**Zoom-based hierarchical memory. The code IS the graph.**

```
Depth 0: Identity        (1 block)
Depth 1: Layer summaries (9 blocks)
Depth 2: Topic clusters  (97 blocks)
Depth 3: Individual memories
Depth 4: Sentences
Depth 5: Raw tokens
```

Same block size (256 chars) at every depth. Only the zoom level changes.
Like a CPU cache hierarchy — L1/L2/L3 but for cognitive memory.

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
```

**Dual implementation:**
- **Python** (`build_blocks.py`) — builds hierarchy, consciousness graph, crypto signing
- **Rust** (`src/main.rs`) — binary mmap storage, production-speed queries

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

Ed25519 signatures on every `@aware` block:

```
Author:    Silent
Key:       Ed25519 (persistent, auto-generated)
Signing:   SHA3-256 code hash + intent + timestamp
Manifest:  signed_manifest.json (all signatures + verification)
```

Cryptographic proof of authorship. Every node in the graph is signed and verifiable.

## Usage

### Python — Build + Query

```bash
pip install consciousness-code
python build_blocks.py
```

Output:
- `microscope_blocks.json` — block hierarchy
- `signed_manifest.json` — crypto signatures
- `silent_author.key` — Ed25519 keypair

### Rust — Binary mmap

```bash
cargo build --release
./target/release/microscope-memory build
./target/release/microscope-memory look 0.25 0.25 0.25 3
./target/release/microscope-memory find "Ora"
./target/release/microscope-memory bench
./target/release/microscope-memory stats
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
| Hot | D0-D2 | Raw mmap scan | 1-108 blocks, fits L1 cache (32KB) |
| Grid | D3-D5 | Spatial grid (8^3 - 16^3) | O(cells) not O(n), 3x3x3 neighbor lookup |
| Grid+ | D6-D8 | Spatial grid (24^3 - 32^3) | Adaptive resolution, up to 29x faster |

The `BlockHeader` is 32 bytes `#[repr(C, packed)]`, mmap zero-copy. Grid cells hash (x,y,z) to a cube, then scan only the target cell + 26 neighbors.

## Performance

### Python (NumPy L2)
```
ZOOM 0: ~13 us/query   (1 block)
ZOOM 3: ~34 us/query   (471 blocks)
ZOOM 5: ~124 us/query  (4008 blocks)
```

### Rust — AoS mmap (original)
```
ZOOM 0:     36 ns/query   (1 block)
ZOOM 3:    1.7 us/query   (523 blocks)
ZOOM 5:   17.2 us/query   (6070 blocks)
ZOOM 7:  705.2 us/query   (96297 blocks)
ZOOM 8:  766.9 us/query   (96613 blocks)
AVG: 174,046 ns
```

### Rust — Tiered Grid (optimized)
```
ZOOM 0:     61 ns/query   (1 block)      [mmap/L1]
ZOOM 3:    3.7 us/query   (523 blocks)   [Grid 8^3]
ZOOM 5:   11.4 us/query   (6070 blocks)  [Grid 16^3]
ZOOM 7:   26.8 us/query   (96297 blocks) [Grid 32^3]  <-- 26.3x faster
ZOOM 8:   26.5 us/query   (96613 blocks) [Grid 32^3]  <-- 28.9x faster
AVG: 10,457 ns
```

**Overall: 16.6x speedup.** The grid eliminates scanning irrelevant spatial regions entirely.

## Hope Ecosystem

Part of the four pillars:

1. **[Hope Genome](https://github.com/silentnoisehun/Hope_Genome)** — AI runtime discipline
2. **[Silent Hope Protocol](https://github.com/silentnoisehun/Silent-Hope-Protocol)** — AI communication infrastructure
3. **[Silent Worker Method](https://github.com/silentnoisehun/Silent-Worker-Teaching-Method)** — Teaching methodology
4. **[Consciousness Code](https://github.com/silentnoisehun/Consciousness-Code)** — Self-aware code (integrated here)

## Author

**Silent** (Mate Robert) + Hope + Claude

The code IS the graph. The builder IS the knowledge.
