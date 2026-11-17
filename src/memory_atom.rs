//! Durable, stable memory atoms.
//!
//! The hierarchical binary index is excellent for fast retrieval, but an
//! index position is not a memory identity: every rebuild can reassign block
//! ids.  A memory atom adds the missing durable identity layer.
//!
//! An atom has a stable SHA-256 id, a semantic kind, an optional subject
//! key, source, confidence, creation time and an explicit relation to an
//! earlier atom.  When two different statements share a subject, the newer
//! atom records a `Contradicts` relation automatically; an explicit
//! `supersedes` id records a stronger `Supersedes` relation.  The old atom
//! remains in the ledger, so the history is auditable instead of silently
//! overwritten.
//!
//! Binary format: `output/memory_atoms.bin` (`MAT1`).  The file is
//! append-only; loading resolves repeated ids by taking the newest record.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 4] = b"MAT1";
// id(16) subject(16) content_hash(8) kind(1) reserved(1) relation(1)
// source_id(8) confidence(1) created_ms(8) relation_target(16) text_len(4)
const RECORD_HEADER: usize = 16 + 16 + 8 + 1 + 1 + 1 + 8 + 1 + 8 + 16 + 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AtomKind {
    Observation = 0,
    Fact = 1,
    Preference = 2,
    Decision = 3,
    Correction = 4,
    Task = 5,
    Episode = 6,
}

impl AtomKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "observation" => Some(Self::Observation),
            "fact" => Some(Self::Fact),
            "preference" => Some(Self::Preference),
            "decision" => Some(Self::Decision),
            "correction" => Some(Self::Correction),
            "task" => Some(Self::Task),
            "episode" => Some(Self::Episode),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Decision => "decision",
            Self::Correction => "correction",
            Self::Task => "task",
            Self::Episode => "episode",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AtomRelation {
    None = 0,
    Supersedes = 1,
    Contradicts = 2,
}

impl AtomRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Supersedes => "supersedes",
            Self::Contradicts => "contradicts",
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemoryAtom {
    pub id: [u8; 16],
    pub subject_id: [u8; 16],
    pub content_hash: u64,
    pub kind: AtomKind,
    pub source_id: u64,
    pub confidence: u8,
    pub created_ms: u64,
    pub relation: AtomRelation,
    pub relation_target: [u8; 16],
    pub text: String,
}

