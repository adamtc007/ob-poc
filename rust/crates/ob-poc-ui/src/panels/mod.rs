//! UI Panels Module
//!
//! Contains the individual panel implementations for the 4-panel layout:
//! - Chat panel (bottom-left): Agent chat interface
//! - DSL panel (top-right): Generated DSL source
//! - AST panel (bottom-right): Interactive AST with EntityRef resolution

pub mod ast_panel;
pub mod chat_panel;
pub mod dsl_panel;

pub use ast_panel::{AstPanel, UnresolvedRefClick};
pub use chat_panel::{ChatPanel, ChatPanelAction};
pub use dsl_panel::DslPanel;
