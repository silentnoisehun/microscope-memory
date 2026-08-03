#![allow(dead_code)]
// Embedding module for semantic vector search
// Supports OpenAI, HuggingFace, and custom embeddings

use std::collections::HashMap;
use std::f32;

pub const EMBEDDING_DIM: usize = 1536; // OpenAI ada-002 dimension

/// Embedding provider trait
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn dimension(&self) -> usize;
}

#[derive(Debug)]
pub enum EmbeddingError {
    ApiError(String),
    InvalidDimension,
    NetworkError,
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingError::ApiError(msg) => write!(f, "API error: {}", msg),
            EmbeddingError::InvalidDimension => write!(f, "Invalid embedding dimension"),
            EmbeddingError::NetworkError => write!(f, "Network error"),
        }
    }
}

/// Cached embedding storage
pub struct EmbeddingCache {
    embeddings: HashMap<String, Vec<f32>>,
    dimension: usize,
}

impl EmbeddingCache {
    pub fn new(dimension: usize) -> Self {
        Self {
            embeddings: HashMap::new(),
            dimension,
        }
    }

    pub fn insert(&mut self, text: String, embedding: Vec<f32>) {
        if embedding.len() == self.dimension {
            self.embeddings.insert(text, embedding);
        }
    }

    pub fn get(&self, text: &str) -> Option<&Vec<f32>> {
        self.embeddings.get(text)
    }

    pub fn contains(&self, text: &str) -> bool {
        self.embeddings.contains_key(text)
    }
}

/// Fast SIMD-accelerated cosine similarity
#[cfg(target_arch = "x86_64")]
pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::x86_64::*;

    if a.len() != b.len() {
        return 0.0;
    }

    unsafe {
        let mut dot_sum = _mm256_setzero_ps();
        let mut norm_a = _mm256_setzero_ps();
        let mut norm_b = _mm256_setzero_ps();

        let chunks = a.len() / 8;

        for i in 0..chunks {
            let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));

            dot_sum = _mm256_fmadd_ps(va, vb, dot_sum);
            norm_a = _mm256_fmadd_ps(va, va, norm_a);
            norm_b = _mm256_fmadd_ps(vb, vb, norm_b);
        }

        // Sum the vector components
        let dot = horizontal_sum_ps256(dot_sum);
        let na = horizontal_sum_ps256(norm_a).sqrt();
        let nb = horizontal_sum_ps256(norm_b).sqrt();

        // Handle remaining elements
        let mut dot_rem = 0.0;
        let mut na_rem = 0.0;
        let mut nb_rem = 0.0;

        for i in (chunks * 8)..a.len() {
            dot_rem += a[i] * b[i];
            na_rem += a[i] * a[i];
            nb_rem += b[i] * b[i];
        }

        (dot + dot_rem) / ((na + na_rem.sqrt()) * (nb + nb_rem.sqrt()))
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> f32 {
    cosine_similarity_scalar(a, b)
}

/// Fallback scalar cosine similarity
pub fn cosine_similarity_scalar(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(target_arch = "x86_64")]
unsafe fn horizontal_sum_ps256(v: std::arch::x86_64::__m256) -> f32 {
    use std::arch::x86_64::*;

    let high = _mm256_extractf128_ps(v, 1);
    let low = _mm256_castps256_ps128(v);
    let sum = _mm_add_ps(high, low);
    let shuf = _mm_shuffle_ps(sum, sum, 0x0E);
    let sums = _mm_add_ps(sum, shuf);
    let shuf2 = _mm_movehl_ps(sums, sums);
    let result = _mm_add_ss(sums, shuf2);
    _mm_cvtss_f32(result)
}

/// Mock embedding provider for testing
pub struct MockEmbeddingProvider {
    dimension: usize,
}

impl MockEmbeddingProvider {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        // Simple hash-based embedding for testing
        let mut embedding = vec![0.0; self.dimension];
        let hash = text
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));

        for (i, slot) in embedding.iter_mut().enumerate() {
            let val = ((hash.wrapping_mul(i as u64 + 1)) % 1000) as f32 / 1000.0;
            *slot = val * 2.0 - 1.0; // Normalize to [-1, 1]
        }

        // Normalize to unit vector
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

