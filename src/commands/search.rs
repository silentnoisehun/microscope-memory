//! CLI command handlers for spatial search, text search, and fingerprint operations.
//!
//! Extracted from `main.rs` to reduce the monolithic match block.

use std::path::Path;
use std::time::Instant;

use colored::Colorize;

use microscope_memory::*;
use microscope_memory::config::Config;
use microscope_memory::reader::{print_append_result};

/// Radial search: find blocks within a radius at a given depth.
pub fn radial_search(config: &Config, x: f32, y: f32, z: f32, depth: u8, radius: f32, k: usize) {
    let t0 = Instant::now();
    let reader = crate::open_reader(config);
    println!(
        "{} ({:.2},{:.2},{:.2}) D{} r={:.3}:",
        "RADIAL".cyan().bold(),
        x, y, z, depth, radius
    );

    let result_set = reader.radial_search(config, x, y, z, depth, radius, k);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);

    if let Some(ref primary) = result_set.primary {
        println!("  {}", "PRIMARY:".green().bold());
        if primary.is_main {
            reader.print_result(primary.block_idx, primary.dist_sq);
        } else {
            print_append_result(&appended, primary.block_idx, primary.dist_sq);
        }
    }

    if !result_set.neighbors.is_empty() {
        println!(
            "  {} ({}):",
            "NEIGHBORS".yellow(),
            result_set.neighbors.len()
        );
        for n in &result_set.neighbors {
            if n.is_main {
                let h = reader.header(n.block_idx);
                let text = reader.text(n.block_idx);
                let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                let preview: String =
                    text.chars().take(60).filter(|&c| c != '\n').collect();
                println!(
                    "    {} {} {} w={:.3} {}",
                    format!("D{}", h.depth).cyan(),
                    format!("L2={:.5}", n.dist_sq).yellow(),
                    format!("[{}]", layer).green(),
                    n.weight,
                    preview
                );
            } else {
                print_append_result(&appended, n.block_idx, n.dist_sq);
            }
        }
    }

    println!(
        "\n  {} within radius, {} shown, {:.0} us",
        result_set.total_within_radius,
        result_set.all().len(),
        t0.elapsed().as_micros()
    );

    // Hebbian: record radial activation
    let output_dir = Path::new(&config.paths.output_dir);
    let mut hebb = hebbian::HebbianState::load_or_init(
        output_dir, reader.block_count,
    );
    let activated = result_set.block_indices();
    if !activated.is_empty() {
        let qh = hebbian::query_hash(&format!("radial:{:.3},{:.3},{:.3}", x, y, z));
        hebb.record_activation(&activated, qh);
        let _ = hebb.save(output_dir);
    }
}

/// Manual look: x y z zoom k
pub fn look(config: &Config, x: f32, y: f32, z: f32, zoom: u8, k: usize) {
    let config_clone = config.clone();
    let r = crate::open_reader(config);
    println!(
        "{} ({:.2},{:.2},{:.2}) zoom={}:",
        "MICROSCOPE".cyan().bold(), x, y, z, zoom
    );
    let res = r.look(&config_clone, x, y, z, zoom, k);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (dist, idx, is_main) in res {
        if is_main {
            r.print_result(idx, dist);
        } else {
            print_append_result(&appended, idx, dist);
        }
    }
}

/// 4D soft zoom: x y z zoom k [--gpu]
pub fn soft_look(config: &Config, x: f32, y: f32, z: f32, zoom: u8, k: usize, use_gpu: bool) {
    let r = crate::open_reader(config);
    let use_gpu = use_gpu || config.performance.use_gpu;
    println!(
        "{} 4D ({:.2},{:.2},{:.2}) z={} {}:",
        "MICROSCOPE".cyan().bold(), x, y, z, zoom,
        if use_gpu { "[GPU]" } else { "[CPU]" }
    );

    #[cfg(feature = "gpu")]
    if use_gpu {
        match microscope_memory::gpu::GpuAccelerator::new(&r) {
            Ok(accel) => {
                let res = accel.l2_search_4d(x, y, z, zoom, config.search.zoom_weight, k);
                for (dist, idx) in res {
                    r.print_result(idx, dist);
                }
                return;
            }
            Err(e) => {
                eprintln!("  {} GPU init failed: {}, falling back to CPU", "WARN".yellow(), e);
            }
        }
    }

    #[cfg(not(feature = "gpu"))]
    if use_gpu {
        eprintln!("  {} GPU feature not compiled. Use --features gpu", "WARN".yellow());
    }

    let config_clone = config.clone();
    let res = r.look_soft(&config_clone, x, y, z, zoom, k, config.search.zoom_weight);
    let append_path = Path::new(&config.paths.output_dir).join("append.bin");
    let appended = read_append_log(&append_path);
    for (dist, idx, is_main) in res {
        if is_main {
            r.print_result(idx, dist);
        } else {
            print_append_result(&appended, idx, dist);
        }
    }
}

