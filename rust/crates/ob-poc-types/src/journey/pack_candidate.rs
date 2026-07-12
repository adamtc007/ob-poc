//! `PackCandidate` — hoisted from `ob-poc`'s `repl::types_v2` (T11.1b, 2026-07-12).
//!
//! Pure data shape, no IO, no crate-internal deps — same "cross-capability
//! DTO" rationale as `pack_types.rs` (see this module's parent doc). Needed
//! by `ob-poc-agent`'s `journey::router` (pack routing candidates) and by
//! `ob-poc`'s own `repl::types_v2` (REPL session state), which re-exports it
//! from here rather than defining it twice.

use serde::{Deserialize, Serialize};

/// A candidate pack for journey selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackCandidate {
    pub pack_id: String,
    pub pack_name: String,
    pub description: String,
    pub score: f32,
}
