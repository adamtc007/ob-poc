-- Migration: 022_control_edges.sql
-- Purpose: Control edges with BODS/GLEIF/PSC standards alignment + derived board controller
--
-- Key concepts:
-- 1. control_edges: Ownership/voting/board control edges with standard xrefs
-- 2. cbu_board_controller: Materialized derived edge (computed, not hand-authored)
-- 3. board_control_evidence: Audit trail of evidence used in derivation
--
-- Standards alignment:
-- - BODS: https://standard.openownership.org/en/0.3.0/schema/reference.html#interest
-- - GLEIF RR: https://www.gleif.org/en/about-lei/common-data-file-format
-- - UK PSC: https://www.gov.uk/guidance/people-with-significant-control-pscs

SET search_path TO "ob-poc", public;

-- ============================================================================
-- CONTROL EDGES TABLE
-- Stores ownership/voting/control relationships with standard cross-references
-- ============================================================================

CREATE TABLE IF NOT EXISTS control_edges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    to_entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,

    -- Our canonical edge type (aligned to BODS interest types where applicable)
    edge_type TEXT NOT NULL,

    -- Standard cross-references
    bods_interest_type TEXT,           -- e.g., 'shareholding', 'voting-rights', 'appointment-of-board'
    gleif_relationship_type TEXT,      -- e.g., 'IS_DIRECTLY_CONSOLIDATED_BY', 'IS_FUND_MANAGED_BY'
    psc_category TEXT,                 -- e.g., 'ownership-of-shares-25-to-50', 'appoints-majority-of-board'

    -- Quantitative
    percentage DECIMAL(5,2),           -- NULL if boolean/qualitative
    is_direct BOOLEAN DEFAULT true,    -- direct vs indirect holding

    -- Qualitative flags
    is_beneficial BOOLEAN DEFAULT false,  -- beneficial vs legal ownership
    is_legal BOOLEAN DEFAULT true,        -- legal ownership (vs purely economic)

    -- Share class specifics (for voting vs economic split)
    share_class_id UUID,                  -- FK to share_classes if applicable
    votes_per_share DECIMAL(10,4),        -- voting power per unit

    -- Provenance
    source_document_id UUID,              -- FK to documents if we have the source doc
    source_register TEXT,                 -- 'gleif', 'uk-psc', 'lux-rbe', 'sec-13d', 'manual'
    source_reference TEXT,                -- External ID in source register
    effective_date DATE,
    end_date DATE,                        -- NULL = still active

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    created_by TEXT,

    -- Constraints
    CONSTRAINT valid_edge_type CHECK (edge_type IN (
        -- Ownership/Voting (BODS-aligned)
        'HOLDS_SHARES',
        'HOLDS_VOTING_RIGHTS',

        -- Board control (BODS-aligned)
        'APPOINTS_BOARD',
        'EXERCISES_INFLUENCE',
        'IS_SENIOR_MANAGER',

        -- Trust arrangements (BODS-aligned)
        'IS_SETTLOR',
        'IS_TRUSTEE',
        'IS_PROTECTOR',
        'IS_BENEFICIARY',

        -- Economic rights (BODS-aligned)
        'HAS_DISSOLUTION_RIGHTS',
        'HAS_PROFIT_RIGHTS',

        -- GLEIF hierarchy
        'CONSOLIDATED_BY',
        'ULTIMATELY_CONSOLIDATED_BY',
        'MANAGED_BY',
        'SUBFUND_OF',
        'FEEDS_INTO'
    )),

    CONSTRAINT valid_bods_interest CHECK (bods_interest_type IS NULL OR bods_interest_type IN (
        'shareholding',
        'voting-rights',
        'appointment-of-board',
        'other-influence-or-control',
        'senior-managing-official',
        'settlor-of-trust',
        'trustee-of-trust',
        'protector-of-trust',
        'beneficiary-of-trust',
        'rights-to-surplus-assets-on-dissolution',
        'rights-to-profit-or-income'
    )),

    CONSTRAINT valid_gleif_relationship CHECK (gleif_relationship_type IS NULL OR gleif_relationship_type IN (
        'IS_DIRECTLY_CONSOLIDATED_BY',
        'IS_ULTIMATELY_CONSOLIDATED_BY',
        'IS_FUND_MANAGED_BY',
        'IS_SUBFUND_OF',
        'IS_FEEDER_TO'
    )),

    CONSTRAINT valid_psc_category CHECK (psc_category IS NULL OR psc_category IN (
        'ownership-of-shares-25-to-50',
        'ownership-of-shares-50-to-75',
        'ownership-of-shares-75-to-100',
        'voting-rights-25-to-50',
        'voting-rights-50-to-75',
        'voting-rights-75-to-100',
        'appoints-majority-of-board',
        'significant-influence-or-control'
    )),

    CONSTRAINT valid_percentage CHECK (percentage IS NULL OR (percentage >= 0 AND percentage <= 100))
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_control_edges_from ON control_edges(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_edges_to ON control_edges(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_edges_type ON control_edges(edge_type);
CREATE INDEX IF NOT EXISTS idx_control_edges_effective ON control_edges(effective_date) WHERE end_date IS NULL;
CREATE INDEX IF NOT EXISTS idx_control_edges_source ON control_edges(source_register);

-- Unique constraint: only one active edge of same type between same entities
CREATE UNIQUE INDEX IF NOT EXISTS idx_control_edges_unique_active
ON control_edges(from_entity_id, to_entity_id, edge_type)
WHERE end_date IS NULL;

-- ============================================================================
-- CBU BOARD CONTROLLER TABLE
-- Materialized derived edge: who controls the board of a CBU's entities
-- This is COMPUTED by the rules engine, not hand-authored
-- ============================================================================

CREATE TABLE IF NOT EXISTS cbu_board_controller (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,

    -- The computed controller (NULL = no single controller identified)
    controller_entity_id UUID REFERENCES entities(entity_id),
    controller_name TEXT,  -- Cached for display

    -- Derivation method (which rule fired)
    method TEXT NOT NULL CHECK (method IN (
        'board_appointment_rights',  -- Rule A: explicit appointment rights
        'voting_rights_majority',    -- Rule B: >50% voting power
        'special_instrument',        -- Rule C: golden share, GP, trustee
        'mixed',                     -- Multiple rules contributed
        'no_single_controller'       -- Rule D: no entity meets threshold
    )),

    -- Confidence in the derivation
    confidence TEXT NOT NULL CHECK (confidence IN ('high', 'medium', 'low')),

    -- Composite score (0.0 - 1.0)
    score DECIMAL(3,2) NOT NULL CHECK (score >= 0 AND score <= 1),

    -- As-of date for the computation
    as_of DATE NOT NULL,

    -- Full explanation payload (JSON)
    -- Contains: candidates, evidence_refs, data_gaps, scoring breakdown
    explanation JSONB NOT NULL DEFAULT '{}',

    -- When this was computed
    computed_at TIMESTAMPTZ DEFAULT NOW(),

    -- Who/what triggered the computation
    computed_by TEXT,  -- 'system', 'user:uuid', 'edge_change:uuid'

    -- One derived controller per CBU
    CONSTRAINT unique_cbu_board_controller UNIQUE(cbu_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_board_controller_controller ON cbu_board_controller(controller_entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_board_controller_method ON cbu_board_controller(method);
CREATE INDEX IF NOT EXISTS idx_cbu_board_controller_confidence ON cbu_board_controller(confidence);

-- ============================================================================
-- BOARD CONTROL EVIDENCE TABLE
-- Audit trail of evidence used in board control derivation
-- ============================================================================

CREATE TABLE IF NOT EXISTS board_control_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_board_controller_id UUID NOT NULL REFERENCES cbu_board_controller(id) ON DELETE CASCADE,

    -- Evidence source type
    source_type TEXT NOT NULL CHECK (source_type IN (
        'gleif_rr',           -- GLEIF Relationship Record
        'bods_statement',     -- BODS ownership statement
        'investor_register',  -- Share register entry
        'governance_doc',     -- Articles of association, shareholder agreement
        'special_instrument', -- Golden share, GP/LP agreement, trust deed
        'manual_entry'        -- User-entered override with justification
    )),

    -- Reference to source
    source_id TEXT NOT NULL,          -- Document ID, register entry ID, external ref
    source_register TEXT,             -- 'gleif', 'uk-psc', 'lux-rbe', etc.

    -- What this evidence shows
    description TEXT NOT NULL,

    -- Evidence details (JSON for flexibility)
    details JSONB DEFAULT '{}',

    -- As-of date for this evidence
    as_of DATE,

    -- When captured
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_board_control_evidence_controller ON board_control_evidence(cbu_board_controller_id);
CREATE INDEX IF NOT EXISTS idx_board_control_evidence_source ON board_control_evidence(source_type);

-- ============================================================================
-- CBU CONTROL ANCHORS TABLE
-- Links CBU to entities that bridge into ownership/control graph
-- These are the "portal" entities - clicking navigates to control sphere
-- ============================================================================

CREATE TABLE IF NOT EXISTS cbu_control_anchors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,

    -- Role of this anchor
    anchor_role TEXT NOT NULL CHECK (anchor_role IN (
        'governance',  -- ManCo, board oversight - who controls the board
        'sponsor',     -- Parent group, ultimate controller
        'issuer'       -- Fund legal entity itself (the thing that's owned)
    )),

    -- Cached display info
    display_name TEXT,
    jurisdiction TEXT,

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Each entity can only have one role per CBU
    CONSTRAINT unique_cbu_anchor_role UNIQUE(cbu_id, entity_id, anchor_role)
);

CREATE INDEX IF NOT EXISTS idx_cbu_control_anchors_cbu ON cbu_control_anchors(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_control_anchors_entity ON cbu_control_anchors(entity_id);

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to auto-set BODS interest type from edge_type
CREATE OR REPLACE FUNCTION set_bods_interest_type()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.bods_interest_type IS NULL THEN
        NEW.bods_interest_type := CASE NEW.edge_type
            WHEN 'HOLDS_SHARES' THEN 'shareholding'
            WHEN 'HOLDS_VOTING_RIGHTS' THEN 'voting-rights'
            WHEN 'APPOINTS_BOARD' THEN 'appointment-of-board'
            WHEN 'EXERCISES_INFLUENCE' THEN 'other-influence-or-control'
            WHEN 'IS_SENIOR_MANAGER' THEN 'senior-managing-official'
            WHEN 'IS_SETTLOR' THEN 'settlor-of-trust'
            WHEN 'IS_TRUSTEE' THEN 'trustee-of-trust'
            WHEN 'IS_PROTECTOR' THEN 'protector-of-trust'
            WHEN 'IS_BENEFICIARY' THEN 'beneficiary-of-trust'
            WHEN 'HAS_DISSOLUTION_RIGHTS' THEN 'rights-to-surplus-assets-on-dissolution'
            WHEN 'HAS_PROFIT_RIGHTS' THEN 'rights-to-profit-or-income'
            ELSE NULL
        END;
    END IF;

    -- Auto-set GLEIF relationship type
    IF NEW.gleif_relationship_type IS NULL THEN
        NEW.gleif_relationship_type := CASE NEW.edge_type
            WHEN 'CONSOLIDATED_BY' THEN 'IS_DIRECTLY_CONSOLIDATED_BY'
            WHEN 'ULTIMATELY_CONSOLIDATED_BY' THEN 'IS_ULTIMATELY_CONSOLIDATED_BY'
            WHEN 'MANAGED_BY' THEN 'IS_FUND_MANAGED_BY'
            WHEN 'SUBFUND_OF' THEN 'IS_SUBFUND_OF'
            WHEN 'FEEDS_INTO' THEN 'IS_FEEDER_TO'
            ELSE NULL
        END;
    END IF;

    -- Auto-set PSC category based on edge type + percentage
    IF NEW.psc_category IS NULL AND NEW.percentage IS NOT NULL THEN
        NEW.psc_category := CASE
            WHEN NEW.edge_type = 'HOLDS_SHARES' AND NEW.percentage > 75 THEN 'ownership-of-shares-75-to-100'
            WHEN NEW.edge_type = 'HOLDS_SHARES' AND NEW.percentage > 50 THEN 'ownership-of-shares-50-to-75'
            WHEN NEW.edge_type = 'HOLDS_SHARES' AND NEW.percentage > 25 THEN 'ownership-of-shares-25-to-50'
            WHEN NEW.edge_type = 'HOLDS_VOTING_RIGHTS' AND NEW.percentage > 75 THEN 'voting-rights-75-to-100'
            WHEN NEW.edge_type = 'HOLDS_VOTING_RIGHTS' AND NEW.percentage > 50 THEN 'voting-rights-50-to-75'
            WHEN NEW.edge_type = 'HOLDS_VOTING_RIGHTS' AND NEW.percentage > 25 THEN 'voting-rights-25-to-50'
            WHEN NEW.edge_type = 'APPOINTS_BOARD' AND NEW.percentage > 50 THEN 'appoints-majority-of-board'
            WHEN NEW.edge_type = 'EXERCISES_INFLUENCE' THEN 'significant-influence-or-control'
            ELSE NULL
        END;
    END IF;

    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_control_edges_set_standards ON control_edges;
CREATE TRIGGER trg_control_edges_set_standards
    BEFORE INSERT OR UPDATE ON control_edges
    FOR EACH ROW
    EXECUTE FUNCTION set_bods_interest_type();

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE control_edges IS 'Ownership/voting/control edges with BODS/GLEIF/PSC standards alignment';
COMMENT ON TABLE cbu_board_controller IS 'Materialized derived edge: computed board controller per CBU (not hand-authored)';
COMMENT ON TABLE board_control_evidence IS 'Audit trail of evidence used in board control derivation';
COMMENT ON TABLE cbu_control_anchors IS 'Portal entities linking CBU to ownership/control graph';

COMMENT ON COLUMN control_edges.edge_type IS 'Canonical edge type aligned to BODS interest types';
COMMENT ON COLUMN control_edges.bods_interest_type IS 'BODS interest type (auto-set from edge_type)';
COMMENT ON COLUMN control_edges.gleif_relationship_type IS 'GLEIF RR type for legal entity hierarchy';
COMMENT ON COLUMN control_edges.psc_category IS 'UK PSC category (auto-set from edge_type + percentage)';

COMMENT ON COLUMN cbu_board_controller.method IS 'Which derivation rule fired: A=appointment, B=voting, C=special, D=none';
COMMENT ON COLUMN cbu_board_controller.explanation IS 'Full derivation audit trail: candidates, evidence, data gaps, scoring';
