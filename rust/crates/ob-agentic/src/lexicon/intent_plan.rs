//! Intent Plan - Intermediate representation between IntentAst and DSL.
//!
//! The Plan is a normalized, verb-agnostic representation of user intent.
//! It consists of:
//! - A semantic action (what the user wants to do)
//! - Slots filled with values (the arguments)
//!
//! This separation allows:
//! 1. IntentAst → Plan: normalize surface variations to canonical form
//! 2. Plan → Verb matching: find verb(s) that can fulfill the plan
//! 3. Plan + Verb → DSL: render the actual s-expression
//!
//! ## Example
//!
//! User: "add Goldman Sachs as counterparty for IRS under NY law"
//!
//! IntentAst::CounterpartyCreate {
//!     counterparty: "Goldman Sachs",
//!     instruments: [IRS],
//!     governing_law: Some(NY),
//! }
//!
//! ↓ to_plan()
//!
//! Plan {
//!     action: SemanticAction::CreateCounterparty,
//!     slots: {
//!         "counterparty" → Entity("Goldman Sachs"),
//!         "instruments" → List([Instrument("IRS")]),
//!         "governing_law" → Law("NY_LAW"),
//!     }
//! }
//!
//! ↓ match_verbs() + render()
//!
//! (entity.ensure-limited-company :name "Goldman Sachs" :as @counterparty)
//! (trading-profile.add-instrument-class :profile-id @cbu :class-code "IRS")
//! (isda.create :cbu-id @cbu :counterparty @counterparty :governing-law "NY_LAW" :as @isda)

use std::collections::HashMap;

use super::intent_ast::{
    CsaType, CurrencyCode, EntityRef, GoverningLaw, InstrumentCode, IntentAst, MarketCode, RoleCode,
};

/// Semantic action - what the user wants to accomplish.
/// This is higher-level than verb names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SemanticAction {
    /// Create/onboard a counterparty
    CreateCounterparty,
    /// Establish ISDA master agreement
    EstablishIsda,
    /// Add CSA to existing ISDA
    AddCsa,
    /// Assign role to entity
    AssignRole,
    /// Remove role from entity
    RemoveRole,
    /// Add to trading universe (markets, instruments)
    AddToUniverse,
    /// Create an entity
    CreateEntity,
    /// Add a product
    AddProduct,
    /// Provision a service
    ProvisionService,
    /// List entities
    ListEntities,
    /// Show entity details
    ShowEntity,
    /// List counterparties
    ListCounterparties,
    /// Show ISDA details
    ShowIsda,
    /// Unknown/unrecognized action
    Unknown,
}

/// Slot value - typed value for a plan slot.
#[derive(Debug, Clone)]
pub enum SlotValue {
    /// Entity reference (resolved or unresolved)
    Entity(EntityRef),
    /// String value
    String(String),
    /// List of values
    List(Vec<SlotValue>),
    /// Instrument code
    Instrument(InstrumentCode),
    /// Market code
    Market(MarketCode),
    /// Currency code
    Currency(CurrencyCode),
    /// Governing law
    Law(GoverningLaw),
    /// CSA type
    CsaType(CsaType),
    /// Role code
    Role(RoleCode),
    /// Boolean flag
    Bool(bool),
    /// Symbol reference (e.g., @cbu, @counterparty)
    Symbol(String),
    /// Null/not provided
    Null,
}

impl SlotValue {
    /// Convert to DSL value string.
    pub fn to_dsl(&self) -> String {
        match self {
            SlotValue::Entity(e) => match e.id() {
                Some(id) => format!("\"{}\"", id),
                None => format!("\"{}\"", e.name()),
            },
            SlotValue::String(s) => format!("\"{}\"", s),
            SlotValue::List(items) => {
                let rendered: Vec<_> = items.iter().map(|v| v.to_dsl()).collect();
                format!("[{}]", rendered.join(" "))
            }
            SlotValue::Instrument(i) => format!("\"{}\"", i.as_str()),
            SlotValue::Market(m) => format!("\"{}\"", m.as_str()),
            SlotValue::Currency(c) => format!("\"{}\"", c.as_str()),
            SlotValue::Law(l) => format!("\"{}\"", l.code()),
            SlotValue::CsaType(c) => format!("\"{}\"", c.code()),
            SlotValue::Role(r) => format!("\"{}\"", r.as_str()),
            SlotValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            SlotValue::Symbol(s) => s.clone(),
            SlotValue::Null => "nil".to_string(),
        }
    }

