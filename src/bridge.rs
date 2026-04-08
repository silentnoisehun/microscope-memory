use crate::config::Config;
use crate::{MicroscopeReader, LAYER_NAMES};
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
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
}

#[derive(Serialize)]
pub struct MemoryResponse {
    pub text: String,
    pub depth: u8,
    pub layer: String,
    pub distance: f32,
}

#[derive(Deserialize)]
pub struct RememberRequest {
    pub text: String,
    pub layer: Option<String>,
    pub importance: Option<u8>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub version: String,
    pub blocks: usize,
    pub append_log: usize,
    pub layers: Vec<String>,
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

    let k = params.k.unwrap_or(10);

    // Determine coordinates for the query
    let (qx, qy, qz) =
        crate::content_coords_blended(&params.q, "long_term", state.config.search.semantic_weight);

    // Depth for recall (default D3-D5 usually)
    let depth = crate::auto_depth(&params.q);

    // Use radial_search which is the correct method name in reader.rs
    let results = reader.radial_search(&state.config, qx, qy, qz, depth, 0.5, k);

    let mut response = Vec::new();
    for res in results.all() {
        let (h, text) = if res.is_main {
            (
                reader.header(res.block_idx),
                reader.text(res.block_idx).to_string(),
            )
        } else {
            // This is simplified, in a real app we'd read from the append log
            // For now, let's just show placeholder or handle it if we have 'appended' local
            continue;
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
            distance: res.dist_sq.sqrt(), // Result returns dist_sq
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

    crate::store_memory(&state.config, &payload.text, &layer, importance)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "committed",
        "message": "Memory stored in append log"
    })))
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
<html><head><title>Microscope Memory — Spine Bridge API</title></head>
<body style="font-family:monospace;max-width:700px;margin:40px auto;line-height:1.6">
<h1>🔬 Microscope Memory — Spine Bridge API</h1>
<p>Sub-microsecond cognitive memory for AI models.</p>
<table border="1" cellpadding="8" style="border-collapse:collapse;width:100%">
<tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
<tr><td>GET</td><td><a href="/status">/status</a></td><td>Engine health &amp; stats</td></tr>
<tr><td>GET</td><td><a href="/recall?q=test&k=3">/recall?q=...&amp;k=10</a></td><td>Recall memories by query</td></tr>
<tr><td>POST</td><td>/remember</td><td>Store a new memory</td></tr>
<tr><td>GET</td><td><a href="/openapi.json">/openapi.json</a></td><td>OpenAPI spec (import into ChatGPT/Claude)</td></tr>
</table>
<h2>Quick Start</h2>
<pre>curl "http://localhost:6060/recall?q=hello&amp;k=3"
curl -X POST http://localhost:6060/remember \
  -H "Content-Type: application/json" \
  -d '{"text":"Hello world","layer":"long_term","importance":7}'</pre>
<p>Import <a href="/openapi.json">/openapi.json</a> into ChatGPT Custom GPT or Claude Projects.</p>
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

    let app = Router::new()
        .route("/", get(get_root))
        .route("/status", get(get_status))
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
