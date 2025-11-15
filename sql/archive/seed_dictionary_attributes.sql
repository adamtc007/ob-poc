-- Seed comprehensive dictionary attributes for DSL onboarding system
-- This populates the dictionary table with 76+ common financial onboarding attributes

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
