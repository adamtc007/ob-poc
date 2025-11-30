# TASK: Custody Bank Product Seed Data & Onboarding Test Harness

## Overview

Seed realistic financial services product taxonomy for a custody bank, then create test harness to exercise full onboarding flows, and finally integrate with agent DSL generation.

**Products to Model:**
1. **Global Custody** — Asset safekeeping, settlement, corporate actions
2. **Fund Accounting** — NAV calculation, investor accounting, reporting
3. **Middle Office / IBOR** — Investment book of record, position management, P&L

---

## Phase 1: Seed Data

### 1.1 Products

| Product Code | Name | Category | Description |
|--------------|------|----------|-------------|
| `GLOB_CUSTODY` | Global Custody | Custody | Institutional asset safekeeping and servicing |
| `FUND_ACCT` | Fund Accounting | Fund Services | NAV calculation and fund administration |
| `MO_IBOR` | Middle Office IBOR | Middle Office | Investment book of record and position management |

### 1.2 Services per Product

**Global Custody (`GLOB_CUSTODY`):**
| Service Code | Name | Category | Mandatory |
|--------------|------|----------|-----------|
| `SAFEKEEPING` | Asset Safekeeping | Custody | Yes |
| `SETTLEMENT` | Trade Settlement | Settlement | Yes |
| `CORP_ACTIONS` | Corporate Actions | Operations | Yes |
| `INCOME_COLLECT` | Income Collection | Operations | No |
| `PROXY_VOTING` | Proxy Voting | Governance | No |
| `FX_EXECUTION` | FX Execution | Trading | No |

**Fund Accounting (`FUND_ACCT`):**
| Service Code | Name | Category | Mandatory |
|--------------|------|----------|-----------|
| `NAV_CALC` | NAV Calculation | Valuation | Yes |
| `INVESTOR_ACCT` | Investor Accounting | Accounting | Yes |
| `FUND_REPORTING` | Fund Reporting | Reporting | Yes |
| `EXPENSE_MGMT` | Expense Management | Accounting | No |
| `PERF_MEASURE` | Performance Measurement | Analytics | No |

**Middle Office IBOR (`MO_IBOR`):**
| Service Code | Name | Category | Mandatory |
|--------------|------|----------|-----------|
| `POSITION_MGMT` | Position Management | IBOR | Yes |
| `TRADE_CAPTURE` | Trade Capture | IBOR | Yes |
| `PNL_ATTRIB` | P&L Attribution | Analytics | Yes |
| `CASH_MGMT` | Cash Management | Treasury | No |
| `COLLATERAL_MGMT` | Collateral Management | Operations | No |

### 1.3 Resource Types (Lifecycle Resources)

| Resource Code | Name | Resource Type | Vendor | Services |
|---------------|------|---------------|--------|----------|
| `CUSTODY_ACCT` | Custody Account | account | Internal | SAFEKEEPING |
| `SETTLE_ACCT` | Settlement Account | account | DTCC/Euroclear | SETTLEMENT |
| `SWIFT_CONN` | SWIFT Connection | connection | SWIFT | SETTLEMENT, INCOME_COLLECT |
| `CA_PLATFORM` | Corporate Actions Platform | platform | Internal | CORP_ACTIONS |
| `NAV_ENGINE` | NAV Calculation Engine | application | Internal | NAV_CALC |
| `INVESTOR_LEDGER` | Investor Ledger | application | Internal | INVESTOR_ACCT |
| `IBOR_SYSTEM` | IBOR System | application | Internal | POSITION_MGMT, TRADE_CAPTURE |
| `PNL_ENGINE` | P&L Engine | application | Internal | PNL_ATTRIB |
| `REPORTING_HUB` | Reporting Hub | platform | Internal | FUND_REPORTING |

### 1.4 Resource Type Attributes

**Custody Account (`CUSTODY_ACCT`):**
| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `account_number` | string | Yes | Custody account number |
| `account_name` | string | Yes | Account name |
| `base_currency` | string | Yes | Base currency (ISO) |
| `account_type` | string | Yes | OMNIBUS, SEGREGATED |
| `sub_custodian` | string | No | Sub-custodian name |
| `market_codes` | json | No | Enabled market codes |

**Settlement Account (`SETTLE_ACCT`):**
| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `account_number` | string | Yes | Settlement account number |
| `bic_code` | string | Yes | BIC/SWIFT code |
| `settlement_currency` | string | Yes | Settlement currency |
| `csd_participant_id` | string | No | CSD participant ID |
| `netting_enabled` | boolean | No | Netting enabled flag |

**SWIFT Connection (`SWIFT_CONN`):**
| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `bic_code` | string | Yes | SWIFT BIC |
| `logical_terminal` | string | Yes | Logical terminal ID |
| `message_types` | json | Yes | Enabled MT types [MT54x, MT9xx] |
| `rma_status` | string | No | RMA authorization status |

**NAV Engine (`NAV_ENGINE`):**
| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `fund_code` | string | Yes | Fund identifier |
| `valuation_frequency` | string | Yes | DAILY, WEEKLY, MONTHLY |
| `pricing_source` | string | Yes | Primary pricing source |
| `nav_cutoff_time` | string | Yes | NAV cutoff time (HH:MM TZ) |
| `share_classes` | json | No | Share class configuration |

**IBOR System (`IBOR_SYSTEM`):**
| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `portfolio_code` | string | Yes | Portfolio identifier |
| `accounting_basis` | string | Yes | TRADE_DATE, SETTLEMENT_DATE |
| `base_currency` | string | Yes | Reporting currency |
| `position_source` | string | Yes | Position source system |
| `reconciliation_enabled` | boolean | No | Auto-recon enabled |

