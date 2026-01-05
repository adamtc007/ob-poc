-- Migration: Document & Attribute Catalogue Completion
-- Generated: 2026-01-05
-- Purpose: Add new document types, attributes, and links to complete the catalogue

-- ============================================================================
-- SECTION 1: NEW DOCUMENT TYPES
-- ============================================================================

-- 1.1 Address Proof Documents
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, required_attributes, applicability, semantic_context) VALUES
('d0080000-0000-0000-0000-000000000001', 'UTILITY_BILL_ELECTRIC', 'Electricity Bill', 'ADDRESS_PROOF', 'address',
 'Electricity utility bill for address verification. Must be dated within 3 months.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 90}',
 '{"purpose": "address_verification", "keywords": ["electricity", "power", "utility", "address proof"], "synonyms": ["electric bill", "power bill"]}'),

('d0080000-0000-0000-0000-000000000002', 'UTILITY_BILL_GAS', 'Gas Bill', 'ADDRESS_PROOF', 'address',
 'Gas utility bill for address verification. Must be dated within 3 months.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 90}',
 '{"purpose": "address_verification", "keywords": ["gas", "utility", "heating", "address proof"], "synonyms": ["gas statement"]}'),

('d0080000-0000-0000-0000-000000000003', 'UTILITY_BILL_WATER', 'Water Bill', 'ADDRESS_PROOF', 'address',
 'Water utility bill for address verification. Must be dated within 3 months.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 90}',
 '{"purpose": "address_verification", "keywords": ["water", "utility", "address proof"], "synonyms": ["water rates"]}'),

('d0080000-0000-0000-0000-000000000004', 'TELEPHONE_BILL', 'Telephone Bill', 'ADDRESS_PROOF', 'address',
 'Landline telephone bill for address verification. Mobile phone bills typically not accepted. Must be dated within 3 months.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 90, "excludes": ["mobile_phone"]}',
 '{"purpose": "address_verification", "keywords": ["telephone", "landline", "phone bill"], "synonyms": ["phone statement"]}'),

('d0080000-0000-0000-0000-000000000005', 'COUNCIL_TAX_BILL', 'Council Tax Bill', 'ADDRESS_PROOF', 'address',
 'UK Council Tax bill for address verification. Valid for current tax year.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["GB"], "max_age_days": 365}',
 '{"purpose": "address_verification", "keywords": ["council tax", "local authority", "rates"], "synonyms": ["council tax statement", "council tax demand"]}'),

('d0080000-0000-0000-0000-000000000006', 'MORTGAGE_STATEMENT', 'Mortgage Statement', 'ADDRESS_PROOF', 'address',
 'Mortgage statement showing property address. Must be dated within 3 months.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["*"], "max_age_days": 90}',
 '{"purpose": "address_verification", "keywords": ["mortgage", "home loan", "property"], "synonyms": ["mortgage letter"]}'),

('d0080000-0000-0000-0000-000000000007', 'TENANCY_AGREEMENT', 'Tenancy Agreement', 'ADDRESS_PROOF', 'address',
 'Current rental/lease agreement showing residential address.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["*"], "requires": ["current_tenancy"]}',
 '{"purpose": "address_verification", "keywords": ["tenancy", "rental", "lease", "landlord"], "synonyms": ["rental agreement", "lease agreement", "AST"]}'),

('d0080000-0000-0000-0000-000000000008', 'PROPERTY_TAX_BILL', 'Property Tax Bill', 'ADDRESS_PROOF', 'address',
 'Property/real estate tax bill for address verification.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 365}',
 '{"purpose": "address_verification", "keywords": ["property tax", "real estate tax", "rates"]}'),

('d0080000-0000-0000-0000-000000000009', 'BANK_LETTER_ADDRESS', 'Bank Address Confirmation Letter', 'ADDRESS_PROOF', 'address',
 'Letter from bank confirming account holder address. Must be dated within 3 months.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 90}',
 '{"purpose": "address_verification", "keywords": ["bank letter", "address confirmation"]}'),

('d0080000-0000-0000-0000-000000000010', 'GOVERNMENT_CORRESPONDENCE', 'Government Correspondence', 'ADDRESS_PROOF', 'address',
 'Official government letter showing name and address (e.g., HMRC, DWP, Social Security).',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["*"], "max_age_days": 180}',
 '{"purpose": "address_verification", "keywords": ["government letter", "official correspondence", "HMRC", "DWP"]}')
ON CONFLICT (type_code) DO NOTHING;

-- 1.2 Additional Tax Documents
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, required_attributes, applicability, semantic_context) VALUES
('d0070000-0000-0000-0000-000000000010', 'W8_ECI', 'Form W-8ECI', 'TAX', 'us_withholding',
 'Certificate of Foreign Person''s Claim That Income Is Effectively Connected With the Conduct of a Trade or Business in the United States.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["US"], "requires": ["us_tin"], "validity_years": 3}',
 '{"purpose": "tax_withholding", "keywords": ["W-8ECI", "effectively connected income", "ECI", "US trade or business"], "irs_form": "W-8ECI"}'),

('d0070000-0000-0000-0000-000000000011', 'W8_EXP', 'Form W-8EXP', 'TAX', 'us_withholding',
 'Certificate of Foreign Government or Other Foreign Organization for United States Tax Withholding and Reporting.',
 '{}',
 '{"entity_types": ["GOVERNMENT", "TAX_EXEMPT_ORG", "CENTRAL_BANK", "INTERNATIONAL_ORG"], "jurisdictions": ["US"], "validity_years": 3}',
 '{"purpose": "tax_exemption", "keywords": ["W-8EXP", "foreign government", "tax exempt", "501(c)", "892"], "irs_form": "W-8EXP"}'),

('d0070000-0000-0000-0000-000000000012', 'W8_IMY', 'Form W-8IMY', 'TAX', 'us_withholding',
 'Certificate of Foreign Intermediary, Foreign Flow-Through Entity, or Certain U.S. Branches for United States Tax Withholding and Reporting.',
 '{}',
 '{"entity_types": ["INTERMEDIARY", "PARTNERSHIP", "TRUST", "QI", "WP", "WT"], "jurisdictions": ["US"], "validity_years": 3, "requires": ["withholding_statement"]}',
 '{"purpose": "intermediary_withholding", "keywords": ["W-8IMY", "intermediary", "flow-through", "qualified intermediary", "QI"], "irs_form": "W-8IMY"}'),

('d0070000-0000-0000-0000-000000000013', 'FORM_1099_DIV', 'Form 1099-DIV', 'TAX', 'us_reporting',
 'Dividends and Distributions information return.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["US"]}',
 '{"purpose": "tax_reporting", "keywords": ["1099-DIV", "dividends", "capital gains distribution"], "irs_form": "1099-DIV"}'),

('d0070000-0000-0000-0000-000000000014', 'FORM_1099_INT', 'Form 1099-INT', 'TAX', 'us_reporting',
 'Interest Income information return.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["US"]}',
 '{"purpose": "tax_reporting", "keywords": ["1099-INT", "interest income"], "irs_form": "1099-INT"}'),

('d0070000-0000-0000-0000-000000000015', 'FORM_1099_B', 'Form 1099-B', 'TAX', 'us_reporting',
 'Proceeds from Broker and Barter Exchange Transactions.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["US"]}',
 '{"purpose": "tax_reporting", "keywords": ["1099-B", "broker", "securities sales", "cost basis"], "irs_form": "1099-B"}'),

('d0070000-0000-0000-0000-000000000016', 'FORM_K1', 'Schedule K-1', 'TAX', 'us_reporting',
 'Partner''s/Shareholder''s Share of Income, Deductions, Credits (Form 1065/1120-S).',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["US"], "required_for": ["PARTNERSHIP", "S_CORP"]}',
 '{"purpose": "partnership_tax", "keywords": ["K-1", "partnership income", "S-corp", "pass-through"], "irs_form": "Schedule K-1"}'),

('d0070000-0000-0000-0000-000000000020', 'P60', 'P60 End of Year Certificate', 'TAX', 'uk_employment',
 'UK annual summary of pay and tax deducted by employer.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["GB"], "issued_by": "employer"}',
 '{"purpose": "income_verification", "keywords": ["P60", "PAYE", "tax year", "NI contributions"], "hmrc_form": "P60"}'),

('d0070000-0000-0000-0000-000000000021', 'P45', 'P45 Leaving Certificate', 'TAX', 'uk_employment',
 'UK certificate given when leaving employment showing tax paid to date.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["GB"], "issued_by": "employer"}',
 '{"purpose": "employment_change", "keywords": ["P45", "leaving employment", "tax code"], "hmrc_form": "P45"}'),

('d0070000-0000-0000-0000-000000000022', 'SA302', 'SA302 Tax Calculation', 'TAX', 'uk_self_assessment',
 'UK HMRC tax calculation from self-assessment return.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["GB"], "issued_by": "HMRC"}',
 '{"purpose": "income_verification", "keywords": ["SA302", "self assessment", "tax calculation"], "hmrc_form": "SA302"}'),

('d0070000-0000-0000-0000-000000000023', 'TAX_CLEARANCE_CERT', 'Tax Clearance Certificate', 'TAX', 'clearance',
 'Certificate confirming tax affairs are in order. Required for certain transactions.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "tax_compliance", "keywords": ["tax clearance", "good standing", "no outstanding taxes"]}'),

