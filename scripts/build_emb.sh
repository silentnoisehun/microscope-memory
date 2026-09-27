#!/usr/bin/env bash
# Build the release binary with real embedding support.
# The candle stack is large; this takes a long time on first build.
set -uo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

echo "disk free before: $(df -h /c | tail -1 | awk '{print $4}')"
echo "building with features: native embeddings"
touch src/lib.rs src/embeddings.rs
cargo build --release --features "native embeddings" 2>&1 | tail -25
echo "exit=$?"
ls -la target/release/microscope-mem.exe 2>/dev/null || ls -la target/release/microscope-mem
echo "disk free after: $(df -h /c | tail -1 | awk '{print $4}')"
