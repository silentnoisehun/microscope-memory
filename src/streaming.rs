//! HTTP server for Microscope Memory — powered by tiny_http.
//!
//! Endpoints:
//!   GET  /stats           → index statistics
//!   POST /store           → store a memory {"text":"...", "layer":"...", "importance": N}
//!   GET  /find?q=...&k=N  → text search
//!   POST /recall          → recall query {"query":"...", "k": N}
//!   POST /query           → MQL query {"mql":"..."}
//!   GET  /health          → health check

use std::sync::Arc;
use std::path::Path;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::{MicroscopeReader, store_memory, read_append_log, LAYER_NAMES, content_coords_blended};

#[derive(Deserialize)]
struct StoreRequest {
    text: String,
    #[serde(default = "default_layer")]
    layer: String,
    #[serde(default = "default_importance")]
    importance: u8,
}

fn default_layer() -> String { "long_term".to_string() }
fn default_importance() -> u8 { 5 }

#[derive(Deserialize)]
struct RecallRequest {
    query: String,
    #[serde(default = "default_k")]
    k: usize,
}

#[derive(Deserialize)]
struct MqlRequest {
    mql: String,
}

fn default_k() -> usize { 10 }

#[derive(Serialize)]
struct ResultEntry {
    text: String,
    depth: u8,
    layer: String,
    score: f32,
    is_append: bool,
}

#[derive(Serialize)]
struct StatsResponse {
    block_count: usize,
    append_count: usize,
    depths: Vec<DepthInfo>,
}

#[derive(Serialize)]
struct DepthInfo {
    depth: usize,
    count: u32,
}

/// Start the HTTP server on the given port.
pub fn start_endpoint_server(config: Config, port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    let server = tiny_http::Server::http(&addr)
        .unwrap_or_else(|e| panic!("Could not bind to {}: {}", addr, e));

    println!("{} on {}", "MICROSCOPE HTTP SERVER".cyan().bold(), addr);
    println!("  GET  /stats");
    println!("  GET  /health");
    println!("  GET  /find?q=...&k=N");
    println!("  POST /store");
    println!("  POST /recall");
    println!("  POST /query  (MQL)");

    let config = Arc::new(config);
    let pool_size = std::thread::available_parallelism()
        .map(|n| n.get().min(8)).unwrap_or(4);
    let server = Arc::new(server);

    let mut handles = Vec::new();
    for _ in 0..pool_size {
        let server = Arc::clone(&server);
        let cfg = Arc::clone(&config);
        handles.push(std::thread::spawn(move || {
            while let Ok(req) = server.recv() {
                handle_request(req, &cfg);
            }
        }));
    }

    for h in handles {
        let _ = h.join();
    }
}

fn handle_request(mut request: tiny_http::Request, config: &Config) {
    let method = request.method().to_string();
    let url = request.url().to_string();
    let cors = config.server.cors_origin.as_deref().unwrap_or("*");
    let path = url.split('?').next().unwrap_or("").to_string();

    // For POST requests, read body upfront
    let body = if method == "POST" {
        let mut buf = String::new();
        if request.as_reader().read_to_string(&mut buf).is_err() {
            respond(request, Err("failed to read body".into()), cors);
            return;
        }
        buf
    } else {
        String::new()
    };

    let result = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => Ok("{\"status\":\"ok\"}".to_string()),
        ("GET", "/stats") => handle_stats(config),
        ("GET", "/find") => handle_find(&url, config),
        ("POST", "/store") => handle_store(&body, config),
        ("POST", "/recall") => handle_recall(&body, config),
        ("POST", "/query") => handle_mql(&body, config),
        _ => {
            let resp = tiny_http::Response::from_string("{\"error\":\"not found\"}")
                .with_status_code(404)
                .with_header(content_type_json());
            let _ = request.respond(resp);
            return;
        }
    };

    respond(request, result, cors);
}

fn content_type_json() -> tiny_http::Header {
    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()
}

fn respond(request: tiny_http::Request, result: Result<String, String>, cors: &str) {
    match result {
        Ok(json) => {
            let mut resp = tiny_http::Response::from_string(&json)
                .with_header(content_type_json());
            if !cors.is_empty() {
                resp = resp.with_header(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Origin", cors.as_bytes()).unwrap()
                );
            }
            let _ = request.respond(resp);
        }
        Err(e) => {
            let body = format!("{{\"error\":\"{}\"}}", e.replace('"', "\\\""));
            let resp = tiny_http::Response::from_string(&body)
                .with_status_code(400)
                .with_header(content_type_json());
            let _ = request.respond(resp);
        }
    }
}

