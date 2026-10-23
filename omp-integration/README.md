# Microscope Memory -> omp (oh-my-pi) integration

Wires the Microscope Memory engine into an `omp` session so that memory works
automatically: the session starts with the standing context, every prompt looks
up what it is about, every exchange is persisted, and the session leaves a
digest behind.

This document first describes **how the engine actually works** (verified in
the source and measured on this machine), because the integration is built on
those facts.

---

## 1. How the engine works

### 1.1 Storage model

The engine is a pure-binary, mmap-backed store. Three different files hold
data, and they are **not** equivalent:

| artifact | role | durability |
|---|---|---|
| `data/append.bin` | **the real store** - every new memory is appended here | durable, protected by a `FileLock` |
| `layers/<name>.txt` | human-readable mirror of each layer | **best effort** - a failure here is only a `WARN` on stderr, the store still reports `STORED` and exits 0 |
| `data/timeline.bin` | chronological log used by `timeline` and `auto-context` | durable |
| `data/microscope.bin` + headers | the zoom-indexed binary index (9 depths, ~1.7M blocks) | rebuilt from the layer files + append log |

**Write path** (`store`):

```
append.bin (locked)  ->  layers/<layer>.txt (best effort)  ->  timeline.bin
                      ->  maybe_auto_rebuild()
```

`maybe_auto_rebuild` rebuilds the binary index when the append log passes
`index.auto_rebuild_entries`. The rebuild reads the **raw layer files** plus
the append log and then **deletes the append log**.

**Read path**: `find` and `recall` search **both** the binary index and the
append log, so a memory is retrievable immediately after `store` - long
before the index catches up.

### 1.2 What the commands are for

| command | use it for | do not use it for |
|---|---|---|
| `auto-context` | the session snapshot: state, last session timeline, open loops, core memories (importance >= 7) | - |
| `find` | **deterministic phrase lookup** - the reliable retrieval primitive | single words (see 1.4) |
| `recall` | semantic recall, topically fuzzy questions | auto-injection (see 1.4) |
| `store` | persisting a memory | - |
| `session-context` | the engine's own session digest | - |
| `timeline` / `loops` | what happened, what is still open | - |
| `stats` / `status` | diagnostics | - |
| `rebuild` | consolidating the append log into the index | inside a hook handler (slow) |
| `mcp` | the universal transport - 54 `memory_*` tools over JSON-RPC | - |

### 1.3 The MCP server is the integration surface

`microscope-mem mcp` speaks MCP over stdio and exposes 54 tools
(`memory_recall`, `memory_store`, `memory_auto_context`, `memory_find`,
`memory_session_context`, `memory_timeline`, `memory_loops`, `memory_status`,
`memory_rebuild`, ...). It is the engine's own recommendation for "any LLM
that supports MCP".

The CLI's stdout was rejected as a transport: it mixes results with ANSI
colouring and stderr diagnostics, and the header line goes to stderr. The MCP
tools return clean structured text:

```
Text search 'Standalone Rust Octopus runtime': 3 results

[D2 long_term] [long_term #10] Standalone Rust Octopus runtime lezárva | ...
```

### 1.4 Measured behaviour that shaped the bridge

These were measured against the real indexes on this machine, not assumed.

| observation | consequence for the bridge |
|---|---|
| `find` prints `L2` = `sqrt(relevance)` (higher = better). The August build in `D:\codex` prints a spatial distance instead, so every hit shows `0.00000`. | the bridge uses the **current build** from this repo; relevance ranking is only meaningful with it |
| `memory_ping` returned hits scored `0.000-0.001` on completely unrelated entries | `ping` is **not** used for retrieval |
| `recall` ranks poorly and appends `NARRATIVE` / `SELF` / `CURIOUS` sections | `recall` is left to the model via MCP, never auto-injected |
| a one-word query returns the bare token block (`Octopus`) instead of the entry carrying the information | lookups use **multi-word phrases** extracted from the prompt |
| `auto-context` takes the core memories from the **append log** | freshly stored high-importance memories show up in the next session - this is the continuity mechanism |
| the engine's hook manager starts read-only (`read_only=true, write_enabled=false`) | writes happen because the bridge calls `memory_store` explicitly |
| a full `rebuild` of the 1.7M-block index takes ~14-20 s | the bridge never triggers one inside a hook handler |

---

## 2. The integration

