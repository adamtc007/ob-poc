-- ═══════════════════════════════════════════════════════════════════════════
-- ROLE TAXONOMY V2.0 - Comprehensive Role Structure
-- ═══════════════════════════════════════════════════════════════════════════
--
-- This migration establishes a validated role taxonomy covering:
--   - Asset managers (Allianz model)
--   - Hedge funds (master-feeder)
--   - Private equity (LP structures)
--   - Trusts (discretionary, purpose, unit)
--   - Prime broker → retail chains
--
-- Key changes:
--   1. Expanded role_category enum
--   2. Added visualization and UBO behavior metadata
--   3. Validation rules for role combinations
--   4. Entity type compatibility constraints
--
-- ═══════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 1: EXTEND ROLES TABLE SCHEMA
-- ═══════════════════════════════════════════════════════════════════════════

-- Add new columns if they don't exist
DO $$
BEGIN
    -- Role category (expanded)
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'role_category') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN role_category VARCHAR(30);
    END IF;

    -- Visualization layout hint
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'layout_category') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN layout_category VARCHAR(30);
    END IF;

    -- UBO calculation behavior
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'ubo_treatment') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN ubo_treatment VARCHAR(30);
    END IF;

    -- Whether this role requires ownership percentage
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'requires_percentage') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN requires_percentage BOOLEAN DEFAULT FALSE;
    END IF;

    -- Whether this role can only be held by natural persons
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'natural_person_only') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN natural_person_only BOOLEAN DEFAULT FALSE;
    END IF;

    -- Whether this role can only be held by legal entities
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'legal_entity_only') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN legal_entity_only BOOLEAN DEFAULT FALSE;
    END IF;

    -- Compatible entity categories (JSON array)
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'compatible_entity_categories') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN compatible_entity_categories JSONB;
    END IF;

    -- Display priority for visualization (higher = closer to apex/center)
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'display_priority') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN display_priority INTEGER DEFAULT 50;
    END IF;

    -- KYC obligation level
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'kyc_obligation') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN kyc_obligation VARCHAR(30) DEFAULT 'FULL_KYC';
    END IF;

    -- Whether role is active/available for use
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'is_active') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN is_active BOOLEAN DEFAULT TRUE;
    END IF;

    -- Sort order within category
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_schema = 'ob-poc' AND table_name = 'roles' 
                   AND column_name = 'sort_order') THEN
        ALTER TABLE "ob-poc".roles ADD COLUMN sort_order INTEGER DEFAULT 100;
    END IF;
END $$;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 2: CREATE REFERENCE TABLES FOR ENUMS
-- ═══════════════════════════════════════════════════════════════════════════

-- Role categories reference
CREATE TABLE IF NOT EXISTS "ob-poc".role_categories (
    category_code VARCHAR(30) PRIMARY KEY,
    category_name VARCHAR(100) NOT NULL,
    description TEXT,
    layout_behavior VARCHAR(30) NOT NULL,  -- PYRAMID_UP, TREE_DOWN, FLAT, RADIAL, SATELLITE, OVERLAY
    sort_order INTEGER DEFAULT 100
);

INSERT INTO "ob-poc".role_categories (category_code, category_name, description, layout_behavior, sort_order)
VALUES
    ('OWNERSHIP_CHAIN', 'Ownership Chain', 'Roles representing ownership/equity interests - forms UBO pyramid', 'PYRAMID_UP', 10),
    ('CONTROL_CHAIN', 'Control Chain', 'Roles representing control without ownership - overlays ownership', 'OVERLAY', 20),
    ('FUND_STRUCTURE', 'Fund Structure', 'Fund-specific structural roles (master/feeder/umbrella)', 'TREE_DOWN', 30),
    ('FUND_MANAGEMENT', 'Fund Management', 'Roles managing funds (ManCo, IM, advisor)', 'SATELLITE', 40),
    ('TRUST_ROLES', 'Trust Roles', 'Trust-specific roles (settlor, trustee, beneficiary)', 'RADIAL', 50),
    ('SERVICE_PROVIDER', 'Service Providers', 'Third-party service providers (custodian, admin, auditor)', 'FLAT_BOTTOM', 60),
    ('TRADING_EXECUTION', 'Trading & Execution', 'Operational authority roles (signatories, traders)', 'FLAT_RIGHT', 70),
    ('INVESTOR_CHAIN', 'Investor Chain', 'Investor/account holder chain (retail path)', 'PYRAMID_DOWN', 80),
    ('RELATED_PARTY', 'Related Parties', 'Connected parties requiring screening', 'PERIPHERAL', 90)
ON CONFLICT (category_code) DO UPDATE SET
    category_name = EXCLUDED.category_name,
    description = EXCLUDED.description,
    layout_behavior = EXCLUDED.layout_behavior,
    sort_order = EXCLUDED.sort_order;


-- UBO treatment reference
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_treatments (
    treatment_code VARCHAR(30) PRIMARY KEY,
    treatment_name VARCHAR(100) NOT NULL,
    description TEXT,
    terminates_chain BOOLEAN DEFAULT FALSE,
    requires_lookthrough BOOLEAN DEFAULT FALSE
);

INSERT INTO "ob-poc".ubo_treatments (treatment_code, treatment_name, description, terminates_chain, requires_lookthrough)
VALUES
    ('TERMINUS', 'Chain Terminus', 'Natural person endpoint - UBO if ≥25%', TRUE, FALSE),
    ('LOOK_THROUGH', 'Look Through', 'Must look through to find underlying owners', FALSE, TRUE),
    ('LOOK_THROUGH_CONDITIONAL', 'Conditional Look Through', 'Look through unless regulated/exempt', FALSE, TRUE),
    ('CONTROL_PRONG', 'Control Prong', 'May be UBO via control even without ownership', TRUE, FALSE),
    ('ALWAYS_UBO', 'Always UBO', 'Always treated as UBO regardless of percentage (e.g., settlor)', TRUE, FALSE),
    ('BY_PERCENTAGE', 'By Percentage', 'UBO status depends on percentage threshold', TRUE, FALSE),
    ('FLAGGED', 'Flagged for Review', 'Not automatic UBO but requires KYC attention', FALSE, FALSE),
    ('EXEMPT', 'Exempt', 'Exempt from UBO calculation (e.g., regulated fund, SWF)', TRUE, FALSE),
    ('NOT_APPLICABLE', 'Not Applicable', 'Role does not participate in UBO calculation', FALSE, FALSE)
