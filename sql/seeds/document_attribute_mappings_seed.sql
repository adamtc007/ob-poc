-- Seed data for document types and document-attribute mappings
-- This demonstrates the bi-directional relationship:
-- - A document type (passport) can provide many attributes
-- - An attribute (full_name) can come from multiple document types

BEGIN;

-- ============================================================================
-- DOCUMENT TYPES
-- ============================================================================

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description)
VALUES 
    ('a1000000-0000-0000-0000-000000000001'::uuid, 'PASSPORT', 'Passport', 'IDENTITY', 
     'Government-issued travel document proving identity and nationality'),
    ('a1000000-0000-0000-0000-000000000002'::uuid, 'DRIVERS_LICENSE', 'Drivers License', 'IDENTITY',
     'Government-issued driving permit, often used as secondary ID'),
    ('a1000000-0000-0000-0000-000000000003'::uuid, 'CERT_OF_INCORPORATION', 'Certificate of Incorporation', 'CORPORATE',
     'Official document proving company registration and formation'),
    ('a1000000-0000-0000-0000-000000000004'::uuid, 'UTILITY_BILL', 'Utility Bill', 'ADDRESS',
     'Recent utility bill for address verification'),
    ('a1000000-0000-0000-0000-000000000005'::uuid, 'BANK_STATEMENT', 'Bank Statement', 'FINANCIAL',
     'Bank account statement for financial verification')
ON CONFLICT (type_code) DO UPDATE SET 
    display_name = EXCLUDED.display_name,
    category = EXCLUDED.category,
    description = EXCLUDED.description;

-- ============================================================================
-- CONSOLIDATED ATTRIBUTES
-- These are the attributes that can be extracted from documents
-- ============================================================================

-- Identity attributes
INSERT INTO "ob-poc".consolidated_attributes (
    attribute_id, attribute_code, attribute_name, data_type, category, subcategory,
    description, privacy_classification, ai_extraction_guidance, business_context, source_documents
) VALUES
    ('b1000000-0000-0000-0000-000000000001'::uuid, 'FULL_NAME', 'Full Legal Name', 'string', 
     'IDENTITY', 'PERSONAL',
     'Complete legal name as it appears on official documents',
     'pii', 
     'Look for name field, usually at top of document. May be split into given/family names.',
     'Primary identifier for individuals, required for KYC',
     ARRAY['PASSPORT', 'DRIVERS_LICENSE', 'UTILITY_BILL']),
    
    ('b1000000-0000-0000-0000-000000000002'::uuid, 'DATE_OF_BIRTH', 'Date of Birth', 'date',
     'IDENTITY', 'PERSONAL',
     'Birth date of the individual',
     'pii',
     'Usually in DD/MM/YYYY or YYYY-MM-DD format. Check MRZ on passports.',
     'Required for identity verification and age confirmation',
     ARRAY['PASSPORT', 'DRIVERS_LICENSE']),
    
    ('b1000000-0000-0000-0000-000000000003'::uuid, 'NATIONALITY', 'Nationality/Citizenship', 'string',
     'IDENTITY', 'PERSONAL', 
     'Country of citizenship',
     'internal',
     'Three-letter country code in MRZ, or full country name in main section.',
     'Required for sanctions screening and regulatory jurisdiction',
     ARRAY['PASSPORT']),
    
    ('b1000000-0000-0000-0000-000000000004'::uuid, 'PASSPORT_NUMBER', 'Passport Number', 'string',
     'IDENTITY', 'DOCUMENT_ID',
     'Unique passport document number',
     'pii',
     'Alphanumeric code, typically 8-9 characters. Found in MRZ and main page.',
     'Primary document identifier for passport verification',
     ARRAY['PASSPORT']),
    
    ('b1000000-0000-0000-0000-000000000005'::uuid, 'DOC_ISSUE_DATE', 'Document Issue Date', 'date',
     'DOCUMENT', 'VALIDITY',
     'Date the document was issued',
     'internal',
     'Look for "Date of Issue" field. Format varies by country.',
     'Used to verify document validity and detect expired documents',
     ARRAY['PASSPORT', 'DRIVERS_LICENSE', 'CERT_OF_INCORPORATION']),
    
    ('b1000000-0000-0000-0000-000000000006'::uuid, 'DOC_EXPIRY_DATE', 'Document Expiry Date', 'date',
     'DOCUMENT', 'VALIDITY',
     'Date the document expires',
     'internal',
     'Usually near issue date. Critical for document validity checks.',
     'Documents must be valid (not expired) for KYC acceptance',
     ARRAY['PASSPORT', 'DRIVERS_LICENSE']),
    
    ('b1000000-0000-0000-0000-000000000007'::uuid, 'PLACE_OF_BIRTH', 'Place of Birth', 'string',
     'IDENTITY', 'PERSONAL',
     'City/country of birth',
     'pii',
     'May be city only, or city + country. Varies by passport issuer.',
     'Used for identity verification and sanction screening',
     ARRAY['PASSPORT']),
    
    ('b1000000-0000-0000-0000-000000000008'::uuid, 'GENDER', 'Gender', 'string',
     'IDENTITY', 'PERSONAL',
     'Gender as recorded on official document (M/F/X)',
     'pii',
     'Single letter in MRZ (M/F/X). May be spelled out in main section.',
     'Required for identity verification against other records',
     ARRAY['PASSPORT', 'DRIVERS_LICENSE']),

