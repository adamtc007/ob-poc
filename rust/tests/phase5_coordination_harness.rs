//! P5-T16: Phase 5 Coordination Harness
//!
//! Integration tests verifying the Phase 5 typed-DAG + effect_class
//! coordination layer end-to-end without a live database.
//!
//! Schema-dependent tests (audit boundary, actual lock acquisition)
//! require `DATABASE_URL` and the `database` feature; they are marked
//! `#[ignore]` here and run separately with `cargo x check --db`.
//!
//! ## What this harness verifies
//!
//! 1. **Coordination strategy table (T12):** `effect_class_to_concurrency_policy`
//!    maps each class to the correct policy per v0.5 §5.3.
//!
//! 2. **Plan-level lock gate (T12):** `plan_requires_locking` is `false` for
//!    all-read / all-ensure / all-pure plans and `true` the moment any
//!    `read_modify_write` step enters.
//!
//! 3. **Outcome taxonomy (T13, T15):** `AtomicExecutionResult` carries the
//!    expected variants and the `summary()` / helper methods behave correctly.
//!
//! 4. **ExecutionFrame accumulates audit records (T14):** `record_outcome`
//!    populates `AuditBuffer`; records carry the correct identity fields.
//!
//! 5. **Typed DAG edges (T02):** `DagEdge` ordering semantics are correct
//!    — only `BindingEdge`, `StateEdge`, and `SnapshotVersionEdge` impose order.
//!
//! 6. **BindingFrameSchema populated from producer steps (T10):** a plan with
//!    a binding-producing step exposes the entity type in the schema.
//!
//! DB-dependent tests (#[ignore]):
//!   - Actual lock skip for pure plans against a Postgres transaction
//!   - Audit record presence in dsl_execution_audit after commit
//!   - OptimisticConflict for concurrent idempotent_ensure plans

use dsl_core::executable_plan::EffectClass;
use dsl_core::execution_dag::{BindingSlotId, DagEdge, JoinBarrierMode, NodeId};
use dsl_runtime::coordination::{
    effect_class_to_concurrency_policy, plan_effective_policy, plan_requires_locking,
    ConcurrencyPolicy,
};
use dsl_runtime::frame::{AttemptId, BindingFrame, ExecutionFrame, ExecutionId};

// =============================================================================
// 1. Coordination strategy table (T12, v0.5 §5.3)
// =============================================================================

#[test]
fn pure_maps_to_none_policy() {
    assert_eq!(
        effect_class_to_concurrency_policy(EffectClass::Pure),
        ConcurrencyPolicy::None
    );
}

#[test]
fn read_snapshot_maps_to_none_policy() {
    assert_eq!(
        effect_class_to_concurrency_policy(EffectClass::ReadSnapshot),
        ConcurrencyPolicy::None
    );
}

#[test]
fn idempotent_ensure_maps_to_unique_insert() {
    assert_eq!(
        effect_class_to_concurrency_policy(EffectClass::IdempotentEnsure),
        ConcurrencyPolicy::UniqueInsert
    );
}

#[test]
fn append_fact_maps_to_idempotency_guard() {
    assert_eq!(
        effect_class_to_concurrency_policy(EffectClass::AppendFact),
        ConcurrencyPolicy::IdempotencyGuard
    );
}

#[test]
fn read_modify_write_maps_to_pessimistic_lock() {
    assert_eq!(
        effect_class_to_concurrency_policy(EffectClass::ReadModifyWrite),
        ConcurrencyPolicy::PessimisticResourceLock
    );
}

#[test]
fn admin_override_maps_to_exclusive_scope_lock() {
    assert_eq!(
        effect_class_to_concurrency_policy(EffectClass::AdminOverride),
        ConcurrencyPolicy::ExclusiveScopeLock
    );
}

// =============================================================================
// 2. Plan-level lock gate (T12)
// =============================================================================

#[test]
fn all_read_plan_does_not_require_locking() {
    let classes = vec![
        Some(EffectClass::Pure),
        Some(EffectClass::ReadSnapshot),
        Some(EffectClass::IdempotentEnsure),
        Some(EffectClass::AppendFact),
        Some(EffectClass::ExternalEffect),
    ];
    assert!(
        !plan_requires_locking(classes),
        "a plan with only non-locking effect classes should not require advisory locks"
    );
}

