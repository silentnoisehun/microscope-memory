//! Recall hot-path benchmark Ä‚ËĂ˘â€šÂ¬Ă˘â‚¬ĹĄ a valĂ„â€šÄąâ€šdi query Ă„â€šÄąĹşt mĂ„â€šĂ‚Â©rĂ„â€šĂ‚Â©se.
//!
//! FĂ„â€šĂ‹â€ˇjl: examples/bench_recall.rs
//! FuttatĂ„â€šĂ‹â€ˇs: cargo run --release --example bench_recall -- [kĂ„â€šĂ‚Â¶rnyezet adatkĂ„â€šĂ‚Â¶nyvtĂ„â€šĂ‹â€ˇr]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use microscope_memory::config::Config;
use microscope_memory::reader::MicroscopeReader;
use microscope_memory::spine::Spine;

/// A benchmark hasznĂ„â€šĂ‹â€ˇlatos lekĂ„â€šĂ‚Â©rdezĂ„â€šĂ‚Â©sei (lĂ„â€šĂ‹â€ˇsd mĂ„â€šĂ‚Â©g: a real query log).
const QUERIES: &[&str] = &[
    "stratos",
    "memory architecture",
    "hebbian learning",
    "emotional state",
    "maradĂ„â€šĂ‚Â©k",
    "projekt cĂ„â€šĂ‚Â©lkitĂ„Ä…Ă‚Â±zĂ„â€šĂ‚Â©s",
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

    println!("Ä‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚Â");
    println!("  MICROSCOPE RECALL Ä‚ËĂ˘â€šÂ¬Ă˘â‚¬ĹĄ hot path benchmark");
    println!("Ä‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚ÂÄ‚ËĂ˘â‚¬ËĂ‚Â\n");
    println!("adatkĂ„â€šĂ‚Â¶nyvtĂ„â€šĂ‹â€ˇr: {}\n", data_dir.display());

    let cfg = config_for(&data_dir);
    let mut reader = match MicroscopeReader::open(&cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("reader nem nyithatĂ„â€šÄąâ€š: {} (futtasd 'build'-el)", e);
            return;
        }
    };

    println!("index: {} blokk\n", reader.block_count);

    // Warm-up: elsĂ„Ä…Ă˘â‚¬Â talĂ„â€šĂ‹â€ˇlat gyorsĂ„â€šĂ‚Â­tĂ„â€šÄąâ€štĂ„â€šĂ‹â€ˇr nĂ„â€šĂ‚Â©lkĂ„â€šĂ„Ëťl is legyen mĂ„â€šĂ‚Â©rendĂ„Ä…Ă˘â‚¬Â.
    let _ = reader.find_text("warmup", 5);

    let n = 200usize;

    // Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬ 1. find_text (case-insensitive contains + depth sort) Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬
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
        "1. find_text ({} query Ă„â€šĂ˘â‚¬â€ť {})      {:>10?}/query   ({} talĂ„â€šĂ‹â€ˇlat)",
        QUERIES.len(),
        n,
        per,
        hits
    );

    // Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬ 2. find_text_ranked Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬
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
        "2. find_text_ranked                    {:>10?}/query   ({} talĂ„â€šĂ‹â€ˇlat)",
        per, hits
    );

    // Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬ 3. find_text_all_ranked (main + append log) Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬
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
        "3. find_text_all_ranked (append log)   {:>10?}/query   ({} talĂ„â€šĂ‹â€ˇlat)",
        per, hits
    );

    // Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬ 4. look() Ä‚ËĂ˘â€šÂ¬Ă˘â‚¬ĹĄ 3D tĂ„â€šĂ‚Â©rbeli lookup Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬
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

    // Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬ 5. radial_search Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬
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

    // Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬ 6. A Spine a recall kĂ„â€šĂ‚Â¶zben: olvasĂ„â€šĂ‹â€ˇs nem IO Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬Ä‚ËĂ˘â‚¬ĹĄĂ˘â€šÂ¬
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
        "6. spine StreamPage olvasĂ„â€šĂ‹â€ˇs             {:>10?}/read",
        t.elapsed() / n2
    );

    println!("\nKĂ„â€šĂ‚Â©sz Ä‚ËĂ˘â€šÂ¬Ă˘â‚¬ĹĄ a fenti szĂ„â€šĂ‹â€ˇmok a baseline-ek a optimalizĂ„â€šĂ‹â€ˇlĂ„â€šĂ‹â€ˇshoz.");
}
