//! Populated Execution DAG — typed edge model for the ob-poc DSL execution engine.
//!
//! Implements v0.5 §4.1 (six typed edge types), §4.2 (ordering semantics),
//! §4.5 (DAG as load-bearing structure), §4.7 (join barrier semantics).
//!
//! # Edge types and their runtime meaning
//!
//! ```text
//! BindingEdge         — hard ordering; consumer needs produced UUID/value
//! StateEdge           — hard ordering; consumer needs state transition precondition
//! ResourceCoordEdge   — coordination per effect class (may or may not impose order)
//! SnapshotVersionEdge — hard ordering + optimistic version check at consumer time
//! JoinBarrierEdge     — fan-in control flow (WaitAll in Phase 5)
//! CancellationScopeEdge — lifecycle metadata stub (Phase 6: CascadePlanner)
//! ```
//!
//! # v1.3 component mapping (per P5-T01 decision record)
//!
//! | Edge type | Runtime evaluator |
//! |-----------|-------------------|
//! | `StateEdge` | `GateChecker::check_transition` (dsl-runtime::cross_workspace::gate_checker) |
//! | `SnapshotVersionEdge` | `DerivedStateEvaluator` (Mode B tollgate, dsl-runtime::cross_workspace::derived_state) |
//! | `ResourceCoordEdge` | coordination strategy table (T12) |
//! | `CancellationScopeEdge` | Phase 6 stub; future home of `CascadePlanner` |

/// Identifies a node (step) within a `PopulatedExecutionDag` by its index in the
/// `ExecutionPlan::steps` vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node({})", self.0)
    }
}

/// Identifies a binding slot by name (the `@name` in DSL source, or `$name` in typed form).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingSlotId(pub String);

impl BindingSlotId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl std::fmt::Display for BindingSlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

/// Join barrier modes (v0.5 §4.7).
///
/// Phase 5 implements `WaitAll` only. `WaitN` and `WaitSubset` are Phase 6
/// (require BPMN inclusive-gateway / race-pattern use cases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinBarrierMode {
    /// All declared predecessor nodes must complete with a compatible outcome
    /// before the join node fires. Failure of any predecessor propagates as
    /// failure of the join.
    ///
    /// v0.5 §4.7: "fires when every declared predecessor has reached a terminal outcome"
    WaitAll,
    // WaitN(usize, RemainderPolicy) — Phase 6
    // WaitSubset(SubsetId) — Phase 6
}

/// A typed edge in the Populated Execution DAG (v0.5 §4.1).
///
/// The six edge types have distinct runtime meanings. The runtime does NOT
/// infer a single meaning from a generic "dependency" edge; each type is
/// dispatched to its own evaluation path.
#[derive(Debug, Clone)]
pub enum DagEdge {
    /// Consumer node consumes a typed binding produced by producer node.
    ///
    /// **Hard execution-order constraint:** producer must complete and populate
    /// the binding slot before consumer fires.
    ///
    /// v0.5 §4.1, §4.2 (first in ordering set)
    /// Runtime: binding-slot population in `ExecutionFrame::binding_slots` (T10).
    BindingEdge {
        from: NodeId,
        to: NodeId,
        /// The binding slot name (`@name`) that flows from producer to consumer.
        slot: BindingSlotId,
    },

    /// Consumer node requires a state transition completed by producer node,
    /// even when no value flows between them.
    ///
    /// **Hard execution-order constraint:** the transition is the precondition.
    ///
    /// v0.5 §4.1, §4.2 (second in ordering set)
    /// Runtime evaluator: `GateChecker::check_transition`
    ///   (dsl-runtime::cross_workspace::gate_checker).
    ///   Wired via `GatePipeline` pre-dispatch hook in
    ///   `src/runbook/step_executor_bridge.rs`.
    StateEdge {
        from: NodeId,
        to: NodeId,
    },

    /// Two nodes touch the same governed resource.
    ///
    /// **Coordination per effect class** — which coordination strategy applies
    /// (None / IdempotencyGuard / UniqueInsert / OptimisticSnapshotCheck /
    /// PessimisticResourceLock / ExclusiveScopeLock) is determined by the
    /// connected nodes' declared `effect_class` values.
    ///
    /// This edge may or may not impose execution ordering depending on the
    /// resolved coordination strategy (v0.5 §4.2, §4.4).
    ///
    /// v0.5 §4.1, §5.3, §6.4
    /// Runtime: coordination strategy table (T12).
    ResourceCoordEdge {
        node_a: NodeId,
        node_b: NodeId,
    },

