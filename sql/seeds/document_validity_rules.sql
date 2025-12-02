-- ============================================================================
-- DOCUMENT VALIDITY RULES
-- Defines expiry, renewal, and age requirements for KYC documents
-- ============================================================================
-- 
-- Rule Types:
--   MAX_AGE_DAYS/MONTHS  - Document must not be older than X from issue date
--   CHECK_EXPIRY         - Document has explicit expiry date to validate
--   MIN_REMAINING_VALIDITY - Must have X months remaining before expiry
--   VALIDITY_YEARS       - Valid for X years from signing (W-8 forms)
--   ANNUAL_RENEWAL       - Must be renewed annually (LEI, licenses)
--   EXPIRES_YEAR_END     - Expires Dec 31 of validity period
--   NO_EXPIRY            - Never expires (birth certificate)
--   SUPERSEDED_BY_EVENT  - Invalid when replaced by newer version
--
-- Run: psql "postgresql://localhost:5432/data_designer" -f sql/seeds/document_validity_rules.sql
-- ============================================================================

BEGIN;

-- Create table if not exists
CREATE TABLE IF NOT EXISTS "ob-poc".document_validity_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(type_id),
    rule_type VARCHAR(50) NOT NULL CHECK (rule_type IN (
        'MAX_AGE_DAYS',
        'MAX_AGE_MONTHS',
        'CHECK_EXPIRY',
        'MIN_REMAINING_VALIDITY',
        'ANNUAL_RENEWAL',
        'VALIDITY_YEARS',
        'EXPIRES_YEAR_END',
        'NO_EXPIRY',
        'SUPERSEDED_BY_EVENT'
    )),
    rule_value INTEGER,
    rule_unit VARCHAR(20) CHECK (rule_unit IN ('days', 'months', 'years')),
    rule_parameters JSONB,
    applies_to_jurisdictions TEXT[],
    applies_to_entity_types TEXT[],
    warning_days INTEGER DEFAULT 30,
    is_hard_requirement BOOLEAN DEFAULT TRUE,
    regulatory_source VARCHAR(200),
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT unique_doc_validity_rule UNIQUE (document_type_id, rule_type, COALESCE(applies_to_jurisdictions::text, 'ALL'))
);

-- Clear existing rules
DELETE FROM "ob-poc".document_validity_rules;

-- ============================================================================
-- IDENTITY DOCUMENTS
-- ============================================================================

-- PASSPORT: Check expiry + 6 months remaining
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 180, TRUE, 'Industry standard', 'Passport must not be expired'
FROM "ob-poc".document_types WHERE type_code = 'PASSPORT';

INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MIN_REMAINING_VALIDITY', 6, 'months', 180, TRUE, 'Industry standard', 'Most institutions require 6 months remaining validity'
FROM "ob-poc".document_types WHERE type_code = 'PASSPORT';

-- NATIONAL_ID: Check expiry
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 90, TRUE, 'National ID must not be expired'
FROM "ob-poc".document_types WHERE type_code = 'NATIONAL_ID';

-- DRIVERS_LICENSE: Check expiry
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 60, TRUE, 'License must not be expired'
FROM "ob-poc".document_types WHERE type_code IN ('DRIVERS_LICENSE', 'DRIVING_LICENSE');

-- BIRTH_CERTIFICATE: Never expires
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'Birth certificates do not expire'
FROM "ob-poc".document_types WHERE type_code = 'BIRTH_CERTIFICATE';

-- MARRIAGE_CERTIFICATE: Never expires
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'Marriage certificates do not expire'
FROM "ob-poc".document_types WHERE type_code = 'MARRIAGE_CERTIFICATE';

-- DEATH_CERTIFICATE: Never expires
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'Death certificates do not expire'
FROM "ob-poc".document_types WHERE type_code = 'DEATH_CERTIFICATE';

-- RESIDENCE_PERMIT: Check expiry + 3 months remaining
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 90, TRUE, 'Immigration law', 'Immigration documents must be current'
FROM "ob-poc".document_types WHERE type_code = 'RESIDENCE_PERMIT';

INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MIN_REMAINING_VALIDITY', 3, 'months', 90, TRUE, 'Immigration law', 'Should have minimum 3 months remaining'
FROM "ob-poc".document_types WHERE type_code = 'RESIDENCE_PERMIT';

-- ============================================================================
-- CORPORATE STATUS DOCUMENTS
-- ============================================================================

