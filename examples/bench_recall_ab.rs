//! A/B recall mérés: indexelt vs teljes pásztázás, **ugyanabban a processzben,
//! felváltva**. Így a géphajrá, a hűtés és a háttérfolyamatok mindkét ágra
//! egyforma hatással vannak — a különbség csak a kód.
//!
//! Két adatkönyvtár kell: azonos blokkokkal, az egyiken van text_index.bin.
//! Előkészítés:
//!   xcopy /E /I /Y data data_noidx
//!   del data_noidx\text_index.bin
//!
//! Futtatás: cargo run --release --example bench_recall_ab -- data data_noidx

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use microscope_memory::config::Config;
use microscope_memory::reader::MicroscopeReader;

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

/// Egy kör: az összes query lefuttatva az adott readeren.
fn run_round(r: &mut MicroscopeReader, cfg: &Config) -> (Duration, usize, usize) {
    let mut total = Duration::ZERO;
    let mut hits = 0usize;
    let mut hits_ranked = 0usize;
    for q in QUERIES {
        let t = Instant::now();
        let found = r.find_text_all_ranked(cfg, q, 10);
        total += t.elapsed();
        hits_ranked += found.len();
        hits += found.iter().filter(|f| f.2).count();
    }
    (total, hits, hits_ranked)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (with_dir, without_dir) = match (args.get(1), args.get(2)) {
        (Some(a), Some(b)) => (PathBuf::from(a), PathBuf::from(b)),
        _ => {
            eprintln!("használat: bench_recall_ab <indexelt_könyvtár> <index_nélküli_könyvtár>");
            std::process::exit(2);
        }
    };

    for d in [&with_dir, &without_dir] {
        let has = d.join("text_index.bin").exists();
        println!(
            "  {}: text_index.bin {}",
            d.display(),
            if has { "VAN" } else { "NINCS" }
        );
    }
    if !with_dir.join("text_index.bin").exists() || without_dir.join("text_index.bin").exists() {
        eprintln!(
            "\nHIBA: az első könyvtárnak kell, hogy tartalmazzza az indexet,\na másodiknak nem szabad."
        );
        std::process::exit(2);
    }

    let cfg_a = config_for(&with_dir);
    let cfg_b = config_for(&without_dir);
    let mut reader_a = MicroscopeReader::open(&cfg_a).expect("indexelt reader");
    let mut reader_b = MicroscopeReader::open(&cfg_b).expect("index nélküli reader");
    assert_eq!(
        reader_a.block_count, reader_b.block_count,
        "a két könyvtár blokkszáma eltér"
    );
    println!(
        "\nindex: {} blokk, {} query",
        reader_a.block_count,
        QUERIES.len()
    );

    // Ugyanazt a lekérdezést mindkettőn: az eredményhalmazoknak egyezniük kell.
    for q in QUERIES {
        let ra = reader_a.find_text_all_ranked(&cfg_a, q, 10);
        let rb = reader_b.find_text_all_ranked(&cfg_b, q, 10);
        assert_eq!(
            ra, rb,
            "AZ EREDMÉNY ELTÉR az index miatt! query={:?}\n indexelt: {:?}\n nélkül:  {:?}",
            q, ra, rb
        );
    }
    println!("helyesség: az indexelt és a teljes keresés eredménye megegyezik minden query-re");

    // Warm-up.
    let _ = run_round(&mut reader_a, &cfg_a);
    let _ = run_round(&mut reader_b, &cfg_b);

    // Felváltva mérünk: 20 A/B kör.
    const ROUNDS: usize = 20;
    let mut a_total = Duration::ZERO;
    let mut b_total = Duration::ZERO;
    let mut a_hits = 0usize;
    let mut b_hits = 0usize;
    for i in 0..ROUNDS {
        let (ta, ha, _) = run_round(&mut reader_a, &cfg_a);
        let (tb, hb, _) = run_round(&mut reader_b, &cfg_b);
        if i == 0 {
            println!("\n  kör=0 (melegítő maradvány, nem számít a végeredménybe)");
            continue;
        }
        a_total += ta;
        b_total += tb;
        a_hits += ha;
        b_hits += hb;
    }
    let rounds = (ROUNDS - 1) as u32;
    let per_a = a_total / (rounds * QUERIES.len() as u32);
    let per_b = b_total / (rounds * QUERIES.len() as u32);

    println!("\n═══════════════════════════════════════════════");
    println!("  {} query × {} kör (felváltva)", QUERIES.len(), rounds);
    println!("═══════════════════════════════════════════════");
    println!(
        "  teljes pásztázás ({} találat)   {:?}",
        b_hits / rounds as usize,
        per_b
    );
    println!(
        "  indexelt        ({} találat)   {:?}",
        a_hits / rounds as usize,
        per_a
    );
    println!("  ─────────────────────────────────────────────");
    println!(
        "  gyorsulás: {:.2}×  ({:?} → {:?} / query)",
        b_total.as_secs_f64() / a_total.as_secs_f64(),
        per_b,
        per_a
    );
}
