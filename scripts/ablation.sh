#!/usr/bin/env bash
# Layer ablation: rebuild the resonance index with one reinforcement layer
# disabled at a time, then re-measure hit@5.
#
# The layers are code modules with no config switch, so each variant is produced
# by a temporary source patch, a rebuild, an index rebuild and a measurement.
# The patch is reverted after each variant.
#
# Usage: bash scripts/ablation.sh [layer ...]      (default: hebbian mirror attention)
set -euo pipefail
cd "$(dirname "$0")/.."

LAYERS=("$@")
if [ ${#LAYERS[@]} -eq 0 ]; then
  LAYERS=(hebbian mirror attention)
fi

log() { printf '%s\n' "$*"; }

# Baseline: everything on.
log "=== baseline (all layers enabled) ==="
rm -rf bench_output
python scripts/build_bench_index.py >/dev/null 2>&1
python scripts/resonance_set.py --measure --mode recall --config bench_config.toml --k 5 \
  | grep -E 'hit@5' || true

for layer in "${LAYERS[@]}"; do
  log ""
  log "=== ablation: $layer disabled ==="
  src="src/${layer}.rs"
  if [ ! -f "$src" ]; then
    log "  no $src, skipping"
    continue
  fi
  cp "$src" "${src}.ablation.bak"

  # Neutralise the module's public entry points used by the recall pipeline.
  # Wrapping the struct's Default is not enough, so the simplest reliable
  # approach is to make the module's state always-empty and identity-scoring.
  python - "$src" <<'PY'
import re, sys
p = sys.argv[1]
s = open(p, encoding="utf-8").read()
# If the module exposes a scoring/boost function, make it a no-op.
for fn in ("pub fn mirror_boost", "pub fn pattern_boost", "pub fn match_archetype",
           "pub fn boost", "pub fn record_activation"):
    i = s.find(fn)
    if i == -1:
        continue
    # insert an early return at the top of the function body
    j = s.find("{", i)
    if j == -1:
        continue
    ret = "\n    return Default::default(); // ABLATION: disabled\n"
    s = s[:j+1] + ret + s[j+1:]
open(p, "w", encoding="utf-8").write(s)
print(f"  patched {p}")
PY

  if cargo build --release --features native >/dev/null 2>&1; then
    rm -rf bench_output
    python scripts/build_bench_index.py >/dev/null 2>&1 || true
    python scripts/resonance_set.py --measure --mode recall --config bench_config.toml --k 5 \
      | grep -E 'hit@5' || log "  (measurement failed)"
  else
    log "  build failed for $layer (patch not compilable); skipping"
  fi

  mv "${src}.ablation.bak" "$src"
done

log ""
log "=== restoring baseline binary ==="
cargo build --release --features native >/dev/null 2>&1
rm -rf bench_output
python scripts/build_bench_index.py >/dev/null 2>&1
log "done"
