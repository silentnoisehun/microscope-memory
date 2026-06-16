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

        let _ = write_message(&mut stdout, &response, incoming.framed);
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
                }
            ]
        }
    })
}

fn handle_tools_call(id: &Value, request: &Value, config: &Config) -> Value {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(|n: &Value| n.as_str())
        .unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

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

    store_memory(config, &tagged, layer, importance, emotion)?;

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
        if arr.len() != 21 { return None; }
        let mut emo = [0.0f32; 21];
        for (i, val) in arr.iter().enumerate() {
            emo[i] = val.as_f64().unwrap_or(0.0) as f32;
        }
        Some(emo)
    });
    let emotional_recall_weight = config.search.emotional_bias_weight * 0.15;

    let reader = MicroscopeReader::open(config)?;
    let output_dir = Path::new(&config.paths.output_dir);

    let (qx, qy, qz) = crate::content_coords_blended(query, "long_term", config.search.semantic_weight);

    let mut attention = crate::attention::AttentionState::load_or_init(output_dir);
    let hebb_pre = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let tg_pre = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let pc_pre = crate::predictive_cache::PredictiveCache::load_or_init(output_dir);

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
        session_depth: tg_pre.current_path().len(),
        pattern_confidence: 0.0,
        cache_hit_rate: pc_pre.stats.hit_rate(),
        archetype_match_score: 0.0,
    };
    let attn = attention.compute_attention(&attn_signals);

    let hebb_eb = crate::hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let emotional_weight = config.search.emotional_bias_weight * attn.weight(4);
    let (qx, qy, qz) = crate::emotional::apply_emotional_bias(
        qx, qy, qz, emotional_weight, &reader, &hebb_eb,
    );

    let (zoom_lo, zoom_hi) = match query.len() {
        0..=8 => (0u8, 2u8),
        9..=20 => (2, 4),
        _ => (2, 5),
    };

    let q_lower = query.to_lowercase();
    let mut keyword_list: Vec<String> = q_lower.split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|s| s.to_string())
        .collect();

    let session_path = Path::new(&config.paths.layers_dir).join("session.txt");
    if session_path.exists() {
        if let Ok(sess) = std::fs::read_to_string(&session_path) {
            let recent: Vec<&str> = sess.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
            let context_start = if recent.len() > 5 { recent.len() - 5 } else { 0 };
            for entry in &recent[context_start..] {
                for word in entry.split_whitespace() {
                    let w = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                    if w.len() > 3 && !keyword_list.contains(&w) {
                        keyword_list.push(w);
                    }
                }
            }
        }
    }
    let keywords: Vec<&str> = keyword_list.iter().map(|s| s.as_str()).collect();

    // Load emotions.bin lookup for main-index emotional recall
    let emotion_lookup = query_emotion.as_ref().and_then(|_| {
        crate::load_emotion_lookup(output_dir)
    });

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
                let emo_boost = query_emotion.as_ref().and_then(|qe| {
                    emotion_lookup.as_ref().and_then(|lookup| lookup(i))
                        .map(|block_emo| crate::emotional_similarity(qe, &block_emo) * emotional_recall_weight)
                }).unwrap_or(0.0);
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
        let keyword_hits = keywords.iter().filter(|&&kw| text_lower.contains(kw)).count();
        let boost = keyword_hits as f32 * 0.1;
        // Emotional boost from inline append entry emotion
        let emo_boost = query_emotion.as_ref()
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
                appended.get(idx - 1_000_000).map(|e| e.text.clone()).unwrap_or_default()
            };
            let similar = lt.find_similar(&text, 5);
            for (linked_idx, sim) in &similar {
                let linked_idx = *linked_idx as usize;
                let found = all_results.iter().any(|(_, ri, rim)| *rim && *ri == linked_idx);
                if !found {
                    let boost = *sim as f32 * 0.12;
                    all_results.push((boost, linked_idx, true));
                } else {
                    for (dist, ri, rim) in &mut all_results {
                        if *rim && *ri == linked_idx {
                            *dist = (*dist - *sim as f32 * 0.08).max(0.0);
                            break;
                        }
                    }
                }
            }
        }
    }

    all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let novel = all_results.first().map_or(true, |(d, _, _)| *d > 0.3);

    let mut thought_graph = crate::thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let mut pred_cache = crate::predictive_cache::PredictiveCache::load_or_init(output_dir);
    let qh_tg = crate::hebbian::query_hash(query);

    if let Some((cached_blocks, confidence)) = pred_cache.check(qh_tg) {
        let boost = confidence * crate::thought_graph::PATTERN_BOOST_WEIGHT * attn.weight(6);
        let cached_set: HashSet<u32> = cached_blocks.iter().copied().collect();
        for (dist, idx, is_main) in &mut all_results {
            if *is_main && cached_set.contains(&(*idx as u32)) {
                *dist = (*dist - boost).max(0.0);
            }
        }
    }

    let pattern_boosts: HashMap<u32, f32> = thought_graph.pattern_boost(qh_tg).into_iter().collect();
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
                h.depth, layer, dist, crate::safe_truncate(text, 150)
            ));
        } else {
            let ai = idx - 1_000_000;
            if let Some(entry) = appended.get(ai) {
                let layer = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
                output.push_str(&format!(
                    "[APPEND {} dist={:.3}] {}\n",
                    layer, dist, crate::safe_truncate(&entry.text, 150)
                ));
            }
        }
        shown += 1;
    }

    let activated: Vec<(u32, f32)> = all_results.iter()
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
        let headers: Vec<(f32, f32, f32)> = activated.iter().map(|&(idx, _)| {
            let h = reader.header(idx as usize);
            (h.x, h.y, h.z)
        }).collect();
        resonance.emit_pulse(&activated, qh, &headers, 1);

        let mut archetypes = crate::archetype::ArchetypeState::load_or_init(output_dir);
        let mut temporal = crate::temporal_archetype::TemporalArchetypeState::load_or_init(output_dir);
        let _ = archetypes.match_archetype(&activated);
        temporal.decay();
        archetypes.reinforce(&activated);

        let dominant_layer = activated.first()
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
            if i >= 3 { break; }
            let text = if *is_main {
                format!("RECALL[{}]: {} -> {}", i, query, crate::safe_truncate(reader.text(*idx), 180))
            } else {
                appended.get(idx - 1_000_000)
                    .map(|e| format!("RECALL[{}]: {} -> {}", i, query, crate::safe_truncate(&e.text, 180)))
                    .unwrap_or_default()
            };
            if !text.is_empty() {
                let _ = store_memory(config, &text, "echo_cache", 8 - i as u8, None);
            }
        }
        // Associative: link top-3 results that share keywords
        for i in 0..all_results.len().min(3) {
            let (_, idx_a, is_a) = all_results[i];
            for j in (i+1)..all_results.len().min(5) {
                let (_, idx_b, is_b) = all_results[j];
                let text_a = if is_a { reader.text(idx_a) } else { "" };
                let text_b = if is_b { reader.text(idx_b) } else { "" };
                if !text_a.is_empty() && !text_b.is_empty() {
                    let link = format!("LINK: [{:.40}] <-> [{:.40}] via '{}'", text_a, text_b, query);
                    let _ = store_memory(config, &link, "associative", 6, None);
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
    let start = if total > n { total - n } else { 0 };
    let recent: Vec<&&str> = entries[start..].iter().rev().collect();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut output = format!("Session Memory — {} total interactions, showing last {}:\n\n", total, recent.len());
    for (i, entry) in recent.iter().enumerate() {
        let num = total - start - i;
        let decay = ebbinghaus_decay(entry, now_secs);
        output.push_str(&format!("{} {}| {}\n", num, decay, crate::safe_truncate(entry, 300)));
    }

    Ok(output)
}

fn ebbinghaus_decay(entry: &str, now_secs: u64) -> &'static str {
    let ts_str = if entry.starts_with('[') {
        entry.split(']').next().unwrap_or("").trim_start_matches('[')
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
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for mi in 0..(mo - 1) as usize {
        days += mdays[mi];
    }
    days += d - 1;
    let entry_secs = days * 86400 + h * 3600 + m * 60;
    let age_hours = if now_secs > entry_secs { (now_secs - entry_secs) / 3600 } else { 0 };

    if age_hours < 1 { "░ FRESH" }
    else if age_hours < 24 { "░ recent" }
    else if age_hours < 72 { "▒ fading" }
    else if age_hours < 168 { "▓ old" }
    else { "█ forgotten" }
}

fn is_leap_yr(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn tool_consolidate(config: &Config, _args: &Value) -> Result<String, String> {
    let file_path = Path::new(&config.paths.layers_dir).join("session.txt");
    let content = if file_path.exists() {
        std::fs::read_to_string(&file_path).unwrap_or_default()
    } else {
        return Ok("Session memory is empty.".to_string());
    };
    let entries: Vec<&str> = content.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
    if entries.len() < 3 {
        return Ok("Not enough entries to consolidate (need 3+).".to_string());
    }

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for entry in &entries {
        let sid = if entry.contains("[sid-") {
            entry.split("[sid-").nth(1)
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
        let top_topics: Vec<String> = group.iter()
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

        store_memory(config, &format!("[{}] CONSOLIDATED: {} interactions from {}", sid, group.len(), top_topics.join(", ")), "long_term", 8, None)?;
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
