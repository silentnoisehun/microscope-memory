//! Negative tests for a damaged index.
//!
//! `MicroscopeReader` reads its headers through `header_unchecked`, whose
//! documented precondition is `i < block_count`. That count comes out of
//! meta.bin, not out of the file the headers are mapped from. Nothing at open
//! time connects the two, so an index whose meta.bin survived and whose
//! microscope.bin was truncated opens cleanly and then reads past the mapping.
//!
//! These cases build a real index, damage one file, and require `open` to refuse.
//! The assertion is on `open`, not on a subsequent read: the point of the fix is
//! that the damage is refused at the boundary, and a test that had to read out
//! of bounds to prove the bug would take the test runner with it.

use std::fs;
use std::path::{Path, PathBuf};

use microscope_memory::config::Config;

fn build_index(root: &Path) -> Config {
    let layers = root.join("layers");
    fs::create_dir_all(&layers).unwrap();
    fs::write(
        layers.join("long_term.txt"),
        "Microscope Memory uses hierarchical indexing.\n\n\
         Memory management in Rust uses ownership and borrowing.\n\n\
         A third statement, long enough to survive the text floor.\n",
    )
    .unwrap();

    let mut config = Config::default();
    config.paths.layers_dir = layers.to_string_lossy().to_string();
    config.paths.output_dir = root.join("data").to_string_lossy().to_string();
    config.paths.temp_dir = root.join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string()];
    config.embedding.provider = "mock".to_string();
    microscope_memory::build::build(&config, true, true).unwrap();
    config
}

fn truncate(path: &Path, keep: usize) {
    let data = fs::read(path).unwrap();
    assert!(data.len() > keep, "file is already shorter than {keep}");
    fs::write(path, &data[..keep]).unwrap();
}

/// A meta.bin that still claims the original block count, over a header file
/// that no longer holds them.
#[test]
fn headers_shorter_than_the_claimed_block_count_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let config = build_index(tmp.path());
    let out = PathBuf::from(&config.paths.output_dir);
    let headers = out.join("microscope.bin");
    let original = fs::metadata(&headers).unwrap().len();
    assert!(original > 0, "build produced no headers");

    // Keep one header's worth, drop the rest. meta.bin is untouched.
    truncate(&headers, 50);

    let err = microscope_memory::reader::MicroscopeReader::open(&config);
    assert!(
        err.is_err(),
        "microscope.bin truncated from {original} to 50 bytes while meta.bin still \
         claims the full count: open() accepted it, so header_unchecked can read \
         past the mapping"
    );
}

/// An empty header file with a meta.bin that still claims blocks.
#[test]
fn empty_header_file_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let config = build_index(tmp.path());
    let out = PathBuf::from(&config.paths.output_dir);
    fs::write(out.join("microscope.bin"), b"").unwrap();

    assert!(
        microscope_memory::reader::MicroscopeReader::open(&config).is_err(),
        "an empty microscope.bin was accepted alongside a meta.bin claiming blocks"
    );
}

/// meta.bin itself cut short of its per-depth table.
#[test]
fn truncated_meta_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let config = build_index(tmp.path());
    let out = PathBuf::from(&config.paths.output_dir);
    let meta = out.join("meta.bin");
    truncate(&meta, 16);

    assert!(
        microscope_memory::reader::MicroscopeReader::open(&config).is_err(),
        "meta.bin truncated mid depth-table was accepted"
    );
}

#[test]
fn meta_with_a_foreign_magic_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let config = build_index(tmp.path());
    let out = PathBuf::from(&config.paths.output_dir);
    let meta = out.join("meta.bin");
    let mut data = fs::read(&meta).unwrap();
    data[0..4].copy_from_slice(b"NOPE");
    fs::write(&meta, &data).unwrap();

    assert!(
        microscope_memory::reader::MicroscopeReader::open(&config).is_err(),
        "meta.bin with an unknown magic was accepted"
    );
}
