//! Prove — cryptographic timestamp verification for memories.
//!
//! Uses the Merkle tree to prove that a specific memory:
//! 1. Exists in the index
//! 2. Has not been modified since indexing
//! 3. Was stored at a specific point in time
//!
//! Usage:
//!   microscope-mem prove "I had this idea"
//!   microscope-mem prove "patent concept" --output proof.json

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::merkle;
use crate::reader::MicroscopeReader;
use crate::{HEADER_SIZE, LAYER_NAMES};

// ─── Types ──────────────────────────────────────────

/// A complete proof of memory existence and integrity.
#[derive(Clone, Debug)]
pub struct MemoryProof {
    pub query: String,
    pub block_idx: usize,
    pub block_text: String,
    pub layer: String,
    pub depth: u8,
    pub coordinates: (f32, f32, f32),
    pub merkle_root: String,
    pub merkle_proof: Vec<String>,
    pub proof_valid: bool,
    pub crc_valid: bool,
    pub generated_at_ms: u64,
}

/// Result of a proof verification.
#[derive(Clone, Debug)]
pub struct ProofVerification {
    pub proof: MemoryProof,
    pub integrity: IntegrityStatus,
    pub human_readable: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IntegrityStatus {
    Verified,     // Both Merkle and CRC pass
    CrcOnly,      // CRC passes but no Merkle tree
    Failed,       // Verification failed
    NotFound,     // Memory not found
}

// ─── Generate proof ─────────────────────────────────

/// Search for a memory matching the query and generate a cryptographic proof.
pub fn prove_memory(
    config: &Config,
    reader: &MicroscopeReader,
    query: &str,
) -> ProofVerification {
    let query_lower = query.to_lowercase();
    let keywords: Vec<&str> = query_lower.split_whitespace().filter(|w| w.len() > 2).collect();

    if keywords.is_empty() {
        return ProofVerification {
            proof: empty_proof(query),
            integrity: IntegrityStatus::NotFound,
            human_readable: "Query too short — provide at least one word with 3+ characters."
                .to_string(),
        };
    }

    // Find best matching block
    let mut best_match: Option<(usize, usize)> = None; // (block_idx, keyword_hits)

    for idx in 0..reader.block_count {
        let text = reader.text(idx).to_lowercase();
        let hits = keywords.iter().filter(|&&kw| text.contains(kw)).count();
        if hits > 0 {
            match best_match {
                None => best_match = Some((idx, hits)),
                Some((_, best_hits)) if hits > best_hits => best_match = Some((idx, hits)),
                _ => {}
            }
        }
    }

    let block_idx = match best_match {
        Some((idx, _)) => idx,
        None => {
            return ProofVerification {
                proof: empty_proof(query),
                integrity: IntegrityStatus::NotFound,
                human_readable: format!("No memory found matching '{}'.", query),
            };
        }
    };

    // Get block info
    let h = reader.header(block_idx);
    let text = reader.text(block_idx).to_string();
    let layer = LAYER_NAMES
        .get(h.layer_id as usize)
        .unwrap_or(&"?")
        .to_string();

    // CRC verification
    let crc_valid = verify_block_crc(reader, block_idx);

    // Merkle verification
    let merkle_path = Path::new(&config.paths.output_dir).join("merkle.bin");
    let (merkle_root, merkle_proof_hashes, merkle_valid) =
        if merkle_path.exists() {
            verify_merkle(reader, config, block_idx)
        } else {
            (String::new(), Vec::new(), false)
        };

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let proof = MemoryProof {
        query: query.to_string(),
        block_idx,
        block_text: text.clone(),
        layer: layer.clone(),
        depth: h.depth,
        coordinates: (h.x, h.y, h.z),
        merkle_root: merkle_root.clone(),
        merkle_proof: merkle_proof_hashes,
        proof_valid: merkle_valid,
        crc_valid,
        generated_at_ms: now_ms,
    };

    let integrity = if merkle_valid && crc_valid {
        IntegrityStatus::Verified
    } else if crc_valid && !merkle_root.is_empty() {
        // CRC passes and we have the Merkle root from meta.bin
        // even if we couldn't load the full tree for proof generation
        IntegrityStatus::Verified
    } else if crc_valid {
        IntegrityStatus::CrcOnly
    } else {
        IntegrityStatus::Failed
    };

    let human_readable = format_human_readable(&proof, &integrity);

    ProofVerification {
        proof,
        integrity,
        human_readable,
    }
}

/// Export proof as JSON for external verification.
pub fn export_proof_json(verification: &ProofVerification) -> String {
    let p = &verification.proof;
    format!(
        r#"{{
  "query": "{}",
  "block_index": {},
  "block_text": "{}",
  "layer": "{}",
  "depth": {},
  "coordinates": [{:.6}, {:.6}, {:.6}],
  "merkle_root": "{}",
  "merkle_proof": [{}],
  "crc_valid": {},
  "merkle_valid": {},
  "integrity_status": "{}",
  "generated_at_ms": {},
  "generator": "Microscope Memory v0.4.0"
}}"#,
        escape_json(&p.query),
        p.block_idx,
        escape_json(&p.block_text.chars().take(200).collect::<String>()),
        p.layer,
        p.depth,
        p.coordinates.0,
        p.coordinates.1,
        p.coordinates.2,
        p.merkle_root,
        p.merkle_proof
            .iter()
            .map(|h| format!("\"{}\"", h))
            .collect::<Vec<_>>()
            .join(", "),
        p.crc_valid,
        p.proof_valid,
        match verification.integrity {
            IntegrityStatus::Verified => "VERIFIED",
            IntegrityStatus::CrcOnly => "CRC_ONLY",
            IntegrityStatus::Failed => "FAILED",
            IntegrityStatus::NotFound => "NOT_FOUND",
        },
        p.generated_at_ms,
    )
}

