//! ExecutablePlan / execution policy golden snapshot tests — Tranche 1 regression baseline.
//!
//! Tests the `TransactionPolicy::from_effect_classes` composition logic and the
//! `PopulatedExecutionDag` edge model with typical binding chains.
//!
//! Full `ExecutablePlan` construction (which wraps `ExecutionStepSummary` from
//! ob-poc's execution pipeline) requires database wiring and is deferred.
//! These tests cover:
//!   - `TransactionPolicy` inference from representative effect-class combinations
//!   - `PopulatedExecutionDag` construction with binding, state, and snapshot edges
//!   - Effect class / transaction policy composition for the most common patterns
//!
//! No DB required — all tests run in pure Rust.

use dsl_core::{
    EffectClass, TransactionPolicy,
    execution_dag::{BindingSlotId, DagEdge, JoinBarrierMode, NodeId, PopulatedExecutionDag},
};

// =============================================================================
// TransactionPolicy inference — golden shapes
// =============================================================================

#[test]
fn plan_policy_pure_read_snapshot_is_read_only() {
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::Pure,
        EffectClass::ReadSnapshot,
        EffectClass::Pure,
    ]);
    insta::assert_debug_snapshot!("plan_policy_pure_read_snapshot_is_read_only", policy);
}

#[test]
fn plan_policy_idempotent_ensure_is_durable_step() {
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::IdempotentEnsure,
        EffectClass::IdempotentEnsure,
    ]);
    insta::assert_debug_snapshot!("plan_policy_idempotent_ensure_is_durable_step", policy);
}

#[test]
fn plan_policy_append_fact_is_durable_step() {
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::AppendFact,
        EffectClass::AppendTransitionSnapshot,
    ]);
    insta::assert_debug_snapshot!("plan_policy_append_fact_is_durable_step", policy);
}

#[test]
fn plan_policy_read_modify_write_is_atomic_short() {
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::ReadModifyWrite,
        EffectClass::AppendFact,
    ]);
    insta::assert_debug_snapshot!("plan_policy_read_modify_write_is_atomic_short", policy);
}

#[test]
fn plan_policy_cross_resource_invariant_is_atomic_short() {
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::ReadModifyWrite,
        EffectClass::CrossResourceInvariant,
    ]);
    insta::assert_debug_snapshot!(
        "plan_policy_cross_resource_invariant_is_atomic_short",
        policy
    );
}

#[test]
fn plan_policy_admin_override_is_atomic_short() {
    let policy = TransactionPolicy::from_effect_classes([EffectClass::AdminOverride]);
    insta::assert_debug_snapshot!("plan_policy_admin_override_is_atomic_short", policy);
}

#[test]
fn plan_policy_empty_is_read_only() {
    let policy = TransactionPolicy::from_effect_classes([]);
    insta::assert_debug_snapshot!("plan_policy_empty_is_read_only", policy);
}

#[test]
fn plan_policy_mixed_onboarding_flow() {
    // Typical onboarding: IdempotentEnsure (cbu.create) +
    //   ReadModifyWrite (cbu.assign-role) + ReadModifyWrite (kyc-case.create)
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::IdempotentEnsure,
        EffectClass::ReadModifyWrite,
        EffectClass::ReadModifyWrite,
    ]);
    insta::assert_debug_snapshot!("plan_policy_mixed_onboarding_flow", policy);
}

#[test]
fn plan_policy_screening_external_effect() {
    // External screening calls use ExternalEffect
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::ExternalEffect,
        EffectClass::ExternalEffect,
    ]);
    insta::assert_debug_snapshot!("plan_policy_screening_external_effect", policy);
}

#[test]
fn plan_policy_governance_publish_flow() {
    // Governance pipeline: ReadModifyWrite + AppendFact + AdminOverride
    let policy = TransactionPolicy::from_effect_classes([
        EffectClass::ReadModifyWrite,
        EffectClass::AppendFact,
        EffectClass::AdminOverride,
    ]);
    insta::assert_debug_snapshot!("plan_policy_governance_publish_flow", policy);
}

