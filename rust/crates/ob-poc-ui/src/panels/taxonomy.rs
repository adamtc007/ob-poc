//! Taxonomy Browser Panel
//!
//! Displays the entity type hierarchy with counts from the current graph.
//! Allows filtering the graph by entity type.
//!
//! Supports fractal navigation via TaxonomyStack:
//! - Zoom in: Double-click a type to navigate into its children
//! - Zoom out: Click the back button or breadcrumb
//! - Breadcrumbs: Show navigation path, clickable to jump to any level
//!
//! Following EGUI rules:
//! - No local state mirroring server data (ontology counts come from graph)
//! - Actions return values, no callbacks
//! - TaxonomyState is UI-only (expand/collapse, selection)
//! - Breadcrumbs come from server via taxonomy_breadcrumbs

use crate::state::AppState;
use egui::{Color32, RichText, Ui};
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
    /// User wants to zoom into a type (fractal navigation)
    ZoomIn { type_code: String },
    /// User wants to zoom out one level
    ZoomOut,
    /// User clicked a breadcrumb to jump to that level
    BackTo { level_index: usize },
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
                // Map filter-to-type to zoom-in for fractal navigation
                TaxonomyPanelAction::ZoomIn { type_code }
            }
            TypeBrowserAction::ExpandAll => TaxonomyPanelAction::ExpandAll,
            TypeBrowserAction::CollapseAll => TaxonomyPanelAction::CollapseAll,
        }
    }
}

/// Render breadcrumb navigation bar
/// Returns Some(action) if user clicked a breadcrumb
fn render_breadcrumbs(
    ui: &mut Ui,
    breadcrumbs: &[(String, String)],
) -> Option<TaxonomyPanelAction> {
    if breadcrumbs.is_empty() {
        return None;
    }

    let mut action = None;

    ui.horizontal(|ui| {
        // Back button (only show if we're not at root)
        if breadcrumbs.len() > 1 {
            if ui
                .small_button("←")
                .on_hover_text("Go back one level")
                .clicked()
            {
                action = Some(TaxonomyPanelAction::ZoomOut);
            }
            ui.separator();
        }

        // Breadcrumb trail
        for (i, (label, _type_code)) in breadcrumbs.iter().enumerate() {
            let is_last = i == breadcrumbs.len() - 1;

            if is_last {
                // Current level - not clickable, bold
                ui.label(RichText::new(label).strong());
            } else {
                // Clickable breadcrumb
                if ui.link(label).on_hover_text("Jump to this level").clicked() {
                    action = Some(TaxonomyPanelAction::BackTo { level_index: i });
                }
                ui.label(RichText::new(" › ").color(Color32::GRAY));
            }
        }
    });

    action
}

/// Render zoom controls
fn render_zoom_controls(ui: &mut Ui, can_zoom_out: bool) -> Option<TaxonomyPanelAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        if ui.small_button("⊖").on_hover_text("Zoom out").clicked() && can_zoom_out {
            action = Some(TaxonomyPanelAction::ZoomOut);
        }
        ui.label("|");
        if ui
            .small_button("⊕")
            .on_hover_text("Zoom in (double-click a type)")
            .clicked()
        {
            // Zoom in is triggered by double-click on a type, this is just informational
        }
    });

    action
}

/// Render the taxonomy browser panel with navigation controls
/// Returns an action if the user interacted with the browser
pub fn taxonomy_panel(ui: &mut Ui, state: &AppState, max_height: f32) -> TaxonomyPanelAction {
    // Header with title and controls
    ui.horizontal(|ui| {
        ui.heading("Types");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Show expand/collapse all buttons
            if ui.small_button("▼").on_hover_text("Expand all").clicked() {
                return TaxonomyPanelAction::ExpandAll;
            }
            if ui.small_button("▲").on_hover_text("Collapse all").clicked() {
                return TaxonomyPanelAction::CollapseAll;
            }
            TaxonomyPanelAction::None
        });
    });

    ui.separator();

    // Breadcrumb navigation (if we have any)
    if !state.taxonomy_breadcrumbs.is_empty() {
        if let Some(action) = render_breadcrumbs(ui, &state.taxonomy_breadcrumbs) {
            return action;
        }
        ui.separator();
    }

    // Zoom controls
    let can_zoom_out = state.taxonomy_breadcrumbs.len() > 1;
    if let Some(action) = render_zoom_controls(ui, can_zoom_out) {
        return action;
    }

    ui.separator();

    // Calculate remaining height for the type browser
    let used_height = ui.min_rect().height();
    let remaining_height = (max_height - used_height - 20.0).max(100.0);

    // Render the type browser from ob-poc-graph
    let action = render_type_browser(
        ui,
        &state.entity_ontology,
        &state.taxonomy_state,
        remaining_height,
    );

    action.into()
}
