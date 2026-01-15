//! DSL Renderer - Converts Plan to DSL s-expressions.
//!
//! This module renders execution plans to DSL source code.
//! The rendering is driven by:
//! 1. The semantic action in the plan
//! 2. The slots filled with values
//! 3. Context (active CBU, session state)
//!
//! ## Future: Verb-Driven Rendering
//!
//! Currently, rendering logic is hardcoded per SemanticAction.
//! The goal is to make this data-driven via verb YAML definitions:
//!
//! ```yaml
//! trading-profile:
//!   add-isda-config:
//!     semantic_action: EstablishIsda
//!     slot_mapping:
//!       counterparty: counterparty-id
//!       governing_law: governing-law
//!     template: |
//!       (trading-profile.add-isda-config
//!         :profile-id {cbu}
//!         :counterparty-id {counterparty}
//!         :governing-law {governing_law})
//! ```

use super::intent_plan::{Plan, SemanticAction, SlotValue};

/// Rendering context - session state for DSL generation.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    /// Active CBU ID (if set).
    pub active_cbu_id: Option<String>,
    /// Active CBU name (for display).
    pub active_cbu_name: Option<String>,
    /// Symbol bindings from previous statements.
    pub symbols: std::collections::HashMap<String, String>,
}

impl RenderContext {
    /// Create a new render context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the active CBU.
    pub fn with_cbu(mut self, id: String, name: String) -> Self {
        self.active_cbu_id = Some(id);
        self.active_cbu_name = Some(name);
        self
    }

    /// Get the CBU reference for DSL.
    pub fn cbu_ref(&self) -> String {
        match &self.active_cbu_id {
            Some(id) => format!("\"{}\"", id),
            None => "@cbu".to_string(),
        }
    }
}

/// Rendered DSL output.
#[derive(Debug, Clone)]
pub struct RenderedDsl {
    /// The DSL source code.
    pub source: String,
    /// Symbols defined by this DSL (e.g., @counterparty, @isda).
    pub defined_symbols: Vec<String>,
    /// Human-readable description of what the DSL does.
    pub description: String,
}

/// Render a plan to DSL s-expressions.
pub fn render_plan(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    match plan.action {
        SemanticAction::CreateCounterparty => render_create_counterparty(plan, ctx),
        SemanticAction::EstablishIsda => render_establish_isda(plan, ctx),
        SemanticAction::AddCsa => render_add_csa(plan, ctx),
        SemanticAction::AssignRole => render_assign_role(plan, ctx),
        SemanticAction::RemoveRole => render_remove_role(plan, ctx),
        SemanticAction::AddToUniverse => render_add_to_universe(plan, ctx),
        SemanticAction::ListEntities => render_list_entities(plan, ctx),
        SemanticAction::ShowEntity => render_show_entity(plan, ctx),
        SemanticAction::ListCounterparties => render_list_counterparties(plan, ctx),
        SemanticAction::ShowIsda => render_show_isda(plan, ctx),
        SemanticAction::Unknown => render_unknown(plan, ctx),
        _ => RenderedDsl {
            source: "; Intent not yet supported".to_string(),
            defined_symbols: vec![],
            description: "Unknown intent".to_string(),
        },
    }
}

