-- ============================================================================
-- ATTRIBUTE SOURCE/SINK MODEL WITH HIERARCHICAL DOCUMENT TYPES
-- ============================================================================
-- 
-- Key concepts:
-- 1. Attributes have SOURCES (where values come from) and SINKS (where they go)
-- 2. Document types are HIERARCHICAL: PASSPORT → PASSPORT_GBR, PASSPORT_USA, etc.
-- 3. Source rules can specify "PASSPORT" (any) or "PASSPORT_GBR" (specific)
-- 4. This creates a SPARSE MATRIX - not every attribute comes from every document
--
-- For KYC/AML:
-- - What attributes do I need? (by customer type, jurisdiction, risk level)
-- - What documents can provide them? (source matrix)
-- - What does this customer have? (sparse - gaps need filling)
-- ============================================================================

BEGIN;

-- ============================================================================
-- DOCUMENT TYPE HIERARCHY
-- Supports "PASSPORT" matching any country-specific variant
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_type_hierarchy (
    type_code VARCHAR(50) PRIMARY KEY,
    parent_type_code VARCHAR(50) REFERENCES "ob-poc".document_type_hierarchy(type_code),
    hierarchy_level INT NOT NULL DEFAULT 0,  -- 0 = root, 1 = child, 2 = grandchild
    is_abstract BOOLEAN DEFAULT false,       -- true = can't be used directly, only children
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert hierarchy
INSERT INTO "ob-poc".document_type_hierarchy (type_code, parent_type_code, hierarchy_level, is_abstract) VALUES
    -- Root types (abstract - use children)
    ('PASSPORT', NULL, 0, true),
    ('NATIONAL_ID', NULL, 0, true),
    ('DRIVERS_LICENSE', NULL, 0, false),  -- Not country-specific for now
    ('PROOF_OF_ADDRESS', NULL, 0, true),
    ('CORPORATE_REGISTRATION', NULL, 0, true),
    
    -- Passport variants (concrete)
    ('PASSPORT_GBR', 'PASSPORT', 1, false),
    ('PASSPORT_USA', 'PASSPORT', 1, false),
    ('PASSPORT_DEU', 'PASSPORT', 1, false),
    ('PASSPORT_FRA', 'PASSPORT', 1, false),
    ('PASSPORT_CHE', 'PASSPORT', 1, false),
    ('PASSPORT_IRL', 'PASSPORT', 1, false),
    ('PASSPORT_CAN', 'PASSPORT', 1, false),
    ('PASSPORT_AUS', 'PASSPORT', 1, false),
    
    -- Proof of address variants
    ('UTILITY_BILL', 'PROOF_OF_ADDRESS', 1, false),
    ('BANK_STATEMENT', 'PROOF_OF_ADDRESS', 1, false),
    ('TAX_DOCUMENT', 'PROOF_OF_ADDRESS', 1, false),
    
    -- Corporate registration variants
    ('CERT_OF_INCORPORATION', 'CORPORATE_REGISTRATION', 1, false),
    ('ARTICLES_OF_ASSOCIATION', 'CORPORATE_REGISTRATION', 1, false),
    ('REGISTRY_EXTRACT', 'CORPORATE_REGISTRATION', 1, false)
ON CONFLICT (type_code) DO NOTHING;

-- ============================================================================
-- ATTRIBUTE SOURCES TABLE
-- Defines where each attribute CAN come from (the sparse matrix)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".attribute_sources (
    source_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".consolidated_attributes(attribute_id),
    
    -- Source specification
    source_type VARCHAR(50) NOT NULL,  -- 'document', 'api', 'manual', 'derived'
    
    -- For document sources: can be abstract (PASSPORT) or concrete (PASSPORT_GBR)
    document_type_code VARCHAR(50) REFERENCES "ob-poc".document_type_hierarchy(type_code),
    
    -- For API sources
    api_endpoint VARCHAR(500),
    api_provider VARCHAR(100),
    
    -- Source metadata
    priority INT DEFAULT 5,              -- Lower = preferred source
    confidence_weight DECIMAL(3,2) DEFAULT 1.0,  -- How much to trust this source
    is_authoritative BOOLEAN DEFAULT false,      -- Is this the "golden" source?
    
    -- Extraction hints (for document sources)
    extraction_hints JSONB,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_source CHECK (
        (source_type = 'document' AND document_type_code IS NOT NULL) OR
        (source_type = 'api' AND api_endpoint IS NOT NULL) OR
        (source_type IN ('manual', 'derived'))
    )
);

CREATE INDEX IF NOT EXISTS idx_attr_sources_attr ON "ob-poc".attribute_sources(attribute_id);
CREATE INDEX IF NOT EXISTS idx_attr_sources_doc ON "ob-poc".attribute_sources(document_type_code);

-- ============================================================================
-- ATTRIBUTE SINKS TABLE
-- Defines where attribute values GO (database, API, report, etc.)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".attribute_sinks (
    sink_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".consolidated_attributes(attribute_id),
    
    -- Sink specification
    sink_type VARCHAR(50) NOT NULL,  -- 'database', 'api', 'report', 'cache'
    
    -- For database sinks
    target_table VARCHAR(100),
    target_column VARCHAR(100),
    
    -- For API sinks
    api_endpoint VARCHAR(500),
    api_method VARCHAR(10),
    
    -- Sink metadata
    is_required BOOLEAN DEFAULT false,   -- Must this sink always receive the value?
    transform_rule JSONB,                -- How to transform before sinking
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_attr_sinks_attr ON "ob-poc".attribute_sinks(attribute_id);

