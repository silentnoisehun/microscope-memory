use crate::config::Config;
use crate::{MicroscopeReader, LAYER_NAMES};
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
}

#[derive(Deserialize)]
pub struct RecallQuery {
    pub q: String,
    pub k: Option<usize>,
    pub user_id: Option<String>,
    pub memory_backend: Option<String>,
    pub memory_scope: Option<String>,
}

#[derive(Serialize)]
pub struct MemoryResponse {
    pub text: String,
    pub depth: u8,
    pub layer: String,
    pub distance: f32,
    pub memory_scope: String,
}

#[derive(Deserialize)]
pub struct RememberRequest {
    pub text: String,
    pub layer: Option<String>,
    pub importance: Option<u8>,
    pub user_id: Option<String>,
    pub memory_backend: Option<String>,
    pub memory_scope: Option<String>,
}

#[derive(Deserialize)]
pub struct SessionQuery {
    pub user_id: Option<String>,
    pub memory_backend: Option<String>,
    pub memory_scope: Option<String>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub version: String,
    pub blocks: usize,
    pub append_log: usize,
    pub layers: Vec<String>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub user_id: String,
    pub memory_backend: String,
    pub memory_scope: String,
    pub namespace_dir: String,
    pub personal_namespace_dir: String,
    pub shared_namespace_dir: String,
}

#[derive(Clone, Copy)]
enum MemoryScope {
    Personal,
    Shared,
    Both,
}

impl MemoryScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Shared => "shared",
            Self::Both => "both",
        }
    }
}

fn sanitize_user_id(raw: Option<&str>) -> String {
    let src = raw.unwrap_or("guest");
    let cleaned: String = src
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "guest".to_string()
    } else {
        cleaned
    }
}

fn sanitize_memory_backend(raw: Option<&str>) -> String {
    match raw.unwrap_or("local").to_ascii_lowercase().as_str() {
        "cloud" => "cloud".to_string(),
        _ => "local".to_string(),
    }
}

fn sanitize_memory_scope(raw: Option<&str>) -> MemoryScope {
    match raw.unwrap_or("both").to_ascii_lowercase().as_str() {
        "personal" => MemoryScope::Personal,
        "shared" => MemoryScope::Shared,
        _ => MemoryScope::Both,
    }
}

fn namespace_dir(config: &Config, user_id: &str, memory_backend: &str) -> PathBuf {
    Path::new(&config.paths.output_dir)
        .join("namespaces")
        .join(memory_backend)
        .join(user_id)
}

fn shared_namespace_dir(config: &Config, memory_backend: &str) -> PathBuf {
    Path::new(&config.paths.output_dir)
        .join("namespaces")
        .join(memory_backend)
        .join("_shared")
}

fn scoped_config(
    config: &Config,
    user_id: Option<&str>,
    memory_backend: Option<&str>,
) -> Result<(Config, String, String, PathBuf), (StatusCode, String)> {
    let user = sanitize_user_id(user_id);
    let backend = sanitize_memory_backend(memory_backend);
    let ns_dir = namespace_dir(config, &user, &backend);
    fs::create_dir_all(&ns_dir).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("create namespace dir: {}", e),
        )
    })?;
    let mut scoped = config.clone();
    scoped.paths.output_dir = ns_dir.to_string_lossy().to_string();
    Ok((scoped, user, backend, ns_dir))
}

fn shared_config(
    config: &Config,
    memory_backend: Option<&str>,
) -> Result<(Config, String, PathBuf), (StatusCode, String)> {
    let backend = sanitize_memory_backend(memory_backend);
    let ns_dir = shared_namespace_dir(config, &backend);
    fs::create_dir_all(&ns_dir).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("create shared namespace dir: {}", e),
        )
    })?;
    let mut scoped = config.clone();
    scoped.paths.output_dir = ns_dir.to_string_lossy().to_string();
    Ok((scoped, backend, ns_dir))
}

#[derive(Deserialize)]
pub struct MobileRecallRequest {
    pub user_id: String,
    pub query: String,
    pub k: Option<usize>,
}

#[derive(Deserialize)]
pub struct MobileRememberRequest {
    pub user_id: String,
    pub text: String,
    pub layer: Option<String>,
    pub importance: Option<u8>,
}

