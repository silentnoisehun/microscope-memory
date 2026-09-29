#!/usr/bin/env python3
"""BEIR SciFact, loaded into the same Case shape as resonance_set.

Why this exists: every number this project has published came from a set of 60
hand-written facts, and a reader is right to discount them. SciFact is a
public claim-verification corpus -- 5,183 abstracts, 300 test queries, gold
documents from `qrels` -- so a number measured on it is one anybody can rerun
and disagree with.

The protocol is the same one resonance_set uses:

  fact     the gold document's title and text, i.e. what gets stored
  question the query text
  match    a phrase from that document, matched case-insensitively

The one thing that differs is how `match` is chosen. The harness scores a hit by
substring, so a token occurring in several documents would credit a retrieval
that found the wrong one. Every match token here is checked to occur in exactly
one document before it is used, and the loader reports how many cases it had to
skip rather than quietly loosening the check.

Usage:
    python scripts/scifact_set.py [--data DIR] [--limit N] [--dump-cases]
"""
from __future__ import annotations

import argparse
import json
import sys
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path

# The canonical BEIR distribution. Fetched once and cached, so a rerun of the
# benchmark is offline and the data cannot drift under the numbers.
URL = "https://public.ukp.informatik.tu-darmstadt.de/thakur/BEIR/datasets/scifact.zip"
DEFAULT_DATA = Path.home() / ".cache" / "beir" / "scifact"


@dataclass
class Case:
    """One document and the query that must retrieve it."""

    id: int
    fact: str
    question: str
    match: list[str]
    category: str


def ensure_data(data_dir: Path) -> Path:
    """Return the extracted SciFact directory, downloading it if absent."""
    data_dir = data_dir.expanduser()
    # The archive contains a top-level `scifact/` directory.
    for candidate in (data_dir, data_dir / "scifact", data_dir.parent / "scifact"):
        if (candidate / "corpus.jsonl").exists():
            return candidate
    data_dir.mkdir(parents=True, exist_ok=True)
    archive = data_dir / "scifact.zip"
    if not archive.exists():
        print(f"downloading SciFact from {URL} ...", file=sys.stderr)
        urllib.request.urlretrieve(URL, archive)
    with zipfile.ZipFile(archive) as z:
        z.extractall(data_dir)
    for candidate in (data_dir / "scifact", data_dir):
        if (candidate / "corpus.jsonl").exists():
            return candidate
    raise SystemExit("SciFact archive extracted but corpus.jsonl not found")


def _words(text: str) -> list[str]:
    return [w.strip(".,;:()[]\"'").lower() for w in text.split()]


def _unique_phrase(corpus_texts: list[str], text: str, max_words: int = 12) -> str | None:
    """The shortest word n-gram of `text` occurring in exactly one document.

    Starts at the title, because in SciFact the title is the most distinctive
    part of the abstract, and grows the window until the phrase is unique. If no
    window is unique, returns None and the caller skips the case: falling back
    to a common word would turn a false positive into a scored hit.
    """
    words = _words(text)
    for n in range(3, max_words + 1):
        if len(words) < n:
            break
        phrase = " ".join(words[:n])
        hits = sum(1 for t in corpus_texts if phrase in t)
        if hits == 1:
            return phrase
        if hits == 0:
            return None  # the stored text is not in this corpus
    return None


def load(data_dir: Path = DEFAULT_DATA, limit: int | None = None) -> list[Case]:
    root = ensure_data(data_dir)

    corpus: dict[str, str] = {}
    for line in (root / "corpus.jsonl").open(encoding="utf-8"):
        d = json.loads(line)
        title = (d.get("title") or "").strip()
        body = (d.get("text") or "").strip()
        corpus[d["_id"]] = f"{title} {body}".strip()
    # The uniqueness check has to run on the same footing as the scorer, which
    # lowercases both sides. Comparing a lowercased phrase against the raw text
    # finds nothing, and the first version of this loader therefore reported 42
    # usable cases out of 300 and attributed the rest to non-unique titles.
    corpus_texts = [t.lower() for t in corpus.values()]

    queries: dict[str, str] = {}
    for line in (root / "queries.jsonl").open(encoding="utf-8"):
        d = json.loads(line)
        queries[d["_id"]] = d["text"].strip()

    # qrels/test.tsv: query-id, corpus-id, score. SciFact's test split is
    # single-claim, but the file can list more than one gold doc per query, so
    # the highest-scoring gold doc is the one used.
    qrels = (root / "qrels" / "test.tsv").open(encoding="utf-8").read().splitlines()
    header = qrels[0].split("\t")
    qi, ci = header.index("query-id"), header.index("corpus-id")
    per_query: dict[str, list[tuple[int, str]]] = {}
    for line in qrels[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        per_query.setdefault(parts[qi], []).append((int(parts[2]), parts[ci]))

    cases: list[Case] = []
    no_query = no_gold = no_unique = 0
    for qid, golds in sorted(per_query.items(), key=lambda kv: int(kv[0])):
        question = queries.get(qid)
        if not question:
            no_query += 1
            continue
        best = max(golds, key=lambda g: g[0])
        doc = corpus.get(best[1])
        if not doc:
            no_gold += 1
            continue
        phrase = _unique_phrase(corpus_texts, doc)
        if not phrase:
            no_unique += 1
            continue
        cases.append(Case(len(cases), doc, question, [phrase], "scifact"))
        if limit and len(cases) >= limit:
            break

    print(
        f"SciFact: {len(cases)} cases from {len(corpus)} documents "
        f"(skipped: {no_query} no query, {no_gold} no gold, {no_unique} no unique match)",
        file=sys.stderr,
    )
    return cases


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default=str(DEFAULT_DATA))
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--dump-cases", action="store_true")
    args = ap.parse_args()
    cases = load(Path(args.data), args.limit)
    if args.dump_cases:
        for c in cases[:5]:
            print(f"--- id={c.id}")
            print(f"  question: {c.question[:100]}")
            print(f"  match   : {c.match[0]}")
            print(f"  fact    : {c.fact[:140]}...")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
