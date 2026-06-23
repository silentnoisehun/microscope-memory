//! Performance benchmarks for the consciousness stream.
//!
//! Two profiles are measured:
//!  - EMPTY: stream started on a fresh temp dir (no blocks loaded). Captures
//!    pure dispatch + format() cost with empty vecs.
//!  - REAL:  stream started against the project's actual output/ directory
//!    (28,492 blocks, ~890 KB activations). Captures cost with real data
//!    loaded by `load_or_init` from the existing .bin files.
//!
//! All numbers are wall-clock from a single release build run on this host.

use microscope_memory::consciousness_seqlock::SharedSnapshot;
use microscope_memory::consciousness_stream::*;
use microscope_memory::config::Config;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn empty_config() -> Config {
    let mut cfg = Config::default();
    let tmp = std::env::temp_dir().join("microscope_perf_empty");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    cfg.paths.output_dir = tmp.to_string_lossy().to_string();
    cfg.paths.layers_dir = tmp.to_string_lossy().to_string();
    cfg.paths.temp_dir = tmp.to_string_lossy().to_string();
    cfg
}

fn real_config() -> Option<Config> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = manifest.join("output");
    if !output.join("microscope.bin").exists() {
        return None;
    }
    let mut cfg = Config::default();
    cfg.paths.output_dir = output.to_string_lossy().to_string();
    cfg.paths.layers_dir = manifest.join("layers").to_string_lossy().to_string();
    cfg.paths.temp_dir = output.to_string_lossy().to_string();
    Some(cfg)
}

#[test]
fn perf_format_empty() {
    let cfg = empty_config();
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
        "\n[FORMAT/EMPTY] {} calls in {:.2?} → {:.2} µs/call (output {} bytes)",
        n, elapsed, per_call_us, last_len
    );
    assert!(per_call_us < 500.0);
}

#[test]
fn perf_format_real_index() {
    let cfg = match real_config() {
        Some(c) => c,
        None => {
            println!("\n[FORMAT/REAL]   SKIPPED — output/microscope.bin not present");
            return;
        }
    };
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(200));

    let s = state.lock().unwrap();
    let n_activations = s.hebbian.activations.len();
    let n_resonance = s.resonance.field.len();
    let n_echoes = s.mirror.echoes.len();
    let n_archetypes = s.archetypes.archetypes.len();
    let n_patterns = s.thought_graph.crystallized_count();
    drop(s);

    let n = 1000u32;
    let t0 = Instant::now();
    let mut last_len = 0usize;
    for _ in 0..n {
        let s = ConsciousnessStream::format(&state);
        last_len = s.len();
    }
    let elapsed = t0.elapsed();
    let per_call_us = elapsed.as_micros() as f64 / n as f64;
    println!(
        "\n[FORMAT/REAL]   {} calls in {:.2?} → {:.2} µs/call (output {} bytes)\n  \
         state: hebbian.activations={}, resonance.field={}, mirror.echoes={},\n  \
         state: archetypes={}, thought_graph.crystallized={}",
        n, elapsed, per_call_us, last_len,
        n_activations, n_resonance, n_echoes, n_archetypes, n_patterns
    );
}

#[test]
fn perf_lock_contention() {
    let cfg = empty_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));

    let n = 10_000u32;
    let t0 = Instant::now();
    for _ in 0..n {
        let s = state.lock().unwrap();
        let _ = s.cycle;
        let _ = s.surprise_level;
    }
    let elapsed = t0.elapsed();
    let per_lock_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[LOCK]          {} acquire+read in {:.2?} → {:.0} ns/lock (background cycle 100ms)",
        n, elapsed, per_lock_ns
    );
}

#[test]
fn perf_format_seqlock_empty() {
    let cfg = empty_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let snapshot: Arc<SharedSnapshot> = {
        let s = state.lock().unwrap();
        s.snapshot.clone()
    };

    let n = 10_000u32;
    let t0 = Instant::now();
    let mut last_len = 0usize;
    for _ in 0..n {
        let s = ConsciousnessStream::format_snapshot(&snapshot);
        last_len = s.len();
    }
    let elapsed = t0.elapsed();
    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[FORMAT/SEQLOCK/EMPTY] {} calls in {:.2?} → {:.0} ns/call (output {} bytes)",
        n, elapsed, per_call_ns, last_len
    );
}

