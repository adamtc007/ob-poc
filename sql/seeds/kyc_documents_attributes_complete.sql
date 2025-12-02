-- ============================================================================
-- COMPREHENSIVE KYC DOCUMENT TYPES AND ATTRIBUTE REGISTRY
-- Seeds for the observation-based KYC model
-- ============================================================================
-- 
-- This file provides:
-- 1. Complete document type catalog (60+ document types)
-- 2. Comprehensive attribute registry (100+ attributes)
-- 3. Document-Attribute links (SOURCE and SINK directions)
--
-- Document → Attribute (SOURCE): Document provides/extracts this attribute
-- Document ← Attribute (SINK): Attribute requires this document as proof
-- ============================================================================

BEGIN;

-- ============================================================================
-- PART 1: DOCUMENT TYPES
-- ============================================================================

-- Clear existing (careful in production!)
-- DELETE FROM "ob-poc".document_types;

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, applicability)
VALUES

-- -----------------------------------------------------------------------------
-- PERSONAL IDENTITY DOCUMENTS
-- -----------------------------------------------------------------------------
('d0010000-0000-0000-0000-000000000001'::uuid, 'PASSPORT', 'Passport', 'IDENTITY', 'personal',
 'Government-issued international travel document. Primary identity document with MRZ for automated reading.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_primary_id": true, "accepted_globally": true}'::jsonb),

('d0010000-0000-0000-0000-000000000002'::uuid, 'NATIONAL_ID', 'National Identity Card', 'IDENTITY', 'personal',
 'Government-issued national identity card. Primary ID in many jurisdictions.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_primary_id": true, "regional_acceptance": ["EU", "EEA"]}'::jsonb),

('d0010000-0000-0000-0000-000000000003'::uuid, 'DRIVERS_LICENSE', 'Driver''s License', 'IDENTITY', 'personal',
 'Government-issued driving permit. Secondary ID document, also proves address in some jurisdictions.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_secondary_id": true, "proves_address": ["US", "UK", "AU"]}'::jsonb),

('d0010000-0000-0000-0000-000000000004'::uuid, 'BIRTH_CERTIFICATE', 'Birth Certificate', 'IDENTITY', 'personal',
 'Official record of birth. Used for identity verification and citizenship proof.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["identity", "nationality", "parentage"]}'::jsonb),

('d0010000-0000-0000-0000-000000000005'::uuid, 'RESIDENCE_PERMIT', 'Residence Permit / Visa', 'IDENTITY', 'personal',
 'Immigration document permitting residence in a country.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["right_to_reside", "immigration_status"]}'::jsonb),

('d0010000-0000-0000-0000-000000000006'::uuid, 'MARRIAGE_CERTIFICATE', 'Marriage Certificate', 'IDENTITY', 'personal',
 'Official record of marriage. Used for name changes and relationship verification.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["marital_status", "name_change"]}'::jsonb),

('d0010000-0000-0000-0000-000000000007'::uuid, 'DEATH_CERTIFICATE', 'Death Certificate', 'IDENTITY', 'personal',
 'Official record of death. Required for estate matters.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["death", "cause_of_death"]}'::jsonb),

