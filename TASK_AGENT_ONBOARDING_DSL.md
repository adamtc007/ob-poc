# TASK: Agent DSL Generation for Onboarding

## Overview

Update the agent system to generate valid onboarding DSL from natural language requests. The agent should understand custody bank products, services, and resource types, and generate complete onboarding workflows.

**Goal:** User says "Onboard Acme Fund for Global Custody" → Agent generates valid, executable DSL.

---

## Current State

- ✅ Product/Service/Resource taxonomy seeded (029, 030 migrations)
- ✅ Resource instance verbs working (resource.create, resource.set-attr, resource.activate)
- ✅ Delivery verbs working (delivery.record, delivery.complete)
- ✅ 4 onboarding test scenarios passing
- ❌ Agent doesn't know about onboarding vocabulary
- ❌ No onboarding templates in registry

---

## Task 1: Update Agent System Prompt

Find the agent system prompt (likely in `rust/src/services/agent_service.rs` or `rust/src/agent/` directory) and add the onboarding context.

### Add to System Prompt:

```markdown
## Onboarding DSL Generation

You can generate DSL to onboard clients to financial services products. The taxonomy is:

**Product** → **Service** → **Resource Instance**

### Available Products

| Code | Name | Description |
|------|------|-------------|
| `GLOB_CUSTODY` | Global Custody | Asset safekeeping, settlement, corporate actions |
| `FUND_ACCT` | Fund Accounting | NAV calculation, investor accounting, reporting |
| `MO_IBOR` | Middle Office IBOR | Position management, trade capture, P&L attribution |

### Product → Service Mappings

**Global Custody (GLOB_CUSTODY):**
- `SAFEKEEPING` - Asset Safekeeping (mandatory)
- `SETTLEMENT` - Trade Settlement (mandatory)
- `CORP_ACTIONS` - Corporate Actions (mandatory)
- `INCOME_COLLECT` - Income Collection (optional)
- `PROXY_VOTING` - Proxy Voting (optional)
- `FX_EXECUTION` - FX Execution (optional)

**Fund Accounting (FUND_ACCT):**
- `NAV_CALC` - NAV Calculation (mandatory)
- `INVESTOR_ACCT` - Investor Accounting (mandatory)
- `FUND_REPORTING` - Fund Reporting (mandatory)
- `EXPENSE_MGMT` - Expense Management (optional)
- `PERF_MEASURE` - Performance Measurement (optional)

**Middle Office IBOR (MO_IBOR):**
- `POSITION_MGMT` - Position Management (mandatory)
- `TRADE_CAPTURE` - Trade Capture (mandatory)
- `PNL_ATTRIB` - P&L Attribution (mandatory)
- `CASH_MGMT` - Cash Management (optional)
- `COLLATERAL_MGMT` - Collateral Management (optional)

### Resource Types and Required Attributes

**CUSTODY_ACCT** (Custody Account):
- `account_number` (required) - Custody account number
- `account_name` (required) - Account display name
- `base_currency` (required) - Base currency (USD, EUR, GBP, etc.)
- `account_type` (required) - SEGREGATED or OMNIBUS
- `sub_custodian` (optional) - Sub-custodian name
- `market_codes` (optional) - JSON array of enabled markets

**SETTLE_ACCT** (Settlement Account):
- `account_number` (required) - Settlement account number
- `bic_code` (required) - BIC/SWIFT code
- `settlement_currency` (required) - Settlement currency
- `csd_participant_id` (optional) - CSD participant ID
- `netting_enabled` (optional) - Boolean

**SWIFT_CONN** (SWIFT Connection):
- `bic_code` (required) - SWIFT BIC
- `logical_terminal` (required) - Logical terminal ID
- `message_types` (required) - JSON array of MT types
- `rma_status` (optional) - RMA authorization status

**NAV_ENGINE** (NAV Calculation Engine):
- `fund_code` (required) - Fund identifier
- `valuation_frequency` (required) - DAILY, WEEKLY, or MONTHLY
- `pricing_source` (required) - Bloomberg, Reuters, ICE, etc.
- `nav_cutoff_time` (required) - Cutoff time with timezone
- `share_classes` (optional) - JSON share class config

**IBOR_SYSTEM** (IBOR System):
- `portfolio_code` (required) - Portfolio identifier
- `accounting_basis` (required) - TRADE_DATE or SETTLEMENT_DATE
- `base_currency` (required) - Reporting currency
- `position_source` (required) - Position source system
- `reconciliation_enabled` (optional) - Boolean

### Onboarding DSL Pattern

Always follow this sequence:

1. **Create CBU** with `cbu.ensure`
2. **Create Resource Instances** with `resource.create` for each required resource
3. **Set Attributes** with `resource.set-attr` for all required attributes
4. **Activate Resources** with `resource.activate`
5. **Record Deliveries** with `delivery.record` for each service
6. **Complete Deliveries** with `delivery.complete`

### Instance URL Convention

Generate instance URLs following this pattern:
- Custody: `https://custody.bank.com/accounts/{client-slug}-{seq}`
- Settlement: `https://settlement.bank.com/accounts/{client-slug}-{seq}`
- SWIFT: `https://swift.bank.com/connections/{bic-code}`
- NAV: `https://nav.fundservices.com/funds/{fund-code}`
- IBOR: `https://ibor.platform.com/portfolios/{portfolio-code}`

