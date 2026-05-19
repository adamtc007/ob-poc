//! Resource dependency taxonomy (v0.5 §6.1–6.3).
//!
//! A `ResourceDependency` describes **what** a verb touches and **when**
//! the concrete value becomes known to the runtime. The combination of
//! kind and resolution mode determines the coordination strategy:
//!
//! | Kind | Common resolution modes | Coordination |
//! |------|------------------------|-------------|
//! | `EntityUuid` | `CompileResolved`, `BindingResolved` | advisory lock or CAS |
//! | `NaturalKey` | `RuntimeCreate`, `RuntimeLookup` | DB unique constraint |
//! | `SnapshotStream` | `CompileResolved`, `BindingResolved` | CAS (expected predecessor) |
//! | `WorkflowInstance` | `CorrelationResolved`, `CompileResolved` | CAS or lock |
//! | `Scope` | `CompileResolved` | scope-level lock (rare) |
//!
//! ## Wiring from `transition_args` (v1.3 catalogue platform, T09)
//!
//! When a verb's YAML declares `transition_args:`, the compiler populates
//! a `ResourceDependency::EntityUuid` for the entity_id argument. This is
//! the primary source of `ResourceCoordEdge` entries in the Populated
//! Execution DAG.
//!
//! ## Wiring from `produces: { resolved: false }` (T09)
//!
//! When a verb produces a binding that does not yet have a UUID (e.g.,
//! `cbu.ensure` by natural key), the compiler emits a `NaturalKey`
//! dependency with `RuntimeCreate` resolution mode.

use crate::execution_dag::BindingSlotId;

// =============================================================================
// ResourceDependency kinds (v0.5 §6.1)
// =============================================================================

/// A resource dependency — **what** a verb touches (v0.5 §6.1).
///
/// Pairs with a `ResolutionMode` (§6.3) to form a `ResolvedResourceDependency`.
/// The runtime's coordination strategy is determined by `(kind, resolution_mode,
/// effect_class)` — all three together, not kind alone.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ResourceDependency {
    /// An existing entity identified by its UUID.
    ///
    /// Typical coordination: advisory lock (`PessimisticResourceLock`) for
    /// `read_modify_write` verbs; version check for `read_snapshot` / `append_*`.
    EntityUuid {
        /// Entity kind (e.g., "cbu", "deal", "kyc_case").
        entity_type: String,
        /// UUID — present when resolution mode is `CompileResolved` or
        /// `BindingResolved` (populated at or before execution time).
        uuid: Option<uuid::Uuid>,
    },

    /// An entity identified by a normalized natural key (no UUID yet).
    ///
    /// Typical coordination: DB unique constraint (`UniqueInsert`) via
    /// `idempotent_ensure` verbs. After the verb executes, the produced UUID
    /// enters a `BindingSlot` for downstream consumers.
    NaturalKey {
        /// Entity kind being created or resolved.
        entity_type: String,
        /// Normalized key hash (for coordination granularity without exposing PII).
        /// Set to `None` when the concrete key value is not yet known at plan
        /// compile time (e.g., a binding reference feeding the natural-key arg).
        normalized_key_hash: Option<u64>,
    },

    /// An event / snapshot stream identified by stream type and ID.
    ///
    /// Typical coordination: optimistic CAS (`append_transition_snapshot`).
    SnapshotStream {
        stream_type: String,
        stream_id: Option<uuid::Uuid>,
    },

    /// A workflow / process instance (BPMN or similar).
    ///
    /// Typical coordination: CAS or advisory lock. Must be fully resolved
    /// before the runtime sees the plan (`CorrelationResolved` mode = correlator
    /// resolved before submission; `CompileResolved` = known at compile time).
    WorkflowInstance {
        process_instance_id: Option<uuid::Uuid>,
    },

    /// A tenant, workspace, or catalogue scope.
    ///
    /// Typical coordination: `ExclusiveScopeLock` (rare; admin operations only).
    Scope {
        scope_type: String,
        scope_id: Option<uuid::Uuid>,
    },
}

impl ResourceDependency {
    /// Construct an `EntityUuid` dependency with a known UUID.
    pub fn entity_uuid(entity_type: impl Into<String>, uuid: uuid::Uuid) -> Self {
        Self::EntityUuid {
            entity_type: entity_type.into(),
            uuid: Some(uuid),
        }
    }

