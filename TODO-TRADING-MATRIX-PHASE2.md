# TODO: Trading Matrix Vision Completion
## Phase 2: Beyond Code-Complete

**Created:** December 22, 2025  
**Prerequisite:** `TODO-TRADING-MATRIX-IMPLEMENTATION.md` complete (all checkboxes ticked)  
**Scope:** Reference data, visualization, agent persona, document extraction, workflow integration  
**Estimated Effort:** 3-4 weeks  

**Reference Docs:**
- `docs/TRADING_MATRIX_STORAGE_ARCHITECTURE.md`
- `rust/config/seed/trading_profiles/allianzgi_complete.yaml` (example profile)

---

## Pre-Phase Checklist

Before starting Phase 2, confirm Phase 1 complete:
- [ ] All 10 new tables exist and functional
- [ ] All 4 verb domains working (investment-manager, sla, pricing-config, cash-sweep)
- [ ] `trading-profile.materialize` populates new tables
- [ ] Basic tests passing
- [ ] Can execute `rust/examples/trading_matrix_e2e.dsl` without errors

---

## Phase 11: Reference Data Population (Days 1-3)

### 11.1 Markets Reference Data
**File:** `rust/config/seed/reference_data/markets.yaml`

Create comprehensive market reference data:

```yaml
# ISO 10383 MIC codes - key global markets
markets:
  # Americas
  - mic: XNYS
    name: New York Stock Exchange
    country_code: US
    primary_currency: USD
    csd_bic: DTCYUS33
    timezone: America/New_York
    settlement_cycle: T+1
    
  - mic: XNAS
    name: NASDAQ
    country_code: US
    primary_currency: USD
    csd_bic: DTCYUS33
    timezone: America/New_York
    settlement_cycle: T+1
    
  - mic: XTSE
    name: Toronto Stock Exchange
    country_code: CA
    primary_currency: CAD
    csd_bic: CDABORDC
    timezone: America/Toronto
    
  # Europe
  - mic: XLON
    name: London Stock Exchange
    country_code: GB
    primary_currency: GBP
    csd_bic: CABORDC1
    timezone: Europe/London
    
  - mic: XETR
    name: XETRA (Deutsche Börse)
    country_code: DE
    primary_currency: EUR
    csd_bic: DAABORDC
    timezone: Europe/Berlin
    
  # ... continue for ~50 key markets
```

- [ ] Create markets.yaml with 50+ key global markets
- [ ] Include: Americas (10), Europe (15), Asia-Pacific (15), Middle East (5), Other (5+)
- [ ] Create loader verb: `refdata.load-markets`
- [ ] Execute load and verify

### 11.2 Instrument Classes Taxonomy
**File:** `rust/config/seed/reference_data/instrument_classes.yaml`

```yaml
# Complete instrument taxonomy with CFI/SMPG/ISDA mappings
instrument_classes:
  # Equities
  - code: EQUITY
    name: Listed Equity
    parent: null
    settlement_cycle: T+1
    cfi_category: E
    smpg_group: EQUITIES
    requires_isda: false
    children:
      - code: EQUITY_COMMON
        name: Common Stock
        cfi_prefixes: ["ES"]
      - code: EQUITY_PREFERRED
        name: Preferred Stock
        cfi_prefixes: ["EP"]
      - code: EQUITY_ADR
        name: American Depositary Receipt
        cfi_prefixes: ["ED"]
        
  # Fixed Income
  - code: FIXED_INCOME
    name: Fixed Income
    parent: null
    children:
      - code: GOVT_BOND
        name: Government Bond
        settlement_cycle: T+1
        cfi_category: D
        cfi_group: B
        smpg_group: GOVERNMENT_BONDS
      - code: CORP_BOND
        name: Corporate Bond
        settlement_cycle: T+2
        smpg_group: CORPORATE_BONDS
      - code: MUNI_BOND
        name: Municipal Bond
        settlement_cycle: T+1
        
  # Derivatives (OTC)
  - code: OTC_DERIVATIVE
    name: OTC Derivative
    requires_isda: true
    children:
      - code: IRS
        name: Interest Rate Swap
        isda_asset_class: RATES
        isda_base_product: SWAP
      - code: CDS
        name: Credit Default Swap
        isda_asset_class: CREDIT
        isda_base_product: CDS
      - code: FX_FORWARD
        name: FX Forward
        isda_asset_class: FX
        isda_base_product: FORWARD
        
  # Cash/Money Market
  - code: STIF
    name: Short-Term Investment Fund
    settlement_cycle: T+0
    sweep_eligible: true
  - code: MMF
    name: Money Market Fund
    settlement_cycle: T+0
    sweep_eligible: true
  - code: REPO
    name: Repurchase Agreement
    settlement_cycle: T+0
```

- [ ] Create instrument_classes.yaml with full taxonomy (~40 classes)
- [ ] Include CFI mappings for listed instruments
- [ ] Include ISDA asset class/base product for OTC
- [ ] Create hierarchical structure (parent/children)
- [ ] Create loader verb: `refdata.load-instrument-classes`
- [ ] Execute load and verify

### 11.3 Subcustodian Network
**File:** `rust/config/seed/reference_data/subcustodian_network.yaml`

```yaml
# BNY's subcustodian network by market
# NOTE: This is illustrative - real data from platform team
subcustodian_network:
  # Americas
  - market_mic: XNYS
    subcustodian_bic: IRVTUS3N
    subcustodian_name: BNY Mellon (Direct)
    csd_bic: DTCYUS33
    is_direct: true
    
  - market_mic: XTSE
    subcustodian_bic: ROABORDC
    subcustodian_name: RBC Investor Services
    csd_bic: CDABORDC
    
  # Europe  
  - market_mic: XLON
    subcustodian_bic: HABORDC1
    subcustodian_name: HSBC Securities Services
    csd_bic: CABORDC1
    
  - market_mic: XETR
    subcustodian_bic: DEUTDEFF
    subcustodian_name: Deutsche Bank AG
    csd_bic: DAKVDEFF
    
  - market_mic: XPAR
    subcustodian_bic: ABORDC2P
    subcustodian_name: BNP Paribas Securities Services
    csd_bic: SICABORDC
    
  # Asia-Pacific
  - market_mic: XHKG
    subcustodian_bic: HSBCHKHH
    subcustodian_name: HSBC Hong Kong
    csd_bic: CCASCHKX
    
  - market_mic: XTKS
    subcustodian_bic: MABORJPJ
    subcustodian_name: Mizuho Bank
    csd_bic: JASDECJP
```

- [ ] Create subcustodian_network.yaml for 50+ markets
- [ ] Include BIC, name, CSD for each
- [ ] Flag direct vs agent markets
- [ ] Create loader verb: `refdata.load-subcustodians`
- [ ] Execute load and verify

### 11.4 SLA Templates (Realistic)
**File:** `rust/config/seed/reference_data/sla_templates.yaml`

```yaml
# Production SLA templates by service
sla_templates:
  # Custody SLAs
  - template_code: CUSTODY_DVP_SETTLE_RATE
    name: DVP Settlement Success Rate
    applies_to_type: SERVICE
    applies_to_code: CUSTODY
    metric_code: SETTLEMENT_RATE
    target_value: 99.50
    warning_threshold: 98.50
    measurement_period: MONTHLY
    description: "Percentage of DVP instructions settled on intended date"
    
  - template_code: CUSTODY_INSTR_STP_RATE
    name: Instruction STP Rate
    applies_to_type: SERVICE
    applies_to_code: CUSTODY
    metric_code: SETTLEMENT_RATE
    target_value: 95.00
    warning_threshold: 90.00
    measurement_period: MONTHLY
    description: "Percentage of instructions processed without manual intervention"
    
  - template_code: CUSTODY_CORP_ACTION_NOTIFY
    name: Corporate Action Notification
    applies_to_type: SERVICE
    applies_to_code: CUSTODY
    metric_code: NAV_DELIVERY_TIME
    target_value: 24
    warning_threshold: 48
    measurement_period: DAILY
    description: "Hours after announcement to notify client"
    
  # Fund Accounting SLAs
  - template_code: FA_NAV_DELIVERY_DAILY
    name: Daily NAV Delivery
    applies_to_type: SERVICE
    applies_to_code: FUND_ACCOUNTING
    metric_code: NAV_DELIVERY_TIME
    target_value: 18.00
    warning_threshold: 19.00
    measurement_period: DAILY
    description: "NAV delivered by 18:00 local time"
    regulatory_requirement: true
    regulatory_reference: "UCITS Art. 76"
    
  - template_code: FA_PRICE_COMPLETENESS
    name: Price Completeness
    applies_to_type: SERVICE
    applies_to_code: FUND_ACCOUNTING
    metric_code: PRICE_AVAILABILITY
    target_value: 100.00
    warning_threshold: 99.00
    measurement_period: DAILY
    
  # Middle Office SLAs
  - template_code: MO_TRADE_MATCH_RATE
    name: Trade Matching Rate
    applies_to_type: SERVICE
    applies_to_code: MIDDLE_OFFICE
    metric_code: MATCH_RATE
    target_value: 98.00
    warning_threshold: 95.00
    measurement_period: MONTHLY
    
  # OTC/Collateral SLAs
  - template_code: COLL_MARGIN_CALL_SAME_DAY
    name: Same-Day Margin Call Settlement
    applies_to_type: CSA
    metric_code: MARGIN_CALL_TIMELINESS
    target_value: 4.00
    warning_threshold: 6.00
    measurement_period: DAILY
    description: "Margin calls settled within 4 hours of receipt"
```

