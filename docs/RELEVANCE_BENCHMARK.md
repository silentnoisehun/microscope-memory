# Recall relevance benchmark

The relevance quality gate is a deterministic regression test for natural-language recall ranking.

## Run

```powershell
cargo test --release --test relevance_benchmark -- --nocapture
```

## Dataset and metrics

- Corpus: `tests/fixtures/relevance/corpus.tsv` (20 memories)
- Judgements: `tests/fixtures/relevance/cases.tsv` (10 English/Hungarian query-to-memory pairs)
- Metrics: Recall@3 and mean reciprocal rank (MRR)
- Required gate: Recall@3 >= 0.90 and MRR >= 0.80

The benchmark evaluates the legacy clamped keyword-distance formula and the shared hybrid lexical-spatial ranker on the same corpus. The hybrid ranker combines normalized query-token coverage, phrase evidence, spatial distance, and a deliberately small importance prior. Cognitive boosts use divisive adjustment so distinct ranks do not collapse to zero.

## Verified result (2026-08-01)

| Ranker | Recall@3 | MRR | Release fixture time |
|--------|----------|-----|----------------------|
| Legacy clamped keyword distance | 0.900 | 0.492 | 45.7 us |
| Shared hybrid lexical-spatial ranker | **1.000** | **1.000** | 228.6 us |

Fixture time covers all 200 query-document comparisons and is tracked separately from mmap spatial microbenchmarks. Existing performance and integration guarantees remain covered by the full `cargo test` suite.
