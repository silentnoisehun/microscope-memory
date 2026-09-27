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

## Inverted text index and reinforcement cost (measured)

This is the benchmark the project's own `.scratch_bench.ps1` performs: the
inverted text index present versus removed, on the real `layers/` corpus.
Reproduced by [`scripts/bench_text_index.sh`](scripts/bench_text_index.sh).

**Run:** 2026-09-27, Windows 11, AMD Ryzen 5 7535HS, 694,868 blocks,
`text_index.bin` = 2.6 MB, page cache warm, `provider = "mock"` and
`semantic_weight = 0.0` (this measures the text path, not the embedding path).
Each figure is a single invocation including process start-up, so the numbers
are end-to-end and dominated by fixed cost.

| Query | With text index | Without (full scan) |
|-------|-----------------|---------------------|
| `find "memory indexing"` | 91 ms | 89 ms |
| `find "binary mmap recall"` | 92 ms | 89 ms |
| `find "cognitive systems"` | 97 ms | 90 ms |
| `recall "memory indexing"` | 218 ms | 227 ms |
| `recall "hebbian remap bug fix"` | 219 ms | 314 ms |

**The inverted text index shows no measurable speedup at this scale.** `find`
is within noise with and without it, and `recall` is not consistently better
with it. The second `recall` row is the one apparent exception (219 ms vs
314 ms) but it is a single sample and the other `recall` row runs the opposite
way (218 vs 227), so no ordering can be claimed from these figures. What the
table does establish is that at 695k blocks the text path is not the bottleneck
— something else dominates.

**The reinforcement write block is the measurable cost.** A `recall` that
returns nothing skips the post-recall write entirely:

| Path | Latency |
|------|---------|
| `recall` with hits (full pipeline + write) | 218 ms |
| `recall`, zero hits (write block skipped) | 114 ms |
| `find`, zero hits | 91 ms |

That is roughly a **100 ms** difference attributable to the reinforcement stack
and its state persistence, on a warm cache. This is the cost the layers impose
on every non-empty recall, and it is a real, reproducible figure.

Two honest caveats: these are single samples, not distributions, so they should
be read as order-of-magnitude; and process start-up is included in every row,
which is why the absolute values sit near 100 ms even for a no-op query. A
p50/p95/p99 over many runs, and an in-process timing that excludes start-up,
would be needed to state this precisely.

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

## Comparison with lexical and vector baselines — NOT YET VALIDLY MEASURED

> **Warning: the numbers previously published in this section have been
> removed.** They were produced with `embedding.provider = "mock"` and
> `semantic_weight = 0.0` -- the shipped default, documented in
> `config.example.toml` as disabling semantic ranking in `recall` -- and against
> a 60-fact index (4,853 blocks) two orders of magnitude smaller than the real
> 695,868-block corpus. Under those settings the system reduces to
> nearest-neighbour over text hashes. Conclusions drawn from that configuration
> describe the configuration, not the architecture, and have been withdrawn.

The harness is committed so a reader can run the comparison correctly:

```bash
# required: a real embedding provider, and semantic_weight > 0
python scripts/build_real_index.sh        # 695,868 blocks from layers/
python scripts/compare_scale.py           # same corpus, same queries, recall@k
```

`scripts/compare_scale.py` reports p50/p95/p99 latency together with hit@k, and
covers a 60-fact and a 695k-block index so the two can be compared directly.
A valid comparison additionally requires a real embedding provider (candle/BERT
or ONNX), since a bag-of-words or hash representation understates what a
production vector store achieves and is not a fair proxy for the semantic path.

Until that run exists, the only measured latency figures in this document are
the in-process spatial query above, and no claim is made about relative
retrieval quality or speed against any other system.



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