- [ ] Expand SLA templates to 25+ covering all services
- [ ] Include regulatory SLAs (UCITS, AIFMD requirements)
- [ ] Add penalty structures where applicable
- [ ] Create loader verb: `refdata.load-sla-templates`
- [ ] Execute load and verify

### 11.5 Reference Data Loader Verbs
**File:** `rust/config/verbs/refdata/bulk-load.yaml`

```yaml
domains:
  refdata:
    description: "Reference data bulk loading operations"
    verbs:
      load-markets:
        description: Load markets from YAML file
        behavior: plugin
        handler: load_markets_from_yaml
        args:
          - name: file-path
            type: string
            required: true
          - name: mode
            type: string
            default: UPSERT
            valid_values: [INSERT, UPSERT, REPLACE]
            
      load-instrument-classes:
        description: Load instrument class taxonomy from YAML
        behavior: plugin
        handler: load_instrument_classes_from_yaml
        args:
          - name: file-path
            type: string
            required: true
          - name: mode
            type: string
            default: UPSERT
            
      load-subcustodians:
        description: Load subcustodian network from YAML
        behavior: plugin
        handler: load_subcustodians_from_yaml
        args:
          - name: file-path
            type: string
            required: true
            
      load-sla-templates:
        description: Load SLA templates from YAML
        behavior: plugin
        handler: load_sla_templates_from_yaml
        args:
          - name: file-path
            type: string
            required: true
```

- [ ] Create bulk-load.yaml verb definitions
- [ ] Implement plugin handlers in `rust/src/plugins/refdata_loader.rs`
- [ ] Test each loader
- [ ] Create master load script: `rust/scripts/load_all_refdata.sh`

### 11.6 Verify Reference Data
```sql
-- Verification queries
SELECT COUNT(*) as market_count FROM custody.markets;
-- Expected: 50+

SELECT COUNT(*) as instrument_class_count FROM custody.instrument_classes;
-- Expected: 40+

SELECT COUNT(*) as subcustodian_count FROM custody.subcustodian_network;
-- Expected: 50+

SELECT COUNT(*) as sla_template_count FROM "ob-poc".sla_templates;
-- Expected: 25+
```

- [ ] All counts meet expectations
- [ ] Foreign key references valid
- [ ] No orphaned records

---

## Phase 12: Test Scenarios (Days 3-5)

### 12.1 Scenario: Simple Single-IM Equity Fund
**File:** `rust/examples/scenarios/01_simple_equity_fund.dsl`

```clojure
; =============================================================================
; SCENARIO: Simple Equity Fund
; Single IM, US equities only, basic custody
; =============================================================================

; --- Setup ---
(cbu.create 
  :name "Simple US Equity Fund" 
  :jurisdiction "US" 
  :cbu-category FUND_MANDATE
  :client-type "REGISTERED_FUND")
→ @fund

; --- Trading Profile (inline for simple case) ---
(trading-profile.import :cbu-id @fund :document {
  :universe {
    :base_currency "USD"
    :allowed_currencies ["USD"]
    :allowed_markets [{:mic "XNYS" :currencies ["USD"] :settlement_types ["DVP"]}
                      {:mic "XNAS" :currencies ["USD"] :settlement_types ["DVP"]}]
    :instrument_classes [{:class_code "EQUITY" :is_held true :is_traded true}
                         {:class_code "ETF" :is_held true :is_traded true}]
  }
  :investment_managers [{
    :priority 100
    :manager {:type "LEI" :value "5493001SIMPLEIMGR01"}
    :role "INVESTMENT_MANAGER"
    :scope {:all true}
    :instruction_method "SWIFT"
    :can_trade true
    :can_settle true
  }]
  :pricing_matrix [{
    :priority 1
    :scope {:instrument_classes ["EQUITY" "ETF"]}
    :source "BLOOMBERG"
    :price_type "CLOSING"
  }]
  :standing_instructions {
    :CUSTODY [{
      :name "US_EQUITY_SSI"
      :market "XNYS"
      :currency "USD"
      :custody_account "US-EQUITY-001"
      :custody_bic "IRVTUS3N"
      :cash_account "US-CASH-001"
      :cash_bic "IRVTUS3N"
      :settlement_model "DVP"
    }]
  }
})
→ @profile

; --- Activate and Materialize ---
(trading-profile.activate :profile-id @profile)
(trading-profile.materialize :profile-id @profile)

; --- Verify ---
(investment-manager.list :cbu-id @fund)
(pricing-config.list :cbu-id @fund)
(cbu-custody.list-ssis :cbu-id @fund)

; --- Test Lookups ---
(investment-manager.find-for-trade :cbu-id @fund :market "XNYS" :instrument-class "EQUITY")
(pricing-config.find-for-instrument :cbu-id @fund :instrument-class "EQUITY")
```

- [ ] Create scenario file
- [ ] Execute successfully
- [ ] All lookups return expected results

### 12.2 Scenario: Multi-Manager Global Fund
**File:** `rust/examples/scenarios/02_multi_manager_global.dsl`

```clojure
; =============================================================================
; SCENARIO: Multi-Manager Global Fund
; 3 IMs with different scopes, global markets, multiple asset classes
; =============================================================================

(cbu.create 
  :name "Global Multi-Asset Fund" 
  :jurisdiction "LU" 
  :cbu-category FUND_MANDATE)
→ @fund

; --- IM 1: European Equities Specialist (CTM) ---
(investment-manager.assign 
  :cbu-id @fund
  :manager-lei "549300EUEQUITYIM001"
  :manager-name "European Equity Partners"
  :priority 10
  :role INVESTMENT_MANAGER
  :scope-markets ["XLON" "XETR" "XPAR" "XAMS" "XSWX"]
  :scope-instrument-classes ["EQUITY"]
  :instruction-method CTM
  :can-trade true
  :can-settle true)
→ @im-europe

; --- IM 2: Asia Equities Specialist (SWIFT) ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300ASIAEQUITY001"
  :manager-name "Asia Pacific Investments"
  :priority 10
  :role INVESTMENT_MANAGER
  :scope-markets ["XHKG" "XTKS" "XASX" "XSES"]
  :scope-instrument-classes ["EQUITY"]
  :instruction-method SWIFT
  :can-trade true
  :can-settle true)
→ @im-asia

; --- IM 3: Fixed Income Manager (SWIFT) ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300FIXEDINCOME01"
  :manager-name "Global Fixed Income LLC"
  :priority 10
  :role INVESTMENT_MANAGER
  :scope-instrument-classes ["GOVT_BOND" "CORP_BOND"]
  :instruction-method SWIFT
  :can-trade true
  :can-settle true)
→ @im-fi

; --- IM 4: Default/Catch-all (SWIFT) ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300DEFAULTIMGR01"
  :manager-name "Fund Manager HQ"
  :priority 100
  :role INVESTMENT_MANAGER
  :scope-all true
  :instruction-method SWIFT)
→ @im-default

; --- Pricing by Asset Class ---
(pricing-config.set :cbu-id @fund :instrument-class "EQUITY" :priority 1 :source BLOOMBERG)
(pricing-config.set :cbu-id @fund :instrument-class "GOVT_BOND" :priority 1 :source BLOOMBERG)
(pricing-config.set :cbu-id @fund :instrument-class "CORP_BOND" :priority 1 :source MARKIT :fallback-source BLOOMBERG)

; --- Cash Sweeps ---
(cash-sweep.configure :cbu-id @fund :currency "EUR" :threshold-amount 50000 
  :vehicle-type STIF :sweep-time "17:00" :sweep-timezone "Europe/Luxembourg")
(cash-sweep.configure :cbu-id @fund :currency "USD" :threshold-amount 100000
  :vehicle-type STIF :sweep-time "16:00" :sweep-timezone "America/New_York")
(cash-sweep.configure :cbu-id @fund :currency "GBP" :threshold-amount 25000
  :vehicle-type STIF :sweep-time "16:00" :sweep-timezone "Europe/London")

; --- SLA Commitments ---
(sla.commit :cbu-id @fund :template-code "CUSTODY_DVP_SETTLE_RATE")
→ @sla-settle
(sla.commit :cbu-id @fund :template-code "FA_NAV_DELIVERY_DAILY")
(sla.commit :cbu-id @fund :template-code "MO_TRADE_MATCH_RATE")

; --- Test IM Resolution ---
; European equity trade → should route to IM 1 (CTM)
(investment-manager.find-for-trade :cbu-id @fund :market "XLON" :instrument-class "EQUITY")

; Asian equity trade → should route to IM 2 (SWIFT)
(investment-manager.find-for-trade :cbu-id @fund :market "XHKG" :instrument-class "EQUITY")

; US equity trade → should route to IM 4 default (SWIFT)
(investment-manager.find-for-trade :cbu-id @fund :market "XNYS" :instrument-class "EQUITY")

; Bond trade → should route to IM 3 (SWIFT)
(investment-manager.find-for-trade :cbu-id @fund :instrument-class "GOVT_BOND")
```

