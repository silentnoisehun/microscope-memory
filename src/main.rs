//! Microscope Memory CLI — zoom-based hierarchical memory
//!
//! Usage:
//!   microscope-mem build                    # layers/ -> binary mmap
//!   microscope-mem look 0.25 0.25 0.25 3    # x y z zoom
//!   microscope-mem bench                    # speed test
//!   microscope-mem stats                    # structure info
//!   microscope-mem find "Ora"               # text search

use std::fs;

use clap::{Parser, Subcommand};
use colored::Colorize;

use microscope_memory::*;

// ─── CLI ─────────────────────────────────────────────
#[derive(Parser)]
#[command(name = "microscope-mem", about = "Zoom-based hierarchical memory -- pure binary, zero JSON")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build binary from raw layer files
    Build,
    /// Store a new memory
    Store {
        text: String,
        #[arg(short, long, default_value = "long_term")]
        layer: String,
        #[arg(short = 'i', long, default_value = "5")]
        importance: u8,
    },
    /// Recall -- natural language query, auto-zoom
    Recall {
        query: String,
        #[arg(default_value = "10")]
        k: usize,
    },
    /// Manual look: x y z zoom [k]
    Look { x: f32, y: f32, z: f32, zoom: u8, #[arg(default_value = "10")] k: usize },
    /// 4D soft zoom: x y z zoom [k]
    Soft { x: f32, y: f32, z: f32, zoom: u8, #[arg(default_value = "10")] k: usize },
    /// Benchmark
    Bench,
    /// Stats
    Stats,
    /// Text search
    Find { query: String, #[arg(default_value = "5")] k: usize },
    /// Rebuild -- incorporate append log into main index
    Rebuild,
    /// Verify crypto integrity (chain, merkle, or all)
    Verify {
        #[arg(default_value = "all")]
        target: String,
        /// Verify a specific block's Merkle branch
        #[arg(short, long)]
        block: Option<u32>,
    },
    /// Show hash chain status
    ChainStatus,
    /// Show Merkle root and tree info
    MerkleRoot,
    /// Show Hope Genome axioms and hash
    Genome,
    /// Teaching validation: verify a response against memory
    Teach {
        query: String,
        response: String,
    },
    /// Start SHP TCP server
    #[cfg(feature = "shp")]
    Serve {
        #[arg(short, long, default_value = "7946")]
        port: u16,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Build => build(),
        Cmd::Store { text, layer, importance } => {
            store_memory(&text, &layer, importance);
        }
        Cmd::Recall { query, k } => {
            recall(&query, k);
        }
        Cmd::Look { x, y, z, zoom, k } => {
            let r = MicroscopeReader::open();
            let tiered = TieredIndex::build(&r);
            let t0 = std::time::Instant::now();
            let results = tiered.look(&r, x, y, z, zoom, k);
            let elapsed = t0.elapsed();
            let tier = match zoom { 0..=2 => "SoA/SIMD", 3..=5 => "Grid", _ => "SoA/lazy" };
            println!("{} ({:.2},{:.2},{:.2}) zoom={} [{}] ({} ns):",
                "MICROSCOPE".cyan().bold(), x, y, z, zoom,
                tier.cyan(), elapsed.as_nanos());
            for (d, i) in results { r.print_result(i, d); }
        }
        Cmd::Soft { x, y, z, zoom, k } => {
            let r = MicroscopeReader::open();
            println!("{} 4D ({:.2},{:.2},{:.2}) z={}:", "MICROSCOPE".cyan().bold(), x, y, z, zoom);
            for (d, i) in r.look_soft(x, y, z, zoom, k, 2.0) { r.print_result(i, d); }
        }
        Cmd::Bench => bench(&MicroscopeReader::open()),
        Cmd::Stats => {
            let r = MicroscopeReader::open();
            stats(&r);
            let appended = read_append_log();
            if !appended.is_empty() {
                println!("  {}: {} entries (pending rebuild)",
                    "Append log".yellow(), appended.len());
            }
        }
        Cmd::Find { query, k } => {
            let r = MicroscopeReader::open();
            println!("{} '{}':", "FIND".cyan().bold(), query);
            let res = r.find_text(&query, k);
            if res.is_empty() { println!("  (none)"); }
            for (_d, i) in res { r.print_result(i, 0.0); }
        }
        Cmd::Rebuild => {
            println!("{}", "Rebuilding with append log...".cyan());
            build();
            let _ = fs::remove_file(APPEND_PATH);
            println!("  Append log cleared.");
        }
        Cmd::Verify { target, block } => {
            if let Some(idx) = block {
                verify_branch(idx);
            } else {
                match target.as_str() {
                    "chain" => verify_chain(),
                    "merkle" => verify_merkle(),
                    _ => {
                        println!("{}", "Verifying crypto integrity...".cyan().bold());
                        verify_chain();
                        verify_merkle();
                    }
                }
            }
        }
        Cmd::ChainStatus => chain_status(),
        Cmd::MerkleRoot => merkle_root_info(),
        Cmd::Genome => {
            microscope_memory::genome::print_genome();
        }
        Cmd::Teach { query, response } => {
            let reader = MicroscopeReader::open();
            let tiered = TieredIndex::build(&reader);
            let ctx = microscope_memory::teacher::TeachingContext::new(&reader, &tiered);
            let verdict = ctx.verify_response(&query, &response);
            match verdict {
                microscope_memory::teacher::TeachVerdict::Approved { confidence, supporting_blocks } => {
                    println!("  {} confidence={:.1}% ({} supporting blocks)",
                        "APPROVED".green().bold(), confidence * 100.0, supporting_blocks.len());
                    for sb in supporting_blocks.iter().take(5) {
                        println!("    D{} [{}] {}", sb.depth, id_to_layer(sb.layer_id), sb.text_preview);
                    }
                }
                microscope_memory::teacher::TeachVerdict::Denied { reason, violations } => {
                    println!("  {} {}", "DENIED".red().bold(), reason);
                    for v in &violations {
                        match v {
                            microscope_memory::teacher::Violation::Unsupported { claim } =>
                                println!("    Unsupported: {}", claim),
                            microscope_memory::teacher::Violation::Contradiction { claim, memory_text, .. } =>
                                println!("    Contradiction: {} vs {}", claim, memory_text),
                            microscope_memory::teacher::Violation::GenomeViolation { axiom_index, reason } =>
                                println!("    Genome[{}]: {}", axiom_index, reason),
                        }
                    }
                }
            }
        }
        #[cfg(feature = "shp")]
        Cmd::Serve { port } => {
            let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
            rt.block_on(async {
                let server = microscope_memory::shp::server::ShpServer::new(port);
                if let Err(e) = server.run().await {
                    eprintln!("SHP server error: {}", e);
                }
            });
        }
    }
}