-- Address attributes
    ('b1000000-0000-0000-0000-000000000009'::uuid, 'RESIDENTIAL_ADDRESS', 'Residential Address', 'string',
     'ADDRESS', 'RESIDENTIAL',
     'Current residential address',
     'pii',
     'Full street address including unit number, city, postcode, country.',
     'Required for address verification and correspondence',
     ARRAY['UTILITY_BILL', 'BANK_STATEMENT', 'DRIVERS_LICENSE']),

-- Corporate attributes  
    ('b1000000-0000-0000-0000-000000000010'::uuid, 'COMPANY_NAME', 'Legal Entity Name', 'string',
     'CORPORATE', 'IDENTITY',
     'Registered legal name of the company',
     'public',
     'Usually prominently displayed. Must match exactly with registry.',
     'Primary identifier for corporate entities',
     ARRAY['CERT_OF_INCORPORATION', 'BANK_STATEMENT']),
    
    ('b1000000-0000-0000-0000-000000000011'::uuid, 'REGISTRATION_NUMBER', 'Company Registration Number', 'string',
     'CORPORATE', 'IDENTITY',
     'Official company registration/incorporation number',
     'public',
     'Format varies by jurisdiction (e.g., UK: 8 digits, US: varies by state).',
     'Unique identifier for company verification against registries',
     ARRAY['CERT_OF_INCORPORATION']),
    
    ('b1000000-0000-0000-0000-000000000012'::uuid, 'INCORPORATION_DATE', 'Date of Incorporation', 'date',
     'CORPORATE', 'FORMATION',
     'Date the company was legally formed',
     'public',
     'Look for "Date of Incorporation" or "Formation Date" field.',
     'Used to verify company history and age for risk assessment',
     ARRAY['CERT_OF_INCORPORATION']),
    
    ('b1000000-0000-0000-0000-000000000013'::uuid, 'REGISTERED_ADDRESS', 'Registered Office Address', 'string',
     'CORPORATE', 'ADDRESS',
     'Official registered office address of the company',
     'public',
     'Legal address for official correspondence. May differ from trading address.',
     'Required for legal notices and regulatory filings',
     ARRAY['CERT_OF_INCORPORATION'])

ON CONFLICT (attribute_code) DO UPDATE SET
    attribute_name = EXCLUDED.attribute_name,
    data_type = EXCLUDED.data_type,
    category = EXCLUDED.category,
    subcategory = EXCLUDED.subcategory,
    description = EXCLUDED.description,
    privacy_classification = EXCLUDED.privacy_classification,
    ai_extraction_guidance = EXCLUDED.ai_extraction_guidance,
    business_context = EXCLUDED.business_context,
    source_documents = EXCLUDED.source_documents;

-- ============================================================================
-- DOCUMENT-ATTRIBUTE MAPPINGS
-- Links document types to the attributes they can provide
-- ============================================================================

-- PASSPORT mappings (8 attributes)
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required, 
    field_location_hints, ai_extraction_notes
) VALUES
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000001'::uuid, 1, true,
     ARRAY['surname_field', 'given_names_field', 'mrz_line_1'],
     'Combine surname and given names. Verify against MRZ.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000002'::uuid, 2, true,
     ARRAY['date_of_birth_field', 'mrz_dob'],
     'Format: DD MMM YYYY or in MRZ as YYMMDD. Cross-validate.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000003'::uuid, 3, true,
     ARRAY['nationality_field', 'mrz_nationality'],
     'Three-letter ISO code in MRZ. May be full name elsewhere.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000004'::uuid, 4, true,
     ARRAY['passport_number_field', 'mrz_doc_number'],
     'Alphanumeric, typically 8-9 chars. Must match MRZ exactly.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000005'::uuid, 5, true,
     ARRAY['date_of_issue_field'],
     'May not be in MRZ. Look for "Date of Issue" label.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000006'::uuid, 6, true,
     ARRAY['expiry_date_field', 'mrz_expiry'],
     'Critical for validity. Reject if expired. YYMMDD in MRZ.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000007'::uuid, 7, false,
     ARRAY['place_of_birth_field'],
     'Not always present. May be city or country or both.'),
    ('PASSPORT', 'b1000000-0000-0000-0000-000000000008'::uuid, 8, true,
     ARRAY['sex_field', 'mrz_sex'],
     'M, F, or X. Single character in MRZ.')

