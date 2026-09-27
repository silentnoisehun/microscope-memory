#!/usr/bin/env bash
# Generate the same synthetic corpus the project's own benchmark used
# (mirrors .scratch_gen.ps1): 200,000 lines of fixed vocabulary, which is what
# the text-index measurement needs to be meaningful. A 4.8k-block corpus is too
# small to show any difference between an inverted index and a full scan.
set -euo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

# The binary is a Windows program: Git Bash style /c/... paths are not
# resolvable by it. Use drive-letter paths with forward slashes.
DIR="C:/Users/mater/AppData/Local/Temp/mm/perf_corpus"
WIN="$PWD/perf_corpus"
DIRF="$DIR"
rm -rf "$WIN"
mkdir -p "$WIN/layers" "$WIN/data" "$WIN/tmp"

python - "$WIN/layers/long_term.txt" <<'PY'
import sys
n = 200000
line = ("(imp=5) unique_term_{i} memory block about cognitive systems and "
        "binary indexing with common filler words for scale testing")
with open(sys.argv[1], "w", encoding="utf-8") as f:
    f.write("\n".join(line.format(i=i) for i in range(n)) + "\n")
print(f"wrote {n} lines")
PY

cat > "$DIR/config.toml" <<EOF
project_id = "global"

[paths]
layers_dir = "$DIRF/layers"
output_dir = "$DIRF/data"
temp_dir = "$DIRF/tmp"

[index]
max_depth = 8
header_size = 32
auto_rebuild = false
auto_rebuild_entries = 50
layer_retention_entries = 2000

[memory_layers]
layers = ["long_term"]

[search]
default_k = 10
zoom_weight = 2.0
keyword_boost = 0.1
semantic_weight = 0.0
emotional_bias_weight = 0.0

[performance]
use_mmap = true
cache_size = 64
build_workers = 4
use_gpu = false
compression = false
cache_ttl_secs = 300

[logging]
level = "info"
file = "microscope.log"

[embedding]
provider = "mock"
dim = 384
max_depth = 4
EOF

echo "building the 200k-block index..."
MICROSCOPE_CONFIG="$WIN/config.toml" ./target/release/microscope-mem.exe build 2>&1 | tail -6
MICROSCOPE_CONFIG="$WIN/config.toml" ./target/release/microscope-mem.exe stats 2>&1 | head -6
ls -la "$DIR/data/text_index.bin" 2>/dev/null || echo "no text_index.bin"


