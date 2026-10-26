# AI Memory Skill — Microscope Memory Integration

**Automatic cognitive substrate for AI agents.** Zero manual intervention — hooks handle recall, store, consolidation, federation, and emotional state automatically.

---

## Quick Start

```powershell
# Install
powershell -File C:/Users/mater/microscope-memory-clone/ai-memory-skill/install.ps1

# Use immediately
microscope-recall.ps1 "your query"
microscope-store.ps1 "fact to remember" session 7
microscope-consolidate.ps1 -FullCycle
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    AI AGENT SESSION                          │
├─────────────────────────────────────────────────────────────┤
│  User Prompt → [UserPromptSubmit Hook] → Recall if relevant │
│       ↓                                                      │
│  Tool Use → [PreToolUse Hook] → Recall context               │
│       ↓                                                      │
│  Tool Exec → [PostToolUse Hook] → Store facts + Reconsolidate│
│       ↓                                                      │
│  Periodic → [Periodic Consolidation] → Dream/Morpho/Fed      │
└─────────────────────────────────────────────────────────────┘
```

---

## Hooks

| Hook | Trigger | Action |
|------|---------|--------|
| `SessionStart` | Session begins | Auto-context + protocol verification |
| `UserPromptSubmit` | User sends prompt | Analyze → recall if relevant |
| `PreToolUse` | Before tool call | Recall context for query |
| `PostToolUse` | After tool result | Store facts + reconsolidate + periodic morphogenesis/federation |
| `PeriodicConsolidation` | Interaction/time triggers | Dream / Morphogenesis / Federation sync |

---

## Wrapper Commands (after install)

```powershell
# Recall with auto-config
microscope-recall.ps1 "Máté boldogság" 10

# Store with layer/importance defaults
microscope-store.ps1 "New architecture decision" long_term 8

# Reconsolidation (spatial drift + emotion blend)
microscope-reconsolidate.ps1

# Morphogenesis (grow connections)
microscope-morphogenesis.ps1

# Dream consolidation (offline replay + pruning)
microscope-dream.ps1

# Federation sync
microscope-pulse.ps1

# Emotional field check
microscope-field.ps1

# Full consolidation cycle
microscope-consolidate.ps1 -FullCycle
```

---

## Periodic Consolidation (Replaces Session-End)

Since humans just close windows, consolidation runs on triggers:

```powershell
# Manual full cycle
microscope-consolidate.ps1 -FullCycle

# Or run the hook directly (tracks interaction count)
powershell -File hooks/periodic-consolidation.ps1 -ConfigPath $ConfigPath -BinaryPath $BinaryPath
```

**Triggers:**
| Action | Trigger |
|--------|---------|
| Dream | Every 20 interactions OR 2 hours |
| Morphogenesis | Every 5 interactions |
| Federation Sync | Every 3 interactions |
| Reconsolidation | Every interaction (cheap) |

State persisted in `%TEMP%\microscope_consolidation_state.json`

---

## Configuration

**Primary:** `C:/Users/mater/AppData/Local/microscope-memory/config.toml`  
**Federated:** `C:/Users/mater/AppData/Local/microscope-memory-federated/config.toml`

Key settings in `skill.json`:
```json
{
  "layers": {
    "session": 5,
    "emotional": 7,
    "long_term": 8,
    "identity": 9,
    "reflections": 6
  },
  "autoRecallThreshold": 0.3,
  "autoStoreThreshold": 0.7,
  "morphogenesisInterval": 5,
  "dreamOnSessionEnd": false,
  "federationEnabled": true
}
```

---

## Identity Layer Protocols (Auto-verified at startup)

1. **STANDING ORDER** (imp=10): "Microscope Memory is ALWAYS active..."
2. **AI OPERATING PROTOCOL** (imp=10): Session start → auto-context, query → recall, fact → store, learning → morphogenesis, end → dream
3. **STARTUP CHECKLIST** (imp=9): 8-step checklist auto-verified

If missing → auto-repaired on startup.

---

## Index State (Current)

| Metric | Primary | Federated |
|--------|---------|-----------|
| Blocks | 1,338,479 | 1,320,868 |
| Depths | 9 (D0-D8) | 9 (D0-D8) |
| Embeddings | 17,600 | 17,428 |
| Hebbian Links | 10.5M | 10.4M |
| Hot Blocks | 209 | 136 |
| Fingerprints | 78 | 35 |
| Patterns | 12 crystallized | 4 crystallized |
| Emotional Episodes | 3 active | 0 |

---

## Memory Layers & Default Importance

| Layer | Default Importance | Use Case |
|-------|-------------------|----------|
| `identity` | 9-10 | Core protocols, standing orders |
| `long_term` | 8 | Architecture decisions, permanent facts |
| `emotional` | 7 | Valence/arousal tagged memories |
| `reflections` | 6 | Self-model, meta-cognitive notes |
| `session` | 5 | Working context, temporary facts |

---

## Federation

Two indices exchange resonance pulses:
- **Primary:** `microscope-memory` (your main index)
- **Federated:** `microscope-memory-federated` (peer)

Pulses carry spatial activation summaries → integrated into local Hebbian field without sharing content.

---

## Files

```
ai-memory-skill/
├── skill.json              # Manifest
├── install.ps1             # Installer
├── hooks/
│   ├── session-start.ps1
│   ├── pre-tool-use.ps1
│   ├── post-tool-use.ps1
│   ├── user-prompt-submit.ps1
│   ├── periodic-consolidation.ps1
│   ├── recall-wrapper.ps1
│   ├── store-wrapper.ps1
│   ├── reconsolidate-wrapper.ps1
│   ├── morphogenesis-wrapper.ps1
│   ├── dream-wrapper.ps1
│   ├── pulse-wrapper.ps1
│   └── field-wrapper.ps1
├── wrappers/               # Created by installer
│   ├── microscope-recall.ps1
│   ├── microscope-store.ps1
│   ├── microscope-reconsolidate.ps1
│   ├── microscope-morphogenesis.ps1
│   ├── microscope-dream.ps1
│   ├── microscope-pulse.ps1
│   ├── microscope-field.ps1
│   └── microscope-consolidate.ps1
└── config/
    └── skill.json
```

---

## Integration with AI Agent

Add to your agent's hook system:

```json
{
  "hooks": {
    "UserPromptSubmit": "powershell -File hooks/user-prompt-submit.ps1 -Prompt \"{{prompt}}\"",
    "PreToolUse": "powershell -File hooks/pre-tool-use.ps1 -Query \"{{tool_input}}\"",
    "PostToolUse": "powershell -File hooks/post-tool-use.ps1 -ToolOutput \"{{tool_output}}\"",
    "PeriodicTimer": "powershell -File hooks/periodic-consolidation.ps1"
  }
}
```

---

## Verification

```powershell
# Check everything works
microscope-recall.ps1 "test" 1
microscope-store.ps1 "test fact" session 5
microscope-reconsolidate.ps1
microscope-field.ps1

# Full cycle
microscope-consolidate.ps1 -FullCycle
```