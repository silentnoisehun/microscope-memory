//! MCP stdio latency: first tool call (context open) vs steady state.
//!
//! Spawns the real `microscope-mem mcp` server, sends initialize + N x
//! tools/call memory_find over one stdio session, and times each round trip.
//! This is what an MCP client (Claude, etc.) actually experiences.
//!
//! Futtatas: cargo run --release --example bench_mcp -- <adatkonyvtar>

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

fn frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

fn read_frame(out: &mut impl BufRead) -> Option<serde_json::Value> {
    // Headers until blank line, then exactly Content-Length bytes.
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if out.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok();
        }
    }
    let len = len?;
    let mut buf = vec![0u8; len];
    out.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn main() {
    let data_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));

    // Minimal config pointing at the data dir.
    let cfg_text = format!(
        "[paths]\nlayers_dir = \"{}\"\noutput_dir = \"{}\"\ntemp_dir = \"{}\"\n[embedding]\nprovider = \"mock\"\n",
        data_dir.display(),
        data_dir.display(),
        data_dir.display()
    );
    let cfg_path = std::env::temp_dir().join(format!("mcp_bench_{}.toml", std::process::id()));
    std::fs::write(&cfg_path, cfg_text).unwrap();

    let bin = std::env::var("MCP_BIN").unwrap_or_else(|_| {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("target");
        p.push("release");
        p.push("microscope-mem.exe");
        p.to_string_lossy().to_string()
    });

    let mut child = Command::new(&bin)
        .arg("mcp")
        .env("MICROSCOPE_CONFIG", &cfg_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mcp server");

    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());

    let send = |stdin: &mut std::process::ChildStdin, v: serde_json::Value| {
        use serde_json::json;
        let _ = json!(null);
        stdin.write_all(&frame(&v.to_string())).unwrap();
        stdin.flush().unwrap();
    };

    // initialize + initialized notification.
    send(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    let _ = read_frame(&mut out);
    send(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );

    // N x memory_find in ONE session (shared context lives across them).
    const N: usize = 10;
    let mut times = Vec::with_capacity(N);
    for i in 0..N {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 100 + i,
            "method": "tools/call",
            "params": {"name": "memory_find", "arguments": {"query": "memory", "k": 5}}
        });
        let t = Instant::now();
        send(&mut stdin, req);
        let resp = read_frame(&mut out);
        let dt = t.elapsed();
        assert!(resp.is_some(), "no response to call {}", i);
        times.push(dt);
    }

    drop(stdin);
    let _ = child.wait();

    println!("MCP memory_find over one stdio session ({} calls):", N);
    println!("  1st call (context open + search): {:?}", times[0]);
    let steady: Vec<_> = times[1..].to_vec();
    let sum: std::time::Duration = steady.iter().sum();
    let avg = sum / steady.len() as u32;
    let min = *steady.iter().min().unwrap();
    let max = *steady.iter().max().unwrap();
    println!("  steady min/avg/max: {:?} / {:?} / {:?}", min, avg, max);
    println!(
        "  first-vs-steady: {:.1}x",
        times[0].as_secs_f64() / avg.as_secs_f64()
    );
}
