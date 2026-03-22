//! Microscope Query Language (MQL)
//!
//! Syntax:
//!   "keyword"                     → text search
//!   layer:long_term "keyword"     → filter by layer
//!   depth:3 "keyword"             → filter by depth
//!   depth:2..5 "keyword"          → depth range
//!   near:0.2,0.3,0.1 "keyword"   → spatial filter (within radius 0.1 of coords)
//!   "foo" AND "bar"               → both keywords must match
//!   "foo" OR "bar"                → either keyword matches
//!   limit:20                      → override default k
//!
//! Filters compose: `layer:long_term depth:3..5 "Ora" AND "memory"`

use crate::{MicroscopeReader, LAYER_NAMES, AppendEntry};

#[derive(Debug, Clone)]
pub struct Query {
    pub keywords: Vec<String>,
    pub op: BoolOp,
    pub layer_filter: Option<u8>,
    pub depth_filter: Option<(u8, u8)>,  // (min, max) inclusive
    pub spatial_filter: Option<(f32, f32, f32, f32)>,  // x, y, z, radius
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoolOp {
    And,
    Or,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            op: BoolOp::And,
            layer_filter: None,
            depth_filter: None,
            spatial_filter: None,
            limit: 10,
        }
    }
}

/// Parse a query string into a structured Query.
pub fn parse(input: &str) -> Query {
    let mut q = Query::default();
    let mut remaining = input.trim();

    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if remaining.is_empty() { break; }

        // layer:NAME
        if let Some(rest) = remaining.strip_prefix("layer:") {
            let (val, rest2) = take_word(rest);
            q.layer_filter = Some(crate::layer_to_id(&val));
            remaining = rest2;
            continue;
        }

        // depth:N or depth:N..M
        if let Some(rest) = remaining.strip_prefix("depth:") {
            let (val, rest2) = take_word(rest);
            if let Some((a, b)) = val.split_once("..") {
                let lo = a.parse::<u8>().unwrap_or(0);
                let hi = b.parse::<u8>().unwrap_or(8);
                q.depth_filter = Some((lo, hi));
            } else if let Ok(d) = val.parse::<u8>() {
                q.depth_filter = Some((d, d));
            }
            remaining = rest2;
            continue;
        }

        // near:X,Y,Z or near:X,Y,Z,R
        if let Some(rest) = remaining.strip_prefix("near:") {
            let (val, rest2) = take_word(rest);
            let parts: Vec<f32> = val.split(',').filter_map(|s| s.parse().ok()).collect();
            if parts.len() >= 3 {
                let r = if parts.len() >= 4 { parts[3] } else { 0.1 };
                q.spatial_filter = Some((parts[0], parts[1], parts[2], r));
            }
            remaining = rest2;
            continue;
        }

        // limit:N
        if let Some(rest) = remaining.strip_prefix("limit:") {
            let (val, rest2) = take_word(rest);
            if let Ok(n) = val.parse::<usize>() {
                q.limit = n;
            }
            remaining = rest2;
            continue;
        }

        // AND / OR operators
        if let Some(rest) = remaining.strip_prefix("AND") {
            if rest.is_empty() || rest.starts_with(' ') {
                q.op = BoolOp::And;
                remaining = rest;
                continue;
            }
        }
        if let Some(rest) = remaining.strip_prefix("OR") {
            if rest.is_empty() || rest.starts_with(' ') {
                q.op = BoolOp::Or;
                remaining = rest;
                continue;
            }
        }

        // Quoted keyword: "..."
        if remaining.starts_with('"') {
            if let Some(end) = remaining[1..].find('"') {
                q.keywords.push(remaining[1..1+end].to_lowercase());
                remaining = &remaining[2+end..];
                continue;
            }
        }

        // Bare word as keyword
        let (word, rest2) = take_word(remaining);
        if !word.is_empty() {
            q.keywords.push(word.to_lowercase());
        }
        remaining = rest2;
    }

    q
}

fn take_word(s: &str) -> (String, &str) {
    let s = s.trim_start();
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    (s[..end].to_string(), &s[end..])
}

/// Result from query execution.
#[derive(Debug)]
pub struct QueryResult {
    pub score: f32,
    pub block_idx: usize,
    pub is_main: bool,  // true = main index, false = append log
}

