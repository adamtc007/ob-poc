//! Viewport dispatcher — routes ShowPacket viewports to panel renderers.

use egui::Ui;

use crate::actions::ObservatoryAction;
use crate::panels::{action_palette, diff_view, focus_cards, gates_panel, object_table};
use crate::state::ObservatoryState;

/// Render viewports from the ShowPacket + action palette. Returns action on interaction.
pub fn ui(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    let mut action = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(ref show_packet) = state.fetch.show_packet.as_ready() {
            if let Some(viewports) = show_packet.get("viewports").and_then(|v| v.as_array()) {
                for vp in viewports {
                    let kind = vp
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let title = vp
                        .get("title")
                        .and_then(|v| v.as_str())
                        .or_else(|| vp.get("id").and_then(|v| v.as_str()))
                        .unwrap_or(kind);
                    let data = vp
                        .get("data")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    ui.collapsing(title, |ui| {
                        let vp_action = match kind {
                            "focus" => focus_cards::ui(ui, &data),
                            "object" => object_table::ui(ui, &data),
                            "diff" => diff_view::ui(ui, &data),
                            "gates" => gates_panel::ui(ui, &data),
                            "taxonomy" => {
                                ui.label("Taxonomy tree (Phase 8)");
                                None
                            }
                            "impact" => {
                                ui.label("Impact graph (Phase 8)");
                                None
                            }
                            "action_surface" => {
                                ui.label("Action surface (Phase 8)");
                                None
                            }
                            "coverage" => {
                                ui.label("Coverage map (Phase 8)");
                                None
                            }
                            _ => {
                                let json_str = serde_json::to_string_pretty(&data)
                                    .unwrap_or_default();
                                ui.monospace(&json_str);
                                None
                            }
                        };
                        if action.is_none() {
                            action = vp_action;
                        }
                    });
                }
            }
        } else if state.fetch.show_packet.is_pending() {
            ui.spinner();
        } else {
            ui.label("No viewport data");
        }

        // Action palette at the bottom
        if let Some(a) = action_palette::ui(ui, state) {
            if action.is_none() {
                action = Some(a);
            }
        }
    });

    action
}
