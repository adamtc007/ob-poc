-- Sparse Matrix Model for KYC Attributes
-- 
-- Key concepts:
-- 1. document_types has hierarchy (PASSPORT_GBR.parent = PASSPORT)
-- 2. attribute_sources references abstract document categories (PASSPORT, not PASSPORT_GBR)
-- 3. attribute_sinks defines where values are used (KYC reports, sanctions screening, etc.)
-- 4. This creates a sparse matrix: Attributes × Document Categories

BEGIN;

-- ============================================================================
-- DOCUMENT TYPE HIERARCHY
-- ============================================================================

ALTER TABLE "ob-poc".document_types 
ADD COLUMN IF NOT EXISTS parent_type_code VARCHAR(100) REFERENCES "ob-poc".document_types(type_code);

CREATE INDEX IF NOT EXISTS idx_document_types_parent ON "ob-poc".document_types(parent_type_code);

COMMENT ON COLUMN "ob-poc".document_types.parent_type_code IS 
'Abstract parent document type. E.g., PASSPORT_GBR.parent = PASSPORT. Used for sparse matrix attribute mapping.';

-- Set hierarchy for country-specific passports
UPDATE "ob-poc".document_types 
SET parent_type_code = 'PASSPORT'
WHERE type_code LIKE 'PASSPORT_%' AND type_code != 'PASSPORT';

-- ============================================================================
-- KYC ATTRIBUTES IN DICTIONARY
-- Using existing naming convention: domain.attribute_name
-- ============================================================================

INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain)
VALUES
    ('d1000000-0000-0000-0000-000000000001'::uuid, 'person.full_name', 
     'Full legal name of individual. Must match across all identity documents.', 
     'identity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000002'::uuid, 'person.date_of_birth',
     'Date of birth. Immutable attribute - must match across all documents.',
     'identity', 'date', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000003'::uuid, 'person.nationality',
     'Nationality/citizenship. Dual nationals may have multiple values - this is valid.',
     'identity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000004'::uuid, 'person.place_of_birth',
     'Place of birth (city/country).',
     'identity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000005'::uuid, 'person.gender',
     'Gender as recorded on official documents (M/F/X).',
     'identity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000006'::uuid, 'person.residential_address',
     'Current residential address. Requires recent proof (utility bill, bank statement).',
     'address', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000007'::uuid, 'document.passport_number',
     'Passport document number. Unique per passport issuance.',
     'document_id', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000008'::uuid, 'document.issue_date',
     'Date the identity document was issued.',
     'document_validity', 'date', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000009'::uuid, 'document.expiry_date',
     'Date the identity document expires. Must be valid (not expired) for KYC.',
     'document_validity', 'date', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000010'::uuid, 'entity.company_name',
     'Registered legal name of company. Must match company registry exactly.',
     'corporate_identity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000011'::uuid, 'entity.registration_number',
     'Company registration/incorporation number.',
     'corporate_identity', 'string', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000012'::uuid, 'entity.incorporation_date',
     'Date the company was legally incorporated.',
     'corporate_identity', 'date', 'KYC'),
    
    ('d1000000-0000-0000-0000-000000000013'::uuid, 'entity.registered_address',
     'Official registered office address of the company.',
     'corporate_address', 'string', 'KYC')
ON CONFLICT (name) DO UPDATE SET
    long_description = EXCLUDED.long_description,
    group_id = EXCLUDED.group_id,
    domain = EXCLUDED.domain;

-- ============================================================================
-- ATTRIBUTE_SOURCES: The Sparse Matrix (Attributes × Document Categories)
-- ============================================================================

DELETE FROM "ob-poc".attribute_sources;

-- person.full_name sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "PASSPORT", "field_hints": ["surname", "given_names", "mrz"]}', 1),
    ('document', '{"document_category": "DRIVERS_LICENSE", "field_hints": ["full_name", "surname"]}', 2),
    ('document', '{"document_category": "UTILITY_BILL", "field_hints": ["account_holder"]}', 3),
    ('document', '{"document_category": "BANK_STATEMENT", "field_hints": ["account_name"]}', 4),
    ('user_input', '{"form_field": "legal_name", "validation": "required"}', 10)
) AS source(source_type, config, priority)
WHERE d.name = 'person.full_name';

-- person.date_of_birth sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "PASSPORT", "field_hints": ["dob", "date_of_birth", "mrz"]}', 1),
    ('document', '{"document_category": "DRIVERS_LICENSE", "field_hints": ["dob"]}', 2),
    ('user_input', '{"form_field": "date_of_birth", "validation": "required, past_date"}', 10)
) AS source(source_type, config, priority)
WHERE d.name = 'person.date_of_birth';

