-- Threshold Matrix Seed Data
-- Risk factors, bands, and requirements for KYC

-- =============================================================================
-- RISK BANDS
-- =============================================================================
INSERT INTO "ob-poc".risk_bands (band_code, min_score, max_score, description, escalation_required, review_frequency_months) VALUES
('LOW', 0, 3, 'Low risk - standard due diligence', false, 24),
('MEDIUM', 4, 6, 'Medium risk - enhanced monitoring', false, 12),
('HIGH', 7, 9, 'High risk - enhanced due diligence', true, 6),
('ENHANCED', 10, 99, 'Enhanced risk - senior approval required', true, 3)
ON CONFLICT (band_code) DO UPDATE SET
    min_score = EXCLUDED.min_score,
    max_score = EXCLUDED.max_score,
    description = EXCLUDED.description,
    escalation_required = EXCLUDED.escalation_required,
    review_frequency_months = EXCLUDED.review_frequency_months;

-- =============================================================================
-- CBU TYPE FACTORS
-- =============================================================================
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('CBU_TYPE', 'LUXSICAV_UCITS', 1, 'Luxembourg SICAV - UCITS regulated'),
('CBU_TYPE', 'LUXSICAV_PART2', 2, 'Luxembourg SICAV - Part II'),
('CBU_TYPE', 'HEDGE_FUND', 3, 'Hedge fund'),
('CBU_TYPE', '40_ACT_FUND', 1, 'US 40 Act fund - SEC regulated'),
('CBU_TYPE', 'FAMILY_TRUST', 2, 'Family trust'),
('CBU_TYPE', 'TRADING_COMPANY', 3, 'Trading company'),
('CBU_TYPE', 'PENSION_FUND', 1, 'Pension fund - regulated'),
('CBU_TYPE', 'CORPORATE', 2, 'Corporate entity'),
('CBU_TYPE', 'FUND', 2, 'Generic fund'),
('CBU_TYPE', 'SPV', 3, 'Special purpose vehicle'),
('CBU_TYPE', 'PRIVATE_EQUITY', 2, 'Private equity fund'),
('CBU_TYPE', 'REAL_ESTATE_FUND', 2, 'Real estate investment fund')
ON CONFLICT (factor_type, factor_code) DO UPDATE SET
    risk_weight = EXCLUDED.risk_weight,
    description = EXCLUDED.description;

-- =============================================================================
-- SOURCE OF FUNDS FACTORS
-- =============================================================================
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('SOURCE_OF_FUNDS', 'REGULATED_INSTITUTION', 0, 'Regulated bank/insurer'),
('SOURCE_OF_FUNDS', 'INSTITUTIONAL_INVESTOR', 1, 'Pension, SWF, endowment'),
('SOURCE_OF_FUNDS', 'PRIVATE_WEALTH', 2, 'HNWI, family office'),
('SOURCE_OF_FUNDS', 'CORPORATE', 2, 'Corporate treasury'),
('SOURCE_OF_FUNDS', 'UNKNOWN', 4, 'Not yet determined'),
('SOURCE_OF_FUNDS', 'MIXED', 2, 'Multiple sources'),
('SOURCE_OF_FUNDS', 'INHERITANCE', 2, 'Inherited wealth'),
('SOURCE_OF_FUNDS', 'BUSINESS_SALE', 2, 'Proceeds from business sale')
ON CONFLICT (factor_type, factor_code) DO UPDATE SET
    risk_weight = EXCLUDED.risk_weight,
    description = EXCLUDED.description;

-- =============================================================================
-- NATURE/PURPOSE FACTORS
-- =============================================================================
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('NATURE_PURPOSE', 'LONG_ONLY', 1, 'Long-only investment'),
('NATURE_PURPOSE', 'LEVERAGED_TRADING', 3, 'Leveraged/short strategies'),
('NATURE_PURPOSE', 'REAL_ESTATE', 2, 'Real estate investment'),
('NATURE_PURPOSE', 'PRIVATE_EQUITY', 2, 'PE/VC'),
('NATURE_PURPOSE', 'HOLDING', 2, 'Holding structure'),
('NATURE_PURPOSE', 'TRADING', 2, 'Active trading'),
('NATURE_PURPOSE', 'TREASURY', 1, 'Treasury management'),
('NATURE_PURPOSE', 'WEALTH_PRESERVATION', 1, 'Wealth preservation')
ON CONFLICT (factor_type, factor_code) DO UPDATE SET
    risk_weight = EXCLUDED.risk_weight,
    description = EXCLUDED.description;

