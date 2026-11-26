# Evaluation Protocol: LongMemEval-S retrieval (pre-registration)

Status: written 2026-11-26 BEFORE any fix is applied or any post-fix run is made.
Version tested: microscope-mem v1.4.5 (release binary).
Commitment: we publish the result of every run defined here, whatever it shows, including no change or a regression. No run is dropped, re-seeded or re-sampled after seeing results.

## 1. What is measured

Retrieval only. No LLM, no answer generation, no paid API, zero cost. The metrics here are retrieval metrics and are NOT comparable with answer-accuracy numbers published by other memory systems.

## 2. Dataset and sample

- Dataset: LongMemEval-S, file longmemeval_s_cleaned.json, from Hugging Face xiaowu0162/longmemeval-cleaned.
- Dataset file hash (SHA-256): `d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442`
- Sample: 60 questions, seed 42, 10 non-abstention questions per question type (6 types), taken in file order by make_sub.py.
- Full run: all 500 questions, abstention questions marked separately. The baseline for the 500 run is the original, unfixed run that is already running in the background (see section 9: it must be restarted, so the baseline is whatever that restarted run produces).
- Gold: a turn is gold if has_answer is true. In the 60-question sample there are 98 gold turns; 27 of the 60 questions have 2 or more gold sessions.

## 3. Systems and settings

- Microscope: v1.4.5, embedding model all-MiniLM-L6-v2 (Candle), max_depth=3, layers=["long_term"], MICROSCOPE_NO_LEARN=1 (read-only, no learning during measurement).
- Baseline: FAISS IndexFlatIP over the same MiniLM-L6-v2 embeddings, same sample, same seed.
- Harness: run_engine2.py, run_faiss2.py, score2.py, diag.py, common.py, stream.py, make_sub.py. The harness is FROZEN. The measuring scripts are not modified by any fix.
- Scripts resume on rerun. Every run writes its own output files; old outputs are kept.

## 4. Metrics

Primary (turn level, gold = has_answer turns):
- NDCG@10
- turn recall@k for k = 1, 3, 5, 10 (fraction of gold turns found)
- turn recall_any@k
Secondary (session level, same run):
- session recall_any@k and recall_all@k for k = 1, 3, 5, 10
Everything is reported overall and per question type.
Note: D0-D2 summary hits (about 14% of hits) cannot be mapped to a turn and drop out. This hurts the engine, it does not cause the gap to FAISS.
Latency is reported in its own tables, never mixed with quality (see section 8).

## 5. Baseline, measured 2026-11-26 (60 questions, seed 42, v1.4.5, before any fix)

Turn level, Microscope / FAISS, ALL:
- NDCG@10: 58.3 / 58.1
- turn recall@1: 30.3 / 30.3
- turn recall@3: 50.6 / 54.7
- turn recall@5: 62.5 / 66.7
- turn recall@10: 79.7 / 77.8
- turn recall_any@1/3/5/10: 40.0/63.3/73.3/86.7 vs 41.7/71.7/81.7/86.7

Session level, same rerun, ALL:
- recall_any@1/3/5/10: 93.3/95.0/96.7/98.3 vs 93.3/96.7/98.3/98.3
- recall_all@1/3/5/10: 48.3/88.3/91.7/98.3 vs 48.3/90.0/95.0/98.3
(recall_all@1 is not meaningful: 27 of 60 questions have 2+ gold sessions.)

NDCG@10 per type, Microscope / FAISS:
- knowledge-update: 67.4 / 53.7
- multi-session: 51.7 / 56.2
- single-session-assistant: 86.5 / 67.8
- single-session-user: 79.1 / 77.8
- temporal-reasoning: 38.9 / 41.9
- single-session-preference: 26.4 / 50.9
Preference turn recall@10: 36.7% vs 56.7%.

Other measured facts:
- 5 of the 98 gold turns are missing from the engine raw top-60 (FAISS: 0).
- The engine recall call sometimes returns 18-60 hits instead of the requested number.
- Preference misses at k=1 are not a hole in search but ranking: the gold session was at rank 3, 4 and 6 (FAISS: 1, 2, 4); at k=10 all three are found.
- In the preference questions the gold turn is an indirect statement, and the engine top-1 was always a lexically similar turn from another session.
- The engine score (L2) is a fusion, not pure cosine.
- Not caused by session mapping (checked with the turn-id rerun).

## 6. Fix hypotheses (hypotheses, not proven causes)