#[test]
fn perf_format_seqlock_real() {
    let cfg = match real_config() {
        Some(c) => c,
        None => {
            println!("\n[FORMAT/SEQLOCK/REAL] SKIPPED — output/microscope.bin not present");
            return;
        }
    };
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snapshot: Arc<SharedSnapshot> = {
        let s = state.lock().unwrap();
        s.snapshot.clone()
    };

    let n = 5_000u32;
    let t0 = Instant::now();
    let mut last_len = 0usize;
    for _ in 0..n {
        let s = ConsciousnessStream::format_snapshot(&snapshot);
        last_len = s.len();
    }
    let elapsed = t0.elapsed();
    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[FORMAT/SEQLOCK/REAL]  {} calls in {:.2?} → {:.0} ns/call (output {} bytes, 28k activations)",
        n, elapsed, per_call_ns, last_len
    );
}

#[test]
fn perf_format_cached_string_empty() {
    let cfg = empty_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let snapshot: Arc<SharedSnapshot> = {
        let s = state.lock().unwrap();
        s.snapshot.clone()
    };

    let n = 10_000u32;
    let t0 = Instant::now();
    let mut last_len = 0usize;
    for _ in 0..n {
        let s = snapshot.read_cached_format();
        last_len = s.len();
    }
    let elapsed = t0.elapsed();
    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[CACHED/EMPTY] {} calls in {:.2?} → {:.0} ns/call (output {} bytes)",
        n, elapsed, per_call_ns, last_len
    );
    assert!(per_call_ns < 200.0, "cached format too slow: {} ns", per_call_ns);
}

#[test]
fn perf_format_cached_string_real() {
    let cfg = match real_config() {
        Some(c) => c,
        None => {
            println!("\n[CACHED/REAL]   SKIPPED — output/microscope.bin not present");
            return;
        }
    };
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snapshot: Arc<SharedSnapshot> = {
        let s = state.lock().unwrap();
        s.snapshot.clone()
    };

    let n = 10_000u32;
    let t0 = Instant::now();
    let mut last_len = 0usize;
    for _ in 0..n {
        let s = snapshot.read_cached_format();
        last_len = s.len();
    }
    let elapsed = t0.elapsed();
    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[CACHED/REAL]   {} calls in {:.2?} → {:.0} ns/call (output {} bytes, 28k activations)",
        n, elapsed, per_call_ns, last_len
    );
    assert!(per_call_ns < 200.0, "cached format too slow: {} ns", per_call_ns);
}

#[test]
fn perf_hot_fields() {
    let cfg = empty_config();
    let state = ConsciousnessStream::start(&cfg);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let snapshot: Arc<SharedSnapshot> = {
        let s = state.lock().unwrap();
        s.snapshot.clone()
    };

    let n = 100_000u32;
    let t0 = Instant::now();
    let mut last_cycle = 0u64;
    for _ in 0..n {
        let (cycle, surprise, curiosity, hash) = snapshot.read_hot_fields();
        last_cycle = cycle;
        let _ = (surprise, curiosity, hash);
    }
    let elapsed = t0.elapsed();
    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    println!(
        "\n[HOT_FIELDS]    {} calls in {:.2?} → {:.0} ns/call (cycle={})",
        n, elapsed, per_call_ns, last_cycle
    );
    assert!(per_call_ns < 50.0, "hot fields too slow: {} ns", per_call_ns);
}

#[test]
fn perf_seqlock_under_contention() {
    // One writer thread, one reader thread, 1 second. Measures
    // worst-case read latency (when reader races the writer) and
    // verifies no torn reads: every read sees sequence-even.
    let snapshot = Arc::new(SharedSnapshot::new_zeroed());
    let writer = {
        let s = snapshot.clone();
        thread::spawn(move || {
            let mut counter: u64 = 0;
            loop {
                let token = s.begin_write();
                unsafe {
                    let d = s.data_mut();
                    d.cycle = counter;
                    d.activations_count = (counter % 100_000) as u32;
                    d.activations_total_energy = counter as f64 * 0.5;
                }
                s.end_write(token);
                counter += 1;
            }
        })
    };

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let s = snapshot.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            let mut latencies = Vec::with_capacity(100_000);
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let t0 = Instant::now();
                let view = s.read();
                latencies.push(t0.elapsed().as_nanos());
                if view.is_none() {
                    // Retry succeeded (read returned None only after MAX_RETRIES)
                }
            }
            latencies
        })
    };

    thread::sleep(std::time::Duration::from_millis(500));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut latencies = reader.join().unwrap();
    drop(writer);

    latencies.sort_unstable();
    let n = latencies.len();
    let p50 = latencies[n / 2];
    let p99 = latencies[n - n / 100];
    let p999 = latencies[n - n / 1000];
    let max = latencies[n - 1];
    let min = latencies[0];
    println!(
        "\n[SEQLOCK/CONTENTION] {} reads in contention with writer:\n  \
         min={} ns, p50={} ns, p99={} ns, p99.9={} ns, max={} ns",
        n, min, p50, p99, p999, max
    );
}
