//! Clarify Module - Unified Decision Point System
//!
//! This module implements the DecisionPacket-based clarification UX,
//! providing a single, deterministic, auditable mechanism for all
//! decision points in the agent pipeline.
//!
//! ## Architecture
//!
//! ```text
//! User Input → Intent Pipeline → DecisionPacket → User Reply → Execute
//!                                     │
//!                                     ├── ClarifyGroup (client selection)
//!                                     ├── ClarifyVerb (verb disambiguation)
//!                                     ├── ClarifyScope (intent tier)
//!                                     ├── Proposal (confirm execution)
//!                                     └── Refuse (cannot proceed)
//! ```
//!
//! ## Key Components
//!
//! - [`packet`]: DecisionPacket builder and validation
//! - [`render`]: Deterministic rendering templates
//! - [`parse`]: User reply parser
//! - [`confirm`]: Confirm token generation and validation

mod confirm;
mod packet;
mod parse;
mod render;

pub use confirm::{generate_confirm_token, validate_confirm_token, ConfirmTokenError};
pub use packet::{DecisionPacketBuilder, PacketBuildError};
pub use parse::{parse_user_reply, ParseError as ReplyParseError};
pub use render::{
    render_decision_packet, render_group_clarification, render_proposal, render_refuse,
    render_scope_clarification, render_verb_clarification,
};
