//! Auto-Context — assemble a compact, always-fresh session snapshot.
//!
//! Built once at session start (or any time the agent asks "what's my
//! current state?"). Combines:
//!   * timeline[last_session] — top-N chronological stores
//!   * open_loops — top-N unresolved tasks
//!   * emotional_field — current mood centroid + energy
//!   * system_state — block count + depth distribution
//!
//! The output is plain text formatted for direct inclusion in an LLM
//! prompt. Every MCP client gets the same format because the assembly
//! happens server-side.

use std::path::Path;

use crate::open_loops;
use crate::timeline;
use crate::LAYER_NAMES;

/// How many items to include in the auto-context snapshot.
pub const TIMELINE_TOP_N: usize = 5;
pub const LOOPS_TOP_N: usize = 5;
/// Max core memories to embed (full text, not just metadata).
pub const CORE_MEMORIES_TOP_N: usize = 3;
/// Importance threshold for core memories (0-10).
pub const CORE_MEMORIES_MIN_IMP: u8 = 7;
/// Max chars per core memory.
pub const CORE_MEMORY_MAX_CHARS: usize = 500;
/// Confidence at or above this value counts as explicitly confirmed for the
/// core-memory policy.  The default atom confidence is 50, so merely storing
/// an atom must not be mistaken for confirmation.
pub const CORE_MEMORY_CONFIRMED_MIN_CONFIDENCE: u8 = 70;

/// A core memory entry with full text content.
#[derive(Debug, Clone, Default)]
pub struct CoreMemory {
    pub text: String,
    pub layer: String,
    pub importance: u8,
    pub depth: u8,
    /// Freshness in the form of the entry timestamp when available.
    pub ts_ms: Option<u64>,
    /// Evidence confidence 0..100 when the ledger knows this content.
    pub confidence: Option<u8>,
}

/// Assembled auto-context, ready to format for an LLM prompt.
#[derive(Debug, Clone, Default)]
pub struct AutoContext {
    pub timeline_entries: Vec<timeline::TimelineEntry>,
    pub open_loops: Vec<open_loops::OpenLoopEntry>,
    pub block_count: usize,
    pub depth_breakdown: Vec<(u8, usize)>,
    pub append_log_size: usize,
    pub last_session_anchor_ms: Option<u64>,
    /// Full-text top memories for direct LLM consumption.
    pub core_memories: Vec<CoreMemory>,
}

pub fn latest_observation_states(
    entries: &[timeline::TimelineEntry],
) -> std::collections::HashMap<String, (u64, u8)> {
    let mut states = std::collections::HashMap::new();
    for entry in entries {
        let key = entry.text.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let replace = states
            .get(&key)
            .map(|(ts, _)| entry.ts_ms >= *ts)
            .unwrap_or(true);
        if replace {
            states.insert(key, (entry.ts_ms, entry.status));
        }
    }
    states
}

