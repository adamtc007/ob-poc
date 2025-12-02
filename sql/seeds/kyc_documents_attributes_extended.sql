-- ============================================================================
-- EXTENDED KYC DOCUMENT TYPES AND ATTRIBUTES
-- Supplements kyc_documents_attributes_complete.sql with additional documents,
-- jurisdiction-specific forms, validity rules, and contract documents
-- ============================================================================

BEGIN;

-- ============================================================================
-- ADDITIONAL DOCUMENT TYPES
-- ============================================================================

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, applicability)
VALUES

-- -----------------------------------------------------------------------------
-- ADDITIONAL IDENTITY DOCUMENTS
-- -----------------------------------------------------------------------------
('d0010000-0000-0000-0000-000000000101'::uuid, 'ARMED_FORCES_ID', 'Armed Forces Identity Card', 'IDENTITY', 'personal',
 'Military identity card. Accepted as primary ID in many jurisdictions.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_primary_id": true, "validity_rules": {"check_expiry": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000102'::uuid, 'REFUGEE_TRAVEL_DOC', 'Refugee Travel Document', 'IDENTITY', 'personal',
 'Travel document issued to refugees per 1951 Convention.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "is_primary_id": true, "validity_rules": {"check_expiry": true, "min_remaining_validity_months": 6}}'::jsonb),

