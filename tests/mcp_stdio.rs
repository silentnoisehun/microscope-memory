//! Black-box MCP protocol test: drives the real `microscope-mem mcp` binary over
//! a real stdin/stdout stream.
//!
//! The existing `test_mcp_protocol_compatibility` calls the request handler
//! directly, so it cannot see the framing, the read loop, or how the server
//! answers bad input. This spawns the binary and talks to it the way a client
//! does, which is the only way to test the parts most likely to be wrong: a
//! header that is too long, a `Content-Length` that lies, a frame with no length.
//!
//! Framing, mirrored from `src/mcp.rs`: a line starting with `{` is unframed
//! newline-delimited JSON; anything else is a header block terminated by a blank
//! line and a body of exactly `Content-Length` bytes. The response is written in
//! the same shape the request arrived in.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::{json, Value};

/// Same ceilings the server enforces, so the oversize cases do not depend on a
/// magic number being right twice.
const MAX_MESSAGE_LEN: usize = 16 * 1024 * 1024;
const MAX_HEADER_LEN: usize = 64 * 1024;

/// A scratch config, written where the child can be pointed at it.
fn scratch_config(tmp: &Path) -> PathBuf {
    let output_dir = tmp.join("data");
    let layers_dir = tmp.join("layers");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::create_dir_all(&layers_dir).unwrap();
    std::fs::write(
        layers_dir.join("long_term.txt"),
        "Microscope Memory uses hierarchical indexing.\n\n\
         Memory management in Rust uses ownership and borrowing.\n",
    )
    .unwrap();

    let mut config = microscope_memory::config::Config::default();
    config.paths.layers_dir = layers_dir.to_string_lossy().to_string();
    config.paths.output_dir = output_dir.to_string_lossy().to_string();
    config.paths.temp_dir = tmp.join("tmp").to_string_lossy().to_string();
    config.memory_layers.layers = vec!["long_term".to_string()];
    config.embedding.provider = "mock".to_string();

    let path = tmp.join("config.toml");
    std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
    path
}

fn spawn(config: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_microscope-mem"))
        .arg("mcp")
        .env("MICROSCOPE_CONFIG", config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Kept, not nulled: when this test fails, the server's own diagnostics
        // are the first thing worth reading, and a harness that throws them away
        // makes every failure a guessing game.
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn microscope-mem mcp")
}

/// Feed the whole script in, close stdin, and collect every response.
///
/// The child's stdout is read to the end first and the frames are parsed out of
/// that buffer, rather than streaming. If a case ever fails, the raw bytes are
/// what explains it, and a streaming reader would have already swallowed them.
fn drive(input: &str) -> Vec<Value> {
    let tmp = tempfile::tempdir().expect("temp dir");
    let config = scratch_config(tmp.path());
    let mut child = spawn(&config);
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(input.as_bytes()).unwrap();
        stdin.flush().unwrap();
    }
    // Dropping stdin is what ends the server's read loop.
    drop(child.stdin.take());

    let mut raw = Vec::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_end(&mut raw);
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let status = child.wait().ok().map(|s| s.to_string()).unwrap_or_default();

    let out = parse_frames(&raw);
    if out.is_empty() {
        panic!(
            "no response parsed.\n  exit: {status}\n  stdout ({} bytes): {:?}\n  \
             stderr: {}",
            raw.len(),
            String::from_utf8_lossy(&raw),
            err.trim()
        );
    }
    if !err.trim().is_empty() {
        eprintln!("server said: {}", err.trim());
    }
    out
}

