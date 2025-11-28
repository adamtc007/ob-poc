-- ============================================================================
-- ATTRIBUTE SOURCE/SINK MODEL - WORKING WITH EXISTING SCHEMA
-- ============================================================================
-- 
-- Existing tables:
-- - attribute_sources: attribute_id, source_type, source_config (JSONB), priority
-- - attribute_sinks: attribute_id, sink_type, sink_config (JSONB)
--
-- This script adds:
-- - document_type_hierarchy: for abstract type expansion (PASSPORT → PASSPORT_GBR, etc.)
-- - Helper functions for sparse matrix queries
-- ============================================================================

BEGIN;

-- ============================================================================
-- DOCUMENT TYPE HIERARCHY
-- ============================================================================

DROP TABLE IF EXISTS "ob-poc".document_type_hierarchy CASCADE;

CREATE TABLE "ob-poc".document_type_hierarchy (
    type_code VARCHAR(50) PRIMARY KEY,
    parent_type_code VARCHAR(50) REFERENCES "ob-poc".document_type_hierarchy(type_code),
    hierarchy_level INT NOT NULL DEFAULT 0,
    is_abstract BOOLEAN DEFAULT false,
    display_name VARCHAR(200),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

INSERT INTO "ob-poc".document_type_hierarchy (type_code, parent_type_code, hierarchy_level, is_abstract, display_name) VALUES
    -- Root abstract types
    ('PASSPORT', NULL, 0, true, 'Passport (Any Country)'),
    ('NATIONAL_ID', NULL, 0, true, 'National ID Card (Any Country)'),
    ('DRIVERS_LICENSE', NULL, 0, false, 'Drivers License'),
    ('PROOF_OF_ADDRESS', NULL, 0, true, 'Proof of Address (Any Type)'),
    ('CORPORATE_REGISTRATION', NULL, 0, true, 'Corporate Registration (Any Type)'),
    
    -- Passport variants by country
    ('PASSPORT_GBR', 'PASSPORT', 1, false, 'UK Passport'),
    ('PASSPORT_USA', 'PASSPORT', 1, false, 'US Passport'),
    ('PASSPORT_DEU', 'PASSPORT', 1, false, 'German Passport'),
    ('PASSPORT_FRA', 'PASSPORT', 1, false, 'French Passport'),
    ('PASSPORT_CHE', 'PASSPORT', 1, false, 'Swiss Passport'),
    ('PASSPORT_IRL', 'PASSPORT', 1, false, 'Irish Passport'),
    ('PASSPORT_CAN', 'PASSPORT', 1, false, 'Canadian Passport'),
    ('PASSPORT_AUS', 'PASSPORT', 1, false, 'Australian Passport'),
    
    -- Proof of address variants
    ('UTILITY_BILL', 'PROOF_OF_ADDRESS', 1, false, 'Utility Bill'),
    ('BANK_STATEMENT', 'PROOF_OF_ADDRESS', 1, false, 'Bank Statement'),
    ('TAX_DOCUMENT', 'PROOF_OF_ADDRESS', 1, false, 'Tax Document'),
    ('COUNCIL_TAX_BILL', 'PROOF_OF_ADDRESS', 1, false, 'Council Tax Bill (UK)'),
    
    -- Corporate registration variants
    ('CERT_OF_INCORPORATION', 'CORPORATE_REGISTRATION', 1, false, 'Certificate of Incorporation'),
    ('ARTICLES_OF_ASSOCIATION', 'CORPORATE_REGISTRATION', 1, false, 'Articles of Association'),
    ('REGISTRY_EXTRACT', 'CORPORATE_REGISTRATION', 1, false, 'Company Registry Extract');

-- ============================================================================
-- HELPER FUNCTION: Expand abstract document type to concrete types
-- e.g., PASSPORT → [PASSPORT_GBR, PASSPORT_USA, PASSPORT_DEU, ...]
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".expand_document_type(p_type_code VARCHAR(50))
RETURNS TABLE(type_code VARCHAR(50), display_name VARCHAR(200)) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE type_tree AS (
        -- Start with the given type
        SELECT dth.type_code, dth.display_name, dth.is_abstract
        FROM "ob-poc".document_type_hierarchy dth
        WHERE dth.type_code = p_type_code
        
        UNION ALL
        
        -- Recursively get children
        SELECT child.type_code, child.display_name, child.is_abstract
        FROM type_tree parent
        JOIN "ob-poc".document_type_hierarchy child ON child.parent_type_code = parent.type_code
    )
    SELECT tt.type_code, tt.display_name
    FROM type_tree tt
    WHERE tt.is_abstract = false;  -- Only return concrete types
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- HELPER FUNCTION: Get ancestors of a document type
-- e.g., PASSPORT_GBR → [PASSPORT_GBR, PASSPORT]
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".get_document_type_ancestors(p_type_code VARCHAR(50))
RETURNS TABLE(type_code VARCHAR(50), hierarchy_level INT) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE ancestors AS (
        SELECT dth.type_code, dth.parent_type_code, dth.hierarchy_level
        FROM "ob-poc".document_type_hierarchy dth
        WHERE dth.type_code = p_type_code
        
        UNION ALL
        
        SELECT parent.type_code, parent.parent_type_code, parent.hierarchy_level
        FROM ancestors a
        JOIN "ob-poc".document_type_hierarchy parent ON parent.type_code = a.parent_type_code
    )
    SELECT a.type_code, a.hierarchy_level
    FROM ancestors a
    ORDER BY a.hierarchy_level DESC;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- SEED: Attribute sources using existing table structure
-- source_config JSONB contains: { "document_type": "PASSPORT" }
-- ============================================================================

-- Clear existing test data
DELETE FROM "ob-poc".attribute_sources WHERE source_type = 'document';

-- FULL_NAME sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT 
    ca.attribute_id,
    'document',
    jsonb_build_object(
        'document_type', 'PASSPORT',  -- Abstract: any passport
        'extraction_hints', jsonb_build_object(
            'fields', ARRAY['surname', 'given_names', 'mrz'],
            'cross_validate', true
        ),
        'is_authoritative', true
    ),
    1  -- Highest priority
FROM "ob-poc".consolidated_attributes ca
WHERE ca.attribute_code = 'FULL_NAME'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'DRIVERS_LICENSE', 'is_authoritative', false),
    2
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'FULL_NAME'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'PROOF_OF_ADDRESS', 'is_authoritative', false),
    3
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'FULL_NAME'
ON CONFLICT DO NOTHING;

