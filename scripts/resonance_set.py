#!/usr/bin/env python3
"""Resonance test set: 60 personal facts with known questions.

Measures hit@k: how many of the 60 known facts appear in the top k results of
`microscope-mem find`. The first two entries are the ones that previously failed
(pine nut allergy, short check-ins) because shallow noise fragments outranked
the exact fact.

Usage:
    python scripts/resonance_set.py --check
    python scripts/resonance_set.py --measure --k 5 10 20
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass, asdict
from pathlib import Path


@dataclass
class Case:
    """One personal fact and the question that should retrieve it."""

    id: int
    fact: str          # text to store
    question: str      # query that must retrieve it
    match: list[str]   # any one substring counts as a correct hit
    category: str


# The first two are known regressions: stored correctly, but previously not
# retrieved in the top 5 because shallow noise outranked the exact fact.
CASES: list[Case] = [
    Case(1, "The user has a pine nut allergy.", "nut allergy", ["pine nut allergy", "nut allergy"], "health"),
    Case(2, "The user prefers short, kind check-ins rather than long ones.", "how does the user like check-ins", ["short", "check-in"], "preference"),
    Case(3, "The user lives in Szeged.", "where does the user live", ["szeged"], "location"),
    Case(4, "The user's dog is called Morzska.", "what is the dog's name", ["morzska"], "pet"),
    Case(5, "The user works as a software engineer.", "what does the user do for work", ["software engineer", "engineer"], "work"),
    Case(6, "The user's favourite language is Rust.", "favourite programming language", ["rust"], "preference"),
    Case(7, "The user is allergic to penicillin.", "penicillin allergy", ["penicillin"], "health"),
    Case(8, "The user has two children.", "how many children", ["two children"], "family"),
    Case(9, "The user's partner is called Anna.", "partner's name", ["anna"], "family"),
    Case(10, "The user was born in 1985.", "birth year", ["1985"], "personal"),
    Case(11, "The user speaks Hungarian, English and German.", "languages spoken", ["hungarian", "german"], "personal"),
    Case(12, "The user is vegetarian.", "diet", ["vegetarian"], "health"),
    Case(13, "The user plays the piano.", "hobby instrument", ["piano"], "hobby"),
    Case(14, "The user runs every morning.", "running habit", ["every morning", "runs"], "habit"),
    Case(15, "The user's city is Budapest.", "which city", ["budapest"], "location"),
    Case(16, "The user does not drink coffee.", "coffee", ["not drink coffee", "does not drink coffee"], "preference"),
    Case(17, "The user prefers dark mode in every editor.", "editor theme", ["dark mode"], "preference"),
    Case(18, "The user has a cat named Bella.", "cat's name", ["bella"], "pet"),
    Case(19, "The user's project is called Hope Ecosystem.", "project name", ["hope ecosystem", "hope"], "project"),
    Case(20, "The user uses a standing desk.", "desk", ["standing desk"], "workspace"),
    Case(21, "The user is allergic to shellfish.", "shellfish allergy", ["shellfish"], "health"),
    Case(22, "The user's mother is called Klara.", "mother's name", ["klara"], "family"),
    Case(23, "The user reads science fiction.", "reading genre", ["science fiction"], "hobby"),
    Case(24, "The user commutes by bicycle.", "commute", ["bicycle", "commutes by"], "habit"),
    Case(25, "The user's favourite food is goulash.", "favourite food", ["goulash"], "preference"),
    Case(26, "The user has a mathematics degree.", "degree", ["mathematics"], "education"),
    Case(27, "The user does not smoke.", "smoking", ["not smoke", "does not smoke"], "health"),
    Case(28, "The user owns a red car.", "car colour", ["red car"], "property"),
    Case(29, "The user is learning Spanish.", "language being learned", ["spanish"], "education"),
    Case(30, "The user's team is called Platform.", "team name", ["platform"], "work"),
    Case(31, "The user takes their coffee black.", "coffee order", ["black"], "preference"),
    Case(32, "The user has a shoulder injury on the left side.", "injury", ["shoulder", "left"], "health"),
    Case(33, "The user supports the Hungarian national team.", "sports team", ["hungarian national"], "hobby"),
    Case(34, "The user works remotely three days a week.", "remote work", ["remote", "three days"], "work"),
    Case(35, "The user's sister is called Emese.", "sister's name", ["emese"], "family"),
    Case(36, "The user keeps a journal.", "journal", ["journal"], "habit"),
    Case(37, "The user is allergic to latex.", "latex allergy", ["latex"], "health"),
    Case(38, "The user plays chess online.", "chess", ["chess"], "hobby"),
    Case(39, "The user's birthday is in March.", "birth month", ["march"], "personal"),
    Case(40, "The user prefers meetings in the morning.", "meeting time", ["morning"], "preference"),
    Case(41, "The user has a garden.", "garden", ["garden"], "property"),
    Case(42, "The user speaks basic Japanese.", "japanese", ["japanese"], "personal"),
    Case(43, "The user is a morning person.", "chronotype", ["morning person"], "personal"),
    Case(44, "The user's car is a Volvo.", "car model", ["volvo"], "property"),
    Case(45, "The user avoids artificial sweeteners.", "sweeteners", ["sweetener"], "preference"),
    Case(46, "The user has studied architecture.", "past study", ["architecture"], "education"),
    Case(47, "The user runs the project called Microscope.", "which project", ["microscope"], "project"),
    Case(48, "The user's favourite season is autumn.", "favourite season", ["autumn"], "preference"),
    Case(49, "The user does not eat mushrooms.", "mushrooms", ["not eat mushroom", "does not eat mushroom"], "preference"),
    Case(50, "The user collects vinyl records.", "vinyl", ["vinyl"], "hobby"),
    Case(51, "The user has a brother called Gabor.", "brother's name", ["gabor"], "family"),
    Case(52, "The user works best in silence.", "work environment", ["silence", "works best in"], "preference"),
    Case(53, "The user has a passport expiring next year.", "passport", ["passport"], "personal"),
    Case(54, "The user likes spicy food.", "spicy food", ["spicy"], "preference"),
    Case(55, "The user owns a bicycle.", "bicycle", ["bicycle"], "property"),
    Case(56, "The user is learning to play the guitar.", "guitar", ["guitar"], "hobby"),
    Case(57, "The user's email address ends with .hu.", "email domain", [".hu"], "personal"),
    Case(58, "The user does not have a driver's licence.", "driving licence", ["not have a driver", "does not have a driver"], "personal"),
    Case(59, "The user prefers asynchronous communication.", "communication style", ["asynchronous", "async"], "work"),
    Case(60, "The user has a subscription to a science magazine.", "magazine", ["magazine", "subscription"], "hobby"),
]


def validate() -> int:
    """Sanity-check the set: unique ids, non-empty fields, unique questions."""
    problems = []
    ids = [c.id for c in CASES]
    if len(set(ids)) != len(ids):
        problems.append("duplicate ids")
    questions = [c.question.lower() for c in CASES]
    if len(set(questions)) != len(questions):
        problems.append("duplicate questions")
    for c in CASES:
        if not c.fact.strip() or not c.question.strip() or not c.match:
            problems.append(f"case {c.id} has an empty field")
        # The fact must actually contain what we look for, otherwise the case
        # can never pass and would silently deflate the score.
        fact_lower = c.fact.lower()
        for m in c.match:
            if m.lower() not in fact_lower:
                problems.append(f"case {c.id}: match {m!r} not in fact")
    if problems:
        print("FAIL:")
        for p in problems:
            print(f"  - {p}")
        return 1
    cats: dict[str, int] = {}
    for c in CASES:
        cats[c.category] = cats.get(c.category, 0) + 1
    print(f"OK: {len(CASES)} cases across {len(cats)} categories")
    for k, v in sorted(cats.items()):
        print(f"  {k}: {v}")
    return 0


def _env_for(config: Path) -> dict:
    """Environment for a child process.

    The config path must be absolute. Inside the config, `layers_dir` and
    `output_dir` are resolved relative to the *current working directory* of
    the binary, so the caller must run from the repository root for the
    relative paths in bench_config.toml to resolve correctly.
    """
    return dict(os.environ, MICROSCOPE_CONFIG=str(Path(config).resolve()))


def run_recall(binary: str, config: Path, question: str, k: int) -> str:
    """Return stdout of a single `recall` invocation, or '' on failure.

    `recall` is the natural-language entry point and is what the resonance set
    is designed to exercise. `find` is a literal substring search and cannot
    answer a paraphrased question.
    """
    env = _env_for(config)
    cmd = [binary, "recall", question, str(k)]
    try:
        out = subprocess.run(
            cmd, capture_output=True, text=True, timeout=120, env=env
        )
    except FileNotFoundError:
        print(f"error: binary not found: {binary}", file=sys.stderr)
        sys.exit(1)
    except subprocess.TimeoutExpired:
        return ""
    return out.stdout


def result_lines(stdout: str) -> list[str]:
    """Extract the result rows.

    A row looks like:
        "  D5 L2=0.96177 [long_term/blue] The user plays chess online."
    but a row can span several lines when a block contains newlines, so rows are
    identified by their depth marker and the text between markers is joined.
    """
    lines = [ln.rstrip() for ln in stdout.splitlines()]
    rows: list[str] = []
    for ln in lines:
        # Compare case-insensitively: the caller lowercases stdout before
        # calling this, so the depth marker arrives as 'd5', not 'D5'.
        if ln.lstrip()[:1].lower() == "d" and ln.lstrip()[1:2].isdigit():
            rows.append(ln.strip())
        elif rows and ln.strip():
            # continuation of the previous multi-line block
            rows[-1] += " " + ln.strip()
    return rows


def run_find(binary: str, config: Path, question: str, k: int) -> str:
    """Return stdout of a single literal `find` invocation, or '' on failure."""
    env = _env_for(config)
    cmd = [binary, "find", question, str(k)]
    try:
        out = subprocess.run(
            cmd, capture_output=True, text=True, timeout=120, env=env
        )
    except FileNotFoundError:
        print(f"error: binary not found: {binary}", file=sys.stderr)
        sys.exit(1)
    except subprocess.TimeoutExpired:
        return ""
    return out.stdout


def measure(binary: str, config: Path, ks: list[int], mode: str = "recall") -> int:
    runner = run_recall if mode == "recall" else run_find
    results = {k: 0 for k in ks}
    misses: list[Case] = []
    top = max(ks)

    for c in CASES:
        out = runner(binary, config, c.question, top).lower()
        lines = result_lines(out)
        for k in ks:
            window = "\n".join(lines[:k])
            if any(m.lower() in window for m in c.match):
                results[k] += 1
            elif k == top:
                misses.append(c)

    print(f"\nResonance over {len(CASES)} cases  (mode: {mode})")
    print("-" * 46)
    for k in sorted(results):
        hit = results[k]
        pct = 100.0 * hit / len(CASES)
        bar = "#" * int(pct / 2)
        print(f"  hit@{k:<3d} {hit:3d}/{len(CASES)}  {pct:5.1f}%  {bar}")

    if misses:
        print(f"\nMissed at k={top} ({len(misses)}):")
        for c in misses:
            print(f"  #{c.id:2d} [{c.category}] {c.question!r} -> {c.fact!r}")

    out_path = Path("docs/measurements/resonance_results.json")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        json.dumps(
            {
                "cases": len(CASES),
                "hit_at_k": {str(k): results[k] for k in results},
                "hit_rate": {
                    str(k): round(100.0 * results[k] / len(CASES), 1) for k in results
                },
                "missed": [asdict(c) for c in misses],
            },
            indent=2,
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    print(f"\nwrote {out_path}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="validate the test set")
    ap.add_argument("--measure", action="store_true", help="run hit@k against an index")
    ap.add_argument("--binary", default="")
    ap.add_argument("--config", default="test_config.toml")
    ap.add_argument("--mode", choices=["recall", "find"], default="recall",
                    help="recall = natural-language entry point; find = literal substring search")
    ap.add_argument("--k", type=int, nargs="+", default=[5])
    a = ap.parse_args()

    if a.check or not a.measure:
        return validate()

    # Resolve the binary: explicit flag, then the release build (with .exe on
    # Windows), then the debug build.
    if a.binary:
        binary = Path(a.binary)
    else:
        root = Path(__file__).resolve().parent.parent
        cands = [
            root / "target/release/microscope-mem",
            root / "target/release/microscope-mem.exe",
            root / "target/debug/microscope-mem",
            root / "target/debug/microscope-mem.exe",
        ]
        found = next((c for c in cands if c.exists()), None)
        if found is None:
            print(
                "error: no microscope-mem binary found; run: cargo build --release",
                file=sys.stderr,
            )
            return 1
        binary = found

    return measure(str(binary), Path(a.config), a.k, a.mode)


if __name__ == "__main__":
    sys.exit(main())


