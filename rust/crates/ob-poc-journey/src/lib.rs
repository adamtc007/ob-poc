//! ob-poc-journey — pack-guided workflow definitions.
//!
//! ## Capability claim
//!
//! Describes the workflows the user can run as named, versioned, hashed
//! pack manifests. A pack tells the system "the user is doing X" — the
//! allowed verbs, the question policy, the templates, the definition of
//! done, the constraints, the progress signals. The pack is also the FSM
//! that tracks per-session pack lifecycle (Dormant → Active → Suspended
//! → Completed) and the handoff envelope that forwards context from a
//! completed pack to its successor.
//!
//! ## Anti-charter
//!
//! - NOT runbook execution. Packs describe intent shape; the runbook
//!   compiler in ob-poc expands packs into ordered atomic DSL.
//! - NOT REPL session state. The pack state FSM is per-session, but the
//!   wider session (workspace stack, hydrated constellation, scope) is
//!   owned by `ob-poc::repl`.
//! - NOT verb resolution. Allowed-verb lists in pack manifests are
//!   declarative; the ranker that scores against them lives in Sage.
//!
//! ## Public surface contract
//!
//! Consumers should reach for:
//! - `PackManifest`, `PackTemplate`, `TemplateStep`, `AnswerKind`,
//!   `RiskPolicy` — pack manifest schema (loaded from YAML).
//! - `load_pack_from_file`, `load_pack_from_bytes`, `PackLoadError` —
//!   YAML loader entry points.
//! - `PackState`, `PackProgress`, `SuspendReason`, `PackTransitionError`
//!   — per-session pack lifecycle FSM.
//! - `PackHandoff` — context-forwarding DTO between sequential packs.
//!
//! ## Dependency discipline
//!
//! Must depend only on `ob-poc-types` and primitives (`chrono`, `serde`,
//! `serde_yaml`, `sha2`/`hex` for content hashing, `uuid`, `thiserror`).
//! Must NOT depend on `dsl-core`, `dsl-runtime`, `sem_os_*`,
//! `ob-poc-boundary`, `ob-poc-sage`, or any execution-tier surface.
//!
//! ## Migration status (2026-05-13)
//!
//! Phase 3C of the capability-crate restructure
//! (`docs/todo/capability-crate-restructure-v1.md`) relocated the three
//! journey modules from `ob-poc-boundary::journey::*` to this crate.
//! The later helpers (pack_manager, router, playback, template) live in
//! `ob-poc/src/journey/` today and stay there unless Phase 3 successor
//! work decouples their REPL deps.
//!
//! Note on the boundary edge: per plan §6 decision 2, `ob-poc-boundary`
//! does NOT depend on this crate at runtime. The ACP discovery projection
//! owns its own `PackProjection` DTO; the projection function
//! (`fn from(&PackManifest) -> PackProjection`) lives in `ob-poc` (the
//! application layer). Boundary may depend on this crate as a
//! `dev-dependency` only — used by `#[cfg(test)]` fixtures that exercise
//! the projection pipeline against real on-disk packs.
#![deny(unreachable_pub)]

pub mod handoff;
pub mod pack;
pub mod pack_state;
