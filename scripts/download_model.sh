#!/usr/bin/env bash
# Download the real embedding model used for the semantic path.
#
# The earlier evaluation runs used embedding.provider="mock", which derives
# coordinates from a text hash and disables the semantic ranking that
# distinguishes the system. This fetches an actual transformer so the semantic
# path can be measured.
set -euo pipefail
cd /c/Users/mater/AppData/Local/Temp/mm

MODEL="${1:-sentence-transformers/all-MiniLM-L6-v2}"
CACHE="$HOME/.cache/huggingface"

echo "model: $MODEL"
echo "cache: $CACHE"
python - "$MODEL" <<'PY'
import os, sys
model = sys.argv[1]
cache = os.path.expanduser("~/.cache/huggingface")
os.environ.setdefault("HF_HOME", cache)
try:
    from huggingface_hub import snapshot_download
except ImportError:
    print("huggingface_hub not installed: pip install huggingface_hub", file=sys.stderr)
    sys.exit(1)
p = snapshot_download(
    repo_id=model,
    cache_dir=os.path.join(cache, "hub"),
    allow_patterns=["*.json", "*.txt", "*.bin", "*.safetensors", "*.model"],
)
print("downloaded to:", p)
for f in sorted(os.listdir(p)):
    print("  ", f)
PY