('d0070000-0000-0000-0000-000000000024', 'VAT_REGISTRATION', 'VAT Registration Certificate', 'TAX', 'indirect_tax',
 'Certificate of VAT/GST registration.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "indirect_tax", "keywords": ["VAT", "GST", "sales tax", "registration"], "synonyms": ["GST registration"]}'),

('d0070000-0000-0000-0000-000000000025', 'DOUBLE_TAX_TREATY_CERT', 'Double Taxation Treaty Certificate', 'TAX', 'treaty',
 'Certificate of residence for double taxation treaty purposes.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 365}',
 '{"purpose": "treaty_benefit", "keywords": ["double taxation", "treaty", "residence certificate", "DTA"]}')
ON CONFLICT (type_code) DO NOTHING;

-- 1.3 UBO/Ownership Documents
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, required_attributes, applicability, semantic_context) VALUES
('d0090000-0000-0000-0000-000000000001', 'UBO_DECLARATION', 'UBO Declaration Form', 'UBO', 'beneficial_ownership',
 'Declaration form identifying Ultimate Beneficial Owners (25%+ ownership or control).',
 '{}',
 '{"entity_types": ["COMPANY", "PARTNERSHIP", "TRUST", "FUND"], "jurisdictions": ["*"], "required_for": ["onboarding"]}',
 '{"purpose": "ubo_identification", "keywords": ["UBO", "beneficial owner", "25%", "control"], "synonyms": ["beneficial ownership declaration"]}'),

('d0090000-0000-0000-0000-000000000002', 'OWNERSHIP_CHART', 'Ownership Structure Chart', 'UBO', 'beneficial_ownership',
 'Visual diagram showing ownership structure and percentages through all layers.',
 '{}',
 '{"entity_types": ["COMPANY", "PARTNERSHIP", "FUND"], "jurisdictions": ["*"]}',
 '{"purpose": "ubo_visualization", "keywords": ["ownership chart", "structure diagram", "org chart", "ownership tree"]}'),

('d0090000-0000-0000-0000-000000000003', 'PSC_REGISTER', 'PSC Register Extract', 'UBO', 'beneficial_ownership',
 'UK Persons with Significant Control register extract from Companies House.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["GB"], "issued_by": "companies_house"}',
 '{"purpose": "ubo_verification", "keywords": ["PSC", "significant control", "Companies House", "UK register"]}'),

('d0090000-0000-0000-0000-000000000004', 'BO_REGISTER_EXTRACT', 'Beneficial Ownership Register Extract', 'UBO', 'beneficial_ownership',
 'Extract from national beneficial ownership register (EU AMLD / jurisdiction specific).',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["EU", "EEA"]}',
 '{"purpose": "ubo_verification", "keywords": ["BO register", "beneficial ownership register", "AMLD", "transparency register"]}'),

('d0090000-0000-0000-0000-000000000005', 'NOMINEE_DECLARATION', 'Nominee Declaration', 'UBO', 'beneficial_ownership',
 'Declaration confirming nominee arrangement and identifying the beneficial owner.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "nominee_disclosure", "keywords": ["nominee", "beneficial owner", "bare trust", "declaration"]}'),

('d0090000-0000-0000-0000-000000000006', 'FAMILY_TREE_DIAGRAM', 'Family Tree Diagram', 'UBO', 'beneficial_ownership',
 'Family relationship diagram for trusts, estates, or family-controlled entities.',
 '{}',
 '{"entity_types": ["TRUST", "ESTATE", "FAMILY_OFFICE"], "jurisdictions": ["*"]}',
 '{"purpose": "relationship_mapping", "keywords": ["family tree", "genealogy", "relationships", "beneficiaries"]}'),

('d0090000-0000-0000-0000-000000000007', 'POWER_OF_ATTORNEY', 'Power of Attorney', 'UBO', 'authority',
 'Legal document granting authority to act on behalf of another.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["*"]}',
 '{"purpose": "authority_delegation", "keywords": ["POA", "power of attorney", "attorney-in-fact", "proxy"], "synonyms": ["POA", "LPA"]}'),

('d0090000-0000-0000-0000-000000000008', 'COURT_ORDER', 'Court Order', 'UBO', 'legal',
 'Court order relating to guardianship, administration, or control.',
 '{}',
 '{"entity_types": ["PERSON", "ESTATE"], "jurisdictions": ["*"]}',
 '{"purpose": "legal_authority", "keywords": ["court order", "guardianship", "administration order"]}'),

('d0090000-0000-0000-0000-000000000009', 'PROBATE_GRANT', 'Grant of Probate', 'UBO', 'estate',
 'Court document granting authority to administer a deceased person''s estate.',
 '{}',
 '{"entity_types": ["ESTATE"], "jurisdictions": ["*"]}',
 '{"purpose": "estate_administration", "keywords": ["probate", "grant", "executor", "estate"]}'),

('d0090000-0000-0000-0000-000000000010', 'LETTERS_OF_ADMINISTRATION', 'Letters of Administration', 'UBO', 'estate',
 'Court document appointing administrator for intestate estate.',
 '{}',
 '{"entity_types": ["ESTATE"], "jurisdictions": ["*"]}',
 '{"purpose": "estate_administration", "keywords": ["letters of administration", "administrator", "intestate"]}'),

('d0090000-0000-0000-0000-000000000011', 'WILL_TESTAMENT', 'Last Will and Testament', 'UBO', 'estate',
 'Deceased person''s will showing beneficiaries and executors.',
 '{}',
 '{"entity_types": ["ESTATE"], "jurisdictions": ["*"]}',
 '{"purpose": "estate_planning", "keywords": ["will", "testament", "beneficiary", "executor", "bequest"]}')
ON CONFLICT (type_code) DO NOTHING;

-- 1.4 Regulatory/Compliance Documents
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, required_attributes, applicability, semantic_context) VALUES
('d0100000-0000-0000-0000-000000000001', 'FCA_AUTHORIZATION', 'FCA Authorization Letter', 'REGULATORY', 'authorization',
 'UK Financial Conduct Authority authorization/permissions letter.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["GB"], "issued_by": "FCA"}',
 '{"purpose": "regulatory_status", "keywords": ["FCA", "authorization", "permissions", "regulated firm"]}'),

('d0100000-0000-0000-0000-000000000002', 'SEC_REGISTRATION', 'SEC Registration', 'REGULATORY', 'authorization',
 'US Securities and Exchange Commission registration (e.g., investment adviser, broker-dealer).',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["US"], "issued_by": "SEC"}',
 '{"purpose": "regulatory_status", "keywords": ["SEC", "registration", "investment adviser", "broker-dealer"]}'),

('d0100000-0000-0000-0000-000000000003', 'FINRA_REGISTRATION', 'FINRA Registration', 'REGULATORY', 'authorization',
 'US FINRA broker-dealer registration.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["US"], "issued_by": "FINRA"}',
 '{"purpose": "regulatory_status", "keywords": ["FINRA", "broker-dealer", "CRD"]}'),

('d0100000-0000-0000-0000-000000000004', 'MIFID_AUTHORIZATION', 'MiFID II Authorization', 'REGULATORY', 'authorization',
 'EU Markets in Financial Instruments Directive authorization.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["EU", "EEA"]}',
 '{"purpose": "regulatory_status", "keywords": ["MiFID", "MiFID II", "investment firm", "passporting"]}'),

('d0100000-0000-0000-0000-000000000005', 'AIFMD_AUTHORIZATION', 'AIFMD Authorization', 'REGULATORY', 'authorization',
 'Alternative Investment Fund Managers Directive authorization.',
 '{}',
 '{"entity_types": ["COMPANY", "FUND"], "jurisdictions": ["EU", "EEA"]}',
 '{"purpose": "regulatory_status", "keywords": ["AIFMD", "AIF", "alternative investment fund", "AIFM"]}'),

('d0100000-0000-0000-0000-000000000006', 'UCITS_AUTHORIZATION', 'UCITS Authorization', 'REGULATORY', 'authorization',
 'UCITS fund authorization certificate.',
 '{}',
 '{"entity_types": ["FUND"], "jurisdictions": ["EU", "EEA"]}',
 '{"purpose": "fund_authorization", "keywords": ["UCITS", "undertaking", "collective investment"]}'),

('d0100000-0000-0000-0000-000000000007', 'AML_REGISTRATION', 'AML Registration Certificate', 'REGULATORY', 'registration',
 'Anti-Money Laundering supervisory registration (e.g., HMRC MSB registration).',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "aml_compliance", "keywords": ["AML", "MSB", "money services", "registration"]}'),

('d0100000-0000-0000-0000-000000000008', 'SANCTIONS_SCREENING_REPORT', 'Sanctions Screening Report', 'REGULATORY', 'screening',
 'Results of sanctions list screening (OFAC, EU, UN, HMT).',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 30}',
 '{"purpose": "sanctions_compliance", "keywords": ["sanctions", "OFAC", "SDN", "screening", "HMT"]}'),

