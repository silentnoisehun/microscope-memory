use crate::config::Config;
use crate::{MicroscopeReader, LAYER_NAMES};
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
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
    fs::create_dir_all(&ns_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("create namespace dir: {}", e)))?;
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
    fs::create_dir_all(&ns_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("create shared namespace dir: {}", e)))?;
    let mut scoped = config.clone();
    scoped.paths.output_dir = ns_dir.to_string_lossy().to_string();
    Ok((scoped, backend, ns_dir))
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
    let reader = MicroscopeReader::open(&state.config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (user_config, _user_id, memory_backend, _namespace) = scoped_config(
        &state.config,
        params.user_id.as_deref(),
        params.memory_backend.as_deref(),
    )?;
    let (shared_cfg, _backend2, _shared_ns) = shared_config(&state.config, Some(&memory_backend))?;
    let scope = sanitize_memory_scope(params.memory_scope.as_deref());

    let k = params.k.unwrap_or(10);
    let query_lower = params.q.to_lowercase();
    let mut response = Vec::new();

    // Prefer session memory first (personal/shared) then global index.
    if matches!(scope, MemoryScope::Personal | MemoryScope::Both) {
        let append_path = Path::new(&user_config.paths.output_dir).join("append.bin");
        let appended = crate::read_append_log(&append_path);
        for entry in &appended {
            if response.len() >= k {
                break;
            }
            if entry.text.to_lowercase().contains(&query_lower) {
                let layer = LAYER_NAMES
                    .get(entry.layer_id as usize)
                    .copied()
                    .unwrap_or("long_term")
                    .to_string();
                response.push(MemoryResponse {
                    text: entry.text.clone(),
                    depth: 4,
                    layer,
                    distance: 0.05,
                    memory_scope: "personal".to_string(),
                });
            }
        }
    }

    if matches!(scope, MemoryScope::Shared | MemoryScope::Both) {
        let append_path = Path::new(&shared_cfg.paths.output_dir).join("append.bin");
        let appended = crate::read_append_log(&append_path);
        for entry in &appended {
            if response.len() >= k {
                break;
            }
            if entry.text.to_lowercase().contains(&query_lower) {
                let layer = LAYER_NAMES
                    .get(entry.layer_id as usize)
                    .copied()
                    .unwrap_or("long_term")
                    .to_string();
                response.push(MemoryResponse {
                    text: entry.text.clone(),
                    depth: 4,
                    layer,
                    distance: 0.08,
                    memory_scope: "shared".to_string(),
                });
            }
        }
    }

    // --- Search main index ---
    let (qx, qy, qz) =
        crate::content_coords_blended(&params.q, "long_term", state.config.search.semantic_weight);
    let depth = crate::auto_depth(&params.q);
    let results = reader.radial_search(&state.config, qx, qy, qz, depth, 0.5, k);
    for res in results.all() {
        if response.len() >= k {
            break;
        }
        if !res.is_main {
            continue;
        }
        let h = reader.header(res.block_idx);
        let text = reader.text(res.block_idx).to_string();
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
            memory_scope: "global".to_string(),
        });
    }

    Ok(Json(response))
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
<tr><td>GET</td><td><a href="/openapi.json">/openapi.json</a></td><td>OpenAPI spec</td></tr>
</table>

<h3>Quick Start</h3>
<pre># Recall via v1 API
curl "http://localhost:6060/v1/recall?q=hello&amp;k=3&amp;user_id=alice&amp;memory_backend=cloud&amp;memory_scope=both"

# Store via v1 API
curl -X POST http://localhost:6060/v1/remember \
  -H "Content-Type: application/json" \
  -d '{"text":"Hello world","layer":"long_term","user_id":"alice","memory_backend":"cloud","memory_scope":"both"}'</pre>

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
        .route("/remember", post(remember_memory));

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
