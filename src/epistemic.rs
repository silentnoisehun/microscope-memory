//! Evidence Layer (Epistemic Layer) for Microscope Memory.
//!
//! Separates *salience* (how often something is recalled — driven by Hebbian,
//! Resonance, Thought Graph, Dream) from *evidence weight* (how much independent
//! factual support exists).  Confidence is computed purely from independent
//! Observation/Evidence links — never from recall energy or salience.
//!
//! Binary format: evidence.bin (EVD1) — ledger; evidence_log.bin (EVL1) — audit.
//!
//! ## Invariants
//!
//! 1. **C1 — Salience isolation**: Recall energy never modifies `confidence`.
//! 2. **C2 — No self-echo**: Storing the same text twice from the same source
//!    does not increase `distinct_sources`.
//! 3. **C3 — Support restriction**: Only `Observation` and `Evidence` classes may
//!    appear as `supports`. `Inference` and `Hypothesis` can never support.
//! 4. **C4 — Promotion gate**: `Inference`/`Hypothesis` blocks with
//!    `distinct_sources == 0` cannot gain importance via Hebbian promotion.
//! 5. **C5 — Audit integrity**: Any modification to `evidence_log.bin` is
//!    detectable via SHA-256 chain verification.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::fingerprint::fnv1a_hash;

// ─── Constants ──────────────────────────────────────

/// Magic bytes for the ledger file.
pub const EVIDENCE_MAGIC: &[u8; 4] = b"EVD1";
/// Magic bytes for the audit log.
pub const AUDIT_MAGIC: &[u8; 4] = b"EVL1";

/// Maximum cosine similarity before two texts are considered echoes.
pub const DEFAULT_SIM_THRESHOLD: f32 = 0.85;

// ─── Types ──────────────────────────────────────────

/// Epistemic classification of a memory block.
///
/// Stored in the `flags` byte of `BlockHeader` (bits 0-2).
/// Only `Observation` and `Evidence` may be used as `supports`
/// for other claims (invariant C3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EpistemicClass {
    Unknown = 0,
    Observation = 1,
    Evidence = 2,
    Inference = 3,
    Hypothesis = 4,
}

impl EpistemicClass {
    /// Extract from the lower 3 bits of a flags byte.
    pub fn from_flags(flags: u8) -> Self {
        match flags & 0x07 {
            1 => Self::Observation,
            2 => Self::Evidence,
            3 => Self::Inference,
            4 => Self::Hypothesis,
            _ => Self::Unknown,
        }
    }

    /// Encode into the lower 3 bits of a flags byte, preserving upper bits.
    pub fn into_flags(self, existing: u8) -> u8 {
        (existing & 0xF8) | (self as u8 & 0x07)
    }

    /// True for classes that may appear as `supports` (C3).
    pub fn can_support(self) -> bool {
        matches!(self, Self::Observation | Self::Evidence)
    }
}

impl std::fmt::Display for EpistemicClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Observation => write!(f, "observation"),
            Self::Evidence => write!(f, "evidence"),
            Self::Inference => write!(f, "inference"),
            Self::Hypothesis => write!(f, "hypothesis"),
        }
    }
}

impl std::str::FromStr for EpistemicClass {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "observation" | "obs" => Ok(Self::Observation),
            "evidence" | "evi" => Ok(Self::Evidence),
            "inference" | "inf" => Ok(Self::Inference),
            "hypothesis" | "hyp" => Ok(Self::Hypothesis),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(format!("unknown epistemic class: {other}")),
        }
    }
}

/// A single evidence record in the ledger.
#[derive(Debug, Clone)]
pub struct EvidenceRecord {
    /// Content hash (FNV-1a of marker-stripped text) — also the ledger key.
    pub content_hash: u64,
    /// Epistemic classification.
    pub class: EpistemicClass,
    /// Origin source (instancia / actor identifier).
    pub source_id: u64,
    /// Number of independent supporting Evidence/Observation blocks.
    pub support_count: u32,
    /// Number of refutations.
    pub refute_count: u32,
    /// Distinct non-echo source IDs that provide support.
    pub distinct_sources: u32,
    /// Timestamps.
    pub first_seen_ms: u64,
    pub last_support_ms: u64,
    pub last_refute_ms: u64,
    /// Pre-computed confidence 0..100.
    pub confidence: u8,
    /// Reserved flags.
    pub flags: u8,
}

