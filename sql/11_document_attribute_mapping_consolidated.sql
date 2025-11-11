-- 11_document_attribute_mapping_consolidated.sql
-- Consolidated Document-Attribute Mapping for DSL-as-State Data Strategy
--
-- This script creates the foundational bridge between unstructured document contents
-- and structured AttributeID-typed data, eliminating duplicates across document types.
--
-- KEY PRINCIPLE: AttributeID-as-Type Pattern
-- - Each data field has ONE universal AttributeID regardless of source document
-- - AI agents extract data using consistent AttributeIDs across all document types
-- - Cross-document validation and reconciliation becomes possible
-- - Complete audit trail from document source to DSL state

-- ============================================================================
-- CONSOLIDATED ATTRIBUTE DICTIONARY
-- Deduplicated attributes that appear across multiple document types
-- ============================================================================

-- First, ensure we have the consolidated dictionary structure
CREATE TABLE IF NOT EXISTS "ob-poc".consolidated_attributes (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attribute_code VARCHAR(100) NOT NULL UNIQUE,
    attribute_name VARCHAR(200) NOT NULL,
    data_type VARCHAR(50) NOT NULL,
    category VARCHAR(100) NOT NULL,
    subcategory VARCHAR(100),
    description TEXT NOT NULL,
    privacy_classification VARCHAR(20) DEFAULT 'internal',
    validation_rules JSONB,
    extraction_patterns JSONB,
    ai_extraction_guidance TEXT NOT NULL,
    business_context TEXT NOT NULL,
    regulatory_significance TEXT,
    cross_document_validation TEXT,
    source_documents TEXT[] NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create document-attribute mapping table
CREATE TABLE IF NOT EXISTS "ob-poc".document_attribute_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_code VARCHAR(50) NOT NULL,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".consolidated_attributes(attribute_id),
    extraction_priority INTEGER DEFAULT 5, -- 1=critical, 10=optional
    is_required BOOLEAN DEFAULT false,
    field_location_hints TEXT[], -- Where to find this field in the document
    validation_cross_refs TEXT[], -- Other attributes to cross-validate against
    ai_extraction_notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_consolidated_attr_code ON "ob-poc".consolidated_attributes (attribute_code);
CREATE INDEX IF NOT EXISTS idx_consolidated_attr_category ON "ob-poc".consolidated_attributes (category);
CREATE INDEX IF NOT EXISTS idx_doc_attr_mapping_doc_type ON "ob-poc".document_attribute_mappings (document_type_code);
CREATE INDEX IF NOT EXISTS idx_doc_attr_mapping_attr_id ON "ob-poc".document_attribute_mappings (attribute_id);

-- ============================================================================
-- CORE ENTITY ATTRIBUTES (appear in most documents)
-- ============================================================================

INSERT INTO "ob-poc".consolidated_attributes (
    attribute_code, attribute_name, data_type, category, subcategory,
    description, privacy_classification, validation_rules, extraction_patterns,
    ai_extraction_guidance, business_context, regulatory_significance,
    cross_document_validation, source_documents
) VALUES

-- Entity Legal Name (appears in 15+ document types)
('entity.legal_name', 'Legal Entity Name', 'string', 'Entity Identity', 'Core Identity',
 'Official legal name of entity exactly as registered with authorities',
 'internal',
 '{"max_length": 200, "required": true, "pattern": "^[A-Za-z0-9\\s\\.\\,\\-\\&\\(\\)]+$"}',
 '{"common_fields": ["company_name", "legal_name", "entity_name", "corporation_name", "fund_name"], "case_sensitive": true}',
 'Extract the complete legal name exactly as it appears on the official document. Do not abbreviate or modify. Look for phrases like "Legal Name:", "Company Name:", or similar headers. In certificates of incorporation, this is typically the first major field. In contracts, it appears in the party identification sections. Ensure exact character matching including punctuation, spaces, and capitalization.',
 'Primary identifier for legal entities across all document types. Used for KYC, compliance screening, and cross-document validation. Must match exactly across all documents for the same entity.',
 'Required for AML screening, sanctions checking, KYC verification, and regulatory reporting. Discrepancies indicate potential compliance issues.',
 'Must match exactly across: Certificate of Incorporation, Articles of Association, Bank Statements, Contracts, Tax Forms, and all other entity documents',
 ARRAY['CERT_INCORPORATION', 'ARTICLES_OF_ASSOCIATION', 'BANK_STATEMENT', 'AUDITED_FINANCIAL_STATEMENTS', 'FORM_W8BEN_E', 'ISDA_MASTER_AGREEMENT', 'SUBSCRIPTION_AGREEMENT', 'INVESTMENT_MANAGEMENT_AGREEMENT']),

-- Registration/Company Number (appears in 10+ document types)
('entity.registration_number', 'Registration/Company Number', 'string', 'Entity Identity', 'Official Numbers',
 'Official registration or company number assigned by government registry',
 'internal',
 '{"max_length": 50, "required": true, "pattern": "^[A-Z0-9\\-\\.\\s]+$"}',
 '{"common_fields": ["company_number", "registration_number", "corp_number", "entity_number", "file_number"], "format_varies_by_jurisdiction": true}',
 'Extract the official registration number assigned by the government registry. Format varies by jurisdiction: UK uses 8-digit numbers, Delaware uses 7-digit numbers, Singapore uses registration number with letters. Look for "Company No:", "Registration No:", "Corp No:" labels. In certificates, this is usually prominently displayed. Verify format matches jurisdiction standards.',
 'Unique government-assigned identifier for legal entities. Essential for corporate registry verification and cross-document validation.',
 'Required for corporate registry verification, good standing confirmation, and regulatory filings. Used to verify entity existence and status.',
 'Must be consistent across all documents and match jurisdiction format standards. Cross-validate with corporate registry databases.',
 ARRAY['CERT_INCORPORATION', 'ARTICLES_OF_ASSOCIATION', 'GOOD_STANDING_CERTIFICATE', 'CORPORATE_REGISTRY_EXTRACT', 'TAX_RETURNS', 'ANNUAL_RETURNS']),

-- Jurisdiction of Incorporation
('entity.jurisdiction_incorporation', 'Jurisdiction of Incorporation', 'string', 'Entity Identity', 'Legal Jurisdiction',
 'Country, state, or territory where the entity was legally formed',
 'internal',
 '{"required": true, "enum": ["US", "UK", "CA", "AU", "SG", "HK", "KY", "BVI", "LU", "IE", "DE", "FR", "NL", "CH", "JE", "GG", "IM"]}',
 '{"common_fields": ["jurisdiction", "country_of_incorporation", "state_of_incorporation", "domicile"], "iso_country_codes": true}',
 'Identify the legal jurisdiction where the entity was incorporated or formed. Look for "Incorporated in", "State of Incorporation", "Country of Formation" or similar. For US entities, extract both country (US) and state (DE, NY, CA, etc.). For offshore jurisdictions, use standard codes (KY=Cayman Islands, BVI=British Virgin Islands). Verify against known jurisdiction codes.',
 'Legal domicile determines applicable laws, regulations, and tax treatment. Critical for regulatory compliance and legal proceedings.',
 'Determines applicable corporate law, tax regulations, reporting requirements, and regulatory oversight. Essential for compliance assessment.',
 'Must be consistent across formation documents and match entity type. Cross-validate with regulatory requirements for that jurisdiction.',
 ARRAY['CERT_INCORPORATION', 'ARTICLES_OF_ASSOCIATION', 'MEMORANDUM_OF_ASSOCIATION', 'FORM_W8BEN_E', 'CRS_SELF_CERTIFICATION', 'TAX_RESIDENCY_CERTIFICATE']),

-- ============================================================================
-- INDIVIDUAL IDENTITY ATTRIBUTES (appear in identity documents)
-- ============================================================================

-- Full Name
('individual.full_name', 'Individual Full Name', 'string', 'Individual Identity', 'Core Identity',
 'Complete legal name of individual exactly as on official government documents',
 'PII',
 '{"max_length": 150, "required": true, "pattern": "^[A-Za-z\\s\\.\\,\\-\\']+$"}',
 '{"common_fields": ["full_name", "legal_name", "name", "given_names", "surname"], "name_order_varies": true}',
 'Extract the complete legal name exactly as shown on the identity document. Include all given names, middle names, and surnames. Preserve exact spelling, punctuation, and spacing. For passports, use the name field, not the machine-readable zone unless specifically required. Account for different naming conventions (Western: Given+Family, Asian: Family+Given, etc.).',
 'Primary legal identity for individuals. Used for KYC verification, account opening, and compliance screening.',
 'Required for AML compliance, sanctions screening, PEP checking, and identity verification. Must match across all documents.',
 'Must match exactly across passport, national ID, driver license, and all financial documents. Variations require explanation.',
 ARRAY['PASSPORT', 'NATIONAL_ID_CARD', 'DRIVERS_LICENSE', 'BIRTH_CERTIFICATE', 'BANK_STATEMENTS', 'SUBSCRIPTION_AGREEMENT']),

