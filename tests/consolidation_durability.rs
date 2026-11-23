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

#[test]
fn truncated_append_is_never_cleared_or_appended_to() {
    use std::io::Write;

    for damaged_tail in [
        vec![1, 2, 3],
        vec![8, 0, 0, 0].into_iter().chain([0; 32]).collect(),
    ] {
        let (_tmp, mut config) = setup();
        store_memory(&config, "valid pending durable memory", "long_term", 5).unwrap();
        let output = Path::new(&config.paths.output_dir);
        let append = output.join("append.bin");
        let mut f = fs::OpenOptions::new().append(true).open(&append).unwrap();
        f.write_all(&damaged_tail).unwrap();
        drop(f);
        let original_log = fs::read(&append).unwrap();
        let original_index = fs::read(output.join("microscope.bin")).unwrap();

        let err = build::rebuild_pending(&config, false, false).unwrap_err();
        assert!(err.contains("incomplete append"), "{err}");
        config.index.auto_rebuild = true;
        assert!(build::maybe_auto_rebuild(&config).is_err());
        assert!(store_memory(&config, "must not append", "long_term", 5).is_err());
        assert_eq!(fs::read(&append).unwrap(), original_log);
        assert_eq!(
            fs::read(output.join("microscope.bin")).unwrap(),
            original_index
        );
    }
}

#[test]
fn oversized_memory_is_rejected_without_persisting_a_truncated_record() {
    let (_tmp, config) = setup();
    let output = Path::new(&config.paths.output_dir);
    let layer = Path::new(&config.paths.layers_dir).join("long_term.txt");
    let before = fs::read(&layer).unwrap();
    let text = format!("{}á", "a".repeat(microscope_memory::BLOCK_DATA_SIZE - 1));
    let err = store_memory(&config, &text, "long_term", 5).unwrap_err();
    assert!(err.contains("maximum"), "{err}");
    assert!(!output.join("append.bin").exists());
    assert_eq!(fs::read(layer).unwrap(), before);
}

#[test]
fn substring_in_another_memory_is_not_a_mirror() {
    let (_tmp, config) = setup();
    let layer = Path::new(&config.paths.layers_dir).join("long_term.txt");
    let marker = "needle only memory";
    store_memory(&config, marker, "long_term", 5).unwrap();
    // Pretend the best-effort mirror failed. A larger entry containing the
    // words is not the stored record and must not suppress re-mirroring.
    fs::write(
        &layer,
        "(imp=5) other prefix needle only memory other suffix\n\nA separate baseline.",
    )
    .unwrap();
    build::rebuild_pending(&config, false, false).unwrap();
    let mirror = fs::read_to_string(&layer).unwrap();
    assert!(mirror
        .split("\n\n")
        .any(|x| x == "(imp=5) needle only memory"));
    build::rebuild_pending(&config, true, false).unwrap();
    assert_find(&config, marker);
}

#[test]
fn headerless_legacy_log_cannot_be_mixed_with_modern_records() {
    let (_tmp, config) = setup();
    let path = Path::new(&config.paths.output_dir).join("append.bin");
    let text = b"legacy unique memory";
    let mut data = Vec::new();
    data.extend_from_slice(&(text.len() as u32).to_le_bytes());
    data.extend_from_slice(&[0, 5]);
    data.extend_from_slice(&[0; 12]);
    data.extend_from_slice(text);
    fs::write(&path, &data).unwrap();
    let err = store_memory(&config, "must not corrupt legacy log", "long_term", 5).unwrap_err();
    assert!(err.contains("legacy"), "{err}");
    assert_eq!(fs::read(&path).unwrap(), data);
    build::rebuild_pending(&config, false, false).unwrap();
    store_memory(&config, "modern subsequent entry", "long_term", 5).unwrap();
    assert_find(&config, "legacy unique memory");
}

#[test]
fn multi_paragraph_pending_record_does_not_get_silently_cleared() {
    let (_tmp, config) = setup();
    let path = Path::new(&config.paths.output_dir).join("append.bin");
    store_memory(&config, "first part\n\nsecond part", "long_term", 5).unwrap();
    let before = fs::read(&path).unwrap();
    let err = build::rebuild_pending(&config, false, false).unwrap_err();
    assert!(err.contains("pending entry missing"), "{err}");
    assert_eq!(fs::read(path).unwrap(), before);
}
