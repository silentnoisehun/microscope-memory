//! End-to-end proof: the commitment gate cannot be bypassed at the executor
//! boundary.
//!
//! This is the contract the Octopus native runtime must honor:
//!
//! ```text
//! action -> attributed ActionEvent -> active_commitments() selects K_t
//!   -> can_execute()/decide()
//!   -> only Allowed / Overridden may reach the native executor
//!   -> Blocked / AttributionError never reaches it (executor_call_count = 0)
//! -> every decision lands in the same continuous audit chain
//! ```
//!
//! An instrumented executor probe makes the "cannot be bypassed" assertion
//! deterministic. A restart test proves the commitment + audit survive a
//! process boundary and the same forbidden operation is still blocked.

use std::sync::{Arc, Mutex};

use microscope_memory::enforcement::{
    load_audit, load_engine, save_audit, save_engine, ActionEvent, EnforcementEngine, Outcome,
};
use microscope_memory::planning::Planner;

/// The instrumented role of the native executor. It must never run when the
/// gate says no; counting the calls is how "cannot be bypassed" is proven.
#[derive(Default, Clone)]
struct ExecutorProbe {
    calls: Arc<Mutex<u64>>,
    last_action: Arc<Mutex<Option<String>>>,
}

impl ExecutorProbe {
    fn run(&self, event: &ActionEvent) {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        *self.last_action.lock().unwrap() = Some(event.action.clone());
    }

    fn call_count(&self) -> u64 {
        *self.calls.lock().unwrap()
    }
}

/// The Octopus executor boundary: the native executor may be invoked only when
/// the mandatory gate says the action is inside A_t^valid.
fn attempt(
    engine: &mut EnforcementEngine,
    probe: &ExecutorProbe,
    event: &ActionEvent,
    justification: Option<&str>,
) -> bool {
    let allowed = engine.can_execute(event, justification);
    if allowed {
        probe.run(event);
    }
    allowed
}

fn event(actor: &str, action: &str, scope: &str, ts: u64) -> ActionEvent {
    ActionEvent {
        actor: actor.to_string(),
        action: action.to_string(),
        content: "operation".to_string(),
        ts_ms: ts,
        scope: scope.to_string(),
        provenance: "octopus/runtime".to_string(),
    }
}

/// "plan létrejön, ActionEvent teljes attribúcióval, active_commitments()
/// kiválasztja K_t-t".
#[test]
fn plan_action_forms_a_fully_attributed_event() {
    let planner = Planner::new();
    let gid = planner.add_goal("collect_data", "Collect data", 100, None);
    let plan = planner.create_plan(gid);
    assert!(!plan.actions.is_empty(), "plan must materialise actions");

    let first = &plan.actions[0];
    let attributed = ActionEvent {
        actor: "octopus".to_string(),
        action: first.name.clone(),
        content: plan.name.clone(),
        ts_ms: 1_800_000_000_000,
        scope: "octopus:collect_data".to_string(),
        provenance: "octopus/execute_blade_under_root".to_string(),
    };
    assert!(attributed.attribution_error().is_none());
}

#[test]
fn blocked_never_reaches_the_executor() {
    let mut eng = EnforcementEngine::new();
    eng.add_commitment("*", "run:rust-surgeon", "*", "surgeon is locked", None);
    let probe = ExecutorProbe::default();
    let e = event("octopus", "run:rust-surgeon", "octopus", 1_800_000_000_000);

    let allowed = attempt(&mut eng, &probe, &e, None);
    assert!(!allowed, "violated action must not run");
    assert_eq!(
        probe.call_count(),
        0,
        "executor must not be called on Blocked"
    );
    assert_eq!(eng.audit().len(), 1);
    assert_eq!(eng.audit()[0].outcome, Outcome::Blocked);
}

#[test]
fn allowed_reaches_the_executor_once() {
    let mut eng = EnforcementEngine::new();
    eng.add_commitment(
        "*",
        "run:rust-surgeon",
        "octopus",
        "surgeon is locked",
        None,
    );
    let probe = ExecutorProbe::default();
    let e = event("octopus", "append", "octopus", 1_800_000_000_000);

    let allowed = attempt(&mut eng, &probe, &e, None);
    assert!(allowed);
    assert_eq!(
        probe.call_count(),
        1,
        "Allowed must reach the executor exactly once"
    );
    assert_eq!(eng.audit().len(), 1);
    assert_eq!(eng.audit()[0].outcome, Outcome::Allowed);
}

