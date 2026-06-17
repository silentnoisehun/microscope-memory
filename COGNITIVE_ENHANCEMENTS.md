# Microscope Memory — Cognitive Modules v0.8.0

## Morphogenesis (`morphogenesis.rs`)
Biological pattern-inspired architecture generation:
- **Mycelium** — fungal network growth for P2P/distributed topologies
- **Capillary** — fractal branching for hierarchical cache/dataflow pipelines
- **Slime Mold** — Physarum-inspired optimal route finding (CDN, routing, mesh)
- **Fractal L-System** — self-similar structure cultivation (microservice trees)
- Evolution engine: genetic algorithm over growth parameters with fitness scoring

## Pattern Recognition (`pattern_recognition.rs`)
Multi-domain pattern detection engine:
- **Sequence** — recurring thought/recall pathway detection (GSP-like)
- **Temporal** — daily/weekly activity rhythm mining
- **Structural** — graph motif detection (fan-in, fan-out, hub nodes)
- **Cluster** — DBSCAN-based spatial grouping in memory space
- **Cross-domain** — pattern correlation across different layers

## Executive (`executive.rs`)
Cognitive conductor — module orchestration and resource management:
- Module registration with priority and energy cost
- Schedule → execute cycle with context switching
- Homeostasis: critical energy handling, module suspension
- Vagus-aware module recommendation (stress → stress-handler priority)

## Planning (`planning.rs`)
Hierarchical goal decomposition and action planning:
- HTN decomposition: goals → subgoals
- Action plan creation with cost, duration, risk estimation
- Step-by-step execution with progress tracking
- Replanning on changing conditions

## Autopoiesis (`autopoiesis.rs`)
Self-modifying code system:
- Template-based code generation (variable interpolation)
- Versioned mutations with rollback support
- Integration with planning → automated fix proposals

## Code Memory (`code_memory.rs`)
Dedicated memory layer for coding agents:
- Code snippet and symbol storage
- Error → solution pair tracking
- Project-level memory with recall by symbol, project, type
- CLI integration: `microscope-mem code --store`, `--recall`, `--error`

## ChatGPT Import (`chatgpt.rs`)
ChatGPT conversation history import:
- Parse conversations.json export
- Google Drive file/folder import (--gdrive, --gdrive-folder)
- Dry-run analysis mode (--dry-run)
- Automatic layer assignment by role (user/assistant)

## PWA Chat
Installable web application with:
- Chat interface connected to local LLM (Ollama)
- Bridge API integration for memory store/recall
- Service worker for offline capability
- Phone access via local network
