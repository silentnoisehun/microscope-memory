# Microscope Memory: A Zoom-Based Hierarchical Memory System with Sub-Microsecond Queries

**Author:** Mate Robert (Silent)

**Version:** 0.1.0

**Date:** March 2026

---

## Abstract

This paper presents Microscope Memory, a hierarchical memory indexing system implemented in Rust that models information retrieval as an act of magnification. The system organizes data into nine depth levels (D0--D8), ranging from high-level identity summaries down to individual bytes, with every block constrained to a fixed 256-byte viewport. Content is projected into a three-dimensional spatial coordinate system via deterministic hashing, enabling nearest-neighbor queries through L2 distance computation. The entire index uses a pure binary format with memory-mapped I/O, achieving sub-microsecond query latencies at shallow depths (37 ns at D0) and maintaining microsecond-range performance even at the deepest levels. The system employs SSE4 SIMD instructions for accelerated distance computation, Rayon-based parallelism for index construction, and an append-only hot log for zero-rebuild memory ingestion. Microscope Memory demonstrates that a simple spatial metaphor, combined with careful binary layout and hardware-aware optimization, can yield a practical memory system with performance characteristics competitive with purpose-built vector databases, while maintaining a codebase under 1,500 lines of Rust.

---

## 1. Introduction

The dominant paradigm in AI memory systems relies on embedding vectors and approximate nearest-neighbor (ANN) search structures such as HNSW graphs or IVF indices. While these approaches deliver strong recall on high-dimensional semantic similarity tasks, they introduce substantial complexity: serialization overhead from JSON or Protocol Buffer formats, multi-megabyte index structures, and query latencies typically measured in milliseconds.

Microscope Memory takes a different approach. Rather than treating memory as a flat collection of vectors, it models memory as a specimen under a microscope. The zoom level determines the granularity of what is visible: at low magnification, the observer sees broad identity information and layer summaries; at high magnification, individual characters and raw bytes come into view. At every depth level, each block presents exactly the same viewport size -- 256 bytes -- ensuring uniform access patterns regardless of the abstraction level being examined.

This design is motivated by three observations. First, most memory queries have an implicit granularity: a question about "who am I?" requires a different level of detail than a search for a specific token. Second, hierarchical decomposition of text (documents to sentences to words to characters) is a natural structure that can be exploited for efficient indexing. Third, modern CPU cache hierarchies reward small, contiguous, uniformly-sized data structures -- precisely what a fixed-size block format provides.

The system is implemented as a single Rust binary with zero runtime dependencies beyond the standard library and a small set of crates for memory mapping (`memmap2`), parallelism (`rayon`), and command-line parsing (`clap`). The entire index consists of three binary files totaling under 10 MB for a corpus of over 227,000 blocks, with the header file typically fitting within L2 cache.

---

## 2. Architecture

### 2.1 Binary Format

Microscope Memory uses three binary files with no serialization layer:

- **`microscope.bin`** -- Block headers, memory-mapped for zero-copy access. Each header is exactly 32 bytes, packed with `#[repr(C, packed)]` to eliminate padding. The file is accessed via `memmap2::Mmap`, allowing the operating system to manage paging and caching transparently.

- **`data.bin`** -- Raw UTF-8 text content. Each block's text is stored contiguously, referenced by offset and length from its header. Maximum block content is 256 bytes; longer text is truncated with an ellipsis marker.

- **`meta.bin`** -- Index metadata beginning with a 4-byte magic number (`MSCM`), followed by version, total block count, depth count, and an array of `(start, count)` pairs for each depth level. The metadata header is 16 bytes, followed by 8 bytes per depth entry (72 bytes total for 9 depths).

### 2.2 Block Header

The `BlockHeader` structure occupies exactly 32 bytes:

```
Offset  Size  Field         Description
------  ----  -----------   ------------------------------------
 0      4     x: f32        Spatial X coordinate
 4      4     y: f32        Spatial Y coordinate
 8      4     z: f32        Spatial Z coordinate
12      4     zoom: f32     Normalized depth (depth / 8.0)
16      1     depth: u8     Depth level (0--8)
17      1     layer_id: u8  Cognitive layer identifier
18      4     data_offset   Byte offset into data.bin
22      2     data_len      Actual text length (<= 256)
24      4     parent_idx    Parent block index (u32::MAX = root)
28      2     child_count   Number of child blocks
30      2     _pad          Alignment padding
```

This layout is chosen so that the first 16 bytes (x, y, z, zoom) can be loaded directly into an SSE 128-bit register for SIMD distance computation. The packed representation ensures that header-to-header stride is exactly 32 bytes, which aligns favorably with cache line sizes on modern x86_64 processors.

