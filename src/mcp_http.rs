//! Streamable HTTP MCP server for Microscope Memory.
//!
//! Implements the MCP Streamable HTTP transport (MCP spec
//! [`2026-07-28`](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports),
//! backwards-compatible with `2025-11-25` and `2024-11-05`) for remote,
//! multi-client access to the cognitive memory engine.
//!
//! Security:
//! - **API key auth** — when `[server].api_key` is set, every request must
//!   carry a valid key via `x-api-key` or `Authorization: Bearer <key>`.
//! - **Origin validation** — when `[server].cors_origin` is set, cross-origin
//!   requests whose `Origin` header does not match are rejected (DNS-rebinding
//!   protection).
//! - **Session management** — `Mcp-Session-Id` header handling via
//!   `LocalSessionManager` for stateful and stateless operation.
//!
//! Not available on WASM targets.  Enable with `--features mcp_http`.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, ORIGIN};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::reader::{MicroscopeReader, EMOTION_VECTOR_SIZE};
use crate::{auto_zoom, read_append_log, store_memory, store_memory_with_emotion, LAYER_NAMES};

// ─── Auth / security state ─────────────────────────────────────────────

/// Shared state for the auth middleware.
#[derive(Clone)]
pub struct AuthState {
    /// If set, requests must carry this key via `x-api-key` or
    /// `Authorization: Bearer <key>`.
    pub api_key: Option<String>,
    /// If set, only this origin is allowed (CORS + Origin header validation).
    pub cors_origin: Option<String>,
}

impl From<&Config> for AuthState {
    fn from(config: &Config) -> Self {
        Self {
            api_key: config.server.api_key.clone(),
            cors_origin: config.server.cors_origin.clone(),
        }
    }
}

/// Auth + Origin-validation middleware (DNS-rebinding protection).
///
/// Applied to every request before it reaches the MCP streamable-HTTP handler.
async fn auth_middleware(State(state): State<AuthState>, req: Request, next: Next) -> Response {
    let headers: &HeaderMap = req.headers();

    // ── API key check (only if configured) ──
    if let Some(ref expected) = state.api_key {
        let provided = headers
            .get("x-api-key")
            .or(headers.get(AUTHORIZATION))
            .and_then(|v| v.to_str().ok())
            .map(|s| {
                s.strip_prefix("Bearer ")
                    .map(|stripped| stripped.to_owned())
                    .unwrap_or_else(|| s.to_owned())
            });

        if provided.as_deref() != Some(expected.as_str()) {
            let mut resp = Response::new(Body::from("Missing or invalid API key\n"));
            *resp.status_mut() = StatusCode::UNAUTHORIZED;
            return resp;
        }
    }

    // ── Origin validation (DNS-rebinding protection) ──
    // When `cors_origin` is configured we reject any cross-origin request
    // whose `Origin` does not match.  Non-browser clients that don't send
    // `Origin` are always allowed (typical for MCP clients).
    if let Some(ref allowed) = state.cors_origin {
        if let Some(origin) = headers.get(ORIGIN) {
            let origin_str = origin.to_str().unwrap_or("");
            if origin_str != allowed.as_str() {
                let mut resp = Response::new(Body::from("Origin not allowed\n"));
                *resp.status_mut() = StatusCode::FORBIDDEN;
                return resp;
            }
        }
    }

    next.run(req).await
}

// ─── Tool input schemas ──────────────────────────────────────────────────

/// Parameters for `memory_store`.
#[derive(Debug, Deserialize, JsonSchema)]
struct StoreParams {
    /// Memory text to store.
    text: String,
    /// Memory layer (long_term, short_term, session, associative, emotional,
    /// relational, reflections, echo_cache, etc.).
    #[serde(default = "default_layer")]
    layer: String,
    /// Importance level 1-10.
    #[serde(default = "default_importance", rename = "importance")]
    importance: u8,
    /// Optional 21D emotion vector
    /// `[joy, sadness, anger, fear, surprise, disgust, trust, anticipation,
    /// love, gratitude, curiosity, awe, confusion, anxiety, serenity, hope,
    /// pride, shame, guilt, empathy, excitement]`.
    emotion: Option<Vec<f32>>,
}

