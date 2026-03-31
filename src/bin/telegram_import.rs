/// telegram_import — Telegram HTML export → D:\Claude Memory\layers\
/// Parsolja a Máté×Ora Telegram chateket és microscope memory layer fájlokba írja.

use std::fs;
use std::path::Path;

const TELEGRAM_DIR: &str = r"C:\Users\mater\Downloads\Telegram Desktop\ChatExport_2026-03-20";
const LAYERS_DIR: &str = r"D:\Claude Memory\layers";
const CHUNK_SIZE: usize = 6; // üzenet / fájl

#[derive(Debug, Clone)]
struct Message {
    date: String,
    sender: String, // "Máté" vagy "Ora"
    text: String,
}

fn main() {
    println!("=== Telegram Import ===");
    println!("Forrás: {}", TELEGRAM_DIR);
    println!("Cél: {}", LAYERS_DIR);

    let files = ["messages.html", "messages2.html"];
    let mut all_messages: Vec<Message> = Vec::new();

    for filename in &files {
        let path = format!("{}\\{}", TELEGRAM_DIR, filename);
        match fs::read_to_string(&path) {
            Ok(html) => {
                let msgs = parse_html(&html);
                println!("{}: {} üzenet kinyerve", filename, msgs.len());
                all_messages.extend(msgs);
            }
            Err(e) => eprintln!("Nem olvasható {}: {}", path, e),
        }
    }

    println!("Összesen: {} üzenet", all_messages.len());

    // Szűrés: üres és parancs üzenetek kihagyása
    let filtered: Vec<Message> = all_messages
        .into_iter()
        .filter(|m| {
            let t = m.text.trim();
            !t.is_empty() && !t.starts_with('/') && t.len() > 2
        })
        .collect();

    println!("Szűrés után: {} üzenet", filtered.len());

    // Chunks → layer fájlok
    let chunks: Vec<&[Message]> = filtered.chunks(CHUNK_SIZE).collect();
    let mut written = 0;

    for (i, chunk) in chunks.iter().enumerate() {
        let filename = format!("{}\\100-telegram-{:04}.md", LAYERS_DIR, i + 1);
        let content = format_chunk(chunk, i + 1, chunks.len());
        match fs::write(&filename, &content) {
            Ok(_) => written += 1,
            Err(e) => eprintln!("Írási hiba {}: {}", filename, e),
        }
    }

    println!("✓ {} layer fájl írva → {}", written, LAYERS_DIR);
    println!("\nKövetkező lépés: cargo run --bin microscope-memory -- build");
}

fn parse_html(html: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut current_sender = String::from("Ora"); // default
    let mut pos = 0;

    while pos < html.len() {
        // Keresünk message div-et
        let msg_start = match find_str(html, pos, "class=\"message default clearfix") {
            Some(p) => p,
            None => break,
        };

        // A következő message div végéig dolgozzuk fel
        let msg_end = find_str(html, msg_start + 10, "class=\"message")
            .unwrap_or(html.len());

        let block = &html[msg_start..msg_end];

        // Sender — ha van from_name div, frissítjük
        if let Some(sender) = extract_div(block, "from_name") {
            let s = sender.trim().to_string();
            current_sender = if s.contains("Silent") || s.contains("SN") {
                "Máté".to_string()
            } else {
                "Ora".to_string()
            };
        }

        // Date
        let date = extract_title_attr(block)
            .map(|d| d[..d.len().min(16)].to_string())
            .unwrap_or_default();

        // Text
        if let Some(raw_text) = extract_div(block, "text") {
            let text = strip_html(raw_text.trim());
            if !text.is_empty() {
                messages.push(Message {
                    date: date.clone(),
                    sender: current_sender.clone(),
                    text,
                });
            }
        }

        pos = msg_end;
    }

    messages
}

fn format_chunk(chunk: &[Message], idx: usize, total: usize) -> String {
    let mut out = format!(
        "# Telegram Konverzáció — {}/{}\n\
         # Máté × Ora valódi üzenetek\n\n",
        idx, total
    );

    for msg in chunk {
        let date = if msg.date.is_empty() {
            String::new()
        } else {
            format!("[{}] ", &msg.date[..msg.date.len().min(16)])
        };
        out.push_str(&format!("{}**{}**: {}\n\n", date, msg.sender, msg.text));
    }

    out
}

// ── HTML helpers ─────────────────────────────────────

fn find_str(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    haystack[from..].find(needle).map(|p| p + from)
}

fn extract_div(block: &str, class: &str) -> Option<String> {
    let search = format!("class=\"{}\"", class);
    let start = find_str(block, 0, &search)?;
    let content_start = find_str(block, start, ">")?  + 1;
    let content_end = find_str(block, content_start, "</div>")?;
    Some(block[content_start..content_end].to_string())
}

fn extract_title_attr(block: &str) -> Option<String> {
    let search = "title=\"";
    let start = find_str(block, 0, search)? + search.len();
    let end = find_str(block, start, "\"")?;
    Some(block[start..end].to_string())
}

fn strip_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '&' if !in_tag => {
                // HTML entitások
                let rest: String = std::iter::once(c)
                    .chain(chars.by_ref().take(6))
                    .collect();
                if rest.starts_with("&amp;") { result.push('&'); }
                else if rest.starts_with("&lt;") { result.push('<'); }
                else if rest.starts_with("&gt;") { result.push('>'); }
                else if rest.starts_with("&quot;") { result.push('"'); }
                else { result.push_str(&rest); }
            }
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Több whitespace → egy szóköz
    let cleaned: String = result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    cleaned
}
