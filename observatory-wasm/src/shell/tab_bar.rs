//! Tab bar — switches between Observe and Mission Control.

use egui::Ui;

use crate::actions::{ObservatoryAction, Tab};
use crate::state::ObservatoryState;

/// Render the tab bar. Returns SwitchTab action on click.
pub fn ui(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        if ui
            .selectable_label(state.active_tab == Tab::Observe, "Observe")
            .clicked()
            && state.active_tab != Tab::Observe
        {
            action = Some(ObservatoryAction::SwitchTab {
                tab: Tab::Observe,
            });
        }

        if ui
            .selectable_label(state.active_tab == Tab::MissionControl, "Mission Control")
            .clicked()
            && state.active_tab != Tab::MissionControl
        {
            action = Some(ObservatoryAction::SwitchTab {
                tab: Tab::MissionControl,
            });
        }

        // Refresh button
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("↻ Refresh").clicked() {
                action = Some(ObservatoryAction::RefreshData);
            }
        });
    });

    action
}
