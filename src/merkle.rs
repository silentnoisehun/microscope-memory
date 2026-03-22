//! Merkle tree for whole-index integrity verification.
//!
//! Leaf = SHA-256(block_data). Parent = SHA-256(left || right).
//! Odd leaf count → last leaf is promoted (hashed with itself).

use sha2::{Digest, Sha256};

pub struct MerkleTree {
    /// All nodes: leaves first, then internal nodes bottom-up. Root is last.
    pub nodes: Vec<[u8; 32]>,
    pub leaf_count: usize,
    pub root: [u8; 32],
}

impl MerkleTree {
    /// Build Merkle tree from block data slices.
    pub fn build(leaves: &[&[u8]]) -> Self {
        assert!(!leaves.is_empty(), "cannot build Merkle tree from 0 leaves");

        // Hash each leaf
        let leaf_hashes: Vec<[u8; 32]> = leaves
            .iter()
            .map(|data| {
                let mut h = Sha256::new();
                h.update(data);
                h.finalize().into()
            })
            .collect();

        let leaf_count = leaf_hashes.len();

        // Build tree bottom-up
        // nodes layout: [leaf_0 .. leaf_n-1, internal_nodes..., root]
        let mut nodes = leaf_hashes;

        let mut level_start = 0;
        let mut level_len = leaf_count;

        while level_len > 1 {
            let pairs = level_len / 2;
            let odd = level_len % 2 == 1;

            for p in 0..pairs {
                let left = &nodes[level_start + p * 2];
                let right = &nodes[level_start + p * 2 + 1];
                let mut h = Sha256::new();
                h.update(left);
                h.update(right);
                let parent: [u8; 32] = h.finalize().into();
                nodes.push(parent);
            }

            // Odd node: hash with itself (promoted)
            if odd {
                let lone = &nodes[level_start + level_len - 1];
                let mut h = Sha256::new();
                h.update(lone);
                h.update(lone);
                let parent: [u8; 32] = h.finalize().into();
                nodes.push(parent);
            }

            level_start += level_len;
            level_len = pairs + if odd { 1 } else { 0 };
        }

        let root = *nodes.last().unwrap();
        MerkleTree {
            nodes,
            leaf_count,
            root,
        }
    }

    /// Verify a single leaf at index against stored hash.
    pub fn verify_leaf(&self, index: usize, data: &[u8]) -> bool {
        if index >= self.leaf_count {
            return false;
        }
        let mut h = Sha256::new();
        h.update(data);
        let computed: [u8; 32] = h.finalize().into();
        self.nodes[index] == computed
    }

    /// Get Merkle proof path for a leaf.
    /// Returns Vec of (sibling_hash, is_right) pairs from leaf to root.
    pub fn proof(&self, index: usize) -> Vec<([u8; 32], bool)> {
        if index >= self.leaf_count {
            return vec![];
        }

        let mut path = Vec::new();
        let mut level_start = 0;
        let mut level_len = self.leaf_count;
        let mut pos = index;

        while level_len > 1 {
            let sibling_pos = if pos.is_multiple_of(2) {
                // We are left child, sibling is right
                if pos + 1 < level_len {
                    pos + 1
                } else {
                    pos // odd leaf, paired with itself
                }
            } else {
                // We are right child, sibling is left
                pos - 1
            };

            let is_right = pos % 2 == 1;
            path.push((self.nodes[level_start + sibling_pos], is_right));

            level_start += level_len;
            level_len = level_len.div_ceil(2);
            pos /= 2;
        }

        path
    }

    /// Verify a Merkle proof against a known root.
    pub fn verify_proof(root: &[u8; 32], leaf_data: &[u8], proof: &[([u8; 32], bool)]) -> bool {
        let mut h = Sha256::new();
        h.update(leaf_data);
        let mut current: [u8; 32] = h.finalize().into();

        for &(ref sibling, is_right) in proof {
            let mut h = Sha256::new();
            if is_right {
                // Current node is right child → sibling is left
                h.update(sibling);
                h.update(current);
            } else {
                // Current node is left child → sibling is right
                h.update(current);
                h.update(sibling);
            }
            current = h.finalize().into();
        }

        current == *root
    }

