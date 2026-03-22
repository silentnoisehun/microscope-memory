//! Criterion benchmarks for the Microscope Memory pipeline.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::Path;

/// Create a temporary test environment with config pointing to fixture data.
fn setup_test_env() -> (tempfile::TempDir, microscope_memory::config::Config) {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = manifest_dir.join("tests").join("fixtures");
    let layers_dir = fixtures.join("layers");

    let mut config = microscope_memory::config::Config::default();
    config.paths.layers_dir = layers_dir.to_string_lossy().to_string();
    config.paths.output_dir = output_dir.to_string_lossy().to_string();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string(), "short_term".to_string()];
    config.embedding.provider = "mock".to_string();
    config.embedding.dim = 128;
    config.embedding.max_depth = 4;

    (tmp, config)
}

/// Build the index once and return the temp dir, config, and reader for reuse.
fn setup_built_env() -> (
    tempfile::TempDir,
    microscope_memory::config::Config,
    microscope_memory::MicroscopeReader,
) {
    let (tmp, config) = setup_test_env();
    microscope_memory::build::build(&config, true).expect("build");
    let reader = microscope_memory::MicroscopeReader::open(&config).expect("open reader");
    (tmp, config, reader)
}

// ─── Build pipeline ──────────────────────────────────────

fn bench_build_pipeline(c: &mut Criterion) {
    c.bench_function("build_pipeline", |b| {
        b.iter_with_setup(setup_test_env, |(_tmp, config)| {
            microscope_memory::build::build(black_box(&config), true).expect("build");
        });
    });
}

// ─── Text search (find_text) ─────────────────────────────

fn bench_find_text(c: &mut Criterion) {
    let (_tmp, _config, reader) = setup_built_env();

    let mut group = c.benchmark_group("find_text");
    group.bench_function("existing_term", |b| {
        b.iter(|| reader.find_text(black_box("Rust"), black_box(10)));
    });
    group.bench_function("missing_term", |b| {
        b.iter(|| reader.find_text(black_box("xyznonexistent123"), black_box(10)));
    });
    group.finish();
}

// ─── Spatial lookup (reader.look) ────────────────────────

fn bench_look(c: &mut Criterion) {
    let (_tmp, config, reader) = setup_built_env();

    let mut group = c.benchmark_group("spatial_look");
    for depth in [1u8, 2, 3] {
        group.bench_function(format!("depth_{}", depth), |b| {
            b.iter(|| {
                reader.look(
                    black_box(&config),
                    black_box(0.5),
                    black_box(0.5),
                    black_box(0.5),
                    black_box(depth),
                    black_box(5),
                )
            });
        });
    }
    group.finish();
}

// ─── MQL query (parse + execute) ─────────────────────────

fn bench_mql_query(c: &mut Criterion) {
    let (_tmp, config, reader) = setup_built_env();
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = microscope_memory::read_append_log(&append_path);

    let mut group = c.benchmark_group("mql_query");
    group.bench_function("keyword_search", |b| {
        b.iter(|| {
            let q = microscope_memory::query::parse(black_box("\"Rust\""));
            microscope_memory::query::execute(&q, black_box(&reader), black_box(&appended))
        });
    });
    group.bench_function("depth_filter", |b| {
        b.iter(|| {
            let q = microscope_memory::query::parse(black_box("depth:3 \"memory\""));
            microscope_memory::query::execute(&q, black_box(&reader), black_box(&appended))
        });
    });
    group.finish();
}

// ─── CRC16 computation ──────────────────────────────────

fn bench_crc16(c: &mut Criterion) {
    let short_data = b"hello world";
    let medium_data = vec![0xABu8; 256];
    let long_data = vec![0xCDu8; 4096];

    let mut group = c.benchmark_group("crc16_ccitt");
    group.bench_function("short_11b", |b| {
        b.iter(|| microscope_memory::crc16_ccitt(black_box(short_data)));
    });
    group.bench_function("medium_256b", |b| {
        b.iter(|| microscope_memory::crc16_ccitt(black_box(&medium_data)));
    });
    group.bench_function("long_4096b", |b| {
        b.iter(|| microscope_memory::crc16_ccitt(black_box(&long_data)));
    });
    group.finish();
}

// ─── Content coords ─────────────────────────────────────

fn bench_content_coords(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_coords");
    group.bench_function("hash_only", |b| {
        b.iter(|| {
            microscope_memory::content_coords(
                black_box("Rust memory system benchmark test"),
                black_box("long_term"),
            )
        });
    });
    group.bench_function("blended_w0.5", |b| {
        b.iter(|| {
            microscope_memory::content_coords_blended(
                black_box("Rust memory system benchmark test"),
                black_box("long_term"),
                black_box(0.5),
            )
        });
    });
    group.bench_function("blended_w0.0", |b| {
        b.iter(|| {
            microscope_memory::content_coords_blended(
                black_box("Rust memory system benchmark test"),
                black_box("long_term"),
                black_box(0.0),
            )
        });
    });
    group.finish();
}

// ─── Append log read/write ──────────────────────────────

fn bench_append_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_log");

    group.bench_function("store_and_read_cycle", |b| {
        b.iter_with_setup(
            || {
                let (tmp, config) = setup_test_env();
                microscope_memory::build::build(&config, true).expect("build");
                (tmp, config)
            },
            |(_tmp, config)| {
                microscope_memory::store_memory(
                    black_box(&config),
                    black_box("Benchmark test memory entry about quantum computing"),
                    black_box("long_term"),
                    black_box(5),
                )
                .expect("store");
                let append_path = Path::new(&config.paths.output_dir).join("append.bin");
                let entries = microscope_memory::read_append_log(black_box(&append_path));
                assert!(!entries.is_empty());
            },
        );
    });

    group.bench_function("read_existing", |b| {
        // Setup: build and store one entry, then benchmark reads only
        let (_tmp, config) = setup_test_env();
        microscope_memory::build::build(&config, true).expect("build");
        microscope_memory::store_memory(
            &config,
            "Benchmark pre-stored memory entry",
            "long_term",
            5,
        )
        .expect("store");
        let append_path = Path::new(&config.paths.output_dir).join("append.bin");

        b.iter(|| {
            let entries = microscope_memory::read_append_log(black_box(&append_path));
            assert!(!entries.is_empty());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_build_pipeline,
    bench_find_text,
    bench_look,
    bench_mql_query,
    bench_crc16,
    bench_content_coords,
    bench_append_log,
);
criterion_main!(benches);
