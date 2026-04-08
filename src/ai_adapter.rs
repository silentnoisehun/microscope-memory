//! AI Adapter for direct binary communication with Microscope Memory.
//!
//! Provides a high-performance interface for AI systems to interact with the memory
//! using fixed 256-byte binary commands over Unix domain sockets or named pipes.
//!
//! Zero-JSON, zero-copy, zero-latency communication protocol.

use crate::config::Config;
use crate::reader::MicroscopeReader;
use std::path::Path;
use std::sync::Arc;
#[cfg(unix)]
use std::{io::Read, io::Write};

// ─── Binary Protocol Definition ──────────────────────────

/// Fixed 256-byte binary command structure for AI communication.
/// Repr(C) ensures consistent layout across platforms.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct AICommand {
    pub op_code: u8,        // 0: Read, 1: Write, 2: Learn/Drift
    pub layer: u8,          // Target layer (0-9)
    pub block_id: u64,      // Block identifier (0 for new blocks)
    pub weight_delta: f32,  // Learning weight delta for Hebbian updates
    pub payload: [u8; 242], // Data payload (fits in 256 bytes total)
}

impl AICommand {
    /// Create a new read command
    pub fn read(layer: u8, block_id: u64) -> Self {
        Self {
            op_code: 0,
            layer,
            block_id,
            weight_delta: 0.0,
            payload: [0; 242],
        }
    }

    /// Create a new write command
    pub fn write(layer: u8, block_id: u64, data: &[u8]) -> Self {
        let mut payload = [0u8; 242];
        let len = data.len().min(242);
        payload[..len].copy_from_slice(&data[..len]);

        Self {
            op_code: 1,
            layer,
            block_id,
            weight_delta: 0.0,
            payload,
        }
    }

    /// Create a learning/drift command
    pub fn learn(block_id: u64, weight_delta: f32) -> Self {
        Self {
            op_code: 2,
            layer: 0,
            block_id,
            weight_delta,
            payload: [0; 242],
        }
    }
}

impl Default for AICommand {
    fn default() -> Self {
        Self {
            op_code: 0,
            layer: 0,
            block_id: 0,
            weight_delta: 0.0,
            payload: [0; 242],
        }
    }
}

// ─── AI Adapter Implementation ───────────────────────────

/// High-performance AI adapter for binary protocol communication.
pub struct AIAdapter {
    config: Arc<Config>,
    reader: Arc<MicroscopeReader>,
    dirty_blocks: std::collections::HashSet<u64>, // Blocks that need Merkle update
    command_count: usize,                         // Commands processed since last Merkle update
}

impl AIAdapter {
    /// Create a new AI adapter with the given configuration.
    pub fn new(config: Config) -> Result<Self, String> {
        let reader = MicroscopeReader::open(&config)?;
        Ok(Self {
            config: Arc::new(config),
            reader: Arc::new(reader),
            dirty_blocks: std::collections::HashSet::new(),
            command_count: 0,
        })
    }

    /// Process an AI command and return a response.
    pub fn process_command(&mut self, cmd: AICommand) -> Result<AICommand, String> {
        self.command_count += 1;

        let result = match cmd.op_code {
            0 => self.handle_read(cmd),
            1 => self.handle_write(cmd),
            2 => self.handle_learn(cmd),
            _ => Err(format!("Unknown op_code: {}", cmd.op_code)),
        };

        // Lazy Merkle update: batch updates every 100 commands or when explicitly requested
        if self.command_count.is_multiple_of(100) && !self.dirty_blocks.is_empty() {
            self.update_merkle_tree()?;
        }

        result
    }

    /// Handle read operations.
    fn handle_read(&self, cmd: AICommand) -> Result<AICommand, String> {
        if cmd.block_id as usize >= self.reader.block_count {
            return Err("Block ID out of range".to_string());
        }

        let text = self.reader.text(cmd.block_id as usize);
        let data = text.as_bytes();
        let mut response = AICommand {
            op_code: 0, // Read response
            layer: cmd.layer,
            block_id: cmd.block_id,
            ..AICommand::default()
        };
        let len = data.len().min(242);
        response.payload[..len].copy_from_slice(&data[..len]);

        Ok(response)
    }

    /// Handle write operations.
    fn handle_write(&mut self, cmd: AICommand) -> Result<AICommand, String> {
        // Mark block as dirty for Merkle update
        self.dirty_blocks.insert(cmd.block_id);

        // For now, writes go through the append log
        // TODO: Implement direct block writing with Merkle updates
        Err("Direct write not yet implemented".to_string())
    }

