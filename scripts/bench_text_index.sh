#!/usr/bin/env bash
# Reproduce the repository's own latency benchmark (.scratch_bench.ps1):
# the inverted text index present vs. removed, on the real layers/ corpus.
#
# The real corpus builds to ~695k blocks, the scale the project's numbers refer
# to. The synthetic 200k-line corpus in .scratch_gen.ps1 expands to 30M blocks
# through the hierarchy and does not fit on disk, so it is not used here.
#
# provider=mock and semantic_weight=0.0 on purpose: this measures the text
# index path, not the embedding path.
set -euo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

DIR="C:/Users/mater/AppData/Local/Temp/mm/bench_real"
WIN="$PWD/bench_real"
rm -rf "$WIN"
mkdir -p "$WIN/data" "$WIN/tmp"

sed -e "s|^layers_dir = .*|layers_dir = \"./layers\"|" \
    -e "s|^output_dir = .*|output_dir = \"$DIR/data\"|" \
    -e "s|^temp_dir = .*|temp_dir = \"$DIR/tmp\"|" \
    config.example.toml > "$WIN/config.toml"

export MICROSCOPE_CONFIG="$WIN/config.toml"
EXE=./target/release/microscope-mem.exe
IDX="$WIN/data/text_index.bin"

echo "building index from the real layers/ ..."
"$EXE" build 2>&1 | tail -3
"$EXE" stats 2>&1 | grep -E 'Blocks|Headers' || true
if [ ! -f "$IDX" ]; then echo "text_index.bin MISSING"; exit 1; fi
echo "text_index.bin: $(stat -c%s "$IDX") bytes"
echo

timeit() {
  local label="$1"; shift
  local start end
  start=$(date +%s%N)
  "$@" >/dev/null 2>&1
  end=$(date +%s%N)
  printf '%-32s %6d ms\n' "$label" $(( (end - start) / 1000000 ))
}

echo "warming page cache..."
"$EXE" find "memory" 5 >/dev/null 2>&1
"$EXE" recall "memory" 5 >/dev/null 2>&1

echo
echo "=== WITH inverted text index ==="
timeit 'FIND   "memory indexing"'      "$EXE" find   "memory indexing" 5
timeit 'FIND   "binary mmap recall"'   "$EXE" find   "binary mmap recall" 5
timeit 'FIND   "cognitive systems"'    "$EXE" find   "cognitive systems" 5
timeit 'RECALL "memory indexing"'      "$EXE" recall "memory indexing" 5
timeit 'RECALL "hebbian remap bug fix"' "$EXE" recall "hebbian remap bug fix" 5

mv "$IDX" "${IDX}.off"
echo
echo "=== WITHOUT inverted text index (full scan) ==="
timeit 'FIND   "memory indexing"'      "$EXE" find   "memory indexing" 5
timeit 'FIND   "binary mmap recall"'   "$EXE" find   "binary mmap recall" 5
timeit 'FIND   "cognitive systems"'    "$EXE" find   "cognitive systems" 5
timeit 'RECALL "memory indexing"'      "$EXE" recall "memory indexing" 5
timeit 'RECALL "hebbian remap bug fix"' "$EXE" recall "hebbian remap bug fix" 5
mv "${IDX}.off" "$IDX"

echo
echo "=== zero-hit path (skips the reinforcement write block) ==="
timeit 'RECALL no hits'                "$EXE" recall "qqqqzzzznonexistent" 5
timeit 'FIND   no hits'                "$EXE" find   "qqqqzzzznonexistent" 5
echo "RESTORED"
