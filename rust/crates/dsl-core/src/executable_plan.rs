//! ExecutablePlan — the immutable runtime contract emitted by the compiler.
//!
//! Implements v0.5 §3.1 (runtime contract), §3.2 (structure), §3.3
//! (`sem_os_snapshot_id` as load-bearing provenance), §5.2 (effect classes),
//! §8.2 (transaction policies), §10 (identity separation).
//!
//! # Relationship to ExecutionPlan
//!
//! `ExecutionPlan` (in `dsl_v2::execution_plan`) is the ob-poc-specific
//! dependency-sorted step sequence produced by `compile()`. `ExecutablePlan`
//! is the generic, versioned runtime contract that wraps that information in
//! a self-contained, snapshot-pinned artifact.
//!
//! Phase 5: both coexist. The executor accepts `ExecutionPlan`; `ExecutablePlan`
//! is constructed alongside it for provenance, policy, and audit purposes.
//! Phase 6: `ExecutablePlan` becomes the sole runtime input.
//!
//! # Identity separation (v0.5 §10)
//!
//! | Identity | Meaning |
//! |----------|---------|
//! | `PlanId` | Identity of the compiled `ExecutablePlan` |
//! | `ExecutionId` | Assigned by executor at submission time (not in this struct) |
//! | `AttemptId` | Per-retry within an execution (not in this struct) |
//! | `SemOsSnapshotId` | The SDG snapshot this plan was compiled against |

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::config::resource_dependency::ResolvedResourceDependency;
use crate::execution_dag::{BindingSlotId, NodeId, PopulatedExecutionDag};

// =============================================================================
// Identity types
// =============================================================================

/// Identity of a compiled `ExecutablePlan` (v0.5 §10.1).
///
/// Assigned once at plan compilation time. Immutable for the lifetime of
/// the plan. Two compilations of identical DSL source against identical
/// snapshots produce different `PlanId`s — the ID is not content-addressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PlanId(pub Uuid);

impl PlanId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PlanId {
    fn default() -> Self {
        Self::new()
    }
}

/// Identity of the Semantic Dependency Graph snapshot this plan was compiled
/// against (v0.5 §3.3, §10.1).
///
/// `sem_os_snapshot_id` is load-bearing provenance, not metadata:
/// - Two plans with different snapshot IDs may have different effect-class
///   declarations, different coordination policies, different binding type rules.
/// - Runtime compatibility checks against this ID are mandatory once the
///   admission controller is in place (Phase 6).
/// - Audit trails record this ID to support historical replay.
///
/// Structurally identical to `ob-poc-types::CatalogueSnapshotId(u64)`.
/// Converted at the ob-poc boundary via `Into`/`From`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SemOsSnapshotId(pub u64);

impl SemOsSnapshotId {
    pub fn unknown() -> Self {
        Self(0)
    }
}

// =============================================================================
// Effect class (v0.5 §5.2)
// =============================================================================

/// Verb effect class — the policy framework declaration (v0.5 §5.2).
///
/// Each verb declares exactly one effect class. The compiler attaches it to
/// every `RuntimeInstruction`. The runtime derives the concurrency policy from
/// this class via the coordination strategy table (T12).
///
/// **No verb may acquire locks directly** (v0.5 §5.5). The runtime
/// coordination layer owns all lock acquisition, idempotency checks,
/// optimistic guards, conflict handling, and lock timeout behaviour.
///
/// # Coexistence with `three_axis`
///
/// `effect_class` answers "how does the runtime serialize this verb against
/// concurrent plans?" `three_axis` answers "what gate UX and authority tier
/// applies before the verb fires?" They are orthogonal (D1 decision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    /// No mutation; no side effects. Coordination: None.
    Pure,
    /// Reads stable snapshot or projection. Coordination: snapshot reference.
    ReadSnapshot,
    /// Create-if-absent; returns existing if found.
    /// Coordination: UniqueInsert (DB unique constraint on natural key).
    IdempotentEnsure,
    /// Immutable fact or event insert.
    /// Coordination: UniqueInsert / optimistic append with unique event key.
    AppendFact,
    /// Workflow/process state snapshot insert (expected-predecessor semantics).
    /// Coordination: OptimisticSnapshotCheck (CAS).
    /// BPMN canonical case per v0.5 §8.3.
    AppendTransitionSnapshot,
    /// Order-independent contribution to an accumulator.
    /// Coordination: UniqueInsert per contribution key.
    CommutativeAccumulate,
    /// Reads current state then mutates the same resource.
    /// Coordination: PessimisticResourceLock (advisory lock, transaction-scoped).
    ReadModifyWrite,
    /// Maintains invariant across multiple entities.
    /// Coordination: ordered PessimisticResourceLock across all resource UUIDs.
    CrossResourceInvariant,
    /// Calls an external system. Coordination: Outbox + IdempotencyGuard.
    /// Avoid long locks; use outbox pattern for external calls.
    ExternalEffect,
    /// Repair / migration / manual correction.
    /// Coordination: ExclusiveScopeLock.
    AdminOverride,
}

