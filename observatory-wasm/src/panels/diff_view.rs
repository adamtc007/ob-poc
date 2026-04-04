//! DiffView — field-level diff renderer for ViewportKind::Diff.

use egui::{Color32, RichText, Ui};

use crate::actions::ObservatoryAction;

/// Render diff viewport showing Active vs Draft changes.
pub fn ui(ui: &mut Ui, data: &serde_json::Value) -> Option<ObservatoryAction> {
    if let Some(message) = data.get("message").and_then(|v| v.as_str()) {
        ui.label(message);
        return None;
    }

    let diffs = data
        .get("diffs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if diffs.is_empty() {
        ui.label("No differences");
        return None;
    }

    egui::Grid::new("diff_grid")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Field");
            ui.strong("Active");
            ui.strong("Draft");
            ui.end_row();

            for diff in &diffs {
                let field = diff.get("field").and_then(|v| v.as_str()).unwrap_or("—");
                let active = diff
                    .get("active_value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—");
                let draft = diff
                    .get("draft_value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("—");
                let change_type = diff
                    .get("change_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("modified");

                ui.monospace(field);

                match change_type {
                    "removed" => {
                        ui.label(RichText::new(active).strikethrough().color(Color32::from_rgb(239, 68, 68)));
                        ui.label("—");
                    }
                    "added" => {
                        ui.label("—");
                        ui.label(RichText::new(draft).color(Color32::from_rgb(34, 197, 94)));
                    }
                    _ => {
                        ui.label(active);
                        ui.label(RichText::new(draft).strong());
                    }
                }
                ui.end_row();
            }
        });

    None
}