fn render_create_counterparty(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    let counterparty = plan
        .get("counterparty")
        .map(|v| v.to_dsl())
        .unwrap_or_default();
    let cp_name = plan
        .get_entity("counterparty")
        .map(|e| e.name())
        .unwrap_or("unknown");

    let mut statements = vec![format!(
        "(entity.ensure-limited-company :name {} :as @counterparty)",
        counterparty
    )];
    let mut symbols = vec!["@counterparty".to_string()];
    let mut desc_parts = vec![format!("Create counterparty {}", cp_name)];

    // Add instruments if present
    if let Some(SlotValue::List(instruments)) = plan.get("instruments") {
        let cbu_ref = ctx.cbu_ref();
        for inst in instruments {
            statements.push(format!(
                "(trading-profile.add-instrument-class :profile-id {} :class-code {})",
                cbu_ref,
                inst.to_dsl()
            ));
        }
        let inst_names: Vec<_> = instruments.iter().map(|i| i.to_dsl()).collect();
        desc_parts.push(format!("for {}", inst_names.join(", ")));
    }

    // Add ISDA if governing law present
    if let Some(law) = plan.get("governing_law") {
        let cbu_ref = ctx.cbu_ref();
        statements.push(format!(
            "(isda.create :cbu-id {} :counterparty @counterparty :agreement-date \"today\" :effective-date \"today\" :governing-law {} :as @isda)",
            cbu_ref,
            law.to_dsl()
        ));
        symbols.push("@isda".to_string());
        desc_parts.push(format!("under {} law", law.to_dsl()));
    }

    RenderedDsl {
        source: statements.join("\n"),
        defined_symbols: symbols,
        description: desc_parts.join(" "),
    }
}

fn render_establish_isda(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    let counterparty = plan
        .get("counterparty")
        .map(|v| v.to_dsl())
        .unwrap_or_default();
    let cp_name = plan
        .get_entity("counterparty")
        .map(|e| e.name())
        .unwrap_or("unknown");
    let law = plan
        .get("governing_law")
        .map(|v| v.to_dsl())
        .unwrap_or_else(|| "\"NY_LAW\"".to_string());
    let cbu_ref = ctx.cbu_ref();

    let mut statements = vec![format!(
        "(isda.create :cbu-id {} :counterparty {} :agreement-date \"today\" :effective-date \"today\" :governing-law {} :as @isda)",
        cbu_ref, counterparty, law
    )];

    // Add coverage for instruments
    if let Some(SlotValue::List(instruments)) = plan.get("instruments") {
        for inst in instruments {
            statements.push(format!(
                "(isda.add-coverage :isda-id @isda :instrument-class {})",
                inst.to_dsl()
            ));
        }
    }

    RenderedDsl {
        source: statements.join("\n"),
        defined_symbols: vec!["@isda".to_string()],
        description: format!("Establish ISDA with {} under {} law", cp_name, law),
    }
}

fn render_add_csa(plan: &Plan, _ctx: &RenderContext) -> RenderedDsl {
    let csa_type = plan
        .get("csa_type")
        .map(|v| v.to_dsl())
        .unwrap_or_else(|| "\"VM\"".to_string());
    let cp_name = plan
        .get_entity("counterparty")
        .map(|e| e.name())
        .unwrap_or("unknown");

    let mut dsl = format!(
        "(isda.add-csa :isda-id @isda :csa-type {} :effective-date \"today\"",
        csa_type
    );

    if let Some(currency) = plan.get("currency") {
        dsl.push_str(&format!(" :threshold-currency {}", currency.to_dsl()));
    }

    dsl.push_str(" :as @csa)");

    RenderedDsl {
        source: dsl,
        defined_symbols: vec!["@csa".to_string()],
        description: format!("Add {} CSA for {}", csa_type, cp_name),
    }
}

fn render_assign_role(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    let cbu = plan
        .get("cbu")
        .map(|v| {
            if let SlotValue::Entity(e) = v {
                if e.is_resolved() {
                    v.to_dsl()
                } else {
                    ctx.cbu_ref()
                }
            } else {
                ctx.cbu_ref()
            }
        })
        .unwrap_or_else(|| ctx.cbu_ref());

    let entity = plan.get("entity").map(|v| v.to_dsl()).unwrap_or_default();
    let role = plan.get("role").map(|v| v.to_dsl()).unwrap_or_default();
    let entity_name = plan
        .get_entity("entity")
        .map(|e| e.name())
        .unwrap_or("unknown");

    RenderedDsl {
        source: format!(
            "(cbu.assign-role :cbu-id {} :entity-id {} :role {})",
            cbu, entity, role
        ),
        defined_symbols: vec![],
        description: format!("Assign {} as {}", entity_name, role),
    }
}

