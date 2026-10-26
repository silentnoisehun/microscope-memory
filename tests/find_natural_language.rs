//! Regressziós tesztek a `find` parancs természetes nyelvű keresésére.
//!
//! A `find` korábbi viselkedése: a teljes query-t EGYETLEN literálként kereste
//! (`text.contains(query)`). Az agent hook-ok viszont egy teljes, természetes
//! nyelvű kérdést adnak át neki — és egy blokk szinte soha nem tartalmazza
//! szó szerint az egész kérdést. A `find` ezért minden valódi kérdésre
//! `(none)`-t adott vissza, vagyis a Claude Code master kernel memória-karja
//! csendben kikapcsolt.
//!
//! A javítás: ha a pontos mondat nincs meg, a kereső többértelmű (token) alakban
//! is fut, a lexical prefilteren keresztül, és a relevance-scorer dönti el,
//! melyik találat a jó.

use std::fs;

/// Build a tiny index whose blocks hold the facts we will query for.
fn setup_fact_env() -> (tempfile::TempDir, microscope_memory::config::Config) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output_dir = tmp.path().join("data");
    let layers_dir = tmp.path().join("layers");
    fs::create_dir_all(&output_dir).unwrap();
    fs::create_dir_all(&layers_dir).unwrap();

    let facts = "ORBITA-7 deployment: the production orchestrator listens on port 4471 and its telemetry sink is port 4472. The deployment codename is Kestrel.\n\nProject HELIOS: the thermal control firmware must be rebuilt with the ZEPHYR-3 toolchain, never with the older FORGE-1 compiler.\n\nInternal metric naming convention: services prefix their metrics with svc_ and the unit suffix is _bytes, _ms or _count.";
    fs::write(layers_dir.join("long_term.txt"), facts).unwrap();

    let mut config = microscope_memory::config::Config::default();
    config.paths.layers_dir = layers_dir.to_string_lossy().to_string();
    config.paths.output_dir = output_dir.to_string_lossy().to_string();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string()];
    config.embedding.provider = "mock".to_string();

    (tmp, config)
}

#[test]
fn find_answers_a_natural_language_question() {
    let (_tmp, config) = setup_fact_env();
    microscope_memory::build::build(&config, true, true).expect("build failed");
    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    // A teljes kérdés szó szerint nem szerepel egyetlen blokkban sem.
    let query = "What port does the ORBITA-7 production orchestrator listen on";
    let ranked = reader.find_text_ranked(query, 5);

    assert!(
        !ranked.is_empty(),
        "a természetes nyelvű kérdésre a find-nek találnia kell"
    );

    let texts: Vec<String> = ranked
        .iter()
        .map(|&(_, idx, _)| reader.text(idx).to_string())
        .collect();
    let joined = texts.join(" | ");
    assert!(
        joined.contains("4471"),
        "a találatok között kell lennie a 4471-es portnak, got: {joined}"
    );
    assert!(
        joined.contains("4472"),
        "a találatok között kell lennie a 4472-es portnak, got: {joined}"
    );
}

#[test]
fn find_keeps_the_exact_phrase_on_top() {
    let (_tmp, config) = setup_fact_env();
    microscope_memory::build::build(&config, true, true).expect("build failed");
    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    // Pontos literál: a találatok között mindenképp legyen.
    let query = "ZEPHYR-3";
    let ranked = reader.find_text_ranked(query, 5);
    assert!(!ranked.is_empty(), "a pontos literálnak találnia kell");

    // A részleges mód nem szabad, hogy a pontos találatot elsüllyessze.
    let exact_first = reader.text(ranked[0].1).contains("ZEPHYR-3");
    assert!(
        exact_first,
        "a pontos literálnak a legelső találatnak kell lennie"
    );
}

#[test]
fn single_term_search_stays_exact() {
    let (_tmp, config) = setup_fact_env();
    microscope_memory::build::build(&config, true, true).expect("build failed");
    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    // Egyetlen szó esetén nincs "részleges" alak: ha egy blokk "Kestrel"-
    // lalálatot ad, az a blokk tényleg tartalmazza a szót.
    let ranked = reader.find_text_ranked("Kestrel", 5);
    for &(_, idx, _) in &ranked {
        assert!(
            reader.text(idx).contains("Kestrel"),
            "egy szavas keresés csak valódi találatokat adhat"
        );
    }
}

#[test]
fn find_with_no_match_stays_empty() {
    let (_tmp, config) = setup_fact_env();
    microscope_memory::build::build(&config, true, true).expect("build failed");
    let reader = microscope_memory::reader::MicroscopeReader::open(&config).expect("open reader");

    // Olyan szavak, amik egyetlen blokkban sem szerepelnek.
    let ranked = reader.find_text_ranked("zzqqxx nonexistent unicorn tokenizer", 5);
    assert!(
        ranked.is_empty(),
        "a nem létező fogalmakra nem kell találatot adnia"
    );
}