H1. Rank fusion favors lexical overlap over semantic match for indirect questions. Suspects: semantic_rank_gain (documented default 1.5), the 0.2 cosine floor (env MICROSCOPE_SIM_FLOOR), and the lexical prefilter requirement of at least 3 matching query tokens (env MICROSCOPE_LEX_MIN_MATCHES). semantic_weight appears to affect only coordinate blending, not ranking. This reading comes from Maya's code reading and has not been checked against the code by the measuring side. Verify before using it.
H1b (pre-registered after Step B showed gain=4/8 regress knowledge-update/assistant/temporal): filter functional stopwords in RelevanceQuery::new (keeping len>2 fallback if empty) and quadratically damp weak lexical scores (<0.35) in rank_candidate so noisy 1-token overlap cannot override strong cosine matches on indirect preference questions while preserving gain=1.5 on high-lexical queries.
H1c (pre-registered after H1b regressed preference NDCG@10): apply Reciprocal Rank Fusion (k=20, semantic weight=0.5) inside `rank_candidates` between the hybrid `gain=1.5` ranking and the pure cosine ranking over deduplicated candidates when `semantic_rank_gain > 0.0`. Simulated on `eng_g1.5` + `eng_g8.0` before coding: ALL NDCG@10 56.7 vs FAISS 51.4, pref 54.3, ku 65.9, asst 83.0, user 61.0, temp 48.7.
H1d (pre-registered after H1c lost cosine scores on deduplicated blocks from different depths): build a `canonical_result_text -> max cosine` lookup from `semantic` inside `rank_candidates` so RRF (k=20, w=0.6) never drops a hit's cosine rank across depth deduplication.
H2. A candidate cutoff or floor truncates the list, which explains the 18-60 hits and the 5 missing gold turns.
H3 (documentation only). Speed claims mix two different operations (see section 8).
Out of scope: index build time (about 26 s per 500 turns; materializes every depth level). This is a cost, not a quality defect.

## 7. Sweep plan and decision rules

Allowed changes: only H1 and H2 above. No new setting or switch is added. What is measured is not changed.

Step A (measure only, no code): on the 10 preference questions, sweep semantic_rank_gain = 0.5 / 1.0 / 2.0 / 4.0, plus MICROSCOPE_SIM_FLOOR and MICROSCOPE_LEX_MIN_MATCHES (env vars, no code change needed). About 5 minutes per setting. Record turn recall@10 and NDCG@10 for each. The baseline for this slice must be repeated first with the same harness.
  Stop rule: if no setting improves the preference slice, stop and report. If the sweep is not enough, the next option is a pure-cosine rerank over the top-N hits (code change, needs a new pre-registered line here before it is run).
Step B: take the best setting from Step A and run all 60 questions. Report turn-level NDCG@10, turn recall@1/3/5/10, session recall_any and recall_all, per type.
Step C: if B passes, run all 500 questions with the same setting, abstention marked separately, compared with the unfixed 500 baseline.
Step D: report honestly: improved / not improved / what got worse.

Decision rules (fixed now):
- IMPROVEMENT of the preference slice = NDCG@10 up and turn recall@10 up versus the repeated baseline. Expectation, written before the run: preference NDCG@10 from 26 toward about 50. This is an expectation, not a promise.
- NO REGRESSION = single-session-assistant and knowledge-update NDCG@10 each drop by no more than 3 points, and overall NDCG@10 does not fall.
- A setting that improves preference but breaks the no-regression rule is reported as a trade-off, not as a win.
- Only differences that appear on the turn-level metrics count. Session recall_any is near its ceiling (about 93-98%) and cannot separate the systems.
- With 10 questions per type, one question is 10 points of per-question recall. A difference of 1-2 questions in a 10-question slice is within noise and is reported as such. The 500 run is the confirmation.
- No claim of "better than FAISS" unless the number shows it on the confirmed run, and only for the metric and setting shown.

## 8. Latency (reported separately, never merged with quality)

Two different operations:
- D0 coordinate lookup: about 605 ns (recorded earlier by the maintainer). This is NOT text search.
- Text recall (query to ranked text), end to end including process start, no LLM call: about 87 ms (0.087 s).
- FAISS, query only (search time, no process start, vectors resident): about 0.054 ms.
- Index build: about 25.5 s per question (about 500 turns).
The 87 ms vs 0.054 ms comparison is NOT like for like (process start vs resident search). How much of the 87 ms is process start was not measured. A resident / daemon-mode measurement is needed before any claim about the split.

## 9. Known limitations

