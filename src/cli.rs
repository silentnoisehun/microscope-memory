//! CLI definitions for Microscope Memory.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "microscope-mem",
    about = "Zoom-based hierarchical memory — pure binary, zero JSON"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Build binary from raw layer files
    Build {
        #[arg(long)]
        force: bool,
    },
    /// Store a new memory
    Store {
        text: String,
        #[arg(short, long, default_value = "long_term")]
        layer: String,
        #[arg(short = 'i', long, default_value = "5")]
        importance: u8,
    },
    /// Recall — natural language query, auto-zoom
    Recall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Manual look: x y z zoom [k]
    Look {
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Radial search: find blocks within radius at a depth
    Radial {
        x: f32,
        y: f32,
        z: f32,
        depth: u8,
        #[arg(short, long, default_value = "0.1")]
        radius: f32,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// 4D soft zoom: x y z zoom [k]
    Soft {
        x: f32,
        y: f32,
        z: f32,
        zoom: u8,
        #[arg(default_value = "10")]
        k: usize,
        /// Use GPU acceleration (requires gpu feature)
        #[arg(long)]
        gpu: bool,
    },
    /// Benchmark
    Bench,
    /// Stats
    Stats,
    /// Text search
    Find {
        query: String,
        #[arg(default_value = "5")]
        k: usize,
    },
    /// Build structural fingerprints and wormhole links
    Fingerprint,
    /// Show structural links (wormholes) for a block
    Links {
        #[arg(help = "Block index")]
        block_index: usize,
    },
    /// Find structurally similar blocks to a text
    Similar {
        text: String,
        #[arg(default_value = "5")]
        k: usize,
    },
    /// Rebuild — incorporate append log into main index
    Rebuild,
    /// Semantic search using embeddings
    Embed {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
        #[arg(short, long, default_value = "cosine")]
        metric: String,
    },
    /// GPU vs CPU benchmark (requires gpu feature)
    GpuBench,
    /// Verify CRC16 integrity of all blocks
    Verify,
    /// Verify Merkle tree integrity of the entire index
    VerifyMerkle,
    /// Show Merkle proof for a specific block
    Proof {
        #[arg(help = "Block index")]
        block_index: usize,
    },
    /// Sequential Thinking — Chain-of-Thought memory sequence
    Think {
        query: String,
        #[arg(default_value = "5")]
        max_steps: usize,
    },
    /// Start the Binary Spine IPC listener (Zero JSON)
    Spine,
    /// MQL query (Microscope Query Language)
    Query {
        /// MQL expression, e.g. 'layer:long_term depth:2..5 "Ora"'
        mql: String,
    },
    /// Export index to .mscope archive
    Export {
        /// Output archive path
        output: String,
    },
    /// Import .mscope archive
    Import {
        /// Input archive path
        input: String,
        /// Output directory (defaults to config output_dir)
        #[arg(long)]
        output_dir: Option<String>,
    },
    /// Diff two .mscope archives
    Diff {
        /// First archive
        a: String,
        /// Second archive
        b: String,
    },
    /// Federated recall across multiple indices
    FederatedRecall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Exchange resonance pulses across federated indices (mirror neuron protocol)
    PulseExchange,
    /// Federated text search across multiple indices
    FederatedFind {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show Hebbian learning state (activations, co-activations, energy)
    Hebbian,
    /// Apply Hebbian drift — co-activated blocks pull coordinates closer
    HebbianDrift,
    /// Show hottest blocks (most recently/frequently activated)
    Hottest {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show emerged archetypes (crystallized activation patterns)
    Archetypes,
    /// Detect new archetypes from resonance field and Hebbian state
    Emerge,
    /// Show resonance protocol state (pulses, field energy)
    Resonance,
    /// Integrate received pulses into local Hebbian state
    Integrate,
    /// Show mirror neuron state (resonance echoes, boosted blocks)
    Mirror,
    /// Show most resonant blocks (strongest mirror neuron signal)
    Resonant {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Export 3D visualization snapshot (Binary)
    Viz {
        /// Output file path (default: viz.bin)
        #[arg(default_value = "viz.bin")]
        output: String,
    },
    /// Export binary density map for fast rendering
    Density {
        /// Output file path
        #[arg(default_value = "density.bin")]
        output: String,
        /// Grid resolution (default: 32)
        #[arg(short, long, default_value = "32")]
        grid: u16,
    },

    /// Show thought patterns (crystallized recall sequences)
    Patterns {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show recent thought paths (recall sequences by session)
    Paths {
        #[arg(default_value = "5")]
        sessions: usize,
    },
    /// Show predictive cache stats and active predictions
    Predictions,
    /// Show temporal archetype patterns (time-of-day activation profiles)
    TemporalPatterns,
    /// Show attention mechanism state (layer weights, quality history)
    Attention,
    /// Exchange thought patterns across federated indices
    PatternExchange,
    /// Run dream consolidation (offline memory replay and pruning)
    Dream,
    /// Show dream consolidation history
    DreamLog {
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Show emotional contagion state (local + remote emotional fields)
    EmotionalField,
    /// Exchange emotional snapshots across federated indices
    EmotionalExchange,
    /// Show multimodal index statistics
    Modalities,
    /// Export full cognitive map (all 13 layers) as JSON for Three.js viewer
    CognitiveMap {
        /// Output file path (default: cognitive_map.bin)
        #[arg(default_value = "cognitive_map.bin")]
        output: String,
    },
    /// Store structured data (key=value pairs)
    StoreData {
        /// Key-value pairs: key1=val1 key2=val2
        pairs: Vec<String>,
        #[arg(short = 'i', long, default_value = "5")]
        importance: u8,
    },
    /// Initialize a demo dataset and configuration for quickstart
    InitDemo {
        /// Force overwrite existing layers/demo.txt
        #[arg(long)]
        force: bool,
    },
    /// Start a local HTTP server for the 3D Viewer (viewer.html)
    Serve {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}