    /// Consumer expects a specific snapshot or version produced or observed
    /// by the producer node.
    ///
    /// **Hard execution-order constraint** plus optimistic version validation
    /// at consumer execution time.
    ///
    /// v0.5 §4.1, §4.2 (third in ordering set)
    /// Runtime evaluator: `DerivedStateEvaluator` for Mode B tollgate conditions
    ///   (dsl-runtime::cross_workspace::derived_state).
    SnapshotVersionEdge {
        from: NodeId,
        to: NodeId,
    },

    /// Join node waits for completion of declared predecessor set.
    ///
    /// Fan-in control flow. Phase 5 supports `WaitAll` only.
    ///
    /// v0.5 §4.1, §4.7
    /// Runtime: join barrier evaluation in executor (populated in Phase 5;
    ///   `WaitN`/`WaitSubset` variants in Phase 6).
    JoinBarrierEdge {
        predecessors: Vec<NodeId>,
        join_node: NodeId,
        mode: JoinBarrierMode,
    },

    /// Trigger node can cancel in-scope descendants when it fires a cancellation
    /// outcome. Lifecycle / control metadata — NOT an ordering edge.
    ///
    /// **Phase 5 stub:** declared but not enforced. Phase 6 will wire
    /// `CascadePlanner` (dsl-runtime::cross_workspace::hierarchy_cascade) here.
    ///
    /// v0.5 §4.1, §7.6
    CancellationScopeEdge {
        trigger: NodeId,
        scope: Vec<NodeId>,
    },
}

impl DagEdge {
    /// Returns `true` if this edge type imposes hard execution ordering (v0.5 §4.2).
    ///
    /// Three edge types impose order: `BindingEdge`, `StateEdge`, `SnapshotVersionEdge`.
    /// `ResourceCoordEdge`, `JoinBarrierEdge`, and `CancellationScopeEdge` do not.
    pub fn imposes_order(&self) -> bool {
        matches!(
            self,
            DagEdge::BindingEdge { .. }
                | DagEdge::StateEdge { .. }
                | DagEdge::SnapshotVersionEdge { .. }
        )
    }

    /// Returns `(from, to)` for ordering edges; `None` for non-ordering edges.
    pub fn ordering_pair(&self) -> Option<(NodeId, NodeId)> {
        match self {
            DagEdge::BindingEdge { from, to, .. } => Some((*from, *to)),
            DagEdge::StateEdge { from, to } => Some((*from, *to)),
            DagEdge::SnapshotVersionEdge { from, to } => Some((*from, *to)),
            _ => None,
        }
    }
}

/// The Populated Execution DAG — the load-bearing runtime structure of the system.
///
/// Contains the typed edge set for one `ExecutablePlan`. Immutable once the
/// compiler has emitted it. The runtime uses it as the sole authority for:
/// - execution order (ordering edges)
/// - coordination requirements (ResourceCoordEdge + effect_class)
/// - barrier conditions (JoinBarrierEdge)
/// - cancellation scope (CancellationScopeEdge stub)
///
/// v0.5 §4.5
#[derive(Debug, Clone, Default)]
pub struct PopulatedExecutionDag {
    pub edges: Vec<DagEdge>,
}

impl PopulatedExecutionDag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_edge(&mut self, edge: DagEdge) {
        self.edges.push(edge);
    }

    /// Returns all edges that impose hard execution ordering (v0.5 §4.2).
    pub fn ordering_edges(&self) -> impl Iterator<Item = &DagEdge> {
        self.edges.iter().filter(|e| e.imposes_order())
    }

    /// Returns `(from_step_idx, to_step_idx)` pairs for all ordering edges.
    ///
    /// Used by the topological sort to derive execution order from typed edges
    /// rather than from untyped `Injection` records.
    pub fn ordering_pairs(&self) -> Vec<(usize, usize)> {
        self.edges
            .iter()
            .filter_map(|e| e.ordering_pair())
            .map(|(from, to)| (from.0, to.0))
            .collect()
    }

    /// Returns all `ResourceCoordEdge` entries.
    ///
    /// Used by the coordination strategy table (T12) to determine which
    /// node pairs need coordination and at what granularity.
    pub fn coordination_edges(&self) -> impl Iterator<Item = (&NodeId, &NodeId)> {
        self.edges.iter().filter_map(|e| {
            if let DagEdge::ResourceCoordEdge { node_a, node_b } = e {
                Some((node_a, node_b))
            } else {
                None
            }
        })
    }
}
