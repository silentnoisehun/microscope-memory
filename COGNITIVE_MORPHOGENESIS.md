# Cognitive Morphogenesis — Architecture Reference

> **Source files**: `src/cognitive_morphogenesis.rs` (826 lines), `src/morphogenesis.rs` (2,200 lines)

Cognitive Morphogenesis is the integration engine that connects all other cognitive modules into a single adaptive system. It does not reason or store memories itself — it orchestrates *how* the system learns, forgets, and grows based on live signals from every other module.

---

## 1. The Cognitive Gradient

Every recall cycle computes a seven-component gradient that determines where and how memory connections should grow:

```
gradient = w₁·relevance + w₂·resonance + w₃·evidence
         + w₄·hebbian   + w₅·prediction + w₆·emotion + w₇·execution
```

The weights default to 1.0 each. All inputs are clamped to [0, 1] before weighting. The gradient is then normalized by the total weight sum.

### Components

| Component | Source module | Meaning |
|---|---|---|
| `relevance` | lexical search score | How well this block matched the query |
| `resonance` | `resonance.rs` | Pulse-based federation strength |
| `evidence` | `epistemic.rs` | Confidence in the memory's factual accuracy (0–100 → 0.0–1.0) |
| `hebbian` | `hebbian.rs` | Co-activation energy (decays with half-life of 24 h) |
| `prediction` | `predictive_cache.rs` | Hit rate of recent predictive prefetches |
| `emotion` | `emotional_contagion.rs` | Emotional valence (−1.0–1.0 → 0.0–1.0) |
| `execution` | morphogenesis audit | Success rate of recent growth operations |

**Implementation** (`cognitive_morphogenesis.rs:148`):

```rust
let relevance  = inputs.lexical_score.clamp(0.0, 1.0) as f64;
let resonance  = inputs.resonance_strength.clamp(0.0, 1.0) as f64;
let evidence   = (inputs.evidence_confidence as f64 / 100.0).clamp(0.0, 1.0);
let hebbian    = inputs.hebbian_energy.clamp(0.0, 1.0) as f64;
let prediction = inputs.prediction_hit_rate.clamp(0.0, 1.0) as f64;
let emotion    = ((inputs.emotional_valence + 1.0) / 2.0).clamp(0.0, 1.0) as f64;
let execution  = inputs.execution_success.clamp(0.0, 1.0) as f64;
```

### Absentia Shadow Term

Before the gradient drives growth, it is modulated by what is *absent*:

```
effective_gradient = gradient × (1 − absence_shadow)
```

The `AbsentiaState` module tracks memory areas that have been queried but never returned results. The shadow term suppresses growth toward empty regions, without blocking exploration entirely. A region with no evidence produces a full shadow (1.0); a region with strong evidence produces no shadow (0.0).

---

## 2. Phase Transitions

The average gradient across a cycle determines the system's growth phase:

| Gradient | Phase | Behaviour |
|---|---|---|
| < 0.3 | **GAS** | Free exploration — high branching probability (0.5), low decay (0.03), minimal anastomosis (0.02). The system tries many directions. |
| 0.3–0.7 | **LIQUID** | Gradient following — balanced branching (0.35), active decay (0.08), moderate anastomosis (0.08). Active learning and binding. |
| > 0.7 | **SOLID** | Consolidated paths — low branching (0.1), low decay (0.02), strong anastomosis (0.15). High-confidence execution mode. |

Each phase sets a different `GrowthConfig` applied to the biological growth algorithms below.

---

## 3. Biological Growth Algorithms (`morphogenesis.rs`)

The growth algorithms are drawn from biological systems. They operate on `MorphNode` / `MorphConnection` / `MorphogenField` structures representing a 3D cognitive field.

### Mycelium Growth

Fungal-network-inspired branching. Nodes spawn child connections with configurable branching probability. Anastomosis (loop formation) merges nearby tips when they come within range, producing the dense interconnected network seen in P2P and distributed topologies.

**Use case**: large-scale associative recall webs, federation overlays.

### Capillary Growth

Fractal branching with exponential diameter reduction. Each branch generation is narrower and shorter. Produces strict hierarchical structures suitable for data-flow pipelines and hierarchical cache layouts.

**Use case**: layered memory hierarchies, depth-indexed access paths.

### Slime Mold (Physarum)

Two-phase solver: (1) nutrient gradient following from seed to target, (2) edge reinforcement proportional to flow. Weak edges decay; strong edges thicken. Converges to near-optimal routing paths.

**Use case**: shortest-path recall, memory routing between distant layers.

### Fractal L-System

Rewriting rules expand a seed symbol into a self-similar structure. Branching angle, length ratio, and depth are configurable. Produces recursive tree topologies.

**Use case**: microservice trees, recursive cognitive subgraphs.

### Hybrid

Any combination of the above, applied sequentially or conditionally. The integration engine selects the algorithm based on current phase and query type.

---

## 4. The Morphogenesis Cycle (T0–T5)

Each cycle is audited in a `MorphogenesisAuditEntry`:

| Step | Name | What happens |
|---|---|---|
| T0 | **Activation** | Recall fires; active blocks are identified and scored |
| T1 | **Gradient** | Seven-component gradient is computed; phase is determined |
| T2 | **Growth** | New nodes and connections are added under the phase-specific `GrowthConfig` |
| T3 | **Anastomosis** | Nearby tips are merged; spurious loops are pruned |
| T4 | **Reinforcement** | Prediction hit/miss feeds back into evidence confidence |
| T5 | **Consolidation** | Stable paths are solidified; weak paths below threshold are pruned |

---

## 5. Metrics

Each cycle produces `MorphogenesisMetrics`:

| Metric | Description |
|---|---|
| `recall_precision` | Fraction of recalled blocks that matched the semantic query |
| `prediction_hit_rate` | Fraction of prefetches that were subsequently requested |
| `false_association_rate` | Connections formed that were later pruned as spurious |
| `graph_entropy` | Shannon entropy of the degree distribution |
| `path_stability` | Fraction of paths that survived since the previous cycle |
| `convergence_cycles` | Consecutive cycles without structural change |
| `counterevidence_reaction_rate` | Speed at which the system revises beliefs under new evidence |
| `restart_continuity` | Fraction of paths recovered after a full rebuild |

---

## 6. Integration Points

```
cognitive_morphogenesis.rs
  ├─ hebbian.rs          → co-activation energy (ActivationRecord, 24h half-life)
  ├─ resonance.rs        → pulse-based strength signals
  ├─ epistemic.rs        → evidence confidence and promotion gates
  ├─ predictive_cache.rs → prefetch hit rate
  ├─ emotional_contagion.rs → emotional valence
  ├─ absentia.rs         → shadow term (what is missing)
  └─ morphogenesis.rs    → growth algorithms (mycelium, capillary, slime mold, L-system)
```

The morphogenesis engine runs on explicit invocation (`microscope-mem morphogenesis run`) or via the autonomous daemon at the configured interval. It does not run on every store or recall — it is a background integration pass, not a hot-path component.
