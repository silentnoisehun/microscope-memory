# Universal AI Agent Memory Hook

Universal, high-performance cognitive memory bridge for AI coding agents (**Claude Code**, **MiniMax Code**, **Codex CLI**, **Goose**, **Antigravity**, **OMP**).

## Why this Hook?

- **Native CLI:** Calls the configured `microscope-mem` binary directly. Latency depends on the machine, model and index.
- **Resilient JSON Parsing:** Implements multi-tier JSON sanitization fixing Windows unescaped path backslashes (e.g. `C:\Users\...` escaping into illegal escape sequences like `\U`), completely eliminating `Bad escaped character in JSON` crashes.
- **Query-aware recall:** Sends the full prompt to `recall` once. A configured semantic index can supply paraphrase candidates; degraded search is labelled.
- **Detached Index Rebuilding:** Auto-rebuild is uncoupled from hook turns; costly folding runs in a detached child process without blocking the agent.
- **Non-blocking failure contract:** Hook handlers exit `0`; retrieval failures are explicitly surfaced as unavailable context, not confused with an empty result. The detached rebuild worker has no fixed time limit.

## Architecture & Lifecycle Events

```
SessionStart       ──▶ Auto-Context Injection (Core Memories imp>=7, Open Loops, Timeline)
UserPromptSubmit   ──▶ Full-question recall (unverified item candidates; read-only)
PostToolUse        ──▶ Automatic Memory Storage (Code/Session layer, Importance 5/6)
Rebuild Trigger    ──▶ Background locked rebuild (every 20 successful stores)
Stop               ──▶ Logging only (no automatic digest or loop resolution)
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

## Safety and retrieval limits

Retrieved candidates are not verified answers: the hook cannot reliably prove
that a topic is absent and does not resolve open loops automatically. It omits
D0-D2 derived summaries, which can contain old versions of corrected facts;
the selected D3 items and pending records should still be checked against the
question. The 2,400-character context budget may clip content.

The hook uses a separate config copy with `auto_rebuild = false`, then runs the
locked `rebuild` command in the background every configured number of successful
stores. This does not make rebuilding crash-atomic. After a crash a leftover
`microscope.lock` requires manual inspection; remove it only after confirming
that no writer or rebuild process is running. Use separate data directories for
projects until end-to-end project isolation has been validated.

Tests: `node --test integrations/ai-agent-hook/test/hook.test.mjs` uses a fake
CLI for wiring; set `MICROSCOPE_TEST_BIN` to also exercise a real native CLI.
The test does not establish real-model semantic accuracy.
