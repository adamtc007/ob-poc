//! Top Toolbar Panel
//!
//! Contains CBU selector, view mode selector, layout selector, and status indicators.
//!
//! Note: CBU selection and view mode changes trigger API calls.
//! These are handled via return values, not callbacks.

use crate::state::LayoutMode;
use egui::{Color32, ComboBox, RichText, Ui};
use ob_poc_graph::ViewMode;
use uuid::Uuid;

/// Data needed to render the toolbar (extracted before render)
pub struct ToolbarData {
    /// Current CBU name (if any)
    pub current_cbu_name: Option<String>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Current layout mode
    pub layout: LayoutMode,
    /// Last error (if any)
    pub last_error: Option<String>,
    /// Whether any loading is in progress
    pub is_loading: bool,
}

/// Actions that can be triggered from the toolbar
#[derive(Default)]
pub struct ToolbarAction {
    /// User selected a different CBU (id, display_name)
    pub select_cbu: Option<(Uuid, String)>,
    /// User clicked to open CBU search modal
    pub open_cbu_search: bool,
    /// User changed view mode
    pub change_view_mode: Option<ViewMode>,
    /// User changed layout mode
    pub change_layout: Option<LayoutMode>,
    /// User dismissed error
    pub dismiss_error: bool,
}

pub fn toolbar(ui: &mut Ui, data: &ToolbarData) -> ToolbarAction {
    let mut action = ToolbarAction::default();

    ui.horizontal(|ui| {
        ui.set_height(32.0);

        // CBU Selector - now a button that opens search modal
        ui.label("CBU:");
        let button_text = data.current_cbu_name.as_deref().unwrap_or("Search...");
        if ui.button(button_text).clicked() {
            action.open_cbu_search = true;
        }

        ui.separator();

        // View Mode Selector
        ui.label("View:");
        ComboBox::from_id_salt("view_mode")
            .selected_text(view_mode_name(data.view_mode))
            .show_ui(ui, |ui| {
                for mode in &[
                    ViewMode::KycUbo,
                    ViewMode::ServiceDelivery,
                    ViewMode::ProductsOnly,
                ] {
                    if ui
                        .selectable_label(data.view_mode == *mode, view_mode_name(*mode))
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
            .selectable_label(data.layout == LayoutMode::FourPanel, "4-Panel")
            .clicked()
        {
            action.change_layout = Some(LayoutMode::FourPanel);
        }
        if ui
            .selectable_label(data.layout == LayoutMode::EditorFocus, "Editor")
            .clicked()
        {
            action.change_layout = Some(LayoutMode::EditorFocus);
        }
        if ui
            .selectable_label(data.layout == LayoutMode::GraphFocus, "Graph")
            .clicked()
        {
            action.change_layout = Some(LayoutMode::GraphFocus);
        }

        // Spacer - push remaining items to the right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Error indicator
            if let Some(ref error) = data.last_error {
                ui.label(RichText::new(error).color(Color32::RED).small());
                if ui.button(RichText::new("X").color(Color32::RED)).clicked() {
                    action.dismiss_error = true;
                }
            }

            // Loading indicator
            if data.is_loading {
                ui.spinner();
            }
        });
    });

    action
}

fn view_mode_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::KycUbo => "KYC/UBO",
        ViewMode::ServiceDelivery => "Services",
        ViewMode::ProductsOnly => "Products",
    }
}
