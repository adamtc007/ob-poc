-- 12_regulatory_documents_complete.sql
-- Comprehensive Regulatory Documents for All Major Financial Regulatory Regimes
--
-- This script completes the document dictionary with regulatory documents from:
-- - FATCA (Foreign Account Tax Compliance Act)
-- - MiFID I/II (Markets in Financial Instruments Directive)
-- - ERISA (Employee Retirement Income Security Act)
-- - EMIR (European Market Infrastructure Regulation)
-- - Dodd-Frank Act
-- - AIFMD (Alternative Investment Fund Managers Directive)
-- - UCITS (Undertakings for Collective Investment in Transferable Securities)
-- - Basel III/CRD IV
-- - GDPR (General Data Protection Regulation)
-- - Anti-Money Laundering (AML) regimes
-- - Securities regulations (SEC, FCA, BaFin, etc.)

INSERT INTO "ob-poc".document_types (
    document_code, document_name, category, subcategory, description,
    issuing_authority, typical_jurisdictions, regulatory_framework,
    validity_period_months, renewal_required, digital_format_accepted,
    standardized_format, multilingual_variants, ai_extraction_complexity,
    ai_narrative, business_purpose, compliance_implications,
    verification_methods, related_document_types
) VALUES

-- ============================================================================
-- FATCA (Foreign Account Tax Compliance Act) DOCUMENTS
-- ============================================================================

('FORM_W8BEN', 'Certificate of Foreign Status of Beneficial Owner (W-8BEN)', 'Tax Documents', 'FATCA Forms',
 'IRS form for foreign individuals to establish tax status and claim treaty benefits',
 'Internal Revenue Service (IRS)',
 ARRAY['US', 'Global - for US tax purposes'],
 'FATCA', 36, true, true, true,
 ARRAY['English'],
 'high',
 'Form W-8BEN is for foreign individuals (not entities) to establish non-US person status. AI agents should extract: (1) Individual full name, (2) Country of citizenship, (3) Permanent residence address, (4) US taxpayer identification number if any, (5) Foreign TIN and issuing country, (6) Date of birth, (7) Treaty country and benefits claimed, (8) Signature and date. Critical for determining US withholding tax rates and treaty benefits. Verify individual vs entity status - entities use W-8BEN-E.',
 'Establishes foreign individual tax status for US withholding purposes. Required for US investment accounts and securities transactions.',
 ARRAY['Required for US securities investments by foreign individuals', 'Determines withholding tax rates', 'FATCA compliance for individuals', 'Treaty benefit eligibility'],
 ARRAY['IRS database verification', 'Treaty benefit validation', 'Individual identity verification', 'Tax residency confirmation'],
 ARRAY['FORM_W8BEN_E', 'PASSPORT', 'TAX_RESIDENCY_CERTIFICATE', 'FATCA_SELF_CERTIFICATION']),

('FORM_W9', 'Request for Taxpayer Identification Number and Certification (W-9)', 'Tax Documents', 'US Tax Forms',
 'IRS form for US persons to provide taxpayer identification number and certifications',
 'Internal Revenue Service (IRS)',
 ARRAY['US'],
 'US Tax Code', 36, true, true, true,
 ARRAY['English'],
 'medium',
 'Form W-9 is completed by US persons to provide TIN and certifications. AI agents should extract: (1) Legal name or business name, (2) Business type (individual, corporation, partnership, etc.), (3) Address, (4) Taxpayer Identification Number (SSN, EIN, ITIN), (5) Backup withholding certification, (6) FATCA reporting code if applicable, (7) Signature and date. Used to establish US person status and proper tax reporting.',
 'Establishes US person status and provides TIN for tax reporting. Required for domestic US accounts and investment relationships.',
 ARRAY['Establishes US person status', 'Provides TIN for 1099 reporting', 'Backup withholding certification', 'FATCA reporting compliance'],
 ARRAY['TIN validation', 'US person status verification', 'Business type confirmation'],
 ARRAY['FORM_1099', 'US_TAX_RETURNS', 'BUSINESS_REGISTRATION']),