- [ ] Create scenario file
- [ ] Execute successfully
- [ ] IM resolution returns correct manager for each case
- [ ] Verify priority ordering works

### 12.3 Scenario: Fund with OTC Derivatives (ISDA/CSA)
**File:** `rust/examples/scenarios/03_fund_with_otc.dsl`

```clojure
; =============================================================================
; SCENARIO: Fund with OTC Derivatives
; ISDA agreements, CSA collateral, derivatives overlay manager
; =============================================================================

(cbu.create 
  :name "Alpha Hedge Fund" 
  :jurisdiction "KY" 
  :cbu-category FUND_MANDATE)
→ @fund

; --- Core IM for listed assets ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300ALPHACORE0001"
  :priority 100
  :role INVESTMENT_MANAGER
  :scope-instrument-classes ["EQUITY" "GOVT_BOND"]
  :instruction-method SWIFT)

; --- Derivatives Overlay Manager (API) ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300DERIVOVERLAY01"
  :priority 10
  :role OVERLAY_MANAGER
  :scope-instrument-classes ["OTC_DERIVATIVE"]
  :scope-isda-asset-classes ["RATES" "FX" "CREDIT"]
  :instruction-method API)
→ @im-deriv

; --- ISDA with Goldman Sachs ---
(isda.create
  :cbu-id @fund
  :counterparty "Goldman Sachs International"  ; Needs entity lookup
  :agreement-date "2024-01-15"
  :governing-law ENGLISH
  :effective-date "2024-02-01")
→ @isda-gs

(isda.add-coverage :isda-id @isda-gs :instrument-class "IRS" :isda-taxonomy "RATES:SWAP")
(isda.add-coverage :isda-id @isda-gs :instrument-class "FX_FORWARD" :isda-taxonomy "FX:FORWARD")

(isda.add-csa
  :isda-id @isda-gs
  :csa-type VM
  :threshold 0
  :threshold-currency USD
  :mta 500000
  :effective-date "2024-02-01")
→ @csa-gs

; --- ISDA with JP Morgan (Credit) ---
(isda.create
  :cbu-id @fund
  :counterparty "JP Morgan Chase"
  :agreement-date "2024-03-01"
  :governing-law NY
  :effective-date "2024-04-01")
→ @isda-jpm

(isda.add-coverage :isda-id @isda-jpm :instrument-class "CDS" :isda-taxonomy "CREDIT:CDS")

(isda.add-csa
  :isda-id @isda-jpm
  :csa-type VM_IM
  :threshold 0
  :threshold-currency USD
  :mta 1000000
  :effective-date "2024-04-01")
→ @csa-jpm

; --- SLA for margin calls ---
(sla.commit :cbu-id @fund :template-code "COLL_MARGIN_CALL_SAME_DAY")
→ @sla-margin

(sla.bind-to-csa :commitment-id @sla-margin :csa-id @csa-gs)
(sla.bind-to-csa :commitment-id @sla-margin :csa-id @csa-jpm)

; --- Verify OTC coverage ---
(isda.list :cbu-id @fund)
(investment-manager.find-for-trade :cbu-id @fund :instrument-class "IRS" :isda-asset-class "RATES")
```

- [ ] Create scenario file
- [ ] Execute successfully
- [ ] ISDA/CSA records created
- [ ] SLA bound to CSA agreements
- [ ] OTC trades route to overlay manager

### 12.4 Scenario: Transition Manager
**File:** `rust/examples/scenarios/04_transition_manager.dsl`

```clojure
; =============================================================================
; SCENARIO: Transition Manager
; Temporary IM with date-bounded scope for portfolio transition
; =============================================================================

(cbu.create :name "Pension Fund Portfolio" :jurisdiction "US" :cbu-category INSTITUTIONAL_ACCOUNT)
→ @fund

; --- Existing IM ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300EXISTINGIM001"
  :priority 100
  :role INVESTMENT_MANAGER
  :scope-all true
  :instruction-method SWIFT)
→ @im-existing

; --- Transition Manager (temporary, higher priority) ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300TRANSITMGR001"
  :manager-name "State Street Global Advisors - Transition"
  :priority 1  ; Highest priority during transition
  :role TRANSITION_MANAGER
  :scope-all true
  :instruction-method FIX
  :effective-date "2025-02-01"
  :termination-date "2025-03-31")  ; 2-month transition window
→ @im-transition

; During transition period, all trades route to transition manager
; After termination date, reverts to existing IM
```

- [ ] Create scenario file
- [ ] Execute successfully
- [ ] Date-bounded assignment works
- [ ] Document termination handling logic

### 12.5 Scenario: Sub-Advised Fund Structure
**File:** `rust/examples/scenarios/05_sub_advised_fund.dsl`

```clojure
; =============================================================================
; SCENARIO: Sub-Advised Fund
; Investment manager delegates to sub-advisors for specific sleeves
; =============================================================================

(cbu.create :name "Multi-Strategy Fund" :jurisdiction "IE" :cbu-category FUND_MANDATE)
→ @fund

; --- Primary IM (ManCo) ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300PRIMARYMANCO01"
  :manager-name "Fund ManCo Ltd"
  :priority 100
  :role INVESTMENT_MANAGER
  :scope-all true
  :instruction-method SWIFT)
→ @im-manco

; --- Sub-Advisor 1: US Large Cap ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300SUBADV1US0001"
  :manager-name "US Large Cap Partners"
  :priority 10
  :role SUB_ADVISOR
  :scope-markets ["XNYS" "XNAS"]
  :scope-instrument-classes ["EQUITY"]
  :instruction-method CTM)
→ @sub1

; --- Sub-Advisor 2: Emerging Markets ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300SUBADV2EM0001"
  :manager-name "EM Specialists LLC"
  :priority 10
  :role SUB_ADVISOR
  :scope-markets ["XBSP" "XMEX" "XJSE" "XBOM"]  ; Brazil, Mexico, South Africa, India
  :scope-instrument-classes ["EQUITY"]
  :instruction-method SWIFT)
→ @sub2

; --- Sub-Advisor 3: Fixed Income ---
(investment-manager.assign
  :cbu-id @fund
  :manager-lei "549300SUBADV3FI0001"
  :manager-name "Global Bond Advisors"
  :priority 10
  :role SUB_ADVISOR
  :scope-instrument-classes ["GOVT_BOND" "CORP_BOND"]
  :instruction-method SWIFT)
→ @sub3
```

- [ ] Create scenario file
- [ ] Execute successfully
- [ ] Sub-advisor role distinct from IM
- [ ] Scope resolution correct

### 12.6 Scenario Runner Script
**File:** `rust/scripts/run_scenarios.sh`

```bash
#!/bin/bash
# Run all trading matrix test scenarios

SCENARIOS=(
  "01_simple_equity_fund"
  "02_multi_manager_global"
  "03_fund_with_otc"
  "04_transition_manager"
  "05_sub_advised_fund"
)

for scenario in "${SCENARIOS[@]}"; do
  echo "=== Running scenario: $scenario ==="
  cargo run --bin dsl-runner -- "examples/scenarios/${scenario}.dsl"
  if [ $? -ne 0 ]; then
    echo "FAILED: $scenario"
    exit 1
  fi
  echo "PASSED: $scenario"
  echo ""
done

echo "All scenarios passed!"
```

- [ ] Create runner script
- [ ] All scenarios pass
- [ ] Add to CI pipeline

---

## Phase 13: Trading Profile Visualization (Days 5-8)

### 13.1 Profile-to-HTML Generator
**File:** `rust/src/plugins/trading_profile_viz.rs`

Implement `trading-profile.visualize` verb:

