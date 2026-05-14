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
//! Phase 1 skeleton of `docs/todo/sem-os-core-split-v1.md`. Modules
//! land in Phases 2–4 via `git mv` from `sem_os_core/src/`. Until then
//! the crate is empty.
