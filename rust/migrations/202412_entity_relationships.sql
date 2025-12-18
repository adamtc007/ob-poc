-- ═══════════════════════════════════════════════════════════════════════════
-- ENTITY RELATIONSHIP GRAPH CONSOLIDATION
-- ═══════════════════════════════════════════════════════════════════════════
--
-- This migration creates a clean separation:
--   1. entity_relationships - Structural facts about the world (no CBU scope)
--   2. cbu_relationship_verification - KYC workflow state per CBU
--
-- Key insight: "Allianz GI owns 100% of Fund X" is a fact that exists
-- independent of any CBU context. Multiple CBUs might reference the same
-- relationship but have different verification states.
--
-- ═══════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 1: CREATE NEW TABLES
-- ═══════════════════════════════════════════════════════════════════════════

-- ───────────────────────────────────────────────────────────────────────────
-- ENTITY_RELATIONSHIPS: Single source of truth for all entity edges
-- ───────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS "ob-poc".entity_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- ═══════════════════════════════════════════════════════════════════════
    -- THE EDGE (structural fact about the world)
    -- ═══════════════════════════════════════════════════════════════════════
    from_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    to_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Relationship type
    relationship_type VARCHAR(30) NOT NULL,
    -- 'ownership'      - A owns X% of B
    -- 'control'        - A controls B (board, voting, executive)
    -- 'trust_role'     - A has role in trust B (settlor, trustee, beneficiary, protector)
    -- 'employment'     - A is employed by B (for control prong)
    -- 'management'     - A manages B (fund/manco relationship)

    -- ═══════════════════════════════════════════════════════════════════════
    -- OWNERSHIP SPECIFICS
    -- ═══════════════════════════════════════════════════════════════════════
    percentage DECIMAL(5,2),  -- NULL for non-ownership relationships

    -- Ownership subtype
    ownership_type VARCHAR(30),
    -- 'direct'         - Direct shareholding
    -- 'indirect'       - Through intermediary
    -- 'beneficial'     - Beneficial interest (trusts)
    -- 'voting'         - Voting rights (may differ from economic)

    -- ═══════════════════════════════════════════════════════════════════════
    -- CONTROL SPECIFICS
    -- ═══════════════════════════════════════════════════════════════════════
    control_type VARCHAR(30),
    -- 'board_member'       - Board/director position
    -- 'executive'          - C-suite (CEO, CFO, etc.)
    -- 'voting_rights'      - >25% voting without ownership
    -- 'appointment_rights' - Right to appoint/remove directors
    -- 'veto_rights'        - Veto over key decisions
    -- 'managing_partner'   - Partnership control

    -- ═══════════════════════════════════════════════════════════════════════
    -- TRUST ROLE SPECIFICS
    -- ═══════════════════════════════════════════════════════════════════════
    trust_role VARCHAR(30),
    -- 'settlor'        - Created the trust
    -- 'trustee'        - Manages trust assets
    -- 'beneficiary'    - Entitled to trust benefits
    -- 'protector'      - Oversight/veto powers

    -- Beneficiary interest type (for beneficiary role)
    interest_type VARCHAR(20),
    -- 'fixed'          - Fixed percentage entitlement (treated as ownership)
    -- 'discretionary'  - At trustee discretion (flagged but not % ownership)
    -- 'contingent'     - Future/conditional interest

    -- ═══════════════════════════════════════════════════════════════════════
    -- TEMPORAL (when was this relationship active?)
    -- ═══════════════════════════════════════════════════════════════════════
    effective_from DATE,          -- NULL = unknown start
    effective_to DATE,            -- NULL = still active

    -- ═══════════════════════════════════════════════════════════════════════
    -- METADATA
    -- ═══════════════════════════════════════════════════════════════════════
    source VARCHAR(100),          -- 'client_disclosure', 'public_registry', 'annual_report'
    source_document_ref VARCHAR(255),  -- Reference to source document
    notes TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID,
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- ═══════════════════════════════════════════════════════════════════════
    -- CONSTRAINTS
    -- ═══════════════════════════════════════════════════════════════════════
    CONSTRAINT chk_er_relationship_type CHECK (
        relationship_type IN ('ownership', 'control', 'trust_role', 'employment', 'management')
    ),
    CONSTRAINT chk_er_ownership_has_percentage CHECK (
        relationship_type != 'ownership' OR percentage IS NOT NULL
    ),
    CONSTRAINT chk_er_temporal_valid CHECK (
        effective_to IS NULL OR effective_from IS NULL OR effective_from <= effective_to
    ),
    CONSTRAINT chk_er_no_self_reference CHECK (
        from_entity_id != to_entity_id
    )
);

