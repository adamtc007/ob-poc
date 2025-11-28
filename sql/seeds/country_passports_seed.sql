-- Extended seed: Country-specific passport types demonstrating
-- that the SAME attributes can come from MULTIPLE document variants
--
-- Key insight: The attribute (FULL_NAME) belongs to the PERSON
-- Documents are just EVIDENCE sources for that attribute
-- Cross-document validation: Name should match across all passports

BEGIN;

-- ============================================================================
-- COUNTRY-SPECIFIC PASSPORT TYPES
-- Same base structure, different field layouts and validation rules
-- ============================================================================

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description)
VALUES 
    -- Generic passport (fallback)
    ('a1000000-0000-0000-0000-000000000001'::uuid, 'PASSPORT', 'Passport (Generic)', 'IDENTITY', 
     'Government-issued travel document - use country-specific type if available'),
    
    -- Country-specific passports
    ('a1000000-0000-0000-0000-000000000101'::uuid, 'PASSPORT_GBR', 'UK Passport', 'IDENTITY',
     'United Kingdom passport. MRZ format: TD3. Issuer: UKPA/HMPO.'),
    ('a1000000-0000-0000-0000-000000000102'::uuid, 'PASSPORT_USA', 'US Passport', 'IDENTITY',
     'United States passport. MRZ format: TD3. Issuer: US State Department.'),
    ('a1000000-0000-0000-0000-000000000103'::uuid, 'PASSPORT_DEU', 'German Passport', 'IDENTITY',
     'German passport (Reisepass). MRZ format: TD3. Issuer: Bundesdruckerei.'),
    ('a1000000-0000-0000-0000-000000000104'::uuid, 'PASSPORT_FRA', 'French Passport', 'IDENTITY',
     'French passport (Passeport). MRZ format: TD3. Issuer: Imprimerie Nationale.'),
    ('a1000000-0000-0000-0000-000000000105'::uuid, 'PASSPORT_CHE', 'Swiss Passport', 'IDENTITY',
     'Swiss passport. MRZ format: TD3. Issuer: fedpol.'),
    ('a1000000-0000-0000-0000-000000000106'::uuid, 'PASSPORT_IRL', 'Irish Passport', 'IDENTITY',
     'Irish passport. MRZ format: TD3. Issuer: Passport Office Dublin/Cork.'),
    ('a1000000-0000-0000-0000-000000000107'::uuid, 'PASSPORT_CAN', 'Canadian Passport', 'IDENTITY',
     'Canadian passport. MRZ format: TD3. Issuer: IRCC.'),
    ('a1000000-0000-0000-0000-000000000108'::uuid, 'PASSPORT_AUS', 'Australian Passport', 'IDENTITY',
     'Australian passport. MRZ format: TD3. Issuer: DFAT.')
ON CONFLICT (type_code) DO UPDATE SET 
    display_name = EXCLUDED.display_name,
    description = EXCLUDED.description;

-- ============================================================================
-- PERSON IDENTITY ATTRIBUTES
-- These belong to the PERSON, documents are just evidence sources
-- ============================================================================

-- Update source_documents to include all passport variants
UPDATE "ob-poc".consolidated_attributes 
SET source_documents = ARRAY[
    'PASSPORT', 'PASSPORT_GBR', 'PASSPORT_USA', 'PASSPORT_DEU', 'PASSPORT_FRA',
    'PASSPORT_CHE', 'PASSPORT_IRL', 'PASSPORT_CAN', 'PASSPORT_AUS',
    'DRIVERS_LICENSE'
],
cross_document_validation = 'MUST match across all identity documents. Flag discrepancies for manual review.'
WHERE attribute_code = 'FULL_NAME';

UPDATE "ob-poc".consolidated_attributes 
SET source_documents = ARRAY[
    'PASSPORT', 'PASSPORT_GBR', 'PASSPORT_USA', 'PASSPORT_DEU', 'PASSPORT_FRA',
    'PASSPORT_CHE', 'PASSPORT_IRL', 'PASSPORT_CAN', 'PASSPORT_AUS',
    'DRIVERS_LICENSE'
],
cross_document_validation = 'MUST match across all identity documents. Immutable attribute.'
WHERE attribute_code = 'DATE_OF_BIRTH';

UPDATE "ob-poc".consolidated_attributes 
SET source_documents = ARRAY[
    'PASSPORT', 'PASSPORT_GBR', 'PASSPORT_USA', 'PASSPORT_DEU', 'PASSPORT_FRA',
    'PASSPORT_CHE', 'PASSPORT_IRL', 'PASSPORT_CAN', 'PASSPORT_AUS'
],
cross_document_validation = 'Dual nationals will have MULTIPLE nationalities - this is valid. Store all.'
WHERE attribute_code = 'NATIONALITY';