#[derive(Deserialize)]
pub struct MobileChatRequest {
    pub user_id: String,
    pub message: String,
    pub provider: String, // ollama | openai | gemini
    pub model: String,
    pub api_base: Option<String>,
    pub recall_k: Option<usize>,
    pub system_prompt: Option<String>,
    pub layer: Option<String>,
    pub importance: Option<u8>,
    pub remember_user: Option<bool>,
    pub remember_assistant: Option<bool>,
    pub temperature: Option<f32>,
    pub extra_headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize)]
pub struct MobileChatResponse {
    pub provider: String,
    pub model: String,
    pub reply: String,
    pub recalled: Vec<MemoryResponse>,
    pub stored: Vec<String>,
}

fn user_prefix(user_id: &str) -> String {
    format!("[user:{}] ", user_id.trim())
}

fn scope_user_text(user_id: &str, text: &str) -> String {
    format!("{}{}", user_prefix(user_id), text)
}

fn strip_user_scope(text: &str, user_id: &str) -> Option<String> {
    let prefix = user_prefix(user_id);
    text.strip_prefix(&prefix).map(str::to_string)
}

fn recall_internal(
    state: &AppState,
    query: &str,
    k: usize,
    user_scope: Option<&str>,
) -> Result<Vec<MemoryResponse>, (StatusCode, String)> {
    let reader = MicroscopeReader::open(&state.config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let query_lower = query.to_lowercase();
    let (qx, qy, qz) =
        crate::content_coords_blended(query, "long_term", state.config.search.semantic_weight);
    let depth = crate::auto_depth(query);
    let results = reader.radial_search(&state.config, qx, qy, qz, depth, 0.5, k);

    let mut response = Vec::new();
    for res in results.all() {
        if !res.is_main {
            continue;
        }
        let h = reader.header(res.block_idx);
        let raw_text = reader.text(res.block_idx).to_string();
        let text = if let Some(uid) = user_scope {
            match strip_user_scope(&raw_text, uid) {
                Some(t) => t,
                None => continue,
            }
        } else {
            raw_text
        };
        let layer = LAYER_NAMES
            .get(h.layer_id as usize)
            .copied()
            .unwrap_or("unknown")
            .to_string();
        response.push(MemoryResponse {
            text,
            depth: h.depth,
            layer,
            distance: res.dist_sq.sqrt(),
            memory_scope: if user_scope.is_some() {
                "personal"
            } else {
                "shared"
            }
            .to_string(),
        });
    }

    let append_path = std::path::Path::new(&state.config.paths.output_dir).join("append.bin");
    let appended = crate::read_append_log(&append_path);
    for entry in &appended {
        if response.len() >= k {
            break;
        }
        if !entry.text.to_lowercase().contains(&query_lower) {
            continue;
        }
        let text = if let Some(uid) = user_scope {
            match strip_user_scope(&entry.text, uid) {
                Some(t) => t,
                None => continue,
            }
        } else {
            entry.text.clone()
        };
        let layer = LAYER_NAMES
            .get(entry.layer_id as usize)
            .copied()
            .unwrap_or("long_term")
            .to_string();
        response.push(MemoryResponse {
            text,
            depth: 4,
            layer,
            distance: 0.1,
            memory_scope: if user_scope.is_some() {
                "personal"
            } else {
                "shared"
            }
            .to_string(),
        });
    }

    Ok(response)
}

async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let reader = MicroscopeReader::open(&state.config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let append_path = std::path::Path::new(&state.config.paths.output_dir).join("append.bin");
    let appended = crate::read_append_log(&append_path);

    Ok(Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        blocks: reader.block_count,
        append_log: appended.len(),
        layers: LAYER_NAMES.iter().map(|&s| s.to_string()).collect(),
    }))
}

async fn recall_memory(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RecallQuery>,
) -> Result<Json<Vec<MemoryResponse>>, (StatusCode, String)> {
    let k = params.k.unwrap_or(10);
    Ok(Json(recall_internal(&state, &params.q, k, None)?))
}

