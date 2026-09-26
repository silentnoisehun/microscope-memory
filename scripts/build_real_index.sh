#!/usr/bin/env bash
# Build an index from the repository's real layers/ corpus (not the 60-fact
# resonance set) and report the resulting block count, so the two corpora can be
# compared directly.
set -euo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

rm -rf real_output
cp config.example.toml real_config.toml
python - <<'PY'
import re
p = 'real_config.toml'
s = open(p, encoding='utf-8', errors='replace').read()
s = re.sub(r'^output_dir\s*=.*$', 'output_dir = "./real_output"', s, flags=re.M)
s = re.sub(r'^temp_dir\s*=.*$', 'temp_dir = "./real_tmp"', s, flags=re.M)
open(p, 'w', encoding='utf-8').write(s)
print("wrote real_config.toml")
PY

export MICROSCOPE_CONFIG="$PWD/real_config.toml"
./target/release/microscope-mem build 2>&1 | tail -12
echo "--- stats ---"
./target/release/microscope-mem stats 2>/dev/null | head -14
