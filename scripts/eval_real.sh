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

rm -rf eval_layers eval_output eval_tmp
mkdir -p eval_layers

# Real corpus, unchanged.
cp layers/*.txt eval_layers/ 2>/dev/null
rm -f eval_layers/session.txt
cp layers/session.txt eval_layers/session.txt

# The 60 known facts, as one extra layer.
python - <<'PY'
import sys
sys.path.insert(0, "scripts")
from resonance_set import CASES
with open("eval_layers/resonance_facts.txt", "w", encoding="utf-8") as f:
    for c in CASES:
        f.write(c.fact + "\n")
print(f"wrote {len(CASES)} resonance facts")
PY

cp config.example.toml eval_config.toml
python - <<'PY'
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
python - <<'PY'
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
python - <<'PY'
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

echo "--- building eval index (real embeddings) ---"
# Do not pipe the build through grep | head: that discards the exit status, so a
# failed or partial embedding build still produced a "successful" measurement.
BUILD_LOG="$(mktemp)"
./target/release/microscope-mem.exe build >"$BUILD_LOG" 2>&1
BUILD_RC=$?
grep -E 'Embedding up to|stored vectors|OK embeddings|ERR|WARN|Blocks:' "$BUILD_LOG" | head -8
if [ $BUILD_RC -ne 0 ]; then
    echo "ABORT: build failed (exit $BUILD_RC). Full log: $BUILD_LOG" >&2
    tail -30 "$BUILD_LOG" >&2
    exit 1
fi