-- ============================================================================
-- HELPER FUNCTION: Get all document types that can source an attribute
-- Expands abstract types (PASSPORT) to concrete types (PASSPORT_GBR, etc.)
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".get_source_document_types(p_attribute_id UUID)
RETURNS TABLE(document_type_code VARCHAR(50), is_direct BOOLEAN) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE type_expansion AS (
        -- Direct sources (may be abstract like PASSPORT)
        SELECT 
            asrc.document_type_code,
            asrc.document_type_code as original_type,
            true as is_direct
        FROM "ob-poc".attribute_sources asrc
        WHERE asrc.attribute_id = p_attribute_id
          AND asrc.source_type = 'document'
        
        UNION
        
        -- Expand abstract types to their children
        SELECT 
            dth.type_code,
            te.original_type,
            false as is_direct
        FROM type_expansion te
        JOIN "ob-poc".document_type_hierarchy dth ON dth.parent_type_code = te.document_type_code
    )
    SELECT DISTINCT 
        te.document_type_code,
        te.is_direct
    FROM type_expansion te
    JOIN "ob-poc".document_type_hierarchy dth ON dth.type_code = te.document_type_code
    WHERE dth.is_abstract = false;  -- Only return concrete types
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- HELPER FUNCTION: Get all attributes a document type can provide
-- Works with both abstract (PASSPORT) and concrete (PASSPORT_GBR) types
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".get_document_attributes(p_document_type_code VARCHAR(50))
RETURNS TABLE(
    attribute_id UUID,
    attribute_code VARCHAR(100),
    attribute_name VARCHAR(200),
    source_priority INT,
    is_authoritative BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE type_ancestors AS (
        -- Start with the given type
        SELECT type_code, parent_type_code, 0 as depth
        FROM "ob-poc".document_type_hierarchy
        WHERE type_code = p_document_type_code
        
        UNION
        
        -- Walk up to parent types
        SELECT dth.type_code, dth.parent_type_code, ta.depth + 1
        FROM type_ancestors ta
        JOIN "ob-poc".document_type_hierarchy dth ON dth.type_code = ta.parent_type_code
    )
    SELECT DISTINCT
        ca.attribute_id,
        ca.attribute_code,
        ca.attribute_name,
        asrc.priority as source_priority,
        asrc.is_authoritative
    FROM "ob-poc".attribute_sources asrc
    JOIN "ob-poc".consolidated_attributes ca ON ca.attribute_id = asrc.attribute_id
    WHERE asrc.source_type = 'document'
      AND asrc.document_type_code IN (SELECT type_code FROM type_ancestors)
    ORDER BY asrc.priority;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- SEED: Attribute Sources (the sparse matrix)
-- Using abstract PASSPORT to mean "any passport"
-- ============================================================================

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative, extraction_hints)
SELECT 
    ca.attribute_id,
    'document',
    'PASSPORT',  -- Abstract = any passport variant
    1,           -- Highest priority
    true,        -- Passport is authoritative for identity
    jsonb_build_object(
        'fields', ARRAY['surname', 'given_names', 'mrz'],
        'validation', 'cross_reference_mrz'
    )
FROM "ob-poc".consolidated_attributes ca
WHERE ca.attribute_code = 'FULL_NAME'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'DRIVERS_LICENSE', 2, false
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'FULL_NAME'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'PROOF_OF_ADDRESS', 3, false  -- Any proof of address
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'FULL_NAME'
ON CONFLICT DO NOTHING;

-- DATE_OF_BIRTH sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'PASSPORT', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'DATE_OF_BIRTH'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'DRIVERS_LICENSE', 2, false
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'DATE_OF_BIRTH'
ON CONFLICT DO NOTHING;

-- NATIONALITY - only from passport
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'PASSPORT', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'NATIONALITY'
ON CONFLICT DO NOTHING;

-- PASSPORT_NUMBER - only from passport (obviously)
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'PASSPORT', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'PASSPORT_NUMBER'
ON CONFLICT DO NOTHING;

-- RESIDENTIAL_ADDRESS - from proof of address documents
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'PROOF_OF_ADDRESS', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'RESIDENTIAL_ADDRESS'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'DRIVERS_LICENSE', 2, false
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'RESIDENTIAL_ADDRESS'
ON CONFLICT DO NOTHING;

-- Corporate attributes - from corporate registration
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'CORPORATE_REGISTRATION', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'COMPANY_NAME'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'CORPORATE_REGISTRATION', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'REGISTRATION_NUMBER'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, document_type_code, priority, is_authoritative)
SELECT ca.attribute_id, 'document', 'CORPORATE_REGISTRATION', 1, true
FROM "ob-poc".consolidated_attributes ca WHERE ca.attribute_code = 'INCORPORATION_DATE'
ON CONFLICT DO NOTHING;

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- 1. Show the sparse matrix: which documents source which attributes
SELECT 
    ca.attribute_code,
    asrc.document_type_code as source_doc,
    dth.is_abstract as is_abstract_source,
    asrc.priority,
    asrc.is_authoritative
FROM "ob-poc".attribute_sources asrc
JOIN "ob-poc".consolidated_attributes ca ON ca.attribute_id = asrc.attribute_id
JOIN "ob-poc".document_type_hierarchy dth ON dth.type_code = asrc.document_type_code
WHERE asrc.source_type = 'document'
ORDER BY ca.attribute_code, asrc.priority;

-- 2. Expand PASSPORT to show all concrete passport types that can source FULL_NAME
SELECT * FROM "ob-poc".get_source_document_types(
    (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'FULL_NAME')
);

-- 3. What attributes can a UK Passport provide? (inherits from PASSPORT)
SELECT * FROM "ob-poc".get_document_attributes('PASSPORT_GBR');

-- 4. What attributes can any PASSPORT provide?
SELECT * FROM "ob-poc".get_document_attributes('PASSPORT');