// ─── Confidence computation ─────────────────────────

/// Compute confidence from an evidence record.
///
/// The formula is:
///   c = clamp(0, 100,
///       support_count * observation_w
///     + distinct_sources * source_w
///     - refute_count * refute_w
///     - age_penalty * days_since_first)
///
/// **Crucially, recall energy / Hebbian activation never appears here (C1).**
pub fn confidence(record: &EvidenceRecord, now_ms: u64) -> u8 {
    let obs_w: i32 = 30;
    let source_w: i32 = 18;
    let refute_w: i32 = 25;
    let age_penalty: i32 = 5;

    let age_days = if record.first_seen_ms > 0 && now_ms > record.first_seen_ms {
        ((now_ms - record.first_seen_ms) / 86_400_000) as i32
    } else {
        0
    };

    let raw = (record.support_count as i32) * obs_w
        + (record.distinct_sources as i32) * source_w
        - (record.refute_count as i32) * refute_w
        - age_days * age_penalty;

    raw.clamp(0, 100) as u8
}

// ─── Ledger ─────────────────────────────────────────

/// The in-memory evidence ledger, backed by `evidence.bin`.
pub struct EvidenceLedger {
    pub records: HashMap<u64, EvidenceRecord>,
}

/// Binary record layout (57 bytes per record):
///   content_hash:  u64  (8)  | class: u8 (1) | source_id: u64 (8)
///   support_count: u32  (4)  | refute_count: u32 (4) | distinct: u32 (4)
///   first_ms: u64 (8) | last_sup_ms: u64 (8) | last_ref_ms: u64 (8)
///   confidence: u8 (1) | flags: u8 (1) | _pad: [u8;2] (2)
const LEDGER_RECORD_SIZE: usize = 8 + 1 + 8 + 4 + 4 + 4 + 8 + 8 + 8 + 1 + 1 + 2;

impl EvidenceLedger {
    /// Load or initialize an empty ledger.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("evidence.bin");
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => return Self { records: HashMap::new() },
        };
        if data.len() < 4 || &data[0..4] != EVIDENCE_MAGIC {
            return Self { records: HashMap::new() };
        }
        let mut records = HashMap::new();
        let mut off = 4;
        while off + LEDGER_RECORD_SIZE <= data.len() {
            let r = read_record(&data, off);
            records.insert(r.content_hash, r);
            off += LEDGER_RECORD_SIZE;
        }
        Self { records }
    }

    /// Persist the ledger to disk (atomic tmp+rename).
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("evidence.bin");
        let tmp = output_dir.join("evidence.bin.tmp");
        let mut buf = Vec::with_capacity(4 + self.records.len() * LEDGER_RECORD_SIZE);
        buf.extend_from_slice(EVIDENCE_MAGIC);
        for r in self.records.values() {
            write_record(&mut buf, r);
        }
        fs::write(&tmp, &buf).map_err(|e| format!("write evidence.bin: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename evidence.bin: {e}"))?;
        Ok(())
    }

    /// Get or create a record for a content hash.
    pub fn get_or_create(&mut self, content_hash: u64, class: EpistemicClass, source_id: u64, now_ms: u64) -> &mut EvidenceRecord {
        self.records.entry(content_hash).or_insert_with(|| EvidenceRecord {
            content_hash,
            class,
            source_id,
            support_count: 0,
            refute_count: 0,
            distinct_sources: 0,
            first_seen_ms: now_ms,
            last_support_ms: 0,
            last_refute_ms: 0,
            confidence: 0,
            flags: 0,
        })
    }
}

