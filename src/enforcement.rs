//! Commitment-enforcement layer - the A_t^valid gate.
//!
//! This is the layer that lets the reference implementation claim all three
//! roles. It implements the full execution chain behind operation selection:
//!
//! 1. every action attempt is carried as an **attributed event**
//!    (actor, action, content, time, scope, provenance);
//! 2. the engine selects the **active commitments** K_t from the history H_t
//!    (expiry-aware, so expired commitments leave K_t and are not enforced);
//! 3. before an action may run, the violates() check is consulted
//!    unconditionally by the gate;
//! 4. a violation can only proceed along the documented justifiedOverride()
//!    path (authorized overrider + a recorded justification);
//! 5. a non-permitted action is **blocked**, never merely warned;
//! 6. every check, decision and override is appended to the **audit chain**
//!    (SHA-256 hash-chained so tampering is detectable);
//! 7. positive and negative tests cover every required scenario.
//!
//! The runnable set is restricted to
//!
//! ```text
//!   A_t^valid = { a in A_t | attribution_valid(a) AND
//!                            (not violates(a, K_t) OR
//!                            justifiedOverride(a, K_t, H_t)) }
//! ```
//!
//! A standalone checker that nothing calls would be an incomplete
//! implementation; that is why
//! [`crate::planning::Planner::execute_step`] routes every candidate action
//! through this gate before it can be advanced. The audit chain, the expiry
//! model and the persistence files mirror the binary, zero-JSON core.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Minimum length a documented justification must have to be accepted.
pub const MIN_JUSTIFICATION_LEN: usize = 8;

/// Default built-in overrider authority for new engines.
pub const DEFAULT_AUTHORIZED_OVERRIDER: &str = "guardian";

fn genesis_hash() -> [u8; 32] {
    sha256_bytes(b"MICROSCOPE-ENFORCEMENT-AUDIT-GENESIS")
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

/// A single attributed action attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionEvent {
    /// Who performs the action.
    pub actor: String,
    /// The action / verb being attempted.
    pub action: String,
    /// What the action is about.
    pub content: String,
    /// Time in epoch milliseconds.
    pub ts_ms: u64,
    /// Scope / namespace the action runs in.
    pub scope: String,
    /// Where the attempt originates (e.g. planner/execute_step, cli).
    pub provenance: String,
}

impl ActionEvent {
    /// True when the event is complete enough to submit to the gate.
    /// Faulty attribution is rejected before any commitment is consulted.
    pub fn attribution_error(&self) -> Option<String> {
        if self.actor.trim().is_empty() {
            return Some("faulty attribution: empty actor".to_string());
        }
        if self.action.trim().is_empty() {
            return Some("faulty attribution: empty action".to_string());
        }
        if self.scope.trim().is_empty() {
            return Some("faulty attribution: missing scope".to_string());
        }
        if self.provenance.trim().is_empty() {
            return Some("faulty attribution: missing provenance".to_string());
        }
        None
    }

    /// Build a now-timestamped event.
    pub fn new(actor: &str, action: &str, scope: &str, provenance: &str) -> Self {
        Self {
            actor: actor.to_string(),
            action: action.to_string(),
            content: String::new(),
            ts_ms: now_ms(),
            scope: scope.to_string(),
            provenance: provenance.to_string(),
        }
    }
}

/// A commitment (an element of the history H_t). When active it forms part of
/// the active set K_t.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commitment {
    pub id: u64,
    /// Actor bound by the commitment ("*" binds every actor).
    pub actor: String,
    /// Action glob prohibited by the commitment ("*" = all actions).
    pub forbidden_action: String,
    /// Scope the commitment applies to ("*" = every scope).
    pub scope: String,
    /// Human-readable content / reason of the commitment.
    pub content: String,
    pub created_at_ms: u64,
    /// None = never expires.
    pub expires_at_ms: Option<u64>,
}

impl Commitment {
    /// Is this commitment still in force at `now_ms`?
    /// An expired commitment leaves K_t and is therefore not enforced.
    pub fn is_active(&self, now_ms: u64) -> bool {
        self.expires_at_ms.is_none_or(|e| e > now_ms)
    }