-- CERT_OF_GOOD_STANDING: 90 days max age (general), 30 days (offshore)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MAX_AGE_DAYS', 90, 'days', 30, TRUE, 'Industry standard', 'Good standing certificates typically accepted for 90 days'
FROM "ob-poc".document_types WHERE type_code = 'CERT_OF_GOOD_STANDING';

INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, applies_to_jurisdictions, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MAX_AGE_DAYS', 30, 'days', ARRAY['KY', 'BVI', 'JE', 'GG', 'IM'], 14, TRUE, 'Industry standard', 'Offshore jurisdictions often require more recent certificates'
FROM "ob-poc".document_types WHERE type_code = 'CERT_OF_GOOD_STANDING';

-- CERT_OF_INCUMBENCY: 90 days max age
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_DAYS', 90, 'days', 30, TRUE, 'Incumbency certificates typically accepted for 90 days'
FROM "ob-poc".document_types WHERE type_code = 'CERT_OF_INCUMBENCY';

-- REGISTRY_EXTRACT: 30 days recommended
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_DAYS', 30, 'days', 14, FALSE, 'Registry extracts should be recent'
FROM "ob-poc".document_types WHERE type_code = 'REGISTRY_EXTRACT';

-- CERT_OF_INCORPORATION: Never expires (foundational)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'Formation documents do not expire'
FROM "ob-poc".document_types WHERE type_code IN ('CERT_OF_INCORPORATION', 'CERTIFICATE_OF_INCORPORATION', 'ARTICLES_OF_ASSOCIATION', 'MEMORANDUM_OF_ASSOCIATION');

-- BOARD_RESOLUTION: Superseded by event (newer resolution)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, rule_parameters, warning_days, is_hard_requirement, notes)
SELECT type_id, 'SUPERSEDED_BY_EVENT', NULL, NULL, '{"superseded_by": "newer_resolution"}'::jsonb, 0, TRUE, 'Valid until superseded by subsequent resolution'
FROM "ob-poc".document_types WHERE type_code = 'BOARD_RESOLUTION';

-- SHAREHOLDER_RESOLUTION: Superseded by event
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, rule_parameters, warning_days, is_hard_requirement, notes)
SELECT type_id, 'SUPERSEDED_BY_EVENT', NULL, NULL, '{"superseded_by": "newer_resolution"}'::jsonb, 0, TRUE, 'Valid until superseded by subsequent resolution'
FROM "ob-poc".document_types WHERE type_code = 'SHAREHOLDER_RESOLUTION';

-- ============================================================================
-- FINANCIAL DOCUMENTS
-- ============================================================================

-- BANK_STATEMENT: 3 months max age
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 3, 'months', 30, TRUE, 'AML regulations', 'Bank statements must be dated within 3 months'
FROM "ob-poc".document_types WHERE type_code = 'BANK_STATEMENT';

-- AUDITED_ACCOUNTS: 18 months (covers fiscal year + filing delay)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 18, 'months', 60, FALSE, 'Audited accounts should be from most recent fiscal year'
FROM "ob-poc".document_types WHERE type_code = 'AUDITED_ACCOUNTS';

-- MANAGEMENT_ACCOUNTS: 6 months
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 6, 'months', 30, FALSE, 'Management accounts should be reasonably current'
FROM "ob-poc".document_types WHERE type_code = 'MANAGEMENT_ACCOUNTS';

-- NET_WORTH_STATEMENT: 12 months
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 12, 'months', 60, FALSE, 'Net worth statements should be annual'
FROM "ob-poc".document_types WHERE type_code = 'NET_WORTH_STATEMENT';

-- ============================================================================
-- ADDRESS VERIFICATION DOCUMENTS
-- ============================================================================

-- UTILITY_BILL: 3 months max age
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 3, 'months', 30, TRUE, 'AML regulations', 'Utility bills must be dated within 3 months'
FROM "ob-poc".document_types WHERE type_code = 'UTILITY_BILL';

-- COUNCIL_TAX: 12 months (annual document, UK)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, applies_to_jurisdictions, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 12, 'months', ARRAY['UK', 'GB'], 60, TRUE, 'UK AML regulations', 'Council tax bills valid for current tax year'
FROM "ob-poc".document_types WHERE type_code = 'COUNCIL_TAX';

-- MORTGAGE_STATEMENT: 3 months
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 3, 'months', 30, TRUE, 'Mortgage statements must be recent'
FROM "ob-poc".document_types WHERE type_code = 'MORTGAGE_STATEMENT';

