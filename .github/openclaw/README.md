# OpenClaw Integration (Microscope-Memory)

This package installs a custom OpenClaw skill that calls your Microscope-Memory API.

## One-command install (Linux/macOS)
```bash
cd .github/openclaw
bash install-openclaw-integration.sh
```

## One-click install (Windows)
Run:
```bat
install-openclaw-integration.bat
```
(Requires WSL.)

## Required environment variables
Set before starting/restarting `openclaw gateway`:
```bash
export MICROSCOPE_BASE_URL="https://memory.example.com"
export MICROSCOPE_USER="memoryadmin"
export MICROSCOPE_PASS="your-password"
```

## Verify
```bash
openclaw skills list
```

The installed skill name is `microscope_memory`.
It uses:
- `~/.openclaw/workspace/skills/microscope_memory/SKILL.md`
- `~/.openclaw/workspace/tools/microscope-memory-api.sh`

## What it enables
- Memory engine status checks
- Recall (`/v1/recall`)
- Remember/write (`/v1/remember`)