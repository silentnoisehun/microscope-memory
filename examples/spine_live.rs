//! Spine live test —价目 elindítja a streamet és figyeli a spine.bin-t.
//!
//! Futtatás: cargo run --release --example spine_live

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use microscope_memory::config::Config;
use microscope_memory::spine::Spine;

fn main() {
    let mut config = Config::default();
    config.paths.output_dir = std::env::var("M1C_OUTPUT_DIR")
        .unwrap_or_else(|_| "data".to_string());
    config.embedding.provider = "none".to_string();

    let spine_path = Path::new(&config.paths.output_dir).join("spine.bin");
    let _ = std::fs::remove_file(&spine_path);

    println!("output_dir: {}", config.paths.output_dir);
    println!("starting consciousness stream...\n");

    let stream = microscope_memory::consciousness_stream::ConsciousnessStream::start(&config);

    // Várjuk meg, hogy a háttérszál pár tick-et elküldjön.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last_cycle = 0u64;
    loop {
        if let Ok(sp) = Spine::open(&config.paths.output_dir) {
            let s = sp.read_stream();
            if s.cycle > last_cycle {
                last_cycle = s.cycle;
                println!(
                    "t={:>3}s  cycle={:>4}  surprise={:.3}  curiosity={:.3}  \
                     predict_conf={:.3}  hebbian_E={:.1}",
                    (Instant::now() - (deadline - Duration::from_secs(10))).as_secs(),
                    s.cycle,
                    s.surprise_level,
                    s.curiosity_level,
                    s.predicted_confidence,
                    s.hebbian_total_energy,
                );
            }
            if s.cycle >= 20 {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "spine.bin nem kapta meg az állapotot 10s alatt"
        );
        thread::sleep(Duration::from_millis(200));
    }

    // Várjunk a réteg-fejlécekre is (10 tick-enként jönnek).
    thread::sleep(Duration::from_secs(2));

    let sp = Spine::open(&config.paths.output_dir).unwrap();
    println!("\n── Spine tartalma {} után ──", config.paths.output_dir);
    println!("{}", microscope_memory::spine::format_summary(&sp));

    println!("\n── Stream formátum (in-memory) ──");
    println!(
        "{}",
        microscope_memory::consciousness_stream::ConsciousnessStream::format(&stream)
    );

    println!(
        "\nspine.bin méret: {} bytes ({} oldal @ {} B)",
        sp.size_bytes(),
        sp.size_bytes() / microscope_memory::spine::PAGE_SIZE,
        microscope_memory::spine::PAGE_SIZE
    );

    // Leállítás.
    if let Some(st) = microscope_memory::consciousness_stream::global_stream() {
        let _ = st;
    }
    println!("\nOK — a stream élőben tartja a spine.bin-t.");
}