-- Date of Birth
('individual.date_of_birth', 'Date of Birth', 'date', 'Individual Identity', 'Personal Details',
 'Individual date of birth in ISO 8601 format (YYYY-MM-DD)',
 'PII',
 '{"required": true, "format": "YYYY-MM-DD", "min_age_years": 18, "max_age_years": 120}',
 '{"common_fields": ["date_of_birth", "birth_date", "dob"], "date_formats": ["DD/MM/YYYY", "MM/DD/YYYY", "DD-MM-YYYY", "YYYY-MM-DD"]}',
 'Extract date of birth and standardize to ISO 8601 format (YYYY-MM-DD). Account for different date formats by country: US uses MM/DD/YYYY, most others use DD/MM/YYYY. Verify logical date (not future, not impossibly old). Cross-reference with calculated age if provided elsewhere in document.',
 'Essential for age verification, identity confirmation, and regulatory compliance. Used for investment suitability and legal capacity assessment.',
 'Required for age verification for investment products, legal capacity assessment, and identity verification. Critical for compliance.',
 'Must be consistent across all identity documents. Discrepancies require investigation and resolution.',
 ARRAY['PASSPORT', 'NATIONAL_ID_CARD', 'DRIVERS_LICENSE', 'BIRTH_CERTIFICATE']),

