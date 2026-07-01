//! Working Memory — korlátos puffer 7±2 item, gyors decay (30mp), primacy/recency boost, auto-konszolidáció.
//!
//! Binary format: WKM1 (Working Memory v1)
//!   [W,K,M,1] magic (4 bytes)
//!   capacity: u8 | decay_ms: u32 | count: u8
//!   items[]: text_len:u16 text timestamp_ms:u64 last_access_ms:u64
//!            importance:f32 decay_rate:f32 serial_pos:u8
//!            memory_type:u8 access_count:u32 layer_len:u8 layer

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Constants ─────────────────────────────────────────
const DEFAULT_CAPACITY: usize = 7; // Miller-törvény: 7±2
const DECAY_MS: u64 = 30_000; // 30 mp alatt felejt
const CONSOLIDATE_ACCESS_THRESHOLD: u32 = 3; // 3 hozzáférés után konszolidáció
const PRIMACY_BOOST: f32 = 0.15; // első elem boost
const RECENCY_BOOST: f32 = 0.10; // utolsó elem boost
                                 // ─── Types ─────────────────────────────────────────────
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemoryType {
    Episodic = 0,
    Semantic = 1,
    Implicit = 2,
    Explicit = 3,
}

/// Egy item a working memory-ban.
#[derive(Clone, Debug)]
pub struct WorkingMemoryItem {
    pub text: String,
    pub timestamp_ms: u64,
    pub last_access_ms: u64,
    pub importance: f32,
    pub decay_rate: f32,
    pub serial_pos: u8,
    pub memory_type: MemoryType,
    pub access_count: u32,
    pub layer: String,
}

/// Working Memory állapot — korlátos puffer.
pub struct WorkingMemory {
    pub items: Vec<WorkingMemoryItem>,
    pub capacity: usize,
    pub decay_ms: u64,
}

