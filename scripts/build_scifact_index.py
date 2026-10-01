#!/usr/bin/env python3
"""Build a retrieval index over the full BEIR SciFact corpus.

The 60-fact benchmark stores only the answers, so retrieval is nearly trivially
easy. This stores all 5,183 abstracts, so the gold documents are a small
minority of what the ranking has to survive -- which is the point of running a
public corpus at all.

`max_depth` is set to 3, so the embedded set is the corpus down to the documents
themselves rather than every sentence inside them. Embedding every D4 sentence
would be ~60,000 vectors and a different system, not a faster one.

That fixes the vector count at 6,230: the 1,047 blocks at D0-D2 (1 + 9 + 1,037)
plus the 5,183 abstracts at D3. FAISS is given those same 5,183 abstracts, so
the document vectors are like for like; the extra 1,047 are the summaries above
them, which can only help Microscope, and that is worth stating before a table
that Microscope wins.

Two storage limits shaped the index in earlier revisions. Both have since been
removed, and the numbers they distorted are recorded here because they are the
ones the R@k figures describe:

  * Each layer line is one entry, and an entry longer than BLOCK_DATA_SIZE is
    split on sentence boundaries rather than truncated, so no bytes are lost.
    BLOCK_DATA_SIZE is now 16 KiB. At the old 1,024-byte limit the reader cut
    4,300 of the 5,183 abstracts, and the clipped tail was neither retrievable,
    nor embedded, nor present in the text the evaluation scores.
  * Consecutive abstracts are never merged into a shared block. Earlier
    revisions packed short lines together; at 16 KiB that turned these same
    5,183 abstracts into 504 blocks of roughly ten documents each, which would
    have measured retrieval over merged documents.

The index therefore stores the corpus whole, and the D3 level holds exactly one
block per abstract: 5,183 abstracts in 5,183 D3 blocks. The index as a whole is
larger -- 5,633,165 blocks, because every depth level is materialised -- and it
carries 6,230 embedded vectors. Every one of the 286 evaluation tokens is
present in the stored blocks. The earlier divergence between stored vectors and
reference MiniLM encodings on long blocks was an artefact of the measuring
script, not a defect: `scripts/verify_stored_embeddings.py` scores all 13,640
embedded blocks of the evaluation index at cosine 1.0000 once the reference is
built by hand instead of via `SentenceTransformer.encode`.

Usage:  python scripts/build_scifact_index.py [--force] [--limit N]
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
from scifact_set import DEFAULT_DATA, ensure_data  # noqa: E402

LAYERS = ROOT / "scifact_layers"
OUT = ROOT / "scifact_output"
TMP = ROOT / "scifact_tmp"
CONFIG = ROOT / "scifact_config.toml"

_candidates = [ROOT / "target/release/microscope-mem", ROOT / "target/release/microscope-mem.exe"]
BIN = next((c for c in _candidates if c.exists()), _candidates[-1])


def corpus_texts(data_dir: Path, limit: int | None) -> list[str]:
    root = ensure_data(data_dir)
    out: list[str] = []
    for line in (root / "corpus.jsonl").open(encoding="utf-8"):
        d = json.loads(line)
        title = (d.get("title") or "").strip()
        body = (d.get("text") or "").strip()
        text = f"{title} {body}".strip()
        if text:
            out.append(text)
        if limit and len(out) >= limit:
            break
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default=str(DEFAULT_DATA))
    ap.add_argument("--limit", type=int, default=None, help="documents, for a smoke run")
    ap.add_argument("--force", action="store_true", help="rebuild even if layers are unchanged")
    args = ap.parse_args()

    if not BIN.exists():
        print(f"error: {BIN} not found; run: cargo build --release --features embeddings", file=sys.stderr)
        return 1

    docs = corpus_texts(Path(args.data), args.limit)
    LAYERS.mkdir(exist_ok=True)
    OUT.mkdir(exist_ok=True)

    # One document per line, the shape the 60-fact benchmark uses. The line
    # structure matters: a layer file with no blank lines is chunked on line
    # boundaries by the parser, so one abstract stays one block instead of being
    # cut at an arbitrary byte.
    (LAYERS / "long_term.txt").write_text("\n".join(docs) + "\n", encoding="utf-8")

    template = ROOT / "config.example.toml"
    cfg = template.read_text(encoding="utf-8", errors="replace")
    cfg = re.sub(r'^layers_dir\s*=.*$', 'layers_dir = "./scifact_layers"', cfg, flags=re.M)
    cfg = re.sub(r'^output_dir\s*=.*$', 'output_dir = "./scifact_output"', cfg, flags=re.M)
    cfg = re.sub(r'^temp_dir\s*=.*$', 'temp_dir = "./scifact_tmp"', cfg, flags=re.M)
    cfg = re.sub(
        r'(\[memory_layers\]\s*\nlayers\s*=\s*\[)[^\]]*(\])',
        r'\1"long_term"\2',
        cfg,
        flags=re.S,
    )
    # Real embeddings, semantic path on, and the measured defaults: floor 20 and
    # a ranking gain of 2.0 (see WHITEPAPER 11.9).
    cfg = re.sub(r'(?m)^(\s*)provider\s*=\s*"[^"]*"', r'\1provider = "candle"', cfg)
    cfg = re.sub(r'(?m)^(\s*)model\s*=\s*"[^"]*"', r'\1model = "sentence-transformers/all-MiniLM-L6-v2"', cfg)
    cfg = re.sub(r'(?m)^(\s*)semantic_weight\s*=.*$', r'\1semantic_weight = 1.0', cfg)
    if re.search(r'(?m)^\s*semantic_rank_gain\s*=', cfg):
        cfg = re.sub(r'(?m)^(\s*)semantic_rank_gain\s*=.*$', r'\1semantic_rank_gain = 2.0', cfg)
    # One vector per document, not per sentence: see the module docstring.
    cfg = re.sub(r'(?m)^(\s*)max_depth\s*=.*$', r'\1max_depth = 3', cfg)
    CONFIG.write_text(cfg, encoding="utf-8")

    print(f"wrote {len(docs)} documents to {LAYERS}")
    print(f"config: {CONFIG}")
    env = dict(os.environ, MICROSCOPE_CONFIG=str(CONFIG))
    cmd = [str(BIN), "build"] + (["--force"] if args.force else [])
    return subprocess.run(cmd, env=env).returncode


if __name__ == "__main__":
    raise SystemExit(main())