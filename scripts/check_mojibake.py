#!/usr/bin/env python3
"""Refuse a commit that carries single-byte-codepage mojibake.

How this happens: a file is decoded with a single-byte codepage instead of
UTF-8 and written back as UTF-8. Every non-ASCII character becomes three or
more, and the result is technically valid UTF-8, so nothing complains. Repeated
saves multiply it again: in src/main.rs one line reached 109,754 characters and
the file grew from 226 KB to 3.0 MB. The corrupted text was committed, pushed,
and shipped inside the binary, where it printed as
"Failed to open microscope index" followed by thousands of stray glyphs.

Two signals are checked:

  * U+00C2 and U+00C3. These are what a UTF-8 lead or continuation byte becomes
    when it is read through a single-byte codepage. They do not occur in
    Hungarian or English source, so any occurrence is corruption.
  * A run of consecutive non-ASCII characters longer than RUN_LIMIT. Accents are
    isolated; corruption is contiguous. This catches the case where a codepage
    other than CP1250/CP1252 was used and the two markers above are absent.

An individual line is exempt when it carries the marker MOJIBAKE_OK, which is
how documentation that has to show the broken form stays committable. The
marker characters are written as escapes so this file does not trip its own
check.

Usage:
    python scripts/check_mojibake.py --staged      # what the pre-commit hook runs
    python scripts/check_mojibake.py --worktree    # everything tracked, now
    python scripts/check_mojibake.py path [path...] # specific files
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="backslashreplace")

# U+00C2 "A-circumflex", U+00C3 "A-tilde" -- escapes on purpose, so that this
# file does not trip the check it performs.
MARKERS = ("\u00c2", "\u00c3")
MOJIBAKE_OK = "mojibake-ok"
RUN_LIMIT = 40

TEXTY = (".rs", ".md", ".toml", ".py", ".sh", ".bash", ".ps1", ".bat",
         ".json", ".yml", ".yaml", ".html", ".ts", ".tsx", ".js", ".jsx",
         ".css", ".txt", ".cfg", ".ini")

# Runtime memory data, not source. layers/session.txt currently holds mojibake
# from a lossy round through a code page where box-drawing characters replaced
# accented ones, and that loss is not reversible -- so excluding the directory
# keeps the hook from blocking every commit that touches it while source stays
# under the check. It is reported, not hidden.
SKIP_PREFIXES = ("layers/",)


def is_checked(path):
    if path.replace("\\", "/").startswith(SKIP_PREFIXES):
        return False
    return path.lower().endswith(TEXTY)


def _repo_root():
    # Git hooks run with the working directory already at the repository root,
    # so ask git first. Fall back to the script's own location, which is
    # <repo>/scripts/, so the check also works when invoked from elsewhere.
    p = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                       capture_output=True)
    top = p.stdout.decode("utf-8", "replace").strip()
    if top and os.path.isdir(top):
        return top
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


# Every git call is pinned to the repository root, so the check behaves the same
# whether it is invoked from the hook, from the repository, or from elsewhere.
ROOT = _repo_root()


def git(*args):
    return subprocess.run(["git", "-C", ROOT, *args],
                          capture_output=True).stdout


def staged_paths():
    out = git("diff", "--cached", "--name-only", "--diff-filter=ACM")
    return [p for p in out.decode("utf-8", "replace").splitlines() if p.strip()]


def worktree_paths():
    out = git("ls-files")
    return [p for p in out.decode("utf-8", "replace").splitlines() if p.strip()]


def read_staged(path):
    return subprocess.run(["git", "-C", ROOT, "show", ":" + path],
                          capture_output=True).stdout


def read_worktree(path):
    try:
        with open(os.path.join(ROOT, path.replace("/", os.sep)), "rb") as fh:
            return fh.read()
    except OSError:
        return b""


# Characters that legitimately sit next to each other in this project: Hungarian
# letters, typographic punctuation, and the decorative runs used for banners
# (a 60-character rule of box-drawing characters is not damage). A mangled line
# is made of Latin Extended and C1 characters that appear in none of these.
# NB: U+00C2 and U+00C3 are deliberately absent. They are MARKERS, and putting
# them here would let a mangled line dodge the run check.
LEGIT = set("\u00a1\u00a9\u00ab\u00ae\u00b0\u00b1\u00b5\u00b7\u00bb\u00bd\u00be"
            "\u00c0\u00c1\u00c4\u00c5\u00c6\u00c7\u00c8\u00c9\u00ca"
            "\u00cb\u00cc\u00cd\u00ce\u00cf\u00d0\u00d1\u00d2\u00d3\u00d4\u00d5"
            "\u00d6\u00d7\u00d8\u00d9\u00da\u00db\u00dc\u00dd\u00de\u00df"
            "\u00e0\u00e1\u00e2\u00e3\u00e4\u00e5\u00e6\u00e7\u00e8\u00e9\u00ea"
            "\u00eb\u00ec\u00ed\u00ee\u00ef\u00f0\u00f1\u00f2\u00f3\u00f4\u00f5"
            "\u00f6\u00f7\u00f8\u00f9\u00fa\u00fb\u00fc\u00fd\u00fe\u00ff"
            "\u0150\u0151\u0170\u0171"
            "\u2010\u2011\u2012\u2013\u2014\u2018\u2019\u201a\u201c\u201d\u201e"
            "\u2020\u2021\u2022\u2026\u2030\u2039\u203a\u203b\u2032\u2033"
            "\u2044\u20ac\u2122\u2190\u2191\u2192\u2193\u21d0\u21d2\u21d4"
            "\u2200\u2202\u2203\u2205\u2206\u2207\u2208\u220b\u220f\u2211"
            "\u2212\u2215\u221a\u221e\u2220\u2227\u2228\u2229\u222a\u222b"
            "\u2234\u2235\u2236\u223c\u2245\u2248\u2260\u2261\u2264\u2265"
            "\u226a\u226b\u22c5\u22ef\u2500\u2501\u2502\u250c\u2510\u2514"
            "\u2518\u251c\u2524\u252c\u2534\u253c\u2550\u2551\u2552\u2553"
            "\u2554\u2555\u2556\u2557\u2558\u2559\u255a\u255b\u255c\u255d"
            "\u255e\u255f\u2560\u2561\u2562\u2563\u2564\u2565\u2566\u2567"
            "\u2568\u2569\u256a\u256b\u256c\u256d\u256e\u256f\u2570\u2571"
            "\u2572\u2573\u2574\u2575\u2576\u2577\u2578\u2579\u257a\u257b"
            "\u2580\u2584\u2588\u258c\u2590\u2591\u2592\u2593\u25a0\u25a1"
            "\u25b2\u25b6\u25b8\u25bc\u25c0\u25c6\u25cb\u25cf\u25d7\u25e6"
            "\u2605\u2606\u260e\u2611\u2612\u2610\u263a\u263c\u2640\u2642"
            "\u2660\u2663\u2665\u2666\u266a\u2713\u2714\u2717\u2718"
            "\u03b1\u03b2\u03b3\u03b4\u03b5\u03b6\u03b7\u03b8\u03b9\u03ba"
            "\u03bb\u03bc\u03bd\u03be\u03c0\u03c1\u03c3\u03c4\u03c6\u03c7"
            "\u03c8\u03c9\u0391\u0392\u0393\u0394\u0398\u039b\u039e\u039f"
            "\u03a3\u03a4\u03a6\u03a9\u03c6")
LEGIT |= {chr(c) for c in range(0x2500, 0x25A0)}   # box drawing and blocks
LEGIT |= {chr(c) for c in range(0x2190, 0x2200)}   # arrows
LEGIT |= {chr(c) for c in range(0x2600, 0x27C0)}   # symbols and dingbats
LEGIT |= {chr(c) for c in range(0x2300, 0x2400)}   # technical and maths
LEGIT |= {chr(c) for c in range(0x00A0, 0x00C0)}   # Latin-1 punctuation and symbols
# The Latin-1 band above covers U+00A6, which src/reader.rs uses as a comment
# rule, and U+00A7, which appears in the deliberate fixture in
# src/embedding_index.rs. That fixture still trips the run check on its Latin
# Extended characters, which is correct: it is the one place in this repository
# where mojibake belongs, and it says so with the marker.

RUN_LIMIT = 12   # consecutive characters that are not in the set above


def junk_run(text):
    """Longest run of characters that cannot legitimately be adjacent."""
    best = cur = 0
    at = 0
    best_at = 0
    for i, ch in enumerate(text):
        if ord(ch) > 127 and ch not in LEGIT:
            cur += 1
            if cur > best:
                best, best_at = cur, i
        else:
            cur = 0
    return best, best_at


def scan(name, data):
    """Return a list of complaints for one file."""
    if b"\x00" in data[:8192]:
        return []                                   # binary
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as exc:
        return ["%s: not valid UTF-8 (%s)" % (name, exc)]

    problems = []
    for lineno, line in enumerate(text.split("\n"), 1):
        if MOJIBAKE_OK in line:
            continue
        hits = [m for m in MARKERS if m in line]
        if hits:
            names = ", ".join("U+%04X" % ord(m) for m in hits)
            problems.append(
                "%s:%d: %s (%s) -- text was read through a single-byte codepage"
                % (name, lineno, names, line.strip()[:70]))

    best, best_at = junk_run(text)
    if best > RUN_LIMIT:
        lineno = text.count("\n", 0, best_at) + 1
        problems.append(
            "%s:%d: %d consecutive characters that do not occur in this "
            "project (limit %d) -- a mangled line; put `%s` on the line to "
            "allow it" % (name, lineno, best, RUN_LIMIT, MOJIBAKE_OK))
    return problems


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    src = ap.add_mutually_exclusive_group()
    src.add_argument("--staged", action="store_true")
    src.add_argument("--worktree", action="store_true")
    ap.add_argument("paths", nargs="*")
    args = ap.parse_args()

    if args.staged:
        paths = staged_paths()
        read = read_staged
        what = "staged"
    elif args.worktree:
        paths = [p for p in worktree_paths() if is_checked(p)]
        read = read_worktree
        what = "tracked"
    elif args.paths:
        paths = args.paths
        read = read_worktree
        what = "given"
    else:
        paths = staged_paths()
        read = read_staged
        what = "staged"

    problems = []
    checked = 0
    for p in paths:
        if not is_checked(p):
            continue
        checked += 1
        problems.extend(scan(p, read(p)))

    if problems:
        print("check_mojibake: %d problem(s) in %s content:"
              % (len(problems), what), file=sys.stderr)
        for line in problems[:40]:
            print("  " + line, file=sys.stderr)
        if len(problems) > 40:
            print("  ... and %d more" % (len(problems) - 40), file=sys.stderr)
        print("", file=sys.stderr)
        print("This is the signature of a UTF-8 file that was saved through a", file=sys.stderr)
        print("single-byte codepage. It is what grew src/main.rs to 3 MB. To see", file=sys.stderr)
        print("what the file looked like before, check the history; to allow a", file=sys.stderr)
        print("deliberate example, put `%s` on the offending line." % MOJIBAKE_OK,
              file=sys.stderr)
        return 1

    skipped = len(paths) - checked
    print("check_mojibake: %d %s file(s) clean%s."
          % (checked, what,
             ", %d under layers/ skipped as runtime memory data" % skipped
             if skipped else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
