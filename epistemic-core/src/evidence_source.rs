//! Evidence source trait — memory-system-agnostic interface.
//!
//! Any AI system (Microscope Memory, vector DB, log store, etc.)
//! can implement this trait to provide evidence to epistemic-core.
//! The core never assumes how evidence is stored — it only requires
//! that evidence can be fetched and its confidence assessed.

use crate::types::{EpistemicClass, Evidence, EvidenceId};

/// A source of evidence for epistemic claims.
///
/// Implementors provide:
/// - `fetch`: retrieve evidence metadata by ID
/// - `compute_confidence`: assess the evidence confidence (0.0–1.0)
///
/// This decouples epistemic-core from any specific memory system.
pub trait EvidenceSource {
    /// Fetch evidence metadata by ID.
    fn fetch(&self, id: EvidenceId) -> Option<Evidence>;

    /// Compute evidence confidence for a given evidence ID.
    ///
    /// This is the **evidence_confidence** dimension of split confidence.
    /// It measures how reliable the raw observed data is, independent
    /// of any reasoning applied to it.
    ///
    /// Default implementation uses the evidence's pre-computed confidence.
    fn compute_confidence(&self, id: EvidenceId) -> f64 {
        self.fetch(id).map(|e| e.evidence_confidence).unwrap_or(0.0)
    }

    /// Check whether an evidence node can serve as a premise.
    /// Only Observation and Evidence classes can support claims.
    fn can_support(&self, id: EvidenceId) -> bool {
        self.fetch(id)
            .map(|e| e.class.can_support())
            .unwrap_or(false)
    }

    /// Batch fetch multiple evidence nodes.
    fn fetch_many(&self, ids: &[EvidenceId]) -> Vec<Option<Evidence>> {
        ids.iter().map(|&id| self.fetch(id)).collect()
    }
}

/// A simple in-memory evidence source for testing and prototyping.
#[derive(Debug, Clone)]
pub struct InMemoryEvidenceSource {
    evidence: std::collections::HashMap<EvidenceId, Evidence>,
}

impl InMemoryEvidenceSource {
    pub fn new() -> Self {
        Self {
            evidence: std::collections::HashMap::new(),
        }
    }

    pub fn add(&mut self, evidence: Evidence) {
        self.evidence.insert(evidence.id, evidence);
    }

    pub fn add_observation(&mut self, id: EvidenceId, distinct_sources: u32, confidence: f64) {
        self.add(Evidence {
            id,
            class: EpistemicClass::Observation,
            distinct_sources,
            refutations: 0,
            first_seen_ms: 0,
            evidence_confidence: confidence,
        });
    }
}

impl Default for InMemoryEvidenceSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceSource for InMemoryEvidenceSource {
    fn fetch(&self, id: EvidenceId) -> Option<Evidence> {
        self.evidence.get(&id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_source_fetch() {
        let mut src = InMemoryEvidenceSource::new();
        src.add_observation(42, 3, 0.85);
        let e = src.fetch(42).expect("should find evidence");
        assert_eq!(e.class, EpistemicClass::Observation);
        assert_eq!(e.distinct_sources, 3);
        assert!((e.evidence_confidence - 0.85).abs() < 0.001);
    }

    #[test]
    fn can_support_checks_class() {
        let mut src = InMemoryEvidenceSource::new();
        src.add_observation(1, 1, 0.5);
        assert!(src.can_support(1));

        // Add an Inference (cannot support)
        src.add(Evidence {
            id: 2,
            class: EpistemicClass::Inference,
            distinct_sources: 1,
            refutations: 0,
            first_seen_ms: 0,
            evidence_confidence: 0.5,
        });
        assert!(!src.can_support(2));
    }

    #[test]
    fn fetch_many() {
        let mut src = InMemoryEvidenceSource::new();
        src.add_observation(1, 1, 0.5);
        src.add_observation(2, 2, 0.7);
        let results = src.fetch_many(&[1, 2, 3]);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_none());
    }
}
