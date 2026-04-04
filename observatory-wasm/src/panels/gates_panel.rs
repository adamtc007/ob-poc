//! GatesPanel — guardrail results with severity cards.

use egui::{Color32, RichText, Ui};

use crate::actions::ObservatoryAction;

/// Render gates viewport showing guardrail results.
pub fn ui(ui: &mut Ui, data: &serde_json::Value) -> Option<ObservatoryAction> {
    let gates = data
        .get("gates")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if gates.is_empty() {
        ui.colored_label(Color32::from_rgb(34, 197, 94), "All gates passed");
        return None;
    }

    for gate in &gates {
        let severity = gate
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("Advisory");
        let message = gate
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let remediation = gate
            .get("remediation")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let guardrail_id = gate
            .get("guardrail_id")
            .and_then(|v| v.as_str())
            .unwrap_or("—");

        let color = match severity {
            "Block" => Color32::from_rgb(239, 68, 68),
            "Warning" => Color32::from_rgb(245, 158, 11),
            _ => Color32::from_rgb(107, 114, 128),
        };

        egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(1.0, color))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(severity).color(color).strong());
                    ui.monospace(guardrail_id);
                });
                ui.label(message);
                ui.small(remediation);
            });
        ui.add_space(4.0);
    }

    None
}
