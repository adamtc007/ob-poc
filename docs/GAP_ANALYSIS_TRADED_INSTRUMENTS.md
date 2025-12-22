# Traded Instruments Day - Gap Analysis
## Investment Mandate / Trading Profile Matrix

**Date:** December 22, 2025  
**Author:** Claude + Adam  
**Status:** Gap Analysis

---

## Executive Summary

The ob-poc platform has **substantial existing infrastructure** for traded instruments configuration via the Trading Profile system. However, several gaps exist for full "Traded Instruments Day" implementation, particularly around:

1. **STIFs/Cash Sweeps** - Not modeled
2. **Service Level Agreements (SLAs)** - Not formalized as domain
3. **Settlement Instruction Capture** - Methods exist but not as service resources
4. **IM-Level Matrix Construction** - Partial DSL support
5. **Document Type Registration** - INVESTMENT_MANDATE not formalized

---

## Current State Assessment

### ✅ What EXISTS (Strong Foundation)

#### 1. Trading Profile Document Structure
**Location:** `rust/config/seed/trading_profiles/allianzgi_complete.yaml`

```yaml
# Currently supports:
universe:
  base_currency: EUR
  allowed_currencies: [EUR, USD, GBP, CHF, JPY, HKD, SGD, AUD]
  allowed_markets:
    - mic: XETR
      currencies: [EUR]
      settlement_types: [DVP, FOP]
  instrument_classes:
    - class_code: EQUITY
      cfi_prefixes: ["ES", "EP", "EC"]
      is_held: true
      is_traded: true
    - class_code: OTC_DERIVATIVE
      isda_asset_classes: [RATES, FX, CREDIT]

investment_managers:
  - priority: 10
    manager:
      type: LEI
      value: "549300EUEQUITYSPEC01"
    scope:
      all: false
      markets: [XETR, XLON, XSWX]
      instrument_classes: [EQUITY]
    instruction_method: CTM  # ← This exists but not as service resource
```

#### 2. Trading Profile DSL Verbs
**Location:** `rust/config/verbs/trading-profile.yaml`

| Verb | Status | Notes |
|------|--------|-------|
| `trading-profile.import` | ✅ | Imports from YAML/JSON |
| `trading-profile.validate` | ✅ | Validates structure |
| `trading-profile.activate` | ✅ | Sets profile as active |
| `trading-profile.materialize` | ✅ | Converts to operational tables |
| `trading-profile.diff` | ✅ | Compares versions |
| `trading-profile.export` | ✅ | Exports to YAML |

#### 3. Custody Operations
**Location:** `rust/config/verbs/custody/cbu-custody.yaml`

| Verb | Status | Notes |
|------|--------|-------|
| `cbu-custody.add-universe` | ✅ | Declares tradeable universe |
| `cbu-custody.create-ssi` | ✅ | Creates standing instructions |
| `cbu-custody.add-booking-rule` | ✅ | ALERT-style SSI routing |
| `cbu-custody.lookup-ssi` | ✅ | Trade → SSI resolution |

#### 4. ISDA/CSA Domain
**Location:** `rust/config/verbs/custody/isda.yaml`

- ISDA agreement creation
- Product coverage (asset class → base products)
- CSA with collateral eligibility
- Collateral SSI references

#### 5. Reference Data
**Location:** `rust/config/verbs/reference/`

- `instrument-class.yaml` - CFI/SMPG/ISDA taxonomy mappings
- `market.yaml` - MIC codes with CSD/timezone
- `subcustodian.yaml` - Subcustodian network

---

## ❌ Gap Analysis - What's MISSING

### Gap 1: STIFs / Cash Sweeps
**Priority:** HIGH  
**Impact:** Cannot model cash management / money market fund sweeps

**Current State:**
- No `STIF` or `MMF` instrument class defined
- No sweep rule configuration
- No cash sweep frequency/threshold modeling

**Required:**
```yaml
# New in instrument_classes:
- code: STIF
  name: Short-Term Investment Fund
  settlement_cycle: T+0
  swift_family: FUND
  requires_isda: false
  sweep_eligible: true

# New section in trading profile:
cash_sweep_config:
  enabled: true
  default_vehicle: 
    type: STIF
    fund_id: "BNYINSTCASH001"
  rules:
    - currency: USD
      threshold_amount: 100000
      sweep_time: "16:00"
      timezone: America/New_York
    - currency: EUR
      threshold_amount: 50000
      sweep_time: "17:00"
      timezone: Europe/Luxembourg
```

**DSL Verbs Needed:**
```
(cbu-custody.configure-sweep 
  :cbu-id @my-cbu 
  :currency "USD" 
  :vehicle-type STIF 
  :threshold 100000)
```

---

### Gap 2: Service Level Agreements (SLAs)
**Priority:** HIGH  
**Impact:** Cannot track/enforce service commitments

