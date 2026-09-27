#!/usr/bin/env bash
# Measure retrieval with REAL embeddings on a corpus that contains the
# resonance facts alongside the real memory content.
#
# Why combine: the 60 resonance facts are a test set, not the real corpus. The
# real corpus (layers/, 695k blocks) does not contain them, so hit@k cannot be
# computed on it. Adding the facts to the real corpus measures the semantic
# path under realistic conditions: 6574 real lines plus 60 known facts, with
# real MiniLM embeddings and semantic_weight=1.0.
set -uo pipefail

# Repository root. Defaults to the checkout this script lives in, so the script
# is not pinned to one machine's temp directory. Override with MM_ROOT=...
MM_ROOT="${MM_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$MM_ROOT" || { echo "MM_ROOT does not exist: $MM_ROOT" >&2; exit 1; }
echo "MM_ROOT=$MM_ROOT"

# Python interpreter. Bare `python` is not on PATH under every shell (Git Bash
# on Windows exposes `python3` and a *different* MSYS interpreter that cannot
# see the site-packages holding numpy/faiss/sentence-transformers), and the
# failure mode is a confusing traceback rather than a clear message. Resolve an
# interpreter that can actually import the dependencies, and let the caller
# override with PY=/path/to/python.
if [ -z "${PY:-}" ]; then
    for cand in python3 python; do
        if command -v "$cand" >/dev/null 2>&1 && \
           "$cand" -c "import numpy" >/dev/null 2>&1; then
            PY="$cand"
            break
        fi
    done
fi
if [ -z "${PY:-}" ]; then
    # Fall back to a Windows-style interpreter if one is on PATH.
    for cand in /c/Python*/python.exe python.exe; do
        if command -v "$cand" >/dev/null 2>&1; then PY="$cand"; break; fi
    done
fi
if [ -z "${PY:-}" ]; then
    echo "ABORT: no usable Python found. Set PY=/path/to/python." >&2
    exit 1
fi
echo "PY=$PY"
if ! "$PY" -c "import numpy, faiss, sentence_transformers" >/dev/null 2>&1; then
    echo "WARNING: $PY cannot import numpy/faiss/sentence-transformers." >&2
    echo "         The Microscope row will still be measured; the FAISS and" >&2
    echo "         SQLite rows will report an error." >&2
    echo "         pip install faiss-cpu numpy sentence-transformers" >&2
fi

rm -rf eval_layers eval_output eval_tmp
mkdir -p eval_layers

