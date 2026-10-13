//! The Spine: every layer header of the stream in one shared mmap file.
//!
//! `spine.bin` is `Slot::WalStart + 8` pages of 16 KiB. Slot 0 carries the
//! superblock, slots 1..=11 carry one header page per layer, slot 12 opens
//! the write-ahead region. A process that owns the stream writes a header
//! when its aggregates change; the `MCP`/CLI readers and the live-viewer
//! process read those pages without going through the writer at all.
//!
//! # The publish protocol, in one paragraph
//!
//! A page write is a 16 KiB `copy_from_slice`. It is **not** an atomic
//! operation, and `Mutex<MmapMut>` only serialises handles of **one**
//! process: two processes mapping the same file have two mutexes, so they
//! have no happens-before relation and a reader can copy a half-written page
//! or pair a new stream header with an older one. That tearing is what the
//! seqlock in this file exists to remove, and nothing else here claims to.
//!
//! The superblock page reserves [`SEQ_SLOT`] as its publish counter, exactly
//! the way [`crate::consciousness_seqlock::MmapSnapshot`] reserves
//! `sequence: AtomicU64` at offset 0 of its own file. A publisher makes the
//! counter odd, writes the bracketed payload, then stores the next even
//! value with `Ordering::Release`; a reader loads with `Ordering::Acquire`,
//! spins while odd, copies, and retries if the counter moved. The reader
//! therefore never returns a payload from inside a publish window, and it
//! does not need the mutex to find that out.
//!
//! # What the counter does and does not cover
//!
//! It brackets the **publish payload** — [`Slot::Superblock`] bytes
//! 0..`SUPERBLOCK_BYTES` and [`Slot::Stream`]. That is the pair a reader
//! compares, so it is the pair that has to agree, and [`Spine::read_snapshot`]
//! is the only read that is told whether it does.
//!
//! The nine other layer headers go through [`Spine::write_slot`]. Its writes
//! exclude each other (mapping mutex plus the publisher lock), but they are
//! not inside a window and their pages are not covered by the counter, so a
//! reader in another process can still copy one of those pages mid-write and
//! get a mix of two publishes of *that* header. Those pages are aggregates of
//! files that are durable by themselves, so the cost is freshness, not
//! correctness — and `read_slot` does not claim otherwise. A future multi-slot
//! seqlock would have to bracket the whole write set and give readers a
//! validated read per slot.
//!
//! # Who is allowed to be inside a window
//!
//! A seqlock only helps if at most one writer is in a window at a time; two
//! overlapping windows can leave a reader validating a counter that neither
//! writer owns. So publishing also takes `flock(LOCK_EX)` on `spine.publish`
//! (see [`Spine::publish_guard`]), which is the piece shared pages cannot
//! provide: mutual exclusion between publishers in different processes.
//!
//! A second lock file answers a different question. `spine.lock` is held
//! *shared* for the whole life of a writer handle, and exclusively by
//! first-time layout and by [`Spine::resize`], because remapping the file
//! invalidates the pages every other handle is reading. It cannot double as
//! the publish lock: a handle holding it shared would deadlock against itself
//! trying to take it exclusively for one tick.
//!
//! `flock` is advisory, is not available on Windows, and does not exist on
//! every filesystem; all of those degrade to "publishing is serialised per
//! process only". That is a real weakening of the guarantee, so it is
//! reported by [`Spine::publish_lock_is_advisory`] instead of being hidden.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::{MmapMut, MmapOptions};
use parking_lot::Mutex;
use std::fs::OpenOptions;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub const PAGE_SIZE: usize = 16 * 1024;
pub const HEADER_SIZE: usize = 64;
pub const MAGIC: u64 = 0x4D31_4353_5031_4E45;
pub const N_LAYERS: usize = 12;
pub const ATTENTION_LAYERS: usize = 7;

/// Superblock format version.
///
/// Version 1 was `magic, version, page_size, wal_start_page, wal_head_page,
/// epoch_ms` — 32 bytes, timestamp last, no publish counter. Version 2 keeps
/// the first five fields exactly where they were, inserts `tick_seq` at
/// bytes 24..32 and moves `epoch_ms` to 32..40, padding the payload out to
/// [`SUPERBLOCK_BYTES`]. That is why [`Spine::open`] upgrades in place: a
/// version-1 file decoded as version 2 would read a millisecond timestamp as
/// a sequence number.
pub const SUPERBLOCK_VERSION: u32 = 2;
/// Bytes of the superblock page that are part of the published payload: the
/// fixed-size header fields, padded out so the payload ends where the counter
/// begins. [`Spine::write_slot`] accepts a whole page for this slot but
/// publishes only these bytes, precisely so that a page-sized write cannot
/// reach into `SEQ_SLOT` and reset the counter under a reader.
pub const SUPERBLOCK_BYTES: usize = 56;
/// Byte offset of the seqlock publish counter inside the superblock **page**
/// (page 0, so this is also the file offset). 8-byte aligned by construction
/// because mappings start on a page boundary.
pub const SEQ_SLOT: usize = 56;
const _: () = assert!(
    SEQ_SLOT.is_multiple_of(8),
    "the counter slot must stay 8-byte aligned"
);
const _: () = assert!(
    SEQ_SLOT >= SUPERBLOCK_BYTES,
    "the payload must end before the counter"
);
/// Bytes of the stream page a snapshot read copies. [`StreamHeader`] is
/// decoded from its first 60 bytes, so the read brackets one cache line
/// instead of 16 KiB — which is the difference between a reader retrying once
/// in a blue moon and retrying on half its attempts against a writer running
/// a tight loop.
pub const SNAPSHOT_PREFIX_BYTES: usize = 64;
/// Read attempts on a publish window that keeps moving under us before the
/// reader gives up and reports `None`. Spins for the first 64, then yields,
/// so the budget is a measure of *patience* rather than of CPU burned
/// competing with the writer. It is large because an attempt costs a lock
/// hand-over when a reader shares its handle with other readers: measured
/// against three threads hammering one file, a 1024-attempt budget gave up on
/// 0.09% of reads even though the writer's window is never longer than a
/// microsecond, purely from losing the lock. At this budget it gives up on
/// none.
pub const SNAPSHOT_MAX_RETRIES: u32 = 65_536;
/// ...and the shortest time it will keep trying, in milliseconds. An attempt
/// is cheap, so on an oversubscribed machine the count can run out while the
/// reader has merely been off-CPU; the deadline is what distinguishes "the
/// writer is relentless" from "I was not scheduled". Measured on a 2-core box
/// with three load tests and a spawned writer running at once: count alone
/// gave up on about 1 read in 60 000, this gives up on none.
pub const SNAPSHOT_MAX_WAIT_MS: u64 = 50;
/// Name of the file whose advisory lock serialises publishers.
pub const PUBLISH_LOCK_FILE_NAME: &str = "spine.publish";
/// Name of the file whose advisory lock marks live writers for `resize`.
pub const WRITER_LOCK_FILE_NAME: &str = "spine.lock";

