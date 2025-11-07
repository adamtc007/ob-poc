-- Seed script for Hedge Fund and US 1940 Act CBUs with associated entities
-- Run after init.sql and seed-catalog

-- First, ensure we have the necessary entity types in the entity_types table
INSERT INTO "dsl-ob-poc".entity_types (entity_type_id, name, description, table_name, created_at, updated_at)
VALUES
    ('11111111-1111-1111-1111-111111111111', 'PARTNERSHIP', 'Limited Liability Partnership', 'entity_partnerships', NOW(), NOW()),
    ('22222222-2222-2222-2222-222222222222', 'LIMITED_COMPANY', 'Limited Company/Corporation', 'entity_limited_companies', NOW(), NOW()),
    ('33333333-3333-3333-3333-333333333333', 'PROPER_PERSON', 'Natural Person/Proper Person', 'entity_proper_persons', NOW(), NOW())
ON CONFLICT (name) DO NOTHING;

-- Create standard roles for financial entities
INSERT INTO "dsl-ob-poc".roles (role_id, name, description, created_at, updated_at)
VALUES
    ('a1111111-1111-1111-1111-111111111111', 'GENERAL_PARTNER', 'General Partner of hedge fund', NOW(), NOW()),
    ('a2222222-2222-2222-2222-222222222222', 'INVESTMENT_MANAGER', 'Investment Manager', NOW(), NOW()),
    ('a3333333-3333-3333-3333-333333333333', 'ASSET_OWNER', 'Asset Owner/Fund Entity', NOW(), NOW()),
    ('a4444444-4444-4444-4444-444444444444', 'MANAGEMENT_COMPANY', 'Management Company', NOW(), NOW()),
    ('a5555555-5555-5555-5555-555555555555', 'SERVICE_PROVIDER', 'Service Provider/Administrator', NOW(), NOW())
ON CONFLICT (name) DO NOTHING;

-- =============================================================================
-- HEDGE FUND CBUs AND ENTITIES
-- =============================================================================

-- Hedge Fund CBU 1: Quantum Capital Partners
INSERT INTO "dsl-ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('b1111111-1111-1111-1111-111111111111', 'HF-QCP-001', 'Quantum Capital Partners Hedge Fund', 'Multi-strategy hedge fund focused on equity long/short and merger arbitrage', NOW(), NOW());

-- Hedge Fund CBU 2: Meridian Alpha Fund
INSERT INTO "dsl-ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('b2222222-2222-2222-2222-222222222222', 'HF-MAF-002', 'Meridian Alpha Fund', 'Quantitative hedge fund specializing in fixed income and derivatives strategies', NOW(), NOW());

-- LLP Entities for Hedge Fund 1 (Quantum Capital Partners)
INSERT INTO "dsl-ob-poc".entity_partnerships (partnership_id, partnership_name, partnership_type, jurisdiction, formation_date, principal_place_business, partnership_agreement_date, created_at, updated_at)
VALUES
    ('c1111111-1111-1111-1111-111111111111', 'Quantum Capital Management LLP', 'Limited Liability', 'US-DE', '2020-03-15', '200 Greenwich Street, New York, NY 10007', '2020-03-10', NOW(), NOW()),
    ('c1222222-1111-1111-1111-111111111111', 'QCP Investment Advisors LLP', 'Limited Liability', 'US-DE', '2020-03-15', '200 Greenwich Street, New York, NY 10007', '2020-03-10', NOW(), NOW());

-- LLP Entities for Hedge Fund 2 (Meridian Alpha Fund)
INSERT INTO "dsl-ob-poc".entity_partnerships (partnership_id, partnership_name, partnership_type, jurisdiction, formation_date, principal_place_business, partnership_agreement_date, created_at, updated_at)
VALUES
    ('c2111111-2222-2222-2222-222222222222', 'Meridian Investment Management LLP', 'Limited Liability', 'US-NY', '2019-09-22', '1345 Avenue of the Americas, New York, NY 10105', '2019-09-20', NOW(), NOW()),
    ('c2222222-2222-2222-2222-222222222222', 'Meridian Alpha GP LLP', 'Limited Liability', 'US-NY', '2019-09-22', '1345 Avenue of the Americas, New York, NY 10105', '2019-09-20', NOW(), NOW());

