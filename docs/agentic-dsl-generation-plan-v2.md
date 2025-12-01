# Agentic DSL Generation Implementation Plan (v2 - Simplified)

**Document**: `agentic-dsl-generation-plan-v2.md`  
**Created**: 2025-12-01  
**Status**: Ready for Implementation  
**Priority**: HIGH - Flagship Feature

## Changes from v1

**Removed**: Vector database / RAG complexity  
**Reason**: Bounded domain (~30 verbs, ~500 lines YAML) fits easily in context. Direct inclusion is more reliable than probabilistic retrieval.

**Added**: Pattern-based generation with deterministic template selection

**Result**: Simpler, faster, more reliable, fewer moving parts

---

## Executive Summary

This plan implements **agentic DSL generation** for the custody/settlement domain. User describes an onboarding scenario in plain English → System classifies the pattern → Generates complete, validated DSL → Executes against database.

**Key insight**: This is a bounded domain. We don't need semantic search - we need deterministic pattern matching and complete schema knowledge.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         USER REQUEST                                         │
│  "Onboard BlackRock for global equities with MS as OTC counterparty"        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 1: INTENT EXTRACTION                                │
│  Claude extracts structured intent                                          │
│  Prompt includes: Full custody verb schemas (in context)                    │
│  Output: OnboardingIntent struct                                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 2: PATTERN CLASSIFICATION                           │
│  Deterministic classification based on intent fields:                       │
│  - Has OTC? → WithOtc pattern                                               │
│  - Multiple markets? → MultiMarket pattern                                  │
│  - Single market? → SimpleEquity pattern                                    │
│  Output: OnboardingPattern enum                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 3: REQUIREMENT DERIVATION                           │
│  Expand intent into complete requirements (Rust code, not AI):              │
│  - Universe = markets × instruments × currencies                            │
│  - SSIs = unique settlement routes                                          │
│  - Rules = specific + currency fallbacks + ultimate fallback                │
│  - ISDA/CSA = per OTC counterparty                                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 4: DSL GENERATION                                   │
│  Claude generates DSL with:                                                 │
│  - Full verb schemas in system prompt                                       │
│  - Few-shot example for the classified pattern                              │
│  - Derived requirements as structured input                                 │
│  - Reference data (markets, BICs) as lookup table                           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 5: VALIDATION + RETRY                               │
│  Parse → CSG Lint → Compile                                                 │
│  If errors: feed back to Claude for correction (max 3 retries)              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 6: EXECUTION (optional)                             │
│  Execute validated DSL against database                                     │
│  Return results with created entity IDs                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Intent Extraction

### 1.1 Define OnboardingIntent Structure

**File**: `rust/src/agentic/intent.rs`

```rust
use serde::{Deserialize, Serialize};

/// Structured representation of user's onboarding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingIntent {
    /// The client being onboarded
    pub client: ClientIntent,
    
    /// Instruments they will trade
    pub instruments: Vec<InstrumentIntent>,
    
    /// Markets they will access
    pub markets: Vec<MarketIntent>,
    
    /// OTC counterparty relationships
    pub otc_counterparties: Vec<CounterpartyIntent>,
    
    /// Explicit requirements mentioned by user
    pub explicit_requirements: Vec<String>,
    
    /// Original natural language request
    pub original_request: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientIntent {
    pub name: String,
    pub entity_type: Option<String>,  // fund, corporate, individual
    pub jurisdiction: Option<String>, // US, LU, IE, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIntent {
    pub market_code: String,           // XNYS, XLON, XFRA
    pub currencies: Vec<String>,       // USD, GBP, EUR
    pub settlement_types: Vec<String>, // DVP, FOP (default: DVP)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentIntent {
    pub class: String,                // EQUITY, GOVT_BOND, OTC_IRS
    pub specific_types: Vec<String>,  // ADR, ETF (optional)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartyIntent {
    pub name: String,
    pub instruments: Vec<String>,      // What they trade with this CP
    pub governing_law: Option<String>, // NY, ENGLISH
    pub csa_required: bool,
}

impl OnboardingIntent {
    /// Classify this intent into an onboarding pattern
    pub fn classify(&self) -> OnboardingPattern {
        let has_otc = !self.otc_counterparties.is_empty();
        let market_count = self.markets.len();
        let has_cross_currency = self.markets.iter()
            .any(|m| m.currencies.len() > 1);
        
        match (has_otc, market_count, has_cross_currency) {
            (true, _, _) => OnboardingPattern::WithOtc,
            (false, 1, false) => OnboardingPattern::SimpleEquity,
            (false, _, _) => OnboardingPattern::MultiMarket,
        }
    }
}
```

**Effort**: Small (0.5 day)

---

### 1.2 Define Onboarding Patterns

**File**: `rust/src/agentic/patterns.rs`

```rust
/// Deterministic classification of onboarding complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingPattern {
    /// Single market, single currency, no OTC
    /// Example: "US equity trading only"
    SimpleEquity,
    
    /// Multiple markets and/or cross-currency
    /// Example: "US, UK, Germany with USD cross-currency"
    MultiMarket,
    
    /// Includes OTC derivatives with ISDA/CSA
    /// Example: "Global equities plus IRS with Morgan Stanley"
    WithOtc,
}

impl OnboardingPattern {
    /// Verbs required for this pattern
    pub fn required_domains(&self) -> Vec<&'static str> {
        match self {
            Self::SimpleEquity => vec![
                "cbu",
                "cbu-custody",  // universe, ssi, rules
            ],
            Self::MultiMarket => vec![
                "cbu",
                "cbu-custody",
            ],
            Self::WithOtc => vec![
                "cbu",
                "cbu-custody",
                "isda",
                "entity-settlement",
            ],
        }
    }
    
    /// Example file for few-shot prompting
    pub fn example_file(&self) -> &'static str {
        match self {
            Self::SimpleEquity => include_str!("examples/simple_equity.dsl"),
            Self::MultiMarket => include_str!("examples/multi_market.dsl"),
            Self::WithOtc => include_str!("examples/with_otc.dsl"),
        }
    }
    
    /// Minimum expected DSL statements
    pub fn expected_statement_count(&self) -> usize {
        match self {
            Self::SimpleEquity => 6,   // CBU + universe + SSI + activate + 2 rules
            Self::MultiMarket => 15,   // CBU + N×(universe + SSI) + rules + fallbacks
            Self::WithOtc => 25,       // Above + entity + ISDA + coverage + CSA
        }
    }
}
```

**Effort**: Small (0.5 day)

---

### 1.3 Implement Intent Extractor

**File**: `rust/src/agentic/intent_extractor.rs`

