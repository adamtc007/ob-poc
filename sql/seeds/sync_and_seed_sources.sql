-- ============================================================================
-- SYNC CONSOLIDATED_ATTRIBUTES TO DICTIONARY + SETUP SOURCE HIERARCHY
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. SYNC consolidated_attributes → dictionary
-- ============================================================================

INSERT INTO "ob-poc".dictionary (
    attribute_id,
    name,
    long_description,
    group_id,
    mask,
    domain
)
SELECT 
    ca.attribute_id,
    ca.attribute_code,  -- Use code as name
    ca.description,
    ca.category,        -- Use category as group
    CASE ca.data_type 
        WHEN 'string' THEN 'string'
        WHEN 'date' THEN 'date'
        WHEN 'boolean' THEN 'boolean'
        ELSE 'string'
    END as mask,
    ca.subcategory      -- Use subcategory as domain
FROM "ob-poc".consolidated_attributes ca
ON CONFLICT (attribute_id) DO UPDATE SET
    name = EXCLUDED.name,
    long_description = EXCLUDED.long_description,
    group_id = EXCLUDED.group_id;

-- ============================================================================
-- 2. CREATE DOCUMENT TYPE HIERARCHY (outside transaction safety)
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
    
    -- Passport variants
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
    
    -- Corporate registration variants
    ('CERT_OF_INCORPORATION', 'CORPORATE_REGISTRATION', 1, false, 'Certificate of Incorporation'),
    ('ARTICLES_OF_ASSOCIATION', 'CORPORATE_REGISTRATION', 1, false, 'Articles of Association'),
    ('REGISTRY_EXTRACT', 'CORPORATE_REGISTRATION', 1, false, 'Company Registry Extract');

-- ============================================================================
-- 3. CREATE HELPER FUNCTIONS
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".expand_document_type(p_type_code VARCHAR(50))
RETURNS TABLE(type_code VARCHAR(50), display_name VARCHAR(200)) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE type_tree AS (
        SELECT dth.type_code, dth.display_name, dth.is_abstract
        FROM "ob-poc".document_type_hierarchy dth
        WHERE dth.type_code = p_type_code
        
        UNION ALL
        
        SELECT child.type_code, child.display_name, child.is_abstract
        FROM type_tree parent
        JOIN "ob-poc".document_type_hierarchy child ON child.parent_type_code = parent.type_code
    )
    SELECT tt.type_code, tt.display_name
    FROM type_tree tt
    WHERE tt.is_abstract = false;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 4. SEED ATTRIBUTE SOURCES (using dictionary table FKs)
-- ============================================================================

-- Clear previous
DELETE FROM "ob-poc".attribute_sources WHERE source_type = 'document';

-- FULL_NAME sources (PASSPORT is abstract - matches any passport variant)
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'PASSPORT', 'is_authoritative', true),
    1
FROM "ob-poc".dictionary d WHERE d.name = 'FULL_NAME';

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'DRIVERS_LICENSE', 'is_authoritative', false),
    2
FROM "ob-poc".dictionary d WHERE d.name = 'FULL_NAME';

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'PROOF_OF_ADDRESS', 'is_authoritative', false),
    3
FROM "ob-poc".dictionary d WHERE d.name = 'FULL_NAME';

-- DATE_OF_BIRTH sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'PASSPORT', 'is_authoritative', true),
    1
FROM "ob-poc".dictionary d WHERE d.name = 'DATE_OF_BIRTH';

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'DRIVERS_LICENSE', 'is_authoritative', false),
    2
FROM "ob-poc".dictionary d WHERE d.name = 'DATE_OF_BIRTH';

-- NATIONALITY - passport only (can have multiple for dual nationals)
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'PASSPORT', 'is_authoritative', true, 'multi_value', true),
    1
FROM "ob-poc".dictionary d WHERE d.name = 'NATIONALITY';

-- PASSPORT_NUMBER - passport only
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'PASSPORT', 'is_authoritative', true),
    1
FROM "ob-poc".dictionary d WHERE d.name = 'PASSPORT_NUMBER';

-- RESIDENTIAL_ADDRESS
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'PROOF_OF_ADDRESS', 'is_authoritative', true),
    1
FROM "ob-poc".dictionary d WHERE d.name = 'RESIDENTIAL_ADDRESS';

INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'DRIVERS_LICENSE', 'is_authoritative', false),
    2
FROM "ob-poc".dictionary d WHERE d.name = 'RESIDENTIAL_ADDRESS';

-- Corporate attributes
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    jsonb_build_object('document_type', 'CORPORATE_REGISTRATION', 'is_authoritative', true),
    1
FROM "ob-poc".dictionary d WHERE d.name IN ('COMPANY_NAME', 'REGISTRATION_NUMBER', 'INCORPORATION_DATE');

COMMIT;

-- ============================================================================
-- VERIFICATION
-- ============================================================================

-- 1. Dictionary sync check
SELECT d.attribute_id, d.name, d.group_id, d.mask
FROM "ob-poc".dictionary d
WHERE d.attribute_id IN (SELECT attribute_id FROM "ob-poc".consolidated_attributes)
ORDER BY d.group_id, d.name;

-- 2. Sparse matrix: attribute → document sources
SELECT 
    d.name as attribute,
    asrc.source_config->>'document_type' as doc_type,
    dth.is_abstract,
    asrc.priority,
    asrc.source_config->>'is_authoritative' as authoritative
FROM "ob-poc".attribute_sources asrc
JOIN "ob-poc".dictionary d ON d.attribute_id = asrc.attribute_id
LEFT JOIN "ob-poc".document_type_hierarchy dth ON dth.type_code = asrc.source_config->>'document_type'
WHERE asrc.source_type = 'document'
ORDER BY d.name, asrc.priority;

-- 3. Expand PASSPORT to all concrete variants
SELECT * FROM "ob-poc".expand_document_type('PASSPORT');

-- 4. KEY QUERY: All concrete docs that can source FULL_NAME
SELECT 
    'FULL_NAME' as attribute,
    asrc.source_config->>'document_type' as configured_source,
    expanded.type_code as concrete_type,
    expanded.display_name
FROM "ob-poc".attribute_sources asrc
JOIN "ob-poc".dictionary d ON d.attribute_id = asrc.attribute_id
CROSS JOIN LATERAL "ob-poc".expand_document_type((asrc.source_config->>'document_type')::varchar) expanded
WHERE d.name = 'FULL_NAME' AND asrc.source_type = 'document'
ORDER BY asrc.priority, expanded.type_code;