-- Register LLP entities in central entities table
INSERT INTO "dsl-ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
VALUES
    -- Quantum Capital Partners LLPs
    ('d1111111-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', 'c1111111-1111-1111-1111-111111111111', 'Quantum Capital Management LLP', NOW(), NOW()),
    ('d1222222-1111-1111-1111-111111111111', '11111111-1111-1111-1111-111111111111', 'c1222222-1111-1111-1111-111111111111', 'QCP Investment Advisors LLP', NOW(), NOW()),
    -- Meridian Alpha Fund LLPs
    ('d2111111-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111', 'c2111111-2222-2222-2222-222222222222', 'Meridian Investment Management LLP', NOW(), NOW()),
    ('d2222222-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111', 'c2222222-2222-2222-2222-222222222222', 'Meridian Alpha GP LLP', NOW(), NOW());

-- Link Hedge Fund CBUs to their LLP entities with roles
INSERT INTO "dsl-ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
VALUES
    -- Quantum Capital Partners CBU relationships
    ('e1111111-1111-1111-1111-111111111111', 'b1111111-1111-1111-1111-111111111111', 'd1111111-1111-1111-1111-111111111111', 'a2222222-2222-2222-2222-222222222222', NOW()), -- Investment Manager
    ('e1222222-1111-1111-1111-111111111111', 'b1111111-1111-1111-1111-111111111111', 'd1222222-1111-1111-1111-111111111111', 'a1111111-1111-1111-1111-111111111111', NOW()), -- General Partner
    -- Meridian Alpha Fund CBU relationships
    ('e2111111-2222-2222-2222-222222222222', 'b2222222-2222-2222-2222-222222222222', 'd2111111-2222-2222-2222-222222222222', 'a2222222-2222-2222-2222-222222222222', NOW()), -- Investment Manager
    ('e2222222-2222-2222-2222-222222222222', 'b2222222-2222-2222-2222-222222222222', 'd2222222-2222-2222-2222-222222222222', 'a1111111-1111-1111-1111-111111111111', NOW()); -- General Partner

-- =============================================================================
-- US 1940 ACT CBUs AND ENTITIES
-- =============================================================================

-- US 1940 Act CBU 1: Asset Owner (Mutual Fund)
INSERT INTO "dsl-ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('f1111111-1111-1111-1111-111111111111', 'US1940-AO-001', 'American Growth Equity Fund', '1940 Act registered mutual fund - large cap growth equity strategy', NOW(), NOW());

-- US 1940 Act CBU 2: Investment Manager
INSERT INTO "dsl-ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('f2222222-2222-2222-2222-222222222222', 'US1940-IM-002', 'Sterling Asset Management Company', 'SEC registered investment advisor providing portfolio management services', NOW(), NOW());

-- US 1940 Act CBU 3: Management Company
INSERT INTO "dsl-ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('f3333333-3333-3333-3333-333333333333', 'US1940-MC-003', 'Continental Fund Services Inc', 'Registered management company providing fund administration and compliance services', NOW(), NOW());

