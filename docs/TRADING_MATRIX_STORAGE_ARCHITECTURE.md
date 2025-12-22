# Traded Instruments Matrix - Storage Architecture Deep Dive
## Document, Database, Service Resource, and SLA Integration

**Date:** December 22, 2025  
**Author:** Claude + Adam  
**Status:** Architectural Design  
**Version:** 1.0

---

## 1. Architectural Overview

### 1.1 The Traceability Chain

Every piece of operational configuration must trace back to a source document and forward to an SLA commitment:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TRACEABILITY CHAIN                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SOURCE DOCUMENTS          TRADING PROFILE           OPERATIONAL TABLES     │
│  ─────────────────        ───────────────           ──────────────────      │
│                                                                             │
│  ┌──────────────┐         ┌──────────────┐         ┌──────────────────┐    │
│  │ Investment   │────────▶│ cbu_trading_ │────────▶│ cbu_instrument_  │    │
│  │ Mandate (IMA)│         │ profiles     │         │ universe         │    │
│  └──────────────┘         │              │         ├──────────────────┤    │
│                           │  (JSONB doc  │         │ cbu_im_          │    │
│  ┌──────────────┐         │   version-   │         │ assignments      │    │
│  │ SSI Template │────────▶│   controlled)│         ├──────────────────┤    │
│  └──────────────┘         │              │         │ cbu_ssi          │    │
│                           │              │         ├──────────────────┤    │
│  ┌──────────────┐         │              │         │ ssi_booking_     │    │
│  │ ISDA Master  │────────▶│              │         │ rules            │    │
│  └──────────────┘         │              │         ├──────────────────┤    │
│                           │              │         │ isda_agreements  │    │
│  ┌──────────────┐         │              │         ├──────────────────┤    │
│  │ CSA Annex    │────────▶│              │         │ csa_agreements   │    │
│  └──────────────┘         └──────────────┘         └──────────────────┘    │
│                                  │                          │               │
│                                  │    MATERIALIZATION       │               │
│                                  ▼                          ▼               │
│                         ┌──────────────────────────────────────────┐       │
│                         │     SERVICE RESOURCE PROVISIONING        │       │
│                         ├──────────────────────────────────────────┤       │
│                         │                                          │       │
│                         │  ┌─────────────┐    ┌─────────────┐     │       │
│                         │  │ Custody     │    │ SWIFT       │     │       │
│                         │  │ Account     │    │ Gateway     │     │       │
│                         │  └─────────────┘    └─────────────┘     │       │
│                         │  ┌─────────────┐    ┌─────────────┐     │       │
│                         │  │ Cash        │    │ CTM         │     │       │
│                         │  │ Account     │    │ Connection  │     │       │
│                         │  └─────────────┘    └─────────────┘     │       │
│                         │  ┌─────────────┐    ┌─────────────┐     │       │
│                         │  │ NAV         │    │ Pricing     │     │       │
│                         │  │ Engine      │    │ Feed        │     │       │
│                         │  └─────────────┘    └─────────────┘     │       │
│                         │                                          │       │
│                         └──────────────────────────────────────────┘       │
│                                           │                                 │
│                                           ▼                                 │
│                         ┌──────────────────────────────────────────┐       │
│                         │        SERVICE LEVEL AGREEMENTS          │       │
│                         ├──────────────────────────────────────────┤       │
│                         │  Settlement Rate: 99.5% DVP by T+2       │       │
│                         │  NAV Delivery: by 18:00 local            │       │
│                         │  Price Availability: by 17:00            │       │
│                         │  Margin Call: same-day settlement        │       │
│                         └──────────────────────────────────────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Design Principles

1. **Document as Source of Truth** - The Trading Profile document (JSONB) is canonical
2. **Materialization is Deterministic** - Operational tables are derived, not authored
3. **Service Resources are Provisioned** - Technical components provisioned per-CBU
4. **SLAs Bind Everything** - Commitments span documents → resources → metrics
5. **Full Provenance** - Every operational row traces to a profile version and source doc

---

## 2. Document Layer

### 2.1 Document Type Registry

```sql
-- New document types for trading lifecycle
INSERT INTO "ob-poc".document_types (type_code, name, category, retention_years, is_kyc_required)
VALUES 
  -- Primary trading documents
  ('INVESTMENT_MANDATE', 'Investment Management Agreement / IMA', 'OPERATIONAL', 7, false),
  ('TRADING_PROFILE', 'Trading Profile Configuration', 'OPERATIONAL', 7, false),
  ('TRADING_AUTHORITY', 'Trading Authority Matrix', 'OPERATIONAL', 5, false),
  
  -- Settlement documents  
  ('SSI_TEMPLATE', 'Standing Settlement Instructions', 'OPERATIONAL', 5, false),
  ('SUBCUSTODIAN_AGREEMENT', 'Subcustodian Network Agreement', 'OPERATIONAL', 7, false),
  
  -- Connectivity documents
  ('SWIFT_CONFIGURATION', 'SWIFT Gateway Configuration', 'TECHNICAL', 3, false),
  ('CTM_ENROLLMENT', 'CTM/ALERT Enrollment Form', 'TECHNICAL', 3, false),
  ('FIX_SESSION_CONFIG', 'FIX Session Configuration', 'TECHNICAL', 3, false),
  
  -- OTC documents
  ('ISDA_MASTER', 'ISDA Master Agreement', 'LEGAL', 10, false),
  ('CSA_ANNEX', 'Credit Support Annex', 'LEGAL', 10, false),
  ('ISDA_SCHEDULE', 'ISDA Schedule', 'LEGAL', 10, false),
  
  -- SLA documents
  ('SERVICE_AGREEMENT', 'Service Level Agreement', 'LEGAL', 7, false),
  ('OLA_INTERNAL', 'Operational Level Agreement (Internal)', 'OPERATIONAL', 5, false),
  
  -- Pricing documents
  ('PRICING_AGREEMENT', 'Pricing Source Agreement', 'OPERATIONAL', 5, false),
  ('VALUATION_POLICY', 'Fund Valuation Policy', 'REGULATORY', 7, false);
```

### 2.2 Document-Profile Linkage Table