// =============================================================================
// PopulatedExecutionDag — golden shapes
// =============================================================================

#[test]
fn dag_empty_has_no_edges() {
    let dag = PopulatedExecutionDag::new();
    insta::assert_debug_snapshot!("dag_empty_has_no_edges", dag);
}

#[test]
fn dag_single_binding_edge() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::BindingEdge {
        from: NodeId(0),
        to: NodeId(1),
        slot: BindingSlotId::new("cbu"),
    });
    insta::assert_debug_snapshot!("dag_single_binding_edge", dag);
}

#[test]
fn dag_binding_chain_three_steps() {
    // Step 0 produces @cbu → Step 1 produces @case → Step 2 consumes @case
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::BindingEdge {
        from: NodeId(0),
        to: NodeId(1),
        slot: BindingSlotId::new("cbu"),
    });
    dag.add_edge(DagEdge::BindingEdge {
        from: NodeId(1),
        to: NodeId(2),
        slot: BindingSlotId::new("case"),
    });
    insta::assert_debug_snapshot!("dag_binding_chain_three_steps", dag);
}

#[test]
fn dag_state_edge() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::StateEdge {
        from: NodeId(0),
        to: NodeId(1),
    });
    insta::assert_debug_snapshot!("dag_state_edge", dag);
}

#[test]
fn dag_snapshot_version_edge() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::SnapshotVersionEdge {
        from: NodeId(0),
        to: NodeId(1),
    });
    insta::assert_debug_snapshot!("dag_snapshot_version_edge", dag);
}

#[test]
fn dag_resource_coord_edge() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::ResourceCoordEdge {
        node_a: NodeId(0),
        node_b: NodeId(2),
    });
    insta::assert_debug_snapshot!("dag_resource_coord_edge", dag);
}

#[test]
fn dag_join_barrier_wait_all() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::JoinBarrierEdge {
        predecessors: vec![NodeId(0), NodeId(1), NodeId(2)],
        join_node: NodeId(3),
        mode: JoinBarrierMode::WaitAll,
    });
    insta::assert_debug_snapshot!("dag_join_barrier_wait_all", dag);
}

#[test]
fn dag_cancellation_scope_stub() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::CancellationScopeEdge {
        trigger: NodeId(0),
        scope: vec![NodeId(1), NodeId(2)],
    });
    insta::assert_debug_snapshot!("dag_cancellation_scope_stub", dag);
}

#[test]
fn dag_ordering_edges_filter() {
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::BindingEdge {
        from: NodeId(0),
        to: NodeId(1),
        slot: BindingSlotId::new("cbu"),
    });
    dag.add_edge(DagEdge::ResourceCoordEdge {
        node_a: NodeId(0),
        node_b: NodeId(1),
    });
    dag.add_edge(DagEdge::StateEdge {
        from: NodeId(1),
        to: NodeId(2),
    });

    let ordering_pairs = dag.ordering_pairs();
    insta::assert_debug_snapshot!("dag_ordering_edges_filter", ordering_pairs);
}

#[test]
fn dag_complex_onboarding_graph() {
    // Represents:  (0) cbu.create → (1) kyc-case.create → (2) screening.pep
    //              (0) resource_coord with (3) entity.create
    let mut dag = PopulatedExecutionDag::new();
    dag.add_edge(DagEdge::BindingEdge {
        from: NodeId(0),
        to: NodeId(1),
        slot: BindingSlotId::new("cbu"),
    });
    dag.add_edge(DagEdge::BindingEdge {
        from: NodeId(1),
        to: NodeId(2),
        slot: BindingSlotId::new("case"),
    });
    dag.add_edge(DagEdge::ResourceCoordEdge {
        node_a: NodeId(0),
        node_b: NodeId(3),
    });
    dag.add_edge(DagEdge::StateEdge {
        from: NodeId(2),
        to: NodeId(4),
    });
    insta::assert_debug_snapshot!("dag_complex_onboarding_graph", dag);
}
