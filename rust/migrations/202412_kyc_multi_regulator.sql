-- KYC Multi-Regulator Support
-- Extends the single-regulator model to support dual-regulation, passporting, etc.
-- This migration builds on 202412_kyc_case_builder.sql

-- =============================================================================
-- 1. CREATE REFERENCE SCHEMA (if not exists)
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS ob_ref;
CREATE SCHEMA IF NOT EXISTS ob_kyc;

-- =============================================================================
-- 2. REGISTRATION TYPES (how entity relates to regulator)
-- =============================================================================

CREATE TABLE IF NOT EXISTS ob_ref.registration_types (
    registration_type VARCHAR(50) PRIMARY KEY,
    description VARCHAR(255) NOT NULL,
    is_primary BOOLEAN DEFAULT FALSE,
    allows_reliance BOOLEAN DEFAULT TRUE
);

INSERT INTO ob_ref.registration_types (registration_type, description, is_primary, allows_reliance)
VALUES
    ('PRIMARY', 'Primary/home state regulator', TRUE, TRUE),
    ('DUAL_CONDUCT', 'Dual regulation - conduct authority (e.g., FCA)', FALSE, TRUE),
    ('DUAL_PRUDENTIAL', 'Dual regulation - prudential authority (e.g., PRA)', FALSE, TRUE),
    ('PASSPORTED', 'EU/EEA passported registration', FALSE, TRUE),
    ('BRANCH', 'Branch registration in jurisdiction', FALSE, TRUE),
    ('SUBSIDIARY', 'Separate subsidiary registration', FALSE, TRUE),
    ('ADDITIONAL', 'Additional registration (same jurisdiction)', FALSE, TRUE),
    ('STATE', 'State/provincial registration (US, CA, AU)', FALSE, FALSE),
    ('SRO', 'Self-regulatory organization membership', FALSE, TRUE)
ON CONFLICT (registration_type) DO NOTHING;

-- =============================================================================
-- 3. COPY REGULATORS TO NEW SCHEMA (for consistency)
-- =============================================================================

-- Create reference copy in ob_ref
CREATE TABLE IF NOT EXISTS ob_ref.regulatory_tiers (
    tier_code VARCHAR(50) PRIMARY KEY,
    description VARCHAR(255) NOT NULL,
    allows_simplified_dd BOOLEAN DEFAULT FALSE,
    requires_enhanced_screening BOOLEAN DEFAULT FALSE
);

INSERT INTO ob_ref.regulatory_tiers (tier_code, description, allows_simplified_dd, requires_enhanced_screening)
SELECT tier_code, COALESCE(description, tier_code), allows_simplified_dd, requires_enhanced_screening
FROM "ob-poc".regulatory_tiers
ON CONFLICT (tier_code) DO NOTHING;

