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

// Phase 3C of capability-crate restructure (2026-05-13): the three
// journey leaves (pack manifest types/loader, lifecycle FSM, handoff
// DTO) live in `ob-poc-journey`. The compat re-exports below preserve
// the `ob_poc::journey::{handoff,pack,pack_state}` paths used across
// the application crate.
pub use ob_poc_journey::handoff;
pub use ob_poc_journey::pack;
pub mod pack_manager;
pub use ob_poc_journey::pack_state;
pub mod playback;
pub mod router;
pub mod template;