('d0100000-0000-0000-0000-000000000009', 'PEP_SCREENING_REPORT', 'PEP Screening Report', 'REGULATORY', 'screening',
 'Politically Exposed Persons screening results.',
 '{}',
 '{"entity_types": ["PERSON"], "jurisdictions": ["*"], "max_age_days": 30}',
 '{"purpose": "pep_compliance", "keywords": ["PEP", "politically exposed", "screening", "public official"]}'),

('d0100000-0000-0000-0000-000000000010', 'ADVERSE_MEDIA_REPORT', 'Adverse Media Report', 'REGULATORY', 'screening',
 'Adverse/negative media screening results.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"], "max_age_days": 30}',
 '{"purpose": "reputation_screening", "keywords": ["adverse media", "negative news", "screening", "reputation"]}'),

('d0100000-0000-0000-0000-000000000011', 'COMPLIANCE_MANUAL', 'Compliance Manual', 'REGULATORY', 'policies',
 'Internal compliance policies and procedures manual.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "policy_documentation", "keywords": ["compliance", "policies", "procedures", "manual"]}'),

('d0100000-0000-0000-0000-000000000012', 'AML_RISK_ASSESSMENT', 'AML Risk Assessment', 'REGULATORY', 'risk',
 'Business-wide or customer-specific AML risk assessment.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "risk_assessment", "keywords": ["risk assessment", "AML", "ML/TF", "inherent risk"]}'),

('d0100000-0000-0000-0000-000000000013', 'LEI_CERTIFICATE', 'LEI Certificate', 'REGULATORY', 'identification',
 'Legal Entity Identifier certificate from GLEIF-accredited LOU.',
 '{}',
 '{"entity_types": ["COMPANY", "FUND"], "jurisdictions": ["*"]}',
 '{"purpose": "entity_identification", "keywords": ["LEI", "legal entity identifier", "GLEIF", "LOU"]}')
ON CONFLICT (type_code) DO NOTHING;

-- 1.5 Corporate Governance Documents
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, required_attributes, applicability, semantic_context) VALUES
('d0110000-0000-0000-0000-000000000001', 'SHAREHOLDER_AGREEMENT', 'Shareholder Agreement', 'CORPORATE', 'governance',
 'Agreement between shareholders governing their relationship and rights.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "shareholder_rights", "keywords": ["shareholder agreement", "SHA", "investor rights"], "synonyms": ["shareholders agreement", "stockholder agreement"]}'),

('d0110000-0000-0000-0000-000000000002', 'VOTING_AGREEMENT', 'Voting Agreement', 'CORPORATE', 'governance',
 'Agreement governing voting rights and arrangements.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "voting_rights", "keywords": ["voting agreement", "voting trust", "proxy"]}'),

('d0110000-0000-0000-0000-000000000003', 'DRAG_ALONG_AGREEMENT', 'Drag-Along Agreement', 'CORPORATE', 'governance',
 'Agreement requiring minority shareholders to join in sale.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "exit_rights", "keywords": ["drag-along", "drag along", "compulsory sale"]}'),

('d0110000-0000-0000-0000-000000000004', 'TAG_ALONG_AGREEMENT', 'Tag-Along Agreement', 'CORPORATE', 'governance',
 'Agreement allowing minority shareholders to join in sale.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "exit_rights", "keywords": ["tag-along", "tag along", "co-sale", "piggyback"]}'),

('d0110000-0000-0000-0000-000000000005', 'OPTION_AGREEMENT', 'Stock Option Agreement', 'CORPORATE', 'equity',
 'Agreement granting stock options.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "equity_compensation", "keywords": ["stock option", "option grant", "ESOP", "vesting"]}'),

('d0110000-0000-0000-0000-000000000006', 'WARRANT_AGREEMENT', 'Warrant Agreement', 'CORPORATE', 'equity',
 'Agreement for share purchase warrants.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "equity_rights", "keywords": ["warrant", "share warrant", "exercise price"]}'),

('d0110000-0000-0000-0000-000000000007', 'CONVERTIBLE_NOTE', 'Convertible Note Agreement', 'CORPORATE', 'debt',
 'Convertible debt instrument agreement.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "convertible_debt", "keywords": ["convertible note", "convertible debt", "conversion price", "discount"]}'),

('d0110000-0000-0000-0000-000000000008', 'SAFE_AGREEMENT', 'SAFE Agreement', 'CORPORATE', 'equity',
 'Simple Agreement for Future Equity.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["US"]}',
 '{"purpose": "future_equity", "keywords": ["SAFE", "simple agreement", "future equity", "Y Combinator"]}'),

('d0110000-0000-0000-0000-000000000009', 'CERT_OF_DISSOLUTION', 'Certificate of Dissolution', 'CORPORATE', 'status',
 'Certificate confirming company dissolution.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "entity_termination", "keywords": ["dissolution", "winding up", "struck off"]}'),

('d0110000-0000-0000-0000-000000000010', 'CERT_OF_MERGER', 'Certificate of Merger', 'CORPORATE', 'status',
 'Certificate confirming merger/amalgamation.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "corporate_action", "keywords": ["merger", "amalgamation", "consolidation"]}'),

('d0110000-0000-0000-0000-000000000011', 'CERT_OF_CONVERSION', 'Certificate of Conversion', 'CORPORATE', 'status',
 'Certificate confirming entity type conversion.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "corporate_action", "keywords": ["conversion", "re-domiciliation", "continuance"]}')
ON CONFLICT (type_code) DO NOTHING;

-- 1.6 Insurance/Security Documents
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, domain, description, required_attributes, applicability, semantic_context) VALUES
('d0120000-0000-0000-0000-000000000001', 'INSURANCE_DO', 'D&O Insurance Policy', 'INSURANCE', 'coverage',
 'Directors and Officers liability insurance policy.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "liability_coverage", "keywords": ["D&O", "directors", "officers", "liability", "insurance"]}'),

('d0120000-0000-0000-0000-000000000002', 'INSURANCE_EO', 'E&O Insurance Policy', 'INSURANCE', 'coverage',
 'Errors and Omissions / Professional Indemnity insurance policy.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "professional_liability", "keywords": ["E&O", "errors", "omissions", "professional indemnity", "PI"]}'),

('d0120000-0000-0000-0000-000000000003', 'GUARANTEE_AGREEMENT', 'Guarantee Agreement', 'SECURITY', 'credit_support',
 'Guarantee or surety agreement.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "credit_enhancement", "keywords": ["guarantee", "surety", "guarantor"]}'),

('d0120000-0000-0000-0000-000000000004', 'PLEDGE_AGREEMENT', 'Pledge Agreement', 'SECURITY', 'collateral',
 'Share or asset pledge agreement.',
 '{}',
 '{"entity_types": ["PERSON", "COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "collateral", "keywords": ["pledge", "security interest", "collateral"]}'),

('d0120000-0000-0000-0000-000000000005', 'SECURITY_AGREEMENT', 'Security Agreement', 'SECURITY', 'collateral',
 'General security agreement granting security interest.',
 '{}',
 '{"entity_types": ["COMPANY"], "jurisdictions": ["*"]}',
 '{"purpose": "secured_lending", "keywords": ["security agreement", "GSA", "debenture", "floating charge"]}')
ON CONFLICT (type_code) DO NOTHING;


-- ============================================================================
-- SECTION 2: NEW ATTRIBUTES
-- ============================================================================

-- 2.1 Address Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.address.utility_account_number', gen_random_uuid(), 'Utility Account Number', 'address', 'string', 'address_proof',
 '{"pattern": "^[A-Za-z0-9-]+$"}',
 '{"document_types": ["UTILITY_BILL_ELECTRIC", "UTILITY_BILL_GAS", "UTILITY_BILL_WATER"]}'),

('attr.address.statement_date', gen_random_uuid(), 'Statement Date', 'address', 'date', 'address_proof',
 '{"max_age_days": 90}',
 '{"document_types": ["UTILITY_BILL_ELECTRIC", "UTILITY_BILL_GAS", "UTILITY_BILL_WATER", "BANK_STATEMENT", "MORTGAGE_STATEMENT"]}'),

('attr.address.billing_period_start', gen_random_uuid(), 'Billing Period Start', 'address', 'date', 'address_proof', '{}', '{}'),
('attr.address.billing_period_end', gen_random_uuid(), 'Billing Period End', 'address', 'date', 'address_proof', '{}', '{}'),
('attr.address.service_address', gen_random_uuid(), 'Service Address', 'address', 'address', 'address_proof', '{}', '{}'),
('attr.address.landlord_name', gen_random_uuid(), 'Landlord Name', 'address', 'string', 'tenancy', '{}', '{}'),
('attr.address.tenancy_start_date', gen_random_uuid(), 'Tenancy Start Date', 'address', 'date', 'tenancy', '{}', '{}'),
('attr.address.tenancy_end_date', gen_random_uuid(), 'Tenancy End Date', 'address', 'date', 'tenancy', '{}', '{}'),
('attr.address.monthly_rent', gen_random_uuid(), 'Monthly Rent', 'address', 'currency', 'tenancy', '{}', '{}'),
('attr.address.council_tax_band', gen_random_uuid(), 'Council Tax Band', 'address', 'string', 'uk_council_tax',
 '{"valid_values": ["A", "B", "C", "D", "E", "F", "G", "H"]}',
 '{"jurisdictions": ["GB"]}'),
