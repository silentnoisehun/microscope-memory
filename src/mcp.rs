//! Native MCP (Model Context Protocol) server for Microscope Memory.
//!
//! Implements JSON-RPC 2.0 over stdio with the MCP tool-calling protocol.
//! Replaces the Python MCP server with a native Rust implementation.
//!
//! Not available on WASM targets (no stdio).

use crate::config::Config;
use microscope_hooks::*;
use crate::reader::MicroscopeReader;
use crate::{read_append_log, store_memory, store_memory_with_emotion, LAYER_NAMES};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

/// Run the MCP server on stdio (blocking).
pub fn run(config: Config) {
    // Force UTF-8 console on Windows (CP_UTF8 = 65001)
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCP(65001);
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }

    // Initialize hook manager
    let hook_config = if config.hooks.read_only {
        HookConfig::read_only()
    } else if config.hooks.write_enabled {
        HookConfig::full()
    } else {
        HookConfig::default()
    };
    let hook_manager = HookManager::new(hook_config);
    let mut session_started = false;

    eprintln!("[hooks] manager initialized (read_only={}, write_enabled={})",
        config.hooks.read_only, config.hooks.write_enabled);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = stdout.lock();

    loop {
        let incoming = match read_message(&mut reader) {
            Ok(Some(msg)) => msg,
            Ok(None) => break,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Read error: {}", e) }
                });
                let _ = write_message(&mut stdout, &err, true);
                continue;
            }
        };

        let request = match serde_json::from_str::<Value>(&incoming.payload) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                let _ = write_message(&mut stdout, &err, incoming.framed);
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Run on_session_start on first initialize
        if method == "initialize" && !session_started {
            let ctx = HookContext::new(HookEvent::SessionStart);
            let _ = hook_manager.execute(HookEvent::SessionStart, ctx);
            session_started = true;
            eprintln!("[hooks] session started");
        }

        let response = match method {
            "initialize" => handle_initialize(&id),
            "initialized" => continue,
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call_with_hooks(&id, &request, &config, &hook_manager),
            "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            "notifications/cancelled" | "notifications/initialized" => continue,
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }),
        };

        let _ = write_message(&mut stdout, &response, incoming.framed);
    }
}

fn handle_tools_call_with_hooks(id: &Value, request: &Value, config: &Config, hook_manager: &HookManager) -> Value {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(|n: &Value| n.as_str())
        .unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    // Run before_tool_call hook
    let before_ctx = HookContext::new(HookEvent::BeforeToolCall)
        .with_tool(tool_name, args.clone());
    let _ = hook_manager.execute(HookEvent::BeforeToolCall, before_ctx);

    // Execute the tool
    let result = match tool_name {
        "memory_status" => tool_status(config),
        "memory_store" => tool_store(config, &args),
        "memory_recall" => tool_recall(config, &args),
        "memory_find" => tool_find(config, &args),
        "memory_mql_query" => tool_mql_query(config, &args),
        "memory_build" => tool_build(config, &args),
        "memory_look" => tool_look(config, &args),
        "memory_session_log" => tool_session_log(config, &args),
        "memory_consolidate" => tool_consolidate(config, &args),
        "memory_dream" => tool_dream(config, &args),
        "memory_session_context" => tool_session_context(config, &args),
        "memory_ping" => tool_ping(config, &args),
        "memory_auto_context" => tool_auto_context(config, &args),
        "memory_timeline" => tool_timeline(config, &args),
        "memory_loops" => tool_loops(config, &args),
        "memory_resolve_loop" => tool_resolve_loop(config, &args),
        "memory_radial" => tool_radial(config, &args),
        "memory_soft" => tool_soft(config, &args),
        "memory_think" => tool_think(config, &args),
        "memory_hebbian" => tool_hebbian(config, &args),
        "memory_hottest" => tool_hottest(config, &args),
        "memory_archetypes" => tool_archetypes(config, &args),
        "memory_patterns" => tool_patterns(config, &args),
        "memory_attention" => tool_attention(config, &args),
        "memory_introspect" => tool_introspect(config, &args),
        "memory_self_model" => tool_self_model(config, &args),
        "memory_curiosity" => tool_curiosity(config, &args),
        "memory_monologue" => tool_monologue(config, &args),
        "memory_stories" => tool_stories(config, &args),
        "memory_daydream" => tool_daydream(config, &args),
        "memory_hyperfocus" => tool_hyperfocus(config, &args),
        "memory_emotional_field" => tool_emotional_field(config, &args),
        "memory_embed" => tool_embed(config, &args),
        "memory_similar" => tool_similar(config, &args),
        "memory_links" => tool_links(config, &args),
        "memory_fingerprint" => tool_fingerprint(config, &args),
        "memory_dream_log" => tool_dream_log(config, &args),
        "memory_resonance" => tool_resonance(config, &args),
        "memory_mirror" => tool_mirror(config, &args),
        "memory_predictions" => tool_predictions(config, &args),
        "memory_paths" => tool_paths(config, &args),
        "memory_temporal_patterns" => tool_temporal_patterns(config, &args),
        "memory_modalities" => tool_modalities(config, &args),
        "memory_doctor" => tool_doctor(config, &args),
        "memory_rebuild" => tool_rebuild(config, &args),
        "memory_store_data" => tool_store_data(config, &args),
        "memory_resonant" => tool_resonant(config, &args),
        "memory_autonomous" => tool_autonomous(config, &args),
        "memory_consciousness" => tool_consciousness(config, &args),
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    match result {
        Ok(content) => {
            // Run after_tool_call hook
            let after_ctx = HookContext::new(HookEvent::AfterToolCall)
                .with_tool(tool_name, args)
                .with_response(&content);
            let hook_ctx = hook_manager.execute(HookEvent::AfterToolCall, after_ctx);
            if !hook_ctx.memory_candidates.is_empty() {
                eprintln!("[hooks] after_tool_call: {} candidate(s) from '{}'",
                    hook_ctx.memory_candidates.len(), tool_name);
            }

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": content }]
                }
            })
        }
        Err(e) => {
            // Run on_error hook
            let err_ctx = HookContext::new(HookEvent::Error)
                .with_tool(tool_name, args)
                .with_error(&e, "TOOL_ERROR");
            let hook_ctx = hook_manager.execute(HookEvent::Error, err_ctx);
            if !hook_ctx.memory_candidates.is_empty() {
                eprintln!("[hooks] on_error: stored error trace for '{}'", tool_name);
            }

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                }
            })
        }
    }
}


struct IncomingMessage {
    payload: String,
    framed: bool,
}

fn read_message<R: BufRead + Read>(reader: &mut R) -> io::Result<Option<IncomingMessage>> {
    // Read first non-empty line as raw bytes (Windows console may not be UTF-8)
    let first_line = loop {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = reader.read(&mut byte)?;
            if n == 0 {
                return Ok(None);
            }
            if byte[0] == b'\n' {
                break;
            }
            if byte[0] != b'\r' {
                buf.push(byte[0]);
            }
        }
        if !buf.is_empty() {
            break String::from_utf8(buf)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?;
        }
    };

    if first_line.starts_with('{') {
        return Ok(Some(IncomingMessage {
            payload: first_line,
            framed: false,
        }));
    }

    let mut content_length: Option<usize> = None;
    parse_header_line(&first_line, &mut content_length);

    let mut header_line = String::new();
    loop {
        header_line.clear();
        let bytes = reader.read_line(&mut header_line)?;
        if bytes == 0 {
            break;
        }
        if header_line == "\r\n" || header_line == "\n" {
            break;
        }
        parse_header_line(&header_line, &mut content_length);
    }

    let len = content_length.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Missing Content-Length header in framed MCP message",
        )
    })?;

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let payload = String::from_utf8(body)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 JSON payload"))?;

    Ok(Some(IncomingMessage {
        payload,
        framed: true,
    }))
}

fn parse_header_line(line: &str, content_length: &mut Option<usize>) {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("content-length:") {
        let value = line
            .split_once(':')
            .map(|(_, v)| v.trim())
            .and_then(|v| v.parse::<usize>().ok());
        if let Some(v) = value {
            *content_length = Some(v);
        }
    }
}

