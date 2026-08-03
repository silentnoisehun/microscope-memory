# Codex Hooks — the integration the Codex agent actually runs

This is the practical, Codex-specific wiring of Microscope Memory as the
persistent memory of a coding agent. It is the layer a Codex/CLI session uses
day-to-day: **auto-context before every response**, **auto-store after every
interaction**, and the **MCP memory tools**. For the hook-manager API
(`on_session_start`, `before_tool_call`, …) see [`HOOKS.md`](./HOOKS.md).

> The model is only the motor.
> Hooks are the nervous system.
> Microscope Memory is the memory.

## What gets wired

```text
Codex session (AGENTS.md instructions)
    |
    +-- before each response:  microscope-mem auto-context        (relevant context)
    +-- after each interaction: microscope-mem store               (durable memory)
    +-- optional MCP server:    microscope-mem mcp                 (memory_* tools)
    |
    v
Microscope Memory core (mmap index, seqlock, Merkle-integrity)
```

Everything is configured with one environment variable:

```powershell
$env:MICROSCOPE_CONFIG = "D:\codex\microscope-memory\config.toml"
```

## Lifecycle (exactly as Codex runs it)

### Before every response — `auto-context`

Runs automatically before replying; the result informs the next answer.

```powershell
& "D:\codex\microscope-memory\target\release\microscope-mem.exe" auto-context 2>&1 | Select-Object -Last 30
```

It returns:
* current memory state (blocks/active depths),
* last session timeline,
* core memories (importance >= 7),
* open loops.

### After every interaction — `store`

Stores the user message + the assistant response as a single memory entry in
the `session` layer, with **escalating importance**:

| score | kind |
|-------|------|
| 5 | routine interaction |
| 6 | decision / fix |
| 7 | milestone / release |
| 8 | identity / vision |

```powershell
& "D:\codex\microscope-memory\target\release\microscope-mem.exe" store "<message + response>" -l session -i 5
```

The core memory shown by `auto-context` is built only from entries with
importance >= 7, so high-importance items are never flattened.

### MCP memory tools (optional)

Start the Model Context Protocol server, then the agent can call
`memory_recall`, `memory_store`, `memory_status` and `memory_dream`:

```json
{
  "mcpServers": {
    "microscope": {
      "command": "D:\\codex\\microscope-memory\\target\\release\\microscope-mem.exe",
      "args": ["mcp"],
      "env": { "MICROSCOPE_CONFIG": "D:\\codex\\microscope-memory\\config.toml" }
    }
  }
}
```

## Project wiring — a ready AGENTS.md snippet

```markdown
# Microscope Memory - Auto Context & Store

## Auto-Call (Before Each Response)
$env:MICROSCOPE_CONFIG = "D:\codex\microscope-memory\config.toml"
& "D:\codex\microscope-memory\target\release\microscope-mem.exe" auto-context 2>&1 | Select-Object -Last 30

## Auto-Store (After Each Response)
& "D:\codex\microscope-memory\target\release\microscope-mem.exe" store "<msg + response>" -l session -i 5
```

## Interaction with the rest of the system

- **Self-re-enforcement / commitment gates**: the `enforcement` module
  (`EnforcementEngine::can_execute`, `load_engine_strict`) can guard actions in
  the same core — see `docs/VALIDATION_REPORT.md` and the
  `v0.9.1-commitment-enforcement` tag.
- **Octopus**: the native `octopus-runtime` wires this gate into
  `execute_component` (fail-closed). This file documents the Codex-side
  *session* hooks; the native gate is a separate mechanism.

## Security

- Never store secrets or API keys. `microscope-hooks` applies a secret filter
  (`password`, `token`, `secret`, `api_key`, `sk-`, …).
- Combined write-protection + importance floor (default 3) prevent noise from
  being persisted.
- Logging is on stderr only.
