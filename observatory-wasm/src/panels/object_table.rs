//! ObjectTable — table renderer for ViewportKind::Object.

use egui::Ui;

use crate::actions::ObservatoryAction;

/// Render object inspector viewport as a table.
pub fn ui(ui: &mut Ui, data: &serde_json::Value) -> Option<ObservatoryAction> {
    let objects = data
        .get("objects")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if objects.is_empty() {
        ui.label("No objects loaded");
        return None;
    }

    for obj in &objects {
        let fqn = obj.get("fqn").and_then(|v| v.as_str()).unwrap_or("—");
        let obj_type = obj.get("object_type").and_then(|v| v.as_str()).unwrap_or("—");

        ui.collapsing(format!("{fqn} ({obj_type})"), |ui| {
            if let Some(definition) = obj.get("definition").and_then(|v| v.as_object()) {
                egui::Grid::new(format!("obj_{fqn}"))
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (key, value) in definition {
                            ui.monospace(key);
                            ui.label(format_value(value));
                            ui.end_row();
                        }
                    });
            } else {
                ui.label("No definition data");
            }
        });
    }

    None
}

fn format_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "—".into(),
        other => other.to_string(),
    }
}
