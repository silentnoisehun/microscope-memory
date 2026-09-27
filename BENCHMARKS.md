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

## Comparison with lexical and vector baselines — partially measured, not yet a fair comparison

> **Warning: the numbers previously published in this section have been
> removed.** They were produced with `embedding.provider = "mock"` and
> `semantic_weight = 0.0` -- the shipped default, documented in
> `config.example.toml` as disabling semantic ranking in `recall` -- and against
> a 60-fact index (4,853 blocks) two orders of magnitude smaller than the real
> 695,868-block corpus. Under those settings the system reduces to
> nearest-neighbour over text hashes. Conclusions drawn from that configuration
> describe the configuration, not the architecture, and have been withdrawn.

A run with a real provider (`candle` + MiniLM) on the full D5 index is
recorded in `WHITEPAPER.md` §10.1. It is reproducible from the committed
config, and it is **worse** than the earlier D4 figure:

| | R@1 | R@5 | R@10 | p50 ms |
|---|---|---|---|---|
| lexical only | 30.0% | 43.3% | 48.3% | 124.9 |
| D5 index, semantic path, `want`=64 (pre-gate) | 31.7% | 46.7% | 48.3% | 287.5 |
| D5 index, semantic path, `want`=256 (pre-gate) | 33.3% | 48.3% | 51.7% | 331.0 |
| **D5 index + embedding quality gate, `want`=256 (current)** | **70.0%** | **80.0%** | **81.7%** | 355.0 / 361.0 |
| *earlier D4 index (9,999 vectors) — superseded, not reproducible* | *56.7%* | *75.0%* | *80.0%* | *283.9* |

The D4 number is retained only to show that it does not reproduce. The 75.0%
was measured on a depth-truncated index with a pre-fix candidate gate; on the
committed D5 configuration the same harness returns 48.3%.

A diagnostic pass classified all 31 remaining misses: 21 are cases where the
correct block never enters the cosine top-1024, 6 are lost to the `want`
pre-fetch, and 4 are lost in the final ranking. Only the pre-fetch group was
addressed, by raising `want` from 64 to 256; 128 recovers none of them and 512
adds nothing over 256.

Three further explanations for the 21 were implemented and measured, and all
three were rejected: raising `want` to 2048 (R@k unchanged, p50 roughly
doubles), capping bit-identical duplicate vectors (unchanged — the apparent
1,996-block cluster was a quantisation artefact; the blocks are near-duplicates)
and scaling the spatial term from 1.0 to 0.0 (unchanged at every scale). The
answers are all embedded with a mean cosine of 0.935 at mean rank 6,209 of
46,565, so the ranking and the embeddings are both sound; the corpus holds too
many near-duplicate D5 summaries for a specific fact to stand out.

Those three rejections held on the **pre-gate** index. The corpus-level remedy
was then implemented — not as the near-duplicate collapse suggested above (a
cosine-keyed dedup would merge contradictory facts: of 40,000 sampled
high-cosine pairs, 1,666 differ in numbers and 245 differ in negation), but as
a text-quality gate in `src/embedding_index.rs`: a 24-character floor
(`MICROSCOPE_MIN_EMBED_CHARS`, default 24), rejection of the reader's `"<bin>"`
/ `"[out of bounds]"` sentinels, and rejection of text whose characters are
more than 25% in the U+0080..U+02FF mojibake band.

| | stored vectors | R@1 | R@5 | R@10 |
|---|---|---|---|---|
| pre-gate (`len >= 3`) | 46,565 | 33.3% | 48.3% | 51.7% |
| **gate, floor 24 (current)** | **9,296** | **70.0%** | **80.0%** | **81.7%** |
| gate, floor 17 (ablation, rejected) | 10,424 | 60.0% | 78.3% | 78.3% |

The gate removed 39,594 of the 48,890 D0–D5 candidates (39,575 short, 18
unencodable, 1 mojibake); the rebuilt index holds 0 blocks of at most 16
characters where the pre-gate index held 36,136 — 78% of its embedded set —
and mean D5→D5 cosine fell 0.9816 → 0.8887. Both harnesses agree
(42/48/49 of 60). Per case: 20 misses recovered, 2 regress — the 22- and
23-character answers #41 "The user has a garden." and #12 "The user is
vegetarian.", which fall under the floor. Floor 17 keeps them and measured
worse overall, so 24 stands.

Two measurement conditions, both measured rather than assumed. **Latency:**
on an idle machine the semantic path (provider, query embedding, vector search)
is 127–137 ms with no variance in 8 of 8 runs and process start is 25–40 ms, so
what is left is learning-state I/O — and that was 22.4 MB of `activations.bin`
read and rewritten per recall. The activation file is now a sparse base plus an
append-only CRC-checked journal of the records a recall touched (372 B + ~24 KB
after 60 recalls on this index), and the two clean-state runs give p50 355.0
and 361.0 ms against 398.5 / 415.6 ms before, with recall unchanged at 42/48/49.
The 1,176 ms figures taken while an index was being rebuilt came from the same
state I/O, not from the gate. **State:** because every recall writes
learning state back, consecutive runs on one build measure different systems —
after ~240 extra recalls R@5/R@10 fell to 76.7%/78.3%, and deleting the
mutable state files restored 42/48/49 twice. `scripts/eval_real.sh` already
does this by removing `eval_output` before it builds.

The FAISS and FTS5 rows are diagnostics, not a like-for-like comparison. They
index only the 60 fact vectors and report query-time search only, while
Microscope scans the full 699,110-block index and its 331 ms is end-to-end
(process start, BERT model load, query embedding, index open). The 96.7% FAISS
R@5 is an upper bound on what the same embeddings achieve with no filtering —
a diagnostic ceiling, not a competitive result.

The harness is committed so a reader can run the comparison correctly:

```bash
# required: a real embedding provider, and semantic_weight > 0
python scripts/build_real_index.sh        # 695,868 blocks from layers/
python scripts/compare_scale.py           # same corpus, same queries, recall@k
bash scripts/eval_real.sh                 # asserts provider/model/weight/depth
```

`scripts/eval_real.sh` now parses the config it generates and aborts unless
`provider`, `model`, `semantic_weight` and `max_depth` are the expected values,
because a malformed TOML makes the binary fall back to built-in defaults
silently, and it no longer rewrites the embedding depth. `compare_scale.py`
reports p50/p95/p99 together with hit@k. A valid comparison additionally
requires the same corpus and the same searchable vector set on both sides, with
end-to-end time and query-only time reported separately.

The script also builds with `--features native,embeddings`. Without that
feature the `candle` provider cannot run, the index is produced without
vectors, and `build` still exits 0 — so the harness would print a result table
measured on a vectorless index. It now asserts `embeddings.bin` exists after
the build, re-checks the index after measuring, and fails if a sanity recall
returns nothing.

Until such a run exists, no claim is made about relative retrieval quality or
speed against any other system.



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