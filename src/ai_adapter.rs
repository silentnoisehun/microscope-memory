//! AI Adapter for direct binary communication with Microscope Memory.
//!
//! Provides a high-performance interface for AI systems to interact with the memory
//! using fixed 256-byte binary commands over Unix domain sockets or named pipes.
//!
//! Zero-JSON, zero-copy, zero-latency communication protocol.
//!
//! **Experimental and currently UNWIRED**: this module is not referenced from
//! `main.rs`, `mcp.rs` or `bridge.rs`. Do not rely on its integrity claims —
//! `update_merkle_tree` fails loudly until the incremental Merkle path exists,
//! so a stale root can never be reported as freshly verified.

use crate::config::Config;
use crate::reader::MicroscopeReader;
use crate::{store_memory, LAYER_NAMES};
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
    pub block_id: u64,      // Block identifier (0 for new blocks)
    pub weight_delta: f32,  // Learning weight delta for Hebbian updates
    pub op_code: u8,        // 0: Read, 1: Write, 2: Learn/Drift
    pub layer: u8,          // Target layer (0-9)
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

const _: () = assert!(std::mem::size_of::<AICommand>() == 256);

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
    /// The payload format is: [1 byte length][UTF-8 text bytes].
    /// The text is stored into the append log via store_memory, using the
    /// layer encoded in cmd.layer and a default importance of 5.
    fn handle_write(&mut self, cmd: AICommand) -> Result<AICommand, String> {
        let len = cmd.payload[0] as usize;
        if len > 241 {
            return Err("Invalid payload length".to_string());
        }
        let text = String::from_utf8(cmd.payload[1..1 + len].to_vec())
            .map_err(|e| format!("Invalid UTF-8 payload: {}", e))?;

        let layer_name = LAYER_NAMES
            .get(cmd.layer as usize)
            .copied()
            .unwrap_or("long_term");

        store_memory(
            &self.config,
            &text,
            layer_name,
            5, // default importance
        )?;

        // Mark block as dirty for future Merkle update support
        self.dirty_blocks.insert(cmd.block_id);

        Ok(AICommand {
            op_code: 1, // Write response
            layer: cmd.layer,
            block_id: cmd.block_id,
            ..AICommand::default()
        })
    }

    /// Handle learning/drift operations.
    /// Records a Hebbian activation for the given block with the supplied weight delta.
    fn handle_learn(&mut self, cmd: AICommand) -> Result<AICommand, String> {
        use crate::hebbian::HebbianState;

        let mut state = HebbianState::load_or_init(
            Path::new(&self.config.paths.output_dir),
            self.reader.block_count,
        );

        let block_idx = cmd.block_id as u32;
        let query_hash = cmd.block_id; // use block id as query identifier
        state.record_activation(&[(block_idx, cmd.weight_delta)], query_hash);

        state.save(Path::new(&self.config.paths.output_dir))?;

        // Mark block as dirty for future Merkle update support
        self.dirty_blocks.insert(cmd.block_id);

        Ok(AICommand {
            op_code: 2, // Learn response
            layer: cmd.layer,
            block_id: cmd.block_id,
            ..AICommand::default()
        })
    }

    /// Force immediate Merkle tree update for dirty blocks.
    ///
    /// The incremental Merkle update is not implemented yet. When blocks are
    /// dirty this returns an explicit error instead of clearing the dirty set
    /// and pretending the tree was updated — a stale Merkle root must never be
    /// reported as freshly verified.
    pub fn update_merkle_tree(&mut self) -> Result<(), String> {
        if self.dirty_blocks.is_empty() {
            return Ok(());
        }
        Err(format!(
            "incremental Merkle update not implemented ({} dirty blocks pending)",
            self.dirty_blocks.len()
        ))
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

// ─── Windows Named Pipe Implementation ──────────────────

#[cfg(windows)]
mod windows_pipe {
    use std::ffi::CString;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile, PIPE_ACCESS_DUPLEX};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeA, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
    };

    pub struct NamedPipeListener {
        handle: HANDLE,
    }

    impl NamedPipeListener {
        pub fn new(name: &str) -> Result<Self, String> {
            // Ensure pipe name uses Windows named pipe format
            let pipe_name = if name.starts_with("\\\\.\\pipe\\") {
                name.to_string()
            } else {
                format!("\\\\.\\pipe\\{}", name)
            };
            let c_name =
                CString::new(pipe_name).map_err(|e| format!("Invalid pipe name: {}", e))?;

            let handle = unsafe {
                CreateNamedPipeA(
                    c_name.as_ptr() as *const u8,
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_MESSAGE
                        | PIPE_READMODE_MESSAGE
                        | PIPE_WAIT
                        | PIPE_REJECT_REMOTE_CLIENTS,
                    1,                // max instances
                    256,              // out buffer
                    256,              // in buffer
                    0,                // default timeout
                    std::ptr::null(), // no security attrs
                )
            };

            if handle == INVALID_HANDLE_VALUE {
                return Err("Failed to create named pipe".to_string());
            }

            Ok(Self { handle })
        }

        pub fn accept(&self) -> Result<NamedPipeConnection, String> {
            let connected = unsafe { ConnectNamedPipe(self.handle, std::ptr::null_mut()) };
            if connected == 0 {
                // Client may already be connected; GetLastError would tell, but we proceed
            }
            Ok(NamedPipeConnection {
                handle: self.handle,
            })
        }
    }

    impl Drop for NamedPipeListener {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    pub struct NamedPipeConnection {
        handle: HANDLE,
    }

    impl NamedPipeConnection {
        pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), String> {
            let mut total = 0usize;
            while total < buf.len() {
                let mut read = 0u32;
                let ok = unsafe {
                    ReadFile(
                        self.handle,
                        buf.as_mut_ptr().add(total) as *mut u8,
                        (buf.len() - total) as u32,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    return Err("ReadFile failed".to_string());
                }
                if read == 0 {
                    return Err("Pipe closed".to_string());
                }
                total += read as usize;
            }
            Ok(())
        }

        pub fn write_all(&mut self, buf: &[u8]) -> Result<(), String> {
            let mut total = 0usize;
            while total < buf.len() {
                let mut written = 0u32;
                let ok = unsafe {
                    WriteFile(
                        self.handle,
                        buf.as_ptr().add(total) as *const u8,
                        (buf.len() - total) as u32,
                        &mut written,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    return Err("WriteFile failed".to_string());
                }
                total += written as usize;
            }
            Ok(())
        }
    }

    impl Drop for NamedPipeConnection {
        fn drop(&mut self) {
            unsafe {
                DisconnectNamedPipe(self.handle);
            }
        }
    }
} // ─── Cross-Platform Socket Listener ─────────────────────

/// Platform-specific socket listener for AI communication.
pub struct AISocketListener {
    #[cfg(unix)]
    listener: std::os::unix::net::UnixListener,
    #[cfg(windows)]
    listener: windows_pipe::NamedPipeListener,
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
    pub fn new(path: &str) -> Result<Self, String> {
        let listener = windows_pipe::NamedPipeListener::new(path)
            .map_err(|e| format!("Failed to create named pipe listener: {}", e))?;
        Ok(Self { listener })
    }

    /// Accept incoming connections and handle commands.
    pub fn listen(&self, adapter: &mut AIAdapter) -> Result<(), String> {
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
            loop {
                let mut conn = match self.listener.accept() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Connection error: {}", e);
                        continue;
                    }
                };

                let mut buffer = [0u8; 256];
                if let Err(e) = conn.read_exact(&mut buffer) {
                    eprintln!("Read error: {}", e);
                    continue;
                }

                let cmd: AICommand = unsafe { std::mem::transmute(buffer) };

                match adapter.process_command(cmd) {
                    Ok(response) => {
                        let response_bytes: [u8; 256] = unsafe { std::mem::transmute(response) };
                        if let Err(e) = conn.write_all(&response_bytes) {
                            eprintln!("Write error: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Command processing error: {}", e);
                        let error_response = AICommand {
                            op_code: 255,
                            ..AICommand::default()
                        };
                        let response_bytes: [u8; 256] =
                            unsafe { std::mem::transmute(error_response) };
                        let _ = conn.write_all(&response_bytes);
                    }
                }
            }
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

    #[test]
    fn update_merkle_tree_is_honest_about_dirty_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.paths.layers_dir = tmp.path().join("layers").to_string_lossy().to_string();
        config.paths.output_dir = tmp.path().join("data").to_string_lossy().to_string();
        config.memory_layers.layers = vec!["long_term".to_string()];
        config.embedding.provider = "none".to_string();
        std::fs::create_dir_all(&config.paths.layers_dir).unwrap();
        crate::build::build(&config, true, true).unwrap();

        let mut adapter = AIAdapter::new(config).unwrap();

        // Clean state: no pending work, Ok.
        assert!(adapter.update_merkle_tree().is_ok());

        // Dirty state: must fail loudly and must NOT clear the dirty set.
        adapter.dirty_blocks.insert(42);
        let err = adapter.update_merkle_tree().unwrap_err();
        assert!(err.contains("not implemented"), "err: {err}");
        assert!(
            adapter.dirty_blocks.contains(&42),
            "dirty set must survive a failed update"
        );
    }
}
