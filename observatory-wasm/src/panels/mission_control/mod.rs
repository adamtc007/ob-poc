//! Mission Control — health dashboard + maintenance timeline.

use egui::Ui;

use crate::actions::ObservatoryAction;
use crate::state::ObservatoryState;

/// Render Mission Control dashboard.
pub fn ui(ui: &mut Ui, state: &ObservatoryState) -> Option<ObservatoryAction> {
    ui.heading("Mission Control");
    ui.separator();

    // ── Health metrics ──
    if let Some(ref health) = state.fetch.health.as_ready() {
        egui::Grid::new("health_grid")
            .num_columns(2)
            .spacing([20.0, 8.0])
            .show(ui, |ui| {
                ui.label("Pending Changesets");
                ui.strong(format!("{}", health.pending_changesets));
                ui.end_row();

                ui.label("Stale Dry-Runs");
                let stale_color = if health.stale_dryruns > 0 {
                    egui::Color32::from_rgb(239, 68, 68)
                } else {
                    egui::Color32::from_rgb(34, 197, 94)
                };
                ui.colored_label(stale_color, format!("{}", health.stale_dryruns));
                ui.end_row();

                ui.label("Active Snapshots");
                ui.strong(format!("{}", health.active_snapshots));
                ui.end_row();

                ui.label("Archived");
                ui.label(format!("{}", health.archived_changesets));
                ui.end_row();

                if let Some(hours) = health.embedding_freshness_hours {
                    ui.label("Embedding Age");
                    ui.label(format!("{:.0}h", hours));
                    ui.end_row();
                }

                if let Some(depth) = health.outbox_depth {
                    ui.label("Outbox Depth");
                    let depth_color = if depth > 100 {
                        egui::Color32::from_rgb(239, 68, 68)
                    } else if depth > 10 {
                        egui::Color32::from_rgb(245, 158, 11)
                    } else {
                        egui::Color32::from_rgb(34, 197, 94)
                    };
                    ui.colored_label(depth_color, format!("{depth}"));
                    ui.end_row();
                }
            });
    } else if state.fetch.health.is_pending() {
        ui.spinner();
        ui.label("Loading health metrics...");
    } else {
        ui.label("Health data not loaded");
    }

    ui.add_space(16.0);

    // ── Quick actions ──
    ui.heading("Quick Actions");
    ui.separator();

    let verbs = [
        ("maintenance.health-pending", "Check Pending"),
        ("maintenance.health-stale-dryruns", "Stale Dry-Runs"),
        ("maintenance.validate-schema-sync", "Schema Sync"),
        ("maintenance.drain-outbox", "Drain Outbox"),
        ("maintenance.reindex-embeddings", "Reindex"),
        ("maintenance.cleanup", "Cleanup"),
    ];

    let mut action = None;
    egui::Grid::new("quick_actions")
        .num_columns(3)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            for (i, (fqn, label)) in verbs.iter().enumerate() {
                if ui.button(*label).clicked() {
                    action = Some(ObservatoryAction::InvokeVerb {
                        verb_fqn: fqn.to_string(),
                    });
                }
                if (i + 1) % 3 == 0 {
                    ui.end_row();
                }
            }
        });

    action
}