- 60 questions (10 per type) is a pilot. Per-type numbers move by whole questions.
- Retrieval metrics are not answer accuracy and cannot be compared with Mem0/Zep style numbers.
- D0-D2 summary hits are not mapped to turns (about 14% of hits).
- With max_depth=3 the D4 hits are lexical only; other depths were not measured.
- The comparison is MiniLM + exact search on both sides, but the retrieval pipelines differ (the engine mixes lexical and semantic candidates; FAISS is pure vector).
- The first 500-question baseline run stalled (one question took 1036 s to index instead of about 26 s; likely memory pressure from two parallel runs on a 2 GB machine). It is being restarted; the earlier time estimate of about 3.5 hours is not reliable. Do not run two jobs in parallel on that machine.
- The expected cause (H1) is unproven until the sweep measures it.

## 10. Predictions (filled in by the owner BEFORE each run)

Run owner: Mate. Runs are started by Mate only.

Step A (preference slice sweep)
- Timestamp of prediction: [_____________________]
- Predicted preference NDCG@10 (best setting): [_____]
- Predicted turn recall@10: [_____]

Step B (60 questions)
- Timestamp: [_____________________]
- Predicted overall NDCG@10: [_____]
- Predicted assistant / knowledge-update change: [_____]

Step C (500 questions)
- Timestamp: [_____________________]
- Predicted overall NDCG@10: [_____]

## 11. Results log (append only, nothing deleted)

| Date | Step | Setting | Result | Verdict |
|------|------|---------|--------|---------|
| 2026-11-26 | Step A | v1.4.6 pref gain=0.5 | pref NDCG@10=31.1, T_R@10=43.3 | worse than baseline |
| 2026-11-26 | Step A | v1.4.6 pref gain=1.0 | pref NDCG@10=42.2, T_R@10=53.3 | slight drop vs 1.5 |
| 2026-11-26 | Step A | v1.4.6 pref gain=1.5 (default) | pref NDCG@10=43.6, T_R@10=53.3 | repeated baseline (60/60 hits) |
| 2026-11-26 | Step A | v1.4.6 pref gain=2.0 | pref NDCG@10=46.7, T_R@10=66.7 | improved |
| 2026-11-26 | Step A | v1.4.6 pref gain=4.0 | pref NDCG@10=55.7, T_R@10=86.7 | improved |
| 2026-11-26 | Step A | v1.4.6 pref gain=8.0 | pref NDCG@10=60.3, T_R@10=90.0 | best Step A setting |
| 2026-11-26 | Step B | v1.4.6 all-60 gain=1.5 (default) | ALL NDCG@10=54.5 (FAISS 51.4), ku=64.5, asst=81.5, pref=43.6 | baseline for v1.4.6 |
| 2026-11-26 | Step B | v1.4.6 all-60 gain=4.0 | ALL NDCG@10=50.7, ku=57.5 (-7.0), asst=75.2 (-6.3), pref=55.7 | FAIL: regression >3 on ku/asst/temp |
| 2026-11-26 | Step B | v1.4.6 all-60 gain=8.0 | ALL NDCG@10=48.7, ku=55.8 (-8.7), asst=66.5 (-15.0), pref=60.3 | FAIL: regression >3 on ku/asst/temp |
| 2026-11-26 | H1b | v1.4.6+H1b pref gain=1.5 | pref NDCG@10=38.3 (vs 43.6), T_R@1=0.0, T_R@10=76.7 | FAIL: stopword removal inflated 1-token coverage; reverted |
| 2026-11-26 | H1c | v1.4.6+RRF(k=20,w=0.5) all-60 | ALL NDCG@10=52.4 (vs 54.5 / FAISS 51.4), T_R@10=78.4, pref=48.4, asst=82.2, ku=62.1, temp=47.8 (-7.3), user=54.4 (-5.3) | trade-off: T_R@10 +2.6 and pref +4.8, but ALL NDCG@10 -2.1; reverted to v1.4.6 |
| 2026-11-26 | H1d | v1.4.6+canon-RRF(k=20,w=0.6) all-60 | ALL NDCG@10=53.1 (vs 54.5 / FAISS 51.4), T_R@10=78.9, pref=52.4, asst=82.2, ku=61.8 (-2.7), user=54.1 (-5.6), temp=47.3 (-7.8), ms=20.7 | trade-off: highest T_R@10 (78.9%) and pref (+8.8), ku/asst pass <=3pt rule, but ALL NDCG@10 -1.4 vs v1.4.6 |