-- DRIVERS_LICENSE mappings (5 attributes)
    ,('DRIVERS_LICENSE', 'b1000000-0000-0000-0000-000000000001'::uuid, 1, true,
     ARRAY['full_name_field', 'surname_field', 'first_name_field'],
     'Name format varies by jurisdiction. May be single field or split.')
    ,('DRIVERS_LICENSE', 'b1000000-0000-0000-0000-000000000002'::uuid, 2, true,
     ARRAY['dob_field'],
     'Usually DD/MM/YYYY format.')
    ,('DRIVERS_LICENSE', 'b1000000-0000-0000-0000-000000000005'::uuid, 3, true,
     ARRAY['issue_date_field'],
     'Date license was issued.')
    ,('DRIVERS_LICENSE', 'b1000000-0000-0000-0000-000000000006'::uuid, 4, true,
     ARRAY['expiry_date_field'],
     'License expiration date.')
    ,('DRIVERS_LICENSE', 'b1000000-0000-0000-0000-000000000008'::uuid, 5, false,
     ARRAY['sex_field'],
     'May be M/F or Male/Female.')
    ,('DRIVERS_LICENSE', 'b1000000-0000-0000-0000-000000000009'::uuid, 6, true,
     ARRAY['address_field'],
     'Residential address on license.')

-- CERT_OF_INCORPORATION mappings (4 attributes)
    ,('CERT_OF_INCORPORATION', 'b1000000-0000-0000-0000-000000000010'::uuid, 1, true,
     ARRAY['company_name_field', 'entity_name_field'],
     'Exact legal name as registered. Watch for Ltd/Limited variations.')
    ,('CERT_OF_INCORPORATION', 'b1000000-0000-0000-0000-000000000011'::uuid, 2, true,
     ARRAY['registration_number_field', 'company_number_field'],
     'Unique identifier. Format varies by jurisdiction.')
    ,('CERT_OF_INCORPORATION', 'b1000000-0000-0000-0000-000000000012'::uuid, 3, true,
     ARRAY['incorporation_date_field', 'formation_date_field'],
     'Date company was legally formed.')
    ,('CERT_OF_INCORPORATION', 'b1000000-0000-0000-0000-000000000005'::uuid, 4, false,
     ARRAY['certificate_date_field'],
     'Date this certificate was issued (may differ from incorporation date).')
    ,('CERT_OF_INCORPORATION', 'b1000000-0000-0000-0000-000000000013'::uuid, 5, true,
     ARRAY['registered_office_field', 'registered_address_field'],
     'Official registered office address.')

-- UTILITY_BILL mappings (2 attributes)
    ,('UTILITY_BILL', 'b1000000-0000-0000-0000-000000000001'::uuid, 1, true,
     ARRAY['account_holder_name', 'customer_name'],
     'Name on the account. May include title (Mr/Mrs).')
    ,('UTILITY_BILL', 'b1000000-0000-0000-0000-000000000009'::uuid, 2, true,
     ARRAY['service_address', 'supply_address'],
     'Address where service is provided. Verify is residential.')

-- BANK_STATEMENT mappings (3 attributes)
    ,('BANK_STATEMENT', 'b1000000-0000-0000-0000-000000000001'::uuid, 1, true,
     ARRAY['account_holder_name'],
     'Name on the bank account.')
    ,('BANK_STATEMENT', 'b1000000-0000-0000-0000-000000000009'::uuid, 2, true,
     ARRAY['correspondence_address'],
     'Address on statement.')
    ,('BANK_STATEMENT', 'b1000000-0000-0000-0000-000000000010'::uuid, 3, false,
     ARRAY['account_name_if_business'],
     'For business accounts, may show company name.')

ON CONFLICT DO NOTHING;

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Show document types and their attribute counts
SELECT 
    dt.type_code,
    dt.display_name,
    dt.category,
    COUNT(dam.attribute_id) as attribute_count
FROM "ob-poc".document_types dt
LEFT JOIN "ob-poc".document_attribute_mappings dam ON dt.type_code = dam.document_type_code
GROUP BY dt.type_id, dt.type_code, dt.display_name, dt.category
ORDER BY attribute_count DESC;

-- Show PASSPORT attributes in extraction order
SELECT 
    dam.extraction_priority,
    ca.attribute_code,
    ca.attribute_name,
    ca.data_type,
    dam.is_required,
    dam.field_location_hints[1] as primary_hint
FROM "ob-poc".document_attribute_mappings dam
JOIN "ob-poc".consolidated_attributes ca ON dam.attribute_id = ca.attribute_id
WHERE dam.document_type_code = 'PASSPORT'
ORDER BY dam.extraction_priority;

-- Show which documents can provide FULL_NAME
SELECT 
    dam.document_type_code,
    dt.display_name,
    dam.extraction_priority,
    dam.is_required
FROM "ob-poc".document_attribute_mappings dam
JOIN "ob-poc".document_types dt ON dam.document_type_code = dt.type_code
WHERE dam.attribute_id = 'b1000000-0000-0000-0000-000000000001'::uuid
ORDER BY dam.extraction_priority;