-- ============================================================================
-- DOCUMENT-ATTRIBUTE MAPPINGS FOR COUNTRY-SPECIFIC PASSPORTS
-- Each passport type extracts the SAME person attributes
-- ============================================================================

-- UK Passport (GBR) - specific field locations
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required, 
    field_location_hints, ai_extraction_notes
) 
SELECT 
    'PASSPORT_GBR',
    attribute_id,
    extraction_priority,
    is_required,
    -- UK-specific hints
    CASE attribute_id
        WHEN 'b1000000-0000-0000-0000-000000000001'::uuid 
            THEN ARRAY['surname_line_3', 'given_names_line_4', 'mrz_line_1']
        WHEN 'b1000000-0000-0000-0000-000000000004'::uuid 
            THEN ARRAY['passport_no_line_1', 'mrz_doc_number']
        ELSE field_location_hints
    END,
    'UK Passport format. ' || COALESCE(ai_extraction_notes, '')
FROM "ob-poc".document_attribute_mappings
WHERE document_type_code = 'PASSPORT'
ON CONFLICT DO NOTHING;

-- US Passport (USA) - specific field locations
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required, 
    field_location_hints, ai_extraction_notes
) 
SELECT 
    'PASSPORT_USA',
    attribute_id,
    extraction_priority,
    is_required,
    -- US-specific hints (name format: SURNAME<<GIVEN<NAMES)
    CASE attribute_id
        WHEN 'b1000000-0000-0000-0000-000000000001'::uuid 
            THEN ARRAY['surname_top', 'given_name_below', 'mrz_name_line']
        WHEN 'b1000000-0000-0000-0000-000000000004'::uuid 
            THEN ARRAY['passport_number_top_right', 'mrz_doc_number']
        ELSE field_location_hints
    END,
    'US Passport format. ' || COALESCE(ai_extraction_notes, '')
FROM "ob-poc".document_attribute_mappings
WHERE document_type_code = 'PASSPORT'
ON CONFLICT DO NOTHING;

-- German Passport (DEU)
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required, 
    field_location_hints, ai_extraction_notes
) 
SELECT 
    'PASSPORT_DEU',
    attribute_id,
    extraction_priority,
    is_required,
    -- German-specific hints (labels in German)
    CASE attribute_id
        WHEN 'b1000000-0000-0000-0000-000000000001'::uuid 
            THEN ARRAY['name_nachname', 'vornamen', 'mrz_line_1']
        WHEN 'b1000000-0000-0000-0000-000000000002'::uuid 
            THEN ARRAY['geburtsdatum', 'mrz_dob']
        ELSE field_location_hints
    END,
    'German Reisepass. Labels in German (Name/Nachname, Geburtsdatum, etc). ' || COALESCE(ai_extraction_notes, '')
FROM "ob-poc".document_attribute_mappings
WHERE document_type_code = 'PASSPORT'
ON CONFLICT DO NOTHING;

-- Add remaining passport types with generic hints
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required, 
    field_location_hints, ai_extraction_notes
) 
SELECT 
    passport_type,
    dam.attribute_id,
    dam.extraction_priority,
    dam.is_required,
    dam.field_location_hints,
    passport_type || ' format. ' || COALESCE(dam.ai_extraction_notes, '')
FROM "ob-poc".document_attribute_mappings dam
CROSS JOIN (
    VALUES ('PASSPORT_FRA'), ('PASSPORT_CHE'), ('PASSPORT_IRL'), ('PASSPORT_CAN'), ('PASSPORT_AUS')
) AS passports(passport_type)
WHERE dam.document_type_code = 'PASSPORT'
ON CONFLICT DO NOTHING;

COMMIT;

-- ============================================================================
-- VERIFICATION: Show the bi-directional relationship
-- ============================================================================

-- All passport types and their attribute counts
SELECT 
    dt.type_code,
    dt.display_name,
    COUNT(dam.attribute_id) as attributes_extracted
FROM "ob-poc".document_types dt
LEFT JOIN "ob-poc".document_attribute_mappings dam ON dt.type_code = dam.document_type_code
WHERE dt.type_code LIKE 'PASSPORT%'
GROUP BY dt.type_id, dt.type_code, dt.display_name
ORDER BY dt.type_code;

-- FULL_NAME: All document types that can provide this person attribute
SELECT 
    dam.document_type_code,
    dt.display_name,
    dam.is_required,
    ca.cross_document_validation
FROM "ob-poc".document_attribute_mappings dam
JOIN "ob-poc".document_types dt ON dam.document_type_code = dt.type_code
JOIN "ob-poc".consolidated_attributes ca ON dam.attribute_id = ca.attribute_id
WHERE ca.attribute_code = 'FULL_NAME'
ORDER BY dam.document_type_code;

-- Show cross-document validation rules
SELECT 
    attribute_code,
    attribute_name,
    cross_document_validation
FROM "ob-poc".consolidated_attributes
WHERE cross_document_validation IS NOT NULL;
