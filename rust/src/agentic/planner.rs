//! Requirement Planner
//!
//! Deterministic Rust code that expands an OnboardingIntent into complete requirements.
//! No AI involved - this is pure business logic.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::agentic::intent::OnboardingIntent;
use crate::agentic::patterns::OnboardingPattern;

/// Complete onboarding plan derived from intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingPlan {
    pub pattern: OnboardingPattern,
    pub cbu: CbuPlan,
    pub entities: Vec<EntityPlan>,
    pub universe: Vec<UniverseEntry>,
    pub ssis: Vec<SsiPlan>,
    pub booking_rules: Vec<BookingRulePlan>,
    pub isdas: Vec<IsdaPlan>,
}

/// CBU creation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuPlan {
    pub name: String,
    pub jurisdiction: String,
    pub client_type: String,
    pub variable: String, // @cbu
}

/// Entity lookup/create plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPlan {
    pub name: String,
    pub action: EntityAction,
    pub variable: String,
}

/// Whether to lookup or create an entity
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntityAction {
    /// Assume exists (counterparties)
    Lookup,
    /// Create new (client entity if needed)
    Create,
}

/// Universe entry (what the client trades)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniverseEntry {
    pub instrument_class: String,
    pub market: Option<String>,
    pub currencies: Vec<String>,
    pub settlement_types: Vec<String>,
    pub counterparty_var: Option<String>,
}

/// SSI creation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsiPlan {
    pub name: String,
    pub ssi_type: String,
    pub market: Option<String>,
    pub currency: String,
    pub variable: String,
}

/// Booking rule plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingRulePlan {
    pub name: String,
    pub priority: u32,
    pub instrument_class: Option<String>,
    pub market: Option<String>,
    pub currency: Option<String>,
    pub settlement_type: Option<String>,
    pub counterparty_var: Option<String>,
    pub ssi_variable: String,
}

/// ISDA agreement plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsdaPlan {
    pub counterparty_var: String,
    pub counterparty_name: String,
    pub governing_law: String,
    pub variable: String,
    pub coverages: Vec<String>,
    pub csa: Option<CsaPlan>,
}

/// CSA (Credit Support Annex) plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsaPlan {
    pub csa_type: String, // VM or IM
    pub variable: String,
}

/// Requirement planner - deterministic business logic
pub struct RequirementPlanner;

impl RequirementPlanner {
    /// Plan complete onboarding from intent
    pub fn plan(intent: &OnboardingIntent) -> OnboardingPlan {
        let pattern = intent.classify();

        let cbu = Self::plan_cbu(intent);
        let entities = Self::plan_entities(intent);
        let universe = Self::derive_universe(intent);
        let ssis = Self::derive_ssis(&universe);
        let booking_rules = Self::derive_booking_rules(&universe, &ssis);
        let isdas = Self::plan_isdas(intent);

        OnboardingPlan {
            pattern,
            cbu,
            entities,
            universe,
            ssis,
            booking_rules,
            isdas,
        }
    }

    fn plan_cbu(intent: &OnboardingIntent) -> CbuPlan {
        CbuPlan {
            name: intent.client.name.clone(),
            jurisdiction: intent
                .client
                .jurisdiction
                .clone()
                .unwrap_or_else(|| "US".to_string()),
            client_type: intent
                .client
                .entity_type
                .clone()
                .map(|t| t.to_uppercase())
                .unwrap_or_else(|| "FUND".to_string()),
            variable: "cbu".to_string(),
        }
    }

    fn plan_entities(intent: &OnboardingIntent) -> Vec<EntityPlan> {
        intent
            .otc_counterparties
            .iter()
            .map(|cp| EntityPlan {
                name: cp.name.clone(),
                action: EntityAction::Lookup,
                variable: Self::entity_variable(&cp.name),
            })
            .collect()
    }

    fn derive_universe(intent: &OnboardingIntent) -> Vec<UniverseEntry> {
        let mut entries = Vec::new();

        // Cash instruments: market × instrument × currencies
        for market in &intent.markets {
            for instrument in &intent.instruments {
                if !instrument.class.starts_with("OTC_") {
                    entries.push(UniverseEntry {
                        instrument_class: instrument.class.clone(),
                        market: Some(market.market_code.clone()),
                        currencies: market.currencies.clone(),
                        settlement_types: if market.settlement_types.is_empty() {
                            vec!["DVP".to_string()]
                        } else {
                            market.settlement_types.clone()
                        },
                        counterparty_var: None,
                    });
                }
            }
        }

        // OTC instruments: per counterparty
        for cp in &intent.otc_counterparties {
            for instr in &cp.instruments {
                // Collect all currencies from all markets for OTC
                let currencies: Vec<String> = if intent.markets.is_empty() {
                    vec!["USD".to_string()]
                } else {
                    let mut all_currencies: Vec<String> = intent
                        .markets
                        .iter()
                        .flat_map(|m| m.currencies.iter().cloned())
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect();
                    all_currencies.sort();
                    if all_currencies.is_empty() {
                        vec!["USD".to_string()]
                    } else {
                        all_currencies
                    }
                };

                entries.push(UniverseEntry {
                    instrument_class: instr.clone(),
                    market: None,
                    currencies,
                    settlement_types: vec!["DVP".to_string()],
                    counterparty_var: Some(Self::entity_variable(&cp.name)),
                });
            }
        }

        entries
    }

