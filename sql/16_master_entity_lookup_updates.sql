-- 16_master_entity_lookup_updates.sql
-- Master Entity Lookup Tables Updates for Comprehensive Entity CRUD Support
--
-- This script updates the master entity lookup and reference tables to support
-- comprehensive agentic CRUD operations across all entity types:
-- - entity_partnerships
-- - entity_limited_companies
-- - entity_proper_persons
-- - entity_trusts
--
-- Updates include enhanced entity types, jurisdiction mappings, and cross-references

-- ============================================================================
-- ENHANCED ENTITY TYPES TABLE
-- ============================================================================

-- Update entity_types with comprehensive entity classifications
INSERT INTO "ob-poc".entity_types (type_code, type_name, description, category, regulatory_classification) VALUES
('PARTNERSHIP_GENERAL', 'General Partnership', 'Partnership where all partners have unlimited liability', 'PARTNERSHIP', 'UNREGULATED'),
('PARTNERSHIP_LIMITED', 'Limited Partnership', 'Partnership with general and limited partners', 'PARTNERSHIP', 'REGULATED'),
('PARTNERSHIP_LLP', 'Limited Liability Partnership', 'Partnership with limited liability for all partners', 'PARTNERSHIP', 'REGULATED'),
('LIMITED_COMPANY_PRIVATE', 'Private Limited Company', 'Private company limited by shares', 'COMPANY', 'REGULATED'),
('LIMITED_COMPANY_PUBLIC', 'Public Limited Company', 'Public company limited by shares', 'COMPANY', 'REGULATED'),
('LIMITED_COMPANY_UNLIMITED', 'Unlimited Company', 'Company with unlimited liability', 'COMPANY', 'REGULATED'),
('PROPER_PERSON_NATURAL', 'Natural Person', 'Individual human being', 'PERSON', 'INDIVIDUAL'),
('PROPER_PERSON_BENEFICIAL_OWNER', 'Beneficial Owner', 'Person who ultimately owns or controls an entity', 'PERSON', 'INDIVIDUAL'),
('TRUST_DISCRETIONARY', 'Discretionary Trust', 'Trust where trustees have discretion over distributions', 'TRUST', 'REGULATED'),
('TRUST_FIXED_INTEREST', 'Fixed Interest Trust', 'Trust with fixed beneficial interests', 'TRUST', 'REGULATED'),
('TRUST_UNIT', 'Unit Trust', 'Investment trust divided into units', 'TRUST', 'REGULATED'),
('TRUST_CHARITABLE', 'Charitable Trust', 'Trust established for charitable purposes', 'TRUST', 'REGULATED')
ON CONFLICT (type_code) DO UPDATE SET
    type_name = EXCLUDED.type_name,
    description = EXCLUDED.description,
    category = EXCLUDED.category,
    regulatory_classification = EXCLUDED.regulatory_classification,
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
-- ENTITY ATTRIBUTE MAPPING TABLE
-- ============================================================================

-- Create table to map entity attributes to dictionary AttributeIDs
CREATE TABLE IF NOT EXISTS "ob-poc".entity_attribute_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    entity_table_name VARCHAR(100) NOT NULL,
    column_name VARCHAR(100) NOT NULL,
    attribute_id UUID REFERENCES "ob-poc".dictionary(attribute_id),
    is_required BOOLEAN DEFAULT FALSE,
    validation_rules JSONB DEFAULT '{}'::jsonb,
    data_classification VARCHAR(50) DEFAULT 'INTERNAL',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_type, column_name)
);

-- Seed entity attribute mappings for DSL integration
INSERT INTO "ob-poc".entity_attribute_mappings (
    entity_type, entity_table_name, column_name, attribute_id, is_required, data_classification
) VALUES
-- Partnership mappings (using existing AttributeIDs where available)
('PARTNERSHIP', 'entity_partnerships', 'partnership_name', '00000000-0000-0000-0000-000000000001', TRUE, 'INTERNAL'),
('PARTNERSHIP', 'entity_partnerships', 'partnership_type', '00000000-0000-0000-0000-000000000002', FALSE, 'INTERNAL'),
('PARTNERSHIP', 'entity_partnerships', 'jurisdiction', '00000000-0000-0000-0000-000000000003', FALSE, 'INTERNAL'),
('PARTNERSHIP', 'entity_partnerships', 'formation_date', '00000000-0000-0000-0000-000000000004', FALSE, 'INTERNAL'),