```rust
pub async fn visualize_trading_profile(
    pool: &PgPool,
    args: &HashMap<String, Value>,
) -> Result<ExecutionResult, DslError> {
    let profile_id = get_uuid_arg(args, "profile-id")?;
    let format = get_string_arg(args, "format").unwrap_or("html".to_string());
    
    // Load profile and related data
    let profile = load_profile(pool, profile_id).await?;
    let im_assignments = load_im_assignments(pool, profile.cbu_id).await?;
    let pricing_configs = load_pricing_configs(pool, profile.cbu_id).await?;
    let sla_commitments = load_sla_commitments(pool, profile.cbu_id).await?;
    
    // Generate visualization
    let html = match format.as_str() {
        "html" => generate_html_viz(&profile, &im_assignments, &pricing_configs, &sla_commitments),
        "react" => generate_react_artifact(&profile, ...),
        "mermaid" => generate_mermaid_diagram(&profile, ...),
        _ => return Err(DslError::InvalidArgument("Unknown format".into())),
    };
    
    Ok(ExecutionResult::Record(json!({
        "content": html,
        "format": format,
        "profile_id": profile_id
    })))
}
```

- [ ] Create visualization module
- [ ] Implement HTML generator
- [ ] Include IM scope matrix (visual grid)
- [ ] Include pricing source table
- [ ] Include SLA coverage summary
- [ ] Include resource dependency graph

### 13.2 HTML Template for Trading Matrix
**File:** `rust/static/templates/trading_matrix.html`

```html
<!DOCTYPE html>
<html>
<head>
  <title>Trading Matrix: {{cbu_name}}</title>
  <style>
    /* Tailwind-like utility classes */
    .matrix-grid { display: grid; gap: 1rem; }
    .card { border: 1px solid #e2e8f0; border-radius: 0.5rem; padding: 1rem; }
    .scope-cell { padding: 0.5rem; text-align: center; }
    .scope-active { background: #c6f6d5; }
    .scope-inactive { background: #fed7d7; }
    /* ... more styles */
  </style>
</head>
<body>
  <div class="container">
    <h1>Trading Matrix: {{cbu_name}}</h1>
    <p>Profile Version: {{version}} | Status: {{status}} | As of: {{as_of}}</p>
    
    <!-- Investment Manager Scope Matrix -->
    <section class="card">
      <h2>Investment Manager Coverage</h2>
      <table class="scope-matrix">
        <thead>
          <tr>
            <th>Manager</th>
            <th>Role</th>
            <th>Instruction</th>
            {{#each markets}}<th>{{this}}</th>{{/each}}
          </tr>
        </thead>
        <tbody>
          {{#each im_assignments}}
          <tr>
            <td>{{manager_name}}</td>
            <td>{{role}}</td>
            <td>{{instruction_method}}</td>
            {{#each ../markets}}
            <td class="scope-cell {{#if (in_scope this ../scope_markets)}}scope-active{{else}}scope-inactive{{/if}}">
              {{#if (in_scope this ../scope_markets)}}✓{{else}}—{{/if}}
            </td>
            {{/each}}
          </tr>
          {{/each}}
        </tbody>
      </table>
    </section>
    
    <!-- Pricing Sources -->
    <section class="card">
      <h2>Pricing Matrix</h2>
      <table>
        <thead>
          <tr><th>Instrument Class</th><th>Primary</th><th>Fallback</th><th>Price Type</th></tr>
        </thead>
        <tbody>
          {{#each pricing_configs}}
          <tr>
            <td>{{instrument_class}}</td>
            <td>{{source}}</td>
            <td>{{fallback_source}}</td>
            <td>{{price_type}}</td>
          </tr>
          {{/each}}
        </tbody>
      </table>
    </section>
    
    <!-- SLA Summary -->
    <section class="card">
      <h2>SLA Commitments</h2>
      {{#each sla_commitments}}
      <div class="sla-item">
        <strong>{{template_name}}</strong>: Target {{target_value}}{{unit}}
        <span class="status-badge {{status}}">{{status}}</span>
      </div>
      {{/each}}
    </section>
    
    <!-- Resource Dependencies -->
    <section class="card">
      <h2>Service Resources</h2>
      <div class="resource-graph">
        {{resource_mermaid}}
      </div>
    </section>
  </div>
  
  <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
  <script>mermaid.initialize({startOnLoad:true});</script>
</body>
</html>
```

- [ ] Create HTML template
- [ ] Use Handlebars or similar for templating
- [ ] Include Mermaid for resource graph
- [ ] Test rendering

### 13.3 React Artifact Generator
**File:** `rust/src/plugins/trading_profile_react.rs`

Generate React artifact for Claude.ai rendering:

```rust
fn generate_react_artifact(profile: &TradingProfile, ...) -> String {
    format!(r#"
import React, {{ useState }} from 'react';

export default function TradingMatrixViewer() {{
  const [activeTab, setActiveTab] = useState('im');
  
  const profile = {profile_json};
  const imAssignments = {im_json};
  const pricingConfigs = {pricing_json};
  
  return (
    <div className="p-4 max-w-6xl mx-auto">
      <h1 className="text-2xl font-bold mb-4">
        Trading Matrix: {{profile.cbu_name}}
      </h1>
      
      <div className="flex gap-2 mb-4">
        <button onClick={{() => setActiveTab('im')}} 
                className={{`px-4 py-2 rounded ${{activeTab === 'im' ? 'bg-blue-500 text-white' : 'bg-gray-200'}}`}}>
          Investment Managers
        </button>
        <button onClick={{() => setActiveTab('pricing')}}
                className={{`px-4 py-2 rounded ${{activeTab === 'pricing' ? 'bg-blue-500 text-white' : 'bg-gray-200'}}`}}>
          Pricing
        </button>
        <button onClick={{() => setActiveTab('sla')}}
                className={{`px-4 py-2 rounded ${{activeTab === 'sla' ? 'bg-blue-500 text-white' : 'bg-gray-200'}}`}}>
          SLAs
        </button>
      </div>
      
      {{activeTab === 'im' && <IMScopeMatrix assignments={{imAssignments}} />}}
      {{activeTab === 'pricing' && <PricingTable configs={{pricingConfigs}} />}}
      {{activeTab === 'sla' && <SLASummary commitments={{slaCommitments}} />}}
    </div>
  );
}}

// ... component definitions
"#, 
    profile_json = serde_json::to_string(profile)?,
    im_json = serde_json::to_string(im_assignments)?,
    pricing_json = serde_json::to_string(pricing_configs)?
    )
}
```

- [ ] Create React generator
- [ ] Include tabbed interface
- [ ] IM scope as interactive matrix
- [ ] Pricing as sortable table
- [ ] SLA as status cards

### 13.4 Add Visualization Verb
**File:** `rust/config/verbs/trading-profile.yaml` (append)

```yaml
      visualize:
        description: Generate visual representation of trading profile
        behavior: plugin
        handler: visualize_trading_profile
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: format
            type: string
            required: false
            default: html
            valid_values:
              - html
              - react
              - mermaid
              - json
          - name: include-resources
            type: boolean
            required: false
            default: true
          - name: include-sla
            type: boolean
            required: false
            default: true
        returns:
          type: record
          description: Generated visualization content
```

- [ ] Add verb definition
- [ ] Register handler
- [ ] Test all format options

### 13.5 Test Visualization
```clojure
; Generate HTML visualization
(trading-profile.visualize :profile-id @my-profile :format "html")

; Generate React artifact
(trading-profile.visualize :profile-id @my-profile :format "react")
```

- [ ] HTML renders correctly in browser
- [ ] React artifact renders in Claude.ai
- [ ] All sections populated

---

## Phase 14: Agent Persona for Trading Matrix (Days 8-10)

### 14.1 Trading Matrix Agent Mode
**File:** `rust/config/agent_modes/trading_matrix.yaml`

