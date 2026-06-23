//! CLI command handler for `init-demo`.
//!
//! Creates an initial demo dataset in `layers/demo.txt` so that users can
//! run `build` and `serve` right away.

use std::fs;
use std::path::Path;

use colored::Colorize;

use microscope_memory::config::Config;

/// Create a demo dataset at `layers/demo.txt`.
/// Returns an error if the file already exists (unless `force` is true).
pub fn init_demo(config: &Config, force: bool) -> Result<(), String> {
    let layers_dir = Path::new(&config.paths.layers_dir);
    if !layers_dir.exists() {
        fs::create_dir_all(layers_dir).map_err(|e| e.to_string())?;
    }

    let demo_path = layers_dir.join("demo.txt");
    if demo_path.exists() && !force {
        return Err("layers/demo.txt already exists. Use --force to overwrite.".to_string());
    }

    let demo_content = "Microscope Memory: Hierarchical Cognitive Engine\n\nThis is a demo dataset for the Microscope Memory. It uses a 9-layer hierarchical model (D0-D8) to store and recall information.\n\nKey Concepts:\n- Hebbian Learning: Blocks that fire together, wire together.\n- Binary Spine: Zero-JSON, mmap-backed performance.\n- Resonance: Federated synchronization protocol.\n\nHow to use:\n1. Run 'microscope-mem build' to index this file.\n2. Run 'microscope-mem think \"Tell me about Hebbian learning\"' to see it in action.\n";
    let demo_tmp = layers_dir.join("demo.txt.tmp");
    fs::write(&demo_tmp, demo_content).map_err(|e| e.to_string())?;
    fs::rename(&demo_tmp, &demo_path).map_err(|e| e.to_string())?;

    println!("{}", "Demo dataset initialized.".green().bold());
    println!("  -> Created {}", demo_path.display());
    println!("\nNext steps:");
    println!(
        "  1. {} build        # Build the binary index",
        "microscope-mem".cyan()
    );
    println!(
        "  2. {} cognitive-map # Export 3D visualization",
        "microscope-mem".cyan()
    );
    println!(
        "  3. {} serve         # Open 3D viewer in browser",
        "microscope-mem".cyan()
    );

    Ok(())
}
