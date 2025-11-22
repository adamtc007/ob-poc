-- ============================================
-- OB-POC Seed Data - Consolidated
-- ============================================
-- Version: 3.0
-- Date: 2025-11-16
-- Description: Essential seed data for ob-poc system
-- ============================================

BEGIN;

-- ============================================
-- ENTITY TYPES
-- ============================================

INSERT INTO "ob-poc".entity_types (type_code, type_name, description, is_active) VALUES
('PERSON', 'Individual Person', 'Natural person/individual', true),
('LIMITED_COMPANY', 'Limited Company', 'Private or public limited company', true),
('PARTNERSHIP', 'Partnership', 'General or limited partnership', true),
('TRUST', 'Trust', 'Trust structure', true),
('FOUNDATION', 'Foundation', 'Private foundation', true),
('LLC', 'Limited Liability Company', 'LLC structure', true)
ON CONFLICT (type_code) DO NOTHING;

-- ============================================
-- ROLES
-- ============================================

INSERT INTO "ob-poc".roles (name, description) VALUES
('Beneficial Owner', 'Ultimate beneficial owner'),
('Director', 'Company director'),
('Shareholder', 'Company shareholder'),
('Trustee', 'Trust trustee'),
('Settlor', 'Trust settlor'),
('Beneficiary', 'Trust beneficiary'),
('Protector', 'Trust protector'),
('Partner', 'Partnership partner'),
('Authorized Signatory', 'Authorized to sign on behalf of entity')
ON CONFLICT (name) DO NOTHING;

-- ============================================
-- PRODUCTS
-- ============================================

INSERT INTO "ob-poc".products (name, product_code, product_category, description, is_active) VALUES
('Institutional Custody', 'CUSTODY_INST', 'custody', 'Full custody services for institutional clients', true),
('Prime Brokerage', 'PRIME_BROKER', 'prime_brokerage', 'Comprehensive prime brokerage services', true),
('Fund Administration', 'FUND_ADMIN', 'fund_admin', 'Complete fund administration services', true)
ON CONFLICT (name) DO UPDATE 
SET product_code = EXCLUDED.product_code,
    product_category = EXCLUDED.product_category,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- ============================================
-- SERVICES
-- ============================================

INSERT INTO "ob-poc".services (name, service_code, service_category, description, is_active) VALUES
('Trade Settlement', 'SETTLEMENT', 'settlement', 'Multi-market trade settlement', true),
('Asset Safekeeping', 'SAFEKEEPING', 'custody', 'Secure asset custody', true),
('Corporate Actions', 'CORP_ACTIONS', 'operations', 'Corporate action processing', true),
('Client Reporting', 'REPORTING', 'reporting', 'Regulatory and client reporting', true)
ON CONFLICT (name) DO UPDATE 
SET service_code = EXCLUDED.service_code,
    service_category = EXCLUDED.service_category,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- ============================================
-- PRODUCT-SERVICE MAPPINGS
-- ============================================

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

-- ============================================
-- SERVICE OPTIONS
-- ============================================

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

-- ============================================
-- PRODUCTION RESOURCES
-- ============================================

INSERT INTO "ob-poc".prod_resources (name, owner, resource_code, resource_type, vendor, capabilities, is_active) VALUES
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

-- ============================================
-- SERVICE-RESOURCE CAPABILITIES
-- ============================================

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

-- ============================================
-- DSL DOMAINS
-- ============================================

INSERT INTO "ob-poc".dsl_domains (domain_name, description, is_active) VALUES
('case', 'Case management domain', true),
('kyc', 'KYC/AML domain', true),
('entity', 'Entity management domain', true),
('products', 'Product configuration domain', true),
('services', 'Service management domain', true),
('ubo', 'Ultimate beneficial ownership domain', true),
('document', 'Document management domain', true),
('compliance', 'Compliance and screening domain', true)
ON CONFLICT (domain_name) DO UPDATE
SET description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- ============================================
-- DOMAIN VOCABULARIES
-- ============================================