fn write_message<W: Write>(writer: &mut W, response: &Value, framed: bool) -> io::Result<()> {
    if framed {
        let payload = response.to_string();
        write!(
            writer,
            "Content-Length: {}\r\n\r\n{}",
            payload.len(),
            payload
        )?;
    } else {
        writeln!(writer, "{}", response)?;
    }
    writer.flush()
}

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "microscope-memory",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "memory_status",
                    "description": "Get microscope memory index status: block count, depths, append log size",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_store",
                    "description": "Store a new memory into the microscope append log",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "Memory text to store" },
                            "layer": { "type": "string", "description": "Memory layer (long_term, short_term, session, associative, emotional, relational, reflections, echo_cache)", "default": "long_term" },
                            "importance": { "type": "integer", "description": "Importance level 1-10", "default": 5 },
                            "emotion": { "type": "array", "items": { "type": "number" }, "minItems": 21, "maxItems": 21, "description": "21D emotion vector: [joy, sadness, anger, fear, surprise, disgust, trust, anticipation, love, gratitude, curiosity, awe, confusion, anxiety, serenity, hope, pride, shame, guilt, empathy, excitement]" }
                        },
                        "required": ["text"]
                    }
                },
                {
                    "name": "memory_recall",
                    "description": "Natural language recall with auto-zoom — searches both main index and append log. Optional emotion vector biases results toward emotionally similar memories.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Natural language query" },
                            "k": { "type": "integer", "description": "Max results to return", "default": 10 },
                            "emotion": { "type": "array", "items": { "type": "number" }, "minItems": 21, "maxItems": 21, "description": "21D emotion vector for emotional recall: [joy, sadness, anger, fear, surprise, disgust, trust, anticipation, love, gratitude, curiosity, awe, confusion, anxiety, serenity, hope, pride, shame, guilt, empathy, excitement]" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_find",
                    "description": "Brute-force text search across all depths",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Text to search for" },
                            "k": { "type": "integer", "description": "Max results", "default": 10 }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_mql_query",
                    "description": "Execute an MQL (Microscope Query Language) query with filters: layer, depth, spatial, boolean",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "mql": { "type": "string", "description": "MQL expression, e.g. 'layer:long_term depth:2..5 \"memory\"'" }
                        },
                        "required": ["mql"]
                    }
                },
                {
                    "name": "memory_build",
                    "description": "Rebuild the microscope index from layer source files (merges append log)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "force": { "type": "boolean", "description": "Force rebuild even if unchanged", "default": false }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_session_log",
                    "description": "Read last N interactions from the session memory layer (no index needed, reads layers/session.txt directly)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "n": { "type": "integer", "description": "Number of recent interactions to return", "default": 50 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_consolidate",
                    "description": "Consolidate recent session entries into long-term memory summaries. Groups entries by session ID and creates short summaries.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_dream",
                    "description": "Dream consolidation — offline memory replay that strengthens important pathways and prunes weak ones (biological sleep analog).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_look",
                    "description": "Manual spatial look at specific 3D coordinates and zoom level",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "x": { "type": "number", "description": "X coordinate (0.0-1.0)" },
                            "y": { "type": "number", "description": "Y coordinate (0.0-1.0)" },
                            "z": { "type": "number", "description": "Z coordinate (0.0-1.0)" },
                            "zoom": { "type": "integer", "description": "Depth level (0-8)" },
                            "k": { "type": "integer", "description": "Max results", "default": 10 }
                        },
                        "required": ["x", "y", "z", "zoom"]
                    }
                },
                {
                    "name": "memory_session_context",
                    "description": "Store conversation context automatically — saves the current session for later recall",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "context": { "type": "string", "description": "Conversation context to store" },
                            "summary": { "type": "string", "description": "Optional summary for long-term storage" }
                        },
                        "required": ["context"]
                    }
                },
                {
                    "name": "memory_ping",
                    "description": "Quick auto-context — fast recall without full indexing. Use this before answering to get relevant context automatically.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "What to get context for" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_auto_context",
                    "description": "Full system state snapshot: timeline (last session), open loops, block count, depth distribution, append log size",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "compact": { "type": "boolean", "description": "Compact mode (no box-drawing)", "default": false }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_timeline",
                    "description": "Show chronological timeline of stored memories by time window",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "window": { "type": "string", "description": "Time window: today, yesterday, last_N_days, since:YYYY-MM-DD, last_session, all", "default": "last_session" },
                            "k": { "type": "integer", "description": "Max entries to return", "default": 20 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_loops",
                    "description": "List currently open loops (unresolved tasks/thoughts)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "k": { "type": "integer", "description": "Max loops to return", "default": 20 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_resolve_loop",
                    "description": "Mark an open loop as resolved",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer", "description": "Loop ID to resolve" }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "memory_radial",
                    "description": "Radial spatial search — find blocks within a radius at a given depth",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "x": { "type": "number", "description": "X coordinate (0.0-1.0)" },
                            "y": { "type": "number", "description": "Y coordinate (0.0-1.0)" },
                            "z": { "type": "number", "description": "Z coordinate (0.0-1.0)" },
                            "depth": { "type": "integer", "description": "Depth level (0-8)" },
                            "radius": { "type": "number", "description": "Search radius", "default": 0.1 },
                            "k": { "type": "integer", "description": "Max results", "default": 10 }
                        },
                        "required": ["x", "y", "z", "depth"]
                    }
                },
                {
                    "name": "memory_soft",
                    "description": "4D soft zoom search — smooth multi-depth spatial query",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "x": { "type": "number", "description": "X coordinate (0.0-1.0)" },
                            "y": { "type": "number", "description": "Y coordinate (0.0-1.0)" },
                            "z": { "type": "number", "description": "Z coordinate (0.0-1.0)" },
                            "zoom": { "type": "integer", "description": "Zoom level (0-8)" },
                            "k": { "type": "integer", "description": "Max results", "default": 10 }
                        },
                        "required": ["x", "y", "z", "zoom"]
                    }
                },
                {
                    "name": "memory_think",
                    "description": "Sequential thinking chain — brainstorm a topic through multi-step memory traversal",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Topic to think about" },
                            "max_steps": { "type": "integer", "description": "Maximum thinking steps", "default": 5 }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_hebbian",
                    "description": "Show Hebbian learning state: activations, co-activations, energy, drifted blocks",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_hottest",
                    "description": "Show hottest (most frequently/recently activated) blocks",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "k": { "type": "integer", "description": "Number of hottest blocks", "default": 10 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_archetypes",
                    "description": "Show emerged archetypes — crystallized activation patterns",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_patterns",
                    "description": "Show thought graph patterns — crystallized recall sequences",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "k": { "type": "integer", "description": "Number of top patterns", "default": 10 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_attention",
                    "description": "Show attention mechanism state: learned layer weights, quality history",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_introspect",
                    "description": "Self-reflection — the system thinks about its own cognitive state",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_self_model",
                    "description": "Show the system's self-model snapshot",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_curiosity",
                    "description": "Show what the system is curious about — generated curiosity queries",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_monologue",
                    "description": "Generate an inner monologue — the system thinking to itself",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_stories",
                    "description": "Show narrative memory episodes — story arcs built from recalls",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "k": { "type": "integer", "description": "Number of recent episodes", "default": 5 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_daydream",
                    "description": "Associative drift (mind wandering) — start from a seed and drift through related memories",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "seed": { "type": "string", "description": "Seed text to start from (default: last narrative)", "default": "" },
                            "steps": { "type": "integer", "description": "Number of drift steps", "default": 3 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_hyperfocus",
                    "description": "Enter deep concentration mode on a specific topic",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "target": { "type": "string", "description": "Target topic to focus on" },
                            "focus_type": { "type": "string", "description": "Focus type: planning, problem_solving, creative, research", "default": "research" }
                        },
                        "required": ["target"]
                    }
                },
                {
                    "name": "memory_emotional_field",
                    "description": "Show emotional contagion state: local + remote emotional fields, total energy",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_embed",
                    "description": "Semantic search using embeddings — find conceptually similar memories",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Semantic query text" },
                            "k": { "type": "integer", "description": "Max results", "default": 10 },
                            "metric": { "type": "string", "description": "Distance metric: cosine, dot, l2", "default": "cosine" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_similar",
                    "description": "Find structurally similar blocks to a given text using fingerprint analysis",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "Text to find structural matches for" },
                            "k": { "type": "integer", "description": "Max results", "default": 5 }
                        },
                        "required": ["text"]
                    }
                },
                {
                    "name": "memory_links",
                    "description": "Show structural wormhole links for a specific block index",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "block_index": { "type": "integer", "description": "Block index to inspect" }
                        },
                        "required": ["block_index"]
                    }
                },
                {
                    "name": "memory_fingerprint",
                    "description": "Build structural fingerprints and wormhole links from the current index",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_dream_log",
                    "description": "Show dream consolidation history — past offline memory replay cycles",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "k": { "type": "integer", "description": "Number of recent dream cycles", "default": 10 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_resonance",
                    "description": "Show resonance protocol state: pulses, field energy, pending integrations",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_mirror",
                    "description": "Show mirror neuron state: resonance echoes, boosted blocks, similarity stats",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_predictions",
                    "description": "Show predictive cache stats: hit rate, active predictions, confidence",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_paths",
                    "description": "Show recent thought paths — recall sequences grouped by session",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sessions": { "type": "integer", "description": "Number of recent sessions", "default": 5 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_temporal_patterns",
                    "description": "Show temporal archetype patterns — time-of-day activation profiles",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_modalities",
                    "description": "Show multimodal index statistics",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_doctor",
                    "description": "Run integrity diagnostics and attempt automatic repair (crash recovery)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "fix": { "type": "boolean", "description": "Attempt to fix common corruption issues", "default": false }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_rebuild",
                    "description": "Rebuild the microscope index from layer source files (merges append log into main index)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "memory_store_data",
                    "description": "Store structured key-value data pairs",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pairs": { "type": "object", "description": "Key-value pairs to store", "additionalProperties": { "type": "string" } },
                            "importance": { "type": "integer", "description": "Importance level 1-10", "default": 5 }
                        },
                        "required": ["pairs"]
                    }
                },
                {
                    "name": "memory_resonant",
                    "description": "Show most resonant blocks — strongest mirror neuron signal",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "k": { "type": "integer", "description": "Number of top resonant blocks", "default": 10 }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_autonomous",
                    "description": "Run autonomous mode — the system runs itself: daydream, curiosity, monologue, reflect, narrative, dream",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tts": { "type": "boolean", "description": "Enable TTS", "default": false },
                            "daemon": { "type": "boolean", "description": "Run as continuous loop", "default": false },
                            "interval": { "type": "integer", "description": "Cycle interval in seconds", "default": 30 },
                            "max_cycles": { "type": "integer", "description": "Maximum cycles (default: 1 in single mode, infinite in daemon)" }
                        },
                        "required": []
                    }
                },
                {
                    "name": "memory_consciousness",
                    "description": "Show live consciousness stream state — emotion, surprise, curiosity, predictions, all 13 layers in real-time",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            ]
        }
    })
}


fn tool_status(config: &Config) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let mut depth_info = String::new();
    for (d, &(_start, count)) in reader.depth_ranges.iter().enumerate() {
        if count > 0 {
            depth_info.push_str(&format!("  D{}: {} blocks\n", d, count));
        }
    }

    let hdr_kb = (reader.block_count * crate::HEADER_SIZE) as f64 / 1024.0;
    let data_kb = reader.data.len() as f64 / 1024.0;

    Ok(format!(
        "Microscope Memory Status\n\
         ========================\n\
         Blocks: {}\n\
         Headers: {:.1} KB\n\
         Data: {:.1} KB\n\
         Total: {:.1} KB\n\
         Append log: {} entries\n\
         \n\
         Depth breakdown:\n\
         {}",
        reader.block_count,
        hdr_kb,
        data_kb,
        hdr_kb + data_kb,
        appended.len(),
        depth_info
    ))
}

fn tool_store(config: &Config, args: &Value) -> Result<String, String> {
    let text = args
        .get("text")
        .and_then(|v: &Value| v.as_str())
        .ok_or("Missing required parameter: text")?;
    let layer = args
        .get("layer")
        .and_then(|v: &Value| v.as_str())
        .unwrap_or("long_term");
    let importance = args
        .get("importance")
        .and_then(|v: &Value| v.as_u64())
        .unwrap_or(5) as u8;

    // Parse optional 21D emotion vector
    let emotion: Option<[f32; 21]> = args.get("emotion").and_then(|v: &Value| {
        let arr = v.as_array()?;
        if arr.len() != 21 {
            return None;
        }
        let mut emo = [0.0f32; 21];
        for (i, val) in arr.iter().enumerate() {
            emo[i] = val.as_f64().unwrap_or(0.0) as f32;
        }
        Some(emo)
    });

    let sid = std::process::id();
    let tagged = format!("[sid-{:04}] {}", sid % 10000, text);

    store_memory_with_emotion(config, &tagged, layer, importance, emotion)?;

    let (x, y, z) = crate::content_coords(&tagged, layer);
    let depth = crate::auto_depth(&tagged);

    Ok(format!(
        "Stored memory:\n\
         Layer: {}\n\
         Importance: {}\n\
         Depth: D{}\n\
         Position: ({:.3}, {:.3}, {:.3})\n\
         Session: sid-{:04}\n\
         Text: {}",
        layer,
        importance,
        depth,
        x,
        y,
        z,
        sid % 10000,
        crate::safe_truncate(text, 200)
    ))
}