    /// Check if this is a resolved entity.
    pub fn is_resolved(&self) -> bool {
        match self {
            SlotValue::Entity(e) => e.is_resolved(),
            SlotValue::List(items) => items.iter().all(|v| v.is_resolved()),
            _ => true,
        }
    }
}

/// Execution plan - normalized representation of user intent.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The semantic action to perform.
    pub action: SemanticAction,
    /// Named slots with values.
    pub slots: HashMap<String, SlotValue>,
    /// Context slots (inherited from session, e.g., active CBU).
    pub context: HashMap<String, SlotValue>,
}

impl Plan {
    /// Create a new plan with the given action.
    pub fn new(action: SemanticAction) -> Self {
        Self {
            action,
            slots: HashMap::new(),
            context: HashMap::new(),
        }
    }

    /// Set a slot value.
    pub fn set(&mut self, name: impl Into<String>, value: SlotValue) -> &mut Self {
        self.slots.insert(name.into(), value);
        self
    }

    /// Set a context value.
    pub fn set_context(&mut self, name: impl Into<String>, value: SlotValue) -> &mut Self {
        self.context.insert(name.into(), value);
        self
    }

    /// Get a slot value.
    pub fn get(&self, name: &str) -> Option<&SlotValue> {
        self.slots.get(name).or_else(|| self.context.get(name))
    }

    /// Get slot as entity reference.
    pub fn get_entity(&self, name: &str) -> Option<&EntityRef> {
        match self.get(name)? {
            SlotValue::Entity(e) => Some(e),
            _ => None,
        }
    }

