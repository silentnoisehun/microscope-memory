<!-- badges:start -->
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.22983478.svg)](https://doi.org/10.5281/zenodo.22983478)
[![ORCID iD](https://img.shields.io/badge/ORCID-0009--0003--3986--6039-green)](https://orcid.org/0009-0003-3986-6039)
<!-- badges:end -->
# Microscope Memory

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/Limit-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-413%20passing-brightgreen.svg)](#-testing)
[![Blocks](https://img.shields.io/badge/Blocks-1.28M-purple.svg)](#-statistics)
[![WASM](https://img.shields.io/badge/WASM-151%20KB-blueviolet.svg)](#-wasm)

**Microscope Memory** is a Rust-native binary cognitive memory engine for AI agents.

> The model is only the motor. Memory is the system.

## What It Is

A living, self-organizing memory architecture with 13 layers, 9 depths, and 1.28 million blocks. Not a database â€” a cognitive engine that learns, dreams, and remembers.

### Core Capabilities

- **Hebbian Learning** â€” co-activation strengthens associations
- **Epistemic Layer** â€” evidence tracking with confidence, refutation, and promotion gates
- **Absentia (Silence Layer)** â€” detects what's missing, not just what's present
- **Cognitive Morphogenesis** â€” mycelium growth through cognitive gradient space
- **Intent Pipeline** â€” auditatable intent generation from genome + memory + absence
- **Predictive Cache** â€” anticipates what you'll need next
- **Emotional Contagion** â€” emotional state influences memory dynamics
- **Reconsolidation** â€” every recall transforms the memory (like a real brain)

## Statistics

| Metric | Value |
|--------|-------|
| Blocks (demo corpus) | 1,285,288 |
| Blocks (benchmark corpus) | 28,679 |
| Depths | 9 (D0-D8) |
| Layers | 13 |
| Data size | 284.7 MB |
| Rust files | 97 |
| Lines of code (`src/**/*.rs`) | 54,053 |
| Tests | 413 library + 16 hooks |
| Build time | ~2m 30s |
| Binary size | ~3.0 MB |
| WASM size | 151.6 KB |

> **Two different corpora.** The *demo corpus* is the 1.28 M-block index built
> from `layers/` and used for the block-distribution table below. The
> *benchmark corpus* is a separate 28,679-block index used for the latency
> numbers in [BENCHMARKS.md](BENCHMARKS.md). They are not interchangeable:
> latency figures refer to the benchmark corpus unless stated otherwise.

### Block Distribution

| Depth | Blocks | Percentage |
|-------|--------|------------|
| D0 | 1 | 0.0001% |
| D1 | 10 | 0.001% |
| D2 | 858 | 0.07% |
| D3 | 4,271 | 0.33% |
| D4 | 12,433 | 0.97% |
| D5 | 72,238 | 5.63% |
| D6 | 175,777 | 13.69% |
| D7 | 497,997 | 38.73% |
| D8 | 521,703 | 40.60% |

## Architecture

### Cognitive Modules

| Module | File | Lines | Purpose |
|--------|------|-------|---------|
| Epistemic | `epistemic.rs` | 1,241 | Evidence tracking, confidence, promotion gates |
| Absentia | `absentia.rs` | 498 | Silence layer â€” detects what's missing |
| Intent | `intent.rs` | 461 | Auditatable intent generation |
| Cognitive Morphogenesis | `cognitive_morphogenesis.rs` | 826 | Integration engine â€” 7-component gradient |
| Morphogenesis | `morphogenesis.rs` | 2,200 | Biological growth algorithms (mycelium, capillary, slime mold) |
| Hebbian | `hebbian.rs` | 754 | Co-activation learning |
| Resonance | `resonance.rs` | 776 | Pulse-based federation protocol |
| Predictive Cache | `predictive_cache.rs` | 635 | Anticipatory caching |
| Emotional Contagion | `emotional_contagion.rs` | 604 | Emotional state sharing |
| Relevance | `relevance.rs` | 177 | Lexical-spatial ranking |
| Reconsolidation | `reconsolidation.rs` | 215 | Memory transformation on recall |
| Dream | `dream.rs` | 1,090 | Consolidation and pruning |
| Pattern Recognition | `pattern_recognition.rs` | 1,022 | Sequence, temporal, structural patterns |
| Attention | `attention.rs` | 510 | Attention mechanisms |
| Enforcement | `enforcement.rs` | 852 | Commitment gates and audit |

### The Cognitive Gradient

```
gradient = w1*relevance + w2*resonance + w3*evidence
         + w4*hebbian + w5*prediction + w6*emotion + w7*execution
```

### Phase Transitions

| Phase | Gradient | Behavior |
|-------|----------|----------|
| GAS | < 0.3 | Free exploration |
| LIQUID | 0.3 - 0.7 | Gradient following |
| SOLID | > 0.7 | Consolidated paths |

### Absentia Shadow Term

```
effective_gradient = positive_gradient * (1 - absence_shadow)
```

The shadow term prevents growth toward areas with weak evidence, without blocking exploration entirely.

## Testing

### Unit Tests
```bash
cargo test                    # All tests (413 passing)
cargo test --lib              # Library tests only
cargo test --test integration # Integration tests only
```

### Adversarial Tests
```bash
microscope-mem morphogenesis adversarial        # Basic (8/8)
microscope-mem morphogenesis deep-adversarial    # Deep (7/7, 5 warnings)
```

### Hook Tests
```bash
cargo test -p microscope-hooks  # 16 hook tests
```

## CLI Commands

### Memory Operations
```bash
microscope-mem recall "query" 5
microscope-mem store "text" -l session -i 5
microscope-mem find "pattern"
microscope-mem stats
microscope-mem verify
```

### Cognitive Morphogenesis
```bash
microscope-mem morphogenesis status
microscope-mem morphogenesis run
microscope-mem morphogenesis adversarial
microscope-mem morphogenesis deep-adversarial
microscope-mem morphogenesis presence-absence-test
```

### Absentia (Silence Layer)
```bash
microscope-mem absentia status
microscope-mem absentia scan
microscope-mem absentia anti-hebbian
microscope-mem absentia causal-laundering
```

### Intent Pipeline
```bash
microscope-mem intent generate
microscope-mem intent genome
```

### Other Cognitive Commands
```bash
microscope-mem hebbian
microscope-mem resonance
microscope-mem patterns
microscope-mem hottest 5
microscope-mem dream
microscope-mem autonomous
```

## WASM

The engine compiles to WebAssembly (151.6 KB) and runs in browsers:

```bash
cargo build --target wasm32-unknown-unknown --release --features wasm --no-default-features
wasm-pack build --target web --release -- --features wasm --no-default-features
```

### WASM Exports

```typescript
class MicroscopeWasm {
  load_binary(meta: Uint8Array, headers: Uint8Array, data: Uint8Array): void
  recall(query: string, k: number): WasmBlock[]
  store(text: string, layer: string, importance: number): void
  look(x: number, y: number, z: number, zoom: number, k: number): WasmBlock[]
  block_count(): number
  is_loaded(): boolean
}

class HopeCli {
  exec(command: string): string  // Virtual CLI â€” same commands as native
}
```

## Nested Fractal Cognitive Morphogenesis Architecture

The system exhibits cross-scale causality:

```
Bottom-up:  node -> path -> organism -> field -> phase
Top-down:   phase -> field -> growth rule -> path -> node dynamics
```

Every scale follows the same pattern:
```
activation -> connection -> path -> selection -> stabilization -> new state
```

## Adversarial Testing Results

### Basic (8/8 passed)
- Geometrical meeting without co-activation: PASS
- Low evidence pruning: PASS
- Phase boundary transitions: PASS
- Gradient normalization: PASS
- Graph entropy bounds: PASS
- Restart continuity: PASS
- Anastomosis validation: PASS
- Metric serialization: PASS

### Deep (7/7 passed, 5 warnings)
- C4 promotion rule: PASS
- False co-activation: PASS (warning: semantic vs statistical)
- Competing attractors: PASS
- Restart + environment change: PASS (warning: blind restore)
- **Causal laundering: DETECTED** (warning: self-reinforcing structure)
- Cross-scale conflict: PASS (warning: local vs global)
- Emergent bad decision: PASS (warning: false confidence)

## Documentation

| Document | Size | Content |
|----------|------|---------|
| `HOPE-AGENT-SPEC.md` | Verified spec | All claims backed by code |
| `COGNITIVE_MORPHOGENESIS.md` | 24 KB | Architecture document |
| `FULL_SYSTEM_DESCRIPTION.md` | 24 KB | Complete system description |
| `ECOSYSTEM_DESCRIPTION.md` | 15 KB | Ecosystem overview with benchmarks |

## Build

```bash
# Native
cargo build --release

# WASM
cargo build --target wasm32-unknown-unknown --release --features wasm --no-default-features
```

## License

MIT