**Current State:**
- Services table has no SLA columns
- ISDA/CSA not linked to SLA framework
- No SLA monitoring or breach tracking

**Required Schema Extension:**
```sql
-- New table: service_level_agreements
CREATE TABLE "ob-poc".service_level_agreements (
    sla_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agreement_type VARCHAR(50) NOT NULL, -- 'STANDARD', 'ISDA', 'CSA', 'OLA'
    reference_entity_type VARCHAR(50), -- 'service', 'isda_agreement', 'csa_agreement'
    reference_entity_id UUID,
    
    -- Service parameters
    target_metric VARCHAR(100),  -- 'settlement_rate', 'nav_delivery_time'
    target_value NUMERIC(10,4),
    target_unit VARCHAR(20),     -- 'percent', 'hours', 'minutes'
    measurement_period VARCHAR(20), -- 'DAILY', 'MONTHLY', 'QUARTERLY'
    
    -- ISDA/CSA specific
    threshold_amount NUMERIC(18,2),
    threshold_currency VARCHAR(3),
    valuation_time TIME,
    settlement_days INTEGER,
    
    effective_date DATE NOT NULL,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Link table: cbu_service_slas
CREATE TABLE "ob-poc".cbu_service_slas (
    cbu_sla_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    sla_id UUID NOT NULL REFERENCES "ob-poc".service_level_agreements(sla_id),
    service_id UUID REFERENCES "ob-poc".services(service_id),
    product_id UUID REFERENCES "ob-poc".products(product_id),
    
    override_target_value NUMERIC(10,4), -- CBU-specific override
    negotiated_date DATE,
    negotiated_by VARCHAR(255),
    
    UNIQUE(cbu_id, sla_id, service_id)
);
```

**DSL Verbs Needed:**
```yaml
domains:
  sla:
    verbs:
      define:
        description: Define a service level agreement
        args:
          - name: agreement-type
            valid_values: [STANDARD, ISDA, CSA, OLA]
          - name: target-metric
          - name: target-value
          - name: measurement-period
          
      attach:
        description: Attach SLA to CBU service
        args:
          - name: cbu-id
          - name: sla-id
          - name: service-id
          
      link-isda:
        description: Link ISDA agreement to SLA framework
        args:
          - name: isda-id
          - name: sla-id
```

---

### Gap 3: Settlement Instruction Capture Methods
**Priority:** MEDIUM  
**Impact:** Cannot model instruction delivery paths as service resources

**Current State:**
- `instruction_method` exists in trading profile (SWIFT, CTM, API)
- Not modeled as service resources
- No Omgeo/DTCC explicit handling

**Required:**
```yaml
# New in service_resource_types:
- resource_code: SWIFT_GATEWAY
  resource_type: CONNECTIVITY
  owner: BNY
  config_schema:
    bic: required
    message_types: [MT540, MT541, MT542, MT543, MT544, MT545, MT546, MT547]
    
- resource_code: CTM_CONNECTION
  resource_type: CONNECTIVITY
  owner: DTCC
  config_schema:
    participant_id: required
    matching_rules: optional
    
- resource_code: OMGEO_ALERT
  resource_type: CONNECTIVITY
  owner: DTCC
  config_schema:
    alert_id: required
    enrichment_sources: optional
    
- resource_code: FIX_GATEWAY
  resource_type: CONNECTIVITY
  owner: CLIENT
  config_schema:
    fix_version: required
    session_config: required
```

**DSL Extension:**
```
(service-resource.provision 
  :cbu-id @my-cbu 
  :resource-type "SWIFT_GATEWAY"
  :config {:bic "ALLIGILA" :message-types ["MT541" "MT543"]})
```

---

### Gap 4: IM-Level Trading Matrix Construction
**Priority:** MEDIUM  
**Impact:** Cannot express complex multi-IM mandates in agent chat

**Current State:**
- Trading profile supports IM scoping in YAML
- No DSL verbs for incremental IM configuration
- No agent-friendly matrix builder

**Required DSL Verbs:**
```yaml
domains:
  investment-manager:
    verbs:
      assign:
        description: Assign IM to CBU with scope
        args:
          - name: cbu-id
          - name: manager-lei
          - name: priority
          - name: scope
            type: json
            description: "{all: true} or {markets: [], instrument_classes: []}"
          - name: instruction-method
            valid_values: [SWIFT, CTM, ALERT, FIX, API]
            
      restrict:
        description: Add restriction to IM scope
        args:
          - name: assignment-id
          - name: restriction-type
            valid_values: [MARKET, INSTRUMENT_CLASS, CURRENCY, COUNTERPARTY]
          - name: excluded-values
            type: string_list
            
      set-pricing-source:
        description: Set pricing source for IM scope
        args:
          - name: assignment-id
          - name: instrument-class
          - name: primary-source
            valid_values: [BLOOMBERG, REUTERS, MARKIT, MODEL]
          - name: fallback-source
```

