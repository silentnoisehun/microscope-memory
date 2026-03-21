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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Logging {
    pub level: String,
    pub file: Option<String>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Self {
            paths: Paths {
                layers_dir: "D:/Claude Memory/layers".to_string(),
                output_dir: "D:/Claude Memory/microscope".to_string(),
                temp_dir: "tmp/microscope".to_string(),
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
            },
            logging: Logging {
                level: "info".to_string(),
                file: Some("microscope.log".to_string()),
            },
        }
    }
}