```
omp session
  |
  +-- ~/.omp/agent/hooks/pre/microscope-memory.ts   (the automatic wiring)
  |     session_start      -> auto-context + STANDING ORDER + SESSION CONTEXT
  |                           + the open loops
  |     before_agent_start -> look up what this prompt is about (memory_find)
  |     context            -> inject the memory into the model call
  |     tool_result        -> extract facts from the tool output and store
  |                           them with the engine's layer/importance mapping
  |     agent_end          -> store the exchange (memory_store)
  |     session_stop       -> same, de-duplicated
  |     session_shutdown   -> store the session digest (memory_session_context)
  |
  +-- ~/.omp/agent/mcp.json                          (the model facing tools)
        omp starts its own `microscope-mem mcp` -> 54 memory_* tools
```

### 2.0 The full lifecycle

The hook mirrors the engine's own agent lifecycle
(`ai-memory-skill/hooks/*.ps1`):

| stage | engine hook | omp event | what the bridge does |
|---|---|---|---|
| session start | `session-start.ps1` | `session_start` | `memory_auto_context` + `memory_find "STANDING ORDER"` + `memory_find "SESSION CONTEXT"` + `memory_loops` + `memory_status` |
| prompt | `user-prompt-submit.ps1` | `before_agent_start` | `memory_find` on the prompt's phrases (sequential); `memory_recall` when the engine has real embeddings |
| tool use | `post-tool-use.ps1` | `tool_result` | pattern based fact extraction -> `memory_store` (session/5, reflections/7, long_term/8) |
| answer done | - | `message_end` | `memory_store` of the exchange (session, importance 5) |
| turn end | `session-end.ps1` | `agent_end`, `session_stop` | the same store as a fallback (the content hash prevents a double store) |
| every 5 turns | `periodic-consolidation.ps1` | `session_stop` | `memory_session_context` digest (importance 7) |
| every 20 turns | `periodic-consolidation.ps1` | `session_stop` | `memory_consolidate` (the session's learnings become a long_term summary) |
| session end | `session-end.ps1` | `session_shutdown` | a best-effort final digest - **it usually never fires** |

**Two measured facts shaped the event choice.**

*People quit, they do not wait for the session to end.* Measured on an
interrupted session, only `session_start`, `before_agent_start`, `context` and
`message_end` fire - `agent_end`, `session_stop`, `turn_end` and
`session_shutdown` do not. So the exchange is stored when the assistant's
answer completes (`message_end`), and the session level work runs on a turn
counter kept in a state file, exactly like the engine's own
`periodic-consolidation.ps1` ("not session-end which never fires for
humans").

*The lookups must be sequential.* The engine maps the index file, and several
concurrent readers of the same mapping collide on Windows (the engine's own
tests document "creating microscope.bin fails while a user-mapped section is
still open"). Measured: six parallel `memory_find` calls returned nothing, the
same six sequential ones returned their hits. A find takes 4-16 ms, so
sequential is cheaper than a tenth of a second.

**The split between the hook and the model.** The engine exposes **54**
`memory_*` tools. The bridge uses a handful of them for the part that must
happen without the model asking, and the model gets **all 54** through the MCP
server in `~/.omp/agent/mcp.json`:

| used by the hook automatically | used by the model on demand |
|---|---|
| `memory_auto_context`, `memory_find`, `memory_recall`, `memory_loops`, `memory_store`, `memory_status`, `memory_session_context`, `memory_consolidate` | `memory_think`, `memory_mql_query`, `memory_resolve_loop`, `memory_dream`, `memory_evidence_*`, `memory_introspect`, `memory_self_model`, `memory_curiosity`, `memory_daydream`, `memory_monologue`, `memory_stories`, `memory_hyperfocus`, `memory_autonomous`, `memory_consciousness`, `memory_hebbian`, `memory_archetypes`, `memory_patterns`, `memory_radial`, `memory_look`, `memory_embed`, `memory_similar`, `memory_links`, `memory_fingerprint`, `memory_resonance`, `memory_mirror`, `memory_predictions`, `memory_paths`, `memory_temporal_patterns`, `memory_modalities`, `memory_emotional_field`, `memory_timeline`, `memory_session_log`, `memory_store_data`, `memory_ping`, `memory_build`, `memory_rebuild`, `memory_doctor`, `memory_resonant` |

So the split is deliberate: **the hook owns the automatic part** (it must not
depend on the model remembering to call it), **the model owns the deliberate
part** (thinking chains, MQL, the epistemic layer, the open loops, the
self-model).

### 2.0.1 The open loops are the model's job

The engine has an "unfinished work" layer (`memory_loops`, `memory_resolve_loop`,
and `store --status open`). The bridge **does not decide what is unfinished** -
a keyword heuristic on the user's prompt is exactly the wrong kind of
automation. The model writes the loop itself:

* `memory_store` now accepts a `status` parameter (`normal | open | resolved |
  archived`) - an engine change in `src/mcp.rs`, so the model can open a loop
  with the tool it already has;
* `memory_resolve_loop` closes it.

The hook only makes sure the open loops are **visible**: they are part of the
session context, so the next session starts with the unfinished work in front
of the model.

### 2.1 Why the `context` event

`before_agent_start` would persist its message into the session, so every turn
would grow the transcript with a copy of the snapshot. The `context` event
changes **only the model call it is attached to**, so the snapshot is always
fresh and the session stays clean.

The message shape was verified against a live session:

```js
{ role: "custom", customType: "microscope-memory", content: "...",
  display: false, attribution: "agent", timestamp: 1792755351063 }
```

### 2.2 Semantic recall only with real embeddings

The engine's embedding provider decides whether the semantic path means
anything:

* `provider = "mock"` (the default): the recall results are scored
  `unknown`, and measured queries returned unrelated entries. The bridge
  therefore keeps the semantic lookup **off** and uses the deterministic
  `memory_find` (4-16 ms, exact) instead.
* a real provider (`candle`, `python`, `bge-small`): the bridge detects it
  from `memory_status` and adds the semantic recall for explicit memory
  questions.

The bridge never guesses: it reads the provider from the running engine.

**Real embeddings, measured.** The engine was built with
`--features embeddings` and the instance switched to the `candle` provider
(`model = sentence-transformers/all-MiniLM-L6-v2`, `max_depth = 3`, so the
embedding index covers the meaningful D0-D3 blocks):

```
Embedding quality gate: -25 short (<20 chars), -0 unencodable, -3 mojibake
Embedding up to 5217 blocks (D0-D3, dim=384)
embeddings.bin: 7845.9 KB (5217 stored vectors of 1359700 blocks, dim 384)
```

The difference in the recall, on the same index:

| query | with the mock provider | with the candle provider |
|---|---|---|
| "hogyan működik a memória motor" | unrelated session rows | **`microscope-memory/ # Perzisztens memória motor`** |
| "octopus contract pipe fields" | unrelated rows | **`STANDING ORDER Octopus contract format learned 2026-10-23: blades require pipe-separated contract fields`** |
| latency | 1239 ms | **308-382 ms** |

So the semantic path is not only better, it is also faster than the mock.

The model is already in the HuggingFace cache
(`~/.cache/huggingface/hub/models--sentence-transformers--all-MiniLM-L6-v2`),
so nothing had to be downloaded - only built. The `python` provider was
measured as too slow for the interactive path (22.7 s model load, 205 ms per
embedding), the `candle` provider is Rust and needs ~20 ms.

### 2.3 The bridge config follows the canonical one

The bridge needs a config of its own: with `auto_rebuild = true` a `store`
can start a ~20 s index rebuild inside a hook handler. The bridge config is
therefore the canonical config with `auto_rebuild = false`, and the hook
**regenerates it at every session start** - a changed embedding provider must
never stay invisible to the bridge.

The source is a fixed path, never the ambient `MICROSCOPE_CONFIG`: a process
can carry a stale environment, and following it would silently point the
bridge at a different memory instance.

### 2.2 Why the bridge owns its own memory

The machine has **two instances** of the engine:

| instance | binary | data | role |
|---|---|---|---|
| `C:\Users\mater\microscope-memory-clone` | this repo, current build | `%LOCALAPPDATA%\microscope-memory` | **the current engine** |
| `D:\codex\microscope-memory` | August build | `D:\codex\microscope-memory\data` | the previous generation, shared by the other tools, holds the real project history |

The machine level environment variables (`MICROSCOPE_CONFIG`, `MICROSCOPE_BIN`)
now point at **this repo** (the new engine and its own instance), which is also
what the scheduled tasks intend:

| consumer | binary | config |
|---|---|---|
| `MICROSCOPE_BIN` / `MICROSCOPE_CONFIG` (User scope) | this repo | `%LOCALAPPDATA%\microscope-memory` |
| `MicroscopeMemoryDaemon` (Logon trigger) | this repo | `%LOCALAPPDATA%\microscope-memory` |
| `MicroscopeNightlyMaintenance` | this repo | `%LOCALAPPDATA%\microscope-memory` |
| `~/.microscope/microscope-recall-hook.ps1` (Claude Code) | resolved from `MICROSCOPE_BIN` | from `MICROSCOPE_CONFIG` |

The old `D:\codex\microscope-memory` instance is the previous generation and
its test data. It is a separate directory with a separate index, and nothing
writes to it any more.

The bridge additionally uses its own config
(`~/.omp/agent/microscope-memory-bridge.toml`, this repo's config with
`auto_rebuild = false`) so that a `store` can never start a ~20 s index
rebuild inside a hook handler. Override the instance with
`MICROSCOPE_BRIDGE_CONFIG`, the binary with `MICROSCOPE_BRIDGE_BIN`.

### 2.3 Commands

| command | effect |
|---|---|
| `/mscope` | show the current auto-context snapshot |
| `/mscope-find <phrase>` | deterministic phrase search |
| `/mscope-store <text>` | store a memory (long_term, importance 7) |
| `/mscope-status` | bridge + engine diagnostics |
| `/mscope-rebuild` | consolidate the append log into the index (slow, explicit) |

---

## 3. Install

```powershell
powershell -ExecutionPolicy Bypass -File omp-integration\install.ps1
```

It installs the hook, merges the MCP server into `~/.omp/agent/mcp.json`,
generates the bridge config and disables the earlier session-start-only
extension. `-Uninstall` reverses all of it.

Other scripts in this folder:

| script | what it does |
|---|---|
| `enable-embeddings.ps1` | build the engine with the `embeddings` feature, switch the instance to the real provider and rebuild the embedding index (kill before build is part of it) |
| `restart-bridge-daemon.ps1` | restart the bridge daemon on port 6060 (`-Stop` only stops it) |
| `cleanup-canonical.ps1` | remove the bridge's own test entries from a layer file (with a backup) |

**The daemon and the build fight over the binary.** A running engine holds
`target/release/microscope-mem.exe`, and the build then fails with
`failed to remove file ... os error 5`. Every script that builds stops the
daemon first.

## 4. Verify

```powershell
omp -p "/extensions"          # the hook is listed
omp -p "/mcp list"            # the microscope server is connected
omp -p "/mscope-status"       # bridge and engine diagnostics
```

Trace file: `C:\Users\mater\.omp\agent\microscope-memory.log`

Every bridge decision is traced there, because the engine's own log does not
always flush the last lines before the process exits - a working store used to
look like a silent failure.

## 5. Known limits

* The hook's slash commands are TUI-only. In `omp -p` a leading `/` is passed
  to the model as a prompt.
* `memory_find` matches phrases, so a prompt that shares no phrase with the
  stored memories retrieves nothing. This is deliberate: silence beats noise.
* The append log grows until the engine consolidates it. `/mscope-rebuild`
  does that on demand.
* The model-facing MCP server runs with the bridge config, and the machine
  level `MICROSCOPE_CONFIG` points at the same instance, so a CLI call from
  the model lands in the same memory as the bridge.
* The `MicroscopeMemoryDaemon` scheduled task runs the bridge on port 6060
  with **elevated privileges** and its command contains a broken assignment
  (`D:\codex\microscope-memory\config.toml='...'`, which PowerShell rejects).
  The assignment is a no-op, so the task falls back to the environment
  variable - which now points at this repo. Until the task is restarted with
  elevation, the already running daemon still serves the old instance.

## 6. Two engine bugs that were found and fixed

Both were measured on this machine and are fixed in `src/build.rs`:

1. **The shrink guard did not protect anything.** The rebuild wrote the new
   index first and only then compared the block counts, so a refusal
   (`refusing rebuild shrink: 1729506 -> 1709541 blocks` + panic) happened
   *after* the index had already been swapped. The guard now runs inside
   `build()`, before any index file is written, so a refusal keeps the
   previous index intact.
2. **The retention cap and the rebuild contradicted each other.** The mirror
   (`layers/*.txt`) is the build source and the cap trims it, so a rebuild
   always produced a smaller index, which tripped the guard every time and
   left the append log uncleared (50 -> 193 entries). The engine now has two
   build paths:
   * `build()` - a plain rebuild. It may shrink: the retention shift of a
     layer file is a legitimate rebuild.
   * `build_consolidating()` - the path `rebuild_pending` uses. It merges the
     pending append-log entries into the build (the mirror write is best
     effort, so an entry whose mirror write failed would otherwise be lost)
     and refuses a shrink **before any index file is written**. With
     `layer_retention_entries` the mirror is trimmed by policy, so that
     shrink is a warning, not a refusal.

   The merge is consolidation-only on purpose: a plain build does not clear
   the append log, so its entries may already be consolidated, and re-adding
   them would resurrect the content the cap trimmed.

Measured after the fix, on the real index:

```
before:  Blocks 1343308,  Append log: 18 entries
rebuild: exit=0 in 16.6 s
         1349929 blocks total
         Append log cleared after consolidating 18 entries
after:   Blocks 1349929,  no append log
```

No panic, no lost entry, no duplicated entry - the merge skips what the
mirror already holds.
