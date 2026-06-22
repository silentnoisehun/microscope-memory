//! Timeline — chronological memory log with time-window queries.
//!
//! Every store appends a TimelineEntry alongside the main append.bin,
//! giving the agent the ability to recall "what happened today/yesterday/
//! last session" without relying on similarity ranking.
//!
//! Binary format: `output/timeline.bin` (magic `TML1`)
//! Entry: 25 bytes header + text bytes
//!   [u8 magic 4 = "TML1"]
//!   [u64 ts_ms]
//!   [u8 layer_id]
//!   [u8 importance]
//!   [u8 depth]
//!   [u8 status (0=open, 1=resolved, 2=archived, 3=normal)]
//!   [u32 text_len]
//!   [bytes text_len]
//!
//! Magic is written only once (file header). Subsequent entries are
//! concatenated. Resolved/archived status flips are written as new
//! entries so the log stays append-only.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 4] = b"TML1";
const ENTRY_HEADER: usize = 8 + 1 + 1 + 1 + 1 + 4; // 16 bytes
pub const MAX_TEXT_LEN: usize = 4096;

/// Status byte in TimelineEntry.
pub const STATUS_NORMAL: u8 = 3;
pub const STATUS_OPEN: u8 = 0;
pub const STATUS_RESOLVED: u8 = 1;
pub const STATUS_ARCHIVED: u8 = 2;

/// One chronological memory entry.
#[derive(Clone, Debug)]
pub struct TimelineEntry {
    pub ts_ms: u64,
    pub layer_id: u8,
    pub importance: u8,
    pub depth: u8,
    pub status: u8,
    pub text: String,
}

/// Append a TimelineEntry. Thread-safe-ish via O_APPEND on POSIX; on Windows
/// the append mode also appends, but we serialize via FileLock at the caller.
pub fn append_entry(path: &Path, entry: &TimelineEntry) -> Result<(), String> {
    let needs_magic = !path.exists() || fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open timeline: {}", e))?;

    if needs_magic {
        file.write_all(MAGIC)
            .map_err(|e| format!("write timeline magic: {}", e))?;
    }

    let text_bytes = entry.text.as_bytes();
    let len = text_bytes.len().min(MAX_TEXT_LEN);

    file.write_all(&entry.ts_ms.to_le_bytes())
        .map_err(|e| format!("write ts: {}", e))?;
    file.write_all(&[entry.layer_id])
        .map_err(|e| format!("write layer: {}", e))?;
    file.write_all(&[entry.importance])
        .map_err(|e| format!("write importance: {}", e))?;
    file.write_all(&[entry.depth])
        .map_err(|e| format!("write depth: {}", e))?;
    file.write_all(&[entry.status])
        .map_err(|e| format!("write status: {}", e))?;
    file.write_all(&(len as u32).to_le_bytes())
        .map_err(|e| format!("write len: {}", e))?;
    file.write_all(&text_bytes[..len])
        .map_err(|e| format!("write text: {}", e))?;

    Ok(())
}

/// Read all entries from the timeline log. Returns them in append order
/// (oldest first) so the caller can sort/filter by window.
pub fn read_all(path: &Path) -> Vec<TimelineEntry> {
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
        let ts_ms = u64::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]);
        let layer_id = data[pos + 8];
        let importance = data[pos + 9];
        let depth = data[pos + 10];
        let status = data[pos + 11];
        let len = u32::from_le_bytes([
            data[pos + 12],
            data[pos + 13],
            data[pos + 14],
            data[pos + 15],
        ]) as usize;
        pos += ENTRY_HEADER;

        if pos + len > data.len() {
            break;
        }
        let text = String::from_utf8_lossy(&data[pos..pos + len]).to_string();
        pos += len;

        entries.push(TimelineEntry {
            ts_ms,
            layer_id,
            importance,
            depth,
            status,
            text,
        });
    }

    entries
}

/// A time window descriptor used for filtering.
#[derive(Clone, Debug, PartialEq)]
pub enum TimeWindow {
    /// Today (00:00 local-time approximate — we use UTC for simplicity).
    Today,
    /// Yesterday.
    Yesterday,
    /// Last N days (inclusive of today).
    LastDays(u64),
    /// Since a specific epoch_ms.
    Since(u64),
    /// Only entries newer than the last session boundary (we approximate
    /// this as the largest gap between consecutive entries, > 6h).
    LastSession,
    /// All entries.
    All,
}

