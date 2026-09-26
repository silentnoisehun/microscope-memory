# Benchmarks — Microscope Memory v0.8.2

## What is measured here (read this first)

Every latency in this file is the **in-process spatial query**: a point lookup in
the mmap'd header array followed by a distance computation. That is the number
the system is designed around, and it is the number below.

It is *not* the end-to-end command latency. A full `microscope-mem find` process
also pays for process start-up, config loading, state file loading and the
post-recall reinforcement pipeline. On a 10-block index those costs dominate:

| Path | Latency | What it includes |
|------|---------|------------------|
| In-process spatial query (this table) | ~112 µs avg | header read + L2 distance only |
| Full `find` command, warm page cache | ~186 ms | process start, config, state, pipeline |
| Full `find` command, cold page cache | up to ~3.6 s | same, plus mmap page faults from disk |

These are all correct measurements of different things. Quoting 112 µs as "the
recall latency" is misleading: it describes the inner loop, not the user-visible
operation.

### Method

- **Percentiles:** the per-zoom figures below are means over 10,000 queries per
  level. p50/p95/p99 for the end-to-end path are recorded by
  `scripts/measure_recall.py`; see its output in `docs/measurements/`.
- **Hardware and toolchain:** record CPU model, RAM, filesystem and commit SHA
  alongside any published number. A latency without that context is not
  reproducible.
- **Corpus:** 28,679 blocks across 9 depths. This is the *benchmark corpus*,
  which is smaller than the 1.28 M-block demo corpus in the README. The two are
  different indexes and their numbers are not interchangeable.

## System

- **Binary size:** 2.2 MB (release, stripped)
- **Memory index:** 1010 KB (28679 blocks, 9 depths)
- **Block size:** 256 bytes data + 32 byte header

## In-process spatial query (10,000 queries per zoom level)

| Zoom | Blocks | Avg Query Time |
|------|--------|---------------|
| D0   | 1      | 63.2 µs |
| D1   | 5      | 57.6 µs |
| D2   | 27     | 63.8 µs |
| D3   | 129    | 69.1 µs |
| D4   | 336    | 80.2 µs |
| D5   | 1632   | 127.5 µs |
| D6   | 4183   | 166.3 µs |
| D7   | 11104  | 191.4 µs |
| D8   | 11262  | 189.3 µs |

**Overall average:** 112 µs/query

## Soft 4D Zoom

| Mode | Time |
|------|------|
| 4D soft (all 28679 blocks) | 380 µs/query |

## Comparison with vector databases — NOT MEASURED

**The rows below are unverified third-party figures, not results from this
project.** They are reproduced as commonly cited reference ranges so the
position of the spatial index can be read in context, but they were not
measured on this hardware with this data, and they are not directly comparable:

1. The figures come from vendor documentation and blog posts, not a controlled run.
2. They are *approximate* k-NN over dense embeddings, while Microscope performs
   *exact* spatial recall over a 3-dimensional hierarchical index. These solve
   different problems, so the latencies are not the same metric.
3. The corpora, hardware and thread counts behind those figures are unknown.

| System | Query Type | Reported latency | Source of figure |
|--------|-----------|------------------|------------------|
| FAISS (flat IP) | Approximate k-NN | ~1-5 ms | third-party docs, not measured here |
| Pinecone | Approximate vector search | ~5-20 ms | vendor marketing, not measured here |
| ChromaDB | Approximate vector search | ~5-50 ms | third-party docs, not measured here |
| Qdrant | Approximate vector search | ~4-15 ms | third-party docs, not measured here |
| Weaviate | Approximate vector search | ~5-30 ms | vendor marketing, not measured here |

**A controlled comparison has not been run.** Making this table defensible would
require benchmarking FAISS (flat) and SQLite FTS5 on the same machine, same
corpus and same thread count, with recall@k measured alongside latency. Until
that exists, the only measured claim is the in-process spatial query above, and
this document does not claim to be faster than any embedding store.

**Key difference:** Microscope uses zoom-based hierarchical spatial indexing
(D0-D8), not approximate vector search. It trades semantic fuzziness for
deterministic, exact recall.

## Integrity

- **CRC16 verified:** 28679 blocks OK, 0 errors
- **Merkle Tree:** verified

## Storage

| Metric | Value |
|--------|-------|
| Total memory index | 1010 KB |
| Headers | 896 KB |
| Data | 114 KB |
| Viewport | 256 chars/block |
| Cache | L3 |

## Tests

- **Library tests:** 413 passed, 0 failed (`cargo test --lib`, 2026-09-27)
- **Hook tests:** 16 (`cargo test -p microscope-hooks`)
- **Build:** release mode, LTO thin, panic=abort

## Build

```
cargo build --release
```