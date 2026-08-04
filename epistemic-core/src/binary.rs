//! Binary serialization for the reasoning graph.
//!
//! Format: RSN1 (Reasoning graph v1)
//!
//! Layout:
//!   [magic: 4B "RSN1"]
//!   [node_count: u32]
//!   [edge_count: u32]
//!   [root_id: u32]
//!   [nodes: node_count × NodeRecord]
//!   [edges: edge_count × EdgeRecord]
//!
//! NodeRecord (variable):
//!   [node_type: u8]      // 0 = Evidence, 1 = Conclusion
//!   [id: u64]            // EvidenceId or ClaimId
//!   [text_len: u16]      // 0 for Evidence, len for Conclusion
//!   [text: text_len × u8] // UTF-8 text (Conclusion only)
//!
//! EdgeRecord (32 bytes):
//!   [premise: u32]
//!   [rule: u8]
//!   [conclusion: u32]
//!   [confidence: f64]
//!   [_pad: u8 × 3]

use crate::graph::ReasoningGraph;
use crate::rules::InferenceRule;
use crate::types::{ReasoningNode, ReasoningNodeId};
use std::fs;
use std::path::Path;

/// Magic bytes for the reasoning graph file.
pub const GRAPH_MAGIC: &[u8; 4] = b"RSN1";

const EDGE_SIZE: usize = 4 + 1 + 4 + 8 + 3; // 20 bytes

/// Save a reasoning graph to a binary file.
pub fn save_graph(graph: &ReasoningGraph, path: &Path) -> Result<(), String> {
    let mut buf = Vec::with_capacity(1024);

    // Magic
    buf.extend_from_slice(GRAPH_MAGIC);

    // Counts
    buf.extend_from_slice(&(graph.nodes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(graph.edges.len() as u32).to_le_bytes());
    buf.extend_from_slice(&graph.root.0.to_le_bytes());

    // Nodes
    for node in &graph.nodes {
        match node {
            ReasoningNode::Evidence { id } => {
                buf.push(0); // Evidence
                buf.extend_from_slice(&id.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes()); // no text
            }
            ReasoningNode::Conclusion { id, text } => {
                buf.push(1); // Conclusion
                buf.extend_from_slice(&id.to_le_bytes());
                let text_bytes = text.as_bytes();
                let len = text_bytes.len() as u16;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(text_bytes);
            }
        }
    }

    // Edges
    for edge in &graph.edges {
        buf.extend_from_slice(&edge.premise.0.to_le_bytes());
        buf.push(edge.rule as u8);
        buf.extend_from_slice(&edge.conclusion.0.to_le_bytes());
        buf.extend_from_slice(&edge.confidence.to_le_bytes());
        buf.extend_from_slice(&[0u8; 3]); // padding
    }

    fs::write(path, &buf).map_err(|e| format!("failed to write graph: {e}"))
}

/// Load a reasoning graph from a binary file.
pub fn load_graph(path: &Path) -> Result<ReasoningGraph, String> {
    let data = fs::read(path).map_err(|e| format!("failed to read graph: {e}"))?;

    if data.len() < 16 {
        return Err("file too short".into());
    }
    if &data[0..4] != GRAPH_MAGIC {
        return Err("invalid magic".into());
    }

    let mut off = 4;
    let node_count = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let edge_count = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let root_id = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
    off += 4;

    let mut graph = ReasoningGraph::new();

    // Nodes
    for _ in 0..node_count {
        let node_type = data[off];
        off += 1;
        let id = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        off += 8;
        let text_len = u16::from_le_bytes(data[off..off + 2].try_into().unwrap()) as usize;
        off += 2;

        match node_type {
            0 => {
                graph.add_evidence(id);
            }
            1 => {
                let text = String::from_utf8(data[off..off + text_len].to_vec())
                    .map_err(|e| format!("invalid UTF-8 in node text: {e}"))?;
                off += text_len;
                graph.add_conclusion(id, text);
            }
            _ => return Err(format!("invalid node type: {node_type}")),
        }
    }

    // Edges
    for _ in 0..edge_count {
        if off + EDGE_SIZE > data.len() {
            return Err("unexpected end of file while reading edges".into());
        }
        let premise = ReasoningNodeId(u32::from_le_bytes(data[off..off + 4].try_into().unwrap()));
        off += 4;
        let rule_byte = data[off];
        off += 1;
        let conclusion =
            ReasoningNodeId(u32::from_le_bytes(data[off..off + 4].try_into().unwrap()));
        off += 4;
        let confidence = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        off += 8;
        off += 3; // padding

        let rule = InferenceRule::from_byte(rule_byte)
            .ok_or_else(|| format!("invalid inference rule byte: {rule_byte}"))?;

        // We need to add the edge directly since we already have the penalized confidence
        graph.edges.push(crate::types::InferenceStep {
            premise,
            rule,
            conclusion,
            confidence,
        });
    }

    graph.set_root(ReasoningNodeId(root_id));
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleRegistry;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_graph.bin");

        let mut g = ReasoningGraph::new();
        let reg = RuleRegistry::new();

        let e1 = g.add_evidence(100);
        let e2 = g.add_evidence(200);
        let c1 = g.add_conclusion(1, "intermediate conclusion");
        let root = g.add_conclusion(2, "final claim");

        g.add_step(e1, InferenceRule::ObservationToExistence, c1, 0.95, &reg);
        g.add_step(e2, InferenceRule::ConvergentEvidence, c1, 0.90, &reg);
        g.add_step(c1, InferenceRule::CounterfactualObserved, root, 0.92, &reg);
        g.set_root(root);

        save_graph(&g, &path).expect("save should succeed");
        let loaded = load_graph(&path).expect("load should succeed");

        assert_eq!(loaded.nodes.len(), g.nodes.len());
        assert_eq!(loaded.edges.len(), g.edges.len());
        assert_eq!(loaded.root, g.root);

        // Check reasoning confidence is preserved
        let original_rc = g.reasoning_confidence();
        let loaded_rc = loaded.reasoning_confidence();
        assert!(
            (original_rc - loaded_rc).abs() < 0.001,
            "reasoning confidence should match"
        );
    }

    #[test]
    fn save_and_load_with_penalized_rule() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("penalized_graph.bin");

        let mut g = ReasoningGraph::new();
        let reg = RuleRegistry::new();

        let e1 = g.add_evidence(300);
        let root = g.add_conclusion(3, "ontological decision");

        g.add_step(e1, InferenceRule::CentralityToMotivation, root, 0.94, &reg);
        g.set_root(root);

        save_graph(&g, &path).expect("save should succeed");
        let loaded = load_graph(&path).expect("load should succeed");

        // The penalized confidence should be preserved
        let original_rc = g.reasoning_confidence();
        let loaded_rc = loaded.reasoning_confidence();
        assert!((original_rc - loaded_rc).abs() < 0.001);

        // Should still detect penalized rules
        assert_eq!(loaded.penalized_steps().len(), 1);
    }

    #[test]
    fn invalid_magic_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad_magic.bin");
        fs::write(
            &path,
            [b'X', b'X', b'X', b'X', 0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        )
        .unwrap();
        assert!(load_graph(&path).is_err());
    }
}