---

## Phase 2: Migration Files

### File: `sql/migrations/029_seed_custody_products.sql`

```sql
-- ============================================
-- Seed Custody Bank Products, Services, Resources
-- ============================================

BEGIN;

-- =============================================================================
-- 1. PRODUCTS
-- =============================================================================

INSERT INTO "ob-poc".products (product_id, name, product_code, product_category, description, is_active)
VALUES 
    (gen_random_uuid(), 'Global Custody', 'GLOB_CUSTODY', 'Custody', 
     'Institutional asset safekeeping, settlement, and servicing', true),
    (gen_random_uuid(), 'Fund Accounting', 'FUND_ACCT', 'Fund Services', 
     'NAV calculation, investor accounting, and fund administration', true),
    (gen_random_uuid(), 'Middle Office IBOR', 'MO_IBOR', 'Middle Office', 
     'Investment book of record, position management, and P&L attribution', true)
ON CONFLICT (name) DO UPDATE SET
    product_code = EXCLUDED.product_code,
    product_category = EXCLUDED.product_category,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- =============================================================================
-- 2. SERVICES
-- =============================================================================

INSERT INTO "ob-poc".services (service_id, name, service_code, service_category, description, is_active)
VALUES 
    -- Custody Services
    (gen_random_uuid(), 'Asset Safekeeping', 'SAFEKEEPING', 'Custody', 
     'Secure custody of financial assets', true),
    (gen_random_uuid(), 'Trade Settlement', 'SETTLEMENT', 'Settlement', 
     'Multi-market trade settlement', true),
    (gen_random_uuid(), 'Corporate Actions', 'CORP_ACTIONS', 'Operations', 
     'Corporate action processing and elections', true),
    (gen_random_uuid(), 'Income Collection', 'INCOME_COLLECT', 'Operations', 
     'Dividend and interest collection', true),
    (gen_random_uuid(), 'Proxy Voting', 'PROXY_VOTING', 'Governance', 
     'Proxy voting and shareholder services', true),
    (gen_random_uuid(), 'FX Execution', 'FX_EXECUTION', 'Trading', 
     'Foreign exchange execution services', true),
    
    -- Fund Accounting Services
    (gen_random_uuid(), 'NAV Calculation', 'NAV_CALC', 'Valuation', 
     'Daily/periodic NAV calculation', true),
    (gen_random_uuid(), 'Investor Accounting', 'INVESTOR_ACCT', 'Accounting', 
     'Shareholder servicing and transfer agency', true),
    (gen_random_uuid(), 'Fund Reporting', 'FUND_REPORTING', 'Reporting', 
     'Regulatory and investor reporting', true),
    (gen_random_uuid(), 'Expense Management', 'EXPENSE_MGMT', 'Accounting', 
     'Fund expense accrual and payment', true),
    (gen_random_uuid(), 'Performance Measurement', 'PERF_MEASURE', 'Analytics', 
     'Performance calculation and attribution', true),
    
    -- Middle Office Services
    (gen_random_uuid(), 'Position Management', 'POSITION_MGMT', 'IBOR', 
     'Real-time position tracking', true),
    (gen_random_uuid(), 'Trade Capture', 'TRADE_CAPTURE', 'IBOR', 
     'Trade booking and lifecycle management', true),
    (gen_random_uuid(), 'P&L Attribution', 'PNL_ATTRIB', 'Analytics', 
     'P&L calculation and attribution analysis', true),
    (gen_random_uuid(), 'Cash Management', 'CASH_MGMT', 'Treasury', 
     'Cash forecasting and management', true),
    (gen_random_uuid(), 'Collateral Management', 'COLLATERAL_MGMT', 'Operations', 
     'Collateral optimization and margin management', true)
ON CONFLICT (name) DO UPDATE SET
    service_code = EXCLUDED.service_code,
    service_category = EXCLUDED.service_category,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- =============================================================================
-- 3. PRODUCT-SERVICE MAPPINGS
-- =============================================================================

-- Global Custody Services
WITH p AS (SELECT product_id FROM "ob-poc".products WHERE product_code = 'GLOB_CUSTODY')
INSERT INTO "ob-poc".product_services (product_id, service_id, is_mandatory, is_default, display_order)
SELECT p.product_id, s.service_id, 
       s.service_code IN ('SAFEKEEPING', 'SETTLEMENT', 'CORP_ACTIONS'),
       true,
       CASE s.service_code
           WHEN 'SAFEKEEPING' THEN 1
           WHEN 'SETTLEMENT' THEN 2
           WHEN 'CORP_ACTIONS' THEN 3
           WHEN 'INCOME_COLLECT' THEN 4
           WHEN 'PROXY_VOTING' THEN 5
           WHEN 'FX_EXECUTION' THEN 6
       END
FROM p, "ob-poc".services s
WHERE s.service_code IN ('SAFEKEEPING', 'SETTLEMENT', 'CORP_ACTIONS', 'INCOME_COLLECT', 'PROXY_VOTING', 'FX_EXECUTION')
ON CONFLICT (product_id, service_id) DO UPDATE SET
    is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- Fund Accounting Services
WITH p AS (SELECT product_id FROM "ob-poc".products WHERE product_code = 'FUND_ACCT')
INSERT INTO "ob-poc".product_services (product_id, service_id, is_mandatory, is_default, display_order)
SELECT p.product_id, s.service_id,
       s.service_code IN ('NAV_CALC', 'INVESTOR_ACCT', 'FUND_REPORTING'),
       true,
       CASE s.service_code
           WHEN 'NAV_CALC' THEN 1
           WHEN 'INVESTOR_ACCT' THEN 2
           WHEN 'FUND_REPORTING' THEN 3
           WHEN 'EXPENSE_MGMT' THEN 4
           WHEN 'PERF_MEASURE' THEN 5
       END
FROM p, "ob-poc".services s
WHERE s.service_code IN ('NAV_CALC', 'INVESTOR_ACCT', 'FUND_REPORTING', 'EXPENSE_MGMT', 'PERF_MEASURE')
ON CONFLICT (product_id, service_id) DO UPDATE SET
    is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- Middle Office IBOR Services
WITH p AS (SELECT product_id FROM "ob-poc".products WHERE product_code = 'MO_IBOR')
INSERT INTO "ob-poc".product_services (product_id, service_id, is_mandatory, is_default, display_order)
SELECT p.product_id, s.service_id,
       s.service_code IN ('POSITION_MGMT', 'TRADE_CAPTURE', 'PNL_ATTRIB'),
       true,
       CASE s.service_code
           WHEN 'POSITION_MGMT' THEN 1
           WHEN 'TRADE_CAPTURE' THEN 2
           WHEN 'PNL_ATTRIB' THEN 3
           WHEN 'CASH_MGMT' THEN 4
           WHEN 'COLLATERAL_MGMT' THEN 5
       END
FROM p, "ob-poc".services s
WHERE s.service_code IN ('POSITION_MGMT', 'TRADE_CAPTURE', 'PNL_ATTRIB', 'CASH_MGMT', 'COLLATERAL_MGMT')
ON CONFLICT (product_id, service_id) DO UPDATE SET
    is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- =============================================================================
-- 4. RESOURCE TYPES (Lifecycle Resources)
-- =============================================================================

INSERT INTO "ob-poc".lifecycle_resources (resource_id, name, owner, resource_code, resource_type, vendor, is_active, capabilities)
VALUES 
    (gen_random_uuid(), 'Custody Account', 'Operations', 'CUSTODY_ACCT', 'account', 'Internal',
     true, '{"markets": ["US", "EU", "APAC"], "asset_classes": ["equity", "fixed_income", "alternatives"]}'::jsonb),
    
    (gen_random_uuid(), 'Settlement Account', 'Operations', 'SETTLE_ACCT', 'account', 'Multi-CSD',
     true, '{"csds": ["DTCC", "Euroclear", "Clearstream"], "settlement_types": ["DVP", "FOP", "RVP"]}'::jsonb),
    
    (gen_random_uuid(), 'SWIFT Connection', 'Technology', 'SWIFT_CONN', 'connection', 'SWIFT',
     true, '{"message_categories": ["MT1xx", "MT2xx", "MT5xx", "MT9xx"], "protocols": ["FIN", "InterAct"]}'::jsonb),
    
    (gen_random_uuid(), 'Corporate Actions Platform', 'Operations', 'CA_PLATFORM', 'platform', 'Internal',
     true, '{"event_types": ["dividend", "rights", "merger", "tender"], "markets": ["global"]}'::jsonb),
    
    (gen_random_uuid(), 'NAV Calculation Engine', 'Fund Services', 'NAV_ENGINE', 'application', 'Internal',
     true, '{"frequencies": ["daily", "weekly", "monthly"], "pricing_sources": ["Bloomberg", "Reuters", "ICE"]}'::jsonb),
    
    (gen_random_uuid(), 'Investor Ledger', 'Fund Services', 'INVESTOR_LEDGER', 'application', 'Internal',
     true, '{"transaction_types": ["subscription", "redemption", "transfer", "switch"]}'::jsonb),
    
    (gen_random_uuid(), 'IBOR System', 'Middle Office', 'IBOR_SYSTEM', 'application', 'Internal',
     true, '{"accounting_bases": ["trade_date", "settlement_date"], "asset_classes": ["all"]}'::jsonb),
    
    (gen_random_uuid(), 'P&L Engine', 'Middle Office', 'PNL_ENGINE', 'application', 'Internal',
     true, '{"attribution_models": ["brinson", "factor", "transaction"], "frequencies": ["daily", "mtd", "ytd"]}'::jsonb),
    
    (gen_random_uuid(), 'Reporting Hub', 'Technology', 'REPORTING_HUB', 'platform', 'Internal',
     true, '{"formats": ["PDF", "Excel", "XML", "JSON"], "delivery": ["email", "sftp", "api"]}'::jsonb)
ON CONFLICT (name) DO UPDATE SET
    resource_code = EXCLUDED.resource_code,
    resource_type = EXCLUDED.resource_type,
    vendor = EXCLUDED.vendor,
    capabilities = EXCLUDED.capabilities,
    is_active = EXCLUDED.is_active;

-- =============================================================================
-- 5. SERVICE-RESOURCE CAPABILITIES
-- =============================================================================

-- Link resources to services
INSERT INTO "ob-poc".service_resource_capabilities (service_id, resource_id, supported_options, priority, is_active)
SELECT s.service_id, r.resource_id, '{}'::jsonb, 100, true
FROM "ob-poc".services s, "ob-poc".lifecycle_resources r
WHERE 
    (s.service_code = 'SAFEKEEPING' AND r.resource_code = 'CUSTODY_ACCT') OR
    (s.service_code = 'SETTLEMENT' AND r.resource_code IN ('SETTLE_ACCT', 'SWIFT_CONN')) OR
    (s.service_code = 'CORP_ACTIONS' AND r.resource_code = 'CA_PLATFORM') OR
    (s.service_code = 'INCOME_COLLECT' AND r.resource_code = 'SWIFT_CONN') OR
    (s.service_code = 'NAV_CALC' AND r.resource_code = 'NAV_ENGINE') OR
    (s.service_code = 'INVESTOR_ACCT' AND r.resource_code = 'INVESTOR_LEDGER') OR
    (s.service_code = 'FUND_REPORTING' AND r.resource_code = 'REPORTING_HUB') OR
    (s.service_code = 'POSITION_MGMT' AND r.resource_code = 'IBOR_SYSTEM') OR
    (s.service_code = 'TRADE_CAPTURE' AND r.resource_code = 'IBOR_SYSTEM') OR
    (s.service_code = 'PNL_ATTRIB' AND r.resource_code = 'PNL_ENGINE')
ON CONFLICT (service_id, resource_id) DO NOTHING;

COMMIT;

-- Verification
SELECT 'Products' as entity, COUNT(*) as count FROM "ob-poc".products WHERE product_code IN ('GLOB_CUSTODY', 'FUND_ACCT', 'MO_IBOR')
UNION ALL SELECT 'Services', COUNT(*) FROM "ob-poc".services WHERE is_active = true
UNION ALL SELECT 'Product-Service Links', COUNT(*) FROM "ob-poc".product_services
UNION ALL SELECT 'Resource Types', COUNT(*) FROM "ob-poc".lifecycle_resources WHERE is_active = true
UNION ALL SELECT 'Service-Resource Links', COUNT(*) FROM "ob-poc".service_resource_capabilities;
```