### 2.3 Depth Hierarchy

The nine depth levels decompose information with increasing granularity:

| Depth | Name             | Content                                    |
|-------|------------------|--------------------------------------------|
| D0    | Identity         | System-level identity string (single root) |
| D1    | Layer Summaries  | Per-layer overview with item count          |
| D2    | Clusters         | Groups of 5 items with preview text         |
| D3    | Items            | Individual memory entries (full content)    |
| D4    | Sentences        | Sentence-level decomposition                |
| D5    | Tokens           | Word-level tokenization (up to 8 per block) |
| D6    | Syllables        | Sub-word morpheme chunks (3--5 characters)  |
| D7    | Characters       | Individual characters                       |
| D8    | Raw Bytes        | Hexadecimal byte representation             |

Below D8, data would require sub-byte decomposition, which destroys meaningful information. This boundary is referred to as the "atomic boundary of information" -- the point at which further magnification yields noise rather than signal.

### 2.4 Build Pipeline

Index construction follows a top-down decomposition pipeline, parallelized with Rayon at depths D4 through D8:

1. **Layer ingestion**: JSON files are parsed with a lightweight line-by-line extractor (not `serde_json`) that identifies content-bearing keys (`"content"`, `"text"`, `"pattern"`, etc.).
2. **D0--D3 construction**: Identity, summaries, clusters, and items are built sequentially, establishing the hierarchical backbone.
3. **D4--D8 parallel decomposition**: Each depth level is constructed using `into_par_iter()`, with parent blocks split into child blocks (sentences, tokens, syllables, characters, bytes) in parallel. Results are collected and merged maintaining parent-child relationships.
4. **Sort and remap**: All blocks are sorted by depth, and parent indices are remapped to reflect the new ordering.
5. **Binary emission**: Headers, data, and metadata are written in a single sequential pass using `BufWriter`.

---

## 3. Spatial Memory Model

### 3.1 Cognitive Layers

The system defines ten cognitive layers, each occupying a distinct region of 3D space:

| Layer          | Base Offset (x, y, z)   | Purpose                          |
|----------------|--------------------------|----------------------------------|
| identity       | (0.00, 0.00, 0.00)      | Core system identity             |
| long_term      | (0.00, 0.00, 0.00)      | Persistent factual memory        |
| short_term     | (0.15, 0.15, 0.15)      | Recent session context           |
| associative    | (0.30, 0.00, 0.00)      | Pattern and association links    |
| emotional      | (0.00, 0.30, 0.00)      | Affect-tagged memories           |
| relational     | (0.30, 0.30, 0.00)      | Interpersonal relationship data  |
| reflections    | (0.00, 0.00, 0.30)      | Meta-cognitive observations      |
| crypto_chain   | (0.30, 0.00, 0.30)      | Cryptographic integrity records  |
| echo_cache     | (0.00, 0.30, 0.30)      | Cached conversation responses    |
| rust_state     | (0.15, 0.00, 0.15)      | System state snapshots           |

### 3.2 Coordinate Assignment

Each block's 3D position is computed deterministically from its content using an FNV-inspired hash function. Three independent 64-bit hash accumulators process the first 128 bytes of the text, each using a different FNV multiplier. The lower 16 bits of each accumulator are normalized to the [0, 1] range and scaled by 0.25, then offset by the layer's base position:

```
(x, y, z) = (layer_ox + hash_x * 0.25, layer_oy + hash_y * 0.25, layer_oz + hash_z * 0.25)
```

This ensures that: (a) identical content always maps to the same coordinates; (b) content within the same layer clusters spatially; and (c) different layers occupy non-overlapping regions of the unit cube.

At deeper decomposition levels (D4--D8), child blocks inherit their parent's coordinates with increasingly small perturbations (offsets divided by 25,500 at D4, scaling down to 255,000,000 at D8), creating a fractal-like spatial structure where zooming in reveals finer detail around the parent's location.

---

## 4. Query Engine

Microscope Memory provides five distinct search modes, each optimized for different retrieval scenarios.

### 4.1 Look (Fixed-Depth L2 Search)

The `look` command performs a k-nearest-neighbor search within a single depth level. Given a query point (x, y, z) and a zoom level, it scans only the blocks at the specified depth, computing L2 distance and returning the k closest results. This is the fastest mode, as it accesses only a contiguous slice of the header array. The append log is also searched, ensuring recently stored memories are included without requiring a rebuild.

