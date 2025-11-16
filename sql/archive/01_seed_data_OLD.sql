-- ============================================================================
-- OB-POC Seed Data
-- ============================================================================
-- This script populates essential reference data for the ob-poc schema.
-- Run this after creating the schema with 00_master_schema.sql
--
-- Contents:
--   1. Dictionary Attributes (comprehensive attribute definitions)
--   2. Reference CBUs (sample Client Business Units)
--   3. Entity Types and Roles
-- ============================================================================

SET search_path TO "ob-poc";

-- ============================================================================
-- 1. DICTIONARY ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at) VALUES

-- Core Onboarding Attributes (referenced by DSL generation)
('123e4567-e89b-12d3-a456-426614174001', 'onboard.cbu_id', 'Client Business Unit identifier', 'Onboarding', 'string', 'Onboarding', '', '{"type": "generated", "pattern": "CBU-[0-9]{4}-[0-9]{3}", "required": true}', '{"type": "database", "table": "onboarding_cases"}', NOW(), NOW()),
('123e4567-e89b-12d3-a456-426614174002', 'onboard.nature_purpose', 'Nature and purpose of the business relationship', 'Onboarding', 'string', 'Onboarding', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "onboarding_cases"}', NOW(), NOW()),
('123e4567-e89b-12d3-a456-426614174003', 'onboard.status', 'Current onboarding status', 'Onboarding', 'enum', 'Onboarding', '', '{"type": "derived", "values": ["PENDING", "IN_PROGRESS", "APPROVED", "REJECTED"]}', '{"type": "database", "table": "onboarding_cases"}', NOW(), NOW()),

-- Entity Information (skip entity.legal_name as it already exists)
('987fcdeb-51a2-43f7-8765-ba9876543202', 'entity.type', 'Type of entity (proper person, corporate, trust, etc.)', 'Entity', 'enum', 'Legal', '', '{"type": "manual", "required": true, "values": ["PROPER_PERSON", "CORPORATE", "TRUST", "PARTNERSHIP"]}', '{"type": "database", "table": "entities"}', NOW(), NOW()),
-- Skip entity.domicile as it already exists
('987fcdeb-51a2-43f7-8765-ba9876543204', 'entity.incorporation_date', 'Date of incorporation', 'Entity', 'date', 'Legal', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "entities"}', NOW(), NOW()),
('987fcdeb-51a2-43f7-8765-ba9876543205', 'entity.registration_number', 'Official registration number', 'Entity', 'string', 'Legal', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "entities"}', NOW(), NOW()),

-- Proper Person KYC Attributes
('456789ab-cdef-1234-5678-9abcdef01201', 'kyc.proper_person.net_worth', 'Proper Person net worth', 'KYC', 'decimal', 'KYC', '', '{"type": "manual", "required": true, "validation": "positive_number"}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01202', 'kyc.proper_person.annual_income', 'Annual income of individual', 'KYC', 'decimal', 'KYC', '', '{"type": "manual", "required": true, "validation": "positive_number"}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01203', 'kyc.proper_person.source_of_wealth', 'Source of wealth description', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01204', 'kyc.proper_person.source_of_funds', 'Source of funds for investment', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01205', 'kyc.proper_person.occupation', 'Current occupation', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01206', 'kyc.proper_person.date_of_birth', 'Date of birth', 'KYC', 'date', 'KYC', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01207', 'kyc.proper_person.nationality', 'Nationality', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01208', 'kyc.proper_person.passport_number', 'Passport number', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": false}', '{"type": "database", "table": "kyc_proper_person"}', NOW(), NOW()),

-- Corporate KYC Attributes
('789abcde-f012-3456-7890-abcdef123401', 'kyc.corporate.business_activity', 'Primary business activity', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "kyc_corporate"}', NOW(), NOW()),
('789abcde-f012-3456-7890-abcdef123402', 'kyc.corporate.regulatory_status', 'Regulatory status or licensing', 'KYC', 'string', 'KYC', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "kyc_corporate"}', NOW(), NOW()),
('789abcde-f012-3456-7890-abcdef123403', 'kyc.corporate.aum', 'Assets under management', 'KYC', 'decimal', 'KYC', '', '{"type": "manual", "required": false, "validation": "positive_number"}', '{"type": "database", "table": "kyc_corporate"}', NOW(), NOW()),
('789abcde-f012-3456-7890-abcdef123404', 'kyc.corporate.employees_count', 'Number of employees', 'KYC', 'integer', 'KYC', '', '{"type": "manual", "required": false, "validation": "positive_integer"}', '{"type": "database", "table": "kyc_corporate"}', NOW(), NOW()),

