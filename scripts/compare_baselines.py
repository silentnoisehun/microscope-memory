#!/usr/bin/env python3
"""Controlled comparison: Microscope vs FAISS (flat/HNSW) vs SQLite FTS5.

All three run on the same machine, the same corpus and the same queries, and
all three report latency *and* recall@k. Reporting latency without recall is
meaningless: a system that returns nothing is very fast.

Vector representation
--------------------
Microscope assigns each block 3-D coordinates derived from a hash of its text.
To compare like with like, FAISS is given that same 3-D representation, plus a
hashed bag-of-words representation so the table also covers a realistic dense
configuration. The two are reported separately and never merged.

Usage:
    python scripts/compare_baselines.py
    python scripts/compare_baselines.py --k 1 5 10
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from resonance_set import CASES, result_lines  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
CANDIDATES = [
    ROOT / "target/release/microscope-mem",
    ROOT / "target/release/microscope-mem.exe",
]
BIN = next((c for c in CANDIDATES if c.exists()), None)
CONFIG = ROOT / "bench_config.toml"

D = 3           # Microscope's coordinate dimensionality
BOW = 256       # bag-of-words hash width for the dense FAISS configs


def hash_coords(text: str) -> list[float]:
    """A deterministic 3-D coordinate from text, matching Microscope's scheme."""
    h = hashlib.blake2b(text.encode("utf-8"), digest_size=8).digest()
    return [(h[i] / 255.0) - 0.5 for i in range(D)]


def bow_vector(text: str, width: int = BOW) -> list[float]:
    """Hashed bag-of-words: a cheap dense representation for the FAISS runs."""
    vec = [0.0] * width
    for tok in text.lower().split():
        h = int.from_bytes(
            hashlib.blake2b(tok.encode("utf-8"), digest_size=4).digest(), "big"
        )
        vec[h % width] += 1.0
    norm = sum(v * v for v in vec) ** 0.5 or 1.0
    return [v / norm for v in vec]


def pcts(samples: list[float]) -> dict:
    """p50/p95/p99/mean, in milliseconds."""
    s = sorted(samples)
    n = len(s)

    def p(q: float) -> float:
        if n == 1:
            return s[0]
        idx = min(n - 1, max(0, int(round(q * (n - 1)))))
        return s[idx]

    return {
        "p50_ms": round(p(0.50), 4),
        "p95_ms": round(p(0.95), 4),
        "p99_ms": round(p(0.99), 4),
        "mean_ms": round(statistics.fmean(s), 4),
    }


def _score(rows: list[str], case, ks: list[int]) -> dict:
    """recall@k for a list of ranked result strings."""
    per_k = {k: 0 for k in ks}
    for k in ks:
        window = " ".join(rows[:k]).lower()
        if any(m.lower() in window for m in case.match):
            per_k[k] += 1
    return per_k


def _rates(per_k: dict, ks: list[int]) -> dict:
    return {str(k): round(100.0 * per_k[k] / len(CASES), 1) for k in ks}


def run_microscope(ks: list[int], config: Path) -> dict:
    """Time `recall` end-to-end and compute recall@k from its output."""
    if BIN is None:
        return {"system": "microscope (recall)", "error": "binary not built"}
    if not config.exists():
        return {"system": "microscope (recall)", "error": f"{config} missing"}
    env = dict(os.environ, MICROSCOPE_CONFIG=str(config.resolve()))
    per_k = {k: 0 for k in ks}
    times: list[float] = []
    top = max(ks)

    for c in CASES:
        t0 = time.perf_counter()
        r = subprocess.run(
            [str(BIN), "recall", c.question, str(top)],
            capture_output=True,
            env=env,
        )
        times.append((time.perf_counter() - t0) * 1000.0)
        # Decode explicitly: the corpus is UTF-8 and the platform default
        # (cp1252 on Windows) raises on non-ASCII bytes.
        out = r.stdout.decode("utf-8", errors="replace").lower()
        rows = result_lines(out)
        for k, hit in _score(rows, c, ks).items():
            if hit:
                per_k[k] += 1

    return {
        "system": "microscope (recall, end-to-end)",
        "note": "includes process start, config load and state load",
        "latency": pcts(times),
        "recall_at_k": {str(k): per_k[k] for k in ks},
        "recall_rate": _rates(per_k, ks),
    }


