//! Lexicon-based agent pipeline.
//!
//! This pipeline replaces the regex-based intent classifier with the
//! formal tokenizer + nom grammar parser approach.
//!
//! ## Pipeline Flow
//!
//! ```text
//! User Input
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Tokenizer                                   │
//! │  - Lexicon lookup (verbs, roles, instruments, prepositions)     │
//! │  - EntityGateway lookup (counterparties, CBUs, persons)         │
//! │  - Session context (coreference resolution)                     │
//! └─────────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! Token Stream
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Nom Grammar Parser                            │
//! │  - Matches token patterns to intent structures                  │
//! │  - Builds typed IntentAst nodes                                 │
//! │  - Handles domain detection (OTC vs Exchange-Traded)            │
//! └─────────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! IntentAst
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    DSL Generator                                 │
//! │  - Converts IntentAst to DSL source code                        │
//! │  - Resolves entity references to UUIDs                          │
//! │  - Generates symbol bindings                                    │
//! └─────────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! DSL Source → Validation → Execution
//! ```

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::intent_ast::{EntityRef, IntentAst};
use super::intent_parser::parse_tokens;
use super::loader::{Lexicon, LifecycleDomain};
use super::tokenizer::{EntityResolver, SessionSalience, Tokenizer};

/// Result of processing a user message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconPipelineResult {
    /// The parsed intent (serialized).
    pub intent_type: String,

    /// The detected lifecycle domain.
    pub domain: Option<String>,

    /// Generated DSL source code.
    pub dsl: Option<String>,

    /// Unresolved entity references that need clarification.
    pub unresolved_entities: Vec<String>,

    /// Whether the intent requires confirmation before execution.
    pub needs_confirmation: bool,

    /// Human-readable description of what will happen.
    pub description: String,

    /// Validation errors if any.
    pub errors: Vec<String>,
}

/// The lexicon-based agent pipeline.
pub struct LexiconPipeline {
    /// The tokenizer with lexicon.
    tokenizer: Tokenizer,

    /// Current session salience (for coreference resolution).
    salience: SessionSalience,

    /// Active CBU context.
    active_cbu: Option<ActiveCbu>,
}

/// Active CBU context.
#[derive(Debug, Clone)]
#[allow(dead_code)] // name used for display/logging in future
pub struct ActiveCbu {
    pub id: String,
    pub name: String,
}

impl LexiconPipeline {
    /// Create a new pipeline with the given lexicon.
    pub fn new(lexicon: Arc<Lexicon>) -> Self {
        Self {
            tokenizer: Tokenizer::new(lexicon),
            salience: SessionSalience::default(),
            active_cbu: None,
        }
    }

    /// Create a pipeline with entity resolver.
    pub fn with_entity_resolver(mut self, resolver: Arc<dyn EntityResolver>) -> Self {
        self.tokenizer = self.tokenizer.with_entity_resolver(resolver);
        self
    }

    /// Load lexicon from YAML file.
    pub fn from_lexicon_file(path: &Path) -> Result<Self> {
        let lexicon = Lexicon::load_from_file(path)?;
        Ok(Self::new(Arc::new(lexicon)))
    }

    /// Set the active CBU context.
    pub fn set_active_cbu(&mut self, id: String, name: String) {
        // Add to salience first (needs clones)
        self.salience.current_cbu = Some(super::tokenizer::SalientEntity {
            id: id.clone(),
            name: name.clone(),
            entity_type: "cbu".to_string(),
            mention_count: 1,
        });

        // Then set active_cbu (consumes originals)
        self.active_cbu = Some(ActiveCbu { id, name });
    }

    /// Clear the active CBU context.
    pub fn clear_active_cbu(&mut self) {
        self.active_cbu = None;
        self.salience.current_cbu = None;
    }

