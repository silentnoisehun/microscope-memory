//! Mermaid Terminal  WebSocket + HTML UI for Microscope Memory.
//!
//! Provides a browser-based terminal interface for interacting with
//! the memory system.

use crate::config::Config;
use colored::Colorize;

/// Run the Mermaid Terminal server.
pub async fn run(_config: Config, _port: u16) -> Result<(), String> {
    eprintln!("  {} Mermaid Terminal not yet implemented", "WARN".yellow());
    eprintln!(
        "  {} Use 'microscope-mem serve' for the HTTP API server",
        "INFO".cyan()
    );
    Ok(())
}
