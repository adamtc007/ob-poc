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
use ob_poc_graph::{render_trading_matrix_browser, TradingMatrixAction, TradingMatrixNodeIdExt};

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