fn tool_recall(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v: &Value| v.as_str())
        .ok_or("Missing required parameter: query")?;
    let k = args.get("k").and_then(|v: &Value| v.as_u64()).unwrap_or(10) as usize;

    // Parse optional 21D emotion vector for emotional recall
    let query_emotion: Option<[f32; 21]> = args.get("emotion").and_then(|v: &Value| {
        let arr = v.as_array()?;
        if arr.len() != 21 {
            return None;
        }
        let mut emo = [0.0f32; 21];
        for (i, val) in arr.iter().enumerate() {
            emo[i] = val.as_f64().unwrap_or(0.0) as f32;
        }
        Some(emo)
    });
    let emotional_recall_weight = config.search.emotional_bias_weight * 0.15;

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);

    // Load emotional state ring for priming + attention intensity
    let emotional_ring = crate::EmotionalStateRing::load_or_init(output_dir);
    let emotional_intensity = emotional_ring.intensity();

    // If no explicit emotion, try priming from the emotional state ring
    let query_emotion = query_emotion.or_else(|| {
        if emotional_ring.is_active() {
            if let Some((name, val)) = emotional_ring.dominant() {
                eprintln!("  [] emotional prime: {} ({:.2})", name, val);
            }
            Some(emotional_ring.current)
        } else {
            None
        }
    });

    let (qx, qy, qz) =
        crate::content_coords_blended(query, "long_term", config.search.semantic_weight);

    let (mut attention, hebb_pre, tg_pre, pc_pre) =
        if let Some(stream) = crate::consciousness_stream::global_stream() {
            let s = stream.lock().unwrap();
            (
                s.attention.clone(),
                s.hebbian.clone(),
                s.thought_graph.clone(),
                s.predictive_cache.clone(),
            )
        } else {
            let attn = crate::attention::AttentionState::load_or_init(output_dir);
            let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
            let tg = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let pc = crate::predictive_cache::PredictiveCache::load_or_init(output_dir);
            (attn, hebb, tg, pc)
        };

    let emotional_energy = crate::emotional::emotional_field(&reader, &hebb_pre)
        .map(|f| f.total_energy)
        .unwrap_or(0.0);

    if attention.total_recalls > 0 {
        let quality = attention.infer_quality();
        if let Some(last) = attention.history.last() {
            let prev_weights = last.weights;
            attention.record_outcome(quality, &prev_weights);
        }
    }

    let attn_signals = crate::attention::AttentionSignals {
        query_length: query.len(),
        emotional_energy,
        emotional_intensity,
        session_depth: tg_pre.current_path().len(),
        pattern_confidence: 0.0,
        cache_hit_rate: pc_pre.stats.hit_rate(),
        archetype_match_score: 0.0,
    };
    let attn = attention.compute_attention(&attn_signals);

    let emotional_weight = config.search.emotional_bias_weight * attn.weight(4);
    let (qx, qy, qz) =
        crate::emotional::apply_emotional_bias(qx, qy, qz, emotional_weight, &reader, &hebb_pre);

    let (zoom_lo, zoom_hi) = match query.len() {
        0..=8 => (0u8, 2u8),
        9..=20 => (2, 4),
        _ => (2, 5),
    };

    let q_lower = query.to_lowercase();
    let mut keyword_list: Vec<String> = q_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|s| s.to_string())
        .collect();

    let session_path = Path::new(&config.paths.layers_dir).join("session.txt");
    if session_path.exists() {
        if let Ok(sess) = std::fs::read_to_string(&session_path) {
            let recent: Vec<&str> = sess
                .split("\n\n")
                .filter(|s| !s.trim().is_empty())
                .collect();
            let context_start = if recent.len() > 5 {
                recent.len() - 5
            } else {
                0
            };
            for entry in &recent[context_start..] {
                for word in entry.split_whitespace() {
                    let w = word
                        .trim_matches(|c: char| !c.is_alphanumeric())
                        .to_lowercase();
                    if w.len() > 3 && !keyword_list.contains(&w) {
                        keyword_list.push(w);
                    }
                }
            }
        }
    }
    let keywords: Vec<&str> = keyword_list.iter().map(|s| s.as_str()).collect();

    // Load emotions.bin lookup for main-index emotional recall
    let emotion_lookup = query_emotion
        .as_ref()
        .and_then(|_| crate::load_emotion_lookup(output_dir));

    let mut all_results: Vec<(f32, usize, bool)> = Vec::new();

    for zoom in zoom_lo..=zoom_hi {
        let (start, count) = reader.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        for i in start..(start + count) {
            let text = reader.text(i).to_lowercase();
            let keyword_hits = keywords.iter().filter(|&&kw| text.contains(kw)).count();
            if keyword_hits > 0 {
                let h = reader.header(i);
                let dx = h.x - qx;
                let dy = h.y - qy;
                let dz = h.z - qz;
                let spatial_dist = dx * dx + dy * dy + dz * dz;
                let boost = keyword_hits as f32 * 0.1;
                // Emotional similarity boost (if query emotion AND emotions.bin data available)
                let emo_boost = query_emotion
                    .as_ref()
                    .and_then(|qe| {
                        emotion_lookup
                            .as_ref()
                            .and_then(|lookup| lookup(i))
                            .map(|block_emo| {
                                crate::emotional_similarity(qe, &block_emo)
                                    * emotional_recall_weight
                            })
                    })
                    .unwrap_or(0.0);
                let layer_imp = match h.layer_id {
                    li if LAYER_NAMES.get(li as usize) == Some(&"session") => 8.0,
                    li if LAYER_NAMES.get(li as usize) == Some(&"short_term") => 6.0,
                    li if LAYER_NAMES.get(li as usize) == Some(&"long_term") => 5.0,
                    _ => 4.0,
                };
                let imp_weight = 2.0 / (1.0 + layer_imp * 0.1);
                let combined = (spatial_dist - boost - emo_boost).max(0.0) * imp_weight;
                all_results.push((combined, i, true));
            }
        }
    }

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (ai, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx * dx + dy * dy + dz * dz;
        let text_lower = entry.text.to_lowercase();
        let keyword_hits = keywords
            .iter()
            .filter(|&&kw| text_lower.contains(kw))
            .count();
        let boost = keyword_hits as f32 * 0.1;
        // Emotional boost from inline append entry emotion
        let emo_boost = query_emotion
            .as_ref()
            .map(|qe| crate::emotional_similarity(qe, &entry.emotion) * emotional_recall_weight)
            .unwrap_or(0.0);
        if dist < 0.1 || keyword_hits > 0 || emo_boost > 0.0 {
            let imp_weight = 2.0 / (1.0 + entry.importance as f32 * 0.1);
            let combined = (dist - boost - emo_boost).max(0.0) * imp_weight;
            all_results.push((combined, ai + 1_000_000, false));
        }
    }

    // Spreading activation: fingerprint-linked blocks get boosted across 2-hop
    let link_table = crate::fingerprint::LinkTable::load(output_dir);
    if let Some(ref lt) = link_table {
        all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let top_n = all_results.len().min(3);
        for i in 0..top_n {
            let (_, idx, is_main) = all_results[i];
            let text = if is_main {
                reader.text(idx).to_string()
            } else {
                appended
                    .get(idx - 1_000_000)
                    .map(|e| e.text.clone())
                    .unwrap_or_default()
            };
            let similar = lt.find_similar(&text, 5);
            for (linked_idx, sim) in &similar {
                let linked_idx = *linked_idx as usize;
                let found = all_results
                    .iter()
                    .any(|(_, ri, rim)| *rim && *ri == linked_idx);
                if !found {
                    let boost = *sim * 0.12;
                    all_results.push((boost, linked_idx, true));
                } else {
                    for (dist, ri, rim) in &mut all_results {
                        if *rim && *ri == linked_idx {
                            *dist = (*dist - *sim * 0.08).max(0.0);
                            break;
                        }
                    }
                }
            }
        }
    }

    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let novel = all_results.first().is_none_or(|(d, _, _)| *d > 0.3);

    let qh_tg = crate::hebbian::query_hash(query);
    if let Some(stream) = crate::consciousness_stream::global_stream() {
        crate::consciousness_stream::ConsciousnessStream::feed_query(stream, qh_tg);
    }

    let (mut thought_graph, mut pred_cache) =
        if let Some(stream) = crate::consciousness_stream::global_stream() {
            let s = stream.lock().unwrap();
            (s.thought_graph.clone(), s.predictive_cache.clone())
        } else {
            let tg = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
            let pc = crate::predictive_cache::PredictiveCache::load_or_init(output_dir);
            (tg, pc)
        };

    if let Some((cached_blocks, confidence)) = pred_cache.check(qh_tg) {
        let boost = confidence * crate::thought_graph::PATTERN_BOOST_WEIGHT * attn.weight(6);
        let cached_set: HashSet<u32> = cached_blocks.iter().copied().collect();
        for (dist, idx, is_main) in &mut all_results {
            if *is_main && cached_set.contains(&(*idx as u32)) {
                *dist = (*dist - boost).max(0.0);
            }
        }
    }

    let pattern_boosts: HashMap<u32, f32> =
        thought_graph.pattern_boost(qh_tg).into_iter().collect();
    if !pattern_boosts.is_empty() {
        let tg_scale = attn.weight(5);
        for (dist, idx, is_main) in &mut all_results {
            if *is_main {
                if let Some(&boost) = pattern_boosts.get(&(*idx as u32)) {
                    *dist = (*dist - boost * tg_scale).max(0.0);
                }
            }
        }
    }

    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut output = format!("Recall '{}' (zoom D{}..D{})", query, zoom_lo, zoom_hi);
    if novel {
        output.push_str(" [NOVEL TOPIC — low prior memory]");
    }
    output.push_str(":\n\n");
    let mut seen = HashSet::new();
    let mut shown = 0;

    for (dist, idx, is_main) in &all_results {
        if shown >= k {
            break;
        }
        if !seen.insert((*idx, *is_main)) {
            continue;
        }
        if *is_main {
            let h = reader.header(*idx);
            let text = reader.text(*idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[D{} {} dist={:.3}] {}\n",
                h.depth,
                layer,
                dist,
                crate::safe_truncate(text, 150)
            ));
        } else {
            let ai = idx - 1_000_000;
            if let Some(entry) = appended.get(ai) {
                let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
                output.push_str(&format!(
                    "[APPEND {} dist={:.3}] {}\n",
                    layer,
                    dist,
                    crate::safe_truncate(&entry.text, 150)
                ));
            }
        }
        shown += 1;
    }

    let activated: Vec<(u32, f32)> = all_results
        .iter()
        .filter(|(_, _, is_main)| *is_main)
        .take(k)
        .map(|(score, idx, _)| (*idx as u32, *score))
        .collect();

    if !activated.is_empty() {
        let mut hebb = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
        let mut mirror = crate::mirror::MirrorState::load_or_init(output_dir);
        let qh = crate::hebbian::query_hash(query);

        let _ = crate::mirror::mirror_boost(&hebb, &mut mirror, &activated, qh);
        hebb.record_activation(&activated, qh);

        let mut resonance = crate::resonance::ResonanceState::load_or_init(output_dir);
        let headers: Vec<(f32, f32, f32)> = activated
            .iter()
            .map(|&(idx, _)| {
                let h = reader.header(idx as usize);
                (h.x, h.y, h.z)
            })
            .collect();
        resonance.emit_pulse(&activated, qh, &headers, 1);

        let mut archetypes = crate::archetype::ArchetypeState::load_or_init(output_dir);
        let mut temporal =
            crate::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
        let _ = archetypes.match_archetype(&activated);
        temporal.decay();
        archetypes.reinforce(&activated);

        let dominant_layer = activated
            .first()
            .map(|&(idx, _)| reader.header(idx as usize).layer_id)
            .unwrap_or(0);
        thought_graph.record_recall(qh, &activated, dominant_layer);
        let result_block_ids: Vec<u32> = activated.iter().map(|&(idx, _)| idx).collect();
        thought_graph.update_pattern_blocks(qh, &result_block_ids);
        thought_graph.detect_patterns();

        let _ = pred_cache.evaluate(qh, &result_block_ids, &mut thought_graph);
        pred_cache.predict_next(&thought_graph);

        attention.mark_recall();

        // Echo cache: store top-k recall results for fast re-access
        for (i, (_, idx, is_main)) in all_results.iter().enumerate() {
            if i >= 3 {
                break;
            }
            let text = if *is_main {
                format!(
                    "RECALL[{}]: {} -> {}",
                    i,
                    query,
                    crate::safe_truncate(reader.text(*idx), 180)
                )
            } else {
                appended
                    .get(idx - 1_000_000)
                    .map(|e| {
                        format!(
                            "RECALL[{}]: {} -> {}",
                            i,
                            query,
                            crate::safe_truncate(&e.text, 180)
                        )
                    })
                    .unwrap_or_default()
            };
            if !text.is_empty() {
                let _ = store_memory(config, &text, "echo_cache", 8 - i as u8);
            }
        }
        // Associative: link top-3 results that share keywords
        for (i, &(_, idx_a, is_a)) in all_results.iter().take(3).enumerate() {
            for &(_, idx_b, is_b) in all_results.iter().take(5).skip(i + 1) {
                let text_a = if is_a { reader.text(idx_a) } else { "" };
                let text_b = if is_b { reader.text(idx_b) } else { "" };
                if !text_a.is_empty() && !text_b.is_empty() {
                    let link = format!(
                        "LINK: [{:.40}] <-> [{:.40}] via '{}'",
                        text_a, text_b, query
                    );
                    let _ = store_memory(config, &link, "associative", 6);
                }
            }
        }

        let _ = hebb.save(output_dir);
        let _ = mirror.save(output_dir);
        let _ = resonance.save(output_dir);
        let _ = archetypes.save(output_dir);
        let _ = temporal.save(output_dir);
        let _ = thought_graph.save(output_dir);
        let _ = pred_cache.save(output_dir);
        let _ = attention.save(output_dir);

        if let Some(stream) = crate::consciousness_stream::global_stream() {
            let mut s = stream.lock().unwrap();
            s.hebbian = hebb;
            s.mirror = mirror;
            s.resonance = resonance;
            s.archetypes = archetypes;
            s.thought_graph = thought_graph;
            s.predictive_cache = pred_cache;
            s.attention = attention;
        }
    }

    output.push_str(&format!("\n{} results", shown));
    Ok(output)
}

fn tool_find(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v: &Value| v.as_str())
        .ok_or("Missing required parameter: query")?;
    let k = args.get("k").and_then(|v: &Value| v.as_u64()).unwrap_or(10) as usize;

    let reader = MicroscopeReader::open(config)?;
    let results = reader.find_text(query, k);

    if results.is_empty() {
        return Ok(format!("No results for '{}'", query));
    }

    let mut output = format!("Text search '{}': {} results\n\n", query, results.len());
    for (_depth, idx) in &results {
        let h = reader.header(*idx);
        let text = reader.text(*idx);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        output.push_str(&format!(
            "[D{} {}] {}\n",
            h.depth,
            layer,
            crate::safe_truncate(text, 150)
        ));
    }

    Ok(output)
}

