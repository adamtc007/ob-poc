//! Trading Matrix Browser Panel
//!
//! Displays the hierarchical custody configuration for a CBU:
//! - Instrument Classes → Markets/Counterparties → Universe Entries → Resources
//!
//! Following EGUI rules:
//! - No local state mirroring server data (matrix comes from API)
//! - Actions return values, no callbacks
//! - TradingMatrixState is UI-only (expand/collapse, selection)

use crate::state::AppState;
use egui::Ui;
use ob_poc_graph::{
    render_node_detail_panel, render_trading_matrix_browser, TradingMatrixAction,
    TradingMatrixNodeIdExt,
};

/// Action returned from trading matrix panel interactions
#[derive(Debug, Clone)]
pub enum TradingMatrixPanelAction {
    /// No action
    None,
    /// Toggle expand/collapse of a node
    ToggleExpand { node_key: String },
    /// Select a node (show in detail panel)
    SelectNode { node_key: String },
    /// Clear selection
    ClearSelection,
    /// Navigate to entity in main graph
    NavigateToEntity { entity_id: String },
    /// Open SSI detail view
    OpenSsiDetail { ssi_id: String },
    /// Open ISDA detail view
    OpenIsdaDetail { isda_id: String },
    /// Expand all nodes
    ExpandAll,
    /// Collapse all nodes
    CollapseAll,
    /// Request to load children (lazy loading)
    LoadChildren { node_key: String },
}

impl From<TradingMatrixAction> for TradingMatrixPanelAction {
    fn from(action: TradingMatrixAction) -> Self {
        match action {
            TradingMatrixAction::None => TradingMatrixPanelAction::None,
            TradingMatrixAction::ToggleExpand { node_id } => {
                TradingMatrixPanelAction::ToggleExpand {
                    node_key: node_id.as_key(),
                }
            }
            TradingMatrixAction::SelectNode { node_id } => TradingMatrixPanelAction::SelectNode {
                node_key: node_id.as_key(),
            },
            TradingMatrixAction::ClearSelection => TradingMatrixPanelAction::ClearSelection,
            TradingMatrixAction::NavigateToEntity { entity_id } => {
                TradingMatrixPanelAction::NavigateToEntity { entity_id }
            }
            TradingMatrixAction::OpenSsiDetail { ssi_id } => {
                TradingMatrixPanelAction::OpenSsiDetail { ssi_id }
            }
            TradingMatrixAction::OpenIsdaDetail { isda_id } => {
                TradingMatrixPanelAction::OpenIsdaDetail { isda_id }
            }
            TradingMatrixAction::ExpandAll => TradingMatrixPanelAction::ExpandAll,
            TradingMatrixAction::CollapseAll => TradingMatrixPanelAction::CollapseAll,
            TradingMatrixAction::LoadChildren { node_id } => {
                TradingMatrixPanelAction::LoadChildren {
                    node_key: node_id.as_key(),
                }
            }
            TradingMatrixAction::ExpandToDepth { .. } => TradingMatrixPanelAction::None,
        }
    }
}

/// Render the trading matrix browser panel
/// Returns an action if the user interacted with the browser
pub fn trading_matrix_panel(
    ui: &mut Ui,
    state: &AppState,
    max_height: f32,
) -> TradingMatrixPanelAction {
    // Check if we have trading matrix data
    let Some(ref matrix) = state.trading_matrix else {
        // Show loading or empty state
        ui.centered_and_justified(|ui| {
            if state
                .async_state
                .lock()
                .map(|s| s.loading_trading_matrix)
                .unwrap_or(false)
            {
                ui.spinner();
                ui.label("Loading trading matrix...");
            } else {
                ui.label("No trading matrix data");
            }
        });
        return TradingMatrixPanelAction::None;
    };

    // Render the trading matrix browser from ob-poc-graph
    let action = render_trading_matrix_browser(ui, matrix, &state.trading_matrix_state, max_height);

    action.into()
}

/// Render the trading matrix detail panel for the selected node
/// Returns an action if the user interacted with the detail panel
pub fn trading_matrix_detail_panel(ui: &mut Ui, state: &AppState) -> TradingMatrixPanelAction {
    // Get selected node from state
    let Some(ref matrix) = state.trading_matrix else {
        return TradingMatrixPanelAction::None;
    };

    let Some(selected_key) = state.trading_matrix_state.selected() else {
        ui.centered_and_justified(|ui| {
            ui.label("Select a node to view details");
        });
        return TradingMatrixPanelAction::None;
    };

    // Find the selected node in the matrix
    // We need to search by key since we store the key in state
    fn find_node_by_key<'a>(
        node: &'a ob_poc_graph::TradingMatrixNode,
        key: &str,
    ) -> Option<&'a ob_poc_graph::TradingMatrixNode> {
        if node.id.as_key() == key {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_node_by_key(child, key) {
                return Some(found);
            }
        }
        None
    }

    // Search across all root children
    let node = matrix
        .children()
        .iter()
        .find_map(|child| find_node_by_key(child, selected_key));

    let Some(node) = node else {
        ui.label("Selected node not found");
        return TradingMatrixPanelAction::None;
    };

    // Render the detail panel
    let action = render_node_detail_panel(ui, node);
    action.into()
}
