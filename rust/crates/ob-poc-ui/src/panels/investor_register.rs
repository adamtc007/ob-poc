//! Investor Register Panel
//!
//! Displays investor register visualization with:
//! - Control holders as individual cards
//! - Aggregate investors as collapsible breakdown
//! - Drill-down list for viewing individual investors
//!
//! Following EGUI rules:
//! - No local state mirroring server data
//! - Actions return values, no callbacks
//! - InvestorRegisterUi is UI-only (expand/collapse, selection)

use crate::state::{AppState, InvestorRegisterUi};
use egui::{Color32, RichText, Ui, Vec2};
use ob_poc_types::investor_register::{
    AggregateBreakdown, AggregateInvestorsNode, BreakdownDimension, ControlHolderNode, ControlTier,
    InvestorRegisterView,
};

/// Action returned from investor register panel interactions
#[derive(Debug, Clone)]
pub enum InvestorRegisterAction {
    /// No action
    None,
    /// User clicked on a control holder
    SelectControlHolder { entity_id: String },
    /// User toggled aggregate expansion
    ToggleAggregate,
    /// User changed breakdown dimension
    SetBreakdownDimension(BreakdownDimension),
    /// User clicked to drill down into aggregate
    DrillDown,
    /// User closed drill-down
    CloseDrillDown,
    /// User changed drill-down page
    SetPage(i32),
    /// User applied filter
    ApplyFilter {
        investor_type: Option<String>,
        kyc_status: Option<String>,
        jurisdiction: Option<String>,
    },
    /// User cleared filters
    ClearFilters,
    /// User selected investor from list
    SelectInvestor { entity_id: String },
    /// User changed sort
    SetSort { field: String, ascending: bool },
    /// User searched
    Search { query: String },
    /// User closed the panel
    ClosePanel,
    /// User requested refresh
    Refresh,
}

/// Render the investor register panel
pub fn investor_register_panel(ui: &mut Ui, state: &AppState) -> InvestorRegisterAction {
    let mut action = InvestorRegisterAction::None;

    // Header
    ui.horizontal(|ui| {
        ui.heading("Investor Register");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("✕").on_hover_text("Close panel").clicked() {
                action = InvestorRegisterAction::ClosePanel;
            }
            if ui.small_button("↻").on_hover_text("Refresh data").clicked() {
                action = InvestorRegisterAction::Refresh;
            }
        });
    });

    ui.separator();

    // Check if we have data
    let Some(register) = &state.investor_register else {
        ui.centered_and_justified(|ui| {
            ui.label("No investor register data loaded");
        });
        return action;
    };

    // Issuer summary
    render_issuer_summary(ui, register);
    ui.add_space(8.0);

    // Threshold info
    render_threshold_info(ui, register);
    ui.separator();

    // Scrollable content area
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Control holders section
            if !register.control_holders.is_empty() {
                ui.heading(format!(
                    "Control Holders ({})",
                    register.control_holders.len()
                ));
                ui.add_space(4.0);

                for holder in &register.control_holders {
                    if let Some(holder_action) = render_control_holder_card(ui, holder) {
                        action = holder_action;
                    }
                }

                ui.add_space(8.0);
            }

            // Aggregate section
            if let Some(aggregate) = &register.aggregate {
                let aggregate_action =
                    render_aggregate_section(ui, aggregate, &state.investor_register_ui);
                if !matches!(aggregate_action, InvestorRegisterAction::None) {
                    action = aggregate_action;
                }
            }

            // Drill-down list (if expanded)
            if state.investor_register_ui.show_drill_down {
                ui.add_space(8.0);
                ui.separator();
                let drilldown_action = render_drilldown_list(ui, state);
                if !matches!(drilldown_action, InvestorRegisterAction::None) {
                    action = drilldown_action;
                }
            }
        });

    action
}

