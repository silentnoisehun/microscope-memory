//! CLI command handlers for visualization commands.
//!
//! Extracted from `main.rs` to reduce the monolithic match block.

use std::fs;
use std::path::Path;

use colored::Colorize;

use microscope_memory::*;
use microscope_memory::config::Config;

/// Export 3D visualization snapshot (Binary).
pub fn viz(config: &Config, output: &str) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let mirror = mirror::MirrorState::load_or_init(output_dir);
    let _resonance = resonance::ResonanceState::load_or_init(output_dir);
    let archetypes = archetype::ArchetypeState::load_or_init(output_dir);
    let thought_graph = thought_graph::ThoughtGraphState::load_or_init(output_dir);

    let dest = Path::new(output);
    microscope_memory::viz::export_to_file(
        output_dir, &reader, &hebb, &mirror, &thought_graph, dest,
    )
    .expect("export viz");

    let hebb_stats = hebb.stats();
    let arc_stats = archetypes.stats();
    println!(
        "{} {} blocks, {} edges, {} archetypes -> {}",
        "VIZ".cyan().bold(),
        reader.block_count,
        hebb_stats.coactivation_pairs,
        arc_stats.archetype_count,
        output
    );
}

/// Export binary density map for fast rendering.
pub fn density(config: &Config, output: &str, grid: u16) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);

    let headers: Vec<(f32, f32, f32)> = (0..reader.block_count)
        .map(|i| {
            let h = reader.header(i);
            (h.x, h.y, h.z)
        })
        .collect();

    let data = microscope_memory::viz::export_density_map(&hebb, &headers, grid);
    fs::write(output, &data).expect("write density map");
    println!(
        "{} {}×{} grid ({} bytes) -> {}",
        "DENSITY".cyan().bold(), grid, grid, data.len(), output
    );
}

/// Export binary cognitive map snapshot for Three.js viewer.
pub fn cognitive_map(config: &Config, output: &str) {
    let reader = crate::open_reader(config);
    let output_dir = Path::new(&config.paths.output_dir);
    let hebb = hebbian::HebbianState::load_or_init(output_dir, reader.block_count);
    let mirror = mirror::MirrorState::load_or_init(output_dir);
    let thought_graph = thought_graph::ThoughtGraphState::load_or_init(output_dir);
    let dest = Path::new(output);
    microscope_memory::viz::export_to_file(output_dir, &reader, &hebb, &mirror, &thought_graph, dest)
        .expect("export cognitive map");
    println!(
        "{} {} blocks -> {}",
        "COGNITIVE MAP".cyan().bold(),
        reader.block_count,
        output
    );
}