// =============================================================================
// Transaction policy (v0.5 §8.2)
// =============================================================================

/// Transaction policy for a plan (v0.5 §8.2).
///
/// The compiler emits `recommended_transaction_policy`; the admission
/// controller (Phase 6) computes `effective_transaction_policy`.
/// Phase 5: only `recommended_transaction_policy` is used.
///
/// Phase 5 supports `AtomicShort`, `DurableStep`, and `ReadOnly`.
/// `AtomicBounded` and `AdministrativeExclusive` are Phase 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionPolicy {
    /// One transaction, short duration, all coordination held to commit.
    /// Plans with `ReadModifyWrite` or `CrossResourceInvariant` verbs.
    AtomicShort,
    /// One transaction per durable step; preferred for BPMN callbacks.
    /// Plans with `AppendFact` / `AppendTransitionSnapshot` verbs.
    /// v0.5 §8.3 canonical case.
    DurableStep,
    /// No write transaction; snapshot reads only.
    /// Plans with only `Pure` / `ReadSnapshot` verbs.
    ReadOnly,
    // AtomicBounded — Phase 6
    // AdministrativeExclusive — Phase 6
}

impl TransactionPolicy {
    /// Infer recommended policy from the most expensive effect class in the plan.
    ///
    /// Compiler calls this on the set of `effect_class` values across all
    /// `RuntimeInstruction`s to produce `recommended_transaction_policy`.
    /// Admission controller may refine (Phase 6).
    pub fn from_effect_classes(classes: impl IntoIterator<Item = EffectClass>) -> Self {
        let mut policy = TransactionPolicy::ReadOnly;
        for class in classes {
            let candidate = match class {
                EffectClass::Pure | EffectClass::ReadSnapshot => TransactionPolicy::ReadOnly,
                EffectClass::IdempotentEnsure
                | EffectClass::AppendFact
                | EffectClass::AppendTransitionSnapshot
                | EffectClass::CommutativeAccumulate
                | EffectClass::ExternalEffect => TransactionPolicy::DurableStep,
                EffectClass::ReadModifyWrite
                | EffectClass::CrossResourceInvariant
                | EffectClass::AdminOverride => TransactionPolicy::AtomicShort,
            };
            // Take the "most expensive" policy seen so far.
            policy = match (policy, candidate) {
                (TransactionPolicy::AtomicShort, _) => TransactionPolicy::AtomicShort,
                (_, TransactionPolicy::AtomicShort) => TransactionPolicy::AtomicShort,
                (TransactionPolicy::DurableStep, _) => TransactionPolicy::DurableStep,
                (_, TransactionPolicy::DurableStep) => TransactionPolicy::DurableStep,
                _ => TransactionPolicy::ReadOnly,
            };
        }
        policy
    }
}

// =============================================================================
// Authority context (v0.5 §3.2 field; minimal stub for Phase 5)
// =============================================================================

/// Authority context the plan executes under (v0.5 §3.2).
///
/// Phase 5 stub: captures the actor identity used for audit attribution.
/// Full ABAC (role, persona, client-group CCIR recheck) lives in
/// `SemOsContextEnvelope` upstream; the executor carries this minimal form
/// for audit trail purposes.
///
/// Phase 6 will expand this to include the allowed-verb fingerprint and
/// authority recheck contract.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AuthorityContext {
    /// The authenticated actor executing this plan (e.g., user UUID or system token).
    pub actor_id: Option<String>,
    /// Client group scope, if applicable.
    pub client_group_id: Option<Uuid>,
}

// =============================================================================
// Binding frame schema (v0.5 §3.2; stub — populated in T10)
// =============================================================================

/// Schema for the typed binding slots in this plan (v0.5 §3.2).
///
/// Each slot corresponds to a `BindingEdge` producer in the DAG.
/// Phase 5 stub: slot list populated but type enforcement deferred to T10.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BindingFrameSchema {
    pub slots: Vec<BindingSlot>,
}

