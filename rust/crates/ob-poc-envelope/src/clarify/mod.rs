//! Clarify Module - Confirm Token System
//!
//! Provides confirm token generation and validation for the DecisionPacket-based
//! clarification UX.

mod confirm;

pub use confirm::{validate_confirm_token, ConfirmTokenError};