/// Execute a parsed query against the reader and append log.
pub fn execute(
    q: &Query,
    reader: &MicroscopeReader,
    appended: &[AppendEntry],
) -> Vec<QueryResult> {
    let mut results = Vec::new();

    // Search main index
    for i in 0..reader.block_count {
        let h = reader.header(i);

        // Layer filter
        if let Some(lid) = q.layer_filter {
            if h.layer_id != lid { continue; }
        }

        // Depth filter
        if let Some((lo, hi)) = q.depth_filter {
            if h.depth < lo || h.depth > hi { continue; }
        }

        // Spatial filter
        if let Some((sx, sy, sz, sr)) = q.spatial_filter {
            let dx = h.x - sx;
            let dy = h.y - sy;
            let dz = h.z - sz;
            if dx*dx + dy*dy + dz*dz > sr*sr { continue; }
        }

        // Keyword match
        let text = reader.text(i).to_lowercase();
        let score = keyword_score(&text, &q.keywords, &q.op);
        if score > 0.0 {
            results.push(QueryResult { score, block_idx: i, is_main: true });
        }
    }

    // Search append log
    for (ai, entry) in appended.iter().enumerate() {
        if let Some(lid) = q.layer_filter {
            if entry.layer_id != lid { continue; }
        }
        if let Some((lo, hi)) = q.depth_filter {
            if entry.depth < lo || entry.depth > hi { continue; }
        }
        if let Some((sx, sy, sz, sr)) = q.spatial_filter {
            let dx = entry.x - sx;
            let dy = entry.y - sy;
            let dz = entry.z - sz;
            if dx*dx + dy*dy + dz*dz > sr*sr { continue; }
        }

        let text = entry.text.to_lowercase();
        let score = keyword_score(&text, &q.keywords, &q.op);
        if score > 0.0 {
            results.push(QueryResult { score, block_idx: ai + 1_000_000, is_main: false });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    results.truncate(q.limit);
    results
}

fn keyword_score(text: &str, keywords: &[String], op: &BoolOp) -> f32 {
    if keywords.is_empty() { return 1.0; }

    let hits: Vec<bool> = keywords.iter().map(|kw| text.contains(kw.as_str())).collect();
    let hit_count = hits.iter().filter(|&&h| h).count();

    match op {
        BoolOp::And => {
            if hit_count == keywords.len() {
                hit_count as f32
            } else {
                0.0
            }
        }
        BoolOp::Or => hit_count as f32,
    }
}

/// Format a layer ID to its name.
#[allow(dead_code)]
pub fn layer_name(id: u8) -> &'static str {
    LAYER_NAMES.get(id as usize).unwrap_or(&"?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let q = parse("hello");
        assert_eq!(q.keywords, vec!["hello"]);
        assert_eq!(q.op, BoolOp::And);
    }

    #[test]
    fn test_parse_quoted() {
        let q = parse("\"hello world\"");
        assert_eq!(q.keywords, vec!["hello world"]);
    }

    #[test]
    fn test_parse_filters() {
        let q = parse("layer:long_term depth:2..5 \"Ora\"");
        assert_eq!(q.layer_filter, Some(1)); // long_term = index 1
        assert_eq!(q.depth_filter, Some((2, 5)));
        assert_eq!(q.keywords, vec!["ora"]);
    }

    #[test]
    fn test_parse_bool_op() {
        let q = parse("\"foo\" OR \"bar\"");
        assert_eq!(q.op, BoolOp::Or);
        assert_eq!(q.keywords, vec!["foo", "bar"]);
    }

    #[test]
    fn test_parse_limit() {
        let q = parse("limit:20 hello");
        assert_eq!(q.limit, 20);
    }

    #[test]
    fn test_parse_spatial() {
        let q = parse("near:0.2,0.3,0.1,0.05 test");
        let (x, y, z, r) = q.spatial_filter.unwrap();
        assert!((x - 0.2).abs() < 0.001);
        assert!((y - 0.3).abs() < 0.001);
        assert!((z - 0.1).abs() < 0.001);
        assert!((r - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_keyword_score_and() {
        assert_eq!(keyword_score("hello world", &["hello".into(), "world".into()], &BoolOp::And), 2.0);
        assert_eq!(keyword_score("hello", &["hello".into(), "world".into()], &BoolOp::And), 0.0);
    }

    #[test]
    fn test_keyword_score_or() {
        assert_eq!(keyword_score("hello", &["hello".into(), "world".into()], &BoolOp::Or), 1.0);
        assert_eq!(keyword_score("nope", &["hello".into(), "world".into()], &BoolOp::Or), 0.0);
    }
}
