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
mod disambiguation;
mod dsl_editor;
mod entity_detail;
mod inspector;
mod investor_register;
mod macro_wizard;
mod repl;
mod resolution;
mod results;
mod service_taxonomy;
mod taxonomy;
mod toolbar;
mod trading_matrix;

pub use ast::ast_panel;
pub use cbu_search::{cbu_search_modal, CbuSearchAction, CbuSearchData};
pub use chat::chat_panel;
pub use container_browse::{
    container_browse_panel, ContainerBrowseAction, ContainerBrowseData, ContainerBrowseState,
};
pub use context::{context_panel, ContextPanelAction};
pub use disambiguation::{disambiguation_modal, DisambiguationAction, DisambiguationModalData};
pub use dsl_editor::{dsl_editor_panel, DslEditorAction};
pub use entity_detail::entity_detail_panel;
pub use inspector::InspectorState;
pub use investor_register::{investor_register_panel, InvestorRegisterAction};
pub use macro_wizard::MacroWizardAction;
pub use repl::{
    repl_panel, DecisionAction, IntentTierAction, ReplAction, VerbDisambiguationAction,
};
pub use resolution::{
    resolution_modal, EntityMatchDisplay, ResolutionPanelAction, ResolutionPanelData,
};
pub use results::results_panel;
pub use service_taxonomy::{service_taxonomy_panel, ServiceTaxonomyPanelAction};
pub use taxonomy::{taxonomy_panel, TaxonomyPanelAction};
pub use toolbar::{toolbar, ToolbarAction, ToolbarData};
pub use trading_matrix::{trading_matrix_panel, TradingMatrixPanelAction};
