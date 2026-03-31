//! HTTP server for Microscope Memory — powered by tiny_http.
//!
//! Endpoints:
//!   GET  /stats                  → index statistics + cache stats
//!   POST /store                  → store a memory {"text":"...", "layer":"...", "importance": N}
//!   GET  /find?q=...&k=N         → text search
//!   POST /recall                 → recall query {"query":"...", "k": N}
//!   POST /query                  → MQL query {"mql":"..."}
//!   GET  /recall/stream?q=...&k=N → SSE streaming recall
//!   POST /federated/recall       → federated recall across indices
//!   POST /federated/find         → federated text search
//!   GET  /health                 → health check

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

use crate::cache::QueryCache;
use crate::config::Config;
use crate::{content_coords_blended, read_append_log, store_memory, MicroscopeReader, LAYER_NAMES};

#[derive(Deserialize)]
struct StoreRequest {
    text: String,
    #[serde(default = "default_layer")]
    layer: String,
    #[serde(default = "default_importance")]
    importance: u8,
}

fn default_layer() -> String {
    "long_term".to_string()
}
fn default_importance() -> u8 {
    5
}

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

#[derive(Deserialize)]
struct FederatedRecallRequest {
    query: String,
    #[serde(default = "default_k")]
    k: usize,
}

#[derive(Deserialize)]
struct FederatedFindRequest {
    query: String,
    #[serde(default = "default_k")]
    k: usize,
}

fn default_k() -> usize {
    10
}

#[derive(Serialize, Clone)]
struct ResultEntry {
    text: String,
    depth: u8,
    layer: String,
    score: f32,
    is_append: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_index: Option<String>,
}

#[derive(Serialize)]
struct StatsResponse {
    block_count: usize,
    append_count: usize,
    depths: Vec<DepthInfo>,
    cache: CacheStatsResponse,
}

#[derive(Serialize)]
struct CacheStatsResponse {
    tier1_entries: usize,
    tier1_hits: u64,
    tier2_entries: usize,
    tier2_hits: u64,
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
    println!("  GET  /recall/stream?q=...&k=N  (SSE)");
    println!("  POST /store");
    println!("  POST /recall");
    println!("  POST /query  (MQL)");
    println!("  POST /federated/recall");
    println!("  POST /federated/find");
    println!("  POST /rebuild  (hot reload)");

    let config = Arc::new(config);
    let cache = Arc::new(QueryCache::new(
        config.performance.cache_size,
        config.performance.cache_size * 4,
        config.performance.cache_ttl_secs,
    ));

    let pool_size = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4);
    let server = Arc::new(server);

    let mut handles = Vec::new();
    for _ in 0..pool_size {
        let server = Arc::clone(&server);
        let cfg = Arc::clone(&config);
        let cache = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            while let Ok(req) = server.recv() {
                handle_request(req, &cfg, &cache);
            }
        }));
    }

    for h in handles {
        let _ = h.join();
    }
}