    fn derive_ssis(universe: &[UniverseEntry]) -> Vec<SsiPlan> {
        let mut ssis = Vec::new();
        let mut seen_routes: HashSet<(Option<String>, String)> = HashSet::new();

        for entry in universe {
            for currency in &entry.currencies {
                let route = (entry.market.clone(), currency.clone());
                if seen_routes.insert(route.clone()) {
                    let name = match &entry.market {
                        Some(m) => format!("{} {}", m, currency),
                        None => format!("{} Primary", currency),
                    };

                    ssis.push(SsiPlan {
                        name: name.clone(),
                        ssi_type: "SECURITIES".to_string(),
                        market: entry.market.clone(),
                        currency: currency.clone(),
                        variable: Self::ssi_variable(&name),
                    });
                }
            }
        }

        // Add collateral SSI if OTC present
        let has_otc = universe.iter().any(|e| e.counterparty_var.is_some());
        if has_otc {
            ssis.push(SsiPlan {
                name: "Collateral".to_string(),
                ssi_type: "COLLATERAL".to_string(),
                market: None,
                currency: "USD".to_string(),
                variable: "ssi-collateral".to_string(),
            });
        }

        ssis
    }

    fn derive_booking_rules(universe: &[UniverseEntry], ssis: &[SsiPlan]) -> Vec<BookingRulePlan> {
        let mut rules = Vec::new();
        let mut priority = 10u32;

        // Specific rules for each universe entry
        for entry in universe {
            for currency in &entry.currencies {
                let ssi_var = Self::find_ssi_variable(entry.market.as_deref(), currency, ssis);

                let name = match (&entry.market, &entry.counterparty_var) {
                    (Some(m), _) => format!(
                        "{} {} {} {}",
                        entry.instrument_class,
                        m,
                        currency,
                        entry.settlement_types.first().unwrap_or(&"DVP".to_string())
                    ),
                    (None, Some(cp)) => format!("{} {} {}", entry.instrument_class, cp, currency),
                    (None, None) => format!("{} {}", entry.instrument_class, currency),
                };

                rules.push(BookingRulePlan {
                    name,
                    priority,
                    instrument_class: Some(entry.instrument_class.clone()),
                    market: entry.market.clone(),
                    currency: Some(currency.clone()),
                    settlement_type: entry.settlement_types.first().cloned(),
                    counterparty_var: entry.counterparty_var.clone(),
                    ssi_variable: ssi_var,
                });

                priority += 5;
            }
        }

        // Currency fallback rules (priority 50+)
        let currencies: HashSet<_> = universe.iter().flat_map(|e| e.currencies.iter()).collect();

        for (i, currency) in currencies.iter().enumerate() {
            let ssi_var = Self::find_ssi_variable(None, currency, ssis);
            rules.push(BookingRulePlan {
                name: format!("{} Fallback", currency),
                priority: 50 + i as u32,
                instrument_class: None,
                market: None,
                currency: Some((*currency).clone()),
                settlement_type: None,
                counterparty_var: None,
                ssi_variable: ssi_var,
            });
        }

        // Ultimate fallback (priority 100)
        let default_ssi = ssis
            .first()
            .map(|s| s.variable.clone())
            .unwrap_or_else(|| "ssi-default".to_string());
        rules.push(BookingRulePlan {
            name: "Ultimate Fallback".to_string(),
            priority: 100,
            instrument_class: None,
            market: None,
            currency: None,
            settlement_type: None,
            counterparty_var: None,
            ssi_variable: default_ssi,
        });

        rules
    }

    fn plan_isdas(intent: &OnboardingIntent) -> Vec<IsdaPlan> {
        intent
            .otc_counterparties
            .iter()
            .map(|cp| {
                let variable = format!("isda-{}", Self::entity_variable(&cp.name));
                IsdaPlan {
                    counterparty_var: Self::entity_variable(&cp.name),
                    counterparty_name: cp.name.clone(),
                    governing_law: cp.governing_law.clone().unwrap_or_else(|| "NY".to_string()),
                    variable: variable.clone(),
                    coverages: cp.instruments.clone(),
                    csa: if cp.csa_required {
                        Some(CsaPlan {
                            csa_type: "VM".to_string(),
                            variable: format!("csa-{}", Self::entity_variable(&cp.name)),
                        })
                    } else {
                        None
                    },
                }
            })
            .collect()
    }

    // Helper functions

    fn entity_variable(name: &str) -> String {
        name.to_lowercase()
            .split_whitespace()
            .next()
            .unwrap_or("entity")
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }

