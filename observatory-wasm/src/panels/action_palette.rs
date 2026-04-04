//! ActionPalette — available actions as clickable buttons.

use egui::Ui;

use crate::actions::ObservatoryAction;
use crate::state::ObservatoryState;

/// Render available actions from the orientation contract.
pub fn ui(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    let Some(ref orientation) = state.fetch.orientation.as_ready() else {
        return None;
    };

    let actions = orientation
        .get("available_actions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if actions.is_empty() {
        return None;
    }

    ui.separator();
    ui.small("AVAILABLE ACTIONS");

    let mut result = None;

    let enabled: Vec<_> = actions
        .iter()
        .filter(|a| a.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false))
        .collect();

    ui.horizontal_wrapped(|ui| {
        for action in enabled.iter().take(12) {
            let label = action
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("—");
            let fqn = action
                .get("action_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if ui.small_button(label).on_hover_text(fqn).clicked() {
                result = Some(ObservatoryAction::InvokeVerb {
                    verb_fqn: fqn.to_string(),
                });
            }
        }
    });

    result
}
