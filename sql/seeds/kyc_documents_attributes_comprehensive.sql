-- ============================================================================
-- COMPREHENSIVE KYC DOCUMENT TYPES AND ATTRIBUTE REGISTRY - EXPANDED
-- ============================================================================
-- 
-- This file provides the complete KYC document and attribute taxonomy:
-- - 120+ document types across all KYC domains
-- - 120+ attributes with semantic IDs
-- - Document validity rules (max_age, notarization, certification requirements)
-- - Bidirectional document-attribute links (SOURCE/SINK)
--
-- Includes: ISDA/CSA, Fund Admin, Prime Brokerage, Regulatory Filings,
-- Jurisdiction-specific forms, Insurance, Banking, Corporate Actions
-- ============================================================================

BEGIN;

-- ============================================================================
-- PART 1: DOCUMENT TYPES - COMPREHENSIVE
-- ============================================================================

-- Clear existing if needed (be careful in production)
-- TRUNCATE "ob-poc".document_types CASCADE;

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, applicability)
VALUES

-- =============================================================================
-- PERSONAL IDENTITY DOCUMENTS
-- =============================================================================
('d0010000-0000-0000-0000-000000000001'::uuid, 'PASSPORT', 'Passport', 'IDENTITY', 'personal',
 'Government-issued international travel document. Primary identity document with MRZ.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_primary_id": true, "accepted_globally": true, 
   "validity": {"must_be_valid": true, "min_validity_months": 3, "reject_if_expired": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000002'::uuid, 'NATIONAL_ID', 'National Identity Card', 'IDENTITY', 'personal',
 'Government-issued national identity card. Primary ID in EU/EEA.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_primary_id": true, 
   "validity": {"must_be_valid": true, "reject_if_expired": true},
   "accepted_regions": ["EU", "EEA", "CH"]}'::jsonb),

('d0010000-0000-0000-0000-000000000003'::uuid, 'DRIVERS_LICENSE', 'Driver''s License', 'IDENTITY', 'personal',
 'Government-issued driving permit. Secondary ID, proves address in some jurisdictions.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_secondary_id": true,
   "validity": {"must_be_valid": true}, 
   "proves_address_in": ["US", "UK", "AU", "CA"]}'::jsonb),

('d0010000-0000-0000-0000-000000000004'::uuid, 'BIRTH_CERTIFICATE', 'Birth Certificate', 'IDENTITY', 'personal',
 'Official record of birth. Used for identity verification and citizenship proof.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "validity": {"no_expiry": true},
   "requires": {"certified_copy": true, "apostille_if_foreign": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000005'::uuid, 'RESIDENCE_PERMIT', 'Residence Permit / Visa', 'IDENTITY', 'personal',
 'Immigration document permitting residence in a country.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "validity": {"must_be_valid": true, "reject_if_expired": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000006'::uuid, 'MARRIAGE_CERTIFICATE', 'Marriage Certificate', 'IDENTITY', 'personal',
 'Official record of marriage. Used for name changes and relationship verification.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "validity": {"no_expiry": true},
   "requires": {"certified_copy": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000007'::uuid, 'DEATH_CERTIFICATE', 'Death Certificate', 'IDENTITY', 'personal',
 'Official record of death. Required for estate matters.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "validity": {"no_expiry": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000008'::uuid, 'DEED_POLL', 'Deed Poll / Name Change Document', 'IDENTITY', 'personal',
 'Legal document evidencing a name change.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "validity": {"no_expiry": true},
   "requires": {"original_or_certified": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000009'::uuid, 'SOCIAL_SECURITY_CARD', 'Social Security Card', 'IDENTITY', 'personal',
 'US Social Security card showing SSN.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["US"], "validity": {"no_expiry": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000010'::uuid, 'MILITARY_ID', 'Military Identity Card', 'IDENTITY', 'personal',
 'Armed forces identification card.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_secondary_id": true, "validity": {"must_be_valid": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000011'::uuid, 'VOTER_ID', 'Voter Registration Card', 'IDENTITY', 'personal',
 'Electoral registration card.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_secondary_id": true, "accepted_jurisdictions": ["IN", "MX", "BR"]}'::jsonb),

('d0010000-0000-0000-0000-000000000012'::uuid, 'AADHAAR', 'Aadhaar Card', 'IDENTITY', 'personal',
 'Indian biometric identity card.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["IN"], "is_primary_id": true}'::jsonb),

-- =============================================================================
-- CORPORATE FORMATION DOCUMENTS
-- =============================================================================
('d0020000-0000-0000-0000-000000000001'::uuid, 'CERT_OF_INCORPORATION', 'Certificate of Incorporation', 'CORPORATE', 'formation',
 'Official document from registrar confirming company formation.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
   "is_primary_formation": true, "validity": {"no_expiry": true},
   "requires": {"certified_copy": true, "apostille_if_foreign": true}}'::jsonb),

('d0020000-0000-0000-0000-000000000002'::uuid, 'ARTICLES_OF_ASSOCIATION', 'Articles of Association / Bylaws', 'CORPORATE', 'formation',
 'Constitutional document defining company rules and governance.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "aka": ["Bylaws", "Constitution", "Charter"], "validity": {"require_current_version": true}}'::jsonb),

('d0020000-0000-0000-0000-000000000003'::uuid, 'MEMORANDUM_OF_ASSOCIATION', 'Memorandum of Association', 'CORPORATE', 'formation',
 'Document stating company objects and authorized capital.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "jurisdictions": ["UK", "HK", "SG", "CY", "MT", "IE"]}'::jsonb),

('d0020000-0000-0000-0000-000000000004'::uuid, 'CERT_OF_GOOD_STANDING', 'Certificate of Good Standing', 'CORPORATE', 'status',
 'Registrar certificate confirming company is active and compliant.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC", "PARTNERSHIP_LIMITED"],
   "validity": {"max_age_days": 90, "reject_if_older": true},
   "aka": ["Certificate of Compliance", "Certificate of Status", "Letter of Good Standing"]}'::jsonb),

('d0020000-0000-0000-0000-000000000005'::uuid, 'CERT_OF_INCUMBENCY', 'Certificate of Incumbency', 'CORPORATE', 'status',
 'Document listing current directors, officers, and shareholders.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"max_age_days": 90, "max_age_days_relaxed": 180},
   "jurisdictions": ["KY", "BVI", "JE", "GG", "BM", "VG"]}'::jsonb),

('d0020000-0000-0000-0000-000000000006'::uuid, 'REGISTER_OF_DIRECTORS', 'Register of Directors', 'CORPORATE', 'governance',
 'Official register of past and present directors.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"require_current": true}}'::jsonb),

('d0020000-0000-0000-0000-000000000007'::uuid, 'REGISTER_OF_SHAREHOLDERS', 'Register of Shareholders/Members', 'CORPORATE', 'ownership',
 'Official register of shareholdings and transfers.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"require_current": true}}'::jsonb),

('d0020000-0000-0000-0000-000000000008'::uuid, 'SHARE_CERTIFICATE', 'Share Certificate', 'CORPORATE', 'ownership',
 'Certificate evidencing ownership of shares.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0020000-0000-0000-0000-000000000009'::uuid, 'BOARD_RESOLUTION', 'Board Resolution', 'CORPORATE', 'governance',
 'Formal decision of the board of directors.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"purpose_specific": true, "max_age_days": 365},
   "requires": {"certified_extract": true}}'::jsonb),

('d0020000-0000-0000-0000-000000000010'::uuid, 'SHAREHOLDER_RESOLUTION', 'Shareholder/Member Resolution', 'CORPORATE', 'governance',
 'Formal decision of shareholders in general meeting.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0020000-0000-0000-0000-000000000011'::uuid, 'SIGNATORY_LIST', 'Authorized Signatory List', 'CORPORATE', 'governance',
 'List of persons authorized to sign on behalf of the entity.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED", "TRUST_DISCRETIONARY"],
   "validity": {"require_current": true}}'::jsonb),

('d0020000-0000-0000-0000-000000000012'::uuid, 'SPECIMEN_SIGNATURES', 'Specimen Signature Card', 'CORPORATE', 'governance',
 'Card showing specimen signatures of authorized signatories.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0020000-0000-0000-0000-000000000013'::uuid, 'ANNUAL_RETURN', 'Annual Return / Confirmation Statement', 'CORPORATE', 'filing',
 'Annual filing with registrar confirming company details.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"max_age_months": 15}}'::jsonb),

('d0020000-0000-0000-0000-000000000014'::uuid, 'CHANGE_OF_DIRECTORS', 'Notice of Change of Directors', 'CORPORATE', 'filing',
 'Filing notifying registrar of director appointments/resignations.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0020000-0000-0000-0000-000000000015'::uuid, 'OPERATING_AGREEMENT', 'LLC Operating Agreement', 'CORPORATE', 'formation',
 'Governing document for Limited Liability Companies.',
 '{"entity_types": ["LLC"], "jurisdictions": ["US"], "is_primary_formation": true}'::jsonb),

('d0020000-0000-0000-0000-000000000016'::uuid, 'CERT_OF_FORMATION_LLC', 'Certificate of Formation (LLC)', 'CORPORATE', 'formation',
 'State filing confirming LLC formation.',
 '{"entity_types": ["LLC"], "jurisdictions": ["US"], "is_primary_formation": true}'::jsonb),

('d0020000-0000-0000-0000-000000000017'::uuid, 'REGISTRY_EXTRACT', 'Commercial Registry Extract', 'CORPORATE', 'status',
 'Official extract from commercial/company registry.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"max_age_days": 30, "max_age_days_relaxed": 90},
   "aka": ["Handelsregisterauszug", "Extrait Kbis", "Visura Camerale"]}'::jsonb),

('d0020000-0000-0000-0000-000000000018'::uuid, 'REGISTER_OF_CHARGES', 'Register of Charges/Mortgages', 'CORPORATE', 'security',
 'Register of security interests over company assets.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0020000-0000-0000-0000-000000000019'::uuid, 'CERTIFICATE_OF_NAME_CHANGE', 'Certificate of Name Change', 'CORPORATE', 'filing',
 'Official certificate confirming corporate name change.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0020000-0000-0000-0000-000000000020'::uuid, 'CERTIFICATE_OF_REGISTRATION_FOREIGN', 'Foreign Company Registration', 'CORPORATE', 'filing',
 'Registration of foreign company in local jurisdiction.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

-- =============================================================================
-- PARTNERSHIP DOCUMENTS
-- =============================================================================
('d0030000-0000-0000-0000-000000000001'::uuid, 'PARTNERSHIP_AGREEMENT', 'Partnership Agreement', 'PARTNERSHIP', 'formation',
 'Agreement governing the partnership (LP, LLP, GP).',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP", "PARTNERSHIP_GENERAL"],
   "is_primary_formation": true, "validity": {"require_current_version": true, "include_amendments": true}}'::jsonb),

('d0030000-0000-0000-0000-000000000002'::uuid, 'LP_CERTIFICATE', 'Certificate of Limited Partnership', 'PARTNERSHIP', 'formation',
 'Official registration certificate for limited partnership.',
 '{"entity_types": ["PARTNERSHIP_LIMITED"], "is_primary_formation": true}'::jsonb),

('d0030000-0000-0000-0000-000000000003'::uuid, 'REGISTER_OF_PARTNERS', 'Register of Partners', 'PARTNERSHIP', 'ownership',
 'Register of general and limited partners with commitments.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
   "validity": {"require_current": true}}'::jsonb),

('d0030000-0000-0000-0000-000000000004'::uuid, 'PARTNER_ADMISSION_LETTER', 'Partner Admission Letter', 'PARTNERSHIP', 'ownership',
 'Letter confirming admission of new partner.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"]}'::jsonb),

('d0030000-0000-0000-0000-000000000005'::uuid, 'GP_CONSENT', 'General Partner Consent', 'PARTNERSHIP', 'governance',
 'Consent of general partner to specific actions.',
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb),

('d0030000-0000-0000-0000-000000000006'::uuid, 'LPAC_RESOLUTION', 'LPAC Resolution', 'PARTNERSHIP', 'governance',
 'Resolution of Limited Partner Advisory Committee.',
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb),

('d0030000-0000-0000-0000-000000000007'::uuid, 'CAPITAL_CALL_NOTICE', 'Capital Call Notice', 'PARTNERSHIP', 'financial',
 'Notice calling capital from limited partners.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "FUND_PE", "FUND_VC"]}'::jsonb),

('d0030000-0000-0000-0000-000000000008'::uuid, 'DISTRIBUTION_NOTICE', 'Distribution Notice', 'PARTNERSHIP', 'financial',
 'Notice of distribution to partners.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "FUND_PE", "FUND_VC"]}'::jsonb),

-- =============================================================================
-- TRUST DOCUMENTS
-- =============================================================================
('d0040000-0000-0000-0000-000000000001'::uuid, 'TRUST_DEED', 'Trust Deed / Trust Instrument', 'TRUST', 'formation',
 'Primary document establishing the trust and its terms.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED", "TRUST_UNIT"],
   "is_primary_formation": true, "validity": {"require_all_amendments": true},
   "requires": {"original_or_certified": true}}'::jsonb),

('d0040000-0000-0000-0000-000000000002'::uuid, 'LETTER_OF_WISHES', 'Letter of Wishes', 'TRUST', 'guidance',
 'Non-binding guidance from settlor to trustees.',
 '{"entity_types": ["TRUST_DISCRETIONARY"], "is_confidential": true}'::jsonb),

('d0040000-0000-0000-0000-000000000003'::uuid, 'DEED_OF_APPOINTMENT', 'Deed of Appointment (Trustees)', 'TRUST', 'governance',
 'Document appointing new trustees.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('d0040000-0000-0000-0000-000000000004'::uuid, 'DEED_OF_RETIREMENT', 'Deed of Retirement (Trustees)', 'TRUST', 'governance',
 'Document evidencing trustee retirement.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('d0040000-0000-0000-0000-000000000005'::uuid, 'SCHEDULE_OF_BENEFICIARIES', 'Schedule of Beneficiaries', 'TRUST', 'beneficial',
 'List of trust beneficiaries and their entitlements.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"],
   "validity": {"require_current": true}}'::jsonb),

('d0040000-0000-0000-0000-000000000006'::uuid, 'TRUST_FINANCIAL_STATEMENT', 'Trust Financial Statement', 'TRUST', 'financial',
 'Statement of trust assets and liabilities.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED", "TRUST_UNIT"]}'::jsonb),

('d0040000-0000-0000-0000-000000000007'::uuid, 'DEED_OF_ADDITION', 'Deed of Addition', 'TRUST', 'assets',
 'Document adding assets to the trust.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('d0040000-0000-0000-0000-000000000008'::uuid, 'DEED_OF_EXCLUSION', 'Deed of Exclusion', 'TRUST', 'beneficial',
 'Document excluding beneficiaries from the trust.',
 '{"entity_types": ["TRUST_DISCRETIONARY"]}'::jsonb),

('d0040000-0000-0000-0000-000000000009'::uuid, 'TRUSTEE_RESOLUTION', 'Trustee Resolution', 'TRUST', 'governance',
 'Formal resolution of the trustees.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('d0040000-0000-0000-0000-000000000010'::uuid, 'TRUST_REGISTRATION', 'Trust Registration Certificate', 'TRUST', 'filing',
 'Certificate of registration with trust registry (where applicable).',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"],
   "jurisdictions": ["UK", "EU"]}'::jsonb),

-- =============================================================================
-- FUND / INVESTMENT DOCUMENTS
-- =============================================================================
('d0050000-0000-0000-0000-000000000001'::uuid, 'FUND_PROSPECTUS', 'Fund Prospectus', 'FUND', 'offering',
 'Regulatory offering document for public/retail funds.',
 '{"entity_types": ["FUND_UCITS", "FUND_MUTUAL"], "regulatory": true,
   "validity": {"require_current_version": true}}'::jsonb),

('d0050000-0000-0000-0000-000000000002'::uuid, 'OFFERING_MEMORANDUM', 'Offering Memorandum / PPM', 'FUND', 'offering',
 'Private Placement Memorandum for private funds.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_VC"],
   "aka": ["PPM", "OM", "Information Memorandum", "Confidential Information Memorandum"]}'::jsonb),

('d0050000-0000-0000-0000-000000000003'::uuid, 'IMA', 'Investment Management Agreement', 'FUND', 'service',
 'Agreement between fund and investment manager.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('d0050000-0000-0000-0000-000000000004'::uuid, 'SUBSCRIPTION_AGREEMENT', 'Subscription Agreement', 'FUND', 'investor',
 'Agreement for investor subscription to fund.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('d0050000-0000-0000-0000-000000000005'::uuid, 'SIDE_LETTER', 'Side Letter', 'FUND', 'investor',
 'Supplemental agreement granting special terms to investor.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('d0050000-0000-0000-0000-000000000006'::uuid, 'NAV_STATEMENT', 'NAV Statement', 'FUND', 'valuation',
 'Statement of Net Asset Value per share/unit.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('d0050000-0000-0000-0000-000000000007'::uuid, 'KIID', 'Key Investor Information Document', 'FUND', 'disclosure',
 'Standardized EU fund disclosure document.',
 '{"entity_types": ["FUND_UCITS"], "jurisdictions": ["EU"], "regulatory": true,
   "validity": {"max_age_months": 12}}'::jsonb),

('d0050000-0000-0000-0000-000000000008'::uuid, 'FUND_CONSTITUTION', 'Fund Constitution / Instrument', 'FUND', 'formation',
 'Constitutional document establishing the fund.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS", "FUND_UNIT_TRUST"], "is_primary_formation": true}'::jsonb),

('d0050000-0000-0000-0000-000000000009'::uuid, 'FUND_ADMIN_AGREEMENT', 'Fund Administration Agreement', 'FUND', 'service',
 'Agreement with fund administrator.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('d0050000-0000-0000-0000-000000000010'::uuid, 'CUSTODIAN_AGREEMENT', 'Custodian Agreement', 'FUND', 'service',
 'Agreement with custodian bank.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('d0050000-0000-0000-0000-000000000011'::uuid, 'DEPOSITARY_AGREEMENT', 'Depositary Agreement', 'FUND', 'service',
 'Agreement with depositary (AIFMD requirement).',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "regulatory": "AIFMD"}'::jsonb),

('d0050000-0000-0000-0000-000000000012'::uuid, 'PRIME_BROKERAGE_AGREEMENT', 'Prime Brokerage Agreement', 'FUND', 'service',
 'Agreement with prime broker.',
 '{"entity_types": ["FUND_HEDGE"]}'::jsonb),

('d0050000-0000-0000-0000-000000000013'::uuid, 'INVESTOR_QUESTIONNAIRE', 'Investor Questionnaire', 'FUND', 'investor',
 'KYC/AML questionnaire for fund investors.',
 '{"entity_types": ["ALL"]}'::jsonb),

('d0050000-0000-0000-0000-000000000014'::uuid, 'REDEMPTION_NOTICE', 'Redemption Notice', 'FUND', 'investor',
 'Notice of redemption from fund.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS"]}'::jsonb),

('d0050000-0000-0000-0000-000000000015'::uuid, 'TRANSFER_AGREEMENT', 'Transfer Agreement (Fund Interest)', 'FUND', 'investor',
 'Agreement transferring fund interest between investors.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('d0050000-0000-0000-0000-000000000016'::uuid, 'DDQ', 'Due Diligence Questionnaire', 'FUND', 'compliance',
 'Operational due diligence questionnaire.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"],
   "validity": {"max_age_months": 12}}'::jsonb),

('d0050000-0000-0000-0000-000000000017'::uuid, 'AIFMD_ANNEX_IV', 'AIFMD Annex IV Report', 'FUND', 'regulatory',
 'Periodic regulatory report under AIFMD.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "regulatory": "AIFMD"}'::jsonb),

('d0050000-0000-0000-0000-000000000018'::uuid, 'FORM_PF', 'Form PF', 'FUND', 'regulatory',
 'SEC filing for private fund advisers.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "jurisdictions": ["US"], "regulatory": "SEC"}'::jsonb),

('d0050000-0000-0000-0000-000000000019'::uuid, 'FORM_ADV', 'Form ADV', 'FUND', 'regulatory',
 'SEC investment adviser registration.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"], "regulatory": "SEC"}'::jsonb),

-- =============================================================================
-- FINANCIAL DOCUMENTS
-- =============================================================================
('d0060000-0000-0000-0000-000000000001'::uuid, 'BANK_STATEMENT', 'Bank Statement', 'FINANCIAL', 'banking',
 'Statement from bank showing account activity.',
 '{"entity_types": ["ALL"], "validity": {"max_age_months": 3, "reject_if_older": true}}'::jsonb),

('d0060000-0000-0000-0000-000000000002'::uuid, 'BANK_REFERENCE', 'Bank Reference Letter', 'FINANCIAL', 'banking',
 'Letter from bank confirming account relationship.',
 '{"entity_types": ["ALL"], "validity": {"max_age_months": 3}}'::jsonb),

('d0060000-0000-0000-0000-000000000003'::uuid, 'AUDITED_ACCOUNTS', 'Audited Financial Statements', 'FINANCIAL', 'accounts',
 'Annual financial statements audited by registered auditor.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"],
   "validity": {"max_age_months": 18}}'::jsonb),

('d0060000-0000-0000-0000-000000000004'::uuid, 'MANAGEMENT_ACCOUNTS', 'Management Accounts', 'FINANCIAL', 'accounts',
 'Internal unaudited financial statements.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"max_age_months": 6}}'::jsonb),

('d0060000-0000-0000-0000-000000000005'::uuid, 'SOURCE_OF_WEALTH', 'Source of Wealth Statement', 'FINANCIAL', 'wealth',
 'Declaration explaining how wealth was accumulated.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "required_for": ["high_net_worth", "pep"]}'::jsonb),

('d0060000-0000-0000-0000-000000000006'::uuid, 'SOURCE_OF_FUNDS', 'Source of Funds Statement', 'FINANCIAL', 'wealth',
 'Declaration explaining source of specific funds.',
 '{"entity_types": ["ALL"], "required_for": ["all_transactions"]}'::jsonb),

('d0060000-0000-0000-0000-000000000007'::uuid, 'NET_WORTH_STATEMENT', 'Net Worth Statement', 'FINANCIAL', 'wealth',
 'Statement of assets and liabilities.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('d0060000-0000-0000-0000-000000000008'::uuid, 'INVESTMENT_PORTFOLIO', 'Investment Portfolio Statement', 'FINANCIAL', 'investments',
 'Statement of investment holdings.',
 '{"entity_types": ["ALL"]}'::jsonb),

('d0060000-0000-0000-0000-000000000009'::uuid, 'CREDIT_REPORT', 'Credit Report', 'FINANCIAL', 'credit',
 'Credit bureau report on individual or entity.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"],
   "validity": {"max_age_months": 3}}'::jsonb),

('d0060000-0000-0000-0000-000000000010'::uuid, 'PROOF_OF_PAYMENT', 'Proof of Payment / Wire Confirmation', 'FINANCIAL', 'transaction',
 'Confirmation of payment or wire transfer.',
 '{"entity_types": ["ALL"]}'::jsonb),

('d0060000-0000-0000-0000-000000000011'::uuid, 'ACCOUNTANT_LETTER', 'Accountant Letter', 'FINANCIAL', 'verification',
 'Letter from qualified accountant confirming financial matters.',
 '{"entity_types": ["ALL"], "validity": {"max_age_months": 6}}'::jsonb),

-- =============================================================================
-- TAX DOCUMENTS
-- =============================================================================
('d0070000-0000-0000-0000-000000000001'::uuid, 'TAX_RETURN_PERSONAL', 'Personal Tax Return', 'TAX', 'filing',
 'Individual income tax return.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('d0070000-0000-0000-0000-000000000002'::uuid, 'TAX_RETURN_CORPORATE', 'Corporate Tax Return', 'TAX', 'filing',
 'Corporate income/profits tax return.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0070000-0000-0000-0000-000000000003'::uuid, 'TAX_RESIDENCY_CERT', 'Tax Residency Certificate', 'TAX', 'status',
 'Official certificate confirming tax residence.',
 '{"entity_types": ["ALL"], "validity": {"max_age_months": 12, "calendar_year": true}}'::jsonb),

('d0070000-0000-0000-0000-000000000004'::uuid, 'W8_BEN', 'Form W-8BEN', 'TAX', 'us_withholding',
 'US tax form for foreign individuals.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["US_REPORTING"],
   "validity": {"validity_years": 3, "calendar_year_expiry": true}}'::jsonb),

('d0070000-0000-0000-0000-000000000005'::uuid, 'W8_BEN_E', 'Form W-8BEN-E', 'TAX', 'us_withholding',
 'US tax form for foreign entities.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED", "FUND_HEDGE"],
   "jurisdictions": ["US_REPORTING"], "validity": {"validity_years": 3, "calendar_year_expiry": true}}'::jsonb),

('d0070000-0000-0000-0000-000000000006'::uuid, 'W9', 'Form W-9', 'TAX', 'us_withholding',
 'US tax form for US persons.',
 '{"entity_types": ["ALL"], "jurisdictions": ["US"], "validity": {"no_expiry": true, "update_on_change": true}}'::jsonb),

('d0070000-0000-0000-0000-000000000007'::uuid, 'FATCA_SELF_CERT', 'FATCA Self-Certification', 'TAX', 'fatca',
 'Self-certification of FATCA status.',
 '{"entity_types": ["ALL"], "regulatory": "FATCA"}'::jsonb),

('d0070000-0000-0000-0000-000000000008'::uuid, 'CRS_SELF_CERT', 'CRS Self-Certification', 'TAX', 'crs',
 'Self-certification for CRS reporting.',
 '{"entity_types": ["ALL"], "regulatory": "CRS"}'::jsonb),

('d0070000-0000-0000-0000-000000000009'::uuid, 'W8_IMY', 'Form W-8IMY', 'TAX', 'us_withholding',
 'US form for intermediaries.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["US_REPORTING"]}'::jsonb),

('d0070000-0000-0000-0000-000000000010'::uuid, 'W8_ECI', 'Form W-8ECI', 'TAX', 'us_withholding',
 'US form for effectively connected income.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US_REPORTING"]}'::jsonb),

('d0070000-0000-0000-0000-000000000011'::uuid, 'W8_EXP', 'Form W-8EXP', 'TAX', 'us_withholding',
 'US form for foreign governments and tax-exempt organizations.',
 '{"entity_types": ["FOUNDATION", "CHARITY"], "jurisdictions": ["US_REPORTING"]}'::jsonb),

('d0070000-0000-0000-0000-000000000012'::uuid, 'IRS_LETTER_147C', 'IRS Letter 147C', 'TAX', 'verification',
 'IRS confirmation of EIN.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"]}'::jsonb),

('d0070000-0000-0000-0000-000000000013'::uuid, 'SS4_CONFIRMATION', 'SS-4 EIN Confirmation', 'TAX', 'filing',
 'IRS confirmation of Employer Identification Number.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"]}'::jsonb),

-- =============================================================================
-- ADDRESS VERIFICATION DOCUMENTS
-- =============================================================================
('d0080000-0000-0000-0000-000000000001'::uuid, 'UTILITY_BILL', 'Utility Bill', 'ADDRESS', 'residence',
 'Recent utility bill (gas, electric, water) for address verification.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"],
   "validity": {"max_age_months": 3, "reject_if_older": true}}'::jsonb),

('d0080000-0000-0000-0000-000000000002'::uuid, 'COUNCIL_TAX', 'Council Tax Bill', 'ADDRESS', 'residence',
 'Local government tax bill.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["UK"],
   "validity": {"max_age_months": 12}}'::jsonb),

('d0080000-0000-0000-0000-000000000003'::uuid, 'LEASE_AGREEMENT', 'Lease / Rental Agreement', 'ADDRESS', 'residence',
 'Property rental agreement.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"],
   "validity": {"must_be_current": true}}'::jsonb),

('d0080000-0000-0000-0000-000000000004'::uuid, 'PROPERTY_DEED', 'Property Title Deed', 'ADDRESS', 'ownership',
 'Legal document proving property ownership.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('d0080000-0000-0000-0000-000000000005'::uuid, 'MORTGAGE_STATEMENT', 'Mortgage Statement', 'ADDRESS', 'ownership',
 'Statement from mortgage lender.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"max_age_months": 12}}'::jsonb),

('d0080000-0000-0000-0000-000000000006'::uuid, 'GOVERNMENT_CORRESPONDENCE', 'Government Correspondence', 'ADDRESS', 'residence',
 'Letter from government agency showing address.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"max_age_months": 12}}'::jsonb),

('d0080000-0000-0000-0000-000000000007'::uuid, 'INSURANCE_CERTIFICATE', 'Insurance Policy/Certificate', 'ADDRESS', 'residence',
 'Insurance document showing address.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"must_be_current": true}}'::jsonb),

-- =============================================================================
-- REGULATORY / COMPLIANCE DOCUMENTS
-- =============================================================================
('d0090000-0000-0000-0000-000000000001'::uuid, 'REGULATORY_LICENSE', 'Regulatory License', 'REGULATORY', 'license',
 'License from financial regulator.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"must_be_current": true, "verify_with_regulator": true}}'::jsonb),

('d0090000-0000-0000-0000-000000000002'::uuid, 'AML_POLICY', 'AML/KYC Policy Document', 'REGULATORY', 'policy',
 'Entity''s anti-money laundering policies.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"],
   "validity": {"max_age_months": 24}}'::jsonb),

('d0090000-0000-0000-0000-000000000003'::uuid, 'LEI_CERTIFICATE', 'LEI Certificate', 'REGULATORY', 'identifier',
 'Legal Entity Identifier certificate.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"],
   "validity": {"annual_renewal": true}}'::jsonb),

('d0090000-0000-0000-0000-000000000004'::uuid, 'POWER_OF_ATTORNEY', 'Power of Attorney', 'REGULATORY', 'authorization',
 'Legal document granting authority to act on behalf of another.',
 '{"entity_types": ["ALL"], "requires": {"notarized": true, "apostille_if_foreign": true}}'::jsonb),

('d0090000-0000-0000-0000-000000000005'::uuid, 'CORP_AUTH_LETTER', 'Corporate Authorization Letter', 'REGULATORY', 'authorization',
 'Letter authorizing individuals to act on behalf of company.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('d0090000-0000-0000-0000-000000000006'::uuid, 'SANCTIONS_CERT', 'Sanctions Compliance Certificate', 'REGULATORY', 'compliance',
 'Certification of sanctions compliance.',
 '{"entity_types": ["ALL"]}'::jsonb),

('d0090000-0000-0000-0000-000000000007'::uuid, 'WOLFSBERG_QUESTIONNAIRE', 'Wolfsberg Questionnaire', 'REGULATORY', 'compliance',
 'Standardized AML questionnaire for financial institutions.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"],
   "validity": {"max_age_months": 24},
   "required_for": ["financial_institutions"]}'::jsonb),

('d0090000-0000-0000-0000-000000000008'::uuid, 'SOC_REPORT', 'SOC 1/2 Report', 'REGULATORY', 'compliance',
 'Service Organization Control report.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"],
   "validity": {"max_age_months": 12}}'::jsonb),

('d0090000-0000-0000-0000-000000000009'::uuid, 'FCA_REGISTER_EXTRACT', 'FCA Register Extract', 'REGULATORY', 'verification',
 'Extract from UK FCA register.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["UK"],
   "validity": {"max_age_days": 30}}'::jsonb),

('d0090000-0000-0000-0000-000000000010'::uuid, 'SEC_REGISTRATION', 'SEC Registration', 'REGULATORY', 'license',
 'SEC registration document.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["US"]}'::jsonb),

('d0090000-0000-0000-0000-000000000011'::uuid, 'NFA_REGISTRATION', 'NFA Registration', 'REGULATORY', 'license',
 'National Futures Association registration.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"]}'::jsonb),

('d0090000-0000-0000-0000-000000000012'::uuid, 'CFTC_REGISTRATION', 'CFTC Registration', 'REGULATORY', 'license',
 'Commodity Futures Trading Commission registration.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"]}'::jsonb),

('d0090000-0000-0000-0000-000000000013'::uuid, 'MAS_LICENSE', 'MAS License', 'REGULATORY', 'license',
 'Monetary Authority of Singapore license.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["SG"]}'::jsonb),

('d0090000-0000-0000-0000-000000000014'::uuid, 'SFC_LICENSE', 'SFC License', 'REGULATORY', 'license',
 'Hong Kong Securities and Futures Commission license.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["HK"]}'::jsonb),

('d0090000-0000-0000-0000-000000000015'::uuid, 'CSSF_AUTHORIZATION', 'CSSF Authorization', 'REGULATORY', 'license',
 'Luxembourg financial regulator authorization.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_UCITS"], "jurisdictions": ["LU"]}'::jsonb),

('d0090000-0000-0000-0000-000000000016'::uuid, 'BAFIN_LICENSE', 'BaFin License', 'REGULATORY', 'license',
 'German financial regulator license.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["DE"]}'::jsonb),

('d0090000-0000-0000-0000-000000000017'::uuid, 'CBI_AUTHORIZATION', 'Central Bank of Ireland Authorization', 'REGULATORY', 'license',
 'Irish financial regulator authorization.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_UCITS"], "jurisdictions": ["IE"]}'::jsonb),

('d0090000-0000-0000-0000-000000000018'::uuid, 'CIMA_LICENSE', 'CIMA License', 'REGULATORY', 'license',
 'Cayman Islands Monetary Authority license.',
 '{"entity_types": ["FUND_HEDGE"], "jurisdictions": ["KY"]}'::jsonb),

-- =============================================================================
-- UBO / OWNERSHIP DOCUMENTS
-- =============================================================================
('d0100000-0000-0000-0000-000000000001'::uuid, 'UBO_DECLARATION', 'UBO Declaration Form', 'UBO', 'declaration',
 'Self-declaration of ultimate beneficial ownership.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PARTNERSHIP_LIMITED", "TRUST_DISCRETIONARY"]}'::jsonb),

('d0100000-0000-0000-0000-000000000002'::uuid, 'OWNERSHIP_CHART', 'Ownership Structure Chart', 'UBO', 'structure',
 'Diagram showing ownership chain up to UBOs.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PARTNERSHIP_LIMITED"],
   "validity": {"require_current": true}}'::jsonb),

('d0100000-0000-0000-0000-000000000003'::uuid, 'SHARE_TRANSFER', 'Share Transfer Agreement', 'UBO', 'transfer',
 'Agreement transferring share ownership.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('d0100000-0000-0000-0000-000000000004'::uuid, 'VOTING_AGREEMENT', 'Voting Agreement', 'UBO', 'control',
 'Agreement regarding voting rights.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('d0100000-0000-0000-0000-000000000005'::uuid, 'NOMINEE_AGREEMENT', 'Nominee Agreement', 'UBO', 'nominee',
 'Agreement between nominee and beneficial owner.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('d0100000-0000-0000-0000-000000000006'::uuid, 'PSC_REGISTER', 'PSC Register / BO Register', 'UBO', 'filing',
 'Register of People with Significant Control.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["UK", "EU"],
   "validity": {"require_current": true}}'::jsonb),

('d0100000-0000-0000-0000-000000000007'::uuid, 'BENEFICIAL_OWNERSHIP_FILING', 'Beneficial Ownership Filing', 'UBO', 'filing',
 'FinCEN BOI Report or equivalent filing.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LLC"], "jurisdictions": ["US"]}'::jsonb),

-- =============================================================================
-- ISDA / DERIVATIVES DOCUMENTS
-- =============================================================================
('d0110000-0000-0000-0000-000000000001'::uuid, 'ISDA_MASTER', 'ISDA Master Agreement', 'ISDA', 'master',
 'Master agreement for OTC derivatives (1992 or 2002 version).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE", "LIMITED_COMPANY_PUBLIC"],
   "versions": ["1992", "2002"]}'::jsonb),

('d0110000-0000-0000-0000-000000000002'::uuid, 'ISDA_SCHEDULE', 'ISDA Schedule', 'ISDA', 'schedule',
 'Customized terms supplementing ISDA Master Agreement.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000003'::uuid, 'CSA', 'Credit Support Annex (CSA)', 'ISDA', 'collateral',
 'Collateral arrangement for derivatives (English Law or NY Law).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"],
   "variants": ["English_Law", "NY_Law", "Japanese_Law"]}'::jsonb),

('d0110000-0000-0000-0000-000000000004'::uuid, 'ISDA_PROTOCOL_ADHERENCE', 'ISDA Protocol Adherence Letter', 'ISDA', 'protocol',
 'Letter evidencing adherence to ISDA protocols (e.g., IBOR Fallbacks).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000005'::uuid, 'ISDA_DEFINITIONS', 'ISDA Definitions Booklet', 'ISDA', 'definitions',
 'ISDA standard definitions incorporated by reference.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"],
   "versions": ["2006_Definitions", "2021_Definitions"]}'::jsonb),

('d0110000-0000-0000-0000-000000000006'::uuid, 'VM_CSA', 'Variation Margin CSA', 'ISDA', 'collateral',
 'CSA specifically for variation margin under EMIR/Dodd-Frank.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "regulatory": ["EMIR", "Dodd-Frank"]}'::jsonb),

('d0110000-0000-0000-0000-000000000007'::uuid, 'IM_CSA', 'Initial Margin CSA', 'ISDA', 'collateral',
 'CSA for initial margin (bilateral or tri-party).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "regulatory": ["EMIR", "Dodd-Frank"]}'::jsonb),

('d0110000-0000-0000-0000-000000000008'::uuid, 'ISDA_DF_PROTOCOL', 'ISDA Dodd-Frank Protocol', 'ISDA', 'protocol',
 'Adherence to Dodd-Frank regulatory protocol.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["US"]}'::jsonb),

('d0110000-0000-0000-0000-000000000009'::uuid, 'ISDA_EMIR_PROTOCOL', 'ISDA EMIR Protocol', 'ISDA', 'protocol',
 'Adherence to EMIR regulatory protocol.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["EU"]}'::jsonb),

('d0110000-0000-0000-0000-000000000010'::uuid, 'CONFIRMATION', 'Trade Confirmation', 'ISDA', 'transaction',
 'Confirmation of individual derivative transaction.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000011'::uuid, 'MNA', 'Master Netting Agreement', 'ISDA', 'netting',
 'Agreement for close-out netting across agreements.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000012'::uuid, 'GMSLA', 'Global Master Securities Lending Agreement', 'ISDA', 'lending',
 'Master agreement for securities lending.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000013'::uuid, 'GMRA', 'Global Master Repurchase Agreement', 'ISDA', 'repo',
 'Master agreement for repo transactions.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000014'::uuid, 'MSFTA', 'Master Securities Forward Transaction Agreement', 'ISDA', 'forward',
 'Master agreement for forward securities transactions.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000015'::uuid, 'ISDA_LEGAL_OPINION', 'ISDA Legal Opinion', 'ISDA', 'legal',
 'Legal opinion on enforceability of ISDA in jurisdiction.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0110000-0000-0000-0000-000000000016'::uuid, 'EMIR_CLASSIFICATION', 'EMIR Classification Letter', 'ISDA', 'regulatory',
 'Counterparty classification under EMIR (FC/NFC/NFC+).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["EU"]}'::jsonb),

('d0110000-0000-0000-0000-000000000017'::uuid, 'MIFID_CLASSIFICATION', 'MiFID II Classification', 'ISDA', 'regulatory',
 'Client classification under MiFID II.',
 '{"entity_types": ["ALL"], "jurisdictions": ["EU"],
   "classifications": ["RETAIL", "PROFESSIONAL", "ELIGIBLE_COUNTERPARTY"]}'::jsonb),

('d0110000-0000-0000-0000-000000000018'::uuid, 'CFTC_LETTER', 'CFTC Representation Letter', 'ISDA', 'regulatory',
 'Representations regarding CFTC swap dealer rules.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["US"]}'::jsonb),

-- =============================================================================
-- EMPLOYMENT / PROFESSIONAL DOCUMENTS
-- =============================================================================
('d0120000-0000-0000-0000-000000000001'::uuid, 'EMPLOYMENT_CONTRACT', 'Employment Contract', 'EMPLOYMENT', 'contract',
 'Contract of employment.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('d0120000-0000-0000-0000-000000000002'::uuid, 'PAY_SLIP', 'Pay Slip / Salary Statement', 'EMPLOYMENT', 'income',
 'Monthly salary payment slip.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"max_age_months": 3}}'::jsonb),

('d0120000-0000-0000-0000-000000000003'::uuid, 'EMPLOYMENT_LETTER', 'Employment Confirmation Letter', 'EMPLOYMENT', 'confirmation',
 'Letter from employer confirming employment.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"max_age_months": 3}}'::jsonb),

('d0120000-0000-0000-0000-000000000004'::uuid, 'PROFESSIONAL_LICENSE', 'Professional License', 'EMPLOYMENT', 'qualification',
 'License to practice a profession.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"must_be_current": true}}'::jsonb),

('d0120000-0000-0000-0000-000000000005'::uuid, 'CV_RESUME', 'CV / Resume', 'EMPLOYMENT', 'background',
 'Curriculum vitae or resume.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('d0120000-0000-0000-0000-000000000006'::uuid, 'REFERENCE_LETTER', 'Professional Reference Letter', 'EMPLOYMENT', 'background',
 'Reference letter from previous employer or colleague.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

-- =============================================================================
-- INSURANCE DOCUMENTS
-- =============================================================================
('d0130000-0000-0000-0000-000000000001'::uuid, 'PI_INSURANCE', 'Professional Indemnity Insurance', 'INSURANCE', 'liability',
 'Professional indemnity / E&O insurance certificate.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"],
   "validity": {"must_be_current": true}}'::jsonb),

('d0130000-0000-0000-0000-000000000002'::uuid, 'DO_INSURANCE', 'Directors & Officers Insurance', 'INSURANCE', 'liability',
 'D&O liability insurance certificate.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"must_be_current": true}}'::jsonb),

('d0130000-0000-0000-0000-000000000003'::uuid, 'FIDELITY_BOND', 'Fidelity Bond / Crime Insurance', 'INSURANCE', 'crime',
 'Insurance against employee dishonesty.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"],
   "validity": {"must_be_current": true}}'::jsonb),

('d0130000-0000-0000-0000-000000000004'::uuid, 'CYBER_INSURANCE', 'Cyber Insurance Certificate', 'INSURANCE', 'cyber',
 'Cyber liability insurance certificate.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"],
   "validity": {"must_be_current": true}}'::jsonb),

('d0130000-0000-0000-0000-000000000005'::uuid, 'LIFE_INSURANCE', 'Life Insurance Policy', 'INSURANCE', 'personal',
 'Life insurance policy document.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

-- =============================================================================
-- BANKING / SECURITY DOCUMENTS
-- =============================================================================
('d0140000-0000-0000-0000-000000000001'::uuid, 'ACCOUNT_OPENING_FORM', 'Account Opening Form', 'BANKING', 'account',
 'Application form for opening bank account.',
 '{"entity_types": ["ALL"]}'::jsonb),

('d0140000-0000-0000-0000-000000000002'::uuid, 'ACCOUNT_MANDATE', 'Account Mandate', 'BANKING', 'account',
 'Mandate specifying account operation rules.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('d0140000-0000-0000-0000-000000000003'::uuid, 'CREDIT_FACILITY', 'Credit Facility Agreement', 'BANKING', 'credit',
 'Agreement for credit/loan facility.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('d0140000-0000-0000-0000-000000000004'::uuid, 'PLEDGE_AGREEMENT', 'Pledge Agreement', 'BANKING', 'security',
 'Agreement pledging assets as security.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0140000-0000-0000-0000-000000000005'::uuid, 'ACCOUNT_CONTROL_AGREEMENT', 'Account Control Agreement', 'BANKING', 'security',
 'Three-party control agreement over securities account.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('d0140000-0000-0000-0000-000000000006'::uuid, 'GUARANTEE', 'Guarantee', 'BANKING', 'security',
 'Guarantee of obligations.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PROPER_PERSON_NATURAL"]}'::jsonb),

('d0140000-0000-0000-0000-000000000007'::uuid, 'LETTER_OF_CREDIT', 'Letter of Credit', 'BANKING', 'trade',
 'Documentary letter of credit.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

-- =============================================================================
-- SCREENING / VERIFICATION DOCUMENTS
-- =============================================================================
('d0150000-0000-0000-0000-000000000001'::uuid, 'SANCTIONS_SCREENING', 'Sanctions Screening Report', 'SCREENING', 'sanctions',
 'Report from sanctions screening provider.',
 '{"entity_types": ["ALL"],
   "validity": {"point_in_time": true, "ongoing_monitoring": true}}'::jsonb),

('d0150000-0000-0000-0000-000000000002'::uuid, 'PEP_SCREENING', 'PEP Screening Report', 'SCREENING', 'pep',
 'Politically Exposed Person screening report.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"point_in_time": true}}'::jsonb),

('d0150000-0000-0000-0000-000000000003'::uuid, 'ADVERSE_MEDIA', 'Adverse Media Report', 'SCREENING', 'media',
 'Negative news/media screening report.',
 '{"entity_types": ["ALL"],
   "validity": {"point_in_time": true}}'::jsonb),

('d0150000-0000-0000-0000-000000000004'::uuid, 'CRIMINAL_RECORD_CHECK', 'Criminal Record Check', 'SCREENING', 'criminal',
 'Criminal background check certificate.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"max_age_months": 6}}'::jsonb),

('d0150000-0000-0000-0000-000000000005'::uuid, 'CREDIT_CHECK', 'Credit Check Report', 'SCREENING', 'credit',
 'Credit history check.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"],
   "validity": {"max_age_months": 3}}'::jsonb),

('d0150000-0000-0000-0000-000000000006'::uuid, 'BANKRUPTCY_CHECK', 'Bankruptcy Search', 'SCREENING', 'insolvency',
 'Search for bankruptcy/insolvency records.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"],
   "validity": {"max_age_days": 30}}'::jsonb),

('d0150000-0000-0000-0000-000000000007'::uuid, 'DIRECTORSHIP_CHECK', 'Directorship Search', 'SCREENING', 'corporate',
 'Search for other directorships held.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"],
   "validity": {"max_age_days": 30}}'::jsonb),

('d0150000-0000-0000-0000-000000000008'::uuid, 'COMPANY_SEARCH', 'Company Registry Search', 'SCREENING', 'corporate',
 'Official search of company registry.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
   "validity": {"max_age_days": 30}}'::jsonb)

ON CONFLICT (type_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    category = EXCLUDED.category,
    domain = EXCLUDED.domain,
    description = EXCLUDED.description,
    applicability = EXCLUDED.applicability;

COMMIT;

-- Verification query
SELECT category, COUNT(*) as doc_count 
FROM "ob-poc".document_types 
GROUP BY category 
ORDER BY doc_count DESC;


-- ============================================================================
-- PART 2: ATTRIBUTE REGISTRY - COMPREHENSIVE
-- ============================================================================

BEGIN;

INSERT INTO "ob-poc".attribute_registry (uuid, id, display_name, category, value_type, validation_rules, applicability)
VALUES

-- =============================================================================
-- IDENTITY ATTRIBUTES (Personal)
-- =============================================================================
('a0010000-0000-0000-0000-000000000001'::uuid, 'attr.identity.full_name', 'Full Legal Name', 'identity', 'string',
 '{"required": true, "min_length": 2, "max_length": 200}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["PASSPORT", "NATIONAL_ID"]}'::jsonb),

('a0010000-0000-0000-0000-000000000002'::uuid, 'attr.identity.given_name', 'Given Name(s) / First Name', 'identity', 'string',
 '{"required": true, "min_length": 1, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["PASSPORT", "NATIONAL_ID", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000003'::uuid, 'attr.identity.family_name', 'Family Name / Surname', 'identity', 'string',
 '{"required": true, "min_length": 1, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["PASSPORT", "NATIONAL_ID", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000004'::uuid, 'attr.identity.middle_names', 'Middle Name(s)', 'identity', 'string',
 '{"required": false, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000005'::uuid, 'attr.identity.date_of_birth', 'Date of Birth', 'identity', 'date',
 '{"required": true, "min_age_years": 18, "max_age_years": 120}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["PASSPORT", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000006'::uuid, 'attr.identity.place_of_birth', 'Place of Birth', 'identity', 'string',
 '{"required": false, "max_length": 200}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["PASSPORT", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000007'::uuid, 'attr.identity.country_of_birth', 'Country of Birth', 'identity', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000008'::uuid, 'attr.identity.gender', 'Gender', 'identity', 'string',
 '{"required": false, "allowed_values": ["M", "F", "X", "MALE", "FEMALE", "OTHER", "UNSPECIFIED"]}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000009'::uuid, 'attr.identity.nationality', 'Nationality / Citizenship', 'identity', 'string',
 '{"required": true, "pattern": "^[A-Z]{2,3}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "authoritative_documents": ["PASSPORT"]}'::jsonb),

('a0010000-0000-0000-0000-000000000010'::uuid, 'attr.identity.dual_nationalities', 'Additional Nationalities', 'identity', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000011'::uuid, 'attr.identity.photo', 'Photograph', 'identity', 'string',
 '{"required": false, "format": "base64_image"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000012'::uuid, 'attr.identity.signature', 'Signature', 'identity', 'string',
 '{"required": false, "format": "base64_image"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000013'::uuid, 'attr.identity.maiden_name', 'Maiden Name / Birth Name', 'identity', 'string',
 '{"required": false, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000014'::uuid, 'attr.identity.former_names', 'Former Names / Aliases', 'identity', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000015'::uuid, 'attr.identity.marital_status', 'Marital Status', 'identity', 'string',
 '{"required": false, "allowed_values": ["SINGLE", "MARRIED", "DIVORCED", "WIDOWED", "CIVIL_PARTNERSHIP", "SEPARATED"]}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000016'::uuid, 'attr.identity.spouse_name', 'Spouse Name', 'identity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0010000-0000-0000-0000-000000000017'::uuid, 'attr.identity.father_name', 'Father''s Name', 'identity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "required_jurisdictions": ["IN", "AE", "SA"]}'::jsonb),

('a0010000-0000-0000-0000-000000000018'::uuid, 'attr.identity.mother_name', 'Mother''s Name', 'identity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

-- =============================================================================
-- DOCUMENT ATTRIBUTES
-- =============================================================================
('a0020000-0000-0000-0000-000000000001'::uuid, 'attr.document.passport_number', 'Passport Number', 'document', 'string',
 '{"required": true, "pattern": "^[A-Z0-9]{6,12}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000002'::uuid, 'attr.document.national_id_number', 'National ID Number', 'document', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000003'::uuid, 'attr.document.drivers_license_number', 'Driver''s License Number', 'document', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000004'::uuid, 'attr.document.issue_date', 'Document Issue Date', 'document', 'date',
 '{"required": true}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000005'::uuid, 'attr.document.expiry_date', 'Document Expiry Date', 'document', 'date',
 '{"required": true, "must_be_future": true}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000006'::uuid, 'attr.document.issuing_authority', 'Issuing Authority', 'document', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000007'::uuid, 'attr.document.issuing_country', 'Issuing Country', 'document', 'string',
 '{"required": true, "pattern": "^[A-Z]{2,3}$"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000008'::uuid, 'attr.document.mrz_line_1', 'MRZ Line 1', 'document', 'string',
 '{"required": false, "pattern": "^[A-Z0-9<]{30,44}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000009'::uuid, 'attr.document.mrz_line_2', 'MRZ Line 2', 'document', 'string',
 '{"required": false, "pattern": "^[A-Z0-9<]{30,44}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000010'::uuid, 'attr.document.visa_number', 'Visa/Permit Number', 'document', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0020000-0000-0000-0000-000000000011'::uuid, 'attr.document.ssn', 'Social Security Number', 'document', 'string',
 '{"required": false, "pattern": "^[0-9]{3}-[0-9]{2}-[0-9]{4}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["US"]}'::jsonb),

-- =============================================================================
-- ADDRESS ATTRIBUTES
-- =============================================================================
('a0030000-0000-0000-0000-000000000001'::uuid, 'attr.address.residential_full', 'Full Residential Address', 'address', 'string',
 '{"required": true, "max_length": 500}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000002'::uuid, 'attr.address.street_line_1', 'Street Address Line 1', 'address', 'string',
 '{"required": true, "max_length": 200}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000003'::uuid, 'attr.address.street_line_2', 'Street Address Line 2', 'address', 'string',
 '{"required": false, "max_length": 200}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000004'::uuid, 'attr.address.city', 'City / Town', 'address', 'string',
 '{"required": true, "max_length": 100}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000005'::uuid, 'attr.address.state_province', 'State / Province / Region', 'address', 'string',
 '{"required": false, "max_length": 100}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000006'::uuid, 'attr.address.postal_code', 'Postal / ZIP Code', 'address', 'string',
 '{"required": true, "max_length": 20}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000007'::uuid, 'attr.address.country', 'Country', 'address', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}$"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000008'::uuid, 'attr.address.registered_office', 'Registered Office Address', 'address', 'string',
 '{"required": true, "max_length": 500}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0030000-0000-0000-0000-000000000009'::uuid, 'attr.address.trading_address', 'Trading / Principal Place of Business', 'address', 'string',
 '{"required": false, "max_length": 500}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0030000-0000-0000-0000-000000000010'::uuid, 'attr.address.mailing_address', 'Mailing / Correspondence Address', 'address', 'string',
 '{"required": false, "max_length": 500}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000011'::uuid, 'attr.address.previous_address', 'Previous Address', 'address', 'string',
 '{"required": false, "max_length": 500}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0030000-0000-0000-0000-000000000012'::uuid, 'attr.address.time_at_address', 'Time at Current Address', 'address', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

-- =============================================================================
-- CONTACT ATTRIBUTES
-- =============================================================================
('a0031000-0000-0000-0000-000000000001'::uuid, 'attr.contact.email', 'Email Address', 'contact', 'email',
 '{"required": true, "pattern": "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0031000-0000-0000-0000-000000000002'::uuid, 'attr.contact.phone_mobile', 'Mobile Phone Number', 'contact', 'phone',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0031000-0000-0000-0000-000000000003'::uuid, 'attr.contact.phone_landline', 'Landline Phone Number', 'contact', 'phone',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0031000-0000-0000-0000-000000000004'::uuid, 'attr.contact.fax', 'Fax Number', 'contact', 'phone',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0031000-0000-0000-0000-000000000005'::uuid, 'attr.contact.website', 'Website URL', 'contact', 'string',
 '{"required": false, "format": "url"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

-- =============================================================================
-- ENTITY / CORPORATE ATTRIBUTES
-- =============================================================================
('a0040000-0000-0000-0000-000000000001'::uuid, 'attr.entity.legal_name', 'Legal Entity Name', 'entity', 'string',
 '{"required": true, "max_length": 300}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED", "LLC"], "authoritative_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000002'::uuid, 'attr.entity.trading_name', 'Trading Name / DBA', 'entity', 'string',
 '{"required": false, "max_length": 300}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000003'::uuid, 'attr.entity.registration_number', 'Company Registration Number', 'entity', 'string',
 '{"required": true, "max_length": 50}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "authoritative_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000004'::uuid, 'attr.entity.incorporation_date', 'Date of Incorporation', 'entity', 'date',
 '{"required": true}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "authoritative_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000005'::uuid, 'attr.entity.jurisdiction', 'Jurisdiction of Formation', 'entity', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}(-[A-Z0-9]+)?$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED"], "authoritative_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000006'::uuid, 'attr.entity.legal_form', 'Legal Form / Entity Type', 'entity', 'string',
 '{"required": true, "allowed_values": ["LIMITED", "LTD", "PLC", "LLC", "LP", "LLP", "CORP", "INC", "SA", "SARL", "GMBH", "AG", "BV", "NV", "AB", "AS", "OY", "SP_ZOO", "SL", "SRL", "PTY_LTD", "PTE_LTD"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000007'::uuid, 'attr.entity.authorized_capital', 'Authorized Share Capital', 'entity', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000008'::uuid, 'attr.entity.issued_capital', 'Issued Share Capital', 'entity', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000009'::uuid, 'attr.entity.paid_up_capital', 'Paid-Up Capital', 'entity', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000010'::uuid, 'attr.entity.business_activity', 'Principal Business Activity', 'entity', 'string',
 '{"required": true, "max_length": 500}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000011'::uuid, 'attr.entity.sic_code', 'SIC / NACE / NAICS Code', 'entity', 'string',
 '{"required": false, "pattern": "^[0-9]{4,6}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000012'::uuid, 'attr.entity.directors', 'Directors', 'entity', 'json',
 '{"required": true, "type": "array", "min_items": 1}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "authoritative_documents": ["REGISTER_OF_DIRECTORS", "CERT_OF_INCUMBENCY"]}'::jsonb),

('a0040000-0000-0000-0000-000000000013'::uuid, 'attr.entity.company_secretary', 'Company Secretary', 'entity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000014'::uuid, 'attr.entity.officers', 'Officers (CEO, CFO, etc.)', 'entity', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000015'::uuid, 'attr.entity.shareholders', 'Shareholders / Members', 'entity', 'json',
 '{"required": true, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "authoritative_documents": ["REGISTER_OF_SHAREHOLDERS"]}'::jsonb),

('a0040000-0000-0000-0000-000000000016'::uuid, 'attr.entity.fiscal_year_end', 'Fiscal Year End', 'entity', 'string',
 '{"required": false, "pattern": "^(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000017'::uuid, 'attr.entity.status', 'Company Status', 'entity', 'string',
 '{"required": true, "allowed_values": ["ACTIVE", "DORMANT", "DISSOLVED", "LIQUIDATION", "ADMINISTRATION", "STRUCK_OFF"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000018'::uuid, 'attr.entity.parent_company', 'Parent Company', 'entity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0040000-0000-0000-0000-000000000019'::uuid, 'attr.entity.employee_count', 'Number of Employees', 'entity', 'integer',
 '{"required": false, "min_value": 0}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

-- =============================================================================
-- TAX ATTRIBUTES
-- =============================================================================
('a0050000-0000-0000-0000-000000000001'::uuid, 'attr.tax.tin', 'Tax Identification Number', 'tax', 'string',
 '{"required": true, "max_length": 50}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0050000-0000-0000-0000-000000000002'::uuid, 'attr.tax.vat_number', 'VAT / GST Number', 'tax', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0050000-0000-0000-0000-000000000003'::uuid, 'attr.tax.tax_residence', 'Tax Residence Country', 'tax', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}$"}'::jsonb,
 '{"entity_types": ["ALL"], "authoritative_documents": ["TAX_RESIDENCY_CERT", "CRS_SELF_CERT"]}'::jsonb),

('a0050000-0000-0000-0000-000000000004'::uuid, 'attr.tax.additional_tax_residences', 'Additional Tax Residences', 'tax', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0050000-0000-0000-0000-000000000005'::uuid, 'attr.tax.giin', 'GIIN (FATCA)', 'tax', 'string',
 '{"required": false, "pattern": "^[A-Z0-9]{6}\\.[A-Z0-9]{5}\\.[A-Z]{2}\\.[0-9]{3}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0050000-0000-0000-0000-000000000006'::uuid, 'attr.tax.fatca_status', 'FATCA Status', 'tax', 'string',
 '{"required": true, "allowed_values": ["US_PERSON", "NON_US_PERSON", "PFFI", "NPFFI", "REGISTERED_DEEMED_COMPLIANT", "CERTIFIED_DEEMED_COMPLIANT", "OWNER_DOCUMENTED_FFI", "NONPARTICIPATING_FFI", "EXEMPT"]}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0050000-0000-0000-0000-000000000007'::uuid, 'attr.tax.crs_status', 'CRS Entity Classification', 'tax', 'string',
 '{"required": true, "allowed_values": ["ACTIVE_NFE", "PASSIVE_NFE", "REPORTING_FI", "NONREPORTING_FI", "GOVERNMENT", "INTERNATIONAL_ORG", "CENTRAL_BANK"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0050000-0000-0000-0000-000000000008'::uuid, 'attr.tax.us_person', 'US Person Status', 'tax', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0050000-0000-0000-0000-000000000009'::uuid, 'attr.tax.us_indicia', 'US Indicia Present', 'tax', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0050000-0000-0000-0000-000000000010'::uuid, 'attr.tax.ein', 'Employer Identification Number (EIN)', 'tax', 'string',
 '{"required": false, "pattern": "^[0-9]{2}-[0-9]{7}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"]}'::jsonb),

-- =============================================================================
-- FINANCIAL ATTRIBUTES
-- =============================================================================
('a0060000-0000-0000-0000-000000000001'::uuid, 'attr.financial.bank_account_number', 'Bank Account Number', 'financial', 'string',
 '{"required": false, "max_length": 34}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000002'::uuid, 'attr.financial.iban', 'IBAN', 'financial', 'string',
 '{"required": false, "pattern": "^[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}$"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000003'::uuid, 'attr.financial.bic_swift', 'BIC / SWIFT Code', 'financial', 'string',
 '{"required": false, "pattern": "^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000004'::uuid, 'attr.financial.bank_name', 'Bank Name', 'financial', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000005'::uuid, 'attr.financial.sort_code', 'Sort Code / Routing Number', 'financial', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000006'::uuid, 'attr.financial.account_currency', 'Account Currency', 'financial', 'string',
 '{"required": false, "pattern": "^[A-Z]{3}$"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000007'::uuid, 'attr.financial.annual_income', 'Annual Income', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000008'::uuid, 'attr.financial.net_worth', 'Net Worth', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000009'::uuid, 'attr.financial.source_of_wealth', 'Source of Wealth', 'financial', 'string',
 '{"required": true, "max_length": 1000}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000010'::uuid, 'attr.financial.source_of_funds', 'Source of Funds', 'financial', 'string',
 '{"required": true, "max_length": 1000}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000011'::uuid, 'attr.financial.expected_activity', 'Expected Account Activity', 'financial', 'json',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0060000-0000-0000-0000-000000000012'::uuid, 'attr.financial.revenue', 'Annual Revenue / Turnover', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0060000-0000-0000-0000-000000000013'::uuid, 'attr.financial.total_assets', 'Total Assets', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"]}'::jsonb),

('a0060000-0000-0000-0000-000000000014'::uuid, 'attr.financial.aum', 'Assets Under Management', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('a0060000-0000-0000-0000-000000000015'::uuid, 'attr.financial.nav', 'Net Asset Value', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS"]}'::jsonb),

-- =============================================================================
-- UBO / OWNERSHIP ATTRIBUTES
-- =============================================================================
('a0070000-0000-0000-0000-000000000001'::uuid, 'attr.ubo.beneficial_owner_name', 'Beneficial Owner Name', 'ubo', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000002'::uuid, 'attr.ubo.ownership_percentage', 'Ownership Percentage', 'ubo', 'percentage',
 '{"required": true, "min_value": 0, "max_value": 100}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000003'::uuid, 'attr.ubo.voting_percentage', 'Voting Rights Percentage', 'ubo', 'percentage',
 '{"required": false, "min_value": 0, "max_value": 100}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000004'::uuid, 'attr.ubo.control_type', 'Type of Control', 'ubo', 'string',
 '{"required": false, "allowed_values": ["OWNERSHIP", "VOTING", "BOARD_CONTROL", "OTHER_MEANS", "SENIOR_MANAGING_OFFICIAL"]}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000005'::uuid, 'attr.ubo.ownership_chain', 'Ownership Chain', 'ubo', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('a0070000-0000-0000-0000-000000000006'::uuid, 'attr.ubo.is_nominee', 'Is Nominee / Custodian', 'ubo', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000007'::uuid, 'attr.ubo.nominator', 'Nominator / True Owner', 'ubo', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000008'::uuid, 'attr.ubo.date_became_ubo', 'Date Became UBO', 'ubo', 'date',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0070000-0000-0000-0000-000000000009'::uuid, 'attr.ubo.share_class', 'Share Class', 'ubo', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

-- =============================================================================
-- TRUST ATTRIBUTES
-- =============================================================================
('a0080000-0000-0000-0000-000000000001'::uuid, 'attr.trust.trust_name', 'Trust Name', 'trust', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "authoritative_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000002'::uuid, 'attr.trust.trust_type', 'Trust Type', 'trust', 'string',
 '{"required": true, "allowed_values": ["DISCRETIONARY", "FIXED", "UNIT", "BARE", "CHARITABLE", "PURPOSE", "SPENDTHRIFT", "REVOCABLE", "IRREVOCABLE"]}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED", "TRUST_UNIT"]}'::jsonb),

('a0080000-0000-0000-0000-000000000003'::uuid, 'attr.trust.establishment_date', 'Trust Establishment Date', 'trust', 'date',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "authoritative_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000004'::uuid, 'attr.trust.governing_law', 'Trust Governing Law', 'trust', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000005'::uuid, 'attr.trust.settlor', 'Settlor / Grantor', 'trust', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "authoritative_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000006'::uuid, 'attr.trust.trustees', 'Trustees', 'trust', 'json',
 '{"required": true, "type": "array", "min_items": 1}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "authoritative_documents": ["TRUST_DEED", "DEED_OF_APPOINTMENT"]}'::jsonb),

('a0080000-0000-0000-0000-000000000007'::uuid, 'attr.trust.protector', 'Trust Protector', 'trust', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY"]}'::jsonb),

('a0080000-0000-0000-0000-000000000008'::uuid, 'attr.trust.beneficiaries', 'Beneficiaries', 'trust', 'json',
 '{"required": true, "type": "array"}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000009'::uuid, 'attr.trust.is_revocable', 'Is Revocable', 'trust', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000010'::uuid, 'attr.trust.termination_date', 'Trust Termination Date', 'trust', 'date',
 '{"required": false}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"]}'::jsonb),

-- =============================================================================
-- FUND ATTRIBUTES
-- =============================================================================
('a0081000-0000-0000-0000-000000000001'::uuid, 'attr.fund.fund_name', 'Fund Name', 'fund', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('a0081000-0000-0000-0000-000000000002'::uuid, 'attr.fund.fund_type', 'Fund Type', 'fund', 'string',
 '{"required": true, "allowed_values": ["HEDGE_FUND", "PRIVATE_EQUITY", "VENTURE_CAPITAL", "REAL_ESTATE", "UCITS", "AIF", "MUTUAL_FUND", "ETF", "FUND_OF_FUNDS"]}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('a0081000-0000-0000-0000-000000000003'::uuid, 'attr.fund.investment_strategy', 'Investment Strategy', 'fund', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('a0081000-0000-0000-0000-000000000004'::uuid, 'attr.fund.investment_manager', 'Investment Manager', 'fund', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('a0081000-0000-0000-0000-000000000005'::uuid, 'attr.fund.fund_administrator', 'Fund Administrator', 'fund', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('a0081000-0000-0000-0000-000000000006'::uuid, 'attr.fund.custodian', 'Custodian', 'fund', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('a0081000-0000-0000-0000-000000000007'::uuid, 'attr.fund.prime_broker', 'Prime Broker', 'fund', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE"]}'::jsonb),

('a0081000-0000-0000-0000-000000000008'::uuid, 'attr.fund.auditor', 'Auditor', 'fund', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"]}'::jsonb),

('a0081000-0000-0000-0000-000000000009'::uuid, 'attr.fund.legal_counsel', 'Legal Counsel', 'fund', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('a0081000-0000-0000-0000-000000000010'::uuid, 'attr.fund.minimum_investment', 'Minimum Investment', 'fund', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

('a0081000-0000-0000-0000-000000000011'::uuid, 'attr.fund.redemption_frequency', 'Redemption Frequency', 'fund', 'string',
 '{"required": false, "allowed_values": ["DAILY", "WEEKLY", "MONTHLY", "QUARTERLY", "ANNUALLY", "NONE"]}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS"]}'::jsonb),

('a0081000-0000-0000-0000-000000000012'::uuid, 'attr.fund.lock_up_period', 'Lock-Up Period', 'fund', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"]}'::jsonb),

-- =============================================================================
-- REGULATORY / COMPLIANCE ATTRIBUTES
-- =============================================================================
('a0090000-0000-0000-0000-000000000001'::uuid, 'attr.regulatory.lei', 'Legal Entity Identifier', 'compliance', 'string',
 '{"required": false, "pattern": "^[A-Z0-9]{20}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000002'::uuid, 'attr.regulatory.license_number', 'Regulatory License Number', 'compliance', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000003'::uuid, 'attr.regulatory.regulator', 'Primary Regulator', 'compliance', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000004'::uuid, 'attr.regulatory.permitted_activities', 'Permitted Activities', 'compliance', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000005'::uuid, 'attr.regulatory.pep_status', 'PEP Status', 'risk', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000006'::uuid, 'attr.regulatory.pep_type', 'PEP Type', 'risk', 'string',
 '{"required": false, "allowed_values": ["DOMESTIC_PEP", "FOREIGN_PEP", "INTERNATIONAL_ORG", "RCA", "CLOSE_ASSOCIATE"]}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000007'::uuid, 'attr.regulatory.pep_position', 'PEP Position Held', 'risk', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000008'::uuid, 'attr.regulatory.sanctions_status', 'Sanctions Status', 'risk', 'string',
 '{"required": true, "allowed_values": ["CLEAR", "POTENTIAL_MATCH", "CONFIRMED_MATCH", "FALSE_POSITIVE"]}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000009'::uuid, 'attr.regulatory.sanctions_lists_checked', 'Sanctions Lists Checked', 'risk', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000010'::uuid, 'attr.regulatory.risk_rating', 'KYC Risk Rating', 'risk', 'string',
 '{"required": true, "allowed_values": ["LOW", "MEDIUM", "HIGH", "PROHIBITED"]}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000011'::uuid, 'attr.regulatory.mifid_classification', 'MiFID II Classification', 'compliance', 'string',
 '{"required": false, "allowed_values": ["RETAIL", "PROFESSIONAL", "ELIGIBLE_COUNTERPARTY"]}'::jsonb,
 '{"entity_types": ["ALL"]}'::jsonb),

('a0090000-0000-0000-0000-000000000012'::uuid, 'attr.regulatory.emir_classification', 'EMIR Classification', 'compliance', 'string',
 '{"required": false, "allowed_values": ["FC", "NFC", "NFC_PLUS", "THIRD_COUNTRY"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

-- =============================================================================
-- EMPLOYMENT ATTRIBUTES
-- =============================================================================
('a0100000-0000-0000-0000-000000000001'::uuid, 'attr.employment.employer_name', 'Employer Name', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0100000-0000-0000-0000-000000000002'::uuid, 'attr.employment.job_title', 'Job Title / Position', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0100000-0000-0000-0000-000000000003'::uuid, 'attr.employment.employment_status', 'Employment Status', 'employment', 'string',
 '{"required": false, "allowed_values": ["EMPLOYED", "SELF_EMPLOYED", "RETIRED", "UNEMPLOYED", "STUDENT", "HOMEMAKER"]}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0100000-0000-0000-0000-000000000004'::uuid, 'attr.employment.industry', 'Industry / Sector', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0100000-0000-0000-0000-000000000005'::uuid, 'attr.employment.employer_address', 'Employer Address', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

('a0100000-0000-0000-0000-000000000006'::uuid, 'attr.employment.years_employed', 'Years at Current Employer', 'employment', 'integer',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"]}'::jsonb),

-- =============================================================================
-- ISDA / DERIVATIVES ATTRIBUTES
-- =============================================================================
('a0110000-0000-0000-0000-000000000001'::uuid, 'attr.isda.master_date', 'ISDA Master Agreement Date', 'isda', 'date',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000002'::uuid, 'attr.isda.master_version', 'ISDA Master Version', 'isda', 'string',
 '{"required": false, "allowed_values": ["1992", "2002"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000003'::uuid, 'attr.isda.governing_law', 'ISDA Governing Law', 'isda', 'string',
 '{"required": false, "allowed_values": ["ENGLISH", "NEW_YORK", "JAPANESE"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000004'::uuid, 'attr.isda.csa_type', 'CSA Type', 'isda', 'string',
 '{"required": false, "allowed_values": ["VM_ONLY", "IM_ONLY", "VM_AND_IM", "NONE"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000005'::uuid, 'attr.isda.csa_governing_law', 'CSA Governing Law', 'isda', 'string',
 '{"required": false, "allowed_values": ["ENGLISH", "NEW_YORK", "JAPANESE"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000006'::uuid, 'attr.isda.threshold_amount', 'CSA Threshold Amount', 'isda', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000007'::uuid, 'attr.isda.mta', 'Minimum Transfer Amount', 'isda', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000008'::uuid, 'attr.isda.rounding', 'CSA Rounding', 'isda', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000009'::uuid, 'attr.isda.eligible_collateral', 'Eligible Collateral', 'isda', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000010'::uuid, 'attr.isda.valuation_agent', 'Valuation Agent', 'isda', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

('a0110000-0000-0000-0000-000000000011'::uuid, 'attr.isda.netting_agreement', 'Cross-Agreement Netting', 'isda', 'boolean',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"]}'::jsonb),

-- =============================================================================
-- PARTNERSHIP-SPECIFIC ATTRIBUTES
-- =============================================================================
('a0120000-0000-0000-0000-000000000001'::uuid, 'attr.partnership.partnership_name', 'Partnership Name', 'partnership', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"]}'::jsonb),

('a0120000-0000-0000-0000-000000000002'::uuid, 'attr.partnership.general_partner', 'General Partner', 'partnership', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb),

('a0120000-0000-0000-0000-000000000003'::uuid, 'attr.partnership.commitment_amount', 'Capital Commitment', 'partnership', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb),

('a0120000-0000-0000-0000-000000000004'::uuid, 'attr.partnership.contributed_capital', 'Contributed Capital', 'partnership', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb),

('a0120000-0000-0000-0000-000000000005'::uuid, 'attr.partnership.unfunded_commitment', 'Unfunded Commitment', 'partnership', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb),

('a0120000-0000-0000-0000-000000000006'::uuid, 'attr.partnership.partnership_interest', 'Partnership Interest %', 'partnership', 'percentage',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PARTNERSHIP_LIMITED"]}'::jsonb)

ON CONFLICT (id) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    category = EXCLUDED.category,
    value_type = EXCLUDED.value_type,
    validation_rules = EXCLUDED.validation_rules,
    applicability = EXCLUDED.applicability;

COMMIT;

-- Verification
SELECT category, COUNT(*) as attr_count 
FROM "ob-poc".attribute_registry 
GROUP BY category 
ORDER BY attr_count DESC;


-- ============================================================================
-- PART 3: DOCUMENT-ATTRIBUTE LINKS (SOURCE / SINK)
-- ============================================================================
-- SOURCE = Document can extract/provide this attribute
-- SINK = Attribute can be proven/fulfilled by this document
-- ============================================================================

BEGIN;

-- Create table if not exists (for observation model)
CREATE TABLE IF NOT EXISTS "ob-poc".document_attribute_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(type_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    direction VARCHAR(10) NOT NULL CHECK (direction IN ('SOURCE', 'SINK', 'BOTH')),
    extraction_method VARCHAR(50),
    extraction_field_path JSONB,
    extraction_confidence_default NUMERIC(3,2) DEFAULT 0.80,
    is_authoritative BOOLEAN DEFAULT FALSE,
    proof_strength VARCHAR(20) CHECK (proof_strength IN ('PRIMARY', 'SECONDARY', 'SUPPORTING')),
    entity_types TEXT[],
    jurisdictions TEXT[],
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT unique_doc_attr_dir UNIQUE (document_type_id, attribute_id, direction)
);

-- Clear existing links
DELETE FROM "ob-poc".document_attribute_links;

INSERT INTO "ob-poc".document_attribute_links 
(document_type_id, attribute_id, direction, extraction_method, extraction_confidence_default, is_authoritative, proof_strength)
VALUES

-- =============================================================================
-- PASSPORT - SOURCE (Extraction)
-- =============================================================================
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'MRZ', 0.95, TRUE, 'PRIMARY'),   -- full_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000002', 'SOURCE', 'MRZ', 0.95, TRUE, 'PRIMARY'),   -- given_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000003', 'SOURCE', 'MRZ', 0.95, TRUE, 'PRIMARY'),   -- family_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- date_of_birth
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),   -- place_of_birth
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000008', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- gender
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000009', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- nationality
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000011', 'SOURCE', 'IMAGE', 0.99, TRUE, 'PRIMARY'), -- photo
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000012', 'SOURCE', 'IMAGE', 0.90, TRUE, 'PRIMARY'), -- signature
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000001', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- passport_number
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.90, TRUE, NULL),        -- issue_date
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000005', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- expiry_date
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.85, FALSE, NULL),       -- issuing_authority
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000007', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- issuing_country
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000008', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- mrz_line_1
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000009', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- mrz_line_2

-- PASSPORT - SINK (What it proves)
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves full_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves dob
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000009', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves nationality
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000011', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves photo/likeness

-- =============================================================================
-- NATIONAL ID
-- =============================================================================
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000008', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000011', 'SOURCE', 'IMAGE', 0.99, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0020000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0020000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.90, TRUE, NULL),
('d0010000-0000-0000-0000-000000000002', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SECONDARY'),
-- SINK
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- DRIVER'S LICENSE
-- =============================================================================
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.85, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.90, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000011', 'SOURCE', 'IMAGE', 0.95, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0020000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000003', 'a0020000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.85, FALSE, NULL),
('d0010000-0000-0000-0000-000000000003', 'a0020000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.90, FALSE, NULL),
('d0010000-0000-0000-0000-000000000003', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SECONDARY'),
-- SINK
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0030000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, FALSE, 'SECONDARY'),

-- =============================================================================
-- BIRTH CERTIFICATE
-- =============================================================================
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000007', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000017', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000018', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),
-- SINK
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000006', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000007', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- CERTIFICATE OF INCORPORATION
-- =============================================================================
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),   -- legal_name
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),   -- registration_number
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),   -- incorporation_date
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),   -- jurisdiction
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),   -- legal_form
('d0020000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000008', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),   -- registered_office
-- SINK
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000003', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000004', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- ARTICLES OF ASSOCIATION / BYLAWS
-- =============================================================================
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.85, FALSE, 'SECONDARY'),
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),    -- authorized_capital
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000010', 'SOURCE', 'AI', 0.75, TRUE, 'PRIMARY'),   -- business_activity
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000016', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),   -- fiscal_year_end

-- =============================================================================
-- CERT OF GOOD STANDING
-- =============================================================================
('d0020000-0000-0000-0000-000000000004', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),
('d0020000-0000-0000-0000-000000000004', 'a0040000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),
('d0020000-0000-0000-0000-000000000004', 'a0040000-0000-0000-0000-000000000017', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),   -- status
-- SINK
('d0020000-0000-0000-0000-000000000004', 'a0040000-0000-0000-0000-000000000017', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- CERT OF INCUMBENCY
-- =============================================================================
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000012', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- directors
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000013', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- company_secretary
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000015', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- shareholders
-- SINK
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000012', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000015', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- REGISTER OF DIRECTORS
-- =============================================================================
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000012', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000013', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000014', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- officers
-- SINK
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000012', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- REGISTER OF SHAREHOLDERS
-- =============================================================================
('d0020000-0000-0000-0000-000000000007', 'a0040000-0000-0000-0000-000000000015', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000007', 'a0040000-0000-0000-0000-000000000008', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- issued_capital
('d0020000-0000-0000-0000-000000000007', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- ownership_percentage
-- SINK
('d0020000-0000-0000-0000-000000000007', 'a0040000-0000-0000-0000-000000000015', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000007', 'a0070000-0000-0000-0000-000000000002', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- TRUST DEED
-- =============================================================================
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- trust_name
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- trust_type
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000003', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- establishment_date
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- governing_law
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000005', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- settlor
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000006', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- trustees
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.80, FALSE, 'PRIMARY'),   -- protector
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000008', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),    -- beneficiaries
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000009', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- is_revocable
-- SINK
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000006', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000008', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- PARTNERSHIP AGREEMENT
-- =============================================================================
('d0030000-0000-0000-0000-000000000001', 'a0120000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- partnership_name
('d0030000-0000-0000-0000-000000000001', 'a0120000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- general_partner
('d0030000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000005', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- jurisdiction
-- SINK
('d0030000-0000-0000-0000-000000000001', 'a0120000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0030000-0000-0000-0000-000000000001', 'a0120000-0000-0000-0000-000000000002', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- UTILITY BILL (Address Verification)
-- =============================================================================
('d0080000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SUPPORTING'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000006', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
-- SINK
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- BANK STATEMENT
-- =============================================================================
('d0060000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.85, FALSE, 'SUPPORTING'),
('d0060000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SECONDARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
-- SINK
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000002', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, FALSE, 'SECONDARY'),

-- =============================================================================
-- TAX DOCUMENTS
-- =============================================================================
-- Tax Residency Certificate
('d0070000-0000-0000-0000-000000000003', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000003', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),
-- SINK
('d0070000-0000-0000-0000-000000000003', 'a0050000-0000-0000-0000-000000000003', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- W-8BEN (Foreign Individual)
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000006', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000008', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
-- SINK
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000006', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000008', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- W-8BEN-E (Foreign Entity)
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000005', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000006', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000007', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000008', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
-- SINK
('d0070000-0000-0000-0000-000000000005', 'a0050000-0000-0000-0000-000000000006', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- W-9 (US Person)
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000008', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000010', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
-- SINK
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000008', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- CRS Self-Certification
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000004', 'SOURCE', 'FORM_FIELD', 0.90, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000007', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
-- SINK
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000003', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000007', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- UBO DOCUMENTS
-- =============================================================================
-- UBO Declaration
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.85, FALSE, 'SECONDARY'),
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'FORM_FIELD', 0.85, FALSE, 'SECONDARY'),
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.80, FALSE, 'SECONDARY'),
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000004', 'SOURCE', 'FORM_FIELD', 0.80, FALSE, 'SECONDARY'),

-- Ownership Chart
('d0100000-0000-0000-0000-000000000002', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),
('d0100000-0000-0000-0000-000000000002', 'a0070000-0000-0000-0000-000000000005', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),
-- SINK
('d0100000-0000-0000-0000-000000000002', 'a0070000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- LEI CERTIFICATE
-- =============================================================================
('d0090000-0000-0000-0000-000000000003', 'a0090000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),
('d0090000-0000-0000-0000-000000000003', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),
-- SINK
('d0090000-0000-0000-0000-000000000003', 'a0090000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- REGULATORY LICENSE
-- =============================================================================
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),
-- SINK
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000002', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- ISDA MASTER AGREEMENT
-- =============================================================================
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.95, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000003', 'SOURCE', 'AI', 0.95, TRUE, 'PRIMARY'),
-- SINK
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000003', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- CSA (Credit Support Annex)
-- =============================================================================
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000005', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000006', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000008', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000009', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000010', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
-- SINK
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000004', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000006', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- EMPLOYMENT DOCUMENTS
-- =============================================================================
-- Employment Contract
('d0120000-0000-0000-0000-000000000001', 'a0100000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0120000-0000-0000-0000-000000000001', 'a0100000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0120000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),

-- Pay Slip
('d0120000-0000-0000-0000-000000000002', 'a0100000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, FALSE, 'SUPPORTING'),
('d0120000-0000-0000-0000-000000000002', 'a0060000-0000-0000-0000-000000000007', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
-- SINK
('d0120000-0000-0000-0000-000000000002', 'a0060000-0000-0000-0000-000000000007', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- AUDITED ACCOUNTS
-- =============================================================================
('d0060000-0000-0000-0000-000000000003', 'a0060000-0000-0000-0000-000000000012', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000003', 'a0060000-0000-0000-0000-000000000013', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
-- SINK
('d0060000-0000-0000-0000-000000000003', 'a0060000-0000-0000-0000-000000000012', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0060000-0000-0000-0000-000000000003', 'a0060000-0000-0000-0000-000000000013', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- MIFID CLASSIFICATION
-- =============================================================================
('d0110000-0000-0000-0000-000000000017', 'a0090000-0000-0000-0000-000000000011', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
-- SINK
('d0110000-0000-0000-0000-000000000017', 'a0090000-0000-0000-0000-000000000011', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- EMIR CLASSIFICATION
-- =============================================================================
('d0110000-0000-0000-0000-000000000016', 'a0090000-0000-0000-0000-000000000012', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'),
-- SINK
('d0110000-0000-0000-0000-000000000016', 'a0090000-0000-0000-0000-000000000012', 'SINK', NULL, NULL, TRUE, 'PRIMARY')

ON CONFLICT (document_type_id, attribute_id, direction) DO UPDATE SET
    extraction_method = EXCLUDED.extraction_method,
    extraction_confidence_default = EXCLUDED.extraction_confidence_default,
    is_authoritative = EXCLUDED.is_authoritative,
    proof_strength = EXCLUDED.proof_strength;

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Document counts by category
SELECT category, COUNT(*) as doc_count 
FROM "ob-poc".document_types 
GROUP BY category 
ORDER BY doc_count DESC;

-- Attribute counts by category
SELECT category, COUNT(*) as attr_count 
FROM "ob-poc".attribute_registry 
GROUP BY category 
ORDER BY attr_count DESC;

-- Links summary
SELECT 
    direction,
    COUNT(*) as link_count,
    COUNT(*) FILTER (WHERE is_authoritative) as authoritative_count
FROM "ob-poc".document_attribute_links
GROUP BY direction;

-- Top document types by extraction coverage
SELECT 
    dt.type_code,
    dt.display_name,
    COUNT(*) FILTER (WHERE dal.direction = 'SOURCE') as extracts,
    COUNT(*) FILTER (WHERE dal.direction = 'SINK') as proves,
    COUNT(*) FILTER (WHERE dal.is_authoritative) as authoritative
FROM "ob-poc".document_types dt
LEFT JOIN "ob-poc".document_attribute_links dal ON dt.type_id = dal.document_type_id
GROUP BY dt.type_id, dt.type_code, dt.display_name
HAVING COUNT(*) > 0
ORDER BY extracts DESC
LIMIT 20;

-- What documents can prove identity attributes?
SELECT 
    ar.id as attribute,
    ar.display_name,
    array_agg(dt.type_code ORDER BY dal.proof_strength) as proof_documents
FROM "ob-poc".document_attribute_links dal
JOIN "ob-poc".document_types dt ON dal.document_type_id = dt.type_id
JOIN "ob-poc".attribute_registry ar ON dal.attribute_id = ar.uuid
WHERE ar.category = 'identity' 
  AND dal.direction IN ('SINK', 'BOTH')
GROUP BY ar.id, ar.display_name
ORDER BY ar.id;
