-- 07_migrate_mock_data_to_db.sql
-- Migration script to move all mock data from JSON files to the database
-- This eliminates the need for mock interceptors and enables end-to-end database operations

-- First, clean up any existing test data
DELETE FROM "ob-poc".attribute_values;
DELETE FROM "ob-poc".dsl_ob;
DELETE FROM "ob-poc".cbu_entity_roles;
DELETE FROM "ob-poc".entity_limited_companies;
DELETE FROM "ob-poc".entity_partnerships;
DELETE FROM "ob-poc".entities;
DELETE FROM "ob-poc".service_resources;
DELETE FROM "ob-poc".product_services;
DELETE FROM "ob-poc".prod_resources;
DELETE FROM "ob-poc".services;
DELETE FROM "ob-poc".products;
DELETE FROM "ob-poc".roles;
DELETE FROM "ob-poc".entity_types;
DELETE FROM "ob-poc".cbus;

-- Migrate CBUs from mock data
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at) VALUES
('123e4567-e89b-12d3-a456-426614174000', 'CBU-1234', 'Aviva Investors Global Fund', 'UCITS equity fund domiciled in LU', '2024-01-15T10:30:00Z', '2024-01-15T10:30:00Z'),
('987fcdeb-51a2-43f7-8765-ba9876543210', 'CBU-5678', 'European Growth Fund', 'Investment fund focused on European growth stocks', '2024-02-01T14:20:00Z', '2024-02-01T14:20:00Z'),
('456789ab-cdef-1234-5678-9abcdef01234', 'CBU-9999', 'Emerging Markets Bond Fund', 'Fixed income fund investing in emerging market bonds', '2024-03-10T09:15:00Z', '2024-03-10T09:15:00Z');