-- Document Attributes
('abcdef12-3456-7890-abcd-ef1234567801', 'document.certificate_of_incorporation', 'Certificate of incorporation document', 'Documents', 'string', 'Legal', '', '{"type": "upload", "required": true, "format": "PDF"}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567802', 'document.articles_of_association', 'Articles of association document', 'Documents', 'string', 'Legal', '', '{"type": "upload", "required": true, "format": "PDF"}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567803', 'document.board_resolution', 'Board resolution for account opening', 'Documents', 'string', 'Legal', '', '{"type": "upload", "required": true, "format": "PDF"}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567804', 'document.passport_copy', 'Copy of passport', 'Documents', 'string', 'Identity', '', '{"type": "upload", "required": true, "format": "PDF"}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567805', 'document.proof_of_address', 'Proof of address document', 'Documents', 'string', 'Identity', '', '{"type": "upload", "required": true, "format": "PDF"}', '{"type": "storage", "service": "document_storage"}', NOW(), NOW()),

-- Fund Attributes
('fedcba98-7654-3210-fedc-ba9876543201', 'fund.name', 'Name of the fund', 'Fund', 'string', 'Investment', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "funds"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543202', 'fund.strategy', 'Investment strategy', 'Fund', 'string', 'Investment', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "funds"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543203', 'fund.base_currency', 'Base currency of the fund', 'Fund', 'string', 'Investment', '', '{"type": "manual", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "funds"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543204', 'fund.minimum_investment', 'Minimum investment amount', 'Fund', 'decimal', 'Investment', '', '{"type": "manual", "required": true, "validation": "positive_number"}', '{"type": "database", "table": "funds"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543205', 'fund.management_fee', 'Management fee percentage', 'Fund', 'decimal', 'Investment', '', '{"type": "manual", "required": true, "validation": "percentage"}', '{"type": "database", "table": "funds"}', NOW(), NOW()),

