//! Top Toolbar Panel
//!
//! Simplified toolbar showing only essential navigation and status.
//! Dropdowns removed for cleaner interface - navigation via graph/chat.

use crate::state::LayoutMode;
use egui::{Color32, RichText, Ui};
use ob_poc_graph::ViewMode;
use ob_poc_types::galaxy::ViewLevel;
use uuid::Uuid;

/// Data needed to render the toolbar (extracted before render)
pub struct ToolbarData {
    /// Current CBU name (if any)
    pub current_cbu_name: Option<String>,
    /// Current view mode (for CBU-level views)
    pub view_mode: ViewMode,
    /// Current navigation level (Universe, Cluster, System, etc.)
    pub view_level: ViewLevel,
    /// Last error (if any)
    pub last_error: Option<String>,
    /// Whether any loading is in progress
    pub is_loading: bool,
    /// Session scope type (galaxy, book, cbu, jurisdiction, neighborhood)
    pub scope_type: Option<String>,
    /// Session scope path for display
    pub scope_path: Option<String>,
    /// Whether scope data is fully loaded
    pub scope_loaded: bool,
}

/// Actions that can be triggered from the toolbar
#[derive(Default)]
pub struct ToolbarAction {
    /// User selected a different CBU (id, display_name)
    pub select_cbu: Option<(Uuid, String)>,
    /// User clicked to open CBU search modal
    pub open_cbu_search: bool,
    /// User changed view mode (CBU-level layer)
    pub change_view_mode: Option<ViewMode>,
    /// User changed view level (navigation scope)
    pub change_view_level: Option<ViewLevel>,
    /// User changed layout mode
    pub change_layout: Option<LayoutMode>,
    /// User dismissed error
    pub dismiss_error: bool,
}

pub fn toolbar(ui: &mut Ui, data: &ToolbarData) -> ToolbarAction {
    let mut action = ToolbarAction::default();

    ui.horizontal(|ui| {
        ui.set_height(28.0);

        // Session scope breadcrumb (primary navigation context)
        if let Some(ref scope_type) = data.scope_type {
            let scope_icon = match scope_type.as_str() {
                "galaxy" => "🌌",
                "book" => "📖",
                "cbu" => "🏢",
                "jurisdiction" => "🌍",
                "neighborhood" => "🔗",
                _ => "📍",
            };

            let scope_color = if data.scope_loaded {
                Color32::from_rgb(100, 180, 255) // Blue when loaded
            } else {
                Color32::from_rgb(180, 130, 50) // Amber when loading
            };

            // Scope icon and type
            ui.label(RichText::new(scope_icon).size(16.0));
            ui.label(
                RichText::new(scope_type.to_uppercase())
                    .strong()
                    .color(scope_color),
            );

            // Scope path as breadcrumb segments
            if let Some(ref path) = data.scope_path {
                if !path.is_empty() {
                    for segment in path.split(" > ") {
                        ui.label(RichText::new(">").weak().small());
                        ui.label(RichText::new(segment).color(scope_color));
                    }
                }
            }

            // Loading indicator for scope
            if !data.scope_loaded {
                ui.spinner();
            }

            ui.label(RichText::new("|").weak().small());
        }

        // Navigation breadcrumb: Universe > Cluster > CBU (view level)
        if data.view_level != ViewLevel::Universe {
            // Show breadcrumb back to universe
            if ui.small_button("Universe").clicked() {
                action.change_view_level = Some(ViewLevel::Universe);
            }
            ui.label(RichText::new(">").weak().small());
        }

        // Current view level indicator
        let level_text = match data.view_level {
            ViewLevel::Universe => "Universe".to_string(),
            ViewLevel::Cluster => "Cluster".to_string(),
            _ => data
                .current_cbu_name
                .clone()
                .unwrap_or_else(|| "CBU".to_string()),
        };
        ui.label(RichText::new(level_text).strong());

        // View mode indicator (read-only, no dropdown)
        if matches!(
            data.view_level,
            ViewLevel::System | ViewLevel::Planet | ViewLevel::Surface | ViewLevel::Core
        ) {
            ui.label(RichText::new("|").weak().small());
            ui.label(RichText::new(view_mode_name(data.view_mode)).small());
        }

        // Spacer - push remaining items to the right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Loading indicator
            if data.is_loading {
                ui.spinner();
            }

            // Error indicator
            if let Some(ref error) = data.last_error {
                if ui
                    .small_button(RichText::new("X").color(Color32::RED))
                    .clicked()
                {
                    action.dismiss_error = true;
                }
                ui.label(RichText::new(error).color(Color32::RED).small());
            }
        });
    });

    action
}

fn view_mode_name(_mode: ViewMode) -> &'static str {
    "CBU"
}
