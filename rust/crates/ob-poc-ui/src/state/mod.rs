//! Application State Module
//!
//! Manages session context, chat messages, and pending states for the UI.
//!
//! ## Architecture Pattern
//!
//! This module follows the egui architecture pattern documented in
//! `EGUI_ARCHITECTURE_PATTERN.MD`:
//!
//! - **DomainState**: Business data (CBUs, sessions, DSL, chat messages)
//! - **UiState**: Interaction state (modals, view settings, forms)
//! - **AppState**: Combined wrapper for convenience
//! - **AppEvent**: User intent (what the user did)
//! - **AppCommand**: Domain operations (IO, background work)
//! - **TaskStatus**: Background task lifecycle tracking
//! - **Modal**: Mutually exclusive modal states (no flag soup)
//!
//! Key patterns:
//! - UI emits AppEvents, handle_event mutates state and emits AppCommands
//! - Only execute_commands performs IO
//! - TaskStatus tracks all async operations explicitly
//!
//! See the pattern document for details on why this structure prevents
//! common egui pitfalls like flag soup, random local state, and UI glitches.

pub mod app_state;
pub mod session;
pub mod types;

pub use app_state::*;
pub use session::*;
pub use types::*;