    fn ssi_variable(name: &str) -> String {
        format!("ssi-{}", name.to_lowercase().replace(' ', "-"))
    }

    fn find_ssi_variable(market: Option<&str>, currency: &str, ssis: &[SsiPlan]) -> String {
        // Try exact match first
        for ssi in ssis {
            if ssi.market.as_deref() == market && ssi.currency == currency {
                return ssi.variable.clone();
            }
        }
        // Fall back to currency match
        for ssi in ssis {
            if ssi.currency == currency {
                return ssi.variable.clone();
            }
        }
        // Last resort
        ssis.first()
            .map(|s| s.variable.clone())
            .unwrap_or_else(|| "ssi-default".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::intent::*;

    fn simple_intent() -> OnboardingIntent {
        OnboardingIntent {
            client: ClientIntent {
                name: "Apex Fund".to_string(),
                entity_type: Some("fund".to_string()),
                jurisdiction: Some("US".to_string()),
            },
            instruments: vec![InstrumentIntent {
                class: "EQUITY".to_string(),
                specific_types: vec![],
            }],
            markets: vec![MarketIntent {
                market_code: "XNYS".to_string(),
                currencies: vec!["USD".to_string()],
                settlement_types: vec!["DVP".to_string()],
            }],
            otc_counterparties: vec![],
            explicit_requirements: vec![],
            original_request: "Set up Apex Fund for US equities".to_string(),
        }
    }

    #[test]
    fn test_simple_equity_plan() {
        let intent = simple_intent();
        let plan = RequirementPlanner::plan(&intent);

        assert_eq!(plan.pattern, OnboardingPattern::SimpleEquity);
        assert_eq!(plan.cbu.name, "Apex Fund");
        assert_eq!(plan.cbu.jurisdiction, "US");
        assert_eq!(plan.cbu.client_type, "FUND");

        // Should have 1 universe entry
        assert_eq!(plan.universe.len(), 1);
        assert_eq!(plan.universe[0].instrument_class, "EQUITY");
        assert_eq!(plan.universe[0].market, Some("XNYS".to_string()));

        // Should have at least 1 SSI
        assert!(!plan.ssis.is_empty());

        // Should have specific rule + currency fallback + ultimate fallback
        assert!(plan.booking_rules.len() >= 3);

        // No ISDA for simple equity
        assert!(plan.isdas.is_empty());
    }

    #[test]
    fn test_multi_market_plan() {
        let mut intent = simple_intent();
        intent.markets.push(MarketIntent {
            market_code: "XLON".to_string(),
            currencies: vec!["GBP".to_string(), "USD".to_string()],
            settlement_types: vec!["DVP".to_string()],
        });

        let plan = RequirementPlanner::plan(&intent);

        assert_eq!(plan.pattern, OnboardingPattern::MultiMarket);

        // Should have 2 universe entries (one per market)
        assert_eq!(plan.universe.len(), 2);

        // Should have SSIs for: XNYS/USD, XLON/GBP, XLON/USD
        assert!(plan.ssis.len() >= 2);
    }

    #[test]
    fn test_otc_plan_has_isda() {
        let mut intent = simple_intent();
        intent.instruments.push(InstrumentIntent {
            class: "OTC_IRS".to_string(),
            specific_types: vec![],
        });
        intent.otc_counterparties.push(CounterpartyIntent {
            name: "Morgan Stanley".to_string(),
            instruments: vec!["OTC_IRS".to_string()],
            governing_law: Some("NY".to_string()),
            csa_required: true,
        });

        let plan = RequirementPlanner::plan(&intent);

        assert_eq!(plan.pattern, OnboardingPattern::WithOtc);

        // Should have entity for Morgan Stanley
        assert_eq!(plan.entities.len(), 1);
        assert_eq!(plan.entities[0].name, "Morgan Stanley");

        // Should have ISDA
        assert_eq!(plan.isdas.len(), 1);
        assert_eq!(plan.isdas[0].counterparty_name, "Morgan Stanley");
        assert_eq!(plan.isdas[0].governing_law, "NY");
        assert!(plan.isdas[0].csa.is_some());

        // Should have collateral SSI
        assert!(plan.ssis.iter().any(|s| s.ssi_type == "COLLATERAL"));

        // Universe should include OTC entry
        assert!(plan
            .universe
            .iter()
            .any(|u| u.instrument_class == "OTC_IRS"));
    }

    #[test]
    fn test_entity_variable() {
        assert_eq!(
            RequirementPlanner::entity_variable("Morgan Stanley"),
            "morgan"
        );
        assert_eq!(
            RequirementPlanner::entity_variable("Goldman Sachs"),
            "goldman"
        );
        assert_eq!(RequirementPlanner::entity_variable("UBS"), "ubs");
    }

    #[test]
    fn test_ssi_variable() {
        assert_eq!(
            RequirementPlanner::ssi_variable("US Primary"),
            "ssi-us-primary"
        );
        assert_eq!(RequirementPlanner::ssi_variable("XNYS USD"), "ssi-xnys-usd");
    }
}
