//! CLI definitions for Microscope Memory.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "microscope-mem",
    about = "Zoom-based hierarchical memory — pure binary, zero JSON"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
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
    /// Start the HTTP server
    Serve {
        #[arg(short, long, default_value_t = 6060)]
        port: u16,
    },
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
}