-- DATE_OF_BIRTH sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'PASSPORT', 'is_authoritative', true),
    1
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'DATE_OF_BIRTH'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'DRIVERS_LICENSE', 'is_authoritative', false),
    2
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'DATE_OF_BIRTH'
ON CONFLICT DO NOTHING;

-- NATIONALITY - passport only
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object(
        'document_type', 'PASSPORT',
        'is_authoritative', true,
        'note', 'Multiple values valid for dual nationals'
    ),
    1
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'NATIONALITY'
ON CONFLICT DO NOTHING;

-- PASSPORT_NUMBER - passport only
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'PASSPORT', 'is_authoritative', true),
    1
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'PASSPORT_NUMBER'
ON CONFLICT DO NOTHING;

-- RESIDENTIAL_ADDRESS
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'PROOF_OF_ADDRESS', 'is_authoritative', true),
    1
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'RESIDENTIAL_ADDRESS'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'DRIVERS_LICENSE', 'is_authoritative', false),
    2
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'RESIDENTIAL_ADDRESS'
ON CONFLICT DO NOTHING;

-- Corporate attributes
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT ca.attribute_id, 'document',
    jsonb_build_object('document_type', 'CORPORATE_REGISTRATION', 'is_authoritative', true),
    1
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code IN ('COMPANY_NAME', 'REGISTRATION_NUMBER', 'INCORPORATION_DATE')
ON CONFLICT DO NOTHING;

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- 1. Show the sparse matrix: attribute → document sources
SELECT 
    ca.attribute_code,
    asrc.source_config->>'document_type' as source_doc_type,
    dth.is_abstract,
    dth.display_name,
    asrc.priority,
    (asrc.source_config->>'is_authoritative')::boolean as is_authoritative
FROM "ob-poc".attribute_sources asrc
JOIN "ob-poc".consolidated_attributes ca ON ca.attribute_id = asrc.attribute_id
LEFT JOIN "ob-poc".document_type_hierarchy dth ON dth.type_code = asrc.source_config->>'document_type'
WHERE asrc.source_type = 'document'
ORDER BY ca.attribute_code, asrc.priority;

-- 2. Expand PASSPORT to concrete types
SELECT * FROM "ob-poc".expand_document_type('PASSPORT');

-- 3. Expand PROOF_OF_ADDRESS to concrete types
SELECT * FROM "ob-poc".expand_document_type('PROOF_OF_ADDRESS');

-- 4. Get ancestors of PASSPORT_GBR (should include PASSPORT)
SELECT * FROM "ob-poc".get_document_type_ancestors('PASSPORT_GBR');

-- 5. THE KEY QUERY: What concrete document types can source FULL_NAME?
WITH attribute_doc_sources AS (
    SELECT 
        ca.attribute_code,
        asrc.source_config->>'document_type' as abstract_type,
        asrc.priority
    FROM "ob-poc".attribute_sources asrc
    JOIN "ob-poc".consolidated_attributes ca ON ca.attribute_id = asrc.attribute_id
    WHERE asrc.source_type = 'document'
      AND ca.attribute_code = 'FULL_NAME'
)
SELECT 
    ads.attribute_code,
    ads.abstract_type as configured_source,
    expanded.type_code as concrete_type,
    expanded.display_name,
    ads.priority
FROM attribute_doc_sources ads
CROSS JOIN LATERAL "ob-poc".expand_document_type(ads.abstract_type) expanded
ORDER BY ads.priority, expanded.type_code;