```sql
-- Links source documents to trading profile sections
CREATE TABLE "ob-poc".trading_profile_documents (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),
    profile_section VARCHAR(50) NOT NULL, -- 'universe', 'investment_managers', 'isda_agreements', etc.
    extraction_status VARCHAR(20) DEFAULT 'PENDING',
    extracted_at TIMESTAMPTZ,
    extraction_notes TEXT,
    
    CONSTRAINT valid_profile_section CHECK (profile_section IN (
        'universe', 'investment_managers', 'isda_agreements', 'settlement_config',
        'booking_rules', 'standing_instructions', 'pricing_matrix', 'valuation_config',
        'constraints', 'cash_sweep_config', 'sla_commitments'
    ))
);

COMMENT ON TABLE "ob-poc".trading_profile_documents IS 
'Links source documents (IMA, ISDA, SSI forms) to trading profile sections they populate.
Enables audit: "Where did this booking rule come from?" → traces to source document.';

CREATE INDEX idx_tpd_profile ON "ob-poc".trading_profile_documents(profile_id);
CREATE INDEX idx_tpd_doc ON "ob-poc".trading_profile_documents(doc_id);
```

### 2.3 Enhanced Trading Profile Table

```sql
-- Extend cbu_trading_profiles with provenance tracking
ALTER TABLE "ob-poc".cbu_trading_profiles 
  ADD COLUMN source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  ADD COLUMN materialization_status VARCHAR(20) DEFAULT 'PENDING',
  ADD COLUMN materialized_at TIMESTAMPTZ,
  ADD COLUMN materialization_hash TEXT,
  ADD COLUMN sla_profile_id UUID; -- Links to SLA profile

COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.source_document_id IS 
'Primary source document (IMA) from which this profile was derived or extracted.';

COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.materialization_hash IS 
'Hash of materialized operational tables - used to detect drift.';
```

---

## 3. Operational Tables Layer

### 3.1 Investment Manager Assignments (NEW TABLE)

Currently, IM assignments are embedded in the JSONB. For queryability and SLA binding, we need a proper table:

```sql
-- Investment Manager assignment to CBU with trading scope
CREATE TABLE custody.cbu_im_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    
    -- Manager identification
    manager_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    manager_lei VARCHAR(20),
    manager_bic VARCHAR(11),
    manager_name VARCHAR(255),
    
    -- Role and priority
    manager_role VARCHAR(30) NOT NULL DEFAULT 'INVESTMENT_MANAGER',
    priority INTEGER NOT NULL DEFAULT 100,
    
    -- Scope definition (what can this IM trade?)
    scope_all BOOLEAN DEFAULT FALSE,
    scope_markets TEXT[], -- MIC codes: ['XNYS', 'XLON']
    scope_instrument_classes TEXT[], -- ['EQUITY', 'GOVT_BOND']
    scope_currencies TEXT[], -- ['USD', 'EUR']
    scope_isda_asset_classes TEXT[], -- ['RATES', 'FX', 'CREDIT']
    
    -- Instruction method (how do they send trades?)
    instruction_method VARCHAR(20) NOT NULL, -- 'SWIFT', 'CTM', 'FIX', 'API', 'MANUAL'
    instruction_resource_id UUID, -- Links to provisioned connectivity resource
    
    -- Permissions
    can_trade BOOLEAN DEFAULT TRUE,
    can_settle BOOLEAN DEFAULT TRUE,
    can_affirm BOOLEAN DEFAULT FALSE,
    
    -- Lifecycle
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_im_role CHECK (manager_role IN (
        'INVESTMENT_MANAGER', 'SUB_ADVISOR', 'OVERLAY_MANAGER', 
        'TRANSITION_MANAGER', 'EXECUTION_BROKER'
    )),
    CONSTRAINT valid_instruction_method CHECK (instruction_method IN (
        'SWIFT', 'CTM', 'FIX', 'API', 'MANUAL', 'ALERT'
    ))
);

COMMENT ON TABLE custody.cbu_im_assignments IS 
'Investment Manager assignments with trading scope. Materialized from trading profile.
Links IM to instruction delivery resource for traceability.';

-- Indexes for common lookups
CREATE INDEX idx_cbu_im_cbu ON custody.cbu_im_assignments(cbu_id);
CREATE INDEX idx_cbu_im_profile ON custody.cbu_im_assignments(profile_id);
CREATE INDEX idx_cbu_im_manager ON custody.cbu_im_assignments(manager_entity_id);
CREATE INDEX idx_cbu_im_method ON custody.cbu_im_assignments(instruction_method);
```

### 3.2 Pricing Configuration (NEW TABLE)

```sql
-- Pricing source configuration per CBU/instrument class
CREATE TABLE custody.cbu_pricing_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    
    -- Scope
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    currency VARCHAR(3),
    
    -- Source hierarchy
    priority INTEGER NOT NULL DEFAULT 1,
    source VARCHAR(30) NOT NULL, -- 'BLOOMBERG', 'REUTERS', 'MARKIT', 'MODEL', 'INTERNAL'
    price_type VARCHAR(20) NOT NULL DEFAULT 'CLOSING', -- 'CLOSING', 'MID', 'BID', 'ASK', 'VWAP'
    fallback_source VARCHAR(30),
    
    -- Validation parameters
    max_age_hours INTEGER DEFAULT 24,
    tolerance_pct NUMERIC(5,2) DEFAULT 5.0,
    stale_action VARCHAR(20) DEFAULT 'WARN', -- 'WARN', 'BLOCK', 'USE_FALLBACK'
    
    -- Service resource linkage
    pricing_resource_id UUID, -- Links to provisioned pricing feed resource
    
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_price_source CHECK (source IN (
        'BLOOMBERG', 'REUTERS', 'MARKIT', 'REFINITIV', 'ICE', 
        'MODEL', 'INTERNAL', 'VENDOR', 'COUNTERPARTY'
    ))
);

COMMENT ON TABLE custody.cbu_pricing_config IS 
'Pricing source configuration by instrument class. Materialized from trading profile.
Links to provisioned pricing feed resource for traceability.';
```

### 3.3 Cash Sweep Configuration (NEW TABLE)

