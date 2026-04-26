//! Entity lifecycle FSM queries (ontology-backed).
//!
//! Narrow read-only trait exposing the three lifecycle state-machine
//! checks relocated plugin ops actually use: transition validity,
//! terminal detection, and the set of valid next states. The full
//! ontology (entity taxonomy, FK inference, implicit-create config,
//! alias resolution, semantic stage map) stays in ob-poc; this trait
//! only lets ops ask "is this transition allowed for entity X?"
//! without dragging the taxonomy loader into `dsl-runtime`.
//!
//! Introduced in Phase 5a composite-blocker #5 for `kyc_case_ops`.
//! The ob-poc bridge ([`crate::services::ServiceRegistry`] impl via
//! `ObPocLifecycleCatalog`) delegates to
//! `crate::ontology::ontology()` (taxonomy-loaded singleton).
//! Consumers obtain the impl via
//! [`crate::VerbExecutionContext::service::<dyn LifecycleCatalog>`].

/// Lifecycle FSM queries for a named entity type (e.g. `"kyc_case"`,
/// `"deal"`). All three methods return the same "no such entity /
/// unknown state" behaviour as the current ob-poc helpers: transitions
/// are rejected, states are flagged non-terminal, and the next-state
/// set is empty.
pub trait LifecycleCatalog: Send + Sync {
    /// `true` iff the YAML state machine for `entity_type` permits
    /// `from → to`. Returns `false` when `entity_type` has no lifecycle
    /// configured or when either state is unknown.
    fn is_valid_transition(&self, entity_type: &str, from: &str, to: &str) -> bool;

    /// `true` iff `state` is a terminal (no outbound transitions) for
    /// the named entity type's FSM. Returns `false` when the entity
    /// type is unknown.
    fn is_terminal_state(&self, entity_type: &str, state: &str) -> bool;

    /// States reachable from `state` in one transition. Empty when the
    /// entity type is unknown, the state is terminal, or the state is
    /// unknown.
    fn valid_next_states(&self, entity_type: &str, state: &str) -> Vec<String>;
}
