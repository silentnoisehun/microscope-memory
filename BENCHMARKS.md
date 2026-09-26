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

## Comparison with lexical and vector baselines — MEASURED

Run by [`scripts/compare_baselines.py`](scripts/compare_baselines.py), which
places all systems on the same machine, the same 60-fact corpus and the same 60
questions, and reports **latency and recall together**. Reporting latency alone
would be meaningless, since a system that returns nothing is very fast.

**Run:** commit `e76153f`, Windows 11, AMD Ryzen 5 7535HS (6C/12T), 15.2 GB RAM,
Python 3.14.4, FAISS-CPU 1.14.2, SQLite 3.50.4 (FTS5). FAISS pinned to a single
thread to match one CLI process.

| System | p50 ms | p95 ms | p99 ms | R@1 | R@5 | R@10 |
|--------|--------|--------|--------|-----|-----|------|
| **Microscope** (`recall`, end-to-end process) | 73.34 | 168.64 | 198.79 | 48.3% | 60.0% | 60.0% |
| FAISS `IndexFlatIP` (d=3, Microscope's own coords) | 0.0061 | 0.0138 | 0.0267 | 1.7% | 5.0% | 11.7% |
| FAISS `IndexFlatIP` (d=256, bag-of-words) | 0.0045 | 0.0057 | 0.0084 | 21.7% | 31.7% | 33.3% |
| FAISS `IndexHNSWFlat` (d=256, bag-of-words) | 0.0093 | 0.0119 | 0.0213 | 18.3% | 28.3% | 38.3% |
| **SQLite FTS5** (BM25) | 0.0311 | 0.1003 | 0.1156 | **53.3%** | 60.0% | **63.3%** |

### What this shows, and what it does not

**SQLite FTS5 is better than Microscope on this corpus at R@1 and R@10, and
ties at R@5, while being roughly 2,400x faster at p50.** That is the honest
result and it is stated as such. Microscope's advantage here is not retrieval
quality; it does not have one on this workload.

The asymmetry in the measurement is deliberately left visible: FAISS and FTS5
timings are **query-only** and exclude index build, while Microscope's figure
is an **end-to-end process invocation** including interpreter start, config load
and state load. That asymmetry works *against* Microscope, so the comparison is
conservative -- correcting it would widen the gap, not close it.

Three caveats bound what can be concluded:

1. **60 facts is not a scale test.** Flat exact search over 60 vectors is
   trivially fast and trivially accurate. At the 1.28 M-block corpus the FAISS
   flat scan would have to touch 1.28 M vectors per query and would not
   remain sub-millisecond. This table therefore does **not** establish that
   FTS5 or FAISS wins at scale; it establishes that at small scale, a
   specialised B-tree beats a spatial index on both axes measured here.
2. **The FAISS d=3 row is not a fair representation of Microscope.** It feeds
   FAISS the same hash-derived 3-D coordinates and scores 1.7% at R@1, far
   below Microscope's own 48.3%. The spatial path clearly does more than nearest
   neighbour on those vectors, which is itself an interesting result: the
   hierarchical depth structure and the reinforcement layers carry the
   retrieval, not the raw coordinate distance. A claim that "Microscope is just
   spatial k-NN" is not supported by this measurement.
3. **The bag-of-words FAISS rows are a weak baseline, not a serious one.** 256
   hashed tokens is not a learned embedding. A real embedding model would
   change those numbers substantially, most likely upward. No such model is
   included here, so these rows understate what a production vector store
   achieves.

**Conclusion.** On the workload measured, the spatial index does not outperform
a general-purpose lexical index, and does not outperform a 60-vector exhaustive
search in latency. The case for the architecture rests on the hierarchical
depth structure and the reinforcement layers -- which this table suggests are
doing the work -- and would need a scale test at 10^5--10^6 blocks, with a real
embedding baseline, before any performance claim could be made.


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