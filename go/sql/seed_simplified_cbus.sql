-- Simplified seed script for Hedge Fund and US 1940 Act CBUs only
-- Run after init.sql and seed-catalog

-- First, ensure we have the necessary entity types in the entity_types table
INSERT INTO "ob-poc".entity_types (entity_type_id, name, description, table_name, created_at, updated_at)
VALUES
    ('11111111-1111-1111-1111-111111111111', 'PARTNERSHIP', 'Limited Liability Partnership', 'entity_partnerships', NOW(), NOW()),
    ('22222222-2222-2222-2222-222222222222', 'LIMITED_COMPANY', 'Limited Company/Corporation', 'entity_limited_companies', NOW(), NOW()),
    ('33333333-3333-3333-3333-333333333333', 'PROPER_PERSON', 'Natural Person/Proper Person', 'entity_proper_persons', NOW(), NOW())
ON CONFLICT (name) DO NOTHING;

-- Create standard roles for financial entities
INSERT INTO "ob-poc".roles (role_id, name, description, created_at, updated_at)
VALUES
    ('1bad3016-0201-418b-ba0b-c90d19ec60cf', 'GENERAL_PARTNER', 'General Partner of hedge fund', NOW(), NOW()),
    ('82237ed0-fbca-4c56-afa6-0077b1e9bb1b', 'INVESTMENT_MANAGER', 'Investment Manager', NOW(), NOW()),
    ('6194082a-cd9f-4f21-aaa6-270ab86dd5ce', 'ASSET_OWNER', 'Asset Owner/Fund Entity', NOW(), NOW()),
    ('ec62f238-c473-49eb-9ec4-b8095e6b0174', 'MANAGEMENT_COMPANY', 'Management Company', NOW(), NOW()),
    ('ebf7b8bb-8806-49d7-85fd-d64f3fffdeb8', 'SERVICE_PROVIDER', 'Service Provider/Administrator', NOW(), NOW())
ON CONFLICT (name) DO NOTHING;

-- =============================================================================
-- HEDGE FUND CBUs
-- =============================================================================

-- Hedge Fund CBU 1: Quantum Capital Partners
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('afcd574a-b311-4af3-a616-6d251d47419e', 'HF-QCP-001', 'Quantum Capital Partners Hedge Fund', 'Multi-strategy hedge fund focused on equity long/short and merger arbitrage', NOW(), NOW());

-- Hedge Fund CBU 2: Meridian Alpha Fund
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('057adde2-e8ad-4345-839d-b00fb124e2e3', 'HF-MAF-002', 'Meridian Alpha Fund', 'Quantitative hedge fund specializing in fixed income and derivatives strategies', NOW(), NOW());

-- LLP Entities for Hedge Fund 1 (Quantum Capital Partners)
INSERT INTO "ob-poc".entity_partnerships (partnership_id, partnership_name, partnership_type, jurisdiction, formation_date, principal_place_business, partnership_agreement_date, created_at, updated_at)
VALUES
    ('d91ded86-600a-419c-bc65-7cf8ea59d9fb', 'Quantum Capital Management LLP', 'Limited Liability', 'US-DE', '2020-03-15', '200 Greenwich Street, New York, NY 10007', '2020-03-10', NOW(), NOW()),
    ('16674451-6f26-4e8a-89e1-8a14380f827b', 'QCP Investment Advisors LLP', 'Limited Liability', 'US-DE', '2020-03-15', '200 Greenwich Street, New York, NY 10007', '2020-03-10', NOW(), NOW());

-- LLP Entities for Hedge Fund 2 (Meridian Alpha Fund)
INSERT INTO "ob-poc".entity_partnerships (partnership_id, partnership_name, partnership_type, jurisdiction, formation_date, principal_place_business, partnership_agreement_date, created_at, updated_at)
VALUES
    ('b6be1e69-5faf-4cb4-a04e-df653817ac64', 'Meridian Investment Management LLP', 'Limited Liability', 'US-NY', '2019-09-22', '1345 Avenue of the Americas, New York, NY 10105', '2019-09-20', NOW(), NOW()),
    ('93cc2439-b47e-499f-8126-ed49c3c15bf6', 'Meridian Alpha GP LLP', 'Limited Liability', 'US-NY', '2019-09-22', '1345 Avenue of the Americas, New York, NY 10105', '2019-09-20', NOW(), NOW());

