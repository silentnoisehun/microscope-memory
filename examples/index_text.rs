//! Build the inverted text index (text_index.bin) for an existing data dir.
//!
//! The index is written by the heavy `build` path, but an index installed by
//! an older release (or one whose copy lost the sidecar) can be regenerated
//! without rebuilding the whole corpus: this walks the blocks the reader
//! already maps and emits the TIX1 file in place.
//!
//! Futtatás: cargo run --release --example index_text -- [adatkönyvtár]

use std::path::PathBuf;

use microscope_memory::config::Config;
use microscope_memory::reader::MicroscopeReader;
use microscope_memory::text_index::build_text_index;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));

    let mut cfg = Config::default();
    cfg.paths.output_dir = dir.to_string_lossy().to_string();
    cfg.paths.layers_dir = dir.to_string_lossy().to_string();
    cfg.paths.temp_dir = dir.to_string_lossy().to_string();

    let t = std::time::Instant::now();
    let reader = MicroscopeReader::open(&cfg).expect("reader nyitás — futtasd előbb 'build'-et");
    let open_ms = t.elapsed();
    println!(
        "reader: {} blokk, {} ms",
        reader.block_count,
        open_ms.as_millis()
    );

    let t = std::time::Instant::now();
    build_text_index(
        (0..reader.block_count).map(|i| reader.text(i)),
        reader.block_count,
        &dir.join("text_index.bin"),
    )
    .expect("text_index.bin építés");
    let build_ms = t.elapsed();

    let index_bytes = std::fs::metadata(dir.join("text_index.bin"))
        .map(|m| m.len())
        .unwrap_or(0);

    // Visszaolvasás és ellenőrzés, hogy érvényes-e.
    match microscope_memory::text_index::TextIndex::open(&dir.join("text_index.bin")) {
        Some(idx) => {
            println!(
                "text_index.bin: {} szó, {} blokk, {} KB, {} ms alatt épült",
                idx.word_count_public(),
                idx.block_count(),
                index_bytes / 1024,
                build_ms.as_millis()
            );
            println!("megnyitás sikeres — a find_text mostantól indexelt.");
        }
        None => {
            eprintln!("HIBA: a generált index nem nyitható meg vissza");
            std::process::exit(1);
        }
    }
}