-- person.nationality sources (passport only)
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "PASSPORT", "field_hints": ["nationality", "mrz_issuer"], "note": "dual_nationals_have_multiple"}', 1),
    ('user_input', '{"form_field": "nationality", "validation": "iso_country_code", "allows_multiple": true}', 10)
) AS source(source_type, config, priority)
WHERE d.name = 'person.nationality';

-- person.residential_address sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "UTILITY_BILL", "field_hints": ["service_address"], "max_age_days": 90}', 1),
    ('document', '{"document_category": "BANK_STATEMENT", "field_hints": ["correspondence_address"], "max_age_days": 90}', 2),
    ('document', '{"document_category": "DRIVERS_LICENSE", "field_hints": ["address"]}', 3),
    ('user_input', '{"form_field": "residential_address", "validation": "required"}', 10)
) AS source(source_type, config, priority)
WHERE d.name = 'person.residential_address';

-- document.passport_number sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "PASSPORT", "field_hints": ["passport_no", "document_number", "mrz"]}', 1)
) AS source(source_type, config, priority)
WHERE d.name = 'document.passport_number';

-- document.expiry_date sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "PASSPORT", "field_hints": ["expiry", "valid_until", "mrz"]}', 1),
    ('document', '{"document_category": "DRIVERS_LICENSE", "field_hints": ["expiry"]}', 2)
) AS source(source_type, config, priority)
WHERE d.name = 'document.expiry_date';

-- entity.company_name sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "CERT_OF_INCORPORATION", "field_hints": ["company_name", "entity_name"]}', 1),
    ('document', '{"document_category": "BANK_STATEMENT", "field_hints": ["account_name"], "note": "for_business_accounts"}', 3),
    ('registry_lookup', '{"registry": "companies_house", "field": "company_name"}', 2),
    ('user_input', '{"form_field": "company_name", "validation": "required"}', 10)
) AS source(source_type, config, priority)
WHERE d.name = 'entity.company_name';

-- entity.registration_number sources
INSERT INTO "ob-poc".attribute_sources (attribute_id, source_type, source_config, priority)
SELECT d.attribute_id, source.source_type, source.config::jsonb, source.priority
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('document', '{"document_category": "CERT_OF_INCORPORATION", "field_hints": ["registration_number", "company_number"]}', 1),
    ('registry_lookup', '{"registry": "companies_house", "field": "company_number"}', 2),
    ('user_input', '{"form_field": "registration_number", "validation": "required"}', 10)
) AS source(source_type, config, priority)
WHERE d.name = 'entity.registration_number';

-- ============================================================================
-- ATTRIBUTE_SINKS: Where values are used (KYC reports, sanctions, filings)
-- ============================================================================

DELETE FROM "ob-poc".attribute_sinks;

-- person.full_name sinks
INSERT INTO "ob-poc".attribute_sinks (attribute_id, sink_type, sink_config)
SELECT d.attribute_id, sink.sink_type, sink.config::jsonb
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('kyc_report', '{"section": "identity", "field": "full_legal_name"}'),
    ('sanctions_screening', '{"search_field": "name", "fuzzy_match": true, "match_threshold": 0.85}'),
    ('pep_screening', '{"search_field": "name", "include_relatives": true}'),
    ('regulatory_filing', '{"forms": ["W8-BEN", "CRS"], "field": "name_of_individual"}'),
    ('account_opening', '{"field": "account_holder_name"}')
) AS sink(sink_type, config)
WHERE d.name = 'person.full_name';

-- person.date_of_birth sinks
INSERT INTO "ob-poc".attribute_sinks (attribute_id, sink_type, sink_config)
SELECT d.attribute_id, sink.sink_type, sink.config::jsonb
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('kyc_report', '{"section": "identity", "field": "date_of_birth"}'),
    ('sanctions_screening', '{"search_field": "dob", "for_disambiguation": true}'),
    ('age_verification', '{"min_age": 18, "jurisdiction_specific": true}'),
    ('regulatory_filing', '{"forms": ["W8-BEN"], "field": "date_of_birth"}')
) AS sink(sink_type, config)
WHERE d.name = 'person.date_of_birth';