fn sort_core_memories(candidates: &mut [CoreMemory], search: &crate::config::Search) {
    candidates.sort_by(|a, b| {
        let confirmed = |memory: &CoreMemory| {
            memory
                .confidence
                .map(|confidence| confidence >= CORE_MEMORY_CONFIRMED_MIN_CONFIDENCE)
                .unwrap_or(false)
        };
        let confirmed_order = || {
            if search.core_memory_prefer_confirmed {
                confirmed(b).cmp(&confirmed(a))
            } else {
                std::cmp::Ordering::Equal
            }
        };
        confirmed_order()
            .then_with(|| b.importance.cmp(&a.importance))
            .then_with(|| {
                if search.core_memory_prefer_confirmed {
                    b.confidence.unwrap_or(0).cmp(&a.confidence.unwrap_or(0))
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| {
                if search.core_memory_freshness {
                    b.ts_ms.unwrap_or(0).cmp(&a.ts_ms.unwrap_or(0))
                } else {
                    std::cmp::Ordering::Equal
                }
            })
    });
}

/// Build the auto-context from disk + in-memory state.
pub fn build(output_dir: &Path, reader: &crate::reader::MicroscopeReader) -> AutoContext {
    build_with_config(output_dir, reader, &crate::config::Search::default())
}

/// Build the snapshot with the configured core-memory policy.
///
/// The previous `build` function ignored `core_memory_freshness` and
/// `core_memory_prefer_confirmed` entirely, and indexed memories carried no
/// timestamp at all (`ts_ms` was always `None`).  The result was that an old
/// high-importance fact and a fresh confirmed correction were impossible to
/// order correctly.  Timeline entries provide the durable timestamp and the
/// latest status for the same exact text.
pub fn build_with_config(
    output_dir: &Path,
    reader: &crate::reader::MicroscopeReader,
    search: &crate::config::Search,
) -> AutoContext {
    let mut ctx = AutoContext::default();

    // Timeline (last session, capped to TIMELINE_TOP_N).
    let entries = timeline::read_all(&output_dir.join("timeline.bin"));
    let filtered = timeline::filter(&entries, &timeline::TimeWindow::LastSession);
    let mut rev: Vec<timeline::TimelineEntry> = filtered.iter().rev().cloned().collect();
    rev.truncate(TIMELINE_TOP_N);
    ctx.last_session_anchor_ms = filtered.first().map(|e| e.ts_ms);
    ctx.timeline_entries = rev;

    // Open loops (top N by importance).
    let mut open = open_loops::read_open(&output_dir.join("open_loops.bin"));
    open.truncate(LOOPS_TOP_N);
    ctx.open_loops = open;

    // System state.
    ctx.block_count = reader.block_count;
    for (d, &(_start, count)) in reader.depth_ranges.iter().enumerate() {
        if count > 0 {
            ctx.depth_breakdown.push((d as u8, count as usize));
        }
    }

    // Append log size.
    let append_path = output_dir.join("append.bin");
    if append_path.exists() {
        if let Ok(meta) = std::fs::metadata(&append_path) {
            // Rough entry estimate: 4 (magic) + 19 (header) + ~128 (avg text)
            const AVG_ENTRY: u64 = 147;
            let total = meta.len();
            ctx.append_log_size = (total / AVG_ENTRY) as usize;
        }
    }

    // Core memories: find top-importance blocks and embed their full text.
    // Pending append-log entries are considered first because they are the
    // freshest observations.  Durable index blocks are the fallback: a normal
    // rebuild clears the append log, so reading only the log made core memory
    // disappear after every consolidation.
    // Latest durable observation state per exact text.  The timeline is
    // append-only, so a later archived/superseded row must win over an older
    // normal row for the same memory.
    let observation_state =
        latest_observation_states(&timeline::read_all(&output_dir.join("timeline.bin")));

    let appended = crate::reader::read_append_log(&append_path);
    let mut candidates: Vec<CoreMemory> = appended
        .iter()
        .filter(|e| e.importance >= CORE_MEMORIES_MIN_IMP)
        .filter_map(|e| {
            let observation = observation_state.get(e.text.trim());
            if observation
                .map(|(_, status)| timeline::is_inactive_status(*status))
                .unwrap_or(false)
            {
                return None;
            }
            let layer = LAYER_NAMES
                .get(e.layer_id as usize)
                .unwrap_or(&"?")
                .to_string();
            Some(CoreMemory {
                text: crate::safe_truncate(&e.text, CORE_MEMORY_MAX_CHARS).to_string(),
                layer,
                importance: e.importance,
                depth: 0,
                ts_ms: observation.map(|(ts, _)| *ts),
                confidence: None,
            })
        })
        .collect();

    let indexed_candidates: Vec<CoreMemory> = reader
        .depth_ranges
        .iter()
        .enumerate()
        .filter(|(depth, _)| *depth == 3)
        .flat_map(|(_, (start, count))| *start as usize..*start as usize + *count as usize)
        .filter_map(|idx| {
            let header = reader.header(idx);
            if header.importance < CORE_MEMORIES_MIN_IMP {
                return None;
            }
            let layer = LAYER_NAMES
                .get(header.layer_id as usize)
                .unwrap_or(&"?")
                .to_string();
            let text = reader.text(idx);
            let observation = observation_state.get(text.trim());
            if observation
                .map(|(_, status)| timeline::is_inactive_status(*status))
                .unwrap_or(false)
            {
                return None;
            }
            Some(CoreMemory {
                text: crate::safe_truncate(text, CORE_MEMORY_MAX_CHARS).to_string(),
                layer,
                importance: header.importance,
                depth: 3,
                ts_ms: observation.map(|(ts, _)| *ts),
                confidence: None,
            })
        })
        .collect();

    // Keep fresh pending observations ahead of durable entries, then take the
    // top N by importance.  Deduplication prevents the same text from being
    // shown twice when it exists in both sources.
    for candidate in indexed_candidates {
        if !candidates
            .iter()
            .any(|existing| existing.text == candidate.text)
        {
            candidates.push(candidate);
        }
    }
    // Attach evidence confidence from the ledger when it is available.
    let ledger = crate::epistemic::EvidenceLedger::load_or_init(output_dir);
    // Durable memory atoms carry explicit supersession and contradiction
    // relations.  An atom that has been superseded, or whose subject has a
    // newer active atom, must not reappear in the active core-memory view.
    let atom_ledger = crate::memory_atom::AtomLedger::load(&output_dir.join("memory_atoms.bin"));
    candidates.retain(|candidate| !atom_ledger.is_inactive_text(&candidate.text));
    for candidate in &mut candidates {
        let hash = crate::epistemic::content_hash(&candidate.text);
        candidate.confidence = atom_ledger
            .confidence_for_text(&candidate.text)
            .or_else(|| ledger.records.get(&hash).map(|record| record.confidence));
    }
    // Explicit confirmation first, then importance, confidence and freshness.
    // A default-confidence atom is not confirmation, so merely storing an
    // observation cannot promote it into the confirmed slot.
    sort_core_memories(&mut candidates, search);
    candidates.truncate(CORE_MEMORIES_TOP_N);
    ctx.core_memories = candidates;

    ctx
}

/// Short header label used to embed auto-context into other tool outputs.
pub const AUTO_CONTEXT_HEADER: &str = "[AUTO-CONTEXT]";

/// Render the auto-context as a compact text block (good for LLM prompts).
pub fn render(ctx: &AutoContext) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n╔═══════════════════════════════════════════════════════╗\n",
        AUTO_CONTEXT_HEADER
    ));

    // System state
    out.push_str(&format!(
        "║ STATE: {} blocks | {} depths active | append log ~{} entries\n",
        ctx.block_count,
        ctx.depth_breakdown.len(),
        ctx.append_log_size,
    ));
    if ctx.last_session_anchor_ms.is_some() {
        out.push_str(&format!(
            "║ SESSION STARTED: {}\n",
            ctx.last_session_anchor_ms
                .map(timeline::format_ts)
                .unwrap_or_else(|| "?".to_string())
        ));
    }

    // Timeline
    if !ctx.timeline_entries.is_empty() {
        out.push_str("║\n║ LAST SESSION TIMELINE (newest first):\n");
        for e in &ctx.timeline_entries {
            let layer = LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
            let status_label = match e.status {
                timeline::STATUS_OPEN => "OPEN",
                timeline::STATUS_RESOLVED => "RESOLVED",
                timeline::STATUS_ARCHIVED => "ARCHIVED",
                _ => "",
            };
            let status_str = if status_label.is_empty() {
                String::new()
            } else {
                format!(" [{}]", status_label)
            };
            out.push_str(&format!(
                "║   {} {} D{} [{}] imp={}{} {}\n",
                timeline::format_ts(e.ts_ms),
                "",
                e.depth,
                layer,
                e.importance,
                status_str,
                crate::safe_truncate(&e.text, 90),
            ));
        }
    } else {
        out.push_str("║ LAST SESSION: empty (no stores yet)\n");
    }

    // Open loops
    if !ctx.open_loops.is_empty() {
        out.push_str(&format!(
            "║\n║ OPEN LOOPS ({} pending):\n",
            ctx.open_loops.len()
        ));
        for l in &ctx.open_loops {
            out.push_str(&format!(
                "║   #{} {} imp={} {}\n",
                l.id,
                timeline::format_ts(l.ts_ms),
                l.importance,
                crate::safe_truncate(&l.text, 100),
            ));
        }
    } else {
        out.push_str("║ OPEN LOOPS: none\n");
    }

    // Core memories — full text for direct LLM consumption (no hallucination needed)
    if !ctx.core_memories.is_empty() {
        out.push_str(&format!(
            "║\n║ CORE MEMORIES (imp>={}, full text):\n",
            CORE_MEMORIES_MIN_IMP
        ));
        for (i, m) in ctx.core_memories.iter().enumerate() {
            out.push_str(&format!(
                "║ [{}/{}] [{}] imp={} >>>\n",
                i + 1,
                ctx.core_memories.len(),
                m.layer,
                m.importance,
            ));
            // Indent each line of the memory text
            for line in m.text.lines() {
                out.push_str(&format!("║   {}\n", line));
            }
            out.push_str("║   <<<\n");
        }
    }

    out.push_str("╚═══════════════════════════════════════════════════════╝\n");
    out
}