/// Render issuer summary header
fn render_issuer_summary(ui: &mut Ui, register: &InvestorRegisterView) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&register.issuer.name).strong().size(16.0));
        if let Some(ref lei) = register.issuer.lei {
            ui.label(
                RichText::new(format!("LEI: {}", lei))
                    .small()
                    .color(Color32::GRAY),
            );
        }
    });

    ui.horizontal(|ui| {
        ui.label(format!("As of: {}", register.as_of_date));
        ui.label("|");
        ui.label(format!(
            "{} total investors",
            format_count(register.total_investor_count)
        ));
        ui.label("|");
        ui.label(format!(
            "{} units issued",
            format_units(register.total_issued_units)
        ));
    });
}

/// Render threshold configuration info
fn render_threshold_info(ui: &mut Ui, register: &InvestorRegisterView) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Thresholds:")
                .small()
                .color(Color32::LIGHT_GRAY),
        );
        ui.label(
            RichText::new(format!(
                "{}% disclosure",
                register.thresholds.disclosure_pct
            ))
            .small()
            .color(Color32::GRAY),
        );
        ui.label(
            RichText::new(format!(
                "{}% significant",
                register.thresholds.significant_pct
            ))
            .small()
            .color(Color32::GRAY),
        );
        ui.label(
            RichText::new(format!("{}% control", register.thresholds.control_pct))
                .small()
                .color(Color32::GRAY),
        );
    });
}

/// Render a control holder card
fn render_control_holder_card(
    ui: &mut Ui,
    holder: &ControlHolderNode,
) -> Option<InvestorRegisterAction> {
    let mut action = None;

    let tier = holder.control_tier();
    let border_color = tier_color(tier);

    egui::Frame::none()
        .fill(Color32::from_rgb(30, 35, 45))
        .stroke(egui::Stroke::new(2.0, border_color))
        .rounding(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Header row
            ui.horizontal(|ui| {
                // Name and type
                ui.vertical(|ui| {
                    if ui
                        .link(RichText::new(&holder.name).strong())
                        .on_hover_text("Click to view details")
                        .clicked()
                    {
                        action = Some(InvestorRegisterAction::SelectControlHolder {
                            entity_id: holder.entity_id.clone(),
                        });
                    }
                    ui.label(
                        RichText::new(&holder.entity_type)
                            .small()
                            .color(Color32::GRAY),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    // KYC badge
                    render_kyc_badge(ui, &holder.kyc_status);

                    // Percentage
                    ui.label(
                        RichText::new(format!("{:.2}%", holder.economic_pct))
                            .strong()
                            .size(14.0),
                    );
                });
            });

            // Control flags row
            ui.horizontal(|ui| {
                if holder.has_control {
                    render_flag_badge(ui, "CONTROL", Color32::from_rgb(220, 38, 38));
                }
                if holder.has_significant_influence {
                    render_flag_badge(ui, "SIGNIFICANT", Color32::from_rgb(234, 179, 8));
                }
                if holder.board_seats > 0 {
                    render_flag_badge(
                        ui,
                        &format!("{} SEATS", holder.board_seats),
                        Color32::from_rgb(59, 130, 246),
                    );
                }
                for veto in &holder.veto_rights {
                    render_flag_badge(ui, veto, Color32::from_rgb(168, 85, 247));
                }
            });

            // Units and reason
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} units", format_units(holder.units)))
                        .small()
                        .color(Color32::GRAY),
                );
                ui.label("|");
                ui.label(
                    RichText::new(&holder.inclusion_reason)
                        .small()
                        .italics()
                        .color(Color32::LIGHT_GRAY),
                );
            });
        });

    ui.add_space(4.0);
    action
}