    /// Process a user message through the pipeline.
    pub async fn process(&mut self, message: &str) -> LexiconPipelineResult {
        // Step 1: Tokenize with session context
        let tokenizer = Tokenizer::new(self.tokenizer.lexicon().clone().into())
            .with_salience(self.salience.clone());

        let tokens = tokenizer.tokenize(message).await;

        // Step 2: Detect domain from tokens
        let domain = tokenizer.detect_domain(&tokens);

        // Step 3: Parse tokens into IntentAst
        let intent = match parse_tokens(&tokens) {
            Ok(intent) => intent,
            Err(e) => {
                return LexiconPipelineResult {
                    intent_type: "unknown".to_string(),
                    domain: domain.map(domain_to_string),
                    dsl: None,
                    unresolved_entities: vec![],
                    needs_confirmation: false,
                    description: format!("Could not parse intent: {}", e),
                    errors: vec![e],
                };
            }
        };

        // Step 4: Check for unresolved entities
        let unresolved = self.find_unresolved_entities(&intent);

        // Step 5: Generate DSL (even for unresolved - uses placeholder names)
        let dsl = self.generate_dsl(&intent);
        let errors = if unresolved.is_empty() {
            vec![]
        } else {
            vec![format!("Unresolved entities: {}", unresolved.join(", "))]
        };

        // Step 6: Build description
        let description = self.describe_intent(&intent);

        // Step 7: Determine if confirmation is needed
        let needs_confirmation = self.requires_confirmation(&intent);

        // Step 8: Update salience with resolved entities
        self.update_salience(&intent);

        LexiconPipelineResult {
            intent_type: intent_type_name(&intent),
            domain: domain.map(domain_to_string),
            dsl: Some(dsl),
            unresolved_entities: unresolved,
            needs_confirmation,
            description,
            errors,
        }
    }

    /// Find unresolved entity references in the intent.
    fn find_unresolved_entities(&self, intent: &IntentAst) -> Vec<String> {
        let mut unresolved = Vec::new();

        match intent {
            IntentAst::CounterpartyCreate { counterparty, .. } => {
                if !counterparty.is_resolved() {
                    unresolved.push(format!("counterparty: {}", counterparty.name()));
                }
            }
            IntentAst::IsdaEstablish { counterparty, .. } => {
                if !counterparty.is_resolved() {
                    unresolved.push(format!("counterparty: {}", counterparty.name()));
                }
            }
            IntentAst::CsaAdd { counterparty, .. } => {
                if !counterparty.is_resolved() {
                    unresolved.push(format!("counterparty: {}", counterparty.name()));
                }
            }
            IntentAst::RoleAssign { cbu, entity, .. } => {
                if !cbu.is_resolved() && self.active_cbu.is_none() {
                    unresolved.push(format!("cbu: {}", cbu.name()));
                }
                if !entity.is_resolved() {
                    unresolved.push(format!("entity: {}", entity.name()));
                }
            }
            IntentAst::UniverseAdd { cbu, .. } => {
                if !cbu.is_resolved() && self.active_cbu.is_none() {
                    unresolved.push(format!("cbu: {}", cbu.name()));
                }
            }
            _ => {}
        }

        unresolved
    }