fn render_remove_role(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    let cbu = plan
        .get("cbu")
        .map(|v| v.to_dsl())
        .unwrap_or_else(|| ctx.cbu_ref());
    let entity = plan.get("entity").map(|v| v.to_dsl()).unwrap_or_default();
    let role = plan.get("role").map(|v| v.to_dsl()).unwrap_or_default();
    let entity_name = plan
        .get_entity("entity")
        .map(|e| e.name())
        .unwrap_or("unknown");

    RenderedDsl {
        source: format!(
            "(cbu.remove-role :cbu-id {} :entity-id {} :role {})",
            cbu, entity, role
        ),
        defined_symbols: vec![],
        description: format!("Remove {} from role {}", entity_name, role),
    }
}

fn render_add_to_universe(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    let cbu = plan
        .get("cbu")
        .map(|v| {
            if let SlotValue::Entity(e) = v {
                if e.is_resolved() {
                    v.to_dsl()
                } else {
                    ctx.cbu_ref()
                }
            } else {
                ctx.cbu_ref()
            }
        })
        .unwrap_or_else(|| ctx.cbu_ref());

    let mut statements = Vec::new();
    let mut desc_parts = Vec::new();

    // Get currencies or default to USD
    let currencies = match plan.get("currencies") {
        Some(SlotValue::List(currs)) => currs.iter().map(|c| c.to_dsl()).collect::<Vec<_>>(),
        _ => vec!["\"USD\"".to_string()],
    };
    let curr_list = format!("[{}]", currencies.join(" "));

    // Add markets
    if let Some(SlotValue::List(markets)) = plan.get("markets") {
        for market in markets {
            statements.push(format!(
                "(trading-profile.add-market :profile-id {} :market-code {} :currencies {})",
                cbu,
                market.to_dsl(),
                curr_list
            ));
        }
        let market_names: Vec<_> = markets.iter().map(|m| m.to_dsl()).collect();
        desc_parts.push(format!("markets: {}", market_names.join(", ")));
    }

    // Add instruments
    if let Some(SlotValue::List(instruments)) = plan.get("instruments") {
        for inst in instruments {
            statements.push(format!(
                "(trading-profile.add-instrument-class :profile-id {} :class-code {})",
                cbu,
                inst.to_dsl()
            ));
        }
        let inst_names: Vec<_> = instruments.iter().map(|i| i.to_dsl()).collect();
        desc_parts.push(format!("instruments: {}", inst_names.join(", ")));
    }

    RenderedDsl {
        source: statements.join("\n"),
        defined_symbols: vec![],
        description: format!("Add to trading universe: {}", desc_parts.join(", ")),
    }
}

fn render_list_entities(plan: &Plan, _ctx: &RenderContext) -> RenderedDsl {
    let entity_type = plan.get_string("entity_type").unwrap_or("entity");

    RenderedDsl {
        source: format!("({}.list)", entity_type),
        defined_symbols: vec![],
        description: format!("List all {}s", entity_type),
    }
}

fn render_show_entity(plan: &Plan, _ctx: &RenderContext) -> RenderedDsl {
    let entity = plan.get("entity").map(|v| v.to_dsl()).unwrap_or_default();
    let entity_name = plan
        .get_entity("entity")
        .map(|e| e.name())
        .unwrap_or("unknown");

    RenderedDsl {
        source: format!("(entity.read :id {})", entity),
        defined_symbols: vec![],
        description: format!("Show details for {}", entity_name),
    }
}

fn render_list_counterparties(plan: &Plan, ctx: &RenderContext) -> RenderedDsl {
    let cbu = plan
        .get("cbu")
        .map(|v| v.to_dsl())
        .unwrap_or_else(|| ctx.cbu_ref());

    RenderedDsl {
        source: format!("(cbu.parties :cbu-id {})", cbu),
        defined_symbols: vec![],
        description: "List counterparties".to_string(),
    }
}