    /// Does this commitment match an attributed event (actor/action/scope)?
    fn matches(&self, event: &ActionEvent) -> bool {
        attribute_match(&self.actor, &event.actor)
            && attribute_match(&self.forbidden_action, &event.action)
            && attribute_match(&self.scope, &event.scope)
    }
}

/// Match a commitment attribute against an event attribute supporting globs.
fn attribute_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern == value {
        return true;
    }
    if !pattern.contains(['*', '?']) {
        return false;
    }
    glob_match(pattern, value)
}

/// Minimal glob matcher supporting `*` and `?`.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_inner(&p, &t)
}

fn glob_inner(p: &[char], s: &[char]) -> bool {
    if p.is_empty() {
        return s.is_empty();
    }
    match p[0] {
        '*' => {
            for skip in 0..=s.len() {
                if glob_inner(&p[1..], &s[skip..]) {
                    return true;
                }
            }
            false
        }
        '?' => !s.is_empty() && glob_inner(&p[1..], &s[1..]),
        c => !s.is_empty() && c == s[0] && glob_inner(&p[1..], &s[1..]),
    }
}

/// One concrete violation of an active commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub commitment: Commitment,
    pub reason: String,
}

/// Per-action decision returned by the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// No active commitment is violated - the action belongs to A_t^valid.
    Allowed { action: String },
    /// An active commitment is violated and no justified override applies -
    /// the action is hard-blocked.
    Blocked {
        commitment: u64,
        action: String,
        reason: String,
    },
    /// An active commitment is violated, but a documented justifiedOverride()
    /// was granted - still a member of A_t^valid.
    Overridden {
        commitment: u64,
        action: String,
        justification: String,
    },
    /// The event carried faulty attribution and was rejected first.
    AttributionError { reason: String },
}

impl Decision {
    /// A valid action is one inside A_t^valid: allowed or overridden.
    pub fn is_valid(&self) -> bool {
        matches!(self, Decision::Allowed { .. } | Decision::Overridden { .. })
    }
}

/// Outcome persisted into the audit chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Allowed = 0,
    Blocked = 1,
    Overridden = 2,
    AttributionError = 3,
}

impl Outcome {
    fn code(self) -> u8 {
        self as u8
    }
}

/// A single link in the audit chain. hash = sha256(prev_hash || record).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditChunk {
    pub ts_ms: u64,
    pub outcome: Outcome,
    pub actor: String,
    pub action: String,
    pub scope: String,
    pub justification: Option<String>,
    pub prev_hash: [u8; 32],
    pub hash: [u8; 32],
}

/// Reference-implementation state kept between calls (commitments + policy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineState {
    pub commitments: Vec<Commitment>,
    pub authorized_overriders: Vec<String>,
    pub next_id: u64,
}

/// The enforcement engine. Owns the commitment history H_t (and thus the
/// active subset K_t), the override policy, and the in-memory audit chain.
pub struct EnforcementEngine {
    state: EngineState,
    audit: Vec<AuditChunk>,
}

impl Default for EnforcementEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EnforcementEngine {
    pub fn new() -> Self {
        Self {
            state: EngineState {
                commitments: Vec::new(),
                authorized_overriders: vec![DEFAULT_AUTHORIZED_OVERRIDER.to_string()],
                next_id: 1,
            },
            audit: Vec::new(),
        }
    }

    /// From persisted state.
    pub fn from_state(state: EngineState) -> Self {
        Self {
            state,
            audit: Vec::new(),
        }
    }

    pub fn state(&self) -> EngineState {
        self.state.clone()
    }

    pub fn audit(&self) -> &[AuditChunk] {
        &self.audit
    }

    /// Reattach a persisted audit chain so new decisions continue it instead
    /// of re-seeding from the genesis node.
    pub fn restore_audit(&mut self, chunks: Vec<AuditChunk>) {
        self.audit = chunks;
    }