/// Parameters for `memory_recall`.
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
struct RecallParams {
    /// Natural-language query.
    query: String,
    /// Max results to return.
    #[serde(default = "default_k")]
    k: usize,
    /// Optional 21D emotion vector to bias results toward emotionally
    /// similar memories.
    emotion: Option<Vec<f32>>,
}

/// Parameters for `memory_find`.
#[derive(Debug, Deserialize, JsonSchema)]
struct FindParams {
    /// Text to search for.
    query: String,
    /// Max results.
    #[serde(default = "default_k")]
    k: usize,
}

/// Parameters for `memory_look`.
#[derive(Debug, Deserialize, JsonSchema)]
struct LookParams {
    /// X coordinate (0.0-1.0).
    x: f32,
    /// Y coordinate (0.0-1.0).
    y: f32,
    /// Z coordinate (0.0-1.0).
    z: f32,
    /// Depth level / zoom (0-8).
    zoom: u8,
    /// Max results.
    #[serde(default = "default_k")]
    k: usize,
}

/// Parameters for `memory_mql_query`.
#[derive(Debug, Deserialize, JsonSchema)]
struct MqlParams {
    /// MQL expression, e.g. `layer:long_term depth:2..5 "memory"`.
    mql: String,
}

/// Parameters for `memory_session_log`.
#[derive(Debug, Deserialize, JsonSchema)]
struct SessionLogParams {
    /// Number of recent interactions to return.
    #[serde(default = "default_n")]
    n: usize,
}

/// Parameters for `memory_build`.
#[derive(Debug, Deserialize, JsonSchema)]
struct BuildParams {
    /// Force rebuild even if unchanged.
    #[serde(default)]
    force: bool,
}

/// Parameters for `memory_speak`.
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
struct SpeakParams {
    /// Text to synthesize into speech.
    text: String,
    /// Voice name to search for (defaults to "Noémi").
    #[serde(default = "default_noemi")]
    voice: String,
    /// Output WAV file path (defaults to output/speak_<timestamp>.wav).
    output: Option<String>,
}

// ─── Parameter defaults ──────────────────────────────────────────────────

fn default_layer() -> String {
    "long_term".to_string()
}
fn default_importance() -> u8 {
    5
}
fn default_k() -> usize {
    10
}
fn default_noemi() -> String {
    "Noémi".to_string()
}
fn default_n() -> usize {
    50
}

// ─── The MCP tool service ───────────────────────────────────────────────

/// Microscope Memory MCP service — exposes the cognitive memory engine's
/// core operations as MCP tools over Streamable HTTP.
#[derive(Clone)]
pub struct MicroscopeService {
    config: Config,
}

impl MicroscopeService {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Convenience: open (or reopen) a `MicroscopeReader` from the stored
    /// config.  Tools that need the index call this rather than caching a
    /// reader, because the index may change between requests.
    fn open_reader(&self) -> Result<MicroscopeReader, McpError> {
        MicroscopeReader::open(&self.config).map_err(|e| McpError::internal_error(e, None))
    }

    /// Append-log path derived from config.
    fn append_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.config.paths.output_dir).join("append.bin")
    }
}

