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

pub mod handoff;
pub mod pack;
pub mod pack_manager;
pub mod pack_state;
pub mod playback;
pub mod router;
pub mod template;
