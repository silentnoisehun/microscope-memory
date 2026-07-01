# Microscope Hooks — MCP Integration

## Overview

The Hook System is a thin orchestration layer around the existing Microscope Memory API.
It allows Codex / LLM agents to trigger memory actions automatically at key lifecycle points.

**Principle:**
- The model is only the motor.
- Hooks are the nervous system.
- Microscope Memory is the memory.

## Lifecycle

### on_session_start

Runs when an MCP session starts (on first `initialize` message).

**Tasks:**
- Load D0 identity context
- Load memory contract
- Load active constraints
- Prepare session context

**Default behavior:**
```rust
pub fn default_on_session_start(mut ctx: HookContext) -> HookContext {
    ctx.memory_contract = Some(include_str!("contract.txt").to_string());
    ctx.constraints.push("Never store secrets or API keys".to_string());
    ctx.constraints.push("Memory is authoritative - never invent memories".to_string());
    ctx
}
```

### before_tool_call

Runs before each MCP tool execution.

**Tasks:**
- Retrieve task-related memory
- Check known constraints
- Add project context
- Prepare context for tool execution

**Default behavior:**
```rust
pub fn default_before_tool_call(mut ctx: HookContext) -> HookContext {
    if let Some(project) = &ctx.project_context {
        ctx.metadata.insert("project_context_active", "true");
    }
    ctx
}
```

### after_tool_call

Runs after each successful MCP tool execution.

**Tasks:**
- Summarize tool result
- Detect useful new facts
- Prepare memory candidate for storage
- Associate result with task/project

**Default behavior:**
```rust
pub fn default_after_tool_call(mut ctx: HookContext) -> HookContext {
    if let Some(result) = &ctx.tool_result {
        if !result.is_empty() && result.len() > 20 {
            ctx.memory_candidates.push(MemoryCandidate {
                text: format!("[Tool: {}] {}", tool, result),
                layer: "session".to_string(),
                importance: 4,
                // ...
            });
        }
    }
    ctx
}
```

### on_error

Runs when a tool execution fails.

**Tasks:**
- Store error trace if useful
- Associate failure with task/project
- Expose recovery context next time

**Default behavior:**
```rust
pub fn default_on_error(mut ctx: HookContext) -> HookContext {
    if let (Some(msg), Some(code)) = (&ctx.error_message, &ctx.error_code) {
        ctx.memory_candidates.push(MemoryCandidate {
            text: format!("ERROR [{}]: {}", code, msg),
            layer: "session".to_string(),
            importance: 7,
            is_error: true,
            is_task: true,
            // ...
        });
    }
    ctx
}
```

## Configuration

### Read-Only Mode (Public Demo)

Default for all MCP deployments. Safe for public use.

```toml
[hooks]
enabled = true
read_only = true
write_enabled = false
min_importance = 3
```

**Guarantees:**
- No memory writes from hooks
- No after_response hook execution
- All memory candidates are discarded
- Secret filtering always active

### Full Mode (Local Development)

Requires explicit opt-in. Never use on public endpoints.

```toml
[hooks]
enabled = true
read_only = false
write_enabled = true
min_importance = 5
```

**Caveats:**
- after_response hook is active
- Memory candidates are created and stored
- Requires user confirmation by default
- Secret filtering still applies

## Security

### Secret Filtering

Before any memory candidate is created, the hook system applies two filters:

1. **contains_secrets()** — blocks text containing:
   - `password`, `passwd`, `pwd`, `secret`, `token`, `credential`
   - `auth_token`, `bearer`, `private_key`, `-----begin`

2. **contains_api_key()** — blocks text containing:
   - `api_key`, `apikey`, `api-key`, `sk-`, `pk-`, `openai_key`

### Write Protection

- `write_enabled = false` by default
- `after_response` hook disabled by default
- All memory candidates cleared if write is disabled
- Importance threshold enforced (default: 3)
- Maximum candidates capped (default: 5)

### Logging

All hook execution is logged to **stderr only** (never stdout):

```
[hooks] manager initialized (read_only=true, write_enabled=false)
[hooks] session started
[hooks] before_tool_call: memory_recall
[hooks] after_tool_call: 1 candidate(s) from 'memory_recall'
[hooks] on_error: stored error trace for 'memory_store'
```

## Architecture

```
microscope-core
    |
    v
microscope-hooks (HookManager)
    |
    +-- on_session_start  --> identity, contract, constraints
    +-- before_tool_call  --> task context, constraints check
    +-- after_tool_call   --> memory candidate generation
    +-- on_error          --> error trace storage
    |
    v
MCP server / WASM / Codex integration
```

## Custom Handlers

You can register custom hook handlers:

```rust
use microscope_hooks::*;

struct MyHandler;
impl HookHandler for MyHandler {
    fn event(&self) -> HookEvent { HookEvent::BeforeToolCall }
    fn execute(&self, mut ctx: HookContext) -> HookContext {
        ctx.metadata.insert("custom", "value".to_string());
        ctx
    }
}

let mut manager = HookManager::new(config);
manager.register(Box::new(MyHandler));
```

## Test Coverage

- microscope-memory: 253 tests
- microscope-hooks: 16 tests
- Total: 269 tests passing