('d0010000-0000-0000-0000-000000000008'::uuid, 'DEED_POLL', 'Deed Poll / Name Change Document', 'IDENTITY', 'personal',
 'Legal document evidencing a name change.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["name_change", "identity_continuity"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- CORPORATE FORMATION DOCUMENTS
-- -----------------------------------------------------------------------------
('d0020000-0000-0000-0000-000000000001'::uuid, 'CERT_OF_INCORPORATION', 'Certificate of Incorporation', 'CORPORATE', 'formation',
 'Official document from registrar confirming company formation.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"], "is_primary_formation": true}'::jsonb),

('d0020000-0000-0000-0000-000000000002'::uuid, 'ARTICLES_OF_ASSOCIATION', 'Articles of Association', 'CORPORATE', 'formation',
 'Constitutional document defining company rules and governance. Called Bylaws in US.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "aka": ["Bylaws", "Constitution"]}'::jsonb),

('d0020000-0000-0000-0000-000000000003'::uuid, 'MEMORANDUM_OF_ASSOCIATION', 'Memorandum of Association', 'CORPORATE', 'formation',
 'Document stating company objects and authorized capital. Often combined with Articles.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["UK", "HK", "SG"]}'::jsonb),

('d0020000-0000-0000-0000-000000000004'::uuid, 'CERT_OF_GOOD_STANDING', 'Certificate of Good Standing', 'CORPORATE', 'status',
 'Registrar certificate confirming company is in compliance and active.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC", "PARTNERSHIP_LIMITED"], "validity_period_days": 90}'::jsonb),

('d0020000-0000-0000-0000-000000000005'::uuid, 'CERT_OF_INCUMBENCY', 'Certificate of Incumbency', 'CORPORATE', 'status',
 'Document listing current directors, officers, and shareholders. Common in offshore jurisdictions.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["KY", "BVI", "JE", "GG"]}'::jsonb),

('d0020000-0000-0000-0000-000000000006'::uuid, 'REGISTER_OF_DIRECTORS', 'Register of Directors', 'CORPORATE', 'governance',
 'Official register of past and present directors.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["directors", "appointments", "resignations"]}'::jsonb),

('d0020000-0000-0000-0000-000000000007'::uuid, 'REGISTER_OF_SHAREHOLDERS', 'Register of Shareholders/Members', 'CORPORATE', 'ownership',
 'Official register of shareholdings and transfers.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["ownership", "shareholdings"]}'::jsonb),

('d0020000-0000-0000-0000-000000000008'::uuid, 'SHARE_CERTIFICATE', 'Share Certificate', 'CORPORATE', 'ownership',
 'Certificate evidencing ownership of shares.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["shareholding", "share_class"]}'::jsonb),

('d0020000-0000-0000-0000-000000000009'::uuid, 'BOARD_RESOLUTION', 'Board Resolution', 'CORPORATE', 'governance',
 'Formal decision of the board of directors.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["authorization", "decision"]}'::jsonb),

('d0020000-0000-0000-0000-000000000010'::uuid, 'SHAREHOLDER_RESOLUTION', 'Shareholder Resolution', 'CORPORATE', 'governance',
 'Formal decision of shareholders in general meeting.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["authorization", "special_resolution"]}'::jsonb),

('d0020000-0000-0000-0000-000000000011'::uuid, 'SIGNATORY_LIST', 'Authorized Signatory List', 'CORPORATE', 'governance',
 'List of persons authorized to sign on behalf of the entity.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED", "TRUST_DISCRETIONARY"], "proves": ["signing_authority"]}'::jsonb),

('d0020000-0000-0000-0000-000000000012'::uuid, 'SPECIMEN_SIGNATURES', 'Specimen Signature Card', 'CORPORATE', 'governance',
 'Card showing specimen signatures of authorized signatories.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["signature_verification"]}'::jsonb),

('d0020000-0000-0000-0000-000000000013'::uuid, 'ANNUAL_RETURN', 'Annual Return / Confirmation Statement', 'CORPORATE', 'filing',
 'Annual filing with registrar confirming company details.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "filing_frequency": "annual"}'::jsonb),

('d0020000-0000-0000-0000-000000000014'::uuid, 'CHANGE_OF_DIRECTORS', 'Notice of Change of Directors', 'CORPORATE', 'filing',
 'Filing notifying registrar of director appointments/resignations.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["director_change"]}'::jsonb),

('d0020000-0000-0000-0000-000000000015'::uuid, 'OPERATING_AGREEMENT', 'LLC Operating Agreement', 'CORPORATE', 'formation',
 'Governing document for Limited Liability Companies.',
 '{"entity_types": ["LLC"], "jurisdictions": ["US"], "is_primary_formation": true}'::jsonb),

-- -----------------------------------------------------------------------------
-- PARTNERSHIP DOCUMENTS
-- -----------------------------------------------------------------------------
('d0030000-0000-0000-0000-000000000001'::uuid, 'PARTNERSHIP_AGREEMENT', 'Partnership Agreement', 'PARTNERSHIP', 'formation',
 'Agreement governing the partnership (LP, LLP, GP).',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP", "PARTNERSHIP_GENERAL"], "is_primary_formation": true}'::jsonb),

('d0030000-0000-0000-0000-000000000002'::uuid, 'LP_CERTIFICATE', 'Certificate of Limited Partnership', 'PARTNERSHIP', 'formation',
 'Official registration certificate for limited partnership.',
 '{"entity_types": ["PARTNERSHIP_LIMITED"], "is_primary_formation": true}'::jsonb),

('d0030000-0000-0000-0000-000000000003'::uuid, 'REGISTER_OF_PARTNERS', 'Register of Partners', 'PARTNERSHIP', 'ownership',
 'Register of general and limited partners.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"], "proves": ["partner_interests", "commitments"]}'::jsonb),

('d0030000-0000-0000-0000-000000000004'::uuid, 'PARTNER_ADMISSION_LETTER', 'Partner Admission Letter', 'PARTNERSHIP', 'ownership',
 'Letter confirming admission of new partner.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"], "proves": ["partner_admission"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- TRUST DOCUMENTS
-- -----------------------------------------------------------------------------
('d0040000-0000-0000-0000-000000000001'::uuid, 'TRUST_DEED', 'Trust Deed / Trust Instrument', 'TRUST', 'formation',
 'Primary document establishing the trust and its terms.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED", "TRUST_UNIT"], "is_primary_formation": true}'::jsonb),

('d0040000-0000-0000-0000-000000000002'::uuid, 'LETTER_OF_WISHES', 'Letter of Wishes', 'TRUST', 'guidance',
 'Non-binding guidance from settlor to trustees regarding trust administration.',
 '{"entity_types": ["TRUST_DISCRETIONARY"], "is_confidential": true}'::jsonb),

('d0040000-0000-0000-0000-000000000003'::uuid, 'DEED_OF_APPOINTMENT', 'Deed of Appointment (Trustees)', 'TRUST', 'governance',
 'Document appointing new trustees.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "proves": ["trustee_appointment"]}'::jsonb),

('d0040000-0000-0000-0000-000000000004'::uuid, 'DEED_OF_RETIREMENT', 'Deed of Retirement (Trustees)', 'TRUST', 'governance',
 'Document evidencing trustee retirement.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "proves": ["trustee_retirement"]}'::jsonb),

('d0040000-0000-0000-0000-000000000005'::uuid, 'SCHEDULE_OF_BENEFICIARIES', 'Schedule of Beneficiaries', 'TRUST', 'beneficial',
 'List of trust beneficiaries and their entitlements.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "proves": ["beneficiaries", "entitlements"]}'::jsonb),

('d0040000-0000-0000-0000-000000000006'::uuid, 'TRUST_FINANCIAL_STATEMENT', 'Trust Financial Statement', 'TRUST', 'financial',
 'Statement of trust assets and liabilities.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED", "TRUST_UNIT"], "proves": ["trust_assets"]}'::jsonb),

('d0040000-0000-0000-0000-000000000007'::uuid, 'DEED_OF_ADDITION', 'Deed of Addition', 'TRUST', 'assets',
 'Document adding assets to the trust.',
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "proves": ["asset_addition"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- FUND / INVESTMENT DOCUMENTS
-- -----------------------------------------------------------------------------
('d0050000-0000-0000-0000-000000000001'::uuid, 'FUND_PROSPECTUS', 'Fund Prospectus', 'FUND', 'offering',
 'Regulatory offering document for public/retail funds (UCITS, etc.).',
 '{"entity_types": ["FUND_UCITS", "FUND_MUTUAL"], "regulatory": true}'::jsonb),

('d0050000-0000-0000-0000-000000000002'::uuid, 'OFFERING_MEMORANDUM', 'Offering Memorandum / PPM', 'FUND', 'offering',
 'Private Placement Memorandum for hedge funds and private funds.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_VC"], "aka": ["PPM", "OM", "Information Memorandum"]}'::jsonb),

('d0050000-0000-0000-0000-000000000003'::uuid, 'IMA', 'Investment Management Agreement', 'FUND', 'service',
 'Agreement between fund and investment manager.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"], "proves": ["manager_appointment"]}'::jsonb),

('d0050000-0000-0000-0000-000000000004'::uuid, 'SUBSCRIPTION_AGREEMENT', 'Subscription Agreement', 'FUND', 'investor',
 'Agreement for investor subscription to fund.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "proves": ["investor_commitment", "subscription_amount"]}'::jsonb),

('d0050000-0000-0000-0000-000000000005'::uuid, 'SIDE_LETTER', 'Side Letter', 'FUND', 'investor',
 'Supplemental agreement granting special terms to investor.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "proves": ["special_terms", "fee_arrangements"]}'::jsonb),

('d0050000-0000-0000-0000-000000000006'::uuid, 'NAV_STATEMENT', 'NAV Statement', 'FUND', 'valuation',
 'Statement of Net Asset Value per share/unit.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"], "proves": ["nav", "investor_balance"]}'::jsonb),

('d0050000-0000-0000-0000-000000000007'::uuid, 'KIID', 'Key Investor Information Document', 'FUND', 'disclosure',
 'Standardized EU fund disclosure document.',
 '{"entity_types": ["FUND_UCITS"], "jurisdictions": ["EU"], "regulatory": true}'::jsonb),

('d0050000-0000-0000-0000-000000000008'::uuid, 'FUND_CONSTITUTION', 'Fund Constitution / Instrument', 'FUND', 'formation',
 'Constitutional document establishing the fund.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS", "FUND_UNIT_TRUST"], "is_primary_formation": true}'::jsonb),

-- -----------------------------------------------------------------------------
-- FINANCIAL DOCUMENTS
-- -----------------------------------------------------------------------------
('d0060000-0000-0000-0000-000000000001'::uuid, 'BANK_STATEMENT', 'Bank Statement', 'FINANCIAL', 'banking',
 'Statement from bank showing account activity.',
 '{"entity_types": ["ALL"], "proves": ["account_existence", "balance", "transactions", "address"], "max_age_months": 3}'::jsonb),

('d0060000-0000-0000-0000-000000000002'::uuid, 'BANK_REFERENCE', 'Bank Reference Letter', 'FINANCIAL', 'banking',
 'Letter from bank confirming account relationship.',
 '{"entity_types": ["ALL"], "proves": ["banking_relationship", "account_standing"]}'::jsonb),

('d0060000-0000-0000-0000-000000000003'::uuid, 'AUDITED_ACCOUNTS', 'Audited Financial Statements', 'FINANCIAL', 'accounts',
 'Annual financial statements audited by registered auditor.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["financial_position", "revenue", "assets"]}'::jsonb),

('d0060000-0000-0000-0000-000000000004'::uuid, 'MANAGEMENT_ACCOUNTS', 'Management Accounts', 'FINANCIAL', 'accounts',
 'Internal unaudited financial statements.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["current_financial_position"]}'::jsonb),

('d0060000-0000-0000-0000-000000000005'::uuid, 'SOURCE_OF_WEALTH', 'Source of Wealth Statement', 'FINANCIAL', 'wealth',
 'Declaration explaining how wealth was accumulated.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["wealth_origin"], "required_for": ["high_net_worth", "pep"]}'::jsonb),

('d0060000-0000-0000-0000-000000000006'::uuid, 'SOURCE_OF_FUNDS', 'Source of Funds Statement', 'FINANCIAL', 'wealth',
 'Declaration explaining source of specific funds being invested/deposited.',
 '{"entity_types": ["ALL"], "proves": ["funds_origin"], "required_for": ["all_transactions"]}'::jsonb),

('d0060000-0000-0000-0000-000000000007'::uuid, 'NET_WORTH_STATEMENT', 'Net Worth Statement', 'FINANCIAL', 'wealth',
 'Statement of assets and liabilities.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["net_worth", "asset_breakdown"]}'::jsonb),

('d0060000-0000-0000-0000-000000000008'::uuid, 'INVESTMENT_PORTFOLIO', 'Investment Portfolio Statement', 'FINANCIAL', 'investments',
 'Statement of investment holdings.',
 '{"entity_types": ["ALL"], "proves": ["investments", "portfolio_value"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- TAX DOCUMENTS
-- -----------------------------------------------------------------------------
('d0070000-0000-0000-0000-000000000001'::uuid, 'TAX_RETURN_PERSONAL', 'Personal Tax Return', 'TAX', 'filing',
 'Individual income tax return.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["income", "tax_residence"]}'::jsonb),

('d0070000-0000-0000-0000-000000000002'::uuid, 'TAX_RETURN_CORPORATE', 'Corporate Tax Return', 'TAX', 'filing',
 'Corporate income/profits tax return.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["taxable_income", "tax_residence"]}'::jsonb),

('d0070000-0000-0000-0000-000000000003'::uuid, 'TAX_RESIDENCY_CERT', 'Tax Residency Certificate', 'TAX', 'status',
 'Official certificate confirming tax residence.',
 '{"entity_types": ["ALL"], "proves": ["tax_residence"], "issued_by": "tax_authority"}'::jsonb),

('d0070000-0000-0000-0000-000000000004'::uuid, 'W8_BEN', 'Form W-8BEN', 'TAX', 'us_withholding',
 'US tax form for foreign persons (individuals).',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["US_REPORTING"], "proves": ["foreign_status", "treaty_benefits"]}'::jsonb),

('d0070000-0000-0000-0000-000000000005'::uuid, 'W8_BEN_E', 'Form W-8BEN-E', 'TAX', 'us_withholding',
 'US tax form for foreign entities.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED"], "jurisdictions": ["US_REPORTING"]}'::jsonb),

('d0070000-0000-0000-0000-000000000006'::uuid, 'W9', 'Form W-9', 'TAX', 'us_withholding',
 'US tax form for US persons.',
 '{"entity_types": ["ALL"], "jurisdictions": ["US"], "proves": ["us_person_status", "tin"]}'::jsonb),

('d0070000-0000-0000-0000-000000000007'::uuid, 'FATCA_SELF_CERT', 'FATCA Self-Certification', 'TAX', 'fatca',
 'Self-certification of FATCA status.',
 '{"entity_types": ["ALL"], "regulatory": "FATCA", "proves": ["fatca_status", "us_indicia"]}'::jsonb),

('d0070000-0000-0000-0000-000000000008'::uuid, 'CRS_SELF_CERT', 'CRS Self-Certification', 'TAX', 'crs',
 'Self-certification of tax residence for CRS reporting.',
 '{"entity_types": ["ALL"], "regulatory": "CRS", "proves": ["tax_residence", "tin"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDRESS VERIFICATION DOCUMENTS
-- -----------------------------------------------------------------------------
('d0080000-0000-0000-0000-000000000001'::uuid, 'UTILITY_BILL', 'Utility Bill', 'ADDRESS', 'residence',
 'Recent utility bill (gas, electric, water, phone) for address verification.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"], "proves": ["address"], "max_age_months": 3}'::jsonb),

('d0080000-0000-0000-0000-000000000002'::uuid, 'COUNCIL_TAX', 'Council Tax Bill', 'ADDRESS', 'residence',
 'Local government tax bill (UK specific).',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["UK"], "proves": ["address"], "max_age_months": 12}'::jsonb),

('d0080000-0000-0000-0000-000000000003'::uuid, 'LEASE_AGREEMENT', 'Lease / Rental Agreement', 'ADDRESS', 'residence',
 'Property rental agreement.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"], "proves": ["address", "tenancy"]}'::jsonb),

('d0080000-0000-0000-0000-000000000004'::uuid, 'PROPERTY_DEED', 'Property Title Deed', 'ADDRESS', 'ownership',
 'Legal document proving property ownership.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"], "proves": ["address", "property_ownership"]}'::jsonb),

('d0080000-0000-0000-0000-000000000005'::uuid, 'MORTGAGE_STATEMENT', 'Mortgage Statement', 'ADDRESS', 'ownership',
 'Statement from mortgage lender.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["address", "property_ownership"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- REGULATORY / COMPLIANCE DOCUMENTS
-- -----------------------------------------------------------------------------
('d0090000-0000-0000-0000-000000000001'::uuid, 'REGULATORY_LICENSE', 'Regulatory License', 'REGULATORY', 'license',
 'License from financial regulator (FCA, SEC, etc.).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["regulatory_status", "permitted_activities"]}'::jsonb),

('d0090000-0000-0000-0000-000000000002'::uuid, 'AML_POLICY', 'AML/KYC Policy Document', 'REGULATORY', 'policy',
 'Entity''s anti-money laundering policies and procedures.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["aml_framework"]}'::jsonb),

('d0090000-0000-0000-0000-000000000003'::uuid, 'LEI_CERTIFICATE', 'LEI Certificate', 'REGULATORY', 'identifier',
 'Legal Entity Identifier certificate.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["lei"]}'::jsonb),

('d0090000-0000-0000-0000-000000000004'::uuid, 'POWER_OF_ATTORNEY', 'Power of Attorney', 'REGULATORY', 'authorization',
 'Legal document granting authority to act on behalf of another.',
 '{"entity_types": ["ALL"], "proves": ["delegated_authority"]}'::jsonb),

('d0090000-0000-0000-0000-000000000005'::uuid, 'CORP_AUTH_LETTER', 'Corporate Authorization Letter', 'REGULATORY', 'authorization',
 'Letter authorizing individuals to act on behalf of company.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["authorized_representatives"]}'::jsonb),

('d0090000-0000-0000-0000-000000000006'::uuid, 'SANCTIONS_CERT', 'Sanctions Compliance Certificate', 'REGULATORY', 'compliance',
 'Certification of sanctions compliance.',
 '{"entity_types": ["ALL"], "proves": ["sanctions_compliance"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- UBO / OWNERSHIP DOCUMENTS
-- -----------------------------------------------------------------------------
('d0100000-0000-0000-0000-000000000001'::uuid, 'UBO_DECLARATION', 'UBO Declaration Form', 'UBO', 'declaration',
 'Self-declaration of ultimate beneficial ownership.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PARTNERSHIP_LIMITED", "TRUST_DISCRETIONARY"], "proves": ["ubo_structure"]}'::jsonb),

('d0100000-0000-0000-0000-000000000002'::uuid, 'OWNERSHIP_CHART', 'Ownership Structure Chart', 'UBO', 'structure',
 'Diagram showing ownership chain up to UBOs.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PARTNERSHIP_LIMITED"], "proves": ["ownership_chain"]}'::jsonb),

('d0100000-0000-0000-0000-000000000003'::uuid, 'SHARE_TRANSFER', 'Share Transfer Agreement', 'UBO', 'transfer',
 'Agreement transferring share ownership.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "proves": ["ownership_transfer"]}'::jsonb),

('d0100000-0000-0000-0000-000000000004'::uuid, 'VOTING_AGREEMENT', 'Voting Agreement', 'UBO', 'control',
 'Agreement regarding voting rights.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "proves": ["voting_control"]}'::jsonb),

('d0100000-0000-0000-0000-000000000005'::uuid, 'NOMINEE_AGREEMENT', 'Nominee Agreement', 'UBO', 'nominee',
 'Agreement between nominee and beneficial owner.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "proves": ["nominee_arrangement", "true_owner"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ISDA / DERIVATIVES DOCUMENTS
-- -----------------------------------------------------------------------------
('d0110000-0000-0000-0000-000000000001'::uuid, 'ISDA_MASTER', 'ISDA Master Agreement', 'ISDA', 'master',
 'Master agreement for OTC derivatives.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["derivatives_authority"]}'::jsonb),

('d0110000-0000-0000-0000-000000000002'::uuid, 'ISDA_SCHEDULE', 'ISDA Schedule', 'ISDA', 'schedule',
 'Customized terms supplementing ISDA Master.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["isda_terms"]}'::jsonb),

('d0110000-0000-0000-0000-000000000003'::uuid, 'CSA', 'Credit Support Annex (CSA)', 'ISDA', 'collateral',
 'Collateral arrangement for derivatives.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["collateral_terms"]}'::jsonb),

('d0110000-0000-0000-0000-000000000004'::uuid, 'ISDA_PROTOCOL_ADHERENCE', 'ISDA Protocol Adherence Letter', 'ISDA', 'protocol',
 'Letter evidencing adherence to ISDA protocols.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["protocol_adherence"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- EMPLOYMENT / PROFESSIONAL DOCUMENTS
-- -----------------------------------------------------------------------------
('d0120000-0000-0000-0000-000000000001'::uuid, 'EMPLOYMENT_CONTRACT', 'Employment Contract', 'EMPLOYMENT', 'contract',
 'Contract of employment.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["employment", "employer", "salary"]}'::jsonb),

('d0120000-0000-0000-0000-000000000002'::uuid, 'PAY_SLIP', 'Pay Slip / Salary Statement', 'EMPLOYMENT', 'income',
 'Monthly salary payment slip.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["income", "employer"]}'::jsonb),

('d0120000-0000-0000-0000-000000000003'::uuid, 'EMPLOYMENT_LETTER', 'Employment Confirmation Letter', 'EMPLOYMENT', 'confirmation',
 'Letter from employer confirming employment.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["employment", "position", "salary"]}'::jsonb),

('d0120000-0000-0000-0000-000000000004'::uuid, 'PROFESSIONAL_LICENSE', 'Professional License', 'EMPLOYMENT', 'qualification',
 'License to practice a profession (lawyer, accountant, doctor).',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["professional_status"]}'::jsonb),

('d0120000-0000-0000-0000-000000000005'::uuid, 'CV_RESUME', 'CV / Resume', 'EMPLOYMENT', 'background',
 'Curriculum vitae or resume.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["career_history"]}'::jsonb)

ON CONFLICT (type_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    category = EXCLUDED.category,
    domain = EXCLUDED.domain,
    description = EXCLUDED.description,
    applicability = EXCLUDED.applicability;

-- ============================================================================
-- PART 2: ATTRIBUTE REGISTRY  
-- Comprehensive attributes that can be observed/extracted
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (uuid, id, display_name, category, value_type, validation_rules, applicability)
VALUES

-- -----------------------------------------------------------------------------
-- IDENTITY ATTRIBUTES (Personal)
-- -----------------------------------------------------------------------------
('a0010000-0000-0000-0000-000000000001'::uuid, 'attr.identity.full_name', 'Full Legal Name', 'identity', 'string',
 '{"required": true, "min_length": 2, "max_length": 200}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "NATIONAL_ID", "DRIVERS_LICENSE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000002'::uuid, 'attr.identity.given_name', 'Given Name(s)', 'identity', 'string',
 '{"required": true, "min_length": 1, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "NATIONAL_ID"]}'::jsonb),

('a0010000-0000-0000-0000-000000000003'::uuid, 'attr.identity.family_name', 'Family Name / Surname', 'identity', 'string',
 '{"required": true, "min_length": 1, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "NATIONAL_ID"]}'::jsonb),

('a0010000-0000-0000-0000-000000000004'::uuid, 'attr.identity.middle_names', 'Middle Name(s)', 'identity', 'string',
 '{"required": false, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000005'::uuid, 'attr.identity.date_of_birth', 'Date of Birth', 'identity', 'date',
 '{"required": true, "min_age_years": 18}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "NATIONAL_ID", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000006'::uuid, 'attr.identity.place_of_birth', 'Place of Birth', 'identity', 'string',
 '{"required": false, "max_length": 200}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000007'::uuid, 'attr.identity.gender', 'Gender', 'identity', 'string',
 '{"required": false, "allowed_values": ["M", "F", "X", "MALE", "FEMALE", "OTHER"]}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "NATIONAL_ID"]}'::jsonb),

('a0010000-0000-0000-0000-000000000008'::uuid, 'attr.identity.nationality', 'Nationality', 'identity', 'string',
 '{"required": true, "pattern": "^[A-Z]{2,3}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT"]}'::jsonb),

('a0010000-0000-0000-0000-000000000009'::uuid, 'attr.identity.citizenship', 'Citizenship(s)', 'identity', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "BIRTH_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000010'::uuid, 'attr.identity.photo', 'Photograph', 'identity', 'string',
 '{"required": false, "format": "base64_image"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "NATIONAL_ID", "DRIVERS_LICENSE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000011'::uuid, 'attr.identity.signature', 'Signature', 'identity', 'string',
 '{"required": false, "format": "base64_image"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT", "SPECIMEN_SIGNATURES"]}'::jsonb),

('a0010000-0000-0000-0000-000000000012'::uuid, 'attr.identity.maiden_name', 'Maiden Name', 'identity', 'string',
 '{"required": false, "max_length": 100}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["MARRIAGE_CERTIFICATE"]}'::jsonb),

('a0010000-0000-0000-0000-000000000013'::uuid, 'attr.identity.former_names', 'Former Names', 'identity', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["DEED_POLL"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- DOCUMENT ID ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0020000-0000-0000-0000-000000000001'::uuid, 'attr.document.passport_number', 'Passport Number', 'document', 'string',
 '{"required": true, "pattern": "^[A-Z0-9]{6,12}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT"]}'::jsonb),

('a0020000-0000-0000-0000-000000000002'::uuid, 'attr.document.national_id_number', 'National ID Number', 'document', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["NATIONAL_ID"]}'::jsonb),

('a0020000-0000-0000-0000-000000000003'::uuid, 'attr.document.drivers_license_number', 'Driver''s License Number', 'document', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["DRIVERS_LICENSE"]}'::jsonb),

('a0020000-0000-0000-0000-000000000004'::uuid, 'attr.document.issue_date', 'Document Issue Date', 'document', 'date',
 '{"required": true}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["PASSPORT", "NATIONAL_ID", "DRIVERS_LICENSE"]}'::jsonb),

('a0020000-0000-0000-0000-000000000005'::uuid, 'attr.document.expiry_date', 'Document Expiry Date', 'document', 'date',
 '{"required": true, "must_be_future": true}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["PASSPORT", "NATIONAL_ID", "DRIVERS_LICENSE"]}'::jsonb),

('a0020000-0000-0000-0000-000000000006'::uuid, 'attr.document.issuing_authority', 'Issuing Authority', 'document', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["PASSPORT", "NATIONAL_ID"]}'::jsonb),

('a0020000-0000-0000-0000-000000000007'::uuid, 'attr.document.issuing_country', 'Issuing Country', 'document', 'string',
 '{"required": true, "pattern": "^[A-Z]{2,3}$"}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["PASSPORT", "NATIONAL_ID"]}'::jsonb),

('a0020000-0000-0000-0000-000000000008'::uuid, 'attr.document.mrz_line_1', 'MRZ Line 1', 'document', 'string',
 '{"required": false, "pattern": "^[A-Z0-9<]{44}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT"]}'::jsonb),

('a0020000-0000-0000-0000-000000000009'::uuid, 'attr.document.mrz_line_2', 'MRZ Line 2', 'document', 'string',
 '{"required": false, "pattern": "^[A-Z0-9<]{44}$"}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PASSPORT"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDRESS ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0030000-0000-0000-0000-000000000001'::uuid, 'attr.address.residential_full', 'Full Residential Address', 'address', 'string',
 '{"required": true, "max_length": 500}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT", "DRIVERS_LICENSE"]}'::jsonb),

('a0030000-0000-0000-0000-000000000002'::uuid, 'attr.address.street_line_1', 'Street Address Line 1', 'address', 'string',
 '{"required": true, "max_length": 200}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT"]}'::jsonb),

('a0030000-0000-0000-0000-000000000003'::uuid, 'attr.address.street_line_2', 'Street Address Line 2', 'address', 'string',
 '{"required": false, "max_length": 200}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT"]}'::jsonb),

('a0030000-0000-0000-0000-000000000004'::uuid, 'attr.address.city', 'City', 'address', 'string',
 '{"required": true, "max_length": 100}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT"]}'::jsonb),

('a0030000-0000-0000-0000-000000000005'::uuid, 'attr.address.state_province', 'State / Province', 'address', 'string',
 '{"required": false, "max_length": 100}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT"]}'::jsonb),

('a0030000-0000-0000-0000-000000000006'::uuid, 'attr.address.postal_code', 'Postal Code', 'address', 'string',
 '{"required": true, "max_length": 20}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT"]}'::jsonb),

('a0030000-0000-0000-0000-000000000007'::uuid, 'attr.address.country', 'Country', 'address', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}$"}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UTILITY_BILL", "BANK_STATEMENT"]}'::jsonb),

('a0030000-0000-0000-0000-000000000008'::uuid, 'attr.address.registered_office', 'Registered Office Address', 'address', 'string',
 '{"required": true, "max_length": 500}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["CERT_OF_INCORPORATION", "ANNUAL_RETURN"]}'::jsonb),

('a0030000-0000-0000-0000-000000000009'::uuid, 'attr.address.trading_address', 'Trading / Business Address', 'address', 'string',
 '{"required": false, "max_length": 500}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["BANK_STATEMENT"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ENTITY / CORPORATE ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0040000-0000-0000-0000-000000000001'::uuid, 'attr.entity.legal_name', 'Legal Entity Name', 'entity', 'string',
 '{"required": true, "max_length": 300}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED"], "source_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000002'::uuid, 'attr.entity.trading_name', 'Trading Name / DBA', 'entity', 'string',
 '{"required": false, "max_length": 300}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["BANK_STATEMENT"]}'::jsonb),

('a0040000-0000-0000-0000-000000000003'::uuid, 'attr.entity.registration_number', 'Registration Number', 'entity', 'string',
 '{"required": true, "max_length": 50}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000004'::uuid, 'attr.entity.incorporation_date', 'Incorporation Date', 'entity', 'date',
 '{"required": true}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000005'::uuid, 'attr.entity.jurisdiction', 'Jurisdiction of Formation', 'entity', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED"], "source_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000006'::uuid, 'attr.entity.legal_form', 'Legal Form', 'entity', 'string',
 '{"required": true, "allowed_values": ["LIMITED", "LTD", "PLC", "LLC", "LP", "LLP", "CORP", "INC", "SA", "GMBH", "BV", "NV"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["CERT_OF_INCORPORATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000007'::uuid, 'attr.entity.authorized_capital', 'Authorized Share Capital', 'entity', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["ARTICLES_OF_ASSOCIATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000008'::uuid, 'attr.entity.issued_capital', 'Issued Share Capital', 'entity', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["ANNUAL_RETURN"]}'::jsonb),

('a0040000-0000-0000-0000-000000000009'::uuid, 'attr.entity.business_activity', 'Principal Business Activity', 'entity', 'string',
 '{"required": true, "max_length": 500}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["ARTICLES_OF_ASSOCIATION"]}'::jsonb),

('a0040000-0000-0000-0000-000000000010'::uuid, 'attr.entity.sic_code', 'SIC / NACE Code', 'entity', 'string',
 '{"required": false, "pattern": "^[0-9]{4,5}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["ANNUAL_RETURN"]}'::jsonb),

('a0040000-0000-0000-0000-000000000011'::uuid, 'attr.entity.directors', 'Directors', 'entity', 'json',
 '{"required": true, "type": "array", "min_items": 1}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["REGISTER_OF_DIRECTORS", "CERT_OF_INCUMBENCY"]}'::jsonb),

('a0040000-0000-0000-0000-000000000012'::uuid, 'attr.entity.company_secretary', 'Company Secretary', 'entity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["REGISTER_OF_DIRECTORS"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- TAX ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0050000-0000-0000-0000-000000000001'::uuid, 'attr.tax.tin', 'Tax Identification Number', 'tax', 'string',
 '{"required": true, "max_length": 50}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["TAX_RETURN_PERSONAL", "TAX_RETURN_CORPORATE", "W9"]}'::jsonb),

('a0050000-0000-0000-0000-000000000002'::uuid, 'attr.tax.vat_number', 'VAT / GST Number', 'tax', 'string',
 '{"required": false, "pattern": "^[A-Z]{2}[A-Z0-9]{8,12}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["TAX_RETURN_CORPORATE"]}'::jsonb),

('a0050000-0000-0000-0000-000000000003'::uuid, 'attr.tax.tax_residence', 'Tax Residence Country', 'tax', 'string',
 '{"required": true, "pattern": "^[A-Z]{2}$"}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["TAX_RESIDENCY_CERT", "CRS_SELF_CERT"]}'::jsonb),

('a0050000-0000-0000-0000-000000000004'::uuid, 'attr.tax.giin', 'GIIN (FATCA)', 'tax', 'string',
 '{"required": false, "pattern": "^[A-Z0-9]{6}\\.[A-Z0-9]{5}\\.[A-Z]{2}\\.[0-9]{3}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["FATCA_SELF_CERT"]}'::jsonb),

('a0050000-0000-0000-0000-000000000005'::uuid, 'attr.tax.fatca_status', 'FATCA Status', 'tax', 'string',
 '{"required": true, "allowed_values": ["US_PERSON", "NON_US_PERSON", "PFFI", "NPFFI", "DEEMED_COMPLIANT", "EXEMPT"]}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["FATCA_SELF_CERT", "W8_BEN", "W8_BEN_E", "W9"]}'::jsonb),

('a0050000-0000-0000-0000-000000000006'::uuid, 'attr.tax.crs_status', 'CRS Entity Type', 'tax', 'string',
 '{"required": true, "allowed_values": ["ACTIVE_NFE", "PASSIVE_NFE", "FI", "GOVERNMENT", "INTERNATIONAL_ORG"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["CRS_SELF_CERT"]}'::jsonb),

('a0050000-0000-0000-0000-000000000007'::uuid, 'attr.tax.us_person', 'US Person Status', 'tax', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["W9", "W8_BEN", "W8_BEN_E"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- FINANCIAL ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0060000-0000-0000-0000-000000000001'::uuid, 'attr.financial.bank_account_number', 'Bank Account Number', 'financial', 'string',
 '{"required": false, "max_length": 34}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["BANK_STATEMENT"]}'::jsonb),

('a0060000-0000-0000-0000-000000000002'::uuid, 'attr.financial.iban', 'IBAN', 'financial', 'string',
 '{"required": false, "pattern": "^[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}$"}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["BANK_STATEMENT"]}'::jsonb),

('a0060000-0000-0000-0000-000000000003'::uuid, 'attr.financial.bic_swift', 'BIC / SWIFT Code', 'financial', 'string',
 '{"required": false, "pattern": "^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$"}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["BANK_STATEMENT"]}'::jsonb),

('a0060000-0000-0000-0000-000000000004'::uuid, 'attr.financial.bank_name', 'Bank Name', 'financial', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["BANK_STATEMENT", "BANK_REFERENCE"]}'::jsonb),

('a0060000-0000-0000-0000-000000000005'::uuid, 'attr.financial.account_balance', 'Account Balance', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["BANK_STATEMENT"]}'::jsonb),

('a0060000-0000-0000-0000-000000000006'::uuid, 'attr.financial.annual_income', 'Annual Income', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["TAX_RETURN_PERSONAL", "EMPLOYMENT_LETTER"]}'::jsonb),

('a0060000-0000-0000-0000-000000000007'::uuid, 'attr.financial.net_worth', 'Net Worth', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["NET_WORTH_STATEMENT"]}'::jsonb),

('a0060000-0000-0000-0000-000000000008'::uuid, 'attr.financial.source_of_wealth', 'Source of Wealth', 'financial', 'string',
 '{"required": true, "max_length": 1000}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["SOURCE_OF_WEALTH"]}'::jsonb),

('a0060000-0000-0000-0000-000000000009'::uuid, 'attr.financial.source_of_funds', 'Source of Funds', 'financial', 'string',
 '{"required": true, "max_length": 1000}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["SOURCE_OF_FUNDS"]}'::jsonb),

('a0060000-0000-0000-0000-000000000010'::uuid, 'attr.financial.revenue', 'Annual Revenue', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["AUDITED_ACCOUNTS"]}'::jsonb),

('a0060000-0000-0000-0000-000000000011'::uuid, 'attr.financial.total_assets', 'Total Assets', 'financial', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "source_documents": ["AUDITED_ACCOUNTS"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- UBO / OWNERSHIP ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0070000-0000-0000-0000-000000000001'::uuid, 'attr.ubo.beneficial_owner_name', 'Beneficial Owner Name', 'ubo', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["UBO_DECLARATION", "PASSPORT"]}'::jsonb),

('a0070000-0000-0000-0000-000000000002'::uuid, 'attr.ubo.ownership_percentage', 'Ownership Percentage', 'ubo', 'percentage',
 '{"required": true, "min_value": 0, "max_value": 100}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UBO_DECLARATION", "REGISTER_OF_SHAREHOLDERS", "OWNERSHIP_CHART"]}'::jsonb),

('a0070000-0000-0000-0000-000000000003'::uuid, 'attr.ubo.control_type', 'Type of Control', 'ubo', 'string',
 '{"required": false, "allowed_values": ["OWNERSHIP", "VOTING", "BOARD", "OTHER"]}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["UBO_DECLARATION", "VOTING_AGREEMENT"]}'::jsonb),

('a0070000-0000-0000-0000-000000000004'::uuid, 'attr.ubo.ownership_chain', 'Ownership Chain', 'ubo', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "source_documents": ["OWNERSHIP_CHART"]}'::jsonb),

('a0070000-0000-0000-0000-000000000005'::uuid, 'attr.ubo.is_nominee', 'Is Nominee', 'ubo', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["NOMINEE_AGREEMENT"]}'::jsonb),

('a0070000-0000-0000-0000-000000000006'::uuid, 'attr.ubo.nominator', 'Nominator / True Owner', 'ubo', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": ["NOMINEE_AGREEMENT"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- TRUST ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0080000-0000-0000-0000-000000000001'::uuid, 'attr.trust.trust_name', 'Trust Name', 'entity', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "source_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000002'::uuid, 'attr.trust.trust_type', 'Trust Type', 'entity', 'string',
 '{"required": true, "allowed_values": ["DISCRETIONARY", "FIXED", "UNIT", "CHARITABLE", "PURPOSE"]}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED", "TRUST_UNIT"], "source_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000003'::uuid, 'attr.trust.establishment_date', 'Trust Establishment Date', 'entity', 'date',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "source_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000004'::uuid, 'attr.trust.governing_law', 'Governing Law', 'entity', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "source_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000005'::uuid, 'attr.trust.settlor', 'Settlor', 'entity', 'string',
 '{"required": true}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "source_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000006'::uuid, 'attr.trust.trustees', 'Trustees', 'entity', 'json',
 '{"required": true, "type": "array", "min_items": 1}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "source_documents": ["TRUST_DEED", "DEED_OF_APPOINTMENT"]}'::jsonb),

('a0080000-0000-0000-0000-000000000007'::uuid, 'attr.trust.protector', 'Protector', 'entity', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY"], "source_documents": ["TRUST_DEED"]}'::jsonb),

('a0080000-0000-0000-0000-000000000008'::uuid, 'attr.trust.beneficiaries', 'Beneficiaries', 'entity', 'json',
 '{"required": true, "type": "array"}'::jsonb,
 '{"entity_types": ["TRUST_DISCRETIONARY", "TRUST_FIXED"], "source_documents": ["TRUST_DEED", "SCHEDULE_OF_BENEFICIARIES"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- REGULATORY ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0090000-0000-0000-0000-000000000001'::uuid, 'attr.regulatory.lei', 'Legal Entity Identifier', 'compliance', 'string',
 '{"required": false, "pattern": "^[A-Z0-9]{20}$"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "source_documents": ["LEI_CERTIFICATE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000002'::uuid, 'attr.regulatory.license_number', 'Regulatory License Number', 'compliance', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "source_documents": ["REGULATORY_LICENSE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000003'::uuid, 'attr.regulatory.regulator', 'Regulator', 'compliance', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "source_documents": ["REGULATORY_LICENSE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000004'::uuid, 'attr.regulatory.permitted_activities', 'Permitted Activities', 'compliance', 'json',
 '{"required": false, "type": "array"}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "source_documents": ["REGULATORY_LICENSE"]}'::jsonb),

('a0090000-0000-0000-0000-000000000005'::uuid, 'attr.regulatory.pep_status', 'PEP Status', 'risk', 'boolean',
 '{"required": true}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": []}'::jsonb),

('a0090000-0000-0000-0000-000000000006'::uuid, 'attr.regulatory.sanctions_status', 'Sanctions Status', 'risk', 'string',
 '{"required": true, "allowed_values": ["CLEAR", "POTENTIAL_MATCH", "CONFIRMED_MATCH"]}'::jsonb,
 '{"entity_types": ["ALL"], "source_documents": []}'::jsonb),

-- -----------------------------------------------------------------------------
-- EMPLOYMENT ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0100000-0000-0000-0000-000000000001'::uuid, 'attr.employment.employer_name', 'Employer Name', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["EMPLOYMENT_CONTRACT", "EMPLOYMENT_LETTER", "PAY_SLIP"]}'::jsonb),

('a0100000-0000-0000-0000-000000000002'::uuid, 'attr.employment.job_title', 'Job Title / Position', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["EMPLOYMENT_CONTRACT", "EMPLOYMENT_LETTER"]}'::jsonb),

('a0100000-0000-0000-0000-000000000003'::uuid, 'attr.employment.employment_start_date', 'Employment Start Date', 'employment', 'date',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["EMPLOYMENT_CONTRACT"]}'::jsonb),

('a0100000-0000-0000-0000-000000000004'::uuid, 'attr.employment.salary', 'Salary', 'employment', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["EMPLOYMENT_CONTRACT", "PAY_SLIP"]}'::jsonb),

('a0100000-0000-0000-0000-000000000005'::uuid, 'attr.employment.profession', 'Profession', 'employment', 'string',
 '{"required": false}'::jsonb,
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "source_documents": ["PROFESSIONAL_LICENSE", "CV_RESUME"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ISDA ATTRIBUTES
-- -----------------------------------------------------------------------------
('a0110000-0000-0000-0000-000000000001'::uuid, 'attr.isda.master_date', 'ISDA Master Agreement Date', 'isda', 'date',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["ISDA_MASTER"]}'::jsonb),

('a0110000-0000-0000-0000-000000000002'::uuid, 'attr.isda.governing_law', 'ISDA Governing Law', 'isda', 'string',
 '{"required": false, "allowed_values": ["NY", "ENGLISH"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["ISDA_MASTER"]}'::jsonb),

('a0110000-0000-0000-0000-000000000003'::uuid, 'attr.isda.csa_type', 'CSA Type', 'isda', 'string',
 '{"required": false, "allowed_values": ["VM", "IM", "BOTH"]}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["CSA"]}'::jsonb),

('a0110000-0000-0000-0000-000000000004'::uuid, 'attr.isda.threshold_amount', 'CSA Threshold Amount', 'isda', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["CSA"]}'::jsonb),

('a0110000-0000-0000-0000-000000000005'::uuid, 'attr.isda.mta', 'Minimum Transfer Amount', 'isda', 'currency',
 '{"required": false}'::jsonb,
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "source_documents": ["CSA"]}'::jsonb)

ON CONFLICT (id) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    category = EXCLUDED.category,
    value_type = EXCLUDED.value_type,
    validation_rules = EXCLUDED.validation_rules,
    applicability = EXCLUDED.applicability;

-- ============================================================================
-- PART 3: DOCUMENT-ATTRIBUTE LINKS
-- SOURCE = Document provides this attribute (extraction)
-- SINK = Attribute requires this document as proof (fulfillment)
-- ============================================================================

-- Note: This uses the NEW document_attribute_links table from the observation model
-- If table doesn't exist yet, create it:

CREATE TABLE IF NOT EXISTS "ob-poc".document_attribute_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_id UUID NOT NULL,
    attribute_id UUID NOT NULL,
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

-- Clear and repopulate
DELETE FROM "ob-poc".document_attribute_links;

INSERT INTO "ob-poc".document_attribute_links 
(document_type_id, attribute_id, direction, extraction_method, extraction_confidence_default, is_authoritative, proof_strength)
VALUES

-- =============================================================================
-- PASSPORT → Attributes (SOURCE)
-- =============================================================================
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'MRZ', 0.95, TRUE, 'PRIMARY'),   -- full_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000002', 'SOURCE', 'MRZ', 0.95, TRUE, 'PRIMARY'),   -- given_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000003', 'SOURCE', 'MRZ', 0.95, TRUE, 'PRIMARY'),   -- family_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- date_of_birth
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),   -- place_of_birth
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000007', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- gender
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000008', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- nationality
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000010', 'SOURCE', 'IMAGE', 0.99, TRUE, 'PRIMARY'), -- photo
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000011', 'SOURCE', 'IMAGE', 0.90, TRUE, 'PRIMARY'), -- signature
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000001', 'SOURCE', 'MRZ', 0.99, TRUE, 'PRIMARY'),   -- passport_number
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.90, TRUE, NULL),        -- issue_date
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000005', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- expiry_date
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.85, FALSE, NULL),       -- issuing_authority
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000007', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- issuing_country
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000008', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- mrz_line_1
('d0010000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000009', 'SOURCE', 'MRZ', 0.99, TRUE, NULL),        -- mrz_line_2

-- PASSPORT SINK (what it proves)
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves full_name
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves dob
('d0010000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000008', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),      -- proves nationality

-- =============================================================================
-- NATIONAL ID → Attributes
-- =============================================================================
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000007', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0010000-0000-0000-0000-000000000010', 'SOURCE', 'IMAGE', 0.99, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000002', 'a0020000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),   -- national_id_number
('d0010000-0000-0000-0000-000000000002', 'a0020000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.90, TRUE, NULL),
('d0010000-0000-0000-0000-000000000002', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SECONDARY'), -- address (some IDs)

-- =============================================================================
-- DRIVER'S LICENSE → Attributes
-- =============================================================================
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.85, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.90, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0010000-0000-0000-0000-000000000010', 'SOURCE', 'IMAGE', 0.95, FALSE, 'SECONDARY'),
('d0010000-0000-0000-0000-000000000003', 'a0020000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),    -- license_number
('d0010000-0000-0000-0000-000000000003', 'a0020000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.85, FALSE, NULL),
('d0010000-0000-0000-0000-000000000003', 'a0020000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.90, FALSE, NULL),
('d0010000-0000-0000-0000-000000000003', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SECONDARY'), -- address

-- =============================================================================
-- BIRTH CERTIFICATE → Attributes
-- =============================================================================
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),
('d0010000-0000-0000-0000-000000000004', 'a0010000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),

-- =============================================================================
-- CERTIFICATE OF INCORPORATION → Attributes
-- =============================================================================
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),   -- legal_name
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),   -- registration_number
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),   -- incorporation_date
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),   -- jurisdiction
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),   -- legal_form
('d0020000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000008', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),   -- registered_office
('d0020000-0000-0000-0000-000000000001', 'a0020000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.90, FALSE, NULL),       -- issue_date

-- CERT OF INCORP as SINK (proves)
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000003', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0020000-0000-0000-0000-000000000001', 'a0040000-0000-0000-0000-000000000004', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- ARTICLES OF ASSOCIATION → Attributes
-- =============================================================================
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.85, FALSE, 'SECONDARY'),
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),    -- authorized_capital
('d0020000-0000-0000-0000-000000000002', 'a0040000-0000-0000-0000-000000000009', 'SOURCE', 'AI', 0.75, TRUE, 'PRIMARY'),    -- business_activity

-- =============================================================================
-- REGISTER OF DIRECTORS → Attributes
-- =============================================================================
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000011', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- directors
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000012', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- company_secretary
('d0020000-0000-0000-0000-000000000006', 'a0040000-0000-0000-0000-000000000011', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- REGISTER OF SHAREHOLDERS → Attributes
-- =============================================================================
('d0020000-0000-0000-0000-000000000007', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- ownership_percentage
('d0020000-0000-0000-0000-000000000007', 'a0070000-0000-0000-0000-000000000002', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- CERT OF GOOD STANDING → Attributes
-- =============================================================================
('d0020000-0000-0000-0000-000000000004', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),
('d0020000-0000-0000-0000-000000000004', 'a0040000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),

-- =============================================================================
-- CERT OF INCUMBENCY → Attributes
-- =============================================================================
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),
('d0020000-0000-0000-0000-000000000005', 'a0040000-0000-0000-0000-000000000011', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- directors
('d0020000-0000-0000-0000-000000000005', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- ownership

-- =============================================================================
-- TRUST DEED → Attributes
-- =============================================================================
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- trust_name
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- trust_type
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000003', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- establishment_date
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),    -- governing_law
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000005', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- settlor
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000006', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),    -- trustees
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.80, FALSE, 'PRIMARY'),   -- protector
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000008', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),    -- beneficiaries

-- TRUST DEED as SINK
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000005', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),
('d0040000-0000-0000-0000-000000000001', 'a0080000-0000-0000-0000-000000000006', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- =============================================================================
-- UTILITY BILL → Attributes (Address Verification)
-- =============================================================================
('d0080000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SUPPORTING'), -- name
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),     -- address
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000006', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000007', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),
('d0080000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),        -- proves address

-- =============================================================================
-- BANK STATEMENT → Attributes
-- =============================================================================
('d0060000-0000-0000-0000-000000000001', 'a0010000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.85, FALSE, 'SUPPORTING'),
('d0060000-0000-0000-0000-000000000001', 'a0030000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.80, FALSE, 'SECONDARY'),
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),     -- account_number
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),     -- iban
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),     -- bic
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),     -- bank_name
('d0060000-0000-0000-0000-000000000001', 'a0060000-0000-0000-0000-000000000005', 'SOURCE', 'OCR', 0.85, FALSE, 'PRIMARY'),    -- balance

-- =============================================================================
-- TAX DOCUMENTS → Attributes
-- =============================================================================
-- Tax Residency Certificate
('d0070000-0000-0000-0000-000000000003', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),    -- tin
('d0070000-0000-0000-0000-000000000003', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),    -- tax_residence
('d0070000-0000-0000-0000-000000000003', 'a0050000-0000-0000-0000-000000000003', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- W-8BEN
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000005', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'), -- fatca_status
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000007', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'), -- us_person (false)
('d0070000-0000-0000-0000-000000000004', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),

-- W-9
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'), -- tin (SSN/EIN)
('d0070000-0000-0000-0000-000000000006', 'a0050000-0000-0000-0000-000000000007', 'SOURCE', 'FORM_FIELD', 0.99, TRUE, 'PRIMARY'), -- us_person (true)

-- CRS Self-Cert
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'),
('d0070000-0000-0000-0000-000000000008', 'a0050000-0000-0000-0000-000000000006', 'SOURCE', 'FORM_FIELD', 0.95, TRUE, 'PRIMARY'), -- crs_status

-- =============================================================================
-- UBO DOCUMENTS → Attributes
-- =============================================================================
-- UBO Declaration
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000001', 'SOURCE', 'FORM_FIELD', 0.85, FALSE, 'SECONDARY'), -- ubo_name
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'FORM_FIELD', 0.85, FALSE, 'SECONDARY'), -- ownership_%
('d0100000-0000-0000-0000-000000000001', 'a0070000-0000-0000-0000-000000000003', 'SOURCE', 'FORM_FIELD', 0.80, FALSE, 'SECONDARY'), -- control_type

-- Ownership Chart
('d0100000-0000-0000-0000-000000000002', 'a0070000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),      -- ownership_chain
('d0100000-0000-0000-0000-000000000002', 'a0070000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),

-- =============================================================================
-- REGULATORY DOCUMENTS → Attributes
-- =============================================================================
-- LEI Certificate
('d0090000-0000-0000-0000-000000000003', 'a0090000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.99, TRUE, 'PRIMARY'),     -- lei
('d0090000-0000-0000-0000-000000000003', 'a0040000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.95, TRUE, 'SUPPORTING'),  -- legal_name
('d0090000-0000-0000-0000-000000000003', 'a0090000-0000-0000-0000-000000000001', 'SINK', NULL, NULL, TRUE, 'PRIMARY'),

-- Regulatory License
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000002', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),     -- license_number
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000003', 'SOURCE', 'OCR', 0.95, TRUE, 'PRIMARY'),     -- regulator
('d0090000-0000-0000-0000-000000000001', 'a0090000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.80, TRUE, 'PRIMARY'),      -- permitted_activities

-- =============================================================================
-- EMPLOYMENT DOCUMENTS → Attributes
-- =============================================================================
-- Employment Contract
('d0120000-0000-0000-0000-000000000001', 'a0100000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),      -- employer
('d0120000-0000-0000-0000-000000000001', 'a0100000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),      -- job_title
('d0120000-0000-0000-0000-000000000001', 'a0100000-0000-0000-0000-000000000003', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),      -- start_date
('d0120000-0000-0000-0000-000000000001', 'a0100000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),      -- salary

-- Pay Slip
('d0120000-0000-0000-0000-000000000002', 'a0100000-0000-0000-0000-000000000001', 'SOURCE', 'OCR', 0.90, FALSE, 'SUPPORTING'),
('d0120000-0000-0000-0000-000000000002', 'a0100000-0000-0000-0000-000000000004', 'SOURCE', 'OCR', 0.90, TRUE, 'PRIMARY'),
('d0120000-0000-0000-0000-000000000002', 'a0060000-0000-0000-0000-000000000006', 'SOURCE', 'OCR', 0.85, TRUE, 'PRIMARY'),     -- income

-- =============================================================================
-- ISDA DOCUMENTS → Attributes
-- =============================================================================
-- ISDA Master
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000001', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),      -- master_date
('d0110000-0000-0000-0000-000000000001', 'a0110000-0000-0000-0000-000000000002', 'SOURCE', 'AI', 0.95, TRUE, 'PRIMARY'),      -- governing_law

-- CSA
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000003', 'SOURCE', 'AI', 0.90, TRUE, 'PRIMARY'),      -- csa_type
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000004', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY'),      -- threshold
('d0110000-0000-0000-0000-000000000003', 'a0110000-0000-0000-0000-000000000005', 'SOURCE', 'AI', 0.85, TRUE, 'PRIMARY')       -- mta

ON CONFLICT (document_type_id, attribute_id, direction) DO UPDATE SET
    extraction_method = EXCLUDED.extraction_method,
    extraction_confidence_default = EXCLUDED.extraction_confidence_default,
    is_authoritative = EXCLUDED.is_authoritative,
    proof_strength = EXCLUDED.proof_strength;

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Document types by category
SELECT category, COUNT(*) as doc_count 
FROM "ob-poc".document_types 
GROUP BY category 
ORDER BY doc_count DESC;

-- Attributes by category
SELECT category, COUNT(*) as attr_count 
FROM "ob-poc".attribute_registry 
GROUP BY category 
ORDER BY attr_count DESC;

-- SOURCE links (what can be extracted from each document)
SELECT 
    dt.type_code,
    dt.display_name as document,
    COUNT(*) FILTER (WHERE dal.direction = 'SOURCE') as extracts_count,
    COUNT(*) FILTER (WHERE dal.direction = 'SINK') as proves_count
FROM "ob-poc".document_types dt
LEFT JOIN "ob-poc".document_attribute_links dal ON dt.type_id = dal.document_type_id
GROUP BY dt.type_id, dt.type_code, dt.display_name
ORDER BY extracts_count DESC;

-- What can prove identity (SINK links for identity attributes)
SELECT 
    dt.type_code,
    dt.display_name as document,
    ar.id as attribute,
    dal.is_authoritative,
    dal.proof_strength
FROM "ob-poc".document_attribute_links dal
JOIN "ob-poc".document_types dt ON dal.document_type_id = dt.type_id
JOIN "ob-poc".attribute_registry ar ON dal.attribute_id = ar.uuid
WHERE ar.category = 'identity' AND dal.direction IN ('SINK', 'BOTH')
ORDER BY ar.id, dal.proof_strength;
