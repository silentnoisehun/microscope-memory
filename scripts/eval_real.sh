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
cd /c/Users/mater/AppData/Local/Temp/mm

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
# D5 doubles the embedding build time for no benefit when measuring whether the
# recall fix works; the recall fix itself is independent of this depth.
s = re.sub(r'^max_depth\s*=\s*5$', 'max_depth = 4', s, flags=re.M)
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

echo "--- config load check (must NOT warn) ---"
./target/release/microscope-mem.exe stats 2>&1 | head -3
echo "--- building eval index (real embeddings) ---"
./target/release/microscope-mem.exe build 2>&1 | grep -E 'Embedding up to|stored vectors|OK embeddings|ERR|WARN|Blocks:' | head -8