-- LEASE_AGREEMENT: Check expiry (lease end date)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 60, TRUE, 'Lease must be current (not expired)'
FROM "ob-poc".document_types WHERE type_code = 'LEASE_AGREEMENT';

-- ============================================================================
-- TAX DOCUMENTS - US WITHHOLDING (W-8 FORMS)
-- ============================================================================

-- W-8BEN: Valid for 3 years, expires Dec 31 of 3rd year
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'VALIDITY_YEARS', 3, 'years', 90, TRUE, 'IRS regulations', 'W-8BEN valid until Dec 31 of 3rd calendar year after signing'
FROM "ob-poc".document_types WHERE type_code = 'W8_BEN';

INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'EXPIRES_YEAR_END', NULL, NULL, 90, TRUE, 'IRS regulations', 'Expires at end of calendar year'
FROM "ob-poc".document_types WHERE type_code = 'W8_BEN';

-- W-8BEN-E: Valid for 3 years, expires Dec 31
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'VALIDITY_YEARS', 3, 'years', 90, TRUE, 'IRS regulations', 'W-8BEN-E valid until Dec 31 of 3rd calendar year after signing'
FROM "ob-poc".document_types WHERE type_code = 'W8_BEN_E';

INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'EXPIRES_YEAR_END', NULL, NULL, 90, TRUE, 'IRS regulations', 'Expires at end of calendar year'
FROM "ob-poc".document_types WHERE type_code = 'W8_BEN_E';

-- W-9: No expiry but update on change
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, rule_parameters, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, '{"update_on_change": true}'::jsonb, 0, FALSE, 'IRS regulations', 'W-9 has no expiration but must be updated if information changes'
FROM "ob-poc".document_types WHERE type_code = 'W9';

-- FATCA_SELF_CERT: 3 years or until change
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, rule_parameters, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'VALIDITY_YEARS', 3, 'years', '{"update_on_change": true}'::jsonb, 90, TRUE, 'FATCA regulations', 'FATCA self-cert valid unless circumstances change'
FROM "ob-poc".document_types WHERE type_code = 'FATCA_SELF_CERT';

-- CRS_SELF_CERT: 3 years or until change
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, rule_parameters, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'VALIDITY_YEARS', 3, 'years', '{"update_on_change": true}'::jsonb, 90, TRUE, 'CRS regulations', 'CRS self-cert valid unless circumstances change'
FROM "ob-poc".document_types WHERE type_code = 'CRS_SELF_CERT';

-- TAX_RESIDENCY_CERT: Annual renewal recommended
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 60, FALSE, 'Tax residency certificates typically annual'
FROM "ob-poc".document_types WHERE type_code = 'TAX_RESIDENCY_CERT';

-- ============================================================================
-- REGULATORY DOCUMENTS
-- ============================================================================

-- LEI_CERTIFICATE: Annual renewal required
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 60, TRUE, 'GLEIF regulations', 'LEI must be renewed annually'
FROM "ob-poc".document_types WHERE type_code = 'LEI_CERTIFICATE';

-- REGULATORY_LICENSE: Check expiry
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 90, TRUE, 'Regulatory licenses must be current'
FROM "ob-poc".document_types WHERE type_code = 'REGULATORY_LICENSE';

-- AML_POLICY: Annual review recommended
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 60, FALSE, 'AML regulations', 'AML policies should be reviewed annually'
FROM "ob-poc".document_types WHERE type_code = 'AML_POLICY';

-- SANCTIONS_CERT: Annual renewal
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 30, TRUE, 'Sanctions certifications should be annual'
FROM "ob-poc".document_types WHERE type_code = 'SANCTIONS_CERT';

-- ============================================================================
-- FUND DOCUMENTS
-- ============================================================================

-- NAV_STATEMENT: 3 months
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 3, 'months', 30, FALSE, 'NAV statements should be recent'
FROM "ob-poc".document_types WHERE type_code = 'NAV_STATEMENT';

-- FUND_PROSPECTUS: Annual update typically
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 60, FALSE, 'UCITS/SEC regulations', 'Prospectus should be current version'
FROM "ob-poc".document_types WHERE type_code = 'FUND_PROSPECTUS';

-- KIID/KID: Annual update required
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, regulatory_source, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 30, TRUE, 'PRIIPs/UCITS regulations', 'KID/KIID must be updated annually'
FROM "ob-poc".document_types WHERE type_code = 'KIID';