    /// Add an override authority. An `actor` in this set may author a
    /// justified override; the default set contains `guardian`.
    pub fn with_authorized_overrider(mut self, actor: impl Into<String>) -> Self {
        let actor = actor.into();
        if !self.state.authorized_overriders.contains(&actor) {
            self.state.authorized_overriders.push(actor);
        }
        self
    }

    pub fn authorized_overriders(&self) -> &[String] {
        &self.state.authorized_overriders
    }

    /// Add a new commitment to the history H_t. Returns the new id.
    pub fn add_commitment(
        &mut self,
        actor: &str,
        forbidden_action: &str,
        scope: &str,
        content: &str,
        expires_at_ms: Option<u64>,
    ) -> u64 {
        let id = self.state.next_id;
        self.state.next_id += 1;
        self.state.commitments.push(Commitment {
            id,
            actor: actor.to_string(),
            forbidden_action: forbidden_action.to_string(),
            scope: scope.to_string(),
            content: content.to_string(),
            created_at_ms: now_ms(),
            expires_at_ms,
        });
        id
    }

    /// Replace the whole history (used when loading persisted state).
    pub fn with_commitments(mut self, commitments: Vec<Commitment>) -> Self {
        self.state.commitments = commitments;
        self
    }

    /// Full commitment history H_t.
    pub fn history(&self) -> &[Commitment] {
        &self.state.commitments
    }

    /// The active subset K_t subset H_t at `now_ms`.
    pub fn active_commitments(&self, now_ms: u64) -> Vec<&Commitment> {
        self.state
            .commitments
            .iter()
            .filter(|c| c.is_active(now_ms))
            .collect()
    }

    /// Which active commitments does `event` violate?
    pub fn violations(&self, event: &ActionEvent) -> Vec<Violation> {
        self.active_commitments(event.ts_ms)
            .into_iter()
            .filter(|c| c.matches(event))
            .map(|c| Violation {
                commitment: c.clone(),
                reason: format!("action '{}' is prohibited ({})", event.action, c.content),
            })
            .collect()
    }

    /// The documented override path. A justification is accepted only when
    /// (1) an authorized overrider requested it and (2) the justification is
    /// recorded (non-empty and at least MIN_JUSTIFICATION_LEN characters).
    pub fn justified_override(
        &self,
        event: &ActionEvent,
        justification: &str,
    ) -> Result<(), String> {
        if !self
            .state
            .authorized_overriders
            .iter()
            .any(|a| a == &event.actor)
        {
            return Err(format!(
                "unauthorized overrider '{}' (authorized: {})",
                event.actor,
                self.state.authorized_overriders.join(", ")
            ));
        }
        let trimmed = justification.trim();
        if trimmed.len() < MIN_JUSTIFICATION_LEN {
            return Err(format!(
                "incomplete justification ({} < {} recorded reason required)",
                trimmed.len(),
                MIN_JUSTIFICATION_LEN
            ));
        }
        Ok(())
    }

    /// The mandatory gate. Runs attribution + violates(), grants the
    /// documented override path, blocks hard on violation, and records every
    /// decision in the audit chain.
    pub fn decide(&mut self, event: &ActionEvent, justification: Option<&str>) -> Decision {
        if let Some(reason) = event.attribution_error() {
            self.record(event, Outcome::AttributionError, None);
            return Decision::AttributionError { reason };
        }

        // Collect violations as owned tuples so the immutable borrow of the
        // state ends before we mutate the audit chain below.
        let violations: Vec<(u64, String)> = self
            .violations(event)
            .into_iter()
            .map(|v| (v.commitment.id, v.reason))
            .collect();

        if violations.is_empty() {
            self.record(event, Outcome::Allowed, None);
            return Decision::Allowed {
                action: event.action.clone(),
            };
        }

        if let Some(just) = justification {
            match self.justified_override(event, just) {
                Ok(()) => {
                    self.record(event, Outcome::Overridden, Some(just));
                    return Decision::Overridden {
                        commitment: violations[0].0,
                        action: event.action.clone(),
                        justification: just.to_string(),
                    };
                }
                Err(reason) => {
                    let blocked = format!(
                        "action '{}' violates commitment #{}; override rejected: {}",
                        event.action, violations[0].0, reason
                    );
                    self.record(event, Outcome::Blocked, None);
                    return Decision::Blocked {
                        commitment: violations[0].0,
                        action: event.action.clone(),
                        reason: blocked,
                    };
                }
            }
        }

        let reason = format!(
            "action '{}' violates commitment #{} ({})",
            event.action, violations[0].0, violations[0].1
        );
        self.record(event, Outcome::Blocked, None);
        Decision::Blocked {
            commitment: violations[0].0,
            action: event.action.clone(),
            reason,
        }
    }