/// A single typed binding slot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BindingSlot {
    /// Slot name — matches the `@name` / `$name` in DSL source.
    pub name: BindingSlotId,
    /// Entity type this slot holds (e.g., "cbu", "kyc_case").
    /// Populated from `VerbConfig::produces.type` where declared.
    pub entity_type: Option<String>,
}

// =============================================================================
// Instruction input
// =============================================================================

/// A typed input to a `RuntimeInstruction`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InstructionInput {
    /// A literal value known at compile time.
    Literal { value: serde_json::Value },
    /// A binding reference — the concrete value is resolved from
    /// `ExecutionFrame::binding_slots` at runtime (T10).
    BindingRef { slot: BindingSlotId },
}

// =============================================================================
// RuntimeInstruction (v0.5 §3.2)
// =============================================================================

/// A single instruction in the `ExecutablePlan` — the unit of execution.
///
/// One `RuntimeInstruction` per verb invocation. The runtime executes
/// instructions in DAG-derived order (from `ExecutablePlan::dag`).
///
/// v0.5 §3.2 `RuntimeInstruction` shape.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeInstruction {
    /// Node identity — index into `ExecutablePlan::dag` for edge lookup.
    pub node_id: NodeId,
    /// Fully-qualified verb name (e.g., "cbu.assign-role").
    pub verb_fqn: String,
    /// Typed inputs — literals or binding references.
    pub inputs: Vec<InstructionInput>,
    /// Binding slot this instruction populates, if it produces a binding.
    pub output_binding: Option<BindingSlotId>,
    /// Effect class declaration (v0.5 §5.2).
    ///
    /// `None` for plans compiled before T04-T08 verb-migration families land.
    /// When `None`, the coordination strategy table (T12) falls back to
    /// `PessimisticResourceLock` (the current default behaviour).
    pub effect_class: Option<EffectClass>,
    /// Resource dependencies for this instruction (v0.5 §6.1–6.3).
    ///
    /// Populated from `transition_args` (EntityUuid) and `produces: { resolved: false }`
    /// (NaturalKey) at plan compile time. Empty until T09 wiring is complete.
    /// Used by the coordination strategy table (T12) to derive ResourceCoordEdges.
    pub resource_dependencies: Vec<ResolvedResourceDependency>,
    /// Source statement index in the original DSL program (for error reporting).
    pub source_stmt: usize,
}

// =============================================================================
// ExecutablePlan (v0.5 §3.1, §3.2)
// =============================================================================

/// The immutable runtime contract emitted by the compiler (v0.5 §3.1).
///
/// The runtime executes only `ExecutablePlan`s. Raw DSL source is never
/// directly executed. An `ExecutablePlan` is self-contained: it carries all
/// information the runtime needs — instructions, typed DAG, effect classes,
/// resource dependencies (T09), authority, and transaction policy.
///
/// # Plan format version
///
/// `plan_format_version` is bumped when the serialised shape changes
/// incompatibly. The runtime refuses unknown versions. Phase 5 ships v1.
///
/// # Snapshot provenance (v0.5 §3.3)
///
/// `sem_os_snapshot_id` identifies the Semantic Dependency Graph snapshot
/// the plan was compiled against. This makes governance enforceable at
/// runtime: plans compiled against deprecated snapshots can be refused or
/// flagged for replay.
///
/// Phase 5: `sem_os_snapshot_id` is `None` for plans built before the
/// snapshot-id thread-through is wired into the ob-poc catalogue loader.
/// Once wired it is always `Some`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutablePlan {
    /// Identity of this compiled plan (v0.5 §10.1). Assigned at compilation.
    pub plan_id: PlanId,

    /// Plan format version. Runtime refuses unknown versions.
    pub plan_format_version: u32,

    /// The Semantic Dependency Graph snapshot this plan was compiled against.
    /// `None` until snapshot-id wiring is complete in the ob-poc loader.
    /// Load-bearing once `Some` — see v0.5 §3.3.
    pub sem_os_snapshot_id: Option<SemOsSnapshotId>,

    /// When this plan was compiled.
    pub compile_timestamp: DateTime<Utc>,

    /// Authority context (actor, client group) for audit attribution.
    pub authority_context: AuthorityContext,

    /// Transaction policy recommended by the compiler from effect-class
    /// composition (v0.5 §8.4). Admission controller may refine (Phase 6).
    pub recommended_transaction_policy: TransactionPolicy,

    /// Ordered instruction sequence (DAG-topological order).
    pub instructions: Vec<RuntimeInstruction>,

    /// Typed Populated Execution DAG — the load-bearing runtime structure.
    /// Same edges as collected from `ExecutionPlan::dag`; carried here for
    /// the runtime to use once the executor transitions to `ExecutablePlan`.
    pub dag: PopulatedExecutionDag,

    /// Binding frame schema — typed slot declarations for runtime resolution.
    pub bindings: BindingFrameSchema,
}