### File: `sql/migrations/030_seed_resource_attributes.sql`

```sql
-- ============================================
-- Seed Resource Type Attribute Requirements
-- ============================================

BEGIN;

-- =============================================================================
-- 1. DICTIONARY ENTRIES for Resource Attributes
-- =============================================================================

INSERT INTO "ob-poc".dictionary (attribute_id, attribute_name, display_name, value_type, category)
VALUES
    -- Account attributes
    (gen_random_uuid(), 'account_number', 'Account Number', 'string', 'resource.account'),
    (gen_random_uuid(), 'account_name', 'Account Name', 'string', 'resource.account'),
    (gen_random_uuid(), 'base_currency', 'Base Currency', 'string', 'resource.account'),
    (gen_random_uuid(), 'account_type', 'Account Type', 'string', 'resource.account'),
    (gen_random_uuid(), 'sub_custodian', 'Sub-Custodian', 'string', 'resource.account'),
    (gen_random_uuid(), 'market_codes', 'Market Codes', 'json', 'resource.account'),
    
    -- Settlement attributes
    (gen_random_uuid(), 'bic_code', 'BIC/SWIFT Code', 'string', 'resource.settlement'),
    (gen_random_uuid(), 'settlement_currency', 'Settlement Currency', 'string', 'resource.settlement'),
    (gen_random_uuid(), 'csd_participant_id', 'CSD Participant ID', 'string', 'resource.settlement'),
    (gen_random_uuid(), 'netting_enabled', 'Netting Enabled', 'boolean', 'resource.settlement'),
    
    -- SWIFT attributes
    (gen_random_uuid(), 'logical_terminal', 'Logical Terminal', 'string', 'resource.swift'),
    (gen_random_uuid(), 'message_types', 'Message Types', 'json', 'resource.swift'),
    (gen_random_uuid(), 'rma_status', 'RMA Status', 'string', 'resource.swift'),
    
    -- Fund attributes
    (gen_random_uuid(), 'fund_code', 'Fund Code', 'string', 'resource.fund'),
    (gen_random_uuid(), 'valuation_frequency', 'Valuation Frequency', 'string', 'resource.fund'),
    (gen_random_uuid(), 'pricing_source', 'Pricing Source', 'string', 'resource.fund'),
    (gen_random_uuid(), 'nav_cutoff_time', 'NAV Cutoff Time', 'string', 'resource.fund'),
    (gen_random_uuid(), 'share_classes', 'Share Classes', 'json', 'resource.fund'),
    
    -- IBOR attributes
    (gen_random_uuid(), 'portfolio_code', 'Portfolio Code', 'string', 'resource.ibor'),
    (gen_random_uuid(), 'accounting_basis', 'Accounting Basis', 'string', 'resource.ibor'),
    (gen_random_uuid(), 'position_source', 'Position Source', 'string', 'resource.ibor'),
    (gen_random_uuid(), 'reconciliation_enabled', 'Reconciliation Enabled', 'boolean', 'resource.ibor')
ON CONFLICT (attribute_name) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    value_type = EXCLUDED.value_type,
    category = EXCLUDED.category;

-- =============================================================================
-- 2. RESOURCE ATTRIBUTE REQUIREMENTS
-- =============================================================================

-- Custody Account attributes
WITH r AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'CUSTODY_ACCT'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('account_number', 'account_name', 'base_currency', 'account_type', 'sub_custodian', 'market_codes'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.attribute_name IN ('account_number', 'account_name', 'base_currency', 'account_type'),
       CASE attrs.attribute_name
           WHEN 'account_number' THEN 1
           WHEN 'account_name' THEN 2
           WHEN 'base_currency' THEN 3
           WHEN 'account_type' THEN 4
           WHEN 'sub_custodian' THEN 5
           WHEN 'market_codes' THEN 6
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- Settlement Account attributes
WITH r AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'SETTLE_ACCT'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('account_number', 'bic_code', 'settlement_currency', 'csd_participant_id', 'netting_enabled'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.attribute_name IN ('account_number', 'bic_code', 'settlement_currency'),
       CASE attrs.attribute_name
           WHEN 'account_number' THEN 1
           WHEN 'bic_code' THEN 2
           WHEN 'settlement_currency' THEN 3
           WHEN 'csd_participant_id' THEN 4
           WHEN 'netting_enabled' THEN 5
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- SWIFT Connection attributes
WITH r AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'SWIFT_CONN'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('bic_code', 'logical_terminal', 'message_types', 'rma_status'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.attribute_name IN ('bic_code', 'logical_terminal', 'message_types'),
       CASE attrs.attribute_name
           WHEN 'bic_code' THEN 1
           WHEN 'logical_terminal' THEN 2
           WHEN 'message_types' THEN 3
           WHEN 'rma_status' THEN 4
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- NAV Engine attributes
WITH r AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'NAV_ENGINE'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('fund_code', 'valuation_frequency', 'pricing_source', 'nav_cutoff_time', 'share_classes'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.attribute_name IN ('fund_code', 'valuation_frequency', 'pricing_source', 'nav_cutoff_time'),
       CASE attrs.attribute_name
           WHEN 'fund_code' THEN 1
           WHEN 'valuation_frequency' THEN 2
           WHEN 'pricing_source' THEN 3
           WHEN 'nav_cutoff_time' THEN 4
           WHEN 'share_classes' THEN 5
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- IBOR System attributes
WITH r AS (SELECT resource_id FROM "ob-poc".lifecycle_resources WHERE resource_code = 'IBOR_SYSTEM'),
     attrs AS (SELECT attribute_id, attribute_name FROM "ob-poc".dictionary 
               WHERE attribute_name IN ('portfolio_code', 'accounting_basis', 'base_currency', 'position_source', 'reconciliation_enabled'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.attribute_name IN ('portfolio_code', 'accounting_basis', 'base_currency', 'position_source'),
       CASE attrs.attribute_name
           WHEN 'portfolio_code' THEN 1
           WHEN 'accounting_basis' THEN 2
           WHEN 'base_currency' THEN 3
           WHEN 'position_source' THEN 4
           WHEN 'reconciliation_enabled' THEN 5
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

COMMIT;

-- Verification
SELECT lr.resource_code, COUNT(rar.attribute_id) as attr_count,
       COUNT(*) FILTER (WHERE rar.is_mandatory) as required_count
FROM "ob-poc".lifecycle_resources lr
LEFT JOIN "ob-poc".resource_attribute_requirements rar ON lr.resource_id = rar.resource_id
WHERE lr.resource_code IN ('CUSTODY_ACCT', 'SETTLE_ACCT', 'SWIFT_CONN', 'NAV_ENGINE', 'IBOR_SYSTEM')
GROUP BY lr.resource_code
ORDER BY lr.resource_code;
```

