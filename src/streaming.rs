use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::Arc;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::{MicroscopeReader, store_memory, layer_to_id, auto_zoom, LAYER_NAMES};

#[derive(Serialize, Deserialize, Debug)]
struct UpdateRequest {
    text: String,
    layer: String,
    #[serde(default = "default_importance")]
    importance: u8,
}

fn default_importance() -> u8 { 5 }

#[derive(Serialize, Deserialize, Debug)]
struct SearchRequest {
    query: String,
    #[serde(default = "default_k")]
    k: usize,
}

fn default_k() -> usize { 5 }

/// Start the Microscope Endpoint Server
/// Supports basic HTTP-like JSON endpoints over TCP
pub fn start_endpoint_server(config: Config) {
    let addr = format!("{}:{}", config.logging.level.replace("info", "0.0.0.0"), 6060); // Simple trick or just 0.0.0.0:6060
    let listener = TcpListener::bind("0.0.0.0:6060").expect("Could not bind to port 6060");
    println!("🔬 {} on 0.0.0.0:6060", "ENDPOINT SERVER ACTIVE".cyan().bold());

    let config = Arc::new(config);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cfg = Arc::clone(&config);
                std::thread::spawn(move || {
                    handle_client(stream, &cfg);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
}

fn handle_client(mut stream: TcpStream, config: &Config) {
    let mut buffer = [0; 4096];
    if let Ok(n) = stream.read(&mut buffer) {
        let request = String::from_utf8_lossy(&buffer[..n]);
        
        // Very basic HTTP routing
        if request.starts_with("POST /store") {
            handle_store(stream, &request, config);
        } else if request.starts_with("GET /recall") || request.starts_with("POST /recall") {
            handle_recall(stream, &request, config);
        } else if request.starts_with("GET /stats") {
            handle_stats(stream, config);
        } else {
            let response = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
        }
    }
}

fn handle_store(mut stream: TcpStream, request: &str, config: &Config) {
    if let Some(json_start) = request.find("\r\n\r\n") {
        let body = &request[json_start + 4..];
        if let Ok(req) = serde_json::from_str::<UpdateRequest>(body) {
            store_memory(config, &req.text, &req.layer, req.importance);
            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"stored\"}";
            let _ = stream.write_all(response.as_bytes());
            return;
        }
    }
    let response = "HTTP/1.1 400 BAD REQUEST\r\n\r\n";
    let _ = stream.write_all(response.as_bytes());
}

fn handle_recall(mut stream: TcpStream, request: &str, config: &Config) {
    let body = if request.starts_with("POST") {
        request.find("\r\n\r\n").map(|i| &request[i+4..]).unwrap_or("")
    } else {
        // Simple mock for query params
        ""
    };

    if let Ok(req) = serde_json::from_str::<SearchRequest>(body) {
        let reader = MicroscopeReader::open(config);
        let (zoom, _) = auto_zoom(&req.query);
        let (x, y, z) = crate::content_coords(&req.query, "long_term"); // Placeholder coord
        
        let results = reader.look(config, x, y, z, zoom, req.k);
        let mut out = Vec::new();
        for (dist, idx, is_main) in results {
            let text = if is_main {
                reader.text(idx).to_string()
            } else {
                let append_path = Path::new(&config.paths.output_dir).join("append.bin");
                let appended = crate::read_append_log(&append_path);
                appended.get(idx - 1_000_000).map(|e| e.text.clone()).unwrap_or_default()
            };
            out.push(serde_json::json!({
                "text": text,
                "distance": dist,
                "is_main": is_main
            }));
        }
        
        let json_res = serde_json::to_string(&out).unwrap_or_default();
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", json_res.len(), json_res);
        let _ = stream.write_all(response.as_bytes());
    } else {
        let response = "HTTP/1.1 400 BAD REQUEST\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
    }
}

fn handle_stats(mut stream: TcpStream, config: &Config) {
    let reader = MicroscopeReader::open(config);
    let stats = serde_json::json!({
        "block_count": reader.block_count,
        "layers": reader.depth_ranges.iter().enumerate().map(|(i, &(_, c))| (format!("D{}", i), c)).collect::<std::collections::HashMap<_, _>>()
    });
    let ns = serde_json::to_string(&stats).unwrap();
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", ns.len(), ns);
    let _ = stream.write_all(response.as_bytes());
}

use colored::Colorize;