ON CONFLICT (treatment_code) DO UPDATE SET
    treatment_name = EXCLUDED.treatment_name,
    description = EXCLUDED.description,
    terminates_chain = EXCLUDED.terminates_chain,
    requires_lookthrough = EXCLUDED.requires_lookthrough;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 3: COMPREHENSIVE ROLE SEED DATA
-- ═══════════════════════════════════════════════════════════════════════════

-- Helper function to upsert roles
CREATE OR REPLACE FUNCTION "ob-poc".upsert_role(
    p_name VARCHAR(255),
    p_description TEXT,
    p_role_category VARCHAR(30),
    p_layout_category VARCHAR(30),
    p_ubo_treatment VARCHAR(30),
    p_requires_percentage BOOLEAN,
    p_natural_person_only BOOLEAN,
    p_legal_entity_only BOOLEAN,
    p_compatible_entity_categories JSONB,
    p_display_priority INTEGER,
    p_kyc_obligation VARCHAR(30),
    p_sort_order INTEGER
) RETURNS UUID AS $$
DECLARE
    v_role_id UUID;
BEGIN
    INSERT INTO "ob-poc".roles (
        name, description, role_category, layout_category, ubo_treatment,
        requires_percentage, natural_person_only, legal_entity_only,
        compatible_entity_categories, display_priority, kyc_obligation,
        sort_order, is_active, created_at, updated_at
    ) VALUES (
        UPPER(p_name), p_description, p_role_category, p_layout_category, p_ubo_treatment,
        p_requires_percentage, p_natural_person_only, p_legal_entity_only,
        p_compatible_entity_categories, p_display_priority, p_kyc_obligation,
        p_sort_order, TRUE, NOW(), NOW()
    )
    ON CONFLICT (name) DO UPDATE SET
        description = EXCLUDED.description,
        role_category = EXCLUDED.role_category,
        layout_category = EXCLUDED.layout_category,
        ubo_treatment = EXCLUDED.ubo_treatment,
        requires_percentage = EXCLUDED.requires_percentage,
        natural_person_only = EXCLUDED.natural_person_only,
        legal_entity_only = EXCLUDED.legal_entity_only,
        compatible_entity_categories = EXCLUDED.compatible_entity_categories,
        display_priority = EXCLUDED.display_priority,
        kyc_obligation = EXCLUDED.kyc_obligation,
        sort_order = EXCLUDED.sort_order,
        updated_at = NOW()
    RETURNING role_id INTO v_role_id;
    
    RETURN v_role_id;
END;
$$ LANGUAGE plpgsql;


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 1: OWNERSHIP_CHAIN
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'ULTIMATE_BENEFICIAL_OWNER',
    'Natural person with ≥25% ownership or control - terminus of UBO chain',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'TERMINUS',
    TRUE, TRUE, FALSE,  -- requires %, natural person only
    '["PERSON"]'::JSONB,
    100, 'FULL_KYC', 10
);

SELECT "ob-poc".upsert_role(
    'BENEFICIAL_OWNER',
    'Natural person with beneficial interest <25% - flagged but not automatic UBO',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'BY_PERCENTAGE',
    TRUE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    95, 'FULL_KYC', 20
);

SELECT "ob-poc".upsert_role(
    'SHAREHOLDER',
    'Corporate or individual shareholder - requires look-through if legal entity',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "FUND", "TRUST"]'::JSONB,
    90, 'FULL_KYC', 30
);

SELECT "ob-poc".upsert_role(
    'LIMITED_PARTNER',
    'Limited partner in LP/LLP structure - passive investor, look-through required',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "FUND", "TRUST", "PARTNERSHIP"]'::JSONB,
    85, 'FULL_KYC', 40
);

SELECT "ob-poc".upsert_role(
    'GENERAL_PARTNER',
    'General partner - control + economics (carried interest), look-through to principals',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "PARTNERSHIP"]'::JSONB,
    84, 'FULL_KYC', 50
);

SELECT "ob-poc".upsert_role(
    'MEMBER',
    'LLC membership interest holder',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "TRUST"]'::JSONB,
    83, 'FULL_KYC', 60
);

SELECT "ob-poc".upsert_role(
    'PARTNER',
    'Generic partnership interest (when LP/GP distinction unclear)',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "PARTNERSHIP"]'::JSONB,
    82, 'FULL_KYC', 70
);

SELECT "ob-poc".upsert_role(
    'PRINCIPAL',
    'Key person with ownership stake - often founder or senior partner',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    80, 'FULL_KYC', 80
);

SELECT "ob-poc".upsert_role(
    'FOUNDER',
    'Original equity holder/founder',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    79, 'FULL_KYC', 90
);

SELECT "ob-poc".upsert_role(
    'HOLDING_COMPANY',
    'Intermediate holding company in ownership chain',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, TRUE,  -- legal entity only
    '["COMPANY"]'::JSONB,
    75, 'FULL_KYC', 100
);

SELECT "ob-poc".upsert_role(
    'PARENT_COMPANY',
    'Direct parent entity in corporate structure',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    74, 'FULL_KYC', 110
);

SELECT "ob-poc".upsert_role(
    'NOMINEE_SHAREHOLDER',
    'Holds shares on behalf of another - must disclose principal',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'LOOK_THROUGH',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    70, 'FULL_KYC', 120
);

