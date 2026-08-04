//! Reasoning graph — a traversable DAG from evidence to claim.

use crate::rules::{InferenceRule, RuleRegistry};
use crate::types::*;

// ─── Reasoning Graph ────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReasoningGraph {
    pub nodes: Vec<ReasoningNode>,
    pub edges: Vec<InferenceStep>,
    pub root: ReasoningNodeId,
}

impl ReasoningGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            root: ReasoningNodeId(0),
        }
    }

    pub fn add_evidence(&mut self, evidence_id: EvidenceId) -> ReasoningNodeId {
        let id = ReasoningNodeId(self.nodes.len() as u32);
        self.nodes.push(ReasoningNode::Evidence { id: evidence_id });
        id
    }

    pub fn add_conclusion(
        &mut self,
        claim_id: ClaimId,
        text: impl Into<String>,
    ) -> ReasoningNodeId {
        let id = ReasoningNodeId(self.nodes.len() as u32);
        self.nodes.push(ReasoningNode::Conclusion {
            id: claim_id,
            text: text.into(),
        });
        id
    }

    pub fn add_step(
        &mut self,
        premise: ReasoningNodeId,
        rule: InferenceRule,
        conclusion: ReasoningNodeId,
        raw_confidence: f64,
        registry: &RuleRegistry,
    ) {
        let penalized = registry.apply(rule, raw_confidence);
        self.edges.push(InferenceStep {
            premise,
            rule,
            conclusion,
            confidence: penalized,
        });
    }

    pub fn set_root(&mut self, root: ReasoningNodeId) {
        self.root = root;
    }

    pub fn node(&self, id: ReasoningNodeId) -> Option<&ReasoningNode> {
        self.nodes.get(id.0 as usize)
    }

    pub fn edges_into(&self, node: ReasoningNodeId) -> Vec<&InferenceStep> {
        self.edges.iter().filter(|e| e.conclusion == node).collect()
    }

    pub fn edges_from(&self, node: ReasoningNodeId) -> Vec<&InferenceStep> {
        self.edges.iter().filter(|e| e.premise == node).collect()
    }

    /// Compute the reasoning confidence for the root claim.
    ///
    /// Walks the DAG from root to leaves. At each conclusion node,
    /// takes the minimum of (edge_confidence × premise_path_confidence)
    /// across all incoming edges. Evidence leaves return 1.0.
    pub fn reasoning_confidence(&self) -> f64 {
        self.path_confidence(self.root)
    }

    fn path_confidence(&self, node: ReasoningNodeId) -> f64 {
        let incoming = self.edges_into(node);
        if incoming.is_empty() {
            return 1.0;
        }

        incoming
            .iter()
            .map(|step| {
                let premise_conf = self.path_confidence(step.premise);
                step.confidence * premise_conf
            })
            .fold(f64::INFINITY, f64::min)
    }

    pub fn penalized_steps(&self) -> Vec<(&InferenceStep, f64)> {
        self.edges
            .iter()
            .filter(|e| e.rule.is_penalized())
            .map(|e| (e, e.confidence))
            .collect()
    }

    /// Trace the full path from root back to all evidence leaves.
    pub fn trace_to_evidence(&self) -> Vec<TraceStep> {
        let mut trace = Vec::new();
        self.trace_node(self.root, &mut trace);
        trace
    }

    fn trace_node(&self, node: ReasoningNodeId, trace: &mut Vec<TraceStep>) {
        let incoming = self.edges_into(node);
        let node_ref = self.node(node);

        for step in &incoming {
            trace.push(TraceStep {
                node,
                node_desc: node_ref.map(n_desc),
                rule: step.rule,
                step_confidence: step.confidence,
            });
            self.trace_node(step.premise, trace);
        }
    }

    /// Compute narrative confidence — the fraction of the claim
    /// that is directly supported by non-penalized reasoning.
    ///
    /// A graph with no penalized rules: narrative = 1.0.
    /// Each penalized step drags narrative down proportional to
    /// its penalty damage (1.0 - penalty_factor).
    pub fn narrative_confidence(&self) -> f64 {
        if self.edges.is_empty() {
            return 0.0;
        }

        let total = self.edges.len() as f64;

        // Sum of penalty damages: how much each penalized step hurts
        let total_damage: f64 = self
            .edges
            .iter()
            .filter(|e| e.rule.is_penalized())
            .map(|e| 1.0 - e.rule.penalty_factor())
            .sum();

        // Narrative = 1.0 - (total_damage / total)
        // If all edges are maximally penalized (×0.30): damage = 0.70 per edge
        //   → narrative = 1.0 - 0.70 = 0.30
        // If no penalized edges: damage = 0 → narrative = 1.0
        (1.0 - total_damage / total).clamp(0.0, 1.0)
    }

    /// Check graph acyclicity (DAG invariant).
    pub fn is_acyclic(&self) -> bool {
        let n = self.nodes.len();
        let mut visited = vec![false; n];
        let mut on_stack = vec![false; n];

        fn dfs(
            graph: &ReasoningGraph,
            node: usize,
            visited: &mut [bool],
            on_stack: &mut [bool],
        ) -> bool {
            visited[node] = true;
            on_stack[node] = true;

            let nid = ReasoningNodeId(node as u32);
            for step in graph.edges_from(nid) {
                let next = step.conclusion.0 as usize;
                if !visited[next] {
                    if !dfs(graph, next, visited, on_stack) {
                        return false;
                    }
                } else if on_stack[next] {
                    return false;
                }
            }

            on_stack[node] = false;
            true
        }

        for i in 0..n {
            if !visited[i] && !dfs(self, i, &mut visited, &mut on_stack) {
                return false;
            }
        }
        true
    }
}

