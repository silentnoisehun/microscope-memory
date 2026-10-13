use std::fs::OpenOptions;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::{MmapMut, MmapOptions};
use parking_lot::Mutex;

pub const PAGE_SIZE: usize = 16 * 1024;
pub const HEADER_SIZE: usize = 64;
pub const MAGIC: u64 = 0x4D31_4353_5031_4E45;
pub const N_LAYERS: usize = 12;
pub const ATTENTION_LAYERS: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Slot {
    Superblock = 0, Stream = 1, Hebbian = 2, Attention = 3,
    Emotion = 4, Resonance = 5, Graph = 6, Cache = 7,
    Archetype = 8, Mirror = 9, Merkle = 10, Meta = 11, WalStart = 12,
}
impl Slot {
    pub fn offset(&self) -> usize { *self as usize * PAGE_SIZE }
    pub fn from_u32(v: u32) -> Option<Slot> {
        Some(match v { 0=>Slot::Superblock, 1=>Slot::Stream, 2=>Slot::Hebbian,
                     3=>Slot::Attention, 4=>Slot::Emotion, 5=>Slot::Resonance,
                     6=>Slot::Graph, 7=>Slot::Cache, 8=>Slot::Archetype,
                     9=>Slot::Mirror, 10=>Slot::Merkle, 11=>Slot::Meta,
                     12=>Slot::WalStart, _=>return None })
    }
}

