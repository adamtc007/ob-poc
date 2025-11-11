-- 10_seed_document_dictionary.sql
-- Comprehensive Document Dictionary Seeding
--
-- This script populates the document management system with official financial documents
-- known across global jurisdictions, complete with rich AI-ready narratives and metadata.
--
-- Document Categories:
-- 1. Corporate Formation & Governance
-- 2. Identity & Verification Documents
-- 3. Financial Statements & Reports
-- 4. Regulatory & Compliance Documents
-- 5. Investment & Trading Documents
-- 6. Banking & Credit Documents
-- 7. Insurance Documents
-- 8. Legal & Contractual Documents
-- 9. Tax Documents
-- 10. Derivatives & ISDA Documents

-- First, ensure we have the document types table structure
CREATE TABLE IF NOT EXISTS "ob-poc".document_types (
    document_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_code VARCHAR(50) NOT NULL UNIQUE,
    document_name VARCHAR(200) NOT NULL,
    category VARCHAR(100) NOT NULL,
    subcategory VARCHAR(100),
    description TEXT,
    issuing_authority VARCHAR(200),
    typical_jurisdictions TEXT[],
    regulatory_framework VARCHAR(100),
    validity_period_months INTEGER,
    renewal_required BOOLEAN DEFAULT false,
    digital_format_accepted BOOLEAN DEFAULT true,
    standardized_format BOOLEAN DEFAULT false,
    multilingual_variants TEXT[],
    ai_extraction_complexity VARCHAR(20) DEFAULT 'medium',
    ai_narrative TEXT NOT NULL,
    business_purpose TEXT NOT NULL,
    compliance_implications TEXT[],
    verification_methods TEXT[],
    related_document_types TEXT[],
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_document_types_category ON "ob-poc".document_types (category);
CREATE INDEX IF NOT EXISTS idx_document_types_code ON "ob-poc".document_types (document_code);
CREATE INDEX IF NOT EXISTS idx_document_types_jurisdiction ON "ob-poc".document_types USING GIN (typical_jurisdictions);

-- ============================================================================
-- CATEGORY 1: CORPORATE FORMATION & GOVERNANCE DOCUMENTS
-- ============================================================================

INSERT INTO "ob-poc".document_types (
    document_code, document_name, category, subcategory, description,
    issuing_authority, typical_jurisdictions, regulatory_framework,
    validity_period_months, renewal_required, digital_format_accepted,
    standardized_format, multilingual_variants, ai_extraction_complexity,
    ai_narrative, business_purpose, compliance_implications,
    verification_methods, related_document_types
) VALUES

-- Certificate of Incorporation
('CERT_INCORPORATION', 'Certificate of Incorporation', 'Corporate Formation', 'Formation Documents',
 'Official document certifying the legal formation and registration of a corporate entity',
 'Corporate Registry/Companies House',
 ARRAY['US', 'UK', 'CA', 'AU', 'SG', 'HK', 'IE', 'DE', 'FR', 'NL', 'CH', 'LU'],
 'Corporate Law', NULL, false, true, true,
 ARRAY['English', 'French', 'German', 'Dutch', 'Chinese'],
 'high',
 'This is the foundational legal document that establishes a corporation''s legal existence. AI agents should extract: (1) Legal entity name exactly as registered, (2) Registration number/company number, (3) Jurisdiction of incorporation, (4) Date of incorporation, (5) Registered office address, (6) Share capital information if present. This document is critical for KYC as it provides the primary legal identity of the corporate entity. Look for official seals, signatures, and government formatting. Cross-reference the company name and number against the issuing jurisdiction''s corporate registry when possible.',
 'Establishes legal corporate identity for KYC, banking, investment, and regulatory purposes. Required for opening corporate accounts, investment subscriptions, and regulatory registrations.',
 ARRAY['Primary KYC document for corporate entities', 'Required for AML compliance', 'Establishes beneficial ownership chain starting point', 'Required for regulatory registrations'],
 ARRAY['Government registry verification', 'Apostille authentication for international use', 'Digital signature verification', 'Cross-reference with corporate registry databases'],
 ARRAY['ARTICLES_OF_ASSOCIATION', 'MEMORANDUM_OF_ASSOCIATION', 'CORPORATE_REGISTRY_EXTRACT', 'GOOD_STANDING_CERTIFICATE']),

-- Articles of Association/Incorporation
('ARTICLES_OF_ASSOCIATION', 'Articles of Association', 'Corporate Formation', 'Formation Documents',
 'Constitutional document defining the company''s internal governance rules and procedures',
 'Corporate Registry/Legal Counsel',
 ARRAY['UK', 'SG', 'HK', 'AU', 'ZA', 'IN', 'MY', 'NZ', 'IE', 'MT'],
 'Corporate Law', NULL, false, true, false,
 ARRAY['English', 'Chinese', 'Malay'],
 'high',
 'Articles of Association define how a company operates internally. AI agents should extract: (1) Company name and registration details, (2) Share capital structure and classes, (3) Voting rights and procedures, (4) Director appointment and removal procedures, (5) Dividend distribution rules, (6) Transfer restrictions on shares, (7) Board meeting requirements. This document is crucial for understanding ownership structures and control mechanisms. Pay attention to special voting rights, share classes, and any restrictions that might affect beneficial ownership calculations.',
 'Defines corporate governance structure, ownership rights, and operational procedures. Essential for understanding control mechanisms and beneficial ownership structures.',
 ARRAY['Defines voting and control structures for UBO analysis', 'Contains share transfer restrictions affecting ownership', 'Specifies director and shareholder rights'],
 ARRAY['Legal review for authenticity', 'Cross-reference with filed versions at corporate registry', 'Notarization verification'],
 ARRAY['CERT_INCORPORATION', 'MEMORANDUM_OF_ASSOCIATION', 'SHARE_REGISTER', 'DIRECTOR_REGISTER']),

-- Articles of Incorporation (US variant)
('ARTICLES_OF_INCORPORATION', 'Articles of Incorporation', 'Corporate Formation', 'Formation Documents',
 'US constitutional document filed with state authorities to establish a corporation',
 'State Secretary of State/Division of Corporations',
 ARRAY['US'],
 'State Corporate Law', NULL, false, true, true,
 ARRAY['English', 'Spanish'],
 'high',
 'US Articles of Incorporation establish the corporation and define its basic structure. AI agents should extract: (1) Corporate name, (2) State of incorporation, (3) Registered agent and office, (4) Purpose of business, (5) Authorized share capital and classes, (6) Par value of shares, (7) Director information, (8) Incorporator details. This document varies significantly by state - Delaware corporations will have different formats than California or New York corporations. Look for state filing stamps and sequential filing numbers.',
 'Establishes US corporation legal identity and basic structure. Required for banking, securities, and regulatory compliance.',
 ARRAY['Primary formation document for US entities', 'Establishes authorized capital structure', 'Required for federal and state tax registration'],
 ARRAY['State registry verification', 'Secretary of State authentication', 'Cross-reference with state corporate database'],
 ARRAY['BYLAWS', 'CERT_INCORPORATION', 'REGISTERED_AGENT_APPOINTMENT']),

-- Memorandum of Association
('MEMORANDUM_OF_ASSOCIATION', 'Memorandum of Association', 'Corporate Formation', 'Formation Documents',
 'Constitutional document defining the company''s external relationships and fundamental objectives',
 'Corporate Registry/Legal Counsel',
 ARRAY['UK', 'SG', 'HK', 'AU', 'IN', 'MY', 'ZA', 'MT', 'CY'],
 'Corporate Law', NULL, false, true, false,
 ARRAY['English', 'Chinese', 'Hindi', 'Malay'],
 'medium',
 'Memorandum of Association defines the company''s relationship with the outside world. AI agents should extract: (1) Company name and type, (2) Registered office jurisdiction, (3) Objects/purpose of the company, (4) Liability clause (limited/unlimited), (5) Share capital clause with authorized amount, (6) Subscriber information (initial shareholders), (7) Date of adoption. This document complements the Articles and together they form the complete constitutional framework. Focus on the objects clause which defines what business activities the company can undertake.',
 'Defines the company''s fundamental purpose, scope of activities, and relationship with external parties. Sets legal boundaries for business operations.',
 ARRAY['Defines permissible business activities', 'Establishes liability framework', 'Contains initial shareholder information for UBO tracing'],
 ARRAY['Legal authentication', 'Corporate registry filing verification', 'Notarization check'],
 ARRAY['ARTICLES_OF_ASSOCIATION', 'CERT_INCORPORATION', 'SHARE_REGISTER']),

-- Corporate Bylaws
('BYLAWS', 'Corporate Bylaws', 'Corporate Formation', 'Governance Documents',
 'Internal rules and procedures governing the day-to-day operations of a corporation',
 'Board of Directors/Corporate Secretary',
 ARRAY['US', 'CA', 'MX'],
 'Corporate Law', NULL, false, true, false,
 ARRAY['English', 'Spanish', 'French'],
 'high',
 'Corporate Bylaws are internal governance rules that complement Articles of Incorporation. AI agents should extract: (1) Corporate name and jurisdiction, (2) Shareholder meeting procedures and voting requirements, (3) Board of directors composition, election, and powers, (4) Officer roles and appointment procedures, (5) Committee structures, (6) Stock transfer procedures and restrictions, (7) Amendment procedures. Bylaws are critical for understanding how decisions are made and control is exercised within the corporation. Pay special attention to supermajority voting requirements and any special control provisions.',
 'Establishes internal governance procedures, decision-making processes, and operational rules for corporate management.',
 ARRAY['Defines control and decision-making mechanisms', 'Contains voting procedures affecting control assessment', 'Specifies management structure'],
 ARRAY['Board resolution adoption verification', 'Corporate secretary authentication', 'Amendment history review'],
 ARRAY['ARTICLES_OF_INCORPORATION', 'BOARD_RESOLUTIONS', 'SHAREHOLDER_AGREEMENTS']),

-- Good Standing Certificate
('GOOD_STANDING_CERTIFICATE', 'Certificate of Good Standing', 'Corporate Formation', 'Status Documents',
 'Official certificate confirming that a corporation is current with all filing requirements and fees',
 'Corporate Registry/Secretary of State',
 ARRAY['US', 'CA', 'UK', 'AU', 'SG', 'HK'],
 'Corporate Law', 12, true, true, true,
 ARRAY['English', 'French'],
 'low',
 'Certificate of Good Standing confirms a company is in compliance with regulatory requirements. AI agents should extract: (1) Company name exactly as registered, (2) Registration/company number, (3) Jurisdiction of incorporation, (4) Certificate issue date, (5) Statement of good standing status, (6) Filing compliance confirmation, (7) Expiration date if applicable. This document has a limited validity period and is frequently required for banking, investment, and regulatory purposes. Verify the certificate is recent (typically within 6 months) and from the correct jurisdiction.',
 'Confirms corporate compliance status and active legal standing. Required for banking relationships, investment transactions, and regulatory applications.',
 ARRAY['Confirms regulatory compliance status', 'Required for banking and investment activities', 'Validates ongoing corporate existence'],
 ARRAY['Government registry verification', 'Date validity confirmation', 'Issuing authority authentication'],
 ARRAY['CERT_INCORPORATION', 'ANNUAL_RETURNS', 'CORPORATE_REGISTRY_EXTRACT']),

-- ============================================================================
-- CATEGORY 2: IDENTITY & VERIFICATION DOCUMENTS
-- ============================================================================

-- Passport
('PASSPORT', 'Passport', 'Identity Documents', 'Government ID',
 'Official government-issued travel document serving as proof of identity and nationality',
 'Passport Office/Department of State',
 ARRAY['Global - All Countries'],
 'International Travel/Identity', 120, true, false, true,
 ARRAY['Multiple - Country Specific'],
 'high',
 'Passports are primary identity documents for international KYC. AI agents should extract: (1) Full name exactly as printed, (2) Passport number, (3) Country of issue, (4) Date of birth, (5) Place of birth, (6) Gender, (7) Issue date and expiration date, (8) Photo verification, (9) Machine-readable zone (MRZ) data. Verify document security features including watermarks, holograms, and RFID chips. Cross-check MRZ data with visual fields for consistency. Be aware of different passport formats and security features by country.',
 'Primary identity verification for individuals in financial services, investment management, and cross-border transactions.',
 ARRAY['Primary KYC document for individual identity', 'Required for AML compliance', 'Establishes nationality for tax and regulatory purposes', 'Required for politically exposed person (PEP) screening'],
 ARRAY['RFID chip verification', 'MRZ data validation', 'Security feature authentication', 'Government database verification where available'],
 ARRAY['NATIONAL_ID_CARD', 'DRIVERS_LICENSE', 'UTILITY_BILL', 'BANK_STATEMENT']),

-- National ID Card
('NATIONAL_ID_CARD', 'National Identity Card', 'Identity Documents', 'Government ID',
 'Government-issued identity card for citizens and residents',
 'National Identity Authority/Ministry of Interior',
 ARRAY['EU', 'SG', 'HK', 'MY', 'IN', 'ZA', 'MX', 'BR', 'AR'],
 'National Identity System', 60, true, false, true,
 ARRAY['Multiple - Country Specific'],
 'high',
 'National ID cards vary significantly by country but serve as primary identity documents. AI agents should extract: (1) Full name, (2) ID number (format varies by country), (3) Date of birth, (4) Place of birth, (5) Address (if present), (6) Issue and expiration dates, (7) Photo verification, (8) Nationality/citizenship status. European ID cards often include RFID chips and standardized security features. Asian ID cards may include biometric data. Verify country-specific security features and numbering formats.',
 'Primary domestic identity verification. Alternative to passport for KYC when local jurisdiction accepts.',
 ARRAY['Primary identity document in many jurisdictions', 'Required for domestic financial services', 'May contain address information for verification'],
 ARRAY['RFID verification where applicable', 'Government database checks', 'Biometric verification', 'Security feature authentication'],
 ARRAY['PASSPORT', 'DRIVERS_LICENSE', 'RESIDENCE_PERMIT', 'BIRTH_CERTIFICATE']),

-- Driver's License
('DRIVERS_LICENSE', 'Driver''s License', 'Identity Documents', 'Government ID',
 'Government-issued license permitting operation of motor vehicles, commonly used for identity verification',
 'Department of Motor Vehicles/Transport Authority',
 ARRAY['US', 'CA', 'AU', 'UK', 'EU', 'SG', 'HK'],
 'Transportation/Identity', 60, true, false, false,
 ARRAY['English', 'French', 'German', 'Spanish'],
 'medium',
 'Driver''s licenses are widely accepted identity documents. AI agents should extract: (1) Full name, (2) License number, (3) Date of birth, (4) Address, (5) Issue and expiration dates, (6) Photo verification, (7) License class/category, (8) Restrictions or endorsements. US licenses vary by state with different formats and security features. Enhanced Driver''s Licenses (EDL) have additional security features. Verify address information is current and matches other documentation.',
 'Secondary identity document and address verification. Widely accepted for domestic KYC requirements.',
 ARRAY['Secondary identity verification', 'Address verification document', 'Age verification for investment products'],
 ARRAY['Magnetic stripe or barcode verification', 'State/provincial database checks', 'Security feature validation'],
 ARRAY['PASSPORT', 'NATIONAL_ID_CARD', 'UTILITY_BILL', 'BANK_STATEMENT']),

-- ============================================================================
-- CATEGORY 3: FINANCIAL STATEMENTS & REPORTS
-- ============================================================================

-- Audited Financial Statements
('AUDITED_FINANCIAL_STATEMENTS', 'Audited Financial Statements', 'Financial Reports', 'Annual Reports',
 'Independently audited annual financial statements including balance sheet, income statement, and cash flow',
 'Independent Auditor/CPA Firm',
 ARRAY['Global'],
 'GAAP/IFRS/Local GAAP', 12, true, true, true,
 ARRAY['English', 'French', 'German', 'Spanish', 'Chinese', 'Japanese'],
 'very_high',
 'Audited financial statements provide comprehensive financial position and performance data. AI agents should extract: (1) Company name and reporting period, (2) Auditor name and opinion type (unqualified/qualified/adverse/disclaimer), (3) Total assets, liabilities, and equity from balance sheet, (4) Revenue, expenses, and net income from income statement, (5) Cash flow from operations, investing, and financing, (6) Key financial ratios, (7) Going concern issues, (8) Related party transactions, (9) Significant accounting policies. Focus on auditor opinion - anything other than unqualified requires attention. Look for management letters and subsequent events.',
 'Comprehensive financial assessment for credit decisions, investment evaluation, and regulatory compliance. Primary document for financial due diligence.',
 ARRAY['Required for banking credit facilities', 'Essential for investment fund subscriptions', 'Regulatory filing requirement for many entities', 'Key document for beneficial ownership wealth verification'],
 ARRAY['Auditor license verification', 'Cross-reference with regulatory filings', 'Financial ratio analysis', 'Trend analysis over multiple periods'],
 ARRAY['MANAGEMENT_ACCOUNTS', 'TAX_RETURNS', 'ANNUAL_REPORT', 'CASH_FLOW_FORECAST']),

-- Management Accounts
('MANAGEMENT_ACCOUNTS', 'Management Accounts', 'Financial Reports', 'Internal Reports',
 'Internal financial reports prepared by management showing current financial position and performance',
 'Internal Finance Team/CFO',
 ARRAY['Global'],
 'Internal Management Reporting', 1, true, true, false,
 ARRAY['English', 'Multiple'],
 'high',
 'Management accounts provide current financial position between formal audit periods. AI agents should extract: (1) Company name and reporting period, (2) Current assets, liabilities, and equity, (3) Year-to-date revenue and expenses, (4) Month-on-month or quarter-on-quarter comparisons, (5) Budget vs. actual analysis, (6) Key performance indicators (KPIs), (7) Cash position and working capital, (8) Significant transactions or events, (9) Management commentary on performance. These are unaudited but provide timely financial insights. Look for consistency with previous audited statements.',
 'Current financial position assessment for ongoing monitoring, credit review, and investment evaluation between audit periods.',
 ARRAY['Monthly/quarterly financial monitoring', 'Credit facility compliance monitoring', 'Investment performance tracking'],
 ARRAY['Reconciliation with bank statements', 'Consistency check with audited statements', 'Management signature verification'],
 ARRAY['AUDITED_FINANCIAL_STATEMENTS', 'BANK_STATEMENTS', 'CASH_FLOW_FORECAST', 'BUDGET_PROJECTIONS']),

-- ============================================================================
-- CATEGORY 4: REGULATORY & COMPLIANCE DOCUMENTS
-- ============================================================================

-- Form W-8BEN-E (US Tax)
('FORM_W8BEN_E', 'Certificate of Status of Beneficial Owner for United States Tax Withholding (W-8BEN-E)', 'Tax Documents', 'US Tax Forms',
 'IRS form for foreign entities to claim treaty benefits and establish tax status',
 'Internal Revenue Service (IRS)',
 ARRAY['US', 'Global - for US tax purposes'],
 'US Federal Tax Code', 36, true, true, true,
 ARRAY['English'],
 'high',
 'Form W-8BEN-E establishes foreign entity tax status for US withholding purposes. AI agents should extract: (1) Entity name exactly as on tax records, (2) Country of incorporation or organization, (3) US taxpayer identification number (TIN) if any, (4) Foreign TIN and issuing country, (5) Entity type classification, (6) Beneficial owner information, (7) Treaty country and benefits claimed, (8) Hybrid entity status, (9) Authorized signature and date. Critical for US investment accounts and securities transactions. Verify signature authority and treaty benefit eligibility.',
 'Establishes foreign entity tax status for US withholding tax purposes. Required for US securities investments and banking relationships.',
 ARRAY['Required for US securities investments', 'Determines withholding tax rates', 'FATCA compliance requirement', 'Treaty benefit eligibility'],
 ARRAY['IRS database verification where possible', 'Treaty benefit validation', 'Signature authority verification', 'Entity classification confirmation'],
 ARRAY['CERT_INCORPORATION', 'TAX_RESIDENCY_CERTIFICATE', 'FATCA_SELF_CERTIFICATION', 'POWER_OF_ATTORNEY']),

-- FATCA Self-Certification
('FATCA_SELF_CERTIFICATION', 'FATCA Self-Certification Form', 'Regulatory Documents', 'FATCA Compliance',
 'Self-certification form for FATCA compliance determining US tax status',
 'Financial Institution/Entity',
 ARRAY['Global'],
 'FATCA', 36, true, true, false,
 ARRAY['English', 'Multiple'],
 'medium',
 'FATCA self-certification determines US person status for reporting purposes. AI agents should extract: (1) Entity name, (2) Country of tax residence, (3) US person status declaration, (4) GIIN (Global Intermediary Identification Number) if applicable, (5) Entity type for FATCA purposes, (6) Substantial US owners information, (7) Controlling persons details, (8) Authorized signature and date. Critical for financial account opening and ongoing compliance. Cross-reference with other tax documents.',
 'FATCA compliance determination for financial institutions to identify US persons and reporting requirements.',
 ARRAY['Required for financial account opening', 'Determines FATCA reporting obligations', 'US person identification for compliance'],
 ARRAY['GIIN validation', 'Cross-reference with IRS databases', 'US person status verification'],
 ARRAY['FORM_W8BEN_E', 'CRS_SELF_CERTIFICATION', 'TAX_RESIDENCY_CERTIFICATE']),

-- CRS Self-Certification
('CRS_SELF_CERTIFICATION', 'Common Reporting Standard Self-Certification', 'Regulatory Documents', 'CRS Compliance',
 'Self-certification form for CRS compliance determining tax residency for automatic exchange of information',
 'Financial Institution/Entity',
 ARRAY['OECD Countries', 'EU', 'Global'],
 'CRS/AEOI', 36, true, true, false,
 ARRAY['English', 'Multiple'],
 'medium',
 'CRS self-certification determines tax residency for automatic exchange of financial information. AI agents should extract: (1) Entity name, (2) Countries of tax residence, (3) Tax identification numbers (TINs) for each jurisdiction, (4) Entity type classification, (5) Controlling persons information, (6) Active/passive NFE status, (7) Financial institution status, (8) Authorized signature and date. Essential for compliance with automatic exchange of information requirements under CRS.',
 'Tax residency determination for Common Reporting Standard compliance and automatic exchange of financial information.',
 ARRAY['Required for CRS reporting compliance', 'Determines tax residency for information exchange', 'Controlling persons identification'],
 ARRAY['TIN validation', 'Tax residency verification', 'Cross-reference with tax authorities'],
 ARRAY['FATCA_SELF_CERTIFICATION', 'TAX_RESIDENCY_CERTIFICATE', 'BENEFICIAL_OWNERSHIP_DECLARATION']),

-- ============================================================================
-- CATEGORY 5: INVESTMENT & TRADING DOCUMENTS
-- ============================================================================

-- Investment Management Agreement
('INVESTMENT_MANAGEMENT_AGREEMENT', 'Investment Management Agreement', 'Investment Documents', 'Management Agreements',
 'Contract between investment manager and client defining investment management services and terms',
 'Investment Manager/Legal Counsel',
 ARRAY['Global'],
 'Securities Regulation', NULL, false, true, false,
 ARRAY['English', 'Multiple'],
 'high',
 'Investment Management Agreement defines the relationship between manager and client. AI agents should extract: (1) Manager and client names and details, (2) Investment objectives and strategy, (3) Investment guidelines and restrictions, (4) Fee structure and calculation methods, (5) Performance benchmarks, (6) Reporting requirements and frequency, (7) Risk management provisions, (8) Termination clauses, (9) Regulatory compliance requirements, (10) Governing law and jurisdiction. Critical for understanding investment authority and constraints.',
 'Establishes investment management relationship, authority, and terms. Defines investment parameters and manager responsibilities.',
 ARRAY['Defines investment authority and limits', 'Establishes fiduciary relationship', 'Contains regulatory compliance requirements'],
 ARRAY['Legal review for enforceability', 'Regulatory compliance verification', 'Cross-reference with manager registrations'],
 ARRAY['POWER_OF_ATTORNEY', 'INVESTMENT_POLICY_STATEMENT', 'CUSTODY_AGREEMENT', 'SUBSCRIPTION_AGREEMENT']),

-- Subscription Agreement
('SUBSCRIPTION_AGREEMENT', 'Subscription Agreement', 'Investment Documents', 'Fund Documents',
 'Legal agreement for investment in private funds, partnerships, or securities',
 'Fund Manager/Legal Counsel',
 ARRAY['Global'],
 'Securities Regulation', NULL, false, true, false,
 ARRAY['English', 'Multiple'],
 'very_high',
 'Subscription Agreement governs investment in private funds or partnerships. AI agents should extract: (1) Fund name and investor details, (2) Investment amount and payment terms, (3) Investor representations and warranties, (4) Accredited investor or qualified purchaser status, (5) Anti-money laundering certifications, (6) Tax withholding information, (7) Transfer restrictions, (8) Withdrawal/redemption rights, (9) Governing law and dispute resolution, (10) Regulatory disclosures. Contains extensive KYC and AML information.',
 'Legal framework for private fund investment. Contains investor suitability, compliance, and operational terms.',
 ARRAY['Contains investor suitability determinations', 'AML and KYC certifications', 'Source of funds representations', 'Accredited investor status'],
 ARRAY['Accredited investor status verification', 'AML compliance check', 'Source of funds validation'],
 ARRAY['PRIVATE_PLACEMENT_MEMORANDUM', 'POWER_OF_ATTORNEY', 'BANK_REFERENCE_LETTER', 'AUDITED_FINANCIAL_STATEMENTS']),

-- ============================================================================
-- CATEGORY 6: DERIVATIVES & ISDA DOCUMENTS
-- ============================================================================

-- ISDA Master Agreement
('ISDA_MASTER_AGREEMENT', 'ISDA Master Agreement', 'Derivatives Documents', 'ISDA Framework',
 'Standardized master agreement for over-the-counter derivatives transactions',
 'International Swaps and Derivatives Association (ISDA)',
 ARRAY['Global'],
 'Derivatives Regulation', NULL, false, true, true,
 ARRAY['English'],
 'very_high',
 'ISDA Master Agreement establishes the legal framework for derivatives trading. AI agents should extract: (1) Counterparty names and details, (2) Agreement version (1992 or 2002), (3) Governing law and jurisdiction, (4) Election of terms in Schedule, (5) Termination events and events of default, (6) Credit support requirements, (7) Netting and close-out provisions, (8) Additional representations, (9) Dispute resolution procedures, (10) Multicurrency cross-default provisions. The Schedule contains party-specific elections that modify the standard terms.',
 'Legal framework for OTC derivatives trading. Establishes netting, collateral, and default procedures between counterparties.',
 ARRAY['Establishes legal framework for derivatives trading', 'Defines credit and operational risk management', 'Required for institutional derivatives business'],
 ARRAY['ISDA template version verification', 'Legal opinion review', 'Regulatory compliance check'],
 ARRAY['CREDIT_SUPPORT_ANNEX', 'TRADE_CONFIRMATIONS', 'LEGAL_OPINION', 'POWER_OF_ATTORNEY']),

-- Credit Support Annex (CSA)
('CREDIT_SUPPORT_ANNEX', 'Credit Support Annex (CSA)', 'Derivatives Documents', 'ISDA Framework',
 'ISDA document defining collateral arrangements for derivatives transactions',
 'International Swaps and Derivatives Association (ISDA)',
 ARRAY['Global'],
 'Derivatives Regulation', NULL, false, true, true,
 ARRAY['English'],
 'very_high',
 'Credit Support Annex defines collateral arrangements for derivatives. AI agents should extract: (1) Counterparty names, (2) Base currency for calculations, (3) Credit support provider details, (4) Threshold amounts for each party, (5) Minimum transfer amounts, (6) Independent amounts/initial margins, (7) Eligible collateral types and haircuts, (8) Valuation and timing provisions, (9) Dispute resolution procedures, (10) Substitution rights. CSA terms directly impact capital requirements and risk management.',
 'Defines collateral requirements and procedures for derivatives transactions. Critical for counterparty risk management.',
 ARRAY['Establishes margin and collateral requirements', 'Defines counterparty credit risk mitigation', 'Required for regulatory capital calculations'],
 ARRAY['Mathematical model validation', 'Eligible collateral verification', 'Regulatory compliance review'],
 ARRAY['ISDA_MASTER_AGREEMENT', 'COLLATERAL_AGREEMENTS', 'MARGIN_CALL_NOTICES']),

-- ============================================================================
-- CATEGORY 7: BANKING & CREDIT DOCUMENTS
-- ============================================================================

-- Bank Reference Letter
('BANK_REFERENCE_LETTER', 'Bank Reference Letter', 'Banking Documents', 'Reference Letters',
 'Letter from bank confirming client relationship, account standing, and financial capacity',
 'Commercial Bank/Relationship Manager',
 ARRAY['Global'],
 'Banking Regulation', 6, true, true, false,
 ARRAY['English', 'Multiple'],
 'medium',
 'Bank reference letter confirms banking relationship and financial standing. AI agents should extract: (1) Bank name and contact details, (2) Client name exactly as on account, (3) Account types held (checking, savings, credit facilities), (4) Length of relationship, (5) Account conduct description (satisfactory/excellent), (6) Credit facilities and utilization, (7) Average balances (if disclosed), (8) Any adverse information, (9) Banker signature and title, (10) Date of letter. Letters have limited validity (typically 3-6 months) and format varies by institution.',
 'Banking relationship confirmation for account opening, credit applications, and investment subscriptions. Provides financial credibility assessment.',
 ARRAY['Confirms banking relationship and standing', 'Supports credit and investment applications', 'Provides financial capacity indication'],
 ARRAY['Bank authentication through SWIFT or direct contact', 'Banker signature verification', 'Relationship tenure confirmation'],
 ARRAY['BANK_STATEMENTS', 'AUDITED_FINANCIAL_STATEMENTS', 'CREDIT_REPORTS', 'ACCOUNT_OPENING_DOCUMENTS']),

-- Bank Statements
('BANK_STATEMENT', 'Bank Statement', 'Banking Documents', 'Account Statements',
 'Monthly statement showing account transactions, balances, and banking activity',
 'Commercial Bank',
 ARRAY['Global'],
 'Banking Regulation', 1, true, true, true,
 ARRAY['English', 'Multiple'],
 'high',
 'Bank statements provide detailed account activity and transaction history. AI agents should extract: (1) Bank name and branch details, (2) Account holder name exactly as registered, (3) Account number (may be partially masked), (4) Statement period and closing date, (5) Opening and closing balances, (6) Transaction details including dates, descriptions, and amounts, (7) Fees and charges, (8) Interest earned, (9) Average daily balance, (10) Overdraft facilities and usage. Verify mathematical accuracy and look for unusual transaction patterns.',
 'Account activity verification for KYC, source of funds analysis, and financial capacity assessment. Primary document for transaction monitoring.',
 ARRAY['Source of funds verification', 'Account conduct assessment', 'Transaction pattern analysis for AML', 'Financial capacity verification'],
 ARRAY['Bank authentication', 'Mathematical reconciliation', 'Transaction pattern analysis', 'Cross-reference with other statements'],
 ARRAY['BANK_REFERENCE_LETTER', 'TRANSACTION_CONFIRMATIONS', 'LOAN_AGREEMENTS', 'DEPOSIT_CERTIFICATES']),

-- ============================================================================
-- CATEGORY 8: LEGAL & CONTRACTUAL DOCUMENTS
-- ============================================================================

-- Power of Attorney
('POWER_OF_ATTORNEY', 'Power of Attorney', 'Legal Documents', 'Authorization Documents',
 'Legal document granting authority to act on behalf of another person or entity',
 'Legal Counsel/Notary Public',
 ARRAY['Global'],
 'Civil Law/Common Law', NULL,
