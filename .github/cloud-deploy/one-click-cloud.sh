#!/usr/bin/env bash
set -euo pipefail

# One-click cloud start for Ubuntu/Debian-like systems.
# Usage:
#   bash one-click-cloud.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v sudo >/dev/null 2>&1; then
  echo "A sudo hianyzik. Telepitsd a sudo-t, majd futtasd ujra."
  exit 1
fi

echo "[1/3] Bootstrap (Docker + dependencies)..."
sudo bash "${SCRIPT_DIR}/bootstrap-ubuntu.sh"

echo "[2/3] One-click wizard indul..."
bash "${SCRIPT_DIR}/easy-start.sh"

echo "[3/3] Kesz. A szolgaltatas fut."