-- Investment Attributes
('13579bdf-2468-ace0-1357-9bdf2468ace0', 'investment.subscription_amount', 'Subscription amount', 'Investment', 'decimal', 'Investment', '', '{"type": "manual", "required": true, "validation": "positive_number"}', '{"type": "database", "table": "investments"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468ace1', 'investment.subscription_currency', 'Subscription currency', 'Investment', 'string', 'Investment', '', '{"type": "manual", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "investments"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468ace2', 'investment.subscription_date', 'Subscription date', 'Investment', 'date', 'Investment', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "investments"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468ace3', 'investment.redemption_notice_period', 'Redemption notice period in days', 'Investment', 'integer', 'Investment', '', '{"type": "manual", "required": true, "validation": "positive_integer"}', '{"type": "database", "table": "investments"}', NOW(), NOW()),

-- Risk Attributes
('24680ace-1357-9bdf-2468-0ace13579bdf', 'risk.profile', 'Risk profile assessment', 'Risk', 'enum', 'Risk', '', '{"type": "derived", "values": ["CONSERVATIVE", "MODERATE", "AGGRESSIVE", "SPECULATIVE"]}', '{"type": "database", "table": "risk_profiles"}', NOW(), NOW()),
('24680ace-1357-9bdf-2468-0ace13579bd0', 'risk.tolerance', 'Risk tolerance score', 'Risk', 'integer', 'Risk', '', '{"type": "calculation", "formula": "risk_assessment_score", "range": [1, 10]}', '{"type": "database", "table": "risk_profiles"}', NOW(), NOW()),
('24680ace-1357-9bdf-2468-0ace13579bd1', 'risk.investment_experience', 'Years of investment experience', 'Risk', 'integer', 'Risk', '', '{"type": "manual", "required": true, "validation": "non_negative_integer"}', '{"type": "database", "table": "risk_profiles"}', NOW(), NOW()),
('24680ace-1357-9bdf-2468-0ace13579bd2', 'risk.previous_losses', 'Previous investment losses percentage', 'Risk', 'decimal', 'Risk', '', '{"type": "manual", "required": false, "validation": "percentage"}', '{"type": "database", "table": "risk_profiles"}', NOW(), NOW()),

-- Custody Attributes
('456789ab-cdef-1234-5678-9abcdef01301', 'custody.account_number', 'Custody account number', 'Custody', 'string', 'Custody', '', '{"type": "generated", "pattern": "CUST-[A-Z0-9]{3}-[0-9]{3}", "required": true}', '{"type": "database", "table": "custody_accounts"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01302', 'custody.custodian_name', 'Name of the custodian', 'Custody', 'string', 'Custody', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "custody_accounts"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01303', 'custody.account_type', 'Type of custody account', 'Custody', 'enum', 'Custody', '', '{"type": "manual", "required": true, "values": ["SEGREGATED", "OMNIBUS", "PRIME_BROKERAGE"]}', '{"type": "database", "table": "custody_accounts"}', NOW(), NOW()),

-- Fund Accounting Attributes
('456789ab-cdef-1234-5678-9abcdef01401', 'accounting.fund_code', 'Fund accounting code', 'Accounting', 'string', 'Accounting', '', '{"type": "generated", "pattern": "FA-[A-Z0-9]{4}-[A-Z]{2}-[0-9]{3}", "required": true}', '{"type": "database", "table": "fund_accounting"}', NOW(), NOW()),
('456789ab-cdef-1234-5678-9abcdef01402', 'accounting.nav_value', 'Net Asset Value', 'Accounting', 'decimal', 'Accounting', '', '{"type": "calculated", "required": true, "validation": "positive_number"}', '{"type": "database", "table": "fund_accounting"}', NOW(), NOW()),
('789abcde-f012-3456-7890-abcdef123502', 'accounting.nav_frequency', 'NAV calculation frequency', 'FundAccounting', 'enum', 'Accounting', '', '{"type": "manual", "required": true, "values": ["DAILY", "WEEKLY", "MONTHLY", "QUARTERLY"]}', '{"type": "database", "table": "nav_calculations"}', NOW(), NOW()),
('789abcde-f012-3456-7890-abcdef123503', 'accounting.valuation_point', 'Valuation point time', 'FundAccounting', 'string', 'Accounting', '', '{"type": "manual", "required": true, "format": "HH:MM"}', '{"type": "database", "table": "nav_calculations"}', NOW(), NOW()),
('789abcde-f012-3456-7890-abcdef123504', 'accounting.base_currency', 'Base currency for accounting', 'FundAccounting', 'string', 'Accounting', '', '{"type": "manual", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "fund_accounting"}', NOW(), NOW()),

-- Compliance Attributes
('abcdef12-3456-7890-abcd-ef1234567901', 'compliance.fatca_status', 'FATCA status', 'Compliance', 'enum', 'Compliance', '', '{"type": "manual", "required": true, "values": ["COMPLIANT", "NON_COMPLIANT", "EXEMPT"]}', '{"type": "database", "table": "compliance"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567902', 'compliance.crs_status', 'Common Reporting Standard status', 'Compliance', 'enum', 'Compliance', '', '{"type": "manual", "required": true, "values": ["COMPLIANT", "NON_COMPLIANT", "EXEMPT"]}', '{"type": "database", "table": "compliance"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567903', 'compliance.aml_status', 'Anti-Money Laundering status', 'Compliance', 'enum', 'Compliance', '', '{"type": "derived", "values": ["PASSED", "FAILED", "PENDING", "REQUIRES_REVIEW"]}', '{"type": "database", "table": "compliance"}', NOW(), NOW()),
('abcdef12-3456-7890-abcd-ef1234567904', 'compliance.sanctions_check', 'Sanctions screening result', 'Compliance', 'enum', 'Compliance', '', '{"type": "derived", "values": ["CLEAR", "HIT", "PENDING"]}', '{"type": "database", "table": "compliance"}', NOW(), NOW()),

-- Contact Information
('fedcba98-7654-3210-fedc-ba9876543301', 'contact.email', 'Primary email address', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": true, "format": "email"}', '{"type": "database", "table": "contacts"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543302', 'contact.phone', 'Primary phone number', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": true, "format": "E.164"}', '{"type": "database", "table": "contacts"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543303', 'contact.address_line1', 'Address line 1', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "addresses"}', NOW(), NOW()),

-- Resource Management Attributes
('24681357-9bdf-ace0-2468-13579bdfabc1', 'resource.custody_account_id', 'Custody account resource identifier', 'Resources', 'string', 'Resources', '', '{"type": "generated", "pattern": "CUST-[A-Z0-9]{8}", "required": true}', '{"type": "database", "table": "resources"}', NOW(), NOW()),
('24681357-9bdf-ace0-2468-13579bdfabc2', 'resource.fund_accounting_id', 'Fund accounting resource identifier', 'Resources', 'string', 'Resources', '', '{"type": "generated", "pattern": "FA-[A-Z0-9]{8}", "required": true}', '{"type": "database", "table": "resources"}', NOW(), NOW()),
('24681357-9bdf-ace0-2468-13579bdfabc3', 'resource.transfer_agency_id', 'Transfer agency resource identifier', 'Resources', 'string', 'Resources', '', '{"type": "generated", "pattern": "TA-[A-Z0-9]{8}", "required": true}', '{"type": "database", "table": "resources"}', NOW(), NOW()),
('24681357-9bdf-ace0-2468-13579bdfabc4', 'resource.risk_system_id', 'Risk management system identifier', 'Resources', 'string', 'Resources', '', '{"type": "generated", "pattern": "RISK-[A-Z0-9]{8}", "required": true}', '{"type": "database", "table": "resources"}', NOW(), NOW()),

-- Transfer Agency Attributes
('13579bdf-2468-ace0-1357-9bdf2468abc1', 'transfer_agency.fund_identifier', 'Transfer agency fund identifier', 'TransferAgency', 'string', 'TransferAgency', '', '{"type": "generated", "pattern": "TA-[A-Z0-9]{4}-[A-Z]{2}", "required": true}', '{"type": "database", "table": "transfer_agency"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468abc2', 'transfer_agency.share_class', 'Share class designation', 'TransferAgency', 'string', 'TransferAgency', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "transfer_agency"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543304', 'contact.address_line2', 'Address line 2', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": false}', '{"type": "database", "table": "addresses"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543305', 'contact.city', 'City', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "addresses"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543306', 'contact.postal_code', 'Postal code', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "addresses"}', NOW(), NOW()),
('fedcba98-7654-3210-fedc-ba9876543307', 'contact.country', 'Country', 'Contact', 'string', 'Contact', '', '{"type": "manual", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "addresses"}', NOW(), NOW()),

-- Tax Information
('13579bdf-2468-ace0-1357-9bdf2468ace4', 'tax.tin', 'Tax Identification Number', 'Tax', 'string', 'Tax', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "tax_info"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468ace5', 'tax.jurisdiction', 'Tax jurisdiction', 'Tax', 'string', 'Tax', '', '{"type": "manual", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "tax_info"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468ace6', 'tax.treaty_benefits', 'Tax treaty benefits eligibility', 'Tax', 'boolean', 'Tax', '', '{"type": "manual", "required": false}', '{"type": "database", "table": "tax_info"}', NOW(), NOW()),
('13579bdf-2468-ace0-1357-9bdf2468ace7', 'tax.withholding_rate', 'Applicable withholding tax rate', 'Tax', 'decimal', 'Tax', '', '{"type": "calculation", "formula": "jurisdiction_rate", "validation": "percentage"}', '{"type": "database", "table": "tax_info"}', NOW(), NOW()),

-- Securities Attributes (skip security.isin as it already exists)
('24680ace-1357-9bdf-2468-0ace13579bd4', 'security.name', 'Security name', 'Security', 'string', 'Security', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "securities"}', NOW(), NOW()),
('24680ace-1357-9bdf-2468-0ace13579bd5', 'security.type', 'Security type', 'Security', 'enum', 'Security', '', '{"type": "manual", "required": true, "values": ["EQUITY", "BOND", "DERIVATIVE", "FUND", "ETF"]}', '{"type": "database", "table": "securities"}', NOW(), NOW()),

-- Banking Attributes
('35791bdf-4680-ace2-3579-1bdf4680ace2', 'banking.account_number', 'Bank account number', 'Banking', 'string', 'Banking', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "bank_accounts"}', NOW(), NOW()),
('35791bdf-4680-ace2-3579-1bdf4680ace3', 'banking.iban', 'International Bank Account Number', 'Banking', 'string', 'Banking', '', '{"type": "manual", "required": false, "format": "IBAN"}', '{"type": "database", "table": "bank_accounts"}', NOW(), NOW()),
('35791bdf-4680-ace2-3579-1bdf4680ace4', 'banking.swift_code', 'SWIFT/BIC code', 'Banking', 'string', 'Banking', '', '{"type": "manual", "required": true, "format": "SWIFT"}', '{"type": "database", "table": "bank_accounts"}', NOW(), NOW()),
('35791bdf-4680-ace2-3579-1bdf4680ace5', 'banking.bank_name', 'Bank name', 'Banking', 'string', 'Banking', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "bank_accounts"}', NOW(), NOW()),

-- Hedge Fund Specific Attributes
('46802ace-5791-bdf4-6802-ace57913579b', 'hedge_fund.high_water_mark', 'High water mark for performance fees', 'HedgeFund', 'decimal', 'Investment', '', '{"type": "calculation", "formula": "max_nav_value"}', '{"type": "database", "table": "hedge_fund_metrics"}', NOW(), NOW()),
('46802ace-5791-bdf4-6802-ace57913579c', 'hedge_fund.performance_fee', 'Performance fee percentage', 'HedgeFund', 'decimal', 'Investment', '', '{"type": "manual", "required": true, "validation": "percentage"}', '{"type": "database", "table": "hedge_fund_terms"}', NOW(), NOW()),
('46802ace-5791-bdf4-6802-ace57913579d', 'hedge_fund.hurdle_rate', 'Hurdle rate for performance fees', 'HedgeFund', 'decimal', 'Investment', '', '{"type": "manual", "required": false, "validation": "percentage"}', '{"type": "database", "table": "hedge_fund_terms"}', NOW(), NOW()),
('46802ace-5791-bdf4-6802-ace57913579e', 'hedge_fund.lock_up_period', 'Lock-up period in months', 'HedgeFund', 'integer', 'Investment', '', '{"type": "manual", "required": true, "validation": "positive_integer"}', '{"type": "database", "table": "hedge_fund_terms"}', NOW(), NOW()),

-- Ultimate Beneficial Owner (UBO) Attributes
('57913bdf-6802-ace4-6802-ace579135791', 'ubo.ownership_percentage', 'Ownership percentage', 'UBO', 'decimal', 'Legal', '', '{"type": "manual", "required": true, "validation": "percentage"}', '{"type": "database", "table": "ubos"}', NOW(), NOW()),
('57913bdf-6802-ace4-6802-ace579135792', 'ubo.control_type', 'Type of control exercised', 'UBO', 'enum', 'Legal', '', '{"type": "manual", "required": true, "values": ["DIRECT", "INDIRECT", "VOTING_RIGHTS", "OTHER"]}', '{"type": "database", "table": "ubos"}', NOW(), NOW()),
('57913bdf-6802-ace4-6802-ace579135793', 'ubo.full_name', 'Full name of UBO', 'UBO', 'string', 'Legal', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "ubos"}', NOW(), NOW()),
('57913bdf-6802-ace4-6802-ace579135794', 'ubo.date_of_birth', 'Date of birth of UBO', 'UBO', 'date', 'Legal', '', '{"type": "manual", "required": true}', '{"type": "database", "table": "ubos"}', NOW(), NOW()),
('57913bdf-6802-ace4-6802-ace579135795', 'ubo.nationality', 'Nationality of UBO', 'UBO', 'string', 'Legal', '', '{"type": "manual", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "ubos"}', NOW(), NOW())

ON CONFLICT (name) DO NOTHING;

-- ============================================================================
-- 2. ENTITY TYPES, ROLES, AND REFERENCE CBUS
-- ============================================================================


-- First, ensure we have the necessary entity types in the entity_types table
INSERT INTO "ob-poc".entity_types (entity_type_id, name, description, table_name, created_at, updated_at)
VALUES
    ('11111111-1111-1111-1111-111111111111', 'PARTNERSHIP', 'Limited Liability Partnership', 'entity_partnerships', NOW(), NOW()),
    ('22222222-2222-2222-2222-222222222222', 'LIMITED_COMPANY', 'Limited Company/Corporation', 'entity_limited_companies', NOW(), NOW()),
    ('33333333-3333-3333-3333-333333333333', 'PROPER_PERSON', 'Natural Person/Proper Person', 'entity_proper_persons', NOW(), NOW())
ON CONFLICT (name) DO NOTHING;

-- Create standard roles for financial entities
INSERT INTO "ob-poc".roles (role_id, name, description, created_at, updated_at)
VALUES
    ('1bad3016-0201-418b-ba0b-c90d19ec60cf', 'GENERAL_PARTNER', 'General Partner of hedge fund', NOW(), NOW()),
    ('82237ed0-fbca-4c56-afa6-0077b1e9bb1b', 'INVESTMENT_MANAGER', 'Investment Manager', NOW(), NOW()),
    ('6194082a-cd9f-4f21-aaa6-270ab86dd5ce', 'ASSET_OWNER', 'Asset Owner/Fund Entity', NOW(), NOW()),
    ('ec62f238-c473-49eb-9ec4-b8095e6b0174', 'MANAGEMENT_COMPANY', 'Management Company', NOW(), NOW()),
    ('ebf7b8bb-8806-49d7-85fd-d64f3fffdeb8', 'SERVICE_PROVIDER', 'Service Provider/Administrator', NOW(), NOW())
ON CONFLICT (name) DO NOTHING;

-- =============================================================================
-- HEDGE FUND CBUs
-- =============================================================================

-- Hedge Fund CBU 1: Quantum Capital Partners
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('afcd574a-b311-4af3-a616-6d251d47419e', 'HF-QCP-001', 'Quantum Capital Partners Hedge Fund', 'Multi-strategy hedge fund focused on equity long/short and merger arbitrage', NOW(), NOW());

-- Hedge Fund CBU 2: Meridian Alpha Fund
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('057adde2-e8ad-4345-839d-b00fb124e2e3', 'HF-MAF-002', 'Meridian Alpha Fund', 'Quantitative hedge fund specializing in fixed income and derivatives strategies', NOW(), NOW());

-- LLP Entities for Hedge Fund 1 (Quantum Capital Partners)
INSERT INTO "ob-poc".entity_partnerships (partnership_id, partnership_name, partnership_type, jurisdiction, formation_date, principal_place_business, partnership_agreement_date, created_at, updated_at)
VALUES
    ('d91ded86-600a-419c-bc65-7cf8ea59d9fb', 'Quantum Capital Management LLP', 'Limited Liability', 'US-DE', '2020-03-15', '200 Greenwich Street, New York, NY 10007', '2020-03-10', NOW(), NOW()),
    ('16674451-6f26-4e8a-89e1-8a14380f827b', 'QCP Investment Advisors LLP', 'Limited Liability', 'US-DE', '2020-03-15', '200 Greenwich Street, New York, NY 10007', '2020-03-10', NOW(), NOW());

-- LLP Entities for Hedge Fund 2 (Meridian Alpha Fund)
INSERT INTO "ob-poc".entity_partnerships (partnership_id, partnership_name, partnership_type, jurisdiction, formation_date, principal_place_business, partnership_agreement_date, created_at, updated_at)
VALUES
    ('b6be1e69-5faf-4cb4-a04e-df653817ac64', 'Meridian Investment Management LLP', 'Limited Liability', 'US-NY', '2019-09-22', '1345 Avenue of the Americas, New York, NY 10105', '2019-09-20', NOW(), NOW()),
    ('93cc2439-b47e-499f-8126-ed49c3c15bf6', 'Meridian Alpha GP LLP', 'Limited Liability', 'US-NY', '2019-09-22', '1345 Avenue of the Americas, New York, NY 10105', '2019-09-20', NOW(), NOW());

-- Register LLP entities in central entities table
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
VALUES
    -- Quantum Capital Partners LLPs
    ('0f0ea5f9-05b1-480b-901d-fdcb319f79a3', '11111111-1111-1111-1111-111111111111', 'd91ded86-600a-419c-bc65-7cf8ea59d9fb', 'Quantum Capital Management LLP', NOW(), NOW()),
    ('8a9a865c-d900-43b1-a9b1-23b3f540843a', '11111111-1111-1111-1111-111111111111', '16674451-6f26-4e8a-89e1-8a14380f827b', 'QCP Investment Advisors LLP', NOW(), NOW()),
    -- Meridian Alpha Fund LLPs
    ('f785423a-015c-40d5-9a1e-7d2dc0fefb23', '11111111-1111-1111-1111-111111111111', 'b6be1e69-5faf-4cb4-a04e-df653817ac64', 'Meridian Investment Management LLP', NOW(), NOW()),
    ('555546cf-6a32-4e8d-8aa3-c8d60580e221', '11111111-1111-1111-1111-111111111111', '93cc2439-b47e-499f-8126-ed49c3c15bf6', 'Meridian Alpha GP LLP', NOW(), NOW());

-- Link Hedge Fund CBUs to their LLP entities with roles
INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
VALUES
    -- Quantum Capital Partners CBU relationships
    ('b11f0fa9-bd11-4572-8176-dec24a9a5425', 'afcd574a-b311-4af3-a616-6d251d47419e', '0f0ea5f9-05b1-480b-901d-fdcb319f79a3', '82237ed0-fbca-4c56-afa6-0077b1e9bb1b', NOW()), -- Investment Manager
    ('9020f6a3-0452-4613-970d-0e7b396d7d33', 'afcd574a-b311-4af3-a616-6d251d47419e', '8a9a865c-d900-43b1-a9b1-23b3f540843a', '1bad3016-0201-418b-ba0b-c90d19ec60cf', NOW()), -- General Partner
    -- Meridian Alpha Fund CBU relationships
    ('1bdf1105-634f-461a-9c81-7ce7e1529868', '057adde2-e8ad-4345-839d-b00fb124e2e3', 'f785423a-015c-40d5-9a1e-7d2dc0fefb23', '82237ed0-fbca-4c56-afa6-0077b1e9bb1b', NOW()), -- Investment Manager
    ('072d80b1-38df-4c61-9f9d-39d8d2011731', '057adde2-e8ad-4345-839d-b00fb124e2e3', '555546cf-6a32-4e8d-8aa3-c8d60580e221', '1bad3016-0201-418b-ba0b-c90d19ec60cf', NOW()); -- General Partner

-- =============================================================================
-- US 1940 ACT CBUs AND ENTITIES
-- =============================================================================

-- US 1940 Act CBU 1: Asset Owner (Mutual Fund)
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('0cd08a4a-dcff-44ed-9e2c-3a41347a9418', 'US1940-AO-001', 'American Growth Equity Fund', '1940 Act registered mutual fund - large cap growth equity strategy', NOW(), NOW());

-- US 1940 Act CBU 2: Investment Manager
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('aa11bb22-cc33-44dd-ee55-ff6677889900', 'US1940-IM-002', 'Sterling Asset Management Company', 'SEC registered investment advisor providing portfolio management services', NOW(), NOW());

-- US 1940 Act CBU 3: Management Company
INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
VALUES ('bb22cc33-dd44-55ee-ff66-778899aabbcc', 'US1940-MC-003', 'Continental Fund Services Inc', 'Registered management company providing fund administration and compliance services', NOW(), NOW());

-- Limited Company Entities for US 1940 Act CBUs
INSERT INTO "ob-poc".entity_limited_companies (limited_company_id, company_name, registration_number, jurisdiction, incorporation_date, registered_address, business_nature, created_at, updated_at)
VALUES
    -- Asset Owner Entity
    ('cc33dd44-ee55-66ff-7788-99aabbccddee', 'American Growth Equity Fund Inc', 'DE-8765432', 'US-DE', '2018-01-15', '1601 Cherry Street, Philadelphia, PA 19102', 'Registered Investment Company under 1940 Act', NOW(), NOW()),
    -- Investment Manager Entity
    ('dd44ee55-ff66-77aa-88bb-99ccddee00ff', 'Sterling Asset Management Company', 'DE-9876543', 'US-DE', '2015-06-10', '100 Federal Street, Boston, MA 02110', 'SEC Registered Investment Advisor', NOW(), NOW()),
    -- Management Company Entity
    ('ee55ff66-aa77-88bb-99cc-ddee00ff1122', 'Continental Fund Services Inc', 'DE-5432198', 'US-DE', '2012-11-20', '2005 Market Street, Philadelphia, PA 19103', 'Fund Administration and Management Services', NOW(), NOW());

-- Register Limited Company entities in central entities table
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, external_id, name, created_at, updated_at)
VALUES
    -- US 1940 Act Companies
    ('ff66aa77-bb88-99cc-ddee-00ff11223344', '22222222-2222-2222-2222-222222222222', 'cc33dd44-ee55-66ff-7788-99aabbccddee', 'American Growth Equity Fund Inc', NOW(), NOW()),
    ('aa77bb88-cc99-ddee-00ff-112233445566', '22222222-2222-2222-2222-222222222222', 'dd44ee55-ff66-77aa-88bb-99ccddee00ff', 'Sterling Asset Management Company', NOW(), NOW()),
    ('bb88cc99-ddee-00ff-1122-334455667788', '22222222-2222-2222-2222-222222222222', 'ee55ff66-aa77-88bb-99cc-ddee00ff1122', 'Continental Fund Services Inc', NOW(), NOW());

-- Link US 1940 Act CBUs to their Limited Company entities with roles
INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
VALUES
    -- Asset Owner CBU relationship
    ('cc99ddee-00ff-1122-3344-556677889900', '0cd08a4a-dcff-44ed-9e2c-3a41347a9418', 'ff66aa77-bb88-99cc-ddee-00ff11223344', '6194082a-cd9f-4f21-aaa6-270ab86dd5ce', NOW()), -- Asset Owner
    -- Investment Manager CBU relationship
    ('ddee00ff-1122-3344-5566-7788990011aa', 'aa11bb22-cc33-44dd-ee55-ff6677889900', 'aa77bb88-cc99-ddee-00ff-112233445566', '82237ed0-fbca-4c56-afa6-0077b1e9bb1b', NOW()), -- Investment Manager
    -- Management Company CBU relationship
    ('ee00ff11-2233-4455-6677-88990011aabb', 'bb22cc33-dd44-55ee-ff66-778899aabbcc', 'bb88cc99-ddee-00ff-1122-334455667788', 'ec62f238-c473-49eb-9ec4-b8095e6b0174', NOW()); -- Management Company

-- =============================================================================
-- SUMMARY VERIFICATION QUERIES (for manual verification after running)
-- =============================================================================

-- Uncomment the following to verify the data was inserted correctly:

-- SELECT 'CBUs Created:' as summary;
-- SELECT name, description, nature_purpose FROM "ob-poc".cbus WHERE name LIKE 'HF-%' OR name LIKE 'US1940-%';

-- SELECT 'LLP Partnerships Created:' as summary;
-- SELECT partnership_name, partnership_type, jurisdiction FROM "ob-poc".entity_partnerships;

-- SELECT 'Limited Companies Created:' as summary;
-- SELECT company_name, registration_number, jurisdiction, business_nature FROM "ob-poc".entity_limited_companies;

-- SELECT 'Entity Relationships:' as summary;
-- SELECT c.name as cbu_name, e.name as entity_name, r.name as role_name
-- FROM "ob-poc".cbu_entity_roles cer
-- JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
-- JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
-- JOIN "ob-poc".roles r ON r.role_id = cer.role_id
-- WHERE c.name LIKE 'HF-%' OR c.name LIKE 'US1940-%'
-- ORDER BY c.name, r.name;
