//! Every `microscope-mem <word>` in the tracked markdown must be a real
//! subcommand.
//!
//! Seven documented commands did not exist. `morph`, `import-chat-gpt`,
//! `sandbox`, `impulse`, `meta` and `code` were all removed in a962ad1, which
//! took 32 command variants with it, and the documentation kept presenting them
//! as things you can type. `remember` never existed under that name either.
//! Someone reading ARCHITECTURE.md would have run `microscope-mem morph --grow
//! "api"` and got `error: unrecognized subcommand`.
//!
//! This walks the markdown, pulls out every command name that follows the
//! binary name, and runs it. A regex alone is not enough, which is the point --
//! the first version of this check found 8 bad commands and missed `remember`,
//! and it reported `cargo`, `dream`, `pattern-exchange` and `store` as missing
//! when all four exist. The only version of this that is worth having asks the
//! binary.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_microscope-mem");

/// Words that legitimately follow the binary name in prose without being
/// subcommands: cargo invocations, and the placeholder names used to mean "any
/// command". Each is here because it was actually found in a checked file, not
/// because it seemed like it might turn up.
const NOT_SUBCOMMANDS: &[&str] = &[
    "cargo", "build", "check", "clippy", "doc", "fmt", "install", "package", "publish", "run",
    "test", "tree", "update", "add", "deny", "help", "version", "bash", "sh",
];

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" || name == "node_modules" || name == ".git" {
            continue;
        }
        if path.is_dir() {
            markdown_files(&path, out);
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}

#[test]
fn documented_commands_all_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    markdown_files(&root, &mut files);
    assert!(
        files.len() > 5,
        "expected to find the docs, found {}",
        files.len()
    );

    let mut missing: Vec<(String, String)> = Vec::new();

    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        // Line by line, and only a line that *starts* with the binary name.
        //
        // Two earlier attempts were wrong in instructive ways. Matching
        // anywhere in the text picked up prose ("this skill calls your
        // Microscope-Memory API") and, after stripping a trailing "ory",
        // compound names like `microscope-memory-backup` -- the hyphen was
        // read as the separator and "backup" came out as a command. A command
        // invocation is a line beginning with the binary name followed by
        // whitespace; anything else is a sentence.
        for line in text.lines() {
            let line = line.trim_start();
            let Some(rest) = line
                .strip_prefix("microscope-memory")
                .or_else(|| line.strip_prefix("microscope-mem"))
            else {
                continue;
            };
            let rest = rest.trim_start();
            if rest.len() == line.len() - line.trim_start().len() && !rest.is_empty() {
                continue; // nothing but the name, or a hyphenated compound
            }

            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                .collect();
            let name = name.trim_end_matches('-').to_string();
            if name.len() < 3 || NOT_SUBCOMMANDS.contains(&name.as_str()) {
                continue;
            }

            // Ask the binary. A doc that says `microscope-mem morph --grow` is
            // only wrong if morph is not a subcommand.
            let out = Command::new(BIN)
                .args([name.as_str(), "--help"])
                .output()
                .expect("run the binary");
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if text.contains("unrecognized subcommand") {
                let rel = file
                    .strip_prefix(&root)
                    .unwrap_or(file)
                    .display()
                    .to_string();
                missing.push((name, rel));
            }
        }
    }

    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "these commands are documented but do not exist:\n{}",
        missing
            .iter()
            .map(|(cmd, file)| format!("  {cmd}  ({file})"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