fn render_show_isda(plan: &Plan, _ctx: &RenderContext) -> RenderedDsl {
    match plan.get("counterparty") {
        Some(cp) => {
            let cp_name = plan
                .get_entity("counterparty")
                .map(|e| e.name())
                .unwrap_or("unknown");
            RenderedDsl {
                source: format!("(isda.list :counterparty-id {})", cp.to_dsl()),
                defined_symbols: vec![],
                description: format!("Show ISDA with {}", cp_name),
            }
        }
        None => RenderedDsl {
            source: "(isda.list)".to_string(),
            defined_symbols: vec![],
            description: "List all ISDAs".to_string(),
        },
    }
}

fn render_unknown(plan: &Plan, _ctx: &RenderContext) -> RenderedDsl {
    let raw_text = plan.get_string("raw_text").unwrap_or("unknown intent");

    RenderedDsl {
        source: format!("; Could not parse: {}", raw_text),
        defined_symbols: vec![],
        description: format!("Unknown intent: {}", raw_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::intent_ast::{EntityRef, GoverningLaw, InstrumentCode};
    use crate::lexicon::intent_plan::intent_to_plan;
    use crate::lexicon::IntentAst;

    #[test]
    fn test_render_counterparty_create() {
        let intent = IntentAst::CounterpartyCreate {
            counterparty: EntityRef::Unresolved {
                name: "Goldman Sachs".to_string(),
                entity_type: Some("counterparty".to_string()),
            },
            instruments: vec![InstrumentCode::new("IRS")],
            governing_law: Some(GoverningLaw::NewYork),
        };

        let plan = intent_to_plan(&intent);
        let ctx = RenderContext::new().with_cbu("cbu-123".to_string(), "Test Fund".to_string());
        let rendered = render_plan(&plan, &ctx);

        assert!(rendered.source.contains("entity.ensure-limited-company"));
        assert!(rendered.source.contains("Goldman Sachs"));
        assert!(rendered
            .source
            .contains("trading-profile.add-instrument-class"));
        assert!(rendered.source.contains("isda.create"));
        assert!(rendered
            .defined_symbols
            .contains(&"@counterparty".to_string()));
        assert!(rendered.defined_symbols.contains(&"@isda".to_string()));
    }

    #[test]
    fn test_render_isda_establish() {
        let intent = IntentAst::IsdaEstablish {
            counterparty: EntityRef::Unresolved {
                name: "Citi".to_string(),
                entity_type: Some("counterparty".to_string()),
            },
            governing_law: GoverningLaw::English,
            instruments: vec![],
        };

        let plan = intent_to_plan(&intent);
        let ctx = RenderContext::new();
        let rendered = render_plan(&plan, &ctx);

        assert!(rendered.source.contains("isda.create"));
        assert!(rendered.source.contains("Citi"));
        assert!(rendered.source.contains("ENGLISH_LAW"));
    }

    #[test]
    fn test_render_with_cbu_context() {
        let intent = IntentAst::UniverseAdd {
            cbu: EntityRef::Pronoun {
                text: "it".to_string(),
                referent: None,
            },
            markets: vec![super::super::intent_ast::MarketCode::new("NYSE")],
            instruments: vec![],
            currencies: vec![],
        };

        let plan = intent_to_plan(&intent);

        // Without CBU context
        let ctx_no_cbu = RenderContext::new();
        let rendered_no_cbu = render_plan(&plan, &ctx_no_cbu);
        assert!(rendered_no_cbu.source.contains("@cbu"));

        // With CBU context
        let ctx_with_cbu =
            RenderContext::new().with_cbu("cbu-456".to_string(), "My Fund".to_string());
        let rendered_with_cbu = render_plan(&plan, &ctx_with_cbu);
        assert!(rendered_with_cbu.source.contains("cbu-456"));
    }
}
