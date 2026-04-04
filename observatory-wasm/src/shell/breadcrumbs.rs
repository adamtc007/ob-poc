//! Breadcrumbs — navigation trail from OrientationContract history.
//!
//! Each crumb clickable for history navigation.
//! Returns NavigateHistory action — never mutates state.

use egui::Ui;

use crate::actions::ObservatoryAction;
use crate::state::ObservatoryState;

/// Render breadcrumbs. Returns NavigateHistory on click.
pub fn ui(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    if state.navigation_history.is_empty() {
        return None;
    }

    let mut action = None;

    ui.horizontal(|ui| {
        for (i, entry) in state.navigation_history.iter().enumerate() {
            let is_last = i == state.navigation_history.len() - 1;
            let label = entry
                .pointer("/focus_identity/business_label")
                .and_then(|v| v.as_str())
                .or_else(|| entry.get("view_level").and_then(|v| v.as_str()))
                .unwrap_or("—");

            if i > 0 {
                ui.label("›");
            }

            if is_last {
                ui.strong(label);
            } else if ui.link(label).clicked() {
                action = Some(ObservatoryAction::NavigateHistory { index: i });
            }
        }
    });

    action
}