INSERT INTO "ob-poc".domain_vocabularies (domain_name, verb, description, is_active) VALUES
-- Case domain
('case', 'create', 'Create new case', true),
('case', 'update', 'Update case details', true),
('case', 'close', 'Close case', true),
-- KYC domain
('kyc', 'start', 'Start KYC process', true),
('kyc', 'collect', 'Collect KYC data', true),
('kyc', 'verify', 'Verify KYC information', true),
-- Entity domain
('entity', 'register', 'Register new entity', true),
('entity', 'classify', 'Classify entity type', true),
('entity', 'link', 'Link entities', true),
-- Product domain
('products', 'add', 'Add product to request', true),
('products', 'configure', 'Configure product', true),
-- Service domain
('services', 'discover', 'Discover available services', true),
('services', 'configure', 'Configure service options', true),
-- UBO domain
('ubo', 'collect-entity-data', 'Collect entity data', true),
('ubo', 'resolve-ubos', 'Resolve ultimate beneficial owners', true),
-- Document domain
('document', 'catalog', 'Catalog document', true),
('document', 'verify', 'Verify document', true),
('document', 'extract', 'Extract data from document', true)
ON CONFLICT (domain_name, verb) DO UPDATE
SET description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

-- ============================================
-- MASTER JURISDICTIONS (Sample)
-- ============================================

INSERT INTO "ob-poc".master_jurisdictions (jurisdiction_code, jurisdiction_name, region) VALUES
('US', 'United States', 'North America'),
('GB', 'United Kingdom', 'Europe'),
('DE', 'Germany', 'Europe'),
('FR', 'France', 'Europe'),
('SG', 'Singapore', 'Asia Pacific'),
('HK', 'Hong Kong', 'Asia Pacific'),
('CH', 'Switzerland', 'Europe'),
('LU', 'Luxembourg', 'Europe'),
('KY', 'Cayman Islands', 'Caribbean'),
('BM', 'Bermuda', 'Atlantic')
ON CONFLICT (jurisdiction_code) DO UPDATE
SET jurisdiction_name = EXCLUDED.jurisdiction_name,
    region = EXCLUDED.region;

-- ============================================
-- DOCUMENT TYPES (Sample)
-- ============================================

INSERT INTO "ob-poc".document_types (type_code, type_name, description, is_active) VALUES
('PASSPORT', 'Passport', 'National passport document', true),
('DRIVERS_LICENSE', 'Drivers License', 'Government-issued drivers license', true),
('NATIONAL_ID', 'National ID Card', 'National identity card', true),
('PROOF_ADDRESS', 'Proof of Address', 'Utility bill or bank statement', true),
('CERT_INCORPORATION', 'Certificate of Incorporation', 'Company incorporation certificate', true),
('ARTICLES_ASSOC', 'Articles of Association', 'Company articles/bylaws', true),
('TRUST_DEED', 'Trust Deed', 'Trust deed document', true),
('PARTNERSHIP_AGREEMENT', 'Partnership Agreement', 'Partnership agreement', true),
('BENEFICIAL_OWNER_CERT', 'Beneficial Ownership Certificate', 'UBO certification', true),
('FINANCIAL_STATEMENTS', 'Financial Statements', 'Audited financial statements', true)
ON CONFLICT (type_code) DO UPDATE
SET type_name = EXCLUDED.type_name,
    description = EXCLUDED.description,
    is_active = EXCLUDED.is_active;

COMMIT;

-- ============================================
-- VERIFICATION QUERIES
-- ============================================

SELECT 'Seed Data Summary' as info;
SELECT '==================' as separator;
SELECT 'Entity Types:' as category, COUNT(*) as count FROM "ob-poc".entity_types
UNION ALL
SELECT 'Roles:', COUNT(*) FROM "ob-poc".roles
UNION ALL
SELECT 'Products:', COUNT(*) FROM "ob-poc".products WHERE product_code IS NOT NULL
UNION ALL
SELECT 'Services:', COUNT(*) FROM "ob-poc".services WHERE service_code IS NOT NULL
UNION ALL
SELECT 'Service Options:', COUNT(*) FROM "ob-poc".service_option_definitions
UNION ALL
SELECT 'Option Choices:', COUNT(*) FROM "ob-poc".service_option_choices
UNION ALL
SELECT 'Production Resources:', COUNT(*) FROM "ob-poc".prod_resources WHERE resource_code IS NOT NULL
UNION ALL
SELECT 'Resource Capabilities:', COUNT(*) FROM "ob-poc".service_resource_capabilities
UNION ALL
SELECT 'DSL Domains:', COUNT(*) FROM "ob-poc".dsl_domains
UNION ALL
SELECT 'Domain Vocabularies:', COUNT(*) FROM "ob-poc".domain_vocabularies
UNION ALL
SELECT 'Jurisdictions:', COUNT(*) FROM "ob-poc".master_jurisdictions
UNION ALL
SELECT 'Document Types:', COUNT(*) FROM "ob-poc".document_types;