const _: () = assert!(SEQ_SLOT + 8 <= PAGE_SIZE);
const _: () = assert!(SNAPSHOT_PREFIX_BYTES >= StreamHeader::HEADER_BYTES);
const _: () = assert!(SNAPSHOT_PREFIX_BYTES <= PAGE_SIZE);
const _: () = assert!(SUPERBLOCK_BYTES + 8 <= PAGE_SIZE);
const _: () = assert!(HEADER_SIZE == 64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Slot {
    Superblock = 0,
    Stream = 1,
    Hebbian = 2,
    Attention = 3,
    Emotion = 4,
    Resonance = 5,
    Graph = 6,
    Cache = 7,
    Archetype = 8,
    Mirror = 9,
    Merkle = 10,
    Meta = 11,
    WalStart = 12,
}
impl Slot {
    pub fn offset(&self) -> usize {
        *self as usize * PAGE_SIZE
    }
    pub fn from_u32(v: u32) -> Option<Slot> {
        Some(match v {
            0 => Slot::Superblock,
            1 => Slot::Stream,
            2 => Slot::Hebbian,
            3 => Slot::Attention,
            4 => Slot::Emotion,
            5 => Slot::Resonance,
            6 => Slot::Graph,
            7 => Slot::Cache,
            8 => Slot::Archetype,
            9 => Slot::Mirror,
            10 => Slot::Merkle,
            11 => Slot::Meta,
            12 => Slot::WalStart,
            _ => return None,
        })
    }
}

// Header structs - simplified, page-aligned, repr(C, align(64))

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct StreamHeader {
    pub cycle: u64,
    pub surprise_level: f32,
    pub curiosity_level: f32,
    pub predicted_query_hash: u64,
    pub predicted_confidence: f32,
    pub last_query_hash: u64,
    pub last_query_ms: u64,
    pub hebbian_total_energy: f64,
    pub updated_ms: u64,
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
        c.write_u64::<LittleEndian>(self.predicted_query_hash)
            .unwrap();
        c.write_f32::<LittleEndian>(self.predicted_confidence)
            .unwrap();
        c.write_u64::<LittleEndian>(self.last_query_hash).unwrap();
        c.write_u64::<LittleEndian>(self.last_query_ms).unwrap();
        c.write_f64::<LittleEndian>(self.hebbian_total_energy)
            .unwrap();
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
        Self {
            cycle: 0,
            surprise_level: 0.0,
            curiosity_level: 0.0,
            predicted_query_hash: 0,
            predicted_confidence: 0.0,
            last_query_hash: 0,
            last_query_ms: 0,
            hebbian_total_energy: 0.0,
            updated_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct HebbianHeader {
    pub block_count: u32,
    pub activation_count: u32,
    pub coactivation_count: u32,
    pub fingerprint_count: u32,
    pub delta_tail_offset: u64,
    /// Count of hebbian energy-decay events. **There is no producer for this
    /// field today.** `HebbianState::apply_drift` decays energies in place and
    /// keeps no count, and nothing in the crate reads the field back; until a
    /// producer exists, the value here is whatever the file happens to hold.
    /// `publish_spine` used to do `decay_event_count += cycle`, which grew the
    /// field as a sum of cycle numbers — quadratically, and unrelated to any
    /// decay that had happened. That write is gone, which is a behaviour change
    /// only for a reader that trusted a number with no source.
    pub decay_event_count: u64,
    pub last_activation_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 40],
}
impl HebbianHeader {
    pub const HEADER_BYTES: usize = 40;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.block_count).unwrap();
        c.write_u32::<LittleEndian>(self.activation_count).unwrap();
        c.write_u32::<LittleEndian>(self.coactivation_count)
            .unwrap();
        c.write_u32::<LittleEndian>(self.fingerprint_count).unwrap();
        c.write_u64::<LittleEndian>(self.delta_tail_offset).unwrap();
        c.write_u64::<LittleEndian>(self.decay_event_count).unwrap();
        c.write_u64::<LittleEndian>(self.last_activation_ms)
            .unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self {
            block_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            activation_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            coactivation_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            fingerprint_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            delta_tail_offset: c.read_u64::<LittleEndian>().unwrap_or(0),
            decay_event_count: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_activation_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for HebbianHeader {
    fn default() -> Self {
        Self {
            block_count: 0,
            activation_count: 0,
            coactivation_count: 0,
            fingerprint_count: 0,
            delta_tail_offset: 0,
            decay_event_count: 0,
            last_activation_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct AttentionHeader {
    pub learned_weights: [f32; ATTENTION_LAYERS],
    pub last_recall_ms: u64,
    pub total_recalls: u32,
    pub quality_signal: f32,
    pub _pad: [u8; PAGE_SIZE - (4 * ATTENTION_LAYERS + 16)],
}
impl AttentionHeader {
    pub const HEADER_BYTES: usize = 4 * ATTENTION_LAYERS + 16;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        for w in &self.learned_weights {
            c.write_f32::<LittleEndian>(*w).unwrap();
        }
        c.write_u64::<LittleEndian>(self.last_recall_ms).unwrap();
        c.write_u32::<LittleEndian>(self.total_recalls).unwrap();
        c.write_f32::<LittleEndian>(self.quality_signal).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        let mut lw = [0.0f32; ATTENTION_LAYERS];
        for w in lw.iter_mut() {
            *w = c.read_f32::<LittleEndian>().unwrap_or(0.0);
        }
        Self {
            learned_weights: lw,
            last_recall_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            total_recalls: c.read_u32::<LittleEndian>().unwrap_or(0),
            quality_signal: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for AttentionHeader {
    fn default() -> Self {
        Self {
            learned_weights: [1.0 / ATTENTION_LAYERS as f32; ATTENTION_LAYERS],
            last_recall_ms: 0,
            total_recalls: 0,
            quality_signal: 0.0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct EmotionHeader {
    pub current: [f32; 21],
    pub head: u32,
    pub count: u32,
    pub last_update: u64,
    pub total_updates: u64,
    pub _pad: [u8; PAGE_SIZE - 108],
}
impl EmotionHeader {
    pub const HEADER_BYTES: usize = 4 * 21 + 24;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        for v in &self.current {
            c.write_f32::<LittleEndian>(*v).unwrap();
        }
        c.write_u32::<LittleEndian>(self.head).unwrap();
        c.write_u32::<LittleEndian>(self.count).unwrap();
        c.write_u64::<LittleEndian>(self.last_update).unwrap();
        c.write_u64::<LittleEndian>(self.total_updates).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        let mut cur = [0.0f32; 21];
        for v in cur.iter_mut() {
            *v = c.read_f32::<LittleEndian>().unwrap_or(0.0);
        }
        Self {
            current: cur,
            head: c.read_u32::<LittleEndian>().unwrap_or(0),
            count: c.read_u32::<LittleEndian>().unwrap_or(0),
            last_update: c.read_u64::<LittleEndian>().unwrap_or(0),
            total_updates: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for EmotionHeader {
    fn default() -> Self {
        Self {
            current: [0.0; 21],
            head: 0,
            count: 0,
            last_update: 0,
            total_updates: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct ResonanceHeader {
    pub instance_id: u64,
    pub field_cells: u32,
    pub outgoing_count: u32,
    pub incoming_count: u32,
    pub pulses_emitted: u64,
    pub pulses_received: u64,
    pub last_decay_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 44],
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
        Self {
            instance_id: c.read_u64::<LittleEndian>().unwrap_or(0),
            field_cells: c.read_u32::<LittleEndian>().unwrap_or(0),
            outgoing_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            incoming_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            pulses_emitted: c.read_u64::<LittleEndian>().unwrap_or(0),
            pulses_received: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_decay_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for ResonanceHeader {
    fn default() -> Self {
        Self {
            instance_id: 0,
            field_cells: 0,
            outgoing_count: 0,
            incoming_count: 0,
            pulses_emitted: 0,
            pulses_received: 0,
            last_decay_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct GraphHeader {
    pub node_count: u32,
    pub edge_count: u32,
    pub pattern_count: u32,
    pub current_session_id: u32,
    pub next_pattern_id: u32,
    pub last_node_ts: u64,
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
        c.write_u32::<LittleEndian>(self.current_session_id)
            .unwrap();
        c.write_u32::<LittleEndian>(self.next_pattern_id).unwrap();
        c.write_u64::<LittleEndian>(self.last_node_ts).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self {
            node_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            edge_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            pattern_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            current_session_id: c.read_u32::<LittleEndian>().unwrap_or(0),
            next_pattern_id: c.read_u32::<LittleEndian>().unwrap_or(1),
            last_node_ts: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for GraphHeader {
    fn default() -> Self {
        Self {
            node_count: 0,
            edge_count: 0,
            pattern_count: 0,
            current_session_id: 0,
            next_pattern_id: 1,
            last_node_ts: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct CacheHeader {
    pub total_predictions: u32,
    pub total_hits: u32,
    pub total_misses: u32,
    pub total_partial_hits: u32,
    pub current_predictions: u32,
    pub avg_confidence: f32,
    pub last_predict_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 32],
}
impl CacheHeader {
    pub const HEADER_BYTES: usize = 32;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.total_predictions).unwrap();
        c.write_u32::<LittleEndian>(self.total_hits).unwrap();
        c.write_u32::<LittleEndian>(self.total_misses).unwrap();
        c.write_u32::<LittleEndian>(self.total_partial_hits)
            .unwrap();
        c.write_u32::<LittleEndian>(self.current_predictions)
            .unwrap();
        c.write_f32::<LittleEndian>(self.avg_confidence).unwrap();
        c.write_u64::<LittleEndian>(self.last_predict_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self {
            total_predictions: c.read_u32::<LittleEndian>().unwrap_or(0),
            total_hits: c.read_u32::<LittleEndian>().unwrap_or(0),
            total_misses: c.read_u32::<LittleEndian>().unwrap_or(0),
            total_partial_hits: c.read_u32::<LittleEndian>().unwrap_or(0),
            current_predictions: c.read_u32::<LittleEndian>().unwrap_or(0),
            avg_confidence: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            last_predict_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
    pub fn hit_rate(&self) -> f32 {
        let total = self.total_hits + self.total_misses + self.total_partial_hits;
        if total == 0 {
            return 0.0;
        }
        self.total_hits as f32 / total as f32
    }
}
impl Default for CacheHeader {
    fn default() -> Self {
        Self {
            total_predictions: 0,
            total_hits: 0,
            total_misses: 0,
            total_partial_hits: 0,
            current_predictions: 0,
            avg_confidence: 0.0,
            last_predict_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct ArchetypeHeader {
    pub active_count: u32,
    pub next_id: u32,
    pub emergence_events: u64,
    /// Same status as [`HebbianHeader::decay_event_count`]: part of the page
    /// layout, no producer (`ArchetypeState::decay` performs the event and
    /// returns nothing), and no consumer.
    pub decay_events: u64,
    pub last_emergence_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 32],
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
        Self {
            active_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            next_id: c.read_u32::<LittleEndian>().unwrap_or(1),
            emergence_events: c.read_u64::<LittleEndian>().unwrap_or(0),
            decay_events: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_emergence_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for ArchetypeHeader {
    fn default() -> Self {
        Self {
            active_count: 0,
            next_id: 1,
            emergence_events: 0,
            decay_events: 0,
            last_emergence_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct MirrorHeader {
    pub echo_count: u32,
    pub block_resonance_count: u32,
    pub strongest_similarity: f32,
    pub total_echoes: u64,
    pub last_mirror_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 28],
}
impl MirrorHeader {
    pub const HEADER_BYTES: usize = 28;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u32::<LittleEndian>(self.echo_count).unwrap();
        c.write_u32::<LittleEndian>(self.block_resonance_count)
            .unwrap();
        c.write_f32::<LittleEndian>(self.strongest_similarity)
            .unwrap();
        c.write_u64::<LittleEndian>(self.total_echoes).unwrap();
        c.write_u64::<LittleEndian>(self.last_mirror_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self {
            echo_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            block_resonance_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            strongest_similarity: c.read_f32::<LittleEndian>().unwrap_or(0.0),
            total_echoes: c.read_u64::<LittleEndian>().unwrap_or(0),
            last_mirror_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for MirrorHeader {
    fn default() -> Self {
        Self {
            echo_count: 0,
            block_resonance_count: 0,
            strongest_similarity: 0.0,
            total_echoes: 0,
            last_mirror_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct MerkleHeader {
    pub root_hash: u64,
    pub chain_height: u64,
    pub block_count: u32,
    pub evidence_audit_entries: u32,
    pub last_anchor_ms: u64,
    pub _pad: [u8; PAGE_SIZE - 32],
}
impl MerkleHeader {
    pub const HEADER_BYTES: usize = 32;
    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut buf = [0u8; PAGE_SIZE];
        let mut c = Cursor::new(&mut buf[..Self::HEADER_BYTES]);
        c.write_u64::<LittleEndian>(self.root_hash).unwrap();
        c.write_u64::<LittleEndian>(self.chain_height).unwrap();
        c.write_u32::<LittleEndian>(self.block_count).unwrap();
        c.write_u32::<LittleEndian>(self.evidence_audit_entries)
            .unwrap();
        c.write_u64::<LittleEndian>(self.last_anchor_ms).unwrap();
        buf
    }
    pub fn from_bytes(buf: &[u8]) -> Self {
        let mut c = Cursor::new(buf);
        Self {
            root_hash: c.read_u64::<LittleEndian>().unwrap_or(0),
            chain_height: c.read_u64::<LittleEndian>().unwrap_or(0),
            block_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            evidence_audit_entries: c.read_u32::<LittleEndian>().unwrap_or(0),
            last_anchor_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for MerkleHeader {
    fn default() -> Self {
        Self {
            root_hash: 0,
            chain_height: 0,
            block_count: 0,
            evidence_audit_entries: 0,
            last_anchor_ms: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct MetaHeader {
    pub block_count: u32,
    pub append_log_len: u32,
    pub dream_cycle_count: u32,
    pub reps_served_count: u32,
    pub last_dream_ms: u64,
    pub promotion_count: u64,
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
        Self {
            block_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            append_log_len: c.read_u32::<LittleEndian>().unwrap_or(0),
            dream_cycle_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            reps_served_count: c.read_u32::<LittleEndian>().unwrap_or(0),
            last_dream_ms: c.read_u64::<LittleEndian>().unwrap_or(0),
            promotion_count: c.read_u64::<LittleEndian>().unwrap_or(0),
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}
impl Default for MetaHeader {
    fn default() -> Self {
        Self {
            block_count: 0,
            append_log_len: 0,
            dream_cycle_count: 0,
            reps_served_count: 0,
            last_dream_ms: 0,
            promotion_count: 0,
            _pad: [0; PAGE_SIZE - Self::HEADER_BYTES],
        }
    }
}

/// Superblock, as it lives in the first [`SUPERBLOCK_BYTES`] bytes of page 0.
///
/// `tick_seq` is a publish sequence number, **not** a timestamp: it counts
/// publishes from 1 upwards, so two ticks in the same millisecond still get
/// distinct values. `epoch_ms` stays for the "when was this last touched"
/// question, which is a wall-clock question and is answered by the clock.
#[derive(Debug, Clone, Copy)]
pub struct Superblock {
    pub magic: u64,
    pub version: u32,
    pub page_size: u32,
    pub wal_start_page: u32,
    pub wal_head_page: u32,
    pub tick_seq: u64,
    pub epoch_ms: i64,
}
impl Default for Superblock {
    fn default() -> Self {
        Self::ZEROED
    }
}
impl Superblock {
    const ZEROED: Self = Self {
        magic: 0,
        version: SUPERBLOCK_VERSION,
        page_size: 0,
        wal_start_page: 0,
        wal_head_page: 0,
        tick_seq: 0,
        epoch_ms: 0,
    };
    pub fn new() -> Self {
        Self {
            magic: MAGIC,
            version: SUPERBLOCK_VERSION,
            page_size: PAGE_SIZE as u32,
            wal_start_page: Slot::WalStart as u32,
            wal_head_page: Slot::WalStart as u32,
            tick_seq: 0,
            epoch_ms: now_ms(),
        }
    }
    /// The published payload. Byte 56..64 of the page is the seqlock counter
    /// and is deliberately **not** part of it, so a rewrite of the payload
    /// can never clear the counter out from under a reader.
    pub fn to_payload(&self) -> [u8; SUPERBLOCK_BYTES] {
        let mut buf = [0u8; SUPERBLOCK_BYTES];
        let mut c = Cursor::new(&mut buf[..]);
        c.write_u64::<LittleEndian>(self.magic).unwrap();
        c.write_u32::<LittleEndian>(self.version).unwrap();
        c.write_u32::<LittleEndian>(self.page_size).unwrap();
        c.write_u32::<LittleEndian>(self.wal_start_page).unwrap();
        c.write_u32::<LittleEndian>(self.wal_head_page).unwrap();
        c.write_u64::<LittleEndian>(self.tick_seq).unwrap();
        c.write_i64::<LittleEndian>(self.epoch_ms).unwrap();
        buf
    }
    /// `None` when the page is not a superblock at all. A version other than
    /// the current one is still decoded: `magic` says whose file this is and
    /// the reader decides what to do with the older fields.
    pub fn from_payload(buf: &[u8]) -> Option<Self> {
        let mut c = Cursor::new(buf);
        let magic = c.read_u64::<LittleEndian>().ok()?;
        if magic != MAGIC {
            return None;
        }
        let version = c.read_u32::<LittleEndian>().ok()?;
        if version != SUPERBLOCK_VERSION {
            return None;
        }
        Some(Self {
            magic,
            version,
            page_size: c.read_u32::<LittleEndian>().ok()?,
            wal_start_page: c.read_u32::<LittleEndian>().ok()?,
            wal_head_page: c.read_u32::<LittleEndian>().ok()?,
            tick_seq: c.read_u64::<LittleEndian>().ok()?,
            epoch_ms: c.read_i64::<LittleEndian>().ok()?,
        })
    }
    /// Decode a version-1 superblock, whose `epoch_ms` sat at bytes 24..32
    /// and which had no publish counter at all. Only the fields that are
    /// still meaningful are carried over; [`Spine::open`] republishes the
    /// result as version 2.
    fn from_v1_payload(buf: &[u8]) -> Option<Self> {
        let mut c = Cursor::new(buf);
        let magic = c.read_u64::<LittleEndian>().ok()?;
        if magic != MAGIC {
            return None;
        }
        let version = c.read_u32::<LittleEndian>().ok()?;
        if version != 1 {
            return None;
        }
        Some(Self {
            magic,
            version,
            page_size: c.read_u32::<LittleEndian>().ok()?,
            wal_start_page: c.read_u32::<LittleEndian>().ok()?,
            wal_head_page: c.read_u32::<LittleEndian>().ok()?,
            tick_seq: 0,
            epoch_ms: c.read_i64::<LittleEndian>().ok()?,
        })
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// One published snapshot: the stream header plus the superblock payload
/// that was in the file at the same moment — the two things the seqlock
/// brackets. Their agreement is the guarantee; see [`Spine::read_snapshot`].
#[derive(Debug, Clone, Copy)]
pub struct PublishSnapshot {
    pub stream: StreamHeader,
    /// Publish counter of the snapshot's superblock payload (monotonic, and
    /// distinct for two ticks inside the same millisecond).
    pub tick_seq: u64,
    /// Wall-clock milliseconds at publish time. Freshness, not ordering.
    pub epoch_ms: i64,
    /// The seqlock value this snapshot was read under; always even.
    pub sequence: u64,
}

/// One publisher's turn. `Drop` releases the advisory lock, so a publish that
/// panics part-way through cannot leave the store un-writable.
pub struct PublishGuard<'a> {
    file: Option<&'a std::fs::File>,
    held: bool,
}

impl Drop for PublishGuard<'_> {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let (true, Some(file)) = (self.held, self.file) {
            use std::os::fd::AsRawFd;
            // Best effort: closing the descriptor would release the lock too,
            // and a caller can do nothing useful with a failed unlock.
            let _ = writer_lock::try_op(file.as_raw_fd(), writer_lock::UN);
        }
    }
}

impl PublishGuard<'_> {
    /// Whether this guard actually excluded other publishers. `false` means
    /// the platform or filesystem gave us nothing to lock, so the publish was
    /// serialised against this process only.
    pub fn is_advisory(&self) -> bool {
        self.held
    }
}

/// Spine file handle. `Clone`-like sharing is by `Arc` inside
/// [`crate::consciousness_stream`]; a second process opens its own handle
/// over the same file and coordinates through the seqlock.
pub struct Spine {
    output_dir: PathBuf,
    mmap: Arc<Mutex<MmapMut>>,
    /// Held open for the lifetime of a *writer* handle so the advisory lock
    /// stays claimed. `None` on a read-only handle, which neither claims
    /// writer status nor may `resize`.
    lock_file: Option<std::fs::File>,
    /// Opened per handle and deliberately *not* held: locking and unlocking
    /// this descriptor around each publish is what serialises publishers.
    /// `None` when the file could not be opened, which costs serialisation.
    publish_lock_file: Option<std::fs::File>,
    /// `false` once the publish lock turns out to be unavailable, so nothing
    /// claims an exclusion it does not have.
    publish_lock_advisory: bool,
}

/// The seqlock counter as an atomic over the live mapping.
///
/// # Safety
/// `mmap` must be mapped, writable, at least `SEQ_SLOT + 8` bytes long, and
/// its base address page-aligned (mappings from `mmap(2)` always are, which
/// is what makes `SEQ_SLOT` — a multiple of 8 — an aligned address). No code
/// may write those 8 bytes non-atomically: [`Spine::write_slot`] is the only
/// path that writes page 0 wholesale and it refuses to.
/// The counter is at a fixed offset in a live mapping, so this is the one
/// place that has to hold the two invariants every caller would otherwise have
/// to remember: the mapping covers `SEQ_SLOT..SEQ_SLOT + 8`, and the address is
/// 8-byte aligned. Alignment holds because `mmap` starts on a page boundary and
/// `SEQ_SLOT` is a multiple of 8; the length is checked here, in release too,
/// so a caller that forgets its guard panics instead of being undefined.
unsafe fn seq_atomic(mmap: &mut MmapMut) -> &AtomicU64 {
    assert!(
        mmap.len() >= SEQ_SLOT + 8,
        "spine mapping is {} bytes, too short to carry a counter at {SEQ_SLOT}",
        mmap.len()
    );
    let base = mmap.as_mut_ptr();
    // The only alignment the atomic needs is 8, and `mmap(2)` hands out
    // page-aligned bases (4096 at the smallest), so `SEQ_SLOT` — a multiple
    // of 8 — lands on an aligned address. Assert that directly rather than
    // assuming the spine's own 16 KiB page size is the OS page size.
    debug_assert_eq!(
        (base as usize + SEQ_SLOT) % std::mem::align_of::<AtomicU64>(),
        0,
        "publish counter is not AtomicU64-aligned"
    );
    // SAFETY: the caller of this function upheld the invariants above, and
    // `AtomicU64::from_ptr` is `unsafe` exactly because it cannot check them.
    unsafe { AtomicU64::from_ptr(base.add(SEQ_SLOT).cast::<u64>()) }
}

/// Read the publish counter with acquire ordering.
fn seq_load(mmap: &mut MmapMut) -> u64 {
    // SAFETY: bounds and alignment per `seq_atomic`; the borrow ends here.
    unsafe { seq_atomic(mmap).load(Ordering::Acquire) }
}

/// What a reader does between two attempts: spin briefly, then hand the core
/// to whoever is publishing, which is the only way the counter can settle.
fn snapshot_backoff(attempt: u32) {
    if attempt < 64 {
        std::hint::spin_loop();
    } else {
        std::thread::yield_now();
    }
}

/// Open a publish window: the counter goes odd, which is what tells a reader
/// "anything you copy now may be mid-write, retry". Returns the odd value, so
/// this publish's serial number is `(n + 1) >> 1`.
fn seq_begin(mmap: &mut MmapMut) -> u64 {
    // SAFETY: as in `seq_load`.
    unsafe { seq_atomic(mmap).fetch_add(1, Ordering::AcqRel) | 1 }
}

/// Close the window opened by `seq_begin`. The `Release` store is what makes
/// every byte written in between visible to a reader that observes the even
/// value — in this process or in another one.
fn seq_end(mmap: &mut MmapMut, n: u64) {
    // SAFETY: as in `seq_load`; `n + 1` is even because `n` was forced odd.
    unsafe { seq_atomic(mmap).store(n + 1, Ordering::Release) }
}

/// Advisory writer lock (`flock(2)` on the sidecar `spine.lock`, so the data
/// file descriptor itself stays uninvolved).
#[cfg(unix)]
mod writer_lock {
    use std::io;

    pub const SH: i32 = 1;
    pub const EX: i32 = 2;
    pub const NB: i32 = 4;
    pub const UN: i32 = 8;

    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }

    /// `Err(true)` == somebody else holds it right now, retrying may help.
    /// `Err(false)` == retrying will not help.
    pub fn try_op(fd: i32, operation: i32) -> Result<(), (bool, io::Error)> {
        // SAFETY: `fd` is an open descriptor owned by our caller.
        if unsafe { flock(fd, operation) } == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        // EWOULDBLOCK (35) and EAGAIN (11) both mean "held elsewhere".
        let contended = operation & NB != 0 && matches!(err.raw_os_error(), Some(11) | Some(35));
        Err((contended, err))
    }
}

/// No `flock(2)` on these targets. Publishing is still safe — the seqlock
/// brackets every write — but `resize` has no way to ask other processes to
/// stand back, so it declines instead of remapping pages out from under them.
#[cfg(not(unix))]
mod writer_lock {
    use std::io;

    // Nothing to lock here, so the operations callers pass are unused.
    #[allow(dead_code)]
    pub const SH: i32 = 1;
    #[allow(dead_code)]
    pub const EX: i32 = 2;
    #[allow(dead_code)]
    pub const NB: i32 = 4;
    #[allow(dead_code)]
    pub const UN: i32 = 8;

    pub fn try_op(_fd: i32, _operation: i32) -> Result<(), (bool, io::Error)> {
        Err((
            false,
            io::Error::other("advisory writer locks are unavailable on this target"),
        ))
    }
}

impl Spine {
    /// Both coordination files live beside `spine.bin`; the data file never
    /// becomes a lock file, so a lock leak cannot corrupt the store.
    ///
    /// A reader that cannot open `spine.publish` (read-only directory) still
    /// reads correctly: it loses only the ability to serialise *against* the
    /// writers, which it never had. A writer that cannot create it fails
    /// loudly, because it would otherwise publish unserialised and believe
    /// otherwise.
    fn open_publish_lock(
        dir: &Path,
        create: bool,
    ) -> std::io::Result<(Option<std::fs::File>, bool)> {
        let path = dir.join(PUBLISH_LOCK_FILE_NAME);
        let opened = if create {
            OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(false)
                .create(true)
                .open(&path)
        } else {
            OpenOptions::new().read(true).write(true).open(&path)
        };
        match opened {
            Ok(file) => Ok((Some(file), true)),
            Err(_) if !create => Ok((None, false)),
            Err(e) => Err(e),
        }
    }

    /// Take the publisher-exclusive advisory lock for as long as the returned
    /// guard lives. Blocking, because a publisher that skipped the queue is
    /// back where the per-process mutex left it: two writers in one window.
    fn publish_guard(&self) -> PublishGuard<'_> {
        #[cfg(unix)]
        if let Some(file) = self.publish_lock_file.as_ref() {
            use std::os::fd::AsRawFd;
            // `LOCK_EX` with no `LOCK_NB`: wait our turn. A publisher that
            // died holding it takes the lock down with it, so the wait cannot
            // stick on a dead process.
            if writer_lock::try_op(file.as_raw_fd(), writer_lock::EX).is_ok() {
                return PublishGuard {
                    file: Some(file),
                    held: true,
                };
            }
        }
        PublishGuard {
            file: None,
            held: false,
        }
    }

    /// `false` when publishers in other processes are not excluded from each
    /// other, i.e. when two of them could open a publish window at the same
    /// time and the seqlock alone is what keeps readers whole.
    pub fn publish_lock_is_advisory(&self) -> bool {
        self.publish_lock_advisory
    }

    /// Open for writing: create the file, lay out or upgrade the superblock,
    /// and mark this process as a live writer.
    pub fn open(output_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = output_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("spine.bin");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .create(true)
            .open(&path)?;
        let min_len = (Slot::WalStart as usize + 8) * PAGE_SIZE;
        if file.metadata()?.len() < min_len as u64 {
            file.set_len(min_len as u64)?;
        }

        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .create(true)
            .open(dir.join(WRITER_LOCK_FILE_NAME))?;
        Self::mark_shared(&lock_file)?;
        let (publish_lock_file, publish_lock_advisory) = Self::open_publish_lock(&dir, true)?;

        // SAFETY: the mapping covers every published slot, and it is only
        // replaced by `resize`, which holds the lock exclusively.
        // SAFETY: the file outlives the mapping, and `resize` is the only
        // place that replaces it.
        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(std::io::Error::other)?
        };
        let spine = Self {
            output_dir: dir,
            mmap: Arc::new(Mutex::new(mmap)),
            lock_file: Some(lock_file),
            publish_lock_file,
            publish_lock_advisory,
        };
        spine.init_superblock()?;
        Ok(spine)
    }

    /// Open an existing spine for reading: no create, no sizing, no writer
    /// mark, and no `resize`. Readers need no lock to be safe, because
    /// [`Spine::read_snapshot`] validates against the seqlock instead.
    pub fn open_ro(output_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = output_dir.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .open(dir.join("spine.bin"))?;
        // SAFETY: a read-only handle never writes through this mapping.
        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(std::io::Error::other)?
        };
        if mmap.len() < SEQ_SLOT + 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "spine.bin is shorter than the publish header",
            ));
        }
        let (publish_lock_file, publish_lock_advisory) = Self::open_publish_lock(&dir, false)?;
        Ok(Self {
            output_dir: dir,
            mmap: Arc::new(Mutex::new(mmap)),
            lock_file: None,
            publish_lock_file,
            publish_lock_advisory,
        })
    }

    /// Shared ("a writer is live") mark, retried because the exclusive holder
    /// is only ever another process's init or `resize`, both short.
    fn mark_shared(lock_file: &std::fs::File) -> std::io::Result<()> {
        #[cfg(unix)]
        use std::os::fd::AsRawFd;
        #[cfg(not(unix))]
        let _ = lock_file; // no advisory lock to take; publishing is still safe
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            match writer_lock::try_op(lock_file.as_raw_fd(), writer_lock::SH | writer_lock::NB) {
                Ok(()) => return Ok(()),
                Err((true, _)) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Err((true, _)) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "another process holds the spine lock exclusively",
                    ))
                }
                Err((false, e)) => return Err(e),
            }
        }
    }

    /// `flock(LOCK_EX | LOCK_NB)` until a deadline, for the callers that must
    /// not proceed while any other writer handle is alive (`resize`, first-time
    /// layout on a shared filesystem). Err(WouldBlock) on timeout.
    #[cfg(unix)]
    fn acquire_exclusive_until(lock: &std::fs::File) -> std::io::Result<()> {
        use std::os::fd::AsRawFd;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            match writer_lock::try_op(lock.as_raw_fd(), writer_lock::EX | writer_lock::NB) {
                Ok(()) => return Ok(()),
                Err((true, _)) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Err((true, _)) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "another process holds the spine lock; resize would remap its pages",
                    ))
                }
                Err((false, e)) => return Err(e),
            }
        }
    }

    /// Lay out the superblock, or upgrade a version-1 one. The payload goes
    /// out inside a seqlock window so a concurrent reader sees either the old
    /// superblock or the new one, never a mix. Both writers here write the
    /// same bytes, so a shared mark is sufficient without excluding the peer.
    fn init_superblock(&self) -> std::io::Result<()> {
        let _publish = self.publish_guard();
        let mut m = self.mmap.lock();
        if Superblock::from_payload(&m[0..SUPERBLOCK_BYTES]).is_some() {
            return Ok(()); // already the current layout
        }
        // Either an older layout worth carrying forward, or no spine here at
        // all. `from_v1_payload` only accepts a matching magic, so an
        // uninitialised or foreign page falls through to a fresh superblock.
        let fresh = match Superblock::from_v1_payload(&m[0..SUPERBLOCK_BYTES]) {
            Some(old) => Superblock {
                version: SUPERBLOCK_VERSION,
                ..old
            },
            // Not a spine file, or not one we recognise: lay it out from
            // scratch. Note this is `Superblock::new`, not `Default`, whose
            // ZEROED superblock is the *absent* magic on purpose.
            None => Superblock::new(),
        };
        let n = seq_begin(&mut m); // odd: publishing
        m[0..SUPERBLOCK_BYTES].copy_from_slice(&fresh.to_payload());
        seq_end(&mut m, n); // even: published
        drop(m);
        self.sync()?; // layout must be on disk before a peer maps it
        Ok(())
    }

    fn read_slot(&self, slot: Slot) -> [u8; PAGE_SIZE] {
        let off = slot.offset();
        let m = self.mmap.lock();
        let mut buf = [0u8; PAGE_SIZE];
        let end = (off + PAGE_SIZE).min(m.len());
        if off < m.len() {
            buf[..end - off].copy_from_slice(&m[off..end]);
        }
        buf
    }

    /// Publish one whole page.
    ///
    /// For [`Slot::Superblock`] the write is bracketed by the seqlock and
    /// trimmed to the payload, because bytes `SEQ_SLOT..` *are* the counter:
    /// a naive page copy would reset it mid-window. Other slots are plain
    /// page writes — serialised against this process by the mutex, and
    /// readable as a stale-but-complete page by peers.
    pub fn write_slot(&self, slot: Slot, bytes: &[u8; PAGE_SIZE]) {
        // Taken before the mapping lock, always in that order, so two
        // publishers cannot both be inside a window; see the module docs.
        let _publish = self.publish_guard();
        let off = slot.offset();
        let mut m = self.mmap.lock();
        if off + PAGE_SIZE > m.len() || m.len() < SEQ_SLOT + 8 {
            return;
        }
        if slot == Slot::Superblock {
            let n = seq_begin(&mut m);
            m[0..SUPERBLOCK_BYTES].copy_from_slice(&bytes[..SUPERBLOCK_BYTES]);
            seq_end(&mut m, n);
            return;
        }
        m[off..off + PAGE_SIZE].copy_from_slice(bytes);
    }

    pub fn read_stream(&self) -> StreamHeader {
        StreamHeader::from_bytes(&self.read_slot(Slot::Stream))
    }
    pub fn write_stream(&self, h: &StreamHeader) {
        self.write_slot(Slot::Stream, &h.to_bytes());
    }

    /// Publish the stream header and the superblock payload as one snapshot:
    /// one publisher lock (`flock` on `spine.publish`, which also excludes
    /// publishers in other processes), one mapping lock, one seqlock window.
    ///
    /// That is what makes the pair readable as a unit. It is **not** one
    /// hardware transaction: the stream page is still a 16 KiB
    /// `copy_from_slice`, so a reader that ignores the seqlock can still catch
    /// it mid-write — [`Spine::read_snapshot`] is the read path that does not.
    /// `updated_ms` in the header and `epoch_ms` in the superblock come from
    /// the same clock reading, so a snapshot that satisfies
    /// `stream.updated_ms == epoch_ms as u64` is a snapshot of one tick.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &self,
        cycle: u64,
        surprise: f32,
        curiosity: f32,
        predicted_hash: u64,
        predicted_confidence: f32,
        last_query_hash: u64,
        last_query_ms: u64,
        hebbian_total_energy: f64,
    ) {
        let _publish = self.publish_guard();
        let epoch_ms = now_ms();
        let h = StreamHeader {
            cycle,
            surprise_level: surprise,
            curiosity_level: curiosity,
            predicted_query_hash: predicted_hash,
            predicted_confidence,
            last_query_hash,
            last_query_ms,
            hebbian_total_energy,
            updated_ms: epoch_ms as u64,
            _pad: [0; PAGE_SIZE - StreamHeader::HEADER_BYTES],
        };
        let page = h.to_bytes();

        let mut m = self.mmap.lock();
        if m.len() < Slot::Stream.offset() + PAGE_SIZE || m.len() < SEQ_SLOT + 8 {
            return;
        }
        let n = seq_begin(&mut m); // odd: publishing
                                   // The superblock payload is derived before the copy, so no borrow of
                                   // `m` is live while `m` is written.
        let payload = Superblock {
            tick_seq: (n + 1) >> 1,
            epoch_ms,
            ..current_superblock(&m)
        }
        .to_payload();
        // Payload first: a reader that ignored the counter would still pair a
        // new superblock with an at-least-as-new stream header, not an old one.
        m[0..SUPERBLOCK_BYTES].copy_from_slice(&payload);
        let off = Slot::Stream.offset();
        m[off..off + PAGE_SIZE].copy_from_slice(&page);
        seq_end(&mut m, n); // even: published
    }

    /// The latest published snapshot, or `None` once both
    /// [`SNAPSHOT_MAX_RETRIES`] attempts and [`SNAPSHOT_MAX_WAIT_MS`] have
    /// been spent on publish windows crossing us.
    ///
    /// Both halves are copied under one lock and checked against the counter
    /// before and after, so a returned snapshot is never a new stream header
    /// next to an older superblock payload — from this process or any other.
    pub fn read_snapshot(&self) -> Option<PublishSnapshot> {
        let mut prefix = [0u8; SNAPSHOT_PREFIX_BYTES];
        let mut payload = [0u8; SUPERBLOCK_BYTES];
        let mut started = None::<std::time::Instant>;
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let mut m = self.mmap.lock();
            if m.len() < Slot::Stream.offset() + SNAPSHOT_PREFIX_BYTES || m.len() < SEQ_SLOT + 8 {
                return None;
            }
            let s1 = seq_load(&mut m);
            if s1 & 1 == 1 {
                drop(m);
                if self.snapshot_patience(&mut started, attempt) {
                    break;
                }
                snapshot_backoff(attempt);
                continue;
            }
            // Copy first, decode after: the copies are what the counter
            // brackets, and the mapping must not be read a second time once
            // the check has passed.
            let off = Slot::Stream.offset();
            prefix.copy_from_slice(&m[off..off + SNAPSHOT_PREFIX_BYTES]);
            payload.copy_from_slice(&m[0..SUPERBLOCK_BYTES]);
            let s2 = seq_load(&mut m);
            drop(m);
            if s1 != s2 {
                if self.snapshot_patience(&mut started, attempt) {
                    break;
                }
                snapshot_backoff(attempt);
                continue;
            }
            let Some(sb) = Superblock::from_payload(&payload) else {
                // No (or no longer) a spine superblock: retrying will not
                // make these bytes decodable.
                return None;
            };
            return Some(PublishSnapshot {
                stream: StreamHeader::from_bytes(&prefix),
                tick_seq: sb.tick_seq,
                epoch_ms: sb.epoch_ms,
                sequence: s2,
            });
        }
        // Out of patience. The one failure a reader must not accept
        // permanently is a window that will never close because the process
        // that opened it was killed; readers would then return `None`
        // forever. Try to close it, and read once more under the counter.
        if self.recover_stalled_publish() {
            return self.read_snapshot_once();
        }
        None
    }

    /// A single bracketed read, no retry. Used right after a stalled window
    /// was closed, where the caller has already established that nobody is
    /// publishing; a fresh pair of counter reads is all the check it needs.
    fn read_snapshot_once(&self) -> Option<PublishSnapshot> {
        let mut m = self.mmap.lock();
        if m.len() < Slot::Stream.offset() + SNAPSHOT_PREFIX_BYTES || m.len() < SEQ_SLOT + 8 {
            return None;
        }
        let s1 = seq_load(&mut m);
        if s1 & 1 == 1 {
            return None;
        }
        let mut prefix = [0u8; SNAPSHOT_PREFIX_BYTES];
        let mut payload = [0u8; SUPERBLOCK_BYTES];
        let off = Slot::Stream.offset();
        prefix.copy_from_slice(&m[off..off + SNAPSHOT_PREFIX_BYTES]);
        payload.copy_from_slice(&m[0..SUPERBLOCK_BYTES]);
        if seq_load(&mut m) != s1 {
            return None;
        }
        let sb = Superblock::from_payload(&payload)?;
        Some(PublishSnapshot {
            stream: StreamHeader::from_bytes(&prefix),
            tick_seq: sb.tick_seq,
            epoch_ms: sb.epoch_ms,
            sequence: s1,
        })
    }

    /// `true` when the read should give up: both the attempt budget and the
    /// time budget spent. `started` is only initialised on the first retry, so
    /// an uninterrupted read costs no clock reading at all.
    fn snapshot_patience(&self, started: &mut Option<std::time::Instant>, attempt: u32) -> bool {
        if attempt < SNAPSHOT_MAX_RETRIES {
            return false;
        }
        let t0 = *started.get_or_insert_with(std::time::Instant::now);
        t0.elapsed() >= std::time::Duration::from_millis(SNAPSHOT_MAX_WAIT_MS)
    }

    /// Whether the store's publish window is open: while a writer is inside
    /// one, and — the reason this is worth exposing — while a writer that was
    /// killed leaves one open. [`Spine::recover_stalled_publish`] clears the
    /// second case.
    pub fn publish_window_open(&self) -> bool {
        self.publish_in_flight()
    }

    /// Close a publish window whose writer is gone.
    ///
    /// A `SIGKILL` between the odd store and the even one leaves the counter
    /// odd, and every reader would then spend its whole budget on a window that
    /// never closes, forever. This is the repair: take the publisher lock
    /// (blocking, which is also what proves no live writer is inside a window)
    /// and, if the counter is still the odd value found without the lock,
    /// close it.
    ///
    /// Returns `true` only when a window was actually closed. There is no
    /// timeout involved, so a writer that is merely slow is never interrupted:
    /// `tick()` opens and closes its window with the publisher lock held, and
    /// the lock is what makes that argument work.
    pub fn recover_stalled_publish(&self) -> bool {
        let mut m = self.mmap.lock();
        if m.len() < SEQ_SLOT + 8 {
            return false;
        }
        // Cheap check first: the overwhelmingly common case is "nothing to do",
        // and it must not cost an flock round trip.
        if seq_load(&mut m) & 1 == 0 {
            return false;
        }
        drop(m);
        let _publish = self.publish_guard();
        let mut m = self.mmap.lock();
        if m.len() < SEQ_SLOT + 8 {
            return false;
        }
        let n = seq_load(&mut m);
        if n & 1 == 0 {
            return false; // a live writer closed it while we waited for the lock
        }
        if !_publish.is_advisory() {
            // Without the publisher lock there is no way to tell a dead writer
            // from a live one, and closing the window mid-write would hand a
            // reader a half-updated page as if it were a publish.
            return false;
        }
        seq_end(&mut m, n);
        true
    }

    /// Publish counter right now. `0` on a file too short to carry one.
    pub fn publish_sequence(&self) -> u64 {
        let mut m = self.mmap.lock();
        if m.len() < SEQ_SLOT + 8 {
            return 0;
        }
        seq_load(&mut m)
    }

    /// True while some writer is mid-publish. Readers never have to act on
    /// it — `read_snapshot` retries on its own — but the live viewer uses it
    /// to explain a stalled `tick_seq` without claiming a torn read.
    pub fn publish_in_flight(&self) -> bool {
        let mut m = self.mmap.lock();
        if m.len() < SEQ_SLOT + 8 {
            return false;
        }
        seq_load(&mut m) & 1 == 1
    }

    pub fn read_hebbian(&self) -> HebbianHeader {
        HebbianHeader::from_bytes(&self.read_slot(Slot::Hebbian))
    }
    pub fn write_hebbian(&self, h: &HebbianHeader) {
        self.write_slot(Slot::Hebbian, &h.to_bytes());
    }
    pub fn read_attention(&self) -> AttentionHeader {
        AttentionHeader::from_bytes(&self.read_slot(Slot::Attention))
    }
    pub fn write_attention(&self, h: &AttentionHeader) {
        self.write_slot(Slot::Attention, &h.to_bytes());
    }
    pub fn read_emotion(&self) -> EmotionHeader {
        EmotionHeader::from_bytes(&self.read_slot(Slot::Emotion))
    }
    pub fn write_emotion(&self, h: &EmotionHeader) {
        self.write_slot(Slot::Emotion, &h.to_bytes());
    }
    pub fn read_resonance(&self) -> ResonanceHeader {
        ResonanceHeader::from_bytes(&self.read_slot(Slot::Resonance))
    }
    pub fn write_resonance(&self, h: &ResonanceHeader) {
        self.write_slot(Slot::Resonance, &h.to_bytes());
    }
    pub fn read_graph(&self) -> GraphHeader {
        GraphHeader::from_bytes(&self.read_slot(Slot::Graph))
    }
    pub fn write_graph(&self, h: &GraphHeader) {
        self.write_slot(Slot::Graph, &h.to_bytes());
    }
    pub fn read_cache(&self) -> CacheHeader {
        CacheHeader::from_bytes(&self.read_slot(Slot::Cache))
    }
    pub fn write_cache(&self, h: &CacheHeader) {
        self.write_slot(Slot::Cache, &h.to_bytes());
    }
    pub fn read_archetype(&self) -> ArchetypeHeader {
        ArchetypeHeader::from_bytes(&self.read_slot(Slot::Archetype))
    }
    pub fn write_archetype(&self, h: &ArchetypeHeader) {
        self.write_slot(Slot::Archetype, &h.to_bytes());
    }
    pub fn read_mirror(&self) -> MirrorHeader {
        MirrorHeader::from_bytes(&self.read_slot(Slot::Mirror))
    }
    pub fn write_mirror(&self, h: &MirrorHeader) {
        self.write_slot(Slot::Mirror, &h.to_bytes());
    }
    pub fn read_merkle(&self) -> MerkleHeader {
        MerkleHeader::from_bytes(&self.read_slot(Slot::Merkle))
    }
    pub fn write_merkle(&self, h: &MerkleHeader) {
        self.write_slot(Slot::Merkle, &h.to_bytes());
    }
    pub fn read_meta(&self) -> MetaHeader {
        MetaHeader::from_bytes(&self.read_slot(Slot::Meta))
    }
    pub fn write_meta(&self, h: &MetaHeader) {
        self.write_slot(Slot::Meta, &h.to_bytes());
    }

    /// The superblock as stored. Prefer [`Spine::read_snapshot`] when the
    /// value has to agree with a stream header.
    pub fn read_superblock(&self) -> Option<Superblock> {
        let m = self.mmap.lock();
        if m.len() < SUPERBLOCK_BYTES {
            return None;
        }
        Superblock::from_payload(&m[0..SUPERBLOCK_BYTES])
    }

    pub fn sync(&self) -> std::io::Result<()> {
        self.mmap.lock().flush()
    }
    pub fn size_bytes(&self) -> usize {
        self.mmap.lock().len()
    }
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
    pub fn path(&self) -> PathBuf {
        self.output_dir.join("spine.bin")
    }

    /// Grow the file and remap. Refuses unless this handle is a writer and no
    /// peer holds the lock exclusively, because remapping invalidates page 0
    /// — including the seqlock counter — for every other mapping.
    pub fn resize(&self, new_len: usize) -> std::io::Result<()> {
        #[cfg(unix)]
        use std::os::fd::AsRawFd;
        let Some(lock) = self.lock_file.as_ref() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "read-only spine handle cannot resize",
            ));
        };
        #[cfg(unix)]
        {
            // Drop the shared mark so the exclusive one can be taken at all.
            let _ = writer_lock::try_op(lock.as_raw_fd(), writer_lock::UN);
            if let Err(e) = Self::acquire_exclusive_until(lock) {
                // The mark has to go back on the failure path too: a writer
                // handle that quietly lost it would let a peer resize the file
                // under this mapping.
                let _ = Self::mark_shared(lock);
                return Err(e);
            }
        }
        #[cfg(not(unix))]
        {
            let _ = lock;
            return Err(std::io::Error::other(
                "resize needs advisory file locks, unavailable on this target",
            ));
        }

        let result = (|| -> std::io::Result<()> {
            {
                let f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .truncate(false)
                    .open(self.path())?;
                f.set_len(new_len as u64)?;
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(false)
                .open(self.path())?;
            // SAFETY: exclusive lock held; no peer mapping is live mid-resize.
            let new_mmap = unsafe {
                MmapOptions::new()
                    .map_mut(&file)
                    .map_err(std::io::Error::other)?
            };
            *self.mmap.lock() = new_mmap;
            Ok(())
        })();

        #[cfg(unix)]
        {
            let _ = writer_lock::try_op(lock.as_raw_fd(), writer_lock::UN);
            let _ = Self::mark_shared(lock);
        }
        result
    }
}

/// Read the current superblock payload from a locked mapping, so a tick
/// rewrites only what it owns and keeps the WAL coordinates.
fn current_superblock(mmap: &MmapMut) -> Superblock {
    match Superblock::from_payload(&mmap[0..SUPERBLOCK_BYTES]) {
        Some(sb) => sb,
        None => Superblock::new(), // as above: `new`, not the zeroed default
    }
}

/// Human-readable dump of the spine.
///
/// The stream header comes from [`Spine::read_snapshot`], so the cycle and
/// the publish record printed next to it are from the same publish. Two
/// separate reads is exactly the mixing the seqlock exists to prevent.
pub fn format_summary(spine: &Spine) -> String {
    let snap = spine.read_snapshot();
    let s = snap
        .as_ref()
        .map(|p| p.stream)
        .unwrap_or_else(|| spine.read_stream());
    let (tick_seq, epoch_ms) = snap
        .as_ref()
        .map(|p| (p.tick_seq, p.epoch_ms))
        .unwrap_or((0, 0));
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
    format!(
        "Spine: {} pages mapped ({} KB)\n\
        Publish:  seq={} epoch_ms={} in_flight={}\n\
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
        spine.size_bytes() / PAGE_SIZE,
        spine.size_bytes() / 1024,
        tick_seq,
        epoch_ms,
        spine.publish_in_flight(),
        s.cycle,
        s.surprise_level,
        s.curiosity_level,
        s.predicted_confidence,
        h.activation_count,
        h.coactivation_count,
        h.delta_tail_offset,
        a.total_recalls,
        a.quality_signal,
        e.total_updates,
        e.current[0],
        r.field_cells,
        r.pulses_emitted,
        r.pulses_received,
        g.node_count,
        g.edge_count,
        g.pattern_count,
        c.total_predictions,
        c.hit_rate() * 100.0,
        ar.active_count,
        ar.emergence_events,
        mi.echo_count,
        mi.strongest_similarity,
        me.chain_height,
        me.block_count,
        meta.block_count,
        meta.dream_cycle_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_spine(tag: &str) -> Spine {
        let dir = std::env::temp_dir().join(format!("m1c_spine_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Spine::open(&dir).unwrap()
    }
    #[test]
    fn test_open_creates_superblock() {
        let spine = temp_spine("sb");
        let sb = spine.read_superblock().unwrap();
        assert_eq!(sb.magic, MAGIC);
        assert_eq!(sb.page_size as usize, PAGE_SIZE);
    }
    #[test]
    fn test_stream_roundtrip() {
        let spine = temp_spine("stream");
        spine.tick(42, 0.5, 0.7, 0xdeadbeef, 0.9, 0xcafebabe, 12345, 1337.5);
        let s = spine.read_stream();
        assert_eq!(s.cycle, 42);
        assert!((s.surprise_level - 0.5).abs() < 1e-6);
        assert_eq!(s.predicted_query_hash, 0xdeadbeef);
        assert!((s.hebbian_total_energy - 1337.5).abs() < 1e-6);
        assert!(s.updated_ms > 0);
    }
    #[test]
    fn test_layer_headers_roundtrip() {
        let spine = temp_spine("layers");
        let mut h = spine.read_hebbian();
        h.activation_count = 999;
        h.delta_tail_offset = 123456789;
        h.decay_event_count = 0;
        spine.write_hebbian(&h);
        assert_eq!(spine.read_hebbian().activation_count, 999);
        assert_eq!(spine.read_hebbian().delta_tail_offset, 123456789);
        let mut a = spine.read_attention();
        a.learned_weights[0] = 0.25;
        a.total_recalls = 17;
        spine.write_attention(&a);
        assert!((spine.read_attention().learned_weights[0] - 0.25).abs() < 1e-6);
        assert_eq!(spine.read_attention().total_recalls, 17);
        let mut e = spine.read_emotion();
        e.current[3] = 0.8;
        e.total_updates = 100;
        spine.write_emotion(&e);
        assert!((spine.read_emotion().current[3] - 0.8).abs() < 1e-6);
        let mut g = spine.read_graph();
        g.node_count = 500;
        spine.write_graph(&g);
        assert_eq!(spine.read_graph().node_count, 500);
        let mut c = spine.read_cache();
        c.total_hits = 10;
        c.total_misses = 2;
        spine.write_cache(&c);
        let rc = spine.read_cache();
        assert!((rc.hit_rate() - 10.0 / 12.0).abs() < 1e-5);
    }
    #[test]
    fn test_all_slots_distinct() {
        let spine = temp_spine("slots");
        spine.tick(1, 1.0, 1.0, 1, 1.0, 1, 1, 1.0);
        let mut g = spine.read_graph();
        g.node_count = 42;
        spine.write_graph(&g);
        let mut m = spine.read_mirror();
        m.echo_count = 7;
        spine.write_mirror(&m);
        assert_eq!(spine.read_stream().cycle, 1);
        assert_eq!(spine.read_graph().node_count, 42);
        assert_eq!(spine.read_mirror().echo_count, 7);
        assert_eq!(spine.read_archetype().active_count, 0);
    }
    #[test]
    fn test_reopen_preserves_all_layers() {
        let dir = std::env::temp_dir().join(format!("m1c_spine_reopen_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let spine = Spine::open(&dir).unwrap();
            spine.tick(7, 0.3, 0.4, 5, 6.0, 7, 8, 9.0);
            let mut a = spine.read_attention();
            a.total_recalls = 55;
            spine.write_attention(&a);
            spine.sync().unwrap();
        }
        let spine = Spine::open(&dir).unwrap();
        assert_eq!(spine.read_stream().cycle, 7);
        assert!((spine.read_stream().surprise_level - 0.3).abs() < 1e-6);
        assert_eq!(spine.read_attention().total_recalls, 55);
    }
    #[test]
    fn test_superblock_payload_carries_tick_seq() {
        let spine = temp_spine("sb_payload");
        assert_eq!(spine.read_superblock().unwrap().tick_seq, 0);
        spine.tick(1, 0.0, 0.0, 0, 0.0, 0, 0, 0.0);
        let after_first = spine.read_superblock().unwrap().tick_seq;
        spine.tick(2, 0.0, 0.0, 0, 0.0, 0, 0, 0.0);
        let after_second = spine.read_superblock().unwrap().tick_seq;
        // A sequence number, not a timestamp: two publishes in the same
        // millisecond still get two different values.
        assert!(after_first > 0, "first publish did not bump the sequence");
        assert_eq!(after_second, after_first + 1);
    }

    #[test]
    fn test_snapshot_pairs_stream_and_superblock() {
        let spine = temp_spine("snapshot");
        for cycle in 100..110 {
            spine.tick(cycle, 0.25, 0.5, cycle, 1.0, cycle, cycle, cycle as f64);
            let snap = spine
                .read_snapshot()
                .expect("seqlock should not starve a same-process reader");
            assert_eq!(snap.stream.cycle, cycle);
            assert_eq!(snap.epoch_ms, snap.stream.updated_ms as i64);
            assert_eq!(
                snap.sequence % 2,
                0,
                "a published snapshot is never taken mid-window"
            );
        }
    }

    #[test]
    fn test_write_slot_on_superblock_keeps_the_counter() {
        let spine = temp_spine("sb_slot");
        spine.tick(7, 0.0, 0.0, 0, 0.0, 0, 0, 0.0);
        let before = spine.publish_sequence();
        assert!(before > 0);
        // A page-wide write to slot 0 is truncated to the payload, so it can
        // never reset the counter that lives at the tail of that page.
        let junk = [0xABu8; PAGE_SIZE];
        spine.write_slot(Slot::Superblock, &junk);
        let after = spine.publish_sequence();
        // Writing slot 0 *is* a publish, so the counter advances by one
        // window — what must not happen is a reset, or an odd value left
        // behind that would wedge every reader.
        assert_eq!(after, before + 2, "counter did not advance as a publish");
        assert_eq!(after % 2, 0, "counter left inside a publish window");
        // The junk page has no magic, so the superblock stops decoding — the
        // point is that the *counter* is intact and even, and that a reader
        // sees a complete page (junk) rather than a mix of old and new.
        assert!(spine.read_superblock().is_none());
        assert!(!spine.publish_in_flight());
    }

    #[test]
    fn test_open_upgrades_a_version_one_superblock() {
        let dir = std::env::temp_dir().join(format!("m1c_spine_v1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let spine = Spine::open(&dir).unwrap();
            // Forge a version-1 payload by hand: magic, version 1, the WAL
            // coordinates, and epoch_ms at 24..32 — where tick_seq now lives.
            let mut payload = [0u8; SUPERBLOCK_BYTES];
            payload[0..8].copy_from_slice(&MAGIC.to_le_bytes());
            payload[8..12].copy_from_slice(&1u32.to_le_bytes());
            payload[12..16].copy_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
            payload[16..20].copy_from_slice(&(Slot::WalStart as u32).to_le_bytes());
            payload[20..24].copy_from_slice(&(Slot::WalStart as u32).to_le_bytes());
            payload[24..32].copy_from_slice(&1_700_000_000_000i64.to_le_bytes());
            let mut page = [0u8; PAGE_SIZE];
            page[..SUPERBLOCK_BYTES].copy_from_slice(&payload);
            // `write_slot` on slot 0 publishes the first `SUPERBLOCK_BYTES`
            // of the page verbatim, which is how a v1 layout gets seeded.
            spine.write_slot(Slot::Superblock, &page);
        }
        // `Spine::open` reads page 0, sees version 1, and republishes it as
        // version 2 with the WAL fields carried over.
        let spine = Spine::open(&dir).unwrap();
        let sb = spine.read_superblock().unwrap();
        assert_eq!(sb.version, SUPERBLOCK_VERSION);
        assert_eq!(sb.wal_start_page, Slot::WalStart as u32);
        assert_eq!(sb.tick_seq, 0);
        assert_eq!(sb.epoch_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_read_only_handle_sees_the_writers_publish() {
        let dir = std::env::temp_dir().join(format!("m1c_spine_ro_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let writer = Spine::open(&dir).unwrap();
        let reader = Spine::open_ro(&dir).unwrap();
        for cycle in 1..6 {
            writer.tick(cycle, 0.0, 0.0, cycle, 0.0, 0, 0, cycle as f64);
            let snap = reader
                .read_snapshot()
                .expect("read-only handle reads published snapshots");
            assert_eq!(snap.stream.cycle, cycle);
            assert_eq!(snap.stream.predicted_query_hash, cycle);
        }
        assert!(
            reader.resize(1 << 20).is_err(),
            "a read-only handle must not remap the file"
        );
    }

    /// The length check inside [`seq_atomic`] is the belt for every `seq_*`
    /// caller, so it is tested on its own: a mapping too short for the counter
    /// has to panic, not hand out an out-of-bounds `&AtomicU64`.
    #[test]
    #[should_panic(expected = "too short to carry a counter")]
    fn test_counter_access_on_a_short_mapping_panics() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("m1c_spine_short_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("tiny.bin");
        {
            let mut f = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .expect("create tiny file");
            f.write_all(&[0u8; 16]).expect("write");
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open tiny file");
        // SAFETY: a private 16-byte mapping of a file this test owns; the
        // point is that the helper refuses to read a counter out of it.
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file).expect("map") };
        let _ = seq_load(&mut mmap);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stalled_publish_window_is_recoverable() {
        let spine = temp_spine("stall");
        spine.tick(1, 0.0, 0.0, 0, 0.0, 0, 0, 0.0);
        // A writer killed between the two counter stores leaves the window open;
        // the only thing that can tell that apart from a writer that is merely
        // slow is the publisher lock, so that is what recovery takes.
        {
            let mut m = spine.mmap.lock();
            let _n = seq_begin(&mut m);
        }
        assert!(spine.publish_window_open(), "window should read as open");
        let before = spine.read_stream().cycle;
        assert!(spine.recover_stalled_publish());
        assert!(
            !spine.publish_window_open(),
            "recovery should close the window"
        );
        assert!(!spine.recover_stalled_publish(), "nothing to recover twice");
        assert_eq!(
            spine.read_stream().cycle,
            before,
            "recovery must not publish"
        );
        // And a wedged window is what `read_snapshot` itself recovers from,
        // not only the explicit call: without that, one killed writer would
        // leave every reader returning `None` for the life of the file.
        {
            let mut m = spine.mmap.lock();
            let _n = seq_begin(&mut m);
        }
        assert!(
            spine.read_snapshot().is_some(),
            "read_snapshot must heal a stalled window"
        );
        assert!(!spine.publish_window_open());
        spine.tick(2, 0.0, 0.0, 0, 0.0, 0, 0, 0.0);
        assert_eq!(spine.read_snapshot().unwrap().stream.cycle, 2);
    }

    #[test]
    fn test_summary_reports_the_publish_record() {
        let spine = temp_spine("summary_publish");
        spine.tick(11, 0.5, 0.6, 0, 0.7, 0, 0, 100.0);
        let out = format_summary(&spine);
        assert!(out.contains("Publish:"), "{out}");
        let seq = spine.read_snapshot().unwrap().tick_seq;
        assert!(
            out.contains(&format!("seq={seq}")),
            "the summary must print the counter, not a literal: {out}"
        );
    }

    #[test]
    fn test_summary_formats() {
        let spine = temp_spine("summary");
        spine.tick(10, 0.5, 0.6, 0, 0.7, 0, 0, 100.0);
        let out = format_summary(&spine);
        assert!(out.contains("Spine:"));
        assert!(out.contains("cycle=10"));
    }
}