-- Nationality/Citizenship
('individual.nationality', 'Nationality/Citizenship', 'string', 'Individual Identity', 'Legal Status',
 'Legal nationality or citizenship of individual',
 'internal',
 '{"required": true, "iso_country_codes": true}',
 '{"common_fields": ["nationality", "citizenship", "country_of_citizenship"], "multiple_allowed": true}',
 'Extract nationality or citizenship information. Use ISO country codes where possible. Some individuals have multiple citizenships - extract all mentioned. Look for country of issue on passports, or explicit nationality statements. Distinguish between nationality (passport issuing country) and place of birth which may differ.',
 'Legal status determining tax obligations, investment restrictions, and regulatory requirements. Critical for compliance assessment.',
 'Determines tax reporting obligations (FATCA, CRS), investment restrictions, and regulatory oversight requirements.',
 'Cross-validate with passport issuing country and tax residency information. Multiple nationalities may have additional reporting requirements.',
 ARRAY['PASSPORT', 'NATIONAL_ID_CARD', 'FATCA_SELF_CERTIFICATION', 'CRS_SELF_CERTIFICATION']),

-- ============================================================================
-- FINANCIAL ATTRIBUTES (appear in financial documents)
-- ============================================================================

-- Total Assets
('financials.total_assets', 'Total Assets', 'decimal', 'Financial Position', 'Balance Sheet',
 'Total assets value from financial statements or declarations',
 'confidential',
 '{"precision": 2, "minimum": 0, "currency_required": true}',
 '{"common_fields": ["total_assets", "assets", "total_balance_sheet"], "currency_indicators": ["$", "USD", "EUR", "GBP", "CHF"]}',
 'Extract total assets figure from balance sheet or financial statements. Include currency denomination. Look for "Total Assets", "Balance Sheet Total", or similar. Ensure figure represents consolidated total, not subtotals. If multiple currencies, note base reporting currency. Verify mathematical consistency with asset subtotals.',
 'Key financial metric for creditworthiness assessment, investment capacity evaluation, and regulatory capital requirements.',
 'Used for credit facility sizing, investment suitability assessment, and regulatory capital adequacy calculations.',
 'Cross-validate with auditor reports, compare with liabilities+equity total, trend analysis with prior periods.',
 ARRAY['AUDITED_FINANCIAL_STATEMENTS', 'MANAGEMENT_ACCOUNTS', 'TAX_RETURNS', 'CREDIT_APPLICATIONS']),

-- Revenue/Turnover
('financials.revenue', 'Revenue/Turnover', 'decimal', 'Financial Performance', 'Income Statement',
 'Annual revenue or turnover from operations',
 'confidential',
 '{"precision": 2, "minimum": 0, "currency_required": true}',
 '{"common_fields": ["revenue", "turnover", "sales", "total_income", "gross_revenue"], "period_specific": true}',
 'Extract annual revenue/turnover figure from income statement. Use gross revenue before deductions. Identify reporting period (annual/quarterly). Look for "Revenue", "Turnover", "Sales", or "Total Income" line items. Exclude non-operating income unless specifically included in business model.',
 'Primary measure of business scale and activity. Used for credit assessment, investment evaluation, and regulatory classification.',
 'Determines credit facility eligibility, investment fund categorization, and regulatory reporting thresholds.',
 'Compare with prior periods for trend analysis, cross-validate with tax returns, verify against industry benchmarks.',
 ARRAY['AUDITED_FINANCIAL_STATEMENTS', 'MANAGEMENT_ACCOUNTS', 'TAX_RETURNS', 'VAT_RETURNS']),

-- Net Worth/Equity
('financials.net_worth', 'Net Worth/Shareholders Equity', 'decimal', 'Financial Position', 'Balance Sheet',
 'Net worth or shareholders equity (assets minus liabilities)',
 'confidential',
 '{"precision": 2, "currency_required": true, "can_be_negative": true}',
 '{"common_fields": ["net_worth", "shareholders_equity", "equity", "net_assets", "capital"], "calculation": "assets_minus_liabilities"}',
 'Extract net worth/equity figure representing assets minus liabilities. For individuals, look for "Net Worth" declarations. For entities, extract "Shareholders Equity" or "Total Equity" from balance sheet. Verify calculation: Total Assets - Total Liabilities = Equity. Note if negative (balance sheet insolvency).',
 'Key solvency indicator and investment capacity measure. Determines creditworthiness and regulatory capital adequacy.',
 'Critical for solvency assessment, investment suitability, regulatory capital requirements, and credit facility approval.',
 'Must equal Assets minus Liabilities. Compare with regulatory minimum capital requirements, trend analysis over time.',
 ARRAY['AUDITED_FINANCIAL_STATEMENTS', 'MANAGEMENT_ACCOUNTS', 'NET_WORTH_STATEMENTS', 'CREDIT_APPLICATIONS']),

-- ============================================================================
-- BANKING ATTRIBUTES (appear in banking documents)
-- ============================================================================

-- Bank Name
('banking.bank_name', 'Bank Name', 'string', 'Banking Information', 'Institution Details',
 'Official name of banking institution',
 'internal',
 '{"max_length": 200, "required": true}',
 '{"common_fields": ["bank_name", "institution_name", "bank"], "include_branch": false}',
 'Extract the official bank name, excluding branch information. Look for letterhead, header information, or explicit bank name fields. Use complete official name, not abbreviations. For international banks, use the full legal name of the specific entity (e.g., "JPMorgan Chase Bank, N.A." not just "JPMorgan").',
 'Identifies banking institution for verification, compliance checking, and relationship validation.',
 'Required for bank verification, sanctions screening of financial institutions, and regulatory compliance.',
 'Cross-validate with known bank databases, verify against sanctions lists, confirm bank license status.',
 ARRAY['BANK_STATEMENTS', 'BANK_REFERENCE_LETTER', 'ACCOUNT_OPENING_DOCUMENTS', 'WIRE_CONFIRMATIONS']),