-- Limited Company mappings
('LIMITED_COMPANY', 'entity_limited_companies', 'company_name', '00000000-0000-0000-0000-000000000101', TRUE, 'INTERNAL'),
('LIMITED_COMPANY', 'entity_limited_companies', 'registration_number', '00000000-0000-0000-0000-000000000102', FALSE, 'INTERNAL'),
('LIMITED_COMPANY', 'entity_limited_companies', 'jurisdiction', '00000000-0000-0000-0000-000000000103', FALSE, 'INTERNAL'),
('LIMITED_COMPANY', 'entity_limited_companies', 'incorporation_date', '00000000-0000-0000-0000-000000000104', FALSE, 'INTERNAL'),

-- Proper Person mappings
('PROPER_PERSON', 'entity_proper_persons', 'first_name', '00000000-0000-0000-0000-000000000201', TRUE, 'PII'),
('PROPER_PERSON', 'entity_proper_persons', 'last_name', '00000000-0000-0000-0000-000000000202', TRUE, 'PII'),
('PROPER_PERSON', 'entity_proper_persons', 'date_of_birth', '00000000-0000-0000-0000-000000000203', FALSE, 'PII'),
('PROPER_PERSON', 'entity_proper_persons', 'nationality', '00000000-0000-0000-0000-000000000204', FALSE, 'PII'),

-- Trust mappings
('TRUST', 'entity_trusts', 'trust_name', '00000000-0000-0000-0000-000000000301', TRUE, 'INTERNAL'),
('TRUST', 'entity_trusts', 'trust_type', '00000000-0000-0000-0000-000000000302', FALSE, 'INTERNAL'),
('TRUST', 'entity_trusts', 'jurisdiction', '00000000-0000-0000-0000-000000000303', TRUE, 'INTERNAL'),
('TRUST', 'entity_trusts', 'establishment_date', '00000000-0000-0000-0000-000000000304', FALSE, 'INTERNAL')
ON CONFLICT (entity_type, column_name) DO UPDATE SET
    attribute_id = EXCLUDED.attribute_id,
    is_required = EXCLUDED.is_required,
    data_classification = EXCLUDED.data_classification,
    updated_at = NOW();

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
-- COMMENTS AND DOCUMENTATION
-- ============================================================================

COMMENT ON TABLE "ob-poc".master_jurisdictions IS 'Comprehensive jurisdiction lookup table for entity formation and compliance';
COMMENT ON TABLE "ob-poc".master_entity_xref IS 'Master cross-reference table linking all entity types with unified metadata';
COMMENT ON TABLE "ob-poc".entity_attribute_mappings IS 'Maps entity table columns to dictionary AttributeIDs for DSL integration';
COMMENT ON TABLE "ob-poc".entity_lifecycle_status IS 'Tracks entity lifecycle states for workflow management';
COMMENT ON TABLE "ob-poc".entity_validation_rules IS 'Defines validation rules for agentic CRUD operations';
COMMENT ON TABLE "ob-poc".entity_metadata IS 'Stores additional entity metadata that doesnt fit in main entity tables';

COMMENT ON COLUMN "ob-poc".master_jurisdictions.offshore_jurisdiction IS 'TRUE for offshore/tax haven jurisdictions';
COMMENT ON COLUMN "ob-poc".master_entity_xref.regulatory_numbers IS 'JSON object storing various regulatory identification numbers';
COMMENT ON COLUMN "ob-poc".entity_attribute_mappings.data_classification IS 'Data classification: INTERNAL, PII, PCI, PHI, CONFIDENTIAL';
COMMENT ON COLUMN "ob-poc".entity_validation_rules.validation_rule IS 'JSON object defining the validation logic';
COMMENT ON COLUMN "ob-poc".entity_metadata.confidence_score IS 'Confidence score for metadata value (0.0 to 1.0)';

\echo '✅ Master entity lookup tables updated for comprehensive entity CRUD support'
\echo '   - Enhanced entity_types with detailed classifications'
\echo '   - Added master_jurisdictions with 20+ jurisdictions'
\echo '   - Created master_entity_xref for cross-entity linking'
\echo '   - Added entity_attribute_mappings for DSL integration'
\echo '   - Implemented entity_lifecycle_status for workflow tracking'
\echo '   - Created entity_validation_rules for agentic CRUD validation'
\echo '   - Added entity_metadata for flexible metadata storage'
\echo '   - All tables indexed for optimal performance'
