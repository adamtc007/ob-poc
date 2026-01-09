-- ============================================================================
-- Migration: 010_bods_gleif_integration.sql
-- Purpose: BODS 0.4 + GLEIF Deep Integration
-- Date: 2025-01-08
--
-- Reference: ai-thoughts/012-bods-gleif-integration-TODO.md
--
-- This migration:
-- 1. EXTENDS entity_identifiers with BODS/LEI validation columns
-- 2. Creates gleif_relationships table (corporate hierarchy, SEPARATE from UBO)
-- 3. Creates bods_interest_types codelist (22 standard types)
-- 4. Extends entity_relationships with BODS fields
-- 5. Creates person_pep_status table (detailed PEP tracking)
-- 6. Creates bods_entity_types codelist
-- 7. Adds BODS columns to entities table
-- ============================================================================

BEGIN;

-- Record migration (check if already recorded)
INSERT INTO "ob-poc".schema_changes (change_type, description, script_name)
SELECT 'ENHANCEMENT', 'BODS 0.4 + GLEIF Deep Integration', '010_bods_gleif_integration.sql'
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".schema_changes WHERE script_name = '010_bods_gleif_integration.sql'
);

-- ============================================================================
-- 1. EXTEND ENTITY_IDENTIFIERS (Already exists - add BODS columns)
-- ============================================================================
-- Existing columns: identifier_id, entity_id, identifier_type, identifier_value,
--                   issuing_authority, is_primary, valid_from, valid_until, source, created_at

-- Add BODS/LEI-specific columns
ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS scheme_name VARCHAR(100);  -- Human readable: 'Global LEI System'

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS uri VARCHAR(500);  -- Full URI (e.g., GLEIF API URL)

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS is_validated BOOLEAN DEFAULT false;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS validated_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS validation_source VARCHAR(100);  -- 'GLEIF_API', 'MANUAL'

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS validation_details JSONB DEFAULT '{}';

-- LEI-specific fields
ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_status VARCHAR(30);  -- 'ISSUED', 'LAPSED', 'RETIRED'

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_next_renewal DATE;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_managing_lou VARCHAR(100);

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_initial_registration DATE;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS lei_last_update TIMESTAMPTZ;

ALTER TABLE "ob-poc".entity_identifiers
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- Create index on identifier_type for LEI lookups
CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lei
    ON "ob-poc".entity_identifiers(identifier_value) WHERE identifier_type = 'LEI';

CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lei_status
    ON "ob-poc".entity_identifiers(lei_status) WHERE identifier_type = 'LEI';

COMMENT ON TABLE "ob-poc".entity_identifiers IS 'Unified identifier storage for entities. LEI is the global spine, supports any identifier scheme.';

-- ============================================================================
-- 2. GLEIF RELATIONSHIPS (Corporate Hierarchy - SEPARATE FROM UBO)
-- ============================================================================
-- CRITICAL: This stores GLEIF corporate structure (accounting consolidation)
-- NOT the same as beneficial ownership (KYC). Keep semantics separate!

CREATE TABLE IF NOT EXISTS "ob-poc".gleif_relationships (
    gleif_rel_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Parent entity (owner in GLEIF consolidation terms)
    parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    parent_lei VARCHAR(20) NOT NULL,

    -- Child entity (owned in GLEIF consolidation terms)
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    child_lei VARCHAR(20) NOT NULL,

    -- GLEIF relationship semantics
    relationship_type VARCHAR(50) NOT NULL,  -- IS_DIRECTLY_CONSOLIDATED_BY, IS_ULTIMATELY_CONSOLIDATED_BY, IS_FUND_MANAGED_BY
    relationship_status VARCHAR(30),         -- ACTIVE, INACTIVE
    relationship_qualifier VARCHAR(50),

    -- Ownership from GLEIF (for consolidation, NOT UBO)
    ownership_percentage NUMERIC(5,2),
    ownership_percentage_min NUMERIC(5,2),
    ownership_percentage_max NUMERIC(5,2),

    -- Accounting consolidation info
    accounting_standard VARCHAR(50),  -- IFRS, US_GAAP, OTHER

    -- Temporal
    start_date DATE,
    end_date DATE,

    -- Source tracking
    gleif_record_id VARCHAR(100),
    gleif_registration_status VARCHAR(30),
    fetched_at TIMESTAMPTZ DEFAULT NOW(),
    raw_data JSONB,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Natural key
    UNIQUE(parent_lei, child_lei, relationship_type)
);

CREATE INDEX IF NOT EXISTS idx_gleif_rel_parent ON "ob-poc".gleif_relationships(parent_entity_id);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_child ON "ob-poc".gleif_relationships(child_entity_id);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_parent_lei ON "ob-poc".gleif_relationships(parent_lei);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_child_lei ON "ob-poc".gleif_relationships(child_lei);
CREATE INDEX IF NOT EXISTS idx_gleif_rel_type ON "ob-poc".gleif_relationships(relationship_type);

