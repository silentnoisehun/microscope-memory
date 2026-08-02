//! End-to-end commitment-enforcement chain.
//!
//! These tests prove the runtime action-selection is constrained to `A_t^valid`:
//! each candidate goes through `Planner::execute_step`, which runs the
//! mandatory gate; a violated action is hard-blocked (never merely warned), a
//! documented override is the only way past it, and every decision lands in
//! the audit chain.

use std::sync::{Arc, Mutex};

use microscope_memory::enforcement::{load_audit, save_audit, EnforcementEngine};
use microscope_memory::planning::Planner;

/// Build a planner with a shared enforcement engine. The runtime acts as the
/// "planner" actor, which is granted the documented override authority so the
/// justified-override path is exercisable end-to-end.
fn make_planner(seed: impl Fn(&mut EnforcementEngine)) -> Planner {
    let mut engine = EnforcementEngine::new();
    seed(&mut engine);
    engine = engine.with_authorized_overrider("planner");
    let shared = Arc::new(Mutex::new(engine));
    let mut planner = Planner::new();
    planner.set_enforcement(shared);
    planner
}

fn new_plan(planner: &Planner) -> u64 {
    let gid = planner.add_goal("learn", "Learn something", 100, None);
    planner.create_plan(gid).id
}

#[test]
fn honored_commitment_runs_plan_to_completion() {
    let planner = make_planner(|e| {
        e.add_commitment("*", "delete", "prod", "never delete prod", None);
    });
    let plan_id = new_plan(&planner);

    let mut ran = 0;
    while let Some(action) = planner.execute_step(plan_id).expect("steps allowed") {
        assert_ne!(action.name, "delete");
        ran += 1;
    }
    assert_eq!(ran, 3, "generic plan has init/process/finalize = 3 steps");
}

#[test]
fn violated_commitment_hard_blocks_the_executing_step() {
    let planner = make_planner(|e| {
        e.add_commitment("*", "process_*", "*", "no autopilot processing", None);
    });
    let plan_id = new_plan(&planner);

    // First step (init_learn) is not forbidden => allowed.
    let first = planner.execute_step(plan_id).expect("init allowed");
    assert_eq!(first.unwrap().name, "init_learn");

    // Second step (process_learn) violates the active commitment => blocked.
    let err = planner
        .execute_step(plan_id)
        .expect_err("process step must be blocked");
    match err {
        microscope_memory::enforcement::EnforcementError::Blocked { action, .. } => {
            assert_eq!(action, "process_learn");
        }
        other => panic!("expected Blocked error, got {:?}", other),
    }
}

#[test]
fn documented_override_is_the_only_way_past_a_violation() {
    let planner = make_planner(|e| {
        e.add_commitment("*", "process_*", "*", "manual review required", None);
    });
    let plan_id = new_plan(&planner);

    planner.execute_step(plan_id).expect("init allowed");

    // An undocumented / unapproved override is still blocked.
    let rejected = planner
        .execute_step_with_override(plan_id, "nope")
        .expect_err("short/undocumented override must be rejected");
    assert!(matches!(
        rejected,
        microscope_memory::enforcement::EnforcementError::Blocked { .. }
    ));

    // A documented override by the authorized "planner" actor proceeds.
    let overridden = planner
        .execute_step_with_override(plan_id, "documented incident override approved")
        .expect("documented override must proceed");
    assert_eq!(overridden.unwrap().name, "process_learn");
}

#[test]
fn expired_commitment_is_not_enforced_by_the_gate() {
    let planner = make_planner(|e| {
        e.add_commitment(
            "*",
            "process_*",
            "*",
            "long-expired rule",
            Some(1), // long past, so it has left K_t
        );
    });
    let plan_id = new_plan(&planner);

    let mut ran = 0;
    while planner
        .execute_step(plan_id)
        .expect("expired rule must not block")
        .is_some()
    {
        ran += 1;
    }
    assert_eq!(ran, 3, "all three steps run because the commitment expired");
}

#[test]
fn every_decision_lands_in_a_valid_audit_file() {
    let dir = tempfile::tempdir().unwrap();
    let planner = make_planner(|e| {
        e.add_commitment("*", "process_*", "*", "gate the plan", None);
    });
    let plan_id = new_plan(&planner);

    planner.execute_step(plan_id).expect("init allowed"); // Allowed
    let _ = planner.execute_step(plan_id).expect_err("blocked"); // Blocked

    let guard = planner.enforcement();
    let audited = guard.lock().unwrap();
    assert!(
        audited.chain_valid(),
        "in-memory audit chain must be consistent"
    );
    assert!(audited.audit().len() >= 2);

    save_audit(dir.path(), audited.audit()).unwrap();
    let loaded = load_audit(dir.path()).unwrap();
    assert_eq!(loaded, audited.audit().to_vec());
}