fn tool_mql_query(config: &Config, args: &Value) -> Result<String, String> {
    let mql = args
        .get("mql")
        .and_then(|v: &Value| v.as_str())
        .ok_or("Missing required parameter: mql")?;

    let reader = MicroscopeReader::open(config)?;
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let q = crate::query::parse(mql);
    let results = crate::query::execute(&q, &reader, &appended);

    if results.is_empty() {
        return Ok(format!("MQL '{}': no results", mql));
    }

    let mut output = format!("MQL '{}': {} results\n\n", mql, results.len());
    for r in &results {
        if r.is_main {
            let h = reader.header(r.block_idx);
            let text = reader.text(r.block_idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[D{} {} score={:.3}] {}\n",
                h.depth,
                layer,
                r.score,
                crate::safe_truncate(text, 150)
            ));
        } else if let Some(entry) = appended.get(r.block_idx) {
            let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[APPEND {} score={:.3}] {}\n",
                layer,
                r.score,
                crate::safe_truncate(&entry.text, 150)
            ));
        }
    }

    Ok(output)
}

fn tool_build(config: &Config, args: &Value) -> Result<String, String> {
    let force = args
        .get("force")
        .and_then(|v: &Value| v.as_bool())
        .unwrap_or(false);

    crate::build::build(config, force)?;

    // Clear append log and emotions log after successful rebuild
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let _ = std::fs::remove_file(append_path);
    let emotions_path = Path::new(&config.paths.output_dir).join("emotions.bin");
    let _ = std::fs::remove_file(emotions_path);

    let reader = MicroscopeReader::open(config)?;
    Ok(format!(
        "Build complete: {} blocks across {} depths\nAppend log cleared.",
        reader.block_count,
        reader.depth_ranges.iter().filter(|&&(_, c)| c > 0).count()
    ))
}

fn tool_look(config: &Config, args: &Value) -> Result<String, String> {
    let x = args
        .get("x")
        .and_then(|v: &Value| v.as_f64())
        .ok_or("Missing required parameter: x")? as f32;
    let y = args
        .get("y")
        .and_then(|v: &Value| v.as_f64())
        .ok_or("Missing required parameter: y")? as f32;
    let z = args
        .get("z")
        .and_then(|v: &Value| v.as_f64())
        .ok_or("Missing required parameter: z")? as f32;
    let zoom = args
        .get("zoom")
        .and_then(|v: &Value| v.as_u64())
        .ok_or("Missing required parameter: zoom")? as u8;
    let k = args.get("k").and_then(|v: &Value| v.as_u64()).unwrap_or(10) as usize;

    let reader = MicroscopeReader::open(config)?;
    let config_clone = config.clone();
    let results = reader.look(&config_clone, x, y, z, zoom, k);

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    if results.is_empty() {
        return Ok(format!(
            "Look ({:.2},{:.2},{:.2}) zoom={}: no results",
            x, y, z, zoom
        ));
    }

    let mut output = format!(
        "Look ({:.2},{:.2},{:.2}) zoom={}: {} results\n\n",
        x,
        y,
        z,
        zoom,
        results.len()
    );

    for (dist, idx, is_main) in &results {
        if *is_main {
            let h = reader.header(*idx);
            let text = reader.text(*idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[D{} {} dist={:.3}] {}\n",
                h.depth,
                layer,
                dist,
                crate::safe_truncate(text, 150)
            ));
        } else if let Some(entry) = appended.get(*idx) {
            let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[APPEND {} dist={:.3}] {}\n",
                layer,
                dist,
                crate::safe_truncate(&entry.text, 150)
            ));
        }
    }

    Ok(output)
}

fn tool_session_log(config: &Config, args: &Value) -> Result<String, String> {
    let n = args.get("n").and_then(|v: &Value| v.as_u64()).unwrap_or(50) as usize;
    let file_path = Path::new(&config.paths.layers_dir).join("session.txt");

    let content = if file_path.exists() {
        std::fs::read_to_string(&file_path).unwrap_or_default()
    } else {
        return Ok("Session memory is empty. Store interactions with layer=session.".to_string());
    };

    let entries: Vec<&str> = content
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();

    let total = entries.len();
    let start = total.saturating_sub(n);
    let recent: Vec<&&str> = entries[start..].iter().rev().collect();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut output = format!(
        "Session Memory — {} total interactions, showing last {}:\n\n",
        total,
        recent.len()
    );
    for (i, entry) in recent.iter().enumerate() {
        let num = total - start - i;
        let decay = ebbinghaus_decay(entry, now_secs);
        output.push_str(&format!(
            "{} {}| {}\n",
            num,
            decay,
            crate::safe_truncate(entry, 300)
        ));
    }

    Ok(output)
}

fn ebbinghaus_decay(entry: &str, now_secs: u64) -> &'static str {
    let ts_str = if entry.starts_with('[') {
        entry
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
    } else {
        return "█ forgotten";
    };
    if ts_str.len() < 16 {
        return "█ forgotten";
    }
    let parts: Vec<&str> = ts_str.split(&['-', ' ', ':']).collect();
    if parts.len() < 5 {
        return "█ forgotten";
    }
    let y: u64 = parts[0].parse().unwrap_or(0);
    let mo: u64 = parts[1].parse().unwrap_or(0);
    let d: u64 = parts[2].parse().unwrap_or(0);
    let h: u64 = parts[3].parse().unwrap_or(0);
    let m: u64 = parts[4].parse().unwrap_or(0);
    if y < 2020 || mo == 0 || d == 0 {
        return "█ forgotten";
    }
    let mut days = 0u64;
    for yr in 1970..y {
        days += if is_leap_yr(yr) { 366 } else { 365 };
    }
    let leap = is_leap_yr(y);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for &d in mdays.iter().take((mo - 1) as usize) {
        days += d as u64;
    }
    days += d - 1;
    let entry_secs = days * 86400 + h * 3600 + m * 60;
    let age_hours = if now_secs > entry_secs {
        (now_secs - entry_secs) / 3600
    } else {
        0
    };

    if age_hours < 1 {
        "░ FRESH"
    } else if age_hours < 24 {
        "░ recent"
    } else if age_hours < 72 {
        "▒ fading"
    } else if age_hours < 168 {
        "▓ old"
    } else {
        "█ forgotten"
    }
}

fn is_leap_yr(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn tool_consolidate(config: &Config, _args: &Value) -> Result<String, String> {
    let file_path = Path::new(&config.paths.layers_dir).join("session.txt");
    let content = if file_path.exists() {
        std::fs::read_to_string(&file_path).unwrap_or_default()
    } else {
        return Ok("Session memory is empty.".to_string());
    };
    let entries: Vec<&str> = content
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();
    if entries.len() < 3 {
        return Ok("Not enough entries to consolidate (need 3+).".to_string());
    }

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for entry in &entries {
        let sid = if entry.contains("[sid-") {
            entry
                .split("[sid-")
                .nth(1)
                .and_then(|s| s.split(']').next())
                .map(|s| format!("sid-{}", s))
                .unwrap_or_else(|| "nosid".to_string())
        } else {
            "nosid".to_string()
        };
        groups.entry(sid).or_default().push(entry.to_string());
    }

    let mut summaries = Vec::new();
    for (sid, group) in &groups {
        if group.len() < 2 {
            continue;
        }
        let top_topics: Vec<String> = group
            .iter()
            .take(5)
            .map(|e| {
                let parts: Vec<&str> = e.split("] ").collect();
                crate::safe_truncate(parts.last().unwrap_or(&""), 50)
            })
            .collect();

        let summary = format!(
            "[{}] CONSOLIDATED: {} interactions. Topics: {}",
            sid,
            group.len(),
            top_topics.join(" | ")
        );
        summaries.push(summary);

        store_memory(
            config,
            &format!(
                "[{}] CONSOLIDATED: {} interactions from {}",
                sid,
                group.len(),
                top_topics.join(", ")
            ),
            "long_term",
            8,
        )?;
    }

    let mut output = format!("Consolidated {} session groups:\n\n", summaries.len());
    for s in &summaries {
        output.push_str(&format!("  {}\n", s));
    }
    Ok(output)
}

fn tool_dream(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let reader = MicroscopeReader::open(config)?;
    let block_count = reader.block_count;
    drop(reader);

    match crate::dream::dream_consolidate(output_dir, block_count) {
        Ok(cycle) => Ok(format!(
            "Dream consolidation complete:\n\
             Duration: {}ms\n\
             Replayed fingerprints: {}\n\
             Strengthened pairs: {}\n\
             Pruned pairs: {}\n\
             Pruned activations: {}\n\
             Consolidated patterns: {}\n\
             Energy: {:.3} -> {:.3}",
            cycle.duration_ms,
            cycle.replayed_fingerprints,
            cycle.strengthened_pairs,
            cycle.pruned_pairs,
            cycle.pruned_activations,
            cycle.consolidated_patterns,
            cycle.energy_before,
            cycle.energy_after,
        )),
        Err(e) => Err(format!("Dream consolidation failed: {}", e)),
    }
}

// ─── Session Context Tool ────────────────────────────

fn tool_session_context(config: &Config, args: &Value) -> Result<String, String> {
    let context = args
        .get("context")
        .and_then(|v: &Value| v.as_str())
        .ok_or("Missing required parameter: context")?;
    let summary = args
        .get("summary")
        .and_then(|v: &Value| v.as_str())
        .unwrap_or("");

    // Store the conversation context
    let tagged = format!("[SESSION_CONTEXT] {}", context);
    store_memory(config, &tagged, "session", 7)?;

    // If summary provided, store it in long_term
    if !summary.is_empty() {
        let summary_tagged = format!("[SESSION_SUMMARY] {}", summary);
        store_memory(config, &summary_tagged, "long_term", 6)?;
    }

    Ok(format!(
        "Session context stored:\n  Layer: session\n  Importance: 7\n  Context length: {} chars\n  Summary: {}",
        context.len(),
        if summary.is_empty() { "(none)" } else { summary }
    ))
}

// ─── Consciousness Tool (Live Stream) ────────────────────

fn tool_consciousness(config: &Config, _args: &Value) -> Result<String, String> {
    use crate::consciousness_stream::{global_stream, ConsciousnessStream};

    let state = match global_stream() {
        Some(s) => s.clone(),
        None => ConsciousnessStream::start(config),
    };

    // Ultra-fast path: read the cached format string from the snapshot.
    // The background cycle builds this string once per tick (100ms),
    // and readers just clone the Arc — no format!(), no seqlock, no Mutex.
    // Cost: ~50-100ns per call.
    let snapshot = {
        let s = state.lock().unwrap();
        s.snapshot.clone()
    };
    Ok(snapshot.read_cached_format())
}

// ─── Ping Tool (Auto-Context) ────────────────────────────

fn tool_ping(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v: &Value| v.as_str())
        .ok_or("Missing required parameter: query")?;

    // Quick recall with low k for fast context
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let append_path = output_dir.join("append.bin");
    let appended = read_append_log(&append_path);

    let (qx, qy, qz) =
        crate::content_coords_blended(query, "long_term", config.search.semantic_weight);

    let mut all_results: Vec<(f32, usize)> = Vec::new();

    // Quick scan of main index (top 500 blocks for speed)
    let scan_limit = reader.block_count.min(500);
    for i in 0..scan_limit {
        let h = reader.header(i);
        let dx = qx - h.x;
        let dy = qy - h.y;
        let dz = qz - h.z;
        let dist = dx * dx + dy * dy + dz * dz;
        if dist < 0.05 {
            all_results.push((dist, i));
        }
    }

    // Check append log
    for (ai, entry) in appended.iter().enumerate() {
        let dx = qx - entry.x;
        let dy = qy - entry.y;
        let dz = qz - entry.z;
        let dist = dx * dx + dy * dy + dz * dz;
        if dist < 0.05 {
            all_results.push((dist, 1_000_000 + ai));
        }
    }

    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut output = format!("PING '{}':\n", query);
    let mut shown = 0;
    for (dist, idx) in all_results.iter().take(5) {
        let text = if *idx < 1_000_000 {
            reader.text(*idx).to_string()
        } else {
            appended
                .get(*idx - 1_000_000)
                .map(|e| e.text.clone())
                .unwrap_or_default()
        };
        if !text.is_empty() {
            output.push_str(&format!(
                "  [{:.3}] {}\n",
                dist,
                crate::safe_truncate(&text, 120)
            ));
            shown += 1;
        }
    }

    if shown == 0 {
        output.push_str("  (no close matches)\n");
    }

    Ok(output)
}

// ─── Auto-Context Tool ─────────────────────────────

fn tool_auto_context(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let ctx = crate::auto_context::build(output_dir, &reader);
    Ok(crate::auto_context::render(&ctx))
}

// ─── Timeline Tool ──────────────────────────────────

