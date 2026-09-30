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

## BEIR SciFact — the measurement to argue with

Everything below the Method section that uses recall was measured on a
60-entry set of hand-written facts, and a reader is right to discount it: that
set stores only the answers, so retrieval is close to trivial. SciFact is a
public claim-verification corpus, so the number is one anybody can rerun and
disagree with.

**Setup.** All 5,183 abstracts stored, 286 test queries, `MICROSCOPE_NO_LEARN=1`,
one clean run, `scifact_config.toml` with `max_depth = 3` so the embedded set is
one vector per abstract — the same count FAISS embeds. Index: 5,183 blocks, 6,230
embedded, 1,331 MB. 14 of the 300 queries are dropped because no unique
3–12 word phrase could be found for them; a match token occurring in more than
one document would credit a retrieval that found the wrong document.

| System | p50 ms | R@1 | R@5 | R@10 |
|---|---|---|---|---|
| **microscope (recall, end-to-end)** | 600.6 | **53.1%** | **74.8%** | **81.8%** |
| faiss IndexFlatIP (MiniLM) | 0.41 | 48.3% | 73.4% | 78.3% |
| faiss IndexHNSWFlat (MiniLM) | 0.06 | 47.6% | 72.0% | 76.6% |
| sqlite fts5 (bm25) | 8.06 | 45.8% | 66.8% | 74.8% |

Microscope leads on every k, and the ordering against the lexical baseline is the
expected one. Two things this table does **not** say:

- **The latency column is not a speed comparison.** It was previously explained
  here as "process start, config load and opening a 1.3 GB index".
  `bench-recall` shows that is wrong: in a resident process with everything
  warm, one recall on the 967,587-block evaluation index costs **23.5 ms at
  p50** against a first call of 95.1 ms, so process start and index load are
  worth about 13 ms of the *first* call and nothing per query. The per-query
  cost is real work, and by phase (mean over 31 calls) it is 15.0 ms embedding
  the query, 5.7 ms scoring 4,310 candidate blocks, 2.1 ms vector search, 1.0 ms
  loading state, and under 1.5 ms for everything else. The gap to FAISS's
  0.41 ms stays real, and the query embedding alone is 36x it -- which FAISS
  does not pay, because it searches pre-computed query vectors. The split has
  not been measured on the SciFact index -- see the note below for why that row
  cannot presently be reproduced.
- **These are recall@k, not the nDCG@10 the BEIR papers report,** so they are not
  comparable to published SciFact numbers. The only comparison here is between
  the four rows, which share a corpus, a query set and a scorer.

### The SciFact row is not reproducible on this machine, and it is a size problem

The 600.6 ms p50 in the table above was measured once, on an index that is not
in this checkout. Rebuilding it to measure the phase split fails:

```
python scripts/build_scifact_index.py --force
  5633165 blocks total
  Depth 0:        1      Depth 5:   377746
  Depth 1:        9      Depth 6:   800610
  Depth 2:     1037      Depth 7:  2198808
  Depth 3:     5183      Depth 8:  2200361
  Depth 4:    49410
  Embedding up to 6230 blocks (D0-D3, dim=384)
  build failed: "write links.bin: Nincs elég hely a lemezen. (os error 112)"
```

The interesting part is the shape of that index. **5,183 documents become
5,633,165 blocks**, 4.4 million of them at depths 7 and 8. The 16 KiB
`BLOCK_DATA_SIZE` change made a stored document one block, but the hierarchy
still subdivides every document down to the leaf depth, and the derived files
scale with the block count rather than the document count. Every file came out
5.8x the evaluation index's, which is exactly the ratio of the two block
counts:

| file | eval (967,587 blocks) | SciFact (5,633,165) |
|---|---:|---:|
| `merkle.bin` | 59.1 MB | 343.8 MB |
| `microscope.bin` | 46.1 MB | 268.6 MB |
| `fingerprints.idx` | 25.8 MB | 150.4 MB |
| `text_index.bin` | ~2.6 MB | 19.6 MB |

With `links.bin` still to write (87.2 MB at 967,587 blocks, so roughly 500 MB
here), the index needs about 1.4 GB. The drive had 0.48 GB free and the build
had already written 822 MB when it stopped. The partial output was removed,
which freed the space back to 1.29 GB -- not enough.

Two things follow, and neither is a latency claim:

- **The headline SciFact number is a measurement from a machine state that no
  longer exists here.** It should be read as "measured once, not re-runnable
  without more disk", not as a current figure.
- **The 25x gap against the evaluation index is not explained.** 5,183
  documents answering in 600 ms while 967,587 blocks answer in 23.5 ms is not
  a size effect, because the smaller corpus should be the faster one. The
  phase trace exists and would answer it in one run; it needs ~1.4 GB to do
  so. Until then the honest statement is that the gap is unexplained, not that
  it is anything in particular.

### Getting here: three runs, and the first two were wrong

This is recorded because the error is instructive and because two earlier
commit messages in this repository state the opposite conclusion.

| | truncated | split | **current** |
|---|---|---|---|
| blocks | 5,181 | 10,518 | **5,183** |
| R@1 | 51.0% | 41.6% | **53.1%** |
| R@5 | 74.8% | 67.5% | **74.8%** |
| R@10 | 80.4% | 76.2% | **81.8%** |
| p50 ms | 434.3 | 605.4 | 600.6 |

**Run 1 (51.0%) led FAISS while discarding 83% of the corpus.** `BLOCK_DATA_SIZE`
was 1,024 bytes and `to_block` truncated 4,300 of the 5,183 abstracts at byte
1,021, so the tail of most documents was never stored, embedded or printed for
the scorer to read. The corpus was not complete; the number was an artefact.