    /// Construct an `EntityUuid` dependency whose UUID is not yet known
    /// (resolved from a binding reference at execution time).
    pub fn entity_uuid_binding(entity_type: impl Into<String>) -> Self {
        Self::EntityUuid {
            entity_type: entity_type.into(),
            uuid: None,
        }
    }

    /// Construct a `NaturalKey` dependency.
    pub fn natural_key(entity_type: impl Into<String>) -> Self {
        Self::NaturalKey {
            entity_type: entity_type.into(),
            normalized_key_hash: None,
        }
    }
}

// =============================================================================
// ResolutionMode (v0.5 §6.3)
// =============================================================================

/// **When** a resource dependency's concrete value becomes known (v0.5 §6.3).
///
/// The resolution mode is what the coordination machinery dispatches on:
/// a `RuntimeCreate` natural-key must coordinate at natural-key level
/// (via unique constraint) **before** the UUID is known. A
/// `CorrelationResolved` workflow instance is already a concrete ID by
/// the time the runtime sees the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMode {
    /// Value fixed at plan-emission time (compiler resolves against entity
    /// store, snapshot catalogue, or static config).
    CompileResolved,
    /// Value produced by a prior node in the same plan (upstream binding slot).
    BindingResolved,
    /// Value looked up at invocation time against a stable identifier.
    RuntimeLookup,
    /// Value created by the verb if absent, returned if present.
    /// Coordination: `UniqueInsert` (DB unique constraint).
    RuntimeCreate,
    /// Value resolved by an external correlator before plan submission.
    /// Used for BPMN `WorkflowInstance` correlation (v0.5 §6.5).
    CorrelationResolved,
}

// =============================================================================
// ResolvedResourceDependency (kind + resolution mode together)
// =============================================================================

/// A resource dependency paired with its resolution mode.
///
/// This is the unit the coordination strategy table (T12) dispatches on:
/// `(dependency, resolution_mode, effect_class)` → coordination strategy.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedResourceDependency {
    pub dependency: ResourceDependency,
    pub resolution_mode: ResolutionMode,
    /// For `BindingResolved` dependencies: the binding slot that supplies
    /// the concrete value at execution time.
    pub binding_slot: Option<BindingSlotId>,
}

impl ResolvedResourceDependency {
    /// An entity UUID that was resolved at compile time.
    pub fn compile_resolved_entity(entity_type: impl Into<String>, uuid: uuid::Uuid) -> Self {
        Self {
            dependency: ResourceDependency::entity_uuid(entity_type, uuid),
            resolution_mode: ResolutionMode::CompileResolved,
            binding_slot: None,
        }
    }

    /// An entity UUID that will be supplied by a binding (produced upstream).
    pub fn binding_resolved_entity(entity_type: impl Into<String>, slot: BindingSlotId) -> Self {
        Self {
            dependency: ResourceDependency::entity_uuid_binding(entity_type),
            resolution_mode: ResolutionMode::BindingResolved,
            binding_slot: Some(slot),
        }
    }

    /// An entity created-or-returned by natural key (ensure pattern).
    pub fn runtime_create_natural_key(entity_type: impl Into<String>) -> Self {
        Self {
            dependency: ResourceDependency::natural_key(entity_type),
            resolution_mode: ResolutionMode::RuntimeCreate,
            binding_slot: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_uuid_compile_resolved_round_trips() {
        let uuid = uuid::Uuid::new_v4();
        let dep = ResolvedResourceDependency::compile_resolved_entity("cbu", uuid);
        assert_eq!(dep.resolution_mode, ResolutionMode::CompileResolved);
        assert!(dep.binding_slot.is_none());
        if let ResourceDependency::EntityUuid {
            entity_type,
            uuid: Some(u),
        } = &dep.dependency
        {
            assert_eq!(entity_type, "cbu");
            assert_eq!(*u, uuid);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn natural_key_runtime_create() {
        let dep = ResolvedResourceDependency::runtime_create_natural_key("kyc_case");
        assert_eq!(dep.resolution_mode, ResolutionMode::RuntimeCreate);
        if let ResourceDependency::NaturalKey { entity_type, .. } = &dep.dependency {
            assert_eq!(entity_type, "kyc_case");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn binding_resolved_entity() {
        let slot = BindingSlotId::new("my_cbu");
        let dep = ResolvedResourceDependency::binding_resolved_entity("cbu", slot.clone());
        assert_eq!(dep.resolution_mode, ResolutionMode::BindingResolved);
        assert_eq!(dep.binding_slot, Some(slot));
    }
}
