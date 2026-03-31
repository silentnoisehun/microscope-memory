//! File watcher — auto-index markdown/text files from a directory.
//!
//! Polls a directory for changes and automatically stores new/modified files
//! into Microscope Memory. No external dependencies (no `notify` crate).
//!
//! Usage:
//!   microscope-mem watch ~/notes
//!   microscope-mem watch ~/notes --interval 5

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::{content_coords_blended, store_memory};

// ─── Constants ──────────────────────────────────────

const SUPPORTED_EXTENSIONS: &[&str] = &[
    ".md", ".txt", ".markdown", ".org", ".rst", ".adoc", ".tex", ".log", ".csv",
];

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".obsidian",
    ".trash",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
];

const MAX_FILE_SIZE: u64 = 1_048_576; // 1 MB
const BLOCK_CHUNK_SIZE: usize = 200; // chars per memory block

// ─── FileState ──────────────────────────────────────

#[derive(Clone, Debug)]
struct FileState {
    modified: u64, // ms since epoch
    size: u64,
    stored: bool,
}

// ─── Public API ─────────────────────────────────────

/// Watch a directory for changes, storing new/modified files into memory.
/// Runs indefinitely with the given poll interval.
pub fn watch_directory(config: &Config, dir: &str, interval_secs: u64) {
    let root = Path::new(dir).canonicalize().unwrap_or_else(|_| PathBuf::from(dir));

    if !root.is_dir() {
        eprintln!("  ERROR: '{}' is not a directory", root.display());
        return;
    }

    println!(
        "  WATCH: monitoring '{}' every {}s",
        root.display(),
        interval_secs
    );
    println!("  Extensions: {}", SUPPORTED_EXTENSIONS.join(", "));
    println!("  Press Ctrl+C to stop.\n");

    // Initial scan
    let initial = scan_directory(&root);
    let mut new_count = 0;
    for (path, _state) in &initial {
        if store_file(config, path, &root) {
            new_count += 1;
        }
    }
    let mut known = initial;
    println!(
        "  INIT: {} files indexed, {} stored\n",
        known.len(),
        new_count
    );

    // Poll loop
    loop {
        thread::sleep(Duration::from_secs(interval_secs));

        let current = scan_directory(&root);
        let mut changes = 0;

        for (path, state) in &current {
            let is_new = match known.get(path) {
                None => true,
                Some(old) => old.modified != state.modified || old.size != state.size,
            };

            if is_new {
                if store_file(config, path, &root) {
                    changes += 1;
                }
            }
        }

        // Detect deletions (optional logging)
        for (path, _) in &known {
            if !current.contains_key(path) {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                println!("  DELETED: {}", rel.display());
            }
        }

        if changes > 0 {
            println!("  SYNC: {} file(s) updated\n", changes);
        }

        known = current;
    }
}

/// One-shot scan: index all supported files in a directory.
pub fn scan_and_store(config: &Config, dir: &str) -> usize {
    let root = Path::new(dir).canonicalize().unwrap_or_else(|_| PathBuf::from(dir));

    if !root.is_dir() {
        eprintln!("  ERROR: '{}' is not a directory", root.display());
        return 0;
    }

    let files = scan_directory(&root);
    let mut stored = 0;

    for (path, _) in &files {
        if store_file(config, path, &root) {
            stored += 1;
        }
    }

    println!("  SCAN: {} files found, {} stored", files.len(), stored);
    stored
}

// ─── Internal ───────────────────────────────────────

fn scan_directory(root: &Path) -> HashMap<PathBuf, FileState> {
    let mut files = HashMap::new();
    scan_recursive(root, root, &mut files);
    files
}

fn scan_recursive(root: &Path, dir: &Path, files: &mut HashMap<PathBuf, FileState>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if SKIP_DIRS.iter().any(|&s| name_str == s) || name_str.starts_with('.') {
                continue;
            }
            scan_recursive(root, &path, files);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e.to_lowercase()));

            if let Some(ext) = ext {
                if !SUPPORTED_EXTENSIONS.iter().any(|&s| s == ext) {
                    continue;
                }
            } else {
                continue;
            }

            if let Ok(meta) = fs::metadata(&path) {
                if meta.len() > MAX_FILE_SIZE {
                    continue;
                }

                let modified = meta
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                files.insert(
                    path,
                    FileState {
                        modified,
                        size: meta.len(),
                        stored: false,
                    },
                );
            }
        }
    }
}

fn store_file(config: &Config, path: &Path, root: &Path) -> bool {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    if content.trim().is_empty() {
        return false;
    }

    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();

    // Split content into chunks for storage
    let chunks = chunk_text(&content, BLOCK_CHUNK_SIZE);

    for (i, chunk) in chunks.iter().enumerate() {
        let text = if chunks.len() == 1 {
            format!("[{}] {}", rel_str, chunk)
        } else {
            format!("[{} {}/{}] {}", rel_str, i + 1, chunks.len(), chunk)
        };

        if let Err(e) = store_memory(config, &text, "long_term", 5) {
            eprintln!("  ERROR storing {}: {}", rel_str, e);
            return false;
        }
    }

    println!("  STORED: {} ({} chunks)", rel_str, chunks.len());
    true
}

fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() && current.is_empty() {
            continue;
        }

        if current.len() + line.len() + 1 > max_chars && !current.is_empty() {
            chunks.push(current.clone());
            current.clear();
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        chunks.push(text.chars().take(max_chars).collect());
    }

    chunks
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_short() {
        let chunks = chunk_text("hello world", 200);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn test_chunk_text_splits() {
        let text = (0..50).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");
        let chunks = chunk_text(&text, 50);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 60); // some slack for word boundaries
        }
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("", 200);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_supported_extensions() {
        assert!(SUPPORTED_EXTENSIONS.contains(&".md"));
        assert!(SUPPORTED_EXTENSIONS.contains(&".txt"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&".exe"));
    }
}