```rust
use crate::agentic::intent::OnboardingIntent;
use anyhow::Result;

pub struct IntentExtractor {
    client: anthropic::Client,
}

impl IntentExtractor {
    pub fn new(client: anthropic::Client) -> Self {
        Self { client }
    }
    
    pub async fn extract(&self, user_request: &str) -> Result<OnboardingIntent> {
        let system_prompt = include_str!("prompts/intent_extraction_system.md");
        
        let response = self.client
            .messages()
            .create(anthropic::MessagesRequest {
                model: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 2000,
                system: Some(system_prompt.to_string()),
                messages: vec![
                    anthropic::Message {
                        role: "user".to_string(),
                        content: format!(
                            "Extract the onboarding intent from this request:\n\n{}",
                            user_request
                        ),
                    }
                ],
            })
            .await?;
        
        // Parse JSON response
        let json_str = extract_json_from_response(&response)?;
        let intent: OnboardingIntent = serde_json::from_str(&json_str)?;
        
        Ok(intent)
    }
}

fn extract_json_from_response(response: &anthropic::MessagesResponse) -> Result<String> {
    // Extract JSON from response, handling markdown code blocks if present
    let text = response.content.first()
        .and_then(|c| c.text.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Empty response"))?;
    
    // Strip ```json ... ``` if present
    let json = if text.contains("```json") {
        text.split("```json").nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(text)
    } else if text.contains("```") {
        text.split("```").nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(text)
    } else {
        text
    };
    
    Ok(json.trim().to_string())
}
```

**Effort**: Small (0.5 day)

---

### 1.4 Intent Extraction Prompt

**File**: `rust/src/agentic/prompts/intent_extraction_system.md`

```markdown
# Custody Onboarding Intent Extraction

You are an expert custody onboarding analyst. Extract structured information from the user's onboarding request.

## Context

This is for a custody bank onboarding a new client. Clients trade:
- **Cash securities**: Equities, bonds, ETFs (settle via markets like NYSE, LSE)
- **OTC derivatives**: Interest rate swaps, credit derivatives (require ISDA agreements)

Each market/currency combination needs Standing Settlement Instructions (SSIs).

## Market Codes

| User Says | Market Code | Primary Currency |
|-----------|-------------|------------------|
| US, NYSE, NASDAQ, American | XNYS | USD |
| UK, London, LSE | XLON | GBP |
| Germany, Frankfurt, Xetra | XFRA | EUR |
| France, Paris, Euronext | XPAR | EUR |
| Japan, Tokyo, TSE | XTKS | JPY |

## Instrument Classes

| User Says | Class Code |
|-----------|------------|
| equity, equities, stocks, shares | EQUITY |
| government bonds, treasuries, gilts | GOVT_BOND |
| corporate bonds | CORP_BOND |
| ETF, exchange traded funds | ETF |
| interest rate swap, IRS | OTC_IRS |
| credit default swap, CDS | OTC_CDS |

## Extraction Rules

1. **Client**: Extract name, infer type (fund/corporate/individual), note jurisdiction if mentioned
2. **Markets**: Map to MIC codes, default currency is market's primary currency
3. **Cross-currency**: If user says "plus USD" or "USD cross-currency", add USD to that market's currencies
4. **Settlement types**: Default to ["DVP"] unless FOP explicitly mentioned
5. **OTC**: If derivatives mentioned, identify counterparties and governing law (default: NY)
6. **CSA**: If "margin", "collateral", or "CSA" mentioned, set csa_required: true

## Output Format

Return a JSON object matching this schema:

```json
{
  "client": {
    "name": "string",
    "entity_type": "fund" | "corporate" | "individual" | null,
    "jurisdiction": "string or null"
  },
  "instruments": [
    {"class": "EQUITY", "specific_types": []}
  ],
  "markets": [
    {"market_code": "XNYS", "currencies": ["USD"], "settlement_types": ["DVP"]}
  ],
  "otc_counterparties": [
    {
      "name": "Morgan Stanley",
      "instruments": ["OTC_IRS"],
      "governing_law": "NY",
      "csa_required": true
    }
  ],
  "explicit_requirements": ["T+1 go-live"],
  "original_request": "the original text"
}
```

## Examples

**Input**: "Set up Pacific Fund for US equities"
**Output**:
```json
{
  "client": {"name": "Pacific Fund", "entity_type": "fund", "jurisdiction": null},
  "instruments": [{"class": "EQUITY", "specific_types": []}],
  "markets": [{"market_code": "XNYS", "currencies": ["USD"], "settlement_types": ["DVP"]}],
  "otc_counterparties": [],
  "explicit_requirements": [],
  "original_request": "Set up Pacific Fund for US equities"
}
```

**Input**: "Onboard BlackRock for UK and Germany with USD cross-currency plus IRS exposure to Goldman under NY law ISDA with VM"
**Output**:
```json
{
  "client": {"name": "BlackRock", "entity_type": "fund", "jurisdiction": null},
  "instruments": [{"class": "EQUITY", "specific_types": []}, {"class": "OTC_IRS", "specific_types": []}],
  "markets": [
    {"market_code": "XLON", "currencies": ["GBP", "USD"], "settlement_types": ["DVP"]},
    {"market_code": "XFRA", "currencies": ["EUR", "USD"], "settlement_types": ["DVP"]}
  ],
  "otc_counterparties": [
    {"name": "Goldman Sachs", "instruments": ["OTC_IRS"], "governing_law": "NY", "csa_required": true}
  ],
  "explicit_requirements": [],
  "original_request": "..."
}
```

Return ONLY the JSON object, no explanation.
```

**Effort**: Small (0.5 day)

---

## Phase 2: Requirement Derivation

### 2.1 Implement Requirement Planner

**File**: `rust/src/agentic/planner.rs`

This is **deterministic Rust code** - no AI involved. It expands the intent into complete requirements.

```rust
use crate::agentic::intent::{OnboardingIntent, MarketIntent, CounterpartyIntent};
use crate::agentic::patterns::OnboardingPattern;

/// Complete onboarding plan derived from intent
#[derive(Debug, Clone)]
pub struct OnboardingPlan {
    pub pattern: OnboardingPattern,
    pub cbu: CbuPlan,
    pub entities: Vec<EntityPlan>,
    pub universe: Vec<UniverseEntry>,
    pub ssis: Vec<SsiPlan>,
    pub booking_rules: Vec<BookingRulePlan>,
    pub isdas: Vec<IsdaPlan>,
}

#[derive(Debug, Clone)]
pub struct CbuPlan {
    pub name: String,
    pub jurisdiction: String,
    pub client_type: String,
    pub variable: String, // @cbu
}

#[derive(Debug, Clone)]
pub struct EntityPlan {
    pub name: String,
    pub action: EntityAction, // Lookup or Create
    pub variable: String,
}

#[derive(Debug, Clone, Copy)]
pub enum EntityAction {
    Lookup,  // Assume exists (counterparties)
    Create,  // Create new (client entity if needed)
}