async fn remember_memory(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RememberRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let layer = payload.layer.unwrap_or_else(|| "long_term".to_string());
    let importance = payload.importance.unwrap_or(5);
    let scope = sanitize_memory_scope(payload.memory_scope.as_deref());
    let (user_config, user_id, memory_backend, personal_ns) = scoped_config(
        &state.config,
        payload.user_id.as_deref(),
        payload.memory_backend.as_deref(),
    )?;
    let (shared_cfg, _backend2, shared_ns) = shared_config(&state.config, Some(&memory_backend))?;
    let mut written_scopes = Vec::new();

    if matches!(scope, MemoryScope::Personal | MemoryScope::Both) {
        crate::store_memory(&user_config, &payload.text, &layer, importance)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        written_scopes.push("personal");
    }
    if matches!(scope, MemoryScope::Shared | MemoryScope::Both) {
        crate::store_memory(&shared_cfg, &payload.text, &layer, importance)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        written_scopes.push("shared");
    }

    Ok(Json(serde_json::json!({
        "status": "committed",
        "message": "Memory stored in append log",
        "user_id": user_id,
        "memory_backend": memory_backend,
        "memory_scope": scope.as_str(),
        "written_scopes": written_scopes,
        "namespace_dir": personal_ns.to_string_lossy(),
        "personal_namespace_dir": personal_ns.to_string_lossy(),
        "shared_namespace_dir": shared_ns.to_string_lossy()
    })))
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SessionQuery>,
) -> Result<Json<SessionResponse>, (StatusCode, String)> {
    let (_cfg, user_id, memory_backend, namespace) = scoped_config(
        &state.config,
        params.user_id.as_deref(),
        params.memory_backend.as_deref(),
    )?;
    let (_shared_cfg, _backend2, shared_ns) = shared_config(&state.config, Some(&memory_backend))?;
    let scope = sanitize_memory_scope(params.memory_scope.as_deref());
    Ok(Json(SessionResponse {
        user_id,
        memory_backend,
        memory_scope: scope.as_str().to_string(),
        namespace_dir: namespace.to_string_lossy().to_string(),
        personal_namespace_dir: namespace.to_string_lossy().to_string(),
        shared_namespace_dir: shared_ns.to_string_lossy().to_string(),
    }))
}

async fn mobile_recall(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MobileRecallRequest>,
) -> Result<Json<Vec<MemoryResponse>>, (StatusCode, String)> {
    let k = payload.k.unwrap_or(10);
    Ok(Json(recall_internal(
        &state,
        &payload.query,
        k,
        Some(&payload.user_id),
    )?))
}

async fn mobile_remember(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MobileRememberRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let layer = payload.layer.unwrap_or_else(|| "long_term".to_string());
    let importance = payload.importance.unwrap_or(7);
    let scoped_text = scope_user_text(&payload.user_id, &payload.text);

    crate::store_memory(&state.config, &scoped_text, &layer, importance)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "committed",
        "message": "User-scoped memory stored"
    })))
}

fn build_header_map(
    extra_headers: Option<&std::collections::HashMap<String, String>>,
    auth: Option<String>,
) -> Result<HeaderMap, (StatusCode, String)> {
    let mut headers = HeaderMap::new();

    if let Some(token) = auth {
        let value = HeaderValue::from_str(&token).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid auth header: {}", e),
            )
        })?;
        headers.insert(AUTHORIZATION, value);
    }

    // Whitelist of allowed extra headers for security
    const ALLOWED_HEADERS: &[&str] = &[
        "accept",
        "content-type",
        "user-agent",
        "x-request-id",
        "x-api-key",
        "x-client-version",
    ];

    if let Some(map) = extra_headers {
        for (k, v) in map {
            let lower_k = k.to_lowercase();
            // Allow headers that start with "x-" or are in the whitelist
            if !lower_k.starts_with("x-") && !ALLOWED_HEADERS.contains(&lower_k.as_str()) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Header '{}' is not allowed. Only 'x-*' headers or whitelisted headers are permitted.", k),
                ));
            }

            let name = HeaderName::from_bytes(k.as_bytes()).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid header name '{}': {}", k, e),
                )
            })?;
            let value = HeaderValue::from_str(v).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid header value for '{}': {}", k, e),
                )
            })?;
            headers.insert(name, value);
        }
    }

    Ok(headers)
}