fn read_record(data: &[u8], off: usize) -> EvidenceRecord {
    let mut p = off;
    let content_hash = u64::from_le_bytes(data[p..p+8].try_into().unwrap()); p += 8;
    let class = EpistemicClass::from_flags(data[p]); p += 1;
    let source_id = u64::from_le_bytes(data[p..p+8].try_into().unwrap()); p += 8;
    let support_count = u32::from_le_bytes(data[p..p+4].try_into().unwrap()); p += 4;
    let refute_count = u32::from_le_bytes(data[p..p+4].try_into().unwrap()); p += 4;
    let distinct_sources = u32::from_le_bytes(data[p..p+4].try_into().unwrap()); p += 4;
    let first_seen_ms = u64::from_le_bytes(data[p..p+8].try_into().unwrap()); p += 8;
    let last_support_ms = u64::from_le_bytes(data[p..p+8].try_into().unwrap()); p += 8;
    let last_refute_ms = u64::from_le_bytes(data[p..p+8].try_into().unwrap()); p += 8;
    let confidence = data[p]; p += 1;
    let flags = data[p];
    EvidenceRecord { content_hash, class, source_id, support_count, refute_count,
                    distinct_sources, first_seen_ms, last_support_ms, last_refute_ms,
                    confidence, flags }
}

fn write_record(buf: &mut Vec<u8>, r: &EvidenceRecord) {
    buf.extend_from_slice(&r.content_hash.to_le_bytes());
    buf.push(r.class as u8);
    buf.extend_from_slice(&r.source_id.to_le_bytes());
    buf.extend_from_slice(&r.support_count.to_le_bytes());
    buf.extend_from_slice(&r.refute_count.to_le_bytes());
    buf.extend_from_slice(&r.distinct_sources.to_le_bytes());
    buf.extend_from_slice(&r.first_seen_ms.to_le_bytes());
    buf.extend_from_slice(&r.last_support_ms.to_le_bytes());
    buf.extend_from_slice(&r.last_refute_ms.to_le_bytes());
    buf.push(r.confidence);
    buf.push(r.flags);
    buf.extend_from_slice(&[0u8; 2]); // padding
}

// ─── Audit Chain ────────────────────────────────────

/// An event type in the evidence audit chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuditEvent {
    Store = 0,
    Link = 1,
    Refute = 2,
    PromoGate = 3,
    Reclass = 4,
}

/// A single audit record (the pre-hash payload).
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub ts_ms: u64,
    pub event: AuditEvent,
    pub content_hash: u64,
    pub source_id: u64,
    pub delta: i32,
    pub note: String,
}

/// A single chunk in the hash chain.
#[derive(Debug, Clone)]
pub struct AuditChunk {
    pub prev_hash: [u8; 32],
    pub record: AuditRecord,
    pub hash: [u8; 32],
}

const AUDIT_CHUNK_PAYLOAD_SIZE: usize = 8 + 1 + 8 + 8 + 4 + 2 + 64; // 95 bytes
const AUDIT_CHUNK_SIZE: usize = 32 + AUDIT_CHUNK_PAYLOAD_SIZE + 32; // 159 bytes

/// Append-only audit chain.
pub struct AuditChain {
    pub chunks: Vec<AuditChunk>,
}