#[derive(Debug, Clone)]
pub struct UniverseEntry {
    pub instrument_class: String,
    pub market: Option<String>,
    pub currencies: Vec<String>,
    pub settlement_types: Vec<String>,
    pub counterparty_var: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SsiPlan {
    pub name: String,
    pub ssi_type: String,
    pub market: Option<String>,
    pub currency: String,
    pub variable: String,
    // Account details will be generated with placeholders
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct IsdaPlan {
    pub counterparty_var: String,
    pub counterparty_name: String,
    pub governing_law: String,
    pub variable: String,
    pub coverages: Vec<String>,  // Instrument classes
    pub csa: Option<CsaPlan>,
}

#[derive(Debug, Clone)]
pub struct CsaPlan {
    pub csa_type: String,  // VM or IM
    pub variable: String,
}

pub struct RequirementPlanner;

impl RequirementPlanner {
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
            jurisdiction: intent.client.jurisdiction.clone().unwrap_or_else(|| "US".to_string()),
            client_type: intent.client.entity_type.clone()
                .map(|t| t.to_uppercase())
                .unwrap_or_else(|| "FUND".to_string()),
            variable: "cbu".to_string(),
        }
    }
    
    fn plan_entities(intent: &OnboardingIntent) -> Vec<EntityPlan> {
        intent.otc_counterparties.iter().map(|cp| {
            EntityPlan {
                name: cp.name.clone(),
                action: EntityAction::Lookup,
                variable: Self::entity_variable(&cp.name),
            }
        }).collect()
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
                        settlement_types: market.settlement_types.clone(),
                        counterparty_var: None,
                    });
                }
            }
        }
        
        // OTC instruments: per counterparty
        for cp in &intent.otc_counterparties {
            for instr in &cp.instruments {
                // Collect all currencies from all markets for OTC
                let currencies: Vec<String> = intent.markets.iter()
                    .flat_map(|m| m.currencies.iter().cloned())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                
                entries.push(UniverseEntry {
                    instrument_class: instr.clone(),
                    market: None,
                    currencies: if currencies.is_empty() { vec!["USD".to_string()] } else { currencies },
                    settlement_types: vec!["DVP".to_string()],
                    counterparty_var: Some(Self::entity_variable(&cp.name)),
                });
            }
        }
        
        entries
    }
    
    fn derive_ssis(universe: &[UniverseEntry]) -> Vec<SsiPlan> {
        use std::collections::HashSet;
        
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
                    (Some(m), _) => format!("{} {} {} DVP", entry.instrument_class, m, currency),
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
        let currencies: std::collections::HashSet<_> = universe.iter()
            .flat_map(|e| e.currencies.iter())
            .collect();
        
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
        let default_ssi = ssis.first().map(|s| s.variable.clone()).unwrap_or_else(|| "ssi-default".to_string());
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
        intent.otc_counterparties.iter().map(|cp| {
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
        }).collect()
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
        ssis.first().map(|s| s.variable.clone()).unwrap_or_else(|| "ssi-default".to_string())
    }
}
```

**Effort**: Medium (1-2 days)

---

## Phase 3: DSL Generation

### 3.1 Implement DSL Generator

**File**: `rust/src/agentic/generator.rs`

```rust
use crate::agentic::planner::*;
use crate::agentic::patterns::OnboardingPattern;
use anyhow::Result;

pub struct DslGenerator {
    client: anthropic::Client,
}

impl DslGenerator {
    pub fn new(client: anthropic::Client) -> Self {
        Self { client }
    }
    