fn handle_request(mut request: tiny_http::Request, config: &Config, cache: &QueryCache) {
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

    // SSE streaming endpoint — handled separately (streams response)
    if method == "GET" && path == "/recall/stream" {
        handle_recall_stream(request, &url, config, cors);
        return;
    }

    let result = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => Ok("{\"status\":\"ok\"}".to_string()),
        ("GET", "/stats") => handle_stats(config, cache),
        ("GET", "/find") => handle_find(&url, config, cache),
        ("POST", "/store") => handle_store(&body, config, cache),
        ("POST", "/recall") => handle_recall(&body, config, cache),
        ("POST", "/query") => handle_mql(&body, config, cache),
        ("POST", "/federated/recall") => handle_federated_recall(&body, config),
        ("POST", "/federated/find") => handle_federated_find(&body, config),
        ("POST", "/rebuild") => handle_rebuild(config, cache),
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
            let mut resp = tiny_http::Response::from_string(&json).with_header(content_type_json());
            if !cors.is_empty() {
                resp = resp.with_header(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Origin", cors.as_bytes())
                        .unwrap(),
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

// ─── Stats (with cache) ─────────────────────────────

fn handle_stats(config: &Config, cache: &QueryCache) -> Result<String, String> {
    let reader = MicroscopeReader::open(config)?;
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let cs = cache.stats();
    let stats = StatsResponse {
        block_count: reader.block_count,
        append_count: appended.len(),
        depths: reader
            .depth_ranges
            .iter()
            .enumerate()
            .map(|(d, &(_, c))| DepthInfo { depth: d, count: c })
            .collect(),
        cache: CacheStatsResponse {
            tier1_entries: cs.tier1_entries,
            tier1_hits: cs.tier1_hits,
            tier2_entries: cs.tier2_entries,
            tier2_hits: cs.tier2_hits,
        },
    };

    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

// ─── Find (with cache) ──────────────────────────────

fn handle_find(url: &str, config: &Config, cache: &QueryCache) -> Result<String, String> {
    let params = parse_query_params(url);
    let q = params.get("q").cloned().unwrap_or_default();
    let k: usize = params.get("k").and_then(|s| s.parse().ok()).unwrap_or(10);

    if q.is_empty() {
        return Err("missing query parameter 'q'".to_string());
    }

    // Check cache
    let cache_key = QueryCache::make_key("find", &q, k);
    if let Some(cached) = cache.get_query(&cache_key) {
        return Ok(cached);
    }

    let reader = MicroscopeReader::open(config)?;
    let results = reader.find_text(&q, k);

    let entries: Vec<ResultEntry> = results
        .iter()
        .map(|&(_, idx)| {
            let h = reader.header(idx);
            ResultEntry {
                text: reader.text(idx).to_string(),
                depth: h.depth,
                layer: LAYER_NAMES
                    .get(h.layer_id as usize)
                    .unwrap_or(&"?")
                    .to_string(),
                score: 0.0,
                is_append: false,
                source_index: None,
            }
        })
        .collect();

    let json = serde_json::to_string(&entries).map_err(|e| e.to_string())?;
    cache.insert_query(cache_key, json.clone());
    Ok(json)
}

// ─── Store (invalidates cache) ──────────────────────

fn handle_store(body: &str, config: &Config, cache: &QueryCache) -> Result<String, String> {
    let req: StoreRequest =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;
    store_memory(config, &req.text, &req.layer, req.importance)?;
    cache.invalidate_all();
    Ok("{\"status\":\"stored\"}".to_string())
}

// ─── Recall (with cache) ────────────────────────────

fn handle_recall(body: &str, config: &Config, cache: &QueryCache) -> Result<String, String> {
    let req: RecallRequest =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    // Check cache
    let cache_key = QueryCache::make_key("recall", &req.query, req.k);
    if let Some(cached) = cache.get_query(&cache_key) {
        return Ok(cached);
    }

    let reader = MicroscopeReader::open(config)?;
    let entries = recall_core(&reader, config, &req.query, req.k);
    let json = serde_json::to_string(&entries).map_err(|e| e.to_string())?;
    cache.insert_query(cache_key, json.clone());
    Ok(json)
}

// ─── MQL (with cache) ──────────────────────────────

fn handle_mql(body: &str, config: &Config, cache: &QueryCache) -> Result<String, String> {
    let req: MqlRequest = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    let cache_key = QueryCache::make_key("mql", &req.mql, 0);
    if let Some(cached) = cache.get_query(&cache_key) {
        return Ok(cached);
    }

    let q = crate::query::parse(&req.mql);
    let reader = MicroscopeReader::open(config)?;
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    let results = crate::query::execute(&q, &reader, &appended);

    let entries: Vec<ResultEntry> = results
        .iter()
        .map(|r| {
            if r.is_main {
                let h = reader.header(r.block_idx);
                ResultEntry {
                    text: reader.text(r.block_idx).to_string(),
                    depth: h.depth,
                    layer: LAYER_NAMES
                        .get(h.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score: r.score,
                    is_append: false,
                    source_index: None,
                }
            } else {
                let ai = r.block_idx - 1_000_000;
                let e = &appended[ai];
                ResultEntry {
                    text: e.text.clone(),
                    depth: e.depth,
                    layer: LAYER_NAMES
                        .get(e.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score: r.score,
                    is_append: true,
                    source_index: None,
                }
            }
        })
        .collect();

    let json = serde_json::to_string(&entries).map_err(|e| e.to_string())?;
    cache.insert_query(cache_key, json.clone());
    Ok(json)
}

// ─── SSE Streaming Recall ───────────────────────────

fn handle_recall_stream(request: tiny_http::Request, url: &str, config: &Config, cors: &str) {
    let params = parse_query_params(url);
    let query = params.get("q").cloned().unwrap_or_default();
    let k: usize = params.get("k").and_then(|s| s.parse().ok()).unwrap_or(10);

    if query.is_empty() {
        let resp = tiny_http::Response::from_string("{\"error\":\"missing q parameter\"}")
            .with_status_code(400)
            .with_header(content_type_json());
        let _ = request.respond(resp);
        return;
    }

    let reader = match MicroscopeReader::open(config) {
        Ok(r) => r,
        Err(e) => {
            let resp = tiny_http::Response::from_string(format!("{{\"error\":\"{}\"}}", e))
                .with_status_code(500)
                .with_header(content_type_json());
            let _ = request.respond(resp);
            return;
        }
    };

    // Build SSE response in a pipe: writer side produces events, reader side feeds tiny_http
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(64);

    let config_clone = config.clone();
    let query_clone = query.clone();
    std::thread::spawn(move || {
        let (qx, qy, qz) = content_coords_blended(
            &query_clone,
            "long_term",
            config_clone.search.semantic_weight,
        );
        let q_lower = query_clone.to_lowercase();
        let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

        let (zoom_lo, zoom_hi) = match query_clone.len() {
            0..=10 => (0u8, 3u8),
            11..=40 => (3, 6),
            _ => (6, 8),
        };

        let mut sent = 0usize;
        let mut all_results: Vec<(f32, ResultEntry)> = Vec::new();

        // Scan each zoom level and emit batch
        for zoom in zoom_lo..=zoom_hi {
            let (start, count) = reader.depth_ranges[zoom as usize];
            let (start, count) = (start as usize, count as usize);

            for i in start..(start + count) {
                let text = reader.text(i);
                let text_lower = text.to_lowercase();
                let hits = keywords
                    .iter()
                    .filter(|&&kw| text_lower.contains(kw))
                    .count();
                if hits > 0 {
                    let h = reader.header(i);
                    let dx = h.x - qx;
                    let dy = h.y - qy;
                    let dz = h.z - qz;
                    let dist = dx * dx + dy * dy + dz * dz;
                    let boost = hits as f32 * 0.1;
                    let score = (dist - boost).max(0.0);
                    all_results.push((
                        score,
                        ResultEntry {
                            text: text.to_string(),
                            depth: h.depth,
                            layer: LAYER_NAMES
                                .get(h.layer_id as usize)
                                .unwrap_or(&"?")
                                .to_string(),
                            score,
                            is_append: false,
                            source_index: None,
                        },
                    ));
                }
            }

            // Sort current results and emit the best unsent ones
            all_results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            for (_, entry) in all_results.iter().skip(sent).take(k.saturating_sub(sent)) {
                let json = match serde_json::to_string(entry) {
                    Ok(j) => j,
                    Err(_) => continue,
                };
                let event = format!("data: {}\n\n", json);
                if tx.send(event.into_bytes()).is_err() {
                    return; // client disconnected
                }
                sent += 1;
                if sent >= k {
                    break;
                }
            }
        }

        // Also check append log
        let append_path = Path::new(&config_clone.paths.output_dir).join("append.bin");
        let appended = read_append_log(&append_path);
        for entry in &appended {
            if sent >= k {
                break;
            }
            let text_lower = entry.text.to_lowercase();
            let hits = keywords
                .iter()
                .filter(|&&kw| text_lower.contains(kw))
                .count();
            let dx = entry.x - qx;
            let dy = entry.y - qy;
            let dz = entry.z - qz;
            let dist = dx * dx + dy * dy + dz * dz;
            if dist < 0.1 || hits > 0 {
                let boost = hits as f32 * 0.1;
                let score = (dist - boost).max(0.0);
                let result_entry = ResultEntry {
                    text: entry.text.clone(),
                    depth: entry.depth,
                    layer: LAYER_NAMES
                        .get(entry.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score,
                    is_append: true,
                    source_index: None,
                };
                let json = match serde_json::to_string(&result_entry) {
                    Ok(j) => j,
                    Err(_) => continue,
                };
                let event = format!("data: {}\n\n", json);
                if tx.send(event.into_bytes()).is_err() {
                    return;
                }
                sent += 1;
            }
        }

        // Send done event
        let _ = tx.send(b"event: done\ndata: {}\n\n".to_vec());
    });

    // Create a reader from the channel
    let pipe_reader = ChannelReader { rx };

    let mut headers = vec![
        tiny_http::Header::from_bytes("Content-Type", "text/event-stream").unwrap(),
        tiny_http::Header::from_bytes("Cache-Control", "no-cache").unwrap(),
        tiny_http::Header::from_bytes("Connection", "keep-alive").unwrap(),
    ];
    if !cors.is_empty() {
        headers.push(
            tiny_http::Header::from_bytes("Access-Control-Allow-Origin", cors.as_bytes()).unwrap(),
        );
    }

    let response = tiny_http::Response::new(
        tiny_http::StatusCode(200),
        headers,
        pipe_reader,
        None, // unknown content length → chunked transfer
        None,
    );
    let _ = request.respond(response);
}

/// Adapter: reads from mpsc channel as an io::Read.
struct ChannelReader {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl std::io::Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.rx.recv() {
            Ok(data) => {
                let len = data.len().min(buf.len());
                buf[..len].copy_from_slice(&data[..len]);
                Ok(len)
            }
            Err(_) => Ok(0), // channel closed → EOF
        }
    }
}

// ─── Federated endpoints ────────────────────────────

fn handle_federated_recall(body: &str, config: &Config) -> Result<String, String> {
    let req: FederatedRecallRequest =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    let federation = crate::federation::FederatedSearch::from_config(config)?;
    let results = federation.recall(&req.query, req.k);

    let entries: Vec<ResultEntry> = results
        .into_iter()
        .map(|r| ResultEntry {
            text: r.text,
            depth: r.depth,
            layer: r.layer,
            score: r.score,
            is_append: r.is_append,
            source_index: Some(r.source_index),
        })
        .collect();

    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

fn handle_federated_find(body: &str, config: &Config) -> Result<String, String> {
    let req: FederatedFindRequest =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    let federation = crate::federation::FederatedSearch::from_config(config)?;
    let results = federation.find_text(&req.query, req.k);

    let entries: Vec<ResultEntry> = results
        .into_iter()
        .map(|r| ResultEntry {
            text: r.text,
            depth: r.depth,
            layer: r.layer,
            score: r.score,
            is_append: r.is_append,
            source_index: Some(r.source_index),
        })
        .collect();

    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

// ─── Shared recall logic ────────────────────────────

fn recall_core(
    reader: &MicroscopeReader,
    config: &Config,
    query: &str,
    k: usize,
) -> Vec<ResultEntry> {
    let (qx, qy, qz) = content_coords_blended(query, "long_term", config.search.semantic_weight);

    let q_lower = query.to_lowercase();
    let keywords: Vec<&str> = q_lower.split_whitespace().filter(|w| w.len() > 2).collect();

    let (zoom_lo, zoom_hi) = match query.len() {
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
                let dist = dx * dx + dy * dy + dz * dz;
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
        let dist = dx * dx + dy * dy + dz * dz;
        let text_lower = entry.text.to_lowercase();
        let hits = keywords
            .iter()
            .filter(|&&kw| text_lower.contains(kw))
            .count();
        if dist < 0.1 || hits > 0 {
            let boost = hits as f32 * 0.1;
            all.push(((dist - boost).max(0.0), ai + 1_000_000, false));
        }
    }

    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    all.truncate(k);

    all.iter()
        .map(|&(score, idx, is_main)| {
            if is_main {
                let h = reader.header(idx);
                ResultEntry {
                    text: reader.text(idx).to_string(),
                    depth: h.depth,
                    layer: LAYER_NAMES
                        .get(h.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score,
                    is_append: false,
                    source_index: None,
                }
            } else {
                let ai = idx - 1_000_000;
                let e = &appended[ai];
                ResultEntry {
                    text: e.text.clone(),
                    depth: e.depth,
                    layer: LAYER_NAMES
                        .get(e.layer_id as usize)
                        .unwrap_or(&"?")
                        .to_string(),
                    score,
                    is_append: true,
                    source_index: None,
                }
            }
        })
        .collect()
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

// ─── Hot Reload Rebuild ─────────────────────────────

fn handle_rebuild(config: &Config, cache: &QueryCache) -> Result<String, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    static REBUILDING: AtomicBool = AtomicBool::new(false);

    // Prevent concurrent rebuilds
    if REBUILDING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Ok("{\"status\":\"already_rebuilding\"}".to_string());
    }

    let start = Instant::now();
    let result = crate::build::build(config, true);

    // Clear cache so next queries use fresh data
    cache.invalidate_all();

    REBUILDING.store(false, Ordering::SeqCst);

    match result {
        Ok(()) => {
            let ms = start.elapsed().as_millis();
            println!("{} in {}ms (hot reload)", "REBUILD OK".green().bold(), ms);
            Ok(format!("{{\"status\":\"ok\",\"rebuild_ms\":{}}}", ms))
        }
        Err(e) => {
            println!("{}: {}", "REBUILD FAILED".red().bold(), e);
            Err(format!("rebuild failed: {}", e))
        }
    }
}