('FFI_AGREEMENT', 'Foreign Financial Institution Agreement', 'Regulatory Documents', 'FATCA Compliance',
 'Agreement between foreign financial institution and IRS for FATCA compliance',
 'Internal Revenue Service (IRS)',
 ARRAY['Global - Non-US jurisdictions'],
 'FATCA', NULL, false, true, true,
 ARRAY['English'],
 'very_high',
 'FFI Agreement establishes compliance obligations for foreign financial institutions under FATCA. AI agents should extract: (1) FFI legal name and address, (2) Global Intermediary Identification Number (GIIN), (3) FFI status and category, (4) Compliance certification requirements, (5) Due diligence obligations, (6) Withholding and reporting requirements, (7) Effective date and term, (8) Responsible Officer certifications. Complex document with significant compliance implications.',
 'Establishes FATCA compliance framework for foreign financial institutions. Defines reporting and withholding obligations.',
 ARRAY['Creates binding FATCA compliance obligations', 'Establishes due diligence requirements', 'Defines withholding and reporting procedures'],
 ARRAY['IRS registration verification', 'GIIN validation', 'Legal opinion review'],
 ARRAY['GIIN_REGISTRATION', 'FATCA_SELF_CERTIFICATION', 'RESPONSIBLE_OFFICER_COMPLIANCE']),

-- ============================================================================
-- MiFID I/II (Markets in Financial Instruments Directive) DOCUMENTS
-- ============================================================================

('MIFID_SUITABILITY_ASSESSMENT', 'MiFID Suitability Assessment', 'Regulatory Documents', 'MiFID Compliance',
 'Assessment of client knowledge, experience, and financial situation under MiFID II',
 'Investment Firm/Financial Institution',
 ARRAY['EU', 'EEA', 'UK'],
 'MiFID II', 24, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian', 'Dutch'],
 'very_high',
 'MiFID Suitability Assessment evaluates client appropriateness for investment services. AI agents should extract: (1) Client identification and category (retail/professional/eligible counterparty), (2) Investment knowledge and experience by instrument type, (3) Financial situation including income, assets, and liabilities, (4) Investment objectives and risk tolerance, (5) Investment horizon and liquidity needs, (6) Suitability determination and restrictions, (7) Ongoing suitability monitoring requirements. Critical for investor protection compliance.',
 'Ensures appropriate investment advice and product recommendations under MiFID II investor protection rules.',
 ARRAY['Required for investment advice under MiFID II', 'Investor protection compliance', 'Establishes investment restrictions and monitoring'],
 ARRAY['Client verification', 'Financial capacity assessment', 'Regulatory compliance review'],
 ARRAY['CLIENT_CATEGORISATION', 'INVESTMENT_POLICY_STATEMENT', 'RISK_PROFILE_QUESTIONNAIRE']),

('MIFID_CLIENT_CATEGORISATION', 'MiFID Client Categorisation', 'Regulatory Documents', 'MiFID Compliance',
 'Classification of clients as retail, professional, or eligible counterparty under MiFID',
 'Investment Firm/Financial Institution',
 ARRAY['EU', 'EEA', 'UK'],
 'MiFID II', 12, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian'],
 'high',
 'Client categorisation determines level of investor protection under MiFID. AI agents should extract: (1) Client name and entity type, (2) Categorisation (retail/professional/eligible counterparty), (3) Quantitative thresholds met (balance sheet, net turnover, own funds), (4) Qualitative criteria assessment, (5) Opt-up/opt-down elections, (6) Effective date and review requirements, (7) Impact on service provision and protections. Affects all subsequent MiFID obligations.',
 'Determines level of investor protection and regulatory obligations under MiFID framework.',
 ARRAY['Determines applicable investor protections', 'Affects disclosure requirements', 'Impacts product access and restrictions'],
 ARRAY['Financial threshold verification', 'Entity status confirmation', 'Regulatory classification validation'],
 ARRAY['MIFID_SUITABILITY_ASSESSMENT', 'ENTITY_FORMATION_DOCUMENTS', 'AUDITED_FINANCIAL_STATEMENTS']),

('MIFID_BEST_EXECUTION_POLICY', 'MiFID Best Execution Policy', 'Regulatory Documents', 'MiFID Compliance',
 'Policy document outlining best execution arrangements under MiFID II',
 'Investment Firm',
 ARRAY['EU', 'EEA', 'UK'],
 'MiFID II', 12, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian'],
 'high',
 'Best execution policy explains how firm achieves best execution for client orders. AI agents should extract: (1) Execution venues and counterparties used, (2) Execution factors and criteria (price, costs, speed, likelihood of execution), (3) Relative importance of execution factors by instrument type, (4) Selection of execution venues, (5) Monitoring and review procedures, (6) Client consent and notification requirements, (7) Annual reporting obligations.',
 'Demonstrates compliance with MiFID II best execution requirements. Required disclosure to clients.',
 ARRAY['Required MiFID II disclosure', 'Demonstrates best execution compliance', 'Client protection mechanism'],
 ARRAY['Regulatory compliance review', 'Execution venue verification', 'Policy effectiveness assessment'],
 ARRAY['EXECUTION_QUALITY_REPORTS', 'ORDER_EXECUTION_POLICY', 'CLIENT_AGREEMENTS']),