-- Account Number
('banking.account_number', 'Account Number', 'string', 'Banking Information', 'Account Details',
 'Bank account number (may be partially masked for security)',
 'confidential',
 '{"max_length": 50, "pattern": "^[0-9X\\*\\-]+$", "may_be_masked": true}',
 '{"common_fields": ["account_number", "account_no", "acct_number"], "masking_patterns": ["****1234", "XXXX-1234"]}',
 'Extract account number, noting if partially masked for security. Accept masked formats (e.g., ****1234) as valid. For full numbers, verify format is consistent with bank standards. Do not attempt to unmask or complete partial numbers. Note the masking pattern for future reference.',
 'Unique account identifier for transaction verification and account validation. Partial masking accepted for security.',
 'Used for account verification, transaction matching, and fraud prevention. Full numbers restricted for security.',
 'Cross-reference with transaction patterns, verify masking is consistent across documents from same source.',
 ARRAY['BANK_STATEMENTS', 'WIRE_CONFIRMATIONS', 'ACCOUNT_OPENING_DOCUMENTS', 'DEPOSIT_CONFIRMATIONS']),

-- SWIFT/BIC Code
('banking.swift_bic', 'SWIFT/BIC Code', 'string', 'Banking Information', 'International Codes',
 'SWIFT Bank Identifier Code for international transactions',
 'internal',
 '{"length": [8, 11], "pattern": "^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$", "required": false}',
 '{"common_fields": ["swift_code", "bic_code", "swift_bic", "bank_code"], "format": "AAAA BB CC DDD"}',
 'Extract SWIFT/BIC code used for international wire transfers. Format: 4 letters (bank code) + 2 letters (country) + 2 characters (location) + optional 3 characters (branch). Validate format and verify against known SWIFT directory. Common in international banking documents.',
 'International bank identifier for wire transfers and correspondent banking. Required for cross-border transactions.',
 'Essential for international wire transfers, regulatory reporting of cross-border transactions, and bank verification.',
 'Validate against SWIFT directory, cross-check with bank name and country, verify branch code if present.',
 ARRAY['WIRE_CONFIRMATIONS', 'ACCOUNT_OPENING_DOCUMENTS', 'INTERNATIONAL_TRANSFER_FORMS']),

-- ============================================================================
-- TAX ATTRIBUTES (appear in tax documents)
-- ============================================================================

-- Tax Identification Number
('tax.tin', 'Tax Identification Number', 'string', 'Tax Information', 'Tax IDs',
 'Tax identification number assigned by tax authority',
 'confidential',
 '{"max_length": 50, "format_varies_by_jurisdiction": true}',
 '{"common_fields": ["tin", "tax_id", "tax_number", "ssn", "ein"], "jurisdiction_specific": true}',
 'Extract tax identification number. Format varies by jurisdiction: US SSN (XXX-XX-XXXX), US EIN (XX-XXXXXXX), UK UTR (10 digits), etc. Identify issuing tax authority. May be partially masked for privacy. Note jurisdiction that issued the TIN.',
 'Unique tax identifier for reporting obligations and compliance. Format varies significantly by jurisdiction.',
 'Required for tax reporting (FATCA, CRS), withholding calculations, and regulatory compliance reporting.',
 'Cross-validate format with jurisdiction standards, verify against tax authority databases where possible.',
 ARRAY['FORM_W8BEN_E', 'FATCA_SELF_CERTIFICATION', 'CRS_SELF_CERTIFICATION', 'TAX_RETURNS', 'TAX_RESIDENCY_CERTIFICATE']),

-- Tax Residency
('tax.tax_residency', 'Tax Residency', 'string', 'Tax Information', 'Residency Status',
 'Country or jurisdiction of tax residency',
 'internal',
 '{"iso_country_codes": true, "multiple_allowed": true}',
 '{"common_fields": ["tax_residency", "tax_resident_of", "resident_country"], "multiple_jurisdictions": true}',
 'Extract tax residency jurisdiction(s). Individual or entity may be tax resident in multiple countries. Use ISO country codes. Look for explicit statements like "Tax Resident of" or "Country of Tax Residence". Distinguish from nationality or place of incorporation.',
 'Determines tax reporting obligations and withholding requirements. Critical for FATCA/CRS compliance.',
 'Determines automatic exchange of information obligations, withholding tax rates, and treaty benefit eligibility.',
 'Cross-validate with CRS/FATCA forms, verify against treaty networks, check for multiple residencies.',
 ARRAY['FATCA_SELF_CERTIFICATION', 'CRS_SELF_CERTIFICATION', 'TAX_RESIDENCY_CERTIFICATE', 'TAX_RETURNS']),

-- ============================================================================
-- ISDA/DERIVATIVES ATTRIBUTES (appear in derivatives documents)
-- ============================================================================