    /// Generate DSL from an onboarding plan
    pub async fn generate(&self, plan: &OnboardingPlan) -> Result<String> {
        let system_prompt = self.build_system_prompt(plan.pattern);
        let user_prompt = self.build_user_prompt(plan);
        
        let response = self.client
            .messages()
            .create(anthropic::MessagesRequest {
                model: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 4000,
                system: Some(system_prompt),
                messages: vec![
                    anthropic::Message {
                        role: "user".to_string(),
                        content: user_prompt,
                    }
                ],
            })
            .await?;
        
        let dsl = response.content.first()
            .and_then(|c| c.text.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Empty response"))?;
        
        // Strip markdown code blocks if present
        let clean_dsl = Self::strip_code_blocks(dsl);
        
        Ok(clean_dsl)
    }
    
    fn build_system_prompt(&self, pattern: OnboardingPattern) -> String {
        let verb_schemas = include_str!("schemas/custody_verbs.md");
        let reference_data = include_str!("schemas/reference_data.md");
        let example = pattern.example_file();
        
        format!(r#"# DSL Generation System

You are a DSL code generator for a custody onboarding system. Generate valid DSL code based on the structured requirements provided.

## DSL Syntax

S-expression format:
```
(domain.verb :arg1 value1 :arg2 value2 :as @variable)
```

- Keywords are prefixed with `:`
- Strings use double quotes: `"value"`
- UUIDs reference variables: `@variable`
- Lists use brackets: `["a" "b" "c"]`
- Comments start with `;`

## Available Verbs

{verb_schemas}

## Reference Data

{reference_data}

## Example ({pattern:?} pattern)

{example}

## Rules

1. Generate ONLY valid DSL code
2. Use `:as @variable` to capture results for later reference
3. Order statements so dependencies are defined before use
4. Include section comments for readability
5. Add validation at the end: `(cbu-custody.validate-booking-coverage :cbu-id @cbu)`
6. Use placeholder values for account numbers (e.g., "SAFE-001", "CASH-001")
7. Use today's date for effective-date: "2024-12-01"
8. Output ONLY the DSL code, no explanations
"#, 
            verb_schemas = verb_schemas,
            reference_data = reference_data,
            example = example,
            pattern = pattern
        )
    }
    
    fn build_user_prompt(&self, plan: &OnboardingPlan) -> String {
        let mut prompt = String::new();
        
        prompt.push_str("Generate DSL for this custody onboarding:\n\n");
        
        // CBU
        prompt.push_str(&format!("## CBU\n- Name: {}\n- Jurisdiction: {}\n- Type: {}\n- Variable: @{}\n\n",
            plan.cbu.name, plan.cbu.jurisdiction, plan.cbu.client_type, plan.cbu.variable));
        
        // Entities
        if !plan.entities.is_empty() {
            prompt.push_str("## Entities (lookup existing)\n");
            for e in &plan.entities {
                prompt.push_str(&format!("- {} → @{}\n", e.name, e.variable));
            }
            prompt.push_str("\n");
        }
        
        // Universe
        prompt.push_str("## Universe Entries\n");
        for u in &plan.universe {
            let market = u.market.as_deref().unwrap_or("OTC");
            let cp = u.counterparty_var.as_ref().map(|v| format!(" (counterparty: @{})", v)).unwrap_or_default();
            prompt.push_str(&format!("- {} in {} with currencies {:?}{}\n", 
                u.instrument_class, market, u.currencies, cp));
        }
        prompt.push_str("\n");
        
        // SSIs
        prompt.push_str("## SSIs Required\n");
        for s in &plan.ssis {
            prompt.push_str(&format!("- {} ({}, {}) → @{}\n", 
                s.name, s.ssi_type, s.currency, s.variable));
        }
        prompt.push_str("\n");
        
        // Booking Rules
        prompt.push_str("## Booking Rules\n");
        for r in &plan.booking_rules {
            let criteria: Vec<String> = [
                r.instrument_class.as_ref().map(|v| format!("class={}", v)),
                r.market.as_ref().map(|v| format!("market={}", v)),
                r.currency.as_ref().map(|v| format!("currency={}", v)),
                r.settlement_type.as_ref().map(|v| format!("settlement={}", v)),
                r.counterparty_var.as_ref().map(|v| format!("counterparty=@{}", v)),
            ].into_iter().flatten().collect();
            
            let criteria_str = if criteria.is_empty() { "ANY".to_string() } else { criteria.join(", ") };
            prompt.push_str(&format!("- {} (priority {}, {}) → @{}\n", 
                r.name, r.priority, criteria_str, r.ssi_variable));
        }
        prompt.push_str("\n");
        
        // ISDA
        if !plan.isdas.is_empty() {
            prompt.push_str("## ISDA Agreements\n");
            for isda in &plan.isdas {
                prompt.push_str(&format!("- With {} ({} law) → @{}\n", 
                    isda.counterparty_name, isda.governing_law, isda.variable));
                prompt.push_str(&format!("  Coverages: {:?}\n", isda.coverages));
                if let Some(csa) = &isda.csa {
                    prompt.push_str(&format!("  CSA: {} → @{}\n", csa.csa_type, csa.variable));
                }
            }
            prompt.push_str("\n");
        }
        
        prompt.push_str("Generate the complete DSL now.\n");
        
        prompt
    }
    
    fn strip_code_blocks(text: &str) -> String {
        let text = text.trim();
        if text.starts_with("```") {
            let lines: Vec<&str> = text.lines().collect();
            if lines.len() > 2 {
                return lines[1..lines.len()-1].join("\n");
            }
        }
        text.to_string()
    }
}
```

**Effort**: Medium (1-2 days)

---

### 3.2 Create Verb Schema Reference

**File**: `rust/src/agentic/schemas/custody_verbs.md`

Extract relevant sections from `verbs.yaml` into a clean markdown format for the prompt. This is a static file, not generated.

```markdown
## cbu domain

### cbu.ensure
Create or update a CBU.
```
(cbu.ensure :name "string" :jurisdiction "XX" :client-type "FUND|CORPORATE|INDIVIDUAL" :as @variable)
```

## cbu-custody domain

### cbu-custody.add-universe
Declare what a CBU trades.
```
(cbu-custody.add-universe 
  :cbu-id @cbu 
  :instrument-class "EQUITY|GOVT_BOND|CORP_BOND|ETF|OTC_IRS|OTC_CDS"
  :market "XNYS|XLON|XFRA|XPAR" ;; optional for OTC
  :currencies ["USD" "GBP"]
  :settlement-types ["DVP"]  ;; optional, default DVP
  :counterparty @entity)     ;; optional, for OTC
```

### cbu-custody.create-ssi
Create Standing Settlement Instruction.
```
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary"
  :type "SECURITIES|CASH|COLLATERAL|FX_NOSTRO"
  :safekeeping-account "SAFE-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "CASH-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi)
```

### cbu-custody.activate-ssi
Activate an SSI.
```
(cbu-custody.activate-ssi :ssi-id @ssi)
```

### cbu-custody.add-booking-rule
Add routing rule. NULL criteria = wildcard (matches any).
```
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi
  :name "Rule Name"
  :priority 10              ;; lower = higher priority
  :instrument-class "EQUITY" ;; optional
  :market "XNYS"            ;; optional
  :currency "USD"           ;; optional
  :settlement-type "DVP"    ;; optional
  :counterparty @entity     ;; optional, for OTC
  :effective-date "2024-12-01")
```

### cbu-custody.validate-booking-coverage
Validate all universe entries have matching rules.
```
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

## isda domain

### isda.create
Create ISDA master agreement.
```
(isda.create
  :cbu-id @cbu
  :counterparty @entity
  :agreement-date "2024-12-01"
  :governing-law "NY|ENGLISH"
  :effective-date "2024-12-01"
  :as @isda)
```

### isda.add-coverage
Add instrument class coverage.
```
(isda.add-coverage :isda-id @isda :instrument-class "OTC_IRS")
```

### isda.add-csa
Add Credit Support Annex.
```
(isda.add-csa
  :isda-id @isda
  :csa-type "VM|IM"
  :threshold 250000
  :threshold-currency "USD"
  :collateral-ssi @ssi
  :effective-date "2024-12-01"
  :as @csa)
```

## entity domain

### entity.read
Lookup existing entity.
```
(entity.read :name "Morgan Stanley" :as @ms)
```
```

**Effort**: Small (0.5 day)

---

### 3.3 Create Reference Data File

**File**: `rust/src/agentic/schemas/reference_data.md`

```markdown
## Markets

| MIC | Name | Country | Currency | CSD BIC | PSET BIC |
|-----|------|---------|----------|---------|----------|
| XNYS | NYSE | US | USD | DTCYUS33 | DTCYUS33 |
| XNAS | NASDAQ | US | USD | DTCYUS33 | DTCYUS33 |
| XLON | London | GB | GBP | CABOROCP | CABOROCP |
| XFRA | Frankfurt | DE | EUR | DAABOROCP | DAABOROCP |
| XPAR | Euronext Paris | FR | EUR | SABOROCP | SABOROCP |

## Instrument Classes

| Code | Name | Requires ISDA |
|------|------|---------------|
| EQUITY | Equities | No |
| GOVT_BOND | Government Bonds | No |
| CORP_BOND | Corporate Bonds | No |
| ETF | ETFs | No |
| OTC_IRS | Interest Rate Swaps | Yes |
| OTC_CDS | Credit Default Swaps | Yes |

## Standard BICs

| Institution | BIC |
|-------------|-----|
| Bank of America | BABOROCP |
| Citi | CITIBORX |
| JP Morgan | CHASUS33 |
| Morgan Stanley | MSNYUS33 |
| Goldman Sachs | GOLDUS33 |
| DTCC | DTCYUS33 |
```

**Effort**: Small (0.5 day)

---

### 3.4 Create Example Files

**File**: `rust/src/agentic/examples/simple_equity.dsl`

```clojure
;; =============================================================================
;; SIMPLE US EQUITY SETUP
;; =============================================================================

;; --- CBU ---
(cbu.ensure :name "Apex Capital" :jurisdiction "US" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])

;; --- Layer 2: SSI ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-us)

;; --- Layer 3: Booking Rules ---
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity DVP" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD")

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

**File**: `rust/src/agentic/examples/multi_market.dsl`

```clojure
;; =============================================================================
;; MULTI-MARKET EQUITY WITH CROSS-CURRENCY
;; =============================================================================

;; --- CBU ---
(cbu.ensure :name "Global Fund" :jurisdiction "LU" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"])
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XLON" :currencies ["GBP" "USD"])
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XFRA" :currencies ["EUR" "USD"])

;; --- Layer 2: SSIs ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-US" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)

(cbu-custody.create-ssi :cbu-id @cbu :name "UK Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-UK" :safekeeping-bic "CABOROCP"
  :cash-account "CASH-GBP" :cash-bic "CABOROCP" :cash-currency "GBP"
  :pset-bic "CABOROCP" :effective-date "2024-12-01" :as @ssi-uk)

(cbu-custody.create-ssi :cbu-id @cbu :name "DE Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-DE" :safekeeping-bic "DAABOROCP"
  :cash-account "CASH-EUR" :cash-bic "DAABOROCP" :cash-currency "EUR"
  :pset-bic "DAABOROCP" :effective-date "2024-12-01" :as @ssi-de)

(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk)
(cbu-custody.activate-ssi :ssi-id @ssi-de)

;; --- Layer 3: Booking Rules ---
;; Specific rules
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity USD" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-uk :name "UK Equity GBP" :priority 15
  :instrument-class "EQUITY" :market "XLON" :currency "GBP" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "UK Equity USD" :priority 16
  :instrument-class "EQUITY" :market "XLON" :currency "USD" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-de :name "DE Equity EUR" :priority 20
  :instrument-class "EQUITY" :market "XFRA" :currency "EUR" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "DE Equity USD" :priority 21
  :instrument-class "EQUITY" :market "XFRA" :currency "USD" :settlement-type "DVP")

;; Currency fallbacks
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-uk :name "GBP Fallback" :priority 51 :currency "GBP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-de :name "EUR Fallback" :priority 52 :currency "EUR")

;; Ultimate fallback
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "Ultimate Fallback" :priority 100)

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