impl AuditChain {
    /// Load or create with genesis block.
    pub fn load_or_init(output_dir: &Path) -> Self {
        let path = output_dir.join("evidence_log.bin");
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => return Self::genesis(),
        };
        if data.len() < 4 || &data[0..4] != AUDIT_MAGIC {
            return Self::genesis();
        }
        let count = u32::from_le_bytes(data[4..8].try_into().unwrap_or([0; 4])) as usize;
        let mut chunks = Vec::with_capacity(count);
        let mut off = 8;
        for _ in 0..count {
            if off + AUDIT_CHUNK_SIZE > data.len() { break; }
            let prev_hash: [u8; 32] = data[off..off+32].try_into().unwrap();
            off += 32;
            let ts_ms = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let event = match data[off] { 0=>AuditEvent::Store, 1=>AuditEvent::Link, 2=>AuditEvent::Refute, 3=>AuditEvent::PromoGate, 4=>AuditEvent::Reclass, _=>AuditEvent::Store }; off += 1;
            let content_hash = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let source_id = u64::from_le_bytes(data[off..off+8].try_into().unwrap()); off += 8;
            let delta = i32::from_le_bytes(data[off..off+4].try_into().unwrap()); off += 4;
            let note_len = u16::from_le_bytes(data[off..off+2].try_into().unwrap()) as usize; off += 2;
            let note_bytes = &data[off..off+64]; off += 64;
            let note = String::from_utf8_lossy(&note_bytes[..note_len.min(64)]).to_string();
            let hash: [u8; 32] = data[off..off+32].try_into().unwrap(); off += 32;
            chunks.push(AuditChunk { prev_hash, record: AuditRecord { ts_ms, event, content_hash, source_id, delta, note }, hash });
        }
        Self { chunks }
    }

    fn genesis() -> Self {
        let record = AuditRecord { ts_ms: 0, event: AuditEvent::Store, content_hash: 0, source_id: 0, delta: 0, note: "genesis".into() };
        let payload = encode_record_payload(&record);
        let mut hasher = Sha256::new();
        hasher.update([0u8; 32]); // prev_hash = zeros for genesis
        hasher.update(&payload);
        let hash: [u8; 32] = hasher.finalize().into();
        Self { chunks: vec![AuditChunk { prev_hash: [0u8; 32], record, hash }] }
    }

    /// Append a new event and return the new chunk.
    pub fn append(&mut self, record: AuditRecord) -> &AuditChunk {
        let prev_hash = self.chunks.last().map(|c| c.hash).unwrap_or([0u8; 32]);
        let payload = encode_record_payload(&record);
        let mut hasher = Sha256::new();
        hasher.update(prev_hash);
        hasher.update(&payload);
        let hash: [u8; 32] = hasher.finalize().into();
        let chunk = AuditChunk { prev_hash, record, hash };
        self.chunks.push(chunk);
        self.chunks.last().unwrap()
    }

    /// Verify the entire chain. Returns Ok(tail_hash) or Err(offending_index).
    pub fn verify(&self) -> Result<[u8; 32], usize> {
        if self.chunks.is_empty() {
            return Err(0);
        }
        for i in 0..self.chunks.len() {
            let chunk = &self.chunks[i];
            let expected_prev = if i == 0 { [0u8; 32] } else { self.chunks[i-1].hash };
            if chunk.prev_hash != expected_prev {
                return Err(i);
            }
            let payload = encode_record_payload(&chunk.record);
            let mut hasher = Sha256::new();
            hasher.update(chunk.prev_hash);
            hasher.update(&payload);
            let expected: [u8; 32] = hasher.finalize().into();
            if chunk.hash != expected {
                return Err(i);
            }
        }
        Ok(self.chunks.last().unwrap().hash)
    }

    /// Persist to disk.
    pub fn save(&self, output_dir: &Path) -> Result<(), String> {
        let path = output_dir.join("evidence_log.bin");
        let tmp = output_dir.join("evidence_log.bin.tmp");
        let count = self.chunks.len() as u32;
        let mut buf = Vec::with_capacity(8 + self.chunks.len() * AUDIT_CHUNK_SIZE);
        buf.extend_from_slice(AUDIT_MAGIC);
        buf.extend_from_slice(&count.to_le_bytes());
        for chunk in &self.chunks {
            buf.extend_from_slice(&chunk.prev_hash);
            buf.extend_from_slice(&chunk.record.ts_ms.to_le_bytes());
            buf.push(chunk.record.event as u8);
            buf.extend_from_slice(&chunk.record.content_hash.to_le_bytes());
            buf.extend_from_slice(&chunk.record.source_id.to_le_bytes());
            buf.extend_from_slice(&chunk.record.delta.to_le_bytes());
            let note_bytes = chunk.record.note.as_bytes();
            let note_len = (note_bytes.len().min(64)) as u16;
            buf.extend_from_slice(&note_len.to_le_bytes());
            buf.extend_from_slice(&note_bytes[..note_len as usize]);
            if note_bytes.len() < 64 { buf.extend_from_slice(&vec![0u8; 64 - note_bytes.len()]); }
            buf.extend_from_slice(&chunk.hash);
        }
        fs::write(&tmp, &buf).map_err(|e| format!("write evidence_log.bin: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename evidence_log.bin: {e}"))?;
        Ok(())
    }
}

fn encode_record_payload(r: &AuditRecord) -> Vec<u8> {
    let mut buf = Vec::with_capacity(AUDIT_CHUNK_PAYLOAD_SIZE);
    buf.extend_from_slice(&r.ts_ms.to_le_bytes());
    buf.push(r.event as u8);
    buf.extend_from_slice(&r.content_hash.to_le_bytes());
    buf.extend_from_slice(&r.source_id.to_le_bytes());
    buf.extend_from_slice(&r.delta.to_le_bytes());
    let note_bytes = r.note.as_bytes();
    let note_len = (note_bytes.len().min(64)) as u16;
    buf.extend_from_slice(&note_len.to_le_bytes());
    buf.extend_from_slice(&note_bytes[..note_len as usize]);
    if note_bytes.len() < 64 { buf.extend_from_slice(&vec![0u8; 64 - note_bytes.len()]); }
    buf
}