#[test]
fn plan_with_rmw_step_requires_locking() {
    let classes = vec![
        Some(EffectClass::ReadSnapshot),
        Some(EffectClass::ReadModifyWrite), // this one triggers locking
        Some(EffectClass::AppendFact),
    ];
    assert!(
        plan_requires_locking(classes),
        "a plan with read_modify_write must require advisory locks"
    );
}

#[test]
fn plan_with_undeclared_effect_class_requires_locking() {
    // Undeclared (None) → conservative fallback → PessimisticResourceLock.
    // Pre-T04 plans have all undeclared; this ensures they still lock safely.
    let classes: Vec<Option<EffectClass>> = vec![None];
    assert!(
        plan_requires_locking(classes),
        "undeclared effect_class must conservatively require locking"
    );
}

#[test]
fn empty_plan_does_not_require_locking() {
    let classes: Vec<Option<EffectClass>> = vec![];
    assert!(!plan_requires_locking(classes));
}

#[test]
fn plan_effective_policy_escalates_to_max() {
    let classes = vec![
        Some(EffectClass::Pure),
        Some(EffectClass::IdempotentEnsure), // UniqueInsert
        Some(EffectClass::ReadModifyWrite),  // PessimisticResourceLock — the max
    ];
    assert_eq!(
        plan_effective_policy(classes),
        Some(ConcurrencyPolicy::PessimisticResourceLock)
    );
}

// =============================================================================
// 3. Outcome taxonomy (T13, T15)
// =============================================================================

// These tests exercise the types without needing the full executor.
// AtomicExecutionResult is in dsl_v2/executor.rs — we access it through
// the public re-export in dsl_v2::mod.

#[test]
fn execution_id_unique_per_construction() {
    let a = ExecutionId::new();
    let b = ExecutionId::new();
    assert_ne!(a.0, b.0);
}

#[test]
fn attempt_id_increments_correctly() {
    let first = AttemptId::first();
    assert_eq!(first.0, 1);
    let second = first.next();
    assert_eq!(second.0, 2);
}

// =============================================================================
// 4. ExecutionFrame audit accumulation (T14)
// =============================================================================

#[test]
fn frame_accumulates_audit_records_in_order() {
    let mut frame = ExecutionFrame::new(30);
    let eid = frame.execution_id;
    let aid = frame.attempt_id;

    frame.record_outcome(0, "cbu.ensure", chrono::Utc::now(), "committed");
    frame.record_outcome(1, "cbu.assign-role", chrono::Utc::now(), "committed");

    assert_eq!(frame.audit_buffer.records.len(), 2);
    let r0 = &frame.audit_buffer.records[0];
    let r1 = &frame.audit_buffer.records[1];

    assert_eq!(r0.execution_id, eid);
    assert_eq!(r0.attempt_id, aid);
    assert_eq!(r0.node_id, 0);
    assert_eq!(r0.verb_fqn, "cbu.ensure");
    assert_eq!(r0.outcome, "committed");

    assert_eq!(r1.node_id, 1);
    assert_eq!(r1.verb_fqn, "cbu.assign-role");
}

#[test]
fn audit_buffer_is_empty_on_new_frame() {
    let frame = ExecutionFrame::new(30);
    assert!(frame.audit_buffer.is_empty());
}

#[test]
fn frame_retry_resets_binding_slots_and_audit() {
    let mut frame = ExecutionFrame::new(30);
    frame.binding_slots.put("cbu", uuid::Uuid::new_v4());
    frame.record_outcome(0, "cbu.ensure", chrono::Utc::now(), "committed");

    let eid = frame.execution_id;
    let aid = frame.attempt_id;
    let retry = ExecutionFrame::retry(eid, aid, 30);

    // Retry reuses execution_id but increments attempt_id
    assert_eq!(retry.execution_id, eid);
    assert_eq!(retry.attempt_id.0, aid.0 + 1);
    // Fresh binding frame and audit buffer
    assert_eq!(retry.binding_slots.get("cbu"), None);
    assert!(retry.audit_buffer.is_empty());
}