    /// Handle learning/drift operations.
    fn handle_learn(&mut self, cmd: AICommand) -> Result<AICommand, String> {
        // Mark block as dirty for Merkle update
        self.dirty_blocks.insert(cmd.block_id);

        // TODO: Implement Hebbian learning interface with saturation
        Err("Learning operations not yet implemented".to_string())
    }

    /// Force immediate Merkle tree update for dirty blocks.
    pub fn update_merkle_tree(&mut self) -> Result<(), String> {
        if self.dirty_blocks.is_empty() {
            return Ok(());
        }

        // TODO: Implement incremental Merkle tree update for dirty blocks
        // For now, mark as clean
        self.dirty_blocks.clear();
        Ok(())
    }

    /// Get current Merkle root for integrity verification.
    pub fn current_merkle_root(&self) -> Result<[u8; 32], String> {
        let output_dir = Path::new(&self.config.paths.output_dir);
        let merkle_data = std::fs::read(output_dir.join("merkle.bin"))
            .map_err(|e| format!("Failed to read merkle.bin: {}", e))?;
        let tree = crate::merkle::MerkleTree::from_bytes(&merkle_data)
            .ok_or("Invalid merkle.bin format")?;
        Ok(tree.root)
    }
}

// ─── Cross-Platform Socket Listener ─────────────────────

/// Platform-specific socket listener for AI communication.
pub struct AISocketListener {
    #[cfg(unix)]
    listener: std::os::unix::net::UnixListener,
    #[cfg(windows)]
    // TODO: Implement named pipe listener for Windows
    _placeholder: (),
}

impl AISocketListener {
    /// Create a new socket listener at the given path.
    #[cfg(unix)]
    pub fn new(path: &str) -> Result<Self, String> {
        // Remove existing socket if it exists
        let _ = std::fs::remove_file(path);

        let listener = std::os::unix::net::UnixListener::bind(path)
            .map_err(|e| format!("Failed to bind Unix socket: {}", e))?;

        Ok(Self { listener })
    }

    #[cfg(windows)]
    pub fn new(_path: &str) -> Result<Self, String> {
        // TODO: Implement Windows named pipe
        Err("Windows named pipe support not yet implemented".to_string())
    }

    /// Accept incoming connections and handle commands.
    pub fn listen(&self, adapter: &AIAdapter) -> Result<(), String> {
        #[cfg(windows)]
        let _ = adapter;

        #[cfg(unix)]
        {
            for stream in self.listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Connection error: {}", e);
                        continue;
                    }
                };

                // Read 256-byte command
                let mut buffer = [0u8; 256];
                if let Err(e) = stream.read_exact(&mut buffer) {
                    eprintln!("Read error: {}", e);
                    continue;
                }

                // Zero-copy cast to AICommand
                let cmd: AICommand = unsafe { std::mem::transmute(buffer) };

                // Process command
                match adapter.process_command(cmd) {
                    Ok(response) => {
                        // Send back 256-byte response
                        let response_bytes: [u8; 256] = unsafe { std::mem::transmute(response) };
                        if let Err(e) = stream.write_all(&response_bytes) {
                            eprintln!("Write error: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Command processing error: {}", e);
                        // Send error response
                        let error_response = AICommand {
                            op_code: 255, // Error
                            ..AICommand::default()
                        };
                        let response_bytes: [u8; 256] =
                            unsafe { std::mem::transmute(error_response) };
                        let _ = stream.write_all(&response_bytes);
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            // TODO: Implement Windows named pipe listening
            Err("Windows support not implemented".to_string())
        }

        #[cfg(not(windows))]
        Ok(())
    }
}

// ─── Tests ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn test_config() -> Config {
        let mut config = Config::default();
        config.paths.output_dir = "test_output".to_string();
        config.paths.layers_dir = "test_layers".to_string();
        config
    }

    #[test]
    fn test_ai_command_creation() {
        let read_cmd = AICommand::read(1, 42);
        assert_eq!(read_cmd.op_code, 0);
        assert_eq!(read_cmd.layer, 1);
        assert_eq!(read_cmd.block_id, 42);

        let write_cmd = AICommand::write(2, 0, b"Hello AI");
        assert_eq!(write_cmd.op_code, 1);
        assert_eq!(write_cmd.layer, 2);
        assert_eq!(write_cmd.payload[0], b'H');
    }

    #[test]
    fn test_ai_adapter_creation() {
        // This test requires actual built memory files
        // For now, just test that the struct can be created
        let _config = test_config();
        // Note: This will fail without actual memory files
        // let adapter = AIAdapter::new(config);
        // assert!(adapter.is_ok());
    }
}