// Header structs - simplified, page-aligned, repr(C, align(64))

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct StreamHeader {
    pub cycle: u64, pub surprise_level: f32, pub curiosity_level: f32,
    pub predicted_query_hash: u64, pub predicted_confidence: f32,
    pub last_query_hash: u64, pub last_query_ms: u64,
    pub hebbian_total_energy: f64, pub updated_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 60],
}
impl StreamHeader {
    pub const HEADER_BYTES: usize = 60;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u64::<LittleEndian>(self.cycle).unwrap();
        c.write_f32::<LittleEndian>(self.surprise_level).unwrap();
        c.write_f32::<LittleEndian>(self.curiosity_level).unwrap();
        c.write_u64::<LittleEndian>(self.predicted_query_hash).unwrap();
        c.write_f32::<LittleEndian>(self.predicted_confidence).unwrap();
        c.write_u64::<LittleEndian>(self.last_query_hash).unwrap();
        c.write_u64::<LittleEndian>(self.last_query_ms).unwrap();
        c.write_f64::<LittleEndian>(self.hebbian_total_energy).unwrap();
        c.write_u64::<LittleEndian>(self.updated_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self {
            cycle: c.read_u64::<LittleEndian>().unwrap_or(0),
            surprise_level: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            curiosity_level: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            predicted_query_hash: c.read_u64::<LittleEndian>().unwrap_or(0),
            predicted_confidence: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            last_query_hash: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_query_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            hebbian_total_energy: c.read_f64::<LittleEndian>().unwrap_or(0.0),
            updated_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for StreamHeader {
    fn default() -> Self {
        Self { cycle:0, surprise_level:0.0, curiosity_level:0.0, predicted_query_hash:0,
            predicted_confidence:0.0, last_query_hash:0, last_query_ms:0,
            hebbian_total_energy:0.0, updated_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct HebbianHeader {
    pub block_count: u32, pub activation_count: u32, pub coactivation_count: u32,
    pub fingerprint_count: u32, pub delta_tail_offset: u64, pub decay_event_count: u64,
    pub last_activation_ms: u64, pub _pad: [u8; PAGE_SIZE - 40],
}
impl HebbianHeader {
    pub const HEADER_BYTES: usize = 40;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.block_count).unwrap();
        c.write_u32::<LittleEndian>(self.activation_count).unwrap();
        c.write_u32::<LittleEndian>(self.coactivation_count).unwrap();
        c.write_u32::<LittleEndian>(self.fingerprint_count).unwrap();
        c.write_u64::<LittleEndian>(self.delta_tail_offset).unwrap();
        c.write_u64::<LittleEndian>(self.decay_event_count).unwrap();
        c.write_u64::<LittleEndian>(self.last_activation_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { block_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            activation_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            coactivation_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            fingerprint_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            delta_tail_offset: c.read_u64::<LittleEndian>().unwrap_or(0),
            decay_event_count: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_activation_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES] }
    }
}
impl Default for HebbianHeader {
    fn default() -> Self {
        Self { block_count:0, activation_count:0, coactivation_count:0, fingerprint_count:0,
            delta_tail_offset:0, decay_event_count:0, last_activation_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct AttentionHeader {
    pub learned_weights: [f32; ATTENTION_LAYERS], pub last_recall_ms: u64,
    pub total_recalls: u32, pub quality_signal: f32,
    pub _pad: [u8; PAGE_SIZE - (4 * ATTENTION_LAYERS + 16)],
}
impl AttentionHeader {
    pub const HEADER_BYTES: usize = 4 * ATTENTION_LAYERS + 16;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        for w in &self.learned_weights { c.write_f32::<LittleEndian>(*w).unwrap(); }
        c.write_u64::<LittleEndian>(self.last_recall_ms).unwrap();
        c.write_u32::<LittleEndian>(self.total_recalls).unwrap();
        c.write_f32::<LittleEndian>(self.quality_signal).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        let mut lw = [0.0f32; ATTENTION_LAYERS];
        for w in lw.iter_mut() { *w = c.read_f32::<LittleEndian>().unwrap_or(0.0); }
        Self { learned_weights: lw, last_recall_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            total_recalls: c.read_u32::<LittleEndian>().unwrap_or(0), quality_signal: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES] }
    }
}
impl Default for AttentionHeader {
    fn default() -> Self {
        Self { learned_weights:[1.0/ATTENTION_LAYERS as f32;ATTENTION_LAYERS],
            last_recall_ms:0, total_recalls:0, quality_signal:0.0,
            _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct EmotionHeader {
    pub current: [f32; 21], pub head: u32, pub count: u32,
    pub last_update: u64, pub total_updates: u64,
    pub _pad: [u8; PAGE_SIZE - 108],
}
impl EmotionHeader {
    pub const HEADER_BYTES: usize = 4 * 21 + 24;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        for v in &self.current { c.write_f32::<LittleEndian>(*v).unwrap(); }
        c.write_u32::<LittleEndian>(self.head).unwrap();
        c.write_u32::<LittleEndian>(self.count).unwrap();
        c.write_u64::<LittleEndian>(self.last_update).unwrap();
        c.write_u64::<LittleEndian>(self.total_updates).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        let mut cur = [0.0f32; 21];
        for v in cur.iter_mut() { *v = c.read_f32::<LittleEndian>().unwrap_or(0.0); }
        Self { current:cur, head: c.read_u32::<LittleEndian>().unwrap_or(0),
            count: c.read_u32::<LittleEndian>().unwrap_or(0), last_update: c.read_u64::<LittleEndian>().unwrap_or(0),
            total_updates: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for EmotionHeader {
    fn default() -> Self {
        Self { current:[0.0;21], head:0, count:0, last_update:0, total_updates:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct ResonanceHeader {
    pub instance_id: u64, pub field_cells: u32, pub outgoing_count: u32,
    pub incoming_count: u32, pub pulses_emitted: u64, pub pulses_received: u64,
    pub last_decay_ms: u64, pub _pad: [u8; PAGE_SIZE - 44],
}
impl ResonanceHeader {
    pub const HEADER_BYTES: usize = 44;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u64::<LittleEndian>(self.instance_id).unwrap();
        c.write_u32::<LittleEndian>(self.field_cells).unwrap();
        c.write_u32::<LittleEndian>(self.outgoing_count).unwrap();
        c.write_u32::<LittleEndian>(self.incoming_count).unwrap();
        c.write_u64::<LittleEndian>(self.pulses_emitted).unwrap();
        c.write_u64::<LittleEndian>(self.pulses_received).unwrap();
        c.write_u64::<LittleEndian>(self.last_decay_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { instance_id: c.read_u64::<LittleEndian>().unwrap_or(0),
            field_cells: c.read_u32::<LittleEndian>().unwrap_or(0),
            outgoing_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            incoming_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            pulses_emitted: c.read_u64::<LittleEndian>().unwrap_or(0),
            pulses_received: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_decay_ms: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for ResonanceHeader {
    fn default() -> Self {
        Self { instance_id:0, field_cells:0, outgoing_count:0, incoming_count:0,
            pulses_emitted:0, pulses_received:0, last_decay_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct GraphHeader {
    pub node_count: u32, pub edge_count: u32, pub pattern_count: u32,
    pub current_session_id: u32, pub next_pattern_id: u32, pub last_node_ts: u64,
    pub _pad: [u8; PAGE_SIZE - 28],
}
impl GraphHeader {
    pub const HEADER_BYTES: usize = 28;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.node_count).unwrap();
        c.write_u32::<LittleEndian>(self.edge_count).unwrap();
        c.write_u32::<LittleEndian>(self.pattern_count).unwrap();
        c.write_u32::<LittleEndian>(self.current_session_id).unwrap();
        c.write_u32::<LittleEndian>(self.next_pattern_id).unwrap();
        c.write_u64::<LittleEndian>(self.last_node_ts).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { node_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            edge_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            pattern_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            current_session_id: c.read_u32::<LittleEndian>().unwrap_or(0),
            next_pattern_id: c.read_u32::<LittleEndian>().unwrap_or(1),
            last_node_ts: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for GraphHeader {
    fn default() -> Self {
        Self { node_count:0, edge_count:0, pattern_count:0, current_session_id:0,
            next_pattern_id:1, last_node_ts:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct CacheHeader {
    pub total_predictions: u32, pub total_hits: u32, pub total_misses: u32,
    pub total_partial_hits: u32, pub current_predictions: u32, pub avg_confidence: f32,
    pub last_predict_ms: u64, pub _pad: [u8; PAGE_SIZE - 32],
}
impl CacheHeader {
    pub const HEADER_BYTES: usize = 32;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.total_predictions).unwrap();
        c.write_u32::<LittleEndian>(self.total_hits).unwrap();
        c.write_u32::<LittleEndian>(self.total_misses).unwrap();
        c.write_u32::<LittleEndian>(self.total_partial_hits).unwrap();
        c.write_u32::<LittleEndian>(self.current_predictions).unwrap();
        c.write_f32::<LittleEndian>(self.avg_confidence).unwrap();
        c.write_u64::<LittleEndian>(self.last_predict_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { total_predictions: c.read_u32::<LittleEndian>().unwrap_or(0),
            total_hits: c.read_u32::<LittleEndian>().unwrap_or(0),
            total_misses: c.read_u32::<LittleEndian>().unwrap_or(0),
            total_partial_hits: c.read_u32::<LittleEndian>().unwrap_or(0),
            current_predictions: c.read_u32::<LittleEndian>().unwrap_or(0),
            avg_confidence: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            last_predict_ms: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
    pub fn hit_rate(&self) -> f32 {
        let total = self.total_hits + self.total_misses + self.total_partial_hits;
        if total == 0 { return 0.0; }
        self.total_hits as f32 / total as f32
    }
}
impl Default for CacheHeader {
    fn default() -> Self {
        Self { total_predictions:0, total_hits:0, total_misses:0, total_partial_hits:0,
            current_predictions:0, avg_confidence:0.0, last_predict_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct ArchetypeHeader {
    pub active_count: u32, pub next_id: u32, pub emergence_events: u64,
    pub decay_events: u64, pub last_emergence_ms: u64, pub _pad: [u8; PAGE_SIZE - 32],
}
impl ArchetypeHeader {
    pub const HEADER_BYTES: usize = 32;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.active_count).unwrap();
        c.write_u32::<LittleEndian>(self.next_id).unwrap();
        c.write_u64::<LittleEndian>(self.emergence_events).unwrap();
        c.write_u64::<LittleEndian>(self.decay_events).unwrap();
        c.write_u64::<LittleEndian>(self.last_emergence_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { active_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            next_id: c.read_u32::<LittleEndian>().unwrap_or(1),
            emergence_events: c.read_u64::<LittleEndian>().unwrap_or(0),
            decay_events: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_emergence_ms: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for ArchetypeHeader {
    fn default() -> Self {
        Self { active_count:0, next_id:1, emergence_events:0, decay_events:0,
            last_emergence_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct MirrorHeader {
    pub echo_count: u32, pub block_resonance_count: u32, pub strongest_similarity: f32,
    pub total_echoes: u64, pub last_mirror_ms: u64, pub _pad: [u8; PAGE_SIZE - 28],
}
impl MirrorHeader {
    pub const HEADER_BYTES: usize = 28;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.echo_count).unwrap();
        c.write_u32::<LittleEndian>(self.block_resonance_count).unwrap();
        c.write_f32::<LittleEndian>(self.strongest_similarity).unwrap();
        c.write_u64::<LittleEndian>(self.total_echoes).unwrap();
        c.write_u64::<LittleEndian>(self.last_mirror_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { echo_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            block_resonance_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            strongest_similarity: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            total_echoes: c.read_u64::<LittleEndian>().unwrap_or(0), last_mirror_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for MirrorHeader {
    fn default() -> Self {
        Self { echo_count:0, block_resonance_count:0, strongest_similarity:0.0,
            total_echoes:0, last_mirror_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct MerkleHeader {
    pub root_hash: u64, pub chain_height: u64, pub block_count: u32,
    pub evidence_audit_entries: u32, pub last_anchor_ms: u64, pub _pad: [u8; PAGE_SIZE - 32],
}
impl MerkleHeader {
    pub const HEADER_BYTES: usize = 32;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u64::<LittleEndian>(self.root_hash).unwrap();
        c.write_u64::<LittleEndian>(self.chain_height).unwrap();
        c.write_u32::<LittleEndian>(self.block_count).unwrap();
        c.write_u32::<LittleEndian>(self.evidence_audit_entries).unwrap();
        c.write_u64::<LittleEndian>(self.last_anchor_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { root_hash: c.read_u64::<LittleEndian>().unwrap_or(0),
            chain_height: c.read_u64::<LittleEndian>().unwrap_or(0),
            block_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            evidence_audit_entries: c.read_u32::<LittleEndian>().unwrap_or(0),
            last_anchor_ms: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for MerkleHeader {
    fn default() -> Self {
        Self { root_hash:0, chain_height:0, block_count:0, evidence_audit_entries:0,
            last_anchor_ms:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct MetaHeader {
    pub block_count: u32, pub append_log_len: u32, pub dream_cycle_count: u32,
    pub reps_served_count: u32, pub last_dream_ms: u64, pub promotion_count: u64,
    pub _pad: [u8; PAGE_SIZE - 32],
}
impl MetaHeader {
    pub const HEADER_BYTES: usize = 32;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.block_count).unwrap();
        c.write_u32::<LittleEndian>(self.append_log_len).unwrap();
        c.write_u32::<LittleEndian>(self.dream_cycle_count).unwrap();
        c.write_u32::<LittleEndian>(self.reps_served_count).unwrap();
        c.write_u64::<LittleEndian>(self.last_dream_ms).unwrap();
        c.write_u64::<LittleEndian>(self.promotion_count).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self { block_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            append_log_len: c.read_u32::<LittleEndian>().unwrap_or(0),
            dream_cycle_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            reps_served_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            last_dream_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            promotion_count: c.read_u64::<LittleEndian>().unwrap_or(0), _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}
impl Default for MetaHeader {
    fn default() -> Self {
        Self { block_count:0, append_log_len:0, dream_cycle_count:0, reps_served_count:0,
            last_dream_ms:0, promotion_count:0, _pad:[0;PAGE_SIZE-Self::HEADER_BYTES] }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Superblock {
    pub magic: u64, pub version: u32, pub page_size: u32,
    pub wal_start_page: u32, pub wal_head_page: u32, pub epoch_ms: i64,
}
impl Default for Superblock { fn default() -> Self { Self::ZEROED } }
impl Superblock {
    const ZEROED: Self = Self { magic:0, version:1, page_size:0, wal_start_page:0, wal_head_page:0, epoch_ms:0 };
    pub fn new() -> Self {
        Self { magic:MAGIC, version:1, page_size:PAGE_SIZE as u32,
            wal_start_page:Slot::WalStart as u32, wal_head_page:Slot::WalStart as u32,
            epoch_ms:now_ms() }
    }
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        let mut c = Cursor::new(&mut buf[..36]);
        c.write_u64::<LittleEndian>(self.magic).unwrap();
        c.write_u32::<LittleEndian>(self.version).unwrap();
        c.write_u32::<LittleEndian>(self.page_size).unwrap();
        c.write_u32::<LittleEndian>(self.wal_start_page).unwrap();
        c.write_u32::<LittleEndian>(self.wal_head_page).unwrap();
        c.write_i64::<LittleEndian>(self.epoch_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        let mut c = Cursor::new(buf);
        let magic = c.read_u64::<LittleEndian>().ok()?;
        if magic != MAGIC { return None; }
        Some(Self { magic, version: c.read_u32::<LittleEndian>().ok()?,
            page_size: c.read_u32::<LittleEndian>().ok()?,
            wal_start_page: c.read_u32::<LittleEndian>().ok()?,
            wal_head_page: c.read_u32::<LittleEndian>().ok()?,
            epoch_ms: c.read_i64::<LittleEndian>().ok()? })
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn update_superblock_epoch(mmap: &mut MmapMut, epoch_ms: i64) {
    let mut sb = Superblock::from_bytes(&mmap[0..HEADER_SIZE]).unwrap();
    sb.epoch_ms = epoch_ms;
    mmap[0..HEADER_SIZE].copy_from_slice(&sb.to_bytes());
}

pub struct Spine {
    output_dir: PathBuf,
    mmap: Arc<Mutex<MmapMut>>,
}

impl Spine {
    pub fn open(output_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = output_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("spine.bin");
        let file = OpenOptions::new().read(true).write(true).truncate(false).create(true).open(&path)?;
        let min_len = (Slot::WalStart as usize + 8) * PAGE_SIZE;
        let file_len = file.metadata()?.len();
        if file_len < min_len as u64 { file.set_len(min_len as u64)?; }
        let mmap = unsafe { MmapOptions::new().map_mut(&file).map_err(std::io::Error::other)? };
        // Cross-process writer coordination: mmap shared pages make epoch visible.
        // No explicit flock; epoch Release/Acquire is the protocol.
        let _ = file;
        let spine = Self { output_dir:dir, mmap:Arc::new(Mutex::new(mmap)) };
        {
            let mut m = spine.mmap.lock();
            if Superblock::from_bytes(&m[0..HEADER_SIZE]).is_none() {
                let sb = Superblock::new();
                let bytes = sb.to_bytes();
                m[0..HEADER_SIZE].copy_from_slice(&bytes);
            }
        }
        Ok(spine)
    }

    fn read_slot(&self, slot: Slot) -> [u8; PAGE_SIZE] {
        let off = slot.offset();
        let m = self.mmap.lock();
        let mut buf = [0u8; PAGE_SIZE];
        let end = (off + PAGE_SIZE).min(m.len());
        if off < m.len() { buf[..end-off].copy_from_slice(&m[off..end]); }
        buf
    }

    pub fn write_slot(&self, slot: Slot, bytes: &[u8; PAGE_SIZE]) {
        let off = slot.offset();
        let mut m = self.mmap.lock();
        if off + PAGE_SIZE <= m.len() { m[off..off+PAGE_SIZE].copy_from_slice(bytes); }
    }

    pub fn read_stream(&self) -> StreamHeader { StreamHeader::from_bytes(&self.read_slot(Slot::Stream)) }
    pub fn write_stream(&self, h: &StreamHeader) { self.write_slot(Slot::Stream, &h.to_bytes()); }

    #[allow(clippy::too_many_arguments)]
pub fn tick(&self, cycle: u64, surprise: f32, curiosity: f32,
        predicted_hash: u64, predicted_confidence: f32,
        last_query_hash: u64, last_query_ms: u64,
        hebbian_total_energy: f64) {
        let h = StreamHeader {
            cycle, surprise_level:surprise, curiosity_level:curiosity,
            predicted_query_hash:predicted_hash, predicted_confidence,
            last_query_hash, last_query_ms,
            hebbian_total_energy, updated_ms:now_ms() as u64,
            _pad:[0;PAGE_SIZE-StreamHeader::HEADER_BYTES],
        };
        self.write_stream(&h);
        let epoch_ms = now_ms();
        let mut m = self.mmap.lock();
        update_superblock_epoch(&mut m, epoch_ms);
        drop(m); // Release ordering via drop
    }

    pub fn read_hebbian(&self) -> HebbianHeader { HebbianHeader::from_bytes(&self.read_slot(Slot::Hebbian)) }
    pub fn write_hebbian(&self, h: &HebbianHeader) { self.write_slot(Slot::Hebbian, &h.to_bytes()); }
    pub fn read_attention(&self) -> AttentionHeader { AttentionHeader::from_bytes(&self.read_slot(Slot::Attention)) }
    pub fn write_attention(&self, h: &AttentionHeader) { self.write_slot(Slot::Attention, &h.to_bytes()); }
    pub fn read_emotion(&self) -> EmotionHeader { EmotionHeader::from_bytes(&self.read_slot(Slot::Emotion)) }
    pub fn write_emotion(&self, h: &EmotionHeader) { self.write_slot(Slot::Emotion, &h.to_bytes()); }
    pub fn read_resonance(&self) -> ResonanceHeader { ResonanceHeader::from_bytes(&self.read_slot(Slot::Resonance)) }
    pub fn write_resonance(&self, h: &ResonanceHeader) { self.write_slot(Slot::Resonance, &h.to_bytes()); }
    pub fn read_graph(&self) -> GraphHeader { GraphHeader::from_bytes(&self.read_slot(Slot::Graph)) }
    pub fn write_graph(&self, h: &GraphHeader) { self.write_slot(Slot::Graph, &h.to_bytes()); }
    pub fn read_cache(&self) -> CacheHeader { CacheHeader::from_bytes(&self.read_slot(Slot::Cache)) }
    pub fn write_cache(&self, h: &CacheHeader) { self.write_slot(Slot::Cache, &h.to_bytes()); }
    pub fn read_archetype(&self) -> ArchetypeHeader { ArchetypeHeader::from_bytes(&self.read_slot(Slot::Archetype)) }
    pub fn write_archetype(&self, h: &ArchetypeHeader) { self.write_slot(Slot::Archetype, &h.to_bytes()); }
    pub fn read_mirror(&self) -> MirrorHeader { MirrorHeader::from_bytes(&self.read_slot(Slot::Mirror)) }
    pub fn write_mirror(&self, h: &MirrorHeader) { self.write_slot(Slot::Mirror, &h.to_bytes()); }
    pub fn read_merkle(&self) -> MerkleHeader { MerkleHeader::from_bytes(&self.read_slot(Slot::Merkle)) }
    pub fn write_merkle(&self, h: &MerkleHeader) { self.write_slot(Slot::Merkle, &h.to_bytes()); }
    pub fn read_meta(&self) -> MetaHeader { MetaHeader::from_bytes(&self.read_slot(Slot::Meta)) }
    pub fn write_meta(&self, h: &MetaHeader) { self.write_slot(Slot::Meta, &h.to_bytes()); }
    pub fn read_superblock(&self) -> Option<Superblock> { let m = self.mmap.lock(); Superblock::from_bytes(&m[0..HEADER_SIZE]) }

    pub fn sync(&self) -> std::io::Result<()> { self.mmap.lock().flush() }
    pub fn size_bytes(&self) -> usize { self.mmap.lock().len() }
    pub fn output_dir(&self) -> &Path { &self.output_dir }
    pub fn path(&self) -> PathBuf { self.output_dir.join("spine.bin") }
    pub fn resize(&self, new_len: usize) -> std::io::Result<()> {
        {
            let f = OpenOptions::new().read(true).write(true).truncate(false).open(self.path())?;
            f.set_len(new_len as u64)?;
        }
        let file = OpenOptions::new().read(true).write(true).truncate(false).open(self.path())?;
        let new_mmap = unsafe { MmapOptions::new().map_mut(&file).map_err(std::io::Error::other)? };
        *self.mmap.lock() = new_mmap;
        Ok(())
    }
}

pub fn format_summary(spine: &Spine) -> String {
    let s = spine.read_stream();
    let h = spine.read_hebbian();
    let a = spine.read_attention();
    let e = spine.read_emotion();
    let r = spine.read_resonance();
    let g = spine.read_graph();
    let c = spine.read_cache();
    let ar = spine.read_archetype();
    let mi = spine.read_mirror();
    let me = spine.read_merkle();
    let meta = spine.read_meta();
    format!("Spine: {} pages mapped ({} KB)\n\
        Stream:   cycle={} surprise={:.3} curiosity={:.3} predict_conf={:.3}\n\
        Hebbian:  {} activations, {} coactivations, delta_tail={}\n\
        Attention: {} recalls, quality={:.2}\n\
        Emotion:  {} updates, dominant_idx={}\n\
        Resonance: {} cells, {} emitted, {} received\n\
        Graph:    {} nodes, {} edges, {} patterns\n\
        Cache:    {} predictions, hit_rate={:.1}%\n\
        Archetype: {} active, {} emerged\n\
        Mirror:   {} echoes, strongest={:.3}\n\
        Merkle:   height={}, {} blocks\n\
        Meta:     {} blocks, {} dream cycles",
        spine.size_bytes()/PAGE_SIZE, spine.size_bytes()/1024,
        s.cycle, s.surprise_level, s.curiosity_level, s.predicted_confidence,
        h.activation_count, h.coactivation_count, h.delta_tail_offset,
        a.total_recalls, a.quality_signal,
        e.total_updates, e.current[0],
        r.field_cells, r.pulses_emitted, r.pulses_received,
        g.node_count, g.edge_count, g.pattern_count,
        c.total_predictions, c.hit_rate()*100.0,
        ar.active_count, ar.emergence_events,
        mi.echo_count, mi.strongest_similarity,
        me.chain_height, me.block_count,
        meta.block_count, meta.dream_cycle_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_spine(tag: &str) -> Spine {
        let dir = std::env::temp_dir().join(format!("m1c_spine_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Spine::open(&dir).unwrap()
    }
    #[test] fn test_open_creates_superblock() { let spine=temp_spine("sb"); let sb=spine.read_superblock().unwrap(); assert_eq!(sb.magic,MAGIC); assert_eq!(sb.page_size as usize,PAGE_SIZE); }
    #[test] fn test_stream_roundtrip() { let spine=temp_spine("stream"); spine.tick(42,0.5,0.7,0xdeadbeef,0.9,0xcafebabe,12345,1337.5); let s=spine.read_stream(); assert_eq!(s.cycle,42); assert!((s.surprise_level-0.5).abs()<1e-6); assert_eq!(s.predicted_query_hash,0xdeadbeef); assert!((s.hebbian_total_energy-1337.5).abs()<1e-6); assert!(s.updated_ms>0); }
    #[test] fn test_layer_headers_roundtrip() { let spine=temp_spine("layers"); let mut h=spine.read_hebbian(); h.activation_count=999; h.delta_tail_offset=123456789; h.decay_event_count=0; spine.write_hebbian(&h); assert_eq!(spine.read_hebbian().activation_count,999); assert_eq!(spine.read_hebbian().delta_tail_offset,123456789); let mut a=spine.read_attention(); a.learned_weights[0]=0.25; a.total_recalls=17; spine.write_attention(&a); assert!((spine.read_attention().learned_weights[0]-0.25).abs()<1e-6); assert_eq!(spine.read_attention().total_recalls,17); let mut e=spine.read_emotion(); e.current[3]=0.8; e.total_updates=100; spine.write_emotion(&e); assert!((spine.read_emotion().current[3]-0.8).abs()<1e-6); let mut g=spine.read_graph(); g.node_count=500; spine.write_graph(&g); assert_eq!(spine.read_graph().node_count,500); let mut c=spine.read_cache(); c.total_hits=10; c.total_misses=2; spine.write_cache(&c); let rc=spine.read_cache(); assert!((rc.hit_rate()-10.0/12.0).abs()<1e-5); }
    #[test] fn test_all_slots_distinct() { let spine=temp_spine("slots"); spine.tick(1,1.0,1.0,1,1.0,1,1,1.0); let mut g=spine.read_graph(); g.node_count=42; spine.write_graph(&g); let mut m=spine.read_mirror(); m.echo_count=7; spine.write_mirror(&m); assert_eq!(spine.read_stream().cycle,1); assert_eq!(spine.read_graph().node_count,42); assert_eq!(spine.read_mirror().echo_count,7); assert_eq!(spine.read_archetype().active_count,0); }
    #[test] fn test_reopen_preserves_all_layers() { let dir=std::env::temp_dir().join(format!("m1c_spine_reopen_{}",std::process::id())); let _=std::fs::remove_dir_all(&dir); { let spine=Spine::open(&dir).unwrap(); spine.tick(7,0.3,0.4,5,6.0,7,8,9.0); let mut a=spine.read_attention(); a.total_recalls=55; spine.write_attention(&a); spine.sync().unwrap(); } let spine=Spine::open(&dir).unwrap(); assert_eq!(spine.read_stream().cycle,7); assert!((spine.read_stream().surprise_level-0.3).abs()<1e-6); assert_eq!(spine.read_attention().total_recalls,55); }
    #[test] fn test_summary_formats() { let spine=temp_spine("summary"); spine.tick(10,0.5,0.6,0,0.7,0,0,100.0); let out=format_summary(&spine); assert!(out.contains("Spine:")); assert!(out.contains("cycle=10")); }
}