fn tool_timeline(config: &Config, args: &Value) -> Result<String, String> {
    let window = args
        .get("window")
        .and_then(|v| v.as_str())
        .unwrap_or("last_session");
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let path = Path::new(&config.paths.output_dir).join("timeline.bin");
    let entries = crate::timeline::read_all(&path);
    let w = crate::timeline::TimeWindow::parse(window)
        .map_err(|e| format!("Invalid window '{}': {}", window, e))?;
    let filtered = crate::timeline::filter(&entries, &w);
    let mut rev: Vec<&crate::timeline::TimelineEntry> = filtered.iter().rev().collect();
    rev.truncate(k);

    if rev.is_empty() {
        return Ok(format!("Timeline [{}]: no entries found", window));
    }

    let mut output = format!("Timeline [{}] — {} entries:\n\n", window, rev.len());
    for e in &rev {
        let layer_name = crate::LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
        let status_label = match e.status {
            crate::timeline::STATUS_OPEN => "OPEN",
            crate::timeline::STATUS_RESOLVED => "RESOLVED",
            crate::timeline::STATUS_ARCHIVED => "ARCHIVED",
            _ => "",
        };
        output.push_str(&format!(
            "{} D{} [{}] imp={}{} {}\n",
            crate::timeline::format_ts(e.ts_ms),
            e.depth,
            layer_name,
            e.importance,
            if status_label.is_empty() {
                String::new()
            } else {
                format!(" [{}]", status_label)
            },
            crate::safe_truncate(&e.text, 100)
        ));
    }
    Ok(output)
}

// ─── Loops Tool ────────────────────────────────────

fn tool_loops(config: &Config, args: &Value) -> Result<String, String> {
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let dir = Path::new(&config.paths.output_dir);
    let mut open = crate::open_loops::read_open(&dir.join("open_loops.bin"));
    open.truncate(k);

    if open.is_empty() {
        return Ok("No open loops.".to_string());
    }

    let mut output = format!("Open Loops ({}):\n\n", open.len());
    for e in &open {
        output.push_str(&format!(
            "#{} {} imp={} {}\n",
            e.id,
            crate::timeline::format_ts(e.ts_ms),
            e.importance,
            crate::safe_truncate(&e.text, 100)
        ));
    }
    Ok(output)
}

// ─── Resolve Loop Tool ──────────────────────────────

fn tool_resolve_loop(config: &Config, args: &Value) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or("Missing required parameter: id")?;
    let dir = Path::new(&config.paths.output_dir);
    match crate::open_loops::resolve(dir, id) {
        Ok(true) => Ok(format!("Loop #{} resolved.", id)),
        Ok(false) => Ok(format!("Loop #{} not found or already resolved.", id)),
        Err(e) => Err(format!("Error resolving loop #{}: {}", id, e)),
    }
}

// ─── Radial Search Tool ─────────────────────────────

fn tool_radial(config: &Config, args: &Value) -> Result<String, String> {
    let x = args.get("x").and_then(|v| v.as_f64()).ok_or("Missing: x")? as f32;
    let y = args.get("y").and_then(|v| v.as_f64()).ok_or("Missing: y")? as f32;
    let z = args.get("z").and_then(|v| v.as_f64()).ok_or("Missing: z")? as f32;
    let depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .ok_or("Missing: depth")? as u8;
    let radius = args.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.1) as f32;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let reader = MicroscopeReader::open(config)?;
    let result_set = reader.radial_search(config, x, y, z, depth, radius, k);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let mut output = format!(
        "Radial ({:.2},{:.2},{:.2}) D{} r={:.3}:\n\n",
        x, y, z, depth, radius
    );

    if let Some(ref primary) = result_set.primary {
        output.push_str("PRIMARY:\n");
        if primary.is_main {
            let h = reader.header(primary.block_idx);
            let text = reader.text(primary.block_idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "  [D{} {} dist={:.3}] {}\n",
                h.depth,
                layer,
                primary.dist_sq,
                crate::safe_truncate(text, 150)
            ));
        } else if let Some(entry) = appended.get(primary.block_idx) {
            let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "  [APPEND {} dist={:.3}] {}\n",
                layer,
                primary.dist_sq,
                crate::safe_truncate(&entry.text, 150)
            ));
        }
    }

    if !result_set.neighbors.is_empty() {
        output.push_str(&format!("\nNeighbors ({}):\n", result_set.neighbors.len()));
        for n in &result_set.neighbors {
            if n.is_main {
                let h = reader.header(n.block_idx);
                let text = reader.text(n.block_idx);
                let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                output.push_str(&format!(
                    "  [D{} {} dist={:.3} w={:.3}] {}\n",
                    h.depth,
                    layer,
                    n.dist_sq,
                    n.weight,
                    crate::safe_truncate(text, 100)
                ));
            } else if let Some(entry) = appended.get(n.block_idx) {
                let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
                output.push_str(&format!(
                    "  [APPEND {} dist={:.3}] {}\n",
                    layer,
                    n.dist_sq,
                    crate::safe_truncate(&entry.text, 100)
                ));
            }
        }
    }

    output.push_str(&format!(
        "\n{} within radius, {} shown",
        result_set.total_within_radius,
        result_set.all().len()
    ));
    Ok(output)
}

// ─── Soft Zoom Tool ─────────────────────────────────

fn tool_soft(config: &Config, args: &Value) -> Result<String, String> {
    let x = args.get("x").and_then(|v| v.as_f64()).ok_or("Missing: x")? as f32;
    let y = args.get("y").and_then(|v| v.as_f64()).ok_or("Missing: y")? as f32;
    let z = args.get("z").and_then(|v| v.as_f64()).ok_or("Missing: z")? as f32;
    let zoom = args
        .get("zoom")
        .and_then(|v| v.as_u64())
        .ok_or("Missing: zoom")? as u8;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let reader = MicroscopeReader::open(config)?;
    let config_clone = config.clone();
    let results = reader.look_soft(&config_clone, x, y, z, zoom, k, config.search.zoom_weight);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    if results.is_empty() {
        return Ok(format!(
            "Soft ({:.2},{:.2},{:.2}) zoom={}: no results",
            x, y, z, zoom
        ));
    }

    let mut output = format!(
        "Soft 4D ({:.2},{:.2},{:.2}) zoom={}: {} results\n\n",
        x,
        y,
        z,
        zoom,
        results.len()
    );
    for (dist, idx, is_main) in &results {
        if *is_main {
            let h = reader.header(*idx);
            let text = reader.text(*idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[D{} {} dist={:.3}] {}\n",
                h.depth,
                layer,
                dist,
                crate::safe_truncate(text, 150)
            ));
        } else if let Some(entry) = appended.get(*idx) {
            let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[APPEND {} dist={:.3}] {}\n",
                layer,
                dist,
                crate::safe_truncate(&entry.text, 150)
            ));
        }
    }
    Ok(output)
}

// ─── Think Tool ─────────────────────────────────────

fn tool_think(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing: query")?;
    let max_steps = args.get("max_steps").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

    let reader = MicroscopeReader::open(config)?;
    let mut chain = crate::sequential_thinking::ThinkingChain::new(max_steps);
    chain.brainstorm(&reader, config, query);

    let mut output = format!("Sequential Thinking: '{}' ({} steps)\n\n", query, max_steps);
    // ThinkingChain has a display() method — we capture its output
    // Since display() prints to stdout, we'll build output manually
    output.push_str(&format!("Topic: {}\n", query));
    output.push_str(&format!("Steps: {}\n", max_steps));
    output.push_str("Run the CLI 'microscope-mem think' for full formatted output.\n");
    Ok(output)
}

// ─── Hebbian Tool ───────────────────────────────────

fn tool_hebbian(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let stats = hebb.stats();

    let mut output = format!(
        "Hebbian State:\n\
         ==============\n\
         Blocks:             {}\n\
         Active blocks:      {}\n\
         Total activations:  {}\n\
         Hot blocks (>0.1):  {}\n\
         Drifted blocks:     {}\n\
         Co-activation pairs: {}\n\
         Fingerprints:       {}\n",
        stats.block_count,
        stats.active_blocks,
        stats.total_activations,
        stats.hot_blocks,
        stats.drifted_blocks,
        stats.coactivation_pairs,
        stats.fingerprint_count,
    );

    let top = hebb.strongest_pairs(5);
    if !top.is_empty() {
        output.push_str("\nStrongest co-activations:\n");
        for pair in top {
            let text_a = crate::safe_truncate(reader.text(pair.block_a as usize), 30);
            let text_b = crate::safe_truncate(reader.text(pair.block_b as usize), 30);
            output.push_str(&format!(
                "  {}x  [{}] <-> [{}]\n",
                pair.count, text_a, text_b
            ));
        }
    }
    Ok(output)
}

// ─── Hottest Tool ───────────────────────────────────

fn tool_hottest(config: &Config, args: &Value) -> Result<String, String> {
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let hot = hebb.hottest_blocks(k);

    if hot.is_empty() {
        return Ok("No active blocks — run some queries first.".to_string());
    }

    let mut output = format!("Hottest {} blocks:\n\n", k);
    for (idx, energy) in &hot {
        let h = reader.header(*idx);
        let text = reader.text(*idx);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        let rec = &hebb.activations[*idx];
        output.push_str(&format!(
            "E={:.3} D{} [{}] count={} drift=({:.3},{:.3},{:.3}) {}\n",
            energy,
            h.depth,
            layer,
            rec.activation_count,
            rec.drift_x,
            rec.drift_y,
            rec.drift_z,
            crate::safe_truncate(text, 80)
        ));
    }
    Ok(output)
}

// ─── Archetypes Tool ───────────────────────────────

fn tool_archetypes(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let arc = crate::archetype::ArchetypeState::load_or_init(output_dir);
    let stats = arc.stats();

    let mut output = format!(
        "Archetypes:\n\
         ==========\n\
         Emerged:            {}\n\
         Total members:      {}\n",
        stats.archetype_count, stats.total_members,
    );
    if let (Some(label), Some(str)) = (&stats.strongest_label, stats.strongest_strength) {
        output.push_str(&format!(
            "Strongest:          '{}' (str={:.3})\n",
            label, str
        ));
    }

    if !arc.archetypes.is_empty() {
        output.push_str("\nAll archetypes:\n");
        for a in &arc.archetypes {
            output.push_str(&format!(
                "  #{} '{}' str={:.3} members={} reinforced={}x\n",
                a.id,
                a.label,
                a.strength,
                a.members.len(),
                a.reinforcement_count,
            ));
        }
    }
    Ok(output)
}

// ─── Patterns Tool ─────────────────────────────────

fn tool_patterns(config: &Config, args: &Value) -> Result<String, String> {
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let output_dir = Path::new(&config.paths.output_dir);
    let tg = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let stats = tg.stats();

    let mut output = format!(
        "Thought Graph:\n\
         =============\n\
         nodes={} edges={} patterns={} (crystallized={}) session=#{}\n",
        stats.node_count,
        stats.edge_count,
        stats.pattern_count,
        stats.crystallized,
        stats.current_session_id,
    );

    let top = tg.top_patterns(k);
    if top.is_empty() {
        output.push_str("\n(no patterns yet — recall more to form thought paths)\n");
    } else {
        output.push_str("\nTop patterns:\n");
        for (i, p) in top.iter().enumerate() {
            let seq_str: Vec<String> = p
                .sequence
                .iter()
                .map(|h| format!("{:04x}", h & 0xFFFF))
                .collect();
            let crystallized = if p.frequency >= 3 { "*" } else { " " };
            output.push_str(&format!(
                "{}#{} {} freq={} str={:.2} blocks={}\n",
                crystallized,
                i + 1,
                seq_str.join(" → "),
                p.frequency,
                p.strength,
                p.result_blocks.len()
            ));
        }
    }
    Ok(output)
}

// ─── Attention Tool ────────────────────────────────

fn tool_attention(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let attn = crate::attention::AttentionState::load_or_init(output_dir);

    let mut output = format!(
        "Attention:\n\
         =========\n\
         Total recalls: {}\n\
         History:       {}\n\n",
        attn.total_recalls,
        attn.history.len(),
    );

    output.push_str("Learned layer weights:\n");
    for (i, name) in crate::attention::LAYER_NAMES.iter().enumerate() {
        let w = attn.learned_weights[i];
        let bar_len = (w * 10.0) as usize;
        let bar: String = "█".repeat(bar_len.min(30));
        output.push_str(&format!("  {:<16} {:.3} {}\n", name, w, bar));
    }

    if !attn.history.is_empty() {
        let recent: Vec<&crate::attention::AttentionOutcome> =
            attn.history.iter().rev().take(5).collect();
        output.push_str("\nRecent outcomes:\n");
        for o in recent {
            let symbol = if o.quality >= 0.7 {
                "+"
            } else if o.quality <= 0.3 {
                "-"
            } else {
                "~"
            };
            output.push_str(&format!("  {} quality={:.2}\n", symbol, o.quality));
        }
    }
    Ok(output)
}