**Agent Chat Example:**
```
User: "CBU IM no1 trades Bonds (and equities US only), IM no2 trades everything else"

Agent generates:
(investment-manager.assign :cbu-id @my-cbu :manager-lei "LEI001" :priority 10
  :scope {:markets ["XNYS" "XNAS"] :instrument_classes ["EQUITY"]})
(investment-manager.assign :cbu-id @my-cbu :manager-lei "LEI001" :priority 10
  :scope {:instrument_classes ["GOVT_BOND" "CORP_BOND"]})
(investment-manager.assign :cbu-id @my-cbu :manager-lei "LEI002" :priority 100
  :scope {:all true})
```

---

### Gap 5: Document Type - INVESTMENT_MANDATE
**Priority:** LOW  
**Impact:** Cannot formally track investment mandate documents

**Current State:**
- Trading profile is a JSON/YAML blob
- No `INVESTMENT_MANDATE` document type in `document_types` table
- No extraction pipeline for mandate documents

**Required:**
```sql
INSERT INTO "ob-poc".document_types (type_code, name, category, retention_years)
VALUES 
  ('INVESTMENT_MANDATE', 'Investment Mandate / IMA', 'OPERATIONAL', 7),
  ('TRADING_AUTHORITY', 'Trading Authority Matrix', 'OPERATIONAL', 7),
  ('SSI_ONBOARDING', 'Standing Settlement Instructions', 'OPERATIONAL', 5);
```

**Attribute Mappings for Extraction:**
```yaml
document_type: INVESTMENT_MANDATE
extractable_attributes:
  - attr_code: ALLOWED_INSTRUMENT_CLASSES
    extraction_type: LIST
  - attr_code: ALLOWED_MARKETS
    extraction_type: LIST
  - attr_code: BASE_CURRENCY
    extraction_type: SINGLE
  - attr_code: INVESTMENT_RESTRICTIONS
    extraction_type: TEXT
  - attr_code: BENCHMARK_INDEX
    extraction_type: SINGLE
```

---

## Storage Strategy Recommendation

Given your original question about Document / JSON / YAML:

| Storage Type | Use Case | Rationale |
|--------------|----------|-----------|
| **YAML** | Trading profile master document | Human-readable, version-controllable, diff-friendly |
| **JSON** | API payloads, config column in DB | Programmatic access, validation |
| **Relational Tables** | Operational data (universe, SSIs, booking rules) | Query performance, referential integrity |
| **Document Catalog** | Source documents (IMA PDFs) | Audit trail, extraction linkage |

**Recommended Flow:**
```
[IMA PDF] → document.catalog → document.extract → 
  → trading-profile.generate (agent-assisted) →
  → trading-profile.import (YAML) →
  → trading-profile.validate →
  → trading-profile.materialize → [Operational Tables]
```

---

## Implementation Priority

| Gap | Priority | Effort | Dependencies |
|-----|----------|--------|--------------|
| STIFs/Cash Sweeps | HIGH | Medium | Instrument class seed |
| SLA Framework | HIGH | High | Schema migration |
| Settlement Connectivity | MEDIUM | Medium | Service resource types |
| IM-Level DSL | MEDIUM | Medium | Trading profile refactor |
| Document Type | LOW | Low | Seed data only |

---

## Next Steps

1. **Immediate:** Add STIF instrument class to seed data
2. **Short-term:** Design SLA schema and verbs
3. **Medium-term:** Implement `investment-manager` domain verbs
4. **Agent Integration:** Train agent on trading matrix construction patterns

---

## Appendix: Current Table Inventory (Custody Schema)

| Table | Purpose | Status |
|-------|---------|--------|
| `cbu_instrument_universe` | What CBU can trade | ✅ |
| `cbu_ssi` | Standing settlement instructions | ✅ |
| `ssi_booking_rules` | SSI routing rules | ✅ |
| `cbu_ssi_agent_override` | Intermediary agent chain | ✅ |
| `isda_agreements` | ISDA master agreements | ✅ |
| `isda_product_coverage` | Asset class coverage | ✅ |
| `csa_agreements` | Credit support annexes | ✅ |
| `instrument_classes` | Reference data | ✅ |
| `markets` | MIC reference data | ✅ |
| `entity_settlement_identity` | Counterparty identities | ✅ |
| `entity_ssi` | Counterparty SSIs | ✅ |
| `cbu_trading_profiles` | Trading profile documents | ✅ |
| `cbu_cash_sweep_config` | Cash sweep rules | ❌ MISSING |
| `service_level_agreements` | SLA definitions | ❌ MISSING |
| `cbu_service_slas` | CBU-specific SLAs | ❌ MISSING |