```sql
-- Cash sweep / STIF configuration
CREATE TABLE custody.cbu_cash_sweep_config (
    sweep_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    
    -- Currency and threshold
    currency VARCHAR(3) NOT NULL,
    threshold_amount NUMERIC(18,2) NOT NULL,
    
    -- Sweep vehicle
    vehicle_type VARCHAR(20) NOT NULL, -- 'STIF', 'MMF', 'DEPOSIT', 'REPO'
    vehicle_id VARCHAR(50), -- Fund ID or account reference
    vehicle_name VARCHAR(255),
    
    -- Timing
    sweep_time TIME NOT NULL,
    sweep_timezone VARCHAR(50) NOT NULL,
    sweep_frequency VARCHAR(20) DEFAULT 'DAILY', -- 'INTRADAY', 'DAILY', 'WEEKLY'
    
    -- Interest handling
    interest_allocation VARCHAR(20) DEFAULT 'ACCRUED', -- 'ACCRUED', 'MONTHLY', 'QUARTERLY'
    interest_account_id UUID, -- Cash account for interest
    
    -- Service resource
    sweep_resource_id UUID, -- Links to cash management resource
    
    is_active BOOLEAN DEFAULT TRUE,
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_vehicle_type CHECK (vehicle_type IN (
        'STIF', 'MMF', 'DEPOSIT', 'OVERNIGHT_REPO', 'TRI_PARTY_REPO', 'MANUAL'
    ))
);

COMMENT ON TABLE custody.cbu_cash_sweep_config IS 
'Cash sweep configuration for idle cash management. STIFs, MMFs, overnight deposits.';
```

---

## 4. Service Resource Layer

### 4.1 New Resource Types (Seed Data)

```sql
-- Connectivity Resources
INSERT INTO "ob-poc".service_resource_types 
(resource_code, name, resource_type, owner, description, capabilities)
VALUES
  -- Trade Instruction Capture
  ('SWIFT_GATEWAY', 'SWIFT Message Gateway', 'CONNECTIVITY', 'BNY',
   'SWIFT message routing for settlement instructions',
   '{"message_types": ["MT540", "MT541", "MT542", "MT543", "MT544", "MT545", "MT546", "MT547"],
     "directions": ["SEND", "RECEIVE"],
     "protocols": ["FIN", "FileAct"]}'::jsonb),
     
  ('CTM_CONNECTION', 'CTM Trade Matching', 'CONNECTIVITY', 'DTCC',
   'Omgeo CTM for automated trade matching and confirmation',
   '{"services": ["MATCH", "AFFIRM", "CONFIRM"],
     "asset_classes": ["EQUITY", "FIXED_INCOME"]}'::jsonb),
     
  ('ALERT_CONNECTION', 'ALERT SSI Enrichment', 'CONNECTIVITY', 'DTCC',
   'Omgeo ALERT for SSI lookup and enrichment',
   '{"services": ["SSI_LOOKUP", "ENRICHMENT", "VALIDATION"]}'::jsonb),
     
  ('FIX_SESSION', 'FIX Protocol Session', 'CONNECTIVITY', 'CLIENT',
   'FIX session for electronic trade capture',
   '{"versions": ["4.2", "4.4", "5.0SP2"],
     "message_types": ["NewOrderSingle", "ExecutionReport", "AllocationInstruction"]}'::jsonb),
     
  ('API_ENDPOINT', 'REST/gRPC API Endpoint', 'CONNECTIVITY', 'CLIENT',
   'API integration for trade and instruction delivery',
   '{"protocols": ["REST", "gRPC", "GraphQL"],
     "auth_methods": ["OAuth2", "mTLS", "API_KEY"]}'::jsonb),

  -- Pricing Resources  
  ('BLOOMBERG_TERMINAL', 'Bloomberg Terminal Feed', 'PRICING', 'BLOOMBERG',
   'Bloomberg pricing and reference data',
   '{"data_types": ["PRICE", "REFERENCE", "CORPORATE_ACTIONS"],
     "asset_classes": ["EQUITY", "FIXED_INCOME", "FX", "COMMODITIES"]}'::jsonb),
     
  ('BLOOMBERG_BVAL', 'Bloomberg BVAL', 'PRICING', 'BLOOMBERG',
   'Bloomberg evaluated pricing for OTC and illiquid instruments',
   '{"data_types": ["EVALUATED_PRICE", "YIELD", "SPREAD"],
     "asset_classes": ["FIXED_INCOME", "DERIVATIVES", "STRUCTURED"]}'::jsonb),
     
  ('REFINITIV_FEED', 'Refinitiv Real-Time Feed', 'PRICING', 'REFINITIV',
   'Refinitiv (formerly Reuters) pricing data',
   '{"data_types": ["PRICE", "REFERENCE"],
     "delivery": ["REAL_TIME", "EOD"]}'::jsonb),
     
  ('MARKIT_PRICING', 'Markit Pricing Service', 'PRICING', 'MARKIT',
   'Markit evaluated pricing for derivatives and credit',
   '{"data_types": ["PRICE", "SPREAD", "CURVE"],
     "asset_classes": ["CDS", "CDX", "LOANS", "BONDS"]}'::jsonb),
     
  ('ICE_PRICING', 'ICE Data Services', 'PRICING', 'ICE',
   'ICE pricing for fixed income and derivatives',
   '{"data_types": ["PRICE", "ANALYTICS"],
     "asset_classes": ["FIXED_INCOME", "DERIVATIVES"]}'::jsonb),

  -- Cash Management Resources
  ('CASH_SWEEP_ENGINE', 'Cash Sweep Engine', 'CASH_MANAGEMENT', 'BNY',
   'Automated cash sweep processing',
   '{"sweep_types": ["STIF", "MMF", "DEPOSIT", "REPO"],
     "frequencies": ["INTRADAY", "DAILY"]}'::jsonb),
     
  ('STIF_ACCOUNT', 'Short-Term Investment Fund', 'CASH_MANAGEMENT', 'BNY',
   'BNY institutional cash fund',
   '{"fund_types": ["GOVT", "PRIME", "TREASURY"],
     "nav_frequency": "DAILY"}'::jsonb),

  -- Settlement Resources (extend existing)
  ('SETTLEMENT_INSTRUCTION_ENGINE', 'Settlement Instruction Generator', 'SETTLEMENT', 'BNY',
   'Generates SWIFT/ISO20022 settlement messages',
   '{"message_standards": ["SWIFT", "ISO20022"],
     "instruction_types": ["DVP", "FOP", "RVP", "DFP"]}'::jsonb),
     
  ('CSD_GATEWAY', 'CSD Direct Connection', 'SETTLEMENT', 'BNY',
   'Direct connectivity to Central Securities Depositories',
   '{"csds": ["DTCC", "Euroclear", "Clearstream", "JASDEC", "CCASS"]}'::jsonb);
```

