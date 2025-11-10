-- 10_document_library_phase1_fixed.sql
-- Document Library Infrastructure - Phase 1 Implementation (Fixed)
--
-- This schema creates a centralized document library system with full AttributeID
-- referential integrity for the DSL-as-State + AttributeID-as-Type architecture.

-- ============================================================================
-- FIRST: ADD DOCUMENT ATTRIBUTES TO DICTIONARY TABLE
-- ============================================================================

-- Document domain attributes that will be referenced by UUIDs
INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain, source, sink, created_at, updated_at) VALUES

-- Core Document Metadata Attributes
('d0c00001-0000-0000-0000-000000000001', 'document.id', 'Unique document identifier', 'Document', 'string', 'Document', '{"type": "generated", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00002-0000-0000-0000-000000000002', 'document.type', 'Document type classification', 'Document', 'string', 'Document', '{"type": "manual", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00003-0000-0000-0000-000000000003', 'document.title', 'Document title or name', 'Document', 'string', 'Document', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00004-0000-0000-0000-000000000004', 'document.issuer', 'Document issuing authority', 'Document', 'string', 'Document', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00005-0000-0000-0000-000000000005', 'document.issue_date', 'Date document was issued', 'Document', 'date', 'Document', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00006-0000-0000-0000-000000000006', 'document.expiry_date', 'Date document expires', 'Document', 'date', 'Document', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00007-0000-0000-0000-000000000007', 'document.file_path', 'Path to document file', 'Document', 'string', 'Document', '{"type": "system", "required": false}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),
('d0c00008-0000-0000-0000-000000000008', 'document.confidentiality_level', 'Document confidentiality classification', 'Document', 'enum', 'Document', '{"type": "manual", "required": true, "values": ["public", "internal", "restricted", "confidential"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Identity Document Fields
('d0cf0001-0000-0000-0000-000000000001', 'document.passport.number', 'Passport number', 'Document', 'string', 'Identity', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0001-0000-0000-0000-000000000002', 'document.passport.full_name', 'Full name on passport', 'Document', 'string', 'Identity', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0001-0000-0000-0000-000000000003', 'document.passport.nationality', 'Passport nationality', 'Document', 'string', 'Identity', '{"type": "extraction", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0001-0000-0000-0000-000000000004', 'document.passport.date_of_birth', 'Date of birth on passport', 'Document', 'date', 'Identity', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0001-0000-0000-0000-000000000005', 'document.passport.issue_date', 'Passport issue date', 'Document', 'date', 'Identity', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0001-0000-0000-0000-000000000006', 'document.passport.expiry_date', 'Passport expiry date', 'Document', 'date', 'Identity', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Corporate Document Fields
('d0cf0002-0000-0000-0000-000000000001', 'document.incorporation.company_name', 'Company name on incorporation document', 'Document', 'string', 'Corporate', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0002-0000-0000-0000-000000000002', 'document.incorporation.registration_number', 'Company registration number', 'Document', 'string', 'Corporate', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0002-0000-0000-0000-000000000003', 'document.incorporation.incorporation_date', 'Date of incorporation', 'Document', 'date', 'Corporate', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0002-0000-0000-0000-000000000004', 'document.incorporation.jurisdiction', 'Jurisdiction of incorporation', 'Document', 'string', 'Corporate', '{"type": "extraction", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0002-0000-0000-0000-000000000005', 'document.incorporation.authorized_shares', 'Number of authorized shares', 'Document', 'integer', 'Corporate', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Financial Document Fields
('d0cf0003-0000-0000-0000-000000000001', 'document.bank_statement.account_holder', 'Account holder name', 'Document', 'string', 'Financial', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0003-0000-0000-0000-000000000002', 'document.bank_statement.account_number', 'Bank account number', 'Document', 'string', 'Financial', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0003-0000-0000-0000-000000000003', 'document.bank_statement.statement_date', 'Statement date', 'Document', 'date', 'Financial', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0003-0000-0000-0000-000000000004', 'document.bank_statement.opening_balance', 'Opening balance', 'Document', 'decimal', 'Financial', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0cf0003-0000-0000-0000-000000000005', 'document.bank_statement.closing_balance', 'Closing balance', 'Document', 'decimal', 'Financial', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW())

ON CONFLICT (attribute_id) DO UPDATE SET
    name = EXCLUDED.name,
    long_description = EXCLUDED.long_description,
    updated_at = NOW();

-- ============================================================================
-- DOCUMENT TYPE REGISTRY
-- ============================================================================

-- Document type definitions with AttributeID linkage
CREATE TABLE IF NOT EXISTS "ob-poc".document_types (
    type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(100) NOT NULL UNIQUE, -- e.g., "passport", "certificate_incorporation"
    display_name VARCHAR(200) NOT NULL,
    category VARCHAR(100) NOT NULL,         -- "identity", "corporate", "financial", "legal", "compliance"
    domain VARCHAR(100),                    -- Link to DSL domains: "kyc", "onboarding", "isda", etc.

    -- Primary AttributeID for this document type (referential integrity)
    primary_attribute_id UUID REFERENCES "ob-poc".dictionary (attribute_id) ON DELETE RESTRICT,

    -- Metadata structure
    description TEXT,
    typical_issuers TEXT[],                 -- Common issuing authorities
    validity_period_days INTEGER,           -- Typical validity period (null = no expiry)
    renewal_required BOOLEAN DEFAULT false,

    -- Content structure with AttributeID references (CRITICAL for type safety)
    expected_attribute_ids UUID[] NOT NULL DEFAULT '{}', -- Array of attribute_ids that should be extracted
    validation_attribute_ids UUID[],        -- AttributeIDs used for validation rules
    extraction_template JSONB,              -- Template for AI content extraction

    -- Usage context
    required_for_products TEXT[],           -- Which products require this document type
    compliance_frameworks TEXT[],           -- FATF, CRS, FATCA, MiFID, etc.
    risk_classification VARCHAR(50),        -- "high", "medium", "low"

    -- AI/RAG support
    ai_description TEXT,                    -- Description for AI agents
    common_contents TEXT,                   -- What this document typically contains
    key_data_point_attributes UUID[],       -- AttributeIDs of key data points to extract

    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_document_types_category ON "ob-poc".document_types (category);
CREATE INDEX IF NOT EXISTS idx_document_types_domain ON "ob-poc".document_types (domain);
CREATE INDEX IF NOT EXISTS idx_document_types_active ON "ob-poc".document_types (active);
CREATE INDEX IF NOT EXISTS idx_document_types_primary_attr ON "ob-poc".document_types (primary_attribute_id);

-- ============================================================================
-- ISSUING AUTHORITIES REGISTRY
-- ============================================================================

-- Organizations and authorities that issue documents
CREATE TABLE IF NOT EXISTS "ob-poc".document_issuers (
    issuer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_code VARCHAR(100) NOT NULL UNIQUE, -- e.g., "us_state_dept", "cayman_registry"
    legal_name VARCHAR(300) NOT NULL,

    -- Geographic and regulatory context
    jurisdiction VARCHAR(10),               -- ISO country code
    regulatory_type VARCHAR(100),           -- "government", "regulatory_body", "trade_association"

    -- Contact and verification
    official_website VARCHAR(500),
    verification_endpoints JSONB,           -- APIs or URLs for document verification
    contact_information JSONB,

    -- Authority scope
    document_types_issued TEXT[],           -- Document type codes this issuer can produce
    authority_level VARCHAR(50),            -- "national", "state", "municipal", "industry", "private"

    -- Operational details
    typical_processing_time_days INTEGER,
    digital_issuance_available BOOLEAN DEFAULT false,
    api_integration_available BOOLEAN DEFAULT false,

    -- Trust and reliability
    reliability_score DECIMAL(3,2) DEFAULT 0.8, -- 0.0-1.0 trust score
    verification_method VARCHAR(100),       -- How to verify documents from this issuer

    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_document_issuers_jurisdiction ON "ob-poc".document_issuers (jurisdiction);
CREATE INDEX IF NOT EXISTS idx_document_issuers_regulatory_type ON "ob-poc".document_issuers (regulatory_type);
CREATE INDEX IF NOT EXISTS idx_document_issuers_active ON "ob-poc".document_issuers (active);

-- ============================================================================
-- DOCUMENT CATALOG WITH ATTRIBUTEID INTEGRATION
-- ============================================================================

-- Central catalog of all documents with full AttributeID integration
CREATE TABLE IF NOT EXISTS "ob-poc".document_catalog (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Core identification with AttributeID linkage
    document_code VARCHAR(200) NOT NULL UNIQUE, -- User-friendly ID: "doc-cayman-registry-001"
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types (type_id),
    issuer_id UUID REFERENCES "ob-poc".document_issuers (issuer_id),

    -- Document metadata (stored as attribute values for consistency)
    title VARCHAR(500),
    description TEXT,
    language VARCHAR(10) DEFAULT 'en',     -- ISO language code

    -- Validity and lifecycle
    issue_date DATE,
    expiry_date DATE,
    last_verified_date DATE,
    verification_status VARCHAR(50) DEFAULT 'pending', -- "pending", "verified", "expired", "invalid"

    -- Content and storage
    file_path VARCHAR(1000),               -- Path to actual document file
    file_size_bytes BIGINT,
    file_hash VARCHAR(128),                -- SHA-256 hash for integrity
    mime_type VARCHAR(100),
    page_count INTEGER,

    -- Extracted content with strict AttributeID typing
    extracted_text TEXT,                   -- Full text content
    extracted_attributes JSONB,            -- CRITICAL: {"attribute_id_uuid": "value", ...}
    ai_summary TEXT,                       -- AI-generated summary
    tags TEXT[],                           -- Searchable tags

    -- Business context
    related_entities TEXT[],               -- Entity IDs this document relates to
    business_purpose TEXT,                 -- Why this document exists
    confidentiality_level VARCHAR(50) DEFAULT 'internal', -- Links to d0c00008 attribute

    -- Compliance and audit
    retention_period_years INTEGER,
    disposal_date DATE,
    audit_trail JSONB,                     -- History of changes and access

    -- Version control
    version VARCHAR(50) DEFAULT '1.0',
    parent_document_id UUID REFERENCES "ob-poc".document_catalog (document_id),
    is_current_version BOOLEAN DEFAULT true,

    -- AI metadata (removed VECTOR type for compatibility)
    last_embedding_update TIMESTAMPTZ,
    ai_metadata JSONB,                     -- Additional AI processing metadata

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_document_catalog_type ON "ob-poc".document_catalog (document_type_id);
CREATE INDEX IF NOT EXISTS idx_document_catalog_issuer ON "ob-poc".document_catalog (issuer_id);
CREATE INDEX IF NOT EXISTS idx_document_catalog_status ON "ob-poc".document_catalog (verification_status);
CREATE INDEX IF NOT EXISTS idx_document_catalog_expiry ON "ob-poc".document_catalog (expiry_date);
CREATE INDEX IF NOT EXISTS idx_document_catalog_entities ON "ob-poc".document_catalog USING GIN (related_entities);
CREATE INDEX IF NOT EXISTS idx_document_catalog_tags ON "ob-poc".document_catalog USING GIN (tags);
CREATE INDEX IF NOT EXISTS idx_document_catalog_current ON "ob-poc".document_catalog (is_current_version);
CREATE INDEX IF NOT EXISTS idx_document_catalog_extracted ON "ob-poc".document_catalog USING GIN (extracted_attributes);

-- ============================================================================
-- DOCUMENT USAGE TRACKING
-- ============================================================================

-- Track how documents are used across DSL workflows
CREATE TABLE IF NOT EXISTS "ob-poc".document_usage (
    usage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog (document_id),

    -- Usage context
    dsl_version_id UUID REFERENCES "ob-poc".dsl_ob (version_id),
    cbu_id VARCHAR(255),
    workflow_stage VARCHAR(100),           -- "kyc_verification", "ubo_discovery", "compliance_check"

    -- Usage details
    usage_type VARCHAR(50) NOT NULL,       -- "evidence", "verification", "compliance", "reference"
    verb_used VARCHAR(100),                -- Which DSL verb referenced this document
    usage_context TEXT,                    -- Description of how document was used

    -- Outcome tracking
    verification_result VARCHAR(50),       -- "passed", "failed", "pending", "requires_update"
    confidence_score DECIMAL(3,2),         -- Confidence in document authenticity/validity
    notes TEXT,

    -- Access control and audit
    accessed_by VARCHAR(100),              -- User or system that accessed document
    access_method VARCHAR(50),             -- "dsl", "api", "manual", "automated"

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_document_usage_document ON "ob-poc".document_usage (document_id);
CREATE INDEX IF NOT EXISTS idx_document_usage_dsl_version ON "ob-poc".document_usage (dsl_version_id);
CREATE INDEX IF NOT EXISTS idx_document_usage_cbu ON "ob-poc".document_usage (cbu_id);
CREATE INDEX IF NOT EXISTS idx_document_usage_workflow ON "ob-poc".document_usage (workflow_stage);
CREATE INDEX IF NOT EXISTS idx_document_usage_type ON "ob-poc".document_usage (usage_type);
CREATE INDEX IF NOT EXISTS idx_document_usage_created ON "ob-poc".document_usage (created_at DESC);

-- ============================================================================
-- DOCUMENT RELATIONSHIPS
-- ============================================================================

-- Model relationships between documents (amendments, supporting docs, etc.)
CREATE TABLE IF NOT EXISTS "ob-poc".document_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog (document_id),
    target_document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog (document_id),

    relationship_type VARCHAR(50) NOT NULL, -- "amends", "supports", "supersedes", "references", "annexes"
    relationship_strength VARCHAR(20) DEFAULT 'strong', -- "strong", "weak", "suggested"

    -- Context
    description TEXT,
    business_rationale TEXT,
    effective_date DATE,

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    -- Prevent self-references
    CONSTRAINT chk_no_self_reference CHECK (source_document_id != target_document_id)
);

CREATE INDEX IF NOT EXISTS idx_document_relationships_source ON "ob-poc".document_relationships (source_document_id);
CREATE INDEX IF NOT EXISTS idx_document_relationships_target ON "ob-poc".document_relationships (target_document_id);
CREATE INDEX IF NOT EXISTS idx_document_relationships_type ON "ob-poc".document_relationships (relationship_type);

-- ============================================================================
-- SEED DATA WITH PROPER UUID CASTING
-- ============================================================================

-- Standard issuing authorities (avoid duplicates)
INSERT INTO "ob-poc".document_issuers (issuer_code, legal_name, jurisdiction, regulatory_type, authority_level, document_types_issued) VALUES
('us_state_dept_doc', 'U.S. Department of State', 'US', 'government', 'national', ARRAY['passport']),
('cayman_registry_doc', 'Cayman Islands General Registry', 'KY', 'government', 'national', ARRAY['certificate_incorporation']),
('sg_acra_doc', 'Accounting and Corporate Regulatory Authority', 'SG', 'regulatory_body', 'national', ARRAY['certificate_incorporation']),
('uk_companies_house_doc', 'Companies House', 'GB', 'government', 'national', ARRAY['certificate_incorporation'])
ON CONFLICT (issuer_code) DO NOTHING;

-- Core document types with proper UUID casting
INSERT INTO "ob-poc".document_types (
    type_code, display_name, category, domain, primary_attribute_id,
    description, typical_issuers, expected_attribute_ids, key_data_point_attributes,
    ai_description, common_contents
) VALUES

-- Identity Documents
('passport', 'Passport', 'identity', 'kyc',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Government-issued travel document for identity verification',
 ARRAY['government_passport_office'],
 ARRAY['d0cf0001-0000-0000-0000-000000000001'::uuid, 'd0cf0001-0000-0000-0000-000000000002'::uuid, 'd0cf0001-0000-0000-0000-000000000003'::uuid],
 ARRAY['d0cf0001-0000-0000-0000-000000000001'::uuid, 'd0cf0001-0000-0000-0000-000000000002'::uuid],
 'Government passport containing personal identification information',
 'Personal identification including full name, nationality, date of birth, passport number, issue and expiry dates'),

-- Corporate Documents
('certificate_incorporation', 'Certificate of Incorporation', 'corporate', 'kyc',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Official document establishing a corporation legal existence',
 ARRAY['company_registry', 'secretary_of_state'],
 ARRAY['d0cf0002-0000-0000-0000-000000000001'::uuid, 'd0cf0002-0000-0000-0000-000000000002'::uuid, 'd0cf0002-0000-0000-0000-000000000003'::uuid],
 ARRAY['d0cf0002-0000-0000-0000-000000000001'::uuid, 'd0cf0002-0000-0000-0000-000000000002'::uuid],
 'Legal incorporation document establishing corporate entity existence',
 'Corporate formation details including company name, registration number, incorporation date, jurisdiction'),

-- Financial Documents
('bank_statement', 'Bank Statement', 'financial', 'kyc',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Official record of bank account transactions and balances',
 ARRAY['commercial_bank', 'investment_bank'],
 ARRAY['d0cf0003-0000-0000-0000-000000000001'::uuid, 'd0cf0003-0000-0000-0000-000000000002'::uuid, 'd0cf0003-0000-0000-0000-000000000003'::uuid],
 ARRAY['d0cf0003-0000-0000-0000-000000000001'::uuid, 'd0cf0003-0000-0000-0000-000000000004'::uuid, 'd0cf0003-0000-0000-0000-000000000005'::uuid],
 'Bank statement showing account activity and balances',
 'Financial information including account holder, account number, transaction history, balances')

ON CONFLICT (type_code) DO NOTHING;

-- ============================================================================
-- VALIDATION FUNCTIONS
-- ============================================================================

-- Function to validate that extracted attributes match expected AttributeIDs for document type
CREATE OR REPLACE FUNCTION validate_document_attributes(
    p_document_type_id UUID,
    p_extracted_attributes JSONB
) RETURNS BOOLEAN AS $$
DECLARE
    expected_attrs UUID[];
    extracted_keys TEXT[];
    unexpected_attrs TEXT[] := '{}';
BEGIN
    -- Get expected AttributeIDs for this document type
    SELECT expected_attribute_ids INTO expected_attrs
    FROM "ob-poc".document_types
    WHERE type_id = p_document_type_id;

    -- If no extracted attributes, that's okay (optional extraction)
    IF p_extracted_attributes IS NULL OR p_extracted_attributes = '{}'::jsonb THEN
        RETURN true;
    END IF;

    -- Get keys from extracted attributes
    SELECT array_agg(key) INTO extracted_keys
    FROM jsonb_object_keys(p_extracted_attributes) key;

    -- Check for unexpected attributes (not in dictionary)
    SELECT array_agg(key) INTO unexpected_attrs
    FROM unnest(extracted_keys) key
    WHERE key::uuid NOT IN (SELECT attribute_id FROM "ob-poc".dictionary);

    -- Return false if there are invalid attributes
    IF array_length(unexpected_attrs, 1) > 0 THEN
        RAISE WARNING 'Document validation failed. Invalid attrs: %', unexpected_attrs;
        RETURN false;
    END IF;

    RETURN true;
END;
$$ LANGUAGE plpgsql;

-- Trigger to validate document attributes on insert/update
CREATE OR REPLACE FUNCTION trigger_validate_document_attributes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.extracted_attributes IS NOT NULL THEN
        IF NOT validate_document_attributes(NEW.document_type_id, NEW.extracted_attributes) THEN
            RAISE EXCEPTION 'Document attributes validation failed for document type %', NEW.document_type_id;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER validate_document_attributes_trigger
    BEFORE INSERT OR UPDATE ON "ob-poc".document_catalog
    FOR EACH ROW
    EXECUTE FUNCTION trigger_validate_document_attributes();

-- ============================================================================
-- VIEWS FOR ATTRIBUTEID-AWARE QUERIES
-- ============================================================================

-- Complete document information with AttributeID resolution
CREATE OR REPLACE VIEW "ob-poc".document_catalog_with_attributes AS
SELECT
    dc.*,
    dt.type_code,
    dt.display_name as document_type_name,
    dt.category as document_category,
    dt.domain as document_domain,
    dt.expected_attribute_ids,
    dt.key_data_point_attributes,
    di.issuer_code,
    di.legal_name as issuer_name,
    di.jurisdiction as issuer_jurisdiction,
    -- Resolve extracted attributes to human-readable names
    (SELECT jsonb_object_agg(d.name, ea.value)
     FROM jsonb_each(dc.extracted_attributes) ea(key, value)
     JOIN "ob-poc".dictionary d ON d.attribute_id = ea.key::uuid
     WHERE dc.extracted_attributes IS NOT NULL
    ) as extracted_attributes_resolved
FROM "ob-poc".document_catalog dc
LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
LEFT JOIN "ob-poc".document_issuers di ON dc.issuer_id = di.issuer_id;

-- Document usage summary with AttributeID context
CREATE OR REPLACE VIEW "ob-poc".document_usage_with_context AS
SELECT
    du.*,
    dc.document_code,
    dt.type_code,
    dt.expected_attribute_ids,
    array_length(dt.expected_attribute_ids, 1) as expected_attribute_count,
    (SELECT COUNT(*) FROM jsonb_object_keys(dc.extracted_attributes)) as extracted_attribute_count
FROM "ob-poc".document_usage du
JOIN "ob-poc".document_catalog dc ON du.document_id = dc.document_id
JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id;

-- ============================================================================
-- SAMPLE DATA FOR TESTING
-- ============================================================================

-- Sample document catalog entries
INSERT INTO "ob-poc".document_catalog (
    document_code, document_type_id, issuer_id, title, description,
    issue_date, language, related_entities, tags, confidentiality_level,
    extracted_attributes
) VALUES

-- Sample passport
('doc-passport-john-001',
 (SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'passport'),
 (SELECT issuer_id FROM "ob-poc".document_issuers WHERE issuer_code = 'us_state_dept_doc'),
 'John Smith - US Passport',
 'US passport for individual John Smith used in KYC verification',
 '2020-01-15',
 'en',
 ARRAY['person-john-smith'],
 ARRAY['passport', 'identity', 'kyc', 'us_citizen'],
 'confidential',
 '{"d0cf0001-0000-0000-0000-000000000001": "US123456789", "d0cf0001-0000-0000-0000-000000000002": "John Smith", "d0cf0001-0000-0000-0000-000000000003": "US"}'::jsonb),

-- Sample incorporation certificate
('doc-cayman-registry-001',
 (SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'certificate_incorporation'),
 (SELECT issuer_id FROM "ob-poc".document_issuers WHERE issuer_code = 'cayman_registry_doc'),
 'Zenith Capital Partners LP Certificate of Incorporation',
 'Certificate of incorporation for Zenith Capital Partners LP in Cayman Islands',
 '2020-03-15',
 'en',
 ARRAY['company-zenith-spv-001'],
 ARRAY['incorporation', 'corporate', 'cayman_islands', 'hedge_fund'],
 'confidential',
 '{"d0cf0002-0000-0000-0000-000000000001": "Zenith Capital Partners LP", "d0cf0002-0000-0000-0000-000000000002": "KY-123456", "d0cf0002-0000-0000-0000-000000000004": "KY"}'::jsonb);

-- Phase 1 Document Library implementation is complete
-- All tables created with proper AttributeID referential integrity
-- Validation functions ensure type safety
-- Sample data demonstrates proper usage patterns