impl MemoryAtom {
    pub fn id_hex(&self) -> String {
        self.id.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn subject_hex(&self) -> String {
        self.subject_id.iter().map(|b| format!("{b:02x}")).collect()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sha16(parts: &[&[u8]]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    let hash = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash[..16]);
    out
}

fn source_hash(source: &str) -> u64 {
    let hash = Sha256::digest(source.trim().as_bytes());
    u64::from_le_bytes(hash[..8].try_into().unwrap_or([0; 8]))
}

/// Normalize a subject key.  Subject equality is explicit: two memories
/// conflict when the caller says they are about the same subject.
pub fn normalize_subject(subject: &str) -> String {
    let mut out = String::new();
    let mut space = true;
    for ch in subject.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Canonical text used to associate a persisted memory with a durable atom.
///
/// The MCP store path prefixes the appended memory with a session marker
/// (`[sid-1234] ...`) while the atom ledger keeps the original user text.
/// Comparing the two verbatim made contradiction and supersession filtering
/// miss the persisted row.  Session markers are transport metadata, not part
/// of the remembered statement, so remove them for identity comparisons.
pub fn canonical_memory_text(text: &str) -> String {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("[sid-") {
        if let Some((digits, tail)) = rest.split_once("] ") {
            if !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()) {
                return tail.trim_start().to_string();
            }
        }
    }
    trimmed.to_string()
}

/// True when a possibly truncated core-memory view still represents the
/// atom's complete statement.
///
/// Core-memory snapshots cap displayed text at 500 characters, while the atom
/// ledger stores the full statement. Exact comparison therefore failed for
/// long corrections: the old atom was marked inactive, but its truncated copy
/// did not match and reappeared in the snapshot. A prefix match is safe here
/// because the candidate is a bounded prefix of the original statement, not
/// an arbitrary search excerpt.
pub fn atom_text_matches(candidate: &str, atom_text: &str) -> bool {
    let candidate = canonical_memory_text(candidate);
    let atom_text = canonical_memory_text(atom_text);
    candidate == atom_text || atom_text.starts_with(&candidate)
}

/// Build an atom id from project, subject and exact content.
pub fn atom_id(project_id: &[u8; 16], subject: &str, text: &str) -> [u8; 16] {
    let subject_key = normalize_subject(subject);
    sha16(&[project_id, subject_key.as_bytes(), text.trim().as_bytes()])
}

pub fn subject_id(subject: &str) -> [u8; 16] {
    let normalized = normalize_subject(subject);
    sha16(&[b"subject", normalized.as_bytes()])
}

/// A parsed atom ledger.
#[derive(Clone, Debug, Default)]
pub struct AtomLedger {
    atoms: Vec<MemoryAtom>,
}

impl AtomLedger {
    pub fn load(path: &Path) -> Self {
        let Ok(data) = fs::read(path) else {
            return Self::default();
        };
        if data.len() < 4 || &data[..4] != MAGIC {
            return Self::default();
        }
        let mut pos = 4;
        let mut latest: HashMap<[u8; 16], MemoryAtom> = HashMap::new();
        while pos + RECORD_HEADER <= data.len() {
            let mut id = [0u8; 16];
            id.copy_from_slice(&data[pos..pos + 16]);
            let mut subject = [0u8; 16];
            subject.copy_from_slice(&data[pos + 16..pos + 32]);
            let content_hash =
                u64::from_le_bytes(data[pos + 32..pos + 40].try_into().unwrap_or([0; 8]));
            let kind = match data[pos + 40] {
                0 => AtomKind::Observation,
                1 => AtomKind::Fact,
                2 => AtomKind::Preference,
                3 => AtomKind::Decision,
                4 => AtomKind::Correction,
                5 => AtomKind::Task,
                6 => AtomKind::Episode,
                _ => AtomKind::Observation,
            };
            let source_id =
                u64::from_le_bytes(data[pos + 43..pos + 51].try_into().unwrap_or([0; 8]));
            let confidence = data[pos + 51];
            let created_ms =
                u64::from_le_bytes(data[pos + 52..pos + 60].try_into().unwrap_or([0; 8]));
            let relation = match data[pos + 42] {
                1 => AtomRelation::Supersedes,
                2 => AtomRelation::Contradicts,
                _ => AtomRelation::None,
            };
            let mut relation_target = [0u8; 16];
            relation_target.copy_from_slice(&data[pos + 60..pos + 76]);
            let text_len =
                u32::from_le_bytes(data[pos + 76..pos + 80].try_into().unwrap_or([0; 4])) as usize;
            pos += RECORD_HEADER;
            if pos + text_len > data.len() {
                break;
            }
            let text = String::from_utf8_lossy(&data[pos..pos + text_len]).to_string();
            pos += text_len;
            let atom = MemoryAtom {
                id,
                subject_id: subject,
                content_hash,
                kind,
                source_id,
                confidence,
                created_ms,
                relation,
                relation_target,
                text,
            };
            let replace = latest
                .get(&id)
                .map(|old| atom.created_ms >= old.created_ms)
                .unwrap_or(true);
            if replace {
                latest.insert(id, atom);
            }
        }
        let mut atoms: Vec<_> = latest.into_values().collect();
        atoms.sort_by(|a, b| {
            a.created_ms
                .cmp(&b.created_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        Self { atoms }
    }

    pub fn atoms(&self) -> &[MemoryAtom] {
        &self.atoms
    }

    pub fn get(&self, id: &[u8; 16]) -> Option<&MemoryAtom> {
        self.atoms.iter().find(|atom| &atom.id == id)
    }

    /// Confidence of the newest atom whose canonical statement matches `text`.
    pub fn confidence_for_text(&self, text: &str) -> Option<u8> {
        self.atoms
            .iter()
            .filter(|atom| atom_text_matches(text, &atom.text))
            .max_by_key(|atom| atom.created_ms)
            .map(|atom| atom.confidence)
    }

    /// True when this statement is superseded or contradicted by a newer atom.
    pub fn is_inactive_text(&self, text: &str) -> bool {
        let inactive = self.inactive_texts();
        inactive.contains(&canonical_memory_text(text))
            || self.atoms().iter().any(|atom| {
                atom_text_matches(text, &atom.text)
                    && inactive.contains(&canonical_memory_text(&atom.text))
            })
    }

    pub fn latest_for_subject(&self, subject: &str) -> Option<&MemoryAtom> {
        let sid = subject_id(subject);
        self.atoms
            .iter()
            .filter(|atom| atom.subject_id == sid)
            .max_by(|a, b| {
                // Subject identity defines the current version.  Confidence is
                // a trust signal for choosing between different memories, not
                // a veto on a later correction: otherwise a newly observed
                // correction with a lower confidence silently leaves the old
                // value active and both facts remain visible.
                a.created_ms
                    .cmp(&b.created_ms)
                    .then_with(|| a.id.cmp(&b.id))
            })
    }

    /// Atom texts that are no longer active for their subject.
    ///
    /// A superseded atom is inactive.  A contradicted atom is also hidden
    /// from the active core-memory view when a newer atom for the same
    /// subject exists, while both remain in the ledger for audit.
    pub fn inactive_texts(&self) -> std::collections::HashSet<String> {
        let mut inactive = std::collections::HashSet::new();
        let mut latest_by_subject: HashMap<[u8; 16], &MemoryAtom> = HashMap::new();
        for atom in &self.atoms {
            let replace = latest_by_subject
                .get(&atom.subject_id)
                .map(|old| {
                    atom.created_ms > old.created_ms
                        || (atom.created_ms == old.created_ms && atom.id >= old.id)
                })
                .unwrap_or(true);
            if replace {
                latest_by_subject.insert(atom.subject_id, atom);
            }
        }
        for atom in &self.atoms {
            match atom.relation {
                AtomRelation::Supersedes => {
                    if let Some(target) = self.get(&atom.relation_target) {
                        inactive.insert(canonical_memory_text(&target.text));
                    }
                }
                AtomRelation::Contradicts => {
                    if let Some(target) = self.get(&atom.relation_target) {
                        if latest_by_subject
                            .get(&target.subject_id)
                            .map(|latest| latest.id != target.id)
                            .unwrap_or(false)
                        {
                            inactive.insert(canonical_memory_text(&target.text));
                        }
                    }
                }
                AtomRelation::None => {}
            }
        }
        inactive
    }

    /// Append one atom.  The file is append-only and receives the atom in a
    /// single append; a failed write leaves the previous ledger intact.
    pub fn append(&mut self, path: &Path, atom: MemoryAtom) -> Result<(), String> {
        let text_bytes = atom.text.as_bytes();
        if text_bytes.len() > u16::MAX as usize {
            return Err("memory atom text exceeds 65535 bytes".into());
        }
        let needs_magic =
            !path.exists() || fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("open memory atom ledger: {e}"))?;
        let mut record = Vec::with_capacity(RECORD_HEADER + text_bytes.len());
        record.extend_from_slice(&atom.id);
        record.extend_from_slice(&atom.subject_id);
        record.extend_from_slice(&atom.content_hash.to_le_bytes());
        record.push(atom.kind as u8);
        record.push(0); // reserved lifecycle byte
        record.push(atom.relation as u8);
        record.extend_from_slice(&atom.source_id.to_le_bytes());
        record.push(atom.confidence);
        record.extend_from_slice(&atom.created_ms.to_le_bytes());
        record.extend_from_slice(&atom.relation_target);
        record.extend_from_slice(&(text_bytes.len() as u32).to_le_bytes());
        record.extend_from_slice(text_bytes);
        if needs_magic {
            file.write_all(MAGIC)
                .map_err(|e| format!("write memory atom magic: {e}"))?;
        }
        file.write_all(&record)
            .map_err(|e| format!("write memory atom: {e}"))?;
        file.flush()
            .map_err(|e| format!("flush memory atom ledger: {e}"))?;

        if let Some(old) = self
            .atoms
            .iter_mut()
            .find(|candidate| candidate.id == atom.id)
        {
            *old = atom.clone();
        } else {
            self.atoms.push(atom.clone());
        }
        Ok(())
    }
}

/// Record an atom and infer a contradiction relation when the same subject
/// already has a different active statement.
#[allow(clippy::too_many_arguments)]
pub fn record_atom(
    output_dir: &Path,
    project_id: &[u8; 16],
    text: &str,
    kind: AtomKind,
    subject: &str,
    source: &str,
    confidence: u8,
    supersedes: Option<[u8; 16]>,
) -> Result<MemoryAtom, String> {
    let path = output_dir.join("memory_atoms.bin");
    let mut ledger = AtomLedger::load(&path);
    let normalized_subject = normalize_subject(subject);
    let id = atom_id(project_id, &normalized_subject, text);
    let relation_target = if let Some(target) = supersedes {
        target
    } else if !normalized_subject.is_empty() {
        ledger
            .latest_for_subject(&normalized_subject)
            .filter(|old| old.content_hash != crate::epistemic::content_hash(text))
            .map(|old| old.id)
            .unwrap_or([0u8; 16])
    } else {
        [0u8; 16]
    };
    let relation = if relation_target == [0u8; 16] {
        AtomRelation::None
    } else if supersedes.is_some() {
        AtomRelation::Supersedes
    } else {
        AtomRelation::Contradicts
    };
    let subject_key = if normalized_subject.is_empty() {
        [0u8; 16]
    } else {
        subject_id(&normalized_subject)
    };
    // Keep ledger order strictly monotonic even when two stores happen in the
    // same millisecond.  Version selection relies on creation time; a tie
    // would otherwise be resolved by an unrelated hash id and could resurrect
    // the old value.
    let created_ms = ledger
        .atoms
        .iter()
        .map(|atom| atom.created_ms)
        .max()
        .map(|last| last.saturating_add(1))
        .unwrap_or_else(now_ms)
        .max(now_ms());
    let atom = MemoryAtom {
        id,
        subject_id: subject_key,
        content_hash: crate::epistemic::content_hash(text),
        kind,
        source_id: source_hash(source),
        confidence: confidence.clamp(0, 100),
        created_ms,
        relation,
        relation_target,
        text: text.to_string(),
    };
    ledger.append(&path, atom.clone())?;
    Ok(atom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_normalization_is_stable() {
        assert_eq!(
            normalize_subject("  Project   Language "),
            "project language"
        );
        assert_eq!(normalize_subject("PROJECT-Language"), "project language");
    }

    #[test]
    fn session_tag_is_transport_metadata_not_memory_identity() {
        assert_eq!(
            canonical_memory_text("[sid-0042] A projekt nyelve Rust."),
            "A projekt nyelve Rust."
        );
        assert_eq!(
            canonical_memory_text("A projekt nyelve Rust."),
            "A projekt nyelve Rust."
        );
    }

    #[test]
    fn truncated_core_memory_still_matches_full_atom() {
        let full = format!("{} — további hosszú részlet", "A".repeat(600));
        let truncated = full.chars().take(500).collect::<String>();
        assert!(atom_text_matches(&truncated, &full));
        assert!(!atom_text_matches("másik emlék", &full));
    }

    #[test]
    fn same_subject_different_text_records_contradiction() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::types::ProjectId::GLOBAL;
        let first = record_atom(
            dir.path(),
            &project.0,
            "A projekt nyelve Rust.",
            AtomKind::Fact,
            "project language",
            "test",
            80,
            None,
        )
        .unwrap();
        let second = record_atom(
            dir.path(),
            &project.0,
            "A projekt nyelve Python.",
            AtomKind::Correction,
            "project language",
            "test",
            90,
            None,
        )
        .unwrap();
        assert_eq!(second.relation, AtomRelation::Contradicts);
        assert_eq!(second.relation_target, first.id);
        let ledger = AtomLedger::load(&dir.path().join("memory_atoms.bin"));
        assert!(ledger.inactive_texts().contains("A projekt nyelve Rust."));
        assert!(!ledger.inactive_texts().contains("A projekt nyelve Python."));
    }

    #[test]
    fn explicit_supersede_wins_over_automatic_contradiction() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::types::ProjectId::GLOBAL;
        let first = record_atom(
            dir.path(),
            &project.0,
            "old",
            AtomKind::Fact,
            "subject",
            "test",
            80,
            None,
        )
        .unwrap();
        let second = record_atom(
            dir.path(),
            &project.0,
            "new",
            AtomKind::Correction,
            "subject",
            "test",
            90,
            Some(first.id),
        )
        .unwrap();
        assert_eq!(second.relation, AtomRelation::Supersedes);
    }

    #[test]
    fn later_lower_confidence_correction_replaces_old_subject_value() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::types::ProjectId::GLOBAL;
        let old = record_atom(
            dir.path(),
            &project.0,
            "A projekt nyelve Rust.",
            AtomKind::Fact,
            "project language",
            "test",
            90,
            None,
        )
        .unwrap();
        let correction = record_atom(
            dir.path(),
            &project.0,
            "A projekt nyelve Python.",
            AtomKind::Correction,
            "project language",
            "test",
            50,
            None,
        )
        .unwrap();

        assert_eq!(correction.relation, AtomRelation::Contradicts);
        assert_eq!(correction.relation_target, old.id);

        let ledger = AtomLedger::load(&dir.path().join("memory_atoms.bin"));
        let inactive = ledger.inactive_texts();
        assert!(inactive.contains("A projekt nyelve Rust."));
        assert!(!inactive.contains("A projekt nyelve Python."));
    }

    #[test]
    fn inactive_text_matches_tagged_persisted_memory() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::types::ProjectId::GLOBAL;
        record_atom(
            dir.path(),
            &project.0,
            "A projekt nyelve Rust.",
            AtomKind::Fact,
            "project language",
            "test",
            80,
            None,
        )
        .unwrap();
        record_atom(
            dir.path(),
            &project.0,
            "A projekt nyelve Python.",
            AtomKind::Correction,
            "project language",
            "test",
            90,
            None,
        )
        .unwrap();
        let ledger = AtomLedger::load(&dir.path().join("memory_atoms.bin"));
        let inactive = ledger.inactive_texts();
        assert!(inactive.contains("A projekt nyelve Rust."));
        assert!(inactive.contains(&canonical_memory_text("[sid-0042] A projekt nyelve Rust.")));
    }

    #[test]
    fn inactive_lookup_accepts_truncated_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::types::ProjectId::GLOBAL;
        let old = format!("{} — régi hosszú állítás", "A".repeat(600));
        let new = "új rövid javítás".to_string();
        record_atom(
            dir.path(),
            &project.0,
            &old,
            AtomKind::Fact,
            "long subject",
            "test",
            80,
            None,
        )
        .unwrap();
        record_atom(
            dir.path(),
            &project.0,
            &new,
            AtomKind::Correction,
            "long subject",
            "test",
            95,
            None,
        )
        .unwrap();
        let ledger = AtomLedger::load(&dir.path().join("memory_atoms.bin"));
        let truncated = old.chars().take(500).collect::<String>();
        assert!(ledger.is_inactive_text(&truncated));
        assert!(!ledger.is_inactive_text(&new));
    }
}