SELECT "ob-poc".upsert_role(
    'CARRIED_INTEREST_HOLDER',
    'Holds carried interest in fund (economics without capital)',
    'OWNERSHIP_CHAIN', 'PYRAMID_UP', 'BY_PERCENTAGE',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "PARTNERSHIP"]'::JSONB,
    68, 'FULL_KYC', 130
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 2: CONTROL_CHAIN
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'CONTROLLING_PERSON',
    'Person with >25% voting/control rights without equivalent ownership',
    'CONTROL_CHAIN', 'OVERLAY', 'CONTROL_PRONG',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    92, 'FULL_KYC', 10
);

SELECT "ob-poc".upsert_role(
    'DIRECTOR',
    'Member of board of directors',
    'CONTROL_CHAIN', 'OVERLAY', 'CONTROL_PRONG',
    FALSE, TRUE, FALSE,  -- natural person only
    '["PERSON"]'::JSONB,
    67, 'SCREEN_AND_ID', 20
);

SELECT "ob-poc".upsert_role(
    'CHAIRMAN',
    'Chairman/Chair of the board',
    'CONTROL_CHAIN', 'OVERLAY', 'CONTROL_PRONG',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    69, 'SCREEN_AND_ID', 30
);

SELECT "ob-poc".upsert_role(
    'MANAGING_DIRECTOR',
    'Managing Director - executive board member',
    'CONTROL_CHAIN', 'OVERLAY', 'CONTROL_PRONG',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    68, 'SCREEN_AND_ID', 40
);

SELECT "ob-poc".upsert_role(
    'CHIEF_EXECUTIVE',
    'CEO or equivalent chief executive officer',
    'CONTROL_CHAIN', 'OVERLAY', 'CONTROL_PRONG',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    70, 'SCREEN_AND_ID', 50
);

SELECT "ob-poc".upsert_role(
    'OFFICER',
    'Corporate officer (C-suite, VP, etc.)',
    'CONTROL_CHAIN', 'OVERLAY', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    65, 'SCREEN_AND_ID', 60
);

SELECT "ob-poc".upsert_role(
    'CONDUCTING_OFFICER',
    'Conducting officer (LU/IE UCITS/AIF regulatory requirement)',
    'CONTROL_CHAIN', 'OVERLAY', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    66, 'SCREEN_AND_ID', 70
);

SELECT "ob-poc".upsert_role(
    'COMPANY_SECRETARY',
    'Corporate/company secretary - statutory role',
    'CONTROL_CHAIN', 'OVERLAY', 'NOT_APPLICABLE',
    FALSE, FALSE, FALSE,  -- can be person or corporate secretary firm
    '["PERSON", "COMPANY"]'::JSONB,
    60, 'SCREEN_ONLY', 80
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 3: FUND_STRUCTURE
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'MASTER_FUND',
    'Master fund in master-feeder structure - aggregates feeder investments',
    'FUND_STRUCTURE', 'TREE_DOWN', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["FUND"]'::JSONB,
    79, 'SIMPLIFIED', 10
);

SELECT "ob-poc".upsert_role(
    'FEEDER_FUND',
    'Feeder fund that invests substantially all assets in master',
    'FUND_STRUCTURE', 'TREE_DOWN', 'LOOK_THROUGH',
    TRUE, FALSE, TRUE,  -- requires % of investment in master
    '["FUND"]'::JSONB,
    78, 'SIMPLIFIED', 20
);

SELECT "ob-poc".upsert_role(
    'UMBRELLA_FUND',
    'Umbrella/SICAV structure containing multiple sub-funds',
    'FUND_STRUCTURE', 'TREE_DOWN', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["FUND"]'::JSONB,
    77, 'SIMPLIFIED', 30
);

SELECT "ob-poc".upsert_role(
    'SUB_FUND',
    'Sub-fund/compartment within umbrella structure',
    'FUND_STRUCTURE', 'TREE_DOWN', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["FUND"]'::JSONB,
    76, 'SIMPLIFIED', 40
);

SELECT "ob-poc".upsert_role(
    'PARALLEL_FUND',
    'Parallel fund with same economics but different legal wrapper',
    'FUND_STRUCTURE', 'TREE_DOWN', 'LOOK_THROUGH',
    TRUE, FALSE, TRUE,
    '["FUND", "PARTNERSHIP"]'::JSONB,
    75, 'SIMPLIFIED', 50
);

SELECT "ob-poc".upsert_role(
    'CO_INVESTMENT_VEHICLE',
    'Deal-specific co-investment vehicle',
    'FUND_STRUCTURE', 'TREE_DOWN', 'LOOK_THROUGH',
    TRUE, FALSE, TRUE,
    '["FUND", "PARTNERSHIP", "COMPANY"]'::JSONB,
    74, 'FULL_KYC', 60
);

SELECT "ob-poc".upsert_role(
    'ASSET_OWNER',
    'Legal owner of underlying assets (fund vehicle itself)',
    'FUND_STRUCTURE', 'TREE_DOWN', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["FUND", "COMPANY", "TRUST"]'::JSONB,
    80, 'SIMPLIFIED', 70
);

SELECT "ob-poc".upsert_role(
    'FUND_INVESTOR',
    'Investor in fund (LP interest, fund shareholder)',
    'FUND_STRUCTURE', 'TREE_DOWN', 'LOOK_THROUGH_CONDITIONAL',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY", "FUND", "TRUST", "PARTNERSHIP"]'::JSONB,
    50, 'FULL_KYC', 80
);

SELECT "ob-poc".upsert_role(
    'SOVEREIGN_WEALTH_FUND',
    'Sovereign wealth fund investor - typically exempt from look-through',
    'FUND_STRUCTURE', 'TREE_DOWN', 'EXEMPT',
    TRUE, FALSE, TRUE,
    '["FUND", "COMPANY"]'::JSONB,
    55, 'SIMPLIFIED', 90
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 4: FUND_MANAGEMENT
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'MANAGEMENT_COMPANY',
    'UCITS ManCo or AIF Manager - regulated fund manager',
    'FUND_MANAGEMENT', 'SATELLITE', 'LOOK_THROUGH_CONDITIONAL',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    75, 'SIMPLIFIED', 10
);

