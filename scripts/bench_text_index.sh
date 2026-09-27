#!/usr/bin/env bash
# Reproduce the project's own performance benchmark, which measures latency
# with the inverted text index enabled vs. removed. This is the measurement the
# paper should report: it is the one the codebase was written for, and it does
# not depend on a synthetic relevance set.
#
# Method mirrors .scratch_bench.ps1: warm the cache, then time `find` and
# `recall` with the index present and with it renamed away.
set -uo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

export MICROSCOPE_CONFIG="$PWD/bench_config.toml"
EXE=./target/release/microscope-mem.exe
IDX=bench_output/text_index.bin

if [ ! -f "$IDX" ]; then
  echo "text_index.bin not present; run scripts/build_bench_index.py first" >&2
  exit 1
fi

timeit() {  # timeit <label> <cmd...>
  local label="$1"; shift
  local start end
  start=$(date +%s%N)
  "$@" >/dev/null 2>&1
  end=$(date +%s%N)
  printf '%-34s %6d ms\n' "$label" $(( (end - start) / 1000000 ))
}

echo "corpus:"; "$EXE" stats 2>/dev/null | grep -E 'Blocks|Headers' | sed 's/^/  /'
echo
echo "warming cache..."
"$EXE" find "memory" 5 >/dev/null 2>&1
"$EXE" recall "memory" 5 >/dev/null 2>&1

echo
echo "=== WITH inverted text index ==="
timeit 'FIND   "memory indexing"'     "$EXE" find   "memory indexing" 5
timeit 'FIND   "binary mmap recall"'  "$EXE" find   "binary mmap recall" 5
timeit 'FIND   "cognitive systems"'   "$EXE" find   "cognitive systems" 5
timeit 'RECALL "memory indexing"'     "$EXE" recall "memory indexing" 5
timeit 'RECALL "hebbian remap bug fix"' "$EXE" recall "hebbian remap bug fix" 5

mv "$IDX" "${IDX}.off"
echo
echo "=== WITHOUT inverted text index (full scan) ==="
timeit 'FIND   "memory indexing"'     "$EXE" find   "memory indexing" 5
timeit 'FIND   "binary mmap recall"'  "$EXE" find   "binary mmap recall" 5
timeit 'FIND   "cognitive systems"'   "$EXE" find   "cognitive systems" 5
timeit 'RECALL "memory indexing"'     "$EXE" recall "memory indexing" 5
timeit 'RECALL "hebbian remap bug fix"' "$EXE" recall "hebbian remap bug fix" 5
mv "${IDX}.off" "$IDX"

echo
echo "=== zero-hit path (skips the reinforcement write block) ==="
timeit 'RECALL no hits'               "$EXE" recall "qqqqzzzznonexistent" 5
timeit 'FIND   no hits'               "$EXE" find   "qqqqzzzznonexistent" 5
echo "RESTORED"