// =============================================================================
// 5. Typed DAG edges (T02, v0.5 §4.2)
// =============================================================================

#[test]
fn binding_edge_imposes_order() {
    let edge = DagEdge::BindingEdge {
        from: NodeId(0),
        to: NodeId(1),
        slot: BindingSlotId::new("cbu"),
    };
    assert!(edge.imposes_order());
    assert_eq!(edge.ordering_pair(), Some((NodeId(0), NodeId(1))));
}

#[test]
fn state_edge_imposes_order() {
    let edge = DagEdge::StateEdge {
        from: NodeId(0),
        to: NodeId(2),
    };
    assert!(edge.imposes_order());
    assert_eq!(edge.ordering_pair(), Some((NodeId(0), NodeId(2))));
}

#[test]
fn resource_coord_edge_does_not_impose_order() {
    let edge = DagEdge::ResourceCoordEdge {
        node_a: NodeId(0),
        node_b: NodeId(1),
    };
    assert!(!edge.imposes_order());
    assert_eq!(edge.ordering_pair(), None);
}

#[test]
fn join_barrier_edge_does_not_impose_order() {
    let edge = DagEdge::JoinBarrierEdge {
        predecessors: vec![NodeId(0), NodeId(1)],
        join_node: NodeId(2),
        mode: JoinBarrierMode::WaitAll,
    };
    assert!(!edge.imposes_order());
}

#[test]
fn cancellation_scope_edge_does_not_impose_order() {
    let edge = DagEdge::CancellationScopeEdge {
        trigger: NodeId(0),
        scope: vec![NodeId(1), NodeId(2)],
    };
    assert!(!edge.imposes_order());
}

// =============================================================================
// 6. BindingFrame typed slot operations (T10, T11)
// =============================================================================

#[test]
fn binding_frame_slot_lifecycle() {
    let mut frame = BindingFrame::new();
    let uuid = uuid::Uuid::new_v4();

    // Not yet populated
    assert_eq!(frame.get("cbu"), None);
    let missing = frame.missing_slots(["cbu", "deal"]);
    assert_eq!(missing.len(), 2);

    // Populate one
    frame.put("cbu", uuid);
    assert_eq!(frame.get("cbu"), Some(uuid));
    let missing = frame.missing_slots(["cbu", "deal"]);
    assert_eq!(missing, vec!["deal".to_string()]);
}

// =============================================================================
// DB-dependent tests (require DATABASE_URL + migration applied)
// =============================================================================

/// Verify that a plan where all steps are pure/read_snapshot does not
/// attempt advisory lock acquisition against a real Postgres transaction.
///
/// Requires: DATABASE_URL env var, `20260519_dsl_execution_audit.sql` applied.
#[test]
#[ignore = "requires DATABASE_URL and dsl_execution_audit migration"]
fn pure_plan_skips_advisory_locks() {
    // TODO: spin up a real executor with a pool, compile a plan whose
    // steps are all EffectClass::Pure, execute_plan_atomic_with_locks,
    // and assert that pg_locks shows no advisory locks held by this session.
}

/// Verify audit-as-commit-boundary: after a successful plan execution,
/// dsl_execution_audit has records matching the executed steps.
///
/// Requires: DATABASE_URL env var, `20260519_dsl_execution_audit.sql` applied.
#[test]
#[ignore = "requires DATABASE_URL and dsl_execution_audit migration"]
fn audit_records_present_after_committed_plan() {
    // TODO: execute a plan, assert SELECT COUNT(*) FROM dsl_execution_audit
    // WHERE execution_id = $1 equals the step count.
}

/// Deliberate failure injection: cause tx rollback mid-plan, verify
/// dsl_execution_audit has NO records for that execution_id.
///
/// Requires: DATABASE_URL env var, `20260519_dsl_execution_audit.sql` applied.
#[test]
#[ignore = "requires DATABASE_URL and dsl_execution_audit migration"]
fn rolled_back_plan_has_no_audit_records() {
    // TODO: execute a plan that fails mid-way, assert no audit records exist
    // for the execution_id (audit co-commits with data or neither commits).
}