SELECT "ob-poc".upsert_role(
    'INVESTMENT_MANAGER',
    'Discretionary investment manager (may be delegated by ManCo)',
    'FUND_MANAGEMENT', 'SATELLITE', 'LOOK_THROUGH_CONDITIONAL',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP"]'::JSONB,
    74, 'SIMPLIFIED', 20
);

SELECT "ob-poc".upsert_role(
    'INVESTMENT_ADVISOR',
    'Non-discretionary investment advisor',
    'FUND_MANAGEMENT', 'SATELLITE', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP"]'::JSONB,
    73, 'SIMPLIFIED', 30
);

SELECT "ob-poc".upsert_role(
    'SUB_ADVISOR',
    'Sub-advisor with delegated investment authority',
    'FUND_MANAGEMENT', 'SATELLITE', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP"]'::JSONB,
    72, 'SIMPLIFIED', 40
);

SELECT "ob-poc".upsert_role(
    'SPONSOR',
    'Fund sponsor (PE/VC firm, asset manager)',
    'FUND_MANAGEMENT', 'SATELLITE', 'LOOK_THROUGH',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP"]'::JSONB,
    76, 'FULL_KYC', 50
);

SELECT "ob-poc".upsert_role(
    'PROMOTER',
    'Fund promoter - initiates fund formation',
    'FUND_MANAGEMENT', 'SATELLITE', 'LOOK_THROUGH',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    71, 'FULL_KYC', 60
);

SELECT "ob-poc".upsert_role(
    'PORTFOLIO_MANAGER',
    'Individual portfolio manager responsible for investment decisions',
    'FUND_MANAGEMENT', 'SATELLITE', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    70, 'SCREEN_AND_ID', 70
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 5: TRUST_ROLES
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'SETTLOR',
    'Creator of trust who contributed assets - always UBO under 5MLD',
    'TRUST_ROLES', 'RADIAL', 'ALWAYS_UBO',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    91, 'FULL_KYC', 10
);

SELECT "ob-poc".upsert_role(
    'TRUSTEE',
    'Legal owner of trust assets with fiduciary duty',
    'TRUST_ROLES', 'RADIAL', 'CONTROL_PRONG',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    88, 'FULL_KYC', 20
);

SELECT "ob-poc".upsert_role(
    'PROTECTOR',
    'Trust protector with oversight/veto powers',
    'TRUST_ROLES', 'RADIAL', 'CONTROL_PRONG',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    86, 'FULL_KYC', 30
);

SELECT "ob-poc".upsert_role(
    'BENEFICIARY_FIXED',
    'Beneficiary with fixed percentage entitlement - treated as ownership',
    'TRUST_ROLES', 'RADIAL', 'BY_PERCENTAGE',
    TRUE, FALSE, FALSE,  -- requires percentage
    '["PERSON", "COMPANY", "TRUST"]'::JSONB,
    85, 'FULL_KYC', 40
);

SELECT "ob-poc".upsert_role(
    'BENEFICIARY_DISCRETIONARY',
    'Beneficiary with discretionary entitlement - flagged as potential UBO',
    'TRUST_ROLES', 'RADIAL', 'FLAGGED',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY", "TRUST"]'::JSONB,
    84, 'FULL_KYC', 50
);

SELECT "ob-poc".upsert_role(
    'BENEFICIARY_CONTINGENT',
    'Beneficiary with contingent/future interest',
    'TRUST_ROLES', 'RADIAL', 'FLAGGED',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY", "TRUST"]'::JSONB,
    83, 'FULL_KYC', 60
);

SELECT "ob-poc".upsert_role(
    'ENFORCER',
    'Purpose trust enforcer (has standing to enforce trust terms)',
    'TRUST_ROLES', 'RADIAL', 'CONTROL_PRONG',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    82, 'FULL_KYC', 70
);

SELECT "ob-poc".upsert_role(
    'APPOINTOR',
    'Person with power to appoint/remove trustees',
    'TRUST_ROLES', 'RADIAL', 'CONTROL_PRONG',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    87, 'FULL_KYC', 80
);

SELECT "ob-poc".upsert_role(
    'TRUST_BENEFICIARY',
    'Generic trust beneficiary (when type unspecified)',
    'TRUST_ROLES', 'RADIAL', 'FLAGGED',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY", "TRUST"]'::JSONB,
    80, 'FULL_KYC', 90
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 6: SERVICE_PROVIDER
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'DEPOSITARY',
    'UCITS/AIF depositary - safekeeping and oversight',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    50, 'SIMPLIFIED', 10
);

SELECT "ob-poc".upsert_role(
    'CUSTODIAN',
    'Securities custodian - safekeeping of assets',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    49, 'SIMPLIFIED', 20
);

SELECT "ob-poc".upsert_role(
    'PRIME_BROKER',
    'Prime broker - custody, margin, securities lending, execution',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    48, 'SIMPLIFIED', 30
);

SELECT "ob-poc".upsert_role(
    'SUB_CUSTODIAN',
    'Sub-custodian for local market custody',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    47, 'SIMPLIFIED', 40
);

SELECT "ob-poc".upsert_role(
    'ADMINISTRATOR',
    'Fund administrator - NAV calculation, accounting',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    45, 'SIMPLIFIED', 50
);

SELECT "ob-poc".upsert_role(
    'TRANSFER_AGENT',
    'Transfer agent - shareholder registry maintenance',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    43, 'SIMPLIFIED', 60
);

SELECT "ob-poc".upsert_role(
    'PAYING_AGENT',
    'Paying agent - dividend/distribution payments',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    42, 'SIMPLIFIED', 70
);

SELECT "ob-poc".upsert_role(
    'AUDITOR',
    'External auditor',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP"]'::JSONB,
    40, 'RECORD_ONLY', 80
);