-- ISDA Master Agreement Version
('isda.master_agreement_version', 'ISDA Master Agreement Version', 'string', 'Derivatives', 'ISDA Framework',
 'Version of ISDA Master Agreement (1992 or 2002)',
 'internal',
 '{"enum": ["1992", "2002"], "required": true}',
 '{"common_fields": ["agreement_version", "isda_version", "master_agreement_version"]}',
 'Identify which version of ISDA Master Agreement is being used. Look for "1992 ISDA Master Agreement" or "2002 ISDA Master Agreement" in document title or header. The version significantly affects legal terms and provisions. 2002 version is more commonly used for new agreements.',
 'Determines applicable legal framework and provisions for derivatives transactions. Different versions have different default terms.',
 'Affects legal interpretation, default procedures, and regulatory treatment of derivatives transactions.',
 'Must be consistent across all ISDA documentation for same counterparty relationship.',
 ARRAY['ISDA_MASTER_AGREEMENT', 'CREDIT_SUPPORT_ANNEX', 'TRADE_CONFIRMATIONS']),

-- Governing Law
('legal.governing_law', 'Governing Law', 'string', 'Legal Framework', 'Jurisdiction',
 'Legal jurisdiction whose laws govern the contract or agreement',
 'internal',
 '{"common_values": ["New York", "English", "Swiss", "German", "Singapore"], "required": true}',
 '{"common_fields": ["governing_law", "applicable_law", "laws_of"], "specific_format": "laws of [jurisdiction]"}',
 'Extract the governing law clause specifying which jurisdiction laws apply. Common formats: "Laws of New York", "English Law", "Swiss Law". This determines legal interpretation, dispute resolution procedures, and enforceability. Critical for cross-border agreements.',
 'Determines legal framework for contract interpretation, dispute resolution, and enforceability across jurisdictions.',
 'Affects contract enforceability, dispute resolution procedures, and regulatory treatment across jurisdictions.',
 'Must be consistent with jurisdiction clauses and dispute resolution provisions in same document.',
 ARRAY['ISDA_MASTER_AGREEMENT', 'INVESTMENT_MANAGEMENT_AGREEMENT', 'SUBSCRIPTION_AGREEMENT', 'CREDIT_SUPPORT_ANNEX']),

-- Credit Support Provider
('isda.credit_support_provider', 'Credit Support Provider', 'string', 'Derivatives', 'Credit Support',
 'Entity providing credit support/collateral under ISDA documentation',
 'internal',
 '{"max_length": 200, "required": false}',
 '{"common_fields": ["credit_support_provider", "collateral_provider", "guarantor"]}',
 'Identify entity providing credit support or collateral. May be same as counterparty or a parent/affiliate company. Extract full legal name. Look for "Credit Support Provider" definitions in CSA or guarantee provisions in master agreement.',
 'Identifies source of credit enhancement for derivatives transactions. May differ from trading counterparty.',
 'Determines actual credit risk exposure and collateral recovery rights. Critical for risk management and regulatory capital.',
 'Cross-validate with entity formation documents, verify corporate relationships, confirm guarantee authority.',
 ARRAY['CREDIT_SUPPORT_ANNEX', 'ISDA_MASTER_AGREEMENT', 'GUARANTEE_AGREEMENTS']),

-- Base Currency
('isda.base_currency', 'Base Currency', 'string', 'Derivatives', 'Currency',
 'Base currency for collateral calculations in derivatives agreements',
 'internal',
 '{"iso_currency_codes": true, "common_values": ["USD", "EUR", "GBP", "JPY", "CHF"]}',
 '{"common_fields": ["base_currency", "calculation_currency", "collateral_currency"]}',
 'Extract base currency used for collateral calculations and threshold amounts. Typically USD, EUR, GBP, JPY, or CHF. All threshold amounts and calculations are performed in this currency. Look for "Base Currency" definition in CSA.',
 'Currency for all collateral calculations, threshold amounts, and margin requirements. Affects FX risk and operational procedures.',
 'Determines currency exposure, collateral posting requirements, and operational complexity of derivatives relationship.',
 'Must be consistent throughout all derivatives documentation and operational procedures.',
 ARRAY['CREDIT_SUPPORT_ANNEX', 'ISDA_MASTER_AGREEMENT', 'MARGIN_AGREEMENTS']),

-- ============================================================================
-- ADDRESS ATTRIBUTES (appear across many document types)
-- ============================================================================

-- Registered Address
('address.registered_address', 'Registered Office Address', 'address', 'Address Information', 'Official Addresses',
 'Official registered address of entity as filed with authorities',
 'internal',
 '{"components": ["street", "city", "state_province", "postal_code", "country"], "required": true}',
 '{"common_fields": ["registered_office", "registered_address", "principal_address"], "parse_components": true}',
 'Extract complete registered address including street, city, state/province, postal code, and country. This is the official address on file with corporate registry. Parse into components when possible. Verify format matches jurisdiction standards (US: state+ZIP, UK: county+postcode, etc.).',
 'Official address for legal correspondence and regulatory filings. Must match corporate registry records.',
 'Required for legal service of process, regulatory correspondence, and corporate registry compliance.',
 'Must match corporate registry records exactly. Cross-validate with good standing certificates and annual filings.',
 ARRAY['CERT_INCORPORATION', 'ARTICLES_OF_ASSOCIATION', 'GOOD_STANDING_CERTIFICATE', 'ANNUAL_RETURNS']),

