-- Driving License seed - follows same pattern as passports
-- 
-- Key insight: US licenses are STATE-issued, not federal
-- This introduces a sub-national hierarchy level
--
-- DRIVERS_LICENSE (abstract)
--   ├── DRIVERS_LICENSE_GBR (UK - DVLA)
--   ├── DRIVERS_LICENSE_DEU (Germany - Führerschein)
--   ├── DRIVERS_LICENSE_USA (US abstract)
--   │     ├── DRIVERS_LICENSE_USA_CA (California DMV)
--   │     ├── DRIVERS_LICENSE_USA_NY (New York DMV)
--   │     └── DRIVERS_LICENSE_USA_TX (Texas DPS)
--   └── etc.

BEGIN;

-- ============================================================================
-- DRIVING LICENSE TYPES - Country/State specific
-- ============================================================================

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description, parent_type_code)
VALUES 
    -- Generic (abstract)
    ('a2000000-0000-0000-0000-000000000001'::uuid, 'DRIVERS_LICENSE', 'Drivers License (Generic)', 'IDENTITY', 
     'Government-issued driving permit. Use jurisdiction-specific type if available.', NULL),
    
    -- UK
    ('a2000000-0000-0000-0000-000000000101'::uuid, 'DRIVERS_LICENSE_GBR', 'UK Driving Licence', 'IDENTITY',
     'UK photocard driving licence issued by DVLA. Format: credit card size with photo.',
     'DRIVERS_LICENSE'),
    
    -- Germany
    ('a2000000-0000-0000-0000-000000000102'::uuid, 'DRIVERS_LICENSE_DEU', 'German Driving Licence', 'IDENTITY',
     'German Führerschein. EU format credit card size.',
     'DRIVERS_LICENSE'),
    
    -- France
    ('a2000000-0000-0000-0000-000000000103'::uuid, 'DRIVERS_LICENSE_FRA', 'French Driving Licence', 'IDENTITY',
     'French Permis de Conduire. EU format.',
     'DRIVERS_LICENSE'),
    
    -- Ireland
    ('a2000000-0000-0000-0000-000000000104'::uuid, 'DRIVERS_LICENSE_IRL', 'Irish Driving Licence', 'IDENTITY',
     'Irish driving licence issued by NDLS. EU format.',
     'DRIVERS_LICENSE'),
    
    -- Canada (provincial but we'll use country level for now)
    ('a2000000-0000-0000-0000-000000000105'::uuid, 'DRIVERS_LICENSE_CAN', 'Canadian Drivers License', 'IDENTITY',
     'Canadian provincial drivers license. Format varies by province.',
     'DRIVERS_LICENSE'),
    
    -- Australia (state-issued like US)
    ('a2000000-0000-0000-0000-000000000106'::uuid, 'DRIVERS_LICENSE_AUS', 'Australian Drivers Licence', 'IDENTITY',
     'Australian state/territory drivers licence. Format varies.',
     'DRIVERS_LICENSE'),

    -- US - Abstract for all states
    ('a2000000-0000-0000-0000-000000000200'::uuid, 'DRIVERS_LICENSE_USA', 'US Drivers License', 'IDENTITY',
     'US state-issued drivers license. Use state-specific type.',
     'DRIVERS_LICENSE'),
    
    -- US States (sample - major financial centers)
    ('a2000000-0000-0000-0000-000000000201'::uuid, 'DRIVERS_LICENSE_USA_CA', 'California Drivers License', 'IDENTITY',
     'California DMV issued drivers license. REAL ID compliant versions available.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000202'::uuid, 'DRIVERS_LICENSE_USA_NY', 'New York Drivers License', 'IDENTITY',
     'New York DMV issued drivers license. Enhanced license available for border crossing.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000203'::uuid, 'DRIVERS_LICENSE_USA_TX', 'Texas Drivers License', 'IDENTITY',
     'Texas DPS issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000204'::uuid, 'DRIVERS_LICENSE_USA_FL', 'Florida Drivers License', 'IDENTITY',
     'Florida DHSMV issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000205'::uuid, 'DRIVERS_LICENSE_USA_IL', 'Illinois Drivers License', 'IDENTITY',
     'Illinois Secretary of State issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000206'::uuid, 'DRIVERS_LICENSE_USA_MA', 'Massachusetts Drivers License', 'IDENTITY',
     'Massachusetts RMV issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000207'::uuid, 'DRIVERS_LICENSE_USA_NJ', 'New Jersey Drivers License', 'IDENTITY',
     'New Jersey MVC issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000208'::uuid, 'DRIVERS_LICENSE_USA_PA', 'Pennsylvania Drivers License', 'IDENTITY',
     'Pennsylvania PennDOT issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000209'::uuid, 'DRIVERS_LICENSE_USA_CT', 'Connecticut Drivers License', 'IDENTITY',
     'Connecticut DMV issued drivers license.',
     'DRIVERS_LICENSE_USA'),
    ('a2000000-0000-0000-0000-000000000210'::uuid, 'DRIVERS_LICENSE_USA_DE', 'Delaware Drivers License', 'IDENTITY',
     'Delaware DMV issued drivers license. Popular for corporate domicile.',
     'DRIVERS_LICENSE_USA')

ON CONFLICT (type_code) DO UPDATE SET 
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    parent_type_code = EXCLUDED.parent_type_code;

-- ============================================================================
-- DRIVING LICENSE SPECIFIC ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain)
VALUES
    ('d1000000-0000-0000-0000-000000000020'::uuid, 'document.license_number',
     'Driving license document number. Format varies by jurisdiction.',
     'document_id', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000021'::uuid, 'document.license_class',
     'License class/categories (e.g., A, B, C for EU; Class C, M for US).',
     'document_validity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000022'::uuid, 'document.issuing_authority',
     'Authority that issued the document (e.g., DVLA, California DMV).',
     'document_id', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000023'::uuid, 'person.eye_color',
     'Eye color as recorded on license. US licenses only.',
     'physical', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000024'::uuid, 'person.height',
     'Height as recorded on license. Format varies (ft/in vs cm).',
     'physical', 'string', 'KYC')
