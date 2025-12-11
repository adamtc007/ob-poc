//! AST Panel
//!
//! Displays the parsed AST with interactive EntityRef resolution.
//! Unresolved EntityRefs are clickable to open the Entity Finder modal.

use egui::{Color32, RichText, ScrollArea, Ui};

use crate::state::{AstNode, SimpleAstArg, SimpleAstStatement};

/// AST panel widget
pub struct AstPanel;

impl Default for AstPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AstPanel {
    pub fn new() -> Self {
        Self
    }

    /// Render the AST panel
    /// Returns Some(ref_click) if an unresolved EntityRef was clicked
    pub fn ui(&self, ui: &mut Ui, statements: &[SimpleAstStatement]) -> Option<UnresolvedRefClick> {
        let mut clicked_ref: Option<UnresolvedRefClick> = None;

        ui.vertical(|ui| {
            // Panel header
            ui.horizontal(|ui| {
                ui.label(RichText::new("AST").strong().size(14.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} statements", statements.len()))
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                });
            });
            ui.separator();

            // AST content
            if statements.is_empty() {
                self.render_empty_state(ui);
            } else {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (idx, stmt) in statements.iter().enumerate() {
                            if let Some(ref_click) = self.render_statement(ui, stmt, idx) {
                                clicked_ref = Some(ref_click);
                            }
                        }
                    });
            }
        });

        clicked_ref
    }

    fn render_empty_state(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("No AST generated yet").color(Color32::GRAY));
            ui.add_space(8.0);
            ui.label(
                RichText::new("Parsed AST will appear here")
                    .color(Color32::DARK_GRAY)
                    .size(12.0),
            );
        });
    }

    fn render_statement(
        &self,
        ui: &mut Ui,
        stmt: &SimpleAstStatement,
        idx: usize,
    ) -> Option<UnresolvedRefClick> {
        let mut clicked_ref: Option<UnresolvedRefClick> = None;

        egui::Frame::none()
            .fill(Color32::from_rgb(35, 35, 35))
            .rounding(4.0)
            .inner_margin(8.0)
            .outer_margin(egui::Margin::symmetric(0.0, 2.0))
            .show(ui, |ui| {
                match stmt {
                    SimpleAstStatement::VerbCall {
                        verb,
                        args,
                        bind_as,
                    } => {
                        // Verb header
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}.", idx + 1))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                            ui.label(
                                RichText::new(verb)
                                    .monospace()
                                    .color(Color32::from_rgb(220, 220, 170)), // Yellow
                            );
                            if let Some(binding) = bind_as {
                                ui.label(
                                    RichText::new(format!(" :as @{}", binding))
                                        .monospace()
                                        .color(Color32::from_rgb(78, 201, 176)), // Teal
                                );
                            }
                        });

                        // Arguments
                        ui.indent("args", |ui| {
                            for arg in args {
                                if let Some(ref_click) =
                                    self.render_argument(ui, arg, idx, verb.clone())
                                {
                                    clicked_ref = Some(ref_click);
                                }
                            }
                        });
                    }
                    SimpleAstStatement::Comment(text) => {
                        ui.label(
                            RichText::new(format!("; {}", text))
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(106, 153, 85)), // Green
                        );
                    }
                }
            });

        clicked_ref
    }

    fn render_argument(
        &self,
        ui: &mut Ui,
        arg: &SimpleAstArg,
        stmt_idx: usize,
        verb: String,
    ) -> Option<UnresolvedRefClick> {
        let mut clicked_ref: Option<UnresolvedRefClick> = None;

        ui.horizontal(|ui| {
            // Keyword
            ui.label(
                RichText::new(format!(":{}", arg.key))
                    .monospace()
                    .size(11.0)
                    .color(Color32::from_rgb(156, 220, 254)), // Light blue
            );

            // Value
            if let Some(ref_click) =
                self.render_node(ui, &arg.value, stmt_idx, verb, arg.key.clone())
            {
                clicked_ref = Some(ref_click);
            }
        });

        clicked_ref
    }

    #[allow(clippy::only_used_in_recursion)]
    fn render_node(
        &self,
        ui: &mut Ui,
        node: &AstNode,
        stmt_idx: usize,
        verb: String,
        arg_key: String,
    ) -> Option<UnresolvedRefClick> {
        let mut clicked_ref: Option<UnresolvedRefClick> = None;

        match node {
            AstNode::String(s) => {
                ui.label(
                    RichText::new(format!("\"{}\"", s))
                        .monospace()
                        .size(11.0)
                        .color(Color32::from_rgb(206, 145, 120)), // Orange
                );
            }
            AstNode::Number(n) => {
                ui.label(
                    RichText::new(n.to_string())
                        .monospace()
                        .size(11.0)
                        .color(Color32::from_rgb(181, 206, 168)), // Light green
                );
            }
            AstNode::Boolean(b) => {
                ui.label(
                    RichText::new(b.to_string())
                        .monospace()
                        .size(11.0)
                        .color(Color32::from_rgb(86, 156, 214)), // Blue
                );
            }
            AstNode::SymbolRef(sym) => {
                ui.label(
                    RichText::new(format!("@{}", sym))
                        .monospace()
                        .size(11.0)
                        .color(Color32::from_rgb(78, 201, 176)), // Teal
                );
            }
            AstNode::EntityRef {
                entity_type,
                value,
                resolved_key,
            } => {
                let is_resolved = resolved_key.is_some();
                let (bg_color, text_color, icon) = if is_resolved {
                    (
                        Color32::from_rgb(30, 50, 30),
                        Color32::from_rgb(134, 239, 172),
                        "✓",
                    )
                } else {
                    (
                        Color32::from_rgb(60, 30, 30),
                        Color32::from_rgb(248, 113, 113),
                        "?",
                    )
                };

                let response = egui::Frame::none()
                    .fill(bg_color)
                    .rounding(3.0)
                    .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).size(10.0).color(text_color));
                            ui.label(
                                RichText::new(format!("{}:", entity_type))
                                    .monospace()
                                    .size(10.0)
                                    .color(Color32::GRAY),
                            );
                            ui.label(
                                RichText::new(format!("\"{}\"", value))
                                    .monospace()
                                    .size(11.0)
                                    .color(text_color),
                            );
                        });
                    })
                    .response;

                // Make unresolved refs clickable
                if !is_resolved {
                    let response = response.interact(egui::Sense::click());
                    if response.clicked() {
                        clicked_ref = Some(UnresolvedRefClick {
                            statement_idx: stmt_idx,
                            verb: verb.clone(),
                            arg_key: arg_key.clone(),
                            entity_type: entity_type.clone(),
                            search_text: value.clone(),
                        });
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            }
            AstNode::List(items) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("[").monospace().size(11.0));
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new(", ").monospace().size(11.0));
                        }
                        if let Some(ref_click) =
                            self.render_node(ui, item, stmt_idx, verb.clone(), arg_key.clone())
                        {
                            clicked_ref = Some(ref_click);
                        }
                    }
                    ui.label(RichText::new("]").monospace().size(11.0));
                });
            }
            AstNode::Map(entries) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("{").monospace().size(11.0));
                    for (i, (k, v)) in entries.iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new(", ").monospace().size(11.0));
                        }
                        ui.label(
                            RichText::new(format!("\"{}\":", k))
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(156, 220, 254)),
                        );
                        if let Some(ref_click) =
                            self.render_node(ui, v, stmt_idx, verb.clone(), arg_key.clone())
                        {
                            clicked_ref = Some(ref_click);
                        }
                    }
                    ui.label(RichText::new("}").monospace().size(11.0));
                });
            }
            AstNode::Null => {
                ui.label(
                    RichText::new("nil")
                        .monospace()
                        .size(11.0)
                        .color(Color32::GRAY),
                );
            }
        }

        clicked_ref
    }
}

/// Information about a clicked unresolved EntityRef
#[derive(Debug, Clone)]
pub struct UnresolvedRefClick {
    pub statement_idx: usize,
    pub verb: String,
    pub arg_key: String,
    pub entity_type: String,
    pub search_text: String,
}