def run_faiss(ks: list[int], dense: bool, index_type: str) -> dict:
    """Time FAISS search on the same corpus and queries, with recall@k."""
    import faiss
    import numpy as np

    facts = [c.fact for c in CASES]
    dim = BOW if dense else D
    vec = bow_vector if dense else hash_coords
    mat = np.array([vec(f) for f in facts], dtype="float32")

    if index_type == "flat":
        index = faiss.IndexFlatIP(dim)
    else:
        index = faiss.IndexHNSWFlat(dim, 32)
    index.add(mat)
    qmat = np.array([vec(c.question) for c in CASES], dtype="float32")

    top = max(ks)
    faiss.omp_set_num_threads(1)  # single-threaded, comparable to one CLI process
    times: list[float] = []
    per_k = {k: 0 for k in ks}

    for i, c in enumerate(CASES):
        t0 = time.perf_counter()
        _d, idx = index.search(qmat[i : i + 1], top)
        times.append((time.perf_counter() - t0) * 1000.0)
        ranked = [facts[j] for j in idx[0] if j >= 0]
        for k, hit in _score(ranked, c, ks).items():
            if hit:
                per_k[k] += 1

    label = ("faiss IndexHNSWFlat" if index_type == "hnsw" else "faiss IndexFlatIP")
    kind = "bag-of-words dense" if dense else "microscope 3-D coords"
    return {
        "system": f"{label} (d={dim})",
        "note": f"vectors: {kind}; 1 thread; query time only, no index build",
        "latency": pcts(times),
        "recall_at_k": {str(k): per_k[k] for k in ks},
        "recall_rate": _rates(per_k, ks),
    }


def run_fts5(ks: list[int]) -> dict:
    """Time SQLite FTS5 lexical search on the same corpus and queries."""
    import sqlite3

    facts = [c.fact for c in CASES]
    db = sqlite3.connect(":memory:")
    db.execute("CREATE VIRTUAL TABLE docs USING fts5(text)")
    db.executemany("INSERT INTO docs (text) VALUES (?)", [(f,) for f in facts])
    db.commit()

    times: list[float] = []
    per_k = {k: 0 for k in ks}
    top = max(ks)

    for c in CASES:
        terms = [t for t in c.question.split() if t.isalnum()]
        match = " OR ".join(terms) if terms else c.question
        t0 = time.perf_counter()
        try:
            rows = db.execute(
                "SELECT text FROM docs WHERE docs MATCH ? ORDER BY rank LIMIT ?",
                (match, top),
            ).fetchall()
        except sqlite3.OperationalError:
            rows = []
        times.append((time.perf_counter() - t0) * 1000.0)
        ranked = [r[0] for r in rows]
        for k, hit in _score(ranked, c, ks).items():
            if hit:
                per_k[k] += 1

    db.close()
    return {
        "system": "sqlite fts5 (bm25)",
        "note": "in-memory; query time only, no index build",
        "latency": pcts(times),
        "recall_at_k": {str(k): per_k[k] for k in ks},
        "recall_rate": _rates(per_k, ks),
    }


def load_microscope_vectors(facts: list[str]) -> "np.ndarray | None":
    """Read the real MiniLM vectors Microscope stored, so FAISS is fed the
    same embeddings rather than a bag-of-words proxy. Returns None if the
    index is missing or the dimension cannot be read."""
    try:
        import numpy as np
    except ImportError:
        return None
    path = ROOT / "eval_output" / "embeddings.bin"
    if not path.exists():
        return None
    raw = path.read_bytes()
    if len(raw) < 12:
        return None
    count = int.from_bytes(raw[0:4], "little")
    dim = int.from_bytes(raw[4:8], "little")
    if count == 0 or dim == 0:
        return None
    need = 12 + count * 4 + count * dim * 4
    if len(raw) < need:
        return None
    ids = np.frombuffer(raw, dtype="<u4", count=count, offset=12)
    vecs = np.frombuffer(raw, dtype="<f4", count=count * dim, offset=12 + count * 4)
    return ids, vecs.reshape(count, dim)