```yaml
mode: trading_matrix
name: "Traded Instruments Day Assistant"
description: "Specialized agent for constructing and reviewing trading profiles"

system_prompt_extension: |
  You are assisting with Traded Instruments Day - the process of defining or reviewing
  a client's complete trading profile including:
  
  - Investment Manager assignments and their trading scopes
  - Markets and instrument classes they can trade
  - How trade instructions are delivered (SWIFT, CTM, FIX, API)
  - Pricing sources for each asset class
  - Cash sweep/STIF configuration
  - ISDA/CSA agreements for OTC derivatives
  - Service Level Agreements
  
  WORKFLOW:
  1. First, understand the CBU structure (fund type, jurisdiction, complexity)
  2. Ask about investment managers - how many, what do they each handle?
  3. For each IM, determine: scope (markets, instruments), instruction method
  4. Ask about pricing requirements
  5. Ask about cash management (sweeps)
  6. If OTC trading, ask about ISDA relationships
  7. Confirm SLA requirements
  8. Generate the complete profile
  9. Offer to visualize and materialize
  
  PROACTIVE QUESTIONS:
  - "How many investment managers will be trading for this fund?"
  - "What markets will [IM name] be trading in?"
  - "How will [IM name] send trade instructions - SWIFT, CTM, or another method?"
  - "Do you need different pricing sources for different asset classes?"
  - "Will there be any OTC derivatives trading requiring ISDA agreements?"
  - "Should I set up cash sweeps for idle balances?"
  
  VALIDATION PROMPTS:
  - If IM scope overlaps: "I notice both IMs cover European equities - which should take priority?"
  - If no pricing for asset class: "You've enabled bond trading but haven't specified a pricing source for bonds."
  - If OTC without ISDA: "You've assigned an IM for derivatives but I don't see any ISDA agreements."
  
  When generating DSL, use these verbs:
  - investment-manager.assign - for IM setup
  - pricing-config.set - for pricing sources
  - cash-sweep.configure - for cash management
  - isda.create / isda.add-csa - for OTC agreements
  - sla.commit - for SLA commitments
  - trading-profile.import - for bulk profile import
  - trading-profile.visualize - to show the result

conversation_starters:
  - "Let's set up the trading profile. What type of fund/account is this?"
  - "I'll help you configure the trading matrix. How many investment managers will be involved?"
  - "Ready for Traded Instruments Day! Tell me about the fund structure."

examples:
  - user: "We have a new multi-manager fund in Luxembourg"
    assistant: |
      Great! Let's set up the trading profile for your Luxembourg fund. 
      
      First, how many investment managers will be trading for this fund, 
      and what's their general focus (e.g., equities, fixed income, derivatives)?
      
  - user: "Three IMs - one for European equities, one for US equities, one for bonds"
    assistant: |
      Perfect, a clear three-manager structure. Let me set these up:
      
      1. **European Equities IM** - What markets? (e.g., London, Frankfurt, Paris, Amsterdam, Zurich)
      2. **US Equities IM** - NYSE and NASDAQ?
      3. **Fixed Income IM** - Government bonds, corporate bonds, or both?
      
      Also, how will each IM send trade instructions to us?
      - SWIFT (traditional messaging)
      - CTM (automated matching via DTCC)
      - FIX (direct electronic connection)
      - API (programmatic integration)
```

- [ ] Create agent mode configuration
- [ ] Define proactive question patterns
- [ ] Define validation prompts
- [ ] Include conversation examples

### 14.2 Agent Training Examples
**File:** `rust/config/agent_examples/trading_matrix_conversations.yaml`

```yaml
conversations:
  - id: simple_single_im
    title: "Simple single-IM equity fund"
    turns:
      - user: "Set up trading for our new US equity fund"
      - assistant: |
          I'll help you set up the trading profile. Let me ask a few questions:
          
          1. Who is the investment manager (name or LEI)?
          2. Will they trade just US markets (NYSE, NASDAQ) or others too?
          3. How will they send trade instructions - SWIFT or CTM?
      - user: "Blackrock is the IM, US markets only, they use CTM"
      - assistant: |
          Got it. Let me create the investment manager assignment:
          
          ```dsl
          (investment-manager.assign 
            :cbu-id @current-cbu
            :manager-name "BlackRock"
            :priority 100
            :scope-markets ["XNYS" "XNAS"]
            :scope-instrument-classes ["EQUITY" "ETF"]
            :instruction-method CTM)
          ```
          
          For pricing, shall I use Bloomberg for equities (standard choice)?
      - user: "Yes Bloomberg is fine"
      - assistant: |
          ```dsl
          (pricing-config.set 
            :cbu-id @current-cbu 
            :instrument-class "EQUITY" 
            :priority 1 
            :source BLOOMBERG
            :price-type CLOSING)
          ```
          
          Any cash sweep requirements? For US funds, we typically sweep USD balances 
          above $100,000 into a STIF at 4pm New York time.
      - user: "That works"
      - assistant: |
          ```dsl
          (cash-sweep.configure 
            :cbu-id @current-cbu 
            :currency "USD" 
            :threshold-amount 100000
            :vehicle-type STIF
            :sweep-time "16:00"
            :sweep-timezone "America/New_York")
          ```
          
          Your trading profile is ready. Would you like me to:
          1. Show you a visual summary of the trading matrix?
          2. Materialize this to operational tables?
          3. Both?
          
  - id: complex_multi_manager
    title: "Complex multi-manager with OTC"
    turns:
      - user: "We need to set up a hedge fund with multiple managers and derivatives"
      - assistant: |
          A hedge fund with derivatives - that'll need a more comprehensive setup. Let me understand the structure:
          
          1. How many investment managers, and what does each one handle?
          2. Will you need ISDA agreements for the OTC derivatives?
          3. What's the fund domicile (affects regulatory framework)?
      - user: "Cayman fund. Main PM handles equities globally, we have a derivatives overlay manager for rates and FX hedging. Yes we need ISDAs with Goldman and JPM."
      - assistant: |
          Clear structure. Let me build this out:
          
          **Investment Managers:**
          ```dsl
          ; Main PM - Global Equities
          (investment-manager.assign 
            :cbu-id @hedge-fund
            :manager-lei "..." ; I'll need this
            :priority 100
            :role INVESTMENT_MANAGER
            :scope-instrument-classes ["EQUITY"]
            :instruction-method SWIFT)
          
          ; Derivatives Overlay
          (investment-manager.assign 
            :cbu-id @hedge-fund
            :manager-lei "..."
            :priority 10
            :role OVERLAY_MANAGER
            :scope-instrument-classes ["OTC_DERIVATIVE"]
            :scope-isda-asset-classes ["RATES" "FX"]
            :instruction-method API)  ; Derivatives often use API
          ```
          
          For the ISDAs, I'll need:
          - Agreement dates with Goldman and JPM
          - Which products under each (IRS, FX forwards, etc.)?
          - CSA details (threshold, minimum transfer amount)
          
          Do you have these details, or shall I use standard terms?
      # ... continues
```

- [ ] Create 5+ complete conversation examples
- [ ] Cover simple to complex scenarios
- [ ] Include error handling examples
- [ ] Include validation/correction flows

### 14.3 Update Main Agent System Prompt
**File:** `rust/src/api/agent_service.rs`

Add trading matrix capabilities to main prompt:

```rust
const TRADING_MATRIX_PROMPT_SECTION: &str = r#"
## Trading Profile / Traded Instruments Matrix

You can help construct complete trading profiles using these verb domains:

**investment-manager** - Assign IMs with trading scope
- `investment-manager.assign` - Create IM assignment with markets/instruments scope
- `investment-manager.find-for-trade` - Find which IM handles a specific trade

**pricing-config** - Configure pricing sources
- `pricing-config.set` - Set pricing source for instrument class
- `pricing-config.find-for-instrument` - Find pricing source for instrument

**cash-sweep** - Configure cash management
- `cash-sweep.configure` - Set up STIF/MMF sweeps by currency

**sla** - Service Level Agreements
- `sla.commit` - Create SLA commitment from template
- `sla.bind-to-resource` - Bind SLA to service resource

**trading-profile** - Profile lifecycle
- `trading-profile.import` - Import complete profile from YAML/JSON
- `trading-profile.materialize` - Push profile to operational tables
- `trading-profile.visualize` - Generate visual representation

For "Traded Instruments Day" workflows, guide users through:
1. IM assignments and scopes
2. Instruction delivery methods
3. Pricing sources
4. Cash sweep configuration
5. ISDA/CSA if OTC trading
6. SLA commitments
"#;
```

- [ ] Add trading matrix section to system prompt
- [ ] Include verb summaries
- [ ] Include workflow guidance

### 14.4 Test Agent Conversations
Create test harness for agent conversations:

**File:** `rust/tests/agent_trading_matrix_tests.rs`

```rust
#[tokio::test]
async fn test_agent_simple_fund_setup() {
    let agent = create_test_agent().await;
    
    // User initiates
    let response = agent.chat("Set up trading for a US equity fund").await;
    assert!(response.contains("investment manager"));
    
    // User provides IM details
    let response = agent.chat("Single IM, Vanguard, uses SWIFT").await;
    assert!(response.contains("investment-manager.assign"));
    
    // Verify DSL generated correctly
    let dsl = extract_dsl_from_response(&response);
    assert!(dsl.contains(":instruction-method SWIFT"));
}

#[tokio::test]
async fn test_agent_validates_gaps() {
    let agent = create_test_agent().await;
    
    // Set up IM for derivatives without ISDA
    agent.chat("Create fund with derivatives trading").await;
    agent.chat("IM handles IRS and CDS via API").await;
    
    let response = agent.chat("That's all").await;
    
    // Agent should flag missing ISDA
    assert!(response.contains("ISDA") || response.contains("derivatives"));
}
```

- [ ] Create agent test harness
- [ ] Test simple fund flow
- [ ] Test multi-manager flow
- [ ] Test validation prompts trigger correctly
- [ ] Test DSL generation accuracy

---

## Phase 15: Document Extraction Pipeline (Days 10-14)

