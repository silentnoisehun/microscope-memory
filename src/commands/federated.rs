//! CLI command handlers for federated search and pulse exchange.
//!
//! Extracted from `main.rs` to reduce the monolithic match block.

use std::path::Path;

use colored::Colorize;

use microscope_memory::*;
use microscope_memory::config::Config;

/// Federated recall across multiple indices.
pub fn federated_recall(config: &Config, query: &str, k: usize) {
    let fed = microscope_memory::federation::FederatedSearch::from_config(config)
        .expect("federation config");
    let results = fed.recall(query, k);
    println!(
        "{} '{}' across {} indices:",
        "FEDERATED RECALL".cyan().bold(),
        query,
        config.federation.indices.len()
    );
    if results.is_empty() {
        println!("  (no results)");
    }
    for r in &results {
        println!(
            "  [D{} {} score={:.3} src={}] {}",
            r.depth, r.layer, r.score, r.source_index.cyan(),
            microscope_memory::safe_truncate(&r.text, 80)
        );
    }
    println!("\n  {} results", results.len());
}

/// Exchange resonance pulses across federated indices.
pub fn pulse_exchange(config: &Config) {
    println!(
        "{} across {} indices...",
        "PULSE EXCHANGE".magenta().bold(),
        config.federation.indices.len()
    );
    match microscope_memory::federation::exchange_pulses(config) {
        Ok(count) => println!("  {} pulses exchanged", count),
        Err(e) => eprintln!("  {} {}", "ERR".red(), e),
    }
}

/// Federated text search across multiple indices.
pub fn federated_find(config: &Config, query: &str, k: usize) {
    let fed = microscope_memory::federation::FederatedSearch::from_config(config)
        .expect("federation config");
    let results = fed.find_text(query, k);
    println!(
        "{} '{}' across {} indices:",
        "FEDERATED FIND".cyan().bold(),
        query,
        config.federation.indices.len()
    );
    if results.is_empty() {
        println!("  (no results)");
    }
    for r in &results {
        println!(
            "  [D{} {} src={}] {}",
            r.depth, r.layer, r.source_index.cyan(),
            microscope_memory::safe_truncate(&r.text, 80)
        );
    }
}

/// Exchange thought patterns across federated indices.
pub fn pattern_exchange(config: &Config) {
    println!(
        "{} across {} indices...",
        "PATTERN EXCHANGE".magenta().bold(),
        config.federation.indices.len()
    );
    match microscope_memory::federation::exchange_patterns(config) {
        Ok(count) => println!("  {} patterns exchanged", count),
        Err(e) => eprintln!("  {} {}", "ERR".red(), e),
    }
}