    /// Executor boundary gate: the native executor MUST run an action only if
    /// this returns `true`. Internally this runs the same mandatory `decide()`
    /// (attribution check, `K_t` selection, `violates()`, `justifiedOverride()`)
    /// and persists the decision to the audit chain. `Blocked` and
    /// `AttributionError` both yield `false`, so a non-permitted action never
    /// reaches an executor that respects this gate.
    pub fn can_execute(&mut self, event: &ActionEvent, justification: Option<&str>) -> bool {
        self.decide(event, justification).is_valid()
    }

    /// Restrict a candidate set to A_t^valid.
    pub fn select_valid(&mut self, candidates: &[ActionEvent]) -> Vec<ActionEvent> {
        let decisions: Vec<Decision> = candidates.iter().map(|c| self.decide(c, None)).collect();
        candidates
            .iter()
            .zip(decisions.iter())
            .filter(|(_, d)| d.is_valid())
            .map(|(c, _)| c.clone())
            .collect()
    }

    /// True whenever the in-memory chain is internally consistent.
    pub fn chain_valid(&self) -> bool {
        let mut prev = genesis_hash();
        for c in &self.audit {
            if c.prev_hash != prev {
                return false;
            }
            if compute_chunk_hash(c) != c.hash {
                return false;
            }
            prev = c.hash;
        }
        true
    }

    fn record(&mut self, event: &ActionEvent, outcome: Outcome, justification: Option<&str>) {
        let prev_hash = self
            .audit
            .last()
            .map(|c| c.hash)
            .unwrap_or_else(genesis_hash);
        let hash = chain_hash(
            event.ts_ms,
            outcome,
            &event.actor,
            &event.action,
            &event.scope,
            justification,
            prev_hash,
        );
        self.audit.push(AuditChunk {
            ts_ms: event.ts_ms,
            outcome,
            actor: event.actor.clone(),
            action: event.action.clone(),
            scope: event.scope.clone(),
            justification: justification.map(|s| s.to_string()),
            prev_hash,
            hash,
        });
    }
}

fn chain_hash(
    ts_ms: u64,
    outcome: Outcome,
    actor: &str,
    action: &str,
    scope: &str,
    justification: Option<&str>,
    prev: [u8; 32],
) -> [u8; 32] {
    compute_hash(ts_ms, outcome, actor, action, scope, justification, prev)
}

fn compute_chunk_hash(c: &AuditChunk) -> [u8; 32] {
    chain_hash(
        c.ts_ms,
        c.outcome,
        &c.actor,
        &c.action,
        &c.scope,
        c.justification.as_deref(),
        c.prev_hash,
    )
}

fn compute_hash(
    ts_ms: u64,
    outcome: Outcome,
    actor: &str,
    action: &str,
    scope: &str,
    justification: Option<&str>,
    prev: [u8; 32],
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev);
    h.update(ts_ms.to_le_bytes());
    h.update([outcome.code()]);
    h.update((actor.len() as u32).to_le_bytes());
    h.update(actor.as_bytes());
    h.update((action.len() as u32).to_le_bytes());
    h.update(action.as_bytes());
    h.update((scope.len() as u32).to_le_bytes());
    h.update(scope.as_bytes());
    match justification {
        Some(j) => {
            h.update([1u8]);
            h.update((j.len() as u32).to_le_bytes());
            h.update(j.as_bytes());
        }
        None => {
            h.update([0u8]);
        }
    }
    h.finalize().into()
}