-- =============================================================================
-- JURISDICTION FACTORS
-- =============================================================================
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('JURISDICTION', 'LU', 0, 'Luxembourg - EU regulated'),
('JURISDICTION', 'IE', 0, 'Ireland - EU regulated'),
('JURISDICTION', 'GB', 1, 'United Kingdom'),
('JURISDICTION', 'US', 1, 'United States'),
('JURISDICTION', 'CH', 1, 'Switzerland'),
('JURISDICTION', 'SG', 1, 'Singapore'),
('JURISDICTION', 'HK', 1, 'Hong Kong'),
('JURISDICTION', 'KY', 2, 'Cayman Islands'),
('JURISDICTION', 'BVI', 3, 'British Virgin Islands'),
('JURISDICTION', 'JE', 2, 'Jersey'),
('JURISDICTION', 'GG', 2, 'Guernsey'),
('JURISDICTION', 'OTHER_HIGH_RISK', 4, 'Other high-risk jurisdiction')
ON CONFLICT (factor_type, factor_code) DO UPDATE SET
    risk_weight = EXCLUDED.risk_weight,
    description = EXCLUDED.description;

-- =============================================================================
-- PRODUCT RISK FACTORS
-- =============================================================================
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('PRODUCT_RISK', 'CUSTODY', 3, 'Custody - high risk'),
('PRODUCT_RISK', 'FUND_ACCOUNTING', 1, 'Fund accounting - low risk'),
('PRODUCT_RISK', 'TRANSFER_AGENCY', 2, 'Transfer agency - medium risk'),
('PRODUCT_RISK', 'MIDDLE_OFFICE', 2, 'Middle office - medium risk'),
('PRODUCT_RISK', 'COLLATERAL_MANAGEMENT', 2, 'Collateral management'),
('PRODUCT_RISK', 'PRIME_BROKERAGE', 3, 'Prime brokerage - high risk'),
('PRODUCT_RISK', 'FX', 2, 'FX services')
ON CONFLICT (factor_type, factor_code) DO UPDATE SET
    risk_weight = EXCLUDED.risk_weight,
    description = EXCLUDED.description;

-- =============================================================================
-- UBO REQUIREMENTS - LOW RISK
-- =============================================================================
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('UBO', 'LOW', 'identity', true, 0.90, NULL, false),
('UBO', 'LOW', 'address', true, 0.85, 365, false),
('UBO', 'LOW', 'date_of_birth', true, 0.90, NULL, false),
('UBO', 'LOW', 'nationality', true, 0.85, NULL, false)
ON CONFLICT (entity_role, risk_band, attribute_code) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    confidence_min = EXCLUDED.confidence_min,
    max_age_days = EXCLUDED.max_age_days,
    must_be_authoritative = EXCLUDED.must_be_authoritative;

-- =============================================================================
-- UBO REQUIREMENTS - MEDIUM RISK
-- =============================================================================
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('UBO', 'MEDIUM', 'identity', true, 0.90, NULL, false),
('UBO', 'MEDIUM', 'address', true, 0.85, 180, false),
('UBO', 'MEDIUM', 'date_of_birth', true, 0.90, NULL, false),
('UBO', 'MEDIUM', 'nationality', true, 0.85, NULL, false),
('UBO', 'MEDIUM', 'source_of_wealth', false, 0.80, NULL, false)
ON CONFLICT (entity_role, risk_band, attribute_code) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    confidence_min = EXCLUDED.confidence_min,
    max_age_days = EXCLUDED.max_age_days,
    must_be_authoritative = EXCLUDED.must_be_authoritative;

-- =============================================================================
-- UBO REQUIREMENTS - HIGH RISK
-- =============================================================================
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('UBO', 'HIGH', 'identity', true, 0.95, NULL, true),
('UBO', 'HIGH', 'address', true, 0.90, 90, false),
('UBO', 'HIGH', 'date_of_birth', true, 0.95, NULL, true),
('UBO', 'HIGH', 'nationality', true, 0.90, NULL, false),
('UBO', 'HIGH', 'source_of_wealth', true, 0.85, NULL, false),
('UBO', 'HIGH', 'tax_residence', true, 0.85, 365, false)
ON CONFLICT (entity_role, risk_band, attribute_code) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    confidence_min = EXCLUDED.confidence_min,
    max_age_days = EXCLUDED.max_age_days,
    must_be_authoritative = EXCLUDED.must_be_authoritative;

-- =============================================================================
-- UBO REQUIREMENTS - ENHANCED RISK
-- =============================================================================
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('UBO', 'ENHANCED', 'identity', true, 0.98, NULL, true),
('UBO', 'ENHANCED', 'address', true, 0.95, 60, true),
('UBO', 'ENHANCED', 'date_of_birth', true, 0.98, NULL, true),
('UBO', 'ENHANCED', 'nationality', true, 0.95, NULL, true),
('UBO', 'ENHANCED', 'source_of_wealth', true, 0.90, NULL, false),
('UBO', 'ENHANCED', 'source_of_funds', true, 0.90, NULL, false),
('UBO', 'ENHANCED', 'tax_residence', true, 0.90, 180, false)
ON CONFLICT (entity_role, risk_band, attribute_code) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    confidence_min = EXCLUDED.confidence_min,
    max_age_days = EXCLUDED.max_age_days,
    must_be_authoritative = EXCLUDED.must_be_authoritative;