SELECT "ob-poc".upsert_role(
    'LEGAL_COUNSEL',
    'Legal advisor/counsel',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP"]'::JSONB,
    35, 'RECORD_ONLY', 90
);

SELECT "ob-poc".upsert_role(
    'TAX_ADVISOR',
    'Tax advisor',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP", "PERSON"]'::JSONB,
    34, 'RECORD_ONLY', 100
);

SELECT "ob-poc".upsert_role(
    'COMPLIANCE_CONSULTANT',
    'Compliance advisory services',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, FALSE,
    '["COMPANY", "PERSON"]'::JSONB,
    33, 'RECORD_ONLY', 110
);

SELECT "ob-poc".upsert_role(
    'VALUATION_AGENT',
    'Independent valuation agent',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    32, 'RECORD_ONLY', 120
);

SELECT "ob-poc".upsert_role(
    'SERVICE_PROVIDER',
    'Generic service provider (when specific type unclear)',
    'SERVICE_PROVIDER', 'FLAT_BOTTOM', 'NOT_APPLICABLE',
    FALSE, FALSE, FALSE,
    '["COMPANY", "PARTNERSHIP", "PERSON"]'::JSONB,
    20, 'SCREEN_ONLY', 130
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 7: TRADING_EXECUTION
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'AUTHORIZED_SIGNATORY',
    'Person authorized to sign documents/transactions on behalf of entity',
    'TRADING_EXECUTION', 'FLAT_RIGHT', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    55, 'SCREEN_AND_ID', 10
);

SELECT "ob-poc".upsert_role(
    'AUTHORIZED_TRADER',
    'Person authorized to execute trades',
    'TRADING_EXECUTION', 'FLAT_RIGHT', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    54, 'SCREEN_AND_ID', 20
);

SELECT "ob-poc".upsert_role(
    'AUTHORIZED_REPRESENTATIVE',
    'General authorized representative',
    'TRADING_EXECUTION', 'FLAT_RIGHT', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    53, 'SCREEN_AND_ID', 30
);

SELECT "ob-poc".upsert_role(
    'POWER_OF_ATTORNEY',
    'Person holding power of attorney',
    'TRADING_EXECUTION', 'FLAT_RIGHT', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    52, 'SCREEN_AND_ID', 40
);

SELECT "ob-poc".upsert_role(
    'ACCOUNT_OPERATOR',
    'Day-to-day account operator',
    'TRADING_EXECUTION', 'FLAT_RIGHT', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    51, 'SCREEN_AND_ID', 50
);

SELECT "ob-poc".upsert_role(
    'SETTLEMENT_CONTACT',
    'Contact for settlement instructions',
    'TRADING_EXECUTION', 'FLAT_RIGHT', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    30, 'RECORD_ONLY', 60
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 8: INVESTOR_CHAIN
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'ACCOUNT_HOLDER',
    'Brokerage/custody account holder - terminus for retail',
    'INVESTOR_CHAIN', 'PYRAMID_DOWN', 'TERMINUS',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY", "TRUST"]'::JSONB,
    60, 'FULL_KYC', 10
);

SELECT "ob-poc".upsert_role(
    'NOMINEE',
    'Holds assets in nominee name on behalf of beneficial owner',
    'INVESTOR_CHAIN', 'PYRAMID_DOWN', 'LOOK_THROUGH',
    FALSE, FALSE, FALSE,
    '["COMPANY", "PERSON"]'::JSONB,
    40, 'FULL_KYC', 20
);

SELECT "ob-poc".upsert_role(
    'CUSTODIAN_FOR',
    'Custodian acting on behalf of underlying client',
    'INVESTOR_CHAIN', 'PYRAMID_DOWN', 'LOOK_THROUGH',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    39, 'SIMPLIFIED', 30
);

SELECT "ob-poc".upsert_role(
    'PLATFORM_INVESTOR',
    'Investor via fund platform/wrap',
    'INVESTOR_CHAIN', 'PYRAMID_DOWN', 'LOOK_THROUGH_CONDITIONAL',
    TRUE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    38, 'FULL_KYC', 40
);

SELECT "ob-poc".upsert_role(
    'OMNIBUS_ACCOUNT',
    'Pooled/omnibus account holder',
    'INVESTOR_CHAIN', 'PYRAMID_DOWN', 'LOOK_THROUGH_CONDITIONAL',
    FALSE, FALSE, TRUE,
    '["COMPANY"]'::JSONB,
    37, 'SIMPLIFIED', 50
);

SELECT "ob-poc".upsert_role(
    'PENSION_PARTICIPANT',
    'Pension/401k participant',
    'INVESTOR_CHAIN', 'PYRAMID_DOWN', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    36, 'RECORD_ONLY', 60
);


-- ═══════════════════════════════════════════════════════════════════════════
-- CATEGORY 9: RELATED_PARTY
-- ═══════════════════════════════════════════════════════════════════════════

SELECT "ob-poc".upsert_role(
    'AFFILIATE',
    'Related/affiliated entity',
    'RELATED_PARTY', 'PERIPHERAL', 'LOOK_THROUGH_CONDITIONAL',
    FALSE, FALSE, TRUE,
    '["COMPANY", "PARTNERSHIP", "FUND"]'::JSONB,
    25, 'SCREEN_ONLY', 10
);

SELECT "ob-poc".upsert_role(
    'ASSOCIATED_PERSON',
    'Associated individual (employee, associate)',
    'RELATED_PARTY', 'PERIPHERAL', 'NOT_APPLICABLE',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    24, 'SCREEN_ONLY', 20
);

SELECT "ob-poc".upsert_role(
    'FAMILY_MEMBER',
    'Family member of UBO/control person',
    'RELATED_PARTY', 'PERIPHERAL', 'FLAGGED',
    FALSE, TRUE, FALSE,
    '["PERSON"]'::JSONB,
    23, 'SCREEN_ONLY', 30
);

SELECT "ob-poc".upsert_role(
    'CONNECTED_PARTY',
    'Other connected party',
    'RELATED_PARTY', 'PERIPHERAL', 'FLAGGED',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    22, 'SCREEN_ONLY', 40
);

