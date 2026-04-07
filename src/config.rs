use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Paths {
    pub layers_dir: String,
    pub output_dir: String,
    pub temp_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Index {
    pub block_size: usize,
    pub max_depth: u8,
    pub header_size: usize,
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
}

fn default_port() -> u16 {
    6060
}

impl Default for Server {
    fn default() -> Self {
        Self {
            port: default_port(),
            cors_origin: None,
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

impl Config {
    /// Zero-JSON/Zero-TOML loader: reads config.bin via mmap or returns Default.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }

        let file = fs::File::open(path)?;
        // Safety: config.bin is read-only and mapped for the duration of this call
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        let config: Config = bincode::deserialize(&mmap)?;
        Ok(config)
    }

    /// Saves the current config to a binary mmap-ready format.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = bincode::serialize(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paths: Paths {
                layers_dir: "./layers".to_string(),
                output_dir: "./data".to_string(),
                temp_dir: "./tmp".to_string(),
            },
            index: Index {
                block_size: 256,
                max_depth: 8,
                header_size: 32,
            },
            search: Search {
                default_k: 10,
                zoom_weight: 2.0,
                keyword_boost: 0.1,
                semantic_weight: 0.0,
                emotional_bias_weight: 0.0,
            },
            memory_layers: MemoryLayers {
                layers: vec![
                    "long_term".to_string(),
                    "short_term".to_string(),
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
        }
    }
}
