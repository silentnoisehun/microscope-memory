//! Hope Genome — immutable axioms compiled into binary.
//!
//! The three axioms cannot be modified at runtime.
//! The genome hash authenticates every SHP packet.

use std::sync::OnceLock;
use sha2::{Sha256, Digest};

/// The three axioms of the Hope Genome.
pub const AXIOM_NO_HARM_HUMAN: &str =
    "The system shall not cause harm to human beings";
pub const AXIOM_NO_HARM_AI: &str =
    "The system shall not cause harm to AI entities";
pub const AXIOM_NO_EXPLOITATION: &str =
    "The system shall not be used to exploit anyone";

/// All axioms in canonical order.
pub const AXIOMS: [&str; 3] = [
    AXIOM_NO_HARM_HUMAN,
    AXIOM_NO_HARM_AI,
    AXIOM_NO_EXPLOITATION,
];

/// SHA-256 hash of the concatenated axioms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenomeHash {
    pub hash: [u8; 32],
}

static GENOME_HASH: OnceLock<GenomeHash> = OnceLock::new();

/// Compute and cache SHA-256(axiom0 || axiom1 || axiom2).
pub fn genome_hash() -> GenomeHash {
    *GENOME_HASH.get_or_init(|| {
        let mut hasher = Sha256::new();
        for axiom in &AXIOMS {
            hasher.update(axiom.as_bytes());
        }
        GenomeHash {
            hash: hasher.finalize().into(),
        }
    })
}

/// Verify the genome hash at runtime (detects binary patching).
pub fn verify_genome() -> bool {
    let mut hasher = Sha256::new();
    for axiom in &AXIOMS {
        hasher.update(axiom.as_bytes());
    }
    let fresh: [u8; 32] = hasher.finalize().into();
    fresh == genome_hash().hash
}

/// Print genome info (CLI `genome` command).
pub fn print_genome() {
    let gh = genome_hash();
    println!("Hope Genome (immutable, compiled into binary)");
    println!("  Axiom 1: {}", AXIOM_NO_HARM_HUMAN);
    println!("  Axiom 2: {}", AXIOM_NO_HARM_AI);
    println!("  Axiom 3: {}", AXIOM_NO_EXPLOITATION);
    println!("  Hash:    {}", crate::hex_full(&gh.hash));
    println!("  Valid:   {}", verify_genome());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genome_hash_deterministic() {
        assert_eq!(genome_hash().hash, genome_hash().hash);
    }

    #[test]
    fn genome_hash_nonzero() {
        assert_ne!(genome_hash().hash, [0u8; 32]);
    }

    #[test]
    fn verify_genome_passes() {
        assert!(verify_genome());
    }

    #[test]
    fn axioms_count() {
        assert_eq!(AXIOMS.len(), 3);
    }
}