/// Render the aggregate investors section
fn render_aggregate_section(
    ui: &mut Ui,
    aggregate: &AggregateInvestorsNode,
    ui_state: &InvestorRegisterUi,
) -> InvestorRegisterAction {
    let mut action = InvestorRegisterAction::None;

    // Collapsible header
    let header_text = format!(
        "{} {} other investors ({:.1}%)",
        if ui_state.aggregate_expanded {
            "▼"
        } else {
            "▶"
        },
        format_count(aggregate.investor_count),
        aggregate.economic_pct
    );

    egui::Frame::none()
        .fill(Color32::from_rgb(40, 45, 55))
        .rounding(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Clickable header
            if ui
                .add(
                    egui::Label::new(RichText::new(&header_text).strong().size(14.0))
                        .sense(egui::Sense::click()),
                )
                .on_hover_text("Click to expand/collapse")
                .clicked()
            {
                action = InvestorRegisterAction::ToggleAggregate;
            }

            // Expanded content
            if ui_state.aggregate_expanded {
                ui.add_space(8.0);

                // Dimension selector
                ui.horizontal(|ui| {
                    ui.label("View by:");
                    if ui
                        .selectable_label(
                            ui_state.breakdown_dimension == BreakdownDimension::InvestorType,
                            "Type",
                        )
                        .clicked()
                    {
                        action = InvestorRegisterAction::SetBreakdownDimension(
                            BreakdownDimension::InvestorType,
                        );
                    }
                    if ui
                        .selectable_label(
                            ui_state.breakdown_dimension == BreakdownDimension::KycStatus,
                            "KYC Status",
                        )
                        .clicked()
                    {
                        action = InvestorRegisterAction::SetBreakdownDimension(
                            BreakdownDimension::KycStatus,
                        );
                    }
                    if ui
                        .selectable_label(
                            ui_state.breakdown_dimension == BreakdownDimension::Jurisdiction,
                            "Jurisdiction",
                        )
                        .clicked()
                    {
                        action = InvestorRegisterAction::SetBreakdownDimension(
                            BreakdownDimension::Jurisdiction,
                        );
                    }
                });

                ui.add_space(4.0);

                // Breakdown bars
                let breakdown = aggregate.get_breakdown(ui_state.breakdown_dimension);
                for item in breakdown {
                    render_breakdown_bar(ui, item, aggregate.economic_pct);
                }

                // Drill-down button
                if aggregate.can_drill_down {
                    ui.add_space(8.0);
                    if ui
                        .button("View All Investors →")
                        .on_hover_text("Open paginated list")
                        .clicked()
                    {
                        action = InvestorRegisterAction::DrillDown;
                    }
                }
            }
        });

    action
}

/// Render a breakdown bar
fn render_breakdown_bar(ui: &mut Ui, item: &AggregateBreakdown, total_pct: f64) {
    let bar_pct = if total_pct > 0.0 {
        (item.pct / total_pct) as f32
    } else {
        0.0
    };

    ui.horizontal(|ui| {
        // Label
        ui.label(
            RichText::new(&item.label)
                .small()
                .color(Color32::LIGHT_GRAY),
        );

        // Bar
        let available = ui.available_width() - 100.0;
        let bar_width = available.max(50.0);

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(bar_width, 16.0), egui::Sense::hover());

        let painter = ui.painter();

        // Background
        painter.rect_filled(rect, 3.0, Color32::from_rgb(50, 55, 65));

        // Fill
        let fill_width = bar_width * bar_pct.min(1.0);
        if fill_width > 0.0 {
            let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, 16.0));
            painter.rect_filled(fill_rect, 3.0, Color32::from_rgb(59, 130, 246));
        }

        // Count and percentage
        ui.label(
            RichText::new(format!("{} ({:.1}%)", format_count(item.count), item.pct))
                .small()
                .color(Color32::GRAY),
        );
    });
}

