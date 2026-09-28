#!/usr/bin/env python3
"""Locate the 60 recall misses: is the answer absent from the index, absent
from the vector list, or present and then dropped by the final ranking?

The benchmark reports R@k. This reports *where* the answer goes, so the next
change has a target instead of a hunch. The binary's diagnostic hook
(MICROSCOPE_EVAL_MATCH + MICROSCOPE_EVAL_DIAG) prints what the vector search
returned and where the expected answer ended up; it never influences ranking,
and MICROSCOPE_NO_LEARN keeps the run from teaching the system while it
measures it.

Usage:  python scripts/diagnose_recall_misses.py [--config bench_config.toml]
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from resonance_set import CASES  # noqa: E402

# The binary's own wording, so the buckets here cannot drift from it. The line
# wraps in a terminal but not in a pipe, so it is read as one string. The fields
# are `vectors`, `want`, `depths`, `all_depths`, `answer` and `answer_sim`;
# `answer` names *where* the expected text turned up, not an index.
VECTOR_LINE = re.compile(
    r"EVALDIAG vectors=(\d+) want=(\d+) depths=\[([^\]]*)\] "
    r"all_depths=\[([^\]]*)\] answer=(\S+)"
)
# Where the binary reports the expected answer having been found.
FOUND_AT = re.compile(r"answer_sim=Some\(\(([\d.eE+-]+),")
# The binary's post-sort verdict: where the answer landed in the final list.
FINAL = re.compile(r"EVALDIAG final=(\S+)(?: pos=(-?\d+))?")
# embedding_index::search will not offer a vector below this cosine as a
# candidate. Measured on the 60-fact benchmark: sweeping it from 0.3 to 0.0
# changes no recall figure, so this constant is a report, not a cause. The
# answers it would exclude still rank inside the fetched list -- they finish
# last. See embedding_index::similarity_floor and MICROSCOPE_SIM_FLOOR.
GATE = 0.3


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", default="bench_config_semantic.toml")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    binary = ROOT / "target/release/microscope-mem.exe"
    if not binary.exists():
        binary = ROOT / "target/release/microscope-mem"
    if not binary.exists():
        print("release binary not found; run: cargo build --release --features embeddings")
        return 1

    config = ROOT / args.config
    env = dict(
        os.environ,
        MICROSCOPE_CONFIG=str(config),
        MICROSCOPE_NO_LEARN="1",
        MICROSCOPE_EVAL_DIAG="1",
    )

    buckets: Counter[str] = Counter()
    # Kept apart from the buckets so the headline count is a case count, not a
    # count of case *and* how it was found.
    verdict_counts: Counter[str] = Counter()
    detail: list[tuple[str, str]] = []
    below_gate: list[tuple[str, str]] = []
    no_vector_line = 0

    for c in CASES:
        case_env = dict(env)
        if not c.match:
            buckets["no match tokens"] += 1
            continue
        case_env["MICROSCOPE_EVAL_MATCH"] = "|".join(c.match)
        r = subprocess.run(
            [str(binary), "recall", c.question, "10"],
            capture_output=True,
            env=case_env,
        )
        err = r.stderr.decode("utf-8", errors="replace")
        out = r.stdout.decode("utf-8", errors="replace").lower()

        m = VECTOR_LINE.search(err)
        if not m:
            no_vector_line += 1
            buckets["no vector line"] += 1
            detail.append((c.question, "no EVALDIAG line: the vector path did not run"))
            continue

        # `answer` names where the binary found the expected text, e.g.
        # `c_in_prefetch`, or says it was outside the fetched depth.
        answer = m.group(5)
        sim = FOUND_AT.search(err)
        sim_txt = f", cosine {sim.group(1)}" if sim else ""
        fin = FINAL.search(err)
        verdict = fin.group(1) if fin else "?"
        # Where it landed in the final list, when the binary says.
        pos = None
        pm = re.search(r"pos=(-?\d+)", verdict)
        if pm:
            pos = int(pm.group(1))
        elif fin and fin.group(2):
            pos = int(fin.group(2))
        pos_txt = f", position {pos}" if pos is not None else ""

        hit_in_ranking = any(tok.lower() in out for tok in c.match)

        if hit_in_ranking:
            buckets["returned in the top 10"] += 1
            verdict_counts[verdict.split(" ")[0]] += 1
        elif verdict.startswith("MISS"):
            # The binary's own verdict, which is the most precise thing here.
            buckets[verdict] += 1
            # A similarity under the gate means the vector path never even
            # considered the answer, which is a different defect from losing a
            # ranked candidate. Keep them apart.
            if sim and float(sim.group(1)) < GATE:
                below_gate.append((c.question, sim.group(1)))
            detail.append((c.question, f"{verdict}{pos_txt}{sim_txt}"))
        else:
            buckets["in the index, not in the top 10"] += 1
            detail.append((c.question, f"found at {answer}{sim_txt}{pos_txt}"))

    total = sum(buckets.values())
    print(f"cases with match tokens: {total}")
    print(f"cases without a vector line: {no_vector_line}\n")
    for k, v in buckets.most_common():
        print(f"  {v:3d}  {k}")
    if verdict_counts:
        print("\nwhere the returns came from:")
        for k, v in verdict_counts.most_common():
            print(f"  {v:3d}  {k}")
    if below_gate:
        print(
            f"\n{len(below_gate)} of the misses scored below the cosine gate "
            f"({GATE}) that embedding_index::search applies, so the vector path"
        )
        print("never offered them as candidates:")
        for q, s in below_gate:
            print(f"  {q}  (cosine {s})")
    if detail:
        print("\nthe misses, in order:")
        for q, why in detail:
            print(f"  {q}\n      {why}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())