impl Default for ReasoningGraph {
    fn default() -> Self {
        Self::new()
    }
}

fn n_desc(n: &ReasoningNode) -> String {
    match n {
        ReasoningNode::Evidence { id } => format!("evidence#{id}"),
        ReasoningNode::Conclusion { id, text } => format!("claim#{id}: {text}"),
    }
}

// ─── Trace Step ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TraceStep {
    pub node: ReasoningNodeId,
    pub node_desc: Option<String>,
    pub rule: InferenceRule,
    pub step_confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> RuleRegistry {
        RuleRegistry::new()
    }

    #[test]
    fn simple_graph_two_evidence_one_conclusion() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let e1 = g.add_evidence(100);
        let e2 = g.add_evidence(200);
        let c1 = g.add_conclusion(1, "intermediate: amnesia is real");
        let root = g.add_conclusion(2, "Mate nelkul semmi");

        g.add_step(e1, InferenceRule::ObservationToExistence, c1, 0.95, &reg);
        g.add_step(e2, InferenceRule::ObservationToExistence, c1, 0.95, &reg);
        g.add_step(c1, InferenceRule::CounterfactualObserved, root, 0.92, &reg);
        g.add_step(e1, InferenceRule::CounterfactualObserved, root, 0.88, &reg);
        g.set_root(root);

        assert!(g.is_acyclic());
        let rc = g.reasoning_confidence();
        assert!(rc > 0.85, "reasoning confidence should be high, got {rc}");
    }

    #[test]
    fn penalized_rule_reduces_reasoning_confidence() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let e1 = g.add_evidence(300);
        let root = g.add_conclusion(3, "ontological decision");

        g.add_step(e1, InferenceRule::CentralityToMotivation, root, 0.94, &reg);
        g.set_root(root);

        let rc = g.reasoning_confidence();
        assert!(
            rc < 0.35,
            "penalized rule should crush confidence, got {rc}"
        );
        assert!((rc - 0.329).abs() < 0.01, "expected ~0.329, got {rc}");

        let penalized = g.penalized_steps();
        assert_eq!(penalized.len(), 1);
        assert_eq!(penalized[0].0.rule, InferenceRule::CentralityToMotivation);
    }

    #[test]
    fn shared_activity_to_relational_bond_is_heavily_penalized() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let e1 = g.add_evidence(400);
        let root = g.add_conclusion(4, "something between us");

        g.add_step(
            e1,
            InferenceRule::SharedActivityToRelationalBond,
            root,
            0.82,
            &reg,
        );
        g.set_root(root);

        let rc = g.reasoning_confidence();
        assert!(rc < 0.42, "should be heavily penalized, got {rc}");
    }

    #[test]
    fn cycle_detection() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let a = g.add_conclusion(10, "A");
        let b = g.add_conclusion(11, "B");
        let c = g.add_conclusion(12, "C");

        g.add_step(a, InferenceRule::Generalization, b, 0.9, &reg);
        g.add_step(b, InferenceRule::Generalization, c, 0.9, &reg);
        g.add_step(c, InferenceRule::Generalization, a, 0.9, &reg);

        assert!(!g.is_acyclic(), "should detect cycle");
    }

    #[test]
    fn trace_to_evidence_works() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let e1 = g.add_evidence(500);
        let root = g.add_conclusion(5, "final claim");

        g.add_step(e1, InferenceRule::ConvergentEvidence, root, 0.90, &reg);
        g.set_root(root);

        let trace = g.trace_to_evidence();
        assert!(!trace.is_empty());
        assert_eq!(trace[0].rule, InferenceRule::ConvergentEvidence);
    }

    #[test]
    fn narrative_confidence_no_penalized() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let e1 = g.add_evidence(600);
        let root = g.add_conclusion(6, "clean claim");
        g.add_step(e1, InferenceRule::ConvergentEvidence, root, 0.90, &reg);
        g.set_root(root);

        let nc = g.narrative_confidence();
        assert!(
            (nc - 1.0).abs() < 0.001,
            "no penalized rules → narrative = 1.0, got {nc}"
        );
    }

    #[test]
    fn narrative_confidence_with_penalized() {
        let mut g = ReasoningGraph::new();
        let reg = make_registry();

        let e1 = g.add_evidence(700);
        let root = g.add_conclusion(7, "speculative claim");
        g.add_step(e1, InferenceRule::CentralityToMotivation, root, 0.90, &reg);
        g.set_root(root);

        let nc = g.narrative_confidence();
        // 1 penalized edge, damage = 1.0 - 0.35 = 0.65
        // narrative = 1.0 - 0.65/1 = 0.35
        assert!(
            nc < 0.40,
            "penalized rule should reduce narrative, got {nc}"
        );
    }
}
