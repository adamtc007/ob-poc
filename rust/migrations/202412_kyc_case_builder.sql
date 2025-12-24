-- KYC Case Builder Extension
-- Adds regulatory profiles, service contexts, sponsor support, and fund investor tracking

-- =============================================================================
-- 1. REGULATORS REFERENCE DATA
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".regulators (
    regulator_code VARCHAR(20) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(10) NOT NULL,
    tier VARCHAR(20) NOT NULL DEFAULT 'NONE',
    registry_url VARCHAR(500),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".regulatory_tiers (
    tier_code VARCHAR(20) PRIMARY KEY,
    description TEXT,
    allows_simplified_dd BOOLEAN DEFAULT FALSE,
    requires_enhanced_screening BOOLEAN DEFAULT FALSE,
    reliance_level VARCHAR(20) DEFAULT 'none',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed regulatory tiers
INSERT INTO "ob-poc".regulatory_tiers (tier_code, description, allows_simplified_dd, requires_enhanced_screening, reliance_level)
VALUES
    ('EQUIVALENT', 'Full reliance permitted - equivalent regulatory standard', TRUE, FALSE, 'full'),
    ('ACCEPTABLE', 'Partial reliance - enhanced checks required', TRUE, TRUE, 'partial'),
    ('LIMITED', 'Limited reliance - additional verification required', FALSE, TRUE, 'limited'),
    ('NONE', 'No reliance - full KYC required regardless of regulation', FALSE, TRUE, 'none')
ON CONFLICT (tier_code) DO NOTHING;

-- Seed key regulators
INSERT INTO "ob-poc".regulators (regulator_code, name, jurisdiction, tier, registry_url)
VALUES
    ('CSSF', 'Commission de Surveillance du Secteur Financier', 'LU', 'EQUIVALENT', 'https://www.cssf.lu/en/supervised-entities/'),
    ('CBI', 'Central Bank of Ireland', 'IE', 'EQUIVALENT', 'https://registers.centralbank.ie/'),
    ('FCA', 'Financial Conduct Authority', 'GB', 'EQUIVALENT', 'https://register.fca.org.uk/s/'),
    ('SEC', 'Securities and Exchange Commission', 'US', 'EQUIVALENT', 'https://www.sec.gov/cgi-bin/browse-edgar'),
    ('BaFin', 'Bundesanstalt für Finanzdienstleistungsaufsicht', 'DE', 'EQUIVALENT', 'https://portal.mvp.bafin.de/database/InstInfo/'),
    ('FINMA', 'Swiss Financial Market Supervisory Authority', 'CH', 'EQUIVALENT', 'https://www.finma.ch/en/authorisation/'),
    ('MAS', 'Monetary Authority of Singapore', 'SG', 'EQUIVALENT', 'https://eservices.mas.gov.sg/fid'),
    ('SFC', 'Securities and Futures Commission', 'HK', 'EQUIVALENT', 'https://www.sfc.hk/publicregWeb/searchByName'),
    ('ASIC', 'Australian Securities and Investments Commission', 'AU', 'EQUIVALENT', 'https://connectonline.asic.gov.au/'),
    ('CIMA', 'Cayman Islands Monetary Authority', 'KY', 'ACCEPTABLE', 'https://www.cima.ky/regulated-entities'),
    ('BMA', 'Bermuda Monetary Authority', 'BM', 'ACCEPTABLE', 'https://www.bma.bm/regulated-entities')
ON CONFLICT (regulator_code) DO NOTHING;