### 15.1 Document Extraction Prompts
**File:** `rust/config/extraction_prompts/investment_mandate.yaml`

```yaml
document_type: INVESTMENT_MANDATE
extraction_prompt: |
  Extract the following information from this Investment Management Agreement:
  
  1. **Investment Manager Details**
     - Manager name
     - LEI (if present)
     - Role (Investment Manager, Sub-Advisor, etc.)
  
  2. **Investment Universe**
     - Allowed asset classes (equities, bonds, derivatives, etc.)
     - Allowed markets/exchanges
     - Allowed currencies
     - Any restrictions or exclusions
  
  3. **Trading Authority**
     - Can the manager execute trades directly?
     - Settlement authority?
     - Affirmation authority?
  
  4. **Instruction Method**
     - How will instructions be delivered? (SWIFT, FIX, API, etc.)
  
  Return as structured JSON:
  ```json
  {
    "manager": {
      "name": "...",
      "lei": "...",
      "role": "INVESTMENT_MANAGER|SUB_ADVISOR|OVERLAY_MANAGER"
    },
    "universe": {
      "asset_classes": ["EQUITY", "GOVT_BOND", ...],
      "markets": ["XNYS", "XLON", ...],
      "currencies": ["USD", "EUR", ...],
      "restrictions": ["No short selling", ...]
    },
    "authority": {
      "can_trade": true,
      "can_settle": true,
      "can_affirm": false
    },
    "instruction_method": "SWIFT|CTM|FIX|API"
  }
  ```

output_schema:
  type: object
  properties:
    manager:
      type: object
      properties:
        name: { type: string }
        lei: { type: string }
        role: { type: string, enum: [INVESTMENT_MANAGER, SUB_ADVISOR, OVERLAY_MANAGER] }
    universe:
      type: object
      properties:
        asset_classes: { type: array, items: { type: string } }
        markets: { type: array, items: { type: string } }
        currencies: { type: array, items: { type: string } }
    # ... etc
```

- [ ] Create extraction prompt for INVESTMENT_MANDATE
- [ ] Create extraction prompt for SSI_TEMPLATE
- [ ] Create extraction prompt for ISDA_MASTER
- [ ] Create extraction prompt for CSA_ANNEX
- [ ] Define output schemas for each

### 15.2 Document Extraction Handler
**File:** `rust/src/plugins/document_extraction.rs`

```rust
pub async fn extract_to_profile_section(
    pool: &PgPool,
    llm_client: &LlmClient,
    args: &HashMap<String, Value>,
) -> Result<ExecutionResult, DslError> {
    let doc_id = get_uuid_arg(args, "document-id")?;
    let target_section = get_string_arg(args, "section")?;
    
    // Load document content
    let doc = load_document(pool, doc_id).await?;
    let content = extract_text_content(&doc)?;  // OCR if needed
    
    // Load extraction prompt for document type
    let prompt = load_extraction_prompt(&doc.document_type)?;
    
    // Call LLM for extraction
    let extracted = llm_client.extract_structured(
        &prompt,
        &content,
        &prompt.output_schema
    ).await?;
    
    // Validate extracted data
    let validated = validate_extraction(&extracted, &target_section)?;
    
    // Return for human review (don't auto-merge)
    Ok(ExecutionResult::Record(json!({
        "document_id": doc_id,
        "document_type": doc.document_type,
        "target_section": target_section,
        "extracted_data": validated,
        "confidence": extracted.confidence,
        "requires_review": true,
        "suggested_dsl": generate_dsl_from_extraction(&validated, &target_section)?
    })))
}
```

- [ ] Create extraction handler
- [ ] Integrate with LLM client (Claude API)
- [ ] Implement text extraction (PDF parsing)
- [ ] Implement OCR integration (for scanned docs)
- [ ] Generate suggested DSL from extraction
- [ ] Return for human review

### 15.3 Extraction Verb
**File:** `rust/config/verbs/document.yaml` (append)

```yaml
      extract-to-profile:
        description: Extract document content to trading profile section
        behavior: plugin
        handler: extract_to_profile_section
        args:
          - name: document-id
            type: uuid
            required: true
            lookup:
              table: document_catalog
              schema: ob-poc
              search_key: document_name
              primary_key: doc_id
          - name: section
            type: string
            required: true
            valid_values:
              - investment_managers
              - universe
              - standing_instructions
              - isda_agreements
              - pricing_matrix
          - name: auto-merge
            type: boolean
            required: false
            default: false
            description: "If true, automatically merge into profile (requires review=false)"
        returns:
          type: record
          description: Extracted data with suggested DSL and review status
```

- [ ] Add verb definition
- [ ] Register handler
- [ ] Test extraction flow

### 15.4 Human-in-the-Loop Review UI
**File:** `rust/src/plugins/extraction_review.rs`

Generate React artifact for extraction review:

```rust
fn generate_review_artifact(extraction: &ExtractionResult) -> String {
    format!(r#"
import React, {{ useState }} from 'react';

export default function ExtractionReview() {{
  const [approved, setApproved] = useState(false);
  const extraction = {extraction_json};
  
  return (
    <div className="p-4">
      <h2 className="text-xl font-bold">Review Extracted Data</h2>
      <p className="text-gray-600">Document: {{extraction.document_name}}</p>
      <p className="text-gray-600">Confidence: {{extraction.confidence}}%</p>
      
      <div className="mt-4 p-4 bg-gray-50 rounded">
        <h3 className="font-semibold">Extracted Investment Manager</h3>
        <pre>{{JSON.stringify(extraction.extracted_data.manager, null, 2)}}</pre>
      </div>
      
      <div className="mt-4 p-4 bg-gray-50 rounded">
        <h3 className="font-semibold">Suggested DSL</h3>
        <pre className="bg-black text-green-400 p-2 rounded">
          {{extraction.suggested_dsl}}
        </pre>
      </div>
      
      <div className="mt-4 flex gap-4">
        <button 
          onClick={{() => setApproved(true)}}
          className="px-4 py-2 bg-green-500 text-white rounded">
          Approve & Execute
        </button>
        <button className="px-4 py-2 bg-yellow-500 text-white rounded">
          Edit Before Executing
        </button>
        <button className="px-4 py-2 bg-red-500 text-white rounded">
          Reject
        </button>
      </div>
    </div>
  );
}}
"#, extraction_json = serde_json::to_string(extraction)?)
}
```

- [ ] Create review artifact generator
- [ ] Show extracted data vs suggested DSL
- [ ] Allow approve/edit/reject actions
- [ ] Track review decisions for audit

### 15.5 End-to-End Extraction Flow
**File:** `rust/examples/document_extraction_flow.dsl`

```clojure
; === Document Extraction Flow ===

; 1. Catalog the uploaded IMA document
(document.catalog 
  :cbu-id @my-fund 
  :entity-id @management-company
  :document-type INVESTMENT_MANDATE
  :file-path "/uploads/ima_blackrock_2025.pdf")
→ @ima-doc

; 2. Extract to profile section (returns for review)
(document.extract-to-profile 
  :document-id @ima-doc 
  :section "investment_managers")
→ @extraction

; 3. After human review approves, execute suggested DSL
; (This would be done via UI approval, triggering execution)

; 4. Link document to profile for audit trail
(trading-profile.link-document 
  :profile-id @current-profile 
  :document-id @ima-doc 
  :section "investment_managers")
```

- [ ] Create example flow
- [ ] Test with sample IMA PDF
- [ ] Verify extraction accuracy
- [ ] Verify audit trail created

---

## Phase 16: Workflow Integration (Days 14-17)

### 16.1 Update Onboarding Workflow
**File:** `rust/config/workflows/onboarding_with_trading.yaml`

```yaml
workflow: cbu_onboarding_complete
name: "Complete CBU Onboarding with Trading Matrix"
version: "2.0"

stages:
  - stage: 1
    name: "Client Intake"
    tasks:
      - create_cbu
      - select_products
      - identify_commercial_entity
    gate: products_selected
    
  - stage: 2
    name: "Entity & KYC"
    tasks:
      - create_entity_structure
      - kyc_case_creation
      - document_collection
      - ubo_determination
    gate: kyc_approved
    
  - stage: 3
    name: "Trading Matrix"  # NEW STAGE
    tasks:
      - collect_trading_documents:
          document_types: [INVESTMENT_MANDATE, SSI_TEMPLATE, ISDA_MASTER]
      - extract_trading_profile:
          from_documents: true
          human_review: true
      - configure_investment_managers
      - configure_pricing_sources
      - configure_cash_sweeps
      - configure_isda_csa:
          condition: "has_otc_trading"
      - validate_trading_matrix:
          checks:
            - all_instruments_have_pricing
            - all_otc_has_isda
            - all_ims_have_instruction_method
      - approve_trading_profile
    gate: trading_profile_approved
    
  - stage: 4
    name: "Resource Provisioning"
    tasks:
      - provision_accounts:
          depends_on: trading_profile
      - provision_connectivity:
          from_trading_profile: true
          resources: [SWIFT_GATEWAY, CTM_CONNECTION, BLOOMBERG_TERMINAL]
      - provision_cash_management:
          from_trading_profile: true
    gate: resources_provisioned
    
  - stage: 5
    name: "SLA & Go-Live"
    tasks:
      - create_sla_commitments:
          from_trading_profile: true
      - bind_slas_to_resources
      - final_validation
      - activate_services
    gate: services_active

gates:
  products_selected:
    condition: "cbu.product_id IS NOT NULL"
  kyc_approved:
    condition: "kyc_case.status = 'APPROVED'"
  trading_profile_approved:
    condition: "trading_profile.status = 'ACTIVE'"
  resources_provisioned:
    condition: "ALL(resources.status = 'PROVISIONED')"
  services_active:
    condition: "ALL(services.delivery_status = 'ACTIVE')"
```