ON CONFLICT (name) DO UPDATE SET
    long_description = EXCLUDED.long_description,
    group_id = EXCLUDED.group_id;

-- ============================================================================
-- ATTRIBUTE SOURCES FOR LICENSE-SPECIFIC ATTRIBUTES
-- ============================================================================

-- document.license_number sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document', 
    '{"document_category": "DRIVERS_LICENSE", "field_hints": ["license_no", "document_number", "dl_number"]}'::jsonb, 
    1
FROM "ob-poc".dictionary d WHERE d.name = 'document.license_number';

-- document.issuing_authority sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "DRIVERS_LICENSE", "field_hints": ["issuing_authority", "issued_by"]}', 1),
    ('document', '{"document_category": "PASSPORT", "field_hints": ["authority", "issuing_office"]}', 2)
) AS source(source_type, config, priority)
WHERE d.name = 'document.issuing_authority';

-- person.residential_address - add drivers license as source (already exists, just ensuring)
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, 'document',
    '{"document_category": "DRIVERS_LICENSE", "field_hints": ["address", "residence"], "note": "address_on_license"}'::jsonb,
    3
FROM "ob-poc".dictionary d 
WHERE d.name = 'person.residential_address'
AND NOT EXISTS (
    SELECT 1 FROM "ob-poc".attribute_sources s 
    WHERE s.attribute_id = d.attribute_id 
    AND s.source_config->>'document_category' = 'DRIVERS_LICENSE'
);

COMMIT;

-- ============================================================================
-- VERIFICATION: Document hierarchy
-- ============================================================================

-- Show full hierarchy including multi-level (USA states)
WITH RECURSIVE doc_tree AS (
    SELECT type_code, parent_type_code, display_name, 0 as level
    FROM "ob-poc".document_types
    WHERE parent_type_code IS NULL AND type_code IN ('PASSPORT', 'DRIVERS_LICENSE')
    
    UNION ALL
    
    SELECT dt.type_code, dt.parent_type_code, dt.display_name, doc_tree.level + 1
    FROM "ob-poc".document_types dt
    JOIN doc_tree ON dt.parent_type_code = doc_tree.type_code
)
SELECT 
    REPEAT('  ', level) || type_code as hierarchy,
    display_name,
    level
FROM doc_tree
ORDER BY 
    CASE WHEN type_code LIKE 'PASSPORT%' THEN 0 ELSE 1 END,
    level, type_code;

-- Count documents at each level
SELECT 
    COALESCE(parent_type_code, type_code) as category,
    COUNT(*) as variants
FROM "ob-poc".document_types
WHERE type_code LIKE 'PASSPORT%' OR type_code LIKE 'DRIVERS_LICENSE%'
GROUP BY COALESCE(parent_type_code, type_code)
ORDER BY category;