COMMENT ON TABLE "ob-poc".gleif_relationships IS 'GLEIF corporate hierarchy (consolidation). SEPARATE from entity_relationships (UBO/KYC). GLEIF = accounting, UBO = beneficial ownership.';

-- ============================================================================
-- 3. BODS INTEREST TYPES (Codelist - 22 Standard Types)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_interest_types (
    type_code VARCHAR(50) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    category VARCHAR(30) NOT NULL,  -- ownership, control, trust, contractual, nominee, unknown
    description TEXT,
    bods_standard BOOLEAN DEFAULT true,
    requires_percentage BOOLEAN DEFAULT false,
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Populate BODS 0.4 standard interest types
INSERT INTO "ob-poc".bods_interest_types (type_code, display_name, category, requires_percentage, display_order) VALUES
-- Ownership interests
('shareholding', 'Shareholding', 'ownership', true, 1),
('votingRights', 'Voting Rights', 'ownership', true, 2),
('rightsToSurplusAssetsOnDissolution', 'Rights to Surplus Assets on Dissolution', 'ownership', true, 3),
('rightsToProfitOrIncome', 'Rights to Profit or Income', 'ownership', true, 4),
-- Control interests
('appointmentOfBoard', 'Appointment of Board', 'control', false, 10),
('otherInfluenceOrControl', 'Other Influence or Control', 'control', false, 11),
('seniorManagingOfficial', 'Senior Managing Official', 'control', false, 12),
('controlViaCompanyRulesOrArticles', 'Control via Company Rules/Articles', 'control', false, 13),
('controlByLegalFramework', 'Control by Legal Framework', 'control', false, 14),
('boardMember', 'Board Member', 'control', false, 15),
('boardChair', 'Board Chair', 'control', false, 16),
-- Trust interests
('settlor', 'Settlor', 'trust', false, 20),
('trustee', 'Trustee', 'trust', false, 21),
('protector', 'Protector', 'trust', false, 22),
('beneficiaryOfLegalArrangement', 'Beneficiary of Legal Arrangement', 'trust', true, 23),
('enjoymentAndUseOfAssets', 'Enjoyment and Use of Assets', 'trust', false, 24),
('rightToProfitOrIncomeFromAssets', 'Right to Profit/Income from Assets', 'trust', true, 25),
-- Contractual interests
('rightsGrantedByContract', 'Rights Granted by Contract', 'contractual', false, 30),
('conditionalRightsGrantedByContract', 'Conditional Rights Granted by Contract', 'contractual', false, 31),
-- Nominee interests
('nominee', 'Nominee', 'nominee', false, 40),
('nominator', 'Nominator', 'nominee', false, 41),
-- Unknown
('unknownInterest', 'Unknown Interest', 'unknown', false, 50),
('unpublishedInterest', 'Unpublished Interest', 'unknown', false, 51)
ON CONFLICT (type_code) DO NOTHING;

COMMENT ON TABLE "ob-poc".bods_interest_types IS 'BODS 0.4 standard interest types (22 types). Codelist for entity_relationships.interest_type.';

-- ============================================================================
-- 4. BODS ENTITY TYPES (Codelist)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_entity_types (
    type_code VARCHAR(30) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,
    subtypes JSONB DEFAULT '[]',
    display_order INTEGER DEFAULT 0
);

INSERT INTO "ob-poc".bods_entity_types (type_code, display_name, subtypes, display_order) VALUES
('registeredEntity', 'Registered Entity', '["other"]', 1),
('legalEntity', 'Legal Entity', '["trust", "other"]', 2),
('arrangement', 'Arrangement', '["trust", "nomination", "other"]', 3),
('anonymousEntity', 'Anonymous Entity', '["other"]', 4),
('unknownEntity', 'Unknown Entity', '["other"]', 5),
('state', 'State', '["other"]', 6),
('stateBody', 'State Body', '["governmentDepartment", "stateAgency", "other"]', 7)
ON CONFLICT (type_code) DO NOTHING;

COMMENT ON TABLE "ob-poc".bods_entity_types IS 'BODS 0.4 entity type codelist.';

-- ============================================================================
-- 5. EXTEND ENTITY_RELATIONSHIPS FOR BODS ALIGNMENT
-- ============================================================================

-- Add interest_type column if not exists, or widen if it exists
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS interest_type VARCHAR(50);

-- Drop ALL dependent views before altering column type
DROP VIEW IF EXISTS "ob-poc".entity_relationships_current;
DROP VIEW IF EXISTS "ob-poc".cbu_ownership_graph;

-- Widen existing interest_type column to accommodate BODS codes (beneficiaryOfLegalArrangement = 28 chars)
ALTER TABLE "ob-poc".entity_relationships
ALTER COLUMN interest_type TYPE VARCHAR(50);

-- Also widen the history table column
ALTER TABLE "ob-poc".entity_relationships_history
ALTER COLUMN interest_type TYPE VARCHAR(50);

-- Recreate entity_relationships_current view
CREATE OR REPLACE VIEW "ob-poc".entity_relationships_current AS
SELECT relationship_id,
    from_entity_id,
    to_entity_id,
    relationship_type,
    percentage,
    ownership_type,
    control_type,
    trust_role,
    interest_type,
    effective_from,
    effective_to,
    source,
    source_document_ref,
    notes,
    created_at,
    created_by,
    updated_at
FROM "ob-poc".entity_relationships
WHERE effective_to IS NULL OR effective_to > CURRENT_DATE;

-- Recreate cbu_ownership_graph view
CREATE OR REPLACE VIEW "ob-poc".cbu_ownership_graph AS
SELECT v.cbu_id,
    r.relationship_id,
    r.from_entity_id,
    e_from.name AS from_entity_name,
    et_from.entity_category AS from_entity_category,
    r.to_entity_id,
    e_to.name AS to_entity_name,
    et_to.entity_category AS to_entity_category,
    r.relationship_type,
    r.percentage,
    r.ownership_type,
    r.control_type,
    r.trust_role,
    r.interest_type,
    v.status AS verification_status,
    v.alleged_percentage,
    v.observed_percentage,
    v.proof_document_id,
    r.effective_from,
    r.effective_to
FROM "ob-poc".entity_relationships r
JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
LEFT JOIN "ob-poc".entities e_from ON e_from.entity_id = r.from_entity_id
LEFT JOIN "ob-poc".entity_types et_from ON et_from.entity_type_id = e_from.entity_type_id
LEFT JOIN "ob-poc".entities e_to ON e_to.entity_id = r.to_entity_id
LEFT JOIN "ob-poc".entity_types et_to ON et_to.entity_type_id = e_to.entity_type_id
WHERE r.effective_to IS NULL OR r.effective_to > CURRENT_DATE;

-- Direct or indirect ownership/control
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS direct_or_indirect VARCHAR(10) DEFAULT 'unknown';

-- Share ranges (BODS allows min/max, not just exact)
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_minimum NUMERIC(5,2);

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_maximum NUMERIC(5,2);

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_exclusive_minimum BOOLEAN DEFAULT false;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS share_exclusive_maximum BOOLEAN DEFAULT false;

-- Component tracking for indirect chains
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS is_component BOOLEAN DEFAULT false;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS component_of_relationship_id UUID;

-- Statement tracking
ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS statement_date DATE;

ALTER TABLE "ob-poc".entity_relationships
ADD COLUMN IF NOT EXISTS replaces_relationship_id UUID;

-- Index for interest type
CREATE INDEX IF NOT EXISTS idx_entity_rel_interest_type
    ON "ob-poc".entity_relationships(interest_type);

CREATE INDEX IF NOT EXISTS idx_entity_rel_component_of
    ON "ob-poc".entity_relationships(component_of_relationship_id)
    WHERE is_component = true;

-- ============================================================================
-- 6. PERSON PEP STATUS
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".person_pep_status (
    pep_status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    status VARCHAR(20) NOT NULL CHECK (status IN ('isPep', 'isNotPep', 'unknown')),
    reason TEXT,
    jurisdiction VARCHAR(10),
    position_held TEXT,
    position_level VARCHAR(30),
    start_date DATE,
    end_date DATE,

    source_type VARCHAR(50),
    source_reference TEXT,
    screening_id UUID,

    verified_at TIMESTAMPTZ,
    verified_by VARCHAR(255),
    verification_notes TEXT,
    pep_risk_level VARCHAR(20),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_person_pep_entity ON "ob-poc".person_pep_status(person_entity_id);
CREATE INDEX IF NOT EXISTS idx_person_pep_status ON "ob-poc".person_pep_status(status);
-- Note: Partial index for active PEP status - using end_date IS NULL for currently active
-- Runtime filtering on end_date > CURRENT_DATE should be done in queries
CREATE INDEX IF NOT EXISTS idx_person_pep_active
    ON "ob-poc".person_pep_status(person_entity_id)
    WHERE status = 'isPep' AND end_date IS NULL;

COMMENT ON TABLE "ob-poc".person_pep_status IS 'BODS-compliant PEP status tracking.';

-- ============================================================================
-- 7. ADD BODS COLUMNS TO ENTITIES TABLE
-- ============================================================================

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS bods_entity_type VARCHAR(30);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS bods_entity_subtype VARCHAR(30);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS founding_date DATE;

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS dissolution_date DATE;

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS is_publicly_listed BOOLEAN DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_entities_bods_type ON "ob-poc".entities(bods_entity_type);

-- ============================================================================
-- 8. HELPER VIEWS
-- ============================================================================

-- View: Entities with their LEIs
CREATE OR REPLACE VIEW "ob-poc".v_entities_with_lei AS
SELECT
    e.entity_id,
    e.name,
    e.bods_entity_type,
    et.type_code AS entity_type_code,
    ei.identifier_value AS lei,
    ei.lei_status,
    ei.lei_next_renewal,
    ei.is_validated AS lei_validated
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
LEFT JOIN "ob-poc".entity_identifiers ei ON ei.entity_id = e.entity_id AND ei.identifier_type = 'LEI';

-- View: UBO interests with BODS type info
CREATE OR REPLACE VIEW "ob-poc".v_ubo_interests AS
SELECT
    er.relationship_id,
    er.from_entity_id AS interested_party_id,
    owner.name AS interested_party_name,
    er.to_entity_id AS subject_id,
    subject.name AS subject_name,
    er.interest_type,
    bit.display_name AS interest_type_display,
    bit.category AS interest_category,
    er.direct_or_indirect,
    COALESCE(er.percentage, er.share_minimum, er.share_maximum) AS ownership_share,
    er.share_minimum,
    er.share_maximum,
    er.effective_from,
    er.effective_to,
    er.is_component,
    er.component_of_relationship_id
FROM "ob-poc".entity_relationships er
JOIN "ob-poc".entities owner ON owner.entity_id = er.from_entity_id
JOIN "ob-poc".entities subject ON subject.entity_id = er.to_entity_id
LEFT JOIN "ob-poc".bods_interest_types bit ON bit.type_code = er.interest_type
WHERE er.relationship_type IN ('ownership', 'control', 'trust_role')
  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE);