# Real corpus, unchanged.
cp layers/*.txt eval_layers/ 2>/dev/null
rm -f eval_layers/session.txt
cp layers/session.txt eval_layers/session.txt

# The 60 known facts, as one extra layer.
  "$PY" - <<'PY'
import sys
sys.path.insert(0, "scripts")
from resonance_set import CASES
with open("eval_layers/resonance_facts.txt", "w", encoding="utf-8") as f:
    for c in CASES:
        f.write(c.fact + "\n")
print(f"wrote {len(CASES)} resonance facts")
PY

cp config.example.toml eval_config.toml
  "$PY" - <<'PY'
import re
p = "eval_config.toml"
s = open(p, encoding="utf-8", errors="replace").read()
s = re.sub(r'^layers_dir\s*=.*$', 'layers_dir = "./eval_layers"', s, flags=re.M)
s = re.sub(r'^output_dir\s*=.*$', 'output_dir = "./eval_output"', s, flags=re.M)
s = re.sub(r'^temp_dir\s*=.*$', 'temp_dir = "./eval_tmp"', s, flags=re.M)
s = re.sub(r'^semantic_weight\s*=.*$', 'semantic_weight = 1.0', s, flags=re.M)
s = re.sub(r'^(provider\s*=\s*)"[^"]*"', r'\1"candle"', s, flags=re.M)
s = re.sub(r'^(model\s*=\s*)"[^"]*"', r'\1"sentence-transformers/all-MiniLM-L6-v2"', s, flags=re.M)
# NOTE: max_depth is deliberately NOT rewritten here. A previous version forced
# D5 -> D4 to shorten the embedding build, so the headline 75.0% R@5 was measured
# on a D4-truncated index while config.example.toml defaults to D5. That made the
# number unreproducible from the committed configuration. The depth is now read
# back from the generated config and asserted below.
# The build reads only the layers named in [memory_layers]. The resonance
# layer must be listed or its 60 facts are silently skipped.
s = re.sub(
    r'(\[memory_layers\][^\[]*layers\s*=\s*\[)([^\]]*)(\])',
    lambda m: m.group(1) + m.group(2).rstrip() + '\n    "resonance_facts"\n    ' + m.group(3),
    s,
    flags=re.S,
)
open(p, "w", encoding="utf-8").write(s)
PY

# The regex above can leave a trailing comma before the closing bracket, which
# TOML rejects and which made the binary fall back to defaults. Drop it.
  "$PY" - <<'PY'
import re
p = "eval_config.toml"
s = open(p, encoding="utf-8").read()
# Add the missing comma between the last original layer and the new one, then
# drop any trailing comma before "]": TOML rejects that, and the binary then
# falls back to default configuration without failing loudly.
s = s.replace('"rust_state"\n    "resonance_facts"', '"rust_state",\n    "resonance_facts"')
s = re.sub(r",(\s*\])", r"\1", s)
open(p, "w", encoding="utf-8").write(s)
m = re.search(r"layers\s*=\s*\[(.*?)\]", s, re.S)
print("layers:", (m.group(1).replace("\n", " ") if m else "NOT FOUND"))
PY

export MICROSCOPE_CONFIG="$PWD/eval_config.toml"
export HF_HOME="$HOME/.cache/huggingface"

# ── Gate: assert the configuration we are about to measure with ────────────
# A malformed TOML makes the binary fall back to built-in defaults *without
# failing*, so a measurement can silently run against a different configuration
# than the one on disk. Parsing the file and asserting the fields is the only
# reliable check; grepping the first lines of `stats` output is not.
  "$PY" - <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError:
        sys.exit("need Python 3.11+ (tomllib) or the tomli package to verify the config")

with open("eval_config.toml", "rb") as f:
    try:
        cfg = tomllib.load(f)
    except Exception as e:
        sys.exit(f"eval_config.toml is not valid TOML: {e}\n"
                 "The binary would silently fall back to defaults; aborting.")

def get(section, key, default=None):
    return cfg.get(section, {}).get(key, default)

problems = []

# The embedding provider and model must be the real ones, not a mock.
provider = get("embedding", "provider")
model = get("embedding", "model")
if provider != "candle":
    problems.append(f"embedding.provider = {provider!r}, expected 'candle'")
if model != "sentence-transformers/all-MiniLM-L6-v2":
    problems.append(f"embedding.model = {model!r}, expected the MiniLM checkpoint")

# semantic_weight must be 1.0 for this benchmark, otherwise the semantic
# contribution is scaled and the number is not comparable.
sw = get("search", "semantic_weight")
if sw != 1.0:
    problems.append(f"search.semantic_weight = {sw!r}, expected 1.0")

# The depth must match the committed default. This is the value a previous
# version silently rewrote to 4, which is what made the 75.0% irreproducible.
# Note this is [embedding].max_depth; [index].max_depth is a different setting.
md = get("embedding", "max_depth")
if md != 5:
    problems.append(f"embedding.max_depth = {md!r}, expected 5")

# The recall layer must be registered, or its 60 facts never enter the corpus.
layers = get("memory_layers", "layers") or []
if "resonance_facts" not in layers:
    problems.append(f"memory_layers.layers does not contain 'resonance_facts': {layers}")

print("verified config: provider=%s model=%s semantic_weight=%s max_depth=%s layers=%d"
      % (provider, model, sw, md, len(layers)))
if problems:
    sys.exit("CONFIG MISMATCH:\n  - " + "\n  - ".join(problems))
PY
if [ $? -ne 0 ]; then
    echo "ABORT: refusing to measure against an unverified configuration." >&2
    exit 1
fi

echo "--- building release binary with the embeddings feature ---"
# The candle provider lives behind the `embeddings` cargo feature, which is NOT
# in the default set (`default = ["native"]`). A build without it compiles fine
# and then fails at run time with "requires the 'embeddings' feature ... Refusing
# to fall back to mock", so the index never gets built and the harness measures
# an empty corpus. Build with the feature explicitly.
CARGO_LOG="$(mktemp)"
cargo build --release --features native,embeddings >"$CARGO_LOG" 2>&1
CARGO_RC=$?
if [ $CARGO_RC -ne 0 ]; then
    echo "ABORT: cargo build --features native,embeddings failed (exit $CARGO_RC)" >&2
    tail -30 "$CARGO_LOG" >&2
    exit 1
fi
if [ ! -x ./target/release/microscope-mem.exe ]; then
    echo "ABORT: release binary missing after a successful cargo build" >&2
    exit 1
fi

echo "--- building eval index (real embeddings) ---"
# Do not pipe the build through grep | head: that discards the exit status, so a
# failed or partial embedding build still produced a "successful" measurement.
BUILD_LOG="$(mktemp)"
./target/release/microscope-mem.exe build >"$BUILD_LOG" 2>&1
BUILD_RC=$?
grep -E 'Embedding up to|stored vectors|OK embeddings|ERR|WARN|ERROR|Blocks:' "$BUILD_LOG" | head -8
if [ $BUILD_RC -ne 0 ]; then
    echo "ABORT: build failed (exit $BUILD_RC). Full log: $BUILD_LOG" >&2
    tail -30 "$BUILD_LOG" >&2
    exit 1
fi
# The binary refuses to substitute the mock provider for candle, and exits
# non-zero, but assert the embedding file too: a build that "succeeds" without
# embeddings must never be measured.
if [ ! -s eval_output/embeddings.bin ]; then
    echo "ABORT: build reported success but eval_output/embeddings.bin is missing." >&2
    echo "       The provider probably fell back to mock." >&2
    tail -20 "$BUILD_LOG" >&2
    exit 1
fi

# ── Verify the index actually survived the build ──────────────────────────
# A previous run built the index successfully and then measured against
# nothing: the output directory had been removed between the build and the
# query loop, so all 60 recalls ran on a missing index. The harness still
# printed a result table -- R@5 6.7%, p50 10.4 s -- which looked like a
# regression but was an artifact of measuring an absent index. Assert the
# files are present, and again after the measurement, so a vanished index
# fails loudly instead of producing a plausible-looking number.
require_index() {
    local missing=0 f
    for f in meta.bin embeddings.bin; do
        if [ ! -s "eval_output/$f" ]; then
            echo "ABORT: eval_output/$f is missing or empty" >&2
            missing=1
        fi
    done
    [ $missing -eq 0 ] || exit 1
    echo "index present: $(ls -1 eval_output | tr '\n' ' ')"
}
echo "--- verifying eval index ---"
require_index

# ── Measure ───────────────────────────────────────────────────────────────
# The query loop lives in compare_baselines.py: it drives the same `recall` CLI
# a user would run, times it end to end, and writes
# docs/measurements/real_embedding_comparison.json. It defaults to
# eval_config.toml, i.e. exactly the config the gate above verified.
#
# Note on the baselines: the FAISS and SQLite rows are diagnostics, not a
# like-for-like comparison. They index only the 60 fact vectors and report
# query-time search only, while the Microscope row scans the full corpus and
# includes process start, provider construction and query embedding.
echo "--- measuring recall + latency vs baselines ---"
"$PY" scripts/compare_baselines.py --config eval_config.toml --k 1 5 10
MEASURE_RC=$?
if [ $MEASURE_RC -ne 0 ]; then
    echo "ABORT: measurement failed (exit $MEASURE_RC)" >&2
    exit 1
fi

# The index must still be there after the run, and recall must return
# something. A table of near-zero recall with a huge p50 is the signature of a
# broken measurement, not a broken retriever; fail rather than publish it.
require_index
if ! MICROSCOPE_CONFIG="$PWD/eval_config.toml" \
     ./target/release/microscope-mem.exe recall coffee 5 2>/dev/null | grep -q .; then
    echo "ABORT: recall returned nothing on a sanity query; the measurement is not trustworthy" >&2
    exit 1
fi
echo "--- sanity check passed ---"

