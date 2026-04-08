#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: bash restore.sh /path/to/microscope-memory_YYYYMMDD_HHMMSS.tar.gz"
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARCHIVE="$1"

if [[ ! -f "${ARCHIVE}" ]]; then
  echo "Archive not found: ${ARCHIVE}"
  exit 1
fi

cd "${ROOT_DIR}"
cp -f "${ARCHIVE}" "./restore_tmp.tar.gz"
tar -xzf "./restore_tmp.tar.gz"
rm -f "./restore_tmp.tar.gz"

echo "Restore completed from: ${ARCHIVE}"