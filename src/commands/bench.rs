//! CLI command handlers for `bench`, `stats`, and `gpu-bench`.
//!
//! These functions measure and display performance characteristics of the
//! Microscope Memory engine.

use std::time::Instant;

use colored::Colorize;

use microscope_memory::*;
use microscope_memory::config::Config;

/// Run the benchmark: 10,000 queries per zoom level (D0–D8) plus soft 4D zoom.
pub fn bench(config: &Config, reader: &MicroscopeReader) {
    println!("{}", "Benchmark: 10,000 queries per zoom level".cyan());
    println!("  Mode: SIMD={} Rayon=true", cfg!(target_arch = "x86_64"));

    let mut rng: u64 = 42;
    let mut next_f32 = || -> f32 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 33) as f32 / (u32::MAX as f32) * 0.5
    };

    let iters = 10_000u64;
    let mut total_ns: u64 = 0;

    for zoom in 0..9u8 {
        let t0 = Instant::now();
        let config_clone = config.clone();
        for _ in 0..iters {
            let r = reader.look(&config_clone, next_f32(), next_f32(), next_f32(), zoom, 5);
            std::hint::black_box(&r);
        }
        let ns = t0.elapsed().as_nanos() as u64;
        total_ns += ns;
        let avg = ns / iters;
        let (_s, c) = reader.depth_ranges[zoom as usize];
        let label = if avg < 1000 {
            format!("{} ns", avg)
        } else {
            format!("{:.1} us", avg as f64 / 1000.0)
        };
        println!(
            "  ZOOM {}: {} / query  ({} blocks)",
            zoom,
            label.yellow(),
            c
        );
    }

    println!(
        "\n  {}: {:.0} ns avg",
        "OVERALL".green().bold(),
        total_ns as f64 / (iters * 9) as f64
    );

    println!("\n{}", "4D soft zoom (all blocks):".cyan());
    let t0 = Instant::now();
    let config_clone = config.clone();
    for _ in 0..iters {
        let z = (next_f32() * 10.0) as u8 % 6;
        let r = reader.look_soft(&config_clone, next_f32(), next_f32(), next_f32(), z, 5, 2.0);
        std::hint::black_box(&r);
    }
    let ns = t0.elapsed().as_nanos() / iters as u128;
    println!("  4D: {} ns/query ({} blocks)", ns, reader.block_count);
}

/// Display memory index statistics: block count, header/data sizes, depth distribution.
pub fn stats(reader: &MicroscopeReader) {
    let hdr_size = reader.block_count * HEADER_SIZE;
    let dat_size = reader.data.len();
    println!("{}", "=".repeat(50));
    println!("  {}", "MICROSCOPE MEMORY (pure binary)".cyan().bold());
    println!("{}", "=".repeat(50));
    println!("  Blocks:    {}", reader.block_count);
    println!("  Headers:   {:.1} KB", hdr_size as f64 / 1024.0);
    println!("  Data:      {:.1} KB", dat_size as f64 / 1024.0);
    println!(
        "  Total:     {:.1} KB",
        (hdr_size + dat_size) as f64 / 1024.0
    );
    println!("  Viewport:  {} chars/block", BLOCK_DATA_SIZE);

    let fits = if hdr_size < 32768 {
        "L1d"
    } else if hdr_size < 262144 {
        "L2"
    } else {
        "L3"
    };
    println!("  Cache:     {}", fits.green().bold());

    println!("\n  Depths:");
    for (d, &(_s, c)) in reader.depth_ranges.iter().enumerate() {
        let bar_len = (c as f64 / reader.block_count as f64 * 40.0) as usize;
        println!("    D{}: {:>5}  {}", d, c, "|".repeat(bar_len).cyan());
    }
    println!("{}", "=".repeat(50));
}

/// Run the GPU benchmark (CPU fallback comparison). Requires `--features gpu`.
pub fn gpu_bench(config: &Config) {
    let reader = crate::open_reader(config);
    println!(
        "{} {} blocks",
        "GPU BENCH".cyan().bold(),
        reader.block_count
    );

    let iters = 1000u64;
    let mut rng: u64 = 42;
    let mut next_f32 = || -> f32 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng >> 33) as f32 / (u32::MAX as f32) * 0.5
    };

    let config_clone = config.clone();
    let t0 = Instant::now();
    for _ in 0..iters {
        let z = (next_f32() * 10.0) as u8 % 6;
        let r = reader.look_soft(
            &config_clone,
            next_f32(),
            next_f32(),
            next_f32(),
            z,
            5,
            config.search.zoom_weight,
        );
        std::hint::black_box(&r);
    }
    let cpu_ns = t0.elapsed().as_nanos() / iters as u128;
    println!("  CPU: {} ns/query", cpu_ns);

    #[cfg(feature = "gpu")]
    {
        match microscope_memory::gpu::GpuAccelerator::new(&reader) {
            Ok(accel) => {
                for _ in 0..10 {
                    let z = (next_f32() * 10.0) as u8 % 6;
                    let _ = accel.l2_search_4d(
                        next_f32(),
                        next_f32(),
                        next_f32(),
                        z,
                        config.search.zoom_weight,
                        5,
                    );
                }

                let t0 = Instant::now();
                for _ in 0..iters {
                    let z = (next_f32() * 10.0) as u8 % 6;
                    let r = accel.l2_search_4d(
                        next_f32(),
                        next_f32(),
                        next_f32(),
                        z,
                        config.search.zoom_weight,
                        5,
                    );
                    std::hint::black_box(&r);
                }
                let gpu_ns = t0.elapsed().as_nanos() / iters as u128;
                println!("  GPU: {} ns/query", gpu_ns);

                if gpu_ns > 0 {
                    let speedup = cpu_ns as f64 / gpu_ns as f64;
                    println!("  Speedup: {:.1}x", speedup);
                }
            }
            Err(e) => {
                eprintln!("  {} GPU init failed: {}", "ERR".red(), e);
            }
        }
    }

    #[cfg(not(feature = "gpu"))]
    {
        println!(
            "  {} GPU feature not compiled. Use: cargo build --features gpu",
            "WARN".yellow()
        );
    }
}
