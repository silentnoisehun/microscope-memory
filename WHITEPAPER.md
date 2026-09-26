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

**Scope of the claims.** This paper describes an engineering artefact and its measured behaviour. It makes no claim about consciousness, sentience, or cognition in the strong sense; "memory" throughout means an index with learned relevance signals. Every performance number is tied to a stated measurement path; see Section 9 and the Limitations section for what is *not* claimed.

---

## 1. Introduction

The dominant paradigm in AI memory systems relies on embedding vectors and approximate nearest-neighbor search. While effective for semantic similarity, these approaches treat memory as static storage — data goes in, query comes out, nothing changes between accesses.

Biological memory works differently. Every act of recall modifies the memory itself: neural pathways strengthen through use (Hebbian learning), similar patterns resonate across brain regions (mirror neurons), and recurring activation patterns crystallize into abstract concepts (archetypes). Memory is not a database — it is a living structure that self-organizes through use.

Microscope Memory implements this principle in a pure binary system. The zoom metaphor provides efficient hierarchical access (37ns at D0 to 500us at D8), while thirteen reinforcement layers turn each recall into a signal that reshapes later retrievals.

---

## 2. Core Architecture

### 2.1 Binary Format

Three primary binary files with no serialization overhead:

- **`microscope.bin`** — Block headers (32 bytes each, mmap'd). The first 16 bytes (x, y, z, zoom) load directly into SSE registers for SIMD distance computation.
- **`data.bin`** — Raw UTF-8 text content, referenced by offset and length from headers.
- **`meta.bin`** — Index metadata (MSC3 format): magic, version, block count, depth ranges, Merkle root, layers hash.

Supporting files: `merkle.bin` (SHA-256 tree), `embeddings.bin` (mmap'd vectors), `append.bin` (hot memory log).

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

Below D8, decomposition destroys meaningful information — the "atomic boundary of information."

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

Builds are incremental — SHA-256 content hash of layer sources is stored in MSC3 meta.

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

### 3.2 Layer 2: Mirror Neurons (`mirror.rs`)

Activation fingerprints from L1 are compared via sparse cosine similarity. When two fingerprints (from different queries) exceed a threshold, a resonance echo is created, boosting the block's future retrieval score.

Each block accumulates a `block_resonance` value — the sum of echo strengths it has received. Echoes decay over time, so only actively resonating blocks maintain their boost.

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

Hot spots in the resonance field crystallize into archetypes — persistent named patterns that represent recurring themes in the memory landscape.

Detection algorithm:
1. Find cells in the resonance field above a strength threshold
2. Cluster nearby Hebbian-active blocks around each hot spot
3. If a cluster has sufficient members and strength, it becomes an archetype
4. Auto-label from the most common words in member block content

Archetypes reinforce when activation patterns overlap their members, creating a positive feedback loop. Archetypes decay when not reinforced.

Binary format: `archetypes.bin` (ARC1).

### 3.5 Layer 5: Query-Space Warping (`emotional.rs`)

The emotional layer (layer_id=4 in the cognitive layer schema) receives special treatment. Active emotional blocks create an "emotional centroid" — the energy-weighted average of their 3D coordinates.

Before search, query coordinates are warped toward this centroid:

```
warped = query + (centroid - query) * weight
```

The weight is configurable (0.0 = disabled, 1.0 = fully warped to emotional centroid). This means the system's current emotional state subtly bends all searches — memories associated with active emotions become easier to reach.

### 3.6 Layer 6: Recall Path Graph (`thought_graph.rs`)

While L1--L5 operate at the block level, L6 operates at the **path level** — tracking sequences of recalls over time.

Every recall creates a **ThoughtNode** (timestamp, query hash, session ID, dominant layer). Consecutive recalls within the same session form **directed edges**. A 30-minute gap starts a new session.

**Pattern detection** uses sliding-window n-grams (lengths 2--5) over the current session's query hashes. When:
- All constituent edges have been traversed ≥2 times
- The sequence has been observed ≥3 times (PATTERN_MIN_FREQ)

...the sequence crystallizes into a **ThoughtPattern** that boosts future searches matching the same thought path.

This is how the system learns to "think in patterns" — recognizing that after querying about "Ora" then "memory", the user typically asks about "Rust" next, and pre-positioning results accordingly.

Binary formats: `thought_graph.bin` (THG1), `thought_patterns.bin` (PTN1).

### 3.7 Layer 7: Predictive Prefetch (`predictive_cache.rs`)

L7 closes the feedback loop. Based on L6's crystallized patterns, the cache predicts which blocks the user will need **before the query executes**.

After each recall:
1. **Predict**: Check if the current session path is a prefix of any known pattern. If so, pre-load the pattern's result blocks into the cache with a confidence score.
2. **Check**: On the next recall, if the query hash matches a cached prediction, instantly boost the pre-fetched blocks.
3. **Evaluate**: After search completes, compare prediction against actual results:
   - **Hit** (≥50% overlap or ≥3 blocks): reward source pattern (+0.3 strength)
   - **Partial hit**: proportional reward
   - **Miss** (0 overlap): penalize source pattern (-0.05 strength), halve cache confidence

This creates a reinforcement loop:
```
Good pattern → Accurate prediction → Hit → Pattern strengthened → Better prediction
Bad pattern → Wrong prediction → Miss → Pattern weakened → Eviction
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
- A **dominant window** is identified (the window with the highest weight, requiring ≥5 total activations)
- If the current query falls within the dominant window: boost = 1.0
- Otherwise: boost scales down proportionally to the off-peak window's weight

This allows the system to learn circadian patterns — for example, that "work" archetypes activate during 08--12 and "creative" archetypes during 20--24.

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

**Attention computation**: Each signal maps to a 7-dimensional weight vector via fixed rules (e.g., long queries boost spatial search, high emotion boosts emotional bias). These raw weights are blended 80/20 with **learned weights** — persistent per-layer multipliers that adapt over time.

**Quality inference**: The system infers whether the previous recall was "good" or "bad" from the time gap to the current recall:
- **>60 seconds**: satisfied (quality = 1.0) — the user found what they needed
- **<5 seconds**: unsatisfied (quality = 0.2) — immediate re-query suggests failure
- **5--60 seconds**: linear interpolation

**Weight learning**: Good outcomes' attention vectors are averaged per layer. Bad outcomes' vectors are averaged. The learned weight for each layer shifts toward what worked and away from what didn't, via exponential moving average (rate = 0.05). Weights are clamped to [0.1, 3.0].

Binary format: `attention.bin` (ATT1), header 48 bytes + 40 bytes per outcome (200 cap).

### 3.10 Layer 10: Cross-Instance Learning (`federation.rs`)

While L3 already exchanges resonance pulses across federated indices, L10 extends this to higher-order knowledge: **ThoughtGraph patterns** and **PredictiveCache statistics**.

**Pattern exchange**:
1. Export local ThoughtGraph's crystallized patterns
2. For each federated index, import their patterns with trust weighting
3. Trust = source's PredictiveCache hit rate × federation weight
4. Low-trust patterns are imported at reduced strength, preventing unreliable peers from polluting local knowledge

**Stats aggregation**:
- Total predictions, hits, and misses are merged bidirectionally
- This allows each instance to benefit from the collective prediction accuracy of the federation

The exchange is triggered explicitly via the `pattern-exchange` CLI command, giving operators control over when cross-pollination occurs.

### 3.11 Layer 11: Offline Consolidation (`dream.rs`)

Offline consolidation replays and prunes — replaying the day's experiences, strengthening important connections, and pruning noise. The dream command implements this in Microscope Memory.

The `dream` command runs an offline consolidation cycle:

1. **Replay**: Scan Hebbian fingerprints from the last 24 hours. For each, partially re-energize the activated blocks (0.3 energy vs 1.0 for real activation).
2. **Strengthen**: Track which co-activation pairs appear across multiple replayed fingerprints. Pairs appearing in ≥3 fingerprints get their count multiplied by 1.5.
3. **Prune pairs**: Remove co-activation pairs with count ≤1 that are older than 48 hours. These are noise — connections that never reinforced.
4. **Prune activations**: Zero out activation records with near-zero energy and zero activation count. These are dead blocks consuming state.
5. **Pattern consolidation**: Run ThoughtGraph pattern detection across all recent sessions, potentially crystallizing new thought patterns.
6. **Field decay**: Apply 0.8× decay to the resonance field and expire old pulses, preventing stale spatial information from persisting.
7. **Cache cleanup**: Remove predictive cache entries with confidence below 0.1.

Each cycle is logged with statistics: replayed fingerprints, strengthened pairs, pruned entries, energy before/after.

Binary format: `dream_log.bin` (DRM1), 40 bytes per cycle record.

### 3.12 Layer 12: Cross-Instance State Sharing (`emotional_contagion.rs`)

While L5 warps the local search space based on local emotional blocks, L12 extends this across federated instances — creating shared emotional context.

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

**Spatial integration**: each modality computes deterministic 3D coordinates from its features — images from phash bytes (in the associative region), audio from spectral features (in the echo_cache region), structured from field name hashing (in the rust_state region). This ensures multi-modal blocks participate naturally in spatial search.

Binary format: `modalities.bin` (MOD1), variable-length entries.

---

## 4. The Complete Recall Pipeline

Every `recall` command triggers the full reinforcement stack:

```
 1. Load reinforcement state (Hebbian, mirror, resonance, archetypes, thoughts, cache, temporal, attention)
 2. Compute attention weights from query signals (L9)
 3. Infer quality of previous recall from inter-recall timing (L9)
 4. Compute query coordinates (content hash + semantic blend)
 5. Check predictive cache — instant boost if prediction exists, scaled by attention weight (L7)
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
16. Evaluate prediction accuracy — hit/miss/partial (L7)
17. Predict next: pre-fetch blocks for likely next query (L7)
18. Mark recall in attention history (L9)
19. Save all state
```

Steps 2--8 happen **before** display (affecting result ranking). Steps 10--18 happen **after** display (learning from the recall).

---

## 5. Supporting Systems

### 5.1 Structural Fingerprinting

Each block receives a structural fingerprint: Shannon entropy, 16-bucket byte histogram, and FNV-1a hash. Blocks with similar fingerprints are connected by "wormhole links" — structural shortcuts across layers and depths.

Binary formats: `fingerprints.idx` (FGP1), `links.bin` (LNK1).

### 5.2 Radial Search

Depth-constrained radius search with SIMD acceleration. Returns a `ResultSet` containing primary matches and distance-weighted neighbors. Used for Hebbian co-activation recording.

### 5.3 Multi-Index Federation

Multiple Microscope indices can be queried in parallel with weighted result merging. Federation also supports activation pulse exchange — activation state can propagate across instances.

### 5.4 MQL (Microscope Query Language)

Structured queries with layer, depth, spatial, keyword, boolean, and limit filters:
```
layer:long_term depth:2..5 near:0.2,0.3,0.1,0.05 "Ora" AND "memory" limit:20
```

### 5.5 Visualization

Three levels of visualization output:

1. **Cognitive Map** (`cognitive-map` command): Full 13-layer export as JSON — blocks with Hebbian drift, co-activation edges, resonance wave field, archetypes with temporal profiles, thought paths, crystallized patterns, predictive cache stats, attention weights, dream cycle history, emotional contagion state, multi-modal stats, and mirror echoes. Ships with an interactive **Three.js viewer** (`viewer.html`) that auto-opens in the browser, featuring:
   - Per-feature toggles (blocks, edges, wave field, thought paths, archetypes, dreams, echoes, emotional centroid)
   - Per-layer visibility toggles with color-coded swatches
   - Collapsible sidebar panels (stats, attention weights, emotional field, predictions)
   - Animated wave field pulsing, dream cycle energy visualization, archetype temporal rings

2. **Basic Snapshot** (`viz` command): JSON export of blocks, edges, field, archetypes, echoes, and aggregate stats.

3. **Density Map** (`density` command): Binary DEN1 format — quantized 3D grid of Hebbian energy for fast volumetric rendering.

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

The reinforcement layers add minimal overhead per recall: state files are loaded once, learning operations are O(k²) where k is the result count (typically 5--10), and binary I/O is sequential with no allocation during the hot path.

The predictive cache, when warmed, provides effectively **zero-cost** result boosting — pre-fetched blocks are a simple HashMap lookup before the spatial search begins.

---

## 7. Binary Formats Summary

| File | Magic | Purpose |
|------|-------|---------|
| `microscope.bin` | — | Block headers (32B each, mmap'd) |
| `data.bin` | — | Raw UTF-8 text content |
| `meta.bin` | MSC3 | Index metadata, Merkle root, layers hash |
| `merkle.bin` | — | SHA-256 Merkle tree |
| `embeddings.bin` | — | Pre-computed embedding vectors |
| `append.bin` | APv2 | Hot memory append log |
| `activations.bin` | HEB1 | Hebbian activation records |
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

413 library tests plus 16 hook tests:

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

**Narrative Memory.** Automatically linking sequences of recalls into coherent narratives — story arcs that emerge from thought patterns and can be replayed as structured episodes.

**Self-Modeling.** A meta-layer that observes the consciousness stack itself — which layers contribute most, how attention weights evolve, which dream cycles produce the most pruning — enabling the system to optimize its own parameters.

**Embodied Perception.** Extending multi-modal memory with real-time sensor fusion — camera feeds, microphone input, accelerometer data — for embodied AI applications.

---

## 9. Related Work

Microscope Memory sits at the intersection of hierarchical index structures,
learning-to-rank feedback loops, and reinforcement-based retrieval. This section
places the system relative to the established literature. The comparison is
deliberately conservative: where a claim is not backed by a measurement in this
paper, that is stated.

### 9.1 Hierarchical and multi-resolution indexing

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

### 9.2 Learned ranking and feedback loops

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

### 9.3 Vector databases and ANN search

- **FAISS** (Johnson, Douze & Jégou, 2017) -- efficient similarity search.
- **HNSW** (Malkov & Yashunin, 2016) -- hierarchical navigable small-world
  graphs, the basis of several production vector stores.
- **Chroma, Qdrant, Weaviate, Pinecone** -- systems built on these indexes.

**A controlled comparison has been run**, and its result is not favourable to
this system. On a 60-fact corpus with 60 questions, all systems on the same
machine ([scripts/compare_baselines.py](scripts/compare_baselines.py)):

| System | p50 ms | R@1 | R@5 | R@10 |
|--------|--------|-----|-----|------|
| **Microscope** (`recall`, end-to-end) | 73.34 | 48.3% | 60.0% | 60.0% |
| FAISS `IndexFlatIP` (d=3) | 0.0061 | 1.7% | 5.0% | 11.7% |
| FAISS `IndexFlatIP` (d=256, BoW) | 0.0045 | 21.7% | 31.7% | 33.3% |
| FAISS `IndexHNSWFlat` (d=256, BoW) | 0.0093 | 18.3% | 28.3% | 38.3% |
| **SQLite FTS5** (BM25) | 0.0311 | **53.3%** | 60.0% | **63.3%** |

**SQLite FTS5 matches or beats Microscope on every recall metric measured here
and is about 2,400x faster at p50.** The measurement asymmetry favours the
baselines (their timings exclude index build; Microscope's include process
start-up), so the gap is conservative. Full discussion, including why 60 facts
is not a scale test, is in [BENCHMARKS.md](BENCHMARKS.md).

One result cuts the other way and is worth stating: FAISS fed Microscope's *own*
3-D coordinates scores 1.7% at R@1, against Microscope's 48.3% on the same
vectors. The hierarchical depth structure and the reinforcement layers are
therefore doing the retrieval work; the raw coordinate distance is not
sufficient. That is evidence against the claim that the system is merely
spatial k-NN.

The architectural difference remains real: Microscope performs *exact* lookup
over an explicit spatial hierarchy with no embedding model, at the cost of
providing no semantic similarity beyond hash-derived coordinates. Whether that
trade suits a given workload is exactly what this table shows has **not** been
demonstrated in Microscope's favour.

### 9.4 Memory architectures for language agents

- **Generative Agents** (Park et al., 2023) -- agent memory as a stream of
  observations, retrieved with recency, importance and relevance scoring.
- **Reflexion** (Shinn et al., 2023) -- storing verbal feedback for later
  retrieval.
- **MemGPT / tiered context management** -- explicit paging between contexts.

Microscope overlaps in intent and differs in on-disk representation. The
three-part scoring used here (recency, importance, relevance) is close to that
used in generative-agent retrieval, and is presented as such.

### 9.5 Consolidation and forgetting

Offline replay-and-prune resembles sleep-dependent memory consolidation in
neuropsychology (Lewis & Durrant, 2011); the engineering pattern of periodic
compaction also appears in log-structured storage (O'Neil et al., 1996). The
naming in the source retains biological vocabulary for continuity; the mechanism
is a background replay pass that strengthens recently accessed blocks and
prunes unreferenced ones.

## 10. Evaluation

### 10.1 Retrieval quality: resonance test set

Retrieval quality is measured on a fixed set of 60 personal facts with known
questions across 13 categories. The set and the index builder are committed at
[`scripts/resonance_set.py`](scripts/resonance_set.py) and
[`scripts/build_bench_index.py`](scripts/build_bench_index.py):

```bash
cargo build --release
python scripts/build_bench_index.py                       # 60 facts -> index
python scripts/resonance_set.py --check                   # validate the set
python scripts/resonance_set.py --measure --mode recall --k 5 10 20
```

**Run:** commit `f64c2fa`, Windows 11, 60-fact index (4,853 blocks across
9 depths), `embedding.provider = "mock"`.

**Metric:** hit@k -- the fraction of the 60 facts appearing in the top k results.

| k | `recall` (spatial + heuristic) | `find` (literal substring) |
|---|-------------------------------|------------------------------|
| 5  | 36/60 (60.0%) | 20/60 (33.3%) |
| 10 | 36/60 (60.0%) | 20/60 (33.3%) |
| 20 | 36/60 (60.0%) | 20/60 (33.3%) |

**Reading these numbers honestly.** Three things limit what they show:

1. **The curve is flat across k, and that is not a strong result.** `recall`
   returns at most 20 rows and `find` often returns fewer, so a fact that is
   absent at k=20 is absent everywhere and a fact present at k=5 is counted at
   every k. The k axis currently separates almost nothing. A discriminating
   evaluation needs a larger index and questions with many plausible
   distractors, so that rank position actually varies.
2. **The two modes measure different things.** `find` is exact substring
   matching and can only answer questions whose wording appears in the stored
   fact. `recall` uses hash-derived coordinates, so it retrieves by spatial
   proximity and can answer paraphrased questions -- which is why it scores
   higher. Neither number is a semantic-retrieval score.
3. **The embedding provider is `mock`.** Coordinates are hash-derived, not
   learned from text, so "paraphrase" here means "hashed to a nearby point",
   which is a weaker property than semantic similarity. A real embedding
   provider may change these numbers substantially in either direction.

**The two known regressions.** Case 1 (pine nut allergy) is retrieved by both
modes. Case 2 (preference for short check-ins) is *not* retrieved at any k by
either mode: the question "how does the user like check-ins" shares almost no
lexical content with the stored fact, and with `mock` coordinates there is
nothing for the spatial path to match. This is a real weakness and the honest
reason hit@5 is 60% rather than higher.

Raw results, including the full miss list, are written to
[`docs/measurements/resonance_results.json`](docs/measurements/resonance_results.json).

### 10.2 Layer ablation — run; three layers contribute nothing measurable

Three of the thirteen reinforcement layers were disabled one at a time, the
index was rebuilt from the same corpus, and hit@5 was re-measured. The layers
are code modules with no configuration switch, so each variant was produced by a
temporary source patch that makes the layer's entry point a no-op, followed by a
release rebuild ([scripts/ablation.sh](scripts/ablation.sh)).

| Variant | hit@5 | Δ |
|---------|-------|---|
| Baseline (all layers enabled) | 36/60 (60.0%) | — |
| `hebbian` disabled | 36/60 (60.0%) | 0.0 |
| `mirror` disabled | 36/60 (60.0%) | 0.0 |
| `attention` disabled | 36/60 (60.0%) | 0.0 |

**No measurable effect.** The patch was verified to compile and the marker was
confirmed present in the source before each measurement, so these are not
silent no-ops.

Two readings are possible, and this paper does not choose between them without
further evidence:

1. **These layers are not load-bearing for this workload.** On a 60-fact index
   with `mock` coordinates there is very little for reinforcement to learn: each
   fact is stored once, retrieved once, and there is no co-activation structure
   to exploit. A layer that reweights a 60-element result set may simply not
   move the metric.
2. **The evaluation cannot resolve the effect.** With 60 cases, hit@5 changes in
   units of 1.7 percentage points, and the metric is coarse enough that a real
   but small effect could be invisible. A larger corpus with many co-occurring
   queries would be needed.

The remaining ten layers were not ablated; each ablation costs a full release
rebuild (~2.5 min) plus an index rebuild and a measurement.

**The honest summary is that the claim "the thirteen layers are necessary" is
not supported by this evidence.** What is supported: the system reaches 60%
hit@5 with the layers enabled, and three of them can be removed without
changing that number. The FAISS d=3 result in Section 9.3 points the same way --
the spatial coordinates alone score 1.7% at R@1 while the full system scores
48.3%, so *something* beyond raw distance matters, but this ablation cannot say
which layer it is.

### 10.3 Performance

In-process spatial query latencies, measurement method, and the distinction
between inner-loop and end-to-end cost are given in
[BENCHMARKS.md](BENCHMARKS.md).

## 11. Limitations

1. **Lexical retrieval is slow at scale.** Text search scans block contents; cost
   is linear in corpus size, and a large index makes this the bottleneck. The
   in-process spatial path stays fast regardless of index size.
2. **The spatial path is fast but not semantic.** Coordinates are assigned at
   write time; quality for a differently-worded query depends on how they were
   derived. Purely lexical queries are the case handled best.
3. **Semantic search is embedding-dependent.** It inherits the quality and cost
   of whatever model is used, including model download and inference latency.
4. **The measured baseline comparison is unfavourable.** SQLite FTS5 matches or
   beats Microscope on every recall metric tested (R@1 53.3% vs 48.3%, R@5 tied
   at 60.0%, R@10 63.3% vs 60.0%) and is roughly 2,400x faster at p50. On this
   workload there is no demonstrated advantage to the spatial index. The
   60-fact corpus is not a scale test, and a scale test plus a real embedding
   baseline would be needed before either system could be called superior, but
   the current evidence does not support a performance claim.
5. **The reinforcement layers are heuristic.** Drift, decay and weight learning
   use hand-chosen constants; there is no evidence here that they are
   near-optimal, and no hyperparameter sweep has been run.
6. **Cold-start cost dominates for small corpora.** End-to-end latency is
   dominated by process start and state loading, so in-process figures
   understate what a user experiences.
7. **Evaluation is single-user, synthetic, and small.** The resonance set is
   hand-authored with 60 facts and one decoy layer. The k axis does not
   discriminate (Section 10.1), and the `mock` embedding provider means
   "paraphrase" is really "hashes nearby". Real multi-user workloads and a real
   embedding provider are untested.
8. **Retrieval quality is measured but weak.** hit@5 is 60% on the resonance
   set, and a paraphrase with no lexical overlap ("how does the user like
   check-ins" vs. a stored preference) is not retrieved at all. The spatial path
   helps, but it does not substitute for semantic matching.
9. **The layer ablation does not support the thirteen-layer claim.** Disabling
   `hebbian`, `mirror` or `attention` leaves hit@5 unchanged at 60.0%
   (Section 10.2). The patches were verified to compile, so this is a real null
   result rather than a broken experiment. It is consistent with a workload too
   small for reinforcement to matter, but it does mean this paper cannot claim
   that the layers are necessary for the retrieval behaviour it reports.

---

## 12. Conclusion

Microscope Memory implements a hierarchical memory index in which every block
occupies a fixed 256-byte viewport across nine depth levels, D0--D8, and in which
retrieval outcomes are fed back into scoring state. Thirteen such feedback
mechanisms are implemented: Hebbian drift, activation-fingerprint matching,
spatial pulse propagation, pattern archetypes, query-space warping, recall-path
tracking, predictive prefetch, time-windowed profiles, learned attention weights,
cross-instance exchange, offline consolidation, shared state propagation, and
multi-modal storage.

**The evidence assembled here does not establish that these thirteen mechanisms
are necessary, or that the index outperforms simpler alternatives.** On a
60-fact corpus, hit@5 is 60%; SQLite FTS5 matches it at R@5, exceeds it at R@1
and R@10, and does so about 2,400x faster (Section 9.3). Disabling the Hebbian,
fingerprint or attention layer changes hit@5 by 0.0 (Section 10.2). The
in-process spatial query is genuinely fast, but that is the inner loop, not the
user-visible operation (Section 6).

What the work does establish is a reproducible characterisation of the system:
a fixed-size binary index whose read path is allocation-free and whose
end-to-end cost is dominated by process start-up rather than query; a set of
feedback mechanisms that are wired into the recall pipeline but whose effect is
not yet demonstrated at the scale this paper evaluates; and a measurement
harness, committed to the repository, that produces these numbers on demand.

The result is an inspectable memory index with an exposed internal state, and an
evaluation that is designed to be run by a reader rather than taken on trust.
Pure Rust, zero JSON, 413 tests, 54,053 lines.

Released under the MIT License at
[github.com/silentnoisehun/microscope-memory](https://github.com/silentnoisehun/microscope-memory),
archived at DOI
[10.5281/zenodo.22983478](https://doi.org/10.5281/zenodo.22983478).

---

*Microscope Memory is part of the Ora project ecosystem.*
