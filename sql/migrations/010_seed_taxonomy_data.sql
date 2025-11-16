-- ============================================
-- Seed Taxonomy Data
-- Version: 1.0.0
-- ============================================

BEGIN;

-- Insert products if they don't exist
INSERT INTO "ob-poc".products (name, product_code, product_category, description, is_active)
VALUES 
    ('Institutional Custody', 'CUSTODY_INST', 'custody', 'Full custody services for institutional clients', true),
    ('Prime Brokerage', 'PRIME_BROKER', 'prime_brokerage', 'Comprehensive prime brokerage services', true),
    ('Fund Administration', 'FUND_ADMIN', 'fund_admin', 'Complete fund administration services', true)
ON CONFLICT (name) DO UPDATE 
SET product_code = EXCLUDED.product_code,
    product_category = EXCLUDED.product_category,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- Insert services
INSERT INTO "ob-poc".services (name, service_code, service_category, description, is_active)
VALUES 
    ('Trade Settlement', 'SETTLEMENT', 'settlement', 'Multi-market trade settlement', true),
    ('Asset Safekeeping', 'SAFEKEEPING', 'custody', 'Secure asset custody', true),
    ('Corporate Actions', 'CORP_ACTIONS', 'operations', 'Corporate action processing', true),
    ('Client Reporting', 'REPORTING', 'reporting', 'Regulatory and client reporting', true)
ON CONFLICT (name) DO UPDATE 
SET service_code = EXCLUDED.service_code,
    service_category = EXCLUDED.service_category,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- Link Custody Product to Services
WITH p AS (SELECT product_id FROM "ob-poc".products WHERE product_code = 'CUSTODY_INST'),
     s AS (SELECT service_id, service_code FROM "ob-poc".services WHERE service_code IN ('SETTLEMENT', 'SAFEKEEPING'))
INSERT INTO "ob-poc".product_services (product_id, service_id, is_mandatory, display_order)
SELECT p.product_id, s.service_id, true,
       CASE s.service_code 
           WHEN 'SETTLEMENT' THEN 1
           WHEN 'SAFEKEEPING' THEN 2
       END
FROM p, s
ON CONFLICT (product_id, service_id) DO UPDATE
SET is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- Service Options for Settlement
WITH s AS (SELECT service_id FROM "ob-poc".services WHERE service_code = 'SETTLEMENT')
INSERT INTO "ob-poc".service_option_definitions (service_id, option_key, option_label, option_type, is_required, display_order)
SELECT service_id, 'markets', 'Settlement Markets', 'multi_select', true, 1 FROM s
UNION ALL
SELECT service_id, 'speed', 'Settlement Speed', 'single_select', true, 2 FROM s
UNION ALL
SELECT service_id, 'cutoff', 'Cut-off Time', 'single_select', false, 3 FROM s
ON CONFLICT (service_id, option_key) DO NOTHING;

-- Market Choices
WITH opt AS (
    SELECT sod.option_def_id 
    FROM "ob-poc".service_option_definitions sod
    JOIN "ob-poc".services s ON sod.service_id = s.service_id
    WHERE s.service_code = 'SETTLEMENT' AND sod.option_key = 'markets'
)
INSERT INTO "ob-poc".service_option_choices (option_def_id, choice_value, choice_label, display_order)
SELECT option_def_id, 'US_EQUITY', 'US Equities', 1 FROM opt
UNION ALL
SELECT option_def_id, 'EU_EQUITY', 'European Equities', 2 FROM opt
UNION ALL
SELECT option_def_id, 'APAC_EQUITY', 'APAC Equities', 3 FROM opt
UNION ALL
SELECT option_def_id, 'FIXED_INCOME', 'Fixed Income', 4 FROM opt
UNION ALL
SELECT option_def_id, 'DERIVATIVES', 'Derivatives', 5 FROM opt
ON CONFLICT (option_def_id, choice_value) DO NOTHING;

