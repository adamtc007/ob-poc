-- 10_document_library_schema.sql
-- Document Library Infrastructure for Comprehensive Document Cataloging
--
-- This schema creates a centralized document library system that enables:
-- 1. Rich document metadata storage and cataloging
-- 2. Document type definitions and templates
-- 3. Issuer authority management
-- 4. Content extraction and AI/RAG integration
-- 5. Document lifecycle and version management
-- 6. Usage tracking across DSL workflows

-- ============================================================================
-- DOCUMENT TYPE REGISTRY
-- ============================================================================

-- Document type definitions with rich metadata and AttributeID linkage
CREATE TABLE IF NOT EXISTS "ob-poc".document_types (
    type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(100) NOT NULL UNIQUE, -- e.g., "passport", "certificate_incorporation", "isda_master_agreement"
    display_name VARCHAR(200) NOT NULL,
    category VARCHAR(100) NOT NULL,         -- "identity", "corporate", "financial", "legal", "compliance"
    domain VARCHAR(100),                    -- Link to DSL domains: "kyc", "onboarding", "isda", etc.

    -- AttributeID linkage for type safety
    attribute_id UUID REFERENCES "ob-poc".dictionary (attribute_id) ON DELETE RESTRICT,

    -- Metadata structure
    description TEXT,
    typical_issuers TEXT[],                 -- Common issuing authorities
    validity_period_days INTEGER,           -- Typical validity period (null = no expiry)
    renewal_required BOOLEAN DEFAULT false,

    -- Content structure with AttributeID references
    expected_fields JSONB,                  -- Schema: {"field_name": "attribute_id_uuid", ...}
    validation_rules JSONB,                 -- Rules for validating document contents
    extraction_template JSONB,             -- Template for AI content extraction

    -- Usage context
    required_for_products TEXT[],           -- Which products require this document type
    compliance_frameworks TEXT[],           -- FATF, CRS, FATCA, MiFID, etc.
    risk_classification VARCHAR(50),        -- "high", "medium", "low"

    -- AI/RAG support
    ai_description TEXT,                    -- Description for AI agents
    common_contents TEXT,                   -- What this document typically contains
    key_data_points TEXT[],                 -- Important data points to extract

    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_document_types_category ON "ob-poc".document_types (category);
CREATE INDEX IF NOT EXISTS idx_document_types_domain ON "ob-poc".document_types (domain);
CREATE INDEX IF NOT EXISTS idx_document_types_active ON "ob-poc".document_types (active);

-- ============================================================================
-- ISSUING AUTHORITIES REGISTRY
-- ============================================================================

-- Organizations and authorities that issue documents
CREATE TABLE IF NOT EXISTS "ob-poc".document_issuers (
    issuer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_code VARCHAR(100) NOT NULL UNIQUE, -- e.g., "us_state_dept", "cayman_registry", "isda_inc"
    legal_name VARCHAR(300) NOT NULL,

    -- Geographic and regulatory context
    jurisdiction VARCHAR(10),               -- ISO country code
    regulatory_type VARCHAR(100),           -- "government", "regulatory_body", "trade_association", "self_regulatory"

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
-- DOCUMENT LIBRARY CATALOG
-- ============================================================================

-- Central catalog of all documents with rich metadata
CREATE TABLE IF NOT EXISTS "ob-poc".document_catalog (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Core identification
    document_code VARCHAR(200) NOT NULL UNIQUE, -- User-friendly ID: "doc-cayman-registry-001"
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types (type_id),
    issuer_id UUID REFERENCES "ob-poc".document_issuers (issuer_id),

    -- Document metadata
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

    -- Extracted content for AI/RAG with AttributeID references
    extracted_text TEXT,                   -- Full text content
    key_data_points JSONB,                 -- Structured data: {"attribute_id": "value", ...}
    ai_summary TEXT,                       -- AI-generated summary
    tags TEXT[],                           -- Searchable tags

    -- Business context
    related_entities TEXT[],               -- Entity IDs this document relates to
    business_purpose TEXT,                 -- Why this document exists
    confidentiality_level VARCHAR(50) DEFAULT 'internal', -- "public", "internal", "restricted", "confidential"

    -- Compliance and audit
    retention_period_years INTEGER,
    disposal_date DATE,
    audit_trail JSONB,                     -- History of changes and access

    -- Version control
    version VARCHAR(50) DEFAULT '1.0',
    parent_document_id UUID REFERENCES "ob-poc".document_catalog (document_id),
    is_current_version BOOLEAN DEFAULT true,

    -- RAG and AI metadata
    embedding_vector VECTOR(1536),         -- OpenAI ada-002 embeddings
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

-- ============================================================================
-- DOCUMENT USAGE TRACKING
-- ============================================================================

-- Track how documents are used across DSL workflows and business processes
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
-- DOCUMENT TEMPLATES
-- ============================================================================

-- Templates for generating or validating documents
CREATE TABLE IF NOT EXISTS "ob-poc".document_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types (type_id),
    issuer_id UUID REFERENCES "ob-poc".document_issuers (issuer_id),

    template_name VARCHAR(200) NOT NULL,
    template_version VARCHAR(50) DEFAULT '1.0',

    -- Template content
    template_structure JSONB,              -- JSON schema for document structure
    required_fields JSONB,                 -- Fields that must be present
    validation_rules JSONB,                -- Validation logic
    extraction_patterns JSONB,             -- Patterns for data extraction

    -- Generation support
    generation_template TEXT,              -- Template for generating documents
    sample_data JSONB,                     -- Example data for testing

    -- AI integration
    ai_prompts JSONB,                      -- Prompts for AI-assisted processing
    quality_checks JSONB,                  -- Automated quality checks

    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_document_templates_type ON "ob-poc".document_templates (document_type_id);
CREATE INDEX IF NOT EXISTS idx_document_templates_issuer ON "ob-poc".document_templates (issuer_id);
CREATE INDEX IF NOT EXISTS idx_document_templates_active ON "ob-poc".document_templates (active);

-- ============================================================================
-- VIEWS FOR COMMON QUERIES
-- ============================================================================

-- Complete document information with type and issuer details
CREATE OR REPLACE VIEW "ob-poc".document_catalog_full AS
SELECT
    dc.*,
    dt.type_code,
    dt.display_name as document_type_name,
    dt.category as document_category,
    dt.domain as document_domain,
    di.issuer_code,
    di.legal_name as issuer_name,
    di.jurisdiction as issuer_jurisdiction,
    di.regulatory_type as issuer_type
FROM "ob-poc".document_catalog dc
LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
LEFT JOIN "ob-poc".document_issuers di ON dc.issuer_id = di.issuer_id;

-- Document usage summary by workflow stage
CREATE OR REPLACE VIEW "ob-poc".document_usage_summary AS
SELECT
    du.document_id,
    dc.document_code,
    dt.type_code,
    COUNT(*) as usage_count,
    array_agg(DISTINCT du.workflow_stage) as workflow_stages,
    array_agg(DISTINCT du.verb_used) as verbs_used,
    MAX(du.created_at) as last_used
FROM "ob-poc".document_usage du
JOIN "ob-poc".document_catalog dc ON du.document_id = dc.document_id
JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
GROUP BY du.document_id, dc.document_code, dt.type_code;

-- ============================================================================
-- DOCUMENT-SPECIFIC ATTRIBUTES FOR ATTRIBUTEID SYSTEM
-- ============================================================================

-- Add document-specific attributes to the dictionary table
INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at) VALUES

-- Core Document Metadata Attributes
('d0c00001-0000-0000-0000-000000000001', 'document.id', 'Unique document identifier', 'Document', 'string', 'Document', '', '{"type": "generated", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00002-0000-0000-0000-000000000002', 'document.title', 'Document title or name', 'Document', 'string', 'Document', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00003-0000-0000-0000-000000000003', 'document.description', 'Document description', 'Document', 'text', 'Document', '', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00004-0000-0000-0000-000000000004', 'document.language', 'Document language code', 'Document', 'string', 'Document', '', '{"type": "manual", "required": false, "format": "ISO-639-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00005-0000-0000-0000-000000000005', 'document.issue_date', 'Date document was issued', 'Document', 'date', 'Document', '', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00006-0000-0000-0000-000000000006', 'document.expiry_date', 'Date document expires', 'Document', 'date', 'Document', '', '{"type": "manual", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00007-0000-0000-0000-000000000007', 'document.file_path', 'Path to document file', 'Document', 'string', 'Document', '', '{"type": "system", "required": false}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),
('d0c00008-0000-0000-0000-000000000008', 'document.confidentiality_level', 'Document confidentiality classification', 'Document', 'enum', 'Document', '', '{"type": "manual", "required": true, "values": ["public", "internal", "restricted", "confidential"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Identity Document Attributes
('d0c00010-0000-0000-0000-000000000010', 'document.passport.number', 'Passport number', 'Document', 'string', 'Identity', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00011-0000-0000-0000-000000000011', 'document.passport.full_name', 'Full name on passport', 'Document', 'string', 'Identity', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00012-0000-0000-0000-000000000012', 'document.passport.nationality', 'Passport nationality', 'Document', 'string', 'Identity', '', '{"type": "extraction", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00013-0000-0000-0000-000000000013', 'document.passport.date_of_birth', 'Date of birth on passport', 'Document', 'date', 'Identity', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00014-0000-0000-0000-000000000014', 'document.passport.issuing_authority', 'Passport issuing authority', 'Document', 'string', 'Identity', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Corporate Document Attributes
('d0c00020-0000-0000-0000-000000000020', 'document.incorporation.company_name', 'Company name on incorporation certificate', 'Document', 'string', 'Corporate', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00021-0000-0000-0000-000000000021', 'document.incorporation.registration_number', 'Company registration number', 'Document', 'string', 'Corporate', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00022-0000-0000-0000-000000000022', 'document.incorporation.jurisdiction', 'Jurisdiction of incorporation', 'Document', 'string', 'Corporate', '', '{"type": "extraction", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00023-0000-0000-0000-000000000023', 'document.incorporation.incorporation_date', 'Date of incorporation', 'Document', 'date', 'Corporate', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00024-0000-0000-0000-000000000024', 'document.incorporation.authorized_shares', 'Authorized share capital', 'Document', 'decimal', 'Corporate', '', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Financial Document Attributes
('d0c00030-0000-0000-0000-000000000030', 'document.bank_statement.account_holder', 'Bank statement account holder name', 'Document', 'string', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00031-0000-0000-0000-000000000031', 'document.bank_statement.account_number', 'Bank account number', 'Document', 'string', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00032-0000-0000-0000-000000000032', 'document.bank_statement.statement_period', 'Statement period', 'Document', 'string', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00033-0000-0000-0000-000000000033', 'document.bank_statement.opening_balance', 'Opening balance', 'Document', 'decimal', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00034-0000-0000-0000-000000000034', 'document.bank_statement.closing_balance', 'Closing balance', 'Document', 'decimal', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- Audited Financials Attributes
('d0c00040-0000-0000-0000-000000000040', 'document.audited_financials.company_name', 'Company name on financial statements', 'Document', 'string', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00041-0000-0000-0000-000000000041', 'document.audited_financials.financial_year', 'Financial year end date', 'Document', 'string', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00042-0000-0000-0000-000000000042', 'document.audited_financials.auditor_name', 'Auditor firm name', 'Document', 'string', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00043-0000-0000-0000-000000000043', 'document.audited_financials.total_assets', 'Total assets value', 'Document', 'decimal', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00044-0000-0000-0000-000000000044', 'document.audited_financials.total_liabilities', 'Total liabilities value', 'Document', 'decimal', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('d0c00045-0000-0000-0000-000000000045', 'document.audited_financials.net_worth', 'Net worth/equity value', 'Document', 'decimal', 'Financial', '', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW())
ON CONFLICT (attribute_id) DOVALUES
-- Core document categories and types with AttributeID linkage
INSERT INTO "ob-poc".document_types (type_code, display_name, category, domain, description, typical_issuers, attribute_id, expected_fields) VALUES
-- Identity Documents
('passport', 'Passport', 'identity', 'kyc', 'Government-issued travel document for identity verification',
 ARRAY['government_passport_office'],
 'doc10000-0000-0000-0000-000000000001',
 '{"passport_number": "docf0000-0000-0000-0000-000000000001", "full_name": "docf0000-0000-0000-0000-000000000002", "issue_date": "docf0000-0000-0000-0000-000000000005", "expiry_date": "docf0000-0000-0000-0000-000000000006"}'),

('drivers_license', 'Driver License', 'identity', 'kyc', 'Government-issued driving permit used for identity verification',
 ARRAY['dmv', 'transport_authority'],
 'doc10000-0000-0000-0000-000000000006',
 '{"full_name": "docf0000-0000-0000-0000-000000000002", "issue_date": "docf0000-0000-0000-0000-000000000005", "expiry_date": "docf0000-0000-0000-0000-000000000006"}'),

-- Corporate Documents
('certificate_incorporation', 'Certificate of Incorporation', 'corporate', 'kyc', 'Official document establishing a corporation legal existence',
 ARRAY['company_registry', 'secretary_of_state'],
 'doc10000-0000-0000-0000-000000000002',
 '{"company_name": "docf0000-0000-0000-0000-000000000003", "registration_number": "docf0000-0000-0000-0000-000000000004", "jurisdiction": "docf0000-0000-0000-0000-000000000007"}'),

('memorandum_articles', 'Memorandum and Articles of Association', 'corporate', 'onboarding', 'Constitutional documents defining company structure and governance',
 ARRAY['company_registry', 'law_firm'],
 NULL,
 '{"company_name": "docf0000-0000-0000-0000-000000000003", "jurisdiction": "docf0000-0000-0000-0000-000000000007"}'),

-- Financial Documents
('bank_statement', 'Bank Statement', 'financial', 'kyc', 'Official record of bank account transactions and balances',
 ARRAY['commercial_bank', 'investment_bank'],
 'doc10000-0000-0000-0000-000000000007',
 '{"full_name": "docf0000-0000-0000-0000-000000000002", "issue_date": "docf0000-0000-0000-0000-000000000005"}'),

('audited_financials', 'Audited Financial Statements', 'financial', 'onboarding', 'Independently audited financial statements',
 ARRAY['accounting_firm', 'certified_public_accountant'],
 NULL,
 '{"company_name": "docf0000-0000-0000-0000-000000000003"}');

-- Standard issuing authorities
INSERT INTO "ob-poc".document_issuers (issuer_code, legal_name, jurisdiction, regulatory_type, authority_level, document_types_issued) VALUES
('us_state_dept', 'U.S. Department of State', 'US', 'government', 'national', ARRAY['passport']),
('cayman_registry', 'Cayman Islands General Registry', 'KY', 'government', 'national', ARRAY['certificate_incorporation', 'memorandum_articles']),
('sg_acra', 'Accounting and Corporate Regulatory Authority', 'SG', 'regulatory_body', 'national', ARRAY['certificate_incorporation', 'memorandum_articles']),
('uk_companies_house', 'Companies House', 'GB', 'government', 'national', ARRAY['certificate_incorporation', 'memorandum_articles']);

-- Document library is now ready for ISDA domain integration