### 4.2 Resource Dependencies for Trading Matrix

```sql
-- Extend resource_dependencies for trading matrix resources
INSERT INTO "ob-poc".resource_dependencies 
(resource_type_id, depends_on_type_id, dependency_type, inject_arg, priority)
SELECT 
  rt1.resource_id,
  rt2.resource_id,
  'required',
  dep.inject_arg,
  dep.priority
FROM (VALUES
  -- Settlement instruction engine depends on custody account
  ('SETTLEMENT_INSTRUCTION_ENGINE', 'CUSTODY_ACCOUNT', 'custody-account-url', 10),
  -- Settlement instruction engine depends on SWIFT gateway
  ('SETTLEMENT_INSTRUCTION_ENGINE', 'SWIFT_GATEWAY', 'swift-gateway-url', 20),
  -- CTM connection depends on settlement identity
  ('CTM_CONNECTION', 'CUSTODY_ACCOUNT', 'settlement-identity-url', 10),
  -- Pricing config depends on NAV engine
  ('BLOOMBERG_TERMINAL', 'NAV_ENGINE', 'nav-engine-url', 10),
  -- Cash sweep depends on cash account
  ('CASH_SWEEP_ENGINE', 'CASH_ACCOUNT', 'cash-account-url', 10),
  -- STIF depends on sweep engine
  ('STIF_ACCOUNT', 'CASH_SWEEP_ENGINE', 'sweep-engine-url', 10)
) AS dep(resource_code, depends_on_code, inject_arg, priority)
JOIN "ob-poc".service_resource_types rt1 ON rt1.resource_code = dep.resource_code
JOIN "ob-poc".service_resource_types rt2 ON rt2.resource_code = dep.depends_on_code;
```

### 4.3 Resource-Profile Linkage Table

```sql
-- Links provisioned resources back to trading profile sections
CREATE TABLE "ob-poc".resource_profile_sources (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    profile_section VARCHAR(50) NOT NULL,
    profile_path TEXT, -- JSON path within profile, e.g., '$.investment_managers[0].instruction_method'
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

COMMENT ON TABLE "ob-poc".resource_profile_sources IS 
'Links provisioned service resources back to their source in the trading profile.
Enables: "Why was this SWIFT gateway provisioned?" → "investment_managers[0].instruction_method = SWIFT"';

CREATE INDEX idx_rps_instance ON "ob-poc".resource_profile_sources(instance_id);
CREATE INDEX idx_rps_profile ON "ob-poc".resource_profile_sources(profile_id);
```

---

## 5. SLA Framework

### 5.1 SLA Definition Tables

```sql
-- SLA metric types (reference data)
CREATE TABLE "ob-poc".sla_metric_types (
    metric_code VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    metric_category VARCHAR(30) NOT NULL, -- 'TIMELINESS', 'ACCURACY', 'AVAILABILITY', 'VOLUME'
    unit VARCHAR(20) NOT NULL, -- 'PERCENT', 'HOURS', 'MINUTES', 'COUNT', 'CURRENCY'
    aggregation_method VARCHAR(20) DEFAULT 'AVERAGE', -- 'AVERAGE', 'SUM', 'MIN', 'MAX', 'MEDIAN'
    higher_is_better BOOLEAN DEFAULT TRUE,
    is_active BOOLEAN DEFAULT TRUE
);

INSERT INTO "ob-poc".sla_metric_types (metric_code, name, metric_category, unit, higher_is_better) VALUES
  -- Settlement metrics
  ('SETTLEMENT_RATE', 'On-time Settlement Rate', 'ACCURACY', 'PERCENT', TRUE),
  ('SETTLEMENT_FAIL_RATE', 'Settlement Failure Rate', 'ACCURACY', 'PERCENT', FALSE),
  ('INSTRUCTION_LATENCY', 'Instruction Processing Latency', 'TIMELINESS', 'MINUTES', FALSE),
  
  -- Pricing/NAV metrics
  ('NAV_DELIVERY_TIME', 'NAV Delivery Time', 'TIMELINESS', 'HOURS', FALSE),
  ('PRICE_AVAILABILITY', 'Price Availability Rate', 'AVAILABILITY', 'PERCENT', TRUE),
  ('PRICE_STALENESS', 'Average Price Age', 'TIMELINESS', 'HOURS', FALSE),
  
  -- Trade capture metrics
  ('MATCH_RATE', 'Trade Match Rate', 'ACCURACY', 'PERCENT', TRUE),
  ('AFFIRMATION_TIME', 'Average Affirmation Time', 'TIMELINESS', 'HOURS', FALSE),
  
  -- Margin/Collateral metrics
  ('MARGIN_CALL_TIMELINESS', 'Margin Call Processing Time', 'TIMELINESS', 'HOURS', FALSE),
  ('COLLATERAL_ELIGIBILITY_ACCURACY', 'Collateral Validation Accuracy', 'ACCURACY', 'PERCENT', TRUE),
  
  -- Cash management metrics
  ('SWEEP_EXECUTION_RATE', 'Sweep Execution Success Rate', 'ACCURACY', 'PERCENT', TRUE),
  ('INTEREST_ALLOCATION_ACCURACY', 'Interest Allocation Accuracy', 'ACCURACY', 'PERCENT', TRUE);
```

