//! sem_os_policy — the SemOS governance + projection plane.
//!
//! ## Capability claim
//!
//! Everything that *decides* or *projects*. ABAC primitives, gate logic,
//! policy enforcement, ACP discovery projection, the affinity graph,
//! observatory orientation/projection, stewardship + authoring lifecycle,
//! state simulation, context policy/resolution, diagram emission,
//! grounding, security labels.
//!
//! Depends on `sem_os_core` for primitives (`types`, `ids`, `error`,
//! `principal`) and `sem_os_ontology` for the vocabulary it enforces
//! against (`attribute_def`, `policy_rule`, `verb_contract`, etc).
//!
//! ## Anti-charter
//!
//! - Does NOT define new ontology shapes — that's `sem_os_ontology`.
//! - Does NOT host engine primitives — that's `sem_os_core`.
//! - Does NOT hold a database connection. Boot-time YAML loading is
//!   permitted (same relaxed line `dsl-analysis` adopted — projection is
//!   data, not verb execution).
//!
//! ## Dependency discipline
//!
//! May depend on `sem_os_core` and `sem_os_ontology`. MUST NOT be
//! depended on by `sem_os_core` or `sem_os_ontology` — sem_os_policy
//! is the downstream sink of the three.
//!
//! ## Migration status (2026-05-14)
//!
//! Phase 1 skeleton of `docs/todo/sem-os-core-split-v1.md`. Modules land
//! in Phases 5–10 via `git mv` from `sem_os_core/src/`. Until then the
//! crate is empty.