-- Migrate Products from mock data
INSERT INTO "ob-poc".products (product_id, name, description, created_at, updated_at) VALUES
('prod-001-custody', 'CUSTODY', 'Custody and safekeeping services for investment assets', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('prod-002-fund-accounting', 'FUND_ACCOUNTING', 'Fund accounting and administration services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('prod-003-transfer-agency', 'TRANSFER_AGENCY', 'Transfer agency and shareholder services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('prod-004-investment-management', 'INVESTMENT_MANAGEMENT', 'Investment management and portfolio services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('prod-005-risk-management', 'RISK_MANAGEMENT', 'Risk management and compliance services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Services from mock data
INSERT INTO "ob-poc".services (service_id, name, description, created_at, updated_at) VALUES
('serv-001-custody', 'CustodyService', 'Core custody and asset safekeeping service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('serv-002-settlement', 'SettlementService', 'Trade settlement and clearing service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('serv-003-fund-accounting', 'FundAccountingService', 'Fund accounting and NAV calculation service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('serv-004-transfer-agency', 'TransferAgencyService', 'Shareholder record keeping and transfer service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('serv-005-reporting', 'ReportingService', 'Regulatory and client reporting service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Entity Types from mock data
INSERT INTO "ob-poc".entity_types (entity_type_id, name, description, created_at, updated_at) VALUES
('etype-001-company', 'Company', 'Corporate entity (corporation, LLC, etc.)', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('etype-002-partnership', 'Partnership', 'Partnership entity (LP, LLP, GP, etc.)', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('etype-003-individual', 'Individual', 'Individual person or natural person', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('etype-004-fund', 'Fund', 'Investment fund or vehicle', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Roles from mock data
INSERT INTO "ob-poc".roles (role_id, name, description, created_at, updated_at) VALUES
('role-001-ultimate-beneficial-owner', 'Ultimate Beneficial Owner', 'Person who ultimately owns or controls 25% or more', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('role-002-director', 'Director', 'Board member or director of the entity', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('role-003-authorized-signatory', 'Authorized Signatory', 'Person authorized to sign on behalf of the entity', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('role-004-beneficial-owner', 'Beneficial Owner', 'Person with significant ownership (10-25%)', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Entities from mock data (sample entities)
INSERT INTO "ob-poc".entities (entity_id, name, entity_type_id, jurisdiction, created_at, updated_at) VALUES
('entity-001-aviva-global', 'Aviva Investors Global Fund SICAV', 'etype-004-fund', 'LU', '2024-01-15T10:00:00Z', '2024-01-15T10:00:00Z'),
('entity-002-european-growth', 'European Growth Fund LP', 'etype-002-partnership', 'US', '2024-02-01T14:00:00Z', '2024-02-01T14:00:00Z'),
('entity-003-emerging-markets', 'Emerging Markets Bond Fund Inc.', 'etype-001-company', 'US', '2024-03-10T09:00:00Z', '2024-03-10T09:00:00Z');

-- Migrate Product-Service relationships
INSERT INTO "ob-poc".product_services (product_id, service_id, created_at) VALUES
('prod-001-custody', 'serv-001-custody', '2024-01-01T00:00:00Z'),
('prod-001-custody', 'serv-002-settlement', '2024-01-01T00:00:00Z'),
('prod-002-fund-accounting', 'serv-003-fund-accounting', '2024-01-01T00:00:00Z'),
('prod-002-fund-accounting', 'serv-005-reporting', '2024-01-01T00:00:00Z'),
('prod-003-transfer-agency', 'serv-004-transfer-agency', '2024-01-01T00:00:00Z');

-- Migrate DSL history from mock data
INSERT INTO "ob-poc".dsl_ob (version_id, cbu_id, dsl_text, created_at) VALUES
('v001-create-cbu-1234', 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)', '2024-01-15T10:30:00Z'),
('v002-add-products-cbu-1234', 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")', '2024-01-15T11:00:00Z'),
('v003-discover-services-cbu-1234', 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")

(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
    (service "SettlementService")
  )
  (for.product "FUND_ACCOUNTING"
    (service "FundAccountingService")
  )
)', '2024-01-15T11:30:00Z'),
('v004-discover-resources-cbu-1234', 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")

(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
    (service "SettlementService")
  )
  (for.product "FUND_ACCOUNTING"
    (service "FundAccountingService")
  )
)

(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "123e4567-e89b-12d3-a456-426614174000"))
  )
)', '2024-01-15T12:00:00Z'),
('v005-bind-values-cbu-1234', 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")

(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
    (service "SettlementService")
  )
  (for.product "FUND_ACCOUNTING"
    (service "FundAccountingService")
  )
)

(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "123e4567-e89b-12d3-a456-426614174000"))
  )
)

(values.bind
  (bind (attr-id "123e4567-e89b-12d3-a456-426614174000") (value "CBU-1234"))
)', '2024-01-15T12:30:00Z');

-- Create KYC rules table to replace hardcoded mock responses
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(100) NOT NULL,  -- 'ucits', 'hedge_fund', 'corporation', etc.
    jurisdiction VARCHAR(10),           -- 'US', 'LU', 'CAYMAN', etc.
    required_documents TEXT[] NOT NULL,  -- Array of required document types
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Create product requirements table to replace hardcoded product logic
CREATE TABLE IF NOT EXISTS "ob-poc".product_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_name VARCHAR(100) NOT NULL,
    required_documents TEXT[] NOT NULL,
    additional_jurisdictions TEXT[],
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Migrate KYC rules from hardcoded mock responses
INSERT INTO "ob-poc".kyc_rules (entity_type, jurisdiction, required_documents) VALUES
('ucits', 'LU', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation', 'W8BEN-E']),
('ucits', 'EU', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation', 'W8BEN-E']),
('hedge_fund', 'US', ARRAY['CertificateOfLimitedPartnership', 'PartnershipAgreement', 'W9', 'AMLPolicy']),
('hedge_fund', 'CAYMAN', ARRAY['CertificateOfLimitedPartnership', 'PartnershipAgreement', 'W9', 'AMLPolicy']),
('corporation', 'US', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation']),
('company', 'US', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation']),
('default', NULL, ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation', 'W8BEN-E']);

-- Migrate product requirements from hardcoded mock responses
INSERT INTO "ob-poc".product_requirements (product_name, required_documents) VALUES
('TRANSFER_AGENT', ARRAY['AMLPolicy', 'InvestorQuestionnaire']),
('CUSTODY', ARRAY['CustodyAgreement']),
('FUND_ACCOUNTING', ARRAY['AccountingPolicy']),
('PRIME_BROKERAGE', ARRAY['PrimeBrokerageAgreement', 'RiskManagementPolicy']);

-- Create DSL transformation rules table to replace hardcoded mock transformations
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_transformation_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instruction_pattern VARCHAR(255) NOT NULL,  -- Pattern to match against instructions
    transformation_type VARCHAR(50) NOT NULL,   -- 'add_product', 'add_jurisdiction', etc.
    target_values JSONB,                        -- Configuration for the transformation
    dsl_template TEXT,                          -- DSL template to generate
    confidence_score DECIMAL(3,2) DEFAULT 0.8,  -- Confidence score for the transformation
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Migrate transformation rules from hardcoded mock responses
INSERT INTO "ob-poc".dsl_transformation_rules (instruction_pattern, transformation_type, target_values, dsl_template, confidence_score) VALUES
('add.*fund_accounting', 'add_product', '{"product": "FUND_ACCOUNTING"}', '(products.add "FUND_ACCOUNTING")', 0.9),
('add.*transfer_agent', 'add_product', '{"product": "TRANSFER_AGENT"}', '(products.add "TRANSFER_AGENT")', 0.9),
('add.*custody', 'add_product', '{"product": "CUSTODY"}', '(products.add "CUSTODY")', 0.9),
('add.*jurisdiction.*lu', 'add_jurisdiction', '{"jurisdiction": "LU"}', '(jurisdictions.add "LU")', 0.8),
('add.*jurisdiction.*us', 'add_jurisdiction', '{"jurisdiction": "US"}', '(jurisdictions.add "US")', 0.8),
('add.*jurisdiction.*uk', 'add_jurisdiction', '{"jurisdiction": "UK"}', '(jurisdictions.add "UK")', 0.8),
('add.*document.*w8ben', 'add_document', '{"document": "W8BEN-E"}', '(documents.add "W8BEN-E")', 0.8),
('add.*document.*w9', 'add_document', '{"document": "W9"}', '(documents.add "W9")', 0.8),
('add.*document.*certificate', 'add_document', '{"document": "CertificateOfIncorporation"}', '(documents.add "CertificateOfIncorporation")', 0.7),
('change.*nature.*hedge fund', 'change_nature', '{"nature": "US-based hedge fund"}', '(nature-purpose.update "US-based hedge fund")', 0.8),
('change.*nature.*ucits', 'change_nature', '{"nature": "UCITS equity fund domiciled in LU"}', '(nature-purpose.update "UCITS equity fund domiciled in LU")', 0.8),
('change.*nature.*corporation', 'change_nature', '{"nature": "US corporation"}', '(nature-purpose.update "US corporation")', 0.7);

-- Create DSL validation rules table to replace hardcoded mock validation
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_validation_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_type VARCHAR(50) NOT NULL,        -- 'required', 'format', 'relationship'
    target_pattern VARCHAR(255) NOT NULL,  -- What to look for in DSL
    error_message TEXT,                     -- Error message if rule fails
    warning_message TEXT,                   -- Warning message if rule partially fails
    suggestion TEXT,                        -- Suggestion for improvement
    severity VARCHAR(20) DEFAULT 'error',  -- 'error', 'warning', 'info'
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Migrate validation rules from hardcoded mock validation
INSERT INTO "ob-poc".dsl_validation_rules (rule_type, target_pattern, error_message, severity) VALUES
('required', 'case.create', 'Missing required case.create block', 'error'),
('required', 'cbu.id', 'Missing required cbu.id in case.create', 'error'),
('format', 'CBU-[A-Z0-9]+', 'CBU ID should follow format CBU-XXXX', 'warning'),
('required', 'nature-purpose', 'Missing nature-purpose specification', 'error'),
('relationship', 'products.add.*kyc.start', 'KYC should be started after adding products', 'warning'),
('relationship', 'services.plan.*resources.plan', 'Resources should be planned after services', 'info');

-- Insert validation suggestions
INSERT INTO "ob-poc".dsl_validation_rules (rule_type, target_pattern, suggestion, severity) VALUES
('suggestion', 'products.add', 'Consider adding KYC requirements after product selection', 'info'),
('suggestion', 'kyc.start', 'Consider specifying document requirements explicitly', 'info'),
('suggestion', 'services.plan', 'Consider adding SLA specifications for services', 'info');

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_kyc_rules_entity_type ON "ob-poc".kyc_rules (entity_type);
CREATE INDEX IF NOT EXISTS idx_kyc_rules_jurisdiction ON "ob-poc".kyc_rules (jurisdiction);
CREATE INDEX IF NOT EXISTS idx_product_requirements_name ON "ob-poc".product_requirements (product_name);
CREATE INDEX IF NOT EXISTS idx_dsl_transformation_pattern ON "ob-poc".dsl_transformation_rules (instruction_pattern);
CREATE INDEX IF NOT EXISTS idx_dsl_validation_type ON "ob-poc".dsl_validation_rules (rule_type);
CREATE INDEX IF NOT EXISTS idx_dsl_validation_pattern ON "ob-poc".dsl_validation_rules (target_pattern);

-- Create views for easier querying
CREATE OR REPLACE VIEW "ob-poc".v_kyc_requirements AS
SELECT
    kr.entity_type,
    kr.jurisdiction,
    array_agg(DISTINCT doc) AS all_required_documents,
    array_agg(DISTINCT pr.required_documents) FILTER (WHERE pr.required_documents IS NOT NULL) AS product_documents
FROM "ob-poc".kyc_rules kr
LEFT JOIN "ob-poc".product_requirements pr ON TRUE
CROSS JOIN unnest(kr.required_documents) AS doc
GROUP BY kr.entity_type, kr.jurisdiction;

-- Success message
DO $$
BEGIN
    RAISE NOTICE 'Successfully migrated all mock data to database tables:';
    RAISE NOTICE '- CBUs: % records', (SELECT COUNT(*) FROM "ob-poc".cbus);
    RAISE NOTICE '- Products: % records', (SELECT COUNT(*) FROM "ob-poc".products);
    RAISE NOTICE '- Services: % records', (SELECT COUNT(*) FROM "ob-poc".services);
    RAISE NOTICE '- Entity Types: % records', (SELECT COUNT(*) FROM "ob-poc".entity_types);
    RAISE NOTICE '- Roles: % records', (SELECT COUNT(*) FROM "ob-poc".roles);
    RAISE NOTICE '- Entities: % records', (SELECT COUNT(*) FROM "ob-poc".entities);
    RAISE NOTICE '- DSL Records: % records', (SELECT COUNT(*) FROM "ob-poc".dsl_ob);
    RAISE NOTICE '- KYC Rules: % records', (SELECT COUNT(*) FROM "ob-poc".kyc_rules);
    RAISE NOTICE '- Product Requirements: % records', (SELECT COUNT(*) FROM "ob-poc".product_requirements);
    RAISE NOTICE '- DSL Transformation Rules: % records', (SELECT COUNT(*) FROM "ob-poc".dsl_transformation_rules);
    RAISE NOTICE '- DSL Validation Rules: % records', (SELECT COUNT(*) FROM "ob-poc".dsl_validation_rules);
    RAISE NOTICE '';
    RAISE NOTICE 'Mock data migration completed successfully!';
    RAISE NOTICE 'You can now disable mock interceptors and use database-driven operations.';
END $$;
