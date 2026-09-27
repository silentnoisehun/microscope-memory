#!/usr/bin/env bash
set -uo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm
cargo build --release --features "native embeddings" 2>&1 | tail -6
echo "build exit=$?"