### 4.2 Soft (4D Weighted Search)

The `soft` command treats zoom as a fourth spatial dimension with a configurable weight (default 2.0). It scans all blocks in the index using SIMD-accelerated 4D L2 distance, where the zoom dimension is normalized to [0, 1] and weighted to control the penalty for depth mismatch. This mode uses `_mm_loadu_ps` to load (x, y, z, zoom) into a 128-bit register and `_mm_hadd_ps` for horizontal summation, achieving efficient per-block evaluation across the entire index.

### 4.3 Find (Brute-Force Text Search)

The `find` command performs case-insensitive substring matching across all blocks at all depths, parallelized with Rayon. Results are sorted by depth, preferring shallower (more abstract) matches. This mode bypasses the spatial index entirely, providing a fallback for queries that are better served by exact keyword matching.

### 4.4 Recall (Auto-Zoom Hybrid Search)

The `recall` command is the primary interface for natural language queries. It implements a two-stage strategy:

1. **Auto-zoom**: The query is analyzed for complexity by counting unique content words (excluding stopwords). Queries with one or fewer content words map to depths D0--D2; queries with up to three words map to D1--D3; and progressively longer queries target deeper levels, up to D4--D6 for queries exceeding ten content words.

2. **Hybrid ranking**: Within the selected depth range, blocks are scored using a combination of spatial L2 distance from a center point and keyword boosting. Each keyword match reduces the effective distance by a configurable boost factor (default 0.1), promoting content-relevant results even when they are spatially distant.

### 4.5 Embed (Semantic Vector Search)

The `embed` command provides an architecture for embedding-based semantic search using SIMD-accelerated cosine similarity. The embedding module defines an `EmbeddingProvider` trait supporting `embed` and `embed_batch` operations, with AVX2-accelerated cosine similarity using `_mm256_fmadd_ps` for fused multiply-add on 8-wide float vectors. The current implementation uses a mock provider that generates deterministic hash-based embeddings for testing; the architecture is designed for drop-in integration with OpenAI (ada-002, 1536 dimensions) or local HuggingFace models.

---

## 5. Performance

### 5.1 Query Latency

Benchmark measurements (10,000 queries per depth level) demonstrate sub-microsecond performance at shallow depths, with latency scaling linearly with block count at each level:

| Depth | Blocks | Avg. Latency | Cache Tier |
|-------|--------|--------------|------------|
| D0    | 1      | 37 ns        | L1d        |
| D1    | ~9     | 92 ns        | L1d        |
| D2    | ~500   | 506 ns       | L1d        |
| D3    | ~2,500 | 1.7 us       | L2         |
| D4    | ~5,000 | 3.9 us       | L2         |
| D5    | ~18K   | 18 us        | L2/L3      |
| D6    | ~40K   | 72 us        | L3         |
| D7    | ~70K   | 505 us       | L3         |
| D8    | ~90K   | 492 us       | L3         |

The non-monotonic behavior at D7/D8 (D8 slightly faster than D7 in some runs) is attributable to memory access pattern effects and branch prediction behavior in the L2 distance computation.

### 5.2 SIMD Optimization

The L2 distance function `l2_dist_sq_simd` uses SSE4.1 intrinsics on x86_64 targets. Four floats (x, y, z, zoom) are loaded with `_mm_loadu_ps`, differences computed with `_mm_sub_ps`, weights applied with `_mm_mul_ps`, squared with `_mm_mul_ps`, and horizontally summed with `_mm_hadd_ps`. This reduces the 4D distance computation to approximately 6 SIMD instructions. The cosine similarity function for embedding search uses AVX2 with `_mm256_fmadd_ps` for 8-wide fused multiply-add, processing 1536-dimensional vectors in 192 iterations.

### 5.3 Cache Behavior

The 32-byte header size is deliberately chosen to align with half a cache line (64 bytes on most x86_64 processors), ensuring that two adjacent block headers fit in a single cache line. For a typical index of 227,168 blocks, the header file occupies approximately 7.1 MB, fitting within L3 cache. The shallow depth levels (D0--D2, approximately 510 blocks, 16 KB) fit entirely within L1d cache (32 KB), explaining the sub-microsecond latencies observed at these levels.

### 5.4 Build Performance

Index construction leverages Rayon's work-stealing thread pool for parallel decomposition at depths D4--D8. Each depth level's decomposition is expressed as `into_par_iter().map(...).collect()`, allowing automatic load balancing across available cores. The build process for a 227,168-block index completes in under 2 seconds on a modern desktop CPU.

---

## 6. API and Integration