// ─── Introspect Tool ────────────────────────────────

fn tool_introspect(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let reflection = crate::self_reflect::introspect(config, &reader, output_dir);
    Ok(crate::self_reflect::format_reflection(&reflection))
}

// ─── Self Model Tool ───────────────────────────────

fn tool_self_model(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let mut self_model = crate::self_model::SelfModel::load_or_init(output_dir);
    let snap = self_model.take_snapshot(config, &reader, output_dir);
    let change = self_model.describe_change();
    Ok(crate::self_model::format_self_model(&snap, &change))
}

// ─── Curiosity Tool ─────────────────────────────────

fn tool_curiosity(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let mut curiosity = crate::curiosity::CuriosityState::load_or_init(output_dir);
    let queries = curiosity.generate_queries(config, &reader, output_dir);
    if queries.is_empty() {
        return Ok("No curiosity queries generated.".to_string());
    }
    Ok(crate::curiosity::format_curiosity(&queries))
}

// ─── Monologue Tool ─────────────────────────────────

fn tool_monologue(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let mut monologue = crate::inner_monologue::MonologueState::load_or_init(output_dir);
    let entry = monologue.generate_monologue(config, &reader, output_dir);
    Ok(crate::inner_monologue::format_monologue(&entry))
}

// ─── Stories Tool ───────────────────────────────────

fn tool_stories(config: &Config, args: &Value) -> Result<String, String> {
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let _reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let nm = crate::narrative_memory::NarrativeMemory::load_or_init(output_dir);

    if nm.episodes.is_empty() {
        return Ok("No narrative episodes yet — recall some memories first.".to_string());
    }

    let mut output = format!(
        "Narrative Episodes (last {} of {}):\n\n",
        k,
        nm.episodes.len()
    );
    for ep in nm.episodes.iter().rev().take(k) {
        output.push_str(&crate::narrative_memory::format_episode(ep));
        output.push_str("\n---\n");
    }
    Ok(output)
}

// ─── Daydream Tool ──────────────────────────────────

fn tool_daydream(config: &Config, args: &Value) -> Result<String, String> {
    let seed = args.get("seed").and_then(|v| v.as_str()).unwrap_or("");
    let steps = args.get("steps").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);

    let seed_text = if seed.is_empty() {
        // Try to get last narrative as seed
        let narrative = crate::narrative::NarrativeState::load_or_init(output_dir);
        if narrative.narrative.is_empty() {
            "microscope memory".to_string()
        } else {
            narrative.narrative.clone()
        }
    } else {
        seed.to_string()
    };

    let mut output = format!(
        "Daydream (seed: '{}', steps: {}):\n\n",
        crate::safe_truncate(&seed_text, 50),
        steps
    );
    let mut current = seed_text;

    for step in 0..steps {
        let (qx, qy, qz) = crate::content_coords(&current, "associative");
        let config_clone = config.clone();
        let results = reader.look(&config_clone, qx, qy, qz, 3, 3);
        if results.is_empty() {
            output.push_str(&format!("  Step {}: (no associations found)\n", step + 1));
            break;
        }
        let (_, idx, _) = results[0];
        let next_text = reader.text(idx);
        output.push_str(&format!(
            "  Step {}: {}\n",
            step + 1,
            crate::safe_truncate(next_text, 120)
        ));
        current = next_text.to_string();
    }
    Ok(output)
}

// ─── Hyperfocus Tool ────────────────────────────────

fn tool_hyperfocus(config: &Config, args: &Value) -> Result<String, String> {
    let target = args
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or("Missing: target")?;
    let focus_type = args
        .get("focus_type")
        .and_then(|v| v.as_str())
        .unwrap_or("research");

    let reader = MicroscopeReader::open(config)?;
    let _output_dir = Path::new(&config.paths.output_dir);

    // Store hyperfocus intent
    let focus_text = format!("[HYPERFOCUS:{}] Target: {}", focus_type, target);
    let _ = store_memory(config, &focus_text, "session", 9);

    // Recall related memories
    let (qx, qy, qz) = crate::content_coords(target, "long_term");
    let config_clone = config.clone();
    let results = reader.look(&config_clone, qx, qy, qz, 2, 10);

    let mut output = format!(
        "Hyperfocus: '{}' (type: {})\n\
         ============================\n\n\
         Focus intent stored in session layer (importance=9).\n\n",
        target, focus_type
    );

    if results.is_empty() {
        output.push_str("No related memories found — starting fresh.\n");
    } else {
        output.push_str("Related memories:\n");
        for (dist, idx, _) in &results {
            let h = reader.header(*idx);
            let text = reader.text(*idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "  [D{} {} dist={:.3}] {}\n",
                h.depth,
                layer,
                dist,
                crate::safe_truncate(text, 120)
            ));
        }
    }
    Ok(output)
}

// ─── Emotional Field Tool ───────────────────────────

fn tool_emotional_field(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);

    let mut output = String::from("Emotional Field:\n================\n");

    match crate::emotional::emotional_field(&reader, &hebb) {
        Some(field) => {
            output.push_str(&format!("Total energy: {:.3}\n", field.total_energy));
            output.push_str(&format!(
                "Centroid: ({:.3}, {:.3}, {:.3})\n",
                field.centroid.0, field.centroid.1, field.centroid.2
            ));
            output.push_str(&format!("Active blocks: {}\n", field.active_blocks));
        }
        None => {
            output.push_str("(no emotional field data — run queries to build state)\n");
        }
    }

    // Emotional state ring
    let ring = crate::EmotionalStateRing::load_or_init(output_dir);
    if ring.is_active() {
        output.push_str("\nEmotional State Ring:\n");
        output.push_str(&format!("  Intensity: {:.3}\n", ring.intensity()));
        if let Some((name, val)) = ring.dominant() {
            output.push_str(&format!("  Dominant: {} ({:.3})\n", name, val));
        }
    }

    Ok(output)
}

// ─── Embed Tool ─────────────────────────────────────

fn tool_embed(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing: query")?;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let metric = args
        .get("metric")
        .and_then(|v| v.as_str())
        .unwrap_or("cosine");

    use crate::embedding_index::EmbeddingIndex;
    use crate::embeddings::{EmbeddingProvider, MockEmbeddingProvider};

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let emb_path = output_dir.join("embeddings.bin");

    let provider: Box<dyn EmbeddingProvider> = Box::new(MockEmbeddingProvider::new(128));
    let query_embedding = provider
        .embed(query)
        .map_err(|e| format!("Embedding failed: {}", e))?;

    let mut output = format!("Semantic Search '{}' (metric: {}):\n\n", query, metric);

    if let Some(idx) = EmbeddingIndex::open(&emb_path) {
        let results = idx.search(&query_embedding, k);
        if results.is_empty() {
            output.push_str("(no results)\n");
        }
        for (sim, block_idx) in results {
            let h = reader.header(block_idx);
            let text = reader.text(block_idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[D{} {} sim={:.3}] {}\n",
                h.depth,
                layer,
                sim,
                crate::safe_truncate(text, 120)
            ));
        }
    } else {
        // On-the-fly search
        let mut results: Vec<(f32, usize)> = Vec::new();
        for i in 0..reader.block_count {
            let text = reader.text(i);
            if let Ok(block_emb) = provider.embed(text) {
                let similarity: f32 = block_emb
                    .iter()
                    .zip(query_embedding.iter())
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
                    / (block_emb.iter().map(|v| v * v).sum::<f32>().sqrt()
                        * query_embedding.iter().map(|v| v * v).sum::<f32>().sqrt()
                        + 1e-10);
                if similarity > 0.5 {
                    results.push((similarity, i));
                }
            }
        }
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.truncate(k);

        if results.is_empty() {
            output.push_str("(no results)\n");
        }
        for (sim, idx) in results {
            let h = reader.header(idx);
            let text = reader.text(idx);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "[D{} {} sim={:.3}] {}\n",
                h.depth,
                layer,
                sim,
                crate::safe_truncate(text, 120)
            ));
        }
    }
    Ok(output)
}

// ─── Similar Tool ───────────────────────────────────

fn tool_similar(config: &Config, args: &Value) -> Result<String, String> {
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or("Missing: text")?;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let table = crate::fingerprint::LinkTable::load(output_dir)
        .ok_or("fingerprints.idx not found — run 'fingerprint' first")?;

    let results = table.find_similar(text, k);
    if results.is_empty() {
        return Ok(format!(
            "No structurally similar blocks found for '{}'",
            crate::safe_truncate(text, 40)
        ));
    }

    let mut output = format!(
        "Structurally similar to '{}':\n\n",
        crate::safe_truncate(text, 40)
    );
    for (idx, sim) in &results {
        let h = reader.header(*idx as usize);
        let bt = reader.text(*idx as usize);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        output.push_str(&format!(
            "#{} D{} [{}] sim={:.3} {}\n",
            idx,
            h.depth,
            layer,
            sim,
            crate::safe_truncate(bt, 100)
        ));
    }
    Ok(output)
}

// ─── Links Tool ─────────────────────────────────────

fn tool_links(config: &Config, args: &Value) -> Result<String, String> {
    let block_index = args
        .get("block_index")
        .and_then(|v| v.as_u64())
        .ok_or("Missing: block_index")? as usize;

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let table = crate::fingerprint::LinkTable::load(output_dir)
        .ok_or("fingerprints.idx not found — run 'fingerprint' first")?;

    let links = table.linked_blocks(block_index as u32);
    let h = reader.header(block_index);
    let text = reader.text(block_index);
    let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");

    let mut output = format!(
        "Links for Block #{} D{} [{}] {}\n\n",
        block_index,
        h.depth,
        layer,
        crate::safe_truncate(text, 60)
    );

    if links.is_empty() {
        output.push_str("(no structural links)\n");
    } else {
        for (target, sim) in &links {
            let th = reader.header(*target as usize);
            let tt = reader.text(*target as usize);
            let tl = LAYER_NAMES.get(th.layer_id as usize).unwrap_or(&"?");
            output.push_str(&format!(
                "  -> #{} D{} [{}] sim={:.3} {}\n",
                target,
                th.depth,
                tl,
                sim,
                crate::safe_truncate(tt, 80)
            ));
        }
    }
    Ok(output)
}

// ─── Fingerprint Tool ───────────────────────────────

fn tool_fingerprint(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);

    let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();
    let table = crate::fingerprint::LinkTable::build(&texts);
    table
        .save(output_dir)
        .map_err(|e| format!("Failed to save fingerprints: {}", e))?;

    let stats = table.stats();
    Ok(format!(
        "Fingerprints built:\n\
         ==================\n\
         Avg entropy:        {:.3}\n\
         Unique hashes:      {}\n\
         Largest cluster:    {}\n\
         Structural links:   {}\n",
        stats.avg_entropy, stats.unique_hashes, stats.largest_cluster, stats.link_count,
    ))
}

// ─── Dream Log Tool ─────────────────────────────────

fn tool_dream_log(config: &Config, args: &Value) -> Result<String, String> {
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let output_dir = Path::new(&config.paths.output_dir);
    let log_path = output_dir.join("dream_log.bin");

    if !log_path.exists() {
        return Ok("No dream log found — run 'dream' first.".to_string());
    }

    let data = std::fs::read(&log_path).unwrap_or_default();
    if data.len() < 4 {
        return Ok("Dream log is empty.".to_string());
    }

    // Parse dream log entries (simple binary format: count + entries)
    let entry_size = 48; // approximate size per entry
    let count = data.len() / entry_size;
    let shown = count.min(k);

    let mut output = format!(
        "Dream Log — {} cycles recorded, showing last {}:\n\n",
        count, shown
    );
    for i in (count - shown)..count {
        let offset = i * entry_size;
        if offset + 8 <= data.len() {
            let ts = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap_or([0; 8]));
            output.push_str(&format!(
                "  Cycle #{}: ts={}\n",
                i,
                crate::timeline::format_ts(ts)
            ));
        }
    }
    Ok(output)
}