('d0010000-0000-0000-0000-000000000103'::uuid, 'PERMANENT_RESIDENT_CARD', 'Permanent Resident Card', 'IDENTITY', 'immigration',
 'Permanent residence permit (US Green Card, UK BRP, EU Long-Term Residence).',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["identity", "right_to_reside"], "validity_rules": {"check_expiry": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000104'::uuid, 'SOCIAL_SECURITY_CARD', 'Social Security Card', 'IDENTITY', 'government',
 'US Social Security card or equivalent national insurance document.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["US"], "proves": ["ssn"], "is_secondary_id": true}'::jsonb),

('d0010000-0000-0000-0000-000000000105'::uuid, 'STATE_ID', 'State/Provincial ID Card', 'IDENTITY', 'personal',
 'State or provincial government-issued ID (not driver license).',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["US", "CA"], "is_primary_id": true}'::jsonb),

('d0010000-0000-0000-0000-000000000106'::uuid, 'WORK_PERMIT', 'Work Permit / Employment Authorization', 'IDENTITY', 'immigration',
 'Authorization to work in a jurisdiction (EAD, work visa).',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "proves": ["right_to_work"], "validity_rules": {"check_expiry": true}}'::jsonb),

('d0010000-0000-0000-0000-000000000107'::uuid, 'ELECTORAL_ROLL', 'Electoral Roll Confirmation', 'IDENTITY', 'government',
 'Confirmation of registration on electoral roll. Address verification.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["UK"], "proves": ["address", "citizenship"]}'::jsonb),

('d0010000-0000-0000-0000-000000000108'::uuid, 'EU_SETTLEMENT_STATUS', 'EU Settlement Scheme Status', 'IDENTITY', 'immigration',
 'UK EU Settlement Scheme share code / status letter.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["UK"], "proves": ["right_to_reside", "settled_status"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- UK-SPECIFIC COMPANIES HOUSE FORMS
-- -----------------------------------------------------------------------------
('d0020000-0000-0000-0000-000000000101'::uuid, 'CH_AP01', 'Companies House Form AP01', 'CORPORATE', 'filing',
 'Appointment of director (UK Companies House).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["UK"], "proves": ["director_appointment"]}'::jsonb),

('d0020000-0000-0000-0000-000000000102'::uuid, 'CH_TM01', 'Companies House Form TM01', 'CORPORATE', 'filing',
 'Termination of appointment of director (UK Companies House).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["UK"], "proves": ["director_resignation"]}'::jsonb),

('d0020000-0000-0000-0000-000000000103'::uuid, 'CH_CS01', 'Companies House Form CS01', 'CORPORATE', 'filing',
 'Confirmation Statement (UK Annual Return replacement).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["UK"], "filing_frequency": "annual"}'::jsonb),

('d0020000-0000-0000-0000-000000000104'::uuid, 'CH_SH01', 'Companies House Form SH01', 'CORPORATE', 'filing',
 'Return of allotment of shares.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["UK"], "proves": ["share_allotment"]}'::jsonb),

('d0020000-0000-0000-0000-000000000105'::uuid, 'CH_PSC01', 'Companies House Form PSC01', 'CORPORATE', 'filing',
 'Notification of person with significant control (PSC).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["UK"], "proves": ["psc_notification"]}'::jsonb),

('d0020000-0000-0000-0000-000000000106'::uuid, 'COMPANY_REGISTRY_EXTRACT', 'Company Registry Extract', 'CORPORATE', 'status',
 'Official extract from company registry (Companies House, Handelsregister, etc.).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "validity_rules": {"max_age_days": 30}}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDITIONAL CORPORATE / LEGAL DOCUMENTS
-- -----------------------------------------------------------------------------
('d0020000-0000-0000-0000-000000000107'::uuid, 'CERT_OF_NAME_CHANGE', 'Certificate of Name Change', 'CORPORATE', 'status',
 'Certificate evidencing corporate name change from registrar.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["name_change", "former_names"]}'::jsonb),

('d0020000-0000-0000-0000-000000000108'::uuid, 'REGISTER_OF_CHARGES', 'Register of Charges / Mortgages', 'CORPORATE', 'encumbrance',
 'Register of security interests, charges, and mortgages over company assets.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["encumbrances", "secured_creditors"]}'::jsonb),

('d0020000-0000-0000-0000-000000000109'::uuid, 'SHAREHOLDERS_AGREEMENT', 'Shareholders Agreement', 'CORPORATE', 'governance',
 'Agreement between shareholders governing their relationship.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "proves": ["shareholder_rights", "drag_along", "tag_along", "pre_emption"]}'::jsonb),

('d0020000-0000-0000-0000-000000000110'::uuid, 'DIRECTOR_SERVICE_CONTRACT', 'Director Service Contract', 'CORPORATE', 'governance',
 'Employment/service contract with a director.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["director_terms", "remuneration"]}'::jsonb),

('d0020000-0000-0000-0000-000000000111'::uuid, 'REGISTERED_AGENT_CERT', 'Certificate of Registered Agent', 'CORPORATE', 'status',
 'Confirmation of registered agent/office appointment.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "jurisdictions": ["US", "KY", "BVI"]}'::jsonb),

('d0020000-0000-0000-0000-000000000112'::uuid, 'LEGAL_OPINION', 'Legal Opinion', 'CORPORATE', 'legal',
 'Formal legal opinion from qualified counsel on corporate matters.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["legal_capacity", "authorization"]}'::jsonb),

('d0020000-0000-0000-0000-000000000113'::uuid, 'SECRETARY_CERTIFICATE', 'Secretary''s Certificate', 'CORPORATE', 'governance',
 'Certificate from company secretary certifying corporate documents/resolutions.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["document_authenticity", "authorization"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDITIONAL FUND/INVESTMENT DOCUMENTS
-- -----------------------------------------------------------------------------
('d0050000-0000-0000-0000-000000000101'::uuid, 'INVESTOR_QUESTIONNAIRE', 'Investor Questionnaire / AML Form', 'FUND', 'investor',
 'Standard investor due diligence questionnaire for fund subscription.',
 '{"entity_types": ["ALL"], "proves": ["investor_profile", "aml_information"]}'::jsonb),

('d0050000-0000-0000-0000-000000000102'::uuid, 'CAPITAL_CALL_NOTICE', 'Capital Call Notice', 'FUND', 'operations',
 'Notice to investors requesting capital contribution.',
 '{"entity_types": ["FUND_PE", "FUND_VC", "PARTNERSHIP_LIMITED"], "proves": ["capital_commitment"]}'::jsonb),

('d0050000-0000-0000-0000-000000000103'::uuid, 'DISTRIBUTION_NOTICE', 'Distribution Notice', 'FUND', 'operations',
 'Notice to investors of distribution/dividend payment.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_VC"], "proves": ["distribution_entitlement"]}'::jsonb),

('d0050000-0000-0000-0000-000000000104'::uuid, 'REDEMPTION_NOTICE', 'Redemption Notice/Request', 'FUND', 'investor',
 'Investor request to redeem fund investment.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS"], "proves": ["redemption_request"]}'::jsonb),

('d0050000-0000-0000-0000-000000000105'::uuid, 'FUND_ADMIN_CERT', 'Fund Administrator Certificate', 'FUND', 'service',
 'Certificate from fund administrator confirming investor details/holdings.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "proves": ["investor_holding", "nav"]}'::jsonb),

('d0050000-0000-0000-0000-000000000106'::uuid, 'TRANSFER_AGENCY_LETTER', 'Transfer Agency Confirmation', 'FUND', 'service',
 'Letter from transfer agent confirming investor registration.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS"], "proves": ["registered_holder", "shareholding"]}'::jsonb),

('d0050000-0000-0000-0000-000000000107'::uuid, 'LIMITED_PARTNER_INTEREST', 'Limited Partner Interest Certificate', 'FUND', 'ownership',
 'Certificate evidencing limited partner interest in a fund.',
 '{"entity_types": ["PARTNERSHIP_LIMITED", "FUND_PE", "FUND_VC"], "proves": ["lp_interest", "commitment_amount"]}'::jsonb),

('d0050000-0000-0000-0000-000000000108'::uuid, 'FUND_FACTSHEET', 'Fund Factsheet / Monthly Report', 'FUND', 'disclosure',
 'Monthly or quarterly fund performance factsheet.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS", "FUND_PE"], "proves": ["performance", "nav", "aum"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDITIONAL FINANCIAL/BANKING DOCUMENTS
-- -----------------------------------------------------------------------------
('d0060000-0000-0000-0000-000000000101'::uuid, 'BROKER_STATEMENT', 'Brokerage Account Statement', 'FINANCIAL', 'investments',
 'Statement from securities broker showing holdings and transactions.',
 '{"entity_types": ["ALL"], "proves": ["investments", "securities_holdings"], "validity_rules": {"max_age_months": 3}}'::jsonb),

('d0060000-0000-0000-0000-000000000102'::uuid, 'CUSTODY_STATEMENT', 'Custody Account Statement', 'FINANCIAL', 'custody',
 'Statement from custodian bank showing assets held in custody.',
 '{"entity_types": ["ALL"], "proves": ["assets_under_custody", "securities_holdings"], "validity_rules": {"max_age_months": 3}}'::jsonb),

('d0060000-0000-0000-0000-000000000103'::uuid, 'CREDIT_REPORT', 'Credit Report / Credit Reference', 'FINANCIAL', 'credit',
 'Credit report from credit reference agency (Experian, Equifax, TransUnion).',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"], "proves": ["credit_history", "credit_score"]}'::jsonb),

('d0060000-0000-0000-0000-000000000104'::uuid, 'LETTER_OF_CREDIT', 'Letter of Credit', 'FINANCIAL', 'trade_finance',
 'Documentary letter of credit from issuing bank.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["credit_facility"]}'::jsonb),

('d0060000-0000-0000-0000-000000000105'::uuid, 'BANK_GUARANTEE', 'Bank Guarantee / Indemnity', 'FINANCIAL', 'guarantee',
 'Guarantee or indemnity issued by a bank.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["guarantee_commitment"]}'::jsonb),

('d0060000-0000-0000-0000-000000000106'::uuid, 'LOAN_AGREEMENT', 'Loan Agreement', 'FINANCIAL', 'credit',
 'Agreement governing a loan facility.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PROPER_PERSON_NATURAL"], "proves": ["indebtedness", "loan_terms"]}'::jsonb),

('d0060000-0000-0000-0000-000000000107'::uuid, 'FACILITY_LETTER', 'Facility Letter / Term Sheet', 'FINANCIAL', 'credit',
 'Bank facility letter or term sheet for credit facility.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["credit_facility", "facility_terms"]}'::jsonb),

('d0060000-0000-0000-0000-000000000108'::uuid, 'SECURITY_AGREEMENT', 'Security Agreement / Pledge', 'FINANCIAL', 'security',
 'Agreement creating security interest over assets.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["security_interest", "collateral"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDITIONAL TAX DOCUMENTS
-- -----------------------------------------------------------------------------
('d0070000-0000-0000-0000-000000000101'::uuid, 'IRS_147C', 'IRS Letter 147C', 'TAX', 'us_tax',
 'IRS letter confirming Employer Identification Number (EIN).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PARTNERSHIP_LIMITED"], "jurisdictions": ["US"], "proves": ["ein"]}'::jsonb),

('d0070000-0000-0000-0000-000000000102'::uuid, 'EIN_CONFIRMATION', 'EIN Confirmation Letter (CP 575)', 'TAX', 'us_tax',
 'IRS CP 575 notice confirming EIN assignment.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "PARTNERSHIP_LIMITED", "TRUST_DISCRETIONARY"], "jurisdictions": ["US"], "proves": ["ein"]}'::jsonb),

('d0070000-0000-0000-0000-000000000103'::uuid, 'W8_BEN_E', 'Form W-8BEN-E', 'TAX', 'us_withholding',
 'Certificate of Status of Beneficial Owner for US Tax Withholding (Entities).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP_LIMITED", "TRUST_DISCRETIONARY"], "validity_rules": {"validity_years": 3, "expires_year_end": true}}'::jsonb),

('d0070000-0000-0000-0000-000000000104'::uuid, 'W8_IMY', 'Form W-8IMY', 'TAX', 'us_withholding',
 'Certificate of Foreign Intermediary (for qualified intermediaries).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "jurisdictions": ["US_REPORTING"], "validity_rules": {"validity_years": 3}}'::jsonb),

('d0070000-0000-0000-0000-000000000105'::uuid, 'W8_ECI', 'Form W-8ECI', 'TAX', 'us_withholding',
 'Certificate of Foreign Person''s Claim for Effectively Connected Income.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US_REPORTING"], "validity_rules": {"validity_years": 3}}'::jsonb),

('d0070000-0000-0000-0000-000000000106'::uuid, 'W8_EXP', 'Form W-8EXP', 'TAX', 'us_withholding',
 'Certificate of Foreign Government or International Organization.',
 '{"entity_types": ["GOVERNMENT", "INTERNATIONAL_ORG"], "jurisdictions": ["US_REPORTING"], "validity_rules": {"validity_years": 3}}'::jsonb),

('d0070000-0000-0000-0000-000000000107'::uuid, 'FORM_1099', 'IRS Form 1099', 'TAX', 'us_tax',
 'US information return reporting various types of income.',
 '{"entity_types": ["PROPER_PERSON_NATURAL", "LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"], "proves": ["income", "dividends", "interest"]}'::jsonb),

('d0070000-0000-0000-0000-000000000108'::uuid, 'FORM_1042S', 'IRS Form 1042-S', 'TAX', 'us_tax',
 'Foreign Person''s US Source Income Subject to Withholding.',
 '{"entity_types": ["ALL"], "jurisdictions": ["US_REPORTING"], "proves": ["us_source_income", "withholding"]}'::jsonb),

('d0070000-0000-0000-0000-000000000109'::uuid, 'HMRC_SA302', 'HMRC SA302 Tax Calculation', 'TAX', 'uk_tax',
 'UK HMRC tax calculation summary for self-assessment.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["UK"], "proves": ["income", "tax_paid"]}'::jsonb),

('d0070000-0000-0000-0000-000000000110'::uuid, 'HMRC_TAX_OVERVIEW', 'HMRC Tax Year Overview', 'TAX', 'uk_tax',
 'UK HMRC overview of tax position for a tax year.',
 '{"entity_types": ["PROPER_PERSON_NATURAL"], "jurisdictions": ["UK"], "proves": ["tax_position"]}'::jsonb),

('d0070000-0000-0000-0000-000000000111'::uuid, 'VAT_REGISTRATION', 'VAT Registration Certificate', 'TAX', 'indirect',
 'Certificate confirming VAT/GST registration.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["vat_number", "vat_registered"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- INSURANCE DOCUMENTS
-- -----------------------------------------------------------------------------
('d0130000-0000-0000-0000-000000000001'::uuid, 'D_AND_O_INSURANCE', 'Directors & Officers Insurance Certificate', 'INSURANCE', 'liability',
 'Certificate of D&O liability insurance coverage.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["dando_coverage", "indemnity_limit"]}'::jsonb),

('d0130000-0000-0000-0000-000000000002'::uuid, 'PI_INSURANCE', 'Professional Indemnity Insurance Certificate', 'INSURANCE', 'liability',
 'Certificate of professional indemnity/errors & omissions insurance.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "proves": ["pi_coverage", "indemnity_limit"]}'::jsonb),

('d0130000-0000-0000-0000-000000000003'::uuid, 'FIDELITY_BOND', 'Fidelity Bond / Crime Insurance', 'INSURANCE', 'crime',
 'Certificate of fidelity bond or crime insurance coverage.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["fidelity_coverage"]}'::jsonb),

('d0130000-0000-0000-0000-000000000004'::uuid, 'CYBER_INSURANCE', 'Cyber Liability Insurance Certificate', 'INSURANCE', 'cyber',
 'Certificate of cyber liability insurance coverage.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"], "proves": ["cyber_coverage"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDITIONAL REGULATORY / COMPLIANCE
-- -----------------------------------------------------------------------------
('d0090000-0000-0000-0000-000000000101'::uuid, 'COMPLIANCE_CERTIFICATE', 'Compliance Certificate', 'REGULATORY', 'compliance',
 'Certificate confirming compliance with regulatory requirements.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["regulatory_compliance"]}'::jsonb),

('d0090000-0000-0000-0000-000000000102'::uuid, 'AUDITOR_LETTER', 'Auditor''s Comfort Letter', 'REGULATORY', 'audit',
 'Letter from auditors providing comfort on specific matters.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["auditor_confirmation"]}'::jsonb),

('d0090000-0000-0000-0000-000000000103'::uuid, 'CONFLICT_DISCLOSURE', 'Conflict of Interest Disclosure', 'REGULATORY', 'governance',
 'Disclosure of conflicts of interest by directors/officers.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["conflicts_disclosed"]}'::jsonb),

('d0090000-0000-0000-0000-000000000104'::uuid, 'MLRO_CERT', 'MLRO Certification', 'REGULATORY', 'aml',
 'Certification from Money Laundering Reporting Officer.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["aml_compliance"]}'::jsonb),

('d0090000-0000-0000-0000-000000000105'::uuid, 'WOLFSBERG_QUESTIONNAIRE', 'Wolfsberg Questionnaire', 'REGULATORY', 'aml',
 'Wolfsberg Group AML questionnaire for correspondent banking.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "regulatory": "WOLFSBERG", "proves": ["aml_framework"]}'::jsonb),

('d0090000-0000-0000-0000-000000000106'::uuid, 'FCA_REGISTER_EXTRACT', 'FCA Register Entry', 'REGULATORY', 'license',
 'Extract from UK Financial Conduct Authority register.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["UK"], "proves": ["fca_authorized", "permissions"]}'::jsonb),

('d0090000-0000-0000-0000-000000000107'::uuid, 'SEC_REGISTRATION', 'SEC Registration Statement', 'REGULATORY', 'license',
 'US Securities and Exchange Commission registration.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "jurisdictions": ["US"], "proves": ["sec_registered"]}'::jsonb),

('d0090000-0000-0000-0000-000000000108'::uuid, 'FORM_ADV', 'SEC Form ADV', 'REGULATORY', 'license',
 'Investment adviser registration with SEC/state regulators.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE"], "jurisdictions": ["US"], "proves": ["investment_adviser_registration"]}'::jsonb),

('d0090000-0000-0000-0000-000000000109'::uuid, 'FORM_PF', 'SEC Form PF', 'REGULATORY', 'reporting',
 'Private fund adviser reporting to SEC.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "jurisdictions": ["US"], "proves": ["form_pf_filing"]}'::jsonb),

('d0090000-0000-0000-0000-000000000110'::uuid, 'CIMA_LICENSE', 'CIMA Fund License', 'REGULATORY', 'license',
 'Cayman Islands Monetary Authority fund registration/license.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE"], "jurisdictions": ["KY"], "proves": ["cima_registered", "fund_category"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- ADDITIONAL ISDA / DERIVATIVES DOCUMENTS  
-- -----------------------------------------------------------------------------
('d0110000-0000-0000-0000-000000000101'::uuid, 'ISDA_DEFINITIONS', 'ISDA Definitions Booklet', 'ISDA', 'definitions',
 'Reference to applicable ISDA Definitions (2006, 2021, etc.).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["applicable_definitions"]}'::jsonb),

('d0110000-0000-0000-0000-000000000102'::uuid, 'TRADE_CONFIRMATION', 'OTC Trade Confirmation', 'ISDA', 'transaction',
 'Confirmation for individual OTC derivative transaction.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["trade_terms"]}'::jsonb),

('d0110000-0000-0000-0000-000000000103'::uuid, 'IM_CSA', 'Initial Margin CSA (IM-CSA)', 'ISDA', 'collateral',
 'ISDA Credit Support Annex for Initial Margin (regulatory).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["im_collateral_terms"]}'::jsonb),

('d0110000-0000-0000-0000-000000000104'::uuid, 'VM_CSA', 'Variation Margin CSA (VM-CSA)', 'ISDA', 'collateral',
 'ISDA Credit Support Annex for Variation Margin.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["vm_collateral_terms"]}'::jsonb),

('d0110000-0000-0000-0000-000000000105'::uuid, 'ISDA_AMENDMENT', 'ISDA Amendment Agreement', 'ISDA', 'amendment',
 'Amendment to existing ISDA Master Agreement.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["amended_terms"]}'::jsonb),

('d0110000-0000-0000-0000-000000000106'::uuid, 'COLLATERAL_ELIGIBILITY', 'Eligible Collateral Schedule', 'ISDA', 'collateral',
 'Schedule specifying eligible collateral under CSA.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["eligible_collateral"]}'::jsonb),

('d0110000-0000-0000-0000-000000000107'::uuid, 'NETTING_OPINION', 'Netting Opinion', 'ISDA', 'legal',
 'Legal opinion on enforceability of close-out netting.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["netting_enforceability"]}'::jsonb),

('d0110000-0000-0000-0000-000000000108'::uuid, 'CREDIT_SUPPORT_DEED', 'ISDA Credit Support Deed (English Law)', 'ISDA', 'collateral',
 'Title transfer collateral arrangement under English law.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["collateral_terms", "title_transfer"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- PRIME BROKERAGE / CUSTODY AGREEMENTS
-- -----------------------------------------------------------------------------
('d0140000-0000-0000-0000-000000000001'::uuid, 'PB_AGREEMENT', 'Prime Brokerage Agreement', 'CONTRACT', 'prime_brokerage',
 'Agreement with prime broker for financing, execution, custody services.',
 '{"entity_types": ["FUND_HEDGE"], "proves": ["pb_relationship", "margin_terms"]}'::jsonb),

('d0140000-0000-0000-0000-000000000002'::uuid, 'CUSTODY_AGREEMENT', 'Custody Agreement', 'CONTRACT', 'custody',
 'Agreement with custodian for safekeeping of assets.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"], "proves": ["custody_arrangement"]}'::jsonb),

('d0140000-0000-0000-0000-000000000003'::uuid, 'EXECUTION_AGREEMENT', 'Execution Agreement / Give-Up', 'CONTRACT', 'execution',
 'Agreement for trade execution and give-up arrangements.',
 '{"entity_types": ["FUND_HEDGE"], "proves": ["execution_terms"]}'::jsonb),

('d0140000-0000-0000-0000-000000000004'::uuid, 'REPO_AGREEMENT', 'GMRA / Repo Agreement', 'CONTRACT', 'financing',
 'Global Master Repurchase Agreement for repo transactions.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["repo_authority"]}'::jsonb),

('d0140000-0000-0000-0000-000000000005'::uuid, 'SECURITIES_LENDING', 'GMSLA / Securities Lending Agreement', 'CONTRACT', 'financing',
 'Global Master Securities Lending Agreement.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["sec_lending_authority"]}'::jsonb),

('d0140000-0000-0000-0000-000000000006'::uuid, 'FUTURES_ACCOUNT_AGREEMENT', 'Futures Account Agreement', 'CONTRACT', 'futures',
 'Agreement for exchange-traded futures/options account.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["futures_authority"]}'::jsonb),

('d0140000-0000-0000-0000-000000000007'::uuid, 'FX_AGREEMENT', 'FX Trading Agreement', 'CONTRACT', 'fx',
 'Agreement for foreign exchange trading (ISDA-type or separate).',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "FUND_HEDGE"], "proves": ["fx_authority"]}'::jsonb),

-- -----------------------------------------------------------------------------
-- SERVICE PROVIDER AGREEMENTS
-- -----------------------------------------------------------------------------
('d0150000-0000-0000-0000-000000000001'::uuid, 'ADMIN_AGREEMENT', 'Administration Agreement', 'CONTRACT', 'service',
 'Fund administration services agreement.',
 '{"entity_types": ["FUND_HEDGE", "FUND_PE", "FUND_UCITS"], "proves": ["administrator_appointment"]}'::jsonb),

('d0150000-0000-0000-0000-000000000002'::uuid, 'DEPOSITARY_AGREEMENT', 'Depositary Agreement', 'CONTRACT', 'service',
 'UCITS/AIFMD depositary appointment agreement.',
 '{"entity_types": ["FUND_HEDGE", "FUND_UCITS"], "proves": ["depositary_appointment"]}'::jsonb),

('d0150000-0000-0000-0000-000000000003'::uuid, 'AUDITOR_ENGAGEMENT', 'Auditor Engagement Letter', 'CONTRACT', 'service',
 'Letter engaging auditors for audit services.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["auditor_appointment"]}'::jsonb),

('d0150000-0000-0000-0000-000000000004'::uuid, 'LEGAL_ENGAGEMENT', 'Legal Engagement Letter', 'CONTRACT', 'service',
 'Letter of engagement with legal counsel.',
 '{"entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "FUND_HEDGE"], "proves": ["legal_counsel"]}'::jsonb)

ON CONFLICT (type_code) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    category = EXCLUDED.category,
    domain = EXCLUDED.domain,
    description = EXCLUDED.description,
    applicability = EXCLUDED.applicability;
