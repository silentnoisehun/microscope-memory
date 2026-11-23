//! Release gate for acknowledged stores when the layer mirror fails.

use microscope_memory::{build, config::Config, reader::MicroscopeReader, store_memory};
use std::fs;
use std::path::Path;

fn setup() -> (tempfile::TempDir, Config) {
    let tmp = tempfile::tempdir().unwrap();
    let layers = tmp.path().join("layers");
    let output = tmp.path().join("output");
    fs::create_dir_all(&layers).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(layers.join("long_term.txt"), "Original baseline memory.").unwrap();
    let mut config = Config::default();
    config.paths.layers_dir = layers.to_string_lossy().into_owned();
    config.paths.output_dir = output.to_string_lossy().into_owned();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().into_owned();
    config.memory_layers.layers = vec!["long_term".into()];
    config.embedding.provider = "none".into();
    config.index.auto_rebuild = false;
    config.index.layer_retention_entries = 0;
    build::build(&config, true, false).unwrap();
    (tmp, config)
}

fn assert_find(config: &Config, marker: &str) {
    let reader = MicroscopeReader::open(config).unwrap();
    assert!(
        !reader.find_text_all(config, marker, 20).is_empty(),
        "missing marker: {marker}"
    );
}

#[test]
fn acknowledged_store_survives_failed_mirror_and_later_consolidations() {
    let (_tmp, mut config) = setup();
    let layers = std::path::PathBuf::from(&config.paths.layers_dir);
    let output = std::path::PathBuf::from(&config.paths.output_dir);
    // A file in place of the directory causes store's best-effort mirror to fail.
    fs::remove_dir_all(&layers).unwrap();
    fs::write(&layers, "not a directory").unwrap();
    let marker = "zirconriver928";
    store_memory(&config, marker, "long_term", 5).unwrap();
    assert_find(&config, marker);
    let append = output.join("append.bin");
    let before_log = fs::read(&append).unwrap();
    let before_index = fs::read(output.join("microscope.bin")).unwrap();

    // The first consolidation must fail without modifying either the log or index.
    assert!(build::rebuild_pending(&config, false, false).is_err());
    assert_eq!(fs::read(&append).unwrap(), before_log);
    assert_eq!(
        fs::read(output.join("microscope.bin")).unwrap(),
        before_index
    );
    assert_find(&config, marker);

    fs::remove_file(&layers).unwrap();
    fs::create_dir(&layers).unwrap();
    fs::write(layers.join("long_term.txt"), "Original baseline memory.").unwrap();
    build::rebuild_pending(&config, false, false).unwrap();
    assert!(!append.exists());
    assert_find(&config, marker);

    // A large independent corpus defeats a block-count-only shrink guard.
    let corpus = (0..200)
        .map(|i| format!("Independent corpus item {i}: a different synthetic memory."))
        .collect::<Vec<_>>()
        .join("\n\n");
    let layer_file = layers.join("long_term.txt");
    let current = fs::read_to_string(&layer_file).unwrap();
    fs::write(&layer_file, format!("{current}\n\n{corpus}")).unwrap();
    config.index.auto_rebuild = true;
    config.index.auto_rebuild_entries = 1;
    store_memory(&config, "independent trigger memory", "long_term", 5).unwrap();
    assert!(
        !append.exists(),
        "second auto-rebuild should have completed"
    );
    assert_find(&config, marker);
    let reader = MicroscopeReader::open(&config).unwrap();
    assert!(
        !reader.find_text(marker, 20).is_empty(),
        "marker must be in main index"
    );
}

#[test]
fn mirror_write_and_read_failures_preserve_log_and_index() {
    for fail_read in [false, true] {
        let (_tmp, config) = setup();
        let layers = Path::new(&config.paths.layers_dir);
        let output = Path::new(&config.paths.output_dir);
        store_memory(&config, "quartzember416", "long_term", 5).unwrap();
        // Block either the temporary write or the mirror read. Directories
        // make this deterministic even when run as root.
        let layer_file = layers.join("long_term.txt");
        fs::remove_file(&layer_file).unwrap();
        if fail_read {
            fs::create_dir(&layer_file).unwrap();
        } else {
            fs::create_dir(layers.join("long_term.txt.tmp")).unwrap();
        }
        let log = fs::read(output.join("append.bin")).unwrap();
        let index = fs::read(output.join("microscope.bin")).unwrap();
        let err = build::rebuild_pending(&config, false, false).unwrap_err();
        // A directory at the target must not be treated as an empty mirror.
        assert!(err.contains("mirror"), "{err}");
        assert_eq!(fs::read(output.join("append.bin")).unwrap(), log);
        assert_eq!(fs::read(output.join("microscope.bin")).unwrap(), index);
        assert_find(&config, "quartzember416");
    }
}

#[test]
fn pending_batch_trimmed_by_retention_is_not_cleared() {
    let (_tmp, mut config) = setup();
    store_memory(&config, "first pending synthetic marker", "long_term", 5).unwrap();
    store_memory(&config, "second pending synthetic marker", "long_term", 5).unwrap();
    // Simulate both pending records being absent from the recovered mirror.
    fs::write(
        Path::new(&config.paths.layers_dir).join("long_term.txt"),
        "",
    )
    .unwrap();
    config.index.layer_retention_entries = 1;
    let append = Path::new(&config.paths.output_dir).join("append.bin");
    let log = fs::read(&append).unwrap();
    let err = build::rebuild_pending(&config, false, false).unwrap_err();
    assert!(err.contains("pending entry missing"), "{err}");
    assert_eq!(fs::read(&append).unwrap(), log);
    config.index.layer_retention_entries = 0;
    build::rebuild_pending(&config, false, false).unwrap();
    assert_find(&config, "first pending synthetic marker");
    assert_find(&config, "second pending synthetic marker");
}

#[test]
fn shared_prefix_does_not_hide_an_unmirrored_entry() {
    let (_tmp, config) = setup();
    let layers = Path::new(&config.paths.layers_dir);
    let first = format!("{} firstdistinct", "shared prefix ".repeat(7));
    let second = format!("{} secondunique", "shared prefix ".repeat(7));
    store_memory(&config, &first, "long_term", 5).unwrap();
    let layer_file = layers.join("long_term.txt");
    let first_mirror = fs::read(&layer_file).unwrap();
    store_memory(&config, &second, "long_term", 5).unwrap();
    fs::write(layer_file, first_mirror).unwrap();
    build::rebuild_pending(&config, false, false).unwrap();
    store_memory(&config, "another independent trigger", "long_term", 5).unwrap();
    build::rebuild_pending(&config, false, false).unwrap();
    let reader = MicroscopeReader::open(&config).unwrap();
    assert!(!reader.find_text("secondunique", 20).is_empty());
}

#[test]
fn unchanged_layers_missing_semantic_sidecar_are_rebuilt() {
    use microscope_memory::embedding_index::EmbeddingIndex;

    let (_tmp, mut config) = setup();
    config.embedding.provider = "mock".into();
    config.embedding.dim = 64;
    build::build(&config, true, true).unwrap();
    let embeddings = Path::new(&config.paths.output_dir).join("embeddings.bin");
    assert!(EmbeddingIndex::open(&embeddings).is_some());
    fs::remove_file(&embeddings).unwrap();
    // The layer hash is unchanged; this must not take the incremental skip.
    build::build(&config, false, true).unwrap();
    let index = EmbeddingIndex::open(&embeddings).expect("semantic sidecar restored");
    assert!(index.block_count() > 0);
    assert_eq!(index.dim(), 64);
}