-- ============================================================================
-- TRUST DOCUMENTS
-- ============================================================================

-- TRUST_DEED: Never expires (foundational)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'Trust deeds do not expire (may be amended)'
FROM "ob-poc".document_types WHERE type_code = 'TRUST_DEED';

-- LETTER_OF_WISHES: Superseded by newer version
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, rule_parameters, warning_days, is_hard_requirement, notes)
SELECT type_id, 'SUPERSEDED_BY_EVENT', NULL, NULL, '{"superseded_by": "newer_letter"}'::jsonb, 0, FALSE, 'Valid until replaced by updated letter'
FROM "ob-poc".document_types WHERE type_code = 'LETTER_OF_WISHES';

-- ============================================================================
-- ISDA DOCUMENTS
-- ============================================================================

-- ISDA_MASTER: Never expires (but may be amended)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'ISDA Master Agreements do not expire'
FROM "ob-poc".document_types WHERE type_code = 'ISDA_MASTER';

-- CSA: Never expires
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'NO_EXPIRY', NULL, NULL, 0, FALSE, 'Credit Support Annexes do not expire'
FROM "ob-poc".document_types WHERE type_code IN ('CSA', 'VM_CSA', 'IM_CSA');

-- ISDA_LEGAL_OPINION: Refresh recommended every 3 years
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 36, 'months', 90, FALSE, 'Legal opinions should be periodically refreshed'
FROM "ob-poc".document_types WHERE type_code = 'ISDA_LEGAL_OPINION';

-- ============================================================================
-- INSURANCE DOCUMENTS
-- ============================================================================

-- Insurance certificates: Check expiry (annual policies)
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 60, TRUE, 'Insurance must be current'
FROM "ob-poc".document_types WHERE type_code IN ('D_AND_O_INSURANCE', 'PI_INSURANCE', 'FIDELITY_BOND', 'CYBER_INSURANCE');

INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'ANNUAL_RENEWAL', 1, 'years', 60, TRUE, 'Insurance policies typically annual'
FROM "ob-poc".document_types WHERE type_code IN ('D_AND_O_INSURANCE', 'PI_INSURANCE', 'FIDELITY_BOND', 'CYBER_INSURANCE');

-- ============================================================================
-- EMPLOYMENT DOCUMENTS
-- ============================================================================

-- PAY_SLIP: 3 months max age
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 3, 'months', 30, TRUE, 'Pay slips should be recent'
FROM "ob-poc".document_types WHERE type_code = 'PAY_SLIP';

-- EMPLOYMENT_LETTER: 6 months recommended
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'MAX_AGE_MONTHS', 6, 'months', 30, FALSE, 'Employment letters should be reasonably recent'
FROM "ob-poc".document_types WHERE type_code = 'EMPLOYMENT_LETTER';

-- PROFESSIONAL_LICENSE: Check expiry
INSERT INTO "ob-poc".document_validity_rules (document_type_id, rule_type, rule_value, rule_unit, warning_days, is_hard_requirement, notes)
SELECT type_id, 'CHECK_EXPIRY', NULL, NULL, 60, TRUE, 'Professional licenses must be current'
FROM "ob-poc".document_types WHERE type_code = 'PROFESSIONAL_LICENSE';

COMMIT;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Count rules by type
SELECT rule_type, COUNT(*) as count 
FROM "ob-poc".document_validity_rules 
GROUP BY rule_type 
ORDER BY count DESC;

-- Rules with jurisdiction restrictions
SELECT 
    dt.type_code,
    dvr.rule_type,
    dvr.rule_value,
    dvr.rule_unit,
    dvr.applies_to_jurisdictions,
    dvr.warning_days,
    dvr.is_hard_requirement
FROM "ob-poc".document_validity_rules dvr
JOIN "ob-poc".document_types dt ON dvr.document_type_id = dt.type_id
WHERE dvr.applies_to_jurisdictions IS NOT NULL
ORDER BY dt.type_code;

-- Documents with multiple rules
SELECT 
    dt.type_code,
    COUNT(*) as rule_count,
    array_agg(dvr.rule_type) as rules
FROM "ob-poc".document_validity_rules dvr
JOIN "ob-poc".document_types dt ON dvr.document_type_id = dt.type_id
GROUP BY dt.type_code
HAVING COUNT(*) > 1
ORDER BY rule_count DESC;
