//! UI Panels
//!
//! Each panel is a function that takes &mut Ui and &mut AppState.
//! Panels render UI and may trigger actions via AppState methods.
//! They do NOT own state - all state is in AppState.

mod ast;
mod cbu_search;
mod chat;
mod container_browse;
mod context;
mod dsl_editor;
mod entity_detail;
mod repl;
mod results;
mod taxonomy;
mod toolbar;

pub use ast::ast_panel;
pub use cbu_search::{cbu_search_modal, CbuSearchAction, CbuSearchData};
pub use chat::chat_panel;
pub use container_browse::{
    container_browse_panel, ContainerBrowseAction, ContainerBrowseData, ContainerBrowseState,
};
pub use context::{context_panel, ContextPanelAction};
pub use dsl_editor::{dsl_editor_panel, DslEditorAction};
pub use entity_detail::entity_detail_panel;
pub use repl::repl_panel;
pub use results::results_panel;
pub use taxonomy::{taxonomy_panel, TaxonomyPanelAction};
pub use toolbar::{toolbar, ToolbarAction, ToolbarData};
