//! Open Loops — track unresolved tasks / TODOs / open questions.
//!
//! Distinct from the regular long-term store: an open loop is a memory
//! item the agent flagged as "still in progress". The agent can list open
//! loops at session-start to remember what was left unfinished, and
//! resolve them when the task completes.
//!
//! Storage: `output/open_loops.bin` (magic `OPL1`)
//! Entry: 22 bytes header + text bytes
//!   [u8 magic 4 = "OPL1"] (file header only)
//!   [u64 id]
//!   [u64 ts_ms]
//!   [u8 importance]
//!   [u8 status (0=open, 1=resolved, 2=archived)]
//!   [u32 text_len]
//!   [bytes text_len]
//!
//! The current max id is stored in `output/open_loops.idx` as plain text
//! (decimal, with newline). Resolution works by appending a new entry
//! with the same id and status=resolved — the log stays append-only and
//! `read_open()` returns the latest status per id.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::timeline::now_epoch_ms;

const MAGIC: &[u8; 4] = b"OPL1";
const ENTRY_HEADER: usize = 8 + 8 + 1 + 1 + 4; // 22 bytes
pub const MAX_TEXT_LEN: usize = 1024;

pub const STATUS_OPEN: u8 = 0;
pub const STATUS_RESOLVED: u8 = 1;
pub const STATUS_ARCHIVED: u8 = 2;

#[derive(Clone, Debug)]
pub struct OpenLoopEntry {
    pub id: u64,
    pub ts_ms: u64,
    pub importance: u8,
    pub status: u8,
    pub text: String,
}

/// Returns the next available id (max+1), starting at 1.
/// Reads `output/open_loops.idx`. If missing, returns 1.
pub fn next_id(dir: &Path) -> u64 {
    let p = dir.join("open_loops.idx");
    let raw = fs::read_to_string(&p).unwrap_or_default();
    raw.trim().parse::<u64>().map(|n| n + 1).unwrap_or(1)
}

fn persist_next_id(dir: &Path, id: u64) -> Result<(), String> {
    let p = dir.join("open_loops.idx");
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&p)
        .map_err(|e| format!("open open_loops.idx: {}", e))?;
    f.write_all(format!("{}\n", id).as_bytes())
        .map_err(|e| format!("write open_loops.idx: {}", e))?;
    Ok(())
}

/// Append a new loop. Assigns an id automatically, persists next-id,
/// and returns the assigned id. Status is open by default.
pub fn append_open(dir: &Path, text: &str, importance: u8) -> Result<u64, String> {
    let id = next_id(dir);
    let entry = OpenLoopEntry {
        id,
        ts_ms: now_epoch_ms(),
        importance,
        status: STATUS_OPEN,
        text: text.to_string(),
    };
    write_entry(&dir.join("open_loops.bin"), &entry)?;
    persist_next_id(dir, id)?;
    Ok(id)
}

