-- ============================================
-- Seed Resource Type Attribute Requirements
-- ============================================

BEGIN;

-- =============================================================================
-- 1. DICTIONARY ENTRIES for Resource Attributes
-- =============================================================================

INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain)
VALUES
    -- Account attributes
    (gen_random_uuid(), 'resource.account.account_number', 'Custody or settlement account number', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.account.account_name', 'Account display name', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.account.base_currency', 'Base currency (ISO 4217)', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.account.account_type', 'Account type (OMNIBUS, SEGREGATED)', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.account.sub_custodian', 'Sub-custodian name', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.account.market_codes', 'Enabled market codes', 'Resource', 'json', 'Resource'),

    -- Settlement attributes
    (gen_random_uuid(), 'resource.settlement.bic_code', 'BIC/SWIFT code', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.settlement.settlement_currency', 'Settlement currency', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.settlement.csd_participant_id', 'CSD participant ID', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.settlement.netting_enabled', 'Netting enabled flag', 'Resource', 'boolean', 'Resource'),

    -- SWIFT attributes
    (gen_random_uuid(), 'resource.swift.logical_terminal', 'Logical terminal ID', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.swift.message_types', 'Enabled MT types', 'Resource', 'json', 'Resource'),
    (gen_random_uuid(), 'resource.swift.rma_status', 'RMA authorization status', 'Resource', 'string', 'Resource'),

    -- Fund attributes
    (gen_random_uuid(), 'resource.fund.fund_code', 'Fund identifier', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.fund.valuation_frequency', 'Valuation frequency (DAILY, WEEKLY, MONTHLY)', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.fund.pricing_source', 'Primary pricing source', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.fund.nav_cutoff_time', 'NAV cutoff time (HH:MM TZ)', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.fund.share_classes', 'Share class configuration', 'Resource', 'json', 'Resource'),

    -- IBOR attributes
    (gen_random_uuid(), 'resource.ibor.portfolio_code', 'Portfolio identifier', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.ibor.accounting_basis', 'Accounting basis (TRADE_DATE, SETTLEMENT_DATE)', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.ibor.position_source', 'Position source system', 'Resource', 'string', 'Resource'),
    (gen_random_uuid(), 'resource.ibor.reconciliation_enabled', 'Auto-reconciliation enabled', 'Resource', 'boolean', 'Resource')
ON CONFLICT (name) DO UPDATE SET
    long_description = EXCLUDED.long_description,
    mask = EXCLUDED.mask,
    domain = EXCLUDED.domain;

-- =============================================================================
-- 2. RESOURCE ATTRIBUTE REQUIREMENTS
-- =============================================================================

-- Custody Account attributes
WITH r AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'CUSTODY_ACCT'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('resource.account.account_number', 'resource.account.account_name',
                              'resource.account.base_currency', 'resource.account.account_type',
                              'resource.account.sub_custodian', 'resource.account.market_codes'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.name IN ('resource.account.account_number', 'resource.account.account_name',
                      'resource.account.base_currency', 'resource.account.account_type'),
       CASE attrs.name
           WHEN 'resource.account.account_number' THEN 1
           WHEN 'resource.account.account_name' THEN 2
           WHEN 'resource.account.base_currency' THEN 3
           WHEN 'resource.account.account_type' THEN 4
           WHEN 'resource.account.sub_custodian' THEN 5
           WHEN 'resource.account.market_codes' THEN 6
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- Settlement Account attributes
WITH r AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'SETTLE_ACCT'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('resource.account.account_number', 'resource.settlement.bic_code',
                              'resource.settlement.settlement_currency', 'resource.settlement.csd_participant_id',
                              'resource.settlement.netting_enabled'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.name IN ('resource.account.account_number', 'resource.settlement.bic_code',
                      'resource.settlement.settlement_currency'),
       CASE attrs.name
           WHEN 'resource.account.account_number' THEN 1
           WHEN 'resource.settlement.bic_code' THEN 2
           WHEN 'resource.settlement.settlement_currency' THEN 3
           WHEN 'resource.settlement.csd_participant_id' THEN 4
           WHEN 'resource.settlement.netting_enabled' THEN 5
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- SWIFT Connection attributes
WITH r AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'SWIFT_CONN'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('resource.settlement.bic_code', 'resource.swift.logical_terminal',
                              'resource.swift.message_types', 'resource.swift.rma_status'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.name IN ('resource.settlement.bic_code', 'resource.swift.logical_terminal',
                      'resource.swift.message_types'),
       CASE attrs.name
           WHEN 'resource.settlement.bic_code' THEN 1
           WHEN 'resource.swift.logical_terminal' THEN 2
           WHEN 'resource.swift.message_types' THEN 3
           WHEN 'resource.swift.rma_status' THEN 4
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- NAV Engine attributes
WITH r AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'NAV_ENGINE'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('resource.fund.fund_code', 'resource.fund.valuation_frequency',
                              'resource.fund.pricing_source', 'resource.fund.nav_cutoff_time',
                              'resource.fund.share_classes'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.name IN ('resource.fund.fund_code', 'resource.fund.valuation_frequency',
                      'resource.fund.pricing_source', 'resource.fund.nav_cutoff_time'),
       CASE attrs.name
           WHEN 'resource.fund.fund_code' THEN 1
           WHEN 'resource.fund.valuation_frequency' THEN 2
           WHEN 'resource.fund.pricing_source' THEN 3
           WHEN 'resource.fund.nav_cutoff_time' THEN 4
           WHEN 'resource.fund.share_classes' THEN 5
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

-- IBOR System attributes
WITH r AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'IBOR_SYSTEM'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('resource.ibor.portfolio_code', 'resource.ibor.accounting_basis',
                              'resource.account.base_currency', 'resource.ibor.position_source',
                              'resource.ibor.reconciliation_enabled'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT r.resource_id, attrs.attribute_id,
       attrs.name IN ('resource.ibor.portfolio_code', 'resource.ibor.accounting_basis',
                      'resource.account.base_currency', 'resource.ibor.position_source'),
       CASE attrs.name
           WHEN 'resource.ibor.portfolio_code' THEN 1
           WHEN 'resource.ibor.accounting_basis' THEN 2
           WHEN 'resource.account.base_currency' THEN 3
           WHEN 'resource.ibor.position_source' THEN 4
           WHEN 'resource.ibor.reconciliation_enabled' THEN 5
       END
FROM r, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE SET is_mandatory = EXCLUDED.is_mandatory, display_order = EXCLUDED.display_order;

COMMIT;

-- Verification
SELECT pr.resource_code, COUNT(rar.attribute_id) as attr_count,
       COUNT(*) FILTER (WHERE rar.is_mandatory) as required_count
FROM "ob-poc".prod_resources pr
LEFT JOIN "ob-poc".resource_attribute_requirements rar ON pr.resource_id = rar.resource_id
WHERE pr.resource_code IN ('CUSTODY_ACCT', 'SETTLE_ACCT', 'SWIFT_CONN', 'NAV_ENGINE', 'IBOR_SYSTEM')
GROUP BY pr.resource_code
ORDER BY pr.resource_code;
