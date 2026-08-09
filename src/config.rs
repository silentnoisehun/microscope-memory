use crate::types::ProjectId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HooksConfig {
    #[serde(default = "default_hooks_enabled")]
    pub enabled: bool,
    pub read_only: bool,
    pub write_enabled: bool,
    #[serde(default = "default_hooks_min_importance")]
    pub min_importance: u8,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            enabled: default_hooks_enabled(),
            read_only: true,
            write_enabled: false,
            min_importance: default_hooks_min_importance(),
        }
    }
}

fn default_hooks_enabled() -> bool { true }
fn default_hooks_min_importance() -> u8 { 3 }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub paths: Paths,
    #[serde(default)]
    pub index: Index,
    #[serde(default)]
    pub search: Search,
    #[serde(default)]
    pub memory_layers: MemoryLayers,
    #[serde(default)]
    pub performance: Performance,
    #[serde(default)]
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
    #[serde(default)]
    pub epistemic: EpistemicConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Paths {
    #[serde(default = "default_layers_dir")]
    pub layers_dir: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_temp_dir")]
    pub temp_dir: String,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            layers_dir: default_layers_dir(),
            output_dir: default_output_dir(),
            temp_dir: default_temp_dir(),
        }
    }
}
fn default_layers_dir() -> String { "./layers".to_string() }
fn default_output_dir() -> String { "./output".to_string() }
fn default_temp_dir() -> String { "./tmp".to_string() }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Index {
    #[serde(default = "default_index_max_depth")]
    pub max_depth: u8,
    #[serde(default = "default_index_header_size")]
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

impl Default for Index {
    fn default() -> Self {
        Self {
            max_depth: default_index_max_depth(),
            header_size: default_index_header_size(),
            auto_rebuild: default_auto_rebuild(),
            auto_rebuild_entries: default_auto_rebuild_entries(),
            layer_retention_entries: default_layer_retention_entries(),
            max_blocks: default_max_blocks(),
            protect_min_importance: default_protect_min_importance(),
            promote_energy_threshold: default_promote_energy_threshold(),
        }
    }
}
fn default_index_max_depth() -> u8 { 8 }
fn default_index_header_size() -> usize { 50 }

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
    #[serde(default = "default_search_default_k")]
    pub default_k: usize,
    #[serde(default = "default_search_zoom_weight")]
    pub zoom_weight: f32,
    #[serde(default = "default_search_keyword_boost")]
    pub keyword_boost: f32,
    #[serde(default)]
    pub semantic_weight: f32,
    /// How much the cosine counts in the *ranking*, as a distance bonus.
    ///
    /// Separate from `semantic_weight`, which is a 0..1 blend deciding how far a
    /// query's spatial coordinates lean toward its embedding. Those are
    /// different units with different meanings, and sharing one knob made the
    /// two impossible to tell apart: setting it to 2.0 to test whether the
    /// semantic term could lead also moved the coordinates, and the measurement
    /// credited both effects to the same number.
    ///
    /// The default is the value both halves independently measured best at; see
    /// docs/measurements/ for the split curves. The two were never measured
    /// apart before this field existed.
    #[serde(default = "default_semantic_rank_gain")]
    pub semantic_rank_gain: f32,
    #[serde(default)]
    pub emotional_bias_weight: f32,
    #[serde(default)]
    pub emotion_21d_weight: f32,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            default_k: default_search_default_k(),
            zoom_weight: default_search_zoom_weight(),
            keyword_boost: default_search_keyword_boost(),
            semantic_weight: 0.0,
            emotional_bias_weight: 0.0,
            emotion_21d_weight: 0.0,
        }
    }
}
fn default_search_default_k() -> usize { 10 }
fn default_search_zoom_weight() -> f32 { 3.0 }
fn default_search_keyword_boost() -> f32 { 0.4 }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryLayers {
    pub layers: Vec<String>,
}