---

## Phase 3: Test Harness

### File: `rust/tests/onboarding_scenarios.rs`

```rust
//! Onboarding Scenario Test Harness
//!
//! Tests full onboarding flows for custody bank products:
//! 1. Global Custody onboarding
//! 2. Fund Accounting onboarding  
//! 3. Middle Office IBOR onboarding

use ob_poc::database::{
    CbuService, ResourceInstanceService, ProductService, ServiceService,
};
use ob_poc::dsl_v2::{parse_program, compile_to_plan, DslExecutor};
use sqlx::PgPool;
use uuid::Uuid;

/// Test Scenario 1: Global Custody Onboarding
/// 
/// Client: Apex Capital Partners (Hedge Fund)
/// Product: Global Custody
/// Services: Safekeeping, Settlement, Corporate Actions
/// Resources: Custody Account, Settlement Account, SWIFT Connection
#[tokio::test]
async fn test_global_custody_onboarding() {
    let pool = get_test_pool().await;
    
    let dsl = r#"
        ;; Create the CBU
        (cbu.ensure 
            :name "Apex Capital Partners" 
            :jurisdiction "US" 
            :client-type "HEDGE_FUND"
            :as @apex)
        
        ;; Create Custody Account
        (resource.create 
            :cbu-id @apex 
            :resource-type "CUSTODY_ACCT"
            :instance-url "https://custody.bank.com/accounts/APEX-001"
            :instance-id "APEX-CUSTODY-001"
            :instance-name "Apex Capital Custody Account"
            :as @custody-acct)
        
        ;; Set custody account attributes
        (resource.set-attr :instance-id @custody-acct :attr "account_number" :value "CUST-2024-APEX-001")
        (resource.set-attr :instance-id @custody-acct :attr "account_name" :value "Apex Capital Partners - Main")
        (resource.set-attr :instance-id @custody-acct :attr "base_currency" :value "USD")
        (resource.set-attr :instance-id @custody-acct :attr "account_type" :value "SEGREGATED")
        
        ;; Activate custody account
        (resource.activate :instance-id @custody-acct)
        
        ;; Create Settlement Account (DTCC)
        (resource.create 
            :cbu-id @apex 
            :resource-type "SETTLE_ACCT"
            :instance-url "https://dtcc.com/participants/APEX-SETTLE"
            :instance-id "APEX-SETTLE-001"
            :as @settle-acct)
        
        (resource.set-attr :instance-id @settle-acct :attr "account_number" :value "DTC-APEX-789")
        (resource.set-attr :instance-id @settle-acct :attr "bic_code" :value "DTCYUS33")
        (resource.set-attr :instance-id @settle-acct :attr "settlement_currency" :value "USD")
        (resource.activate :instance-id @settle-acct)
        
        ;; Create SWIFT Connection
        (resource.create 
            :cbu-id @apex 
            :resource-type "SWIFT_CONN"
            :instance-url "https://swift.com/connections/APEXUS33"
            :instance-id "APEXUS33"
            :as @swift)
        
        (resource.set-attr :instance-id @swift :attr "bic_code" :value "APEXUS33XXX")
        (resource.set-attr :instance-id @swift :attr "logical_terminal" :value "APEXUS33AXXX")
        (resource.set-attr :instance-id @swift :attr "message_types" :value "[\"MT540\", \"MT541\", \"MT542\", \"MT543\", \"MT950\"]")
        (resource.activate :instance-id @swift)
        
        ;; Record service deliveries
        (delivery.record :cbu-id @apex :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody-acct)
        (delivery.record :cbu-id @apex :product "GLOB_CUSTODY" :service "SETTLEMENT" :instance-id @settle-acct)
        (delivery.complete :cbu-id @apex :product "GLOB_CUSTODY" :service "SAFEKEEPING")
        (delivery.complete :cbu-id @apex :product "GLOB_CUSTODY" :service "SETTLEMENT")
    "#;
    
    let result = execute_dsl(&pool, dsl).await;
    assert!(result.is_ok(), "Global Custody onboarding failed: {:?}", result.err());
    
    // Verify resources created
    let svc = ResourceInstanceService::new(pool.clone());
    let instances = svc.list_instances_for_cbu(get_cbu_id(&pool, "Apex Capital Partners").await).await.unwrap();
    assert_eq!(instances.len(), 3, "Expected 3 resource instances");
    assert!(instances.iter().all(|i| i.status == "ACTIVE"), "All instances should be ACTIVE");
}

/// Test Scenario 2: Fund Accounting Onboarding
///
/// Client: Pacific Growth Fund
/// Product: Fund Accounting
/// Services: NAV Calculation, Investor Accounting, Fund Reporting
/// Resources: NAV Engine, Investor Ledger, Reporting Hub
#[tokio::test]
async fn test_fund_accounting_onboarding() {
    let pool = get_test_pool().await;
    
    let dsl = r#"
        ;; Create the CBU
        (cbu.ensure 
            :name "Pacific Growth Fund" 
            :jurisdiction "LU" 
            :client-type "UCITS_FUND"
            :as @pgf)
        
        ;; Create NAV Engine instance
        (resource.create 
            :cbu-id @pgf 
            :resource-type "NAV_ENGINE"
            :instance-url "https://nav.fundservices.com/funds/PGF-001"
            :instance-id "PGF-NAV-001"
            :instance-name "Pacific Growth Fund NAV"
            :as @nav)
        
        (resource.set-attr :instance-id @nav :attr "fund_code" :value "PGF-LU-001")
        (resource.set-attr :instance-id @nav :attr "valuation_frequency" :value "DAILY")
        (resource.set-attr :instance-id @nav :attr "pricing_source" :value "Bloomberg")
        (resource.set-attr :instance-id @nav :attr "nav_cutoff_time" :value "16:00 CET")
        (resource.activate :instance-id @nav)
        
        ;; Create Investor Ledger instance
        (resource.create 
            :cbu-id @pgf 
            :resource-type "INVESTOR_LEDGER"
            :instance-url "https://ta.fundservices.com/funds/PGF-001"
            :instance-id "PGF-TA-001"
            :as @ledger)
        
        (resource.activate :instance-id @ledger)
        
        ;; Record deliveries
        (delivery.record :cbu-id @pgf :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
        (delivery.record :cbu-id @pgf :product "FUND_ACCT" :service "INVESTOR_ACCT" :instance-id @ledger)
        (delivery.complete :cbu-id @pgf :product "FUND_ACCT" :service "NAV_CALC")
        (delivery.complete :cbu-id @pgf :product "FUND_ACCT" :service "INVESTOR_ACCT")
    "#;
    
    let result = execute_dsl(&pool, dsl).await;
    assert!(result.is_ok(), "Fund Accounting onboarding failed: {:?}", result.err());
}

/// Test Scenario 3: Middle Office IBOR Onboarding
///
/// Client: Quantum Asset Management
/// Product: Middle Office IBOR
/// Services: Position Management, Trade Capture, P&L Attribution
/// Resources: IBOR System, P&L Engine
#[tokio::test]
async fn test_ibor_onboarding() {
    let pool = get_test_pool().await;
    
    let dsl = r#"
        ;; Create the CBU
        (cbu.ensure 
            :name "Quantum Asset Management" 
            :jurisdiction "UK" 
            :client-type "ASSET_MANAGER"
            :as @quantum)
        
        ;; Create IBOR System instance
        (resource.create 
            :cbu-id @quantum 
            :resource-type "IBOR_SYSTEM"
            :instance-url "https://ibor.platform.com/portfolios/QAM-001"
            :instance-id "QAM-IBOR-001"
            :instance-name "Quantum IBOR"
            :as @ibor)
        
        (resource.set-attr :instance-id @ibor :attr "portfolio_code" :value "QAM-MASTER")
        (resource.set-attr :instance-id @ibor :attr "accounting_basis" :value "TRADE_DATE")
        (resource.set-attr :instance-id @ibor :attr "base_currency" :value "GBP")
        (resource.set-attr :instance-id @ibor :attr "position_source" :value "OMS")
        (resource.activate :instance-id @ibor)
        
        ;; Create P&L Engine instance
        (resource.create 
            :cbu-id @quantum 
            :resource-type "PNL_ENGINE"
            :instance-url "https://pnl.platform.com/portfolios/QAM-001"
            :instance-id "QAM-PNL-001"
            :as @pnl)
        
        (resource.activate :instance-id @pnl)
        
        ;; Record deliveries
        (delivery.record :cbu-id @quantum :product "MO_IBOR" :service "POSITION_MGMT" :instance-id @ibor)
        (delivery.record :cbu-id @quantum :product "MO_IBOR" :service "TRADE_CAPTURE" :instance-id @ibor)
        (delivery.record :cbu-id @quantum :product "MO_IBOR" :service "PNL_ATTRIB" :instance-id @pnl)
        (delivery.complete :cbu-id @quantum :product "MO_IBOR" :service "POSITION_MGMT")
        (delivery.complete :cbu-id @quantum :product "MO_IBOR" :service "TRADE_CAPTURE")
        (delivery.complete :cbu-id @quantum :product "MO_IBOR" :service "PNL_ATTRIB")
    "#;
    
    let result = execute_dsl(&pool, dsl).await;
    assert!(result.is_ok(), "IBOR onboarding failed: {:?}", result.err());
}

/// Test Scenario 4: Multi-Product Onboarding
///
/// Client: Atlas Institutional Investors
/// Products: Global Custody + Fund Accounting
#[tokio::test]
async fn test_multi_product_onboarding() {
    let pool = get_test_pool().await;
    
    let dsl = r#"
        ;; Create the CBU
        (cbu.ensure 
            :name "Atlas Institutional Investors" 
            :jurisdiction "US" 
            :client-type "PENSION_FUND"
            :as @atlas)
        
        ;; === GLOBAL CUSTODY ===
        (resource.create 
            :cbu-id @atlas 
            :resource-type "CUSTODY_ACCT"
            :instance-url "https://custody.bank.com/accounts/ATLAS-001"
            :instance-id "ATLAS-CUSTODY-001"
            :as @custody)
        
        (resource.set-attr :instance-id @custody :attr "account_number" :value "CUST-ATLAS-001")
        (resource.set-attr :instance-id @custody :attr "account_name" :value "Atlas Pension - Main Custody")
        (resource.set-attr :instance-id @custody :attr "base_currency" :value "USD")
        (resource.set-attr :instance-id @custody :attr "account_type" :value "OMNIBUS")
        (resource.activate :instance-id @custody)
        
        ;; === FUND ACCOUNTING ===
        (resource.create 
            :cbu-id @atlas 
            :resource-type "NAV_ENGINE"
            :instance-url "https://nav.fundservices.com/funds/ATLAS-001"
            :instance-id "ATLAS-NAV-001"
            :as @nav)
        
        (resource.set-attr :instance-id @nav :attr "fund_code" :value "ATLAS-PEN-001")
        (resource.set-attr :instance-id @nav :attr "valuation_frequency" :value "DAILY")
        (resource.set-attr :instance-id @nav :attr "pricing_source" :value "Reuters")
        (resource.set-attr :instance-id @nav :attr "nav_cutoff_time" :value "17:00 EST")
        (resource.activate :instance-id @nav)
        
        ;; Record all deliveries
        (delivery.record :cbu-id @atlas :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
        (delivery.record :cbu-id @atlas :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
        (delivery.complete :cbu-id @atlas :product "GLOB_CUSTODY" :service "SAFEKEEPING")
        (delivery.complete :cbu-id @atlas :product "FUND_ACCT" :service "NAV_CALC")
    "#;
    
    let result = execute_dsl(&pool, dsl).await;
    assert!(result.is_ok(), "Multi-product onboarding failed: {:?}", result.err());
    
    // Verify deliveries
    let svc = ResourceInstanceService::new(pool.clone());
    let cbu_id = get_cbu_id(&pool, "Atlas Institutional Investors").await;
    let deliveries = svc.get_cbu_deliveries(cbu_id).await.unwrap();
    assert_eq!(deliveries.len(), 2, "Expected 2 service deliveries");
    assert!(deliveries.iter().all(|d| d.delivery_status == "DELIVERED"));
}

// Helper functions
async fn get_test_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    PgPool::connect(&url).await.expect("Failed to connect to database")
}

async fn execute_dsl(pool: &PgPool, dsl: &str) -> anyhow::Result<()> {
    let ast = parse_program(dsl)?;
    let plan = compile_to_plan(&ast)?;
    let executor = DslExecutor::new(pool.clone());
    executor.execute(&plan).await?;
    Ok(())
}

async fn get_cbu_id(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("CBU not found")
}
```

