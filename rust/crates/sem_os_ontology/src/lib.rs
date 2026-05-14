//! sem_os_ontology — the SemOS `*_def` vocabulary.
//!
//! ## Capability claim
//!
//! Definition bodies for everything SemOS catalogues: attributes, entity
//! types, document types, relationship types, state graphs, taxonomies,
//! verbs, views, requirement profiles, evidence strategies, observations,
//! universe / constellation maps, derivations, evidence DTOs, and the
//! snapshot-body wrapper that registry storage round-trips through.
//!
//! Pure data-with-validation. No ABAC, no projection, no enforcement,
//! no registry mutation.
//!
//! ## Anti-charter
//!
//! - Does NOT decide what may happen — that's `sem_os_policy::{abac,
//!   enforce, gates, ...}`.
//! - Does NOT project or observe — that's `sem_os_policy::{acp_projection,
//!   affinity, observatory, diagram, ...}`.
//! - Does NOT mutate the registry — that's the SemOS authoring / stewardship
//!   pipeline in `sem_os_policy::{authoring, stewardship}`.
//! - Does NOT depend on `sem_os_policy`. Ever.
//!
//! ## Dependency discipline
//!
//! May depend on `sem_os_core` (the engine: `types`, `ids`, `error`,
//! `principal`, `ports`, `proto`, `seeds`, `service`, `execution`).
//! MUST NOT depend on `sem_os_policy`.
//!
//! ## Migration status (2026-05-14)
//!
//! Phase 2 of `docs/todo/sem-os-core-split-v1.md`. Modules land in
//! Phases 2–4 via `git mv` from `sem_os_core/src/`.
//!
//! - Phase 2 (current): 9 pure-type leaves (policy_rule, observation_def,
//!   verb_contract, view_def, taxonomy_def, universe_def,
//!   requirement_profile_def, relationship_type_def, state_machine_def).
//!   Every module has zero crate-internal imports — serde-only.
//!
//! Compat re-exported from `sem_os_core` until Phase 12 cleanup.

pub mod observation_def;
pub mod policy_rule;
pub mod relationship_type_def;
pub mod requirement_profile_def;
pub mod state_machine_def;
pub mod taxonomy_def;
pub mod universe_def;
pub mod verb_contract;
pub mod view_def;