```sql
-- SLA Templates (standard commitments)
CREATE TABLE "ob-poc".sla_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    
    -- What this SLA applies to
    applies_to_type VARCHAR(30) NOT NULL, -- 'SERVICE', 'RESOURCE_TYPE', 'ISDA', 'CSA'
    applies_to_code VARCHAR(50), -- service_code or resource_code
    
    -- Commitment
    metric_code VARCHAR(50) NOT NULL REFERENCES "ob-poc".sla_metric_types(metric_code),
    target_value NUMERIC(10,4) NOT NULL,
    warning_threshold NUMERIC(10,4), -- Threshold before breach
    measurement_period VARCHAR(20) DEFAULT 'MONTHLY', -- 'DAILY', 'WEEKLY', 'MONTHLY', 'QUARTERLY'
    
    -- Response commitments
    response_time_hours NUMERIC(5,2), -- For issue resolution
    escalation_path TEXT,
    
    -- Metadata
    regulatory_requirement BOOLEAN DEFAULT FALSE,
    regulatory_reference TEXT,
    
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Standard SLA templates
INSERT INTO "ob-poc".sla_templates 
(template_code, name, applies_to_type, applies_to_code, metric_code, target_value, warning_threshold, measurement_period)
VALUES
  -- Custody settlement SLAs
  ('CUSTODY_SETTLE_DVP', 'DVP Settlement Rate', 'SERVICE', 'CUSTODY', 'SETTLEMENT_RATE', 99.5, 98.0, 'MONTHLY'),
  ('CUSTODY_SETTLE_INSTR', 'Instruction Processing', 'SERVICE', 'CUSTODY', 'INSTRUCTION_LATENCY', 30, 60, 'DAILY'),
  
  -- Fund Accounting SLAs
  ('FA_NAV_DELIVERY', 'NAV Delivery Time', 'SERVICE', 'FUND_ACCOUNTING', 'NAV_DELIVERY_TIME', 18, 19, 'DAILY'),
  ('FA_PRICE_AVAIL', 'Price Availability', 'SERVICE', 'FUND_ACCOUNTING', 'PRICE_AVAILABILITY', 99.0, 97.0, 'DAILY'),
  
  -- Trade capture SLAs
  ('TC_MATCH_RATE', 'Trade Match Rate', 'RESOURCE_TYPE', 'CTM_CONNECTION', 'MATCH_RATE', 98.0, 95.0, 'MONTHLY'),
  ('TC_AFFIRM_TIME', 'Affirmation Turnaround', 'RESOURCE_TYPE', 'CTM_CONNECTION', 'AFFIRMATION_TIME', 4, 8, 'DAILY'),
  
  -- Pricing feed SLAs
  ('PRICE_BLOOMBERG', 'Bloomberg Feed Availability', 'RESOURCE_TYPE', 'BLOOMBERG_TERMINAL', 'PRICE_AVAILABILITY', 99.9, 99.5, 'MONTHLY'),
  
  -- CSA-specific SLAs
  ('CSA_MARGIN_CALL', 'Margin Call Timeliness', 'CSA', NULL, 'MARGIN_CALL_TIMELINESS', 2, 4, 'DAILY'),
  ('CSA_COLLATERAL_VAL', 'Collateral Validation', 'CSA', NULL, 'COLLATERAL_ELIGIBILITY_ACCURACY', 100.0, 99.0, 'DAILY');
```

### 5.2 CBU-Specific SLA Bindings

```sql
-- CBU-specific SLA commitments
CREATE TABLE "ob-poc".cbu_sla_commitments (
    commitment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    profile_id UUID REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    template_id UUID REFERENCES "ob-poc".sla_templates(template_id),
    
    -- Override target if negotiated differently
    override_target_value NUMERIC(10,4),
    override_warning_threshold NUMERIC(10,4),
    
    -- Specific bindings (what exactly does this SLA cover?)
    bound_service_id UUID REFERENCES "ob-poc".services(service_id),
    bound_resource_instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    bound_isda_id UUID, -- REFERENCES custody.isda_agreements
    bound_csa_id UUID, -- REFERENCES custody.csa_agreements
    
    -- Scope restrictions (SLA only applies to this subset)
    scope_instrument_classes TEXT[],
    scope_markets TEXT[],
    scope_currencies TEXT[],
    scope_counterparties UUID[],
    
    -- Commercial terms
    penalty_structure JSONB, -- {"breach_pct": 10, "credit_type": "FEE_REBATE"}
    incentive_structure JSONB, -- {"exceed_pct": 5, "bonus_type": "FEE_REDUCTION"}
    
    -- Lifecycle
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    negotiated_by VARCHAR(255),
    negotiated_date DATE,
    
    -- Source tracking
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

COMMENT ON TABLE "ob-poc".cbu_sla_commitments IS 
'CBU-specific SLA commitments. Links trading profile sections, service resources, 
and ISDA/CSA agreements to measurable SLA targets. Enables: 
"What is the settlement SLA for this CBU?" and 
"What resources are covered by this SLA?"';

CREATE INDEX idx_cbu_sla_cbu ON "ob-poc".cbu_sla_commitments(cbu_id);
CREATE INDEX idx_cbu_sla_profile ON "ob-poc".cbu_sla_commitments(profile_id);
CREATE INDEX idx_cbu_sla_resource ON "ob-poc".cbu_sla_commitments(bound_resource_instance_id);
```

### 5.3 SLA Measurement and Breach Tracking

```sql
-- SLA measurements (periodic snapshots)
CREATE TABLE "ob-poc".sla_measurements (
    measurement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commitment_id UUID NOT NULL REFERENCES "ob-poc".cbu_sla_commitments(commitment_id),
    
    -- Measurement period
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    
    -- Actual performance
    measured_value NUMERIC(10,4) NOT NULL,
    sample_size INTEGER, -- Number of transactions/events measured
    
    -- Status
    status VARCHAR(20) NOT NULL, -- 'MET', 'WARNING', 'BREACH'
    variance_pct NUMERIC(6,2), -- How far from target (positive = better)
    
    -- Details
    measurement_notes TEXT,
    measurement_method VARCHAR(50), -- 'AUTOMATED', 'MANUAL', 'ESTIMATED'
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- SLA breaches (for remediation tracking)
CREATE TABLE "ob-poc".sla_breaches (
    breach_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    measurement_id UUID NOT NULL REFERENCES "ob-poc".sla_measurements(measurement_id),
    commitment_id UUID NOT NULL REFERENCES "ob-poc".cbu_sla_commitments(commitment_id),
    
    -- Breach details
    breach_severity VARCHAR(20) NOT NULL, -- 'MINOR', 'MAJOR', 'CRITICAL'
    breach_date DATE NOT NULL,
    detected_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Root cause
    root_cause_category VARCHAR(50), -- 'SYSTEM', 'VENDOR', 'MARKET', 'CLIENT', 'INTERNAL'
    root_cause_description TEXT,
    
    -- Remediation
    remediation_status VARCHAR(20) DEFAULT 'OPEN', -- 'OPEN', 'IN_PROGRESS', 'RESOLVED', 'WAIVED'
    remediation_plan TEXT,
    remediation_due_date DATE,
    remediation_completed_at TIMESTAMPTZ,
    
    -- Financial impact
    penalty_applied BOOLEAN DEFAULT FALSE,
    penalty_amount NUMERIC(18,2),
    penalty_currency VARCHAR(3),
    
    -- Escalation
    escalated_to VARCHAR(255),
    escalated_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## 6. DSL Verb Definitions

### 6.1 Trading Profile Verbs (Extended)

```yaml
# config/verbs/trading-profile.yaml (extensions)
domains:
  trading-profile:
    verbs:
      # ... existing verbs ...
      
      link-document:
        description: Link source document to trading profile section
        behavior: crud
        crud:
          operation: insert
          table: trading_profile_documents
          schema: ob-poc
          returning: link_id
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: document-id
            type: uuid
            required: true
            lookup:
              table: document_catalog
              entity_type: document
          - name: section
            type: string
            required: true
            valid_values:
              - universe
              - investment_managers
              - isda_agreements
              - settlement_config
              - booking_rules
              - standing_instructions
              - pricing_matrix
              - valuation_config
              - cash_sweep_config
              - sla_commitments
        returns:
          type: uuid
          name: link_id
          
      generate-from-document:
        description: Extract and generate trading profile section from document
        behavior: plugin
        handler: generate_profile_from_document
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: document-id
            type: uuid
            required: true
          - name: section
            type: string
            required: true
          - name: merge-mode
            type: string
            required: false
            default: APPEND
            valid_values: [REPLACE, APPEND, MERGE]
        returns:
          type: record
          
      provision-resources:
        description: Provision all service resources required by trading profile
        behavior: plugin
        handler: provision_profile_resources
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: dry-run
            type: boolean
            required: false
            default: false
          - name: sections
            type: string_list
            required: false
            description: Specific sections to provision (default all)
        returns:
          type: record
          description: Provisioning results with resource instance URLs
