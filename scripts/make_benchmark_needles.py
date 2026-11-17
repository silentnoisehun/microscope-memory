#!/usr/bin/env python3
"""Derive verified ground-truth needles from a live instance.

The exact stored text of a statement differs from a fixture copy in
whitespace and line breaks: the hierarchy keeps one statement inside a larger
session text at one depth and on its own at another.  A needle that is taken
from a newline-joined copy therefore does not occur in the stored text, and
the CLI diagnostic reports MISS for a memory that is actually ranked.

A needle is a single-line substring of the stored block text that occurs only
in blocks carrying the same memory.  The engine's own de-duplication
collapses identical texts, so a needle may match several blocks -- but never
one that belongs to a different memory.

Usage:
    python scripts/make_benchmark_needles.py \
        --config "$HOME/.codex/microscope-memory/config.toml" \
        --fixture tests/fixtures/relevance/real_corpus_cases.json \
        --out fixtures_with_needles.json
"""
from __future__ import annotations

import argparse
import json
import re
import struct
from collections import Counter
from pathlib import Path

TOKEN = re.compile(r"[\wÀ-ɏ]+", re.UNICODE)

# BlockHeader v4 (MSC4): 32-byte spatial header + project_id(16) +
# importance + flags.
BLOCK_HEADER = 50


def output_dir_of(config: Path) -> Path:
    text = config.read_text(encoding="utf-8", errors="replace")
    m = re.search(r'^\s*output_dir\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        raise SystemExit(f"{config}: no output_dir")
    out = Path(m.group(1))
    return out if out.is_absolute() else config.parent / out


def read_blocks(out_dir: Path):
    headers = (out_dir / "microscope.bin").read_bytes()
    data = (out_dir / "data.bin").read_bytes()
    n = len(headers) // BLOCK_HEADER
    blocks = []
    for b in range(n):
        o = b * BLOCK_HEADER
        off = struct.unpack_from("<I", headers, o + 18)[0]
        ln = struct.unpack_from("<H", headers, o + 22)[0]
        end = min(off + ln, len(data))
        if off >= end:
            continue
        try:
            blocks.append(data[off:end].decode("utf-8"))
        except UnicodeDecodeError:
            continue
    return blocks


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--fixture", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    config = Path(a.config).resolve()
    out_dir = output_dir_of(config)
    blocks = read_blocks(out_dir)
    print(f"instance: {out_dir} ({len(blocks)} blocks)")

    token_blocks: Counter[str] = Counter()
    block_tokens: list[set[str]] = []
    for txt in blocks:
        toks = set(t.lower() for t in TOKEN.findall(txt))
        block_tokens.append(toks)
        for t in toks:
            token_blocks[t] += 1

    fixture = json.loads(Path(a.fixture).read_text(encoding="utf-8"))
    cases = fixture["cases"] if isinstance(fixture, dict) else fixture

    out_cases = []
    problems = []
    for case in cases:
        want = case["expected"]
        want_tokens = [t.lower() for t in TOKEN.findall(want)]
        rare = sorted(set(want_tokens), key=lambda t: token_blocks[t])[:6]
        cands = [i for i, toks in enumerate(block_tokens) if all(t in toks for t in rare)]
        if not cands:
            problems.append((case["id"], "no block carries the expected tokens"))
            continue
        cands.sort(key=lambda i: -len(set(want_tokens) & block_tokens[i]))
        stored = blocks[cands[0]]

        lines = []
        for line in stored.split("\n"):
            line = line.strip()
            if len(line) < 20:
                continue
            if len(set(t.lower() for t in TOKEN.findall(line)) & set(want_tokens)) < 2:
                continue
            lines.append(line)
        expanded = []
        for line in lines:
            expanded.append(line)
            words = line.split()
            for size in (10, 7, 5):
                if len(words) > size:
                    expanded.append(" ".join(words[:size]))

        chosen = None
        for cand in expanded:
            needle = cand if len(cand) <= 34 else cand[:34].rsplit(" ", 1)[0]
            if len(needle) < 12:
                continue
            hits = [i for i, txt in enumerate(blocks) if needle in txt]
            if hits and all(len(set(want_tokens) & block_tokens[i]) >= 2 for i in hits):
                chosen = needle
                break
        if chosen is None:
            exact = [i for i, txt in enumerate(blocks) if want in txt]
            if exact:
                chosen = want if len(want) <= 34 else want[:34]
        if chosen is None:
            problems.append((case["id"], "no unambiguous needle"))
            continue
        out_cases.append({**case, "needle": chosen,
                          "needle_blocks": sum(1 for txt in blocks if chosen in txt)})

    if problems:
        for pid, why in problems:
            print(f"  PROBLEM {pid}: {why}")
        return 1

    Path(a.out).write_text(json.dumps(out_cases, ensure_ascii=False, indent=2) + "\n",
                            encoding="utf-8")
    print(f"{len(out_cases)} cases with verified needles -> {a.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