// ─── Python subprocess embedding provider ────────────
/// Real embedding provider backed by the bundled `embed.py` subprocess
/// (sentence-transformers / MiniLM). Wire protocol (see embed.py):
/// the script writes a u32 LE dimension header on stdout, then for every
/// line it reads on stdin it writes `dim * f32 LE` bytes on stdout.
///
/// The child is spawned once and kept alive; reads are bounded by a timeout
/// so a hung model can never block the host process forever.
#[cfg(not(target_arch = "wasm32"))]
pub struct PythonEmbeddingProvider {
    child: std::sync::Mutex<std::process::Child>,
    stdin: std::sync::Mutex<std::process::ChildStdin>,
    rx: std::sync::Mutex<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>>,
    dim: usize,
    timeout: std::time::Duration,
}

#[cfg(not(target_arch = "wasm32"))]
impl PythonEmbeddingProvider {
    /// Spawn `embed.py` with the given HuggingFace model id and read the
    /// dimension header. Python binary, script path and timeout can be
    /// overridden via MICROSCOPE_PYTHON, MICROSCOPE_EMBED_SCRIPT and
    /// MICROSCOPE_EMBED_TIMEOUT_MS respectively.
    pub fn new(model: &str) -> Result<Self, EmbeddingError> {
        let python = std::env::var("MICROSCOPE_PYTHON").unwrap_or_else(|_| "python".to_string());
        let script = std::env::var("MICROSCOPE_EMBED_SCRIPT").unwrap_or_else(|_| {
            let manifest = format!("{}/embed.py", env!("CARGO_MANIFEST_DIR"));
            if std::path::Path::new(&manifest).exists() {
                manifest
            } else {
                std::env::var("MICROSCOPE_HOME")
                    .map(|h| format!("{}/embed.py", h))
                    .unwrap_or_else(|_| "embed.py".to_string())
            }
        });
        let timeout_ms = std::env::var("MICROSCOPE_EMBED_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120_000);

        let mut child = std::process::Command::new(&python)
            .arg(&script)
            .arg(model)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| EmbeddingError::ApiError(format!("spawn {} {}: {}", python, script, e)))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| EmbeddingError::ApiError("embedding stdin unavailable".into()))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| EmbeddingError::ApiError("embedding stdout unavailable".into()))?;

        // Persistent reader thread: header first, then one vector per line.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut header = [0u8; 4];
            let dim = match stdout.read_exact(&mut header) {
                Ok(()) => u32::from_le_bytes(header) as usize,
                Err(e) => {
                    let _ = tx.send(Err(format!("read dim header: {}", e)));
                    return;
                }
            };
            let _ = tx.send(Ok(header.to_vec()));
            let mut buf = vec![0u8; dim * 4];
            loop {
                match stdout.read_exact(&mut buf) {
                    Ok(()) => {
                        if tx.send(Ok(buf.clone())).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("read embedding: {}", e)));
                        break;
                    }
                }
            }
        });

        let timeout = std::time::Duration::from_millis(timeout_ms);
        let header_msg = rx
            .recv_timeout(timeout)
            .map_err(|_| {
                let _ = child.kill();
                EmbeddingError::ApiError(format!(
                    "embedding model '{}' did not initialize within {} ms",
                    model, timeout_ms
                ))
            })?
            .map_err(EmbeddingError::ApiError)?;
        let dim = u32::from_le_bytes(header_msg[0..4].try_into().unwrap()) as usize;

        Ok(Self {
            child: std::sync::Mutex::new(child),
            stdin: std::sync::Mutex::new(stdin),
            rx: std::sync::Mutex::new(rx),
            dim,
            timeout,
        })
    }

    fn read_vector(&self) -> Result<Vec<f32>, EmbeddingError> {
        let rx = self
            .rx
            .lock()
            .map_err(|_| EmbeddingError::ApiError("embedding receiver lock poisoned".into()))?;
        match rx.recv_timeout(self.timeout) {
            Ok(Ok(bytes)) => {
                let mut v = Vec::with_capacity(bytes.len() / 4);
                for chunk in bytes.chunks_exact(4) {
                    v.push(f32::from_le_bytes(chunk.try_into().unwrap()));
                }
                Ok(v)
            }
            Ok(Err(e)) => Err(EmbeddingError::ApiError(e)),
            Err(_) => {
                let mut child = self.child.lock().map_err(|_| {
                    EmbeddingError::ApiError("embedding child lock poisoned".into())
                })?;
                let _ = child.kill();
                Err(EmbeddingError::ApiError(format!(
                    "embedding provider timed out after {} ms",
                    self.timeout.as_millis()
                )))
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for PythonEmbeddingProvider {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl EmbeddingProvider for PythonEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        use std::io::Write;
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| EmbeddingError::ApiError("embedding stdin lock poisoned".into()))?;
        writeln!(stdin, "{}", text)
            .map_err(|e| EmbeddingError::ApiError(format!("write stdin: {}", e)))?;
        stdin
            .flush()
            .map_err(|e| EmbeddingError::ApiError(format!("flush stdin: {}", e)))?;
        drop(stdin);
        self.read_vector()
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        use std::io::Write;
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| EmbeddingError::ApiError("embedding stdin lock poisoned".into()))?;
        for t in texts {
            writeln!(stdin, "{}", t)
                .map_err(|e| EmbeddingError::ApiError(format!("write stdin: {}", e)))?;
        }
        stdin
            .flush()
            .map_err(|e| EmbeddingError::ApiError(format!("flush stdin: {}", e)))?;
        drop(stdin);
        let mut out = Vec::with_capacity(texts.len());
        for _ in texts {
            out.push(self.read_vector()?);
        }
        Ok(out)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// Build the embedding provider requested by config with an honest fallback:
/// a configured provider that cannot be initialized is reported on stderr
/// instead of silently degrading to the mock. The mock is returned for
/// "mock"/"none" (and as a last resort), with `idx_dim` as its dimension.
pub fn provider_from_config(
    cfg: &crate::config::Embedding,
    idx_dim: usize,
) -> Box<dyn EmbeddingProvider> {
    match cfg.provider.as_str() {
        #[cfg(not(target_arch = "wasm32"))]
        "python" => match PythonEmbeddingProvider::new(&cfg.model) {
            Ok(p) => Box::new(p),
            Err(e) => {
                eprintln!(
                    "  WARN: python embedding provider unavailable: {} — using mock",
                    e
                );
                Box::new(MockEmbeddingProvider::new(idx_dim))
            }
        },
        "candle" => {
            #[cfg(feature = "embeddings")]
            {
                match CandleEmbeddingProvider::new(&cfg.model) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        eprintln!("  WARN: candle init failed: {:?} — using mock", e);
                        Box::new(MockEmbeddingProvider::new(idx_dim))
                    }
                }
            }
            #[cfg(not(feature = "embeddings"))]
            {
                eprintln!("  WARN: candle provider requires the 'embeddings' feature — using mock");
                Box::new(MockEmbeddingProvider::new(idx_dim))
            }
        }
        "none" | "mock" => Box::new(MockEmbeddingProvider::new(idx_dim)),
        "bge-small" => {
            #[cfg(feature = "embeddings")]
            {
                // BAAI/bge-small-en-v1.5: 33M params, 384 dim
                // Config is loaded from the model's config.json on first use.
                match CandleEmbeddingProvider::with_config(
                    "BAAI/bge-small-en-v1.5", None, 384, cfg.use_gpu,
                ) {
                    Ok(p) => Box::new(p),
                    Err(e) => {
                        eprintln!("  WARN: bge-small init failed: {:?} — using mock", e);
                        Box::new(MockEmbeddingProvider::new(idx_dim))
                    }
                }
            }
            #[cfg(not(feature = "embeddings"))]
            {
                eprintln!("  WARN: bge-small requires the 'embeddings' feature — using mock");
                Box::new(MockEmbeddingProvider::new(idx_dim))
            }
        }
        other => {
            eprintln!(
                "  WARN: unknown embedding provider '{}' — using mock",
                other
            );
            Box::new(MockEmbeddingProvider::new(idx_dim))
        }
    }
}

