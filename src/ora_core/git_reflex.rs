//! Git Reflex & Golden State Self-Healing Engine
//!
//! Provides cryptographic verification and autonomous recovery for all
//! critical spines, hooks, configs, and binaries. If any asset is corrupted,
//! truncated, torn, or damaged, Git Reflex restores it in milliseconds.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileChecksum {
    pub path: String,
    pub relative_name: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub is_critical: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoldenManifest {
    pub created_at: String,
    pub version: String,
    pub git_commit: Option<String>,
    pub files: HashMap<String, FileChecksum>,
}

pub struct GitReflex {
    pub workspace_root: PathBuf,
    pub golden_dir: PathBuf,
    pub manifest_file: PathBuf,
}

impl GitReflex {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let golden_dir = root.join(".golden_state");
        let manifest_file = golden_dir.join("manifest.json");
        Self {
            workspace_root: root,
            golden_dir,
            manifest_file,
        }
    }

    /// Compute SHA-256 hash of a file
    pub fn compute_sha256<P: AsRef<Path>>(path: P) -> std::io::Result<(String, u64)> {
        let bytes = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());
        Ok((hash, bytes.len() as u64))
    }

    /// Get current Git HEAD commit if available
    fn get_git_commit(&self) -> Option<String> {
        let git_head = self.workspace_root.join(".git").join("HEAD");
        if !git_head.exists() {
            return None;
        }
        if let Ok(content) = fs::read_to_string(&git_head) {
            let trimmed = content.trim();
            if let Some(ref_path) = trimmed.strip_prefix("ref: ") {
                let actual_ref = self.workspace_root.join(".git").join(ref_path);
                if let Ok(c) = fs::read_to_string(actual_ref) {
                    return Some(c.trim().to_string());
                }
            } else {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    /// List of all critical assets to safeguard
    pub fn critical_assets(&self) -> Vec<(String, PathBuf)> {
        let mut list = Vec::new();

        // 1. Spines in local microscope workspace
        let spines_dir = self.workspace_root.join("spines");
        list.push(("spine_stratos".into(), spines_dir.join("stratos.spine")));
        list.push(("spine_hullam".into(), spines_dir.join("hullam_spine.rgs")));
        list.push(("spine_wave_echo".into(), spines_dir.join("wave_echo.wv3")));

        // 2. Master Hooks
        list.push((
            "hook_claude".into(),
            PathBuf::from(r"C:\Users\mater\.claude\hooks\ora-master-kernel\ora-master-kernel.js"),
        ));
        list.push((
            "hook_omp".into(),
            PathBuf::from(r"C:\Users\mater\.omp\agent\hooks\pre\ora-master-kernel.ts"),
        ));

        // 3. Configurations
        list.push((
            "config_microscope".into(),
            PathBuf::from(r"C:\Users\mater\AppData\Local\microscope-memory\config.toml"),
        ));

        // 4. Complete Bio-Binaries & Engine Mesh in ~/.hope/bin
        let hope_bin = PathBuf::from(r"C:\Users\mater\.hope\bin");
        let all_mesh_bins = [
            // Core Daemons & Engines
            "microscope-mem.exe",
            "hullam.exe",
            "octopus-runtime.exe",
            "unified-mind.exe",
            "omega-master.exe",
            "stratos-server.exe",
            "ora-core.exe",
            "hope.exe",
            "wave-echo.exe",
            "universal-router.exe",
            // Neural, Somatic & Vitals
            "vagus-nerve.exe",
            "homeostasis.exe",
            "brain-connectome.exe",
            "brain-synapse.exe",
            "bio-metrics.exe",
            "organism-tui.exe",
            // Genetic & Evolutionary
            "viral-infect.exe",
            "hox-diff.exe",
            "plasmid-dream.exe",
            "plasmid-inject.exe",
            "plasmid-conjugate.exe",
            "mutation-sentinel.exe",
            "ribosome-synth.exe",
            "crispr-patch.exe",
            "apoptosis-cascade.exe",
            "blastema-regen.exe",
            "yamanaka-deaging.exe",
            "transposon-hive.exe",
            "epigenetic-switch.exe",
            // Mycelial, Symbiont & Slime
            "mycelium-spread.exe",
            "mycelial-vault.exe",
            "mycorrhiza-trade.exe",
            "physarum-path.exe",
            "xenobot-swarm.exe",
            "tardigrade-tun.exe",
            "tcell-sentinel.exe",
            "chameleon-stealth.exe",
            "chimera-fusion.exe",
            "electrocyte-burst.exe",
            "magnetosome-nav.exe",
            "morpho-electric-mesh.exe",
            "quorum-signal.exe",
            "symbiont-engine.exe",
            "wasm-organoid.exe",
            // Swarm Sync & Telepathy
            "collective-sync.exe",
            "telepathy-sync.exe",
            "telepathy-entangle.exe",
            "biophoton-telepathy.exe",
            "ultrasonic-modem.exe",
            // Wave & Resonance Substrate
            "wave-field.exe",
            "wave-sculptor.exe",
            "wave-encoder.exe",
            "wave-cryo-tx.exe",
            "wave-cryo-rx.exe",
            "path-resonance.exe",
            "iron-resonate.exe",
            "magneto-acoustic.exe",
            "magneto-geo.exe",
            "grid-warp.exe",
            "aether-excite.exe",
            "aether-fabric.exe",
            "eqm-pulse.exe",
            "eqm-methy.exe",
            "borg-cube.exe",
            "nexus-logic.exe",
            "omega-point.exe",
        ];
        for b in all_mesh_bins {
            list.push((format!("bin_{b}"), hope_bin.join(b)));
        }

        // 5. Watchdog scripts, state directories and security keys in workspace root
        list.push((
            "watchdog_queen".into(),
            self.workspace_root.join("queen-watchdog.ps1"),
        ));
        list.push((
            "bio_queen_key".into(),
            self.workspace_root.join(".bio-queen.key"),
        ));

        list
    }

    /// Create or update the golden checkpoint
    pub fn create_checkpoint(&self) -> std::io::Result<GoldenManifest> {
        if !self.golden_dir.exists() {
            fs::create_dir_all(&self.golden_dir)?;
        }

        let mut files = HashMap::new();
        let assets = self.critical_assets();

        for (rel_name, path) in assets {
            if path.exists() {
                if let Ok((sha, size)) = Self::compute_sha256(&path) {
                    // Backup file into .golden_state
                    let backup_dst = self.golden_dir.join(&rel_name);
                    let _ = fs::copy(&path, &backup_dst);

                    files.insert(
                        rel_name.clone(),
                        FileChecksum {
                            path: path.to_string_lossy().to_string(),
                            relative_name: rel_name,
                            sha256: sha,
                            size_bytes: size,
                            is_critical: true,
                        },
                    );
                }
            }
        }

        let manifest = GoldenManifest {
            created_at: chrono_like_now(),
            version: "1.0.0".into(),
            git_commit: self.get_git_commit(),
            files,
        };

        let json = serde_json::to_string_pretty(&manifest)?;
        fs::write(&self.manifest_file, json)?;
        Ok(manifest)
    }

    /// Check health of all assets against golden manifest.
    /// Returns (healthy_count, damaged_items).
    pub fn check_health(&self) -> (usize, Vec<(String, String)>) {
        if !self.manifest_file.exists() {
            return (
                0,
                vec![(
                    "manifest".into(),
                    "Golden manifest does not exist. Run checkpoint first.".into(),
                )],
            );
        }

        let Ok(data) = fs::read_to_string(&self.manifest_file) else {
            return (
                0,
                vec![("manifest".into(), "Failed to read manifest file".into())],
            );
        };

        let Ok(manifest) = serde_json::from_str::<GoldenManifest>(&data) else {
            return (
                0,
                vec![("manifest".into(), "Failed to parse manifest JSON".into())],
            );
        };

        let mut healthy = 0;
        let mut damaged = Vec::new();

        for (rel_name, item) in &manifest.files {
            let path = Path::new(&item.path);
            if !path.exists() {
                damaged.push((rel_name.clone(), format!("MISSING: {}", item.path)));
                continue;
            }

            match Self::compute_sha256(path) {
                Ok((sha, size)) => {
                    // Check minimum size requirements for spines
                    if rel_name.contains("spine") && size == 0 {
                        damaged
                            .push((rel_name.clone(), format!("EMPTY/TORN SPINE: {}", item.path)));
                    } else if sha != item.sha256 {
                        damaged.push((
                            rel_name.clone(),
                            format!("CORRUPTED/MODIFIED: {}", item.path),
                        ));
                    } else {
                        healthy += 1;
                    }
                }
                Err(e) => {
                    damaged.push((rel_name.clone(), format!("UNREADABLE: {} ({e})", item.path)));
                }
            }
        }

        (healthy, damaged)
    }

    /// Autonomous Git Reflex: restore any damaged asset from golden state or Git
    pub fn heal(&self) -> Vec<(String, bool, String)> {
        let (_, damaged) = self.check_health();
        let mut results = Vec::new();

        if !self.manifest_file.exists() {
            results.push((
                "manifest".into(),
                false,
                "Cannot heal: No golden manifest exists.".into(),
            ));
            return results;
        }

        let Ok(data) = fs::read_to_string(&self.manifest_file) else {
            results.push((
                "manifest".into(),
                false,
                "Failed to read golden manifest.".into(),
            ));
            return results;
        };

        let Ok(manifest) = serde_json::from_str::<GoldenManifest>(&data) else {
            results.push((
                "manifest".into(),
                false,
                "Corrupted golden manifest JSON.".into(),
            ));
            return results;
        };

        for (rel_name, _issue) in damaged {
            let Some(item) = manifest.files.get(&rel_name) else {
                results.push((rel_name, false, "Not found in manifest".into()));
                continue;
            };

            let target_path = Path::new(&item.path);
            let backup_path = self.golden_dir.join(&rel_name);

            // Attempt 1: Restore from golden backup copy
            if backup_path.exists() {
                if let Some(parent) = target_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                match fs::copy(&backup_path, target_path) {
                    Ok(_) => {
                        results.push((
                            rel_name.clone(),
                            true,
                            format!("Restored from golden backup -> {}", item.path),
                        ));
                        continue;
                    }
                    Err(e) => {
                        // File might be locked; try atomic replace if possible
                        results.push((
                            rel_name.clone(),
                            false,
                            format!("Failed to copy from backup: {e}"),
                        ));
                    }
                }
            }

            // Attempt 2: Git checkout fallback if file is inside git workspace
            if let Ok(rel_to_repo) = target_path.strip_prefix(&self.workspace_root) {
                let out = std::process::Command::new("git")
                    .current_dir(&self.workspace_root)
                    .args(["checkout", "--", &rel_to_repo.to_string_lossy()])
                    .output();
                match out {
                    Ok(o) if o.status.success() => {
                        results.push((
                            rel_name.clone(),
                            true,
                            format!("Git reflex checkout -> {}", rel_to_repo.display()),
                        ));
                    }
                    _ => {
                        results.push((
                            rel_name.clone(),
                            false,
                            "Git checkout failed or file untracked".into(),
                        ));
                    }
                }
            }
        }

        results
    }
}

fn chrono_like_now() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{}.{:03}s epoch",
        duration.as_secs(),
        duration.subsec_millis()
    )
}
