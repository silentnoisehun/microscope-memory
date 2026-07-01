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
    /// Build binary index from raw layer files (from scratch)
    Build {
        /// Force rebuild even if layer files are unchanged
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
        /// Status flag: open (open loop) | resolved | archived | normal
        #[arg(long)]
        status: Option<String>,
    },
    /// Show timeline (chronological stores) by window
    Timeline {
        /// Time window: today, yesterday, last_N_days, since:YYYY-MM-DD,
        /// last_session, all
        #[arg(default_value = "last_session")]
        window: String,
        #[arg(default_value = "20")]
        k: usize,
    },
    /// List currently-open loops
    Loops {
        #[arg(default_value = "50")]
        k: usize,
    },
    /// Mark a loop resolved
    ResolveLoop { id: u64 },
    /// Universal auto-context snapshot — for any LLM wrapper script.
    /// Writes to stdout (default) or to a file path.
    AutoContext {
        /// Compact mode (no box-drawing)
        #[arg(long)]
        compact: bool,
        /// Output to this file instead of stdout
        #[arg(long)]
        output: Option<String>,
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
    /// Rebuild — merge pending observations from append log into the main index
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
    /// Start the MCP (Model Context Protocol) server for Claude Desktop integration
    Mcp,
    /// Print drop-in MCP server config + auto-context wrapper instructions
    /// for a specific AI client: claude | hermes | cursor | cline | generic
    Config {
        /// Target client: claude, hermes, cursor, cline, generic
        client: String,
    },
    /// Run integrity diagnostics and attempt automatic repair (Crash Recovery)
    Doctor {
        /// Attempt to fix common corruption issues (e.g. malformed append log tail)
        #[arg(long)]
        fix: bool,
    },
    /// Start the Mermaid Terminal (WebSocket + HTML UI on port 8080)
    Mermaid {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Autonomous mode - the system runs itself: daydream, curiosity, monologue, reflect, narrative, dream
    Autonomous {
        /// Enable TTS (text-to-speech) via Windows System.Speech
        #[arg(long)]
        tts: bool,
        /// Run as daemon (continuous loop) instead of single cycle
        #[arg(long)]
        daemon: bool,
        /// Cycle interval in seconds (default: 30)
        #[arg(long, default_value = "30")]
        interval: u64,
        /// Maximum number of cycles (default: infinite in daemon mode, 1 in single mode)
        #[arg(long)]
        max_cycles: Option<usize>,
    },
    /// Introspect - self-reflection: the system thinks about itself
    Introspect,
    /// SelfModel - show the system's self-model snapshot
    SelfModel,
    /// Curiosity - show what the system is curious about
    Curiosity,
    /// Monologue - generate an inner monologue (the system thinking)
    Monologue,
    /// Stories - show narrative memory episodes (story arcs from recalls)
    Stories {
        #[arg(default_value = "5")]
        k: usize,
    },
    /// Daydream - associative drift (mind wandering)
    Daydream {
        /// Seed text to start from (default: last narrative)
        #[arg(default_value = "")]
        seed: String,
        /// Number of drift steps
        #[arg(default_value = "3")]
        steps: usize,
    },
    /// Hyperfocus - enter deep concentration mode on a topic
    Hyperfocus {
        /// Target topic
        target: String,
        /// Focus type: planning, problem_solving, creative, research
        #[arg(default_value = "research")]
        focus_type: String,
    },
    /// Key management — binary key store (keys.bin)
    Keys {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Zen key management — binary zen key store (zen_keys.bin)
    ZenKeys {
        #[command(subcommand)]
        action: ZenKeyAction,
    },
}

#[derive(Subcommand)]
pub enum KeyAction {
    /// Set a key: keys set <service> <key> [priority]
    Set {
        /// Service name: openai | gemini | ollama
        service: String,
        /// The API key
        key: String,
        /// Priority (0=primary, 1=secondary, ...)
        #[arg(default_value = "0")]
        priority: u8,
    },
    /// Remove a key: keys remove <service> [priority]
    Remove {
        /// Service name
        service: String,
        /// Priority (omit to remove all for this service)
        priority: Option<u8>,
    },
    /// List all stored keys (without revealing them)
    List,
    /// Show key status (quota, errors, disabled state)
    Status,
    /// Reset all disabled keys
    Reset,
}

#[derive(Subcommand)]
pub enum ZenKeyAction {
    /// Import zen_keys.json → zen_keys.bin
    Import {
        /// Path to zen_keys.json
        #[arg(default_value = "zen_keys.json")]
        json_path: String,
        /// Output path for zen_keys.bin
        #[arg(long, default_value = "zen_keys.bin")]
        output: String,
    },
    /// Show zen key store stats
    Stats,
    /// List all keys (without revealing them)
    List,
    /// Show key status (quota, errors, disabled state)
    Status,
    /// Reset all disabled keys
    Reset,
}