-- =============================================================================
-- 2. ENTITY REGULATORY PROFILES
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_regulatory_profiles (
    entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    is_regulated BOOLEAN DEFAULT FALSE,
    regulator_code VARCHAR(20) REFERENCES "ob-poc".regulators(regulator_code),
    registration_number VARCHAR(100),
    registration_verified BOOLEAN DEFAULT FALSE,
    verification_date DATE,
    verification_method VARCHAR(50),
    verification_reference VARCHAR(500),
    regulatory_tier VARCHAR(20) DEFAULT 'NONE' REFERENCES "ob-poc".regulatory_tiers(tier_code),
    next_verification_due DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entity_reg_profile_regulated
    ON "ob-poc".entity_regulatory_profiles(is_regulated);
CREATE INDEX IF NOT EXISTS idx_entity_reg_profile_regulator
    ON "ob-poc".entity_regulatory_profiles(regulator_code);

-- =============================================================================
-- 3. ROLE TYPES REFERENCE DATA
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".role_types (
    role_code VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    triggers_full_kyc BOOLEAN DEFAULT FALSE,
    triggers_screening BOOLEAN DEFAULT FALSE,
    triggers_id_verification BOOLEAN DEFAULT FALSE,
    check_regulatory_status BOOLEAN DEFAULT FALSE,
    if_regulated_obligation VARCHAR(50),
    cascade_to_entity_ubos BOOLEAN DEFAULT FALSE,
    threshold_based BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed role types
INSERT INTO "ob-poc".role_types (role_code, name, description, triggers_full_kyc, triggers_screening, triggers_id_verification, check_regulatory_status, if_regulated_obligation, cascade_to_entity_ubos, threshold_based)
VALUES
    ('ACCOUNT_HOLDER', 'Account Holder', 'The primary legal entity holding the account', TRUE, TRUE, TRUE, FALSE, NULL, TRUE, FALSE),
    ('UBO', 'Ultimate Beneficial Owner', 'Natural person with 25%+ ownership or control', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, FALSE),
    ('CONTROLLER', 'Controller', 'Person with significant control (non-ownership)', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, FALSE),
    ('MANCO', 'Management Company', 'Fund management company (UCITS/AIF ManCo)', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, FALSE),
    ('INVESTMENT_MGR', 'Investment Manager', 'Discretionary investment manager', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, FALSE),
    ('DIRECTOR', 'Director', 'Board director of entity', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, FALSE),
    ('SIGNATORY', 'Authorized Signatory', 'Person authorized to sign on behalf of entity', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, FALSE),
    ('DELEGATE', 'Delegate/Service Provider', 'Third-party service provider with delegated functions', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, FALSE),
    ('INVESTOR', 'Fund Investor', 'Investor in a fund (TA context)', TRUE, TRUE, TRUE, TRUE, 'SIMPLIFIED', FALSE, TRUE),
    ('ASSET_OWNER', 'Asset Owner', 'Entity that owns the assets', TRUE, TRUE, TRUE, FALSE, NULL, TRUE, FALSE),
    ('PRINCIPAL', 'Principal', 'Principal shareholder or partner', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, FALSE),
    ('BENEFICIAL_OWNER', 'Beneficial Owner', 'Beneficial owner (may be below UBO threshold)', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, FALSE)
ON CONFLICT (role_code) DO NOTHING;

-- =============================================================================
-- 4. SERVICE CONTEXT
-- =============================================================================

-- Service contexts for a CBU
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_contexts (
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    service_context VARCHAR(50) NOT NULL,
    effective_date DATE DEFAULT CURRENT_DATE,
    PRIMARY KEY (cbu_id, service_context)
);

-- =============================================================================
-- 5. KYC SERVICE AGREEMENTS (for KYC-as-a-Service)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_service_agreements (
    agreement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sponsor_cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    sponsor_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    agreement_reference VARCHAR(100),
    effective_date DATE NOT NULL,
    termination_date DATE,
    kyc_standard VARCHAR(50) NOT NULL DEFAULT 'BNY_STANDARD',
    auto_accept_threshold VARCHAR(50),
    sponsor_review_required BOOLEAN DEFAULT TRUE,
    target_turnaround_days INTEGER DEFAULT 5,
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_kyc_agreement_sponsor
    ON "ob-poc".kyc_service_agreements(sponsor_cbu_id);

-- =============================================================================
-- 6. EXTEND KYC CASES FOR SERVICE CONTEXT
-- =============================================================================

-- Add columns to kyc.cases if they don't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'kyc' AND table_name = 'cases' AND column_name = 'service_context') THEN
        ALTER TABLE kyc.cases ADD COLUMN service_context VARCHAR(50);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'kyc' AND table_name = 'cases' AND column_name = 'sponsor_cbu_id') THEN
        ALTER TABLE kyc.cases ADD COLUMN sponsor_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'kyc' AND table_name = 'cases' AND column_name = 'service_agreement_id') THEN
        ALTER TABLE kyc.cases ADD COLUMN service_agreement_id UUID REFERENCES "ob-poc".kyc_service_agreements(agreement_id);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'kyc' AND table_name = 'cases' AND column_name = 'kyc_standard') THEN
        ALTER TABLE kyc.cases ADD COLUMN kyc_standard VARCHAR(50);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'kyc' AND table_name = 'cases' AND column_name = 'subject_entity_id') THEN
        ALTER TABLE kyc.cases ADD COLUMN subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id);
    END IF;
END $$;

-- =============================================================================
-- 7. SPONSOR DECISION TRACKING
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_case_sponsor_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    our_recommendation VARCHAR(50),
    our_recommendation_date TIMESTAMPTZ,
    our_recommendation_by UUID,
    our_findings JSONB,
    sponsor_decision VARCHAR(50),
    sponsor_decision_date TIMESTAMPTZ,
    sponsor_decision_by VARCHAR(255),
    sponsor_comments TEXT,
    final_status VARCHAR(50),
    effective_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sponsor_decision_case
    ON "ob-poc".kyc_case_sponsor_decisions(case_id);

