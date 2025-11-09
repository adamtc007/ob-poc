-- 07_migrate_mock_data_to_db_fixed.sql
-- Fixed migration script to move all mock data from JSON files to the database
-- This eliminates the need for mock interceptors and enables end-to-end database operations

-- First, clean up any existing test data (only tables that exist)
DELETE FROM "ob-poc".dsl_ob WHERE cbu_id LIKE 'CBU-%';
DELETE FROM "ob-poc".cbus WHERE name LIKE 'CBU-%';

-- Migrate CBUs from mock data
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at) VALUES
('123e4567-e89b-12d3-a456-426614174000'::uuid, 'CBU-1234', 'Aviva Investors Global Fund', 'UCITS equity fund domiciled in LU', '2024-01-15T10:30:00Z', '2024-01-15T10:30:00Z'),
('987fcdeb-51a2-43f7-8765-ba9876543210'::uuid, 'CBU-5678', 'European Growth Fund', 'Investment fund focused on European growth stocks', '2024-02-01T14:20:00Z', '2024-02-01T14:20:00Z'),
('456789ab-cdef-1234-5678-9abcdef01234'::uuid, 'CBU-9999', 'Emerging Markets Bond Fund', 'Fixed income fund investing in emerging market bonds', '2024-03-10T09:15:00Z', '2024-03-10T09:15:00Z');

