"""Enforce a dependency policy: licences, and where the code comes from.

Written against `cargo metadata` rather than cargo-deny, because that runs with
what CI already installs and needs no extra toolchain step to verify. What it
cannot do is check advisory databases -- there is no CVE data offline here, so
vulnerability scanning is not covered and CLAIMS.md says so.

The policy:
  * every package must declare a licence, and it must be on the allow list;
  * every package must come from crates.io, not a git URL or a local path,
    because a dependency that can be rewritten between two builds of the same
    commit cannot be `--locked` against;
  * duplicate versions are reported, not rejected -- Cargo's resolver produces
    them routinely and failing here would be noise.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

# Permissive, plus the two that are not OSI-approved but are what unicode and
# ICU crates use. Anything else has to be added here deliberately.
ALLOWED = {
    "MIT", "Apache-2.0", "ISC", "BSD-2-Clause", "BSD-3-Clause", "Zlib",
    "Unlicense", "CC0-1.0", "MIT-0", "BSL-1.0", "CDLA-Permissive-2.0",
    "Unicode-3.0", "Unicode-DFS-2016",
    # Weak copyleft, file scope only. Kept because ICU needs it, not because it
    # is in the same class as the rest; see CLAIMS.md.
    "MPL-2.0",
}


def split_expr(expr):
    """`MIT OR Apache-2.0 AND BSD-3-Clause` -> {MIT, Apache-2.0, BSD-3-Clause}.

    SPDX `AND` binds tighter than `OR`, so a licence is acceptable when any
    alternative in the expression is; that is how cargo-deny reads it too. Some
    crates spell the slash form `MIT/Apache-2.0`, which means the same OR.
    """
    expr = expr.replace("(", " ").replace(")", " ")
    out = set()
    for part in expr.split(" OR "):
        for term in part.split(" AND "):
            for alt in term.split("/"):
                alt = alt.strip()
                if alt and alt not in ("N/A", "unknown"):
                    out.add(alt)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--quiet", action="store_true",
                    help="suppress the duplicate-version report")
    args = ap.parse_args()

    meta = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"],
                          cwd=ROOT, capture_output=True, text=True)
    if meta.returncode != 0:
        print("cargo metadata failed:\n" + meta.stderr[:800], file=sys.stderr)
        return 2

    doc = json.loads(meta.stdout)
    # A virtual workspace has no resolve.root, and the metadata still lists this
    # crate as one of its own members, so identity is decided by path as well.
    def is_this_repo(pkg):
        manifest = os.path.abspath(os.path.join(ROOT, pkg["manifest_path"]))
        return manifest == os.path.abspath(os.path.join(ROOT, "Cargo.toml"))

    deps = [p for p in doc["packages"] if not is_this_repo(p)]

    problems = []
    licences = collections.Counter()
    for p in deps:
        name = "%s %s" % (p["name"], p["version"])

        raw = p.get("license")
        if not raw:
            problems.append("%s declares no licence" % name)
        else:
            licences.update(split_expr(raw))
            if not (split_expr(raw) & ALLOWED):
                problems.append("%s licence is %r, not on the allow list"
                                % (name, raw))

        src = p.get("source")
        if src is None:
            # A path dependency. Fine when it points inside this repository --
            # that is a workspace member, our own code, which Cargo.lock cannot
            # pin and does not need to.
            manifest = os.path.abspath(
                os.path.join(ROOT, p.get("manifest_path", "")))
            if not manifest.startswith(os.path.abspath(ROOT) + os.sep):
                problems.append("%s is a path dependency outside this repository: %s"
                                % (name, p.get("manifest_path")))
        elif not src.startswith(CRATES_IO):
            problems.append("%s comes from %s, not crates.io" % (name, src))

    print("checked %d packages against the dependency policy" % len(deps))
    print("licences in use: %s"
          % ", ".join("%s(%d)" % kv for kv in licences.most_common(8)))

    if not args.quiet:
        byname = collections.defaultdict(set)
        for p in deps:
            byname[p["name"]].add(p["version"])
        dups = {n: v for n, v in byname.items() if len(v) > 1}
        if dups:
            print("\n%d crate(s) resolved to more than one version:" % len(dups))
            for n, v in sorted(dups.items()):
                print("  %-26s %s" % (n, ", ".join(sorted(v))))
            print("  informational: these are normal for a graph this size")

    if problems:
        print("\n%d policy violation(s):" % len(problems), file=sys.stderr)
        for line in problems:
            print("  " + line, file=sys.stderr)
        return 1

    print("\nno policy violations")
    return 0


if __name__ == "__main__":
    sys.exit(main())