impl TimeWindow {
    /// Parse a window string. Accepts:
    ///   "today", "yesterday", "last_N_days" (e.g. "last_3_days"),
    ///   "since:<iso8601 or epoch_ms>", "last_session", "all".
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "today" => Ok(TimeWindow::Today),
            "yesterday" => Ok(TimeWindow::Yesterday),
            "last_session" => Ok(TimeWindow::LastSession),
            "all" => Ok(TimeWindow::All),
            other if other.starts_with("last_") && other.ends_with("_days") => {
                let n: u64 = other[5..other.len() - 5]
                    .parse()
                    .map_err(|_| format!("invalid window: {}", s))?;
                if n == 0 || n > 365 {
                    return Err(format!("window days out of range: {}", n));
                }
                Ok(TimeWindow::LastDays(n))
            }
            other if other.starts_with("since:") => {
                let v = &other[6..];
                if let Ok(ms) = v.parse::<u64>() {
                    Ok(TimeWindow::Since(ms))
                } else {
                    // Try ISO-8601 (YYYY-MM-DDTHH:MM:SS or YYYY-MM-DD HH:MM:SS)
                    let ms = parse_iso8601_to_epoch_ms(v)
                        .ok_or_else(|| format!("invalid since date: {}", v))?;
                    Ok(TimeWindow::Since(ms))
                }
            }
            _ => Err(format!(
                "unknown window: {} (use today|yesterday|last_N_days|since:<date>|last_session|all)",
                s
            )),
        }
    }

    /// Resolve to a (start_ms, end_ms) window relative to now.
    /// LastSession returns (0, u64::MAX) — the actual session boundary is
    /// computed by the caller from the entry list.
    pub fn to_range(&self, now_ms: u64) -> (u64, u64) {
        const DAY_MS: u64 = 86_400_000;
        match self {
            TimeWindow::Today => {
                let start = now_ms - (now_ms % DAY_MS);
                (start, now_ms)
            }
            TimeWindow::Yesterday => {
                let today_start = now_ms - (now_ms % DAY_MS);
                (today_start - DAY_MS, today_start)
            }
            TimeWindow::LastDays(n) => {
                let n = (*n).max(1);
                let start = now_ms - n * DAY_MS;
                (start, now_ms)
            }
            TimeWindow::Since(ms) => (*ms, u64::MAX),
            TimeWindow::LastSession => (0, u64::MAX),
            TimeWindow::All => (0, u64::MAX),
        }
    }
}

/// Filter entries by time window. For `LastSession`, finds the last gap
/// > 6 hours between consecutive entries and returns everything after it.
/// `now_ms` is injected so tests can use a fixed timestamp.
pub fn filter(entries: &[TimelineEntry], window: &TimeWindow) -> Vec<TimelineEntry> {
    filter_at(entries, window, now_epoch_ms())
}

/// Same as `filter` but with explicit `now_ms` for deterministic testing.
pub fn filter_at(
    entries: &[TimelineEntry],
    window: &TimeWindow,
    now_ms: u64,
) -> Vec<TimelineEntry> {
    if entries.is_empty() {
        return Vec::new();
    }
    match window {
        TimeWindow::LastSession => {
            // Find the largest gap; that's the session boundary.
            let mut last_gap_pos = 0usize;
            let mut last_gap_ms: u64 = 0;
            for i in 1..entries.len() {
                let gap = entries[i].ts_ms.saturating_sub(entries[i - 1].ts_ms);
                if gap > last_gap_ms {
                    last_gap_ms = gap;
                    last_gap_pos = i;
                }
            }
            // 6h threshold: if the biggest gap is < 6h, fall back to "all".
            const SIX_H_MS: u64 = 6 * 3_600_000;
            if last_gap_ms < SIX_H_MS {
                entries.to_vec()
            } else {
                entries[last_gap_pos..].to_vec()
            }
        }
        _ => {
            let (start, end) = window.to_range(now_ms);
            entries
                .iter()
                .filter(|e| e.ts_ms >= start && e.ts_ms <= end)
                .cloned()
                .collect()
        }
    }
}

/// Current epoch in milliseconds.
pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Format an epoch_ms into "YYYY-MM-DD HH:MM" using a small pure-Rust
/// calendar (UTC). Good enough for human display in tool output.
pub fn format_ts(ms: u64) -> String {
    let secs = ms / 1000;
    let total_days = secs / 86400;
    let mut y = 1970u64;
    let mut remaining = total_days;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if remaining < diy {
            break;
        }
        remaining -= diy;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u64;
    for &md in &mdays {
        if remaining < md {
            break;
        }
        remaining -= md;
        mo += 1;
    }
    let day = remaining + 1;
    let secs_in_day = secs % 86400;
    let h = secs_in_day / 3600;
    let m = (secs_in_day % 3600) / 60;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, day, h, m)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Minimal ISO-8601 parser. Accepts:
