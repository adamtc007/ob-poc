//! LocationHeader — always-visible orientation bar.
//!
//! Renders: Mode • Level • Focus • Lens • Status
//! Returns Option<ObservatoryAction> — never mutates state directly.

use egui::Ui;

use crate::actions::ObservatoryAction;
use crate::state::ObservatoryState;

/// Render the Location Header. Returns an action if the user interacts.
pub fn ui(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        if let Some(ref orientation) = state.fetch.orientation.as_ready() {
            // Extract fields from the JSON value
            let mode = orientation
                .get("session_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("governed");
            let level = orientation
                .get("view_level")
                .and_then(|v| v.as_str())
                .unwrap_or("universe");
            let label = orientation
                .pointer("/focus_identity/business_label")
                .and_then(|v| v.as_str())
                .unwrap_or("—");
            let lens_mode = orientation
                .pointer("/lens/overlay/mode")
                .and_then(|v| v.as_str())
                .unwrap_or("active_only");
            let action_count = orientation
                .get("available_actions")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);

            // Mode badge with color
            let mode_color = match mode {
                "research" => egui::Color32::from_rgb(59, 130, 246),
                "maintenance" => egui::Color32::from_rgb(245, 158, 11),
                _ => egui::Color32::from_rgb(34, 197, 94),
            };
            ui.colored_label(mode_color, capitalize(mode));

            ui.label("•");
            ui.label(capitalize(level));
            ui.label("•");
            ui.strong(label);
            ui.label("•");
            ui.label(if lens_mode == "draft_overlay" {
                "Draft Overlay"
            } else {
                "Active Only"
            });
            ui.label("•");
            ui.label(format!("{action_count} actions"));
        } else if state.fetch.orientation.is_pending() {
            ui.spinner();
            ui.label("Loading orientation...");
        } else {
            ui.label("No session");
        }
    });

    None
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().chain(c).collect(),
    }
}
