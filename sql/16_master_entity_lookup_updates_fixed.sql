-- 16_master_entity_lookup_updates_fixed.sql
-- Master Entity Lookup Tables Updates for Comprehensive Entity CRUD Support (Fixed)
--
-- This script updates the master entity lookup and reference tables to support
-- comprehensive agentic CRUD operations across all entity types:
-- - entity_partnerships
-- - entity_limited_companies
-- - entity_proper_persons
-- - entity_trusts
--
-- Compatible with existing schema structure

-- ============================================================================
-- UPDATE EXISTING ENTITY TYPES TABLE
-- ============================================================================

-- Update existing entity_types table with comprehensive entity data
INSERT INTO "ob-poc".entity_types (name, description, table_name) VALUES
('PARTNERSHIP_GENERAL', 'General Partnership where all partners have unlimited liability', 'entity_partnerships'),
('PARTNERSHIP_LIMITED', 'Limited Partnership with general and limited partners', 'entity_partnerships'),
('PARTNERSHIP_LLP', 'Limited Liability Partnership with limited liability for all partners', 'entity_partnerships'),
('LIMITED_COMPANY_PRIVATE', 'Private Limited Company limited by shares', 'entity_limited_companies'),
('LIMITED_COMPANY_PUBLIC', 'Public Limited Company limited by shares', 'entity_limited_companies'),
('LIMITED_COMPANY_UNLIMITED', 'Unlimited Company with unlimited liability', 'entity_limited_companies'),
('PROPER_PERSON_NATURAL', 'Natural Person - Individual human being', 'entity_proper_persons'),
('PROPER_PERSON_BENEFICIAL_OWNER', 'Beneficial Owner - Person who ultimately owns or controls an entity', 'entity_proper_persons'),
('TRUST_DISCRETIONARY', 'Discretionary Trust where trustees have discretion over distributions', 'entity_trusts'),
('TRUST_FIXED_INTEREST', 'Fixed Interest Trust with fixed beneficial interests', 'entity_trusts'),
('TRUST_UNIT', 'Unit Trust - Investment trust divided into units', 'entity_trusts'),
('TRUST_CHARITABLE', 'Charitable Trust established for charitable purposes', 'entity_trusts')
ON CONFLICT (name) DO UPDATE SET
    description = EXCLUDED.description,
    table_name = EXCLUDED.table_name,
    updated_at = NOW();

-- ============================================================================
-- MASTER JURISDICTION REFERENCE TABLE
-- ============================================================================

