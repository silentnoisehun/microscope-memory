//! Liora Memory Import — seed fájlokból Microscope Memory-ba tölt
//!
//! Használat:
//!   liora_import.exe                          -- alapértelmezett seed mappa
//!   liora_import.exe --seed memory/seed       -- egyéni seed mappa
//!   liora_import.exe --file seed.txt          -- egyetlen fájl
//!   liora_import.exe --rebuild                -- import után hot rebuild
//!
//! A seed fájlok szöveges fájlok, soronként egy emlék.
//! Fájlnév konvenció: seed_[layer]_[leírás].txt
//!   pl: seed_liora_persona_oath.txt → "liora_persona" rétegbe
//!       seed_long_term_history.txt  → "long_term" rétegbe

use std::fs;
use std::path::Path;

const DEFAULT_SEED_DIR: &str = "memory/seed";
const DEFAULT_HOST: &str = "http://127.0.0.1:6060";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let mut seed_dir = DEFAULT_SEED_DIR.to_string();
    let mut single_file: Option<String> = None;
    let mut do_rebuild = false;
    let mut host = DEFAULT_HOST.to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => { i += 1; seed_dir = args[i].clone(); }
            "--file" => { i += 1; single_file = Some(args[i].clone()); }
            "--rebuild" => { do_rebuild = true; }
            "--host" => { i += 1; host = args[i].clone(); }
            _ => {}
        }
        i += 1;
    }

    println!("Liora Memory Import");
    println!("  Host: {}", host);

    let mut total = 0usize;

    if let Some(file) = single_file {
        total += import_file(&file, &host)?;
    } else {
        let dir = Path::new(&seed_dir);
        if !dir.exists() {
            eprintln!("Seed mappa nem található: {}", seed_dir);
            eprintln!("Hozd létre és tegyél bele .txt fájlokat.");
            std::process::exit(1);
        }

        let mut files: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "txt").unwrap_or(false))
            .collect();
        files.sort_by_key(|e| e.file_name());

        if files.is_empty() {
            println!("  Nincs seed fájl a {} mappában.", seed_dir);
            return Ok(());
        }

        for entry in &files {
            let path = entry.path();
            total += import_file(path.to_str().unwrap_or(""), &host)?;
        }
    }

    println!("\n  Összesen {} emlék importálva.", total);

    if do_rebuild {
        println!("\n  Hot rebuild...");
        match http_post(&format!("{}/rebuild", host), "{}") {
            Ok(resp) => println!("  {}", resp),
            Err(e) => eprintln!("  Rebuild hiba: {}", e),
        }
    }

    Ok(())
}

fn import_file(path: &str, host: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let filename = Path::new(path).file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Réteg a fájlnévből: seed_[layer]_... → layer
    let layer = extract_layer(filename);
    let importance = if filename.contains("oath") || filename.contains("persona") { 9 } else { 5 };

    println!("  {} → layer:{} importance:{}", filename, layer, importance);

    let mut count = 0;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let body = format!(
            r#"{{"text":"{}","layer":"{}","importance":{}}}"#,
            line.replace('\\', "\\\\").replace('"', "\\\""),
            layer,
            importance
        );

        match http_post(&format!("{}/store", host), &body) {
            Ok(_) => count += 1,
            Err(e) => eprintln!("    Hiba: {} — {}", line.chars().take(50).collect::<String>(), e),
        }
    }

    println!("    {} emlék importálva", count);
    Ok(count)
}

fn extract_layer(filename: &str) -> String {
    // seed_liora_persona_oath → liora_persona
    // seed_long_term_history → long_term
    let parts: Vec<&str> = filename.split('_').collect();
    if parts.len() >= 3 && parts[0] == "seed" {
        // Próbáljuk megtalálni az ismert rétegeket
        let known = ["long_term", "short_term", "emotional", "associative",
                      "relational", "reflections", "liora_persona"];
        for layer in &known {
            if filename.contains(layer) {
                return layer.to_string();
            }
        }
        // Fallback: seed_ utáni rész az utolsó _ előtt
        parts[1..parts.len()-1].join("_")
    } else {
        "long_term".to_string()
    }
}

fn http_post(url: &str, body: &str) -> Result<String, String> {
    // Minimal HTTP POST without external dependencies
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let url_parts: Vec<&str> = url.strip_prefix("http://").unwrap_or(url).splitn(2, '/').collect();
    let host_port = url_parts[0];
    let path = if url_parts.len() > 1 { format!("/{}", url_parts[1]) } else { "/".to_string() };

    let mut stream = TcpStream::connect(host_port)
        .map_err(|e| format!("connect: {}", e))?;

    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, host_port, body.len(), body
    );

    stream.write_all(request.as_bytes()).map_err(|e| format!("write: {}", e))?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|e| format!("read: {}", e))?;

    // Extract body from HTTP response
    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos+4..].to_string())
    } else {
        Ok(response)
    }
}