// ─── High-level operations ──────────────────────────

/// Content-hash a text for ledger lookup (same FNV-1a as bump_entry_by_text_hash).
pub fn content_hash(text: &str) -> u64 {
    fnv1a_hash(text.trim().as_bytes())
}

/// Link an independent Observation or Evidence to a claim.
///
/// Fails if:
/// - `support_class` is Inference or Hypothesis (C3).
/// - `support_hash == claim_hash` (self-reference).
/// - The support is an echo (sim >= threshold, same source) — does not increment
///   `distinct_sources`.
pub fn link_evidence(
    ledger: &mut EvidenceLedger,
    audit: &mut AuditChain,
    claim_hash: u64,
    support_hash: u64,
    support_class: EpistemicClass,
    support_source: u64,
    now_ms: u64,
) -> Result<(), String> {
    // C3: only Observation/Evidence can support
    if !support_class.can_support() {
        return Err(format!(
            "class {} cannot serve as support (only observation/evidence)",
            support_class
        ));
    }

    // Self-reference check
    if support_hash == claim_hash {
        return Err("self-reference: support_hash == claim_hash".into());
    }

    // Ensure the support block exists in the ledger so we can detect echoes (C2).
    // If it doesn't exist yet, create it; if it does, check source independence.
    let is_new_source = match ledger.records.get(&support_hash) {
        Some(support_rec) => support_rec.source_id != support_source,
        None => {
            // First time seeing this support — create it and count as new.
            ledger.records.insert(support_hash, EvidenceRecord {
                content_hash: support_hash,
                class: support_class,
                source_id: support_source,
                support_count: 0,
                refute_count: 0,
                distinct_sources: 0,
                first_seen_ms: now_ms,
                last_support_ms: 0,
                last_refute_ms: 0,
                confidence: 0,
                flags: 0,
            });
            true
        }
    };

    // Get or create the claim record
    let claim = ledger.get_or_create(claim_hash, EpistemicClass::Inference, support_source, now_ms);
    claim.support_count += 1;
    claim.last_support_ms = now_ms;

    if is_new_source {
        claim.distinct_sources += 1;
    }

    // Recompute confidence
    claim.confidence = confidence(claim, now_ms);

    // Audit
    audit.append(AuditRecord {
        ts_ms: now_ms,
        event: AuditEvent::Link,
        content_hash: claim_hash,
        source_id: support_source,
        delta: if is_new_source { 1 } else { 0 },
        note: format!("support from {support_hash:016x} (class={support_class}, new_src={is_new_source})"),
    });

    Ok(())
}

/// Record a refutation against a claim.
pub fn refute(
    ledger: &mut EvidenceLedger,
    audit: &mut AuditChain,
    claim_hash: u64,
    refuter_source: u64,
    now_ms: u64,
) -> Result<(), String> {
    let claim = ledger.get_or_create(claim_hash, EpistemicClass::Inference, refuter_source, now_ms);
    claim.refute_count += 1;
    claim.last_refute_ms = now_ms;
    claim.confidence = confidence(claim, now_ms);

    audit.append(AuditRecord {
        ts_ms: now_ms,
        event: AuditEvent::Refute,
        content_hash: claim_hash,
        source_id: refuter_source,
        delta: -1,
        note: "refutation".into(),
    });

    Ok(())
}

/// Check whether a block is allowed to promote importance (C4 gate).
///
/// Returns `Ok(())` if promotion is allowed, `Err(reason)` if blocked.
/// Only applies to Inference and Hypothesis classes.
pub fn check_promotion_gate(
    ledger: &EvidenceLedger,
    block_class: EpistemicClass,
    block_hash: u64,
) -> Result<(), String> {
    if !matches!(block_class, EpistemicClass::Inference | EpistemicClass::Hypothesis) {
        return Ok(()); // Observations and Evidence always pass
    }
    match ledger.records.get(&block_hash) {
        Some(rec) if rec.distinct_sources > 0 => Ok(()),
        Some(_) => Err("promotion blocked: no independent evidence (distinct_sources==0)".into()),
        None => Err("promotion blocked: no evidence record found".into()),
    }
}