    /// Generate DSL from an IntentAst.
    fn generate_dsl(&self, intent: &IntentAst) -> String {
        match intent {
            IntentAst::CounterpartyCreate {
                counterparty,
                instruments,
                governing_law,
            } => {
                let _cp_id = self.resolve_entity_id(counterparty); // TODO: inject into DSL when resolved
                let mut dsl = format!(
                    "(entity.ensure-limited-company :name \"{}\" :as @counterparty)",
                    counterparty.name()
                );

                if !instruments.is_empty() {
                    let cbu_id = self.get_cbu_id();
                    for inst in instruments {
                        dsl.push_str(&format!(
                            "\n(trading-profile.add-instrument-class :profile-id {} :class-code \"{}\")",
                            cbu_id,
                            inst.as_str()
                        ));
                    }
                }

                if let Some(law) = governing_law {
                    // isda.create requires :cbu-id, :counterparty, :agreement-date, :effective-date, :governing-law
                    dsl.push_str(&format!(
                        "\n(isda.create :cbu-id {} :counterparty @counterparty :agreement-date \"today\" :effective-date \"today\" :governing-law \"{}\" :as @isda)",
                        self.get_cbu_id(),
                        law.code()
                    ));
                }

                dsl
            }

            IntentAst::IsdaEstablish {
                counterparty,
                governing_law,
                instruments,
            } => {
                let cbu_id = self.get_cbu_id();
                let cp_id = self.resolve_entity_id(counterparty);

                // isda.create requires :cbu-id, :counterparty, :agreement-date, :effective-date, :governing-law
                let mut dsl = format!(
                    "(isda.create :cbu-id {} :counterparty {} :agreement-date \"today\" :effective-date \"today\" :governing-law \"{}\" :as @isda)",
                    cbu_id, cp_id, governing_law.code()
                );

                for inst in instruments {
                    dsl.push_str(&format!(
                        "\n(isda.add-coverage :isda-id @isda :instrument-class \"{}\")",
                        inst.as_str()
                    ));
                }

                dsl
            }

            IntentAst::CsaAdd {
                counterparty,
                csa_type,
                currency,
            } => {
                // CSA requires an existing ISDA. We reference it via @isda symbol or need to look it up.
                // isda.add-csa requires :isda-id, :csa-type, :effective-date
                let _cp_id = self.resolve_entity_id(counterparty);

                let mut dsl = format!(
                    "(isda.add-csa :isda-id @isda :csa-type \"{}\" :effective-date \"today\"",
                    csa_type.code()
                );

                if let Some(curr) = currency {
                    dsl.push_str(&format!(" :threshold-currency \"{}\"", curr.as_str()));
                }

                dsl.push_str(" :as @csa)");
                dsl
            }

            IntentAst::RoleAssign { cbu, entity, role } => {
                let cbu_id = if cbu.is_resolved() {
                    self.resolve_entity_id(cbu)
                } else {
                    self.get_cbu_id()
                };
                let entity_id = self.resolve_entity_id(entity);

                format!(
                    "(cbu.assign-role :cbu-id {} :entity-id {} :role \"{}\")",
                    cbu_id,
                    entity_id,
                    role.as_str()
                )
            }

            IntentAst::UniverseAdd {
                cbu,
                markets,
                instruments,
                currencies,
            } => {
                let cbu_id = if cbu.is_resolved() {
                    self.resolve_entity_id(cbu)
                } else {
                    self.get_cbu_id()
                };

                let mut dsl = String::new();

                for market in markets {
                    let insts = if instruments.is_empty() {
                        vec!["EQUITY".to_string()]
                    } else {
                        instruments.iter().map(|i| i.as_str().to_string()).collect()
                    };

                    let currs = if currencies.is_empty() {
                        vec!["USD".to_string()]
                    } else {
                        currencies.iter().map(|c| c.as_str().to_string()).collect()
                    };

                    for _inst in &insts {
                        if !dsl.is_empty() {
                            dsl.push('\n');
                        }
                        dsl.push_str(&format!(
                            "(trading-profile.add-market :profile-id {} :market-code \"{}\" :currencies [{}])",
                            cbu_id,
                            market.as_str(),
                            currs.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(" ")
                        ));
                    }
                }

                dsl
            }

            IntentAst::EntityList { entity_type, .. } => {
                let et = entity_type.as_deref().unwrap_or("entity");
                format!("({}.list)", et)
            }

            IntentAst::EntityShow { entity } => {
                let entity_id = self.resolve_entity_id(entity);
                format!("(entity.read :id {})", entity_id)
            }

            IntentAst::CounterpartyList { cbu } => {
                let cbu_id = cbu
                    .as_ref()
                    .map(|c| self.resolve_entity_id(c))
                    .unwrap_or_else(|| self.get_cbu_id());
                format!("(cbu.parties :cbu-id {})", cbu_id)
            }

            IntentAst::IsdaShow { counterparty } => {
                let cp_id = self.resolve_entity_id(counterparty);
                format!("(isda.list :counterparty-id {})", cp_id)
            }

            IntentAst::Unknown { raw_text, .. } => {
                format!("; Could not parse: {}", raw_text)
            }

            _ => "; Intent not yet supported for DSL generation".to_string(),
        }
    }

