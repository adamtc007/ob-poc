//! Journey Module — Pack-Guided REPL v2
//!
//! Journey Packs are the product-level interface between the user and the platform.
//! They sit above atomic DSL verbs and define structured, versioned workflows:
//!
//! - **Pack Manifest** (`pack.rs`): Versioned YAML manifests with question policy,
//!   allowed verbs, templates, and definition-of-done.
//! - **Template Instantiation** (`template.rs`): Expand pack template skeletons into
//!   runbook entries with slot provenance tracking.
//! - **Pack Router** (`router.rs`): Deterministic pack selection from user input
//!   (force-select > substring > semantic).
//! - **Pack Playback** (`playback.rs`): Pack-level summary and chapter view generation.
//! - **Pack Handoff** (`handoff.rs`): Context forwarding between packs.

// Phase 3 slice 2u (2026-05-13): handoff DTO relocated to ob-poc-boundary.
pub use ob_poc_boundary::journey::handoff;
// Phase 3 slice 2d.2 (2026-05-12): pack manifest types relocated to ob-poc-boundary.
pub use ob_poc_boundary::journey::pack;
pub mod pack_manager;
// Phase 3 slice 2u (2026-05-13): pack lifecycle FSM relocated to ob-poc-boundary.
pub use ob_poc_boundary::journey::pack_state;
pub mod playback;
pub mod router;
pub mod template;