('attr.address.council_tax_reference', gen_random_uuid(), 'Council Tax Reference', 'address', 'string', 'uk_council_tax', '{}', '{}'),
('attr.address.property_address', gen_random_uuid(), 'Property Address', 'address', 'address', 'property', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.2 Tax Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.tax.chapter3_status', gen_random_uuid(), 'Chapter 3 Status', 'tax', 'string', 'us_withholding',
 '{"valid_values": ["Corporation", "Partnership", "Simple Trust", "Grantor Trust", "Complex Trust", "Estate", "Government", "Central Bank", "Tax Exempt Org", "Private Foundation", "International Organization"]}',
 '{"document_types": ["W8_BEN_E"]}'),

('attr.tax.chapter4_status', gen_random_uuid(), 'Chapter 4 Status (FATCA)', 'tax', 'string', 'us_withholding',
 '{"valid_values": ["Participating FFI", "Reporting Model 1 FFI", "Reporting Model 2 FFI", "Nonreporting IGA FFI", "Nonparticipating FFI", "Owner Documented FFI", "Passive NFFE", "Active NFFE", "Excepted NFFE", "Direct Reporting NFFE"]}',
 '{"document_types": ["W8_BEN_E"]}'),

('attr.tax.giin', gen_random_uuid(), 'GIIN (Global Intermediary ID)', 'tax', 'string', 'fatca',
 '{"pattern": "^[A-Z0-9]{6}\\.[A-Z0-9]{5}\\.[A-Z]{2}\\.[0-9]{3}$"}',
 '{"document_types": ["W8_BEN_E", "FATCA_SELF_CERT"]}'),

('attr.tax.treaty_country', gen_random_uuid(), 'Treaty Country', 'tax', 'string', 'treaty',
 '{"lookup": "country_codes"}',
 '{"document_types": ["W8_BEN", "W8_BEN_E"]}'),

('attr.tax.treaty_article', gen_random_uuid(), 'Treaty Article', 'tax', 'string', 'treaty', '{}', '{}'),
('attr.tax.treaty_withholding_rate', gen_random_uuid(), 'Treaty Withholding Rate', 'tax', 'percentage', 'treaty',
 '{"min": 0, "max": 30}', '{}'),

('attr.tax.lob_provision', gen_random_uuid(), 'LOB Provision Met', 'tax', 'string', 'treaty',
 '{"valid_values": ["Government", "Tax Exempt Pension", "Other Tax Exempt", "Publicly Traded", "Subsidiary of Publicly Traded", "Ownership/Base Erosion", "Active Trade or Business", "Derivative Benefits", "Discretionary Determination"]}',
 '{}'),

('attr.tax.us_tin', gen_random_uuid(), 'US Tax ID (EIN/SSN/ITIN)', 'tax', 'tax_id', 'us_tax',
 '{"pattern": "^[0-9]{2}-[0-9]{7}$|^[0-9]{3}-[0-9]{2}-[0-9]{4}$|^9[0-9]{2}-[0-9]{2}-[0-9]{4}$"}',
 '{}'),

('attr.tax.ftin', gen_random_uuid(), 'Foreign Tax ID Number', 'tax', 'tax_id', 'foreign_tax', '{}', '{}'),

('attr.tax.effectively_connected', gen_random_uuid(), 'Effectively Connected Income', 'tax', 'boolean', 'us_withholding', '{}',
 '{"document_types": ["W8_ECI"]}'),

('attr.tax.us_branch', gen_random_uuid(), 'US Branch', 'tax', 'boolean', 'us_withholding', '{}', '{}'),

('attr.tax.ni_number', gen_random_uuid(), 'National Insurance Number', 'tax', 'string', 'uk_tax',
 '{"pattern": "^[A-CEGHJ-PR-TW-Z]{2}[0-9]{6}[A-D]$"}',
 '{"jurisdictions": ["GB"]}'),

('attr.tax.paye_reference', gen_random_uuid(), 'PAYE Reference', 'tax', 'string', 'uk_tax', '{}', '{"jurisdictions": ["GB"]}'),
('attr.tax.utr', gen_random_uuid(), 'Unique Taxpayer Reference', 'tax', 'string', 'uk_tax',
 '{"pattern": "^[0-9]{10}$"}',
 '{"jurisdictions": ["GB"]}'),

('attr.tax.tax_code', gen_random_uuid(), 'Tax Code', 'tax', 'string', 'uk_tax', '{}', '{"jurisdictions": ["GB"]}'),
('attr.tax.gross_pay_ytd', gen_random_uuid(), 'Gross Pay Year to Date', 'tax', 'currency', 'employment', '{}', '{}'),
('attr.tax.tax_deducted_ytd', gen_random_uuid(), 'Tax Deducted Year to Date', 'tax', 'currency', 'employment', '{}', '{}'),
('attr.tax.ni_contributions_ytd', gen_random_uuid(), 'NI Contributions Year to Date', 'tax', 'currency', 'uk_tax', '{}', '{}'),

('attr.tax.vat_number', gen_random_uuid(), 'VAT Number', 'tax', 'string', 'indirect_tax',
 '{"pattern": "^(GB)?[0-9]{9}$|^(EU)?[0-9]{9}$"}',
 '{}'),

('attr.tax.partner_share_income', gen_random_uuid(), 'Partner Share of Income', 'tax', 'currency', 'partnership_tax', '{}', '{}'),
('attr.tax.partner_share_loss', gen_random_uuid(), 'Partner Share of Loss', 'tax', 'currency', 'partnership_tax', '{}', '{}'),
('attr.tax.partner_capital_account', gen_random_uuid(), 'Partner Capital Account', 'tax', 'currency', 'partnership_tax', '{}', '{}'),
('attr.tax.partner_ownership_pct', gen_random_uuid(), 'Partner Ownership Percentage', 'tax', 'percentage', 'partnership_tax', '{}', '{}'),
('attr.tax.total_income', gen_random_uuid(), 'Total Income', 'tax', 'currency', 'income', '{}', '{}'),
('attr.tax.total_tax_due', gen_random_uuid(), 'Total Tax Due', 'tax', 'currency', 'income', '{}', '{}'),
('attr.tax.tax_year', gen_random_uuid(), 'Tax Year', 'tax', 'string', 'general', '{}', '{}'),
('attr.tax.country_of_citizenship', gen_random_uuid(), 'Country of Citizenship', 'tax', 'string', 'general', '{}', '{}'),
('attr.tax.country_of_incorporation', gen_random_uuid(), 'Country of Incorporation', 'tax', 'string', 'general', '{}', '{}'),
('attr.tax.fatca_status', gen_random_uuid(), 'FATCA Status', 'tax', 'string', 'fatca', '{}', '{}'),
('attr.tax.us_indicia', gen_random_uuid(), 'US Indicia', 'tax', 'boolean', 'fatca', '{}', '{}'),
('attr.tax.tax_residence_countries', gen_random_uuid(), 'Tax Residence Countries', 'tax', 'json', 'crs', '{}', '{}'),
('attr.tax.crs_entity_type', gen_random_uuid(), 'CRS Entity Type', 'tax', 'string', 'crs', '{}', '{}'),
('attr.tax.controlling_persons', gen_random_uuid(), 'Controlling Persons', 'tax', 'json', 'crs', '{}', '{}'),
('attr.tax.tax_residence_country', gen_random_uuid(), 'Tax Residence Country', 'tax', 'string', 'general', '{}', '{}'),
('attr.tax.certificate_date', gen_random_uuid(), 'Certificate Date', 'tax', 'date', 'general', '{}', '{}'),
('attr.tax.tax_classification', gen_random_uuid(), 'Tax Classification', 'tax', 'string', 'us_tax', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.3 UBO Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.ubo.ownership_percentage', gen_random_uuid(), 'Ownership Percentage', 'ubo', 'percentage', 'beneficial_ownership',
 '{"min": 0, "max": 100}', '{}'),

('attr.ubo.ownership_type', gen_random_uuid(), 'Ownership Type', 'ubo', 'string', 'beneficial_ownership',
 '{"valid_values": ["Direct", "Indirect", "Combined"]}', '{}'),

('attr.ubo.control_type', gen_random_uuid(), 'Control Type', 'ubo', 'string', 'beneficial_ownership',
 '{"valid_values": ["Voting Rights", "Right to Appoint Directors", "Significant Influence", "Other"]}', '{}'),

('attr.ubo.is_pep', gen_random_uuid(), 'Is Politically Exposed Person', 'ubo', 'boolean', 'pep', '{}', '{}'),
('attr.ubo.pep_category', gen_random_uuid(), 'PEP Category', 'ubo', 'string', 'pep',
 '{"valid_values": ["Foreign PEP", "Domestic PEP", "International Org PEP", "RCA", "Close Associate"]}', '{}'),

('attr.ubo.layers_to_ubo', gen_random_uuid(), 'Layers to UBO', 'ubo', 'integer', 'beneficial_ownership',
 '{"min": 0, "max": 10}', '{}'),

('attr.ubo.intermediate_entities', gen_random_uuid(), 'Intermediate Entities', 'ubo', 'json', 'beneficial_ownership', '{}', '{}'),

('attr.ubo.nominee_arrangement', gen_random_uuid(), 'Nominee Arrangement', 'ubo', 'boolean', 'beneficial_ownership', '{}', '{}'),
('attr.ubo.nominee_name', gen_random_uuid(), 'Nominee Name', 'ubo', 'string', 'beneficial_ownership', '{}', '{}'),