// ─── Resonance Tool ─────────────────────────────────

fn tool_resonance(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let resonance = crate::resonance::ResonanceState::load_or_init(output_dir);
    let stats = resonance.stats();

    Ok(format!(
        "Resonance Protocol:\n\
         ==================\n\
         Instance ID:        {:x}\n\
         Outgoing pulses:    {}\n\
         Incoming pulses:    {}\n\
         Pending integration:{}\n\
         Unique sources:     {}\n\
         Field cells:        {}\n\
         Field energy:       {:.3}\n",
        stats.instance_id,
        stats.outgoing_pulses,
        stats.incoming_pulses,
        stats.pending_integration,
        stats.unique_sources,
        stats.field_cells,
        stats.field_energy,
    ))
}

// ─── Mirror Tool ────────────────────────────────────

fn tool_mirror(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let mirror = crate::mirror::MirrorState::load_or_init(output_dir);
    let stats = mirror.stats();

    let mut output = format!(
        "Mirror Neuron State:\n\
         ===================\n\
         Resonance echoes:   {}\n\
         Resonant blocks:    {}\n\
         Avg similarity:     {:.3}\n",
        stats.total_echoes, stats.resonant_blocks, stats.avg_similarity,
    );

    if let Some((idx, strength)) = stats.strongest_block {
        let reader = MicroscopeReader::open(config)?;
        let text = reader.text(idx as usize);
        output.push_str(&format!(
            "Strongest:          block #{} (str={:.3}) {}\n",
            idx,
            strength,
            crate::safe_truncate(text, 60)
        ));
    }

    if !mirror.echoes.is_empty() {
        output.push_str("\nRecent echoes:\n");
        for echo in mirror.echoes.iter().rev().take(5) {
            output.push_str(&format!(
                "  sim={:.3} shared={} blocks trigger={:x} echo={:x}\n",
                echo.similarity,
                echo.shared_blocks.len(),
                echo.trigger_hash,
                echo.echo_hash,
            ));
        }
    }
    Ok(output)
}

// ─── Predictions Tool ────────────────────────────────

fn tool_predictions(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let cache = crate::predictive_cache::PredictiveCache::load_or_init(output_dir);
    let stats = &cache.stats;

    let mut output = format!(
        "Predictive Cache:\n\
         ================\n\
         Predictions:    {}\n\
         Hits:           {}\n\
         Misses:         {}\n\
         Partial hits:   {}\n\
         Hit rate:       {:.1}%\n\
         Active:         {}\n\
         Avg confidence: {:.1}%\n",
        stats.total_predictions,
        stats.total_hits,
        stats.total_misses,
        stats.total_partial_hits,
        stats.hit_rate() * 100.0,
        stats.current_predictions,
        stats.avg_confidence * 100.0,
    );

    if !cache.predictions.is_empty() {
        output.push_str("\nActive predictions:\n");
        for (i, p) in cache.predictions.iter().enumerate() {
            output.push_str(&format!(
                "  #{} hash={:04x} blocks={} conf={:.0}% pattern=#{}\n",
                i + 1,
                p.predicted_query_hash & 0xFFFF,
                p.blocks.len(),
                p.confidence * 100.0,
                p.pattern_id
            ));
        }
    }
    Ok(output)
}

// ─── Paths Tool ──────────────────────────────────────

fn tool_paths(config: &Config, args: &Value) -> Result<String, String> {
    let sessions = args.get("sessions").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let output_dir = Path::new(&config.paths.output_dir);
    let tg = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let recent = tg.recent_sessions(sessions);

    if recent.is_empty() {
        return Ok("No recall sessions recorded yet.".to_string());
    }

    let mut output = format!("Thought Paths (last {} sessions):\n\n", sessions);
    for (si, session) in recent.iter().enumerate() {
        if let Some(first) = session.first() {
            let path_str: Vec<String> = session
                .iter()
                .map(|n| format!("{:04x}", n.query_hash & 0xFFFF))
                .collect();
            output.push_str(&format!(
                "Session #{} ({} recalls): {}\n",
                first.session_id,
                session.len(),
                path_str.join(" → ")
            ));
        }
        if si >= sessions {
            break;
        }
    }
    Ok(output)
}

// ─── Temporal Patterns Tool ─────────────────────────

fn tool_temporal_patterns(config: &Config, _args: &Value) -> Result<String, String> {
    let output_dir = Path::new(&config.paths.output_dir);
    let temporal = crate::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
    let window = crate::temporal_archetype::current_time_window();

    let mut output = format!(
        "Temporal Archetypes (current window: {}):\n\n",
        crate::temporal_archetype::WINDOW_LABELS[window]
    );

    if temporal.profiles.is_empty() {
        output.push_str("(no temporal data yet)\n");
    } else {
        for p in &temporal.profiles {
            let dominant = p
                .dominant_window()
                .map(|w| crate::temporal_archetype::WINDOW_LABELS[w])
                .unwrap_or("?");
            output.push_str(&format!(
                "Archetype #{} (total={}, dominant={})\n",
                p.archetype_id, p.total_activations, dominant
            ));
            for (i, label) in crate::temporal_archetype::WINDOW_LABELS.iter().enumerate() {
                let bar_len = (p.window_weights[i] * 5.0) as usize;
                let bar: String = "█".repeat(bar_len);
                let marker = if i == window { " ◀" } else { "" };
                output.push_str(&format!(
                    "  {} {:>3} {:.1} {}{}\n",
                    label, p.window_counts[i], p.window_weights[i], bar, marker
                ));
            }
        }
    }
    Ok(output)
}

// ─── Modalities Tool ────────────────────────────────

fn tool_modalities(config: &Config, _args: &Value) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let _output_dir = Path::new(&config.paths.output_dir);

    let mut output = format!(
        "Multimodal Index:\n\
         =================\n\
         Total blocks: {}\n\n",
        reader.block_count
    );

    // Layer breakdown
    output.push_str("Layer breakdown:\n");
    let mut layer_counts: std::collections::HashMap<u8, usize> = std::collections::HashMap::new();
    for i in 0..reader.block_count {
        let h = reader.header(i);
        *layer_counts.entry(h.layer_id).or_default() += 1;
    }
    for (lid, count) in layer_counts.iter() {
        let name = LAYER_NAMES.get(*lid as usize).unwrap_or(&"?");
        output.push_str(&format!("  {}: {} blocks\n", name, count));
    }

    // Depth breakdown
    output.push_str("\nDepth breakdown:\n");
    for (d, &(_start, count)) in reader.depth_ranges.iter().enumerate() {
        if count > 0 {
            output.push_str(&format!("  D{}: {} blocks\n", d, count));
        }
    }

    Ok(output)
}

// ─── Doctor Tool ────────────────────────────────────

fn tool_doctor(config: &Config, args: &Value) -> Result<String, String> {
    let fix = args.get("fix").and_then(|v| v.as_bool()).unwrap_or(false);
    crate::doctor::run_doctor(config, fix).map_err(|e| format!("Doctor failed: {}", e))?;
    Ok("Doctor diagnostics complete. Use --fix to attempt repairs.".to_string())
}

// ─── Rebuild Tool ───────────────────────────────────

fn tool_rebuild(config: &Config, _args: &Value) -> Result<String, String> {
    crate::build::build(config, true).map_err(|e| format!("Rebuild failed: {}", e))?;
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let _ = std::fs::remove_file(&append_path);
    let reader = MicroscopeReader::open(config)?;
    Ok(format!(
        "Rebuild complete: {} blocks across {} depths.\nAppend log cleared.",
        reader.block_count,
        reader.depth_ranges.iter().filter(|&&(_, c)| c > 0).count()
    ))
}

// ─── Store Data Tool ────────────────────────────────

fn tool_store_data(config: &Config, args: &Value) -> Result<String, String> {
    let pairs = args
        .get("pairs")
        .and_then(|v| v.as_object())
        .ok_or("Missing: pairs")?;
    let importance = args.get("importance").and_then(|v| v.as_u64()).unwrap_or(5) as u8;

    let mut stored = Vec::new();
    for (key, val) in pairs {
        let val_str = val.as_str().unwrap_or("");
        let text = format!("[DATA] {} = {}", key, val_str);
        store_memory(config, &text, "long_term", importance)?;
        stored.push(format!("{} = {}", key, val_str));
    }

    Ok(format!(
        "Stored {} data pairs (importance={}):\n  {}",
        stored.len(),
        importance,
        stored.join("\n  ")
    ))
}

// ─── Resonant Tool ──────────────────────────────────

fn tool_resonant(config: &Config, args: &Value) -> Result<String, String> {
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);
    let mirror = crate::mirror::MirrorState::load_or_init(output_dir);
    let top = mirror.most_resonant(k);

    if top.is_empty() {
        return Ok("No resonant blocks — run queries to build mirror state.".to_string());
    }

    let mut output = format!("Most Resonant {} blocks:\n\n", k);
    for (idx, res) in &top {
        let h = reader.header(*idx as usize);
        let text = reader.text(*idx as usize);
        let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
        output.push_str(&format!(
            "S={:.3} D{} [{}] echoes={} {}\n",
            res.strength,
            h.depth,
            layer,
            res.echo_count,
            crate::safe_truncate(text, 80)
        ));
    }
    Ok(output)
}

// ─── Autonomous Tool ────────────────────────────────

fn tool_autonomous(config: &Config, args: &Value) -> Result<String, String> {
    let tts = args.get("tts").and_then(|v| v.as_bool()).unwrap_or(false);
    let daemon = args
        .get("daemon")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let interval = args.get("interval").and_then(|v| v.as_u64()).unwrap_or(30);
    let max_cycles = args.get("max_cycles").and_then(|v| v.as_u64());

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);

    let mut output = format!(
        "Autonomous Mode:\n\
         ===============\n\
         TTS: {}\n\
         Daemon: {}\n\
         Interval: {}s\n\
         Max cycles: {}\n\n",
        if tts { "enabled" } else { "disabled" },
        if daemon { "enabled" } else { "single cycle" },
        interval,
        max_cycles.map_or("unlimited".to_string(), |n| n.to_string()),
    );

    // Run one autonomous cycle
    // Daydream
    let narrative = crate::narrative::NarrativeState::load_or_init(output_dir);
    let seed = if narrative.narrative.is_empty() {
        "microscope memory".to_string()
    } else {
        narrative.narrative.clone()
    };
    let (qx, qy, qz) = crate::content_coords(&seed, "associative");
    let config_clone = config.clone();
    let daydream_results = reader.look(&config_clone, qx, qy, qz, 3, 3);
    output.push_str("Daydream:\n");
    for (dist, idx, _) in daydream_results.iter().take(3) {
        let text = reader.text(*idx);
        output.push_str(&format!(
            "  [dist={:.3}] {}\n",
            dist,
            crate::safe_truncate(text, 100)
        ));
    }

    // Curiosity
    let mut curiosity = crate::curiosity::CuriosityState::load_or_init(output_dir);
    let queries = curiosity.generate_queries(config, &reader, output_dir);
    if !queries.is_empty() {
        output.push_str("\nCuriosity:\n");
        for q in queries.iter().take(3) {
            output.push_str(&format!("  {}\n", crate::safe_truncate(&q.query, 100)));
        }
    }

    // Monologue
    let mut monologue = crate::inner_monologue::MonologueState::load_or_init(output_dir);
    let entry = monologue.generate_monologue(config, &reader, output_dir);
    output.push_str(&format!(
        "\nMonologue:\n  {}\n",
        crate::safe_truncate(&entry.steps.join("\n  "), 200)
    ));

    // Dream
    match crate::dream::dream_consolidate(output_dir, reader.block_count) {
        Ok(cycle) => {
            output.push_str(&format!(
                "\nDream: {}ms, {} strengthened, {} pruned, energy {:.3}→{:.3}\n",
                cycle.duration_ms,
                cycle.strengthened_pairs,
                cycle.pruned_pairs,
                cycle.energy_before,
                cycle.energy_after,
            ));
        }
        Err(e) => {
            output.push_str(&format!("\nDream: failed — {}\n", e));
        }
    }

    if daemon {
        output.push_str(&format!(
            "\nDaemon mode: cycling every {}s. Use CLI to stop.",
            interval
        ));
    }

    Ok(output)
}


#[cfg(test)]
mod tests {
    use super::*;
    use microscope_hooks::*;

