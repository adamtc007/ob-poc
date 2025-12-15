//! Top Toolbar Panel
//!
//! Contains CBU selector, view mode selector, layout selector, and status indicators.
//!
//! Note: CBU selection and view mode changes trigger API calls.
//! These are handled via return values, not callbacks.

use crate::state::{AppState, LayoutMode};
use egui::{Color32, ComboBox, RichText, Ui};
use ob_poc_graph::ViewMode;
use uuid::Uuid;

/// Actions that can be triggered from the toolbar
#[derive(Default)]
pub struct ToolbarAction {
    /// User selected a different CBU (id, display_name)
    pub select_cbu: Option<(Uuid, String)>,
    /// User changed view mode
    pub change_view_mode: Option<ViewMode>,
    /// User dismissed error
    pub dismiss_error: bool,
}

pub fn toolbar(ui: &mut Ui, state: &mut AppState) -> ToolbarAction {
    let mut action = ToolbarAction::default();

    ui.horizontal(|ui| {
        ui.set_height(32.0);

        // CBU Selector
        ui.label("CBU:");
        let current_name = state
            .session
            .as_ref()
            .and_then(|s| s.active_cbu.as_ref())
            .map(|c| c.name.as_str())
            .unwrap_or("Select...");

        ComboBox::from_id_salt("cbu_selector")
            .selected_text(current_name)
            .show_ui(ui, |ui| {
                for cbu in &state.cbu_list {
                    let is_selected = state
                        .session
                        .as_ref()
                        .and_then(|s| s.active_cbu.as_ref())
                        .map(|c| c.id == cbu.cbu_id)
                        .unwrap_or(false);

                    if ui.selectable_label(is_selected, &cbu.name).clicked() {
                        if let Ok(uuid) = Uuid::parse_str(&cbu.cbu_id) {
                            action.select_cbu = Some((uuid, cbu.name.clone()));
                        }
                    }
                }
            });

        ui.separator();

        // View Mode Selector
        ui.label("View:");
        ComboBox::from_id_salt("view_mode")
            .selected_text(view_mode_name(state.view_mode))
            .show_ui(ui, |ui| {
                for mode in &[
                    ViewMode::KycUbo,
                    ViewMode::ServiceDelivery,
                    ViewMode::Custody,
                    ViewMode::ProductsOnly,
                ] {
                    if ui
                        .selectable_label(state.view_mode == *mode, view_mode_name(*mode))
                        .clicked()
                    {
                        action.change_view_mode = Some(*mode);
                    }
                }
            });

        ui.separator();

        // Layout selector
        ui.label("Layout:");
        if ui
            .selectable_label(state.panels.layout == LayoutMode::FourPanel, "4-Panel")
            .clicked()
        {
            state.panels.layout = LayoutMode::FourPanel;
        }
        if ui
            .selectable_label(state.panels.layout == LayoutMode::EditorFocus, "Editor")
            .clicked()
        {
            state.panels.layout = LayoutMode::EditorFocus;
        }
        if ui
            .selectable_label(state.panels.layout == LayoutMode::GraphFocus, "Graph")
            .clicked()
        {
            state.panels.layout = LayoutMode::GraphFocus;
        }

        // Spacer - push remaining items to the right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Error indicator
            if let Ok(async_state) = state.async_state.lock() {
                if let Some(ref error) = async_state.last_error {
                    ui.label(RichText::new(error).color(Color32::RED).small());
                    if ui.button(RichText::new("X").color(Color32::RED)).clicked() {
                        action.dismiss_error = true;
                    }
                }

                // Loading indicator
                if async_state.loading_session
                    || async_state.loading_graph
                    || async_state.loading_chat
                    || async_state.executing
                {
                    ui.spinner();
                }
            }
        });
    });

    action
}

fn view_mode_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::KycUbo => "KYC/UBO",
        ViewMode::ServiceDelivery => "Services",
        ViewMode::Custody => "Custody",
        ViewMode::ProductsOnly => "Products",
    }
}