SELECT "ob-poc".upsert_role(
    'INTRODUCER',
    'Party that introduced/referred the client',
    'RELATED_PARTY', 'PERIPHERAL', 'NOT_APPLICABLE',
    FALSE, FALSE, FALSE,
    '["PERSON", "COMPANY"]'::JSONB,
    21, 'RECORD_ONLY', 50
);


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 4: ROLE COMPATIBILITY RULES
-- ═══════════════════════════════════════════════════════════════════════════

-- Table to define invalid role combinations on same entity within same CBU
CREATE TABLE IF NOT EXISTS "ob-poc".role_incompatibilities (
    incompatibility_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role_a VARCHAR(255) NOT NULL,
    role_b VARCHAR(255) NOT NULL,
    reason TEXT NOT NULL,
    exception_allowed BOOLEAN DEFAULT FALSE,
    exception_condition TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Both roles must exist
    CONSTRAINT fk_role_a FOREIGN KEY (role_a) REFERENCES "ob-poc".roles(name),
    CONSTRAINT fk_role_b FOREIGN KEY (role_b) REFERENCES "ob-poc".roles(name),
    
    -- Prevent duplicates (A,B) and (B,A) are same
    CONSTRAINT uq_role_pair UNIQUE (role_a, role_b),
    CONSTRAINT chk_role_order CHECK (role_a < role_b)  -- enforce ordering
);

-- Insert incompatibility rules
INSERT INTO "ob-poc".role_incompatibilities (role_a, role_b, reason, exception_allowed, exception_condition)
VALUES
    -- Ownership conflicts
    ('GENERAL_PARTNER', 'LIMITED_PARTNER', 
     'Same entity cannot be both GP and LP of same fund', FALSE, NULL),
    
    ('MASTER_FUND', 'FEEDER_FUND',
     'Same entity cannot be both master and feeder', FALSE, NULL),
    
    ('BENEFICIARY_DISCRETIONARY', 'BENEFICIARY_FIXED',
     'Beneficiary interest type must be consistent', FALSE, NULL),
    
    -- Control conflicts
    ('SETTLOR', 'TRUSTEE',
     'Settlor typically cannot be sole trustee (defeats trust purpose)', TRUE, 
     'Allowed if independent co-trustee exists'),
    
    -- Service provider independence
    ('AUDITOR', 'ADMINISTRATOR',
     'Auditor must be independent of administrator', FALSE, NULL),
    
    ('AUDITOR', 'MANAGEMENT_COMPANY',
     'Auditor must be independent of ManCo', FALSE, NULL),
    
    -- Structural conflicts
    ('HOLDING_COMPANY', 'ULTIMATE_BENEFICIAL_OWNER',
     'Holding company is a legal entity, UBO must be natural person', FALSE, NULL),
    
    ('MANAGEMENT_COMPANY', 'DEPOSITARY',
     'ManCo and depositary must be separate entities', FALSE, NULL),

    -- Trust role conflicts  
    ('ENFORCER', 'TRUST_BENEFICIARY',
     'Enforcer of purpose trust should not be beneficiary (no beneficiaries exist)', FALSE, NULL),

    -- Entity type conflicts (handled by natural_person_only/legal_entity_only but also here)
    ('DIRECTOR', 'HOLDING_COMPANY',
     'Director must be natural person, holding company is legal entity', FALSE, NULL)
    
ON CONFLICT (role_a, role_b) DO UPDATE SET
    reason = EXCLUDED.reason,
    exception_allowed = EXCLUDED.exception_allowed,
    exception_condition = EXCLUDED.exception_condition;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 5: ROLE REQUIREMENT RULES
-- ═══════════════════════════════════════════════════════════════════════════

-- Define roles that require other roles to be present
CREATE TABLE IF NOT EXISTS "ob-poc".role_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requiring_role VARCHAR(255) NOT NULL,
    required_role VARCHAR(255) NOT NULL,
    requirement_type VARCHAR(30) NOT NULL,  -- 'MANDATORY', 'CONDITIONAL', 'RECOMMENDED'
    scope VARCHAR(30) NOT NULL,             -- 'SAME_ENTITY', 'SAME_CBU', 'ANY'
    condition_description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT fk_requiring_role FOREIGN KEY (requiring_role) REFERENCES "ob-poc".roles(name),
    CONSTRAINT fk_required_role FOREIGN KEY (required_role) REFERENCES "ob-poc".roles(name)
);

INSERT INTO "ob-poc".role_requirements (requiring_role, required_role, requirement_type, scope, condition_description)
VALUES
    -- Fund structure requirements
    ('FEEDER_FUND', 'MASTER_FUND', 'MANDATORY', 'SAME_CBU',
     'Feeder fund must have corresponding master fund in CBU'),
    
    ('SUB_FUND', 'UMBRELLA_FUND', 'MANDATORY', 'SAME_CBU',
     'Sub-fund must be under an umbrella fund'),
    
    -- Management requirements
    ('SUB_ADVISOR', 'INVESTMENT_MANAGER', 'MANDATORY', 'SAME_CBU',
     'Sub-advisor must have primary investment manager'),
    
    -- Trust requirements
    ('BENEFICIARY_FIXED', 'TRUSTEE', 'MANDATORY', 'SAME_CBU',
     'Trust with beneficiaries must have trustee'),
    
    ('BENEFICIARY_DISCRETIONARY', 'TRUSTEE', 'MANDATORY', 'SAME_CBU',
     'Trust with beneficiaries must have trustee'),
    
    ('PROTECTOR', 'TRUSTEE', 'MANDATORY', 'SAME_CBU',
     'Protector role only valid with trustee'),
    
    ('ENFORCER', 'TRUSTEE', 'MANDATORY', 'SAME_CBU',
     'Enforcer (purpose trust) requires trustee'),
     
    -- Regulatory requirements
    ('CONDUCTING_OFFICER', 'MANAGEMENT_COMPANY', 'MANDATORY', 'SAME_CBU',
     'Conducting officer is required by ManCo'),
    
    -- UBO requirements
    ('NOMINEE_SHAREHOLDER', 'SHAREHOLDER', 'CONDITIONAL', 'SAME_CBU',
     'Nominee must disclose the actual shareholder/principal')
    