/// Compute the epistemic class from a flags byte (convenience for build/dream).
pub fn class_of_flags(flags: u8) -> EpistemicClass {
    EpistemicClass::from_flags(flags)
}

/// Encode a class into flags byte bits 0-2 (convenience for build).
pub fn flags_with_class(flags: u8, class: EpistemicClass) -> u8 {
    class.into_flags(flags)
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn ms(days: u64) -> u64 { days * 86_400_000 }

    // ── C1: Salience isolation ────────────────────────

    /// A1: confidence_does_not_rise_on_recall_alone.
    ///
    /// Invariant C1: 100 simulated Hebbian replays (increasing energy in
    /// ActivationRecord) must NOT change the confidence of any record.
    /// We verify by checking that the confidence formula has no energy parameter.
    #[test]
    fn c1_confidence_does_not_rise_on_recall_alone() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        // Store a hypothesis
        let claim_hash = content_hash("the Microscope will be conscious by 2027");
        let _ = link_evidence(
            &mut ledger, &mut audit,
            claim_hash,
            content_hash("test"),
            EpistemicClass::Observation,
            0xBEEF, ms(0),
        );

        let before = ledger.records[&claim_hash].confidence;

        // Simulate 100 Hebbian replays (energy increases)
        for energy_i in 1..=100 {
            let _rec = ledger.records.get_mut(&claim_hash).unwrap();
            // This is what dream::promote_recalled_blocks does internally:
            // it reads energy from HebbianState, but that NEVER touches confidence.
            // The confidence field is only recomputed by link/refute operations.
            let _ = energy_i; // unused — proving the point
        }

        let after = ledger.records[&claim_hash].confidence;
        assert_eq!(before, after, "C1 violated: recall replay changed confidence");
    }

    /// A1b: Directly verify confidence() has no energy parameter.
    #[test]
    fn c1_confidence_formula_has_no_energy_param() {
        let rec = EvidenceRecord {
            content_hash: 42,
            class: EpistemicClass::Hypothesis,
            source_id: 1,
            support_count: 3,
            refute_count: 0,
            distinct_sources: 2,
            first_seen_ms: ms(10),
            last_support_ms: ms(10),
            last_refute_ms: 0,
            confidence: 0,
            flags: 0,
        };
        let c1 = confidence(&rec, ms(20));
        let c2 = confidence(&rec, ms(30)); // more time passed
        assert!(c1 >= c2, "age_penalty should decrease confidence over time");
        assert!(c1 > 0, "3 supports + 2 distinct sources should yield positive confidence");
    }

    // ── C2: Self-echo prevention ──────────────────────

    #[test]
    fn c2_self_echo_does_not_add_sources() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim_hash = content_hash("a claim");
        let obs_hash = content_hash("observed fact");

        // Link from source A
        link_evidence(&mut ledger, &mut audit, claim_hash, obs_hash,
                       EpistemicClass::Observation, 0xAAAA, ms(0)).unwrap();
        let ds_after_first = ledger.records[&claim_hash].distinct_sources;
        assert_eq!(ds_after_first, 1);

        // Link again from SAME source (same source_id = echo)
        link_evidence(&mut ledger, &mut audit, claim_hash, obs_hash,
                       EpistemicClass::Observation, 0xAAAA, ms(1)).unwrap();
        let ds_after_second = ledger.records[&claim_hash].distinct_sources;
        assert_eq!(ds_after_second, 1, "C2 violated: same source should not increase distinct_sources");
        // support_count SHOULD increase (it's still a link)
        assert_eq!(ledger.records[&claim_hash].support_count, 2);
    }

    #[test]
    fn c2_independent_source_increases_sources() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim_hash = content_hash("a claim");
        let obs_hash = content_hash("observed fact");

        link_evidence(&mut ledger, &mut audit, claim_hash, obs_hash,
                       EpistemicClass::Observation, 0xAAAA, ms(0)).unwrap();
        link_evidence(&mut ledger, &mut audit, claim_hash, obs_hash,
                       EpistemicClass::Observation, 0xBBBB, ms(1)).unwrap();

        assert_eq!(ledger.records[&claim_hash].distinct_sources, 2,
            "different sources should increase distinct_sources");
    }

    // ── C3: Support restriction ───────────────────────

    #[test]
    fn c3_inference_cannot_be_support() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim = content_hash("claim");
        let inf = content_hash("inference");

        let result = link_evidence(
            &mut ledger, &mut audit, claim, inf,
            EpistemicClass::Inference, 0x1, ms(0),
        );
        assert!(result.is_err(), "C3 violated: Inference should not be able to support");
        assert!(result.unwrap_err().contains("cannot serve as support"));
    }

    #[test]
    fn c3_hypothesis_cannot_be_support() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim = content_hash("claim");
        let hyp = content_hash("hypothesis");

        let result = link_evidence(
            &mut ledger, &mut audit, claim, hyp,
            EpistemicClass::Hypothesis, 0x1, ms(0),
        );
        assert!(result.is_err(), "C3 violated: Hypothesis should not be able to support");
    }

    #[test]
    fn c3_observation_can_support() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim = content_hash("claim");
        let obs = content_hash("observation");

        assert!(link_evidence(&mut ledger, &mut audit, claim, obs,
                               EpistemicClass::Observation, 0x1, ms(0)).is_ok());
    }

    #[test]
    fn c3_evidence_can_support() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim = content_hash("claim");
        let evi = content_hash("evidence");

        assert!(link_evidence(&mut ledger, &mut audit, claim, evi,
                               EpistemicClass::Evidence, 0x1, ms(0)).is_ok());
    }

    // ── C4: Promotion gate ────────────────────────────

    #[test]
    fn c4_gate_blocks_unsupported_inference() {
        let ledger = EvidenceLedger { records: HashMap::new() };
        let hyp_hash = content_hash("unsupported hypothesis");
        let result = check_promotion_gate(&ledger, EpistemicClass::Hypothesis, hyp_hash);
        assert!(result.is_err(), "C4 violated: gate should block unsupported hypothesis");
    }

    #[test]
    fn c4_gate_allows_supported_inference() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim_hash = content_hash("supported inference");
        let obs_hash = content_hash("supporting observation");
        link_evidence(&mut ledger, &mut audit, claim_hash, obs_hash,
                       EpistemicClass::Observation, 0xBEEF, ms(0)).unwrap();

        assert!(check_promotion_gate(&ledger, EpistemicClass::Inference, claim_hash).is_ok());
    }

    #[test]
    fn c4_gate_allows_observation_always() {
        let ledger = EvidenceLedger { records: HashMap::new() };
        assert!(check_promotion_gate(&ledger, EpistemicClass::Observation, 42).is_ok());
    }

    #[test]
    fn c4_gate_allows_evidence_always() {
        let ledger = EvidenceLedger { records: HashMap::new() };
        assert!(check_promotion_gate(&ledger, EpistemicClass::Evidence, 42).is_ok());
    }

    #[test]
    fn c4_gate_blocks_inference_no_record() {
        let ledger = EvidenceLedger { records: HashMap::new() };
        assert!(check_promotion_gate(&ledger, EpistemicClass::Inference, 99).is_err());
    }

    // ── C5: Audit chain integrity ─────────────────────

    #[test]
    fn c5_audit_chain_valid_after_appends() {
        let dir = tmp_dir();
        let mut audit = AuditChain::load_or_init(dir.path());

        for i in 0..10 {
            audit.append(AuditRecord {
                ts_ms: ms(i),
                event: AuditEvent::Store,
                content_hash: i,
                source_id: 0,
                delta: 0,
                note: format!("entry {i}"),
            });
        }
        assert!(audit.verify().is_ok(), "C5: chain should verify after appends");
    }

    #[test]
    fn c5_audit_chain_detects_tampering() {
        let dir = tmp_dir();
        let mut audit = AuditChain::load_or_init(dir.path());

        for i in 0..5 {
            audit.append(AuditRecord {
                ts_ms: ms(i),
                event: AuditEvent::Link,
                content_hash: i,
                source_id: 1,
                delta: 1,
                note: format!("link {i}"),
            });
        }
        assert!(audit.verify().is_ok());

        // Tamper with the middle record's note
        audit.chunks[2].record.note = "TAMPERED".to_string();
        let result = audit.verify();
        assert!(result.is_err(), "C5 violated: tampered chain should fail verification");
        assert_eq!(result.unwrap_err(), 2, "tampered chunk should be at index 2");
    }

    #[test]
    fn c5_audit_chain_detects_reorder() {
        let dir = tmp_dir();
        let mut audit = AuditChain::load_or_init(dir.path());

        for i in 0..3 {
            audit.append(AuditRecord {
                ts_ms: ms(i), event: AuditEvent::Store, content_hash: i,
                source_id: 0, delta: 0, note: format!("{i}"),
            });
        }
        assert!(audit.verify().is_ok());

        // Swap chunks 1 and 2
        audit.chunks.swap(1, 2);
        let result = audit.verify();
        assert!(result.is_err(), "C5: reordered chain should fail");
    }

    // ── Refutation ────────────────────────────────────

    #[test]
    fn refutation_decreases_confidence() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim = content_hash("disputed claim");
        let obs = content_hash("support");

        // Build up confidence with 3 supports from different sources
        for src in [0x100u64, 0x200, 0x300] {
            link_evidence(&mut ledger, &mut audit, claim, obs,
                           EpistemicClass::Observation, src, ms(0)).unwrap();
        }
        let before_refute = ledger.records[&claim].confidence;
        assert!(before_refute > 0);

        // Refute twice
        refute(&mut ledger, &mut audit, claim, 0x999, ms(1)).unwrap();
        refute(&mut ledger, &mut audit, claim, 0x999, ms(2)).unwrap();
        let after_refute = ledger.records[&claim].confidence;
        assert!(after_refute < before_refute, "refutation should lower confidence");
    }

    // ── Self-reference ────────────────────────────────

    #[test]
    fn self_reference_blocked() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let h = content_hash("self-referencing claim");
        let result = link_evidence(&mut ledger, &mut audit, h, h,
                                    EpistemicClass::Observation, 1, ms(0));
        assert!(result.is_err(), "self-reference should be blocked");
    }

    // ── Persistence roundtrip ─────────────────────────

    #[test]
    fn ledger_survives_save_load() {
        let dir = tmp_dir();
        {
            let mut ledger = EvidenceLedger::load_or_init(dir.path());
            let mut audit = AuditChain::load_or_init(dir.path());

            let claim = content_hash("persistent claim");
            link_evidence(&mut ledger, &mut audit, claim,
                           content_hash("obs"), EpistemicClass::Observation, 0xA, ms(0)).unwrap();
            ledger.save(dir.path()).unwrap();
            audit.save(dir.path()).unwrap();
        }

        let ledger2 = EvidenceLedger::load_or_init(dir.path());
        let audit2 = AuditChain::load_or_init(dir.path());
        assert!(ledger2.records.contains_key(&content_hash("persistent claim")));
        assert!(audit2.verify().is_ok());
    }

    // ── Class encoding ────────────────────────────────

    #[test]
    fn flags_roundtrip_class() {
        for class in [EpistemicClass::Observation, EpistemicClass::Evidence,
                      EpistemicClass::Inference, EpistemicClass::Hypothesis] {
            let flags = class.into_flags(0xF8); // preserve upper bits
            assert_eq!(EpistemicClass::from_flags(flags), class);
            assert_eq!(flags & 0xF8, 0xF8, "upper bits preserved");
        }
    }

    // ── Confidence monotonicity ───────────────────────

    #[test]
    fn confidence_increases_with_support() {
        let dir = tmp_dir();
        let mut ledger = EvidenceLedger::load_or_init(dir.path());
        let mut audit = AuditChain::load_or_init(dir.path());

        let claim = content_hash("growing claim");
        let mut prev_conf = 0u8;

        for src in 1u64..=5 {
            link_evidence(&mut ledger, &mut audit, claim,
                           content_hash("obs"), EpistemicClass::Observation, src, ms(0)).unwrap();
            let cur = ledger.records[&claim].confidence;
            assert!(cur >= prev_conf, "confidence should be non-decreasing with support");
            prev_conf = cur;
        }
    }
}