```

### 6.2 Investment Manager Verbs (NEW)

```yaml
# config/verbs/investment-manager.yaml
domains:
  investment-manager:
    description: Investment manager assignment and scope management
    verbs:
      assign:
        description: Assign investment manager to CBU with trading scope
        behavior: crud
        crud:
          operation: insert
          table: cbu_im_assignments
          schema: custody
          returning: assignment_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
          - name: manager-lei
            type: string
            required: true
            maps_to: manager_lei
          - name: priority
            type: integer
            required: true
          - name: role
            type: string
            required: false
            default: INVESTMENT_MANAGER
            valid_values:
              - INVESTMENT_MANAGER
              - SUB_ADVISOR
              - OVERLAY_MANAGER
              - TRANSITION_MANAGER
          - name: scope-all
            type: boolean
            required: false
            default: false
          - name: scope-markets
            type: string_list
            required: false
          - name: scope-instrument-classes
            type: string_list
            required: false
          - name: scope-currencies
            type: string_list
            required: false
          - name: instruction-method
            type: string
            required: true
            valid_values: [SWIFT, CTM, FIX, API, ALERT, MANUAL]
        returns:
          type: uuid
          name: assignment_id
          capture: true
          
      set-scope:
        description: Update scope for existing IM assignment
        behavior: crud
        crud:
          operation: update
          table: cbu_im_assignments
          schema: custody
          key: assignment_id
        args:
          - name: assignment-id
            type: uuid
            required: true
          - name: scope-markets
            type: string_list
            required: false
          - name: scope-instrument-classes
            type: string_list
            required: false
          - name: scope-currencies
            type: string_list
            required: false
        returns:
          type: affected
          
      link-connectivity:
        description: Link IM to instruction delivery resource
        behavior: crud
        crud:
          operation: update
          table: cbu_im_assignments
          schema: custody
          key: assignment_id
        args:
          - name: assignment-id
            type: uuid
            required: true
          - name: resource-instance-id
            type: uuid
            required: true
        returns:
          type: affected
```

### 6.3 SLA Verbs (NEW)

```yaml
# config/verbs/sla.yaml
domains:
  sla:
    description: Service Level Agreement management
    verbs:
      commit:
        description: Create SLA commitment for CBU
        behavior: crud
        crud:
          operation: insert
          table: cbu_sla_commitments
          schema: ob-poc
          returning: commitment_id
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: template-code
            type: string
            required: true
            lookup:
              table: sla_templates
              code_column: template_code
              id_column: template_id
          - name: profile-id
            type: uuid
            required: false
          - name: service-id
            type: uuid
            required: false
          - name: resource-instance-id
            type: uuid
            required: false
          - name: override-target
            type: decimal
            required: false
          - name: scope-instrument-classes
            type: string_list
            required: false
          - name: scope-markets
            type: string_list
            required: false
          - name: effective-date
            type: date
            required: false
        returns:
          type: uuid
          name: commitment_id
          capture: true
          
      bind-to-profile:
        description: Bind SLA commitment to trading profile
        behavior: crud
        crud:
          operation: update
          table: cbu_sla_commitments
          schema: ob-poc
          key: commitment_id
        args:
          - name: commitment-id
            type: uuid
            required: true
          - name: profile-id
            type: uuid
            required: true
        returns:
          type: affected
          
      bind-to-resource:
        description: Bind SLA commitment to service resource instance
        behavior: crud
        crud:
          operation: update
          table: cbu_sla_commitments
          schema: ob-poc
          key: commitment_id
        args:
          - name: commitment-id
            type: uuid
            required: true
          - name: resource-instance-id
            type: uuid
            required: true
        returns:
          type: affected
          
      bind-to-isda:
        description: Bind SLA commitment to ISDA agreement
        behavior: crud
        crud:
          operation: update
          table: cbu_sla_commitments
          schema: ob-poc
          key: commitment_id
        args:
          - name: commitment-id
            type: uuid
            required: true
          - name: isda-id
            type: uuid
            required: true
        returns:
          type: affected
          
      record-measurement:
        description: Record SLA measurement for period
        behavior: crud
        crud:
          operation: insert
          table: sla_measurements
          schema: ob-poc
          returning: measurement_id
        args:
          - name: commitment-id
            type: uuid
            required: true
          - name: period-start
            type: date
            required: true
          - name: period-end
            type: date
            required: true
          - name: measured-value
            type: decimal
            required: true
          - name: sample-size
            type: integer
            required: false
        returns:
          type: uuid
          name: measurement_id
          
      report-breach:
        description: Report SLA breach
        behavior: plugin
        handler: report_sla_breach
        args:
          - name: measurement-id
            type: uuid
            required: true
          - name: severity
            type: string
            required: true
            valid_values: [MINOR, MAJOR, CRITICAL]
          - name: root-cause-category
            type: string
            required: true
            valid_values: [SYSTEM, VENDOR, MARKET, CLIENT, INTERNAL]
          - name: root-cause-description
            type: string
            required: true
          - name: remediation-plan
            type: string
            required: false
        returns:
          type: uuid
          name: breach_id
