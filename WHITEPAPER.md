# Microscope Memory: An Adaptive Hierarchical Memory Index with Reinforcement Signals

**Author:** Mate Robert (Silent)

**Version:** 0.8.2

**Date:** September 2026

**DOI:** 10.5281/zenodo.22983478
**ORCID:** 0009-0003-3986-6039
**Code:** https://github.com/silentnoisehun/microscope-memory

---

## Abstract

This paper presents Microscope Memory, a hierarchical memory index implemented in Rust that models information retrieval as magnification: data is organised into nine depth levels (D0--D8), from identity summaries to raw bytes, with every block constrained to a 256-byte viewport. Beyond the core indexing engine, the system implements thirteen adaptive-ranking layers that let retrieval outcomes feed back into the index. These are described in engineering terms throughout: Hebbian reinforcement (block-level activation and coordinate drift), activation-fingerprint matching, spatial pulse propagation, archetype extraction from recurring activation patterns, query-space warping, recall-path tracking, predictive prefetch with reinforcement feedback, time-windowed activation profiles, a learned attention weight vector, cross-instance pattern exchange, offline consolidation with pruning, shared state propagation across instances, and multi-modal memory (images, audio, structured data). The system achieves sub-microsecond in-process query latencies at shallow depths while maintaining reinforcement loops at multiple levels -- predictions, attention weights, and temporal profiles all adapt from observed usage. Pure binary, zero JSON, 54,053 lines of Rust.

**Scope of the claims.** This paper describes an engineering artefact and its measured behaviour. It makes no claim about consciousness, sentience, or cognition in the strong sense; "memory" throughout means an index with learned relevance signals. Every performance number is tied to a stated measurement path; see Section 11 and the Limitations section for what is *not* claimed.

---

## 1. Introduction

The dominant paradigm in AI memory systems relies on embedding vectors and approximate nearest-neighbor search. While effective for semantic similarity, these approaches treat memory as static storage Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž data goes in, query comes out, nothing changes between accesses.

Biological memory works differently. Every act of recall modifies the memory itself: neural pathways strengthen through use (Hebbian learning), similar patterns resonate across brain regions (mirror neurons), and recurring activation patterns crystallize into abstract concepts (archetypes). Memory is not a database Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž it is a living structure that self-organizes through use.

Microscope Memory implements this principle in a pure binary system. The zoom metaphor provides efficient hierarchical access (37ns at D0 to 500us at D8), while thirteen reinforcement layers turn each recall into a signal that reshapes later retrievals.

---

## 2. Core Architecture

### 2.1 Binary Format

Three primary binary files with no serialization overhead:

