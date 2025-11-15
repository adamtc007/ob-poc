-- ============================================================================
-- Seed Document Types and Attribute Mappings
-- ============================================================================
-- Populates document types and their extractable attributes
-- Uses ONLY existing UUIDs from attribute_registry

SET search_path TO "ob-poc";

-- ============================================================================
-- 1. SEED DOCUMENT TYPES
-- ============================================================================

INSERT INTO "ob-poc".document_types
(type_code, display_name, category, domain, description)
VALUES
('PASSPORT', 'Passport', 'IDENTITY', 'KYC', 'International travel document'),
('DRIVING_LICENSE', 'Driving License', 'IDENTITY', 'KYC', 'Government-issued driving permit'),
('NATIONAL_ID', 'National ID Card', 'IDENTITY', 'KYC', 'National identification document'),
('BANK_STATEMENT', 'Bank Statement', 'FINANCIAL', 'KYC', 'Bank account statement'),
('UTILITY_BILL', 'Utility Bill', 'PROOF_OF_ADDRESS', 'KYC', 'Utility service bill for address verification'),
('TAX_RETURN', 'Tax Return', 'FINANCIAL', 'TAX', 'Annual tax filing document'),
('ARTICLES_OF_INCORPORATION', 'Articles of Incorporation', 'CORPORATE', 'ENTITY', 'Company formation document'),
('FINANCIAL_STATEMENT', 'Financial Statement', 'FINANCIAL', 'ENTITY', 'Audited financial statements'),
('BUSINESS_LICENSE', 'Business License', 'CORPORATE', 'ENTITY', 'Business operating license'),
('PROOF_OF_FUNDS', 'Proof of Funds', 'FINANCIAL', 'KYC', 'Document proving availability of funds')
ON CONFLICT (type_code) DO NOTHING;

-- ============================================================================
-- 2. SEED PASSPORT ATTRIBUTE MAPPINGS
-- ============================================================================

INSERT INTO "ob-poc".document_attribute_mappings
(document_type_id, attribute_uuid, extraction_method, is_required, confidence_threshold)
VALUES
-- PASSPORT -> Identity Attributes (using MRZ extraction)
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '3020d46f-472c-5437-9647-1b0682c35935', -- attr.identity.first_name
 'MRZ', true, 0.95),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '0af112fd-ec04-5938-84e8-6e5949db0b52', -- attr.identity.last_name
 'MRZ', true, 0.95),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 'c09501c7-2ea9-5ad7-b330-7d664c678e37', -- attr.identity.passport_number
 'MRZ', true, 0.98),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '1211e18e-fffe-5e17-9836-fb3cd70452d3', -- attr.identity.date_of_birth
 'MRZ', true, 0.95),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '33d0752b-a92c-5e20-8559-43ab3668ecf5', -- attr.identity.nationality
 'MRZ', true, 0.90)
ON CONFLICT (document_type_id, attribute_uuid) DO NOTHING;

-- ============================================================================
-- 3. SEED BANK STATEMENT ATTRIBUTE MAPPINGS
-- ============================================================================

INSERT INTO "ob-poc".document_attribute_mappings
(document_type_id, attribute_uuid, extraction_method, is_required, confidence_threshold)
VALUES
-- BANK_STATEMENT -> Financial Attributes (using OCR)
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'BANK_STATEMENT'),
 'd022d8f1-8ae1-55c8-84b8-e8203e17e369', -- attr.banking.account_number
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'BANK_STATEMENT'),
 '5fb01b57-e622-53b4-a503-dfced456fae2', -- attr.banking.bank_name
 'OCR', true, 0.85),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'BANK_STATEMENT'),
 '6fd0e89d-5ce9-5e96-b359-be0867643f27', -- attr.banking.iban
 'OCR', false, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'BANK_STATEMENT'),
 '7a658a7c-6865-5941-a069-277c42e10492', -- attr.banking.swift_code
 'OCR', false, 0.85)
ON CONFLICT (document_type_id, attribute_uuid) DO NOTHING;

-- ============================================================================
-- 4. SEED UTILITY BILL ATTRIBUTE MAPPINGS
-- ============================================================================

INSERT INTO "ob-poc".document_attribute_mappings
(document_type_id, attribute_uuid, extraction_method, is_required, confidence_threshold)
VALUES
-- UTILITY_BILL -> Address Attributes (using OCR)
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'UTILITY_BILL'),
 '7c7cdc82-b261-57c7-aee2-36e17dcd1d5d', -- attr.contact.address_line1
 'OCR', true, 0.85),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'UTILITY_BILL'),
 'e90b9045-d0c6-52db-989a-96ad37152e3a', -- attr.contact.address_line2
 'OCR', false, 0.80),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'UTILITY_BILL'),
 '2eca2245-5d14-57b2-9a53-6f90d2b7a9d6', -- attr.contact.city
 'OCR', true, 0.85),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'UTILITY_BILL'),
 '36df5ec0-f1b8-50a0-ac50-b510f0cda2fb', -- attr.contact.postal_code
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'UTILITY_BILL'),
 '24e2072e-db54-547b-9e5d-0762a26261a6', -- attr.contact.country
 'OCR', true, 0.85)
ON CONFLICT (document_type_id, attribute_uuid) DO NOTHING;

-- ============================================================================
-- 5. SEED NATIONAL ID ATTRIBUTE MAPPINGS
-- ============================================================================

INSERT INTO "ob-poc".document_attribute_mappings
(document_type_id, attribute_uuid, extraction_method, is_required, confidence_threshold)
VALUES
-- NATIONAL_ID -> Identity Attributes
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'NATIONAL_ID'),
 '3020d46f-472c-5437-9647-1b0682c35935', -- attr.identity.first_name
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'NATIONAL_ID'),
 '0af112fd-ec04-5938-84e8-6e5949db0b52', -- attr.identity.last_name
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'NATIONAL_ID'),
 '1211e18e-fffe-5e17-9836-fb3cd70452d3', -- attr.identity.date_of_birth
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'NATIONAL_ID'),
 '33d0752b-a92c-5e20-8559-43ab3668ecf5', -- attr.identity.nationality
 'OCR', true, 0.85)
ON CONFLICT (document_type_id, attribute_uuid) DO NOTHING;

-- ============================================================================
-- 6. SEED ARTICLES OF INCORPORATION MAPPINGS
-- ============================================================================

INSERT INTO "ob-poc".document_attribute_mappings
(document_type_id, attribute_uuid, extraction_method, is_required, confidence_threshold)
VALUES
-- ARTICLES_OF_INCORPORATION -> Entity Attributes
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'ARTICLES_OF_INCORPORATION'),
 'd655aadd-3605-5490-80be-20e6202b004b', -- attr.identity.legal_name
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'ARTICLES_OF_INCORPORATION'),
 '57b3ac74-182e-5ca6-b94c-46ee2a05998b', -- attr.identity.registration_number
 'OCR', true, 0.90),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'ARTICLES_OF_INCORPORATION'),
 '132a5d3c-e809-5978-ab54-ccacfcbeb4aa', -- attr.identity.incorporation_date
 'OCR', true, 0.85),

((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'ARTICLES_OF_INCORPORATION'),
 '78883df8-c953-5d6e-90c2-c95ade0243bd', -- attr.entity.domicile
 'OCR', true, 0.85)
ON CONFLICT (document_type_id, attribute_uuid) DO NOTHING;

COMMENT ON TABLE "ob-poc".document_attribute_mappings IS 'Seeded with common document type to attribute mappings for KYC and onboarding';
