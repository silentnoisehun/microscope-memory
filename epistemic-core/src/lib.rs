//! # epistemic-core
//!
//! Epistemic accountability for AI systems. Memory-system-agnostic.
//!
//! Every claim a system makes must be decomposed into:
//!
//! 1. **Claim** — what is asserted
//! 2. **Evidence** — what observed data supports it
//! 3. **Reasoning** — a traversable graph of inference steps from evidence to claim
//! 4. **Split Confidence** — three separate scores:
//!    - evidence_confidence: how reliable is the raw evidence
//!    - reasoning_confidence: how sound is the logical path (penalized rules reduce this)
//!    - narrative_confidence: how much is interpretation vs. supported
//!
//! The gate blocks claims from promotion if any confidence dimension
//! falls below its threshold. Penalized inference rules (e.g.
//! `ObservationToMotivation`) automatically reduce reasoning confidence
//! and are flagged in the audit.

pub mod binary;
pub mod evidence_source;
pub mod gate;
pub mod graph;
pub mod rules;
pub mod types;

pub use binary::*;
pub use evidence_source::*;
pub use gate::*;
pub use graph::*;
pub use rules::*;
pub use types::*;