-- Create comprehensive jurisdiction lookup table for entity CRUD operations
CREATE TABLE IF NOT EXISTS "ob-poc".master_jurisdictions (
    jurisdiction_code VARCHAR(10) PRIMARY KEY,
    jurisdiction_name VARCHAR(200) NOT NULL,
    country_code VARCHAR(3) NOT NULL,
    region VARCHAR(100),
    regulatory_framework VARCHAR(100),
    entity_formation_allowed BOOLEAN DEFAULT TRUE,
    offshore_jurisdiction BOOLEAN DEFAULT FALSE,
    regulatory_authority VARCHAR(300),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed comprehensive jurisdiction data
INSERT INTO "ob-poc".master_jurisdictions (
    jurisdiction_code, jurisdiction_name, country_code, region, regulatory_framework,
    entity_formation_allowed, offshore_jurisdiction, regulatory_authority
) VALUES
-- Major Onshore Jurisdictions
('US', 'United States', 'USA', 'North America', 'Federal + State', TRUE, FALSE, 'SEC, State Regulators'),
('US-DE', 'Delaware, United States', 'USA', 'North America', 'Delaware Corporate Law', TRUE, FALSE, 'Delaware Division of Corporations'),
('US-NY', 'New York, United States', 'USA', 'North America', 'New York State Law', TRUE, FALSE, 'New York Department of State'),
('GB', 'United Kingdom', 'GBR', 'Europe', 'UK Company Law', TRUE, FALSE, 'Companies House'),
('CA', 'Canada', 'CAN', 'North America', 'Canadian Corporate Law', TRUE, FALSE, 'Corporations Canada'),
('AU', 'Australia', 'AUS', 'Oceania', 'Australian Corporate Law', TRUE, FALSE, 'ASIC'),
('FR', 'France', 'FRA', 'Europe', 'French Commercial Code', TRUE, FALSE, 'INPI'),
('DE', 'Germany', 'DEU', 'Europe', 'German Corporate Law', TRUE, FALSE, 'Federal Ministry of Justice'),
('CH', 'Switzerland', 'CHE', 'Europe', 'Swiss Corporate Law', TRUE, FALSE, 'Swiss Commercial Register'),
('SG', 'Singapore', 'SGP', 'Asia', 'Singapore Companies Act', TRUE, FALSE, 'ACRA'),
('HK', 'Hong Kong', 'HKG', 'Asia', 'Hong Kong Companies Ordinance', TRUE, FALSE, 'Companies Registry'),

-- Major Offshore Jurisdictions
('KY', 'Cayman Islands', 'CYM', 'Caribbean', 'Cayman Islands Companies Act', TRUE, TRUE, 'Cayman Islands Monetary Authority'),
('BVI', 'British Virgin Islands', 'VGB', 'Caribbean', 'BVI Business Companies Act', TRUE, TRUE, 'BVI Financial Services Commission'),
('BS', 'Bahamas', 'BHS', 'Caribbean', 'Bahamas Companies Act', TRUE, TRUE, 'Securities Commission of The Bahamas'),
('BM', 'Bermuda', 'BMU', 'Atlantic', 'Bermuda Companies Act', TRUE, TRUE, 'Bermuda Monetary Authority'),
('JE', 'Jersey', 'JEY', 'Europe', 'Jersey Companies Law', TRUE, TRUE, 'Jersey Financial Services Commission'),
('GG', 'Guernsey', 'GGY', 'Europe', 'Guernsey Companies Law', TRUE, TRUE, 'Guernsey Financial Services Commission'),
('IM', 'Isle of Man', 'IMN', 'Europe', 'Isle of Man Companies Act', TRUE, TRUE, 'Isle of Man Financial Services Authority'),
('LU', 'Luxembourg', 'LUX', 'Europe', 'Luxembourg Company Law', TRUE, TRUE, 'Commission de Surveillance du Secteur Financier'),
('MT', 'Malta', 'MLT', 'Europe', 'Malta Companies Act', TRUE, TRUE, 'Malta Financial Services Authority'),
('CY', 'Cyprus', 'CYP', 'Europe', 'Cyprus Companies Law', TRUE, TRUE, 'Cyprus Securities and Exchange Commission'),

-- Emerging Jurisdictions
('AE', 'United Arab Emirates', 'ARE', 'Middle East', 'UAE Commercial Companies Law', TRUE, FALSE, 'Securities and Commodities Authority'),
('QA', 'Qatar', 'QAT', 'Middle East', 'Qatar Commercial Companies Law', TRUE, FALSE, 'Qatar Financial Markets Authority')
ON CONFLICT (jurisdiction_code) DO UPDATE SET
    jurisdiction_name = EXCLUDED.jurisdiction_name,
    country_code = EXCLUDED.country_code,
    region = EXCLUDED.region,
    regulatory_framework = EXCLUDED.regulatory_framework,
    entity_formation_allowed = EXCLUDED.entity_formation_allowed,
    offshore_jurisdiction = EXCLUDED.offshore_jurisdiction,
    regulatory_authority = EXCLUDED.regulatory_authority,
    updated_at = NOW();

-- ============================================================================
-- ENTITY CROSS-REFERENCE TABLE
-- ============================================================================

-- Create master entity cross-reference table for linking entities across types
CREATE TABLE IF NOT EXISTS "ob-poc".master_entity_xref (
    xref_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL CHECK (entity_type IN ('PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 'TRUST')),
    entity_id UUID NOT NULL,
    entity_name VARCHAR(500) NOT NULL,
    jurisdiction_code VARCHAR(10) REFERENCES "ob-poc".master_jurisdictions(jurisdiction_code),
    entity_status VARCHAR(50) DEFAULT 'ACTIVE' CHECK (entity_status IN ('ACTIVE', 'INACTIVE', 'DISSOLVED', 'SUSPENDED')),
    business_purpose TEXT,
    primary_contact_person UUID,
    regulatory_numbers JSONB DEFAULT '{}'::jsonb,
    additional_metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create indexes for efficient entity lookup
CREATE INDEX IF NOT EXISTS idx_master_entity_xref_type ON "ob-poc".master_entity_xref(entity_type);
CREATE INDEX IF NOT EXISTS idx_master_entity_xref_entity_id ON "ob-poc".master_entity_xref(entity_id);
CREATE INDEX IF NOT EXISTS idx_master_entity_xref_jurisdiction ON "ob-poc".master_entity_xref(jurisdiction_code);
CREATE INDEX IF NOT EXISTS idx_master_entity_xref_status ON "ob-poc".master_entity_xref(entity_status);
CREATE INDEX IF NOT EXISTS idx_master_entity_xref_name ON "ob-poc".master_entity_xref USING gin(to_tsvector('english', entity_name));

-- ============================================================================
-- ENTITY LIFECYCLE STATUS TABLE
-- ============================================================================

-- Create table to track entity lifecycle states for workflow management
CREATE TABLE IF NOT EXISTS "ob-poc".entity_lifecycle_status (
    status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    status_code VARCHAR(50) NOT NULL,
    status_description VARCHAR(200),
    effective_date DATE NOT NULL,
    end_date DATE,
    reason_code VARCHAR(100),
    notes TEXT,
    created_by VARCHAR(100) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_type, entity_id, status_code, effective_date)
);

-- Create indexes for lifecycle tracking
CREATE INDEX IF NOT EXISTS idx_entity_lifecycle_type_id ON "ob-poc".entity_lifecycle_status(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_lifecycle_status ON "ob-poc".entity_lifecycle_status(status_code);
CREATE INDEX IF NOT EXISTS idx_entity_lifecycle_effective ON "ob-poc".entity_lifecycle_status(effective_date);

-- ============================================================================
-- ENTITY VALIDATION RULES TABLE
-- ============================================================================

-- Create table for entity-specific validation rules used by agentic CRUD
CREATE TABLE IF NOT EXISTS "ob-poc".entity_validation_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    field_name VARCHAR(100) NOT NULL,
    validation_type VARCHAR(50) NOT NULL CHECK (validation_type IN ('REQUIRED', 'FORMAT', 'RANGE', 'REFERENCE', 'CUSTOM')),
    validation_rule JSONB NOT NULL,
    error_message VARCHAR(500),
    severity VARCHAR(20) DEFAULT 'ERROR' CHECK (severity IN ('ERROR', 'WARNING', 'INFO')),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed comprehensive validation rules
INSERT INTO "ob-poc".entity_validation_rules (
    entity_type, field_name, validation_type, validation_rule, error_message, severity
) VALUES
-- Partnership validation rules
('PARTNERSHIP', 'partnership_name', 'REQUIRED', '{"required": true}', 'Partnership name is required', 'ERROR'),
('PARTNERSHIP', 'partnership_type', 'FORMAT', '{"pattern": "^(General|Limited|Limited Liability)$"}', 'Partnership type must be General, Limited, or Limited Liability', 'ERROR'),
('PARTNERSHIP', 'jurisdiction', 'REFERENCE', '{"table": "master_jurisdictions", "column": "jurisdiction_code"}', 'Invalid jurisdiction code', 'ERROR'),
('PARTNERSHIP', 'formation_date', 'RANGE', '{"min_date": "1800-01-01", "max_date": "today"}', 'Formation date must be between 1800 and today', 'ERROR'),

-- Limited Company validation rules
('LIMITED_COMPANY', 'company_name', 'REQUIRED', '{"required": true}', 'Company name is required', 'ERROR'),
('LIMITED_COMPANY', 'registration_number', 'FORMAT', '{"min_length": 3, "max_length": 20}', 'Registration number must be 3-20 characters', 'WARNING'),
('LIMITED_COMPANY', 'jurisdiction', 'REFERENCE', '{"table": "master_jurisdictions", "column": "jurisdiction_code"}', 'Invalid jurisdiction code', 'ERROR'),
('LIMITED_COMPANY', 'incorporation_date', 'RANGE', '{"min_date": "1800-01-01", "max_date": "today"}', 'Incorporation date must be between 1800 and today', 'ERROR'),

-- Proper Person validation rules
('PROPER_PERSON', 'first_name', 'REQUIRED', '{"required": true}', 'First name is required', 'ERROR'),
('PROPER_PERSON', 'last_name', 'REQUIRED', '{"required": true}', 'Last name is required', 'ERROR'),
('PROPER_PERSON', 'date_of_birth', 'RANGE', '{"min_date": "1900-01-01", "max_date": "today"}', 'Date of birth must be between 1900 and today', 'ERROR'),
('PROPER_PERSON', 'nationality', 'REFERENCE', '{"table": "master_jurisdictions", "column": "country_code"}', 'Invalid nationality code', 'WARNING'),
('PROPER_PERSON', 'id_document_type', 'FORMAT', '{"pattern": "^(Passport|National ID|Driving License)$"}', 'ID document type must be Passport, National ID, or Driving License', 'WARNING'),

-- Trust validation rules
('TRUST', 'trust_name', 'REQUIRED', '{"required": true}', 'Trust name is required', 'ERROR'),
('TRUST', 'trust_type', 'FORMAT', '{"pattern": "^(Discretionary|Fixed Interest|Unit Trust|Charitable)$"}', 'Trust type must be Discretionary, Fixed Interest, Unit Trust, or Charitable', 'ERROR'),
('TRUST', 'jurisdiction', 'REQUIRED', '{"required": true}', 'Trust jurisdiction is required', 'ERROR'),
('TRUST', 'establishment_date', 'RANGE', '{"min_date": "1800-01-01", "max_date": "today"}', 'Establishment date must be between 1800 and today', 'ERROR')
ON CONFLICT DO NOTHING;

-- Create indexes for validation rules
CREATE INDEX IF NOT EXISTS idx_entity_validation_type ON "ob-poc".entity_validation_rules(entity_type);
CREATE INDEX IF NOT EXISTS idx_entity_validation_field ON "ob-poc".entity_validation_rules(field_name);
CREATE INDEX IF NOT EXISTS idx_entity_validation_active ON "ob-poc".entity_validation_rules(is_active);

-- ============================================================================
-- ENTITY METADATA ENRICHMENT TABLE
-- ============================================================================

-- Create table for additional entity metadata that doesn't fit in main tables
CREATE TABLE IF NOT EXISTS "ob-poc".entity_metadata (
    metadata_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    metadata_key VARCHAR(100) NOT NULL,
    metadata_value JSONB,
    metadata_source VARCHAR(100),
    confidence_score DECIMAL(3,2),
    last_updated TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_type, entity_id, metadata_key)
);

-- Create indexes for metadata lookup
CREATE INDEX IF NOT EXISTS idx_entity_metadata_type_id ON "ob-poc".entity_metadata(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_metadata_key ON "ob-poc".entity_metadata(metadata_key);
CREATE INDEX IF NOT EXISTS idx_entity_metadata_source ON "ob-poc".entity_metadata(metadata_source);

-- ============================================================================
-- ENTITY ATTRIBUTE DICTIONARY ENTRIES
-- ============================================================================

-- Add entity-specific attributes to dictionary for DSL integration
INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain, source, sink, created_at, updated_at) VALUES

-- Partnership attributes
('e0000001-0000-0000-0000-000000000001', 'entity.partnership.name', 'Partnership legal name', 'Partnership', 'string', 'Entity', '{"type": "manual", "required": true}', '{"type": "database", "table": "entity_partnerships"}', NOW(), NOW()),
('e0000001-0000-0000-0000-000000000002', 'entity.partnership.type', 'Type of partnership (General, Limited, LLP)', 'Partnership', 'string', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_partnerships"}', NOW(), NOW()),
('e0000001-0000-0000-0000-000000000003', 'entity.partnership.jurisdiction', 'Jurisdiction of partnership formation', 'Partnership', 'string', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_partnerships"}', NOW(), NOW()),
('e0000001-0000-0000-0000-000000000004', 'entity.partnership.formation_date', 'Date of partnership formation', 'Partnership', 'date', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_partnerships"}', NOW(), NOW()),

-- Limited Company attributes
('e0000002-0000-0000-0000-000000000001', 'entity.company.name', 'Company legal name', 'Company', 'string', 'Entity', '{"type": "manual", "required": true}', '{"type": "database", "table": "entity_limited_companies"}', NOW(), NOW()),
('e0000002-0000-0000-0000-000000000002', 'entity.company.registration_number', 'Company registration number', 'Company', 'string', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_limited_companies"}', NOW(), NOW()),
('e0000002-0000-0000-0000-000000000003', 'entity.company.jurisdiction', 'Jurisdiction of incorporation', 'Company', 'string', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_limited_companies"}', NOW(), NOW()),
('e0000002-0000-0000-0000-000000000004', 'entity.company.incorporation_date', 'Date of incorporation', 'Company', 'date', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_limited_companies"}', NOW(), NOW()),

-- Proper Person attributes
('e0000003-0000-0000-0000-000000000001', 'entity.person.first_name', 'Person first name', 'Person', 'string', 'Entity', '{"type": "manual", "required": true, "pii": true}', '{"type": "database", "table": "entity_proper_persons"}', NOW(), NOW()),
('e0000003-0000-0000-0000-000000000002', 'entity.person.last_name', 'Person last name', 'Person', 'string', 'Entity', '{"type": "manual", "required": true, "pii": true}', '{"type": "database", "table": "entity_proper_persons"}', NOW(), NOW()),
('e0000003-0000-0000-0000-000000000003', 'entity.person.date_of_birth', 'Person date of birth', 'Person', 'date', 'Entity', '{"type": "manual", "required": false, "pii": true}', '{"type": "database", "table": "entity_proper_persons"}', NOW(), NOW()),
('e0000003-0000-0000-0000-000000000004', 'entity.person.nationality', 'Person nationality', 'Person', 'string', 'Entity', '{"type": "manual", "required": false, "pii": true}', '{"type": "database", "table": "entity_proper_persons"}', NOW(), NOW()),

-- Trust attributes
('e0000004-0000-0000-0000-000000000001', 'entity.trust.name', 'Trust legal name', 'Trust', 'string', 'Entity', '{"type": "manual", "required": true}', '{"type": "database", "table": "entity_trusts"}', NOW(), NOW()),
('e0000004-0000-0000-0000-000000000002', 'entity.trust.type', 'Type of trust (Discretionary, Fixed Interest, etc)', 'Trust', 'string', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_trusts"}', NOW(), NOW()),
('e0000004-0000-0000-0000-000000000003', 'entity.trust.jurisdiction', 'Trust jurisdiction', 'Trust', 'string', 'Entity', '{"type": "manual", "required": true}', '{"type": "database", "table": "entity_trusts"}', NOW(), NOW()),
('e0000004-0000-0000-0000-000000000004', 'entity.trust.establishment_date', 'Date of trust establishment', 'Trust', 'date', 'Entity', '{"type": "manual", "required": false}', '{"type": "database", "table": "entity_trusts"}', NOW(), NOW())

ON CONFLICT (attribute_id) DO UPDATE SET
    name = EXCLUDED.name,
    long_description = EXCLUDED.long_description,
    group_id = EXCLUDED.group_id,
    mask = EXCLUDED.mask,
    domain = EXCLUDED.domain,
    source = EXCLUDED.source,
    sink = EXCLUDED.sink,
    updated_at = NOW();

-- ============================================================================
-- COMMENTS AND DOCUMENTATION
-- ============================================================================

COMMENT ON TABLE "ob-poc".master_jurisdictions IS 'Comprehensive jurisdiction lookup table for entity formation and compliance';
COMMENT ON TABLE "ob-poc".master_entity_xref IS 'Master cross-reference table linking all entity types with unified metadata';
COMMENT ON TABLE "ob-poc".entity_lifecycle_status IS 'Tracks entity lifecycle states for workflow management';
COMMENT ON TABLE "ob-poc".entity_validation_rules IS 'Defines validation rules for agentic CRUD operations';
COMMENT ON TABLE "ob-poc".entity_metadata IS 'Stores additional entity metadata that doesnt fit in main entity tables';

COMMENT ON COLUMN "ob-poc".master_jurisdictions.offshore_jurisdiction IS 'TRUE for offshore/tax haven jurisdictions';
COMMENT ON COLUMN "ob-poc".master_entity_xref.regulatory_numbers IS 'JSON object storing various regulatory identification numbers';
COMMENT ON COLUMN "ob-poc".entity_validation_rules.validation_rule IS 'JSON object defining the validation logic';
COMMENT ON COLUMN "ob-poc".entity_metadata.confidence_score IS 'Confidence score for metadata value (0.0 to 1.0)';

\echo '✅ Master entity lookup tables updated for comprehensive entity CRUD support'
\echo '   - Updated entity_types with 12 detailed entity classifications'
\echo '   - Added master_jurisdictions with 23 major onshore/offshore jurisdictions'
\echo '   - Created master_entity_xref for cross-entity linking'
\echo '   - Implemented entity_lifecycle_status for workflow tracking'
\echo '   - Created entity_validation_rules for agentic CRUD validation'
\echo '   - Added entity_metadata for flexible metadata storage'
\echo '   - Added entity-specific AttributeIDs to dictionary'
\echo '   - All tables indexed for optimal performance'
