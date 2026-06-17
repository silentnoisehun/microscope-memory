//! ChatGPT Export Importer — Microscope Memory számára
//!
//! Képes:
//! - ChatGPT konverzációs export JSON feldolgozása
//! - Beszélgetések betöltése Microscope Memory rétegekbe
//! - Érzelmi kontextus és asszociációk építése
//! - Személyazonosítás (user vs AI)
//!
//! Használat:
//!   microscope-mem import-chatgpt <path> --persona Liora

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

// ─── ChatGPT Export Adatszerkezetek ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatGPTExport {
    #[serde(default)]
    pub conversations: Vec<Conversation>,
}

#[derive(Debug, Deserialize)]
pub struct Conversation {
    pub title: Option<String>,
    pub create_time: Option<f64>,
    pub update_time: Option<f64>,
    pub mapping: Option<HashMap<String, MappingNode>>,
    pub current_node: Option<String>,
    #[serde(default)]
    pub moderation_results: Vec<serde_json::Value>,
    pub plugin_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MappingNode {
    pub id: Option<String>,
    pub message: Option<ChatMessageData>,
    pub parent: Option<String>,
    pub children: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatMessageData {
    pub id: Option<String>,
    pub author: Option<Author>,
    pub content: Option<MessageContent>,
    pub create_time: Option<f64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Author {
    pub role: String,
    pub name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MessageContent {
    pub content_type: Option<String>,
    pub parts: Option<Vec<serde_json::Value>>,
    pub text: Option<String>,
}

// ─── Feldolgozott Adatszerkezetek ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProcessedMessage {
    pub conversation_title: String,
    pub role: String,          // "user" | "assistant"
    pub sender_name: String,   // Szilvi vagy Liora
    pub text: String,
    pub timestamp_ms: u64,
    pub message_index: usize,
    pub message_count: usize,
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub conversations_found: usize,
    pub total_messages: usize,
    pub user_messages: usize,
    pub ai_messages: usize,
    pub total_size_bytes: usize,
    pub errors: Vec<String>,
    pub import_duration_ms: u64,
}

// ─── Importáló Motor ───────────────────────────────────────────────────────

pub struct ChatGPTImporter {
    pub persona_name: String,
    pub user_name: String,
}

impl ChatGPTImporter {
    pub fn new(persona: &str) -> Self {
        Self {
            persona_name: persona.to_string(),
            user_name: "user".to_string(),
        }
    }

    /// JSON fájl beolvasása és feldolgozása
    pub fn parse_export(&self, path: &str) -> Result<VecDeque<ProcessedMessage>, String> {
        let content = fs::read_to_string(Path::new(path))
            .map_err(|e| format!("Nem lehet olvasni a fájlt: {}", e))?;

        // A ChatGPT export lehet tömbfájl vagy a {conversations: [...]} formátum
        let conversations: Vec<Conversation> = if content.trim().starts_with('[') {
            serde_json::from_str(&content)
                .map_err(|e| format!("JSON parse hiba (tömb): {}", e))?
        } else {
            let export: ChatGPTExport = serde_json::from_str(&content)
                .map_err(|e| format!("JSON parse hiba (objektum): {}", e))?;
            export.conversations
        };

        if conversations.is_empty() {
            return Err("Nincs egy beszélgetés sem az exportban.".to_string());
        }

        let mut all_messages = VecDeque::new();

        for conv in &conversations {
            let title = conv.title.as_deref().unwrap_or("Untitled");
            let mapping = match &conv.mapping {
                Some(m) => m,
                None => continue,
            };

            // Útvonal a current_node-ig
            let current = match &conv.current_node {
                Some(n) => n.clone(),
                None => continue,
            };

            // Visszafelé építjük a láncot: current_node → root
            let mut path = Vec::new();
            let mut node_id = current.clone();
            loop {
                path.push(node_id.clone());
                match mapping.get(&node_id) {
                    Some(node) => match &node.parent {
                        Some(parent) if !parent.is_empty() && parent != &node_id => {
                            node_id = parent.clone();
                        }
                        _ => break,
                    },
                    None => break,
                }
            }
            path.reverse();

            // Szűrés: csak a user/assistant üzenetek
            let conv_messages: Vec<ProcessedMessage> = path.iter()
                .filter_map(|nid| {
                    let node = mapping.get(nid)?;
                    let msg = node.message.as_ref()?;
                    let author = msg.author.as_ref()?;
                    let role = author.role.clone();

                    // Csak user és assistant
                    if role != "user" && role != "assistant" {
                        // Try "system" or others - only take user/assistant
                        if role != "system" { return None; }
                    }

                    let content = msg.content.as_ref()?;
                    let text = content.parts.as_ref()
                        .and_then(|parts| {
                            parts.first()
                                .and_then(|p| p.as_str())
                                .map(|s| s.to_string())
                        })
                        .or_else(|| content.text.clone())
                        .unwrap_or_default();

                    if text.trim().is_empty() { return None; }

                    let ts = (msg.create_time.unwrap_or(0.0) * 1000.0) as u64;
                    let sender = if role == "assistant" {
                        self.persona_name.clone()
                    } else {
                        self.user_name.clone()
                    };

                    Some(ProcessedMessage {
                        conversation_title: title.to_string(),
                        role,
                        sender_name: sender,
                        text: text.trim().to_string(),
                        timestamp_ms: ts,
                        message_index: 0,
                        message_count: path.len(),
                    })
                })
                .collect();

            for msg in conv_messages {
                all_messages.push_back(msg);
            }
        }

        Ok(all_messages)
    }

    /// Beszélgetések importálása Microscope Memory-ba
    pub fn import(&self, path: &str, microscope_binary: &str) -> ImportResult {
        let start = std::time::Instant::now();
        let mut errors = Vec::new();
        let mut total_messages = 0;
        let mut user_msgs = 0;
        let mut ai_msgs = 0;

        let messages = match self.parse_export(path) {
            Ok(m) => m,
            Err(e) => {
                return ImportResult {
                    conversations_found: 0, total_messages: 0,
                    user_messages: 0, ai_messages: 0,
                    total_size_bytes: 0, errors: vec![e],
                    import_duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        let mut conv_count = 0;
        let mut last_title = String::new();

        for msg in &messages {
            if msg.conversation_title != last_title {
                conv_count += 1;
                last_title = msg.conversation_title.clone();
            }

            // Tárolás Microscope Memory-ba
            let layer = if msg.role == "assistant" { "long_term" } else { "short_term" };
            let importance = if msg.role == "assistant" { 8 } else { 6 };
            let store_text = format!(
                "[{} | {}] {}: {}",
                msg.conversation_title, msg.sender_name,
                if msg.role == "assistant" { &self.persona_name } else { &self.user_name },
                msg.text
            );

            let cmd = format!(
                "{} store -l {} -i {} \"{}\"",
                microscope_binary, layer, importance, store_text.replace('\"', "\\\"")
            );

            // Futtatás (egyszerű: syscall)
            if std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .is_err()
            {
                errors.push(format!("Hiba üzenet tárolásakor: {}", msg.text.chars().take(50).collect::<String>()));
            }

            total_messages += 1;
            if msg.role == "assistant" { ai_msgs += 1; } else { user_msgs += 1; }
        }

        let file_size = fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);

        ImportResult {
            conversations_found: conv_count,
            total_messages,
            user_messages: user_msgs,
            ai_messages: ai_msgs,
            total_size_bytes: file_size,
            errors,
            import_duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}