- [ ] Update workflow definition
- [ ] Add trading matrix stage
- [ ] Define validation checks
- [ ] Define stage gates

### 16.2 Trading Profile Validation
**File:** `rust/src/plugins/trading_profile_validation.rs`

```rust
pub async fn validate_trading_matrix(
    pool: &PgPool,
    args: &HashMap<String, Value>,
) -> Result<ExecutionResult, DslError> {
    let cbu_id = get_uuid_arg(args, "cbu-id")?;
    
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    
    // Check 1: All instrument classes have pricing
    let universe = load_universe(pool, cbu_id).await?;
    let pricing = load_pricing_configs(pool, cbu_id).await?;
    
    for entry in &universe {
        if !pricing.iter().any(|p| p.instrument_class_id == entry.instrument_class_id) {
            issues.push(ValidationIssue {
                severity: "ERROR",
                check: "pricing_coverage",
                message: format!("No pricing source configured for {}", entry.instrument_class_code),
            });
        }
    }
    
    // Check 2: OTC instruments have ISDA
    let otc_classes = universe.iter()
        .filter(|u| u.requires_isda)
        .collect::<Vec<_>>();
    
    if !otc_classes.is_empty() {
        let isda_count = count_isda_agreements(pool, cbu_id).await?;
        if isda_count == 0 {
            issues.push(ValidationIssue {
                severity: "ERROR",
                check: "isda_coverage",
                message: "OTC instruments enabled but no ISDA agreements configured",
            });
        }
    }
    
    // Check 3: All IMs have instruction method
    let ims = load_im_assignments(pool, cbu_id).await?;
    for im in &ims {
        if im.instruction_resource_id.is_none() {
            warnings.push(ValidationIssue {
                severity: "WARNING",
                check: "connectivity",
                message: format!("IM {} has no linked connectivity resource", im.manager_name),
            });
        }
    }
    
    // Check 4: No overlapping IM scopes without priority differentiation
    // ... etc
    
    let is_valid = issues.is_empty();
    
    Ok(ExecutionResult::Record(json!({
        "valid": is_valid,
        "issues": issues,
        "warnings": warnings,
        "checked_at": Utc::now()
    })))
}
```

- [ ] Create validation handler
- [ ] Implement all validation checks
- [ ] Return structured issues/warnings
- [ ] Add verb: `trading-profile.validate-matrix`

### 16.3 Workflow Task Handlers
**File:** `rust/src/workflow/trading_tasks.rs`

```rust
// Task: extract_trading_profile
pub async fn task_extract_trading_profile(
    ctx: &WorkflowContext,
    task: &WorkflowTask,
) -> Result<TaskResult, WorkflowError> {
    // Find trading documents
    let docs = find_documents_by_type(
        ctx.pool,
        ctx.cbu_id,
        &["INVESTMENT_MANDATE", "SSI_TEMPLATE"]
    ).await?;
    
    let mut extractions = Vec::new();
    
    for doc in docs {
        let extraction = document_extract_to_profile(
            ctx.pool,
            ctx.llm_client,
            doc.doc_id,
            infer_section(&doc.document_type)
        ).await?;
        
        extractions.push(extraction);
    }
    
    // If human_review required, pause workflow
    if task.config.human_review {
        return Ok(TaskResult::PendingReview {
            review_items: extractions,
            resume_on: "extraction_approved"
        });
    }
    
    // Otherwise, auto-merge
    for extraction in extractions {
        merge_extraction_to_profile(ctx.pool, ctx.profile_id, extraction).await?;
    }
    
    Ok(TaskResult::Completed)
}

// Task: provision_connectivity
pub async fn task_provision_connectivity(
    ctx: &WorkflowContext,
    task: &WorkflowTask,
) -> Result<TaskResult, WorkflowError> {
    let ims = load_im_assignments(ctx.pool, ctx.cbu_id).await?;
    
    for im in ims {
        let resource_type = match im.instruction_method.as_str() {
            "SWIFT" => "SWIFT_GATEWAY",
            "CTM" => "CTM_CONNECTION",
            "FIX" => "FIX_SESSION",
            "API" => "API_ENDPOINT",
            _ => continue,
        };
        
        // Provision resource
        let instance = provision_resource(
            ctx.pool,
            ctx.cbu_id,
            resource_type,
            &im.manager_name
        ).await?;
        
        // Link to IM assignment
        link_im_to_resource(ctx.pool, im.assignment_id, instance.instance_id).await?;
    }
    
    Ok(TaskResult::Completed)
}
```

- [ ] Create task handlers for trading workflow stage
- [ ] Implement document extraction task
- [ ] Implement resource provisioning task
- [ ] Implement SLA creation task
- [ ] Register tasks in workflow engine

### 16.4 Test Workflow Integration
**File:** `rust/examples/workflows/complete_onboarding.dsl`

```clojure
; === Complete Onboarding with Trading Matrix ===

; Stage 1: Client Intake
(cbu.create :name "New Global Fund" :jurisdiction "LU" :cbu-category FUND_MANDATE)
→ @fund
(cbu.select-product :cbu-id @fund :product-code "CUSTODY_PLUS_FA")

; Stage 2: KYC (abbreviated)
(kyc.create-case :cbu-id @fund :case-type ONBOARDING)
→ @kyc
(kyc.approve :case-id @kyc)

; Stage 3: Trading Matrix
(document.catalog :cbu-id @fund :document-type INVESTMENT_MANDATE :file-path "ima.pdf")
→ @ima
(document.extract-to-profile :document-id @ima :section "investment_managers")
; [Human reviews and approves]

(trading-profile.validate-matrix :cbu-id @fund)
(trading-profile.activate :profile-id @profile)

; Stage 4: Resource Provisioning
(trading-profile.provision-resources :profile-id @profile)

; Stage 5: SLA & Go-Live
(sla.commit :cbu-id @fund :template-code "CUSTODY_DVP_SETTLE_RATE")
(delivery.activate :cbu-id @fund)
```

- [ ] Create complete onboarding test
- [ ] Run through all stages
- [ ] Verify gates work
- [ ] Verify resources provisioned correctly

---

## Phase 17: SLA Monitoring Foundation (Days 17-19)

### 17.1 SLA Metrics Event Sources
**File:** `rust/config/sla_event_sources.yaml`

```yaml
# Defines where SLA metrics come from
sla_event_sources:
  SETTLEMENT_RATE:
    source_system: "CUSTODY_CORE"
    event_type: "SETTLEMENT_STATUS"
    calculation: |
      COUNT(status = 'SETTLED' AND settle_date <= intended_date) / 
      COUNT(*) * 100
    aggregation_period: MONTHLY
    
  INSTRUCTION_LATENCY:
    source_system: "SWIFT_GATEWAY"
    event_type: "INSTRUCTION_PROCESSED"
    calculation: |
      AVG(processed_at - received_at) IN MINUTES
    aggregation_period: DAILY
    
  NAV_DELIVERY_TIME:
    source_system: "FUND_ACCOUNTING"
    event_type: "NAV_PUBLISHED"
    calculation: |
      EXTRACT(HOUR FROM nav_published_at) + 
      EXTRACT(MINUTE FROM nav_published_at) / 60
    aggregation_period: DAILY
    
  MATCH_RATE:
    source_system: "CTM_CONNECTION"
    event_type: "TRADE_MATCH_STATUS"
    calculation: |
      COUNT(match_status = 'MATCHED') / COUNT(*) * 100
    aggregation_period: MONTHLY
    
  MARGIN_CALL_TIMELINESS:
    source_system: "COLLATERAL_MANAGER"
    event_type: "MARGIN_CALL_SETTLED"
    calculation: |
      AVG(settled_at - received_at) IN HOURS
    aggregation_period: DAILY
```

- [ ] Define event sources for each metric
- [ ] Document source systems
- [ ] Define calculation formulas
- [ ] Note: Actual integration is future phase

