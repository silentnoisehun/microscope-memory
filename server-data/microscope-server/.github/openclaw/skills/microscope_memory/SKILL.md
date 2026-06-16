---
name: microscope_memory
description: Connect OpenClaw to a Microscope-Memory API for status, recall, and remember operations.
metadata:
  openclaw:
    requires:
      bins: ["bash", "curl"]
---

# Microscope Memory Skill

Use this skill when the user asks to:
- recall earlier facts/preferences/context,
- persist important new facts,
- inspect memory engine status.

## Required environment

Expect these env vars to be available in the OpenClaw runtime:
- `MICROSCOPE_BASE_URL` (example: `https://memory.example.com`)
- `MICROSCOPE_USER`
- `MICROSCOPE_PASS`

Helper script path:
`~/.openclaw/workspace/tools/microscope-memory-api.sh`

## Commands

1. Status:
```bash
bash ~/.openclaw/workspace/tools/microscope-memory-api.sh status
```

2. Recall:
```bash
bash ~/.openclaw/workspace/tools/microscope-memory-api.sh recall "<query>" 10
```

3. Remember:
```bash
bash ~/.openclaw/workspace/tools/microscope-memory-api.sh remember "<text>" long_term 7
```

## Behavior rules

- Always run `recall` before answering memory-dependent questions.
- If `recall` returns useful context, cite it briefly in your answer.
- Before `remember`, confirm that the new fact is durable and worth storing.
- Default `layer` is `long_term`; use `emotional` for user preferences and tone.
- If API call fails, report the error and suggest checking credentials or endpoint.