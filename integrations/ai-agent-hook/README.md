# Universal AI Agent Memory Hook

Universal, high-performance cognitive memory bridge for AI coding agents (**Claude Code**, **MiniMax Code**, **Codex CLI**, **Goose**, **Antigravity**, **OMP**).

## Why this Hook?

- **Zero-MCP Overhead:** Calls the native `microscope-mem` binary directly via sequential CLI queries (~95 ms), avoiding HTTP / JSON-RPC / WebSocket latency and handshake bottlenecks.
- **Resilient JSON Parsing:** Implements multi-tier JSON sanitization fixing Windows unescaped path backslashes (e.g. `C:\Users\...` escaping into illegal escape sequences like `\U`), completely eliminating `Bad escaped character in JSON` crashes.
- **Windows mmap Collision Safety:** Executes multi-phrase retrievals sequentially to prevent Windows mmap lock collisions.
- **Detached Index Rebuilding:** Auto-rebuild is uncoupled from hook turns; costly folding runs in a detached child process without blocking the agent.
- **Fail-Silent Contract:** Every hook handler is guaranteed to exit `0` within safe execution budgets (<8s), ensuring agent workflows are never interrupted.

## Architecture & Lifecycle Events

```
SessionStart       ──▶ Auto-Context Injection (Core Memories imp>=7, Open Loops, Timeline)
UserPromptSubmit   ──▶ Semantic Phrase Search (Multi-word phrase extraction -> find)
PostToolUse        ──▶ Automatic Memory Storage (Code/Session layer, Importance 5-8)
Rebuild Trigger    ──▶ Background Detached Rebuild (every 20 stores)
```

## Quick Start & Installation

Run PowerShell installer:
```powershell
.\install.ps1 -Target all
```

Or configure manually in your agent's `hooks.json`:
```json
{
  "SessionStart": [{ "matcher": "*", "hooks": [{ "command": "node path/to/microscope-hook.js session-start" }] }],
  "UserPromptSubmit": [{ "hooks": [{ "command": "node path/to/microscope-hook.js prompt" }] }],
  "PostToolUse": [{ "matcher": "*", "hooks": [{ "command": "node path/to/microscope-hook.js tool-result" }] }]
}
```

## Environment Variables

- `MICROSCOPE_BIN`: Explicit path to `microscope-mem.exe` binary.
- `MICROSCOPE_CONFIG`: Explicit path to `config.toml`.
- `MICROSCOPE_MEMORY_ENABLED`: `1` to enable (default), `0` to disable.
- `MICROSCOPE_MAX_HITS`: Maximum retrieved memory chunks to inject (default: `4`).
- `MICROSCOPE_REBUILD_EVERY`: Stores between background index rebuilds (default: `20`).