-- Correspondence Address
('address.correspondence_address', 'Correspondence Address', 'address', 'Address Information', 'Operational Addresses',
 'Address for business correspondence and operational communications',
 'internal',
 '{"components": ["street", "city", "state_province", "postal_code", "country"], "may_differ_from_registered": true}',
 '{"common_fields": ["correspondence_address", "mailing_address", "business_address", "contact_address"]}',
 'Extract correspondence or mailing address used for business communications. May differ from registered address. Include all components. This is where operational correspondence and statements are sent.',
 'Operational address for business correspondence, statements, and day-to-day communications.',
 'Used for operational communications, statement delivery, and business correspondence.',
 'May differ from registered address. Verify deliverability and update currency.',
 ARRAY['BANK_STATEMENTS', 'INVESTMENT_STATEMENTS', 'CORRESPONDENCE', 'ACCOUNT_OPENING_FORMS']),

-- ============================================================================
-- DOCUMENT METADATA ATTRIBUTES
-- ============================================================================

-- Document Date
('document.date', 'Document Date', 'date', 'Document Metadata', 'Dates',
 'Official date of document creation, execution, or effectiveness',
 'internal',
 '{"required": true, "format": "YYYY-MM-DD"}',
 '{"common_fields": ["date", "document_date", "execution_date", "effective_date"], "date_formats": ["DD/MM/YYYY", "MM/DD/YYYY", "Month DD, YYYY"]}',
 'Extract the primary date associated with the document. May be creation date, execution date, or effective date depending on document type. Convert to ISO format (YYYY-MM-DD). Distinguish from received date or filing date if different.',
 'Establishes document validity period and legal effectiveness. Critical for time-sensitive compliance requirements.',
 'Determines document validity, regulatory compliance deadlines, and legal effectiveness periods.',
 'Cross-validate with validity periods, compare with related document dates for logical consistency.',
 ARRAY['ALL_DOCUMENT_TYPES']),

-- Document Version
('document.version', 'Document Version', 'string', 'Document Metadata', 'Version Control',
 'Version number or identifier for document revisions',
 'internal',
 '{"max_length": 20, "pattern": "^[0-9\\.v]+$"}',
 '{"common_fields": ["version", "revision", "amendment"], "formats": ["v1.0", "1.0", "Rev 1"]}',
 'Extract version number or revision identifier. Look for "Version", "Rev", "Amendment" indicators. Important for tracking document changes and ensuring current version is used.',
 'Ensures current version of agreements and forms are used. Critical for legal validity and operational accuracy.',
 'Ensures parties are working with current terms and conditions. Outdated versions may be legally invalid.',
 'Verify against master document registers, confirm latest version is being used.',
 ARRAY['CONTRACTS', 'FORMS', 'AGREEMENTS', 'TEMPLATES']),

-- ON CONFLICT DO NOTHING to handle re-runs
ON CONFLICT (attribute_code) DO NOTHING;

-- ============================================================================
-- DOCUMENT-ATTRIBUTE MAPPINGS
-- Links each document type to its extractable attributes with priorities
-- ============================================================================

-- Certificate of Incorporation Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
-- Get attribute IDs and map to Certificate of Incorporation
('CERT_INCORPORATION',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.legal_name'),
 1, true,
 ARRAY['document header', 'company name field', 'legal entity name'],
 'Primary identifier - must be exact match with other corporate documents'),

('CERT_INCORPORATION',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.registration_number'),
 1, true,
 ARRAY['company number', 'registration number', 'certificate number'],
 'Government assigned unique identifier - verify format matches jurisdiction'),

('CERT_INCORPORATION',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.jurisdiction_incorporation'),
 1, true,
 ARRAY['state of incorporation', 'jurisdiction', 'incorporated in'],
 'Legal domicile - determines applicable corporate law'),

('CERT_INCORPORATION',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'address.registered_address'),
 2, true,
 ARRAY['registered office', 'principal address', 'registered address'],
 'Must match corporate registry records exactly'),

('CERT_INCORPORATION',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'document.date'),
 2, true,
 ARRAY['date of incorporation', 'certificate date', 'issued date'],
 'Establishes entity age and legal existence period');

-- ISDA Master Agreement Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
('ISDA_MASTER_AGREEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'isda.master_agreement_version'),
 1, true,
 ARRAY['agreement title', 'header', 'version reference'],
 'Critical for determining applicable legal framework - 1992 vs 2002 versions have different terms'),

('ISDA_MASTER_AGREEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'legal.governing_law'),
 1, true,
 ARRAY['governing law clause', 'jurisdiction section', 'applicable law'],
 'Determines legal framework and dispute resolution - common values: New York, English, Swiss law'),

('ISDA_MASTER_AGREEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.legal_name'),
 1, true,
 ARRAY['party A name', 'party B name', 'counterparty identification'],
 'Extract both counterparty names exactly as they appear in party definitions'),

('ISDA_MASTER_AGREEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'document.date'),
 2, true,
 ARRAY['execution date', 'agreement date', 'effective date'],
 'Date when agreement becomes legally binding');

-- Credit Support Annex Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
('CREDIT_SUPPORT_ANNEX',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'isda.base_currency'),
 1, true,
 ARRAY['base currency', 'calculation currency', 'currency definition'],
 'All threshold and margin amounts calculated in this currency'),