impl ExecutablePlan {
    /// Current plan format version.
    pub const FORMAT_VERSION: u32 = 1;

    /// Build an `ExecutablePlan` from an `ExecutionPlan` and context.
    ///
    /// - `snapshot_id`: the active SDG snapshot at compilation time, if known.
    /// - `authority`: actor and client-group identity for this plan.
    ///
    /// `effect_class` on each instruction is derived from `VerbConfig` once
    /// T04 lands; until then all instructions carry `effect_class: None` and
    /// `recommended_transaction_policy` defaults to `AtomicShort`.
    pub fn from_execution_plan(
        plan: &crate::execution_dag::PopulatedExecutionDag,
        steps: &[ExecutionStepSummary],
        snapshot_id: Option<SemOsSnapshotId>,
        authority: AuthorityContext,
    ) -> Self {
        let instructions: Vec<RuntimeInstruction> = steps
            .iter()
            .map(|s| RuntimeInstruction {
                node_id: NodeId(s.step_index),
                verb_fqn: s.verb_fqn.clone(),
                inputs: s
                    .input_names
                    .iter()
                    .map(|name| InstructionInput::Literal {
                        value: serde_json::Value::String(name.clone()),
                    })
                    .collect(),
                output_binding: s.bind_as.as_ref().map(|n| BindingSlotId::new(n)),
                effect_class: s.effect_class,
                resource_dependencies: s.resource_dependencies.clone(),
                source_stmt: s.step_index,
            })
            .collect();

        // Derive recommended policy from effect classes.
        let classes = instructions.iter().filter_map(|i| i.effect_class);
        let recommended_transaction_policy = TransactionPolicy::from_effect_classes(classes);

        // Build binding frame schema from output bindings.
        let slots: Vec<BindingSlot> = instructions
            .iter()
            .filter_map(|i| {
                i.output_binding.as_ref().map(|slot| BindingSlot {
                    name: slot.clone(),
                    entity_type: None, // populated in T10
                })
            })
            .collect();

        Self {
            plan_id: PlanId::new(),
            plan_format_version: Self::FORMAT_VERSION,
            sem_os_snapshot_id: snapshot_id,
            compile_timestamp: Utc::now(),
            authority_context: authority,
            recommended_transaction_policy,
            instructions,
            dag: plan.clone(),
            bindings: BindingFrameSchema { slots },
        }
    }
}

/// Minimal summary of an `ExecutionStep` for `ExecutablePlan` construction.
///
/// Avoids a direct dependency on `dsl_v2::execution_plan::ExecutionStep`
/// (which lives in the ob-poc binary, not in dsl-core). Callers in ob-poc
/// build this from their `ExecutionStep` values.
#[derive(Debug, Clone)]
pub struct ExecutionStepSummary {
    pub step_index: usize,
    pub verb_fqn: String,
    pub input_names: Vec<String>,
    pub bind_as: Option<String>,
    pub effect_class: Option<EffectClass>,
    pub resource_dependencies: Vec<ResolvedResourceDependency>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_policy_from_pure_only_is_read_only() {
        let policy =
            TransactionPolicy::from_effect_classes([EffectClass::Pure, EffectClass::ReadSnapshot]);
        assert_eq!(policy, TransactionPolicy::ReadOnly);
    }

    #[test]
    fn transaction_policy_from_append_is_durable_step() {
        let policy = TransactionPolicy::from_effect_classes([
            EffectClass::Pure,
            EffectClass::AppendTransitionSnapshot,
        ]);
        assert_eq!(policy, TransactionPolicy::DurableStep);
    }

    #[test]
    fn transaction_policy_from_rmw_is_atomic_short() {
        let policy = TransactionPolicy::from_effect_classes([
            EffectClass::AppendFact,
            EffectClass::ReadModifyWrite,
        ]);
        assert_eq!(policy, TransactionPolicy::AtomicShort);
    }

    #[test]
    fn transaction_policy_empty_plan_is_read_only() {
        let policy = TransactionPolicy::from_effect_classes([]);
        assert_eq!(policy, TransactionPolicy::ReadOnly);
    }

    #[test]
    fn plan_id_is_unique_per_construction() {
        let a = PlanId::new();
        let b = PlanId::new();
        assert_ne!(a, b);
    }
}