///   YYYY-MM-DD
///   YYYY-MM-DDTHH:MM
///   YYYY-MM-DDTHH:MM:SS
///   YYYY-MM-DD HH:MM:SS
/// Returns epoch_ms (UTC) or None.
pub fn parse_iso8601_to_epoch_ms(s: &str) -> Option<u64> {
    let s = s.trim().replace(' ', "T");
    let mut parts = s.split('T');
    let date_part = parts.next()?;
    let time_part = parts.next();

    let date: Vec<u64> = date_part
        .split('-')
        .filter_map(|p| p.parse().ok())
        .collect();
    if date.len() != 3 {
        return None;
    }
    let (y, mo, d) = (date[0], date[1], date[2]);
    if !(1970..=3000).contains(&y) || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }

    let (mut h, mut mi, mut sec) = (0u64, 0u64, 0u64);
    if let Some(t) = time_part {
        let tps: Vec<u64> = t.split(':').filter_map(|p| p.parse().ok()).collect();
        if tps.len() >= 1 {
            h = tps[0];
        }
        if tps.len() >= 2 {
            mi = tps[1];
        }
        if tps.len() >= 3 {
            sec = tps[2];
        }
    }
    if h > 23 || mi > 59 || sec > 60 {
        return None;
    }

    // Days from 1970-01-01 to (y, mo, d)
    let mut days: u64 = 0;
    for yr in 1970..y {
        days += if is_leap(yr) { 366 } else { 365 };
    }
    let leap = is_leap(y);
    let mdays = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for mi_idx in 0..(mo - 1) as usize {
        days += mdays[mi_idx];
    }
    days += d - 1;

    Some(days * 86_400_000 + h * 3_600_000 + mi * 60_000 + sec * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, ts_ms: u64, status: u8) -> TimelineEntry {
        TimelineEntry {
            ts_ms,
            layer_id: 1,
            importance: 5,
            depth: 4,
            status,
            text: text.to_string(),
        }
    }

    #[test]
    fn test_append_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("timeline.bin");
        append_entry(&p, &entry("first", 1_700_000_000_000, STATUS_NORMAL)).unwrap();
        append_entry(&p, &entry("second", 1_700_001_000_000, STATUS_OPEN)).unwrap();
        let got = read_all(&p);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].text, "first");
        assert_eq!(got[1].status, STATUS_OPEN);
    }

    #[test]
    fn test_window_parse() {
        assert_eq!(TimeWindow::parse("today").unwrap(), TimeWindow::Today);
        assert_eq!(
            TimeWindow::parse("last_3_days").unwrap(),
            TimeWindow::LastDays(3)
        );
        assert_eq!(
            TimeWindow::parse("last_session").unwrap(),
            TimeWindow::LastSession
        );
        assert!(TimeWindow::parse("nonsense").is_err());
        assert!(TimeWindow::parse("last_0_days").is_err());
        assert!(TimeWindow::parse("last_1000_days").is_err());
    }

    #[test]
    fn test_filter_today() {
        let now = 1_700_000_000_000u64; // 2023-11-14 22:13:20 UTC
        let today_start = now - (now % 86_400_000);
        let entries = vec![
            entry("yesterday_late", today_start - 3_600_000, STATUS_NORMAL),
            entry("today_early", today_start + 1_000, STATUS_NORMAL),
            entry("now", now, STATUS_NORMAL),
        ];
        let f = filter_at(&entries, &TimeWindow::Today, now);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].text, "today_early");
    }

    #[test]
    fn test_filter_last_session() {
        // Three entries separated by a 10h gap.
        let entries = vec![
            entry("old", 1_700_000_000_000, STATUS_NORMAL),
            entry("older", 1_700_000_000_000 - 10 * 3_600_000, STATUS_NORMAL),
            // 10h gap here
            entry("recent", 1_700_000_000_000 + 1000, STATUS_NORMAL),
        ];
        let f = filter(&entries, &TimeWindow::LastSession);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].text, "recent");
    }

    #[test]
    fn test_iso8601_parse() {
        let ms = parse_iso8601_to_epoch_ms("2026-06-22T10:30:00").unwrap();
        // Just verify it's a sensible positive number.
        assert!(ms > 1_700_000_000_000);
        assert!(ms < 2_000_000_000_000);
    }

    #[test]
    fn test_format_ts() {
        // 2023-11-14 22:13:20 UTC
        let s = format_ts(1_700_000_000_000);
        assert_eq!(s, "2023-11-14 22:13");
    }

    #[test]
    fn test_window_last_days_range() {
        let now = 1_700_000_000_000u64;
        let (start, end) = TimeWindow::LastDays(2).to_range(now);
        assert!(start < now);
        assert_eq!(end, now);
        assert!(now - start <= 2 * 86_400_000);
    }
}