-- Register LLP entities in central entities table
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
VALUES
    -- Quantum Capital Partners LLPs
    ('0f0ea5f9-05b1-480b-901d-fdcb319f79a3', '11111111-1111-1111-1111-111111111111', 'd91ded86-600a-419c-bc65-7cf8ea59d9fb', 'Quantum Capital Management LLP', NOW(), NOW()),
    ('8a9a865c-d900-43b1-a9b1-23b3f540843a', '11111111-1111-1111-1111-111111111111', '16674451-6f26-4e8a-89e1-8a14380f827b', 'QCP Investment Advisors LLP', NOW(), NOW()),
    -- Meridian Alpha Fund LLPs
    ('f785423a-015c-40d5-9a1e-7d2dc0fefb23', '11111111-1111-1111-1111-111111111111', 'b6be1e69-5faf-4cb4-a04e-df653817ac64', 'Meridian Investment Management LLP', NOW(), NOW()),
    ('555546cf-6a32-4e8d-8aa3-c8d60580e221', '11111111-1111-1111-1111-111111111111', '93cc2439-b47e-499f-8126-ed49c3c15bf6', 'Meridian Alpha GP LLP', NOW(), NOW());

-- Link Hedge Fund CBUs to their LLP entities with roles
INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
VALUES
    -- Quantum Capital Partners CBU relationships
    ('b11f0fa9-bd11-4572-8176-dec24a9a5425', 'afcd574a-b311-4af3-a616-6d251d47419e', '0f0ea5f9-05b1-480b-901d-fdcb319f79a3', '82237ed0-fbca-4c56-afa6-0077b1e9bb1b', NOW()), -- Investment Manager
    ('9020f6a3-0452-4613-970d-0e7b396d7d33', 'afcd574a-b311-4af3-a616-6d251d47419e', '8a9a865c-d900-43b1-a9b1-23b3f540843a', '1bad3016-0201-418b-ba0b-c90d19ec60cf', NOW()), -- General Partner
    -- Meridian Alpha Fund CBU relationships
    ('1bdf1105-634f-461a-9c81-7ce7e1529868', '057adde2-e8ad-4345-839d-b00fb124e2e3', 'f785423a-015c-40d5-9a1e-7d2dc0fefb23', '82237ed0-fbca-4c56-afa6-0077b1e9bb1b', NOW()), -- Investment Manager
    ('072d80b1-38df-4c61-9f9d-39d8d2011731', '057adde2-e8ad-4345-839d-b00fb124e2e3', '555546cf-6a32-4e8d-8aa3-c8d60580e221', '1bad3016-0201-418b-ba0b-c90d19ec60cf', NOW()); -- General Partner

-- =============================================================================
-- US 1940 ACT CBUs AND ENTITIES
-- =============================================================================

-- US 1940 Act CBU 1: Asset Owner (Mutual Fund)
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('0cd08a4a-dcff-44ed-9e2c-3a41347a9418', 'US1940-AO-001', 'American Growth Equity Fund', '1940 Act registered mutual fund - large cap growth equity strategy', NOW(), NOW());

-- US 1940 Act CBU 2: Investment Manager
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('aa11bb22-cc33-44dd-ee55-ff6677889900', 'US1940-IM-002', 'Sterling Asset Management Company', 'SEC registered investment advisor providing portfolio management services', NOW(), NOW());

-- US 1940 Act CBU 3: Management Company
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('bb22cc33-dd44-55ee-ff66-778899aabbcc', 'US1940-MC-003', 'Continental Fund Services Inc', 'Registered management company providing fund administration and compliance services', NOW(), NOW());