impl Default for MemoryLayers {
    fn default() -> Self {
        Self {
            layers: vec![
                "long_term".to_string(),
                "short_term".to_string(),
                "session".to_string(),
                "associative".to_string(),
                "echo_cache".to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Performance {
    #[serde(default = "default_perf_use_mmap")]
    pub use_mmap: bool,
    #[serde(default = "default_perf_cache_size")]
    pub cache_size: usize,
    #[serde(default = "default_perf_build_workers")]
    pub build_workers: usize,
    #[serde(default)]
    pub use_gpu: bool,
    #[serde(default)]
    pub compression: bool,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            use_mmap: default_perf_use_mmap(),
            cache_size: default_perf_cache_size(),
            build_workers: default_perf_build_workers(),
            use_gpu: false,
            compression: false,
            cache_ttl_secs: default_cache_ttl(),
        }
    }
}
fn default_perf_use_mmap() -> bool { true }
fn default_perf_cache_size() -> usize { 64 }
fn default_perf_build_workers() -> usize { 4 }

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
    #[serde(default)]
    pub use_gpu: bool,
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
    // D5 holds 15..39 character statements (auto_depth), which is where short
    // personal facts are stored. Embedding only to D4 left those facts without
    // any vector, making them unreachable from the semantic path.
    5
}
fn default_semantic_rank_gain() -> f32 {
    // Set from the measured curve, not chosen: see WHITEPAPER 11.9. The
    // knee is at 2.0 -- 1.0 costs about 1.7 points of R@1 and nothing above 2.0
    // adds any.
    2.0
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
            use_gpu: false,
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
    #[serde(default)]
    pub file: Option<String>,
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: Some("microscope.log".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Federation {
    #[serde(default)]
    pub indices: Vec<FederatedIndex>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EpistemicConfig {
    /// Gate importance promotion for unsupported Inference/Hypothesis.
    #[serde(default = "default_true")]
    pub gate_promotion: bool,
    /// Minimum independent sources needed for non-zero confidence.
    #[serde(default = "default_min_sources")]
    pub min_independent_sources: u32,
    /// Cosine similarity threshold for echo detection.
    #[serde(default = "default_sim_threshold")]
    pub sim_threshold: f32,
    /// Near-duplicate scan depth for link operations.
    #[serde(default = "default_search_k")]
    pub search_k: usize,
    /// Confidence formula weights.
    #[serde(default)]
    pub confidence_weights: ConfidenceWeights,
    /// Bias added to eviction score from confidence (0 = disabled).
    #[serde(default)]
    pub eviction_evidence_bias: f32,
}

fn default_true() -> bool {
    true
}
fn default_min_sources() -> u32 {
    1
}
fn default_sim_threshold() -> f32 {
    0.85
}
fn default_search_k() -> usize {
    8
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfidenceWeights {
    #[serde(default = "default_obs_w")]
    pub observation_w: i32,
    #[serde(default = "default_src_w")]
    pub source_w: i32,
    #[serde(default = "default_refute_w")]
    pub refute_w: i32,
    #[serde(default = "default_age_pen")]
    pub age_penalty: i32,
}

impl Default for ConfidenceWeights {
    fn default() -> Self {
        Self {
            observation_w: 30,
            source_w: 18,
            refute_w: 25,
            age_penalty: 5,
        }
    }
}

fn default_obs_w() -> i32 {
    30
}
fn default_src_w() -> i32 {
    18
}
fn default_refute_w() -> i32 {
    25
}
fn default_age_pen() -> i32 {
    5
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
            paths: Paths::default(),
            index: Index::default(),
            search: Search::default(),
            memory_layers: MemoryLayers::default(),
            performance: Performance::default(),
            logging: Logging::default(),
            embedding: Embedding::default(),
            server: Server::default(),
            federation: Federation::default(),
            hooks: HooksConfig::default(),
            project_id: ProjectId::GLOBAL,
            epistemic: EpistemicConfig::default(),
        }
    }
}
