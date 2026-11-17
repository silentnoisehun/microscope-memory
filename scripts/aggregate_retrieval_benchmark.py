#!/usr/bin/env python3
"""Aggregate a retrieval benchmark TSV into R@1 / R@5 / R@10 / MRR.

Input  : the TSV written by scripts/retrieval_benchmark.ps1
         (case_id, lang, variant, query, needle, evaldiag)
Output : a table per variant, overall and split by language, plus the
         comparison against the lexical baseline (how many top-5 hits the
         hybrid path gained and lost).

Usage:
    python scripts/aggregate_retrieval_benchmark.py bench_results.tsv
"""
from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

POS_RE = re.compile(r"pos=(\d+)")
SRC_RE = re.compile(r"source=(\w+)")


def parse(path: Path):
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines:
        return []
    header = lines[0].split("\t")
    idx = {name: i for i, name in enumerate(header)}
    rows = []
    for line in lines[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        diag = parts[idx["evaldiag"]]
        m = POS_RE.search(diag)
        s = SRC_RE.search(diag)
        rows.append({
            "case": parts[idx["case_id"]],
            "lang": parts[idx["lang"]],
            "variant": parts[idx["variant"]],
            "query": parts[idx["query"]],
            "pos": int(m.group(1)) if m else None,
            "src": s.group(1) if s else "none",
        })
    return rows


def metrics(rows):
    n = len(rows)
    if n == 0:
        return None
    r1 = sum(1 for r in rows if r["pos"] == 0)
    r5 = sum(1 for r in rows if r["pos"] is not None and r["pos"] < 5)
    r10 = sum(1 for r in rows if r["pos"] is not None and r["pos"] < 10)
    rr = sum(1.0 / (r["pos"] + 1) for r in rows if r["pos"] is not None)
    src = defaultdict(int)
    for r in rows:
        src[r["src"]] += 1
    return {"n": n, "R@1": r1 / n, "R@5": r5 / n, "R@10": r10 / n, "MRR": rr / n,
            "found": sum(1 for r in rows if r["pos"] is not None), "src": dict(src)}


def fmt(m):
    if m is None:
        return "no rows"
    return (f"R@1={m['R@1']:.3f}  R@5={m['R@5']:.3f}  R@10={m['R@10']:.3f}  "
            f"MRR={m['MRR']:.3f}  found={m['found']}/{m['n']}")


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "bench_results.tsv")
    if not path.exists():
        print(f"{path} missing", file=sys.stderr)
        return 1
    rows = parse(path)
    if not rows:
        print("no rows", file=sys.stderr)
        return 1

    variants = []
    for r in rows:
        if r["variant"] not in variants:
            variants.append(r["variant"])

    print(f"\n=== RETRIEVAL BENCHMARK ({path.name}) ===")
    print(f"queries: {len(rows) // len(variants)} per variant   variants: {len(variants)}\n")

    for variant in variants:
        vrows = [r for r in rows if r["variant"] == variant]
        print(f"--- {variant} ---")
        print(f"  all      {fmt(metrics(vrows))}")
        for lang in ("hu", "en"):
            lrows = [r for r in vrows if r["lang"] == lang]
            if lrows:
                print(f"  {lang:<8} {fmt(metrics(lrows))}")
        print(f"  source: {dict(metrics(vrows)['src'])}")
        print()

    if "lexical" in variants:
        lex = {(r["case"], r["lang"]): r for r in rows if r["variant"] == "lexical"}
        base = metrics([r for r in rows if r["variant"] == "lexical"])
        print("=== against the lexical baseline ===")
        print(f"  lexical baseline: {fmt(base)}")
        for variant in variants:
            if variant == "lexical":
                continue
            vrows = [r for r in rows if r["variant"] == variant]
            m = metrics(vrows)
            gained = lost = 0
            for r in vrows:
                b = lex.get((r["case"], r["lang"]))
                if b is None:
                    continue
                b_hit = b["pos"] is not None and b["pos"] < 5
                v_hit = r["pos"] is not None and r["pos"] < 5
                if v_hit and not b_hit:
                    gained += 1
                if b_hit and not v_hit:
                    lost += 1
            print(f"  {variant:<10} {fmt(m)}")
            print(f"             R@5 delta = {m['R@5'] - base['R@5']:+.3f}   "
                  f"top5 gained={gained}  lost={lost}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