-- ============================================================================
-- ERISA (Employee Retirement Income Security Act) DOCUMENTS
-- ============================================================================

('ERISA_FIDUCIARY_ACKNOWLEDGMENT', 'ERISA Fiduciary Acknowledgment', 'Regulatory Documents', 'ERISA Compliance',
 'Acknowledgment of ERISA fiduciary status and responsibilities',
 'Plan Fiduciary/Investment Manager',
 ARRAY['US'],
 'ERISA', NULL, false, true, false,
 ARRAY['English'],
 'very_high',
 'ERISA fiduciary acknowledgment establishes fiduciary responsibilities under ERISA. AI agents should extract: (1) Fiduciary name and role, (2) Plan identification and details, (3) Fiduciary responsibilities acknowledged, (4) Investment discretion scope, (5) Prohibited transaction awareness, (6) Fee disclosure requirements, (7) Reporting and monitoring obligations, (8) Effective date and term. Creates legal fiduciary obligations with significant liability.',
 'Establishes ERISA fiduciary relationship and responsibilities. Creates legal obligations for plan asset management.',
 ARRAY['Creates ERISA fiduciary liability', 'Establishes investment discretion', 'Requires prohibited transaction monitoring'],
 ARRAY['Fiduciary registration verification', 'Plan qualification confirmation', 'Legal compliance review'],
 ARRAY['PLAN_DOCUMENTS', 'INVESTMENT_POLICY_STATEMENT', 'PROHIBITED_TRANSACTION_EXEMPTION']),

('ERISA_PLAN_DOCUMENT', 'ERISA Plan Document', 'Regulatory Documents', 'ERISA Plans',
 'Governing document for employee benefit plan under ERISA',
 'Plan Sponsor/Employer',
 ARRAY['US'],
 'ERISA', NULL, false, true, false,
 ARRAY['English'],
 'very_high',
 'ERISA plan document governs employee benefit plan operations. AI agents should extract: (1) Plan name and sponsor, (2) Plan type (defined benefit/contribution, 401k, etc.), (3) Eligibility and participation requirements, (4) Benefit formulas and vesting schedules, (5) Investment options and fiduciary structure, (6) Distribution and withdrawal provisions, (7) Amendment and termination procedures, (8) Administrative responsibilities. Legal foundation for plan operations.',
 'Governs employee benefit plan operations under ERISA. Defines benefits, eligibility, and administrative procedures.',
 ARRAY['Legal foundation for plan operations', 'Defines participant rights and benefits', 'Establishes fiduciary framework'],
 ARRAY['Legal document review', 'IRS qualification verification', 'DOL compliance assessment'],
 ARRAY['SUMMARY_PLAN_DESCRIPTION', 'FORM_5500', 'TRUST_AGREEMENT']),

('FORM_5500', 'Annual Return/Report of Employee Benefit Plan', 'Regulatory Documents', 'ERISA Reporting',
 'Annual report required for employee benefit plans under ERISA',
 'Department of Labor/IRS',
 ARRAY['US'],
 'ERISA', 12, true, true, true,
 ARRAY['English'],
 'very_high',
 'Form 5500 is annual report for employee benefit plans. AI agents should extract: (1) Plan identification and type, (2) Plan sponsor and administrator information, (3) Participant counts and demographics, (4) Financial information (assets, liabilities, contributions, benefits), (5) Service provider information and fees, (6) Investment details and performance, (7) Compliance certifications, (8) Audit requirements and accountant information. Complex form with multiple schedules.',
 'Annual regulatory filing for employee benefit plans. Provides transparency on plan operations and finances.',
 ARRAY['Required annual ERISA filing', 'Public disclosure document', 'Regulatory compliance monitoring'],
 ARRAY['DOL database verification', 'Audit report consistency', 'Mathematical reconciliation'],
 ARRAY['PLAN_AUDIT_REPORT', 'PLAN_DOCUMENT', 'TRUSTEE_REPORTS']),

