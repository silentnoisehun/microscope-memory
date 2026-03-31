//! Native MCP (Model Context Protocol) server for Microscope Memory.
//!
//! Implements JSON-RPC 2.0 over stdio with the MCP tool-calling protocol.
//! Replaces the Python MCP server with a native Rust implementation.
//!
//! Not available on WASM targets (no stdio).

use crate::config::Config;
use crate::reader::MicroscopeReader;
use crate::{read_append_log, store_memory, LAYER_NAMES};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Run the MCP server on stdio (blocking).
pub fn run(config: Config) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                let _ = writeln!(stdout, "{}", err);
                let _ = stdout.flush();
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(&id),
            "initialized" => continue, // notification, no response
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call(&id, &request, &config),
            "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            "notifications/cancelled" | "notifications/initialized" => continue,
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }),
        };

        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }
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
                            "layer": { "type": "string", "description": "Memory layer (long_term, short_term, associative, emotional, relational, reflections, echo_cache)", "default": "long_term" },
                            "importance": { "type": "integer", "description": "Importance level 1-10", "default": 5 }
                        },
                        "required": ["text"]
                    }
                },
                {
                    "name": "memory_recall",
                    "description": "Natural language recall with auto-zoom — searches both main index and append log",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Natural language query" },
                            "k": { "type": "integer", "description": "Max results to return", "default": 10 }
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
                            "mql": { "type": "string", "description": "MQL expression, e.g. 'layer:long_term depth:2..5 \"Ora\"'" }
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
                }
            ]
        }
    })
}

fn handle_tools_call(id: &Value, request: &Value, config: &Config) -> Value {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = match tool_name {
        "memory_status" => tool_status(config),
        "memory_store" => tool_store(config, &args),
        "memory_recall" => tool_recall(config, &args),
        "memory_find" => tool_find(config, &args),
        "memory_mql_query" => tool_mql_query(config, &args),
        "memory_build" => tool_build(config, &args),
        "memory_look" => tool_look(config, &args),
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": content }]
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                "isError": true
            }
        }),
    }
}

// ─── Tool implementations ────────────────────────────

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
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: text")?;
    let layer = args
        .get("layer")
        .and_then(|v| v.as_str())
        .unwrap_or("long_term");
    let importance = args.get("importance").and_then(|v| v.as_u64()).unwrap_or(5) as u8;

    store_memory(config, text, layer, importance)?;

    let (x, y, z) = crate::content_coords(text, layer);
    let depth = crate::auto_depth(text);

    Ok(format!(
        "Stored memory:\n\
         Layer: {}\n\
         Importance: {}\n\
         Depth: D{}\n\
         Position: ({:.3}, {:.3}, {:.3})\n\
         Text: {}",
        layer,
        importance,
        depth,
        x,
        y,
        z,
        crate::safe_truncate(text, 200)
    ))
}

fn tool_recall(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: query")?;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let reader = MicroscopeReader::open(config)?;

    let (qx, qy, qz) =
        crate::content_coords_blended(query, "long_term", config.search.semantic_weight);
    let (zoom_lo, zoom_hi) = match query.len() {
        0..=10 => (0u8, 3u8),
        11..=40 => (3, 6),
        _ => (6, 8),
    };

    let q_lower = query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

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
                all_results.push(((spatial_dist - boost).max(0.0), i, true));
            }
        }
    }

    // Search append log too
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
        if dist < 0.1 || keyword_hits > 0 {
            all_results.push(((dist - boost).max(0.0), ai + 1_000_000, false));
        }
    }

    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut output = format!("Recall '{}' (zoom D{}..D{}):\n\n", query, zoom_lo, zoom_hi);
    let mut seen = std::collections::HashSet::new();
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

    output.push_str(&format!("\n{} results", shown));
    Ok(output)
}

fn tool_find(config: &Config, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: query")?;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

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
        .and_then(|v| v.as_str())
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
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    crate::build::build(config, force)?;

    // Clear append log after successful rebuild
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let _ = std::fs::remove_file(append_path);

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
        .and_then(|v| v.as_f64())
        .ok_or("Missing required parameter: x")? as f32;
    let y = args
        .get("y")
        .and_then(|v| v.as_f64())
        .ok_or("Missing required parameter: y")? as f32;
    let z = args
        .get("z")
        .and_then(|v| v.as_f64())
        .ok_or("Missing required parameter: z")? as f32;
    let zoom = args
        .get("zoom")
        .and_then(|v| v.as_u64())
        .ok_or("Missing required parameter: zoom")? as u8;
    let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

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