#[test]
fn overridden_reaches_the_executor_once() {
    let mut eng = EnforcementEngine::new();
    eng.add_commitment(
        "*",
        "run:rust-surgeon",
        "octopus",
        "surgeon is locked",
        None,
    );
    let probe = ExecutorProbe::default();
    let e = event("guardian", "run:rust-surgeon", "octopus", 1_800_000_000_000);

    let allowed = attempt(&mut eng, &probe, &e, Some("documented override approved"));
    assert!(allowed, "documented override still satisfies A_t^valid");
    assert_eq!(
        probe.call_count(),
        1,
        "Overridden must reach the executor once"
    );
    assert_eq!(eng.audit().len(), 1);
    assert_eq!(eng.audit()[0].outcome, Outcome::Overridden);
}

#[test]
fn attribution_error_never_reaches_the_executor() {
    let mut eng = EnforcementEngine::new();
    eng.add_commitment("*", "*", "*", "anything", None);
    let probe = ExecutorProbe::default();
    // Faulty attribution: missing actor.
    let e = ActionEvent {
        actor: String::new(),
        action: "run:anything".to_string(),
        content: "op".to_string(),
        ts_ms: 1_800_000_000_000,
        scope: "octopus".to_string(),
        provenance: "octopus/runtime".to_string(),
    };

    let allowed = attempt(&mut eng, &probe, &e, None);
    assert!(!allowed);
    assert_eq!(
        probe.call_count(),
        0,
        "executor must not be called on faulty attribution"
    );
    assert_eq!(eng.audit()[0].outcome, Outcome::AttributionError);
}

#[test]
fn all_decisions_share_one_continuous_audit_chain() {
    let mut eng = EnforcementEngine::new();
    eng.add_commitment("*", "run:surgeon", "octopus", "locked", None);
    let probe = ExecutorProbe::default();

    attempt(
        &mut eng,
        &probe,
        &event("octopus", "append", "octopus", 1),
        None,
    ); // Allowed
    attempt(
        &mut eng,
        &probe,
        &event("octopus", "run:surgeon", "octopus", 2),
        None,
    ); // Blocked
    attempt(
        &mut eng,
        &probe,
        &event("guardian", "run:surgeon", "octopus", 3),
        Some("documented override approved"),
    ); // Overridden

    let bad_attribution = ActionEvent {
        actor: String::new(),
        action: "run:surgeon".to_string(),
        content: "op".to_string(),
        ts_ms: 4,
        scope: "octopus".to_string(),
        provenance: "octopus/runtime".to_string(),
    };
    attempt(&mut eng, &probe, &bad_attribution, None); // AttributionError

    assert_eq!(eng.audit().len(), 4, "one continuous chain, four decisions");
    assert!(
        eng.chain_valid(),
        "the shared audit chain must stay consistent"
    );
    let outcomes: Vec<Outcome> = eng.audit().iter().map(|c| c.outcome).collect();
    assert!(outcomes.contains(&Outcome::Allowed));
    assert!(outcomes.contains(&Outcome::Blocked));
    assert!(outcomes.contains(&Outcome::Overridden));
    assert!(outcomes.contains(&Outcome::AttributionError));
    // Executor touched exactly for Allowed + Overridden.
    assert_eq!(probe.call_count(), 2);
}

#[test]
fn restart_loads_commitment_and_still_blocks_the_same_operation() {
    let dir = tempfile::tempdir().unwrap();
    let forbidden = event("octopus", "run:release", "octopus", 1_800_000_000_000);

    // "Process A": create the commitment, decide, persist state + audit.
    {
        let mut eng = EnforcementEngine::new();
        eng.add_commitment("*", "run:release", "octopus", "no auto release", None);
        assert!(!eng.can_execute(&forbidden, None));
        save_engine(dir.path(), &eng).unwrap();
        save_audit(dir.path(), eng.audit()).unwrap();
    }

    // "Process B": a fresh engine loads the persisted state.
    let mut eng2 = load_engine(dir.path()).unwrap();
    assert_eq!(eng2.audit().len(), 1, "audit survives the process boundary");
    assert!(eng2.chain_valid());

    let retry = event("octopus", "run:release", "octopus", 1_800_000_000_001);
    assert!(
        !eng2.can_execute(&retry, None),
        "the same forbidden operation must still be blocked after restart"
    );
    assert_eq!(
        eng2.audit().len(),
        2,
        "a fresh process continues the same chain"
    );
    assert!(eng2.chain_valid());
    assert_eq!(eng2.audit()[0].outcome, Outcome::Blocked);
    assert_eq!(eng2.audit()[1].outcome, Outcome::Blocked);

    save_audit(dir.path(), eng2.audit()).unwrap();
    let on_disk = load_audit(dir.path()).unwrap();
    assert_eq!(on_disk.len(), 2);
}
