#!/usr/bin/env python3
"""Scale comparison on the repository's real corpus (~695k blocks).

The 60-fact resonance set is far too small to compare against a vector store: a
60-document index is trivial for an exhaustive scan. This runs the same
latency-and-recall comparison against the index built from layers/, so the
numbers reflect the actual corpus.
"""

from __future__ import annotations

import json
import os
import re
import statistics
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = next(
    (p for p in [
        ROOT / "target/release/microscope-mem",
        ROOT / "target/release/microscope-mem.exe",
    ] if p.exists()),
    None,
)
REAL_CONFIG = ROOT / "real_config.toml"
OUT = Path("docs/measurements/scale_comparison.json")

# Queries from the real corpus vocabulary, including paraphrases that share no
# literal wording with the stored text.
QUERIES = [
    ("pine nut allergy", ["allergy"]),
    ("diĂłallergia", ["allergy"]),
    ("where does the user live", ["szeged", "budapest", "lives"]),
    ("project name", ["microscope", "hope"]),
    ("what is the dogs name", ["morzska", "dog"]),
    ("favourite programming language", ["rust", "python"]),
    ("commute", ["bicycle", "bike", "commut"]),
    ("coffee", ["coffee"]),
    ("running habit", ["run", "morning"]),
    ("birthday month", ["birthday", "born", "birth"]),
]


def load_corpus() -> list[str]:
    lines: list[str] = []
    for p in sorted((ROOT / "layers").glob("*.txt")):
        try:
            for ln in p.read_text(encoding="utf-8", errors="replace").splitlines():
                ln = ln.strip()
                if ln:
                    lines.append(ln)
        except OSError:
            pass
    return lines


def result_rows(stdout: str) -> list[str]:
    rows: list[str] = []
    for ln in stdout.splitlines():
        s = ln.rstrip()
        st = s.lstrip()
        if st[:1].lower() == "d" and st[1:2].isdigit():
            rows.append(st)
        elif rows and s.strip():
            rows[-1] += " " + s.strip()
    return rows


def pcts(s: list[float]) -> dict:
    s = sorted(s)
    n = len(s)

    def p(q: float) -> float:
        if n == 1:
            return s[0]
        return s[min(n - 1, max(0, int(round(q * (n - 1)))))]

    return {
        "p50_ms": round(p(0.50), 3),
        "p95_ms": round(p(0.95), 3),
        "p99_ms": round(p(0.99), 3),
        "mean_ms": round(statistics.fmean(s), 3),
    }


def measure_microscope(ks: list[int]) -> dict:
    if BIN is None or not REAL_CONFIG.exists():
        return {"system": "microscope (real corpus)", "error": "no index or binary"}
    env = dict(os.environ, MICROSCOPE_CONFIG=str(REAL_CONFIG.resolve()))
    times, per_k = [], {k: 0 for k in ks}
    top = max(ks)
    for q, needles in QUERIES:
        t0 = time.perf_counter()
        r = subprocess.run([str(BIN), "recall", q, str(top)],
                           capture_output=True, env=env)
        times.append((time.perf_counter() - t0) * 1000.0)
        # The real corpus is UTF-8; decode explicitly rather than letting the
        # platform default (cp1252 on Windows) drop non-ASCII bytes.
        out = r.stdout.decode("utf-8", errors="replace").lower()
        rows = result_rows(out)
        for k in ks:
            if any(nd in " ".join(rows[:k]).lower() for nd in needles):
                per_k[k] += 1
    return {
        "system": "microscope (recall, end-to-end)",
        "latency": pcts(times),
        "hit": {str(k): per_k[k] for k in ks},
    }


def measure_fts5(corpus: list[str], ks: list[int]) -> dict:
    t0 = time.perf_counter()
    db = sqlite3.connect(":memory:")
    db.execute("CREATE VIRTUAL TABLE docs USING fts5(text)")
    db.executemany("INSERT INTO docs (text) VALUES (?)", [(c,) for c in corpus])
    db.commit()
    build_ms = (time.perf_counter() - t0) * 1000.0

    times, per_k = [], {k: 0 for k in ks}
    top = max(ks)
    for q, needles in QUERIES:
        terms = [t for t in re.findall(r"\w+", q) if len(t) > 1]
        match = " OR ".join(terms) if terms else q
        t0 = time.perf_counter()
        try:
            rows = db.execute(
                "SELECT text FROM docs WHERE docs MATCH ? ORDER BY rank LIMIT ?",
                (match, top),
            ).fetchall()
        except sqlite3.OperationalError:
            rows = []
        times.append((time.perf_counter() - t0) * 1000.0)
        got = [r[0] for r in rows]
        for k in ks:
            if any(nd in " ".join(got[:k]).lower() for nd in needles):
                per_k[k] += 1
    db.close()
    return {
        "system": "sqlite fts5 (bm25)",
        "index_build_ms": round(build_ms, 1),
        "documents": len(corpus),
        "latency": pcts(times),
        "hit": {str(k): per_k[k] for k in ks},
    }


def main() -> int:
    corpus = load_corpus()
    ks = [1, 5, 10]
    print(f"Real corpus: {len(corpus)} source lines from layers/")

    m = measure_microscope(ks)
    f = measure_fts5(corpus, ks)

    print(f"\n{'system':<34} {'p50 ms':>10} {'p95 ms':>10} {'p99 ms':>10}   hits")
    print("-" * 84)
    for r in (m, f):
        if "error" in r:
            print(f"{r['system']:<34} {r['error']}")
            continue
        lat = r["latency"]
        hits = "  ".join(f"H@{k}={r['hit'][str(k)]}/{len(QUERIES)}" for k in ks)
        print(f"{r['system']:<34} {lat['p50_ms']:>10.3f} {lat['p95_ms']:>10.3f} "
              f"{lat['p99_ms']:>10.3f}   {hits}")

    if "index_build_ms" in f:
        print(f"\nFTS5 index build: {f['index_build_ms']:.0f} ms for {f['documents']} docs")
    print("\nMicroscope = end-to-end process (start-up + state load included).")
    print("FTS5 = query only; its index build cost is listed separately above.")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps({
        "corpus_lines": len(corpus),
        "queries": len(QUERIES),
        "k_values": ks,
        "microscope": m,
        "fts5": f,
    }, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\nwrote {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

