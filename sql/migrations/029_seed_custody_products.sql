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
-- 4. RESOURCE TYPES (prod_resources table)
-- =============================================================================

INSERT INTO "ob-poc".prod_resources (resource_id, name, owner, resource_code, resource_type, vendor, is_active, capabilities)
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
FROM "ob-poc".services s, "ob-poc".prod_resources r
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
UNION ALL SELECT 'Resource Types', COUNT(*) FROM "ob-poc".prod_resources WHERE is_active = true
UNION ALL SELECT 'Service-Resource Links', COUNT(*) FROM "ob-poc".service_resource_capabilities;