-- ============================================================================
-- EMIR (European Market Infrastructure Regulation) DOCUMENTS
-- ============================================================================

('EMIR_TRADE_REPOSITORY_REPORT', 'EMIR Trade Repository Report', 'Regulatory Documents', 'EMIR Compliance',
 'Derivatives transaction report submitted to trade repository under EMIR',
 'Trade Repository',
 ARRAY['EU', 'EEA', 'UK'],
 'EMIR', 1, true, true, true,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian'],
 'very_high',
 'EMIR trade repository reporting captures all derivatives transactions. AI agents should extract: (1) Counterparty identifiers (LEI codes), (2) Trade details (product, notional, maturity, price), (3) Transaction type (new, modification, termination), (4) Clearing status (cleared/uncleared), (5) Collateral information, (6) Valuation details, (7) Action type and timestamp, (8) Risk mitigation techniques applied. Complex technical format with strict data standards.',
 'Mandatory reporting of derivatives transactions for regulatory transparency and systemic risk monitoring.',
 ARRAY['Required EMIR reporting obligation', 'Systemic risk monitoring', 'Market transparency enhancement'],
 ARRAY['Trade repository validation', 'LEI code verification', 'Data quality checks'],
 ARRAY['TRADE_CONFIRMATIONS', 'CLEARING_CONFIRMATIONS', 'COLLATERAL_AGREEMENTS']),

('EMIR_RISK_MITIGATION_PROCEDURES', 'EMIR Risk Mitigation Procedures', 'Regulatory Documents', 'EMIR Compliance',
 'Documentation of risk mitigation techniques for uncleared derivatives under EMIR',
 'Counterparties/Financial Institution',
 ARRAY['EU', 'EEA'],
 'EMIR', 12, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian'],
 'high',
 'EMIR risk mitigation procedures document compliance with uncleared derivatives requirements. AI agents should extract: (1) Risk mitigation techniques implemented, (2) Timely confirmation procedures, (3) Portfolio reconciliation frequency and procedures, (4) Portfolio compression arrangements, (5) Dispute resolution procedures, (6) Exchange of collateral arrangements, (7) Operational procedures and responsibilities, (8) Monitoring and review requirements.',
 'Demonstrates compliance with EMIR risk mitigation requirements for uncleared derivatives.',
 ARRAY['Required EMIR compliance documentation', 'Risk management framework', 'Operational procedures'],
 ARRAY['Regulatory compliance assessment', 'Procedure effectiveness review', 'Documentation completeness'],
 ARRAY['COLLATERAL_AGREEMENTS', 'TRADE_CONFIRMATIONS', 'DISPUTE_RESOLUTION_PROCEDURES']),

-- ============================================================================
-- DODD-FRANK ACT DOCUMENTS
-- ============================================================================

('CFTC_SWAP_DEALER_REGISTRATION', 'CFTC Swap Dealer Registration', 'Regulatory Documents', 'Dodd-Frank Compliance',
 'Registration with CFTC as swap dealer under Dodd-Frank Act',
 'Commodity Futures Trading Commission (CFTC)',
 ARRAY['US'],
 'Dodd-Frank Act', NULL, false, true, true,
 ARRAY['English'],
 'very_high',
 'CFTC swap dealer registration establishes regulatory status under Dodd-Frank. AI agents should extract: (1) Registrant legal name and identifiers, (2) Business activities and swap dealing thresholds, (3) Organizational structure and ownership, (4) Key personnel and qualifications, (5) Capital and financial resources, (6) Risk management and compliance procedures, (7) Recordkeeping and reporting systems, (8) Third-party service providers. Comprehensive regulatory filing.',
 'Establishes regulatory status as swap dealer with comprehensive compliance obligations under Dodd-Frank.',
 ARRAY['Creates swap dealer regulatory status', 'Establishes comprehensive compliance framework', 'Enables swap dealing business'],
 ARRAY['CFTC database verification', 'Background checks', 'Financial adequacy assessment'],
 ARRAY['CAPITAL_ADEQUACY_REPORTS', 'RISK_MANAGEMENT_POLICIES', 'COMPLIANCE_PROCEDURES']),