    /// Get slot as string.
    pub fn get_string(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            SlotValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Find all unresolved entity references.
    pub fn unresolved_entities(&self) -> Vec<(String, String)> {
        let mut unresolved = Vec::new();

        for (name, value) in &self.slots {
            if let SlotValue::Entity(e) = value {
                if !e.is_resolved() {
                    unresolved.push((name.clone(), e.name().to_string()));
                }
            }
        }

        unresolved
    }
}

/// Convert IntentAst to Plan.
pub fn intent_to_plan(intent: &IntentAst) -> Plan {
    match intent {
        IntentAst::CounterpartyCreate {
            counterparty,
            instruments,
            governing_law,
        } => {
            let mut plan = Plan::new(SemanticAction::CreateCounterparty);
            plan.set("counterparty", SlotValue::Entity(counterparty.clone()));

            if !instruments.is_empty() {
                plan.set(
                    "instruments",
                    SlotValue::List(
                        instruments
                            .iter()
                            .map(|i| SlotValue::Instrument(i.clone()))
                            .collect(),
                    ),
                );
            }

            if let Some(law) = governing_law {
                plan.set("governing_law", SlotValue::Law(*law));
            }

            plan
        }

        IntentAst::IsdaEstablish {
            counterparty,
            governing_law,
            instruments,
        } => {
            let mut plan = Plan::new(SemanticAction::EstablishIsda);
            plan.set("counterparty", SlotValue::Entity(counterparty.clone()));
            plan.set("governing_law", SlotValue::Law(*governing_law));

            if !instruments.is_empty() {
                plan.set(
                    "instruments",
                    SlotValue::List(
                        instruments
                            .iter()
                            .map(|i| SlotValue::Instrument(i.clone()))
                            .collect(),
                    ),
                );
            }

            plan
        }

        IntentAst::CsaAdd {
            counterparty,
            csa_type,
            currency,
        } => {
            let mut plan = Plan::new(SemanticAction::AddCsa);
            plan.set("counterparty", SlotValue::Entity(counterparty.clone()));
            plan.set("csa_type", SlotValue::CsaType(*csa_type));

            if let Some(curr) = currency {
                plan.set("currency", SlotValue::Currency(curr.clone()));
            }

            plan
        }

        IntentAst::RoleAssign { cbu, entity, role } => {
            let mut plan = Plan::new(SemanticAction::AssignRole);
            plan.set("cbu", SlotValue::Entity(cbu.clone()));
            plan.set("entity", SlotValue::Entity(entity.clone()));
            plan.set("role", SlotValue::Role(role.clone()));
            plan
        }

        IntentAst::RoleRemove { cbu, entity, role } => {
            let mut plan = Plan::new(SemanticAction::RemoveRole);
            plan.set("cbu", SlotValue::Entity(cbu.clone()));
            plan.set("entity", SlotValue::Entity(entity.clone()));
            plan.set("role", SlotValue::Role(role.clone()));
            plan
        }

        IntentAst::UniverseAdd {
            cbu,
            markets,
            instruments,
            currencies,
        } => {
            let mut plan = Plan::new(SemanticAction::AddToUniverse);
            plan.set("cbu", SlotValue::Entity(cbu.clone()));

            if !markets.is_empty() {
                plan.set(
                    "markets",
                    SlotValue::List(
                        markets
                            .iter()
                            .map(|m| SlotValue::Market(m.clone()))
                            .collect(),
                    ),
                );
            }

            if !instruments.is_empty() {
                plan.set(
                    "instruments",
                    SlotValue::List(
                        instruments
                            .iter()
                            .map(|i| SlotValue::Instrument(i.clone()))
                            .collect(),
                    ),
                );
            }

            if !currencies.is_empty() {
                plan.set(
                    "currencies",
                    SlotValue::List(
                        currencies
                            .iter()
                            .map(|c| SlotValue::Currency(c.clone()))
                            .collect(),
                    ),
                );
            }

            plan
        }

        IntentAst::EntityList {
            entity_type,
            filters,
        } => {
            let mut plan = Plan::new(SemanticAction::ListEntities);
            if let Some(et) = entity_type {
                plan.set("entity_type", SlotValue::String(et.clone()));
            }
            if !filters.is_empty() {
                // Filters are (key, value) pairs
                plan.set(
                    "filters",
                    SlotValue::List(
                        filters
                            .iter()
                            .map(|(k, v)| SlotValue::String(format!("{}={}", k, v)))
                            .collect(),
                    ),
                );
            }
            plan
        }

        IntentAst::EntityShow { entity } => {
            let mut plan = Plan::new(SemanticAction::ShowEntity);
            plan.set("entity", SlotValue::Entity(entity.clone()));
            plan
        }

        IntentAst::CounterpartyList { cbu } => {
            let mut plan = Plan::new(SemanticAction::ListCounterparties);
            if let Some(c) = cbu {
                plan.set("cbu", SlotValue::Entity(c.clone()));
            }
            plan
        }

        IntentAst::IsdaShow { counterparty } => {
            let mut plan = Plan::new(SemanticAction::ShowIsda);
            if let Some(cp) = counterparty {
                plan.set("counterparty", SlotValue::Entity(cp.clone()));
            }
            plan
        }

        IntentAst::Unknown { raw_text, .. } => {
            let mut plan = Plan::new(SemanticAction::Unknown);
            plan.set("raw_text", SlotValue::String(raw_text.clone()));
            plan
        }

        // Handle remaining variants with Unknown action
        _ => Plan::new(SemanticAction::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counterparty_create_to_plan() {
        let intent = IntentAst::CounterpartyCreate {
            counterparty: EntityRef::Unresolved {
                name: "Goldman Sachs".to_string(),
                entity_type: Some("counterparty".to_string()),
            },
            instruments: vec![InstrumentCode::new("IRS")],
            governing_law: Some(GoverningLaw::NewYork),
        };

        let plan = intent_to_plan(&intent);

        assert_eq!(plan.action, SemanticAction::CreateCounterparty);
        assert!(plan.get_entity("counterparty").is_some());
        assert_eq!(
            plan.get_entity("counterparty").unwrap().name(),
            "Goldman Sachs"
        );
    }

    #[test]
    fn test_plan_unresolved_entities() {
        let intent = IntentAst::RoleAssign {
            cbu: EntityRef::Pronoun {
                text: "it".to_string(),
                referent: None,
            },
            entity: EntityRef::Unresolved {
                name: "John Smith".to_string(),
                entity_type: Some("person".to_string()),
            },
            role: RoleCode::new("DIRECTOR"),
        };

        let plan = intent_to_plan(&intent);
        let unresolved = plan.unresolved_entities();

        assert_eq!(unresolved.len(), 2);
    }

    #[test]
    fn test_slot_value_to_dsl() {
        assert_eq!(SlotValue::String("test".to_string()).to_dsl(), "\"test\"");
        assert_eq!(SlotValue::Symbol("@cbu".to_string()).to_dsl(), "@cbu");
        assert_eq!(SlotValue::Bool(true).to_dsl(), "true");
        assert_eq!(SlotValue::Law(GoverningLaw::NewYork).to_dsl(), "\"NY_LAW\"");
    }
}