('attr.ubo.psc_nature_of_control', gen_random_uuid(), 'PSC Nature of Control', 'ubo', 'json', 'uk_psc',
 '{}',
 '{"jurisdictions": ["GB"], "document_types": ["PSC_REGISTER"]}'),

('attr.ubo.ubo_name', gen_random_uuid(), 'UBO Name', 'ubo', 'string', 'beneficial_ownership', '{}', '{}'),
('attr.ubo.shareholder_name', gen_random_uuid(), 'Shareholder Name', 'ubo', 'string', 'beneficial_ownership', '{}', '{}'),
('attr.ubo.ownership_structure', gen_random_uuid(), 'Ownership Structure', 'ubo', 'json', 'beneficial_ownership', '{}', '{}'),
('attr.ubo.psc_name', gen_random_uuid(), 'PSC Name', 'ubo', 'string', 'uk_psc', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.4 Regulatory Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.regulatory.fca_firm_ref', gen_random_uuid(), 'FCA Firm Reference Number', 'compliance', 'string', 'uk_regulatory',
 '{"pattern": "^[0-9]{6}$"}',
 '{"jurisdictions": ["GB"]}'),

('attr.regulatory.fca_permissions', gen_random_uuid(), 'FCA Permissions', 'compliance', 'json', 'uk_regulatory', '{}', '{}'),

('attr.regulatory.sec_file_number', gen_random_uuid(), 'SEC File Number', 'compliance', 'string', 'us_regulatory', '{}', '{}'),
('attr.regulatory.crd_number', gen_random_uuid(), 'CRD Number', 'compliance', 'string', 'us_regulatory', '{}', '{}'),

('attr.regulatory.lei', gen_random_uuid(), 'Legal Entity Identifier', 'compliance', 'string', 'entity_id',
 '{"pattern": "^[A-Z0-9]{4}00[A-Z0-9]{12}[0-9]{2}$"}',
 '{}'),

('attr.regulatory.lei_status', gen_random_uuid(), 'LEI Status', 'compliance', 'string', 'entity_id',
 '{"valid_values": ["ISSUED", "LAPSED", "MERGED", "RETIRED", "ANNULLED", "PENDING_VALIDATION"]}',
 '{}'),

('attr.regulatory.sanctions_match', gen_random_uuid(), 'Sanctions Match Found', 'compliance', 'boolean', 'screening', '{}', '{}'),
('attr.regulatory.sanctions_lists_checked', gen_random_uuid(), 'Sanctions Lists Checked', 'compliance', 'json', 'screening', '{}', '{}'),
('attr.regulatory.screening_date', gen_random_uuid(), 'Screening Date', 'compliance', 'date', 'screening', '{}', '{}'),
('attr.regulatory.screening_provider', gen_random_uuid(), 'Screening Provider', 'compliance', 'string', 'screening', '{}', '{}'),