('VOLCKER_RULE_COMPLIANCE_PROGRAM', 'Volcker Rule Compliance Program', 'Regulatory Documents', 'Dodd-Frank Compliance',
 'Compliance program for Volcker Rule proprietary trading restrictions',
 'Banking Entity',
 ARRAY['US'],
 'Dodd-Frank Act/Volcker Rule', 12, true, true, false,
 ARRAY['English'],
 'very_high',
 'Volcker Rule compliance program addresses proprietary trading restrictions. AI agents should extract: (1) Banking entity identification and scope, (2) Prohibited proprietary trading activities, (3) Permitted activities and exemptions, (4) Compliance monitoring and controls, (5) Risk management and limits, (6) Audit and review procedures, (7) Training and personnel requirements, (8) Reporting and recordkeeping obligations. Complex regulatory framework.',
 'Ensures compliance with Volcker Rule proprietary trading restrictions for banking entities.',
 ARRAY['Required Volcker Rule compliance', 'Proprietary trading restrictions', 'Risk management framework'],
 ARRAY['Banking regulator review', 'Trading activity monitoring', 'Compliance effectiveness assessment'],
 ARRAY['TRADING_POLICIES', 'RISK_LIMITS', 'AUDIT_REPORTS']),

-- ============================================================================
-- AIFMD (Alternative Investment Fund Managers Directive) DOCUMENTS
-- ============================================================================

('AIFMD_REGISTRATION', 'AIFMD Registration', 'Regulatory Documents', 'AIFMD Compliance',
 'Registration of Alternative Investment Fund Manager under AIFMD',
 'National Competent Authority',
 ARRAY['EU', 'EEA'],
 'AIFMD', NULL, false, true, true,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian', 'Dutch'],
 'very_high',
 'AIFMD registration authorizes alternative investment fund management. AI agents should extract: (1) AIFM legal name and address, (2) Authorized activities and fund types, (3) Organizational structure and ownership, (4) Key personnel and qualifications, (5) Capital requirements and own funds, (6) Risk management and compliance systems, (7) Depositary arrangements, (8) Valuation procedures, (9) Liquidity management, (10) Leverage limits and procedures.',
 'Authorizes alternative investment fund management activities under AIFMD regulatory framework.',
 ARRAY['Authorizes AIF management activities', 'Establishes regulatory compliance framework', 'Enables fund marketing'],
 ARRAY['Regulatory authority verification', 'Personnel qualification checks', 'Capital adequacy assessment'],
 ARRAY['FUND_DOCUMENTS', 'DEPOSITARY_AGREEMENT', 'RISK_MANAGEMENT_PROCEDURES']),

('AIFMD_ANNUAL_REPORT', 'AIFMD Annual Report', 'Regulatory Documents', 'AIFMD Reporting',
 'Annual report for Alternative Investment Fund under AIFMD',
 'Alternative Investment Fund Manager',
 ARRAY['EU', 'EEA'],
 'AIFMD', 12, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian'],
 'high',
 'AIFMD annual report provides comprehensive fund information. AI agents should extract: (1) Fund identification and strategy, (2) Investment portfolio and performance, (3) Risk profile and management, (4) Leverage information and sources, (5) Liquidity management, (6) Fee structure and expenses, (7) Service provider information, (8) Professional liability and coverage, (9) Percentage of assets subject to special arrangements.',
 'Required annual disclosure for Alternative Investment Funds under AIFMD transparency requirements.',
 ARRAY['Required AIFMD disclosure', 'Investor transparency', 'Regulatory monitoring'],
 ARRAY['Regulatory submission verification', 'Data consistency checks', 'Performance validation'],
 ARRAY['FUND_AUDITED_ACCOUNTS', 'RISK_REPORTS', 'INVESTOR_STATEMENTS']),

-- ============================================================================
-- UCITS (Undertakings for Collective Investment in Transferable Securities)
-- ============================================================================

('UCITS_PROSPECTUS', 'UCITS Prospectus', 'Regulatory Documents', 'UCITS Compliance',
 'Prospectus for UCITS fund under EU regulatory framework',
 'UCITS Management Company',
 ARRAY['EU', 'EEA'],
 'UCITS Directive', NULL, false, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian', 'Dutch'],
 'very_high',
 'UCITS prospectus is primary disclosure document for retail funds. AI agents should extract: (1) Fund name and legal structure, (2) Investment objectives and policy, (3) Risk factors and profile, (4) Fee structure and charges, (5) Share classes and characteristics, (6) Management company and key personnel, (7) Depositary information, (8) Subscription and redemption procedures, (9) Distribution policy, (10) Performance and benchmark information.',
 'Primary disclosure document for UCITS funds. Required for retail fund marketing across EU.',
 ARRAY['Required UCITS disclosure document', 'Enables EU-wide marketing', 'Investor protection compliance'],
 ARRAY['Regulatory approval verification', 'Content accuracy review', 'Translation consistency checks'],
 ARRAY['KIID', 'FUND_RULES', 'ANNUAL_REPORT']),