-- Limited Company Entities for US 1940 Act CBUs
INSERT INTO "ob-poc".entity_limited_companies (limited_company_id, company_name, registration_number, jurisdiction, incorporation_date, registered_address, business_nature, created_at, updated_at)
VALUES
    -- Asset Owner Entity
    ('cc33dd44-ee55-66ff-7788-99aabbccddee', 'American Growth Equity Fund Inc', 'DE-8765432', 'US-DE', '2018-01-15', '1601 Cherry Street, Philadelphia, PA 19102', 'Registered Investment Company under 1940 Act', NOW(), NOW()),
    -- Investment Manager Entity
    ('dd44ee55-ff66-77aa-88bb-99ccddee00ff', 'Sterling Asset Management Company', 'DE-9876543', 'US-DE', '2015-06-10', '100 Federal Street, Boston, MA 02110', 'SEC Registered Investment Advisor', NOW(), NOW()),
    -- Management Company Entity
    ('ee55ff66-aa77-88bb-99cc-ddee00ff1122', 'Continental Fund Services Inc', 'DE-5432198', 'US-DE', '2012-11-20', '2005 Market Street, Philadelphia, PA 19103', 'Fund Administration and Management Services', NOW(), NOW());

-- Register Limited Company entities in central entities table
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
VALUES
    -- US 1940 Act Companies
    ('ff66aa77-bb88-99cc-ddee-00ff11223344', '22222222-2222-2222-2222-222222222222', 'cc33dd44-ee55-66ff-7788-99aabbccddee', 'American Growth Equity Fund Inc', NOW(), NOW()),
    ('aa77bb88-cc99-ddee-00ff-112233445566', '22222222-2222-2222-2222-222222222222', 'dd44ee55-ff66-77aa-88bb-99ccddee00ff', 'Sterling Asset Management Company', NOW(), NOW()),
    ('bb88cc99-ddee-00ff-1122-334455667788', '22222222-2222-2222-2222-222222222222', 'ee55ff66-aa77-88bb-99cc-ddee00ff1122', 'Continental Fund Services Inc', NOW(), NOW());

-- Link US 1940 Act CBUs to their Limited Company entities with roles
INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
VALUES
    -- Asset Owner CBU relationship
    ('cc99ddee-00ff-1122-3344-556677889900', '0cd08a4a-dcff-44ed-9e2c-3a41347a9418', 'ff66aa77-bb88-99cc-ddee-00ff11223344', '6194082a-cd9f-4f21-aaa6-270ab86dd5ce', NOW()), -- Asset Owner
    -- Investment Manager CBU relationship
    ('ddee00ff-1122-3344-5566-7788990011aa', 'aa11bb22-cc33-44dd-ee55-ff6677889900', 'aa77bb88-cc99-ddee-00ff-112233445566', '82237ed0-fbca-4c56-afa6-0077b1e9bb1b', NOW()), -- Investment Manager
    -- Management Company CBU relationship
    ('ee00ff11-2233-4455-6677-88990011aabb', 'bb22cc33-dd44-55ee-ff66-778899aabbcc', 'bb88cc99-ddee-00ff-1122-334455667788', 'ec62f238-c473-49eb-9ec4-b8095e6b0174', NOW()); -- Management Company

-- =============================================================================
-- SUMMARY VERIFICATION QUERIES (for manual verification after running)
-- =============================================================================

-- Uncomment the following to verify the data was inserted correctly:

-- SELECT 'CBUs Created:' as summary;
-- SELECT name, description, nature_purpose FROM "ob-poc".cbus WHERE name LIKE 'HF-%' OR name LIKE 'US1940-%';

-- SELECT 'LLP Partnerships Created:' as summary;
-- SELECT partnership_name, partnership_type, jurisdiction FROM "ob-poc".entity_partnerships;

-- SELECT 'Limited Companies Created:' as summary;
-- SELECT company_name, registration_number, jurisdiction, business_nature FROM "ob-poc".entity_limited_companies;

-- SELECT 'Entity Relationships:' as summary;
-- SELECT c.name as cbu_name, e.name as entity_name, r.name as role_name
-- FROM "ob-poc".cbu_entity_roles cer
-- JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
-- JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
-- JOIN "ob-poc".roles r ON r.role_id = cer.role_id
-- WHERE c.name LIKE 'HF-%' OR c.name LIKE 'US1940-%'
-- ORDER BY c.name, r.name;
