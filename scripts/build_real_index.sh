#!/usr/bin/env bash
# Build a real index with a real embedding model and semantic ranking enabled.
#
# The earlier evaluation used provider="mock" and semantic_weight=0.0, which
# disables semantic ranking entirely. This produces the configuration the
# system is actually designed for, so the evaluation measures the architecture
# rather than a hash-based fallback.
set -uo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

rm -rf real_output real_tmp
cp config.example.toml real_config.toml

python - <<'PY'
import re
p = "real_config.toml"
s = open(p, encoding="utf-8", errors="replace").read()
s = re.sub(r'^output_dir\s*=.*$', 'output_dir = "./real_output"', s, flags=re.M)
s = re.sub(r'^temp_dir\s*=.*$', 'temp_dir = "./real_tmp"', s, flags=re.M)
s = re.sub(r'^semantic_weight\s*=.*$', 'semantic_weight = 1.0', s, flags=re.M)
s = re.sub(r'^(provider\s*=\s*)"[^"]*"', r'\1"candle"', s, flags=re.M)
s = re.sub(r'^(model\s*=\s*)"[^"]*"', r'\1"sentence-transformers/all-MiniLM-L6-v2"', s, flags=re.M)
open(p, "w", encoding="utf-8").write(s)
print("--- effective config ---")
for ln in s.splitlines():
    t = ln.strip()
    if t.startswith(("semantic_weight", "provider", "model", "dim", "output_dir", "max_depth", "temp_dir")):
        print("  ", ln)
PY

export MICROSCOPE_CONFIG="$PWD/real_config.toml"
export HF_HOME="$HOME/.cache/huggingface"

echo "--- building index with real embeddings (this is slow: 695k blocks) ---"
time ./target/release/microscope-mem.exe build 2>&1 | tail -20
echo "--- stats ---"
./target/release/microscope-mem.exe stats 2>&1 | head -12
echo "--- embeddings present? ---"
ls -la real_output/embeddings.bin 2>/dev/null || echo "no embeddings.bin"

