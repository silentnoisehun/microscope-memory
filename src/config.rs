use crate::types::ProjectId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HooksConfig {
    pub read_only: bool,
    pub write_enabled: bool,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            read_only: true,
            write_enabled: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub paths: Paths,
    pub index: Index,
    pub search: Search,
    pub memory_layers: MemoryLayers,
    pub performance: Performance,
    pub logging: Logging,
    #[serde(default)]
    pub embedding: Embedding,
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    pub federation: Federation,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default = "default_project_id")]
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Paths {
    pub layers_dir: String,
    pub output_dir: String,
    pub temp_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Index {
    pub max_depth: u8,
    pub header_size: usize,
    #[serde(default = "default_auto_rebuild")]
    pub auto_rebuild: bool,
    #[serde(default = "default_auto_rebuild_entries")]
    pub auto_rebuild_entries: usize,
    /// Maximum persisted entries per layer. Zero keeps the full history.
    #[serde(default = "default_layer_retention_entries")]
    pub layer_retention_entries: usize,
    /// Maximum blocks in the index before low-score blocks are evicted.
    /// Zero keeps the index unbounded.
    #[serde(default = "default_max_blocks")]
    pub max_blocks: usize,
    /// Blocks with importance >= this value are never evicted.
    #[serde(default = "default_protect_min_importance")]
    pub protect_min_importance: u8,
    /// Minimum Hebbian recall energy for automatic importance promotion.
    #[serde(default = "default_promote_energy_threshold")]
    pub promote_energy_threshold: f32,
}

fn default_auto_rebuild() -> bool {
    true
}

fn default_layer_retention_entries() -> usize {
    2000
}

fn default_max_blocks() -> usize {
    0
}

fn default_protect_min_importance() -> u8 {
    8
}

fn default_promote_energy_threshold() -> f32 {
    0.35
}

fn default_auto_rebuild_entries() -> usize {
    25
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Search {
    pub default_k: usize,
    pub zoom_weight: f32,
    pub keyword_boost: f32,
    #[serde(default)]
    pub semantic_weight: f32,
    #[serde(default)]
    pub emotional_bias_weight: f32,
    #[serde(default)]
    pub emotion_21d_weight: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryLayers {
    pub layers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Performance {
    pub use_mmap: bool,
    pub cache_size: usize,
    pub build_workers: usize,
    #[serde(default)]
    pub use_gpu: bool,
    #[serde(default)]
    pub compression: bool,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
}

fn default_cache_ttl() -> u64 {
    300
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Embedding {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_dim")]
    pub dim: usize,
    #[serde(default = "default_max_depth")]
    pub max_depth: u8,
    #[serde(default)]
    pub onnx_model_path: Option<String>,
    #[serde(default)]
    pub tokenizer_path: Option<String>,
}

fn default_provider() -> String {
    "mock".to_string()
}
fn default_model() -> String {
    "sentence-transformers/all-MiniLM-L6-v2".to_string()
}
fn default_dim() -> usize {
    384
}
fn default_max_depth() -> u8 {
    4
}

impl Default for Embedding {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            dim: default_dim(),
            max_depth: default_max_depth(),
            onnx_model_path: None,
            tokenizer_path: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub cors_origin: Option<String>,
    #[serde(default)]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub gemini_api_key: Option<String>,
    /// Optional shared API key for inbound HTTP requests. Empty = no auth
    /// (single-user localhost default). When set, every bridge request must
    /// carry `X-API-Key: <key>`. Required when binding on a non-loopback host.
    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_port() -> u16 {
    6060
}

impl Default for Server {
    fn default() -> Self {
        Self {
            port: default_port(),
            cors_origin: None,
            openai_api_key: None,
            gemini_api_key: None,
            api_key: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Logging {
    pub level: String,
    pub file: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Federation {
    #[serde(default)]
    pub indices: Vec<FederatedIndex>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FederatedIndex {
    pub name: String,
    pub config_path: String,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

fn default_weight() -> f32 {
    1.0
}

fn default_project_id() -> ProjectId {
    ProjectId::GLOBAL
}

impl Config {
    /// Loads config from a TOML file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Saves the current config to a TOML file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        let tmp_path = path.as_ref().with_extension("toml.tmp");
        fs::write(&tmp_path, &content)?;
        fs::rename(&tmp_path, path.as_ref())?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paths: Paths {
                layers_dir: "./layers".to_string(),
                output_dir: "./output".to_string(),
                temp_dir: "./tmp".to_string(),
            },
            index: Index {
                max_depth: 8,
                header_size: 50,
                auto_rebuild: default_auto_rebuild(),
                auto_rebuild_entries: default_auto_rebuild_entries(),
                layer_retention_entries: default_layer_retention_entries(),
                max_blocks: default_max_blocks(),
                protect_min_importance: default_protect_min_importance(),
                promote_energy_threshold: default_promote_energy_threshold(),
            },
            search: Search {
                default_k: 10,
                zoom_weight: 3.0,   // boost recent memories more
                keyword_boost: 0.4, // tuned for better recall precision
                semantic_weight: 0.0,
                emotional_bias_weight: 0.0,
                emotion_21d_weight: 0.0,
            },
            memory_layers: MemoryLayers {
                layers: vec![
                    "long_term".to_string(),
                    "short_term".to_string(),
                    "session".to_string(),
                    "associative".to_string(),
                    "echo_cache".to_string(),
                ],
            },
            performance: Performance {
                use_mmap: true,
                cache_size: 64,
                build_workers: 4,
                use_gpu: false,
                compression: false,
                cache_ttl_secs: default_cache_ttl(),
            },
            logging: Logging {
                level: "info".to_string(),
                file: Some("microscope.log".to_string()),
            },
            embedding: Embedding::default(),
            server: Server::default(),
            federation: Federation::default(),
            hooks: HooksConfig::default(),
            project_id: ProjectId::GLOBAL,
        }
    }
}