/// Embedding-enhanced block header
#[repr(C, packed)]
pub struct EmbeddedBlockHeader {
    // Original fields
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zoom: f32,
    pub depth: u8,
    pub layer_id: u8,
    pub data_offset: u32,
    pub data_len: u16,
    pub parent_idx: u32,
    pub child_count: u16,
    pub crc16: [u8; 2], // CRC16-CCITT (0x0000 = no checksum)

    // New embedding fields
    pub embedding_offset: u32, // Offset into embedding file
    pub has_embedding: bool,   // Whether this block has an embedding
}

// ─── Candle-based real embedding provider ────────────
#[cfg(feature = "embeddings")]
pub struct CandleEmbeddingProvider {
    model: candle_transformers::models::bert::BertModel,
    tokenizer: tokenizers::Tokenizer,
    dim: usize,
    device: candle_core::Device,
}

#[cfg(feature = "embeddings")]
impl CandleEmbeddingProvider {
    pub fn new(model_id: &str) -> Result<Self, EmbeddingError> {
        Self::with_config(model_id, None, 768, false)
    }

    /// Create with explicit BERT config (for non-standard models like bge-small).
    pub fn with_config(
        model_id: &str,
        config_override: Option<candle_transformers::models::bert::Config>,
        dim_override: usize,
        use_gpu: bool,
    ) -> Result<Self, EmbeddingError> {
        use candle_core::Device;
        use hf_hub::api::sync::Api;

        // Lazy GPU activation: try CUDA only if requested and available.
        let device = if use_gpu {
            match Device::cuda_if_available(0) {
                Ok(d) => {
                    eprintln!("  [bge] GPU device: {:?}", d);
                    d
                }
                Err(e) => {
                    eprintln!("  [bge] CUDA unavailable ({}) — falling back to CPU", e);
                    Device::Cpu
                }
            }
        } else {
            Device::Cpu
        };
        let api = Api::new().map_err(|e| EmbeddingError::ApiError(e.to_string()))?;
        let repo = api.model(model_id.to_string());

        // Load tokenizer
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| EmbeddingError::ApiError(format!("tokenizer download: {}", e)))?;
        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbeddingError::ApiError(format!("tokenizer load: {}", e)))?;

        // Load model weights
        let weights_path = repo
            .get("model.safetensors")
            .map_err(|e| EmbeddingError::ApiError(format!("weights download: {}", e)))?;
        // Safety: safetensors file is valid and will remain mapped for the lifetime of the model
        let vb = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(
                &[weights_path],
                candle_core::DType::F32,
                &device,
            )
        }
        .map_err(|e| EmbeddingError::ApiError(format!("varbuilder: {}", e)))?;

        // Load config: either from override, or from model's config.json
        let config = if let Some(cfg) = config_override {
            cfg
        } else {
            // Try to load config.json from the model repo
            let config_path = repo.get("config.json")
                .map_err(|e| EmbeddingError::ApiError(format!("config download: {}", e)))?;
            let config_str = std::fs::read_to_string(&config_path)
                .map_err(|e| EmbeddingError::ApiError(format!("config read: {}", e)))?;
            serde_json::from_str(&config_str)
                .map_err(|e| EmbeddingError::ApiError(format!("config parse: {}", e)))?
        };
        let dim = dim_override;

        let model = candle_transformers::models::bert::BertModel::load(vb, &config)
            .map_err(|e| EmbeddingError::ApiError(format!("model load: {}", e)))?;

        Ok(Self {
            model,
            tokenizer,
            dim,
            device,
        })
    }

    fn embed_inner(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        use candle_core::Tensor;

        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbeddingError::ApiError(format!("tokenize: {}", e)))?;

        let ids = encoding.get_ids();
        let type_ids = encoding.get_type_ids();
        let len = ids.len();

        let input_ids = Tensor::new(ids, &self.device)
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?
            .reshape((1, len))
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;
        let token_type_ids = Tensor::new(type_ids, &self.device)
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?
            .reshape((1, len))
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        let output = self
            .model
            .forward(&input_ids, &token_type_ids)
            .map_err(|e| EmbeddingError::ApiError(format!("forward: {}", e)))?;

        // Mean pooling over sequence dimension
        let pooled = output
            .mean(1)
            .map_err(|e| EmbeddingError::ApiError(format!("mean pool: {}", e)))?
            .squeeze(0)
            .map_err(|e| EmbeddingError::ApiError(format!("squeeze: {}", e)))?;

        let mut embedding: Vec<f32> = pooled
            .to_vec1()
            .map_err(|e| EmbeddingError::ApiError(format!("to_vec: {}", e)))?;

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }
}

#[cfg(feature = "embeddings")]
impl EmbeddingProvider for CandleEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.embed_inner(text)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        texts.iter().map(|t| self.embed_inner(t)).collect()
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity_scalar(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity_scalar(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity_scalar(&a, &d) - -1.0).abs() < 0.001);
    }

    #[test]
    fn test_mock_embeddings() {
        let provider = MockEmbeddingProvider::new(128);
        let embedding = provider.embed("test text").unwrap();

        assert_eq!(embedding.len(), 128);

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_embedding_cache() {
        let mut cache = EmbeddingCache::new(3);
        let embedding = vec![1.0, 0.0, 0.0];

        cache.insert("test".to_string(), embedding.clone());
        assert!(cache.contains("test"));
        assert_eq!(cache.get("test"), Some(&embedding));
    }
}