('CREDIT_SUPPORT_ANNEX',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'isda.credit_support_provider'),
 1, false,
 ARRAY['credit support provider', 'collateral provider', 'guarantee section'],
 'May be same as counterparty or separate guarantor entity'),

('CREDIT_SUPPORT_ANNEX',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.legal_name'),
 1, true,
 ARRAY['party names', 'counterparty identification', 'secured party'],
 'Must match ISDA Master Agreement party names exactly');

-- Bank Statement Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
('BANK_STATEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'banking.bank_name'),
 1, true,
 ARRAY['bank header', 'institution name', 'letterhead'],
 'Extract official bank name from header or letterhead'),

('BANK_STATEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'banking.account_number'),
 1, true,
 ARRAY['account number', 'account details', 'account information'],
 'May be partially masked for security - extract as shown'),

('BANK_STATEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.legal_name'),
 1, true,
 ARRAY['account holder name', 'account title', 'customer name'],
 'Must match exactly with other entity documents for same client'),

('BANK_STATEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'address.correspondence_address'),
 2, false,
 ARRAY['statement address', 'mailing address', 'account address'],
 'Address where statements are mailed - may differ from registered address'),

('BANK_STATEMENT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'document.date'),
 2, true,
 ARRAY['statement date', 'as of date', 'closing date'],
 'Statement closing date - determines data currency');

-- Passport Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
('PASSPORT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'individual.full_name'),
 1, true,
 ARRAY['surname', 'given names', 'name field'],
 'Extract complete name exactly as printed - not from machine readable zone'),

('PASSPORT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'individual.date_of_birth'),
 1, true,
 ARRAY['date of birth', 'birth date', 'personal details'],
 'Convert to ISO format YYYY-MM-DD regardless of source format'),

('PASSPORT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'individual.nationality'),
 1, true,
 ARRAY['nationality', 'issuing country', 'country code'],
 'Use ISO country codes - passport issuing country indicates nationality'),

('PASSPORT',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'document.date'),
 2, true,
 ARRAY['date of issue', 'expiry date', 'valid until'],
 'Extract both issue and expiry dates - critical for document validity');

-- Form W-8BEN-E Tax Form Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
('FORM_W8BEN_E',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.legal_name'),
 1, true,
 ARRAY['line 1 name', 'entity name', 'beneficial owner name'],
 'Must match exactly with certificate of incorporation and other entity documents'),

('FORM_W8BEN_E',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.jurisdiction_incorporation'),
 1, true,
 ARRAY['country of incorporation', 'line 4', 'jurisdiction'],
 'Country where entity was legally formed'),

('FORM_W8BEN_E',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'tax.tin'),
 1, false,
 ARRAY['US TIN', 'foreign TIN', 'tax identification number'],
 'May have US TIN, foreign TIN, or both - extract all present'),

('FORM_W8BEN_E',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'tax.tax_residency'),
 1, true,
 ARRAY['country of residence', 'tax residence', 'resident of'],
 'For treaty benefits and withholding tax determination'),

('FORM_W8BEN_E',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'address.registered_address'),
 2, true,
 ARRAY['permanent residence address', 'mailing address', 'line 3'],
 'May differ from mailing address - use permanent residence address');

-- Audited Financial Statements Mappings
INSERT INTO "ob-poc".document_attribute_mappings (
    document_type_code, attribute_id, extraction_priority, is_required,
    field_location_hints, ai_extraction_notes
) VALUES
('AUDITED_FINANCIAL_STATEMENTS',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'entity.legal_name'),
 1, true,
 ARRAY['company name', 'entity name', 'consolidated statements header'],
 'Must match legal entity name from formation documents'),

('AUDITED_FINANCIAL_STATEMENTS',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'financials.total_assets'),
 1, true,
 ARRAY['total assets', 'balance sheet total', 'assets total'],
 'From balance sheet - include currency and ensure consolidated figures'),

('AUDITED_FINANCIAL_STATEMENTS',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'financials.revenue'),
 1, true,
 ARRAY['revenue', 'turnover', 'total revenue', 'sales'],
 'From income statement - use gross revenue figure'),

('AUDITED_FINANCIAL_STATEMENTS',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'financials.net_worth'),
 1, true,
 ARRAY['shareholders equity', 'total equity', 'net worth'],
 'From balance sheet - should equal assets minus liabilities'),

('AUDITED_FINANCIAL_STATEMENTS',
 (SELECT attribute_id FROM "ob-poc".consolidated_attributes WHERE attribute_code = 'document.date'),
 2, true,
 ARRAY['financial year end', 'balance sheet date', 'reporting date'],
 'Financial year end date - determines data currency and compliance periods');

-- ============================================================================
-- VALIDATION VIEWS AND FUNCTIONS
-- ============================================================================

-- View for AI agents to get complete attribute context
CREATE OR REPLACE VIEW "ob-poc".v_ai_document_attributes AS
SELECT
    dam.document_type_code,
    ca.attribute_code,
    ca.attribute_name,
    ca.data_type,
    ca.category,
    ca.subcategory,
    ca.description,
    ca.ai_extraction_guidance,
    ca.business_context,
    ca.validation_rules,
    ca.extraction_patterns,
    dam.extraction_priority,
    dam.is_required,
    dam.field_location_hints,
    dam.ai_extraction_notes,
    ca.cross_document_validation,
    ca.source_documents