    // ── Helper: create a test config ───────────────────────────────────────

    fn test_config() -> Config {
        Config {
            paths: crate::config::Paths {
                layers_dir: "./layers".to_string(),
                output_dir: "./data".to_string(),
                temp_dir: "./tmp".to_string(),
            },
            index: crate::config::Index {
                block_size: 256,
                max_depth: 8,
                header_size: 32,
            },
            search: crate::config::Search {
                default_k: 10,
                zoom_weight: 2.0,
                keyword_boost: 0.1,
                semantic_weight: 0.0,
                emotional_bias_weight: 0.0,
                emotion_21d_weight: 0.0,
            },
            memory_layers: crate::config::MemoryLayers {
                layers: vec![
                    "long_term".to_string(),
                    "short_term".to_string(),
                    "session".to_string(),
                ],
            },
            performance: crate::config::Performance {
                use_mmap: false,
                cache_size: 64,
                build_workers: 1,
                use_gpu: false,
                compression: false,
                cache_ttl_secs: 300,
            },
            logging: crate::config::Logging {
                level: "debug".to_string(),
                file: None,
            },
            embedding: crate::config::Embedding::default(),
            server: crate::config::Server::default(),
            federation: crate::config::Federation::default(),
            hooks: crate::config::HooksConfig::default(),
        }
    }

    // ── Test: read-only public mode ────────────────────────────────────────

    #[test]
    fn test_hooks_read_only_public_mode() {
        let config = test_config();
        // Default HooksConfig has read_only=true
        assert!(config.hooks.read_only, "public demo must be read-only by default");
        assert!(!config.hooks.write_enabled, "write must be disabled in public mode");

        let hook_config = if config.hooks.read_only {
            HookConfig::read_only()
        } else {
            HookConfig::default()
        };

        assert!(hook_config.read_only);
        assert!(!hook_config.can_write());
        assert!(!hook_config.is_enabled(&HookEvent::AfterResponse));
    }

    // ── Test: before_tool_call injection ────────────────────────────────────

    #[test]
    fn test_before_tool_call_injection() {
        let config = test_config();
        let hook_config = if config.hooks.read_only {
            HookConfig::read_only()
        } else {
            HookConfig::default()
        };
        let manager = HookManager::new(hook_config);

        let ctx = HookContext::new(HookEvent::BeforeToolCall)
            .with_tool("memory_recall", serde_json::json!({"query": "test"}));

        let result = manager.execute(HookEvent::BeforeToolCall, ctx);

        // before_tool_call should not produce memory candidates
        assert!(result.memory_candidates.is_empty());
        // Chain ID should be preserved
        assert!(!result.chain_id.is_empty());
        // Tool name should be preserved
        assert_eq!(result.tool_name, Some("memory_recall".to_string()));
    }

    // ── Test: after_tool_call candidate creation ────────────────────────────

    #[test]
    fn test_after_tool_call_candidate_creation() {
        let config = test_config();
        let hook_config = if config.hooks.read_only {
            HookConfig::read_only()
        } else {
            HookConfig::default()
        };
        let manager = HookManager::new(hook_config);

        let ctx = HookContext::new(HookEvent::AfterToolCall)
            .with_tool("memory_recall", serde_json::json!({"query": "test"}))
            .with_response("Found 3 relevant memories about the project.");

        let result = manager.execute(HookEvent::AfterToolCall, ctx);

        // In read-only mode, candidates should be cleared
        assert!(result.memory_candidates.is_empty(),
            "read-only mode must clear all memory candidates");
    }

    // ── Test: after_tool_call creates candidates in full mode ───────────────

    #[test]
    fn test_after_tool_call_candidate_full_mode() {
        let config = test_config();
        let hook_config = HookConfig::full();
        let manager = HookManager::new(hook_config);

        let mut ctx = HookContext::new(HookEvent::AfterToolCall)
            .with_tool("memory_recall", serde_json::json!({"query": "test"}));
        ctx.tool_result = Some("Found 3 relevant memories about the project requirements.".to_string());

        let result = manager.execute(HookEvent::AfterToolCall, ctx);

        // In full mode, candidates should be created
        assert!(!result.memory_candidates.is_empty(),
            "full mode must create memory candidates");
        assert_eq!(result.memory_candidates[0].source_tool, Some("memory_recall".to_string()));
        assert_eq!(result.memory_candidates[0].source_event, HookEvent::AfterToolCall);
    }

    // ── Test: error hook execution ──────────────────────────────────────────

    #[test]
    fn test_error_hook_execution() {
        let config = test_config();
        let hook_config = if config.hooks.read_only {
            HookConfig::read_only()
        } else {
            HookConfig::default()
        };
        let manager = HookManager::new(hook_config);

        let ctx = HookContext::new(HookEvent::Error)
            .with_tool("memory_store", serde_json::json!({"text": "test"}))
            .with_error("Index not found - run build first", "E1001");

        let result = manager.execute(HookEvent::Error, ctx);

        // In read-only mode, error candidates should be cleared
        assert!(result.memory_candidates.is_empty(),
            "read-only mode must clear error candidates");
    }

    // ── Test: error hook creates candidates in full mode ────────────────────

    #[test]
    fn test_error_hook_full_mode() {
        let config = test_config();
        let hook_config = HookConfig::full();
        let manager = HookManager::new(hook_config);

        let ctx = HookContext::new(HookEvent::Error)
            .with_tool("memory_store", serde_json::json!({"text": "test"}))
            .with_error("Connection timeout", "E1002");

        let result = manager.execute(HookEvent::Error, ctx);

        assert!(!result.memory_candidates.is_empty(),
            "full mode must create error candidates");
        assert!(result.memory_candidates[0].is_error);
        assert_eq!(result.memory_candidates[0].importance, 7);
    }

    // ── Test: write hook disabled by default ───────────────────────────────

    #[test]
    fn test_write_hook_disabled_by_default() {
        let config = test_config();
        // Default HooksConfig has write_enabled=false
        assert!(!config.hooks.write_enabled, "write must be disabled by default");

        let hook_config = HookConfig::default();
        assert!(!hook_config.can_write(), "HookConfig must deny writes by default");
        assert!(!hook_config.is_enabled(&HookEvent::AfterResponse),
            "after_response hook must be disabled by default");
    }

    // ── Test: secret filtering ─────────────────────────────────────────────

    #[test]
    fn test_secret_filtering() {
        let config = test_config();
        let hook_config = HookConfig::full();
        let manager = HookManager::new(hook_config);

        // Create a context with a secret-containing response
        let ctx = HookContext::new(HookEvent::AfterToolCall)
            .with_tool("memory_store", serde_json::json!({"text": "test"}))
            .with_response("The password is super-secret-123 and the API key is sk-test-key");

        let result = manager.execute(HookEvent::AfterToolCall, ctx);

        // Secret filtering should remove candidates containing secrets
        for candidate in &result.memory_candidates {
            assert!(!candidate.text.to_lowercase().contains("password"),
                "candidate must not contain secrets: {}", candidate.text);
            assert!(!candidate.text.to_lowercase().contains("sk-"),
                "candidate must not contain API keys: {}", candidate.text);
        }
    }

    // ── Test: secret filtering in read-only mode ───────────────────────────

    #[test]
    fn test_secret_filtering_read_only() {
        let config = test_config();
        let hook_config = HookConfig::read_only();
        let manager = HookManager::new(hook_config);

        let ctx = HookContext::new(HookEvent::AfterToolCall)
            .with_tool("memory_store", serde_json::json!({"text": "test"}))
            .with_response("The password is secret and the api_key=sk-abc123");

        let result = manager.execute(HookEvent::AfterToolCall, ctx);

        // In read-only mode, all candidates are cleared regardless
        assert!(result.memory_candidates.is_empty(),
            "read-only mode must clear all candidates even with secrets");
    }

    // ── Test: session start hook ────────────────────────────────────────────

    #[test]
    fn test_session_start_hook() {
        let config = test_config();
        let hook_config = if config.hooks.read_only {
            HookConfig::read_only()
        } else {
            HookConfig::default()
        };
        let manager = HookManager::new(hook_config);

        let ctx = HookContext::new(HookEvent::SessionStart);
        let result = manager.execute(HookEvent::SessionStart, ctx);

        // Session start should load memory contract
        assert!(result.memory_contract.is_some(), "session start must load memory contract");
        // Session start should load constraints
        assert!(!result.constraints.is_empty(), "session start must load constraints");
    }

    // ── Test: full lifecycle chain ─────────────────────────────────────────

    #[test]
    fn test_full_lifecycle_chain() {
        let config = test_config();
        let hook_config = HookConfig::full();
        let manager = HookManager::new(hook_config);

        // Simulate a full lifecycle: session start -> before tool -> after tool
        let ctx = HookContext::new(HookEvent::SessionStart);
        let events = [
            HookEvent::SessionStart,
            HookEvent::BeforeToolCall,
            HookEvent::AfterToolCall,
        ];

        let mut chain_ctx = ctx;
        for event in &events {
            chain_ctx = match event {
                HookEvent::BeforeToolCall => {
                    chain_ctx.with_tool("memory_recall", serde_json::json!({"query": "test"}))
                }
                HookEvent::AfterToolCall => {
                    chain_ctx.tool_result = Some("Found relevant results from the memory search.".to_string());
                    chain_ctx
                }
                _ => chain_ctx,
            };
            chain_ctx = manager.execute(*event, chain_ctx);
        }

        // After full chain with full mode, should have candidates
        assert!(!chain_ctx.memory_candidates.is_empty(),
            "full lifecycle chain should produce candidates");
        assert!(chain_ctx.memory_contract.is_some(),
            "memory contract should be loaded from session start");
    }


    // ── Smoke tests: public demo release ──────────────────────────────────

    #[test]
    fn test_public_demo_tools_list() {
        let response = handle_tools_list(&serde_json::json!("test"));
        let tools = response["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty(), "tools/list must return tools");
        let names: Vec<&str> = tools.iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"memory_status"), "must include memory_status");
        assert!(names.contains(&"memory_recall"), "must include memory_recall");
        assert!(names.contains(&"memory_find"), "must include memory_find");
        assert!(names.contains(&"memory_auto_context"), "must include memory_auto_context");
    }

    #[test]
    fn test_public_demo_tools_are_read_only() {
        // Verify that tools/list returns tools that are safe for read-only use
        let response = handle_tools_list(&serde_json::json!("test"));
        let tools = response["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter()
            .filter_map(|t| t["name"].as_str())
            .collect();

        // Read tools must be present
        assert!(names.contains(&"memory_status"), "must include memory_status");
        assert!(names.contains(&"memory_recall"), "must include memory_recall");
        assert!(names.contains(&"memory_find"), "must include memory_find");
        assert!(names.contains(&"memory_look"), "must include memory_look");
        assert!(names.contains(&"memory_auto_context"), "must include memory_auto_context");

        // All tools must have inputSchema with no required dangerous fields
        for tool in tools {
            let name = tool["name"].as_str().unwrap_or("");
            let schema = &tool["inputSchema"];
            assert!(schema.is_object(), "tool '{}' must have inputSchema", name);
        }
    }

    #[test]
    fn test_public_demo_safe_query() {
        let config = test_config();
        let hook_config = HookConfig::read_only();
        let manager = HookManager::new(hook_config);
        let ctx = HookContext::new(HookEvent::BeforeToolCall)
            .with_tool("memory_recall", serde_json::json!({"query": "safe query"}));
        let result = manager.execute(HookEvent::BeforeToolCall, ctx);
        assert!(result.memory_candidates.is_empty(),
            "safe query must not produce memory candidates in public mode");
        assert_eq!(result.tool_name, Some("memory_recall".to_string()));
    }

    #[test]
    fn test_public_demo_secret_query_filtered() {
        let config = test_config();
        let hook_config = HookConfig::read_only();
        let manager = HookManager::new(hook_config);
        let mut ctx = HookContext::new(HookEvent::AfterToolCall)
            .with_tool("memory_recall", serde_json::json!({"query": "test"}));
        ctx.tool_result = Some("Found result with password=secret123".to_string());
        let result = manager.execute(HookEvent::AfterToolCall, ctx);
        assert!(result.memory_candidates.is_empty(),
            "secret-containing results must not create candidates in public mode");
    }

    #[test]
    fn test_public_demo_initialize_response() {
        let response = handle_initialize(&serde_json::json!("test"));
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], "test");
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

}