('attr.regulatory.adverse_media_found', gen_random_uuid(), 'Adverse Media Found', 'compliance', 'boolean', 'screening', '{}', '{}'),
('attr.regulatory.adverse_media_categories', gen_random_uuid(), 'Adverse Media Categories', 'compliance', 'json', 'screening', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.5 Additional Identity/Corporate Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.identity.account_holder_name', gen_random_uuid(), 'Account Holder Name', 'identity', 'string', 'financial', '{}', '{}'),
('attr.identity.taxpayer_name', gen_random_uuid(), 'Taxpayer Name', 'identity', 'string', 'tax', '{}', '{}'),
('attr.identity.tenant_name', gen_random_uuid(), 'Tenant Name', 'identity', 'string', 'tenancy', '{}', '{}'),
('attr.identity.principal_name', gen_random_uuid(), 'Principal Name', 'identity', 'string', 'legal', '{}', '{}'),
('attr.identity.attorney_name', gen_random_uuid(), 'Attorney Name', 'identity', 'string', 'legal', '{}', '{}'),
('attr.identity.father_name', gen_random_uuid(), 'Father Name', 'identity', 'string', 'family', '{}', '{}'),
('attr.identity.mother_name', gen_random_uuid(), 'Mother Name', 'identity', 'string', 'family', '{}', '{}'),
('attr.identity.aadhaar_number', gen_random_uuid(), 'Aadhaar Number', 'identity', 'string', 'india', '{}', '{"jurisdictions": ["IN"]}'),
('attr.corporate.good_standing_date', gen_random_uuid(), 'Good Standing Date', 'corporate', 'date', 'status', '{}', '{}'),
('attr.corporate.resolution_date', gen_random_uuid(), 'Resolution Date', 'corporate', 'date', 'governance', '{}', '{}'),
('attr.corporate.resolution_subject', gen_random_uuid(), 'Resolution Subject', 'corporate', 'string', 'governance', '{}', '{}'),
('attr.corporate.authorised_signatory', gen_random_uuid(), 'Authorised Signatory', 'corporate', 'string', 'governance', '{}', '{}'),
('attr.legal.poa_scope', gen_random_uuid(), 'Power of Attorney Scope', 'legal', 'string', 'authority', '{}', '{}'),
('attr.legal.poa_effective_date', gen_random_uuid(), 'Power of Attorney Effective Date', 'legal', 'date', 'authority', '{}', '{}'),
('attr.legal.poa_expiry_date', gen_random_uuid(), 'Power of Attorney Expiry Date', 'legal', 'date', 'authority', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.6 Financial Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.financial.account_number', gen_random_uuid(), 'Account Number', 'financial', 'string', 'banking', '{}', '{}'),
('attr.financial.sort_code', gen_random_uuid(), 'Sort Code', 'financial', 'string', 'banking', '{}', '{"jurisdictions": ["GB"]}'),
('attr.financial.iban', gen_random_uuid(), 'IBAN', 'financial', 'string', 'banking', '{}', '{}'),
('attr.financial.swift_bic', gen_random_uuid(), 'SWIFT/BIC', 'financial', 'string', 'banking', '{}', '{}'),
('attr.financial.opening_balance', gen_random_uuid(), 'Opening Balance', 'financial', 'currency', 'banking', '{}', '{}'),
('attr.financial.closing_balance', gen_random_uuid(), 'Closing Balance', 'financial', 'currency', 'banking', '{}', '{}'),
('attr.financial.currency', gen_random_uuid(), 'Currency', 'financial', 'string', 'banking', '{}', '{}'),
('attr.financial.bank_name', gen_random_uuid(), 'Bank Name', 'financial', 'string', 'banking', '{}', '{}'),
('attr.financial.total_assets', gen_random_uuid(), 'Total Assets', 'financial', 'currency', 'accounting', '{}', '{}'),
('attr.financial.total_liabilities', gen_random_uuid(), 'Total Liabilities', 'financial', 'currency', 'accounting', '{}', '{}'),
('attr.financial.shareholders_equity', gen_random_uuid(), 'Shareholders Equity', 'financial', 'currency', 'accounting', '{}', '{}'),
('attr.financial.revenue', gen_random_uuid(), 'Revenue', 'financial', 'currency', 'accounting', '{}', '{}'),
('attr.financial.net_income', gen_random_uuid(), 'Net Income', 'financial', 'currency', 'accounting', '{}', '{}'),
('attr.financial.fiscal_year_end', gen_random_uuid(), 'Fiscal Year End', 'financial', 'date', 'accounting', '{}', '{}'),
('attr.financial.wealth_source', gen_random_uuid(), 'Wealth Source', 'financial', 'string', 'sow', '{}', '{}'),
('attr.financial.estimated_net_worth', gen_random_uuid(), 'Estimated Net Worth', 'financial', 'currency', 'sow', '{}', '{}'),
('attr.financial.inheritance_amount', gen_random_uuid(), 'Inheritance Amount', 'financial', 'currency', 'sow', '{}', '{}'),
('attr.financial.business_sale_amount', gen_random_uuid(), 'Business Sale Amount', 'financial', 'currency', 'sow', '{}', '{}'),
('attr.financial.employment_income', gen_random_uuid(), 'Employment Income', 'financial', 'currency', 'sow', '{}', '{}'),
('attr.financial.funds_source', gen_random_uuid(), 'Funds Source', 'financial', 'string', 'sof', '{}', '{}'),
('attr.financial.funds_amount', gen_random_uuid(), 'Funds Amount', 'financial', 'currency', 'sof', '{}', '{}'),
('attr.financial.originating_bank', gen_random_uuid(), 'Originating Bank', 'financial', 'string', 'sof', '{}', '{}'),
('attr.financial.originating_account', gen_random_uuid(), 'Originating Account', 'financial', 'string', 'sof', '{}', '{}'),
('attr.compliance.auditor_name', gen_random_uuid(), 'Auditor Name', 'compliance', 'string', 'audit', '{}', '{}'),
('attr.compliance.audit_opinion', gen_random_uuid(), 'Audit Opinion', 'compliance', 'string', 'audit', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.7 Fund Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.fund.fund_name', gen_random_uuid(), 'Fund Name', 'fund', 'string', 'general', '{}', '{}'),
('attr.fund.fund_type', gen_random_uuid(), 'Fund Type', 'fund', 'string', 'general', '{}', '{}'),
('attr.fund.investment_objective', gen_random_uuid(), 'Investment Objective', 'fund', 'string', 'strategy', '{}', '{}'),
('attr.fund.investment_strategy', gen_random_uuid(), 'Investment Strategy', 'fund', 'string', 'strategy', '{}', '{}'),
('attr.fund.benchmark', gen_random_uuid(), 'Benchmark', 'fund', 'string', 'strategy', '{}', '{}'),
('attr.fund.base_currency', gen_random_uuid(), 'Base Currency', 'fund', 'string', 'general', '{}', '{}'),
('attr.fund.management_company', gen_random_uuid(), 'Management Company', 'fund', 'string', 'governance', '{}', '{}'),
('attr.fund.depositary', gen_random_uuid(), 'Depositary', 'fund', 'string', 'governance', '{}', '{}'),
('attr.fund.isin', gen_random_uuid(), 'ISIN', 'fund', 'string', 'identification', '{}', '{}'),
('attr.fund.general_partner', gen_random_uuid(), 'General Partner', 'fund', 'string', 'governance', '{}', '{}'),
('attr.fund.investment_manager', gen_random_uuid(), 'Investment Manager', 'fund', 'string', 'governance', '{}', '{}'),
('attr.fund.management_fee', gen_random_uuid(), 'Management Fee', 'fund', 'percentage', 'fees', '{}', '{}'),
('attr.fund.performance_fee', gen_random_uuid(), 'Performance Fee', 'fund', 'percentage', 'fees', '{}', '{}'),
('attr.fund.hurdle_rate', gen_random_uuid(), 'Hurdle Rate', 'fund', 'percentage', 'fees', '{}', '{}'),
('attr.fund.lock_up_period', gen_random_uuid(), 'Lock-up Period', 'fund', 'string', 'liquidity', '{}', '{}'),
('attr.fund.minimum_investment', gen_random_uuid(), 'Minimum Investment', 'fund', 'currency', 'subscription', '{}', '{}'),
('attr.fund.investor_name', gen_random_uuid(), 'Investor Name', 'fund', 'string', 'subscription', '{}', '{}'),
('attr.fund.subscription_amount', gen_random_uuid(), 'Subscription Amount', 'fund', 'currency', 'subscription', '{}', '{}'),
('attr.fund.share_class', gen_random_uuid(), 'Share Class', 'fund', 'string', 'subscription', '{}', '{}'),
('attr.fund.investor_type', gen_random_uuid(), 'Investor Type', 'fund', 'string', 'subscription', '{}', '{}'),
('attr.fund.srri', gen_random_uuid(), 'SRRI', 'fund', 'integer', 'risk', '{"min": 1, "max": 7}', '{}'),
('attr.fund.ongoing_charges', gen_random_uuid(), 'Ongoing Charges', 'fund', 'percentage', 'fees', '{}', '{}')
ON CONFLICT (id) DO NOTHING;

-- 2.8 Trust Attributes
INSERT INTO "ob-poc".attribute_registry (id, uuid, display_name, category, value_type, domain, validation_rules, applicability) VALUES
('attr.trust.trust_name', gen_random_uuid(), 'Trust Name', 'trust', 'string', 'general', '{}', '{}'),
('attr.trust.settlement_date', gen_random_uuid(), 'Settlement Date', 'trust', 'date', 'general', '{}', '{}'),
('attr.trust.settlor_name', gen_random_uuid(), 'Settlor Name', 'trust', 'string', 'parties', '{}', '{}'),
('attr.trust.trustee_name', gen_random_uuid(), 'Trustee Name', 'trust', 'string', 'parties', '{}', '{}'),
('attr.trust.beneficiary_class', gen_random_uuid(), 'Beneficiary Class', 'trust', 'string', 'parties', '{}', '{}'),
('attr.trust.governing_law', gen_random_uuid(), 'Governing Law', 'trust', 'string', 'legal', '{}', '{}'),
('attr.trust.trust_type', gen_random_uuid(), 'Trust Type', 'trust', 'string', 'general', '{}', '{}'),
('attr.trust.revocable', gen_random_uuid(), 'Revocable', 'trust', 'boolean', 'general', '{}', '{}'),
('attr.trust.beneficiary_name', gen_random_uuid(), 'Beneficiary Name', 'trust', 'string', 'parties', '{}', '{}'),
('attr.trust.beneficiary_share', gen_random_uuid(), 'Beneficiary Share', 'trust', 'percentage', 'parties', '{}', '{}'),
('attr.trust.beneficiary_conditions', gen_random_uuid(), 'Beneficiary Conditions', 'trust', 'string', 'parties', '{}', '{}'),
('attr.trust.protector_name', gen_random_uuid(), 'Protector Name', 'trust', 'string', 'parties', '{}', '{}')
ON CONFLICT (id) DO NOTHING;


-- ============================================================================
-- SECTION 3: HELPER FUNCTION FOR DOCUMENT-ATTRIBUTE LINKS
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".link_document_to_attribute(
    p_doc_code TEXT,
    p_attr_id TEXT,
    p_direction TEXT DEFAULT 'SOURCE',
    p_extraction_method TEXT DEFAULT 'AI',
    p_is_authoritative BOOLEAN DEFAULT false,
    p_proof_strength TEXT DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_doc_type_id UUID;
    v_attr_uuid UUID;
    v_link_id UUID;
BEGIN
    -- Lookup document type
    SELECT type_id INTO v_doc_type_id
    FROM "ob-poc".document_types
    WHERE type_code = p_doc_code;

    IF v_doc_type_id IS NULL THEN
        RAISE NOTICE 'Document type not found: %', p_doc_code;
        RETURN NULL;
    END IF;

    -- Lookup attribute
    SELECT uuid INTO v_attr_uuid
    FROM "ob-poc".attribute_registry
    WHERE id = p_attr_id;

    IF v_attr_uuid IS NULL THEN
        RAISE NOTICE 'Attribute not found: %', p_attr_id;
        RETURN NULL;
    END IF;

    -- Check if link already exists
    SELECT link_id INTO v_link_id
    FROM "ob-poc".document_attribute_links
    WHERE document_type_id = v_doc_type_id
      AND attribute_id = v_attr_uuid
      AND direction = p_direction;

    IF v_link_id IS NOT NULL THEN
        RETURN v_link_id;
    END IF;

    -- Insert new link
    INSERT INTO "ob-poc".document_attribute_links (
        document_type_id, attribute_id, direction, extraction_method,
        is_authoritative, proof_strength
    ) VALUES (
        v_doc_type_id, v_attr_uuid, p_direction, p_extraction_method,
        p_is_authoritative, p_proof_strength
    ) RETURNING link_id INTO v_link_id;

    RETURN v_link_id;
END;
$$ LANGUAGE plpgsql;


-- ============================================================================
-- SECTION 4: DOCUMENT-ATTRIBUTE LINKS
-- ============================================================================

-- 4.1 Identity Documents
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.passport_number', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.surname', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.given_names', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.date_of_birth', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.nationality', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.sex', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.expiry_date', 'SOURCE', 'MRZ', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.issue_date', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.place_of_birth', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PASSPORT', 'attr.identity.issuing_country', 'SOURCE', 'MRZ', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('NATIONAL_ID', 'attr.identity.document_number', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('NATIONAL_ID', 'attr.identity.surname', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('NATIONAL_ID', 'attr.identity.given_names', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('NATIONAL_ID', 'attr.identity.date_of_birth', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('NATIONAL_ID', 'attr.identity.nationality', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('NATIONAL_ID', 'attr.address.residential_address', 'SOURCE', 'OCR', false, 'SECONDARY');

SELECT "ob-poc".link_document_to_attribute('DRIVERS_LICENSE', 'attr.identity.document_number', 'SOURCE', 'OCR', true, 'SECONDARY');
SELECT "ob-poc".link_document_to_attribute('DRIVERS_LICENSE', 'attr.identity.surname', 'SOURCE', 'OCR', true, 'SECONDARY');
SELECT "ob-poc".link_document_to_attribute('DRIVERS_LICENSE', 'attr.identity.given_names', 'SOURCE', 'OCR', true, 'SECONDARY');
SELECT "ob-poc".link_document_to_attribute('DRIVERS_LICENSE', 'attr.identity.date_of_birth', 'SOURCE', 'OCR', true, 'SECONDARY');
SELECT "ob-poc".link_document_to_attribute('DRIVERS_LICENSE', 'attr.address.residential_address', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('DRIVERS_LICENSE', 'attr.identity.expiry_date', 'SOURCE', 'OCR', false, 'SECONDARY');

SELECT "ob-poc".link_document_to_attribute('BIRTH_CERTIFICATE', 'attr.identity.surname', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BIRTH_CERTIFICATE', 'attr.identity.given_names', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BIRTH_CERTIFICATE', 'attr.identity.date_of_birth', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BIRTH_CERTIFICATE', 'attr.identity.place_of_birth', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BIRTH_CERTIFICATE', 'attr.identity.father_name', 'SOURCE', 'OCR', false, 'SUPPORTING');
SELECT "ob-poc".link_document_to_attribute('BIRTH_CERTIFICATE', 'attr.identity.mother_name', 'SOURCE', 'OCR', false, 'SUPPORTING');

-- 4.2 Corporate Documents
SELECT "ob-poc".link_document_to_attribute('CERT_OF_INCORPORATION', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_INCORPORATION', 'attr.corporate.registration_number', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_INCORPORATION', 'attr.corporate.incorporation_date', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_INCORPORATION', 'attr.corporate.jurisdiction', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_INCORPORATION', 'attr.corporate.company_type', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_INCORPORATION', 'attr.address.registered_office', 'SOURCE', 'OCR', false, 'SECONDARY');

SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_INCORPORATION', 'attr.corporate.legal_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_INCORPORATION', 'attr.corporate.authorized_shares', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_INCORPORATION', 'attr.corporate.share_classes', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_INCORPORATION', 'attr.corporate.par_value', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_INCORPORATION', 'attr.corporate.business_purpose', 'SOURCE', 'AI', false, 'SUPPORTING');

SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_ASSOCIATION', 'attr.corporate.share_classes', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_ASSOCIATION', 'attr.corporate.voting_rights', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_ASSOCIATION', 'attr.corporate.transfer_restrictions', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('ARTICLES_OF_ASSOCIATION', 'attr.corporate.preemption_rights', 'SOURCE', 'AI', false, 'SUPPORTING');

SELECT "ob-poc".link_document_to_attribute('CERT_OF_GOOD_STANDING', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_GOOD_STANDING', 'attr.corporate.registration_number', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_GOOD_STANDING', 'attr.corporate.status', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CERT_OF_GOOD_STANDING', 'attr.corporate.good_standing_date', 'SOURCE', 'OCR', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_SHAREHOLDERS', 'attr.ubo.shareholder_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_SHAREHOLDERS', 'attr.ubo.ownership_percentage', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_SHAREHOLDERS', 'attr.corporate.share_class', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_SHAREHOLDERS', 'attr.corporate.number_of_shares', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_DIRECTORS', 'attr.corporate.director_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_DIRECTORS', 'attr.corporate.director_appointment_date', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_DIRECTORS', 'attr.corporate.director_address', 'SOURCE', 'AI', false, 'SECONDARY');
SELECT "ob-poc".link_document_to_attribute('REGISTER_OF_DIRECTORS', 'attr.corporate.director_nationality', 'SOURCE', 'AI', false, 'SUPPORTING');

SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.corporate.legal_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.corporate.registration_number', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.corporate.incorporation_date', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.address.registered_office', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.corporate.share_capital', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.corporate.director_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('REGISTRY_EXTRACT', 'attr.corporate.status', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('BOARD_RESOLUTION', 'attr.corporate.resolution_date', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BOARD_RESOLUTION', 'attr.corporate.resolution_subject', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BOARD_RESOLUTION', 'attr.corporate.authorised_signatory', 'SOURCE', 'AI', true, 'PRIMARY');

-- 4.3 Tax Documents
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.identity.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.identity.date_of_birth', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.tax.country_of_citizenship', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.address.permanent_residence', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.tax.ftin', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.tax.us_tin', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.tax.treaty_country', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.tax.treaty_article', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN', 'attr.tax.treaty_withholding_rate', 'SOURCE', 'OCR', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.country_of_incorporation', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.chapter3_status', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.chapter4_status', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.address.permanent_residence', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.us_tin', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.giin', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.treaty_country', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W8_BEN_E', 'attr.tax.lob_provision', 'SOURCE', 'OCR', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('W9', 'attr.identity.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W9', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W9', 'attr.tax.us_tin', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('W9', 'attr.tax.tax_classification', 'SOURCE', 'OCR', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('FATCA_SELF_CERT', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FATCA_SELF_CERT', 'attr.tax.giin', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FATCA_SELF_CERT', 'attr.tax.fatca_status', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FATCA_SELF_CERT', 'attr.tax.us_indicia', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('CRS_SELF_CERT', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CRS_SELF_CERT', 'attr.tax.tax_residence_countries', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CRS_SELF_CERT', 'attr.tax.crs_entity_type', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('CRS_SELF_CERT', 'attr.tax.controlling_persons', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('TAX_RESIDENCY_CERT', 'attr.identity.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TAX_RESIDENCY_CERT', 'attr.corporate.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TAX_RESIDENCY_CERT', 'attr.tax.tax_residence_country', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TAX_RESIDENCY_CERT', 'attr.tax.certificate_date', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TAX_RESIDENCY_CERT', 'attr.tax.tax_year', 'SOURCE', 'OCR', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('P60', 'attr.identity.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('P60', 'attr.tax.ni_number', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('P60', 'attr.tax.paye_reference', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('P60', 'attr.tax.gross_pay_ytd', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('P60', 'attr.tax.tax_deducted_ytd', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('P60', 'attr.tax.ni_contributions_ytd', 'SOURCE', 'OCR', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('P60', 'attr.tax.tax_year', 'SOURCE', 'OCR', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('SA302', 'attr.identity.legal_name', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SA302', 'attr.tax.utr', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SA302', 'attr.tax.total_income', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SA302', 'attr.tax.total_tax_due', 'SOURCE', 'OCR', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SA302', 'attr.tax.tax_year', 'SOURCE', 'OCR', true, 'PRIMARY');

-- 4.4 Financial Documents
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.identity.account_holder_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.account_number', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.sort_code', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.iban', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.swift_bic', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.address.statement_date', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.opening_balance', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.closing_balance', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.currency', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.address.residential_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('BANK_STATEMENT', 'attr.financial.bank_name', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.corporate.legal_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.financial.total_assets', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.financial.total_liabilities', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.financial.shareholders_equity', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.financial.revenue', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.financial.net_income', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.financial.fiscal_year_end', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.compliance.auditor_name', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('AUDITED_ACCOUNTS', 'attr.compliance.audit_opinion', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_WEALTH', 'attr.financial.wealth_source', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_WEALTH', 'attr.financial.estimated_net_worth', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_WEALTH', 'attr.financial.inheritance_amount', 'SOURCE', 'AI', false, 'SUPPORTING');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_WEALTH', 'attr.financial.business_sale_amount', 'SOURCE', 'AI', false, 'SUPPORTING');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_WEALTH', 'attr.financial.employment_income', 'SOURCE', 'AI', false, 'SUPPORTING');

SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_FUNDS', 'attr.financial.funds_source', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_FUNDS', 'attr.financial.funds_amount', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_FUNDS', 'attr.financial.originating_bank', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SOURCE_OF_FUNDS', 'attr.financial.originating_account', 'SOURCE', 'AI', false, 'SUPPORTING');

-- 4.5 Address Proof Documents
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_ELECTRIC', 'attr.identity.account_holder_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_ELECTRIC', 'attr.address.service_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_ELECTRIC', 'attr.address.statement_date', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_ELECTRIC', 'attr.address.utility_account_number', 'SOURCE', 'AI', false, 'SUPPORTING');

SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_GAS', 'attr.identity.account_holder_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_GAS', 'attr.address.service_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_GAS', 'attr.address.statement_date', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_WATER', 'attr.identity.account_holder_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_WATER', 'attr.address.service_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UTILITY_BILL_WATER', 'attr.address.statement_date', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('COUNCIL_TAX_BILL', 'attr.identity.taxpayer_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('COUNCIL_TAX_BILL', 'attr.address.property_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('COUNCIL_TAX_BILL', 'attr.address.council_tax_band', 'SOURCE', 'AI', false, 'SUPPORTING');
SELECT "ob-poc".link_document_to_attribute('COUNCIL_TAX_BILL', 'attr.address.council_tax_reference', 'SOURCE', 'AI', false, 'SUPPORTING');

SELECT "ob-poc".link_document_to_attribute('TENANCY_AGREEMENT', 'attr.identity.tenant_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TENANCY_AGREEMENT', 'attr.address.property_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TENANCY_AGREEMENT', 'attr.address.landlord_name', 'SOURCE', 'AI', false, 'SUPPORTING');
SELECT "ob-poc".link_document_to_attribute('TENANCY_AGREEMENT', 'attr.address.tenancy_start_date', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TENANCY_AGREEMENT', 'attr.address.tenancy_end_date', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TENANCY_AGREEMENT', 'attr.address.monthly_rent', 'SOURCE', 'AI', false, 'SUPPORTING');

-- 4.6 UBO Documents
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.ubo.ubo_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.ubo.ownership_percentage', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.ubo.ownership_type', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.ubo.control_type', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.identity.date_of_birth', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.identity.nationality', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.address.residential_address', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('UBO_DECLARATION', 'attr.ubo.is_pep', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('OWNERSHIP_CHART', 'attr.ubo.ownership_structure', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OWNERSHIP_CHART', 'attr.ubo.layers_to_ubo', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OWNERSHIP_CHART', 'attr.ubo.intermediate_entities', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('PSC_REGISTER', 'attr.ubo.psc_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PSC_REGISTER', 'attr.ubo.psc_nature_of_control', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PSC_REGISTER', 'attr.identity.date_of_birth', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PSC_REGISTER', 'attr.identity.nationality', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('PSC_REGISTER', 'attr.address.service_address', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('POWER_OF_ATTORNEY', 'attr.identity.principal_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('POWER_OF_ATTORNEY', 'attr.identity.attorney_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('POWER_OF_ATTORNEY', 'attr.legal.poa_scope', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('POWER_OF_ATTORNEY', 'attr.legal.poa_effective_date', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('POWER_OF_ATTORNEY', 'attr.legal.poa_expiry_date', 'SOURCE', 'AI', false, 'PRIMARY');

-- 4.7 Fund Documents
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.fund_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.fund_type', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.investment_objective', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.investment_strategy', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.benchmark', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.base_currency', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.management_company', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.depositary', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('FUND_PROSPECTUS', 'attr.fund.isin', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.fund_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.general_partner', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.investment_manager', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.management_fee', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.performance_fee', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.hurdle_rate', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.lock_up_period', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('OFFERING_MEMORANDUM', 'attr.fund.minimum_investment', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('SUBSCRIPTION_AGREEMENT', 'attr.fund.investor_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SUBSCRIPTION_AGREEMENT', 'attr.fund.subscription_amount', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SUBSCRIPTION_AGREEMENT', 'attr.fund.share_class', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SUBSCRIPTION_AGREEMENT', 'attr.fund.investor_type', 'SOURCE', 'AI', true, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('KIID', 'attr.fund.fund_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('KIID', 'attr.fund.srri', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('KIID', 'attr.fund.ongoing_charges', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('KIID', 'attr.fund.isin', 'SOURCE', 'AI', true, 'PRIMARY');

-- 4.8 Trust Documents
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.trust_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.settlement_date', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.settlor_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.trustee_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.beneficiary_class', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.governing_law', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.trust_type', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('TRUST_DEED', 'attr.trust.revocable', 'SOURCE', 'AI', false, 'PRIMARY');

SELECT "ob-poc".link_document_to_attribute('SCHEDULE_OF_BENEFICIARIES', 'attr.trust.beneficiary_name', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SCHEDULE_OF_BENEFICIARIES', 'attr.trust.beneficiary_class', 'SOURCE', 'AI', true, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SCHEDULE_OF_BENEFICIARIES', 'attr.trust.beneficiary_share', 'SOURCE', 'AI', false, 'PRIMARY');
SELECT "ob-poc".link_document_to_attribute('SCHEDULE_OF_BENEFICIARIES', 'attr.trust.beneficiary_conditions', 'SOURCE', 'AI', false, 'SUPPORTING');


-- ============================================================================
-- SECTION 5: REQUIRED ATTRIBUTES JSONB UPDATES
-- ============================================================================

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.identity.passport_number",
    "attr.identity.surname",
    "attr.identity.given_names",
    "attr.identity.date_of_birth",
    "attr.identity.nationality",
    "attr.identity.expiry_date"
  ],
  "optional": [
    "attr.identity.place_of_birth",
    "attr.identity.sex",
    "attr.identity.issue_date",
    "attr.identity.issuing_country"
  ],
  "extraction_zones": {
    "mrz": {"page": 1, "region": "bottom", "method": "MRZ"},
    "photo": {"page": 1, "region": "top-left", "method": "IMAGE"}
  },
  "validity_rules": {
    "expiry_check": true,
    "min_validity_days": 180
  }
}'::jsonb
WHERE type_code = 'PASSPORT';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.corporate.legal_name",
    "attr.corporate.registration_number",
    "attr.corporate.incorporation_date",
    "attr.corporate.jurisdiction"
  ],
  "optional": [
    "attr.address.registered_office",
    "attr.corporate.company_type",
    "attr.corporate.share_capital"
  ],
  "validity_rules": {
    "certified_copy_required": true
  }
}'::jsonb
WHERE type_code = 'CERT_OF_INCORPORATION';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.corporate.legal_name",
    "attr.tax.country_of_incorporation",
    "attr.tax.chapter3_status",
    "attr.address.permanent_residence"
  ],
  "conditional": {
    "attr.tax.giin": {"if": "chapter4_status IN (''Participating FFI'', ''Reporting Model 1 FFI'', ''Reporting Model 2 FFI'')"},
    "attr.tax.us_tin": {"if": "claiming_treaty_benefits = true"},
    "attr.tax.treaty_country": {"if": "claiming_treaty_benefits = true"}
  },
  "optional": [
    "attr.tax.chapter4_status",
    "attr.tax.lob_provision",
    "attr.tax.treaty_article",
    "attr.tax.treaty_withholding_rate"
  ],
  "validity_rules": {
    "max_age_years": 3,
    "signature_required": true
  }
}'::jsonb
WHERE type_code = 'W8_BEN_E';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.identity.account_holder_name",
    "attr.financial.account_number",
    "attr.address.statement_date",
    "attr.address.residential_address"
  ],
  "optional": [
    "attr.financial.sort_code",
    "attr.financial.iban",
    "attr.financial.swift_bic",
    "attr.financial.opening_balance",
    "attr.financial.closing_balance",
    "attr.financial.currency",
    "attr.financial.bank_name"
  ],
  "validity_rules": {
    "max_age_days": 90
  }
}'::jsonb
WHERE type_code = 'BANK_STATEMENT';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.ubo.ubo_name",
    "attr.ubo.ownership_percentage",
    "attr.identity.date_of_birth",
    "attr.identity.nationality",
    "attr.address.residential_address"
  ],
  "optional": [
    "attr.ubo.ownership_type",
    "attr.ubo.control_type",
    "attr.ubo.is_pep",
    "attr.ubo.pep_category"
  ],
  "validity_rules": {
    "signature_required": true,
    "certification_required": true
  }
}'::jsonb
WHERE type_code = 'UBO_DECLARATION';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.trust.trust_name",
    "attr.trust.settlement_date",
    "attr.trust.settlor_name",
    "attr.trust.trustee_name",
    "attr.trust.governing_law"
  ],
  "optional": [
    "attr.trust.beneficiary_class",
    "attr.trust.trust_type",
    "attr.trust.revocable",
    "attr.trust.protector_name"
  ],
  "validity_rules": {
    "original_or_certified": true,
    "amendments_required": true
  }
}'::jsonb
WHERE type_code = 'TRUST_DEED';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.fund.fund_name",
    "attr.fund.fund_type",
    "attr.fund.investment_objective",
    "attr.fund.management_company",
    "attr.fund.base_currency"
  ],
  "optional": [
    "attr.fund.investment_strategy",
    "attr.fund.benchmark",
    "attr.fund.depositary",
    "attr.fund.isin",
    "attr.fund.management_fee"
  ],
  "validity_rules": {
    "regulatory_approval_required": true
  }
}'::jsonb
WHERE type_code = 'FUND_PROSPECTUS';

UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.corporate.legal_name",
    "attr.financial.fiscal_year_end",
    "attr.compliance.audit_opinion"
  ],
  "optional": [
    "attr.financial.total_assets",
    "attr.financial.total_liabilities",
    "attr.financial.shareholders_equity",
    "attr.financial.revenue",
    "attr.financial.net_income",
    "attr.compliance.auditor_name"
  ],
  "validity_rules": {
    "max_age_months": 18,
    "signed_audit_report_required": true
  }
}'::jsonb
WHERE type_code = 'AUDITED_ACCOUNTS';

-- Apply to all utility bills
UPDATE "ob-poc".document_types
SET required_attributes = '{
  "required": [
    "attr.identity.account_holder_name",
    "attr.address.service_address",
    "attr.address.statement_date"
  ],
  "optional": [
    "attr.address.utility_account_number"
  ],
  "validity_rules": {
    "max_age_days": 90
  }
}'::jsonb
WHERE type_code IN ('UTILITY_BILL_ELECTRIC', 'UTILITY_BILL_GAS', 'UTILITY_BILL_WATER');


-- ============================================================================
-- SECTION 6: EXTRACTION JOBS TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".extraction_jobs (
    job_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),
    entity_id UUID NOT NULL,
    status VARCHAR(20) DEFAULT 'PENDING',
    priority VARCHAR(10) DEFAULT 'normal',
    attempts INT DEFAULT 0,
    max_attempts INT DEFAULT 3,
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    CONSTRAINT check_status CHECK (status IN ('PENDING', 'RUNNING', 'COMPLETED', 'FAILED'))
);

CREATE INDEX IF NOT EXISTS idx_extraction_jobs_pending
ON "ob-poc".extraction_jobs(status, priority, created_at)
WHERE status = 'PENDING';


-- ============================================================================
-- SECTION 7: ATTRIBUTE LINEAGE VIEW
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_attribute_lineage_summary AS
SELECT
    ar.id as attribute_id,
    ar.display_name,
    ar.category,
    COUNT(DISTINCT CASE WHEN dal.direction IN ('SOURCE', 'BOTH') THEN dal.document_type_id END) as source_count,
    COUNT(DISTINCT CASE WHEN dal.direction IN ('SINK', 'BOTH') THEN dal.document_type_id END) as sink_count,
    COUNT(DISTINCT rar.resource_id) as resource_count,
    ar.requires_authoritative_source,
    BOOL_OR(dal.is_authoritative) as has_authoritative_source
FROM "ob-poc".attribute_registry ar
LEFT JOIN "ob-poc".document_attribute_links dal ON dal.attribute_id = ar.uuid
LEFT JOIN "ob-poc".resource_attribute_requirements rar ON rar.attribute_id = ar.uuid
GROUP BY ar.id, ar.display_name, ar.category, ar.requires_authoritative_source;


-- ============================================================================
-- END OF MIGRATION
-- ============================================================================