    /// Resolve an entity reference to an ID string for DSL.
    fn resolve_entity_id(&self, entity: &EntityRef) -> String {
        match entity.id() {
            Some(id) => format!("\"{}\"", id),
            None => format!("\"{}\"", entity.name()), // Use name as placeholder
        }
    }

    /// Get the current CBU ID for DSL.
    fn get_cbu_id(&self) -> String {
        match &self.active_cbu {
            Some(cbu) => format!("\"{}\"", cbu.id),
            None => "@cbu".to_string(), // Symbol reference
        }
    }

    /// Describe what an intent will do.
    fn describe_intent(&self, intent: &IntentAst) -> String {
        match intent {
            IntentAst::CounterpartyCreate {
                counterparty,
                instruments,
                governing_law,
            } => {
                let mut desc = format!("Add {} as a counterparty", counterparty.name());
                if !instruments.is_empty() {
                    desc.push_str(&format!(
                        " for {}",
                        instruments
                            .iter()
                            .map(|i| i.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if let Some(law) = governing_law {
                    desc.push_str(&format!(" under {} law", law.code()));
                }
                desc
            }

            IntentAst::IsdaEstablish {
                counterparty,
                governing_law,
                ..
            } => {
                format!(
                    "Establish ISDA master agreement with {} under {} law",
                    counterparty.name(),
                    governing_law.code()
                )
            }

            IntentAst::CsaAdd {
                counterparty,
                csa_type,
                ..
            } => {
                format!(
                    "Add {} CSA to {} ISDA",
                    csa_type.code(),
                    counterparty.name()
                )
            }

            IntentAst::RoleAssign { entity, role, .. } => {
                format!("Assign {} as {}", entity.name(), role.as_str())
            }

            IntentAst::UniverseAdd { markets, .. } => {
                format!(
                    "Add trading universe for {}",
                    markets
                        .iter()
                        .map(|m| m.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }

            IntentAst::Unknown { raw_text, .. } => {
                format!("Unknown intent: {}", raw_text)
            }

            _ => "Intent recognized".to_string(),
        }
    }

    /// Check if an intent requires confirmation before execution.
    fn requires_confirmation(&self, intent: &IntentAst) -> bool {
        matches!(
            intent,
            IntentAst::IsdaEstablish { .. } | IntentAst::RoleRemove { .. }
        )
    }

    /// Update salience with entities from the intent.
    fn update_salience(&mut self, intent: &IntentAst) {
        match intent {
            IntentAst::CounterpartyCreate { counterparty, .. }
            | IntentAst::IsdaEstablish { counterparty, .. }
            | IntentAst::CsaAdd { counterparty, .. } => {
                if let Some(id) = counterparty.id() {
                    self.salience.add_entity(
                        id.to_string(),
                        counterparty.name().to_string(),
                        "counterparty".to_string(),
                    );
                    self.salience.current_counterparty = Some(super::tokenizer::SalientEntity {
                        id: id.to_string(),
                        name: counterparty.name().to_string(),
                        entity_type: "counterparty".to_string(),
                        mention_count: 1,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Convert domain to string.
fn domain_to_string(domain: LifecycleDomain) -> String {
    match domain {
        LifecycleDomain::Otc => "otc".to_string(),
        LifecycleDomain::ExchangeTraded => "exchange_traded".to_string(),
    }
}

/// Get the type name for an intent.
fn intent_type_name(intent: &IntentAst) -> String {
    match intent {
        IntentAst::CounterpartyCreate { .. } => "counterparty_create".to_string(),
        IntentAst::IsdaEstablish { .. } => "isda_establish".to_string(),
        IntentAst::CsaAdd { .. } => "csa_add".to_string(),
        IntentAst::IsdaAddCoverage { .. } => "isda_add_coverage".to_string(),
        IntentAst::UniverseAdd { .. } => "universe_add".to_string(),
        IntentAst::SsiCreate { .. } => "ssi_create".to_string(),
        IntentAst::BookingRuleAdd { .. } => "booking_rule_add".to_string(),
        IntentAst::RoleAssign { .. } => "role_assign".to_string(),
        IntentAst::RoleRemove { .. } => "role_remove".to_string(),
        IntentAst::EntityCreate { .. } => "entity_create".to_string(),
        IntentAst::ProductAdd { .. } => "product_add".to_string(),
        IntentAst::ServiceProvision { .. } => "service_provision".to_string(),
        IntentAst::EntityList { .. } => "entity_list".to_string(),
        IntentAst::EntityShow { .. } => "entity_show".to_string(),
        IntentAst::CounterpartyList { .. } => "counterparty_list".to_string(),
        IntentAst::IsdaShow { .. } => "isda_show".to_string(),
        IntentAst::Unknown { .. } => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::loader::LexiconConfig;
    use super::*;

    fn test_lexicon() -> Arc<Lexicon> {
        let config = LexiconConfig {
            verbs: super::super::loader::VerbsConfig {
                create: vec![
                    "add".to_string(),
                    "create".to_string(),
                    "establish".to_string(),
                ],
                link: vec!["assign".to_string()],
                query: vec!["list".to_string(), "show".to_string()],
                ..Default::default()
            },
            entities: super::super::loader::EntitiesConfig {
                counterparty: vec!["counterparty".to_string()],
                isda: vec!["isda".to_string()],
                csa: vec!["csa".to_string()],
                person: vec!["person".to_string()],
                ..Default::default()
            },
            instruments: super::super::loader::InstrumentsConfig {
                otc: vec!["irs".to_string(), "cds".to_string()],
                exchange_traded: vec!["equity".to_string()],
            },
            roles: vec!["director".to_string(), "manager".to_string()],
            prepositions: super::super::loader::PrepositionsConfig {
                as_: vec!["as".to_string()],
                for_: vec!["for".to_string()],
                under: vec!["under".to_string()],
                with: vec!["with".to_string()],
                to: vec!["to".to_string()],
                ..Default::default()
            },
            laws: vec![super::super::loader::LawEntry {
                code: "NY_LAW".to_string(),
                aliases: vec!["ny law".to_string(), "new york law".to_string()],
            }],
            articles: vec!["a".to_string(), "an".to_string(), "the".to_string()],
            ..Default::default()
        };

        Arc::new(Lexicon::from_config(config).unwrap())
    }

    #[tokio::test]
    async fn test_pipeline_counterparty_create() {
        let lexicon = test_lexicon();
        let mut pipeline = LexiconPipeline::new(lexicon);
        pipeline.set_active_cbu("cbu-123".to_string(), "Test Fund".to_string());

        let result = pipeline.process("add counterparty for irs").await;

        assert_eq!(result.intent_type, "counterparty_create");
        assert_eq!(result.domain, Some("otc".to_string()));
        assert!(result.dsl.is_some());
    }

    #[tokio::test]
    async fn test_pipeline_role_assign() {
        let lexicon = test_lexicon();
        let mut pipeline = LexiconPipeline::new(lexicon);
        pipeline.set_active_cbu("cbu-123".to_string(), "Test Fund".to_string());

        let result = pipeline.process("assign person as director").await;

        assert_eq!(result.intent_type, "role_assign");
        assert!(result.dsl.is_some());
        let dsl = result.dsl.unwrap();
        assert!(dsl.contains("cbu.assign-role"));
        assert!(dsl.contains("DIRECTOR"));
    }

    #[tokio::test]
    async fn test_pipeline_unknown_intent() {
        let lexicon = test_lexicon();
        let mut pipeline = LexiconPipeline::new(lexicon);

        let result = pipeline.process("hello world").await;

        assert_eq!(result.intent_type, "unknown");
    }
}