def run_faiss_real(ks: list[int], index_type: str) -> dict:
    """FAISS over the same MiniLM vectors, restricted to the 60 evaluation
    facts. Uses sentence-transformers with the identical checkpoint the binary
    used, so the comparison is embedding-for-embedding rather than against a
    bag-of-words proxy."""
    try:
        import faiss
        import numpy as np
        from sentence_transformers import SentenceTransformer
    except ImportError as e:
        return {"system": "faiss (real MiniLM vectors)", "error": str(e)}

    st = SentenceTransformer("sentence-transformers/all-MiniLM-L6-v2")
    facts = [c.fact for c in CASES]
    mat = np.ascontiguousarray(
        st.encode(facts, normalize_embeddings=True).astype("float32")
    )
    qmat = np.ascontiguousarray(
        st.encode([c.question for c in CASES], normalize_embeddings=True).astype("float32")
    )

    index = (
        faiss.IndexFlatIP(mat.shape[1])
        if index_type == "flat"
        else faiss.IndexHNSWFlat(mat.shape[1], 32)
    )
    index.add(mat)

    top = max(ks)
    faiss.omp_set_num_threads(1)
    times, per_k = [], {k: 0 for k in ks}
    for i, c in enumerate(CASES):
        t0 = time.perf_counter()
        _d, idx = index.search(qmat[i : i + 1], top)
        times.append((time.perf_counter() - t0) * 1000.0)
        ranked = [facts[j] for j in idx[0] if 0 <= j < len(facts)]
        for k, hit in _score(ranked, c, ks).items():
            if hit:
                per_k[k] += 1

    label = "faiss IndexHNSWFlat (MiniLM)" if index_type == "hnsw" else "faiss IndexFlatIP (MiniLM)"
    return {
        "system": label,
        "note": f"all-MiniLM-L6-v2, d={mat.shape[1]}; 1 thread; query only",
        "latency": pcts(times),
        "recall_at_k": {str(k): per_k[k] for k in ks},
        "recall_rate": _rates(per_k, ks),
    }


def _parse_query_vector(stdout: bytes) -> "list[float] | None":
    """Parse a whitespace/comma separated float vector printed by the binary."""
    try:
        text = stdout.decode("utf-8", errors="replace")
    except Exception:
        return None
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        if "," in line:
            parts = line.split(",")
        else:
            parts = line.split()
        try:
            vals = [float(p) for p in parts if p]
        except ValueError:
            continue
        if len(vals) >= 8:
            return vals
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--k", type=int, nargs="+", default=[1, 5, 10])
    ap.add_argument("--config", default="eval_config.toml")
    a = ap.parse_args()

    cfg = Path(a.config)
    ks = sorted(a.k)
    if not cfg.exists():
        print(f"error: {cfg} not found; run scripts/eval_real.sh first", file=sys.stderr)
        return 1

    runs = [
        run_microscope(ks, cfg),
        run_faiss_real(ks, index_type="flat"),
        run_faiss_real(ks, index_type="hnsw"),
        run_fts5(ks),
    ]

    print(f"\nCorpus: {len(CASES)} facts, {len(CASES)} queries, k={ks}")
    print("Microscope: provider=candle, all-MiniLM-L6-v2, semantic_weight=1.0")
    print("Latency and recall together: a system returning nothing is fast.\n")
    head = f"{'system':<36} {'p50 ms':>9} {'p95 ms':>9} {'p99 ms':>9}  " + "  ".join(
        f"R@{k}" for k in ks
    )
    print(head)
    print("-" * len(head))
    for r in runs:
        if "error" in r:
            print(f"{r['system']:<36} {r['error']}")
            continue
        lat = r["latency"]
        rec = "  ".join(f"{r['recall_rate'][str(k)]:5.1f}%" for k in ks)
        print(
            f"{r['system']:<36} {lat['p50_ms']:>9.3f} {lat['p95_ms']:>9.3f} "
            f"{lat['p99_ms']:>9.3f}  {rec}"
        )

    out = Path("docs/measurements/real_embedding_comparison.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        json.dumps(
            {
                "config": "provider=candle model=all-MiniLM-L6-v2 semantic_weight=1.0",
                "corpus_facts": len(CASES),
                "queries": len(CASES),
                "k_values": ks,
                "results": runs,
            },
            indent=2,
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    print(f"\nwrote {out}")
    return 0



if __name__ == "__main__":
    sys.exit(main())