-- person.nationality sinks
INSERT INTO "ob-poc".attribute_sinks (attribute_id, sink_type, sink_config)
SELECT d.attribute_id, sink.sink_type, sink.config::jsonb
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('kyc_report', '{"section": "identity", "field": "nationality"}'),
    ('sanctions_screening', '{"search_field": "country", "check_all_nationalities": true}'),
    ('jurisdiction_check', '{"blocked": ["KP", "IR", "SY"], "enhanced_dd": ["RU", "BY", "VE"]}'),
    ('tax_residency', '{"determines": "withholding_rate", "treaty_lookup": true}'),
    ('pep_screening', '{"high_risk_countries": true}')
) AS sink(sink_type, config)
WHERE d.name = 'person.nationality';

-- person.residential_address sinks
INSERT INTO "ob-poc".attribute_sinks (attribute_id, sink_type, sink_config)
SELECT d.attribute_id, sink.sink_type, sink.config::jsonb
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('kyc_report', '{"section": "address", "field": "residential_address"}'),
    ('sanctions_screening', '{"search_field": "address", "country_extraction": true}'),
    ('jurisdiction_check', '{"determines": "regulatory_regime"}'),
    ('correspondence', '{"mailing_address": true, "format": "local"}')
) AS sink(sink_type, config)
WHERE d.name = 'person.residential_address';

-- entity.company_name sinks
INSERT INTO "ob-poc".attribute_sinks (attribute_id, sink_type, sink_config)
SELECT d.attribute_id, sink.sink_type, sink.config::jsonb
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('kyc_report', '{"section": "corporate_identity", "field": "legal_entity_name"}'),
    ('sanctions_screening', '{"search_field": "entity_name", "fuzzy_match": true}'),
    ('beneficial_ownership', '{"entity_identification": true}'),
    ('regulatory_filing', '{"forms": ["W8-BEN-E"], "field": "name_of_organization"}')
) AS sink(sink_type, config)
WHERE d.name = 'entity.company_name';

-- entity.registration_number sinks
INSERT INTO "ob-poc".attribute_sinks (attribute_id, sink_type, sink_config)
SELECT d.attribute_id, sink.sink_type, sink.config::jsonb
FROM "ob-poc".dictionary d
CROSS JOIN (VALUES
    ('kyc_report', '{"section": "corporate_identity", "field": "registration_number"}'),
    ('registry_verification', '{"cross_check": true, "registry": "jurisdiction_dependent"}'),
    ('lei_lookup', '{"reference_data": true}')
) AS sink(sink_type, config)
WHERE d.name = 'entity.registration_number';

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Document type hierarchy
SELECT 
    type_code,
    parent_type_code,
    display_name
FROM "ob-poc".document_types
WHERE parent_type_code IS NOT NULL OR type_code = 'PASSPORT'
ORDER BY COALESCE(parent_type_code, type_code), type_code;

-- THE SPARSE MATRIX: Attributes × Document Categories
SELECT 
    d.name as attribute,
    array_agg(DISTINCT src.source_config->>'document_category' ORDER BY src.source_config->>'document_category') 
        FILTER (WHERE src.source_type = 'document') as document_sources,
    COUNT(*) FILTER (WHERE src.source_type = 'document') as doc_source_count,
    COUNT(*) FILTER (WHERE src.source_type = 'user_input') as has_user_input,
    COUNT(*) FILTER (WHERE src.source_type = 'registry_lookup') as has_registry
FROM "ob-poc".dictionary d
JOIN "ob-poc".attribute_sources src ON d.attribute_id = src.attribute_id
WHERE d.domain = 'KYC'
GROUP BY d.attribute_id, d.name
ORDER BY d.name;

-- Attribute → Sinks mapping
SELECT 
    d.name as attribute,
    array_agg(DISTINCT snk.sink_type ORDER BY snk.sink_type) as sinks
FROM "ob-poc".dictionary d
JOIN "ob-poc".attribute_sinks snk ON d.attribute_id = snk.attribute_id
WHERE d.domain = 'KYC'
GROUP BY d.attribute_id, d.name
ORDER BY d.name;

-- Full picture: sources AND sinks
SELECT 
    d.name as attribute,
    d.group_id,
    COUNT(DISTINCT src.id) as sources,
    COUNT(DISTINCT snk.id) as sinks
FROM "ob-poc".dictionary d
LEFT JOIN "ob-poc".attribute_sources src ON d.attribute_id = src.attribute_id
LEFT JOIN "ob-poc".attribute_sinks snk ON d.attribute_id = snk.attribute_id
WHERE d.domain = 'KYC'
GROUP BY d.attribute_id, d.name, d.group_id
ORDER BY d.group_id, d.name;