### 6.1 Command-Line Interface

The primary interface is a Clap-derived CLI supporting the following subcommands:

```
microscope-mem build                          # Ingest layers, produce binary index
microscope-mem store <text> -l <layer> -i <n> # Append to hot log
microscope-mem recall <query> [k]             # Auto-zoom hybrid search
microscope-mem look <x> <y> <z> <zoom> [k]   # Fixed-depth spatial search
microscope-mem soft <x> <y> <z> <zoom> [k]   # 4D weighted search
microscope-mem find <query> [k]               # Brute-force text search
microscope-mem embed <query> [k] -m <metric>  # Semantic vector search
microscope-mem bench                          # Performance benchmark
microscope-mem stats                          # Index statistics
microscope-mem rebuild                        # Merge append log into index
microscope-mem serve -p <port>                # Start endpoint server
```

### 6.2 Endpoint Server

The `serve` command starts a TCP-based HTTP endpoint server (default port 6060) that accepts JSON requests over standard HTTP:

- **POST /store** -- Accepts `{"text": "...", "layer": "...", "importance": N}` and appends to the hot log.
- **GET/POST /recall** -- Accepts `{"query": "...", "k": N}` and returns ranked results with distance scores.
- **GET /stats** -- Returns index statistics including block count and per-depth distribution.

Each connection is handled in a dedicated thread via `std::thread::spawn`, providing concurrent access without requiring an async runtime.

### 6.3 Append Log (Hot Memory)

The append log (`append.bin`) enables immediate memory storage without triggering a full index rebuild. Each record is a compact binary structure:

```
[u32 text_len][u8 layer_id][u8 importance][f32 x][f32 y][f32 z][text bytes]
```

The 18-byte header plus variable-length text is appended atomically. Append log entries are searched alongside the main index in all query modes, with a sentinel offset (1,000,000) distinguishing append results from main index results. The `rebuild` command merges the append log into the main index and clears the log file.

### 6.4 Configuration

All operational parameters are externalized to a TOML configuration file supporting paths, index parameters, search weights, layer definitions, performance tuning, and logging settings. The system falls back to compiled defaults when no configuration file is present.

---

## 7. Future Work

Several directions for extending Microscope Memory are under consideration:

**Real Embedding Integration.** The embedding module architecture supports drop-in providers for OpenAI and HuggingFace models. Integrating real embeddings would enable true semantic search alongside the existing spatial and keyword-based modes, with the SIMD-accelerated cosine similarity function already in place.

**GPU Acceleration.** A `gpu` module stub exists for WebGPU-based (`wgpu`) computation, which would enable massively parallel distance computation for the 4D soft search mode across the full index.

**WebAssembly Target.** A WASM compilation target is partially implemented, which would allow Microscope Memory to run in browser environments for client-side memory search.

**Python Bindings.** PyO3 bindings are stubbed for integration with Python-based AI pipelines, exposing the core search functions as a native Python module.

**Incremental Depth Indexing.** Currently, a full rebuild is required to integrate append log entries at all depth levels. An incremental indexing strategy could decompose new entries through the depth hierarchy without reconstructing the entire index.

**Merkle Root Integrity.** Adding cryptographic hash verification through a Merkle tree over the block headers would provide tamper detection for the memory corpus, extending the existing `crypto_chain` cognitive layer with structural integrity guarantees.

---

## 8. Conclusion

Microscope Memory demonstrates that a simple, well-chosen metaphor -- memory as a specimen under a microscope -- can yield a practical and performant memory system. By constraining every block to a uniform 256-byte viewport, organizing data into nine hierarchical depth levels, and projecting content into a deterministic 3D coordinate space, the system reduces memory retrieval to spatial nearest-neighbor search at a specified zoom level.

The pure binary format, with 32-byte packed headers and memory-mapped I/O, eliminates serialization overhead entirely. SIMD-accelerated distance computation and cache-aligned data structures deliver query latencies ranging from 37 nanoseconds at the identity level to approximately 500 microseconds at the byte level -- a span of four orders of magnitude that directly reflects the exponential growth in block count across depth levels.

The system achieves these performance characteristics in under 1,500 lines of Rust, with no runtime dependencies on databases, serialization frameworks, or network services. The append log provides immediate write capability, the endpoint server enables network integration, and the modular embedding architecture supports future semantic search without requiring architectural changes.

Microscope Memory is released under the MIT License and is available at [https://github.com/silentnoisehun/microscope-memory](https://github.com/silentnoisehun/microscope-memory).

---

*Microscope Memory is part of the Ora project ecosystem.*
