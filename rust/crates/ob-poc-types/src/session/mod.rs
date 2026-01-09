//! Unified session context system.
//!
//! This module provides the single source of truth for session state between
//! REPL and Viewport. It includes:
//!
//! - `ScopePath` - hierarchical navigation path (e.g., "allianz.trading.germany")
//! - `SessionContext` - complete session state container
//! - `SessionManager` - thread-safe manager with watch channel broadcasts

mod context;
mod manager;
mod scope;

pub use context::{FilterSet, SessionContext, ViewMode};
pub use manager::{SessionManager, SessionSnapshot};
pub use scope::ScopePath;