**File**: `rust/src/agentic/examples/with_otc.dsl`

```clojure
;; =============================================================================
;; EQUITY + OTC IRS WITH ISDA/CSA
;; =============================================================================

;; --- Entities (lookup existing counterparties) ---
(entity.read :name "Morgan Stanley" :as @ms)

;; --- CBU ---
(cbu.ensure :name "Pacific Fund" :jurisdiction "US" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
;; Cash instruments
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"])
;; OTC (counterparty-specific)
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "OTC_IRS" :currencies ["USD"] :counterparty @ms)

;; --- Layer 2: SSIs ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)

(cbu-custody.create-ssi :cbu-id @cbu :name "Collateral" :type "COLLATERAL"
  :collateral-account "COLL-001" :collateral-bic "BABOROCP"
  :effective-date "2024-12-01" :as @ssi-collateral)

(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-collateral)

;; --- Layer 3: Booking Rules ---
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity DVP" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "OTC IRS MS" :priority 20
  :instrument-class "OTC_IRS" :currency "USD" :counterparty @ms)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "Ultimate Fallback" :priority 100)

;; --- ISDA ---
(isda.create :cbu-id @cbu :counterparty @ms :agreement-date "2024-12-01"
  :governing-law "NY" :effective-date "2024-12-01" :as @isda-ms)
(isda.add-coverage :isda-id @isda-ms :instrument-class "OTC_IRS")
(isda.add-csa :isda-id @isda-ms :csa-type "VM" :threshold 250000
  :threshold-currency "USD" :collateral-ssi @ssi-collateral :effective-date "2024-12-01")

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

**Effort**: Small (0.5 day)

---

## Phase 4: Validation & Feedback Loop

### 4.1 Implement Validator Integration

**File**: `rust/src/agentic/validator.rs`

```rust
use crate::dsl_v2::parser::Parser;
use crate::dsl_v2::csg_linter::CsgLinter;
use crate::dsl_v2::execution_plan::ExecutionPlanner;
use anyhow::Result;

pub struct AgentValidator {
    parser: Parser,
    linter: CsgLinter,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub line: usize,
    pub message: String,
    pub context: Option<String>,
}

impl AgentValidator {
    pub fn new() -> Result<Self> {
        Ok(Self {
            parser: Parser::new(),
            linter: CsgLinter::new()?,
        })
    }
    
    pub fn validate(&self, dsl_source: &str) -> ValidationResult {
        // Phase 1: Parse
        let ast = match self.parser.parse(dsl_source) {
            Ok(ast) => ast,
            Err(e) => {
                return ValidationResult {
                    is_valid: false,
                    errors: vec![ValidationError {
                        line: e.line.unwrap_or(0),
                        message: e.message,
                        context: e.context,
                    }],
                    warnings: vec![],
                };
            }
        };
        
        // Phase 2: CSG Lint
        let lint_result = self.linter.lint(&ast);
        
        if !lint_result.errors.is_empty() {
            return ValidationResult {
                is_valid: false,
                errors: lint_result.errors.into_iter().map(|e| ValidationError {
                    line: e.line,
                    message: e.message,
                    context: e.suggestion,
                }).collect(),
                warnings: lint_result.warnings,
            };
        }
        
        ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: lint_result.warnings,
        }
    }
}

impl Default for AgentValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create validator")
    }
}
```

**Effort**: Small (0.5 day)

---

### 4.2 Implement Feedback Loop

**File**: `rust/src/agentic/feedback.rs`

```rust
use crate::agentic::generator::DslGenerator;
use crate::agentic::validator::{AgentValidator, ValidationResult, ValidationError};
use crate::agentic::planner::OnboardingPlan;
use anyhow::{Result, anyhow};

pub struct FeedbackLoop {
    generator: DslGenerator,
    validator: AgentValidator,
    client: anthropic::Client,
    max_retries: usize,
}

pub struct ValidatedDsl {
    pub source: String,
    pub attempts: usize,
}

impl FeedbackLoop {
    pub fn new(client: anthropic::Client, max_retries: usize) -> Result<Self> {
        Ok(Self {
            generator: DslGenerator::new(client.clone()),
            validator: AgentValidator::new()?,
            client,
            max_retries,
        })
    }
    
    pub async fn generate_valid_dsl(&self, plan: &OnboardingPlan) -> Result<ValidatedDsl> {
        let mut attempts = 0;
        let mut current_dsl = self.generator.generate(plan).await?;
        
        loop {
            attempts += 1;
            let validation = self.validator.validate(&current_dsl);
            
            if validation.is_valid {
                return Ok(ValidatedDsl {
                    source: current_dsl,
                    attempts,
                });
            }
            
            if attempts >= self.max_retries {
                return Err(anyhow!(
                    "Failed to generate valid DSL after {} attempts.\nLast errors: {:?}\nLast DSL:\n{}",
                    attempts,
                    validation.errors,
                    current_dsl
                ));
            }
            
            // Ask Claude to fix
            current_dsl = self.request_fix(&current_dsl, &validation.errors).await?;
        }
    }
    