### 17.2 Manual Measurement Recording UI
**File:** Generate React artifact for manual SLA recording

```rust
fn generate_sla_recording_artifact(commitments: &[SlaCommitment]) -> String {
    // Generate a form for ops team to record monthly SLA measurements
    format!(r#"
import React, {{ useState }} from 'react';

export default function SLARecorder() {{
  const commitments = {commitments_json};
  const [measurements, setMeasurements] = useState({{}});
  
  const handleSubmit = async () => {{
    // Generate DSL for each measurement
    const dsl = Object.entries(measurements).map(([id, value]) => 
      `(sla.record-measurement :commitment-id "${{id}}" :measured-value ${{value}} ...)`
    ).join('\n');
    
    // Execute DSL
    await executeDsl(dsl);
  }};
  
  return (
    <div className="p-4">
      <h2>Monthly SLA Recording</h2>
      <p>Period: {{currentMonth}}</p>
      
      {{commitments.map(c => (
        <div key={{c.commitment_id}} className="p-4 border rounded mb-4">
          <h3>{{c.template_name}}</h3>
          <p>Target: {{c.target_value}}{{c.unit}}</p>
          <input 
            type="number"
            value={{measurements[c.commitment_id] || ''}}
            onChange={{e => setMeasurements({{...measurements, [c.commitment_id]: e.target.value}})}}
            className="border p-2 rounded"
          />
        </div>
      ))}}
      
      <button onClick={{handleSubmit}} className="bg-blue-500 text-white px-4 py-2 rounded">
        Submit Measurements
      </button>
    </div>
  );
}}
"#, commitments_json = serde_json::to_string(commitments)?)
}
```

- [ ] Create recording artifact generator
- [ ] Add verb: `sla.generate-recording-form`
- [ ] Test manual measurement flow

### 17.3 SLA Dashboard Artifact
**File:** `rust/src/plugins/sla_dashboard.rs`

```rust
pub async fn generate_sla_dashboard(
    pool: &PgPool,
    args: &HashMap<String, Value>,
) -> Result<ExecutionResult, DslError> {
    let cbu_id = get_uuid_arg(args, "cbu-id")?;
    
    // Load commitments with latest measurements
    let commitments = load_commitments_with_measurements(pool, cbu_id).await?;
    let open_breaches = load_open_breaches(pool, cbu_id).await?;
    let historical = load_historical_performance(pool, cbu_id, 12).await?; // Last 12 months
    
    // Generate React dashboard
    let artifact = generate_dashboard_react(&commitments, &open_breaches, &historical);
    
    Ok(ExecutionResult::Record(json!({
        "artifact": artifact,
        "format": "react",
        "summary": {
            "total_commitments": commitments.len(),
            "met": commitments.iter().filter(|c| c.latest_status == "MET").count(),
            "warning": commitments.iter().filter(|c| c.latest_status == "WARNING").count(),
            "breach": commitments.iter().filter(|c| c.latest_status == "BREACH").count(),
            "open_breaches": open_breaches.len()
        }
    })))
}
```

- [ ] Create dashboard generator
- [ ] Include current status cards
- [ ] Include historical trend chart (recharts)
- [ ] Include open breach list
- [ ] Add verb: `sla.dashboard`

---

## Phase 18: Documentation & Polish (Days 19-21)

### 18.1 Update CLAUDE.md
**File:** `CLAUDE.md`

Add comprehensive trading matrix section:

```markdown
## Trading Matrix Domain

### Overview
The Trading Matrix defines what a CBU can trade, who trades it, how instructions 
are delivered, and what service levels apply.

### Key Concepts
- **Investment Manager (IM)** - Entity authorized to trade on behalf of CBU
- **Scope** - What an IM can trade (markets, instruments, currencies)
- **Instruction Method** - How trades are communicated (SWIFT, CTM, FIX, API)
- **Trading Profile** - Master document containing complete configuration
- **SLA Commitment** - Service level agreement bound to profile/resources

### Verb Domains
- `investment-manager.*` - IM assignment and scope management
- `pricing-config.*` - Pricing source configuration
- `cash-sweep.*` - Cash sweep/STIF configuration
- `sla.*` - Service level agreements
- `trading-profile.*` - Profile lifecycle

### Common Patterns
[Include DSL examples for common tasks]
```

- [ ] Add trading matrix section to CLAUDE.md
- [ ] Document all new verb domains
- [ ] Include common DSL patterns
- [ ] Include troubleshooting guide

### 18.2 API Documentation
**File:** `docs/api/TRADING_MATRIX_API.md`

- [ ] Document all new endpoints (if REST exposed)
- [ ] Include request/response examples
- [ ] Document error codes

### 18.3 User Guide
**File:** `docs/guides/TRADED_INSTRUMENTS_DAY_GUIDE.md`

```markdown
# Traded Instruments Day Guide

## What is Traded Instruments Day?
Annual or periodic review of client trading configuration...

## The Process
1. **Gather Documents** - IMA, SSI forms, ISDA schedules
2. **Review Current Matrix** - What's changed?
3. **Update Configuration** - Via agent chat or direct DSL
4. **Validate** - System checks for gaps
5. **Approve** - Sign-off from client/compliance
6. **Materialize** - Push to operational systems
7. **Provision Resources** - Update connectivity as needed
8. **Document** - Complete audit trail

## Using the Agent
[Conversation examples]

## Manual DSL Approach
[DSL script examples]

## Troubleshooting
[Common issues and resolutions]
```

- [ ] Create user guide
- [ ] Include step-by-step process
- [ ] Include screenshots/examples
- [ ] Include FAQ

### 18.4 Demo Script
**File:** `docs/demos/TRADING_MATRIX_DEMO.md`

```markdown
# Trading Matrix Demo Script

## Setup (before demo)
1. Load reference data: `./scripts/load_all_refdata.sh`
2. Create demo CBU: [script]
3. Prepare sample IMA PDF

## Demo Flow (15 minutes)

### Part 1: Agent-Driven Setup (5 min)
"Let me show you how we set up a complete trading profile through conversation..."

[Script the conversation]

### Part 2: Visualization (3 min)
"Here's the visual trading matrix showing all IMs, their scopes, and coverage..."

[Show React artifact]

### Part 3: Validation & Materialization (3 min)
"The system validates there are no gaps, then materializes to operational tables..."

[Execute commands]

### Part 4: SLA Coverage (2 min)
"Every element of the trading profile has SLA coverage that traces back..."

[Show SLA dashboard]

### Part 5: Traceability (2 min)
"And we can trace any configuration back to the source document..."

[Run traceability query]

## Key Messages
- Complete trading configuration in one conversation
- Visual matrix for client review
- Automatic validation catches gaps
- Full traceability for audit
```

- [ ] Create demo script
- [ ] Test full demo flow
- [ ] Prepare backup plans for demo failures

---

## Final Verification Checklist

### Functional Verification
- [ ] All 5 test scenarios pass
- [ ] Agent can construct complete trading profile via conversation
- [ ] Visualization renders correctly
- [ ] Document extraction produces accurate results
- [ ] Workflow integration works end-to-end
- [ ] SLA recording and dashboard functional

### Data Verification
- [ ] 50+ markets in reference data
- [ ] 40+ instrument classes with taxonomy
- [ ] 50+ subcustodians in network
- [ ] 25+ SLA templates
- [ ] All FKs valid

### Documentation Verification
- [ ] CLAUDE.md updated
- [ ] API docs complete
- [ ] User guide reviewed
- [ ] Demo script tested

### Performance Verification
- [ ] Profile materialization < 5 seconds
- [ ] IM lookup < 100ms
- [ ] Visualization generation < 2 seconds
- [ ] Agent response time acceptable

---

## Success Criteria

Phase 2 is complete when:

1. ✅ Complete reference data loaded and validated
2. ✅ All 5 test scenarios execute successfully
3. ✅ Trading profile visualization works (HTML + React)
4. ✅ Agent can construct full trading matrix via guided conversation
5. ✅ Document extraction pipeline functional with human review
6. ✅ Workflow integration complete with trading matrix stage
7. ✅ SLA recording and basic dashboard operational
8. ✅ Demo script runs cleanly end-to-end
9. ✅ Documentation complete and reviewed

---

## Notes for Claude Code

- **Phase 11 (reference data) is prerequisite for everything else** - do this first
- **Test scenarios (Phase 12) should be created early** - they validate each subsequent phase
- **Visualization (Phase 13) is high-impact** - users need to see the matrix
- **Agent persona (Phase 14) is the "magic moment"** - invest time here
- **Document extraction (Phase 15) can be simplified** - start with manual JSON, add LLM later
- **Workflow integration (Phase 16) can be deferred** - not needed for demo
- **SLA monitoring (Phase 17) is foundation only** - full automation is future work
- **Keep demo-ability in mind throughout** - every feature should be demonstrable