-- Limited Company Entities for US 1940 Act CBUs
INSERT INTO "dsl-ob-poc".entity_limited_companies (limited_company_id, company_name, registration_number, jurisdiction, incorporation_date, registered_address, business_nature, created_at, updated_at)
VALUES
    -- Asset Owner Entity
    ('g1111111-1111-1111-1111-111111111111', 'American Growth Equity Fund Inc', 'DE-8765432', 'US-DE', '2018-01-15', '1601 Cherry Street, Philadelphia, PA 19102', 'Registered Investment Company under 1940 Act', NOW(), NOW()),
    -- Investment Manager Entity
    ('g2222222-2222-2222-2222-222222222222', 'Sterling Asset Management Company', 'DE-9876543', 'US-DE', '2015-06-10', '100 Federal Street, Boston, MA 02110', 'SEC Registered Investment Advisor', NOW(), NOW()),
    -- Management Company Entity
    ('g3333333-3333-3333-3333-333333333333', 'Continental Fund Services Inc', 'DE-5432198', 'US-DE', '2012-11-20', '2005 Market Street, Philadelphia, PA 19103', 'Fund Administration and Management Services', NOW(), NOW());

-- Register Limited Company entities in central entities table
INSERT INTO "dsl-ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
VALUES
    -- US 1940 Act Companies
    ('h1111111-1111-1111-1111-111111111111', '22222222-2222-2222-2222-222222222222', 'g1111111-1111-1111-1111-111111111111', 'American Growth Equity Fund Inc', NOW(), NOW()),
    ('h2222222-2222-2222-2222-222222222222', '22222222-2222-2222-2222-222222222222', 'g2222222-2222-2222-2222-222222222222', 'Sterling Asset Management Company', NOW(), NOW()),
    ('h3333333-3333-3333-3333-333333333333', '22222222-2222-2222-2222-222222222222', 'g3333333-3333-3333-3333-333333333333', 'Continental Fund Services Inc', NOW(), NOW());

-- Link US 1940 Act CBUs to their Limited Company entities with roles
INSERT INTO "dsl-ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
VALUES
    -- Asset Owner CBU relationship
    ('i1111111-3333-3333-3333-333333333333', 'f1111111-1111-1111-1111-111111111111', 'h1111111-1111-1111-1111-111111111111', 'a3333333-3333-3333-3333-333333333333', NOW()), -- Asset Owner
    -- Investment Manager CBU relationship
    ('i2222222-3333-3333-3333-333333333333', 'f2222222-2222-2222-2222-222222222222', 'h2222222-2222-2222-2222-222222222222', 'a2222222-2222-2222-2222-222222222222', NOW()), -- Investment Manager
    -- Management Company CBU relationship
    ('i3333333-3333-3333-3333-333333333333', 'f3333333-3333-3333-3333-333333333333', 'h3333333-3333-3333-3333-333333333333', 'a4444444-4444-4444-4444-444444444444', NOW()); -- Management Company

-- =============================================================================
-- SUMMARY VERIFICATION QUERIES (for manual verification after running)
-- =============================================================================

-- Uncomment the following to verify the data was inserted correctly:

-- SELECT 'CBUs Created:' as summary;
-- SELECT name, description, nature_purpose FROM "dsl-ob-poc".cbus WHERE name LIKE 'HF-%' OR name LIKE 'US1940-%';

-- SELECT 'LLP Partnerships Created:' as summary;
-- SELECT partnership_name, partnership_type, jurisdiction FROM "dsl-ob-poc".entity_partnerships;

-- SELECT 'Limited Companies Created:' as summary;
-- SELECT company_name, registration_number, jurisdiction, business_nature FROM "dsl-ob-poc".entity_limited_companies;

-- SELECT 'Entity Relationships:' as summary;
-- SELECT c.name as cbu_name, e.name as entity_name, r.name as role_name
-- FROM "dsl-ob-poc".cbu_entity_roles cer
-- JOIN "dsl-ob-poc".cbus c ON c.cbu_id = cer.cbu_id
-- JOIN "dsl-ob-poc".entities e ON e.entity_id = cer.entity_id
-- JOIN "dsl-ob-poc".roles r ON r.role_id = cer.role_id
-- WHERE c.name LIKE 'HF-%' OR c.name LIKE 'US1940-%'
-- ORDER BY c.name, r.name;