fn handle_stats(config: &Config) -> Result<String, String> {
    let reader = MicroscopeReader::open(config);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let stats = StatsResponse {
        block_count: reader.block_count,
        append_count: appended.len(),
        depths: reader.depth_ranges.iter().enumerate()
            .map(|(d, &(_, c))| DepthInfo { depth: d, count: c })
            .collect(),
    };

    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

fn handle_find(url: &str, config: &Config) -> Result<String, String> {
    let params = parse_query_params(url);
    let q = params.get("q").cloned().unwrap_or_default();
    let k: usize = params.get("k").and_then(|s| s.parse().ok()).unwrap_or(10);

    if q.is_empty() {
        return Err("missing query parameter 'q'".to_string());
    }

    let reader = MicroscopeReader::open(config);
    let results = reader.find_text(&q, k);

    let entries: Vec<ResultEntry> = results.iter().map(|&(_, idx)| {
        let h = reader.header(idx);
        ResultEntry {
            text: reader.text(idx).to_string(),
            depth: h.depth,
            layer: LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?").to_string(),
            score: 0.0,
            is_append: false,
        }
    }).collect();

    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

fn handle_store(body: &str, config: &Config) -> Result<String, String> {
    let req: StoreRequest = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;
    store_memory(config, &req.text, &req.layer, req.importance);
    Ok("{\"status\":\"stored\"}".to_string())
}

fn handle_recall(body: &str, config: &Config) -> Result<String, String> {
    let req: RecallRequest = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    let reader = MicroscopeReader::open(config);
    let (qx, qy, qz) = content_coords_blended(&req.query, "long_term", config.search.semantic_weight);

    let q_lower = req.query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

    let (zoom_lo, zoom_hi) = match req.query.len() {
        0..=10 => (0, 3),
        11..=40 => (3, 6),
        _ => (6, 8),
    };

    let mut all: Vec<(f32, usize, bool)> = Vec::new();

    for zoom in zoom_lo..=zoom_hi {
        let (start, count) = reader.depth_ranges[zoom as usize];
        let (start, count) = (start as usize, count as usize);
        for i in start..(start + count) {
            let text = reader.text(i).to_lowercase();
            let hits = keywords.iter().filter(|&&kw| text.contains(kw)).count();
            if hits > 0 {
                let h = reader.header(i);
                let dx = h.x - qx;
                let dy = h.y - qy;
                let dz = h.z - qz;
                let dist = dx*dx + dy*dy + dz*dz;
                let boost = hits as f32 * 0.1;
                all.push(((dist - boost).max(0.0), i, true));
            }
        }
    }

    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (ai, entry) in appended.iter().enumerate() {
        let dx = entry.x - qx;
        let dy = entry.y - qy;
        let dz = entry.z - qz;
        let dist = dx*dx + dy*dy + dz*dz;
        let text_lower = entry.text.to_lowercase();
        let hits = keywords.iter().filter(|&&kw| text_lower.contains(kw)).count();
        if dist < 0.1 || hits > 0 {
            let boost = hits as f32 * 0.1;
            all.push(((dist - boost).max(0.0), ai + 1_000_000, false));
        }
    }

    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    all.truncate(req.k);

    let entries: Vec<ResultEntry> = all.iter().map(|&(score, idx, is_main)| {
        if is_main {
            let h = reader.header(idx);
            ResultEntry {
                text: reader.text(idx).to_string(),
                depth: h.depth,
                layer: LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?").to_string(),
                score,
                is_append: false,
            }
        } else {
            let ai = idx - 1_000_000;
            let e = &appended[ai];
            ResultEntry {
                text: e.text.clone(),
                depth: e.depth,
                layer: LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?").to_string(),
                score,
                is_append: true,
            }
        }
    }).collect();

    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

fn handle_mql(body: &str, config: &Config) -> Result<String, String> {
    let req: MqlRequest = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    let q = crate::query::parse(&req.mql);
    let reader = MicroscopeReader::open(config);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let results = crate::query::execute(&q, &reader, &appended);

    let entries: Vec<ResultEntry> = results.iter().map(|r| {
        if r.is_main {
            let h = reader.header(r.block_idx);
            ResultEntry {
                text: reader.text(r.block_idx).to_string(),
                depth: h.depth,
                layer: LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?").to_string(),
                score: r.score,
                is_append: false,
            }
        } else {
            let ai = r.block_idx - 1_000_000;
            let e = &appended[ai];
            ResultEntry {
                text: e.text.clone(),
                depth: e.depth,
                layer: LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?").to_string(),
                score: r.score,
                is_append: true,
            }
        }
    }).collect();

    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

fn parse_query_params(url: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    if let Some(query) = url.split('?').nth(1) {
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                let decoded = v.replace("%20", " ").replace('+', " ");
                params.insert(k.to_string(), decoded);
            }
        }
    }
    params
}