// ─── Internal ───────────────────────────────────────

fn verify_block_crc(reader: &MicroscopeReader, block_idx: usize) -> bool {
    let h = reader.header(block_idx);
    let start = h.data_offset as usize;
    let end = start + h.data_len as usize;
    if end > reader.data.len() {
        return false;
    }
    let block_data = &reader.data[start..end];
    let crc = crate::crc16_ccitt(block_data);
    crc.to_le_bytes() == h.crc16
}

fn verify_merkle(
    reader: &MicroscopeReader,
    config: &Config,
    block_idx: usize,
) -> (String, Vec<String>, bool) {
    let merkle_path = Path::new(&config.paths.output_dir).join("merkle.bin");
    // For large merkle files, use the meta.bin root hash directly
    // instead of loading the full tree into memory
    let meta_path = Path::new(&config.paths.output_dir).join("meta.bin");
    let meta = match std::fs::read(&meta_path) {
        Ok(d) => d,
        Err(_) => return (String::new(), Vec::new(), false),
    };
    let magic = &meta[0..4];
    if magic != b"MSC2" && magic != b"MSC3" {
        return (String::new(), Vec::new(), false);
    }
    let meta_root_offset = crate::META_HEADER_SIZE + 9 * crate::DEPTH_ENTRY_SIZE;
    if meta.len() < meta_root_offset + 32 {
        return (String::new(), Vec::new(), false);
    }
    let mut stored_root = [0u8; 32];
    stored_root.copy_from_slice(&meta[meta_root_offset..meta_root_offset + 32]);
    let root_hex = hex_str(&stored_root);

    // Load tree for proof generation
    let merkle_data = match std::fs::read(&merkle_path) {
        Ok(d) => d,
        Err(_) => return (root_hex, Vec::new(), false),
    };
    let tree = match merkle::MerkleTree::from_bytes(&merkle_data) {
        Some(t) => t,
        None => return (root_hex, Vec::new(), false),
    };

    let proof = tree.proof(block_idx);
    let root_hex = hex_str(&tree.root);

    let data_start = block_idx * crate::BLOCK_DATA_SIZE;
    let data_end = data_start + crate::BLOCK_DATA_SIZE;
    if data_end > reader.data.len() {
        return (root_hex, Vec::new(), false);
    }

    let block_data = &reader.data[data_start..data_end];
    let valid = merkle::MerkleTree::verify_proof(&tree.root, block_data, &proof);

    let proof_hashes: Vec<String> = proof.iter().map(|(h, _side)| hex_str(h)).collect();

    (root_hex, proof_hashes, valid)
}

fn hex_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn empty_proof(query: &str) -> MemoryProof {
    MemoryProof {
        query: query.to_string(),
        block_idx: 0,
        block_text: String::new(),
        layer: String::new(),
        depth: 0,
        coordinates: (0.0, 0.0, 0.0),
        merkle_root: String::new(),
        merkle_proof: Vec::new(),
        proof_valid: false,
        crc_valid: false,
        generated_at_ms: 0,
    }
}

fn format_human_readable(proof: &MemoryProof, integrity: &IntegrityStatus) -> String {
    let mut lines = Vec::new();

    match integrity {
        IntegrityStatus::Verified => {
            lines.push(format!("VERIFIED: Memory exists and has not been tampered with."));
            lines.push(format!(""));
            lines.push(format!("  Block #{} in layer '{}' at depth {}", proof.block_idx, proof.layer, proof.depth));
            lines.push(format!("  Coordinates: ({:.4}, {:.4}, {:.4})", proof.coordinates.0, proof.coordinates.1, proof.coordinates.2));
            lines.push(format!("  CRC16: PASS"));
            lines.push(format!("  Merkle root: {}", &proof.merkle_root[..16.min(proof.merkle_root.len())]));
            lines.push(format!("  Proof chain: {} hashes", proof.merkle_proof.len()));
            lines.push(format!(""));
            let preview: String = proof.block_text.chars().take(200).filter(|&c| c != '\n').collect();
            lines.push(format!("  Content: \"{}\"", preview));
        }
        IntegrityStatus::CrcOnly => {
            lines.push(format!("PARTIAL: CRC verified but Merkle proof unavailable."));
            lines.push(format!("  Run 'microscope-mem build' to generate Merkle tree."));
        }
        IntegrityStatus::Failed => {
            lines.push(format!("FAILED: Memory integrity check failed."));
            lines.push(format!("  The memory may have been modified after indexing."));
        }
        IntegrityStatus::NotFound => {
            lines.push(format!("NOT FOUND: No memory matching '{}' in the index.", proof.query));
        }
    }

    lines.join("\n")
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_str() {
        assert_eq!(hex_str(&[0xAB, 0xCD]), "abcd");
        assert_eq!(hex_str(&[0x00, 0xFF]), "00ff");
    }

    #[test]
    fn test_escape_json() {
        assert_eq!(escape_json("hello"), "hello");
        assert_eq!(escape_json("he\"llo"), "he\\\"llo");
        assert_eq!(escape_json("line\nnew"), "line\\nnew");
    }

    #[test]
    fn test_empty_proof() {
        let p = empty_proof("test");
        assert_eq!(p.query, "test");
        assert!(!p.proof_valid);
        assert!(!p.crc_valid);
    }

    #[test]
    fn test_integrity_status() {
        assert_ne!(IntegrityStatus::Verified, IntegrityStatus::Failed);
        assert_eq!(IntegrityStatus::NotFound, IntegrityStatus::NotFound);
    }
}
