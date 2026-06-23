use microscope_memory::config::Config;
use microscope_memory::consciousness_stream::*;
use std::path::PathBuf;
use std::time::Instant;

fn bench_config() -> Config {
    let mut cfg = Config::default();
    let tmp = std::env::temp_dir().join("microscope_perf");
    std::fs::create_dir_all(&tmp).ok();
    cfg.paths.output_dir = tmp.to_string_lossy().to_string();
    cfg.paths.layers_dir = tmp.to_string_lossy().to_string();
    cfg.paths.temp_dir = tmp.to_string_lossy().to_string();
    cfg
}

#[test]
fn perf_cycle_under_5ms() {
    let cfg = bench_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));

    let s = state.lock().unwrap();
    let n = 1000u64;

    let t0 = Instant::now();
    for _ in 0..n {
        let _ = s.hebbian.activations.len();
        let _ = s.predictive_cache.predictions.len();
        let _ = s.thought_graph.crystallized_count();
        let _ = s.resonance.field.len();
        let _ = s.archetypes.archetypes.len();
        let _ = s.mirror.echoes.len();
    }
    let elapsed = t0.elapsed();
    let per_iter_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[CYCLE]   {} iters in {:.2?} → {:.1} ns/iter ({:.2} µs/iter)",
        n,
        elapsed,
        per_iter_ns,
        per_iter_ns / 1000.0
    );
    assert!(per_iter_ns < 5_000_000.0, "cycle read took > 5ms");
}

#[test]
fn perf_format_under_500us() {
    let cfg = bench_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));

    let n = 5000u32;
    let t0 = Instant::now();
    let mut last_len = 0usize;
    for _ in 0..n {
        let s = ConsciousnessStream::format(&state);
        last_len = s.len();
    }
    let elapsed = t0.elapsed();
    let per_call_us = elapsed.as_micros() as f64 / n as f64;
    println!(
        "\n[FORMAT]  {} calls in {:.2?} → {:.2} µs/call (output {} bytes)",
        n, elapsed, per_call_us, last_len
    );
    assert!(per_call_us < 500.0, "format took > 500 µs");
}

#[test]
fn perf_lock_contention() {
    let cfg = bench_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));

    let n = 1000u32;
    let t0 = Instant::now();
    for _ in 0..n {
        let s = state.lock().unwrap();
        let _ = s.cycle;
        let _ = s.surprise_level;
    }
    let elapsed = t0.elapsed();
    let per_lock_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[LOCK]    {} acquire+read in {:.2?} → {:.0} ns/lock",
        n, elapsed, per_lock_ns
    );
    assert!(per_lock_ns < 100_000.0, "lock acquire took > 100µs");
}