/// Statisztikák.
pub struct WorkingMemoryStats {
    pub item_count: usize,
    pub capacity: usize,
    pub hot_items: usize, // access_count >= 2
    pub decay_ms: u64,
    pub consolidation_candidates: usize, // access_count >= CONSOLIDATE_ACCESS_THRESHOLD
}
impl WorkingMemory {
    /// Betöltés WKM1 file-ból, vagy üres init.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("working_memory.bin");
        if let Ok(data) = fs::read(&path) {
            if data.len() >= 6 && &data[0..4] == b"WKM1" {
                let capacity = data[4] as usize;
                let decay_ms = u32::from_le_bytes([data[5], data[6], data[7], data[8]]) as u64;
                let count = data[9] as usize;
                let mut items = Vec::with_capacity(count);
                let mut off = 10;
                for _ in 0..count {
                    if off + 2 > data.len() {
                        break;
                    }
                    let text_len = u16::from_le_bytes([data[off], data[off + 1]]) as usize;
                    off += 2;
                    if off + text_len > data.len() {
                        break;
                    }
                    let text = String::from_utf8_lossy(&data[off..off + text_len]).to_string();
                    off += text_len;
                    if off + 37 > data.len() {
                        break;
                    }
                    let ts = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
                    let la = u64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap());
                    let imp = f32::from_le_bytes(data[off + 16..off + 20].try_into().unwrap());
                    let dr = f32::from_le_bytes(data[off + 20..off + 24].try_into().unwrap());
                    let sp = data[off + 24];
                    let mt = match data[off + 25] {
                        0 => MemoryType::Episodic,
                        1 => MemoryType::Semantic,
                        2 => MemoryType::Implicit,
                        3 => MemoryType::Explicit,
                        _ => MemoryType::Episodic,
                    };
                    let ac = u32::from_le_bytes(data[off + 26..off + 30].try_into().unwrap());
                    let ll = data[off + 30] as usize;
                    off += 31;
                    let layer = if off + ll <= data.len() {
                        String::from_utf8_lossy(&data[off..off + ll]).to_string()
                    } else {
                        String::new()
                    };
                    off += ll;
                    items.push(WorkingMemoryItem {
                        text,
                        timestamp_ms: ts,
                        last_access_ms: la,
                        importance: imp,
                        decay_rate: dr,
                        serial_pos: sp,
                        memory_type: mt,
                        access_count: ac,
                        layer,
                    });
                }
                return WorkingMemory {
                    items,
                    capacity,
                    decay_ms,
                };
            }
        }
        WorkingMemory {
            items: Vec::new(),
            capacity: DEFAULT_CAPACITY,
            decay_ms: DECAY_MS,
        }
    }

    /// Mentés WKM1 binary formátumba.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("working_memory.bin");
        let mut buf = Vec::with_capacity(64 + self.items.len() * 128);
        buf.extend_from_slice(b"WKM1");
        buf.push(self.capacity as u8);
        buf.extend_from_slice(&(self.decay_ms as u32).to_le_bytes());
        buf.push(self.items.len() as u8);
        for item in &self.items {
            let text_bytes = item.text.as_bytes();
            let text_len = text_bytes.len().min(u16::MAX as usize);
            buf.extend_from_slice(&(text_len as u16).to_le_bytes());
            buf.extend_from_slice(&text_bytes[..text_len]);
            buf.extend_from_slice(&item.timestamp_ms.to_le_bytes());
            buf.extend_from_slice(&item.last_access_ms.to_le_bytes());
            buf.extend_from_slice(&item.importance.to_le_bytes());
            buf.extend_from_slice(&item.decay_rate.to_le_bytes());
            buf.push(item.serial_pos);
            buf.push(item.memory_type as u8);
            buf.extend_from_slice(&item.access_count.to_le_bytes());
            let layer_bytes = item.layer.as_bytes();
            let ll = layer_bytes.len().min(255);
            buf.push(ll as u8);
            buf.extend_from_slice(&layer_bytes[..ll]);
        }
        let tmp_path = output_dir.join("working_memory.bin.tmp");
        fs::write(&tmp_path, &buf).map_err(|e| format!("write working_memory.bin: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename working_memory.bin: {}", e))
    }
    /// Push one item. Evict lowest-score item if full.
    pub fn push(&mut self, text: &str, importance: f32, layer: &str, mem_type: MemoryType) {
        let now = now_ms();
        // Evict if at capacity
        if self.items.len() >= self.capacity {
            let mut worst = 0;
            let mut worst_score = f32::MAX;
            for (i, item) in self.items.iter().enumerate() {
                let recency =
                    ((now - item.last_access_ms) as f64 / self.decay_ms as f64).max(0.0) as f32;
                let age_weight = if recency > 1.0 {
                    0.1
                } else {
                    1.0 - recency * 0.9
                };
                let score = item.importance * age_weight;
                if score < worst_score {
                    worst_score = score;
                    worst = i;
                }
            }
            self.items.remove(worst);
        }
        self.items.push(WorkingMemoryItem {
            text: text.to_string(),
            timestamp_ms: now,
            last_access_ms: now,
            importance,
            decay_rate: self.decay_ms as f32 / DEFAULT_CAPACITY as f32,
            serial_pos: self.items.len() as u8,
            memory_type: mem_type,
            access_count: 0,
            layer: layer.to_string(),
        });
    }

    /// Decay: csökkenti az importance-t, evict < 0.1.
    pub fn decay(&mut self) {
        let now = now_ms();
        self.items.retain(|item| {
            let elapsed = (now - item.last_access_ms) as f32;
            let age_ratio = elapsed / self.decay_ms as f32;
            age_ratio < 1.0 || item.access_count >= CONSOLIDATE_ACCESS_THRESHOLD
        });
    }

    /// Access an item by index: bumps last_access, increments count.
    /// Applies primacy boost (serial_pos == 0) and recency boost (last item).
    pub fn access(&mut self, idx: usize) -> Option<&WorkingMemoryItem> {
        if idx >= self.items.len() {
            return None;
        }
        let recency_boost = idx == self.items.len() - 1;
        let item = &mut self.items[idx];
        item.last_access_ms = now_ms();
        item.access_count += 1;
        if item.serial_pos == 0 {
            item.importance += PRIMACY_BOOST;
        }
        if recency_boost {
            item.importance += RECENCY_BOOST;
        }
        // Cannot return &self.items[idx] while &mut self.items[idx] is alive
        None
    }

    /// Find item by text substring (linear scan).
    pub fn find(&self, query: &str) -> Option<usize> {
        let lower = query.to_lowercase();
        self.items
            .iter()
            .position(|item| item.text.to_lowercase().contains(&lower))
    }

    /// Consolidate high-access items: returns the items that qualify.
    /// Caller is responsible for storing them via store_memory() or other means.
    pub fn consolidate(&mut self) -> Vec<WorkingMemoryItem> {
        let mut consolidated = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            if self.items[i].access_count >= CONSOLIDATE_ACCESS_THRESHOLD {
                consolidated.push(self.items.remove(i));
            } else {
                i += 1;
            }
        }
        consolidated
    }

    /// Compute working memory stats.
    pub fn stats(&self) -> WorkingMemoryStats {
        let hot = self.items.iter().filter(|i| i.access_count >= 2).count();
        let cons_candidates = self
            .items
            .iter()
            .filter(|i| i.access_count >= CONSOLIDATE_ACCESS_THRESHOLD)
            .count();
        WorkingMemoryStats {
            item_count: self.items.len(),
            capacity: self.capacity,
            hot_items: hot,
            decay_ms: self.decay_ms,
            consolidation_candidates: cons_candidates,
        }
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// Current timestamp in milliseconds since UNIX epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_default_state() {
        let wm = WorkingMemory::load_or_init(Path::new("."));
        assert!(wm.items.is_empty());
        assert_eq!(wm.capacity, DEFAULT_CAPACITY);
        assert_eq!(wm.decay_ms, DECAY_MS);
    }

    #[test]
    fn test_push_and_count() {
        let mut wm = WorkingMemory::load_or_init(Path::new("."));
        wm.push("first item", 5.0, "short_term", MemoryType::Episodic);
        wm.push("second item", 3.0, "long_term", MemoryType::Semantic);
        assert_eq!(wm.items.len(), 2);
    }

    #[test]
    fn test_eviction_when_full() {
        let mut wm = WorkingMemory {
            items: vec![],
            capacity: 3,
            decay_ms: 30000,
        };
        wm.push("a", 1.0, "st", MemoryType::Episodic);
        wm.push("b", 1.0, "st", MemoryType::Episodic);
        wm.push("c", 1.0, "st", MemoryType::Episodic);
        wm.push("d", 1.0, "st", MemoryType::Episodic);
        assert_eq!(wm.items.len(), 3);
    }

    #[test]
    fn test_access_bumps_count() {
        let mut wm = WorkingMemory {
            items: vec![],
            capacity: 7,
            decay_ms: 30000,
        };
        wm.push("test", 5.0, "st", MemoryType::Episodic);
        wm.access(0);
        assert_eq!(wm.items[0].access_count, 1);
    }

    #[test]
    fn test_decay_removes_old() {
        let mut wm = WorkingMemory {
            items: vec![],
            capacity: 7,
            decay_ms: 1,
        }; // 1ms decay
        wm.push("old", 5.0, "st", MemoryType::Episodic);
        std::thread::sleep(std::time::Duration::from_millis(5));
        wm.decay();
        assert!(wm.items.is_empty());
    }

    #[test]
    fn test_stats() {
        let mut wm = WorkingMemory {
            items: vec![],
            capacity: 5,
            decay_ms: 30000,
        };
        wm.push("a", 5.0, "st", MemoryType::Episodic);
        wm.push("b", 3.0, "lt", MemoryType::Semantic);
        let s = wm.stats();
        assert_eq!(s.item_count, 2);
        assert_eq!(s.capacity, 5);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = std::env::temp_dir().join("microscope_wm_test");
        let _ = std::fs::create_dir_all(&dir);
        let mut wm = WorkingMemory::load_or_init(&dir);
        wm.push("hello", 7.0, "short_term", MemoryType::Episodic);
        wm.save(&dir).unwrap();

        let loaded = WorkingMemory::load_or_init(&dir);
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].text, "hello");
        assert!((loaded.items[0].importance - 7.0).abs() < 0.01);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clear() {
        let mut wm = WorkingMemory {
            items: vec![],
            capacity: 7,
            decay_ms: 30000,
        };
        wm.push("x", 5.0, "st", MemoryType::Episodic);
        wm.clear();
        assert!(wm.items.is_empty());
    }
}
