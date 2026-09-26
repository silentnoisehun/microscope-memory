#!/usr/bin/env python3
"""Build a benchmark index from the 60 resonance facts.

Writes the facts into a layers/ directory, then calls `microscope-mem build`
to produce the binary index that resonance_set.py measures against.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from resonance_set import CASES  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
LAYERS = ROOT / "bench_layers"
OUT = ROOT / "bench_output"
CONFIG = ROOT / "bench_config.toml"
# cargo appends .exe on Windows
_candidates = [ROOT / "target/release/microscope-mem", ROOT / "target/release/microscope-mem.exe"]
BIN = next((c for c in _candidates if c.exists()), _candidates[0])


def main() -> int:
    if not BIN.exists():
        print(f"error: {BIN} not found; run: cargo build --release", file=sys.stderr)
        return 1

    LAYERS.mkdir(exist_ok=True)
    OUT.mkdir(exist_ok=True)

    # The facts go into long_term. A second filler layer adds shallow noise so
    # the ranking is not trivially perfect on an all-matching corpus.
    facts = [c.fact for c in CASES]
    (LAYERS / "long_term.txt").write_text("\n".join(facts) + "\n", encoding="utf-8")

    filler = [
        "The system checked the build status and everything was green.",
        "A routine maintenance task completed without errors.",
        "The cache was cleared during the nightly rebuild.",
        "No anomalies were detected in the last integrity sweep.",
        "Configuration defaults were restored from the template.",
    ]
    (LAYERS / "associative.txt").write_text("\n".join(filler) + "\n", encoding="utf-8")

    # The Config struct deserialises strictly and several sections are not
    # #[serde(default)], so start from the shipped example and rewrite only the
    # paths. Writing a partial file by hand fails to load.
    template = ROOT / "config.example.toml"
    cfg = template.read_text(encoding="utf-8", errors="replace")
    cfg = re.sub(r'^layers_dir\s*=.*$', 'layers_dir = "./bench_layers"', cfg, flags=re.M)
    cfg = re.sub(r'^output_dir\s*=.*$', 'output_dir = "./bench_output"', cfg, flags=re.M)
    cfg = re.sub(r'^temp_dir\s*=.*$', 'temp_dir = "./bench_tmp"', cfg, flags=re.M)
    # Keep only the two layers this benchmark uses.
    cfg = re.sub(
        r'(\[memory_layers\]\s*\nlayers\s*=\s*\[)[^\]]*(\])',
        r'\1"long_term", "associative"\2',
        cfg,
        flags=re.S,
    )
    CONFIG.write_text(cfg, encoding="utf-8")

    print(f"wrote {len(facts)} facts to {LAYERS}")
    # The binary selects its config through the MICROSCOPE_CONFIG environment
    # variable; there is no --config flag.
    env = dict(os.environ, MICROSCOPE_CONFIG=str(CONFIG))
    r = subprocess.run(
        [str(BIN), "build"],
        capture_output=True,
        text=True,
        env=env,
    )
    sys.stdout.write(r.stdout[-2000:])
    sys.stderr.write(r.stderr[-2000:])
    if r.returncode != 0:
        print(f"build failed: {r.returncode}", file=sys.stderr)
        return r.returncode
    print(f"index built in {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
