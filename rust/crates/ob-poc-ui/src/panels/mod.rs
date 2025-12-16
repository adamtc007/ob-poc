//! UI Panels
//!
//! Each panel is a function that takes &mut Ui and &mut AppState.
//! Panels render UI and may trigger actions via AppState methods.
//! They do NOT own state - all state is in AppState.

mod ast;
mod chat;
mod dsl_editor;
mod entity_detail;
mod repl;
mod results;
mod toolbar;

pub use ast::ast_panel;
pub use chat::chat_panel;
pub use dsl_editor::dsl_editor_panel;
pub use entity_detail::entity_detail_panel;
pub use repl::repl_panel;
pub use results::results_panel;
pub use toolbar::{toolbar, ToolbarAction};
