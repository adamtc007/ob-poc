//! Service Taxonomy Browser Panel
//!
//! Displays the hierarchical Product → Service → Resource configuration for a CBU:
//! - Products → Services → Intents → SRDEFs → Attributes
//!
//! Following EGUI rules:
//! - No local state mirroring server data (taxonomy comes from API)
//! - Actions return values, no callbacks
//! - ServiceTaxonomyState is UI-only (expand/collapse, selection, filters)

use crate::state::AppState;
use egui::Ui;
use ob_poc_graph::{
    render_service_detail_panel, render_service_taxonomy, ServiceTaxonomyAction,
    ServiceTaxonomyNodeId,
};

/// Action returned from service taxonomy panel interactions
#[derive(Debug, Clone)]
pub enum ServiceTaxonomyPanelAction {
    /// No action
    None,
    /// Toggle expand/collapse of a node
    ToggleExpand { node_id: ServiceTaxonomyNodeId },
    /// Select a node (show in detail panel)
    SelectNode { node_id: ServiceTaxonomyNodeId },
    /// Drill into resource details
    DrillIntoResource { srdef_id: String },
    /// Show blocking reason detail
    ShowBlockingReason {
        node_id: ServiceTaxonomyNodeId,
        reason: String,
    },
    /// Toggle blocked-only filter
    ToggleBlockedFilter,
    /// Toggle attribute detail visibility
    ToggleAttributeDetail,
    /// Expand all nodes
    ExpandAll,
    /// Collapse all nodes
    CollapseAll,
    /// Refresh data from server
    Refresh,
}

impl From<ServiceTaxonomyAction> for ServiceTaxonomyPanelAction {
    fn from(action: ServiceTaxonomyAction) -> Self {
        match action {
            ServiceTaxonomyAction::None => ServiceTaxonomyPanelAction::None,
            ServiceTaxonomyAction::ToggleExpand { node_id } => {
                ServiceTaxonomyPanelAction::ToggleExpand { node_id }
            }
            ServiceTaxonomyAction::SelectNode { node_id } => {
                ServiceTaxonomyPanelAction::SelectNode { node_id }
            }
            ServiceTaxonomyAction::DrillIntoResource { srdef_id } => {
                ServiceTaxonomyPanelAction::DrillIntoResource { srdef_id }
            }
            ServiceTaxonomyAction::ShowBlockingReason { node_id, reason } => {
                ServiceTaxonomyPanelAction::ShowBlockingReason { node_id, reason }
            }
            ServiceTaxonomyAction::ToggleBlockedFilter => {
                ServiceTaxonomyPanelAction::ToggleBlockedFilter
            }
            ServiceTaxonomyAction::ToggleAttributeDetail => {
                ServiceTaxonomyPanelAction::ToggleAttributeDetail
            }
            ServiceTaxonomyAction::ExpandAll => ServiceTaxonomyPanelAction::ExpandAll,
            ServiceTaxonomyAction::CollapseAll => ServiceTaxonomyPanelAction::CollapseAll,
            ServiceTaxonomyAction::Refresh => ServiceTaxonomyPanelAction::Refresh,
        }
    }
}

/// Render the service taxonomy browser panel
/// Returns an action if the user interacted with the browser
pub fn service_taxonomy_panel(
    ui: &mut Ui,
    state: &AppState,
    max_height: f32,
) -> ServiceTaxonomyPanelAction {
    // Check if we have service taxonomy data
    let Some(ref taxonomy) = state.service_taxonomy else {
        // Show loading or empty state
        ui.centered_and_justified(|ui| {
            if state
                .async_state
                .lock()
                .map(|s| s.loading_service_taxonomy)
                .unwrap_or(false)
            {
                ui.spinner();
                ui.label("Loading service taxonomy...");
            } else {
                ui.label("No service taxonomy data");
                ui.label("Select a CBU to view service resources");
            }
        });
        return ServiceTaxonomyPanelAction::None;
    };

    // Render the service taxonomy browser from ob-poc-graph
    // Note: state.service_taxonomy_state is mutable, but we need to work with the borrow checker
    // We'll handle state mutations in the action handler
    let mut state_clone = state.service_taxonomy_state.clone();
    let action = render_service_taxonomy(ui, taxonomy, &mut state_clone, max_height);

    // If there's a selected node, show detail panel
    if let Some(selected_key) = state.service_taxonomy_state.selected() {
        // Find the selected node in the taxonomy
        if let Some(node) = find_node_by_key(taxonomy, selected_key) {
            ui.separator();
            let detail_action = render_service_detail_panel(ui, node);
            if !matches!(detail_action, ServiceTaxonomyAction::None) {
                return detail_action.into();
            }
        }
    }

    action.into()
}

/// Find a node in the taxonomy by its key string
fn find_node_by_key<'a>(
    taxonomy: &'a ob_poc_graph::ServiceTaxonomy,
    key: &str,
) -> Option<&'a ob_poc_graph::ServiceTaxonomyNode> {
    fn find_recursive<'a>(
        node: &'a ob_poc_graph::ServiceTaxonomyNode,
        key: &str,
    ) -> Option<&'a ob_poc_graph::ServiceTaxonomyNode> {
        if node.id.as_key() == key {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_recursive(child, key) {
                return Some(found);
            }
        }
        None
    }

    // Check root
    if taxonomy.root.id.as_key() == key {
        return Some(&taxonomy.root);
    }

    // Search in products (root children)
    for product in taxonomy.products() {
        if let Some(found) = find_recursive(product, key) {
            return Some(found);
        }
    }

    None
}