ON CONFLICT DO NOTHING;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 6: UPDATE VIEWS FOR VISUALIZATION
-- ═══════════════════════════════════════════════════════════════════════════

-- Drop and recreate v_cbu_entity_with_roles to use new taxonomy
CREATE OR REPLACE VIEW "ob-poc".v_cbu_entity_with_roles AS
WITH role_data AS (
    SELECT
        cer.cbu_id,
        cer.entity_id,
        e.name AS entity_name,
        et.type_code AS entity_type,
        et.entity_category,
        COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) AS jurisdiction,
        r.name AS role_name,
        r.role_category,
        r.layout_category,
        r.display_priority,
        r.ubo_treatment,
        r.requires_percentage,
        r.kyc_obligation
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
    LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
    LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
    LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
)
SELECT
    cbu_id,
    entity_id,
    entity_name,
    entity_type,
    entity_category,
    jurisdiction,
    -- Aggregate roles
    array_agg(role_name ORDER BY display_priority DESC) AS roles,
    array_agg(DISTINCT role_category) FILTER (WHERE role_category IS NOT NULL) AS role_categories,
    array_agg(DISTINCT layout_category) FILTER (WHERE layout_category IS NOT NULL) AS layout_categories,
    -- Primary role (highest priority)
    (array_agg(role_name ORDER BY display_priority DESC))[1] AS primary_role,
    -- Primary layout (for positioning)
    (array_agg(layout_category ORDER BY display_priority DESC))[1] AS primary_layout,
    -- Max priority for sorting
    max(display_priority) AS max_role_priority,
    -- UBO treatment (most restrictive)
    CASE 
        WHEN 'ALWAYS_UBO' = ANY(array_agg(ubo_treatment)) THEN 'ALWAYS_UBO'
        WHEN 'TERMINUS' = ANY(array_agg(ubo_treatment)) THEN 'TERMINUS'
        WHEN 'CONTROL_PRONG' = ANY(array_agg(ubo_treatment)) THEN 'CONTROL_PRONG'
        WHEN 'BY_PERCENTAGE' = ANY(array_agg(ubo_treatment)) THEN 'BY_PERCENTAGE'
        WHEN 'LOOK_THROUGH' = ANY(array_agg(ubo_treatment)) THEN 'LOOK_THROUGH'
        ELSE 'NOT_APPLICABLE'
    END AS effective_ubo_treatment,
    -- KYC obligation (most stringent)
    CASE
        WHEN 'FULL_KYC' = ANY(array_agg(kyc_obligation)) THEN 'FULL_KYC'
        WHEN 'SCREEN_AND_ID' = ANY(array_agg(kyc_obligation)) THEN 'SCREEN_AND_ID'
        WHEN 'SIMPLIFIED' = ANY(array_agg(kyc_obligation)) THEN 'SIMPLIFIED'
        WHEN 'SCREEN_ONLY' = ANY(array_agg(kyc_obligation)) THEN 'SCREEN_ONLY'
        ELSE 'RECORD_ONLY'
    END AS effective_kyc_obligation
FROM role_data
GROUP BY cbu_id, entity_id, entity_name, entity_type, entity_category, jurisdiction;


-- View for role taxonomy reference
CREATE OR REPLACE VIEW "ob-poc".v_role_taxonomy AS
SELECT
    r.role_id,
    r.name AS role_code,
    r.description,
    r.role_category,
    rc.category_name,
    rc.layout_behavior,
    r.layout_category,
    r.ubo_treatment,
    ut.treatment_name,
    ut.terminates_chain,
    ut.requires_lookthrough,
    r.display_priority,
    r.requires_percentage,
    r.natural_person_only,
    r.legal_entity_only,
    r.compatible_entity_categories,
    r.kyc_obligation,
    r.sort_order,
    r.is_active
FROM "ob-poc".roles r
LEFT JOIN "ob-poc".role_categories rc ON r.role_category = rc.category_code
LEFT JOIN "ob-poc".ubo_treatments ut ON r.ubo_treatment = ut.treatment_code
WHERE r.is_active = TRUE
ORDER BY rc.sort_order, r.sort_order, r.display_priority DESC;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 7: VALIDATION FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════

-- Function to validate role assignment
CREATE OR REPLACE FUNCTION "ob-poc".validate_role_assignment(
    p_entity_id UUID,
    p_role_name VARCHAR(255),
    p_cbu_id UUID
) RETURNS TABLE (
    is_valid BOOLEAN,
    error_code VARCHAR(50),
    error_message TEXT
) AS $$
DECLARE
    v_role RECORD;
    v_entity RECORD;
    v_existing_roles TEXT[];
    v_incompatible RECORD;
