//! Recall hot-path benchmark — a valódi query út mérése.
//!
//! Fájl: examples/bench_recall.rs
//! Futtatás: cargo run --release --example bench_recall -- [környezet adatkönyvtár]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use microscope_memory::config::Config;
use microscope_memory::reader::MicroscopeReader;
use microscope_memory::spine::Spine;

/// A benchmark használatos lekérdezései (lásd még: a real query log).
const QUERIES: &[&str] = &[
    "stratos",
    "memory architecture",
    "hebbian learning",
    "emotional state",
    "maradék",
    "projekt célkitűzés",
];

fn config_for(dir: &Path) -> Config {
    let mut cfg = Config::default();
    cfg.paths.output_dir = dir.to_string_lossy().to_string();
    cfg.paths.layers_dir = dir.to_string_lossy().to_string();
    cfg.paths.temp_dir = dir.to_string_lossy().to_string();
    cfg
}

fn main() {
    let data_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));

    println!("═══════════════════════════════════════════════");
    println!("  MICROSCOPE RECALL — hot path benchmark");
    println!("═══════════════════════════════════════════════\n");
    println!("adatkönyvtár: {}\n", data_dir.display());

    let cfg = config_for(&data_dir);
    let mut reader = match MicroscopeReader::open(&cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("reader nem nyitható: {} (futtasd 'build'-el)", e);
            return;
        }
    };

    println!("index: {} blokk\n", reader.block_count);

    // Warm-up: első találat gyorsítótár nélkül is legyen mérendő.
    let _ = reader.find_text("warmup", 5);

    let n = 200usize;

    // ── 1. find_text (case-insensitive contains + depth sort) ──
    let mut total = Duration::ZERO;
    let mut hits = 0usize;
    for q in QUERIES {
        let t = Instant::now();
        for _ in 0..n {
            let r = reader.find_text(q, 10);
            hits += r.len();
        }
        total += t.elapsed();
    }
    let per = total / (n * QUERIES.len()) as u32;
    println!(
        "1. find_text ({} query × {})      {:>10?}/query   ({} találat)",
        QUERIES.len(),
        n,
        per,
        hits
    );

    // ── 2. find_text_ranked ──
    let mut total = Duration::ZERO;
    let mut hits = 0usize;
    for q in QUERIES {
        let t = Instant::now();
        for _ in 0..n {
            let r = reader.find_text_ranked(q, 10);
            hits += r.len();
        }
        total += t.elapsed();
    }
    let per = total / (n * QUERIES.len()) as u32;
    println!(
        "2. find_text_ranked                    {:>10?}/query   ({} találat)",
        per, hits
    );

    // ── 3. find_text_all_ranked (main + append log) ──
    let mut total = Duration::ZERO;
    let mut hits = 0usize;
    for q in QUERIES {
        let t = Instant::now();
        for _ in 0..n {
            let r = reader.find_text_all_ranked(&cfg, q, 10);
            hits += r.len();
        }
        total += t.elapsed();
    }
    let per = total / (n * QUERIES.len()) as u32;
    println!(
        "3. find_text_all_ranked (append log)   {:>10?}/query   ({} találat)",
        per, hits
    );

    // ── 4. look() — 3D térbeli lookup ──
    let mut total = Duration::ZERO;
    for _ in 0..n {
        let t = Instant::now();
        let _ = reader.look(&cfg, 0.5, 0.5, 0.5, 3, 5);
        total += t.elapsed();
    }
    println!(
        "4. look() 3D center                    {:>10?}/call",
        total / n as u32
    );

    // ── 5. radial_search ──
    let mut total = Duration::ZERO;
    for _ in 0..n {
        let t = Instant::now();
        let _ = reader.radial_search(&cfg, 0.5, 0.5, 0.5, 1, 5.0, 10);
        total += t.elapsed();
    }
    println!(
        "5. radial_search r=1.0                 {:>10?}/call\n",
        total / n as u32
    );

    // ── 6. A Spine a recall közben: olvasás nem IO ──
    let spine_dir = std::env::temp_dir().join("m1c_bench_recall_spine");
    let _ = std::fs::remove_dir_all(&spine_dir);
    let spine = Spine::open(&spine_dir).unwrap();
    spine.tick(1, 0.5, 0.5, 1, 1.0, 1, 1, 1.0);

    let n2 = 200_000u32;
    let t = Instant::now();
    for i in 0..n2 {
        let _ = spine.read_stream();
        if i % 1000 == 0 {
            let _ = spine.read_hebbian();
        }
    }
    println!(
        "6. spine StreamPage olvasás             {:>10?}/read",
        t.elapsed() / n2
    );

    println!("\nKész — a fenti számok a baseline-ek a optimalizáláshoz.");
}