**Run 2 (41.6%) removed the data loss and lost 9.4 points of R@1.** Splitting the
tail into further blocks kept every byte, but a 1,400-character abstract is a
better retrieval unit than two ~700-character fragments: the sentence that
answers the query is separated from the title it matches against. The fix
removed a data-loss bug and made retrieval worse, which is the honest reading.

**Run 3 (53.1%) stores whole documents.** `BLOCK_DATA_SIZE` is now 16 KiB, which
holds the longest abstract (10,127 bytes), so one block is one document. This was
safe to do because `data.bin` is a packed, variable-length file whose real span
lives in `BlockHeader`'s `data_offset`/`data_len` — not a fixed-stride grid, as two
earlier commit messages here claimed. The real ceiling is the `u16` `data_len`,
65,535 bytes.

Raising the limit also exposed a second bug. The layer reader *packed* consecutive
lines into one block until the limit was reached, which was invisible at 1,024
bytes where a typical abstract overflowed the limit alone. At 16 KiB it turned
5,183 abstracts into 504 blocks of roughly ten documents each. Caught during the
rebuild, before it could produce a plausible-looking but meaningless R@k.

The three baseline rows are byte-identical across all three runs (48.3/73.4/78.3,
47.6/72.0/76.6, 45.8/66.8/74.8), which is the evidence that the movement is ours
and the measurement is deterministic.

### Reproduce

```
python scripts/build_scifact_index.py --force     # downloads SciFact, builds the index
python scripts/compare_baselines.py --corpus scifact
```

Output: `docs/measurements/scifact_comparison.json`, which carries the corpus
name and config file in the payload. Results are written to a corpus-specific
file so a SciFact number can never be mistaken for a 60-fact number.

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
recorded in `WHITEPAPER.md` §11.1. It is reproducible from the committed
config, and it is **worse** than the earlier D4 figure:

| | R@1 | R@5 | R@10 | p50 ms |
|---|---|---|---|---|
| lexical only | 30.0% | 43.3% | 48.3% | 124.9 |
| D5 index, semantic path, `want`=64 (pre-gate) | 31.7% | 46.7% | 48.3% | 287.5 |
| D5 index, semantic path, `want`=256 (pre-gate) | 33.3% | 48.3% | 51.7% | 331.0 |
| D5 index + gate, before the padding fix | 70.0% | 80.0% | 81.7% | 323.2 / 331.5 |
| D5 eval index, rebuilt at 16 KiB blocks (current) | 78.3% | 80.0% | 80.0% | 119.4 |
| *earlier D4 index (9,999 vectors) — superseded, not reproducible* | *56.7%* | *75.0%* | *80.0%* | *283.9* |

The current row is the same harness on the same 967,587-block corpus, after
rebuilding the index so the vectors are not 99% padding. R@1 moved by 8.3
points and p50 by 2.7×; R@5 and R@10 did not move at all.

**That row was superseded for one revision and has since been rebuilt**, so it is
current again. The old index was built with the 1,024-byte limit and with the
layer reader that packed consecutive lines into shared blocks, neither of which
the current code does. Rebuilt at 16,384 bytes: **967,587 blocks, 13,640
embedded, 246 MB**. R@1 and R@5 returned identical (78.3% / 80.0%), p50 moved
1.9 ms, and R@10 fell from 81.7% to 80.0% — one case in 60, which a single
60-query run cannot separate from noise, so read it as 80% ± one case rather
than a regression. For a public corpus with enough queries to resolve a
one-case difference, see the SciFact table at the top of this file.

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
| gate, floor 20, rebuilt at 16 KiB blocks (current) | 13,640 | 78.3% | 80.0% | 80.0% |
| gate, floor 24, before the padding fix | 9,296 | 70.0% | 80.0% | 81.7% |
| gate, floor 17 (ablation, rejected) | 10,424 | 60.0% | 78.3% | 78.3% |

The floor-17 row is void: it was measured before the padding fix, on vectors
that were 99% padding. Re-measured on the fixed build the curve is flat from 20
down to 12, so 20 is the floor and 17 is no longer the question.

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
read and rewritten per recall, a full-corpus header scan for the emotional
field (11.7 ms) and a 58.7 MB emotion lookup that `detect_eureka` loaded and
never read (17.5 ms). With the activation file sparse plus a CRC-checked delta
journal, the header scan restricted to the hot set, and the lookup loaded only
when a query emotion exists, the clean-state runs give p50 323.2 and 331.5 ms
against 405.9 ms before, with recall unchanged at 42/48/49. A run started
right after a build measured 1,123 ms with the same recall: the published
figures are unloaded runs. The small state files are deliberately not
dirty-tracked: per file they look expensive (7.2 ms for a 48-byte
`attention.bin`, 13.6 ms for a 12 KB `thought_graph.bin`) but the cost does not
track size — in a fresh process the first read costs 21.7 ms, the second 9.5 ms
and the rest 0.06–0.26 ms, so skipping a file only moves the first-touch
penalty onto the next one. **State:** because every recall writes
learning state back, consecutive runs on one build measure different systems —
after ~240 extra recalls R@5/R@10 fell to 76.7%/78.3%, and deleting the
mutable state files restored 42/48/49 twice. `scripts/eval_real.sh` already
does this by removing `eval_output` before it builds.

The FAISS and FTS5 rows are diagnostics, not a like-for-like comparison. They
index only the 60 fact vectors and report query-time search only, while
Microscope scans the full 967,587-block index and its 331 ms is end-to-end
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