('KIID', 'Key Investor Information Document', 'Regulatory Documents', 'UCITS Compliance',
 'Standardized key investor information for UCITS funds',
 'UCITS Management Company',
 ARRAY['EU', 'EEA'],
 'UCITS Directive', 12, true, true, true,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian', 'Dutch'],
 'high',
 'KIID provides standardized key information for retail investors. AI agents should extract: (1) Fund name and share class, (2) Investment objectives and policy summary, (3) Risk and reward indicator (1-7 scale), (4) Key risk factors, (5) Past performance data, (6) Total expense ratio (TER), (7) Entry and exit charges, (8) Management company information, (9) Depositary details, (10) Publication date and validity.',
 'Standardized investor information required before investment in UCITS funds. Consumer protection document.',
 ARRAY['Required pre-investment disclosure', 'Standardized format across EU', 'Consumer protection compliance'],
 ARRAY['Regulatory format compliance', 'Data accuracy verification', 'Translation quality review'],
 ARRAY['UCITS_PROSPECTUS', 'FUND_FACTSHEET', 'ANNUAL_REPORT']),

-- ============================================================================
-- BASEL III/CRD IV DOCUMENTS
-- ============================================================================

('CRD_CAPITAL_ADEQUACY_REPORT', 'CRD Capital Adequacy Report', 'Regulatory Documents', 'Basel III/CRD IV',
 'Capital adequacy and risk management report under CRD IV',
 'Credit Institution',
 ARRAY['EU', 'EEA', 'UK'],
 'CRD IV/Basel III', 12, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian'],
 'very_high',
 'Capital adequacy report demonstrates compliance with CRD IV requirements. AI agents should extract: (1) Institution identification and scope, (2) Capital ratios (CET1, Tier 1, Total Capital), (3) Risk-weighted assets calculation, (4) Credit risk exposures and provisions, (5) Market risk measures, (6) Operational risk assessment, (7) Leverage ratio calculation, (8) Liquidity coverage ratio, (9) Stress test results, (10) Risk governance and management.',
 'Demonstrates capital adequacy and risk management compliance under EU banking regulation.',
 ARRAY['Required regulatory capital reporting', 'Prudential supervision', 'Financial stability monitoring'],
 ARRAY['Regulatory supervisor review', 'Mathematical validation', 'Risk model verification'],
 ARRAY['STRESS_TEST_REPORTS', 'LIQUIDITY_REPORTS', 'RISK_MANAGEMENT_POLICIES']),

('LCR_REPORT', 'Liquidity Coverage Ratio Report', 'Regulatory Documents', 'Basel III/CRD IV',
 'Liquidity coverage ratio calculation and reporting under Basel III',
 'Credit Institution/Bank',
 ARRAY['Global - Basel III jurisdictions'],
 'Basel III', 1, true, true, true,
 ARRAY['English', 'Multiple'],
 'very_high',
 'LCR report demonstrates short-term liquidity adequacy. AI agents should extract: (1) Institution identification, (2) High-quality liquid assets (HQLA) composition, (3) Net cash outflows calculation, (4) LCR ratio and minimum requirements, (5) Stress scenario assumptions, (6) Asset and liability breakdowns, (7) Maturity analysis, (8) Currency-specific calculations, (9) Intraday liquidity management, (10) Contingency planning measures.',
 'Demonstrates short-term liquidity adequacy under Basel III regulatory framework.',
 ARRAY['Required Basel III liquidity reporting', 'Short-term liquidity monitoring', 'Financial stability assessment'],
 ARRAY['Regulatory calculation verification', 'Asset quality assessment', 'Stress scenario validation'],
 ARRAY['NSFR_REPORT', 'LIQUIDITY_RISK_POLICIES', 'CONTINGENCY_FUNDING_PLAN']),

-- ============================================================================
-- GDPR (General Data Protection Regulation) DOCUMENTS
-- ============================================================================