    async fn request_fix(&self, dsl: &str, errors: &[ValidationError]) -> Result<String> {
        let error_text: String = errors.iter()
            .map(|e| {
                let ctx = e.context.as_ref().map(|c| format!(" (hint: {})", c)).unwrap_or_default();
                format!("Line {}: {}{}", e.line, e.message, ctx)
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        let prompt = format!(
            r#"The following DSL has validation errors. Fix them and return ONLY the corrected DSL.

## Errors
{}

## Current DSL
```
{}
```

Return ONLY the corrected DSL code, no explanations."#,
            error_text,
            dsl
        );
        
        let response = self.client
            .messages()
            .create(anthropic::MessagesRequest {
                model: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 4000,
                messages: vec![
                    anthropic::Message {
                        role: "user".to_string(),
                        content: prompt,
                    }
                ],
                ..Default::default()
            })
            .await?;
        
        let fixed = response.content.first()
            .and_then(|c| c.text.as_ref())
            .ok_or_else(|| anyhow!("Empty response from fix request"))?;
        
        Ok(Self::strip_code_blocks(fixed))
    }
    
    fn strip_code_blocks(text: &str) -> String {
        let text = text.trim();
        if text.starts_with("```") {
            let lines: Vec<&str> = text.lines().collect();
            if lines.len() > 2 {
                return lines[1..lines.len()-1].join("\n");
            }
        }
        text.to_string()
    }
}
```

**Effort**: Medium (1 day)

---

## Phase 5: Orchestration & API

### 5.1 Create Agent Orchestrator

**File**: `rust/src/agentic/orchestrator.rs`

```rust
use crate::agentic::intent::{OnboardingIntent};
use crate::agentic::intent_extractor::IntentExtractor;
use crate::agentic::planner::{OnboardingPlan, RequirementPlanner};
use crate::agentic::feedback::{FeedbackLoop, ValidatedDsl};
use crate::dsl_v2::executor::DslExecutor;
use anyhow::Result;

pub struct AgentOrchestrator {
    intent_extractor: IntentExtractor,
    feedback_loop: FeedbackLoop,
    executor: Option<DslExecutor>,
}

pub struct GenerationResult {
    pub intent: OnboardingIntent,
    pub plan: OnboardingPlan,
    pub dsl: ValidatedDsl,
    pub execution_result: Option<ExecutionResult>,
}

pub struct ExecutionResult {
    pub success: bool,
    pub bindings: Vec<(String, String)>,  // variable name -> UUID
    pub error: Option<String>,
}

impl AgentOrchestrator {
    pub fn new(client: anthropic::Client, db_pool: Option<sqlx::PgPool>) -> Result<Self> {
        let executor = db_pool.map(|pool| DslExecutor::new(pool));
        
        Ok(Self {
            intent_extractor: IntentExtractor::new(client.clone()),
            feedback_loop: FeedbackLoop::new(client, 3)?,
            executor,
        })
    }
    
    pub async fn generate(&self, request: &str, execute: bool) -> Result<GenerationResult> {
        // Phase 1: Extract intent
        let intent = self.intent_extractor.extract(request).await?;
        
        // Phase 2: Classify and plan (deterministic)
        let plan = RequirementPlanner::plan(&intent);
        
        // Phase 3-4: Generate and validate DSL (with retry)
        let dsl = self.feedback_loop.generate_valid_dsl(&plan).await?;
        
        // Phase 5: Execute if requested
        let execution_result = if execute {
            if let Some(ref executor) = self.executor {
                Some(self.execute_dsl(executor, &dsl.source).await?)
            } else {
                return Err(anyhow::anyhow!("Execution requested but no database connection"));
            }
        } else {
            None
        };
        
        Ok(GenerationResult {
            intent,
            plan,
            dsl,
            execution_result,
        })
    }
    
    async fn execute_dsl(&self, executor: &DslExecutor, source: &str) -> Result<ExecutionResult> {
        match executor.execute(source).await {
            Ok(result) => Ok(ExecutionResult {
                success: true,
                bindings: result.bindings.into_iter()
                    .map(|(k, v)| (k, v.to_string()))
                    .collect(),
                error: None,
            }),
            Err(e) => Ok(ExecutionResult {
                success: false,
                bindings: vec![],
                error: Some(e.to_string()),
            }),
        }
    }
}
```

**Effort**: Medium (1 day)

---

### 5.2 Extend API Routes

**File**: `rust/src/api/agent_routes.rs` (extend existing)

```rust
use axum::{
    extract::{State, Json},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use crate::agentic::orchestrator::{AgentOrchestrator, GenerationResult};
use crate::AppState;

pub fn custody_agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/custody/generate", post(generate_custody))
        .route("/api/agent/custody/plan", post(plan_only))
}

#[derive(Deserialize)]
pub struct CustodyGenerateRequest {
    pub instruction: String,
    #[serde(default)]
    pub execute: bool,
}

#[derive(Serialize)]
pub struct CustodyGenerateResponse {
    pub success: bool,
    pub intent: serde_json::Value,
    pub pattern: String,
    pub dsl: String,
    pub attempts: usize,
    pub execution: Option<ExecutionResponse>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ExecutionResponse {
    pub success: bool,
    pub bindings: Vec<BindingEntry>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct BindingEntry {
    pub variable: String,
    pub uuid: String,
}

async fn generate_custody(
    State(state): State<AppState>,
    Json(request): Json<CustodyGenerateRequest>,
) -> Json<CustodyGenerateResponse> {
    let orchestrator = state.agent_orchestrator();
    
    match orchestrator.generate(&request.instruction, request.execute).await {
        Ok(result) => Json(CustodyGenerateResponse {
            success: true,
            intent: serde_json::to_value(&result.intent).unwrap_or_default(),
            pattern: format!("{:?}", result.plan.pattern),
            dsl: result.dsl.source,
            attempts: result.dsl.attempts,
            execution: result.execution_result.map(|e| ExecutionResponse {
                success: e.success,
                bindings: e.bindings.into_iter()
                    .map(|(v, u)| BindingEntry { variable: v, uuid: u })
                    .collect(),
                error: e.error,
            }),
            error: None,
        }),
        Err(e) => Json(CustodyGenerateResponse {
            success: false,
            intent: serde_json::Value::Null,
            pattern: "".to_string(),
            dsl: "".to_string(),
            attempts: 0,
            execution: None,
            error: Some(e.to_string()),
        }),
    }
}

#[derive(Deserialize)]
pub struct PlanRequest {
    pub instruction: String,
}

#[derive(Serialize)]
pub struct PlanResponse {
    pub success: bool,
    pub intent: serde_json::Value,
    pub pattern: String,
    pub plan: serde_json::Value,
    pub error: Option<String>,
}

async fn plan_only(
    State(state): State<AppState>,
    Json(request): Json<PlanRequest>,
) -> Json<PlanResponse> {
    let orchestrator = state.agent_orchestrator();
    
    match orchestrator.intent_extractor.extract(&request.instruction).await {
        Ok(intent) => {
            let plan = RequirementPlanner::plan(&intent);
            Json(PlanResponse {
                success: true,
                intent: serde_json::to_value(&intent).unwrap_or_default(),
                pattern: format!("{:?}", plan.pattern),
                plan: serde_json::to_value(&plan).unwrap_or_default(),
                error: None,
            })
        }
        Err(e) => Json(PlanResponse {
            success: false,
            intent: serde_json::Value::Null,
            pattern: "".to_string(),
            plan: serde_json::Value::Null,
            error: Some(e.to_string()),
        }),
    }
}
```

**Effort**: Small (0.5 day)

---

### 5.3 Extend CLI

**File**: `rust/src/bin/dsl_cli.rs` (extend existing)

Add subcommand:

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...
    
    /// Generate custody onboarding DSL from natural language
    Custody {
        /// Natural language instruction
        #[arg(short, long)]
        instruction: String,
        
        /// Execute generated DSL against database
        #[arg(long)]
        execute: bool,
        
        /// Show plan without generating DSL
        #[arg(long)]
        plan_only: bool,
        
        /// Save generated DSL to file
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Database URL (required with --execute)
        #[arg(long, env = "DATABASE_URL")]
        db_url: Option<String>,
    },
}

// In match arms:
Commands::Custody { instruction, execute, plan_only, output, db_url } => {
    let client = create_anthropic_client()?;
    
    if plan_only {
        // Just show the plan
        let extractor = IntentExtractor::new(client);
        let intent = extractor.extract(&instruction).await?;
        let plan = RequirementPlanner::plan(&intent);
        
        println!("Intent: {:#?}", intent);
        println!("\nPattern: {:?}", plan.pattern);
        println!("\nPlan: {:#?}", plan);
        return Ok(());
    }
    
    let db_pool = if execute {
        let url = db_url.ok_or_else(|| anyhow!("--db-url required with --execute"))?;
        Some(sqlx::PgPool::connect(&url).await?)
    } else {
        None
    };
    
    let orchestrator = AgentOrchestrator::new(client, db_pool)?;
    let result = orchestrator.generate(&instruction, execute).await?;
    
    println!(";; Pattern: {:?}", result.plan.pattern);
    println!(";; Attempts: {}", result.dsl.attempts);
    println!();
    println!("{}", result.dsl.source);
    
    if let Some(exec) = result.execution_result {
        println!();
        if exec.success {
            println!(";; Execution: SUCCESS");
            for (var, uuid) in exec.bindings {
                println!(";;   @{} = {}", var, uuid);
            }
        } else {
            println!(";; Execution: FAILED - {}", exec.error.unwrap_or_default());
        }
    }
    
    if let Some(path) = output {
        std::fs::write(&path, &result.dsl.source)?;
        println!("\nSaved to: {}", path.display());
    }
}
```

**Usage**:
```bash
# Generate DSL
dsl_cli custody -i "Onboard Pacific Fund for US equities"

# Generate and execute
dsl_cli custody -i "..." --execute --db-url postgresql:///data_designer

# Show plan only
dsl_cli custody -i "..." --plan-only

# Save to file
dsl_cli custody -i "..." -o onboarding.dsl
```

**Effort**: Small (0.5 day)

---

## Phase 6: Testing

### 6.1 Unit Tests

**File**: `rust/src/agentic/tests/mod.rs`

```rust
mod intent_tests;
mod planner_tests;
mod validator_tests;
```

**File**: `rust/src/agentic/tests/intent_tests.rs`

```rust
use crate::agentic::intent::*;
use crate::agentic::patterns::OnboardingPattern;

#[test]
fn test_simple_equity_classification() {
    let intent = OnboardingIntent {
        client: ClientIntent { name: "Test".to_string(), entity_type: None, jurisdiction: None },
        instruments: vec![InstrumentIntent { class: "EQUITY".to_string(), specific_types: vec![] }],
        markets: vec![MarketIntent { 
            market_code: "XNYS".to_string(), 
            currencies: vec!["USD".to_string()],
            settlement_types: vec!["DVP".to_string()],
        }],
        otc_counterparties: vec![],
        explicit_requirements: vec![],
        original_request: "".to_string(),
    };
    
    assert_eq!(intent.classify(), OnboardingPattern::SimpleEquity);
}

#[test]
fn test_multi_market_classification() {
    let intent = OnboardingIntent {
        client: ClientIntent { name: "Test".to_string(), entity_type: None, jurisdiction: None },
        instruments: vec![InstrumentIntent { class: "EQUITY".to_string(), specific_types: vec![] }],
        markets: vec![
            MarketIntent { market_code: "XNYS".to_string(), currencies: vec!["USD".to_string()], settlement_types: vec!["DVP".to_string()] },
            MarketIntent { market_code: "XLON".to_string(), currencies: vec!["GBP".to_string()], settlement_types: vec!["DVP".to_string()] },
        ],
        otc_counterparties: vec![],
        explicit_requirements: vec![],
        original_request: "".to_string(),
    };
    
    assert_eq!(intent.classify(), OnboardingPattern::MultiMarket);
}

#[test]
fn test_otc_classification() {
    let intent = OnboardingIntent {
        client: ClientIntent { name: "Test".to_string(), entity_type: None, jurisdiction: None },
        instruments: vec![
            InstrumentIntent { class: "EQUITY".to_string(), specific_types: vec![] },
            InstrumentIntent { class: "OTC_IRS".to_string(), specific_types: vec![] },
        ],
        markets: vec![MarketIntent { 
            market_code: "XNYS".to_string(), 
            currencies: vec!["USD".to_string()],
            settlement_types: vec!["DVP".to_string()],
        }],
        otc_counterparties: vec![CounterpartyIntent {
            name: "Morgan Stanley".to_string(),
            instruments: vec!["OTC_IRS".to_string()],
            governing_law: Some("NY".to_string()),
            csa_required: true,
        }],
        explicit_requirements: vec![],
        original_request: "".to_string(),
    };
    
    assert_eq!(intent.classify(), OnboardingPattern::WithOtc);
}
```

**File**: `rust/src/agentic/tests/planner_tests.rs`

```rust
use crate::agentic::intent::*;
use crate::agentic::planner::RequirementPlanner;

#[test]
fn test_simple_equity_plan() {
    let intent = OnboardingIntent {
        client: ClientIntent { name: "Apex Fund".to_string(), entity_type: Some("fund".to_string()), jurisdiction: Some("US".to_string()) },
        instruments: vec![InstrumentIntent { class: "EQUITY".to_string(), specific_types: vec![] }],
        markets: vec![MarketIntent { 
            market_code: "XNYS".to_string(), 
            currencies: vec!["USD".to_string()],
            settlement_types: vec!["DVP".to_string()],
        }],
        otc_counterparties: vec![],
        explicit_requirements: vec![],
        original_request: "".to_string(),
    };
    
    let plan = RequirementPlanner::plan(&intent);
    
    assert_eq!(plan.cbu.name, "Apex Fund");
    assert_eq!(plan.universe.len(), 1);
    assert!(plan.ssis.len() >= 1);
    assert!(plan.booking_rules.len() >= 2); // Specific + fallback
    assert!(plan.isdas.is_empty());
}

#[test]
fn test_multi_market_plan_ssi_count() {
    let intent = OnboardingIntent {
        client: ClientIntent { name: "Global Fund".to_string(), entity_type: None, jurisdiction: None },
        instruments: vec![InstrumentIntent { class: "EQUITY".to_string(), specific_types: vec![] }],
        markets: vec![
            MarketIntent { market_code: "XNYS".to_string(), currencies: vec!["USD".to_string()], settlement_types: vec!["DVP".to_string()] },
            MarketIntent { market_code: "XLON".to_string(), currencies: vec!["GBP".to_string(), "USD".to_string()], settlement_types: vec!["DVP".to_string()] },
        ],
        otc_counterparties: vec![],
        explicit_requirements: vec![],
        original_request: "".to_string(),
    };
    
    let plan = RequirementPlanner::plan(&intent);
    
    // Should have SSIs for: XNYS/USD, XLON/GBP, XLON/USD (or reuse XNYS/USD)
    assert!(plan.ssis.len() >= 2);
    assert!(plan.universe.len() == 2);
}

#[test]
fn test_otc_plan_has_isda() {
    let intent = OnboardingIntent {
        client: ClientIntent { name: "Pacific Fund".to_string(), entity_type: None, jurisdiction: None },
        instruments: vec![InstrumentIntent { class: "OTC_IRS".to_string(), specific_types: vec![] }],
        markets: vec![],
        otc_counterparties: vec![CounterpartyIntent {
            name: "Morgan Stanley".to_string(),
            instruments: vec!["OTC_IRS".to_string()],
            governing_law: Some("NY".to_string()),
            csa_required: true,
        }],
        explicit_requirements: vec![],
        original_request: "".to_string(),
    };
    
    let plan = RequirementPlanner::plan(&intent);
    
    assert_eq!(plan.isdas.len(), 1);
    assert_eq!(plan.isdas[0].counterparty_name, "Morgan Stanley");
    assert!(plan.isdas[0].csa.is_some());
}
```

**Effort**: Medium (1 day)

---

### 6.2 Integration Tests

**File**: `rust/tests/agentic_integration.rs`

```rust
use ob_poc::agentic::orchestrator::AgentOrchestrator;

#[tokio::test]
#[ignore] // Requires API key
async fn test_simple_equity_generation() {
    let client = create_test_client();
    let orchestrator = AgentOrchestrator::new(client, None).unwrap();
    
    let result = orchestrator.generate(
        "Set up Apex Capital for US equity trading",
        false
    ).await.unwrap();
    
    // Check intent
    assert_eq!(result.intent.client.name, "Apex Capital");
    assert!(result.intent.markets.iter().any(|m| m.market_code == "XNYS"));
    
    // Check DSL validity
    assert!(result.dsl.source.contains("cbu.ensure"));
    assert!(result.dsl.source.contains("cbu-custody.add-universe"));
    assert!(result.dsl.source.contains("cbu-custody.create-ssi"));
    assert!(result.dsl.source.contains("cbu-custody.add-booking-rule"));
    assert!(result.dsl.source.contains("cbu-custody.validate-booking-coverage"));
}

#[tokio::test]
#[ignore]
async fn test_multi_market_with_cross_currency() {
    let client = create_test_client();
    let orchestrator = AgentOrchestrator::new(client, None).unwrap();
    
    let result = orchestrator.generate(
        "Onboard Global Fund for UK and Germany equities with USD cross-currency",
        false
    ).await.unwrap();
    
    // Check markets extracted
    assert!(result.intent.markets.len() >= 2);
    
    // Check cross-currency detected
    let has_usd_cross = result.intent.markets.iter()
        .any(|m| m.market_code != "XNYS" && m.currencies.contains(&"USD".to_string()));
    assert!(has_usd_cross);
}

#[tokio::test]
#[ignore]
async fn test_otc_with_isda() {
    let client = create_test_client();
    let orchestrator = AgentOrchestrator::new(client, None).unwrap();
    
    let result = orchestrator.generate(
        "Onboard Pacific Fund for US equities plus IRS exposure to Morgan Stanley under NY law ISDA with VM",
        false
    ).await.unwrap();
    
    // Check OTC detected
    assert!(!result.intent.otc_counterparties.is_empty());
    assert_eq!(result.intent.otc_counterparties[0].name, "Morgan Stanley");
    assert!(result.intent.otc_counterparties[0].csa_required);
    
    // Check ISDA in DSL
    assert!(result.dsl.source.contains("isda.create"));
    assert!(result.dsl.source.contains("isda.add-coverage"));
    assert!(result.dsl.source.contains("isda.add-csa"));
}

fn create_test_client() -> anthropic::Client {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY required for integration tests");
    anthropic::Client::new(&api_key)
}
```

**Effort**: Medium (1 day)

---

## Implementation Summary

### File Structure

```
rust/src/agentic/
├── mod.rs
├── intent.rs              # OnboardingIntent struct
├── patterns.rs            # OnboardingPattern enum
├── intent_extractor.rs    # NL → structured intent (Claude)
├── planner.rs             # Intent → requirements (deterministic Rust)
├── generator.rs           # Requirements → DSL (Claude)
├── validator.rs           # DSL validation
├── feedback.rs            # Retry loop
├── orchestrator.rs        # Main pipeline
├── prompts/
│   └── intent_extraction_system.md
├── schemas/
│   ├── custody_verbs.md   # Verb reference for prompts
│   └── reference_data.md  # Markets, BICs, etc.
├── examples/
│   ├── simple_equity.dsl
│   ├── multi_market.dsl
│   └── with_otc.dsl
└── tests/
    ├── mod.rs
    ├── intent_tests.rs
    └── planner_tests.rs
```

### Task Summary

| Phase | Tasks | Effort |
|-------|-------|--------|
| 1. Intent Extraction | 4 tasks | 2 days |
| 2. Requirement Derivation | 1 task | 1-2 days |
| 3. DSL Generation | 4 tasks | 2.5 days |
| 4. Validation & Feedback | 2 tasks | 1.5 days |
| 5. Orchestration & API | 3 tasks | 2 days |
| 6. Testing | 2 tasks | 2 days |
| **Total** | **16 tasks** | **11-13 days** |

### What We Removed (vs v1)

- ❌ Qdrant vector database setup
- ❌ Embedding generation
- ❌ Verb schema indexing
- ❌ Domain knowledge indexing
- ❌ Semantic search
- ❌ RAG retrieval logic

### What We Kept

- ✅ Intent extraction (Claude)
- ✅ Pattern classification (deterministic)
- ✅ Requirement derivation (deterministic Rust)
- ✅ DSL generation (Claude with full schemas in context)
- ✅ Validation feedback loop
- ✅ API and CLI integration

### Why This Is Better

| Aspect | v1 (with RAG) | v2 (direct) |
|--------|---------------|-------------|
| Complexity | High | Low |
| Reliability | Probabilistic retrieval | Deterministic |
| Latency | +200-500ms for search | No search overhead |
| Dependencies | Qdrant, embeddings | Just Claude API |
| Completeness | Might miss verbs | All schemas in context |
| Effort | 17-23 days | 11-13 days |

---

## Success Criteria

1. **Intent Extraction**: Correctly identifies client, markets, currencies, OTC in 90%+ of test cases
2. **Pattern Classification**: 100% deterministic based on intent fields
3. **Validation Pass Rate**: >90% first-attempt success
4. **Retry Success**: 100% success within 3 retries for valid requests
5. **Generated DSL**: Includes all required statements for the pattern
6. **Execution**: Successfully creates database records when executed

---

*End of Plan v2*