/// Text search across the index.
pub fn find_text(config: &Config, query: &str, k: usize) {
    let r = crate::open_reader(config);
    println!("{} '{}':", "FIND".cyan().bold(), query);
    let res = r.find_text(query, k);
    if res.is_empty() {
        println!("  (none)");
    }
    for (_d, i) in res {
        r.print_result(i, 0.0);
    }
}

/// Build structural fingerprints and wormhole links.
pub fn fingerprint(config: &Config) {
    let t0 = Instant::now();
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    println!(
        "{} {} blocks...",
        "FINGERPRINT".cyan().bold(), reader.block_count
    );

    let texts: Vec<&str> = (0..reader.block_count).map(|i| reader.text(i)).collect();
    let table = fingerprint::LinkTable::build(&texts);
    table.save(output_dir).expect("save fingerprints");

    let stats = table.stats();
    println!("  Avg entropy:        {:.3}", stats.avg_entropy);
    println!("  Unique hashes:      {}", stats.unique_hashes);
    println!("  Largest cluster:    {}", stats.largest_cluster);
    println!("  Structural links:   {}", stats.link_count);
    println!("  {:.0} ms", t0.elapsed().as_millis());
}

/// Show structural links (wormholes) for a block.
pub fn links(config: &Config, block_index: usize) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let table = fingerprint::LinkTable::load(output_dir);

    match table {
        Some(t) => {
            let links = t.linked_blocks(block_index as u32);
            let h = reader.header(block_index);
            let text = reader.text(block_index);
            let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
            println!(
                "{} Block #{} D{} [{}] {}",
                "LINKS".cyan().bold(), block_index, h.depth, layer,
                safe_truncate(text, 50)
            );

            if links.is_empty() {
                println!("  (no structural links)");
            } else {
                println!("  {} wormholes:", links.len());
                for (target, sim) in &links {
                    let th = reader.header(*target as usize);
                    let tt = reader.text(*target as usize);
                    let tl = LAYER_NAMES.get(th.layer_id as usize).unwrap_or(&"?");
                    println!(
                        "    -> #{} {} {} sim={:.3} {}",
                        target,
                        format!("D{}", th.depth).cyan(),
                        format!("[{}]", tl).green(),
                        sim,
                        safe_truncate(tt, 50)
                    );
                }
            }
        }
        None => {
            println!("  {} fingerprints.idx not found — run 'fingerprint' first", "ERR".red());
        }
    }
}

/// Find structurally similar blocks to a text.
pub fn similar(config: &Config, text: &str, k: usize) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let table = fingerprint::LinkTable::load(output_dir);

    match table {
        Some(t) => {
            let results = t.find_similar(text, k);
            println!(
                "{} '{}' ({} results):",
                "SIMILAR".cyan().bold(),
                safe_truncate(text, 40),
                results.len()
            );
            for (idx, sim) in &results {
                let h = reader.header(*idx as usize);
                let bt = reader.text(*idx as usize);
                let layer = LAYER_NAMES.get(h.layer_id as usize).unwrap_or(&"?");
                println!(
                    "  #{} {} {} sim={:.3} {}",
                    idx,
                    format!("D{}", h.depth).cyan(),
                    format!("[{}]", layer).green(),
                    sim,
                    safe_truncate(bt, 50)
                );
            }
        }
        None => {
            println!("  {} fingerprints.idx not found — run 'fingerprint' first", "ERR".red());
        }
    }
}
