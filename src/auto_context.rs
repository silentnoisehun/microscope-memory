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

/// Assembled auto-context, ready to format for an LLM prompt.
#[derive(Debug, Clone, Default)]
pub struct AutoContext {
    pub timeline_entries: Vec<timeline::TimelineEntry>,
    pub open_loops: Vec<open_loops::OpenLoopEntry>,
    pub block_count: usize,
    pub depth_breakdown: Vec<(u8, usize)>,
    pub append_log_size: usize,
    pub last_session_anchor_ms: Option<u64>,
}

/// Build the auto-context from disk + in-memory state.
pub fn build(output_dir: &Path, reader: &crate::reader::MicroscopeReader) -> AutoContext {
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

    out.push_str("╚═══════════════════════════════════════════════════════╝\n");
    out
}

/// Compact version of the auto-context (for embedding in recall output).
pub fn render_compact(ctx: &AutoContext) -> String {
    let mut out = String::new();
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
}
