#!/usr/bin/env bash
# auto-inject.sh — Universal auto-context injector for any LLM wrapper.
#
# Usage:
#   ./auto-inject.sh                # prints auto-context to stdout
#   ./auto-inject.sh --compact      # compact version
#   ./auto-inject.sh --output FILE  # write to file
#
# Drop this into your LLM wrapper's startup:
#   - Bash / git-bash:    eval "$(auto-inject.sh --output /tmp/ctx.txt)"; cat /tmp/ctx.txt
#   - Claude Code:        see scripts/microscope-recall-hook.ps1 (Windows)
#   - Generic AI wrapper: source the file or call it before each session
#
# Env vars (optional):
#   MICROSCOPE_BIN   — path to microscope-mem executable (default: ./target/release/microscope-mem.exe)
#   MICROSCOPE_CFG   — path to config.toml (default: ./config.toml)

set -e

# Resolve binary
if [ -z "$MICROSCOPE_BIN" ]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
  if [ -f "$ROOT_DIR/target/release/microscope-mem.exe" ]; then
    MICROSCOPE_BIN="$ROOT_DIR/target/release/microscope-mem.exe"
  elif [ -f "$ROOT_DIR/target/release/microscope-mem" ]; then
    MICROSCOPE_BIN="$ROOT_DIR/target/release/microscope-mem"
  elif command -v microscope-mem >/dev/null 2>&1; then
    MICROSCOPE_BIN="microscope-mem"
  else
    echo "[auto-inject] ERROR: microscope-mem binary not found" >&2
    exit 1
  fi
fi

COMPACT=""
OUTPUT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --compact) COMPACT="--compact"; shift ;;
    --output)  OUTPUT="$2"; shift 2 ;;
    *) echo "[auto-inject] unknown arg: $1" >&2; exit 1 ;;
  esac
done

CMD="$MICROSCOPE_BIN auto-context $COMPACT"
if [ -n "$OUTPUT" ]; then
  $CMD --output "$OUTPUT"
  echo "Auto-context written to $OUTPUT" >&2
else
  $CMD
fi