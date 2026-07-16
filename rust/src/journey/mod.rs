//! Journey module — slim remnant after T11.1b (2026-07-12, agent-tier
//! extraction).
//!
//! `pack_manager`/`providers`/`router` moved to `ob-poc-agent::journey` —
//! re-exported here so every existing `crate::journey::{pack_manager,
//! providers,router}` caller continues to resolve unchanged.
//!
//! `playback`/`template` stay here — both are deeply coupled to
//! `repl::Runbook`/`repl::sentence_gen::SentenceGenerator`, the same
//! `repl::session_v2` inversion blocker `acp_runtime_context.rs` already
//! documents.
//!
//! `handoff`/`pack`/`pack_state` are unaffected by this move — they were
//! already `ob-poc-journey` re-exports (Phase 3C, 2026-05-13), unchanged.

pub use ob_poc_journey::handoff;
pub use ob_poc_journey::pack;
pub use ob_poc_journey::pack_state;

pub use ob_poc_agent::journey::{pack_manager, providers, router};

pub mod playback;
pub mod template;