-- =============================================================================
-- DIRECTOR REQUIREMENTS
-- =============================================================================
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('DIRECTOR', 'LOW', 'identity', true, 0.85, NULL, false),
('DIRECTOR', 'LOW', 'address', true, 0.80, 365, false),
('DIRECTOR', 'MEDIUM', 'identity', true, 0.90, NULL, false),
('DIRECTOR', 'MEDIUM', 'address', true, 0.85, 180, false),
('DIRECTOR', 'HIGH', 'identity', true, 0.95, NULL, true),
('DIRECTOR', 'HIGH', 'address', true, 0.90, 90, false),
('DIRECTOR', 'ENHANCED', 'identity', true, 0.98, NULL, true),
('DIRECTOR', 'ENHANCED', 'address', true, 0.95, 60, true)
ON CONFLICT (entity_role, risk_band, attribute_code) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    confidence_min = EXCLUDED.confidence_min,
    max_age_days = EXCLUDED.max_age_days,
    must_be_authoritative = EXCLUDED.must_be_authoritative;

-- =============================================================================
-- SHAREHOLDER REQUIREMENTS
-- =============================================================================
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('SHAREHOLDER', 'LOW', 'identity', true, 0.85, NULL, false),
('SHAREHOLDER', 'MEDIUM', 'identity', true, 0.90, NULL, false),
('SHAREHOLDER', 'HIGH', 'identity', true, 0.95, NULL, true),
('SHAREHOLDER', 'HIGH', 'ownership_percentage', true, 0.90, 180, false),
('SHAREHOLDER', 'ENHANCED', 'identity', true, 0.98, NULL, true),
('SHAREHOLDER', 'ENHANCED', 'ownership_percentage', true, 0.95, 90, true)
ON CONFLICT (entity_role, risk_band, attribute_code) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    confidence_min = EXCLUDED.confidence_min,
    max_age_days = EXCLUDED.max_age_days,
    must_be_authoritative = EXCLUDED.must_be_authoritative;

-- =============================================================================
-- LINK REQUIREMENTS TO ACCEPTABLE DOCUMENT TYPES
-- =============================================================================

-- Identity proof documents (for natural persons)
INSERT INTO "ob-poc".requirement_acceptable_docs (requirement_id, document_type_code, priority)
SELECT r.requirement_id, dt.type_code,
    CASE dt.type_code 
        WHEN 'PASSPORT' THEN 1 
        WHEN 'NATIONAL_ID' THEN 2 
        WHEN 'DRIVERS_LICENSE' THEN 3
        ELSE 4 
    END
FROM "ob-poc".threshold_requirements r
CROSS JOIN "ob-poc".document_types dt
WHERE r.attribute_code = 'identity'
  AND dt.type_code IN ('PASSPORT', 'NATIONAL_ID', 'DRIVERS_LICENSE')
ON CONFLICT DO NOTHING;

-- Address proof documents
INSERT INTO "ob-poc".requirement_acceptable_docs (requirement_id, document_type_code, priority)
SELECT r.requirement_id, dt.type_code,
    CASE dt.type_code 
        WHEN 'UTILITY_BILL' THEN 1 
        WHEN 'BANK_STATEMENT' THEN 2 
        WHEN 'COUNCIL_TAX_BILL' THEN 3
        WHEN 'TENANCY_AGREEMENT' THEN 4
        ELSE 5 
    END
FROM "ob-poc".threshold_requirements r
CROSS JOIN "ob-poc".document_types dt
WHERE r.attribute_code = 'address'
  AND dt.type_code IN ('UTILITY_BILL', 'BANK_STATEMENT', 'COUNCIL_TAX_BILL', 'TENANCY_AGREEMENT')
ON CONFLICT DO NOTHING;

-- =============================================================================
-- SCREENING REQUIREMENTS
-- =============================================================================
INSERT INTO "ob-poc".screening_requirements (risk_band, screening_type, is_required, frequency_months) VALUES
('LOW', 'SANCTIONS', true, 12),
('LOW', 'PEP', true, 12),
('LOW', 'ADVERSE_MEDIA', false, 24),
('MEDIUM', 'SANCTIONS', true, 12),
('MEDIUM', 'PEP', true, 12),
('MEDIUM', 'ADVERSE_MEDIA', true, 12),
('HIGH', 'SANCTIONS', true, 6),
('HIGH', 'PEP', true, 6),
('HIGH', 'ADVERSE_MEDIA', true, 6),
('ENHANCED', 'SANCTIONS', true, 3),
('ENHANCED', 'PEP', true, 3),
('ENHANCED', 'ADVERSE_MEDIA', true, 3)
ON CONFLICT (risk_band, screening_type) DO UPDATE SET
    is_required = EXCLUDED.is_required,
    frequency_months = EXCLUDED.frequency_months;