async fn call_provider(
    req: &MobileChatRequest,
    memory_context: &str,
    config: &Config,
) -> Result<String, (StatusCode, String)> {
    let provider = req.provider.to_lowercase();
    let client = reqwest::Client::new();
    let temperature = req.temperature.unwrap_or(0.2);
    let system_prompt = req.system_prompt.clone().unwrap_or_else(|| {
        "You are a helpful assistant. Use recalled memories as trusted user context.".to_string()
    });
    let combined_system = format!(
        "{}\n\nRecalled Memories:\n{}",
        system_prompt, memory_context
    );
    let headers = build_header_map(req.extra_headers.as_ref(), None)?;

    let response_json: Value = match provider.as_str() {
        "ollama" => {
            let api_base = req
                .api_base
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
            let url = format!("{}/api/chat", api_base.trim_end_matches('/'));
            let payload = serde_json::json!({
                "model": req.model,
                "stream": false,
                "messages": [
                    {"role": "system", "content": combined_system},
                    {"role": "user", "content": req.message}
                ],
                "options": {"temperature": temperature}
            });
            client
                .post(url)
                .headers(headers)
                .json(&payload)
                .send()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Ollama request failed: {}", e),
                    )
                })?
                .json()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Ollama invalid JSON: {}", e),
                    )
                })?
        }
        "openai" => {
            let api_base = req
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".to_string());
            let api_key = config.server.openai_api_key.as_ref().ok_or((
                StatusCode::INTERNAL_SERVER_ERROR,
                "OpenAI API key not configured on server".to_string(),
            ))?;
            let url = format!("{}/v1/chat/completions", api_base.trim_end_matches('/'));
            let mut headers = headers;
            let bearer = format!("Bearer {}", api_key);
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&bearer).map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Invalid OpenAI api_key: {}", e),
                    )
                })?,
            );
            let payload = serde_json::json!({
                "model": req.model,
                "temperature": temperature,
                "messages": [
                    {"role": "system", "content": combined_system},
                    {"role": "user", "content": req.message}
                ]
            });
            client
                .post(url)
                .headers(headers)
                .json(&payload)
                .send()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("OpenAI request failed: {}", e),
                    )
                })?
                .json()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("OpenAI invalid JSON: {}", e),
                    )
                })?
        }
        "gemini" => {
            let api_base = req
                .api_base
                .clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());
            let api_key = config.server.gemini_api_key.as_ref().ok_or((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gemini API key not configured on server".to_string(),
            ))?;
            let url = format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                api_base.trim_end_matches('/'),
                req.model,
                api_key
            );
            let payload = serde_json::json!({
                "contents": [
                    {
                        "role": "user",
                        "parts": [
                            { "text": format!("{}\n\nUser: {}", combined_system, req.message) }
                        ]
                    }
                ],
                "generationConfig": {
                    "temperature": temperature
                }
            });
            client
                .post(url)
                .headers(headers)
                .json(&payload)
                .send()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Gemini request failed: {}", e),
                    )
                })?
                .json()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_GATEWAY,
                        format!("Gemini invalid JSON: {}", e),
                    )
                })?
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Unsupported provider. Use: ollama | openai | gemini".to_string(),
            ));
        }
    };

    let reply = match provider.as_str() {
        "ollama" => response_json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        "openai" => response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        "gemini" => response_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };

    if reply.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("{} returned an empty response", req.provider),
        ));
    }

    Ok(reply)
}