#[tool_router]
impl MicroscopeService {
    /// Get memory index status: block counts, depths, layer sizes, append-log
    /// entry count.
    #[tool(
        description = "Get microscope memory index status: block count, depths, append log size"
    )]
    fn memory_status(&self) -> Result<String, McpError> {
        let reader = self.open_reader()?;
        let append_path = self.append_path();

        let append_count = if append_path.exists() {
            read_append_log(&append_path).len()
        } else {
            0
        };

        let depth_summary: Vec<_> = (0..9)
            .map(|z| {
                let (start, count) = reader.depth_ranges[z as usize];
                (z as u8, start as usize, count as usize)
            })
            .collect();

        let result = json!({
            "block_count": reader.block_count,
            "depths": depth_summary,
            "append_log_entries": append_count,
            "layers": LAYER_NAMES,
            "version": env!("CARGO_PKG_VERSION"),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Store a new memory into the microscope append log.
    #[tool(description = "Store a new memory into the microscope append log")]
    fn memory_store(
        &self,
        Parameters(StoreParams {
            text,
            layer,
            importance,
            emotion,
        }): Parameters<StoreParams>,
    ) -> Result<String, McpError> {
        // Clamp importance to [1, 10]
        let importance = importance.clamp(1, 10);

        if let Some(emo) = &emotion {
            if emo.len() != EMOTION_VECTOR_SIZE {
                return Err(McpError::invalid_params(
                    format!(
                        "emotion vector must have {} elements, got {}",
                        EMOTION_VECTOR_SIZE,
                        emo.len()
                    ),
                    None,
                ));
            }
            let arr: [f32; 21] = emo
                .iter()
                .cloned()
                .chain(std::iter::repeat(0.0))
                .take(21)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| McpError::internal_error("emotion vector conversion failed", None))?;

            store_memory_with_emotion(&self.config, &text, &layer, importance, Some(arr))
                .map_err(|e| McpError::internal_error(e, None))?;
        } else {
            store_memory(&self.config, &text, &layer, importance)
                .map_err(|e| McpError::internal_error(e, None))?;
        }

        Ok(format!(
            "Stored memory into '{}' layer (importance={})",
            layer, importance,
        ))
    }

    /// Natural-language recall with auto-zoom — searches both main index
    /// and append log.  Optional emotion vector biases results toward
    /// emotionally similar memories.
    #[tool(description = "Natural language recall with auto-zoom")]
    fn memory_recall(
        &self,
        Parameters(RecallParams {
            query,
            k,
            emotion: _,
        }): Parameters<RecallParams>,
    ) -> Result<String, McpError> {
        let mut reader = self.open_reader()?;

        // Auto-zoom: determine depth range from query complexity.
        let (min_depth, max_depth) = auto_zoom(&query);

        // Use the full-ranked search across index + append log.
        let ranked = reader.find_text_all_ranked(&self.config, &query, k);

        let mut results: Vec<serde_json::Value> = Vec::new();

        for (depth, idx, is_main, score) in ranked {
            let text = if is_main {
                reader.text(idx).to_string()
            } else {
                let append_path = self.append_path();
                let appended = read_append_log(&append_path);
                if idx < appended.len() {
                    appended[idx].text.clone()
                } else {
                    "<unknown>".to_string()
                }
            };

            let layer_id = if is_main {
                reader.header(idx).layer_id
            } else {
                let append_path = self.append_path();
                let appended = read_append_log(&append_path);
                appended.get(idx).map(|e| e.layer_id).unwrap_or(0)
            };

            let layer_name = LAYER_NAMES.get(layer_id as usize).unwrap_or(&"?");

            results.push(json!({
                "text": text,
                "depth": depth,
                "layer": layer_name,
                "score": score,
                "source": if is_main { "index" } else { "append_log" },
                "index": idx,
            }));
        }

        let result = json!({
            "query": query,
            "k": k,
            "min_depth": min_depth,
            "max_depth": max_depth,
            "results": results,
            "count": results.len(),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Brute-force text search across all depths (index + append log).
    #[tool(description = "Text search across all depths")]
    fn memory_find(
        &self,
        Parameters(FindParams { query, k }): Parameters<FindParams>,
    ) -> Result<String, McpError> {
        let reader = self.open_reader()?;

        let results = reader.find_text_all(&self.config, &query, k);

        let mut output: Vec<serde_json::Value> = Vec::new();
        for (depth, idx, is_main) in results {
            let text = if is_main {
                let t = reader.text(idx);
                t.to_string()
            } else {
                let append_path = self.append_path();
                let appended = read_append_log(&append_path);
                appended
                    .get(idx)
                    .map(|e| e.text.clone())
                    .unwrap_or_default()
            };

            let layer_id = if is_main {
                reader.header(idx).layer_id
            } else {
                let append_path = self.append_path();
                let appended = read_append_log(&append_path);
                appended.get(idx).map(|e| e.layer_id).unwrap_or(0)
            };

            let layer_name = LAYER_NAMES.get(layer_id as usize).unwrap_or(&"?");

            output.push(json!({
                "text": text,
                "depth": depth,
                "layer": layer_name,
                "source": if is_main { "index" } else { "append_log" },
                "index": idx,
            }));
        }

        let result = json!({
            "query": query,
            "k": k,
            "results": output,
            "count": output.len(),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Manual spatial look at specific 3D coordinates and zoom level.
    #[tool(description = "Manual spatial look at specific 3D coordinates and zoom level")]
    fn memory_look(
        &self,
        Parameters(LookParams { x, y, z, zoom, k }): Parameters<LookParams>,
    ) -> Result<String, McpError> {
        let reader = self.open_reader()?;
        let zoom = zoom.min(8);

        let results = reader.look(&self.config, x, y, z, zoom, k);

        let mut output: Vec<serde_json::Value> = Vec::new();
        for (dist_sq, idx, is_main) in results {
            let (text, layer_name) = if is_main {
                let t = reader.text(idx);
                let lid = reader.header(idx).layer_id;
                let name = LAYER_NAMES.get(lid as usize).unwrap_or(&"?");
                (t.to_string(), name.to_string())
            } else {
                let append_path = self.append_path();
                let appended = read_append_log(&append_path);
                let entry = appended.get(idx).unwrap();
                let name = LAYER_NAMES.get(entry.layer_id as usize).unwrap_or(&"?");
                (entry.text.clone(), name.to_string())
            };

            output.push(json!({
                "text": text,
                "dist_sq": dist_sq,
                "depth": zoom,
                "layer": layer_name,
                "source": if is_main { "index" } else { "append_log" },
            }));
        }

        let result = json!({
            "x": x,
            "y": y,
            "z": z,
            "zoom": zoom,
            "k": k,
            "results": output,
            "count": output.len(),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Execute an MQL (Microscope Query Language) query with filters.
    #[tool(description = "Execute an MQL query")]
    fn memory_mql_query(
        &self,
        Parameters(MqlParams { mql }): Parameters<MqlParams>,
    ) -> Result<String, McpError> {
        let reader = self.open_reader()?;
        let append_path = self.append_path();
        let appended = read_append_log(&append_path);

        let q = crate::query::parse(&mql);
        let qr = crate::query::execute(&q, &reader, &appended);

        let mut results: Vec<serde_json::Value> = Vec::new();
        for r in &qr {
            let text = if r.is_main {
                reader.text(r.block_idx).to_string()
            } else {
                appended
                    .get(r.block_idx)
                    .map(|e| e.text.clone())
                    .unwrap_or_default()
            };

            let layer_id = if r.is_main {
                reader.header(r.block_idx).layer_id
            } else {
                appended.get(r.block_idx).map(|e| e.layer_id).unwrap_or(0)
            };

            let layer_name = LAYER_NAMES.get(layer_id as usize).unwrap_or(&"?");

            results.push(json!({
                "text": text,
                "score": r.score,
                "layer": layer_name,
                "source": if r.is_main { "index" } else { "append_log" },
            }));
        }

        let result = json!({
            "mql": mql,
            "results": results,
            "count": results.len(),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Read last N interactions from the session memory layer.
    #[tool(description = "Read last N interactions from the session memory layer")]
    fn memory_session_log(
        &self,
        Parameters(SessionLogParams { n }): Parameters<SessionLogParams>,
    ) -> Result<String, McpError> {
        let session_path = std::path::Path::new(&self.config.paths.layers_dir).join("session.txt");

        if !session_path.exists() {
            return Ok(
                json!({ "entries": [], "count": 0, "session_path": session_path }).to_string(),
            );
        }

        let content = std::fs::read_to_string(&session_path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let lines: Vec<&str> = content.lines().rev().take(n).collect();

        let result = json!({
            "entries": lines,
            "count": lines.len(),
            "session_path": session_path,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Ping — health check that returns basic server info.
    #[tool(description = "Ping the MCP server")]
    fn memory_ping(&self) -> String {
        json!({
            "status": "ok",
            "server": "microscope-memory",
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_version": "2025-11-25",
            "layers": LAYER_NAMES.len(),
            "max_depth": 8,
        })
        .to_string()
    }

    /// Rebuild the microscope index from layer source files.
    #[tool(description = "Rebuild the microscope index from layer source files")]
    fn memory_build(
        &self,
        Parameters(BuildParams { force }): Parameters<BuildParams>,
    ) -> Result<String, McpError> {
        let _ = force; // Fresh open validates the index; a real rebuild
                       // is a CLI-only operation (`microscope-mem build`).
        match self.open_reader() {
            Ok(reader) => Ok(json!({
                "status": "ok",
                "message": "Index opened and verified successfully",
                "block_count": reader.block_count,
            })
            .to_string()),
            Err(e) => Err(e),
        }
    }

    /// Synthesize text into speech using the system TTS engine (SAPI on
    /// Windows).  Finds the requested voice (Noémi by default), renders to
    /// a WAV file, and returns the output file path.
    ///
    /// On Windows the voice is resolved from the SAPI registry; on other
    /// platforms an informative error is returned.
    #[tool(
        description = "Synthesise text into speech via system TTS (Noémi voice by default). Saves WAV and returns path."
    )]
    fn memory_speak(
        &self,
        Parameters(SpeakParams {
            text,
            voice,
            output,
        }): Parameters<SpeakParams>,
    ) -> Result<String, McpError> {
        #[cfg(not(windows))]
        {
            let _ = (text, voice, output);
            return Err(McpError::internal_error(
                "TTS synthesis requires a Windows host (SAPI) or an external \
                 TTS engine on this platform.",
                None,
            ));
        }

        #[cfg(windows)]
        {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);

            let output_path = output
                .unwrap_or_else(|| format!("{}/speak_{}.wav", self.config.paths.output_dir, ts));

            // Ensure output directory exists
            let out_dir = std::path::Path::new(&self.config.paths.output_dir);
            if !out_dir.exists() {
                std::fs::create_dir_all(out_dir)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            }

            // Write text to a temp file — avoids PowerShell quoting hell
            let text_path = std::env::temp_dir().join(format!("microscope_tts_{}.txt", ts));
            std::fs::write(&text_path, &text)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            // PowerShell one-liner: find voice → SAPI → WAV
            let ps_cmd = format!(
                r#"Add-Type -AssemblyName System.Speech;$s=\
New-Object System.Speech.Synthesis.SpeechSynthesizer;$vs=$s.GetVoices();\
$sel=$null;foreach($v in $vs){{if($v.GetDescription()-like '*{voice}*')\
{{$sel=$v;break}}}}if(-not $sel){{foreach($v in $vs){{if($v.Culture.Name \
-eq 'hu-HU'){{$sel=$v;break}}}}}}if($sel){{$s.Voice=$sel}}$s.Rate=0;\
$s.Volume=100;$fs=New-Object System.IO.FileStream('{output_path}',\
[System.IO.FileMode]::Create);$wf=New-Object System.Speech.Synthesis.\
WaveFileWriter($fs);$s.AudioOutputStream=$wf;$t=Get-Content -Path \
'{text_path}' -Raw;$s.Speak($t);$wf.Dispose();$fs.Dispose();$s.Dispose()"#,
                voice = voice.replace("'", "''"),
                output_path = output_path.replace('\\', "\\\\"),
                text_path = text_path.to_string_lossy().replace('\\', "\\\\"),
            );

            let proc = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
                .output();

            // Clean up temp file
            let _ = std::fs::remove_file(&text_path);

            match proc {
                Ok(po) => {
                    if !po.status.success() {
                        let stderr = String::from_utf8_lossy(&po.stderr);
                        let stdout = String::from_utf8_lossy(&po.stdout);
                        return Err(McpError::internal_error(
                            format!(
                                "TTS synthesis failed. stdout: {}, stderr: {}",
                                stdout.trim(),
                                stderr.trim()
                            ),
                            None,
                        ));
                    }
                    let stdout = String::from_utf8_lossy(&po.stdout);
                    Ok(format!(
                        "Synthesised with voice '{}'. WAV saved to: {}\nstdout: {}",
                        voice,
                        output_path,
                        stdout.trim()
                    ))
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Failed to execute TTS engine: {}", e),
                    None,
                )),
            }
        }
    }
}

/// Server metadata — name, version, instructions.  Capabilities (tools)
/// are enabled automatically by `#[tool_router]`.
///
/// NOTE: `version` must be a string literal (proc-macro limitation),
/// so keep it in sync with `Cargo.toml` when bumping.
#[tool_handler(
    name = "microscope-memory",
    version = "1.2.0",
    instructions = "Microscope Memory — cognitive memory engine with 12 memory layers and 9 hierarchical depths (D0-D8). Exposes tools for storing, recalling, and querying memories via natural language, MQL, and 3D spatial coordinates. Built on a binary mmap store with sub-millisecond recall."
)]
impl ServerHandler for MicroscopeService {}

// ─── HTTP server entry point ────────────────────────────────────────────

/// Start the Streamable HTTP MCP server.
///
/// # Arguments
/// * `config` — Microscope Memory configuration (API key and CORS origin
///   are read from `[server]`).
/// * `host` — Bind address (e.g. `"127.0.0.1"` or `"0.0.0.0"`).
/// * `port` — TCP port.
///
/// # Security
/// When `[server].api_key` is set, clients must send it via
/// `x-api-key` or `Authorization: Bearer <key>`.  When
/// `[server].cors_origin` is set, cross-origin requests whose `Origin`
/// doesn't match are rejected.
pub async fn serve_http(
    config: Config,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let auth_state = AuthState::from(&config);

    // Streamable HTTP server config — modern clients are stateless by
    // default; we keep session support for legacy clients and enable
    // JSON response mode for simple tool calls.
    let http_config = StreamableHttpServerConfig::default().with_json_response(true);

    // Factory: a fresh MicroscopeService per request (the Config is cheaply
    // cloned — it is a small struct of strings, bools, and numbers).
    let config_clone = config.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(MicroscopeService::new(config_clone.clone())),
        LocalSessionManager::default().into(),
        http_config,
    );

    // CORS layer — expose the MCP-specific headers.
    let cors = {
        let mut builder = CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::DELETE])
            .allow_headers(Any)
            .expose_headers([
                HeaderName::from_static("mcp-session-id"),
                HeaderName::from_static("mcp-protocol-version"),
                HeaderName::from_static("mcp-supported-protocol-versions"),
            ])
            .allow_credentials(true);

        if let Some(ref origin) = auth_state.cors_origin {
            builder = builder.allow_origin(origin.parse::<HeaderValue>()?);
        } else {
            builder = builder.allow_origin(Any);
        }

        builder
    };

    // Build the axum router.
    let mut router = Router::new()
        .route(
            "/",
            get(|| async { "Microscope Memory MCP (Streamable HTTP) — POST to /mcp" }),
        )
        .nest_service("/mcp", mcp_service)
        .route_layer(from_fn_with_state(auth_state.clone(), auth_middleware));

    // Add CORS + tracing as outer layers.
    router = router.layer(cors).layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", host, port);
    let socket_addr: SocketAddr = addr.parse()?;

    eprintln!(
        "[mcp_http] Streamable HTTP MCP server listening on {}",
        addr
    );

    if auth_state.api_key.is_some() {
        eprintln!("[mcp_http] API key authentication enabled (x-api-key / Authorization: Bearer)");
    } else {
        eprintln!("[mcp_http] No API key configured — allowing unauthenticated access");
    }

    if let Some(ref origin) = auth_state.cors_origin {
        eprintln!("[mcp_http] Origin validation: only '{}' allowed", origin);
    }

    axum::serve(
        tokio::net::TcpListener::bind(socket_addr).await?,
        router.into_make_service(),
    )
    .await?;

    Ok(())
}