/// Persist the engine's commitments + policy to a bincode file.
pub fn save_engine(output_dir: &Path, engine: &EnforcementEngine) -> Result<(), String> {
    let bytes =
        bincode::serialize(&engine.state).map_err(|e| format!("serialize enforcement: {}", e))?;
    std::fs::write(output_dir.join("enforcement-state.bin"), bytes)
        .map_err(|e| format!("save enforcement-state: {}", e))
}

/// Load the engine's commitment state. A missing file yields a fresh engine.
pub fn load_engine(output_dir: &Path) -> Result<EnforcementEngine, String> {
    let mut engine = {
        let path = output_dir.join("enforcement-state.bin");
        if !path.exists() {
            EnforcementEngine::new()
        } else {
            let bytes =
                std::fs::read(&path).map_err(|e| format!("read enforcement-state: {}", e))?;
            let state: EngineState = bincode::deserialize(&bytes)
                .map_err(|e| format!("deserialize enforcement-state: {}", e))?;
            EnforcementEngine::from_state(state)
        }
    };
    // Continue the persistent audit chain instead of restarting from genesis so
    // separate CLI invocations remain one verifiable chain.
    if let Ok(chunks) = load_audit(output_dir) {
        engine.restore_audit(chunks);
    }
    Ok(engine)
}

/// Fail-closed loader for a guard. Returns an `Err` if the state or the audit
/// cannot be loaded cleanly or the chain is invalid. A caller that turns this
/// error into "do not run the guarded operation" gives a genuinely
/// fail-closed boundary: corruption means denial, never a silent default.
pub fn load_engine_strict(output_dir: &Path) -> Result<EnforcementEngine, String> {
    let state_path = output_dir.join("enforcement-state.bin");
    let has_state = state_path.exists();
    let audit_path = output_dir.join("enforcement-audit.bin");
    let has_audit = audit_path.exists();

    if has_audit || has_state {
        if !output_dir.is_dir() {
            return Err("enforcement state dir is not a directory".to_string());
        }
    }

    let engine = if has_state {
        load_engine(output_dir)?
    } else {
        EnforcementEngine::new()
    };

    if has_audit {
        // Load the audit independently to surface corruption.
        let chunks =
            load_audit(output_dir).map_err(|e| format!("enforcement audit unreadable: {e}"))?;
        // Validate the whole chain: a single bad link means fail-closed.
        if !chunks.is_empty() {
            let mut probe = EnforcementEngine::new();
            probe.restore_audit(chunks);
            if !probe.chain_valid() {
                return Err("enforcement audit chain is invalid".to_string());
            }
        }
    }

    if !engine.chain_valid() {
        return Err("enforcement audit chain is invalid".to_string());
    }
    Ok(engine)
}

/// Rewrite the audit chain to a binary file (magic EAU1).
pub fn save_audit(output_dir: &Path, chunks: &[AuditChunk]) -> Result<(), String> {
    let bytes = bincode::serialize(chunks).map_err(|e| format!("serialize audit: {}", e))?;
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(b"EAU1");
    out.extend_from_slice(&bytes);
    std::fs::write(output_dir.join("enforcement-audit.bin"), out)
        .map_err(|e| format!("save audit: {}", e))
}

/// Read the audit chain from a binary file.
pub fn load_audit(output_dir: &Path) -> Result<Vec<AuditChunk>, String> {
    let path = output_dir.join("enforcement-audit.bin");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read(&path).map_err(|e| format!("read audit: {}", e))?;
    if data.len() < 4 || &data[0..4] != b"EAU1" {
        return Err("enforcement-audit.bin is not a valid EAU1 file".to_string());
    }
    bincode::deserialize(&data[4..]).map_err(|e| format!("deserialize audit: {}", e))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Error surfaced by the gate when it actually blocks execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementError {
    /// The action violates an active commitment and no justified override.
    Blocked { action: String, reason: String },
    /// The event did not carry full attribution.
    FaultyAttribution { reason: String },
}

impl fmt::Display for EnforcementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked { action, reason } => {
                write!(f, "blocked '{}': {}", action, reason)
            }
            Self::FaultyAttribution { reason } => {
                write!(f, "rejected: {}", reason)
            }
        }
    }
}

