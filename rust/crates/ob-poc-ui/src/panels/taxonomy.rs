//! Taxonomy Browser Panel
//!
//! Displays the entity type hierarchy with counts from the current graph.
//! Allows filtering the graph by entity type.
//!
//! Following EGUI rules:
//! - No local state mirroring server data (ontology counts come from graph)
//! - Actions return values, no callbacks
//! - TaxonomyState is UI-only (expand/collapse, selection)

use crate::state::AppState;
use egui::Ui;
use ob_poc_graph::{render_type_browser, TypeBrowserAction};

/// Action returned from taxonomy panel interactions
#[derive(Debug, Clone)]
pub enum TaxonomyPanelAction {
    /// No action
    None,
    /// User toggled expand/collapse on a type node
    ToggleExpand { type_code: String },
    /// User selected a type (highlight matching entities)
    SelectType { type_code: String },
    /// User cleared the type selection
    ClearSelection,
    /// User double-clicked to filter graph to this type only
    FilterToType { type_code: String },
    /// User wants to expand all nodes
    ExpandAll,
    /// User wants to collapse all nodes
    CollapseAll,
}

impl From<TypeBrowserAction> for TaxonomyPanelAction {
    fn from(action: TypeBrowserAction) -> Self {
        match action {
            TypeBrowserAction::None => TaxonomyPanelAction::None,
            TypeBrowserAction::ToggleExpand { type_code } => {
                TaxonomyPanelAction::ToggleExpand { type_code }
            }
            TypeBrowserAction::SelectType { type_code } => {
                TaxonomyPanelAction::SelectType { type_code }
            }
            TypeBrowserAction::ClearSelection => TaxonomyPanelAction::ClearSelection,
            TypeBrowserAction::FilterToType { type_code } => {
                TaxonomyPanelAction::FilterToType { type_code }
            }
            TypeBrowserAction::ExpandAll => TaxonomyPanelAction::ExpandAll,
            TypeBrowserAction::CollapseAll => TaxonomyPanelAction::CollapseAll,
        }
    }
}

/// Render the taxonomy browser panel
/// Returns an action if the user interacted with the browser
pub fn taxonomy_panel(ui: &mut Ui, state: &AppState, max_height: f32) -> TaxonomyPanelAction {
    // Render the type browser from ob-poc-graph
    let action = render_type_browser(
        ui,
        &state.entity_ontology,
        &state.taxonomy_state,
        max_height,
    );

    action.into()
}