### File: `rust/tests/scenarios/onboarding/` (DSL files)

Create directory structure:
```
rust/tests/scenarios/onboarding/
├── 01_global_custody.dsl
├── 02_fund_accounting.dsl
├── 03_ibor.dsl
└── 04_multi_product.dsl
```

---

## Phase 4: Agent Integration

### Update Agent System Prompt

Add to agent prompt (in `rust/src/services/agent_service.rs` or equivalent):

```markdown
## Onboarding DSL Generation

When generating DSL for client onboarding requests, follow this pattern:

### Available Products
- `GLOB_CUSTODY` - Global Custody (safekeeping, settlement, corporate actions)
- `FUND_ACCT` - Fund Accounting (NAV calculation, investor accounting)
- `MO_IBOR` - Middle Office IBOR (position management, trade capture, P&L)

### Available Resource Types
- `CUSTODY_ACCT` - Custody Account (requires: account_number, account_name, base_currency, account_type)
- `SETTLE_ACCT` - Settlement Account (requires: account_number, bic_code, settlement_currency)
- `SWIFT_CONN` - SWIFT Connection (requires: bic_code, logical_terminal, message_types)
- `NAV_ENGINE` - NAV Calculation Engine (requires: fund_code, valuation_frequency, pricing_source, nav_cutoff_time)
- `IBOR_SYSTEM` - IBOR System (requires: portfolio_code, accounting_basis, base_currency, position_source)

### Onboarding Pattern
1. Create CBU with `cbu.ensure`
2. Create resource instances with `resource.create`
3. Set required attributes with `resource.set-attr`
4. Activate resources with `resource.activate`
5. Record deliveries with `delivery.record`
6. Complete deliveries with `delivery.complete`

### Example: Global Custody Onboarding
```clojure
(cbu.ensure :name "Client Name" :jurisdiction "US" :client-type "HEDGE_FUND" :as @client)
(resource.create :cbu-id @client :resource-type "CUSTODY_ACCT" :instance-url "https://..." :instance-id "..." :as @acct)
(resource.set-attr :instance-id @acct :attr "account_number" :value "...")
(resource.set-attr :instance-id @acct :attr "account_name" :value "...")
(resource.set-attr :instance-id @acct :attr "base_currency" :value "USD")
(resource.set-attr :instance-id @acct :attr "account_type" :value "SEGREGATED")
(resource.activate :instance-id @acct)
(delivery.record :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @acct)
(delivery.complete :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING")
```
```

### Add Onboarding Templates

Add to `rust/src/templates/registry.rs`:

```rust
Template {
    id: "global_custody_onboarding",
    name: "Global Custody Onboarding",
    description: "Full custody account setup with settlement and SWIFT",
    category: "onboarding",
    parameters: vec![
        TemplateParam { name: "client_name", param_type: "string", required: true },
        TemplateParam { name: "jurisdiction", param_type: "string", required: true },
        TemplateParam { name: "client_type", param_type: "string", required: true },
        TemplateParam { name: "base_currency", param_type: "string", required: true },
        TemplateParam { name: "account_type", param_type: "enum", required: true, 
                       options: Some(vec!["SEGREGATED", "OMNIBUS"]) },
    ],
    template: r#"
(cbu.ensure :name "{{client_name}}" :jurisdiction "{{jurisdiction}}" :client-type "{{client_type}}" :as @client)

(resource.create 
    :cbu-id @client 
    :resource-type "CUSTODY_ACCT"
    :instance-url "https://custody.bank.com/accounts/{{client_name | slugify}}"
    :instance-id "{{client_name | slugify | upper}}-CUSTODY-001"
    :instance-name "{{client_name}} Custody Account"
    :as @custody)

(resource.set-attr :instance-id @custody :attr "account_number" :value "CUST-{{timestamp}}")
(resource.set-attr :instance-id @custody :attr "account_name" :value "{{client_name}} - Main")
(resource.set-attr :instance-id @custody :attr "base_currency" :value "{{base_currency}}")
(resource.set-attr :instance-id @custody :attr "account_type" :value "{{account_type}}")
(resource.activate :instance-id @custody)

(delivery.record :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
(delivery.complete :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING")
    "#,
},

Template {
    id: "fund_accounting_onboarding",
    name: "Fund Accounting Onboarding",
    description: "NAV calculation and investor accounting setup",
    category: "onboarding",
    // ... similar structure
},

Template {
    id: "ibor_onboarding", 
    name: "Middle Office IBOR Onboarding",
    description: "IBOR system and P&L engine setup",
    category: "onboarding",
    // ... similar structure
},
```

---

## Execution Checklist

### Phase 1: Seed Data
- [ ] Run `029_seed_custody_products.sql`
- [ ] Run `030_seed_resource_attributes.sql`
- [ ] Verify: `SELECT * FROM "ob-poc".products WHERE product_code IN ('GLOB_CUSTODY', 'FUND_ACCT', 'MO_IBOR')`
- [ ] Verify: `SELECT COUNT(*) FROM "ob-poc".services` (should be 16+)
- [ ] Verify: `SELECT COUNT(*) FROM "ob-poc".lifecycle_resources` (should be 9+)

### Phase 2: Test Harness
- [ ] Create `rust/tests/onboarding_scenarios.rs`
- [ ] Create DSL test files in `rust/tests/scenarios/onboarding/`
- [ ] Run: `cargo test --features database onboarding`
- [ ] All 4 scenarios pass

### Phase 3: Agent Integration
- [ ] Update agent system prompt with onboarding vocabulary
- [ ] Add onboarding templates to registry
- [ ] Test: "Onboard Acme Fund for Global Custody" generates valid DSL

---

## Verification Queries

```sql
-- Full taxonomy view
SELECT 
    p.product_code,
    p.name as product_name,
    s.service_code,
    s.name as service_name,
    ps.is_mandatory,
    lr.resource_code,
    lr.name as resource_name
FROM "ob-poc".products p
JOIN "ob-poc".product_services ps ON p.product_id = ps.product_id
JOIN "ob-poc".services s ON ps.service_id = s.service_id
LEFT JOIN "ob-poc".service_resource_capabilities src ON s.service_id = src.service_id
LEFT JOIN "ob-poc".lifecycle_resources lr ON src.resource_id = lr.resource_id
WHERE p.product_code IN ('GLOB_CUSTODY', 'FUND_ACCT', 'MO_IBOR')
ORDER BY p.product_code, ps.display_order;

-- Resource attribute requirements
SELECT 
    lr.resource_code,
    d.attribute_name,
    d.value_type,
    rar.is_mandatory,
    rar.display_order
FROM "ob-poc".lifecycle_resources lr
JOIN "ob-poc".resource_attribute_requirements rar ON lr.resource_id = rar.resource_id
JOIN "ob-poc".dictionary d ON rar.attribute_id = d.attribute_id
ORDER BY lr.resource_code, rar.display_order;

-- Onboarding summary (after tests run)
SELECT 
    c.name as client,
    p.product_code,
    s.service_code,
    cri.instance_url,
    cri.status as instance_status,
    sdm.delivery_status
FROM "ob-poc".cbus c
JOIN "ob-poc".service_delivery_map sdm ON c.cbu_id = sdm.cbu_id
JOIN "ob-poc".products p ON sdm.product_id = p.product_id
JOIN "ob-poc".services s ON sdm.service_id = s.service_id
LEFT JOIN "ob-poc".cbu_resource_instances cri ON sdm.instance_id = cri.instance_id
ORDER BY c.name, p.product_code, s.service_code;
```