async fn mobile_chat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MobileChatRequest>,
) -> Result<Json<MobileChatResponse>, (StatusCode, String)> {
    let k = payload.recall_k.unwrap_or(8);
    let recalled = recall_internal(&state, &payload.message, k, Some(&payload.user_id))?;

    let memory_context = if recalled.is_empty() {
        "(no prior memory)".to_string()
    } else {
        recalled
            .iter()
            .enumerate()
            .map(|(i, m)| {
                format!(
                    "{}. [{} D{} dist={:.3}] {}",
                    i + 1,
                    m.layer,
                    m.depth,
                    m.distance,
                    m.text
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    };

    let reply = call_provider(&payload, &memory_context, &state.config).await?;

    let layer = payload
        .layer
        .clone()
        .unwrap_or_else(|| "long_term".to_string());
    let importance = payload.importance.unwrap_or(7);
    let mut stored = Vec::new();

    if payload.remember_user.unwrap_or(true) {
        let text = scope_user_text(&payload.user_id, &payload.message);
        crate::store_memory(&state.config, &text, &layer, importance)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        stored.push("user_message".to_string());
    }

    if payload.remember_assistant.unwrap_or(true) {
        let text = scope_user_text(&payload.user_id, &reply);
        crate::store_memory(&state.config, &text, &layer, importance)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        stored.push("assistant_reply".to_string());
    }

    Ok(Json(MobileChatResponse {
        provider: payload.provider,
        model: payload.model,
        reply,
        recalled,
        stored,
    }))
}

async fn get_openapi() -> Json<serde_json::Value> {
    static SPEC: &str = include_str!("../openapi.json");
    Json(
        serde_json::from_str(SPEC)
            .unwrap_or_else(|_| serde_json::json!({"error": "spec parse failed"})),
    )
}

async fn get_root() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!DOCTYPE html>
<html><head><title>Microscope Memory — Spine Bridge API</title>
<style>
    body { font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; max-width: 800px; margin: 40px auto; line-height: 1.6; background: #0f1115; color: #e0e0e0; }
    h1 { color: #00f2ff; border-bottom: 1px solid #333; padding-bottom: 10px; }
    code { background: #1a1d23; padding: 2px 5px; border-radius: 4px; color: #ff9d00; }
    table { width: 100%; border-collapse: collapse; margin: 20px 0; background: #16181d; }
    th, td { border: 1px solid #333; padding: 12px; text-align: left; }
    th { background: #1e2229; color: #00f2ff; }
    a { color: #00f2ff; text-decoration: none; }
    a:hover { text-decoration: underline; }
    .status-ok { color: #00ff88; font-weight: bold; }
    pre { background: #1a1d23; padding: 15px; border-radius: 8px; border: 1px solid #333; overflow-x: auto; }
</style>
</head>
<body>
<h1>🔬 Microscope Memory — Spine Bridge API <span style="font-size: 0.5em; vertical-align: middle; background: #00f2ff; color: #000; padding: 2px 8px; border-radius: 10px;">v1.0</span></h1>
<p>Sub-microsecond cognitive memory for AI models. <span class="status-ok">● Engine Active</span></p>

<h3>API Endpoints (v1)</h3>
<table>
<tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
<tr><td>GET</td><td><code>/v1/status</code></td><td>Engine health &amp; stats</td></tr>
<tr><td>GET</td><td><code>/v1/session?user_id=u1&amp;memory_backend=cloud&amp;memory_scope=both</code></td><td>Resolve personal+shared namespaces</td></tr>
<tr><td>GET</td><td><code>/v1/recall?q=...&amp;k=10</code></td><td>Recall memories by query</td></tr>
<tr><td>POST</td><td><code>/v1/remember</code></td><td>Store a new memory</td></tr>
<tr><td>POST</td><td><code>/v1/mobile/chat</code></td><td>Provider-agnostic mobile chat (Ollama/OpenAI/Gemini)</td></tr>
<tr><td>GET</td><td><a href="/openapi.json">/openapi.json</a></td><td>OpenAPI spec</td></tr>
</table>

<h3>Quick Start</h3>
<pre># Recall via v1 API
curl "http://localhost:6060/v1/recall?q=hello&amp;k=3&amp;user_id=alice&amp;memory_backend=cloud&amp;memory_scope=both"

# Store via v1 API
curl -X POST http://localhost:6060/v1/remember \
  -H "Content-Type: application/json" \
  -d '{"text":"Hello world","layer":"long_term","user_id":"alice","memory_backend":"cloud","memory_scope":"both"}'

# Mobile unified chat (user-scoped persistent memory)
curl -X POST http://localhost:6060/v1/mobile/chat \
  -H "Content-Type: application/json" \
  -d '{"user_id":"demo-user","message":"what did we discuss?","provider":"ollama","model":"llama3"}'</pre>

<p style="font-size: 0.9em; color: #888;">Note: Legacy routes (<code>/status</code>, <code>/recall</code>, <code>/remember</code>) are supported but deprecated.</p>
</body></html>"#,
    )
}

pub async fn run(
    config: Config,
    host: String,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState { config });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let v1_routes = Router::new()
        .route("/status", get(get_status))
        .route("/session", get(get_session))
        .route("/recall", get(recall_memory))
        .route("/remember", post(remember_memory))
        .route("/mobile/recall", post(mobile_recall))
        .route("/mobile/remember", post(mobile_remember))
        .route("/mobile/chat", post(mobile_chat));

    let app = Router::new()
        .route("/", get(get_root))
        // v1 API
        .nest("/v1", v1_routes)
        // Backward compatibility (Legacy)
        .route("/status", get(get_status))
        .route("/session", get(get_session))
        .route("/recall", get(recall_memory))
        .route("/remember", post(remember_memory))
        .route("/openapi.json", get(get_openapi))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr_str = format!("{}:{}", host, port);
    let addr: SocketAddr = addr_str.parse()?;

    eprintln!(
        "Microscope Memory Spine Bridge API starting on http://{}",
        addr
    );
    eprintln!("  OpenAPI spec: http://{}/openapi.json", addr);
    eprintln!("  Import URL into ChatGPT/Claude for tool access.");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
