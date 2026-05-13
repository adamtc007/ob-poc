//! Entity type definition body — re-export of the canonical
//! definition in `sem_os_core::entity_type_def`.
//!
//! Phase 4.7 audit follow-up (2026-05-13): the local parallel
//! definition that lived here was a strict subset of
//! `sem_os_core::entity_type_def::EntityTypeDefBody` (missing 5
//! governance / visibility fields). Collapsing to a re-export
//! removes 3 entries from the schema-authority drift allowlist
//! (`EntityTypeDefBody`, `LifecycleStateDef`, `LifecycleTransition`)
//! and keeps `sem_os_core` as the single schema authority per
//! V&S §O7 / ADN §7.3.
//!
//! All existing import paths through `crate::sem_reg::entity_type_def::*`
//! and the `ob_poc::sem_reg::*` compat re-export continue to work
//! verbatim. Struct-literal callers in `src/sem_reg/onboarding/*`
//! were updated in the same commit to list the 5 sem_os_core-only
//! fields (`governance_tier`, `security_classification`, `pii`,
//! `read_by_verbs`, `written_by_verbs`) with `None` / `Vec::new()`
//! defaults.

pub use sem_os_core::entity_type_def::*;