('GDPR_DATA_PROCESSING_AGREEMENT', 'GDPR Data Processing Agreement', 'Regulatory Documents', 'GDPR Compliance',
 'Data processing agreement between controller and processor under GDPR',
 'Data Controller/Data Processor',
 ARRAY['EU', 'EEA', 'UK'],
 'GDPR', NULL, false, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian', 'Dutch'],
 'high',
 'GDPR data processing agreement governs personal data processing relationships. AI agents should extract: (1) Controller and processor identification, (2) Purpose and scope of processing, (3) Categories of personal data, (4) Data subject categories, (5) Processing duration, (6) Technical and organizational measures, (7) Sub-processor arrangements, (8) Data transfer mechanisms, (9) Breach notification procedures, (10) Data subject rights procedures.',
 'Governs personal data processing relationships under GDPR regulatory framework.',
 ARRAY['Required GDPR compliance documentation', 'Data protection obligations', 'Privacy rights framework'],
 ARRAY['Legal compliance review', 'Technical measures verification', 'Privacy impact assessment'],
 ARRAY['PRIVACY_POLICY', 'CONSENT_FORMS', 'DATA_BREACH_PROCEDURES']),

('GDPR_PRIVACY_NOTICE', 'GDPR Privacy Notice', 'Regulatory Documents', 'GDPR Compliance',
 'Privacy notice to data subjects under GDPR transparency requirements',
 'Data Controller',
 ARRAY['EU', 'EEA', 'UK'],
 'GDPR', 24, true, true, false,
 ARRAY['English', 'German', 'French', 'Spanish', 'Italian', 'Dutch'],
 'medium',
 'GDPR privacy notice informs data subjects about processing. AI agents should extract: (1) Controller identity and contact details, (2) Data Protection Officer contact, (3) Processing purposes and legal bases, (4) Categories of personal data, (5) Recipients and transfers, (6) Retention periods, (7) Data subject rights, (8) Right to withdraw consent, (9) Complaint procedures, (10) Automated decision-making information.',
 'Required transparency disclosure to data subjects under GDPR.',
 ARRAY['Required GDPR transparency obligation', 'Data subject information rights', 'Consent and legal basis disclosure'],
 ARRAY['Content completeness review', 'Legal basis verification', 'Accessibility assessment'],
 ARRAY['CONSENT_FORMS', 'DATA_SUBJECT_REQUEST_PROCEDURES', 'RETENTION_SCHEDULES']),

-- ============================================================================
-- ANTI-MONEY LAUNDERING (AML) DOCUMENTS
-- ============================================================================

('AML_RISK_ASSESSMENT', 'AML Risk Assessment', 'Regulatory Documents', 'AML Compliance',
 'Anti-money laundering risk assessment and methodology',
 'Financial Institution',
 ARRAY['Global'],
 'AML/BSA', 12, true, true, false,
 ARRAY['English', 'Multiple'],
 'very_high',
 'AML risk assessment identifies and evaluates money laundering risks. AI agents should extract: (1) Institution scope and business lines, (2) Customer risk factors and categories, (3) Product and service risks, (4) Geographic risk assessment, (5) Delivery channel risks, (6) Risk mitigation measures, (7) Monitoring and detection systems, (8) Risk appetite and tolerance, (9) Review and update procedures, (10) Board and senior management oversight.',
 'Fundamental AML compliance document identifying and managing money laundering risks.',
 ARRAY['Required AML program component', 'Risk-based compliance approach', 'Regulatory examination focus'],
 ARRAY['Risk methodology validation', 'Regulatory guidance compliance', 'Risk factor completeness'],
 ARRAY['AML_POLICIES', 'CUSTOMER_DUE_DILIGENCE', 'TRANSACTION_MONITORING']),

('SUSPICIOUS_ACTIVITY_REPORT', 'Suspicious Activity Report (SAR)', 'Regulatory Documents', 'AML Reporting',
 'Report of suspicious activities to financial intelligence unit',
 'Financial Crimes Enforcement Network (FinCEN)/FIU',
 ARRAY['US', 'Global - jurisdiction specific'],
 'AML/BSA', NULL, false, true, true,
 ARRAY['English', 'Multiple'],
 'very_high',
 'SAR reports suspicious activities to law enforcement. AI agents should extract: (1) Subject identification information, (2) Suspicious activity description, (3) Financial institution information, (4) Transaction details and amounts, (5) Time period of activity, (6) Detection method, (7) Prior SAR filings, (8) Law enforcement contacts, (9) Supporting documentation, (10) Compliance officer certification. Highly confidential and regulated.',
 'Confidential report of suspicious activities to financial intelligence units for investigation.',
 ARRAY['Required AML reporting obligation', 'Law enforcement intelligence', 'Confidential regulatory filing'],
 ARRAY['Regulatory filing verification', 'Confidentiality protection', 'Timeline compliance'],
 ARRAY['TRANSACTION_MONITORING_ALERTS', 'CUSTOMER_DUE_DILIGENCE', 'AML_INVESTIGATION_FILES']),

