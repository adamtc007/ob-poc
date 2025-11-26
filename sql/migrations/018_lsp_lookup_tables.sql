-- Migration: LSP Lookup Tables
-- Purpose: Add missing lookup tables required for DSL schema validation and LSP completions
-- Date: 2025-11-26

-- ============================================
-- 1. Create view for jurisdictions (alias for master_jurisdictions)
-- ============================================
CREATE OR REPLACE VIEW "ob-poc".jurisdictions AS
SELECT 
    jurisdiction_code AS iso_code,
    jurisdiction_name AS name,
    region,
    regulatory_framework AS description
FROM "ob-poc".master_jurisdictions;

-- ============================================
-- 2. Create attribute_dictionary table
-- ============================================
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_dictionary (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attr_id VARCHAR(100) UNIQUE NOT NULL,  -- e.g., "CBU.LEGAL_NAME", "PERSON.DATE_OF_BIRTH"
    attr_name VARCHAR(255) NOT NULL,        -- Human-readable name
    domain VARCHAR(50) NOT NULL,            -- "CBU", "ENTITY", "PERSON", "COMPANY", "DOCUMENT"
    data_type VARCHAR(50) NOT NULL DEFAULT 'STRING',  -- STRING, DATE, DECIMAL, BOOLEAN, UUID
    description TEXT,
    validation_pattern VARCHAR(255),        -- Regex for validation
    is_required BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed attribute dictionary with common attributes
INSERT INTO "ob-poc".attribute_dictionary (attr_id, attr_name, domain, data_type, description) VALUES
-- CBU attributes
('CBU.LEGAL_NAME', 'Legal Name', 'CBU', 'STRING', 'Official registered name of the CBU'),
('CBU.REGISTRATION_NUMBER', 'Registration Number', 'CBU', 'STRING', 'Official registration/incorporation number'),
('CBU.INCORPORATION_DATE', 'Incorporation Date', 'CBU', 'DATE', 'Date of formation/incorporation'),
('CBU.JURISDICTION', 'Jurisdiction', 'CBU', 'STRING', 'Country of registration (ISO code)'),
('CBU.NATURE_PURPOSE', 'Nature and Purpose', 'CBU', 'STRING', 'Business description and purpose'),
('CBU.CLIENT_TYPE', 'Client Type', 'CBU', 'STRING', 'Type of client structure'),
-- Person attributes
('PERSON.FULL_NAME', 'Full Name', 'PERSON', 'STRING', 'Complete legal name'),
('PERSON.FIRST_NAME', 'First Name', 'PERSON', 'STRING', 'First/given name'),
('PERSON.LAST_NAME', 'Last Name', 'PERSON', 'STRING', 'Last/family name'),
('PERSON.DATE_OF_BIRTH', 'Date of Birth', 'PERSON', 'DATE', 'Birth date'),
('PERSON.NATIONALITY', 'Nationality', 'PERSON', 'STRING', 'Country of citizenship (ISO code)'),
('PERSON.TAX_ID', 'Tax ID', 'PERSON', 'STRING', 'Tax identification number'),
('PERSON.RESIDENTIAL_ADDRESS', 'Residential Address', 'PERSON', 'STRING', 'Home address'),
-- Company attributes
('COMPANY.COMPANY_NAME', 'Company Name', 'COMPANY', 'STRING', 'Official company name'),
('COMPANY.COMPANY_NUMBER', 'Company Number', 'COMPANY', 'STRING', 'Official registration number'),
('COMPANY.REGISTERED_OFFICE', 'Registered Office', 'COMPANY', 'STRING', 'Official registered address'),
('COMPANY.SHARE_CAPITAL', 'Share Capital', 'COMPANY', 'DECIMAL', 'Authorized share capital amount'),
('COMPANY.INCORPORATION_DATE', 'Incorporation Date', 'COMPANY', 'DATE', 'Date of incorporation'),
-- Document attributes
('DOCUMENT.ISSUE_DATE', 'Issue Date', 'DOCUMENT', 'DATE', 'Date document was issued'),
('DOCUMENT.EXPIRY_DATE', 'Expiry Date', 'DOCUMENT', 'DATE', 'Date document expires'),
('DOCUMENT.ISSUER', 'Issuer', 'DOCUMENT', 'STRING', 'Entity that issued the document'),
('DOCUMENT.DOCUMENT_NUMBER', 'Document Number', 'DOCUMENT', 'STRING', 'Official document reference number')
ON CONFLICT (attr_id) DO NOTHING;

-- ============================================
-- 3. Create screening_lists table
-- ============================================
CREATE TABLE IF NOT EXISTS "ob-poc".screening_lists (
    screening_list_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_code VARCHAR(50) UNIQUE NOT NULL,
    list_name VARCHAR(255) NOT NULL,
    list_type VARCHAR(50) NOT NULL,  -- 'SANCTIONS', 'PEP', 'ADVERSE_MEDIA', 'WATCHLIST'
    provider VARCHAR(100),
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed screening lists
INSERT INTO "ob-poc".screening_lists (list_code, list_name, list_type, provider, description) VALUES
('OFAC_SDN', 'OFAC SDN List', 'SANCTIONS', 'US Treasury', 'US Specially Designated Nationals'),
('EU_SANCTIONS', 'EU Consolidated Sanctions', 'SANCTIONS', 'European Union', 'EU consolidated list of persons subject to financial sanctions'),
('UN_SANCTIONS', 'UN Security Council Sanctions', 'SANCTIONS', 'United Nations', 'UN Security Council consolidated list'),
('UK_SANCTIONS', 'UK Sanctions List', 'SANCTIONS', 'UK OFSI', 'UK Office of Financial Sanctions Implementation list'),
('WORLD_CHECK_PEP', 'World-Check PEP', 'PEP', 'Refinitiv', 'Politically Exposed Persons database'),
('DOW_JONES_PEP', 'Dow Jones PEP', 'PEP', 'Dow Jones', 'Dow Jones Risk & Compliance PEP list'),
('ADVERSE_MEDIA', 'Adverse Media Screening', 'ADVERSE_MEDIA', 'Various', 'Negative news and media screening'),
('FATF_HIGH_RISK', 'FATF High-Risk Jurisdictions', 'WATCHLIST', 'FATF', 'FATF list of high-risk and non-cooperative jurisdictions')
ON CONFLICT (list_code) DO NOTHING;

-- ============================================
-- 4. Create currencies table
-- ============================================
CREATE TABLE IF NOT EXISTS "ob-poc".currencies (
    currency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code VARCHAR(3) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    symbol VARCHAR(10),
    decimal_places INTEGER DEFAULT 2,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed currencies
INSERT INTO "ob-poc".currencies (iso_code, name, symbol, decimal_places) VALUES
('USD', 'US Dollar', '$', 2),
('EUR', 'Euro', '€', 2),
('GBP', 'British Pound', '£', 2),
('CHF', 'Swiss Franc', 'CHF', 2),
('JPY', 'Japanese Yen', '¥', 0),
('CNY', 'Chinese Yuan', '¥', 2),
('HKD', 'Hong Kong Dollar', 'HK$', 2),
('SGD', 'Singapore Dollar', 'S$', 2),
('AUD', 'Australian Dollar', 'A$', 2),
('CAD', 'Canadian Dollar', 'C$', 2),
('NZD', 'New Zealand Dollar', 'NZ$', 2),
('SEK', 'Swedish Krona', 'kr', 2),
('NOK', 'Norwegian Krone', 'kr', 2),
('DKK', 'Danish Krone', 'kr', 2),
('KYD', 'Cayman Islands Dollar', 'CI$', 2),
('BVI', 'British Virgin Islands (USD)', '$', 2)
ON CONFLICT (iso_code) DO NOTHING;

-- ============================================
-- 5. Add category column to document_types if not exists
-- ============================================
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_types' 
        AND column_name = 'category'
    ) THEN
        ALTER TABLE "ob-poc".document_types ADD COLUMN category VARCHAR(50);
    END IF;
END $$;

-- Update document types with categories
UPDATE "ob-poc".document_types SET category = 'Corporate' WHERE type_code IN ('CERT_OF_INCORP', 'ARTICLES_OF_ASSOC', 'SHARE_REGISTER', 'MEMORANDUM_OF_ASSOC', 'CERT_GOOD_STANDING');
UPDATE "ob-poc".document_types SET category = 'Identity' WHERE type_code IN ('PASSPORT', 'NATIONAL_ID', 'DRIVING_LICENSE');
UPDATE "ob-poc".document_types SET category = 'Address' WHERE type_code IN ('UTILITY_BILL', 'PROOF_OF_ADDRESS');
UPDATE "ob-poc".document_types SET category = 'Financial' WHERE type_code IN ('BANK_STATEMENT', 'AUDITED_ACCOUNTS', 'TAX_RETURN');
UPDATE "ob-poc".document_types SET category = 'Legal' WHERE type_code IN ('TRUST_DEED', 'PARTNERSHIP_AGREEMENT');

-- ============================================
-- 6. Create indexes for lookup performance
-- ============================================
CREATE INDEX IF NOT EXISTS idx_attribute_dictionary_domain ON "ob-poc".attribute_dictionary(domain);
CREATE INDEX IF NOT EXISTS idx_attribute_dictionary_active ON "ob-poc".attribute_dictionary(is_active);
CREATE INDEX IF NOT EXISTS idx_screening_lists_type ON "ob-poc".screening_lists(list_type);
CREATE INDEX IF NOT EXISTS idx_currencies_active ON "ob-poc".currencies(is_active);
