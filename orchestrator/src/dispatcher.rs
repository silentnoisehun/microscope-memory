// dispatcher.rs — The Spine Orchestrator
use crate::commands::{CommandIntent, CommandType};
use crate::modules::{route_command, ModuleTarget};
use memmap2::MmapMut;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::process::Command;

pub struct SpineDispatcher {
    mmap: Arc<Mutex<MmapMut>>,
    spine_path: String,
}

impl SpineDispatcher {
    pub async fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        
        Ok(Self {
            mmap: Arc::new(Mutex::new(mmap)),
            spine_path: path.to_string(),
        })
    }

    /// Translates a natural language string into a Spine command via the Agent
    pub async fn dispatch_intent(&self, raw_text: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("[Dispatcher] Processing intent: {}", raw_text);

        // 1. Request the Agent (Rongyasz) to parse the intent
        let intent = self.agent_parse(raw_text).await?;
        
        println!("[Dispatcher] Parsed intent: {:?}", intent.cmd);

        // 2. Write the command to the Spine (Command Slot 0)
        self.write_to_spine(&intent).await?;

        // 3. Route to the target module (for logging/tracking)
        let target = route_command(&intent.cmd);
        println!("[Dispatcher] Routing to: {:?}", target);

        Ok(())
    }

    async fn agent_parse(&self, text: &str) -> Result<CommandIntent, Box<dyn std::error::Error>> {
        let text_lower = text.to_lowercase();
        
        // 1. Check for direct commands first
        if text_lower.starts_with('/') {
            let parts: Vec<&str> = text_lower.split_whitespace().collect();
            let cmd_str = parts[0].trim_start_matches('/');
            let arg_str = if parts.len() > 1 { parts[1..].join(" ") } else { "".to_string() };

            let cmd = match cmd_str {
                "recall" => CommandType::Recall,
                "remember" => CommandType::Remember,
                "find" => CommandType::Find,
                "look" => CommandType::Look,
                "hebbian" => CommandType::Hebbian,
                "mirror" => CommandType::Mirror,
                "archetypes" => CommandType::Archetype,
                "patterns" => CommandType::Patterns,
                "dream" => CommandType::Dream,
                "doctor" => CommandType::Doctor,
                "status" => CommandType::Status,
                _ => CommandType::Unknown,
            };

            return Ok(CommandIntent { cmd, args: arg_str });
        }

        // 2. Attempt to use LLM (Ollama) if available for real intent parsing
        let ollama_result = Command::new("ollama")
            .arg(format!("Extract intent from this text: '{}'. Return ONLY a single word from this list: recall, remember, find, look, mutate, crispr, read, pipeline, hebbian, mirror, resonance, dream, status, doctor, unknown. No other text.", text))
            .output();

        if let Ok(output) = ollama_result {
            if output.status.success() {
                let response = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
                let cmd = match response.as_str() {
                    "recall" => CommandType::Recall,
                    "remember" => CommandType::Remember,
                    "find" => CommandType::Find,
                    "look" => CommandType::Look,
                    "mutate" => CommandType::Mutate,
                    "crispr" => CommandType::Crispr,
                    "read" => CommandType::Read,
                    "pipeline" => CommandType::Pipeline,
                    "hebbian" => CommandType::Hebbian,
                    "mirror" => CommandType::Mirror,
                    "resonance" => CommandType::Resonance,
                    "dream" => CommandType::Dream,
                    "status" => CommandType::Status,
                    "doctor" => CommandType::Doctor,
                    _ => CommandType::Unknown,
                };
                return Ok(CommandIntent { cmd, args: text.to_string() });
            }
        }

        // 3. Fallback to keyword matching (The "Heuristic" layer)
        if text_lower.contains("emlékszel") || text_lower.contains("keress") || text_lower.contains("mit") {
            return Ok(CommandIntent { cmd: CommandType::Recall, args: text.to_string() });
        } else if text_lower.contains("módosítsd") || text_lower.contains("javíts") || text_lower.contains("kód") {
            return Ok(CommandIntent { cmd: CommandType::Mutate, args: text.to_string() });
        } else if text_lower.contains("állapot") || text_lower.contains("statusz") || text_lower.contains("hogyan") {
            return Ok(CommandIntent { cmd: CommandType::Status, args: "".into() });
        }

        Ok(CommandIntent { cmd: CommandType::Unknown, args: text.to_string() })
    }

    async fn write_to_spine(&self, intent: &CommandIntent) -> Result<(), Box<dyn std::error::Error>> {
        let mut mmap = self.mmap.lock().await;
        
        // Slot 0: [0] = Command ID, [1..64] = Args
        mmap[0] = u8::from(intent.cmd.clone());
        
        let args_bytes = intent.args.as_bytes();
        let len = std::cmp::min(args_bytes.len(), 63);
        
        // Clear the rest of the slot
        for i in 1..64 {
            mmap[i] = 0;
        }
        
        // Write args
        mmap[1..1 + len].copy_from_slice(&args_bytes[..len]);
        
        // Flush to disk (mmap ensures it's visible to other processes via OS cache)
        mmap.flush()?;
        
        println!("[Spine] Command written: {:?} | Args: {}", intent.cmd, intent.args);
        Ok(())
    }
}