FROM "ob-poc".document_attribute_mappings dam
JOIN "ob-poc".consolidated_attributes ca ON dam.attribute_id = ca.attribute_id
ORDER BY dam.document_type_code, dam.extraction_priority, ca.attribute_code;

-- View for cross-document validation
CREATE OR REPLACE VIEW "ob-poc".v_cross_document_validation AS
SELECT
    ca.attribute_code,
    ca.attribute_name,
    ca.cross_document_validation,
    array_agg(DISTINCT dam.document_type_code) as appears_in_documents,
    count(dam.document_type_code) as document_count
FROM "ob-poc".consolidated_attributes ca
JOIN "ob-poc".document_attribute_mappings dam ON ca.attribute_id = dam.attribute_id
WHERE ca.cross_document_validation IS NOT NULL
GROUP BY ca.attribute_code, ca.attribute_name, ca.cross_document_validation
ORDER BY document_count DESC, ca.attribute_code;

-- Function to get attributes for a specific document type
CREATE OR REPLACE FUNCTION "ob-poc".get_document_attributes(doc_type VARCHAR)
RETURNS TABLE (
    attribute_code VARCHAR,
    attribute_name VARCHAR,
    data_type VARCHAR,
    extraction_priority INTEGER,
    is_required BOOLEAN,
    ai_guidance TEXT,
    field_hints TEXT[]
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ca.attribute_code,
        ca.attribute_name,
        ca.data_type,
        dam.extraction_priority,
        dam.is_required,
        ca.ai_extraction_guidance,
        dam.field_location_hints
    FROM "ob-poc".consolidated_attributes ca
    JOIN "ob-poc".document_attribute_mappings dam ON ca.attribute_id = dam.attribute_id
    WHERE dam.document_type_code = doc_type
    ORDER BY dam.extraction_priority, ca.attribute_code;
END;
$$ LANGUAGE plpgsql;

-- Function to validate cross-document consistency
CREATE OR REPLACE FUNCTION "ob-poc".validate_attribute_consistency(
    attr_code VARCHAR,
    value1 TEXT,
    value2 TEXT
) RETURNS JSONB AS $$
DECLARE
    validation_rules JSONB;
    result JSONB;
BEGIN
    -- Get validation rules for attribute
    SELECT ca.validation_rules INTO validation_rules
    FROM "ob-poc".consolidated_attributes ca
    WHERE ca.attribute_code = attr_code;

    -- Basic consistency check
    IF value1 = value2 THEN
        result := jsonb_build_object(
            'is_consistent', true,
            'message', 'Values match exactly',
            'confidence_score', 1.0
        );
    ELSE
        result := jsonb_build_object(
            'is_consistent', false,
            'message', 'Values do not match',
            'value1', value1,
            'value2', value2,
            'confidence_score', 0.0
        );
    END IF;

    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- SUMMARY STATISTICS AND VALIDATION
-- ============================================================================

-- View to show mapping completeness
CREATE OR REPLACE VIEW "ob-poc".v_mapping_completeness AS
SELECT
    'Total Attributes' as metric,
    count(*) as count
FROM "ob-poc".consolidated_attributes
UNION ALL
SELECT
    'Attributes with Mappings' as metric,
    count(DISTINCT dam.attribute_id) as count
FROM "ob-poc".document_attribute_mappings dam
UNION ALL
SELECT
    'Document Types with Mappings' as metric,
    count(DISTINCT dam.document_type_code) as count
FROM "ob-poc".document_attribute_mappings dam
UNION ALL
SELECT
    'Total Document-Attribute Mappings' as metric,
    count(*) as count
FROM "ob-poc".document_attribute_mappings dam;

-- Comment explaining the data strategy
COMMENT ON TABLE "ob-poc".consolidated_attributes IS
'Foundational attribute dictionary implementing AttributeID-as-Type pattern.
Eliminates duplication across document types and enables consistent AI extraction
and cross-document validation. Each business concept has exactly one AttributeID
regardless of source document type.';

COMMENT ON TABLE "ob-poc".document_attribute_mappings IS
'Maps document types to extractable attributes with AI guidance and priorities.
Enables document-specific extraction strategies while maintaining consistent
AttributeID mapping across all document types.';

COMMENT ON VIEW "ob-poc".v_ai_document_attributes IS
'Complete context for AI agents performing document extraction. Includes
extraction guidance, validation rules, business context, and cross-document
validation requirements for each attribute in each document type.';

-- Final validation query to check for mapping completeness
DO $$
DECLARE
    attr_count INTEGER;
    mapping_count INTEGER;
    doc_count INTEGER;
BEGIN
    SELECT count(*) INTO attr_count FROM "ob-poc".consolidated_attributes;
    SELECT count(*) INTO mapping_count FROM "ob-poc".document_attribute_mappings;
    SELECT count(DISTINCT document_type_code) INTO doc_count FROM "ob-poc".document_attribute_mappings;

    RAISE NOTICE 'Document-Attribute Mapping Summary:';
    RAISE NOTICE '- Total Consolidated Attributes: %', attr_count;
    RAISE NOTICE '- Total Document-Attribute Mappings: %', mapping_count;
    RAISE NOTICE '- Document Types with Mappings: %', doc_count;
    RAISE NOTICE 'Data bridge foundation established for DSL-as-State architecture';
END $$;
