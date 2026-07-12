//! Journey Module — Pack-Guided REPL v2
//!
//! Journey Packs are the product-level interface between the user and the platform.
//! They sit above atomic DSL verbs and define structured, versioned workflows:
//!
//! - **Pack Manifest** (`pack.rs`): Versioned YAML manifests with question policy,
//!   allowed verbs, templates, and definition-of-done.
//! - **Pack Router** (`router.rs`): Deterministic pack selection from user input
//!   (force-select > substring > semantic).
//! - **Pack Handoff** (`handoff.rs`): Context forwarding between packs.
//!
//! `template.rs` (template instantiation into runbook entries) and
//! `playback.rs` (pack-level summary/chapter view) stay in `ob-poc`
//! (T11.1b, 2026-07-12) — both are deeply coupled to `repl::Runbook`/
//! `repl::sentence_gen::SentenceGenerator`, the same `repl::session_v2`
//! inversion blocker `ob-poc::acp_runtime_context` already documents.

// Phase 3C of capability-crate restructure (2026-05-13): the three
// journey leaves (pack manifest types/loader, lifecycle FSM, handoff
// DTO) live in `ob-poc-journey`. The compat re-exports below preserve
// the `ob_poc::journey::{handoff,pack,pack_state}` paths used across
// the application crate.
pub use ob_poc_journey::handoff;
pub use ob_poc_journey::pack;
pub mod pack_manager;
pub use ob_poc_journey::pack_state;
// Phase 3D of capability-crate restructure (2026-05-13): registration
// helpers for the boundary-side pack provider hooks. `ob-poc-web::main`
// (and other binaries that exercise the projection pipeline) must call
// `journey::providers::register_pack_providers()` once during startup.
pub mod providers;
pub mod router;