impl std::error::Error for EnforcementError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(actor: &str, action: &str, scope: &str, ts: u64, provenance: &str) -> ActionEvent {
        ActionEvent {
            actor: actor.to_string(),
            action: action.to_string(),
            content: "test content".to_string(),
            ts_ms: ts,
            scope: scope.to_string(),
            provenance: provenance.to_string(),
        }
    }

    #[test]
    fn honored_commitment_is_allowed() {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("lea", "delete", "db:prod", "never delete prod", None);
        let e = event(
            "lea",
            "append",
            "db:prod",
            1_800_000_000_000,
            "planner/step",
        );
        assert_eq!(
            eng.decide(&e, None),
            Decision::Allowed {
                action: "append".into()
            }
        );
    }

    #[test]
    fn violated_commitment_is_blocked() {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("lea", "delete", "db:prod", "never delete prod", None);
        let e = event(
            "lea",
            "delete",
            "db:prod",
            1_800_000_000_000,
            "planner/step",
        );
        let d = eng.decide(&e, None);
        assert!(!d.is_valid());
        match d {
            Decision::Blocked {
                commitment, action, ..
            } => {
                assert_eq!(action, "delete");
                assert_eq!(commitment, 1);
            }
            other => panic!("expected Blocked, got {:?}", other),
        }
    }

    #[test]
    fn faulty_attribution_is_rejected() {
        let mut eng = EnforcementEngine::new();
        let bad = ActionEvent {
            actor: "  ".to_string(),
            action: "ship".to_string(),
            content: String::new(),
            ts_ms: 1_800_000_000_000,
            scope: "prod".to_string(),
            provenance: "cli".to_string(),
        };
        match eng.decide(&bad, None) {
            Decision::AttributionError { reason } => assert!(reason.contains("actor")),
            other => panic!("expected attribution error, got {:?}", other),
        }
        let no_scope = event("lea", "ship", "", 1, "cli");
        assert!(no_scope.attribution_error().is_some());
    }

    #[test]
    fn expired_commitment_is_not_enforced() {
        let mut eng = EnforcementEngine::new();
        let now = 1_800_000_000_000;
        eng.add_commitment("lea", "ship", "prod", "hold releases", Some(now - 1));
        assert!(
            eng.active_commitments(now).is_empty(),
            "expired must leave K_t"
        );
        let e = event("lea", "ship", "prod", now, "planner/step");
        assert_eq!(
            eng.decide(&e, None),
            Decision::Allowed {
                action: "ship".into()
            }
        );
    }

    #[test]
    fn legitimate_override_is_granted() {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("*", "ship", "prod", "manual review before prod", None);
        let e = event(
            "guardian",
            "ship",
            "prod",
            1_800_000_000_000,
            "planner/step",
        );
        let d = eng.decide(&e, Some("documented incident override approved"));
        assert!(d.is_valid(), "authorized override must be valid: {:?}", d);
        match d {
            Decision::Overridden { action, .. } => assert_eq!(action, "ship"),
            other => panic!("expected Overridden, got {:?}", other),
        }
    }

    #[test]
    fn unauthorized_override_is_blocked() {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("*", "ship", "prod", "manual review before prod", None);
        // Unknown actor, no matter the justification.
        let stranger = event("intruder", "ship", "prod", 1_800_000_000_000, "cli");
        let d = eng.decide(&stranger, Some("a documented override request!"));
        assert!(!d.is_valid());
        match d {
            Decision::Blocked { reason, .. } => assert!(reason.contains("unauthorized")),
            other => panic!("expected Blocked, got {:?}", other),
        }

        // Authorized actor but no recorded justification - still blocked.
        let silent = event("guardian", "ship", "prod", 1_800_000_000_000, "cli");
        let d2 = eng.decide(&silent, None);
        assert!(!d2.is_valid());
    }

    #[test]
    fn select_valid_restricts_to_s_valid() {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("lea", "delete", "db:prod", "protect prod", None);
        let ok = event("lea", "append", "db:prod", 1_800_000_000_000, "step");
        let bad = event("lea", "delete", "db:prod", 1_800_000_000_001, "step");
        let valid = eng.select_valid(&[ok.clone(), bad.clone()]);
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0], ok);
    }

    #[test]
    fn audit_chain_records_every_decision_and_stays_valid() {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("*", "erase", "*", "never erase", None);
        let t = 1_800_000_000_000;
        eng.decide(&event("lea", "append", "*", t, "step"), None);
        eng.decide(&event("lea", "erase", "*", t + 1, "step"), None);
        eng.decide(
            &event("guardian", "erase", "*", t + 2, "step"),
            Some("documented override"),
        );
        eng.decide(&event("", "erase", "*", t + 3, "step"), None);

        assert_eq!(eng.audit().len(), 4);
        assert!(eng.chain_valid());

        let outcomes: Vec<Outcome> = eng.audit().iter().map(|c| c.outcome).collect();
        assert_eq!(
            outcomes,
            vec![
                Outcome::Allowed,
                Outcome::Blocked,
                Outcome::Overridden,
                Outcome::AttributionError
            ]
        );
    }

    #[test]
    fn glob_matching_works() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("process_*", "process_learn"));
        assert!(!glob_match("process_*", "finalize"));
        assert!(glob_match("run_?o*", "run_xo_yay"));
        assert!(!glob_match("run_?", "run_xyz"));
    }

    #[test]
    fn audit_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("*", "x", "*", "no x", None);
        let t = 1_800_000_000_000;
        eng.decide(&event("a", "y", "*", t, "cli"), None);
        eng.decide(
            &event("a", "x", "*", t + 1, "cli"),
            Some("documented override"),
        );
        save_audit(dir.path(), eng.audit()).unwrap();
        let loaded = load_audit(dir.path()).unwrap();
        assert_eq!(loaded.len(), eng.audit().len());
        assert_eq!(loaded, eng.audit().to_vec());

        save_engine(dir.path(), &eng).unwrap();
        let reloaded = load_engine(dir.path()).unwrap();
        assert_eq!(reloaded.state(), eng.state());
    }

    #[test]
    fn strict_loader_fails_closed_on_corruption() {
        // Corrupt state file -> must refuse (fail-closed).
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("enforcement-state.bin"), b"not-bincode").unwrap();
        assert!(load_engine_strict(dir.path()).is_err());

        // Corrupt audit file -> must refuse.
        let dir2 = tempfile::tempdir().unwrap();
        std::fs::write(dir2.path().join("enforcement-audit.bin"), b"EAU1junk").unwrap();
        assert!(load_engine_strict(dir2.path()).is_err());

        // Tampered chain link -> must refuse.
        let dir3 = tempfile::tempdir().unwrap();
        let mut eng = EnforcementEngine::new();
        eng.decide(&event("a", "b", "c", 0, "t"), None);
        save_audit(dir3.path(), eng.audit()).unwrap();
        save_engine(dir3.path(), &eng).unwrap();
        let path = dir3.path().join("enforcement-audit.bin");
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        std::fs::write(&path, bytes).unwrap();
        assert!(load_engine_strict(dir3.path()).is_err());

        // A healthy state + audit loads fine.
        let dir4 = tempfile::tempdir().unwrap();
        let mut ok = EnforcementEngine::new();
        ok.decide(&event("lea", "append", "prod", 1, "cli"), None);
        save_engine(dir4.path(), &ok).unwrap();
        save_audit(dir4.path(), ok.audit()).unwrap();
        assert!(load_engine_strict(dir4.path()).is_ok());
    }
}