```

### 6.4 Pricing Configuration Verbs (NEW)

```yaml
# config/verbs/pricing-config.yaml
domains:
  pricing-config:
    description: Pricing source configuration management
    verbs:
      set:
        description: Set pricing source for instrument class
        behavior: crud
        crud:
          operation: upsert
          table: cbu_pricing_config
          schema: custody
          conflict_keys: [cbu_id, instrument_class_id, market_id, priority]
          returning: config_id
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: profile-id
            type: uuid
            required: true
          - name: instrument-class
            type: lookup
            required: true
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: priority
            type: integer
            required: true
          - name: source
            type: string
            required: true
            valid_values: [BLOOMBERG, REUTERS, MARKIT, REFINITIV, ICE, MODEL, INTERNAL]
          - name: price-type
            type: string
            required: false
            default: CLOSING
            valid_values: [CLOSING, MID, BID, ASK, VWAP]
          - name: fallback-source
            type: string
            required: false
          - name: max-age-hours
            type: integer
            required: false
            default: 24
          - name: tolerance-pct
            type: decimal
            required: false
            default: 5.0
        returns:
          type: uuid
          name: config_id
          
      link-resource:
        description: Link pricing config to pricing feed resource
        behavior: crud
        crud:
          operation: update
          table: cbu_pricing_config
          schema: custody
          key: config_id
        args:
          - name: config-id
            type: uuid
            required: true
          - name: pricing-resource-id
            type: uuid
            required: true
        returns:
          type: affected
```

### 6.5 Cash Sweep Verbs (NEW)

```yaml
# config/verbs/cash-sweep.yaml
domains:
  cash-sweep:
    description: Cash sweep and STIF configuration
    verbs:
      configure:
        description: Configure cash sweep for currency
        behavior: crud
        crud:
          operation: upsert
          table: cbu_cash_sweep_config
          schema: custody
          conflict_keys: [cbu_id, currency]
          returning: sweep_id
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: profile-id
            type: uuid
            required: true
          - name: currency
            type: string
            required: true
          - name: threshold-amount
            type: decimal
            required: true
          - name: vehicle-type
            type: string
            required: true
            valid_values: [STIF, MMF, DEPOSIT, OVERNIGHT_REPO, TRI_PARTY_REPO]
          - name: vehicle-id
            type: string
            required: false
          - name: sweep-time
            type: string
            required: true
          - name: sweep-timezone
            type: string
            required: true
        returns:
          type: uuid
          name: sweep_id
          capture: true
          
      link-resource:
        description: Link sweep config to cash management resource
        behavior: crud
        crud:
          operation: update
          table: cbu_cash_sweep_config
          schema: custody
          key: sweep_id
        args:
          - name: sweep-id
            type: uuid
            required: true
          - name: sweep-resource-id
            type: uuid
            required: true
        returns:
          type: affected
```

---

## 7. Complete Example: Agent Chat Flow

### 7.1 Building the Matrix Conversationally

```
USER: I need to set up the trading profile for AllianzGI Global Multi-Asset.
      IM1 (European Equities Specialist, LEI: 549300EUEQUITYSPEC01) handles
      European equities via CTM. IM2 (Derivatives Overlay, LEI: 549300DERIVOVERLAY01)
      handles OTC derivatives via API. Default IM handles everything else via SWIFT.

AGENT: I'll create the investment manager assignments for your CBU.

(investment-manager.assign 
  :cbu-id @allianzgi-global-multi-asset
  :manager-lei "549300EUEQUITYSPEC01"
  :priority 10
  :role INVESTMENT_MANAGER
  :scope-markets ["XETR" "XLON" "XSWX" "XPAR" "XAMS"]
  :scope-instrument-classes ["EQUITY"]
  :instruction-method CTM)
  
(investment-manager.assign 
  :cbu-id @allianzgi-global-multi-asset
  :manager-lei "549300DERIVOVERLAY01"
  :priority 10
  :role OVERLAY_MANAGER
  :scope-instrument-classes ["OTC_DERIVATIVE"]
  :instruction-method API)
  
(investment-manager.assign 
  :cbu-id @allianzgi-global-multi-asset
  :manager-lei "5493001KJTIIGC8Y1R12"
  :priority 100
  :role INVESTMENT_MANAGER
  :scope-all true
  :instruction-method SWIFT)

USER: Now provision the connectivity resources for each IM.

AGENT: I'll provision the required connectivity resources and link them.

; CTM for European Equities IM
(service-resource.provision 
  :cbu-id @allianzgi-global-multi-asset
  :resource-type "CTM_CONNECTION"
  :instance-name "CTM-EU-EQUITY"
  :config {:participant_id "ALLIANZGI-CTM-001" 
           :matching_rules {:auto_match true :tolerance_price_pct 0.01}})
→ @ctm-eu-equity

; Link IM to CTM
(investment-manager.link-connectivity
  :assignment-id @im-eu-equity
  :resource-instance-id @ctm-eu-equity)

; API for Derivatives Overlay
(service-resource.provision
  :cbu-id @allianzgi-global-multi-asset
  :resource-type "API_ENDPOINT"
  :instance-name "API-DERIV-OVERLAY"
  :config {:protocol "gRPC" :auth_method "mTLS"})
→ @api-deriv

(investment-manager.link-connectivity
  :assignment-id @im-deriv-overlay
  :resource-instance-id @api-deriv)

; SWIFT for default IM
(service-resource.provision
  :cbu-id @allianzgi-global-multi-asset
  :resource-type "SWIFT_GATEWAY"
  :instance-name "SWIFT-DEFAULT"
  :config {:bic "ALLIGILA" :message_types ["MT541" "MT543" "MT545" "MT547"]})
→ @swift-default

(investment-manager.link-connectivity
  :assignment-id @im-default
  :resource-instance-id @swift-default)

USER: Set up pricing - Bloomberg for equities and ETFs, Markit for credit.

AGENT: I'll configure the pricing sources.

(pricing-config.set
  :cbu-id @allianzgi-global-multi-asset
  :profile-id @current-profile
  :instrument-class "EQUITY"
  :priority 1
  :source BLOOMBERG
  :price-type CLOSING
  :max-age-hours 24)
  