/// Split a byte stream into JSON values, honouring both shapes the server uses.
fn parse_frames(raw: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < raw.len() {
        let nl = match raw[pos..].iter().position(|b| *b == b'\n') {
            Some(i) => pos + i,
            None => break,
        };
        let line = String::from_utf8_lossy(&raw[pos..nl]).trim().to_string();
        pos = nl + 1;
        if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str(&line) {
                out.push(v);
            }
            continue;
        }
        // Header block. The line just consumed is itself a header, so its
        // Content-Length has to be picked up here; the loop below only sees
        // the lines after it.
        let mut len: Option<usize> = None;
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().ok();
            }
        }
        while pos < raw.len() {
            let nl = match raw[pos..].iter().position(|b| *b == b'\n') {
                Some(i) => pos + i,
                None => break,
            };
            let h = String::from_utf8_lossy(&raw[pos..nl]).trim().to_string();
            pos = nl + 1;
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                if k.trim().eq_ignore_ascii_case("content-length") {
                    len = v.trim().parse().ok();
                }
            }
        }
        let Some(len) = len else { break };
        if pos + len > raw.len() {
            break;
        }
        if let Ok(v) = serde_json::from_slice(&raw[pos..pos + len]) {
            out.push(v);
        }
        pos += len;
    }
    out
}

fn framed(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

#[test]
fn initialize_and_ping_answer_over_unframed_stdio() {
    let input = format!(
        "{}\n{}\n",
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
    );
    let out = drive(&input);
    assert_eq!(out.len(), 2, "expected one response per request: {out:?}");
    assert_eq!(out[0]["id"], 1);
    assert_eq!(out[0]["jsonrpc"], "2.0");
    assert!(out[0]["result"]["protocolVersion"].is_string());
    assert!(out[0]["result"]["serverInfo"]["name"].is_string());
    assert_eq!(out[1]["id"], 2);
    assert!(out[1]["result"].is_object());
}

#[test]
fn framed_request_gets_a_framed_response() {
    let body = json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}).to_string();
    let out = drive(&framed(&body));
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0]["id"], 7);
}

#[test]
fn unknown_method_is_method_not_found() {
    let input = format!(
        "{}\n",
        json!({"jsonrpc": "2.0", "id": 3, "method": "nope/nope"})
    );
    let out = drive(&input);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0]["error"]["code"], -32601);
    assert_eq!(out[0]["id"], 3);
}

#[test]
fn malformed_json_is_a_parse_error() {
    let out = drive("{\"jsonrpc\": \"2.0\", \"id\": 1, \n");
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0]["error"]["code"], -32700);
    assert!(out[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Parse error"));
}

/// The point of this one: a peer claiming a body larger than the ceiling must be
/// refused on the header alone. The server checks the length before allocating,
/// so this returns immediately instead of waiting for 100 MB that never comes.
#[test]
fn oversized_content_length_is_refused_from_the_header_alone() {
    let input = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_LEN + 1);
    let out = drive(&input);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0]["error"]["code"], -32700);
    assert!(out[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("exceeds maximum"));
}

#[test]
fn framed_message_without_content_length_is_refused() {
    let out = drive("Content-Type: application/json\r\n\r\n{\"id\":1}");
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0]["error"]["code"], -32700);
    assert!(out[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Content-Length"));
}

#[test]
fn oversized_header_line_is_refused() {
    // Nothing follows the oversized line on purpose. The server answers a read
    // error and then carries on reading, so anything after it would arrive as a
    // second message and this test would be asserting on the wrong thing.
    let padding = "x".repeat(MAX_HEADER_LEN + 16);
    let out = drive(&format!("X-Pad: {}", padding));
    assert!(
        !out.is_empty() && out.len() == 1,
        "expected exactly one response: {out:?}"
    );
    assert_eq!(out[0]["error"]["code"], -32700);
    assert!(out[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("header line exceeds"));
}

#[test]
fn a_notification_gets_no_response() {
    // `notifications/initialized` is a notification: the loop must continue
    // without answering, and the next request still gets its own response.
    let input = format!(
        "{}\n{}\n",
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "ping"}),
    );
    let out = drive(&input);
    assert_eq!(out.len(), 1, "a notification must not be answered: {out:?}");
    assert_eq!(out[0]["id"], 5);
}
