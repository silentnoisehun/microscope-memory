//! Exercise the real MCP process rather than the in-process handler.
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(&path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn no_learn_mcp_recall_does_not_change_memory_files() {
    let tmp = tempfile::tempdir().unwrap();
    let layers = tmp.path().join("layers");
    let output = tmp.path().join("output");
    fs::create_dir_all(&layers).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(
        layers.join("long_term.txt"),
        "(imp=8) mcpreadonlymarker the project deadline is Monday.",
    )
    .unwrap();
    let mut config = microscope_memory::config::Config::default();
    config.paths.layers_dir = layers.to_string_lossy().into_owned();
    config.paths.output_dir = output.to_string_lossy().into_owned();
    config.paths.temp_dir = tmp.path().join("tmp").to_string_lossy().into_owned();
    config.memory_layers.layers = vec!["long_term".into()];
    config.embedding.provider = "none".into();
    microscope_memory::build::build(&config, true, false).unwrap();
    let config_file = tmp.path().join("config.toml");
    fs::write(&config_file, toml::to_string(&config).unwrap()).unwrap();
    let before = snapshot(&output);

    let mut child = Command::new(env!("CARGO_BIN_EXE_microscope-mem"))
        .arg("mcp")
        .env("MICROSCOPE_CONFIG", config_file)
        .env("MICROSCOPE_NO_LEARN", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{
            "name":"memory_recall", "arguments":{"query":"mcpreadonlymarker", "k":5}
        }})
    )
    .unwrap();
    drop(child.stdin.take());
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let response = String::from_utf8_lossy(&result.stdout);
    assert!(response.contains("mcpreadonlymarker"), "{response}");
    assert_eq!(snapshot(&output), before, "read-only recall changed files");
}
