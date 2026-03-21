#![allow(dead_code)]
// Real-time streaming updates module
// Allows live updates to the microscope index

use std::sync::{Arc, RwLock, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::fs::{File, OpenOptions};
use std::io::{Write, BufReader, BufRead};
use std::path::Path;

pub struct StreamingUpdate {
    sender: mpsc::Sender<UpdateCommand>,
    receiver: Arc<Mutex<mpsc::Receiver<UpdateCommand>>>,
    index_lock: Arc<RwLock<StreamingIndex>>,
}

pub struct StreamingIndex {
    blocks: Vec<LiveBlock>,
    pending_updates: Vec<PendingUpdate>,
    last_persist: Instant,
}

#[derive(Clone)]
pub struct LiveBlock {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub depth: u8,
    pub layer_id: u8,
    pub timestamp: u64,
    pub version: u32,
}

pub struct PendingUpdate {
    pub block_id: usize,
    pub field: UpdateField,
    pub timestamp: u64,
}

pub enum UpdateField {
    Text(String),
    Position(f32, f32, f32),
    Depth(u8),
}

impl Clone for UpdateField {
    fn clone(&self) -> Self {
        match self {
            UpdateField::Text(t) => UpdateField::Text(t.clone()),
            UpdateField::Position(x, y, z) => UpdateField::Position(*x, *y, *z),
            UpdateField::Depth(d) => UpdateField::Depth(*d),
        }
    }
}

pub enum UpdateCommand {
    Add(LiveBlock),
    Update(usize, UpdateField),
    Delete(usize),
    Batch(Vec<UpdateCommand>),
    Persist,
    Stop,
}

impl StreamingUpdate {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let index = StreamingIndex {
            blocks: Vec::new(),
            pending_updates: Vec::new(),
            last_persist: Instant::now(),
        };

        Self {
            sender: tx,
            receiver: Arc::new(Mutex::new(rx)),
            index_lock: Arc::new(RwLock::new(index)),
        }
    }

    /// Start the streaming update worker thread
    pub fn start(&self) -> thread::JoinHandle<()> {
        let receiver = Arc::clone(&self.receiver);
        let index = Arc::clone(&self.index_lock);

        thread::spawn(move || {
            let rx = receiver.lock().unwrap();
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(cmd) => {
                        match cmd {
                            UpdateCommand::Stop => break,
                            UpdateCommand::Add(block) => {
                                let mut idx = index.write().unwrap();
                                idx.blocks.push(block);
                            }
                            UpdateCommand::Update(id, field) => {
                                let mut idx = index.write().unwrap();
                                if id < idx.blocks.len() {
                                    match &field {
                                        UpdateField::Text(text) => {
                                            idx.blocks[id].text = text.clone();
                                            idx.blocks[id].version += 1;
                                        }
                                        UpdateField::Position(x, y, z) => {
                                            idx.blocks[id].x = *x;
                                            idx.blocks[id].y = *y;
                                            idx.blocks[id].z = *z;
                                            idx.blocks[id].version += 1;
                                        }
                                        UpdateField::Depth(depth) => {
                                            idx.blocks[id].depth = *depth;
                                            idx.blocks[id].version += 1;
                                        }
                                    }
                                    idx.pending_updates.push(PendingUpdate {
                                        block_id: id,
                                        field: field.clone(),
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs(),
                                    });
                                }
                            }
                            UpdateCommand::Delete(id) => {
                                let mut idx = index.write().unwrap();
                                if id < idx.blocks.len() {
                                    idx.blocks.remove(id);
                                }
                            }
                            UpdateCommand::Batch(commands) => {
                                for _cmd in commands {
                                    // Recursively process batch commands
                                    // In real implementation, would send through channel
                                }
                            }
                            UpdateCommand::Persist => {
                                let idx = index.read().unwrap();
                                Self::persist_to_disk(&idx);
                            }
                        }
                    }
                    Err(_) => {
                        // Timeout - check if we need to auto-persist
                        let mut idx = index.write().unwrap();
                        if idx.last_persist.elapsed() > Duration::from_secs(60) {
                            Self::persist_to_disk(&idx);
                            idx.last_persist = Instant::now();
                            idx.pending_updates.clear();
                        }
                    }
                }
            }
        })
    }

    fn persist_to_disk(index: &StreamingIndex) {
        // Write pending updates to append log
        let path = Path::new("D:/Claude Memory/microscope/stream_updates.log");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            for update in &index.pending_updates {
                let line = format!(
                    "{},{},{:?}\n",
                    update.timestamp,
                    update.block_id,
                    match &update.field {
                        UpdateField::Text(t) => format!("text:{}", t),
                        UpdateField::Position(x, y, z) => format!("pos:{},{},{}", x, y, z),
                        UpdateField::Depth(d) => format!("depth:{}", d),
                    }
                );
                let _ = file.write_all(line.as_bytes());
            }
        }
    }

    /// Send an update command
    pub fn send(&self, cmd: UpdateCommand) -> Result<(), mpsc::SendError<UpdateCommand>> {
        self.sender.send(cmd)
    }

    /// Get current block count
    pub fn block_count(&self) -> usize {
        self.index_lock.read().unwrap().blocks.len()
    }

    /// Get pending update count
    pub fn pending_count(&self) -> usize {
        self.index_lock.read().unwrap().pending_updates.len()
    }

    /// Search blocks in real-time
    pub fn search(&self, query: &str, k: usize) -> Vec<LiveBlock> {
        let index = self.index_lock.read().unwrap();
        let mut results: Vec<(f32, &LiveBlock)> = Vec::new();

        for block in &index.blocks {
            if block.text.to_lowercase().contains(&query.to_lowercase()) {
                let relevance = 1.0 / (1.0 + block.version as f32); // Prefer newer versions
                results.push((relevance, block));
            }
        }

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.truncate(k);
        results.into_iter().map(|(_, b)| b.clone()).collect()
    }
}

