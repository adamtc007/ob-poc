-- ============================================
-- Seed Resource Type Attribute Requirements
-- ============================================

BEGIN;

-- Get or create dictionary entries for resource attributes
INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain)
VALUES
    (gen_random_uuid(), 'account_number', 'Account Number for resource instance', 'resource.account', 'string', 'resource'),
    (gen_random_uuid(), 'bic_code', 'BIC/SWIFT Code for routing', 'resource.routing', 'string', 'resource'),
    (gen_random_uuid(), 'settlement_currency', 'Settlement Currency code', 'resource.account', 'string', 'resource'),
    (gen_random_uuid(), 'api_key', 'API Key for authentication', 'resource.connection', 'string', 'resource'),
    (gen_random_uuid(), 'api_secret', 'API Secret for authentication', 'resource.connection', 'string', 'resource'),
    (gen_random_uuid(), 'platform_user_id', 'Platform User ID', 'resource.access', 'string', 'resource'),
    (gen_random_uuid(), 'access_level', 'Access Level (READ, WRITE, ADMIN)', 'resource.access', 'string', 'resource'),
    (gen_random_uuid(), 'routing_number', 'Bank Routing Number', 'resource.routing', 'string', 'resource'),
    (gen_random_uuid(), 'iban', 'International Bank Account Number', 'resource.account', 'string', 'resource'),
    (gen_random_uuid(), 'custodian_code', 'Custodian identification code', 'resource.custody', 'string', 'resource')
ON CONFLICT (name) DO UPDATE
SET long_description = EXCLUDED.long_description,
    group_id = EXCLUDED.group_id,
    domain = EXCLUDED.domain;

-- Link attributes to DTCC resource type
WITH dtcc AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'DTCC_SETTLE'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('account_number', 'bic_code', 'settlement_currency'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT dtcc.resource_id, attrs.attribute_id,
       CASE attrs.name
           WHEN 'account_number' THEN true
           WHEN 'bic_code' THEN true
           WHEN 'settlement_currency' THEN false
       END,
       CASE attrs.name
           WHEN 'account_number' THEN 1
           WHEN 'bic_code' THEN 2
           WHEN 'settlement_currency' THEN 3
       END
FROM dtcc, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE
SET is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- Link attributes to Euroclear resource type
WITH euro AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'EUROCLEAR'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('account_number', 'iban', 'settlement_currency'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT euro.resource_id, attrs.attribute_id,
       CASE attrs.name
           WHEN 'account_number' THEN true
           WHEN 'iban' THEN true
           WHEN 'settlement_currency' THEN false
       END,
       CASE attrs.name
           WHEN 'account_number' THEN 1
           WHEN 'iban' THEN 2
           WHEN 'settlement_currency' THEN 3
       END
FROM euro, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE
SET is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

-- Link attributes to APAC Clearinghouse resource type
WITH apac AS (SELECT resource_id FROM "ob-poc".prod_resources WHERE resource_code = 'APAC_CLEAR'),
     attrs AS (SELECT attribute_id, name FROM "ob-poc".dictionary
               WHERE name IN ('account_number', 'settlement_currency', 'custodian_code'))
INSERT INTO "ob-poc".resource_attribute_requirements (resource_id, attribute_id, is_mandatory, display_order)
SELECT apac.resource_id, attrs.attribute_id,
       CASE attrs.name
           WHEN 'account_number' THEN true
           WHEN 'custodian_code' THEN true
           WHEN 'settlement_currency' THEN false
       END,
       CASE attrs.name
           WHEN 'account_number' THEN 1
           WHEN 'custodian_code' THEN 2
           WHEN 'settlement_currency' THEN 3
       END
FROM apac, attrs
ON CONFLICT (resource_id, attribute_id) DO UPDATE
SET is_mandatory = EXCLUDED.is_mandatory,
    display_order = EXCLUDED.display_order;

COMMIT;

-- Verify seed data
SELECT pr.name as resource_name, pr.resource_code, d.name as attribute_name, rar.is_mandatory, rar.display_order
FROM "ob-poc".resource_attribute_requirements rar
JOIN "ob-poc".prod_resources pr ON rar.resource_id = pr.resource_id
JOIN "ob-poc".dictionary d ON rar.attribute_id = d.attribute_id
WHERE d.domain = 'resource'
ORDER BY pr.name, rar.display_order;