### Example: Global Custody Onboarding

User: "Onboard Apex Capital as a US hedge fund for Global Custody with a segregated USD account"

```clojure
;; Create the client
(cbu.ensure 
    :name "Apex Capital" 
    :jurisdiction "US" 
    :client-type "HEDGE_FUND"
    :as @apex)

;; Create Custody Account
(resource.create 
    :cbu-id @apex 
    :resource-type "CUSTODY_ACCT"
    :instance-url "https://custody.bank.com/accounts/apex-capital-001"
    :instance-id "APEX-CUSTODY-001"
    :instance-name "Apex Capital Custody Account"
    :as @custody)

;; Set required attributes
(resource.set-attr :instance-id @custody :attr "account_number" :value "CUST-APEX-001")
(resource.set-attr :instance-id @custody :attr "account_name" :value "Apex Capital - Main Custody")
(resource.set-attr :instance-id @custody :attr "base_currency" :value "USD")
(resource.set-attr :instance-id @custody :attr "account_type" :value "SEGREGATED")

;; Activate the resource
(resource.activate :instance-id @custody)

;; Record and complete delivery
(delivery.record :cbu-id @apex :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
(delivery.complete :cbu-id @apex :product "GLOB_CUSTODY" :service "SAFEKEEPING")
```

### Example: Fund Accounting Onboarding

User: "Set up Pacific Growth Fund for daily NAV calculation with Bloomberg pricing"

```clojure
(cbu.ensure 
    :name "Pacific Growth Fund" 
    :jurisdiction "LU" 
    :client-type "UCITS_FUND"
    :as @pgf)

(resource.create 
    :cbu-id @pgf 
    :resource-type "NAV_ENGINE"
    :instance-url "https://nav.fundservices.com/funds/pgf-001"
    :instance-id "PGF-NAV-001"
    :instance-name "Pacific Growth Fund NAV"
    :as @nav)

(resource.set-attr :instance-id @nav :attr "fund_code" :value "PGF-LU-001")
(resource.set-attr :instance-id @nav :attr "valuation_frequency" :value "DAILY")
(resource.set-attr :instance-id @nav :attr "pricing_source" :value "Bloomberg")
(resource.set-attr :instance-id @nav :attr "nav_cutoff_time" :value "16:00 CET")

(resource.activate :instance-id @nav)

(delivery.record :cbu-id @pgf :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
(delivery.complete :cbu-id @pgf :product "FUND_ACCT" :service "NAV_CALC")
```

### Client Types

Use these standardized client types:
- `HEDGE_FUND` - Hedge fund
- `PENSION_FUND` - Pension fund
- `ASSET_MANAGER` - Asset management company
- `UCITS_FUND` - UCITS fund (EU regulated)
- `PRIVATE_EQUITY` - Private equity fund
- `INSURANCE` - Insurance company
- `SOVEREIGN_WEALTH` - Sovereign wealth fund
- `ENDOWMENT` - Endowment or foundation

### Jurisdictions