CREATE TABLE IF NOT EXISTS ob_ref.regulators (
    regulator_code VARCHAR(50) PRIMARY KEY,
    regulator_name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(2) NOT NULL,
    regulatory_tier VARCHAR(50) NOT NULL REFERENCES ob_ref.regulatory_tiers(tier_code),
    regulator_type VARCHAR(50) DEFAULT 'GOVERNMENT',
    registry_url VARCHAR(500),
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

INSERT INTO ob_ref.regulators (regulator_code, regulator_name, jurisdiction, regulatory_tier, regulator_type, registry_url)
SELECT regulator_code, name, jurisdiction, tier, 'GOVERNMENT', registry_url
FROM "ob-poc".regulators
ON CONFLICT (regulator_code) DO NOTHING;

-- Add additional regulators for dual-regulation support
INSERT INTO ob_ref.regulators (regulator_code, regulator_name, jurisdiction, regulatory_tier, regulator_type, registry_url)
VALUES
    ('PRA', 'Prudential Regulation Authority', 'GB', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('FINRA', 'Financial Industry Regulatory Authority', 'US', 'EQUIVALENT', 'SRO', 'https://brokercheck.finra.org/'),
    ('CFTC', 'Commodity Futures Trading Commission', 'US', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('OCC', 'Office of the Comptroller of the Currency', 'US', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('FDIC', 'Federal Deposit Insurance Corporation', 'US', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('AMF', 'Autorité des marchés financiers', 'FR', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('AFM', 'Autoriteit Financiële Markten', 'NL', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('CONSOB', 'Commissione Nazionale per le Società e la Borsa', 'IT', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('CNMV', 'Comisión Nacional del Mercado de Valores', 'ES', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('JFSA', 'Japan Financial Services Agency', 'JP', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('GFSC', 'Guernsey Financial Services Commission', 'GG', 'EQUIVALENT', 'GOVERNMENT', NULL),
    ('JFSC', 'Jersey Financial Services Commission', 'JE', 'EQUIVALENT', 'GOVERNMENT', NULL)
ON CONFLICT (regulator_code) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_ob_ref_regulators_jurisdiction ON ob_ref.regulators(jurisdiction);
CREATE INDEX IF NOT EXISTS idx_ob_ref_regulators_tier ON ob_ref.regulators(regulatory_tier);

-- =============================================================================
-- 4. MULTI-REGULATOR REGISTRATIONS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS ob_kyc.entity_regulatory_registrations (
    registration_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    regulator_code VARCHAR(50) NOT NULL REFERENCES ob_ref.regulators(regulator_code),

    -- Registration details
    registration_number VARCHAR(100),
    registration_type VARCHAR(50) NOT NULL REFERENCES ob_ref.registration_types(registration_type),
    activity_scope TEXT,

    -- For passporting/branch
    home_regulator_code VARCHAR(50) REFERENCES ob_ref.regulators(regulator_code),
    passport_reference VARCHAR(100),

    -- Verification
    registration_verified BOOLEAN DEFAULT FALSE,
    verification_date DATE,
    verification_method VARCHAR(50),
    verification_reference VARCHAR(500),
    verification_expires DATE,

    -- Status and validity
    status VARCHAR(50) DEFAULT 'ACTIVE',
    effective_date DATE DEFAULT CURRENT_DATE,
    expiry_date DATE,

    -- Audit
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    created_by UUID,
    updated_by UUID,

    -- Unique: one registration per entity per regulator
    CONSTRAINT uq_entity_regulator UNIQUE (entity_id, regulator_code)
);

CREATE INDEX IF NOT EXISTS idx_ereg_entity ON ob_kyc.entity_regulatory_registrations(entity_id);
CREATE INDEX IF NOT EXISTS idx_ereg_regulator ON ob_kyc.entity_regulatory_registrations(regulator_code);
CREATE INDEX IF NOT EXISTS idx_ereg_status ON ob_kyc.entity_regulatory_registrations(status);
CREATE INDEX IF NOT EXISTS idx_ereg_type ON ob_kyc.entity_regulatory_registrations(registration_type);
CREATE INDEX IF NOT EXISTS idx_ereg_verified ON ob_kyc.entity_regulatory_registrations(registration_verified);
CREATE INDEX IF NOT EXISTS idx_ereg_expires ON ob_kyc.entity_regulatory_registrations(verification_expires);

-- =============================================================================
-- 5. MIGRATE EXISTING DATA FROM SINGLE-REGULATOR TABLE
-- =============================================================================

-- Migrate existing entity_regulatory_profiles to multi-regulator table
INSERT INTO ob_kyc.entity_regulatory_registrations (
    entity_id,
    regulator_code,
    registration_number,
    registration_type,
    registration_verified,
    verification_date,
    verification_method,
    verification_reference,
    status,
    effective_date
)
SELECT
    entity_id,
    regulator_code,
    registration_number,
    'PRIMARY',
    registration_verified,
    verification_date,
    verification_method,
    verification_reference,
    'ACTIVE',
    COALESCE(verification_date, CURRENT_DATE)
FROM "ob-poc".entity_regulatory_profiles
WHERE is_regulated = TRUE AND regulator_code IS NOT NULL
ON CONFLICT (entity_id, regulator_code) DO NOTHING;

-- =============================================================================
-- 6. COPY ROLE TYPES TO NEW SCHEMA
-- =============================================================================

CREATE TABLE IF NOT EXISTS ob_ref.role_types (
    role_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(50),
    -- KYC triggers
    triggers_full_kyc BOOLEAN DEFAULT FALSE,
    triggers_screening BOOLEAN DEFAULT FALSE,
    triggers_id_verification BOOLEAN DEFAULT FALSE,
    -- Regulatory check behavior
    check_regulatory_status BOOLEAN DEFAULT FALSE,
    if_regulated_obligation VARCHAR(50),
    -- Cascade behavior
    cascade_to_entity_ubos BOOLEAN DEFAULT FALSE,
    -- Threshold-based (for investors)
    threshold_based BOOLEAN DEFAULT FALSE,
    -- Metadata
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Copy from ob-poc.role_types to ob_ref.role_types
INSERT INTO ob_ref.role_types (
    code, name, description, category,
    triggers_full_kyc, triggers_screening, triggers_id_verification,
    check_regulatory_status, if_regulated_obligation,
    cascade_to_entity_ubos, threshold_based
)
SELECT
    role_code, name, description,
    CASE
        WHEN role_code IN ('ACCOUNT_HOLDER', 'ASSET_OWNER', 'PRINCIPAL') THEN 'PRINCIPAL'
        WHEN role_code IN ('UBO', 'CONTROLLER', 'BENEFICIAL_OWNER') THEN 'OWNERSHIP'
        WHEN role_code IN ('MANCO', 'INVESTMENT_MGR', 'DELEGATE') THEN 'DELEGATE'
        WHEN role_code IN ('DIRECTOR') THEN 'GOVERNANCE'
        WHEN role_code IN ('SIGNATORY') THEN 'AUTHORITY'
        WHEN role_code IN ('INVESTOR') THEN 'INVESTOR'
        ELSE 'OTHER'
    END,
    triggers_full_kyc, triggers_screening, triggers_id_verification,
    check_regulatory_status, if_regulated_obligation,
    cascade_to_entity_ubos, threshold_based
FROM "ob-poc".role_types
ON CONFLICT (code) DO NOTHING;

-- Add additional role types
INSERT INTO ob_ref.role_types (code, name, category, triggers_full_kyc, triggers_screening, triggers_id_verification, check_regulatory_status, if_regulated_obligation, cascade_to_entity_ubos, description)
VALUES
    ('JOINT_HOLDER', 'Joint Account Holder', 'PRINCIPAL', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, 'Joint account holder'),
    ('PARENT_COMPANY', 'Parent Company', 'OWNERSHIP', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Corporate parent'),
    ('PRIME_BROKER', 'Prime Broker', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Prime brokerage provider'),
    ('AUTHORIZED_PERSON', 'Authorized Person', 'AUTHORITY', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, 'Person with power of attorney'),
    ('CONDUCTING_OFFICER', 'Conducting Officer', 'GOVERNANCE', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, 'Luxembourg conducting officer'),
    ('CUSTODIAN', 'Custodian', 'DELEGATE', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Assets custodian'),
    ('DEPOSITARY', 'Depositary', 'DELEGATE', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Fund depositary'),
    ('ADMINISTRATOR', 'Fund Administrator', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Fund administrator'),
    ('TRANSFER_AGENT', 'Transfer Agent', 'DELEGATE', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Transfer agent/registrar'),
    ('AUDITOR', 'Auditor', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'External auditor'),
    ('LEGAL_COUNSEL', 'Legal Counsel', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Legal advisor'),
    ('SUBSIDIARY', 'Subsidiary', 'OWNERSHIP', FALSE, TRUE, FALSE, FALSE, NULL, FALSE, 'Corporate subsidiary'),
    ('NOMINEE', 'Nominee', 'INVESTOR', FALSE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Nominee/omnibus account')
ON CONFLICT (code) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_ob_ref_role_types_code ON ob_ref.role_types(code);
CREATE INDEX IF NOT EXISTS idx_ob_ref_role_types_category ON ob_ref.role_types(category);

-- =============================================================================
-- 7. VIEWS
-- =============================================================================

-- View: Entity regulatory summary (multi-regulator)
CREATE OR REPLACE VIEW ob_kyc.v_entity_regulatory_summary AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    COUNT(r.registration_id) AS registration_count,
    COUNT(r.registration_id) FILTER (WHERE r.registration_verified AND r.status = 'ACTIVE') AS verified_count,
    BOOL_OR(r.registration_verified AND r.status = 'ACTIVE' AND rt.allows_simplified_dd) AS allows_simplified_dd,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (WHERE r.status = 'ACTIVE') AS active_regulators,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (WHERE r.registration_verified AND r.status = 'ACTIVE') AS verified_regulators,
    MAX(r.verification_date) AS last_verified,
    MIN(r.verification_expires) FILTER (WHERE r.verification_expires > CURRENT_DATE) AS next_expiry
FROM "ob-poc".entities e
LEFT JOIN ob_kyc.entity_regulatory_registrations r ON e.entity_id = r.entity_id
LEFT JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
LEFT JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
GROUP BY e.entity_id, e.name;

-- =============================================================================
-- 8. HELPER FUNCTION
-- =============================================================================

-- Function: Check if entity allows simplified due diligence
CREATE OR REPLACE FUNCTION ob_kyc.entity_allows_simplified_dd(p_entity_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM ob_kyc.entity_regulatory_registrations r
        JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
        JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
        WHERE r.entity_id = p_entity_id
          AND r.status = 'ACTIVE'
          AND r.registration_verified = TRUE
          AND rt.allows_simplified_dd = TRUE
    );
END;
$$ LANGUAGE plpgsql STABLE;