-- =============================================================================
-- 8. FUND INVESTORS TABLE (TA Context)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".fund_investors (
    investor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fund_cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    investor_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    investor_type VARCHAR(50) NOT NULL,
    investment_amount DECIMAL(20,2),
    currency VARCHAR(3) DEFAULT 'EUR',
    subscription_date DATE,
    kyc_tier VARCHAR(50),
    kyc_status VARCHAR(50) DEFAULT 'PENDING',
    kyc_case_id UUID REFERENCES kyc.cases(case_id),
    last_kyc_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT uq_fund_investor UNIQUE (fund_cbu_id, investor_entity_id)
);

CREATE INDEX IF NOT EXISTS idx_fund_investor_fund
    ON "ob-poc".fund_investors(fund_cbu_id);
CREATE INDEX IF NOT EXISTS idx_fund_investor_entity
    ON "ob-poc".fund_investors(investor_entity_id);
CREATE INDEX IF NOT EXISTS idx_fund_investor_status
    ON "ob-poc".fund_investors(kyc_status);

-- =============================================================================
-- 9. ADD KYC CONFIG TO PRODUCTS
-- =============================================================================

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'products' AND column_name = 'kyc_risk_rating') THEN
        ALTER TABLE "ob-poc".products ADD COLUMN kyc_risk_rating VARCHAR(20);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'products' AND column_name = 'kyc_context') THEN
        ALTER TABLE "ob-poc".products ADD COLUMN kyc_context VARCHAR(50);
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'products' AND column_name = 'requires_kyc') THEN
        ALTER TABLE "ob-poc".products ADD COLUMN requires_kyc BOOLEAN DEFAULT TRUE;
    END IF;
END $$;

-- Update product KYC config
UPDATE "ob-poc".products SET kyc_risk_rating = 'HIGH', kyc_context = 'CUSTODY', requires_kyc = TRUE
    WHERE product_code IN ('CUSTODY', 'PRIME_BROKERAGE');
UPDATE "ob-poc".products SET kyc_risk_rating = 'MEDIUM', kyc_context = 'CUSTODY', requires_kyc = TRUE
    WHERE product_code IN ('FUND_ACCOUNTING', 'SECURITIES_LENDING');
UPDATE "ob-poc".products SET kyc_risk_rating = 'MEDIUM', kyc_context = 'TRANSFER_AGENT', requires_kyc = TRUE
    WHERE product_code = 'TRANSFER_AGENCY';
UPDATE "ob-poc".products SET kyc_risk_rating = 'LOW', requires_kyc = FALSE
    WHERE product_code = 'REPORTING_ONLY';

-- =============================================================================
-- 10. ADD KYC SCOPE TEMPLATE TO CBUS
-- =============================================================================

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'ob-poc' AND table_name = 'cbus' AND column_name = 'kyc_scope_template') THEN
        ALTER TABLE "ob-poc".cbus ADD COLUMN kyc_scope_template VARCHAR(50);
    END IF;
END $$;

-- =============================================================================
-- 11. VIEWS
-- =============================================================================

-- View: Entities with regulatory profiles
CREATE OR REPLACE VIEW "ob-poc".v_entity_regulatory_status AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    et.name AS entity_type,
    COALESCE(erp.is_regulated, FALSE) AS is_regulated,
    erp.regulator_code,
    r.name AS regulator_name,
    erp.registration_number,
    erp.registration_verified,
    erp.regulatory_tier,
    rt.allows_simplified_dd,
    rt.requires_enhanced_screening
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_regulatory_profiles erp ON e.entity_id = erp.entity_id
LEFT JOIN "ob-poc".regulators r ON erp.regulator_code = r.regulator_code
LEFT JOIN "ob-poc".regulatory_tiers rt ON erp.regulatory_tier = rt.tier_code;

-- View: CBU entities with roles and KYC obligations
CREATE OR REPLACE VIEW "ob-poc".v_cbu_kyc_scope AS
SELECT
    c.cbu_id,
    c.name AS cbu_name,
    c.client_type,
    c.kyc_scope_template,
    cer.entity_id,
    e.name AS entity_name,
    et.name AS entity_type,
    ro.name AS role_name,
    ro.role_id,
    rtypes.role_code,
    rtypes.triggers_full_kyc,
    rtypes.triggers_screening,
    rtypes.triggers_id_verification,
    rtypes.check_regulatory_status,
    rtypes.if_regulated_obligation,
    COALESCE(erp.is_regulated, FALSE) AS is_regulated,
    erp.regulator_code,
    erp.registration_verified,
    erp.regulatory_tier,
    COALESCE(regtier.allows_simplified_dd, FALSE) AS allows_simplified_dd
FROM "ob-poc".cbus c
JOIN "ob-poc".cbu_entity_roles cer ON c.cbu_id = cer.cbu_id
JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
JOIN "ob-poc".roles ro ON cer.role_id = ro.role_id
LEFT JOIN "ob-poc".role_types rtypes ON UPPER(ro.name) = rtypes.role_code
LEFT JOIN "ob-poc".entity_regulatory_profiles erp ON e.entity_id = erp.entity_id
LEFT JOIN "ob-poc".regulatory_tiers regtier ON erp.regulatory_tier = regtier.tier_code;
