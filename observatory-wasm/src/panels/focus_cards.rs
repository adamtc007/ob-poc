//! FocusCards — card-based renderer for ViewportKind::Focus.

use egui::Ui;

use crate::actions::ObservatoryAction;

/// Render focus viewport data as cards.
pub fn ui(ui: &mut Ui, data: &serde_json::Value) -> Option<ObservatoryAction> {
    let objects = data
        .get("objects")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if objects.is_empty() {
        ui.label("No objects in focus");
        return None;
    }

    for obj in &objects {
        let obj_type = obj.get("object_type").and_then(|v| v.as_str()).unwrap_or("—");
        let fqn = obj.get("fqn").and_then(|v| v.as_str()).unwrap_or("—");
        let obj_id = obj.get("object_id").and_then(|v| v.as_str()).unwrap_or("—");

        egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.small(obj_type.to_uppercase());
                ui.strong(fqn);
                ui.small(obj_id);
            });
        ui.add_space(4.0);
    }

    None
}