-- View: GLEIF corporate hierarchy
CREATE OR REPLACE VIEW "ob-poc".v_gleif_hierarchy AS
SELECT
    gr.gleif_rel_id,
    gr.parent_entity_id,
    parent.name AS parent_name,
    gr.parent_lei,
    gr.child_entity_id,
    child.name AS child_name,
    gr.child_lei,
    gr.relationship_type,
    gr.relationship_status,
    gr.ownership_percentage,
    gr.accounting_standard,
    gr.start_date,
    gr.end_date
FROM "ob-poc".gleif_relationships gr
JOIN "ob-poc".entities parent ON parent.entity_id = gr.parent_entity_id
JOIN "ob-poc".entities child ON child.entity_id = gr.child_entity_id
WHERE gr.relationship_status = 'ACTIVE' OR gr.relationship_status IS NULL;

-- ============================================================================
-- 9. BACKFILL: Map existing relationship_type to interest_type
-- ============================================================================

UPDATE "ob-poc".entity_relationships
SET interest_type = CASE
    WHEN relationship_type = 'ownership' AND (ownership_type = 'DIRECT' OR ownership_type IS NULL) THEN 'shareholding'
    WHEN relationship_type = 'ownership' AND ownership_type = 'VOTING' THEN 'votingRights'
    WHEN relationship_type = 'ownership' AND ownership_type = 'BENEFICIAL' THEN 'shareholding'
    WHEN relationship_type = 'control' AND control_type = 'board_member' THEN 'boardMember'
    WHEN relationship_type = 'control' AND control_type = 'executive' THEN 'seniorManagingOfficial'
    WHEN relationship_type = 'control' AND control_type = 'voting_rights' THEN 'votingRights'
    WHEN relationship_type = 'control' THEN 'otherInfluenceOrControl'
    WHEN relationship_type = 'trust_role' AND trust_role = 'settlor' THEN 'settlor'
    WHEN relationship_type = 'trust_role' AND trust_role = 'trustee' THEN 'trustee'
    WHEN relationship_type = 'trust_role' AND trust_role = 'beneficiary' THEN 'beneficiaryOfLegalArrangement'
    WHEN relationship_type = 'trust_role' AND trust_role = 'protector' THEN 'protector'
    ELSE NULL
END
WHERE interest_type IS NULL;

UPDATE "ob-poc".entity_relationships
SET direct_or_indirect = CASE
    WHEN ownership_type = 'DIRECT' THEN 'direct'
    WHEN ownership_type = 'INDIRECT' THEN 'indirect'
    ELSE 'unknown'
END
WHERE direct_or_indirect = 'unknown' OR direct_or_indirect IS NULL;

COMMIT;