(pricing-config.set
  :cbu-id @allianzgi-global-multi-asset
  :profile-id @current-profile
  :instrument-class "ETF"
  :priority 1
  :source BLOOMBERG
  :price-type CLOSING)
  
(pricing-config.set
  :cbu-id @allianzgi-global-multi-asset
  :profile-id @current-profile
  :instrument-class "CORP_BOND"
  :priority 1
  :source MARKIT
  :price-type MID
  :fallback-source BLOOMBERG)

; Provision and link pricing resources
(service-resource.provision
  :cbu-id @allianzgi-global-multi-asset
  :resource-type "BLOOMBERG_TERMINAL"
  :instance-name "BBG-PRICING")
→ @bbg-pricing

(pricing-config.link-resource
  :config-id @equity-pricing-config
  :pricing-resource-id @bbg-pricing)

USER: Set up the settlement SLA - 99.5% DVP by T+2, with penalty for breach.

AGENT: I'll create the SLA commitment.

(sla.commit
  :cbu-id @allianzgi-global-multi-asset
  :template-code "CUSTODY_SETTLE_DVP"
  :profile-id @current-profile
  :override-target 99.5
  :effective-date "2025-01-01")
→ @settle-sla

; Bind to custody service
(sla.bind-to-resource
  :commitment-id @settle-sla
  :resource-instance-id @custody-account)

USER: Finally, configure cash sweeps - EUR to STIF above 50k at 17:00 Luxembourg.

AGENT: I'll configure the cash sweep.

(cash-sweep.configure
  :cbu-id @allianzgi-global-multi-asset
  :profile-id @current-profile
  :currency "EUR"
  :threshold-amount 50000
  :vehicle-type STIF
  :vehicle-id "BNYINSTCASH001"
  :sweep-time "17:00"
  :sweep-timezone "Europe/Luxembourg")
→ @eur-sweep

(service-resource.provision
  :cbu-id @allianzgi-global-multi-asset
  :resource-type "CASH_SWEEP_ENGINE"
  :instance-name "SWEEP-EUR"
  :depends-on [@cash-account-eur])
→ @sweep-engine

(cash-sweep.link-resource
  :sweep-id @eur-sweep
  :sweep-resource-id @sweep-engine)
```

---

## 8. Traceability Queries

### 8.1 "What resources power this CBU's trading?"

```sql
SELECT 
    ri.instance_name,
    srt.resource_code,
    srt.resource_type,
    ri.status,
    rps.profile_section,
    rps.profile_path
FROM "ob-poc".cbu_resource_instances ri
JOIN "ob-poc".service_resource_types srt ON ri.resource_type_id = srt.resource_id
LEFT JOIN "ob-poc".resource_profile_sources rps ON ri.instance_id = rps.instance_id
WHERE ri.cbu_id = :cbu_id
ORDER BY srt.resource_type, ri.instance_name;
```

### 8.2 "What's the SLA coverage for this instrument class?"

```sql
SELECT 
    sc.commitment_id,
    st.template_code,
    st.name,
    smt.metric_code,
    COALESCE(sc.override_target_value, st.target_value) as target,
    sc.scope_instrument_classes,
    ri.instance_name as bound_resource
FROM "ob-poc".cbu_sla_commitments sc
JOIN "ob-poc".sla_templates st ON sc.template_id = st.template_id
JOIN "ob-poc".sla_metric_types smt ON st.metric_code = smt.metric_code
LEFT JOIN "ob-poc".cbu_resource_instances ri ON sc.bound_resource_instance_id = ri.instance_id
WHERE sc.cbu_id = :cbu_id
  AND (sc.scope_instrument_classes IS NULL 
       OR :instrument_class = ANY(sc.scope_instrument_classes))
  AND sc.status = 'ACTIVE';
```

### 8.3 "Trace booking rule back to source document"

```sql
SELECT 
    br.rule_name,
    br.priority,
    tp.version as profile_version,
    tp.created_at as profile_created,
    tpd.profile_section,
    dc.document_name,
    dt.name as document_type,
    dc.uploaded_at as document_date
FROM custody.ssi_booking_rules br
JOIN "ob-poc".cbu_trading_profiles tp ON br.cbu_id = (
    SELECT cbu_id FROM "ob-poc".cbu_trading_profiles WHERE profile_id = :profile_id
)
LEFT JOIN "ob-poc".trading_profile_documents tpd ON tp.profile_id = tpd.profile_id
    AND tpd.profile_section = 'booking_rules'
LEFT JOIN "ob-poc".document_catalog dc ON tpd.doc_id = dc.doc_id
LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
WHERE br.rule_id = :rule_id;
```

---

## 9. Migration Path

### Phase 1: Schema (Week 1)
1. Create new tables (cbu_im_assignments, cbu_pricing_config, cbu_cash_sweep_config)
2. Create SLA framework tables
3. Create resource-profile linkage tables
4. Add document types

### Phase 2: Seed Data (Week 1)
1. Insert new service resource types
2. Insert SLA metric types and templates
3. Update resource dependencies

### Phase 3: Verbs (Week 2)
1. Add investment-manager domain verbs
2. Add sla domain verbs
3. Add pricing-config domain verbs
4. Add cash-sweep domain verbs
5. Extend trading-profile verbs

### Phase 4: Materialization Logic (Week 2-3)
1. Update trading-profile.materialize to populate new tables
2. Implement resource provisioning from profile
3. Implement SLA binding from profile

### Phase 5: Agent Training (Week 3)
1. Add examples to agent prompt
2. Train on matrix construction patterns
3. Test conversational flows

---

## 10. Summary

This design provides:

1. **Clean Document Storage** - Trading Profile as versioned JSONB with source document linkage
2. **Queryable Operational Tables** - IM assignments, pricing configs, sweep configs materialized for performance
3. **Service Resource Traceability** - Every resource instance traces back to profile section
4. **SLA Integration** - Commitments bind to services, resources, ISDA/CSA agreements
5. **Full Provenance** - Every operational row can be traced to source document
6. **DSL Verb Coverage** - Declarative verbs for all operations
7. **Agent-Friendly** - Conversational matrix construction supported

The key insight is that the **Trading Profile document is the hub** - documents flow into it, operational tables flow out of it, resources are provisioned from it, and SLAs bind across it.