/// WebSocket server for streaming updates (future enhancement)
pub struct StreamingServer {
    port: u16,
    update_handler: Arc<StreamingUpdate>,
}

impl StreamingServer {
    pub fn new(port: u16, handler: Arc<StreamingUpdate>) -> Self {
        Self {
            port,
            update_handler: handler,
        }
    }

    /// Start WebSocket server (stub for now)
    pub fn start(&self) {
        println!("Streaming server would start on port {}", self.port);
        // In real implementation:
        // - Accept WebSocket connections
        // - Parse JSON update commands
        // - Send updates through StreamingUpdate
        // - Broadcast changes to connected clients
    }
}

/// Load streaming updates from log file
pub fn load_stream_log(path: &Path) -> Vec<PendingUpdate> {
    let mut updates = Vec::new();

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(line) = line {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 3 {
                    let timestamp = parts[0].parse::<u64>().unwrap_or(0);
                    let block_id = parts[1].parse::<usize>().unwrap_or(0);
                    let field_str = parts[2];

                    let field = if field_str.starts_with("text:") {
                        UpdateField::Text(field_str[5..].to_string())
                    } else if field_str.starts_with("pos:") {
                        let coords: Vec<f32> = field_str[4..]
                            .split(',')
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if coords.len() == 3 {
                            UpdateField::Position(coords[0], coords[1], coords[2])
                        } else {
                            continue;
                        }
                    } else if field_str.starts_with("depth:") {
                        UpdateField::Depth(field_str[6..].parse().unwrap_or(0))
                    } else {
                        continue;
                    };

                    updates.push(PendingUpdate {
                        block_id,
                        field,
                        timestamp,
                    });
                }
            }
        }
    }

    updates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_updates() {
        let stream = StreamingUpdate::new();
        let handle = stream.start();

        // Add a block
        let block = LiveBlock {
            text: "Test block".to_string(),
            x: 0.5,
            y: 0.5,
            z: 0.5,
            depth: 3,
            layer_id: 0,
            timestamp: 0,
            version: 1,
        };

        stream.send(UpdateCommand::Add(block)).unwrap();
        thread::sleep(Duration::from_millis(200));

        assert_eq!(stream.block_count(), 1);

        // Update the block
        stream.send(UpdateCommand::Update(0, UpdateField::Text("Updated".to_string()))).unwrap();
        thread::sleep(Duration::from_millis(200));

        assert_eq!(stream.pending_count(), 1);

        // Stop the worker
        stream.send(UpdateCommand::Stop).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_search() {
        let stream = StreamingUpdate::new();
        let _handle = stream.start();

        // Add some blocks
        for i in 0..10 {
            let block = LiveBlock {
                text: format!("Block {}", i),
                x: i as f32 / 10.0,
                y: 0.5,
                z: 0.5,
                depth: 3,
                layer_id: 0,
                timestamp: i as u64,
                version: 1,
            };
            stream.send(UpdateCommand::Add(block)).unwrap();
        }

        thread::sleep(Duration::from_millis(200));

        let results = stream.search("Block", 5);
        assert_eq!(results.len(), 5);

        stream.send(UpdateCommand::Stop).unwrap();
    }
}