/// Compact version of the auto-context (for embedding in recall output).
pub fn render_compact(ctx: &AutoContext) -> String {
    let mut out = String::new();
    // Core first so a downstream context budget does not silently clip it.
    if !ctx.core_memories.is_empty() {
        out.push_str(&format!("{} core memories:\n", AUTO_CONTEXT_HEADER));
        for memory in &ctx.core_memories {
            out.push_str(&format!(
                "  • [{}] imp={} {}\n",
                memory.layer, memory.importance, memory.text
            ));
        }
    }
    if !ctx.timeline_entries.is_empty() {
        out.push_str(&format!(
            "{} last session: {} stores.\n",
            AUTO_CONTEXT_HEADER,
            ctx.timeline_entries.len()
        ));
        for e in ctx.timeline_entries.iter().take(3) {
            let layer = LAYER_NAMES.get(e.layer_id as usize).unwrap_or(&"?");
            let status_label = match e.status {
                timeline::STATUS_OPEN => "[OPEN]",
                timeline::STATUS_RESOLVED => "[RESOLVED]",
                _ => "",
            };
            out.push_str(&format!(
                "  • {} [{}] imp={} {} {}\n",
                timeline::format_ts(e.ts_ms),
                layer,
                e.importance,
                status_label,
                crate::safe_truncate(&e.text, 70),
            ));
        }
    }
    if !ctx.open_loops.is_empty() {
        out.push_str(&format!(
            "{} {} open loop{}:\n",
            AUTO_CONTEXT_HEADER,
            ctx.open_loops.len(),
            if ctx.open_loops.len() == 1 { "" } else { "s" }
        ));
        for l in ctx.open_loops.iter().take(3) {
            out.push_str(&format!(
                "  • #{} imp={} {}\n",
                l.id,
                l.importance,
                crate::safe_truncate(&l.text, 80),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty() {
        let ctx = AutoContext::default();
        let s = render(&ctx);
        assert!(s.contains(AUTO_CONTEXT_HEADER));
        assert!(s.contains("OPEN LOOPS: none"));
        assert!(s.contains("empty"));
    }

    #[test]
    fn test_render_compact_includes_timeline_and_loops() {
        let mut ctx = AutoContext::default();
        ctx.timeline_entries.push(timeline::TimelineEntry {
            ts_ms: 1_700_000_000_000,
            layer_id: 1,
            importance: 7,
            depth: 4,
            status: timeline::STATUS_NORMAL,
            text: "compacted".to_string(),
        });
        ctx.open_loops.push(open_loops::OpenLoopEntry {
            id: 1,
            ts_ms: 1_700_000_000_000,
            importance: 8,
            status: open_loops::STATUS_OPEN,
            text: "todo".to_string(),
        });
        let s = render_compact(&ctx);
        assert!(s.contains("compacted"));
        assert!(s.contains("#1"));
        assert!(s.contains("todo"));
    }

    #[test]
    fn render_includes_core_memories() {
        let mut ctx = AutoContext::default();
        ctx.core_memories.push(CoreMemory {
            text: "durable preference".to_string(),
            layer: "long_term".to_string(),
            importance: 8,
            depth: 3,
            ts_ms: None,
            confidence: None,
        });
        let rendered = render(&ctx);
        assert!(rendered.contains("CORE MEMORIES"));
        assert!(rendered.contains("durable preference"));
        let compact = render_compact(&ctx);
        assert!(compact.contains("core memories"));
        assert!(compact.contains("durable preference"));
    }

    #[test]
    fn core_memory_prefers_confirmed_then_important_then_fresh() {
        let mut candidates = vec![
            CoreMemory {
                text: "old important".to_string(),
                layer: "long_term".to_string(),
                importance: 9,
                depth: 3,
                ts_ms: Some(1),
                confidence: None,
            },
            CoreMemory {
                text: "confirmed".to_string(),
                layer: "long_term".to_string(),
                importance: 7,
                depth: 3,
                ts_ms: Some(2),
                confidence: Some(80),
            },
            CoreMemory {
                text: "fresh important".to_string(),
                layer: "long_term".to_string(),
                importance: 8,
                depth: 3,
                ts_ms: Some(3),
                confidence: None,
            },
        ];
        sort_core_memories(
            &mut candidates,
            &crate::config::Search {
                core_memory_freshness: true,
                core_memory_prefer_confirmed: true,
                ..crate::config::Search::default()
            },
        );
        assert_eq!(candidates[0].text, "confirmed");
        assert_eq!(candidates[1].text, "old important");
        assert_eq!(candidates[2].text, "fresh important");
    }

    #[test]
    fn default_atom_confidence_is_not_treated_as_confirmation() {
        let mut candidates = vec![
            CoreMemory {
                text: "important unconfirmed".to_string(),
                layer: "long_term".to_string(),
                importance: 8,
                depth: 3,
                ts_ms: Some(1),
                confidence: Some(50),
            },
            CoreMemory {
                text: "default confidence".to_string(),
                layer: "long_term".to_string(),
                importance: 7,
                depth: 3,
                ts_ms: Some(2),
                confidence: Some(50),
            },
        ];
        let search = crate::config::Search {
            core_memory_freshness: false,
            core_memory_prefer_confirmed: true,
            ..crate::config::Search::default()
        };
        sort_core_memories(&mut candidates, &search);
        assert_eq!(candidates[0].text, "important unconfirmed");
    }

    #[test]
    fn freshness_policy_can_be_disabled() {
        let mut candidates = vec![
            CoreMemory {
                text: "fresh low".to_string(),
                layer: "long_term".to_string(),
                importance: 7,
                depth: 3,
                ts_ms: Some(200),
                confidence: None,
            },
            CoreMemory {
                text: "old high".to_string(),
                layer: "long_term".to_string(),
                importance: 8,
                depth: 3,
                ts_ms: Some(100),
                confidence: None,
            },
        ];
        let search = crate::config::Search {
            core_memory_freshness: false,
            core_memory_prefer_confirmed: false,
            ..crate::config::Search::default()
        };
        sort_core_memories(&mut candidates, &search);
        assert_eq!(candidates[0].text, "old high");
    }

    #[test]
    fn confirmed_policy_can_be_disabled() {
        let mut candidates = vec![
            CoreMemory {
                text: "confirmed low".to_string(),
                layer: "long_term".to_string(),
                importance: 7,
                depth: 3,
                ts_ms: Some(200),
                confidence: Some(100),
            },
            CoreMemory {
                text: "unconfirmed high".to_string(),
                layer: "long_term".to_string(),
                importance: 8,
                depth: 3,
                ts_ms: Some(100),
                confidence: None,
            },
        ];
        let search = crate::config::Search {
            core_memory_freshness: false,
            core_memory_prefer_confirmed: false,
            ..crate::config::Search::default()
        };
        sort_core_memories(&mut candidates, &search);
        assert_eq!(candidates[0].text, "unconfirmed high");
    }

    #[test]
    fn latest_inactive_timeline_status_wins() {
        let entries = vec![
            timeline::TimelineEntry {
                ts_ms: 100,
                layer_id: 3,
                importance: 8,
                depth: 3,
                status: timeline::STATUS_NORMAL,
                text: "fact".to_string(),
            },
            timeline::TimelineEntry {
                ts_ms: 200,
                layer_id: 3,
                importance: 8,
                depth: 3,
                status: timeline::STATUS_SUPERSEDED,
                text: "fact".to_string(),
            },
        ];
        let states = latest_observation_states(&entries);
        assert_eq!(
            states.get("fact"),
            Some(&(200, timeline::STATUS_SUPERSEDED))
        );
        assert!(timeline::is_inactive_status(states["fact"].1));
    }
}