-- Speed Choices
WITH opt AS (
    SELECT sod.option_def_id 
    FROM "ob-poc".service_option_definitions sod
    JOIN "ob-poc".services s ON sod.service_id = s.service_id
    WHERE s.service_code = 'SETTLEMENT' AND sod.option_key = 'speed'
)
INSERT INTO "ob-poc".service_option_choices (option_def_id, choice_value, choice_label, display_order)
SELECT option_def_id, 'T0', 'Same Day (T+0)', 1 FROM opt
UNION ALL
SELECT option_def_id, 'T1', 'Next Day (T+1)', 2 FROM opt
UNION ALL
SELECT option_def_id, 'T2', 'T+2', 3 FROM opt
ON CONFLICT (option_def_id, choice_value) DO NOTHING;

-- Production Resources (must include 'owner' field)
INSERT INTO "ob-poc".prod_resources (name, owner, resource_code, resource_type, vendor, capabilities, is_active)
VALUES 
    ('DTCC Settlement System', 'Operations', 'DTCC_SETTLE', 'settlement_system', 'DTCC', 
     '{"markets": ["US_EQUITY"], "asset_classes": ["equity", "etf"], "speed": ["T0", "T1", "T2"]}'::jsonb, true),
    ('Euroclear Settlement', 'Operations', 'EUROCLEAR', 'settlement_system', 'Euroclear',
     '{"markets": ["EU_EQUITY"], "asset_classes": ["equity", "bond"], "speed": ["T1", "T2"]}'::jsonb, true),
    ('APAC Clearinghouse', 'Operations', 'APAC_CLEAR', 'settlement_system', 'ASX',
     '{"markets": ["APAC_EQUITY"], "asset_classes": ["equity"], "speed": ["T2"]}'::jsonb, true)
ON CONFLICT (name) DO UPDATE 
SET resource_code = EXCLUDED.resource_code,
    resource_type = EXCLUDED.resource_type,
    vendor = EXCLUDED.vendor,
    capabilities = EXCLUDED.capabilities,
    is_active = EXCLUDED.is_active,
    owner = EXCLUDED.owner;

-- Link Resources to Services with Capabilities
WITH s AS (SELECT service_id FROM "ob-poc".services WHERE service_code = 'SETTLEMENT'),
     r AS (SELECT resource_id, resource_code FROM "ob-poc".prod_resources WHERE resource_code IN ('DTCC_SETTLE', 'EUROCLEAR', 'APAC_CLEAR'))
INSERT INTO "ob-poc".service_resource_capabilities (service_id, resource_id, supported_options, priority)
SELECT s.service_id, r.resource_id,
       CASE r.resource_code
           WHEN 'DTCC_SETTLE' THEN '{"markets": ["US_EQUITY"], "speed": ["T0", "T1", "T2"]}'::jsonb
           WHEN 'EUROCLEAR' THEN '{"markets": ["EU_EQUITY"], "speed": ["T1", "T2"]}'::jsonb
           WHEN 'APAC_CLEAR' THEN '{"markets": ["APAC_EQUITY"], "speed": ["T2"]}'::jsonb
       END,
       CASE r.resource_code
           WHEN 'DTCC_SETTLE' THEN 100
           WHEN 'EUROCLEAR' THEN 90
           WHEN 'APAC_CLEAR' THEN 80
       END
FROM s, r
ON CONFLICT (service_id, resource_id) DO UPDATE
SET supported_options = EXCLUDED.supported_options,
    priority = EXCLUDED.priority;

COMMIT;

-- Verification
SELECT 'Products:' as info, COUNT(*) as count FROM "ob-poc".products WHERE product_code IS NOT NULL
UNION ALL
SELECT 'Services:', COUNT(*) FROM "ob-poc".services WHERE service_code IS NOT NULL
UNION ALL
SELECT 'Product-Service mappings:', COUNT(*) FROM "ob-poc".product_services
UNION ALL
SELECT 'Service options:', COUNT(*) FROM "ob-poc".service_option_definitions
UNION ALL
SELECT 'Option choices:', COUNT(*) FROM "ob-poc".service_option_choices
UNION ALL
SELECT 'Production resources:', COUNT(*) FROM "ob-poc".prod_resources WHERE resource_code IS NOT NULL
UNION ALL
SELECT 'Resource capabilities:', COUNT(*) FROM "ob-poc".service_resource_capabilities;