/// Render the drill-down investor list
fn render_drilldown_list(ui: &mut Ui, state: &AppState) -> InvestorRegisterAction {
    let mut action = InvestorRegisterAction::None;

    ui.heading("All Investors");

    // Close button
    ui.horizontal(|ui| {
        if ui.button("← Back to Summary").clicked() {
            action = InvestorRegisterAction::CloseDrillDown;
        }
    });

    // Check if we have list data
    let Some(list) = &state.investor_list else {
        ui.centered_and_justified(|ui| {
            ui.spinner();
            ui.label("Loading investors...");
        });
        return action;
    };

    // Filters row
    ui.horizontal(|ui| {
        ui.label("Filters:");
        // TODO: Add filter dropdowns
        if ui.button("Clear").clicked() {
            action = InvestorRegisterAction::ClearFilters;
        }
    });

    ui.separator();

    // Table header
    ui.horizontal(|ui| {
        ui.label(RichText::new("Name").strong());
        ui.add_space(100.0);
        ui.label(RichText::new("Type").strong());
        ui.add_space(50.0);
        ui.label(RichText::new("Units").strong());
        ui.add_space(50.0);
        ui.label(RichText::new("%").strong());
        ui.add_space(30.0);
        ui.label(RichText::new("KYC").strong());
    });

    ui.separator();

    // Investor rows
    for investor in &list.items {
        ui.horizontal(|ui| {
            if ui.link(&investor.name).clicked() {
                action = InvestorRegisterAction::SelectInvestor {
                    entity_id: investor.entity_id.clone(),
                };
            }
            ui.add_space(100.0);
            ui.label(&investor.entity_type);
            ui.add_space(50.0);
            ui.label(format_units(investor.units));
            ui.add_space(50.0);
            ui.label(format!("{:.2}%", investor.economic_pct));
            ui.add_space(30.0);
            render_kyc_badge(ui, &investor.kyc_status);
        });
    }

    // Pagination
    ui.separator();
    ui.horizontal(|ui| {
        let pagination = &list.pagination;

        if ui
            .add_enabled(pagination.page > 1, egui::Button::new("← Prev"))
            .clicked()
        {
            action = InvestorRegisterAction::SetPage(pagination.page - 1);
        }

        ui.label(format!(
            "Page {} of {}",
            pagination.page, pagination.total_pages
        ));

        if ui
            .add_enabled(
                pagination.page < pagination.total_pages,
                egui::Button::new("Next →"),
            )
            .clicked()
        {
            action = InvestorRegisterAction::SetPage(pagination.page + 1);
        }

        ui.label(format!("({} total)", format_count(pagination.total_items)));
    });

    action
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Get color for control tier
fn tier_color(tier: ControlTier) -> Color32 {
    match tier {
        ControlTier::Control => Color32::from_rgb(220, 38, 38), // Red
        ControlTier::Significant => Color32::from_rgb(234, 179, 8), // Yellow
        ControlTier::Disclosure => Color32::from_rgb(59, 130, 246), // Blue
        ControlTier::SpecialRights => Color32::from_rgb(168, 85, 247), // Purple
    }
}

/// Render a KYC status badge
fn render_kyc_badge(ui: &mut Ui, status: &str) {
    let (color, text) = match status.to_uppercase().as_str() {
        "APPROVED" => (Color32::from_rgb(34, 197, 94), "✓"),
        "PENDING" => (Color32::from_rgb(234, 179, 8), "⏳"),
        "REJECTED" => (Color32::from_rgb(220, 38, 38), "✗"),
        "EXPIRED" => (Color32::from_rgb(156, 163, 175), "⚠"),
        _ => (Color32::GRAY, "?"),
    };

    ui.label(RichText::new(text).color(color).strong());
}

/// Render a flag badge
fn render_flag_badge(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.gamma_multiply(0.3))
        .rounding(3.0)
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}

/// Format large counts with separators
fn format_count(count: i32) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

/// Format units with appropriate precision
fn format_units(units: f64) -> String {
    if units >= 1_000_000.0 {
        format!("{:.2}M", units / 1_000_000.0)
    } else if units >= 1_000.0 {
        format!("{:.2}K", units / 1_000.0)
    } else {
        format!("{:.2}", units)
    }
}
