#!/usr/bin/env bash
# Generate .claude/settings.json with the current project path baked in.
# Usage:
#   bash scripts/install-claude-hooks.sh
#
# See scripts/install-claude-hooks.ps1 for full rationale. This is the bash
# equivalent; both produce the same settings.json.
#
# Override defaults with env vars:
#   MICROSCOPE_CLAUDE_DIR  - target directory (default: ./.claude)
#   MICROSCOPE_HOOK_SCRIPT - hook script path (default: ./scripts/microscope-recall-hook.ps1)
set -e

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

CLAUDE_DIR="${MICROSCOPE_CLAUDE_DIR:-$REPO_ROOT/.claude}"
HOOK_SCRIPT="${MICROSCOPE_HOOK_SCRIPT:-$SCRIPT_DIR/microscope-recall-hook.ps1}"
SETTINGS_PATH="$CLAUDE_DIR/settings.json"

if [ "${1:-}" = "--uninstall" ]; then
    [ -f "$SETTINGS_PATH" ] && rm -v "$SETTINGS_PATH"
    exit 0
fi

mkdir -p "$CLAUDE_DIR"

# Use python to safely emit JSON (avoids quoting hell in pure bash).
HOOK_ABS=$(cd "$(dirname "$HOOK_SCRIPT")" && pwd)/$(basename "$HOOK_SCRIPT")

python3 - "$HOOK_ABS" "$SETTINGS_PATH" <<'PY' 2>/dev/null || python - "$HOOK_ABS" "$SETTINGS_PATH" <<'PY'
import json, sys
hook = sys.argv[1]
out = sys.argv[2]
settings = {
    "hooks": {
        "UserPromptSubmit": {
            "command": "powershell.exe",
            "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", hook]
        },
        "PostToolUse": {
            "command": "powershell.exe",
            "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", hook]
        },
        "Stop": {
            "command": "powershell.exe",
            "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", hook]
        }
    }
}
with open(out, "w", encoding="utf-8") as f:
    json.dump(settings, f, indent=2, ensure_ascii=False)
    f.write("\n")
print(f"wrote {out}")
print(f"  hook script: {hook}")
print()
print("Restart Claude Code for the hooks to take effect.")
PY