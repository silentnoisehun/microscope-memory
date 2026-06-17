//! CLI command handler for `serve`.
//!
//! Serves the PWA chat and 3D viewer via a simple TCP/HTTP server.

use std::fs;
use std::io::{BufRead, Write};
use std::net::TcpListener;

use colored::Colorize;

/// Start a simple TCP/HTTP file server that serves the PWA chat interface,
/// the 3D viewer (viewer.html), and the cognitive_map.bin data file.
pub fn serve_viewer(port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("  {} Cannot bind to {}: {}", "ERROR:".red(), addr, e);
            return;
        }
    };

    println!("{} http://{}", "SERVE".cyan().bold(), addr);
    println!(
        "  Open your browser: {}",
        format!("http://localhost:{}/viewer.html", port).green()
    );
    println!(
        "  PWA Chat:    {}",
        format!("http://localhost:{}/chat.html", port).green()
    );
    println!("  Press Ctrl+C to stop.\n");

    let pwa_path = std::env::current_dir().unwrap().join("pwa");
    let html_path = std::env::current_dir().unwrap().join("viewer.html");
    let bin_path = std::env::current_dir().unwrap().join("cognitive_map.bin");

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut reader = std::io::BufReader::new(&stream);
        let mut request_line = String::new();
        let _ = reader.read_line(&mut request_line);

        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string();

        let (status, content_type, body): (&str, &str, Vec<u8>) =
            if path == "/viewer.html" {
                match fs::read(&html_path) {
                    Ok(b) => ("200 OK", "text/html; charset=utf-8", b),
                    Err(_) => (
                        "404 Not Found",
                        "text/plain",
                        b"viewer.html not found. Run 'cognitive-map' first.".to_vec(),
                    ),
                }
            } else if path == "/" || path == "/chat.html" || path == "/app.js" || path == "/styles.css" || path == "/manifest.json" || path == "/service-worker.js" || path == "/icon.svg" {
                        let file_name = if path == "/" { "chat.html" } else { &path[1..] };
                        let file_path = pwa_path.join(file_name);
                        let ext = file_name.rsplit('.').next().unwrap_or("");
                        let ct = match ext {
                            "html" => "text/html; charset=utf-8",
                            "js" => "application/javascript",
                            "css" => "text/css",
                            "json" => "application/json",
                            "svg" => "image/svg+xml",
                            _ => "text/plain",
                        };
                        match fs::read(&file_path) {
                            Ok(b) => ("200 OK", ct, b),
                            Err(_) => ("404 Not Found", "text/plain", format!("{} not found", file_name).into_bytes().to_vec()),
                        }
                    } else if path == "/cognitive_map.bin" {
                match fs::read(&bin_path) {
                    Ok(b) => ("200 OK", "application/octet-stream", b),
                    Err(_) => (
                        "404 Not Found",
                        "text/plain",
                        b"cognitive_map.bin not found. Run 'cognitive-map' first.".to_vec(),
                    ),
                }
            } else {
                ("404 Not Found", "text/plain", b"Not found".to_vec())
            };

        let header = format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n", status, content_type, body.len());
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    }
}
