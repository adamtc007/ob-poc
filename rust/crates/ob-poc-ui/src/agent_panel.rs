//! Agent panel for DSL generation

use egui::{RichText, TextEdit};

/// Agent panel for generating DSL from natural language
pub struct AgentPanel {
    domain: AgentDomain,
    prompt: String,
    generated_dsl: Option<String>,
    plan_only: bool,
    loading: bool,
    error: Option<String>,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum AgentDomain {
    #[default]
    Custody,
    Cbu,
    Entity,
}

impl Default for AgentPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPanel {
    pub fn new() -> Self {
        Self {
            domain: AgentDomain::Custody,
            prompt: String::new(),
            generated_dsl: None,
            plan_only: false,
            loading: false,
            error: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Agent DSL Generator");
        ui.separator();

        // Domain selector
        ui.horizontal(|ui| {
            ui.label("Domain:");
            ui.selectable_value(&mut self.domain, AgentDomain::Custody, "Custody");
            ui.selectable_value(&mut self.domain, AgentDomain::Cbu, "CBU");
            ui.selectable_value(&mut self.domain, AgentDomain::Entity, "Entity");
        });

        ui.add_space(8.0);

        // Prompt input
        ui.label("Describe what you want to set up:");
        ui.add(
            TextEdit::multiline(&mut self.prompt)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text(
                    "e.g., Onboard Pacific Fund for US and UK equities with USD cross-currency...",
                ),
        );

        ui.add_space(8.0);

        // Options
        ui.checkbox(&mut self.plan_only, "Show plan only (don't generate DSL)");

        ui.add_space(8.0);

        // Generate button
        ui.horizontal(|ui| {
            let button_text = if self.loading {
                "Generating..."
            } else {
                "Generate DSL"
            };
            if ui.button(button_text).clicked() && !self.loading && !self.prompt.is_empty() {
                self.generate();
            }

            if ui.button("Clear").clicked() {
                self.prompt.clear();
                self.generated_dsl = None;
                self.error = None;
            }
        });

        ui.separator();

        // Output
        if let Some(ref err) = self.error {
            ui.colored_label(egui::Color32::RED, err);
        }

        if let Some(ref dsl) = self.generated_dsl {
            ui.label(RichText::new("Generated DSL:").strong());

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    let mut dsl_text = dsl.clone();
                    ui.add(
                        TextEdit::multiline(&mut dsl_text)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });

            ui.add_space(8.0);

            if ui.button("Copy to Clipboard").clicked() {
                ui.output_mut(|o| o.copied_text = dsl.clone());
            }
        }
    }

    fn generate(&mut self) {
        self.loading = true;
        self.error = None;

        // TODO: Implement async API call
        // For now, show a placeholder
        self.generated_dsl = Some(format!(
            "; Generated DSL for: {}\n; Domain: {:?}\n\n(cbu.ensure :name \"Example\" :jurisdiction \"US\")",
            self.prompt,
            self.domain
        ));
        self.loading = false;
    }

    /// Get the generated DSL if available
    pub fn get_generated_dsl(&self) -> Option<&str> {
        self.generated_dsl.as_deref()
    }
}

impl std::fmt::Debug for AgentDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentDomain::Custody => write!(f, "custody"),
            AgentDomain::Cbu => write!(f, "cbu"),
            AgentDomain::Entity => write!(f, "entity"),
        }
    }
}