-- Unique constraint: one relationship of each type between two entities at a time
-- Using a partial unique index to handle NULL effective_to properly
CREATE UNIQUE INDEX IF NOT EXISTS idx_entity_rel_unique_active
ON "ob-poc".entity_relationships(from_entity_id, to_entity_id, relationship_type)
WHERE effective_to IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_entity_rel_unique_historical
ON "ob-poc".entity_relationships(from_entity_id, to_entity_id, relationship_type, effective_to)
WHERE effective_to IS NOT NULL;

-- Query indexes
CREATE INDEX IF NOT EXISTS idx_entity_rel_from ON "ob-poc".entity_relationships(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_rel_to ON "ob-poc".entity_relationships(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_rel_type ON "ob-poc".entity_relationships(relationship_type);
CREATE INDEX IF NOT EXISTS idx_entity_rel_temporal ON "ob-poc".entity_relationships(effective_from, effective_to);


-- ───────────────────────────────────────────────────────────────────────────
-- CBU_RELATIONSHIP_VERIFICATION: KYC workflow state per relationship per CBU
-- ───────────────────────────────────────────────────────────────────────────
--
-- This table tracks the KYC verification STATUS of relationships within
-- a specific CBU context. The relationship itself exists in entity_relationships.
-- This tracks: "Has this relationship been verified for this CBU's KYC?"
--
-- A relationship may be:
-- - Unverified in CBU A (new client)
-- - Verified/proven in CBU B (existing client with proofs on file)
-- - Disputed in CBU C (proof contradicts allegation)

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_relationship_verification (
    verification_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- ═══════════════════════════════════════════════════════════════════════
    -- REFERENCES
    -- ═══════════════════════════════════════════════════════════════════════
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    relationship_id UUID NOT NULL REFERENCES "ob-poc".entity_relationships(relationship_id),

    -- ═══════════════════════════════════════════════════════════════════════
    -- ALLEGATION (what client claims for THIS CBU)
    -- ═══════════════════════════════════════════════════════════════════════
    -- Note: This may differ from entity_relationships.percentage if client
    -- provides different information than what's in the canonical record
    alleged_percentage DECIMAL(5,2),
    alleged_at TIMESTAMPTZ,
    alleged_by UUID,
    allegation_source VARCHAR(100),  -- 'client_disclosure', 'onboarding_form'

    -- ═══════════════════════════════════════════════════════════════════════
    -- PROOF
    -- ═══════════════════════════════════════════════════════════════════════
    proof_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    observed_percentage DECIMAL(5,2),  -- What the proof shows

    -- ═══════════════════════════════════════════════════════════════════════
    -- VERIFICATION STATUS (KYC workflow state)
    -- ═══════════════════════════════════════════════════════════════════════
    status VARCHAR(20) NOT NULL DEFAULT 'unverified',
    -- 'unverified'     - Relationship exists, not yet part of KYC
    -- 'alleged'        - Client has made allegation, no proof yet
    -- 'pending'        - Proof linked, awaiting verification
    -- 'proven'         - Proof confirms allegation
    -- 'disputed'       - Proof contradicts allegation
    -- 'waived'         - Verification waived (with reason)

    -- Dispute details
    discrepancy_notes TEXT,

    -- Resolution
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    resolution_notes TEXT,

    -- ═══════════════════════════════════════════════════════════════════════
    -- AUDIT
    -- ═══════════════════════════════════════════════════════════════════════
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- ═══════════════════════════════════════════════════════════════════════
    -- CONSTRAINTS
    -- ═══════════════════════════════════════════════════════════════════════
    CONSTRAINT chk_crv_status CHECK (
        status IN ('unverified', 'alleged', 'pending', 'proven', 'disputed', 'waived')
    ),

    -- One verification record per CBU per relationship
    UNIQUE(cbu_id, relationship_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbu_rel_verif_cbu ON "ob-poc".cbu_relationship_verification(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_rel_verif_status ON "ob-poc".cbu_relationship_verification(cbu_id, status);
CREATE INDEX IF NOT EXISTS idx_cbu_rel_verif_rel ON "ob-poc".cbu_relationship_verification(relationship_id);


-- ═══════════════════════════════════════════════════════════════════════════
-- CONVENIENCE VIEWS
-- ═══════════════════════════════════════════════════════════════════════════

-- ───────────────────────────────────────────────────────────────────────────
-- VIEW: Current (non-expired) relationships
-- ───────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW "ob-poc".entity_relationships_current AS
SELECT *
FROM "ob-poc".entity_relationships
WHERE effective_to IS NULL
   OR effective_to > CURRENT_DATE;


-- ───────────────────────────────────────────────────────────────────────────
-- VIEW: CBU ownership graph with verification status
-- ───────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW "ob-poc".cbu_ownership_graph AS
SELECT
    v.cbu_id,
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


-- ───────────────────────────────────────────────────────────────────────────
-- VIEW: CBU convergence status (aggregated from cbu_relationship_verification)
-- ───────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW "ob-poc".cbu_convergence_status AS
SELECT
    cbu_id,
    COUNT(*) AS total_relationships,
    COUNT(*) FILTER (WHERE status = 'proven') AS proven_count,
    COUNT(*) FILTER (WHERE status = 'alleged') AS alleged_count,
    COUNT(*) FILTER (WHERE status = 'pending') AS pending_count,
    COUNT(*) FILTER (WHERE status = 'disputed') AS disputed_count,
    COUNT(*) FILTER (WHERE status = 'unverified') AS unverified_count,
    COUNT(*) FILTER (WHERE status = 'waived') AS waived_count,
    COUNT(*) FILTER (WHERE status IN ('proven', 'waived')) = COUNT(*) AS is_converged
FROM "ob-poc".cbu_relationship_verification
GROUP BY cbu_id;


-- ───────────────────────────────────────────────────────────────────────────
-- VIEW: UBO candidates (natural persons with ≥25% through proven edges)
-- ───────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW "ob-poc".v_ubo_candidates AS
WITH RECURSIVE ownership_chain AS (
    -- Base: direct ownership edges that are verified for this CBU
    SELECT
        v.cbu_id,
        r.from_entity_id AS owned_entity_id,
        r.to_entity_id AS owner_entity_id,
        COALESCE(v.observed_percentage, v.alleged_percentage, r.percentage) AS effective_percentage,
        1 AS depth,
        ARRAY[r.from_entity_id, r.to_entity_id] AS path
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
    WHERE r.relationship_type = 'ownership'
      AND v.status IN ('proven', 'alleged', 'pending')  -- Include alleged for visibility
      AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)

    UNION ALL

    -- Recursive: follow the chain upward
    SELECT
        oc.cbu_id,
        oc.owned_entity_id,  -- Keep the original owned entity
        r.to_entity_id AS owner_entity_id,
        oc.effective_percentage * COALESCE(v.observed_percentage, v.alleged_percentage, r.percentage) / 100 AS effective_percentage,
        oc.depth + 1,
        oc.path || r.to_entity_id
    FROM ownership_chain oc
    JOIN "ob-poc".entity_relationships r ON r.from_entity_id = oc.owner_entity_id
    JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
        AND v.cbu_id = oc.cbu_id
    WHERE r.relationship_type = 'ownership'
      AND v.status IN ('proven', 'alleged', 'pending')
      AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
      AND oc.depth < 10  -- Prevent infinite loops
      AND NOT r.to_entity_id = ANY(oc.path)  -- Prevent cycles
)
SELECT
    oc.cbu_id,
    oc.owner_entity_id AS entity_id,
    e.name AS entity_name,
    et.entity_category,
    et.name AS entity_type_name,
    SUM(oc.effective_percentage) AS total_effective_percentage,
    et.entity_category = 'PERSON' AS is_natural_person,
    et.entity_category = 'PERSON' AND SUM(oc.effective_percentage) >= 25 AS is_ubo,
    -- Include verification status info
    bool_and(v.status = 'proven') AS all_paths_proven,
    bool_or(v.status = 'disputed') AS has_disputed_path
FROM ownership_chain oc
JOIN "ob-poc".entities e ON e.entity_id = oc.owner_entity_id
JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
-- Join back to get verification status of the edges in the path
LEFT JOIN "ob-poc".entity_relationships r ON r.to_entity_id = oc.owner_entity_id
    AND r.relationship_type = 'ownership'
    AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
LEFT JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
    AND v.cbu_id = oc.cbu_id
GROUP BY oc.cbu_id, oc.owner_entity_id, e.name, et.entity_category, et.name
HAVING SUM(oc.effective_percentage) >= 25 OR et.entity_category = 'PERSON';


-- ───────────────────────────────────────────────────────────────────────────
-- HELPER FUNCTION: Check if entity is a natural person
-- ───────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION "ob-poc".is_natural_person(p_entity_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE e.entity_id = p_entity_id
        AND et.entity_category = 'PERSON'
    );
END;
$$ LANGUAGE plpgsql STABLE;


-- ═══════════════════════════════════════════════════════════════════════════
-- PHASE 2: DATA MIGRATION
-- ═══════════════════════════════════════════════════════════════════════════

-- ───────────────────────────────────────────────────────────────────────────
-- Migrate from legacy ownership_relationships
-- ───────────────────────────────────────────────────────────────────────────
INSERT INTO "ob-poc".entity_relationships (
    relationship_id,
    from_entity_id,
    to_entity_id,
    relationship_type,
    percentage,
    ownership_type,
    effective_from,
    effective_to,
    source,
    created_at
)
SELECT
    COALESCE(relationship_id, gen_random_uuid()),
    owned_entity_id,      -- FROM = entity being owned
    owner_entity_id,      -- TO = the owner
    'ownership',
    ownership_percent,
    COALESCE(ownership_type, 'direct'),
    effective_from,
    effective_to,
    'legacy_migration',
    COALESCE(created_at, NOW())
FROM "ob-poc".ownership_relationships
ON CONFLICT DO NOTHING;


-- ───────────────────────────────────────────────────────────────────────────
-- Migrate from legacy control_relationships
-- ───────────────────────────────────────────────────────────────────────────
INSERT INTO "ob-poc".entity_relationships (
    from_entity_id,
    to_entity_id,
    relationship_type,
    control_type,
    effective_from,
    effective_to,
    source,
    created_at
)
SELECT
    controlled_entity_id,  -- FROM = entity being controlled
    controller_entity_id,  -- TO = the controller
    'control',
    control_type,
    effective_from,
    effective_to,
    'legacy_migration',
    COALESCE(created_at, NOW())
FROM "ob-poc".control_relationships
ON CONFLICT DO NOTHING;


-- ───────────────────────────────────────────────────────────────────────────
-- Migrate from ubo_edges (if it exists and has data not in legacy tables)
-- ───────────────────────────────────────────────────────────────────────────
DO $$
BEGIN
    -- Only run if ubo_edges table exists
    IF EXISTS (SELECT 1 FROM information_schema.tables
               WHERE table_schema = 'ob-poc' AND table_name = 'ubo_edges') THEN

        -- Insert relationships from ubo_edges that don't already exist
        INSERT INTO "ob-poc".entity_relationships (
            from_entity_id,
            to_entity_id,
            relationship_type,
            percentage,
            control_type,
            trust_role,
            interest_type,
            effective_from,
            effective_to,
            source,
            created_at
        )
        SELECT
            ue.from_entity_id,
            ue.to_entity_id,
            ue.edge_type,
            COALESCE(ue.proven_percentage, ue.alleged_percentage, ue.percentage),
            ue.control_role,
            ue.trust_role,
            ue.interest_type,
            ue.effective_from,
            ue.effective_to,
            COALESCE(ue.allegation_source, 'ubo_edges_migration'),
            COALESCE(ue.created_at, NOW())
        FROM "ob-poc".ubo_edges ue
        WHERE NOT EXISTS (
            -- Don't duplicate if already exists from legacy migration
            SELECT 1 FROM "ob-poc".entity_relationships er
            WHERE er.from_entity_id = ue.from_entity_id
              AND er.to_entity_id = ue.to_entity_id
              AND er.relationship_type = ue.edge_type
              AND (er.effective_to IS NULL AND ue.effective_to IS NULL
                   OR er.effective_to = ue.effective_to)
        )
        ON CONFLICT DO NOTHING;

        -- Create verification records for CBU-scoped edges
        INSERT INTO "ob-poc".cbu_relationship_verification (
            cbu_id,
            relationship_id,
            alleged_percentage,
            alleged_at,
            proof_document_id,
            observed_percentage,
            status,
            created_at
        )
        SELECT
            ue.cbu_id,
            er.relationship_id,
            ue.alleged_percentage,
            ue.alleged_at,
            ue.proof_id,
            ue.proven_percentage,
            COALESCE(ue.status, 'unverified'),
            COALESCE(ue.created_at, NOW())
        FROM "ob-poc".ubo_edges ue
        JOIN "ob-poc".entity_relationships er
            ON er.from_entity_id = ue.from_entity_id
           AND er.to_entity_id = ue.to_entity_id
           AND er.relationship_type = ue.edge_type
        WHERE ue.cbu_id IS NOT NULL
        ON CONFLICT (cbu_id, relationship_id) DO NOTHING;

    END IF;
END $$;


-- ═══════════════════════════════════════════════════════════════════════════
-- VERIFICATION QUERIES (run these manually after migration)
-- ═══════════════════════════════════════════════════════════════════════════
--
-- SELECT
--     (SELECT COUNT(*) FROM "ob-poc".ownership_relationships) AS legacy_ownership,
--     (SELECT COUNT(*) FROM "ob-poc".control_relationships) AS legacy_control,
--     (SELECT COUNT(*) FROM "ob-poc".ubo_edges) AS legacy_ubo_edges,
--     (SELECT COUNT(*) FROM "ob-poc".entity_relationships) AS new_relationships,
--     (SELECT COUNT(*) FROM "ob-poc".cbu_relationship_verification) AS new_verifications;
--
-- -- Convergence view works
-- SELECT * FROM "ob-poc".cbu_convergence_status;
--
-- -- UBO candidates view works
-- SELECT * FROM "ob-poc".v_ubo_candidates WHERE is_ubo = true;