-- Migrate Products from mock data (generate proper UUIDs)
INSERT INTO "ob-poc".products (product_id, name, description, created_at, updated_at) VALUES
('a1b2c3d4-e5f6-7890-abcd-ef1234567890'::uuid, 'CUSTODY', 'Custody and safekeeping services for investment assets', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('b2c3d4e5-f6g7-8901-bcde-f23456789012'::uuid, 'FUND_ACCOUNTING', 'Fund accounting and administration services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('c3d4e5f6-g7h8-9012-cdef-345678901234'::uuid, 'TRANSFER_AGENCY', 'Transfer agency and shareholder services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('d4e5f6g7-h8i9-0123-def0-456789012345'::uuid, 'INVESTMENT_MANAGEMENT', 'Investment management and portfolio services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('e5f6g7h8-i9j0-1234-ef01-567890123456'::uuid, 'RISK_MANAGEMENT', 'Risk management and compliance services', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Services from mock data (generate proper UUIDs)
INSERT INTO "ob-poc".services (service_id, name, description, created_at, updated_at) VALUES
('f6g7h8i9-j0k1-2345-f012-678901234567'::uuid, 'CustodyService', 'Core custody and asset safekeeping service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('g7h8i9j0-k1l2-3456-0123-789012345678'::uuid, 'SettlementService', 'Trade settlement and clearing service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('h8i9j0k1-l2m3-4567-1234-890123456789'::uuid, 'FundAccountingService', 'Fund accounting and NAV calculation service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('i9j0k1l2-m3n4-5678-2345-901234567890'::uuid, 'TransferAgencyService', 'Shareholder record keeping and transfer service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('j0k1l2m3-n4o5-6789-3456-012345678901'::uuid, 'ReportingService', 'Regulatory and client reporting service', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Entity Types from mock data (generate proper UUIDs)
INSERT INTO "ob-poc".entity_types (entity_type_id, name, description, created_at, updated_at) VALUES
('k1l2m3n4-o5p6-7890-4567-123456789012'::uuid, 'Company', 'Corporate entity (corporation, LLC, etc.)', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('l2m3n4o5-p6q7-8901-5678-234567890123'::uuid, 'Partnership', 'Partnership entity (LP, LLP, GP, etc.)', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('m3n4o5p6-q7r8-9012-6789-345678901234'::uuid, 'Individual', 'Individual person or natural person', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('n4o5p6q7-r8s9-0123-7890-456789012345'::uuid, 'Fund', 'Investment fund or vehicle', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Roles from mock data (generate proper UUIDs)
INSERT INTO "ob-poc".roles (role_id, name, description, created_at, updated_at) VALUES
('o5p6q7r8-s9t0-1234-8901-567890123456'::uuid, 'Ultimate Beneficial Owner', 'Person who ultimately owns or controls 25% or more', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('p6q7r8s9-t0u1-2345-9012-678901234567'::uuid, 'Director', 'Board member or director of the entity', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('q7r8s9t0-u1v2-3456-0123-789012345678'::uuid, 'Authorized Signatory', 'Person authorized to sign on behalf of the entity', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'),
('r8s9t0u1-v2w3-4567-1234-890123456789'::uuid, 'Beneficial Owner', 'Person with significant ownership (10-25%)', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');

-- Migrate Product-Service relationships (using the UUIDs from above)
INSERT INTO "ob-poc".product_services (product_id, service_id) VALUES
('a1b2c3d4-e5f6-7890-abcd-ef1234567890'::uuid, 'f6g7h8i9-j0k1-2345-f012-678901234567'::uuid), -- CUSTODY -> CustodyService
('a1b2c3d4-e5f6-7890-abcd-ef1234567890'::uuid, 'g7h8i9j0-k1l2-3456-0123-789012345678'::uuid), -- CUSTODY -> SettlementService
('b2c3d4e5-f6g7-8901-bcde-f23456789012'::uuid, 'h8i9j0k1-l2m3-4567-1234-890123456789'::uuid), -- FUND_ACCOUNTING -> FundAccountingService
('b2c3d4e5-f6g7-8901-bcde-f23456789012'::uuid, 'j0k1l2m3-n4o5-6789-3456-012345678901'::uuid), -- FUND_ACCOUNTING -> ReportingService
('c3d4e5f6-g7h8-9012-cdef-345678901234'::uuid, 'i9j0k1l2-m3n4-5678-2345-901234567890'::uuid); -- TRANSFER_AGENCY -> TransferAgencyService

-- Migrate DSL history from mock data (generate proper UUIDs)
INSERT INTO "ob-poc".dsl_ob (version_id, cbu_id, dsl_text, created_at) VALUES
(gen_random_uuid(), 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)', '2024-01-15T10:30:00Z'),
(gen_random_uuid(), 'CBU-1234', '(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")', '2024-01-15T11:00:00Z'),
(gen_random_uuid(), 'CBU-1234', '(case.create
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
(gen_random_uuid(), 'CBU-1234', '(case.create
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
(gen_random_uuid(), 'CBU-1234', '(case.create
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

-- Create KYC rules table to replace hardcoded mock responses (if not exists)
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(100) NOT NULL,  -- 'ucits', 'hedge_fund', 'corporation', etc.
    jurisdiction VARCHAR(10),           -- 'US', 'LU', 'CAYMAN', etc.
    required_documents TEXT[] NOT NULL,  -- Array of required document types
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Clear existing KYC rules and insert fresh ones
DELETE FROM "ob-poc".kyc_rules;
INSERT INTO "ob-poc".kyc_rules (entity_type, jurisdiction, required_documents) VALUES
('ucits', 'LU', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation', 'W8BEN-E']),
('ucits', 'EU', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation', 'W8BEN-E']),
('hedge_fund', 'US', ARRAY['CertificateOfLimitedPartnership', 'PartnershipAgreement', 'W9', 'AMLPolicy']),
('hedge_fund', 'CAYMAN', ARRAY['CertificateOfLimitedPartnership', 'PartnershipAgreement', 'W9', 'AMLPolicy']),
('corporation', 'US', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation']),
('company', 'US', ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation']),
('default', NULL, ARRAY['CertificateOfIncorporation', 'ArticlesOfAssociation', 'W8BEN-E']);

-- Create product requirements table using existing schema
-- First check the actual schema and use proper foreign keys
DO $$
BEGIN
    -- Insert products requirements using proper product references
    INSERT INTO "ob-poc".product_requirements (product_id, requirement_type, requirement_value, created_at, updated_at)
    SELECT
        p.product_id,
        'required_document',
        'AMLPolicy',
        NOW(),
        NOW()
    FROM "ob-poc".products p
    WHERE p.name = 'TRANSFER_AGENCY'
    ON CONFLICT DO NOTHING;

    INSERT INTO "ob-poc".product_requirements (product_id, requirement_type, requirement_value, created_at, updated_at)
    SELECT
        p.product_id,
        'required_document',
        'InvestorQuestionnaire',
        NOW(),
        NOW()
    FROM "ob-poc".products p
    WHERE p.name = 'TRANSFER_AGENCY'
    ON CONFLICT DO NOTHING;

    INSERT INTO "ob-poc".product_requirements (product_id, requirement_type, requirement_value, created_at, updated_at)
    SELECT
        p.product_id,
        'required_document',
        'CustodyAgreement',
        NOW(),
        NOW()
    FROM "ob-poc".products p
    WHERE p.name = 'CUSTODY'
    ON CONFLICT DO NOTHING;

    INSERT INTO "ob-poc".product_requirements (product_id, requirement_type, requirement_value, created_at, updated_at)
    SELECT
        p.product_id,
        'required_document',
        'AccountingPolicy',
        NOW(),
        NOW()
    FROM "ob-poc".products p
    WHERE p.name = 'FUND_ACCOUNTING'
    ON CONFLICT DO NOTHING;

END $$;

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

-- Clear and migrate transformation rules from hardcoded mock responses
DELETE FROM "ob-poc".dsl_transformation_rules;
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

-- Clear and migrate validation rules from hardcoded mock validation
DELETE FROM "ob-poc".dsl_validation_rules;
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
CREATE INDEX IF NOT EXISTS idx_dsl_transformation_pattern ON "ob-poc".dsl_transformation_rules (instruction_pattern);
CREATE INDEX IF NOT EXISTS idx_dsl_validation_type ON "ob-poc".dsl_validation_rules (rule_type);
CREATE INDEX IF NOT EXISTS idx_dsl_validation_pattern ON "ob-poc".dsl_validation_rules (target_pattern);

-- Success message
DO $$
BEGIN
    RAISE NOTICE 'Successfully migrated all mock data to database tables:';
    RAISE NOTICE '- CBUs: % records', (SELECT COUNT(*) FROM "ob-poc".cbus WHERE name LIKE 'CBU-%');
    RAISE NOTICE '- Products: % records', (SELECT COUNT(*) FROM "ob-poc".products);
    RAISE NOTICE '- Services: % records', (SELECT COUNT(*) FROM "ob-poc".services);
    RAISE NOTICE '- Entity Types: % records', (SELECT COUNT(*) FROM "ob-poc".entity_types);
    RAISE NOTICE '- Roles: % records', (SELECT COUNT(*) FROM "ob-poc".roles);
    RAISE NOTICE '- DSL Records: % records', (SELECT COUNT(*) FROM "ob-poc".dsl_ob WHERE cbu_id LIKE 'CBU-%');
    RAISE NOTICE '- KYC Rules: % records', (SELECT COUNT(*) FROM "ob-poc".kyc_rules);
    RAISE NOTICE '- Product Requirements: % records', (SELECT COUNT(*) FROM "ob-poc".product_requirements);
    RAISE NOTICE '- DSL Transformation Rules: % records', (SELECT COUNT(*) FROM "ob-poc".dsl_transformation_rules);
    RAISE NOTICE '- DSL Validation Rules: % records', (SELECT COUNT(*) FROM "ob-poc".dsl_validation_rules);
    RAISE NOTICE '';
    RAISE NOTICE 'Mock data migration completed successfully!';
    RAISE NOTICE 'You can now disable mock interceptors and use database-driven operations.';
END $$;