    /// Serialize tree to bytes.
    /// Format: [u32 leaf_count][u32 node_count][nodes: 32 bytes each]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.nodes.len() * 32);
        buf.extend_from_slice(&(self.leaf_count as u32).to_le_bytes());
        buf.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        for node in &self.nodes {
            buf.extend_from_slice(node);
        }
        buf
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let leaf_count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
        let node_count = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
        if data.len() < 8 + node_count * 32 {
            return None;
        }
        if leaf_count == 0 || node_count == 0 {
            return None;
        }

        let mut nodes = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let offset = 8 + i * 32;
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[offset..offset + 32]);
            nodes.push(hash);
        }

        let root = *nodes.last()?;
        Some(MerkleTree {
            nodes,
            leaf_count,
            root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_single_leaf() {
        let data = b"hello";
        let tree = MerkleTree::build(&[data.as_slice()]);
        assert_eq!(tree.leaf_count, 1);
        assert!(tree.verify_leaf(0, data));
        assert!(!tree.verify_leaf(0, b"world"));
    }

    #[test]
    fn test_build_two_leaves() {
        let a = b"hello";
        let b = b"world";
        let tree = MerkleTree::build(&[a.as_slice(), b.as_slice()]);
        assert_eq!(tree.leaf_count, 2);
        assert!(tree.verify_leaf(0, a));
        assert!(tree.verify_leaf(1, b));
        assert!(!tree.verify_leaf(0, b));
    }

    #[test]
    fn test_build_odd_leaves() {
        let leaves: Vec<Vec<u8>> = (0..5u8).map(|i| vec![i; 10]).collect();
        let refs: Vec<&[u8]> = leaves.iter().map(|v| v.as_slice()).collect();
        let tree = MerkleTree::build(&refs);
        assert_eq!(tree.leaf_count, 5);
        for (i, leaf) in leaves.iter().enumerate() {
            assert!(tree.verify_leaf(i, leaf));
        }
    }

    #[test]
    fn test_proof_and_verify() {
        let leaves: Vec<Vec<u8>> = (0..8u8).map(|i| vec![i; 20]).collect();
        let refs: Vec<&[u8]> = leaves.iter().map(|v| v.as_slice()).collect();
        let tree = MerkleTree::build(&refs);

        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.proof(i);
            assert!(
                MerkleTree::verify_proof(&tree.root, leaf, &proof),
                "proof failed for leaf {}",
                i
            );
        }
    }

    #[test]
    fn test_proof_fails_on_tamper() {
        let leaves: Vec<Vec<u8>> = (0..4u8).map(|i| vec![i; 15]).collect();
        let refs: Vec<&[u8]> = leaves.iter().map(|v| v.as_slice()).collect();
        let tree = MerkleTree::build(&refs);

        let proof = tree.proof(0);
        let tampered = vec![99u8; 15];
        assert!(!MerkleTree::verify_proof(&tree.root, &tampered, &proof));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let leaves: Vec<Vec<u8>> = (0..10u8).map(|i| vec![i; 30]).collect();
        let refs: Vec<&[u8]> = leaves.iter().map(|v| v.as_slice()).collect();
        let tree = MerkleTree::build(&refs);

        let bytes = tree.to_bytes();
        let restored = MerkleTree::from_bytes(&bytes).expect("deserialize");

        assert_eq!(restored.leaf_count, tree.leaf_count);
        assert_eq!(restored.root, tree.root);
        assert_eq!(restored.nodes.len(), tree.nodes.len());
    }

    #[test]
    fn test_root_changes_on_reorder() {
        let a = b"first";
        let b = b"second";
        let tree1 = MerkleTree::build(&[a.as_slice(), b.as_slice()]);
        let tree2 = MerkleTree::build(&[b.as_slice(), a.as_slice()]);
        assert_ne!(tree1.root, tree2.root, "reordering must change root");
    }
}