Common jurisdiction codes:
- `US` - United States
- `UK` - United Kingdom
- `LU` - Luxembourg
- `IE` - Ireland
- `KY` - Cayman Islands
- `CH` - Switzerland
- `SG` - Singapore
- `HK` - Hong Kong

### Generation Guidelines

1. **Always generate all required attributes** - Don't skip required fields
2. **Use sensible defaults** - If user doesn't specify, use reasonable values
3. **Generate unique identifiers** - Use client name slug + sequence for IDs
4. **Include comments** - Add comments explaining what each section does
5. **Complete the flow** - Always include delivery.record and delivery.complete
6. **Validate before output** - Ensure the DSL would pass validation
```

---

## Task 2: Create Onboarding Templates

Add templates to the template registry. Find the template registry (likely `rust/src/templates/` or `rust/src/dsl_v2/templates/`).

### Template: Global Custody Onboarding

```rust
Template {
    id: "onboarding_global_custody",
    name: "Global Custody Onboarding",
    description: "Complete global custody setup with safekeeping and settlement",
    category: "onboarding",
    parameters: vec![
        TemplateParam { name: "client_name", display: "Client Name", param_type: ParamType::String, required: true, default: None },
        TemplateParam { name: "jurisdiction", display: "Jurisdiction", param_type: ParamType::String, required: true, default: Some("US".into()) },
        TemplateParam { name: "client_type", display: "Client Type", param_type: ParamType::Enum, required: true, 
                       options: vec!["HEDGE_FUND", "PENSION_FUND", "ASSET_MANAGER", "INSURANCE"], default: Some("HEDGE_FUND".into()) },
        TemplateParam { name: "base_currency", display: "Base Currency", param_type: ParamType::String, required: true, default: Some("USD".into()) },
        TemplateParam { name: "account_type", display: "Account Type", param_type: ParamType::Enum, required: true,
                       options: vec!["SEGREGATED", "OMNIBUS"], default: Some("SEGREGATED".into()) },
        TemplateParam { name: "include_settlement", display: "Include Settlement", param_type: ParamType::Boolean, required: false, default: Some("true".into()) },
        TemplateParam { name: "include_swift", display: "Include SWIFT", param_type: ParamType::Boolean, required: false, default: Some("false".into()) },
    ],
    template_body: r#"
;; Global Custody Onboarding for {{client_name}}
;; Generated: {{timestamp}}

;; 1. Create Client Business Unit
(cbu.ensure 
    :name "{{client_name}}" 
    :jurisdiction "{{jurisdiction}}" 
    :client-type "{{client_type}}"
    :as @client)

;; 2. Create Custody Account
(resource.create 
    :cbu-id @client 
    :resource-type "CUSTODY_ACCT"
    :instance-url "https://custody.bank.com/accounts/{{client_name | slugify}}-001"
    :instance-id "{{client_name | slugify | uppercase}}-CUSTODY-001"
    :instance-name "{{client_name}} Custody Account"
    :as @custody)

(resource.set-attr :instance-id @custody :attr "account_number" :value "CUST-{{client_name | slugify | uppercase}}-001")
(resource.set-attr :instance-id @custody :attr "account_name" :value "{{client_name}} - Main Custody")
(resource.set-attr :instance-id @custody :attr "base_currency" :value "{{base_currency}}")
(resource.set-attr :instance-id @custody :attr "account_type" :value "{{account_type}}")

(resource.activate :instance-id @custody)

(delivery.record :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
(delivery.complete :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING")

{{#if include_settlement}}
;; 3. Create Settlement Account
(resource.create 
    :cbu-id @client 
    :resource-type "SETTLE_ACCT"
    :instance-url "https://settlement.bank.com/accounts/{{client_name | slugify}}-001"
    :instance-id "{{client_name | slugify | uppercase}}-SETTLE-001"
    :as @settlement)

(resource.set-attr :instance-id @settlement :attr "account_number" :value "SETT-{{client_name | slugify | uppercase}}-001")
(resource.set-attr :instance-id @settlement :attr "bic_code" :value "CUSTUS33XXX")
(resource.set-attr :instance-id @settlement :attr "settlement_currency" :value "{{base_currency}}")

(resource.activate :instance-id @settlement)

(delivery.record :cbu-id @client :product "GLOB_CUSTODY" :service "SETTLEMENT" :instance-id @settlement)
(delivery.complete :cbu-id @client :product "GLOB_CUSTODY" :service "SETTLEMENT")
{{/if}}

{{#if include_swift}}
;; 4. Create SWIFT Connection
(resource.create 
    :cbu-id @client 
    :resource-type "SWIFT_CONN"
    :instance-url "https://swift.bank.com/connections/{{client_name | slugify | uppercase}}US33"
    :instance-id "{{client_name | slugify | uppercase}}US33"
    :as @swift)

(resource.set-attr :instance-id @swift :attr "bic_code" :value "{{client_name | slugify | uppercase}}US33XXX")
(resource.set-attr :instance-id @swift :attr "logical_terminal" :value "{{client_name | slugify | uppercase}}US33AXXX")
(resource.set-attr :instance-id @swift :attr "message_types" :value "[\"MT540\", \"MT541\", \"MT542\", \"MT543\", \"MT950\"]")

(resource.activate :instance-id @swift)
{{/if}}
"#,
}
```

### Template: Fund Accounting Onboarding

```rust
Template {
    id: "onboarding_fund_accounting",
    name: "Fund Accounting Onboarding",
    description: "NAV calculation and investor accounting setup",
    category: "onboarding",
    parameters: vec![
        TemplateParam { name: "fund_name", display: "Fund Name", param_type: ParamType::String, required: true, default: None },
        TemplateParam { name: "jurisdiction", display: "Jurisdiction", param_type: ParamType::String, required: true, default: Some("LU".into()) },
        TemplateParam { name: "fund_type", display: "Fund Type", param_type: ParamType::Enum, required: true,
                       options: vec!["UCITS_FUND", "HEDGE_FUND", "PRIVATE_EQUITY"], default: Some("UCITS_FUND".into()) },
        TemplateParam { name: "valuation_frequency", display: "Valuation Frequency", param_type: ParamType::Enum, required: true,
                       options: vec!["DAILY", "WEEKLY", "MONTHLY"], default: Some("DAILY".into()) },
        TemplateParam { name: "pricing_source", display: "Pricing Source", param_type: ParamType::Enum, required: true,
                       options: vec!["Bloomberg", "Reuters", "ICE"], default: Some("Bloomberg".into()) },
        TemplateParam { name: "nav_cutoff", display: "NAV Cutoff Time", param_type: ParamType::String, required: true, default: Some("16:00 CET".into()) },
        TemplateParam { name: "include_investor_accounting", display: "Include Investor Accounting", param_type: ParamType::Boolean, required: false, default: Some("true".into()) },
    ],
    template_body: r#"
;; Fund Accounting Onboarding for {{fund_name}}
;; Generated: {{timestamp}}

;; 1. Create Fund as CBU
(cbu.ensure 
    :name "{{fund_name}}" 
    :jurisdiction "{{jurisdiction}}" 
    :client-type "{{fund_type}}"
    :as @fund)

;; 2. Create NAV Calculation Engine
(resource.create 
    :cbu-id @fund 
    :resource-type "NAV_ENGINE"
    :instance-url "https://nav.fundservices.com/funds/{{fund_name | slugify}}"
    :instance-id "{{fund_name | slugify | uppercase}}-NAV-001"
    :instance-name "{{fund_name}} NAV Engine"
    :as @nav)

(resource.set-attr :instance-id @nav :attr "fund_code" :value "{{fund_name | slugify | uppercase}}-001")
(resource.set-attr :instance-id @nav :attr "valuation_frequency" :value "{{valuation_frequency}}")
(resource.set-attr :instance-id @nav :attr "pricing_source" :value "{{pricing_source}}")
(resource.set-attr :instance-id @nav :attr "nav_cutoff_time" :value "{{nav_cutoff}}")

(resource.activate :instance-id @nav)

(delivery.record :cbu-id @fund :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
(delivery.complete :cbu-id @fund :product "FUND_ACCT" :service "NAV_CALC")

{{#if include_investor_accounting}}
;; 3. Create Investor Ledger
(resource.create 
    :cbu-id @fund 
    :resource-type "INVESTOR_LEDGER"
    :instance-url "https://ta.fundservices.com/funds/{{fund_name | slugify}}"
    :instance-id "{{fund_name | slugify | uppercase}}-TA-001"
    :instance-name "{{fund_name}} Investor Ledger"
    :as @ledger)

(resource.activate :instance-id @ledger)

(delivery.record :cbu-id @fund :product "FUND_ACCT" :service "INVESTOR_ACCT" :instance-id @ledger)
(delivery.complete :cbu-id @fund :product "FUND_ACCT" :service "INVESTOR_ACCT")
{{/if}}
"#,
}
```

### Template: Middle Office IBOR Onboarding

```rust
Template {
    id: "onboarding_ibor",
    name: "Middle Office IBOR Onboarding",
    description: "IBOR system and P&L attribution setup",
    category: "onboarding",
    parameters: vec![
        TemplateParam { name: "client_name", display: "Client Name", param_type: ParamType::String, required: true, default: None },
        TemplateParam { name: "jurisdiction", display: "Jurisdiction", param_type: ParamType::String, required: true, default: Some("UK".into()) },
        TemplateParam { name: "client_type", display: "Client Type", param_type: ParamType::Enum, required: true,
                       options: vec!["ASSET_MANAGER", "HEDGE_FUND", "PENSION_FUND"], default: Some("ASSET_MANAGER".into()) },
        TemplateParam { name: "base_currency", display: "Base Currency", param_type: ParamType::String, required: true, default: Some("GBP".into()) },
        TemplateParam { name: "accounting_basis", display: "Accounting Basis", param_type: ParamType::Enum, required: true,
                       options: vec!["TRADE_DATE", "SETTLEMENT_DATE"], default: Some("TRADE_DATE".into()) },
        TemplateParam { name: "position_source", display: "Position Source", param_type: ParamType::String, required: true, default: Some("OMS".into()) },
        TemplateParam { name: "include_pnl", display: "Include P&L Attribution", param_type: ParamType::Boolean, required: false, default: Some("true".into()) },
    ],
    template_body: r#"
;; Middle Office IBOR Onboarding for {{client_name}}
;; Generated: {{timestamp}}

;; 1. Create Client Business Unit
(cbu.ensure 
    :name "{{client_name}}" 
    :jurisdiction "{{jurisdiction}}" 
    :client-type "{{client_type}}"
    :as @client)

;; 2. Create IBOR System
(resource.create 
    :cbu-id @client 
    :resource-type "IBOR_SYSTEM"
    :instance-url "https://ibor.platform.com/portfolios/{{client_name | slugify}}"
    :instance-id "{{client_name | slugify | uppercase}}-IBOR-001"
    :instance-name "{{client_name}} IBOR"
    :as @ibor)

(resource.set-attr :instance-id @ibor :attr "portfolio_code" :value "{{client_name | slugify | uppercase}}-MASTER")
(resource.set-attr :instance-id @ibor :attr "accounting_basis" :value "{{accounting_basis}}")
(resource.set-attr :instance-id @ibor :attr "base_currency" :value "{{base_currency}}")
(resource.set-attr :instance-id @ibor :attr "position_source" :value "{{position_source}}")

(resource.activate :instance-id @ibor)

(delivery.record :cbu-id @client :product "MO_IBOR" :service "POSITION_MGMT" :instance-id @ibor)
(delivery.record :cbu-id @client :product "MO_IBOR" :service "TRADE_CAPTURE" :instance-id @ibor)
(delivery.complete :cbu-id @client :product "MO_IBOR" :service "POSITION_MGMT")
(delivery.complete :cbu-id @client :product "MO_IBOR" :service "TRADE_CAPTURE")

{{#if include_pnl}}
;; 3. Create P&L Engine
(resource.create 
    :cbu-id @client 
    :resource-type "PNL_ENGINE"
    :instance-url "https://pnl.platform.com/portfolios/{{client_name | slugify}}"
    :instance-id "{{client_name | slugify | uppercase}}-PNL-001"
    :instance-name "{{client_name}} P&L Engine"
    :as @pnl)

(resource.activate :instance-id @pnl)

(delivery.record :cbu-id @client :product "MO_IBOR" :service "PNL_ATTRIB" :instance-id @pnl)
(delivery.complete :cbu-id @client :product "MO_IBOR" :service "PNL_ATTRIB")
{{/if}}
"#,
}
```

---

## Task 3: Add Template Rendering

If templates don't already have a rendering system, add one. Create `rust/src/templates/renderer.rs`:

```rust
//! Template Renderer
//! 
//! Renders onboarding templates with parameter substitution

use anyhow::{Context, Result};
use handlebars::Handlebars;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub struct TemplateRenderer {
    handlebars: Handlebars<'static>,
}

impl TemplateRenderer {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        
        // Register custom helpers
        handlebars.register_helper("slugify", Box::new(slugify_helper));
        handlebars.register_helper("uppercase", Box::new(uppercase_helper));
        handlebars.register_helper("timestamp", Box::new(timestamp_helper));
        
        Self { handlebars }
    }
    
    pub fn render(&self, template: &str, params: &HashMap<String, String>) -> Result<String> {
        let json_params: JsonValue = serde_json::to_value(params)?;
        
        self.handlebars
            .render_template(template, &json_params)
            .context("Failed to render template")
    }
}

// Helper: slugify
fn slugify_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let slug = param
        .to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");
    out.write(&slug)?;
    Ok(())
}

// Helper: uppercase
fn uppercase_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    out.write(&param.to_uppercase())?;
    Ok(())
}

// Helper: timestamp
fn timestamp_helper(
    _: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    out.write(&chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string())?;
    Ok(())
}
```

Add to `Cargo.toml` if not present:
```toml
handlebars = "4.5"
```

---

## Task 4: Agent Integration Tests

Create `rust/tests/agent_onboarding_tests.rs`:

```rust
//! Agent Onboarding DSL Generation Tests
//!
//! Tests that the agent generates valid onboarding DSL from natural language

use ob_poc::agent::AgentService;
use ob_poc::dsl_v2::{parse_program, validate_program};

/// Test: Global Custody onboarding request
#[tokio::test]
async fn test_agent_generates_global_custody_dsl() {
    let agent = AgentService::new_test();
    
    let request = "Onboard Blackstone Capital as a US hedge fund for Global Custody. \
                   They need a segregated USD account.";
    
    let response = agent.generate_dsl(request).await.unwrap();
    
    // Verify DSL is valid
    let ast = parse_program(&response.dsl).expect("Generated DSL should parse");
    let validation = validate_program(&ast);
    assert!(validation.is_ok(), "Generated DSL should validate: {:?}", validation.err());
    
    // Verify key elements present
    assert!(response.dsl.contains("cbu.ensure"), "Should create CBU");
    assert!(response.dsl.contains("Blackstone Capital"), "Should include client name");
    assert!(response.dsl.contains("HEDGE_FUND"), "Should set client type");
    assert!(response.dsl.contains("CUSTODY_ACCT"), "Should create custody account");
    assert!(response.dsl.contains("SEGREGATED"), "Should set account type");
    assert!(response.dsl.contains("USD"), "Should set currency");
    assert!(response.dsl.contains("resource.activate"), "Should activate resource");
    assert!(response.dsl.contains("delivery.record"), "Should record delivery");
}

/// Test: Fund Accounting onboarding request
#[tokio::test]
async fn test_agent_generates_fund_accounting_dsl() {
    let agent = AgentService::new_test();
    
    let request = "Set up Alpine Growth Fund as a Luxembourg UCITS for daily NAV \
                   with Bloomberg pricing, cutoff at 4pm CET.";
    
    let response = agent.generate_dsl(request).await.unwrap();
    
    let ast = parse_program(&response.dsl).expect("Generated DSL should parse");
    let validation = validate_program(&ast);
    assert!(validation.is_ok(), "Generated DSL should validate");
    
    assert!(response.dsl.contains("Alpine Growth Fund"), "Should include fund name");
    assert!(response.dsl.contains("LU"), "Should set Luxembourg jurisdiction");
    assert!(response.dsl.contains("NAV_ENGINE"), "Should create NAV engine");
    assert!(response.dsl.contains("DAILY"), "Should set daily frequency");
    assert!(response.dsl.contains("Bloomberg"), "Should set pricing source");
}

/// Test: IBOR onboarding request
#[tokio::test]
async fn test_agent_generates_ibor_dsl() {
    let agent = AgentService::new_test();
    
    let request = "Onboard Quantum Asset Management for Middle Office IBOR. \
                   UK-based asset manager, trade date accounting, GBP base currency.";
    
    let response = agent.generate_dsl(request).await.unwrap();
    
    let ast = parse_program(&response.dsl).expect("Generated DSL should parse");
    let validation = validate_program(&ast);
    assert!(validation.is_ok(), "Generated DSL should validate");
    
    assert!(response.dsl.contains("Quantum Asset Management"), "Should include client name");
    assert!(response.dsl.contains("UK"), "Should set UK jurisdiction");
    assert!(response.dsl.contains("IBOR_SYSTEM"), "Should create IBOR system");
    assert!(response.dsl.contains("TRADE_DATE"), "Should set trade date accounting");
    assert!(response.dsl.contains("GBP"), "Should set GBP currency");
}

/// Test: Multi-product onboarding request
#[tokio::test]
async fn test_agent_generates_multi_product_dsl() {
    let agent = AgentService::new_test();
    
    let request = "Onboard Atlas Pension Fund for both Global Custody and Fund Accounting. \
                   US pension fund, USD base, daily NAV with Reuters pricing.";
    
    let response = agent.generate_dsl(request).await.unwrap();
    
    let ast = parse_program(&response.dsl).expect("Generated DSL should parse");
    let validation = validate_program(&ast);
    assert!(validation.is_ok(), "Generated DSL should validate");
    
    // Should have both products
    assert!(response.dsl.contains("CUSTODY_ACCT") || response.dsl.contains("GLOB_CUSTODY"), 
            "Should include custody");
    assert!(response.dsl.contains("NAV_ENGINE") || response.dsl.contains("FUND_ACCT"), 
            "Should include fund accounting");
}

/// Test: Ambiguous request gets clarification
#[tokio::test]
async fn test_agent_handles_ambiguous_request() {
    let agent = AgentService::new_test();
    
    let request = "Onboard a new client for custody services";
    
    let response = agent.generate_dsl(request).await.unwrap();
    
    // Should either generate with defaults or ask for clarification
    assert!(
        response.dsl.contains("cbu.ensure") || response.needs_clarification,
        "Should either generate DSL or request clarification"
    );
}
```

---

## Task 5: API Endpoint for Agent Onboarding

Add endpoint to `rust/src/ui/routes.rs` or API routes:

```rust
/// POST /api/agent/onboard
/// 
/// Generate onboarding DSL from natural language
#[derive(Deserialize)]
pub struct OnboardingRequest {
    pub description: String,
    pub execute: Option<bool>,  // If true, execute the DSL after generation
}

#[derive(Serialize)]
pub struct OnboardingResponse {
    pub dsl: String,
    pub explanation: String,
    pub validation_result: ValidationResult,
    pub execution_result: Option<ExecutionResult>,
}

pub async fn generate_onboarding_dsl(
    State(state): State<AppState>,
    Json(request): Json<OnboardingRequest>,
) -> Result<Json<OnboardingResponse>, ApiError> {
    let agent = &state.agent_service;
    
    // Generate DSL
    let generated = agent.generate_dsl(&request.description).await?;
    
    // Validate
    let ast = parse_program(&generated.dsl)?;
    let validation = validate_program(&ast)?;
    
    // Optionally execute
    let execution_result = if request.execute.unwrap_or(false) {
        let executor = DslExecutor::new(state.pool.clone());
        Some(executor.execute_with_result(&ast).await?)
    } else {
        None
    };
    
    Ok(Json(OnboardingResponse {
        dsl: generated.dsl,
        explanation: generated.explanation,
        validation_result: validation,
        execution_result,
    }))
}

// Add route
.route("/api/agent/onboard", post(generate_onboarding_dsl))
```

---

## Execution Checklist

### Task 1: Agent System Prompt
- [ ] Locate agent system prompt file
- [ ] Add onboarding context (products, services, resources, attributes)
- [ ] Add example DSL patterns
- [ ] Add generation guidelines

### Task 2: Templates
- [ ] Locate or create template registry
- [ ] Add `onboarding_global_custody` template
- [ ] Add `onboarding_fund_accounting` template
- [ ] Add `onboarding_ibor` template

### Task 3: Template Rendering
- [ ] Add `handlebars` to Cargo.toml (if needed)
- [ ] Create template renderer with helpers (slugify, uppercase, timestamp)
- [ ] Test template rendering

### Task 4: Integration Tests
- [ ] Create `rust/tests/agent_onboarding_tests.rs`
- [ ] Test global custody generation
- [ ] Test fund accounting generation
- [ ] Test IBOR generation
- [ ] Test multi-product generation

### Task 5: API Endpoint
- [ ] Add `/api/agent/onboard` endpoint
- [ ] Connect to agent service
- [ ] Add validation and optional execution

### Verification
- [ ] `cargo test agent_onboarding` passes
- [ ] Manual test via API: `curl -X POST /api/agent/onboard -d '{"description": "..."}'`
- [ ] Generated DSL validates
- [ ] Generated DSL executes successfully

---

## Test Commands

```bash
# Run agent tests
cargo test --features database agent_onboarding

# Manual API test
curl -X POST http://localhost:3000/api/agent/onboard \
  -H "Content-Type: application/json" \
  -d '{"description": "Onboard Acme Fund for Global Custody with USD segregated account"}'

# Validate generated DSL
cargo run --bin ob-poc -- dsl validate --input generated.dsl

# Execute generated DSL
cargo run --bin ob-poc -- dsl execute --input generated.dsl
```

---

## Example End-to-End Flow

```
User Input:
"Onboard Meridian Partners as a UK hedge fund for Global Custody 
 and Middle Office IBOR. GBP base currency, trade date accounting."

Agent Output:
```clojure
;; Onboarding: Meridian Partners
;; Products: Global Custody, Middle Office IBOR
;; Generated: 2024-01-15 10:30:00 UTC

(cbu.ensure 
    :name "Meridian Partners" 
    :jurisdiction "UK" 
    :client-type "HEDGE_FUND"
    :as @meridian)

;; === GLOBAL CUSTODY ===
(resource.create 
    :cbu-id @meridian 
    :resource-type "CUSTODY_ACCT"
    :instance-url "https://custody.bank.com/accounts/meridian-partners-001"
    :instance-id "MERIDIAN-CUSTODY-001"
    :as @custody)

(resource.set-attr :instance-id @custody :attr "account_number" :value "CUST-MERIDIAN-001")
(resource.set-attr :instance-id @custody :attr "account_name" :value "Meridian Partners - Main Custody")
(resource.set-attr :instance-id @custody :attr "base_currency" :value "GBP")
(resource.set-attr :instance-id @custody :attr "account_type" :value "SEGREGATED")
(resource.activate :instance-id @custody)

(delivery.record :cbu-id @meridian :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
(delivery.complete :cbu-id @meridian :product "GLOB_CUSTODY" :service "SAFEKEEPING")

;; === MIDDLE OFFICE IBOR ===
(resource.create 
    :cbu-id @meridian 
    :resource-type "IBOR_SYSTEM"
    :instance-url "https://ibor.platform.com/portfolios/meridian-partners"
    :instance-id "MERIDIAN-IBOR-001"
    :as @ibor)

(resource.set-attr :instance-id @ibor :attr "portfolio_code" :value "MERIDIAN-MASTER")
(resource.set-attr :instance-id @ibor :attr "accounting_basis" :value "TRADE_DATE")
(resource.set-attr :instance-id @ibor :attr "base_currency" :value "GBP")
(resource.set-attr :instance-id @ibor :attr "position_source" :value "OMS")
(resource.activate :instance-id @ibor)

(delivery.record :cbu-id @meridian :product "MO_IBOR" :service "POSITION_MGMT" :instance-id @ibor)
(delivery.record :cbu-id @meridian :product "MO_IBOR" :service "TRADE_CAPTURE" :instance-id @ibor)
(delivery.complete :cbu-id @meridian :product "MO_IBOR" :service "POSITION_MGMT")
(delivery.complete :cbu-id @meridian :product "MO_IBOR" :service "TRADE_CAPTURE")
```
