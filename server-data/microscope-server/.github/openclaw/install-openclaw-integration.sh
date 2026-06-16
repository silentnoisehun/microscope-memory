#!/usr/bin/env bash
set -euo pipefail

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_WORKSPACE="${OPENCLAW_WORKSPACE:-$HOME/.openclaw/workspace}"
TARGET_SKILL_DIR="${TARGET_WORKSPACE}/skills/microscope_memory"
TARGET_TOOLS_DIR="${TARGET_WORKSPACE}/tools"

mkdir -p "${TARGET_SKILL_DIR}" "${TARGET_TOOLS_DIR}"

cp -f "${SRC_DIR}/skills/microscope_memory/SKILL.md" "${TARGET_SKILL_DIR}/SKILL.md"
cp -f "${SRC_DIR}/tools/microscope-memory-api.sh" "${TARGET_TOOLS_DIR}/microscope-memory-api.sh"
chmod +x "${TARGET_TOOLS_DIR}/microscope-memory-api.sh"

cat <<'EOF'
OpenClaw Microscope-Memory integration installed.

Next steps:
1) Export env vars before starting OpenClaw gateway:
   export MICROSCOPE_BASE_URL="https://memory.example.com"
   export MICROSCOPE_USER="memoryadmin"
   export MICROSCOPE_PASS="your-password"

2) Restart gateway or start new session:
   openclaw gateway restart
   /new

3) Verify skill:
   openclaw skills list
EOF