-- ============================================================================
-- SECURITIES REGULATION DOCUMENTS
-- ============================================================================

('FORM_ADV', 'Form ADV - Investment Adviser Registration', 'Regulatory Documents', 'SEC/Investment Advisers',
 'Registration and disclosure document for investment advisers',
 'Securities and Exchange Commission (SEC)',
 ARRAY['US'],
 'Investment Advisers Act', 12, true, true, true,
 ARRAY['English'],
 'very_high',
 'Form ADV is comprehensive registration and disclosure for investment advisers. AI agents should extract: (1) Adviser identification and business structure, (2) Services offered and client types, (3) Assets under management, (4) Fee structure and billing practices, (5) Custody arrangements, (6) Conflicts of interest, (7) Disciplinary history, (8) Key personnel and ownership, (9) Financial condition, (10) Business practices and code of ethics. Critical regulatory disclosure document.',
 'Primary registration and disclosure document for investment advisers. Required for SEC registration and client disclosure.',
 ARRAY['Required SEC registration document', 'Public disclosure filing', 'Client relationship disclosure'],
 ARRAY['SEC database verification', 'Background check validation', 'Financial adequacy review'],
 ARRAY['FORM_ADV_PART2', 'COMPLIANCE_MANUAL', 'AUDIT_REPORTS']),

('FORM_13F', 'Form 13F - Quarterly Holdings Report', 'Regulatory Documents', 'SEC Reporting',
 'Quarterly report of equity holdings by institutional investment managers',
 'Securities and Exchange Commission (SEC)',
 ARRAY['US'],
 'Securities Exchange Act', 3, true, true, true,
 ARRAY['English'],
 'high',
 'Form 13F reports quarterly equity holdings of institutional managers. AI agents should extract: (1) Manager identification and reporting period, (2) Holdings list with CUSIP numbers, (3) Share quantities and market values, (4) Investment discretion indicators, (5) Voting authority classifications, (6) Summary totals, (7) Amendment indicators, (8) Signature and certification. Public disclosure of large holdings.',
 'Public disclosure of institutional equity holdings. Provides market transparency on large manager positions.',
 ARRAY['Required quarterly SEC filing for large managers', 'Public market transparency', 'Position disclosure'],
 ARRAY['CUSIP validation', 'Mathematical reconciliation', 'Market value verification'],
 ARRAY['PORTFOLIO_STATEMENTS', 'CUSTODY_STATEMENTS', 'TRADING_RECORDS']);

-- Add completion comment
COMMENT ON TABLE "ob-poc".document_types IS
'Comprehensive document dictionary containing 70+ official financial documents across all major regulatory regimes:
- FATCA, MiFID I/II, ERISA, EMIR, Dodd-Frank, AIFMD, UCITS, Basel III/CRD IV, GDPR, AML
- Corporate formation, identity verification, financial reporting, investment, derivatives
- Each document includes AI extraction guidance, compliance implications, and cross-validation rules
- Supports complete document-to-DSL data bridge for financial services operations';

-- Summary query
DO $$
DECLARE
    doc_count INTEGER;
    category_count INTEGER;
BEGIN
    SELECT count(*) INTO doc_count FROM "ob-poc".document_types;
    SELECT count(DISTINCT category) INTO category_count FROM "ob-poc".document_types;

    RAISE NOTICE 'Document Dictionary Complete:';
    RAISE NOTICE '- Total Documents: %', doc_count;
    RAISE NOTICE '- Document Categories: %', category_count;
    RAISE NOTICE '- Regulatory Regimes Covered: FATCA, MiFID, ERISA, EMIR, Dodd-Frank, AIFMD, UCITS, Basel III, GDPR, AML, SEC';
    RAISE NOTICE 'Comprehensive regulatory document framework established for AI-driven DSL extraction';
END $$;