- **`microscope.bin`** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž Block headers (32 bytes each, mmap'd). The first 16 bytes (x, y, z, zoom) load directly into SSE registers for SIMD distance computation.
- **`data.bin`** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž Raw UTF-8 text content, referenced by offset and length from headers.
- **`meta.bin`** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž Index metadata (MSC3 format): magic, version, block count, depth ranges, Merkle root, layers hash.

Supporting files: `merkle.bin` (SHA-256 tree), `embeddings.bin` (mmap'd vectors), `append.bin` (hot memory log), `append_embeddings.bin` (vectors for entries still in the append log, keyed by position).

### 2.2 Depth Hierarchy (D0--D8)

| Depth | Name | Content |
|-------|------|---------|
| D0 | Identity | System-level identity (single root block) |
| D1 | Layer Summaries | Per-layer overview (9 blocks) |
| D2 | Clusters | Groups of 5 items |
| D3 | Items | Individual memory entries |
| D4 | Sentences | Sentence-level splits |
| D5 | Tokens | Word-level (max 8 per parent) |
| D6 | Syllables | 3--5 character morpheme chunks |
| D7 | Characters | Individual characters |
| D8 | Raw Bytes | Hexadecimal byte representation |

Below D8, decomposition destroys meaningful information Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the "atomic boundary of information."

### 2.3 Spatial Memory Model

Content is projected into 3D space via deterministic FNV hashing, with each of the ten cognitive layers occupying a distinct spatial region. Coordinates are computed as:

```
(x, y, z) = (layer_offset + hash * 0.25)
```

This ensures identical content always maps to the same coordinates, content within the same layer clusters spatially, and different layers occupy non-overlapping regions. Child blocks at deeper depths inherit parent coordinates with fractal perturbations.

### 2.4 Build Pipeline

Index construction uses Rayon-based parallelism at D4--D8. Post-build automatically:
1. Applies Hebbian drift deltas to block header coordinates
2. Generates structural fingerprints and wormhole links
3. Rebuilds embedding index

Builds are incremental Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž SHA-256 content hash of layer sources is stored in MSC3 meta.

---

## 3. Adaptive Ranking Layers

The thirteen layers below are reinforcement mechanisms: each observes retrieval
outcomes and adjusts scoring or state so that later retrievals differ from
earlier ones. They are independent modules, each with its own on-disk state
file, and each can be disabled to measure its contribution. The biological names
are retained in the source for continuity with the project's history; the
mechanisms are described here in engineering terms.

### 3.1 Layer 1: Hebbian Reinforcement (`hebbian.rs`)

The core mechanism: reinforcement layers that turn a passive read into an
event that updates index state.

Every block has an activation record tracking: activation count, last activation
time, energy (decaying with 24h half-life), and coordinate drift deltas (dx, dy,
dz).

### 3.2 Layer 2: Activation Fingerprints (`mirror.rs`)

Matches blocks that were retrieved together under similar queries, boosting
co-retrieved neighbours.

When a recall activates blocks, the system:
1. Increments activation counters and resets energy to 1.0
2. Records co-activation pairs for all result block combinations
3. Stores an activation fingerprint (8D vector) for mirror neuron resonance

**Coordinate drift**: Co-activated blocks accumulate small drift deltas (0.01 per step, max 0.1). During rebuild, these deltas are applied to the actual block header coordinates in `microscope.bin`. Over time, frequently co-accessed blocks physically migrate closer in 3D space, creating organic memory clusters.

Binary formats: `activations.bin` (HEB1), `coactivations.bin` (COA1).

#### 3.2.1 Mirror Neurons (`mirror.rs`)

Activation fingerprints from L1 are compared via sparse cosine similarity. When two fingerprints (from different queries) exceed a threshold, a resonance echo is created, boosting the block's future retrieval score.

Each block accumulates a `block_resonance` value Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the sum of echo strengths it has received. Echoes decay over time, so only actively resonating blocks maintain their boost.

Binary format: `resonance.bin` (RES1).

### 3.3 Layer 3: Spatial Pulse Propagation (`resonance.rs`)

Each Hebbian activation emits a pulse into a quantized spatial field (0.05 grid resolution). The field is a sparse HashMap of `(i16, i16, i16)` grid cells to `f32` strength values.

Pulses carry: source instance ID, spatial coordinates, layer hint, and strength. They can be:
- **Emitted** locally from recall activations
- **Exchanged** across federated indices via the PXC1 wire format
- **Integrated** into local Hebbian state (receiving pulses from other instances)

The field decays over time, creating transient "hot spots" where repeated activations converge.

Binary formats: `pulses.bin` (PLS1), wire format (PXC1).

### 3.4 Layer 4: Pattern Archetypes (`archetype.rs`)

Hot spots in the resonance field crystallize into archetypes Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž persistent named patterns that represent recurring themes in the memory landscape.

Detection algorithm:
1. Find cells in the resonance field above a strength threshold
2. Cluster nearby Hebbian-active blocks around each hot spot
3. If a cluster has sufficient members and strength, it becomes an archetype
4. Auto-label from the most common words in member block content

Archetypes reinforce when activation patterns overlap their members, creating a positive feedback loop. Archetypes decay when not reinforced.

Binary format: `archetypes.bin` (ARC1).

### 3.5 Layer 5: Query-Space Warping (`emotional.rs`)

The emotional layer (layer_id=4 in the cognitive layer schema) receives special treatment. Active emotional blocks create an "emotional centroid" Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the energy-weighted average of their 3D coordinates.

Before search, query coordinates are warped toward this centroid:

```
warped = query + (centroid - query) * weight
```

The weight is configurable (0.0 = disabled, 1.0 = fully warped to emotional centroid). This means the system's current emotional state subtly bends all searches Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž memories associated with active emotions become easier to reach.

### 3.6 Layer 6: Recall Path Graph (`thought_graph.rs`)

While L1--L5 operate at the block level, L6 operates at the **path level** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž tracking sequences of recalls over time.

Every recall creates a **ThoughtNode** (timestamp, query hash, session ID, dominant layer). Consecutive recalls within the same session form **directed edges**. A 30-minute gap starts a new session.

**Pattern detection** uses sliding-window n-grams (lengths 2--5) over the current session's query hashes. When:
- All constituent edges have been traversed Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąÄľ2 times
- The sequence has been observed Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąÄľ3 times (PATTERN_MIN_FREQ)

...the sequence crystallizes into a **ThoughtPattern** that boosts future searches matching the same thought path.

This is how the system learns to "think in patterns" Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž recognizing that after querying about "Ora" then "memory", the user typically asks about "Rust" next, and pre-positioning results accordingly.

Binary formats: `thought_graph.bin` (THG1), `thought_patterns.bin` (PTN1).

### 3.7 Layer 7: Predictive Prefetch (`predictive_cache.rs`)

L7 closes the feedback loop. Based on L6's crystallized patterns, the cache predicts which blocks the user will need **before the query executes**.

After each recall:
1. **Predict**: Check if the current session path is a prefix of any known pattern. If so, pre-load the pattern's result blocks into the cache with a confidence score.
2. **Check**: On the next recall, if the query hash matches a cached prediction, instantly boost the pre-fetched blocks.
3. **Evaluate**: After search completes, compare prediction against actual results:
   - **Hit** (Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąÄľ50% overlap or Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąÄľ3 blocks): reward source pattern (+0.3 strength)
   - **Partial hit**: proportional reward
   - **Miss** (0 overlap): penalize source pattern (-0.05 strength), halve cache confidence

This creates a reinforcement loop:
```
Good pattern Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Accurate prediction Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Hit Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Pattern strengthened Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Better prediction
Bad pattern Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Wrong prediction Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Miss Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Pattern weakened Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â Eviction
```

Over time, only reliably predictive patterns survive. The system tracks total predictions, hits, misses, and partial hits for observability.

Binary format: `predictive_cache.bin` (PRC1).

### 3.8 Layer 8: Time-Windowed Profiles (`temporal_archetype.rs`)

Time introduces a dimension that spatial clustering alone cannot capture. Temporal Archetypes track when each archetype is most active across six 4-hour windows (00--04, 04--08, 08--12, 12--16, 16--20, 20--24).

Each archetype maintains a `TemporalProfile`:
- **Window counts** (6 values): raw activation count per time window
- **Window weights** (6 values): normalized activation density per window
- **Total activations**: lifetime activation count

When an archetype is activated during recall, its current time window's count increments. The system computes a **temporal boost** for search results:
- A **dominant window** is identified (the window with the highest weight, requiring Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąÄľ5 total activations)
- If the current query falls within the dominant window: boost = 1.0
- Otherwise: boost scales down proportionally to the off-peak window's weight

This allows the system to learn circadian patterns Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž for example, that "work" archetypes activate during 08--12 and "creative" archetypes during 20--24.

Profiles decay over time (factor 0.99 per cycle), ensuring recent temporal patterns take precedence over historical ones.

Binary format: `temporal_archetypes.bin` (TAR1), 56 bytes per record.

### 3.9 Layer 9: Learned Attention Weights (`attention.rs`)

Layers L1--L8 each contribute to the recall pipeline, but their relative importance varies with context. The Attention Mechanism dynamically weights each layer based on the current query.

**Input signals** (computed per query):
- `query_length`: normalized query complexity (0.0--1.0)
- `emotional_energy`: total Hebbian energy in emotional blocks (0.0--1.0)
- `session_depth`: how deep into a session the user is (recall count / 50, capped)
- `pattern_confidence`: strongest ThoughtGraph pattern match (0.0--1.0)
- `cache_hit_rate`: PredictiveCache running hit rate (0.0--1.0)
- `archetype_match_score`: best archetype match score (0.0--1.0)

**Attention computation**: Each signal maps to a 7-dimensional weight vector via fixed rules (e.g., long queries boost spatial search, high emotion boosts emotional bias). These raw weights are blended 80/20 with **learned weights** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž persistent per-layer multipliers that adapt over time.

**Quality inference**: The system infers whether the previous recall was "good" or "bad" from the time gap to the current recall:
- **>60 seconds**: satisfied (quality = 1.0) Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the user found what they needed
- **<5 seconds**: unsatisfied (quality = 0.2) Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž immediate re-query suggests failure
- **5--60 seconds**: linear interpolation

**Weight learning**: Good outcomes' attention vectors are averaged per layer. Bad outcomes' vectors are averaged. The learned weight for each layer shifts toward what worked and away from what didn't, via exponential moving average (rate = 0.05). Weights are clamped to [0.1, 3.0].

Binary format: `attention.bin` (ATT1), header 48 bytes + 40 bytes per outcome (200 cap).

### 3.10 Layer 10: Cross-Instance Learning (`federation.rs`)

While L3 already exchanges resonance pulses across federated indices, L10 extends this to higher-order knowledge: **ThoughtGraph patterns** and **PredictiveCache statistics**.

**Pattern exchange**:
1. Export local ThoughtGraph's crystallized patterns
2. For each federated index, import their patterns with trust weighting
3. Trust = source's PredictiveCache hit rate Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â€šÂ¬ÄąÄ„ federation weight
4. Low-trust patterns are imported at reduced strength, preventing unreliable peers from polluting local knowledge

**Stats aggregation**:
- Total predictions, hits, and misses are merged bidirectionally
- This allows each instance to benefit from the collective prediction accuracy of the federation

The exchange is triggered explicitly via the `pattern-exchange` CLI command, giving operators control over when cross-pollination occurs.

### 3.11 Layer 11: Offline Consolidation (`dream.rs`)

Offline consolidation replays and prunes Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž replaying the day's experiences, strengthening important connections, and pruning noise. The dream command implements this in Microscope Memory.

The `dream` command runs an offline consolidation cycle:

1. **Replay**: Scan Hebbian fingerprints from the last 24 hours. For each, partially re-energize the activated blocks (0.3 energy vs 1.0 for real activation).
2. **Strengthen**: Track which co-activation pairs appear across multiple replayed fingerprints. Pairs appearing in Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąÄľ3 fingerprints get their count multiplied by 1.5.
3. **Prune pairs**: Remove co-activation pairs with count Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â°Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â¤1 that are older than 48 hours. These are noise Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž connections that never reinforced.
4. **Prune activations**: Zero out activation records with near-zero energy and zero activation count. These are dead blocks consuming state.
5. **Pattern consolidation**: Run ThoughtGraph pattern detection across all recent sessions, potentially crystallizing new thought patterns.
6. **Field decay**: Apply 0.8Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â€šÂ¬ÄąÄ„ decay to the resonance field and expire old pulses, preventing stale spatial information from persisting.
7. **Cache cleanup**: Remove predictive cache entries with confidence below 0.1.

Each cycle is logged with statistics: replayed fingerprints, strengthened pairs, pruned entries, energy before/after.

Binary format: `dream_log.bin` (DRM1), 40 bytes per cycle record.

### 3.12 Layer 12: Cross-Instance State Sharing (`emotional_contagion.rs`)

While L5 warps the local search space based on local emotional blocks, L12 extends this across federated instances Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž creating shared emotional context.

Each instance maintains an **EmotionalSnapshot**: centroid (energy-weighted average of active emotional block coordinates), total energy, active block count, and **valence** (-1.0 to +1.0).

Valence is computed from the text content of active emotional blocks using keyword-based sentiment analysis, supporting both English and Hungarian word lists.

**Contagion mechanics**:
- Local snapshots are captured during federation exchanges
- Remote snapshots are stored with source ID and timestamp
- The **blended centroid** is a weighted average of local and remote emotional centroids
- Local weight is configurable (default 0.7 = 70% local influence, 30% remote)
- Remote snapshots decay by recency (linear from 1.0 at fresh to 0.1 at 48h)
- Expired snapshots (>48h) are excluded from blending

Binary format: `emotional_field.bin` (EMO1), wire format: `EXS1`.

### 3.13 Layer 13: Multi-Modal Storage (`multimodal.rs`)

Memory is not limited to text. L13 extends the block system to store and recall images, audio, and structured data within the same spatial coordinate framework.

The core `BlockHeader` (32 bytes, mmap-aligned) is unchanged. Instead, `modalities.bin` acts as a **sidecar index** mapping block indices to their modality metadata:

- **Image**: width, height, perceptual hash (dHash, 8 bytes), quantized color histogram (12 bytes), content hash
- **Audio**: duration, sample rate, spectral fingerprint (16 frequency bands), peak frequency, BPM estimate
- **Structured**: typed key-value pairs (string, int, float, bool)

**Search by modality**:
- Image similarity: Hamming distance on perceptual hashes (lower = more similar)
- Audio similarity: normalized dot product of spectral fingerprints
- Structured: exact field name + value matching

**Spatial integration**: each modality computes deterministic 3D coordinates from its features Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž images from phash bytes (in the associative region), audio from spectral features (in the echo_cache region), structured from field name hashing (in the rust_state region). This ensures multi-modal blocks participate naturally in spatial search.

Binary format: `modalities.bin` (MOD1), variable-length entries.

---

## 4. The Complete Recall Pipeline

Every `recall` command triggers the full reinforcement stack:

```
 1. Load reinforcement state (Hebbian, mirror, resonance, archetypes, thoughts, cache, temporal, attention)
 2. Compute attention weights from query signals (L9)
 3. Infer quality of previous recall from inter-recall timing (L9)
 4. Compute query coordinates (content hash + semantic blend)
 5. Check predictive cache Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž instant boost if prediction exists, scaled by attention weight (L7)
 6. Apply emotional bias warp, scaled by attention weight (L5)
 7. Search across zoom-appropriate depths (L2 distance + keyword boost)
 8. Apply ThoughtGraph pattern boost, scaled by attention weight (L6)
 9. Sort and display results
10. Record Hebbian activation and co-activations (L1)
11. Detect mirror neuron resonance (L2)
12. Emit resonance pulse into spatial field (L3)
13. Reinforce matching archetypes (L4)
14. Track temporal archetype activation (L8)
15. Record thought graph node and edges (L6)
16. Evaluate prediction accuracy Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž hit/miss/partial (L7)
17. Predict next: pre-fetch blocks for likely next query (L7)
18. Mark recall in attention history (L9)
19. Save all state
```

Steps 2--8 happen **before** display (affecting result ranking). Steps 10--18 happen **after** display (learning from the recall).

---

## 5. Supporting Systems

### 5.1 Structural Fingerprinting

Each block receives a structural fingerprint: Shannon entropy, 16-bucket byte histogram, and FNV-1a hash. Blocks with similar fingerprints are connected by "wormhole links" Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž structural shortcuts across layers and depths.

Binary formats: `fingerprints.idx` (FGP1), `links.bin` (LNK1).

### 5.2 Radial Search

Depth-constrained radius search with SIMD acceleration. Returns a `ResultSet` containing primary matches and distance-weighted neighbors. Used for Hebbian co-activation recording.

### 5.3 Multi-Index Federation

Multiple Microscope indices can be queried in parallel with weighted result merging. Federation also supports activation pulse exchange Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž activation state can propagate across instances.

### 5.4 MQL (Microscope Query Language)

Structured queries with layer, depth, spatial, keyword, boolean, and limit filters:
```
layer:long_term depth:2..5 near:0.2,0.3,0.1,0.05 "Ora" AND "memory" limit:20
```

### 5.5 Visualization

Three levels of visualization output:

1. **Cognitive Map** (`cognitive-map` command): Full 13-layer export as JSON Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž blocks with Hebbian drift, co-activation edges, resonance wave field, archetypes with temporal profiles, thought paths, crystallized patterns, predictive cache stats, attention weights, dream cycle history, emotional contagion state, multi-modal stats, and mirror echoes. Ships with an interactive **Three.js viewer** (`viewer.html`) that auto-opens in the browser, featuring:
   - Per-feature toggles (blocks, edges, wave field, thought paths, archetypes, dreams, echoes, emotional centroid)
   - Per-layer visibility toggles with color-coded swatches
   - Collapsible sidebar panels (stats, attention weights, emotional field, predictions)
   - Animated wave field pulsing, dream cycle energy visualization, archetype temporal rings

2. **Basic Snapshot** (`viz` command): JSON export of blocks, edges, field, archetypes, echoes, and aggregate stats.
3. **Density Map** (`density` command): Binary DEN1 format Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž quantized 3D grid of Hebbian energy for fast volumetric rendering.

---


## 6. Performance

Benchmarked on 227,168 blocks (10,000 queries per depth):

| Depth | Blocks | Query Time | Cache Tier |
|-------|--------|------------|------------|
| D0 | 1 | **37 ns** | L1d |
| D1 | 9 | **92 ns** | L1d |
| D2 | 108 | **506 ns** | L1d |
| D3 | 523 | **1.7 us** | L2 |
| D4 | 1,349 | **3.9 us** | L2 |
| D5 | 6,070 | **18 us** | L2/L3 |
| D6 | 26,198 | **72 us** | L3 |
| D7 | 96,297 | **505 us** | L3 |
| D8 | 96,613 | **492 us** | L3 |

The reinforcement layers add minimal overhead per recall: state files are loaded once, learning operations are O(kÄ‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬Ä…Ä‚ËĂ˘â€šÂ¬ÄąĹş) where k is the result count (typically 5--10), and binary I/O is sequential with no allocation during the hot path.

The predictive cache, when warmed, provides effectively **zero-cost** result boosting Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž pre-fetched blocks are a simple HashMap lookup before the spatial search begins.

---

## 7. Binary Formats Summary

| File | Magic | Purpose |
|------|-------|---------|
| `microscope.bin` | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž | Block headers (32B each, mmap'd) |
| `data.bin` | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž | Raw UTF-8 text content |
| `meta.bin` | MSC3 | Index metadata, Merkle root, layers hash |
| `merkle.bin` | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž | SHA-256 Merkle tree |
| `embeddings.bin` | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž | Pre-computed embedding vectors |
| `append.bin` | APv2 | Hot memory append log |
| `append_embeddings.bin` | AEM1 | Vectors of append-log entries, by position |
| `activations.bin` | HEB2 | Hebbian activation records, sparse (only non-default) |
| `activations_delta.bin` | AEM2 | Append-only, CRC-checked journal of activation updates |
| `coactivations.bin` | COA1 | Co-activation pairs |
| `fingerprints.idx` | FGP1 | Structural fingerprints |
| `links.bin` | LNK1 | Wormhole links |
| `resonance.bin` | RES1 | Mirror neuron state |
| `pulses.bin` | PLS1 | Resonance pulses |
| `archetypes.bin` | ARC1 | Emerged archetypes |
| `thought_graph.bin` | THG1 | Recall path graph (nodes + edges) |
| `thought_patterns.bin` | PTN1 | Crystallized thought patterns |
| `predictive_cache.bin` | PRC1 | Predictive block cache + stats |
| `temporal_archetypes.bin` | TAR1 | Temporal activation profiles (56B each) |
| `attention.bin` | ATT1 | Attention weights + quality history |
| `dream_log.bin` | DRM1 | Dream consolidation cycle history |
| `emotional_field.bin` | EMO1 | Emotional contagion state + remote snapshots |
| `modalities.bin` | MOD1 | Multi-modal sidecar index |

All binary formats use safe manual byte-level serialization (no unsafe pointer casts), little-endian encoding, and 4-byte magic headers for format identification.

---

## 8. Test Coverage

417 library tests plus 16 hook tests:

| Module | Tests | Coverage |
|--------|-------|----------|
| Hebbian | 10 | Activation, co-activation, drift, energy, serialization |
| Mirror | 9 | Sparse cosine, resonance detection, echo decay, boost |
| Resonance | 11 | Pulses, field, quantization, integration, wire format |
| Archetype | 8 | Detection, reinforcement, labeling, decay |
| Emotional | 5 | Warp math, zero weight, full weight, centroid |
| Fingerprint | 12 | Entropy, histograms, similarity, links, wormholes |
| ThoughtGraph | 10 | Nodes, edges, sessions, patterns, boost, ring buffer |
| PredictiveCache | 9 | Check, evaluate, hit/miss, predict, decay, roundtrip |
| TemporalArchetype | 7 | Time windows, activation, decay, boost, dominant window, roundtrip |
| Attention | 10 | Signals, normalization, quality inference, learning, history cap, roundtrip |
| Dream | 5 | Replay, strengthen, prune, no-fingerprints, stats, roundtrip |
| EmotionalContagion | 8 | Contagion weight, blend, valence, wire format, expiry, dedup, roundtrip |
| MultiModal | 11 | Phash, hamming, spectral, coords, image/audio/structured roundtrip, search |
| Core + others | 35 | CRC, MQL, cache, merkle, snapshot, embedding index |
| Retrieval, dedup, relevance, layout | 273 | Reader, writer, ranking scorer, content dedup, coordinate maths, bounds, safety |

The table above enumerates the per-module suites; the remaining 273 tests cover
the core reader/writer, query paths, serialization, and safety properties. The
authoritative total is the output of `cargo test --lib`, which reports the exact
figure at the time of writing rather than a hand-maintained count.

All tests use safe binary I/O roundtrip verification.

---

## 9. Future Work

**Narrative Memory.** Automatically linking sequences of recalls into coherent narratives Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž story arcs that emerge from thought patterns and can be replayed as structured episodes.

**Self-Modeling.** A meta-layer that observes the consciousness stack itself Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž which layers contribute most, how attention weights evolve, which dream cycles produce the most pruning Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž enabling the system to optimize its own parameters.

**Embodied Perception.** Extending multi-modal memory with real-time sensor fusion Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž camera feeds, microphone input, accelerometer data Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž for embodied AI applications.

---

## 10. Related Work

Microscope Memory sits at the intersection of hierarchical index structures,
learning-to-rank feedback loops, and reinforcement-based retrieval. This section
places the system relative to the established literature. The comparison is
deliberately conservative: where a claim is not backed by a measurement in this
paper, that is stated.

### 10.1 Hierarchical and multi-resolution indexing

The zoom-based D0--D8 structure follows the general idea of indexing data at
multiple resolutions so a query can be answered at a coarser level when detail
is not required:

- **B-trees and B+ trees** (Bayer & McIlroy, 1992) -- the canonical
  disk-resident multi-level index. Microscope is similar in that a query may
  touch a single level rather than the full key space, but replaces ordered keys
  with a spatial coordinate hierarchy over fixed-size blocks.
- **Quadtrees and k-d trees** (Bentley, 1975; Buchwald et al., 1989) --
  spatial indices that partition space recursively. Microscope's depth levels
  serve a comparable partitioning role, though the partition is implicit in
  block coordinates rather than materialised as child pointers.
- **Summary indexes and wavelet trees** -- pre-computed coarse structures that
  accelerate candidate selection; the D0 identity summaries serve this role.

The distinguishing choice is that Microscope fixes every block to 256 bytes and
stores all of them on an mmap'd plane. This makes index size predictable from
corpus size and removes deserialisation from the read path, at the cost of not
compressing text.

### 10.2 Learned ranking and feedback loops

Using retrieval outcomes to improve future ranking is established in IR:

- **Learning to Rank** (Liu, 2009) -- optimising a ranking function from
  relevance judgements. Microscope's relevance scorer is a hand-designed
  heuristic rather than a learned model, but occupies the same role: converting
  a match into a score.
- **Click models** (Joachims, Chapelle & Zhang, 2005) -- learning from implicit
  feedback. Microscope's Hebbian reinforcement records which blocks were
  retrieved and adjusts their coordinates, a simpler positional analogue.
- **Recency and importance weighting** are standard IR practice; the
  `importance` field and the reinforcement loop implement both.

### 10.3 Vector databases and ANN search

- **FAISS** (Johnson, Douze & JĂ„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â©gou, 2017) -- efficient similarity search.
- **HNSW** (Malkov & Yashunin, 2016) -- hierarchical navigable small-world
  graphs, the basis of several production vector stores.
- **Chroma, Qdrant, Weaviate, Pinecone** -- systems built on these indexes.

**A controlled comparison has been run**, on 60 facts and 60 questions, all
systems on the same machine and the same MiniLM vectors
([scripts/compare_baselines.py](scripts/compare_baselines.py)):

| System | p50 ms | R@1 | R@5 | R@10 |
|--------|--------|-----|-----|------|
| Microscope, D5 eval index, rebuilt at 16 KiB blocks (current) | 119.4 | 78.3% | 80.0% | 80.0% |
| Microscope, D5 eval index, before the padding fix | 323Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź332 | 70.0% | 80.0% | 81.7% |
| Microscope, D5 index, semantic path (pre-gate) | 331.0 | 33.3% | 48.3% | 51.7% |
| FAISS `IndexFlatIP` (60 fact vectors, query time only) | 0.007 | 90.0% | 96.7% | 96.7% |
| SQLite FTS5 (BM25, 60 facts, query time only) | 0.032 | 53.3% | 60.0% | 63.3% |

**The numbers 86.7 / 96.7 / 96.7% quoted in Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.1 and Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§12 are on a different index** and do not
belong in this table. Those are the 60-fact benchmark corpus
(`bench_config_semantic.toml`, 153 stored vectors), where the whole index is the
evaluation set. This table is the 967,587-block evaluation index with 13,640
stored vectors, where 60 blocks are the answers and the rest is noise the
ranking has to survive. Same harness, same vectors, same questions; the
difference is entirely how much there is to be wrong about. Quoting the
benchmark number here would quietly swap one corpus for another, which is the
mistake that made 42/48/49 unreproducible in the first place.

**With the quality gate in place the system beats a general-purpose lexical
index, and still loses to exhaustive vector search over the same vectors.**
Microscope answers 81.7% at k=10 where FAISS answers 96.7% and SQLite FTS5
answers 63.3%; pre-gate the same harness returned 51.7% and lost to both. The
remaining gap to the flat scan is 15 points Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž it was 45 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž while Microscope
costs end-to-end time against their query-only 0.007 ms and 0.032 ms.

The latency rows are not comparable, and the table should not be read as a
speed comparison: FAISS and FTS5 index only the 60 fact vectors and report
query-time search only, while Microscope scans a 967,587-block index and its
121 ms includes process start, BERT model load, query embedding and index open.

That 121 ms is itself a result worth stating plainly: before the padding fix
the same row read 323Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź332 ms, because every embedding ran a 512-token forward
for a sentence of ten. The accuracy gain from rebuilding (R@1 70.0 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 78.3%) and
the 2.7Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â€šÂ¬ÄąÄ„ latency gain come from the same commit.

What the same vectors *do* show is that the hierarchy is doing something a
flat vector scan does not. Given the same model, FAISS reaches 96.7% and the
full system 81.7%: the depth structure, the reinforcement layers and the
candidate filtering cost 15 points of accuracy and buy inspectability, bounded
memory, and a per-recall write path. On the 60-fact benchmark index, where
there is no noise to survive, the same system reaches 98.3% Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž so the 15 points
are spent on the other 699,050 blocks, not on the ranking. The pre-gate
diagnostic that located most of that loss is recorded in Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.1: the expected
answers were embedded and scored a mean cosine of 0.935, yet ranked 6,209th on
average out of 46,565 stored vectors, 36,136 of which were at most 16
characters of degenerate text. A build-time text-quality gate now removes
them.

Two caveats that bound the table. The measurement asymmetry favours the
baselines: their timings are query-only, while Microscope's include process
start-up, config load, state load, query embedding and the post-recall write.
And 60 facts is not a scale test Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž an exhaustive scan over 60 vectors is
trivially fast, and at 10^6 blocks the FAISS row would not remain sub-
millisecond.

### 10.4 Memory architectures for language agents

- **Generative Agents** (Park et al., 2023) -- agent memory as a stream of
  observations, retrieved with recency, importance and relevance scoring.
- **Reflexion** (Shinn et al., 2023) -- storing verbal feedback for later
  retrieval.
- **MemGPT / tiered context management** -- explicit paging between contexts.

Microscope overlaps in intent and differs in on-disk representation. The
three-part scoring used here (recency, importance, relevance) is close to that
used in generative-agent retrieval, and is presented as such.

### 10.5 Consolidation and forgetting

Offline replay-and-prune resembles sleep-dependent memory consolidation in
neuropsychology (Lewis & Durrant, 2011); the engineering pattern of periodic
compaction also appears in log-structured storage (O'Neil et al., 1996). The
naming in the source retains biological vocabulary for continuity; the mechanism
is a background replay pass that strengthens recently accessed blocks and
prunes unreferenced ones.

## 11. Evaluation

### 11.0 BEIR SciFact Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the public-corpus result

The rest of Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11 measures recall on 60 hand-written facts, and on a synthetic
967,587-block evaluation index. Both should be discounted accordingly: the
60-fact set stores only the answers, so retrieval is nearly trivial, and the
evaluation index cannot be rebuilt by the current code (see 11.1). SciFact is a
public claim-verification corpus, so the number is one anybody can rerun and
disagree with.

All 5,183 abstracts stored, 286 test queries, `MICROSCOPE_NO_LEARN=1`, one clean
run, `scifact_config.toml` with `max_depth = 3` Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the embedded set is one vector
per abstract, the same count FAISS embeds. Index: 5,183 blocks, 6,230 embedded,
1,331 MB. 14 of the 300 queries are excluded because no unique 3Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź12 word phrase
could be found for them; a match token appearing in more than one document would
credit a retrieval that found the wrong document.

| System | p50 ms | R@1 | R@5 | R@10 |
|---|---|---|---|---|
| **Microscope (recall, end-to-end)** | 600.6 | **53.1%** | **74.8%** | **81.8%** |
| FAISS `IndexFlatIP` (MiniLM) | 0.41 | 48.3% | 73.4% | 78.3% |
| FAISS `IndexHNSWFlat` (MiniLM) | 0.06 | 47.6% | 72.0% | 76.6% |
| SQLite FTS5 (BM25) | 8.06 | 45.8% | 66.8% | 74.8% |

Microscope leads on every k, with the lexical baseline last, which is the
expected ordering. Two limits on what this table supports:

- **The latency column is not a speed comparison, and not in Microscope's favour
  either.** It was previously explained here as "process start, config load and
  opening a 1.3 GB index". `bench-recall` shows that is wrong: in a resident
  process with everything warm, one recall on the 967,587-block evaluation index
  costs **26.5 ms at p50** against a first call of 95.1 ms, so process start
  and index load are worth about 13 ms of the *first* call and nothing per
  query. The per-query cost is real work, and by phase (mean over 31 calls) it is
  14.4 ms embedding the query, 8.8 ms scoring the candidate set, 2.1 ms vector
  search, 0.9 ms loading state, and under 1.5 ms for everything else -- 19 traced
  phases summing to 28.1 ms, so there is no unexplained remainder left. The gap
  to FAISS's 0.41 ms therefore stays real, and the query embedding alone is 35x
  it -- and FAISS does not pay that, because it searches pre-computed query
  vectors. The split has not been measured on the SciFact index, so the 600 ms
  there is not decomposed.
- **These are recall@k, not the nDCG@10 of the BEIR literature,** so they are
  not comparable to published SciFact results. Only the four rows are comparable
  to each other, and they share a corpus, a query set and a scorer.

Reproduce with `python scripts/build_scifact_index.py --force` followed by
`python scripts/compare_baselines.py --corpus scifact`; the output is
`docs/measurements/scifact_comparison.json`, which records the corpus and config
in the payload.

**How this number was reached, because the first two attempts were wrong.** At
`BLOCK_DATA_SIZE = 1024` the storage layer truncated 4,300 of the 5,183
abstracts at byte 1,021, so 83% of the corpus was never stored. That index
scored R@1 51.0% and led FAISS, which is why the truncation is worth naming
rather than quietly fixing. Splitting the tail into extra blocks recovered every
byte and dropped the score to 41.6%, because a whole abstract is a better
retrieval unit than two fragments of it. Storing one document per block at
16 KiB gives 53.1% on a complete corpus. The baseline rows are byte-identical
across all three runs, so the movement is the system, not the measurement.

The same investigation also fixed a latent defect in `data.bin` handling: the
file is packed and variable-length, with each block's span in its header, but
`dream.rs` addressed it as a fixed `idx * BLOCK_DATA_SIZE` grid. On an index
whose mean block is 5.6 bytes that read the wrong bytes for every block past the
first.

**There is no embedding defect.** Earlier revisions of this document recorded a
divergence between stored vectors and reference MiniLM encodings on long blocks
(0.9382 for 300-700 character blocks, 0.7410 for 700-1100). It was attributed
first to the 1,024-byte truncation and later to the provider's forward pass.
**Both attributions were wrong, and there was never a defect.**

`scripts/verify_stored_embeddings.py` measures all 13,640 embedded blocks of the
evaluation index against a from-scratch reference -- raw `tokenizers.Tokenizer`,
`AutoModel`, mean-pooled over the un-padded sequence -- and every one of them
scores **cosine 1.0000, minimum included**, in every length bucket from 25 bytes
to 4,096. The provider trims to the real token length and runs the model
without padding, and the correct comparison has to do the same.

The divergence was an artefact of the measuring script. `SentenceTransformer.encode`
is the obvious reference and it does not reproduce the provider on long inputs;
every earlier number came from using it. The lesson worth keeping: a correctness
discrepancy measured against a library convenience API is a claim about the library
until it has been reproduced by hand.

### 11.1 Retrieval quality, with the semantic path connected

> **Reproducing these numbers requires a feature-gated build.** The `candle` and
> `onnx` providers are behind the `embeddings` cargo feature, which is not in the
> default feature set (`default = ["native"]`):
>
> ```bash
> cargo build --release --features native,embeddings
> ```
>
> A build without it compiles and links cleanly, fails at run time with
> *"requires the 'embeddings' feature"*, and Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž because `build` still exits 0 on
> that path Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž leaves a script that checks only the exit code measuring an index
> with no vectors at all. The consequence is not a clean failure: it is a
> plausible-looking result table with near-zero recall, which is easily mistaken
> for a regression in the retriever. This was observed in practice during this
> work. `scripts/eval_real.sh` now builds with the feature, asserts that
> `embeddings.bin` exists afterwards, and re-checks the index after measuring.

**The measurement below is reproducible from the committed configuration.**

**Configuration:** `provider = "candle"`, `all-MiniLM-L6-v2`, dim 384,
`semantic_weight = 1.0`, `embedding.max_depth = 5`, index built from `layers/`
plus the 60 resonance facts Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž **967,587 blocks**, 60 questions, built three
times: once with the original `text.len() >= 3` admission (**46,565 stored
vectors**, the pre-gate rows below), once through the embedding quality gate
before the padding fix (**9,296 stored vectors**, the before row), and once
after it with the fixed parser and a floor of 20 characters (**13,640 stored
vectors**, the current rows). Vector count and depth were read back from the
`embeddings.bin` header (`dim=384 max_depth=5`).

| System | p50 ms | p95 ms | p99 ms | R@1 | R@5 | R@10 |
|--------|--------|--------|--------|-----|-----|------|
| Microscope, D5 eval index, rebuilt at 16 KiB blocks (current) | 119.4 | 130.2 | 142.7 | 78.3% | 80.0% | 80.0% |
| Microscope, D5 eval index, before the padding fix | 323.2 / 331.5 | 510.3 / 503.1 | 549.0 / 524.2 | 70.0% | 80.0% | 81.7% |
| Microscope, lexical only | 124.9 | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž | 30.0% | 43.3% | 48.3% |
| Microscope, D5, semantic path, `want`=64 (pre-gate) | 287.5 | 386.1 | 428.2 | 31.7% | 46.7% | 48.3% |
| Microscope, D5, semantic path, `want`=256 (pre-gate) | 331.0 | 451.9 | 505.2 | 33.3% | 48.3% | 51.7% |
| SQLite FTS5 (BM25), 60 facts, query time only | 0.032 | 0.158 | 0.178 | 53.3% | 60.0% | 63.3% |
| FAISS `IndexFlatIP`, 60 fact vectors, query time only | 0.007 | 0.009 | 0.019 | 90.0% | 96.7% | 96.7% |
| FAISS `IndexHNSWFlat`, 60 fact vectors, query time only | 0.017 | 0.014 | 0.019 | 90.0% | 96.7% | 96.7% |

The two Microscope rows that are labelled "current" in older revisions of this
document Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 70.0 / 80.0 / 81.7% at 323Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź332 ms Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž were measured before the padding
fix, on an index whose vectors were 99% padding. They are kept above as the
before row rather than deleted, because the comparison is the point: rebuilding
moved R@1 by 8.3 points and p50 by 2.7Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â€šÂ¬ÄąÄ„, and neither change is visible in
R@5 or R@10.

The 78.3 / 80.0 / 81.7% row was **superseded and has now been rebuilt**, so it is
current again. It had been built when `BLOCK_DATA_SIZE` was 1,024 bytes and the
layer reader packed consecutive lines into shared blocks, neither of which the
current code does. Rebuilt at 16,384 bytes: R@1 and R@5 returned identical
(78.3% / 80.0%), p50 moved 1.9 ms, and **R@10 fell from 81.7% to 80.0%**. That
last figure is one case in 60; a single run cannot resolve a one-case difference
at this sample size, so R@10 should be read as 80% Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â± one case rather than as a
regression. The 60 queries are too few to say more. Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.0 remains the
measurement worth arguing with, because this index is synthetic.

The 86.7 / 98.3 / 98.3% figures in Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.1 come from a *different* index --
`bench_config_semantic.toml`, the 60-fact benchmark corpus, 81 stored vectors,
where the evaluation set is the entire index. Putting them in this table would
swap a 967,587-block index for a 81-block one without saying so. On this index
the system is at 81.7% at k=10; on the benchmark index it is at 98.3%. Both are
real; only one of them is evidence about a corpus with anything in it to be
wrong about.

**Where the correct answer is lost.** A diagnostic pass classified every one of
the 31 pre-gate misses, using the match tokens passed to the binary via
`MICROSCOPE_EVAL_MATCH` (diagnostic only Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž they never enter the ranking):

| Class | Count | Meaning |
|-------|-------|---------|
| (a) not in the vector list | 21 | The correct block is absent from the cosine top-1024 entirely |
| (b) lost in the pre-fetch | 6 | Present in the top-1024, dropped by the `want`-entry cut |
| (c) lost in the final ranking | 4 | Admitted, but ranked below k by the combined score |

**Class (a) is the largest group Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž measured on the pre-gate index Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž and four
hypotheses for it were tested and rejected.** The depth histogram of the
returned hits is almost degenerate
(typically `[0, 0, 0, 0, 3, 1021]`), which suggested that D5 crowds out every
other depth. Extending the diagnostic to report each answer's score and rank
across the *whole* 46,565-vector set showed the opposite of a scoring problem:

- All 60 answers **are** embedded (60/60).
- Mean cosine of the expected block is **0.935** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž a strong match.
- Mean full rank is **6,209** of 46,565. Only 36/60 are inside rank 256.

So roughly six thousand blocks score *higher* than a 0.935 match. Four
explanations were implemented and measured, and only the first survived:

| Hypothesis | Experiment | Result |
|---|---|---|
| Pre-fetch too shallow | `want` 64 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 256 | **helps**: R@5 46.7% Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 48.3% |
| Query needs a deeper list | `want` Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 2048 | **no change** in R@k; p50 roughly doubles |
| Bit-identical duplicate vectors flood the list | cap identical vectors | **no change** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the apparent 1,996-block cluster was a quantisation artefact; the blocks are *near*-duplicates, not byte-identical |
| The spatial term drowns the cosine bonus | scale spatial 1.0 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 0.0 | **no change** at any scale |

Raising `want` to 2048 admits the deeply-ranked answers and still changes
nothing, so the obstruction is not a shortage of candidates. Removing the
spatial term entirely also changes nothing, so it is not the spatial term.
**The honest conclusion was that the ranking is behaving correctly and the
corpus is the problem**: 36,136 of the 46,565 embedded blocks were at most 16
characters of degenerate text, so an exact answer did not stand out in the
vector space. The remedy had to be corpus-level Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž applied at build time,
before embedding Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž not a reweighting of the score. It is implemented and
measured in the next block.

**The corpus-level remedy, implemented Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž as a text-quality gate, not a
similarity filter.** The pre-gate diagnosis pointed at the corpus, but the fix
had a hard constraint: of 40,000 sampled high-cosine pairs, 1,666 differ in
numbers and 245 differ in negation, so any dedup keyed on cosine similarity
would merge contradictory facts. The implemented gate therefore filters on
what the text *is* (`src/embedding_index.rs`): a 24-character floor
(`MICROSCOPE_MIN_EMBED_CHARS`, default 24), rejection of the reader's
`"<bin>"` / `"[out of bounds]"` sentinels, and rejection of text whose
characters are more than 25% in the U+0080..U+02FF mojibake band. Nothing
about how blocks score is used, so nothing two blocks *say* can be merged.

Measured on the same 60 questions, same harness, same `want`:

| | stored vectors | R@1 | R@5 | R@10 |
|---|---|---|---|---|
| pre-gate (`len >= 3`) | 46,565 | 33.3% | 48.3% | 51.7% |
| quality gate, floor 20, rebuilt at 16 KiB blocks (current) | 13,640 | 78.3% | 80.0% | 80.0% |
| quality gate, floor 24, before the padding fix | 9,296 | 70.0% | 80.0% | 81.7% |
| gate, floor 17 (ablation, rejected) | 10,424 | 60.0% | 78.3% | 78.3% |

The floor-17 row is kept for the record but is **void**: it was measured before
the padding fix, on vectors that were 99% padding, so it compared the wrong
thing. Re-measured on the fixed build the curve is flat from 20 down to 12 and
falls only below that, so 20 is the current floor and 17 is no longer the
interesting question. What the current rows show is that the gate and the
padding fix are separable: the gate alone took hit@10 from 51.7% to 81.7%, and
the padding fix then moved R@1 from 70.0% to 78.3% without touching R@5 or
R@10.

The gate removed 39,594 of the 48,890 D0Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬ĹźD5 candidates: 39,575 short
(<24 chars), 18 unencodable, 1 mojibake. The degenerate-text census on the
rebuilt index reads **0 blocks of at most 16 characters**, where the pre-gate
index held 36,136 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 78% of its embedded set. Mean D5Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹ÂD5 cosine fell
0.9816 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 0.8887, below the mean queryÄ‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺźanswer cosine of 0.935 that the
pre-gate index could not beat. Both harnesses agree on the new numbers
(compare_baselines and resonance_set both report 42/48/49 of 60). Per case
against the pre-gate run: **20 misses recovered, 2 regressions** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž #12 "The
user is vegetarian." (23 chars) and #41 "The user has a garden." (22 chars),
both cut by the 24-character floor. Lowering the floor to 17 to keep them was
measured and is worse across the board (60.0 / 78.3 / 78.3), so 24 stands as
the measured floor; those two questions remain the gate's measured cost.

**Class (b) is what the `want` constant controls.** Measured directly:
`want`=64 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â R@5 46.7%, `want`=128 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 46.7% (no change), `want`=256 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 48.3%,
`want`=512 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 48.3% (no further gain). All six class-(b) answers sit in the
128..256 band. The constant is set to 256, the measured floor.

Note on latency: repeated runs of the same build give p50 between 331 and
376 ms, so the cost attributed to the 64 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 256 change is within run-to-run
variance and is not quoted as a precise figure. The pre-gate recall numbers
(33.3 / 48.3 / 51.7%) are stable across every run, and so are the post-gate
figures (70.0 / 80.0 / 81.7%) in every run.

**What the post-gate latency is, phase by phase.** Instrumented over 10 identical
queries from a clean state, on an idle machine and again on a loaded disk, the
in-process 290 ms broke down as: provider construction, query embedding and the
vector search 138.8 ms; loading the learning state 61.2 ms; the post-recall
writes 28.2 ms; the tail (eureka, spaced repetition, narrative) 43.1 ms; the
emotional-field header scan 11.7 ms. Process start and config are another
25Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź40 ms of wall time outside all of it. The search phase is the embedding
model, not state I/O, and is out of reach of an I/O fix; an MCP server pays it
once instead of per request.

**The learning state was the part that could be fixed, and it was.** The
activation vector was 32 bytes per corpus block Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 22.4 MB for the 967,587-block
eval index, almost all of it default records Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž and it was read and rewritten
in full on every recall: ~20 ms of writing on an idle machine, ~60 ms on a
loaded one, plus the 55 ms load. It is now a sparse base (`HEB2`: only records
that differ from the default, plus the block count) plus an append-only,
CRC-checked journal (`activations_delta.bin`) of the records a recall actually
touched; the base is rewritten when the journal passes 4096 records or a full
save (rebuild, remap) asks for it. On the eval index the base is 372 bytes and
the journal ~24 KB after 60 recalls. Nothing is deferred to process exit Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž a
CLI invocation is a whole process, so a buffered write would lose the learning.

Measured, not estimated: clean-state p50 405.9 ms before, 355.0 and 361.0 ms
after the sparse activation file, 323.2 and 331.5 ms after the two changes
below, with recall unchanged at 42/48/49 on every run.

**The two findings that followed the first fix**, both from the same
instrumentation and both a case of doing work whose result nobody reads:

- The emotional field read all 967,587 block headers on every recall to find
  the handful of hot emotional blocks (11.7 ms). The hot set is computable from
  the in-memory energy alone, so `HebbianState::hot_indices` builds it before
  any header is touched and only those headers are read. The result is the
  same set in the same order, so the centroid, the total energy and
  `hottest_block` are bit-identical Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the integration test asserts that
  against the original full-scan formulation, which stays in the test as the
  reference. Phase: 11.7 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 1.5 ms. The same construction is applied to
  `apply_emotional_bias`, which runs on every recall wherever
  `emotional_bias_weight > 0`.
- `detect_eureka` loaded the 58.7 MB emotion lookup on every recall, and
  `recall` calls it with `emotion: None` Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the lookup is read only inside the
  `(Some(qe), Some(lookup))` arm, so it was built and never touched. It is now
  loaded when there is a query emotion to compare against and not otherwise.
  Phase: 17.5 ms Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â ~0.

**What is left, measured.** Provider construction, query embedding and the
vector search are 135 ms of the remaining ~237 ms in-process and are not state
I/O: an MCP server pays the model once instead of per request, which is the
fix for that, not a code change here. Materialising the activation vector
costs 12.2 ms per process Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž not the file (372 bytes) but zeroing 22.4 MB of
`ActivationRecord` in memory, which needs a sparse in-memory representation
rather than an I/O change.

The small state files are not worth dirty-tracking, and the reason is measured
rather than assumed. Per file they look expensive Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 7.2 ms for a 48-byte
`attention.bin`, 13.6 ms for a 12 KB `thought_graph.bin` Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž which does not
correlate with size at all. A probe in a fresh process reads the 48-byte file
in 21.7 ms, a second file in 9.5 ms, and everything after that in 0.06Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź0.26
ms: the cost is the first touch of a file in a process (filesystem and
antivirus filter warm-up), and it decays. Skipping a state file therefore
moves that penalty onto the next one instead of removing it, which is why the
seven files are left alone.

One measurement is reported as an outlier rather than smoothed away: a run
started immediately after a release build measured p50 1,123 ms with recall
unchanged, so the published figures are unloaded runs and the
build-then-measure sequence is exactly what the protocol below warns about.

**Measurement protocol: the state must be reset, not just the index.** Every
`recall` writes learning state back, so consecutive measurement runs on one
build do not measure the same system. Measured: after roughly 240 additional
recalls, R@5 fell 80.0% Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 76.7% and R@10 81.7% Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 78.3% (the reinforced blocks
of earlier questions re-rank later ones). Deleting the 20 mutable state files
Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž `load_or_init` recreates them Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž restored 42/48/49 exactly, twice.
`scripts/eval_real.sh` already guarantees this by removing `eval_output`
before it builds; a measurement taken any other way must reset the state
first.

**Class (c) was not addressed.** Only 4 questions are affected, and any
quota reserving top-N slots for vector hits would need to be measured against
all three k values and the latency percentiles before being adopted. It is
left as open work rather than a guess.

**The D5 result remains below the earlier D4 figure.** A previous run reported
R@5 75.0% on an index embedded to D4 (9,999 vectors) behind a pre-fix
candidate gate. On the committed D5 configuration the same harness returns
48.3% even after the pre-fetch fix. The D4 number is not reproducible from the
committed configuration and should not be cited.

Two further facts from the reproducible run:

- **The p50 cost is 331 ms** end to end, dominated by process start, BERT model
  load and query embedding rather than by search.
- **The depth collapse above is the single most promising next lever**, and it
  is not addressed here.

**Caveat on the baselines.** FAISS and FTS5 index only the 60 fact vectors and
report query-time search only; Microscope scans the full 967,587-block index
and its 288 ms includes process start, provider construction, query embedding
and index open. These rows are **not comparable**. The FAISS 96.7% is an upper
bound on what the same embeddings achieve with no filtering Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž a diagnostic
ceiling, not a competitive result.

**Scope.** The 60 facts and 60 questions are synthetic and test *reachability*:
whether a stored fact can be found at all. They do not estimate retrieval
quality for real user searches.

A worked example, because it is checkable by hand: querying `coffee` returns
"The user does not drink coffee." at rank 1 and "The user takes their coffee
black." at rank 2. Before the fix the same query returned nothing at all.

**The resonance set is synthetic** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 60 facts written for this evaluation, not
a sample of real usage. It measures whether the retrieval path can answer a
known question, not how well it serves a real corpus.

Raw results: [`docs/measurements/real_embedding_comparison.json`](docs/measurements/real_embedding_comparison.json).

### 11.2 Layer ablation Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž inconclusive; run under a disabled configuration

Three of the thirteen reinforcement layers were disabled one at a time, the
index was rebuilt, and hit@5 was re-measured. The patches were verified to
compile and to be present in the source before each measurement.

| Variant | hit@5 | Ä‚â€žĂ˘â‚¬ĹˇĂ„Ä…Ă‹ĹĄĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž |
|---------|-------|---|
| Baseline (all layers enabled) | 36/60 (60.0%) | Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž |
| `hebbian` disabled | 36/60 (60.0%) | 0.0 |
| `mirror` disabled | 36/60 (60.0%) | 0.0 |
| `attention` disabled | 36/60 (60.0%) | 0.0 |

**The result is a null delta, but it does not support a conclusion about the
layers.** The measurement was run under the configuration described in
Section 11.1 -- semantic ranking disabled, hash-derived vectors, a 60-fact
corpus. Under those conditions there is no co-activation structure to learn
from and no semantic signal for a reinforcement layer to reweight, so a zero
delta is the expected outcome whether or not the layers matter in normal
operation.

This ablation is therefore recorded as **inconclusive and not yet repeated.**
It establishes neither that the layers are load-bearing nor that they are not.
The patch mechanism itself is also coarse: it neutralises one entry point per
module rather than disabling the layer end to end, so it does not cleanly answer
the question even under a correct configuration.

The remaining ten layers were not ablated.

### 11.3 Performance

Two separate measurements, both reproduced by scripts committed to the
repository.

**Text index and reinforcement cost.** Reproducing the project's own latency
benchmark (`.scratch_bench.ps1`) on the real corpus -- 694,868 blocks, warm page
cache, one end-to-end invocation per row -- via
[`scripts/bench_text_index.sh`](scripts/bench_text_index.sh):

| Path | Latency |
|------|---------|
| `find`, with the inverted text index | 91-97 ms |
| `find`, index removed (full scan) | 89-90 ms |
| `recall` with hits | 218-219 ms |
| `recall`, zero hits (write block skipped) | 114 ms |

Two findings, stated at the confidence the data supports. First, **the inverted
text index shows no measurable speedup at this scale**: the `find` rows are
within noise with and without it, and the two `recall` rows disagree in
direction, so no ordering is claimed. Second, **the post-recall reinforcement
and its state write cost roughly 100 ms** on a warm cache -- the price the
layers impose on every non-empty recall. Both are single-sample figures that
include process start-up; the full table and caveats are in
[BENCHMARKS.md](BENCHMARKS.md).

**In-process spatial query.** The per-zoom latencies, the measurement method, and
the distinction between inner-loop and end-to-end cost are also in
[BENCHMARKS.md](BENCHMARKS.md).

## 12. Limitations

1. **Lexical retrieval is slow at scale.** Text search scans block contents; cost
   is linear in corpus size, and a large index makes this the bottleneck. The
   in-process spatial path stays fast regardless of index size.
2. **The spatial path is fast but not semantic.** Coordinates are assigned at
   write time; quality for a differently-worded query depends on how they were
   derived. Purely lexical queries are the case handled best.
3. **Semantic search is embedding-dependent.** It inherits the quality and cost
   of whatever model is used, including model download and inference latency.
4. **A fixed-size binary index** Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the system is slower than a plain B-tree and
   still less accurate than an exhaustive vector scan over the same vectors
   (Section 10.3: 81.7% vs 96.7% at k=10 on the 967,587-block index after the
   padding fix Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 51.7% before the quality gate Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž against their query-only
   0.007 ms; the latency figures are not comparable, see Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§10.3). The hierarchy
   costs accuracy and latency here, and buys inspectability, bounded memory and
   a per-recall write path. That is the trade this paper documents, not an
   argument that the hierarchy is faster.
5. **The corpus contains degenerate blocks Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž and the gate that removes them
   cost two real facts, which the gate's own floor then had to be lowered to
   fix.** Pre-gate, all 60 evaluation answers were embedded and scored a mean
   cosine of 0.935, but their mean rank across the 46,565 stored vectors was
   6,209: roughly six thousand blocks outscored an exact answer, and 36,136 of
   those stored vectors were at most 16 characters of `"<bin>"` sentinels,
   mojibake and code fragments. A build-time text-quality gate (length floor,
   sentinel and mojibake filters Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž explicitly no similarity-based dedup, which
   would merge contradictory facts) cut the index and moved hit@10 from 51.7% to
   81.7%. Two of the 60 answers are 22Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź23 characters and fell under the
   original 24-character floor, which is why the floor is 20 now. The earlier
   justification for 24 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž a floor-17 ablation that measured *worse* Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž was taken
   on an index whose vectors were 99% padding and is void; on the fixed build
   the curve is flat from 20 down to 12.
6. **The resonance set is synthetic, and the index it is measured against
   matters more than the questions.** 60 hand-written facts, not a sample of
   real usage. On the 967,587-block evaluation index the system answers 81.7%
   at k=10 (78.3% at k=1) with those 60 facts buried in 13,640 stored vectors;
   on the 60-fact benchmark index, where the evaluation set *is* the whole
   index, the same build reaches 98.3% at k=10. Both numbers are correct and
   they are not interchangeable: the second is evidence that the ranking works
   when there is nothing else to confuse it, and the first is evidence about
   scale. Neither is evidence about a real corpus, which is the gap this
   section is really about Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§12 records what would close it.
7. **The reinforcement layers are heuristic.** Drift, decay and weight learning
   use hand-chosen constants; there is no evidence here that they are
   near-optimal, and no hyperparameter sweep has been run.
8. **Cold-start cost dominates for small corpora.** End-to-end latency is
   dominated by process start, state loading and the query embedding, so
   in-process figures understate what a user experiences.
9. **The MCP path writes memories during a query, which is why it is slower
   than the CLI.** Measured on the same index with the same questions, one
   process serving twenty `memory_recall` calls: warm p50 1,771 ms before any
   of this work, and 4,448 ms in the session where the attribution was done
   (a bimodal per-call series Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž 4516 4336 712 4328 305 4259 254 259 251 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â¦ Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž
   against 260Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź331 ms for the CLI). Phase markers inside one MCP recall
   located the cost after everything else: the 22.4 MB state clone 4.9Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź34 ms,
   the consciousness-stream locks 0.1Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź0.4 ms, search and ranking 220 ms,
   spreading activation 32Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź68 ms, and 4,395Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź4,513 ms in the block that follows

   loop: an MCP recall writes up to a dozen "associative link" memories (the
   top-3 Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â€šÂ¬ÄąÄ„ top-5 keyword pairs), and since store-time embedding was added
   (c562166) each of those writes embedded its text through a freshly
   constructed provider Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž one MiniLM construction per stored link. Routing
   that path through the cached provider took the warm p50 from 4,448 ms to
   1,005 ms. Two other real defects were fixed on the way and are not the same
   cost: emotion blending rewrote the whole 58.7 MB `emotions.bin` once per
   activated block, and the structural-similarity scan sorted ~699k matches
   three times per recall.

   **Re-measured after the padding, parser, block-size and ranking changes:**
   one process, twenty `memory_recall` calls on the rebuilt evaluation index,
   `python scripts/bench_mcp_recall.py 20 --config eval_config.toml`:

   | | ms |
   |---|---|
   | first call (one-time cost) | 417.0 |
   | warm p50 | **253.9** |
   | warm p95 | 350.9 |
   | all-call p50 | 259.8 |
   | min / max | 207.1 / 507.5 |

   The bimodality is gone and the MCP path is no longer slower than the CLI
   (117-119 ms on the same index). The learning writes it performs during a
   query are therefore no longer the story they were; the state-clone and lock
   costs identified above are what remains, and they are small.

   That run wrote memories into the index, which is the point of the benchmark.
   Re-running the Ä‚â€šĂ‚Â§11.1 accuracy measurement afterwards with
   `MICROSCOPE_NO_LEARN=1` still returned **78.3 / 80.0 / 80.0%**, unchanged,
   which is the first direct confirmation that the flag makes an accuracy
   measurement immune to whatever ran against the index beforehand.

   What remains is 229Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź1,253 ms per call, still bimodal, and it is now
   attributed as well: the cost tracks the number of writes, not anything
   ambient. A call that wrote nine links took ~1,030 ms, one that wrote one
   took 342 ms, one that wrote none took 286 ms. Each write costs 72.8Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź80.1 ms
   in-process with the cached provider, of which 63.5Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź70.6 ms is a single
   MiniLM inference on the stored text; the lock is 0.3Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź0.4 ms, the layer
   file 1.8Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź10.3 ms and the auto-rebuild check 5.8Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă˘â‚¬Ĺź6.3 ms. So what is left is
   nine embedding inferences per query, which is arithmetic rather than waste:
   the same embedding of a short text costs ~65 ms on this machine.

   The fix is therefore not another cache. It is that a query should not be
   writing memories at all Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the associative linking belongs in a background
   consolidation step. That is a design question, so it is not done here. What
   *was* done, and measured: the nine link texts are now built first and
   embedded together through rayon on the cached provider, so the same nine
   vectors are computed at once instead of one after another. That took the
   warm MCP p50 from 1,005 ms to 739.7 ms. The writes stay sequential, because
   each one takes the file lock.
11. **The recall control is not a control.** The figures above and below are
   the ones this project has been steering by, so they deserve the finding that
   came out of checking them. Running the same binary against the same index
   three times gave R@1 of 37, 34 and 34 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž not one answer, and not the 42 the
   previous session recorded. The ranking is deterministic (six queries, run
   twice each, identical rank order every time). What moves is the *distance*:
   the top hit came back at L2=0.45051, then 0.25906, 0.20475, 0.17290,
   0.16108 Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž monotone, across consecutive runs of one query. A single recall
   rewrites 15 files in the output directory: activations, coactivations,
   emotion_log, narrative, narrative_memory, predictive_cache, pulses,
   resonance, thought_graph, thought_patterns. The pre-fetch confidence on one
   query climbed 62% Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 81% Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚â€šĂ‚Â Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â‚¬ĹľĂ‹Â 98% over three runs.

   So the system is learning, and it is learning the *questions*. Replaying a
   query pulls its own answer closer each time, which is exactly what a
   retrieval memory does, and it is why the number moved every time it was
   measured. It also means R@k measured this way reports how much the system
   has been asked, not what it knows. 42/48/49 was a real number from a real
   run; it just was not a fixed point, and neither is 37/34/34. Nothing here
   shows the change in this commit caused it: the CLI recall path does not call
   the store path that was touched, and the same drift is present across runs
   of a single unmodified binary.

   A trustworthy control needs the writes turned off for the duration of the
   measurement, or a fresh copy of the index per run, and the distance printed
   alongside the rank so a drifting system is visible rather than averaged
   away. `MICROSCOPE_NO_LEARN=1` now does the first of those. The CLI recall
   returns as soon as the ranking is printed -- the search runs in full, the same
   blocks are scored, boosted and ranked, and only the learning writes after it
   are skipped -- and an MCP recall skips its associative links. It is opt-in and
   off by default, because a server that forgets a query it just served is
   broken, and it is unrelated to `[hooks] read_only`, which governs the hook
   manager.

   It works, and the numbers it produces are the ones worth having. Six replays
   of the same query under the flag returned the identical distance to five
   decimal places and wrote nothing at all; three full benchmark runs gave
   R@1/R@5/R@10 of 56.7%/56.7%/63.3% every time, at a p50 of 169.7, 169.7 and
   171.0 ms. The reproducibility is the point. The recall is lower than the
   70.0% recorded earlier, and that is not a regression: the earlier number was
   measured on an index the benchmark had already warmed, so it partly counted
   what previous runs had taught. 34/34/38 is the answer to 'what does this
   index know', and it is now a number that can be checked.

   The p50 drop from 324 ms to 170 ms is the same effect seen as latency: about
   150 ms per recall was the learning pipeline's own writes, which a real,
   learning server still pays. The honest summary is that this figure is the
   read cost, and the number for a serving system is this plus the writes it
   does on purpose.

   And then the control turned out to be measuring nothing. The benchmark's
   own config, `bench_config.toml`, sets `semantic_weight = 0.0` and
   `provider = "mock"`: the index it builds holds no real vectors, and the
   diagnostic confirms the vector path returns nothing at all on it
   (`EVALDIAG vectors=0`). Every R@k above was a lexical-only number, and the
   gap to FAISS was not a ranking gap but the absence of ranking by vectors.
   With a real MiniLM index over the same 60 facts
   (`bench_config_semantic.toml`) the picture is: R@1/R@5/R@10 of
   **86.7 / 96.7 / 96.7%** at a p50 of 102 ms,
   against FAISS 90.0 / 96.7 / 96.7% and sqlite fts5 53.3 / 60.0 / 63.3%. The
   script's default now points at that index, because leaving it on the mock
   one is how this went unnoticed.

   The index has been rebuilt after `BLOCK_DATA_SIZE` moved from 1,024 to
   16,384 bytes, so these figures reproduce. R@5 and R@10 read 96.7% where older
   revisions said 98.3%; that is one case in 60, which a 60-query run cannot
   resolve. More to the point these figures should not be compared with
   anything at all Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž the 60 facts *are* the query set, so the index contains
   only the answers. Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.0 is the retrieval number that means something.

   Of the five remaining misses, one is absent from the ranked list and four are
   reachable but outranked. An earlier reading of this blamed the cosine floor
   that `embedding_index::search` applies, on the strength of two answers
   scoring 0.089 and 0.073 against it. That was an inference from a diagnostic
   line and it was wrong: the floor is now settable via
   `MICROSCOPE_SIM_FLOOR`, and sweeping it from 0.3 down to 0.0 changes
   R@1/R@5/R@10 not at all (73.3 / 86.7 / 90.0% at every value). With the floor
   removed the diagnostic returns all 71 vectors and the vegetarian block is
   there, at index 69 of 71. It was never filtered; it finishes last.

   What ranks it last is not the score shape either. Sweeping
   `semantic_weight` from 1.0 to 5.0 changes nothing: 73.3 / 86.7 / 90.0% at
   every value, flat like the floor. The printed distances explain why no
   weight could matter -- the top results for "diet" come back at L2 = -0.064,
   -0.052, -0.047, already negative, so the semantic term subtracts more than
   the lexical base adds and the scale is saturated. The clamp at 1.0 is not
   what holds the recall back.

   Crowding was the next guess and it was wrong too: the depth-4 blocks are 69
   distinct entries, not near-duplicates. Reading the index directly found the
   real cause. "The user is vegetarian." exists as block 27 and was never
   embedded -- the quality gate's minimum is 24 characters and the fact is 23.
   That gate is the one this project added first, and it was refusing a
   legitimate short memory. Lowering it is measurable, on this corpus:

     minimum   blocks embedded   R@1     R@5     R@10
     24 (default)      71        73.3%   86.7%   90.0%
     16                82        76.7%   90.0%   95.0%
     8                 83        76.7%   88.3%   95.0%

   16 is better on all three and 8 starts admitting noise (R@5 falls back), so
   the floor is a real cost and a real lever. The default is left at 24: that
   number was chosen against a 944,808-block corpus where 50,509 blocks sit
   under it, and a 60-fact synthetic set of short sentences is the wrong corpus
   to decide a global admission policy from. The measurement is the argument
   for whoever deploys a short-note memory to set
   `MICROSCOPE_MIN_EMBED_CHARS=16`.

   Both of those turned out to be data loss in the build rather than anything
   about thresholds, and both are now fixed (71e938d). A layer file with one
   entry per line has no blank lines, so the parser's blank-line split missed it
   and the size fallback cut the corpus into fixed 1024-byte pieces at arbitrary
   boundaries -- the 60-fact corpus became one block ending "...ends with
   .hu.\r
The user does not hav", severed mid-word. And the sentence splitter broke on
   every period, so the ".hu" in that very fact was split into its own block and
   the fragment a search would rank no longer held the token. Chunks are now cut
   on line boundaries and a terminator only counts before whitespace, a capital
   or the end.

   With the text intact, on the same corpus and config, reproduced twice:

                     R@1      R@5      R@10     p50
     before          73.3%    86.7%    90.0%   151.9 ms
     + line-boundary  75.0%    90.0%    93.3%   149.8 ms
     + sentence split 76.7%    91.7%    95.0%   151.2 ms
     FAISS            90.0%    96.7%    96.7%     0.01 ms

   With the text intact, the quality gate's own floor was re-measured, because
   its justification had been measured on the build that was eating the corpus.
   The old comment claimed 24 was a floor, on the grounds that 17 re-introduced
   crowding and cost hit@10 (81.7% -> 78.3%). That number is void. On the fixed
   build the curve is flat:

     minimum   R@1      R@5      R@10
     24        76.7%    91.7%    95.0%
     20        80.0%    95.0%    98.3%
     16        80.0%    95.0%    98.3%
     12        80.0%    95.0%    98.3%

   20 is the smallest value that captures the whole gain, so it is now the
   default. Reproduced twice: **80.0 / 95.0 / 98.3%** at a p50 of 150.7 ms,
   against FAISS 90.0 / 96.7 / 96.7% and sqlite fts5 53.3 / 60.0 / 63.3%. R@10 now
   exceeds the vector baseline on this corpus; the remaining gap is 10 points at
   R@1 and 1.7 at R@5.

   The count the old default was tuned against is now measured, on the fixed
   parser and the new floor: rebuilding the 874,913-block evaluation index
   reports `-44,336 short (<20 chars)`, against the 50,509-under-24 that was
   recorded before on a build that was truncating the corpus. Both figures were
   suspect; the new one is not. On a corpus of genuinely degenerate fragments
   rather than sentences, raise the floor back.

   Every stored index was invalid once the padding bug was found, and has been
   rebuilt. Verification against the reference on 60 sampled blocks of the
   rebuilt 13,640-vector index, bucketed by stored text length: exact (mean and
   minimum 1.0000) up to 300 characters, 0.938 at 300-700, 0.741 at 700-1100.
   Six samples in the long buckets is not enough to diagnose, so the residual
   is recorded rather than explained -- long-tail blocks should be treated as
   unverified rather than assumed correct.

   There is a deeper one underneath all of that, and it is the reason the
   remaining gap should not be read as a ranking result. The candle provider is
   internally consistent -- the same text always yields the same vector, and
   querying with a fact verbatim returns that fact at Sim=1.000 -- and it is
   still not the model it names. Against the reference all-MiniLM-L6-v2
   embedding of the same text, the stored vectors score a mean cosine of
   0.32-0.37, not 1.0. So two things are true at once: the provider is
   deterministic, and the binary's embeddings are not the ones the FAISS row
   is built from.

   That makes the comparison uneven on the semantic axis, and it shows up in the
   one case the diagnostic still misses: for "chronotype" the binary reports
   cosine 0.860 against "The user is a morning person." and ranks it 49th,
   while the reference gives 0.045 for the same pair and ranks it 20th. Same
   query, different neighbourhood.

   Masked and unmasked mean pooling were measured and are identical here
   (0.369 both) because these texts never pad; add_special_tokens on or off
   makes no difference (0.316 vs 0.311); and the mock fallback is ruled out by
   construction, since the candle path exits rather than degrading. What is
   left is the forward pass itself. Reading the library rather than guessing
   rules the obvious suspect out: candle-transformers 0.3.3 never builds an
   attention mask at all -- `BertModel::forward` calls the embeddings and the
   encoder and nothing else, `token_type_ids` feeding only the segment
   embedding, and `BertEncoder::forward` is a bare loop over layers. It also
   always loads the position embeddings, which are applied as 0..seq_len. On
   our side the weights come from model.safetensors as F32, the config is
   parsed, and the mock fallback cannot happen.

   The answer came from inside, with a diagnostic that dumps the token ids
   (`MICROSCOPE_EMBED_DEBUG=<path>`). They are padded to 512:

       [101, 10381, 4948, 26305, 102, 0, 0, 0, ... 0]
       [CLS]  chronotype  [SEP]   + 507 [PAD]

   And since `BertModel::forward` takes no mask, the encoder attended over all
   512 positions and the mean pooled all 512. Every embedding in the system was
   99% padding. That is why the provider passed every consistency check --
   the same text always gave the same padding-dominated vector, so querying
   with a fact verbatim returned it at exactly 1.000 -- while scoring 0.32-0.37
   against the reference. The bug was invisible to any test that only asked
   whether the provider was deterministic.

   Trimming to the real length, taken from the attention mask, before the
   forward fixes both halves at once: the encoder no longer attends to padding
   and the mean is over real tokens only. The stored vectors now match the
   reference at a mean cosine of 1.0000, minimum 1.0000, over 79 blocks, and
   none below 0.9.

       before the fix    R@1 80.0%  R@5 95.0%  R@10 98.3%   p50 150.7 ms
       after             R@1 85.0%  R@5 96.7%  R@10 96.7%   p50 105.7 ms
       FAISS             R@1 90.0%  R@5 96.7%  R@10 96.7%   p50   0.01 ms

   R@5 now equals the vector baseline exactly and the gap at R@1 is five
   points. The p50 fell 30% as a side effect: a five-token sentence no longer
   runs a 512-position forward. Every embedding this system has ever stored
   was affected, not just the benchmark's.
   The ranking itself, once the ranking was finally looked at, held one more
   lesson. The score folds cosine in as `combined -= sim * w` with
   `w = semantic_weight.clamp(0.0, 1.0)`. Sweeping 1.0 through 10.0 returned
   identical recall at every value, and two sessions back that flat line was
   written down as "the semantic weight is not the constraint". It was not flat
   because the term did not matter -- it was flat because the parameter could
   not be set above 1. A sweep that cannot reach the effect is not evidence of
   its absence.

   With the clamp raised so the sweep can move, and the cosine floor swept
   against the corrected embeddings, the two effects are independent and
   additive, each at the knee of its curve:

     w     floor   R@1     R@5     R@10
     1.0   0.30    85.0%   96.7%   96.7%
     2.0   0.30    86.7%   96.7%   96.7%
     1.0   0.20    85.0%   98.3%   98.3%
     2.0   0.20    86.7%   98.3%   98.3%
     3.0   0.20    86.7%   98.3%   98.3%

   The floor default is 0.2 and the benchmark asks for a weight of 2.0, giving
   86.7 / 98.3 / 98.3% at a p50 of 101.5 ms against FAISS 90.0 / 96.7 / 96.7%:
   R@5 and R@10 above the vector baseline, R@1 three points behind it.

   That caveat was closed by doing the split rather than by arguing about it.
   `search.semantic_rank_gain` is a separate field; `semantic_weight` went
   back to being only the 0..1 coordinate blend. Measured one at a time:

   ranking gain, coordinates held at blend 1.0      coordinate blend, gain at 2.0

     gain    R@1                                blend    R@1
     0.0    61.7%                                0.0    86.7%
     1.0    85.0%                                0.5    86.7%
     2.0    86.7%                                1.0    86.7%
     4.0    86.7%

   The effect is entirely the ranking gain, and it is worth 25 points of R@1: at
   gain 0 the semantic term contributes nothing and the system is lexical, at
   61.7%. The coordinate blend does nothing at any value on this corpus. "Flat
   here" is not "inert" -- the blend decides how far query coordinates move
   toward the embedding, and a corpus where semantics already dominates the
   ranking would not show it -- so it stays wired and measured rather than
   deleted on the strength of one corpus.
   Five things in this measurement chain turned out not to be ranking problems -- a
   mock config, a learning side effect, a cosine floor that was not the cause, a
   weight that was not the cause, and a parser eating the corpus. None of the four
   earlier conclusions would have been worth much without this one underneath them.

12. **Fresh entries are embedded at store time, and that has a cost.** The
    append log used to be invisible to the semantic path until the next full
    rebuild. It no longer is: `store` embeds the text once, under the same
    quality gate the index build uses, and records the vector in
    `append_embeddings.bin` keyed by its position in the log. Measured on a
    scratch index: a paraphrase query sharing no token with a freshly stored
    memory returns that memory at rank 1, and with the sidecar deleted the same
    query does not return it at all. The costs are real and unmeasured at
    scale Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž a CLI `store` now pays a model load (201 ms wall against ~30 ms
    before, while an MCP server loads the model once), the sidecar is discarded
    at the next rebuild, where the entries are re-embedded as main blocks, and
    how this behaves with thousands of pending entries is untested.
13. **The layer ablation is inconclusive.** The null deltas in Section 11.2 were
    measured with the semantic path disconnected, and the patch mechanism
    neutralises one entry point per module rather than disabling the layer end to
    end. It establishes neither that the thirteen layers are load-bearing nor
    that they are not.

---

## 13. Conclusion

Microscope Memory implements a hierarchical memory index in which every block
occupies a fixed 256-byte viewport across nine depth levels, D0--D8, and in which
retrieval outcomes are fed back into scoring state. Thirteen such feedback
mechanisms are implemented: Hebbian drift, activation-fingerprint matching,
spatial pulse propagation, pattern archetypes, query-space warping, recall-path
tracking, predictive prefetch, time-windowed profiles, learned attention weights,
cross-instance exchange, offline consolidation, shared state propagation, and
multi-modal storage.

**The evaluation is reproducible, and its result is mixed but improved.**
With the semantic path connected, the pre-fetch widened, a text-quality gate
applied at build time, the embedding padding fixed, and whole documents stored
per block, the system reaches
**78.3% hit@1 / 80.0% hit@5 / 80.0% hit@10** on the 967,587-block evaluation
index (30.0% for lexical-only, 63.3% for SQLite FTS5, 96.7% for FAISS
`IndexFlatIP` over the same vectors) at a p50 of 119 ms. It beats a
general-purpose lexical index on this set and remains below an exhaustive vector
scan, which is the trade Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§10.3 describes rather than a claim of superiority.

Two numbers in this paper are not interchangeable, and conflating them is what
made an earlier revision of this conclusion unreproducible. On the 60-fact
benchmark index, where the evaluation set is the entire index, the same build
reaches **86.7 / 96.7 / 96.7%**. The first row is evidence about scale; the
second is evidence that the ranking works when there is nothing else to
confuse it. Neither is evidence about a real corpus, and Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.0 is where that
was finally measured: on BEIR SciFact, 5,183 abstracts and 286 queries, the
system reaches 53.1 / 74.8 / 81.8% against FAISS's 48.3 / 73.4 / 78.3%.

The diagnosis behind the earlier 51.7% is recorded in Ä‚â€žĂ˘â‚¬ĹˇÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ˘â‚¬ĹˇÄ‚â€šĂ‚Â§11.1: 78% of the stored
vectors were at most 16 characters of degenerate text that out-scored every
real answer. Removing them by a text-quality filter Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž length, sentinels,
mojibake; explicitly not cosine dedup, which would merge contradictory facts Ä‚â€žĂ˘â‚¬ĹˇÄ‚â€ąĂ‚ÂĂ„â€šĂ‹ÂÄ‚ËĂ˘â€šÂ¬ÄąË‡Ä‚â€šĂ‚Â¬Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ă„Ä…Ă„â€ž
moved hit@10 by 30 points and hit@1 by 36.7 points, larger than any
architectural difference measured elsewhere in this paper. A second defect
found later was larger still and quieter: every embedding the system had ever
stored was 99% padding, because the tokenizer pads to 512 and the forward pass
was given no attention mask. Fixing it moved hit@1 by 8.3 points and cut p50 by
2.7Ă„â€šĂ˘â‚¬ĹľÄ‚ËĂ˘â€šÂ¬ÄąË‡Ă„â€šĂ‹ÂÄ‚ËĂ˘â‚¬ĹˇĂ‚Â¬Ä‚ËĂ˘â€šÂ¬ÄąÄ„, and it is the kind of bug that no consistency check can catch, because it
was perfectly deterministic.

What the work establishes is therefore narrower than the design's ambitions
and worth stating plainly: a fixed-size binary index whose retrieval path can be
correctly connected to its own embedding index; a measurement harness that
produced the numbers above on demand, and that caught six of the claims made
along the way being wrong; and a corrected account of where the system stands,
including the cases where a plain B-tree or a flat vector scan is the better
tool. Pure Rust, zero JSON, 464 tests, 54,053 lines.

Released under the MIT License at
[github.com/silentnoisehun/microscope-memory](https://github.com/silentnoisehun/microscope-memory),
archived at DOI
[10.5281/zenodo.22983478](https://doi.org/10.5281/zenodo.22983478).

---

*Microscope Memory is part of the Ora project ecosystem.*