/// Mark a loop resolved. Returns Ok(true) if a matching open id was
/// found, Ok(false) if not (already resolved or unknown id).
pub fn resolve(dir: &Path, id: u64) -> Result<bool, String> {
    let latest = read_latest(&dir.join("open_loops.bin"));
    if let Some(entry) = latest.iter().find(|e| e.id == id) {
        if entry.status == STATUS_RESOLVED {
            return Ok(false);
        }
        let resolved = OpenLoopEntry {
            id,
            ts_ms: now_epoch_ms(),
            importance: entry.importance,
            status: STATUS_RESOLVED,
            text: entry.text.clone(),
        };
        write_entry(&dir.join("open_loops.bin"), &resolved)?;
        // Mirror the resolution to the timeline log so timeline queries
        // reflect the latest status without scanning open_loops.
        let timeline_entry = crate::timeline::TimelineEntry {
            ts_ms: resolved.ts_ms,
            layer_id: crate::layer_to_id("long_term"),
            importance: resolved.importance,
            depth: 4,
            status: crate::timeline::STATUS_RESOLVED,
            text: format!("[loop#{} resolved] {}", id, entry.text),
        };
        let _ = crate::timeline::append_entry(&dir.join("timeline.bin"), &timeline_entry);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Archive a loop (similar to resolve but distinguishable).
pub fn archive(dir: &Path, id: u64) -> Result<bool, String> {
    let latest = read_latest(&dir.join("open_loops.bin"));
    if let Some(entry) = latest.iter().find(|e| e.id == id) {
        if entry.status == STATUS_ARCHIVED {
            return Ok(false);
        }
        let archived = OpenLoopEntry {
            id,
            ts_ms: now_epoch_ms(),
            importance: entry.importance,
            status: STATUS_ARCHIVED,
            text: entry.text.clone(),
        };
        write_entry(&dir.join("open_loops.bin"), &archived)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Read all entries (raw log). Each id may appear multiple times if it
/// was resolved/archived — use `read_latest` for current status.
pub fn read_all(path: &Path) -> Vec<OpenLoopEntry> {
    if !path.exists() {
        return Vec::new();
    }
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    if data.len() < 4 || &data[0..4] != MAGIC {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let mut pos = 4;
    while pos + ENTRY_HEADER <= data.len() {
        let id = u64::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]);
        let ts_ms = u64::from_le_bytes([
            data[pos + 8],
            data[pos + 9],
            data[pos + 10],
            data[pos + 11],
            data[pos + 12],
            data[pos + 13],
            data[pos + 14],
            data[pos + 15],
        ]);
        let importance = data[pos + 16];
        let status = data[pos + 17];
        let len = u32::from_le_bytes([
            data[pos + 18],
            data[pos + 19],
            data[pos + 20],
            data[pos + 21],
        ]) as usize;
        pos += ENTRY_HEADER;
        if pos + len > data.len() {
            break;
        }
        let text = String::from_utf8_lossy(&data[pos..pos + len]).to_string();
        pos += len;

        entries.push(OpenLoopEntry {
            id,
            ts_ms,
            importance,
            status,
            text,
        });
    }
    entries
}

/// Read current status of all known loops (latest entry per id wins).
pub fn read_latest(path: &Path) -> Vec<OpenLoopEntry> {
    let mut by_id: std::collections::HashMap<u64, OpenLoopEntry> = std::collections::HashMap::new();
    for e in read_all(path) {
        // Later writes win; loop preserves insertion order via iteration.
        by_id.insert(e.id, e);
    }
    let mut out: Vec<OpenLoopEntry> = by_id.into_values().collect();
    out.sort_by_key(|e| e.id);
    out
}

/// Return only currently-open loops (latest status = open), sorted by
/// importance desc, then ts desc.
pub fn read_open(path: &Path) -> Vec<OpenLoopEntry> {
    let mut v: Vec<OpenLoopEntry> = read_latest(path)
        .into_iter()
        .filter(|e| e.status == STATUS_OPEN)
        .collect();
    v.sort_by(|a, b| b.importance.cmp(&a.importance).then(b.ts_ms.cmp(&a.ts_ms)));
    v
}

fn write_entry(path: &Path, entry: &OpenLoopEntry) -> Result<(), String> {
    let needs_magic = !path.exists() || fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open open_loops: {}", e))?;

    if needs_magic {
        file.write_all(MAGIC)
            .map_err(|e| format!("write magic: {}", e))?;
    }

    let text_bytes = entry.text.as_bytes();
    let len = text_bytes.len().min(MAX_TEXT_LEN);

    file.write_all(&entry.id.to_le_bytes())
        .map_err(|e| format!("write id: {}", e))?;
    file.write_all(&entry.ts_ms.to_le_bytes())
        .map_err(|e| format!("write ts: {}", e))?;
    file.write_all(&[entry.importance])
        .map_err(|e| format!("write imp: {}", e))?;
    file.write_all(&[entry.status])
        .map_err(|e| format!("write status: {}", e))?;
    file.write_all(&(len as u32).to_le_bytes())
        .map_err(|e| format!("write len: {}", e))?;
    file.write_all(&text_bytes[..len])
        .map_err(|e| format!("write text: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_read_open() {
        let dir = tempfile::tempdir().unwrap();
        let id1 = append_open(dir.path(), "task A", 7).unwrap();
        let id2 = append_open(dir.path(), "task B", 9).unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let open = read_open(&dir.path().join("open_loops.bin"));
        assert_eq!(open.len(), 2);
        // Importance desc → B first.
        assert_eq!(open[0].text, "task B");
        assert_eq!(open[1].text, "task A");
    }

    #[test]
    fn test_resolve_hides_from_open() {
        let dir = tempfile::tempdir().unwrap();
        let id = append_open(dir.path(), "doomed task", 5).unwrap();
        let changed = resolve(dir.path(), id).unwrap();
        assert!(changed);

        let open = read_open(&dir.path().join("open_loops.bin"));
        assert!(open.is_empty());

        // Idempotent: second resolve returns false.
        let changed2 = resolve(dir.path(), id).unwrap();
        assert!(!changed2);
    }

    #[test]
    fn test_latest_status_per_id() {
        let dir = tempfile::tempdir().unwrap();
        let id = append_open(dir.path(), "test", 5).unwrap();
        resolve(dir.path(), id).unwrap();

        let latest = read_latest(&dir.path().join("open_loops.bin"));
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].id, id);
        assert_eq!(latest[0].status, STATUS_RESOLVED);

        // Raw log should have 2 entries (open + resolved).
        let raw = read_all(&dir.path().join("open_loops.bin"));
        assert_eq!(raw.len(), 2);
    }

    #[test]
    fn test_resolve_unknown_id_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let changed = resolve(dir.path(), 9999).unwrap();
        assert!(!changed);
    }
}