BEGIN
    -- Get role details
    SELECT * INTO v_role FROM "ob-poc".roles WHERE name = UPPER(p_role_name);
    IF NOT FOUND THEN
        RETURN QUERY SELECT FALSE, 'ROLE_NOT_FOUND'::VARCHAR(50), 
            format('Role %s does not exist', p_role_name);
        RETURN;
    END IF;
    
    -- Get entity details
    SELECT e.entity_id, e.name, et.entity_category, et.type_code
    INTO v_entity
    FROM "ob-poc".entities e
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE e.entity_id = p_entity_id;
    
    IF NOT FOUND THEN
        RETURN QUERY SELECT FALSE, 'ENTITY_NOT_FOUND'::VARCHAR(50),
            format('Entity %s does not exist', p_entity_id);
        RETURN;
    END IF;
    
    -- Check natural person constraint
    IF v_role.natural_person_only AND v_entity.entity_category != 'PERSON' THEN
        RETURN QUERY SELECT FALSE, 'NATURAL_PERSON_REQUIRED'::VARCHAR(50),
            format('Role %s can only be assigned to natural persons, but %s is %s',
                   p_role_name, v_entity.name, v_entity.entity_category);
        RETURN;
    END IF;
    
    -- Check legal entity constraint
    IF v_role.legal_entity_only AND v_entity.entity_category = 'PERSON' THEN
        RETURN QUERY SELECT FALSE, 'LEGAL_ENTITY_REQUIRED'::VARCHAR(50),
            format('Role %s can only be assigned to legal entities, but %s is a person',
                   p_role_name, v_entity.name);
        RETURN;
    END IF;
    
    -- Check entity category compatibility
    IF v_role.compatible_entity_categories IS NOT NULL THEN
        IF NOT (v_role.compatible_entity_categories ? v_entity.entity_category) THEN
            RETURN QUERY SELECT FALSE, 'INCOMPATIBLE_ENTITY_TYPE'::VARCHAR(50),
                format('Role %s is not compatible with entity category %s. Compatible: %s',
                       p_role_name, v_entity.entity_category, v_role.compatible_entity_categories);
            RETURN;
        END IF;
    END IF;
    
    -- Get existing roles for this entity in this CBU
    SELECT array_agg(r.name) INTO v_existing_roles
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    WHERE cer.entity_id = p_entity_id AND cer.cbu_id = p_cbu_id;
    
    -- Check for incompatible role combinations
    FOR v_incompatible IN
        SELECT ri.role_a, ri.role_b, ri.reason, ri.exception_allowed
        FROM "ob-poc".role_incompatibilities ri
        WHERE (ri.role_a = UPPER(p_role_name) OR ri.role_b = UPPER(p_role_name))
    LOOP
        IF v_incompatible.role_a = UPPER(p_role_name) THEN
            IF v_incompatible.role_b = ANY(v_existing_roles) THEN
                IF NOT v_incompatible.exception_allowed THEN
                    RETURN QUERY SELECT FALSE, 'INCOMPATIBLE_ROLES'::VARCHAR(50),
                        format('Role %s is incompatible with existing role %s: %s',
                               p_role_name, v_incompatible.role_b, v_incompatible.reason);
                    RETURN;
                END IF;
            END IF;
        ELSE
            IF v_incompatible.role_a = ANY(v_existing_roles) THEN
                IF NOT v_incompatible.exception_allowed THEN
                    RETURN QUERY SELECT FALSE, 'INCOMPATIBLE_ROLES'::VARCHAR(50),
                        format('Role %s is incompatible with existing role %s: %s',
                               p_role_name, v_incompatible.role_a, v_incompatible.reason);
                    RETURN;
                END IF;
            END IF;
        END IF;
    END LOOP;
    
    -- All checks passed
    RETURN QUERY SELECT TRUE, NULL::VARCHAR(50), NULL::TEXT;
END;
$$ LANGUAGE plpgsql;


-- Function to check CBU role requirements
CREATE OR REPLACE FUNCTION "ob-poc".check_cbu_role_requirements(p_cbu_id UUID)
RETURNS TABLE (
    requirement_type VARCHAR(30),
    requiring_role VARCHAR(255),
    required_role VARCHAR(255),
    is_satisfied BOOLEAN,
    message TEXT
) AS $$
BEGIN
    RETURN QUERY
    WITH cbu_roles AS (
        SELECT DISTINCT r.name AS role_name
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON cer.role_id = r.role_id
        WHERE cer.cbu_id = p_cbu_id
    )
    SELECT 
        rr.requirement_type,
        rr.requiring_role,
        rr.required_role,
        EXISTS (SELECT 1 FROM cbu_roles WHERE role_name = rr.required_role) AS is_satisfied,
        CASE 
            WHEN EXISTS (SELECT 1 FROM cbu_roles WHERE role_name = rr.required_role)
            THEN format('Requirement satisfied: %s present', rr.required_role)
            ELSE format('Missing required role %s for %s: %s', 
                        rr.required_role, rr.requiring_role, rr.condition_description)
        END AS message
    FROM "ob-poc".role_requirements rr
    WHERE rr.scope = 'SAME_CBU'
      AND EXISTS (SELECT 1 FROM cbu_roles WHERE role_name = rr.requiring_role);
END;
$$ LANGUAGE plpgsql;


-- ═══════════════════════════════════════════════════════════════════════════
-- CLEANUP
-- ═══════════════════════════════════════════════════════════════════════════

-- Drop helper function (no longer needed after seeding)
-- DROP FUNCTION IF EXISTS "ob-poc".upsert_role;

COMMENT ON TABLE "ob-poc".roles IS 
'Master role taxonomy with visualization metadata, UBO treatment rules, and entity compatibility constraints. Version 2.0.';

COMMENT ON TABLE "ob-poc".role_categories IS 
'Reference table for role categories with layout behavior hints for visualization.';

COMMENT ON TABLE "ob-poc".ubo_treatments IS
'Reference table for UBO calculation behaviors (terminus, look-through, etc.).';

COMMENT ON TABLE "ob-poc".role_incompatibilities IS
'Defines invalid role combinations that cannot coexist on same entity within same CBU.';

COMMENT ON TABLE "ob-poc".role_requirements IS
'Defines role dependencies - when one role requires another to be present.';

COMMENT ON VIEW "ob-poc".v_cbu_entity_with_roles IS
'Aggregated view of entities with their roles, categories, and effective KYC/UBO treatment. Used for visualization.';

COMMENT ON VIEW "ob-poc".v_role_taxonomy IS
'Complete role taxonomy reference with category and treatment details.';

COMMENT ON FUNCTION "ob-poc".validate_role_assignment IS
'Validates that a role can be assigned to an entity, checking entity type compatibility and role conflicts.';

COMMENT ON FUNCTION "ob-poc".check_cbu_role_requirements IS
'Checks if all role requirements are satisfied for a CBU (e.g., feeder needs master).';
