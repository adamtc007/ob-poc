-- Migration 040: Primary Governance Controller Model
--
-- Purpose: Enable governance-controller-centric CBU grouping and shareholding traversal
--
-- Key Concepts:
--   1. Primary Governance Controller = Entity that controls a CBU via board appointment rights
--   2. CBU Group = Collection of CBUs under same governance controller (the "Allianz Lux Book")
--   3. Holding Control Link = Shareholding that confers control (≥ threshold)
--
-- Signal Priority (deterministic):
--   1. Board appointment rights via control share class (primary)
--   2. MANAGEMENT_COMPANY role assignment (fallback)
--   3. GLEIF IS_FUND_MANAGED_BY (fallback)
--
-- Design Principles:
--   - Control share class = who appoints the board (canonical definition)
--   - Class-level board rights flow to holders of that class
--   - Single deterministic winner per CBU (tie-break by seats, then %, then UUID)
--   - Groups are derivable from data, not manually maintained

BEGIN;

-- =============================================================================
-- 1. CBU GROUPS (Governance-controller-anchored collections)
-- =============================================================================
-- A "group" or "book" is a logical collection of CBUs sharing a governance controller.
-- This provides the "Allianz Lux Book" concept.

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_groups (
    group_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The entity that anchors this group (governance controller or ManCo)
    manco_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Group metadata
    group_name VARCHAR(255) NOT NULL,
    group_code VARCHAR(50),  -- Short code like "ALLIANZ_LUX"
    group_type VARCHAR(30) NOT NULL DEFAULT 'GOVERNANCE_BOOK',

    -- Jurisdiction scope (optional - a controller might have multiple books per jurisdiction)
    jurisdiction VARCHAR(10),

    -- Ultimate parent (for display - e.g., Allianz SE)
    ultimate_parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Description
    description TEXT,

    -- Auto-derived flag (true = computed from governance controller, false = manually created)
    is_auto_derived BOOLEAN DEFAULT true,

    -- Temporal
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    created_by VARCHAR(100),

    CONSTRAINT chk_group_type CHECK (group_type IN (
        'GOVERNANCE_BOOK',      -- Computed from board appointment / control signals
        'MANCO_BOOK',           -- Standard ManCo management group (fallback)
        'CORPORATE_GROUP',      -- Corporate entity group (non-fund)
        'INVESTMENT_MANAGER',   -- Grouped by IM rather than ManCo
        'UMBRELLA_SICAV',       -- Sub-funds of a SICAV umbrella
        'CUSTOM'                -- Manual grouping
    )),

    -- One active group per controller per jurisdiction
    UNIQUE NULLS NOT DISTINCT (manco_entity_id, jurisdiction, effective_to)
);

CREATE INDEX IF NOT EXISTS idx_cbu_groups_manco
    ON "ob-poc".cbu_groups(manco_entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_groups_active
    ON "ob-poc".cbu_groups(manco_entity_id) WHERE effective_to IS NULL;

COMMENT ON TABLE "ob-poc".cbu_groups IS
    'Governance-controller-anchored CBU groups ("books"). Enables querying all CBUs under a controller.';

-- =============================================================================
-- 2. CBU GROUP MEMBERSHIP (link CBU to group)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_group_members (
    membership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    group_id UUID NOT NULL REFERENCES "ob-poc".cbu_groups(group_id) ON DELETE CASCADE,
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- How was this membership determined?
    source VARCHAR(30) NOT NULL DEFAULT 'GOVERNANCE_CONTROLLER',

    -- Order within group (for display)
    display_order INTEGER DEFAULT 0,

    -- Temporal
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_membership_source CHECK (source IN (
        'GOVERNANCE_CONTROLLER', -- Computed from board appointment / control signals
        'MANCO_ROLE',            -- From cbu_entity_roles MANAGEMENT_COMPANY
        'GLEIF_MANAGED',         -- From gleif_relationships IS_FUND_MANAGED_BY
        'SHAREHOLDING',          -- From controlling shareholding
        'MANUAL'                 -- Manually assigned
    )),

    -- One active membership per CBU per group
    UNIQUE NULLS NOT DISTINCT (group_id, cbu_id, effective_to)
);

CREATE INDEX IF NOT EXISTS idx_cbu_group_members_group
    ON "ob-poc".cbu_group_members(group_id);
CREATE INDEX IF NOT EXISTS idx_cbu_group_members_cbu
    ON "ob-poc".cbu_group_members(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_group_members_active
    ON "ob-poc".cbu_group_members(cbu_id) WHERE effective_to IS NULL;

COMMENT ON TABLE "ob-poc".cbu_group_members IS
    'CBU membership in groups. One CBU can belong to one group at a time (active).';

-- =============================================================================
-- 3. HOLDING CONTROL LINKS (shareholdings that confer control)
-- =============================================================================
-- Materializes the "controlling interest" relationships derived from holdings.
-- Answers: "Which entities control which other entities via shareholding?"

CREATE TABLE IF NOT EXISTS kyc.holding_control_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The holder (controller)
    holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- The issuer (controlled entity)
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- The share class(es) that establish control (nullable = aggregated across classes)
    share_class_id UUID REFERENCES kyc.share_classes(id),

    -- Control metrics (aggregated if share_class_id is NULL)
    total_units NUMERIC(20,6),
    voting_pct NUMERIC(8,4),
    economic_pct NUMERIC(8,4),

    -- Control classification
    control_type VARCHAR(30) NOT NULL,

    -- Threshold used (for audit)
    threshold_pct NUMERIC(5,2),

    -- Is this a direct holding or computed through chain?
    is_direct BOOLEAN DEFAULT true,
    chain_depth INTEGER DEFAULT 1,  -- 1 = direct, 2+ = indirect

    -- Source holdings (for traceability)
    source_holding_ids UUID[],

    -- Temporal
    as_of_date DATE NOT NULL DEFAULT CURRENT_DATE,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT chk_control_type CHECK (control_type IN (
        'CONTROLLING',           -- ≥ 50% (or issuer-specific control threshold)
        'SIGNIFICANT_INFLUENCE', -- ≥ 25% (or issuer-specific significant threshold)
        'MATERIAL',              -- ≥ 10% (or issuer-specific material threshold)
        'NOTIFIABLE',            -- ≥ 5% (or issuer-specific disclosure threshold)
        'MINORITY'               -- < disclosure threshold but tracked
    ))
);

CREATE INDEX IF NOT EXISTS idx_control_links_holder
    ON kyc.holding_control_links(holder_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_links_issuer
    ON kyc.holding_control_links(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_links_controlling
    ON kyc.holding_control_links(holder_entity_id)
    WHERE control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE');
CREATE INDEX IF NOT EXISTS idx_control_links_date
    ON kyc.holding_control_links(as_of_date DESC);

COMMENT ON TABLE kyc.holding_control_links IS
    'Materialized control relationships derived from shareholdings. Enables efficient graph traversal.';

-- =============================================================================
-- 4. FUNCTION: Holder control position (FIXED - class-level board rights flow to holders)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_holder_control_position(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE,
    p_basis TEXT DEFAULT 'VOTES'
)
RETURNS TABLE (
    issuer_entity_id UUID,
    issuer_name TEXT,
    holder_entity_id UUID,
    holder_name TEXT,
    holder_type TEXT,
    holder_units NUMERIC,
    holder_votes NUMERIC,
    holder_economic NUMERIC,
    total_issuer_votes NUMERIC,
    total_issuer_economic NUMERIC,
    voting_pct NUMERIC,
    economic_pct NUMERIC,
    control_threshold_pct NUMERIC,
    significant_threshold_pct NUMERIC,
    has_control BOOLEAN,
    has_significant_influence BOOLEAN,
    has_board_rights BOOLEAN,
    board_seats INTEGER
) AS $$
BEGIN
    RETURN QUERY
    WITH issuer_supply AS (
        -- Aggregate supply across all share classes for issuer (as-of)
        SELECT
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.votes_per_unit, 1)) AS total_votes,
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.economic_per_unit, 1)) AS total_economic
        FROM kyc.share_classes sc
        LEFT JOIN LATERAL (
            SELECT scs2.*
            FROM kyc.share_class_supply scs2
            WHERE scs2.share_class_id = sc.id
              AND scs2.as_of_date <= p_as_of
            ORDER BY scs2.as_of_date DESC
            LIMIT 1
        ) scs ON true
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    holder_positions AS (
        -- Aggregate holdings per holder across all classes
        SELECT
            h.investor_entity_id,
            SUM(h.units) AS units,
            SUM(h.units * COALESCE(sc.votes_per_unit, 1)) AS votes,
            SUM(h.units * COALESCE(sc.economic_per_unit, 1)) AS economic
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND h.status = 'active'
        GROUP BY h.investor_entity_id
    ),

    -- -----------------------------
    -- Board rights: holder-attached
    -- -----------------------------
    holder_specific_rights AS (
        SELECT
            sr.holder_entity_id,
            COALESCE(SUM(COALESCE(sr.board_seats, 1)), 0) AS board_seats
        FROM kyc.special_rights sr
        WHERE sr.issuer_entity_id = p_issuer_entity_id
          AND sr.holder_entity_id IS NOT NULL
          AND sr.right_type = 'BOARD_APPOINTMENT'
          AND (sr.effective_to IS NULL OR sr.effective_to > p_as_of)
          AND (sr.effective_from IS NULL OR sr.effective_from <= p_as_of)
        GROUP BY sr.holder_entity_id
    ),

    -- --------------------------------------
    -- Board rights: class-attached allocation
    -- Deterministic v1 policy:
    --   - determine eligible holders of that class
    --   - allocate ALL seats to the single top eligible holder
    --     (highest pct_of_class; tie-breaker holder UUID)
    -- --------------------------------------
    class_rights AS (
        SELECT
            sr.right_id,
            sr.share_class_id,
            COALESCE(sr.board_seats, 1) AS board_seats,
            sr.threshold_pct,
            sr.threshold_basis
        FROM kyc.special_rights sr
        WHERE sr.issuer_entity_id = p_issuer_entity_id
          AND sr.share_class_id IS NOT NULL
          AND sr.right_type = 'BOARD_APPOINTMENT'
          AND (sr.effective_to IS NULL OR sr.effective_to > p_as_of)
          AND (sr.effective_from IS NULL OR sr.effective_from <= p_as_of)
    ),
    class_supply AS (
        SELECT
            sc.id AS share_class_id,
            COALESCE(scs.outstanding_units, scs.issued_units, 0) AS class_units
        FROM kyc.share_classes sc
        LEFT JOIN LATERAL (
            SELECT scs2.*
            FROM kyc.share_class_supply scs2
            WHERE scs2.share_class_id = sc.id
              AND scs2.as_of_date <= p_as_of
            ORDER BY scs2.as_of_date DESC
            LIMIT 1
        ) scs ON true
        WHERE sc.issuer_entity_id = p_issuer_entity_id
    ),
    holders_in_class AS (
        SELECT
            h.share_class_id,
            h.investor_entity_id AS holder_entity_id,
            SUM(h.units) AS holder_units_in_class,
            cs.class_units,
            CASE
                WHEN COALESCE(cs.class_units, 0) > 0
                THEN (SUM(h.units) / cs.class_units) * 100
                ELSE 0
            END AS pct_of_class
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        JOIN class_supply cs ON cs.share_class_id = h.share_class_id
        WHERE sc.issuer_entity_id = p_issuer_entity_id
          AND h.status = 'active'
        GROUP BY h.share_class_id, h.investor_entity_id, cs.class_units
    ),
    class_right_candidates AS (
        SELECT
            cr.right_id,
            cr.share_class_id,
            cr.board_seats,
            hic.holder_entity_id,
            hic.pct_of_class,
            CASE
                WHEN cr.threshold_pct IS NULL THEN true
                -- treat null/UNITS/CLASS_UNITS/VOTES the same at class scope (v1)
                WHEN COALESCE(cr.threshold_basis, 'UNITS') IN ('UNITS','CLASS_UNITS','VOTES')
                     AND hic.pct_of_class >= cr.threshold_pct THEN true
                ELSE false
            END AS is_eligible
        FROM class_rights cr
        JOIN holders_in_class hic ON hic.share_class_id = cr.share_class_id
    ),
    class_rights_allocated AS (
        SELECT
            x.holder_entity_id AS alloc_holder_entity_id,
            SUM(x.board_seats) AS board_seats
        FROM (
            SELECT
                crc.holder_entity_id,
                crc.board_seats,
                ROW_NUMBER() OVER (
                    PARTITION BY crc.right_id
                    ORDER BY crc.is_eligible DESC, crc.pct_of_class DESC, crc.holder_entity_id ASC
                ) AS rn
            FROM class_right_candidates crc
            WHERE crc.is_eligible = true
        ) x
        WHERE x.rn = 1
        GROUP BY x.holder_entity_id
    ),

    -- -----------------------------
    -- Unified holder_rights
    -- -----------------------------
    holder_rights AS (
        SELECT
            u.hr_holder_entity_id AS rights_holder_entity_id,
            SUM(u.board_seats) AS board_seats
        FROM (
            SELECT hsr.holder_entity_id AS hr_holder_entity_id, hsr.board_seats FROM holder_specific_rights hsr
            UNION ALL
            SELECT cra.alloc_holder_entity_id AS hr_holder_entity_id, cra.board_seats FROM class_rights_allocated cra
        ) u
        GROUP BY u.hr_holder_entity_id
    ),

    config AS (
        SELECT
            COALESCE(icc.control_threshold_pct, 50) AS control_threshold,
            COALESCE(icc.significant_threshold_pct, 25) AS significant_threshold
        FROM kyc.issuer_control_config icc
        WHERE icc.issuer_entity_id = p_issuer_entity_id
          AND (icc.effective_to IS NULL OR icc.effective_to > p_as_of)
          AND icc.effective_from <= p_as_of
        ORDER BY icc.effective_from DESC
        LIMIT 1
    ),
    -- Combine holders from positions (have holdings) and rights (may have board rights without holdings)
    all_holders AS (
        SELECT investor_entity_id AS holder_entity_id, units, votes, economic
        FROM holder_positions
        UNION
        SELECT rights_holder_entity_id AS holder_entity_id, 0::NUMERIC, 0::NUMERIC, 0::NUMERIC
        FROM holder_rights
        WHERE rights_holder_entity_id NOT IN (SELECT investor_entity_id FROM holder_positions)
    )
    SELECT
        p_issuer_entity_id,
        ie.name::TEXT,
        ah.holder_entity_id,
        he.name::TEXT,
        het.type_code::TEXT,
        ah.units,
        ah.votes,
        ah.economic,
        isu.total_votes,
        isu.total_economic,
        CASE WHEN isu.total_votes > 0 THEN ROUND((ah.votes / isu.total_votes) * 100, 4) ELSE 0 END,
        CASE WHEN isu.total_economic > 0 THEN ROUND((ah.economic / isu.total_economic) * 100, 4) ELSE 0 END,
        COALESCE(cfg.control_threshold, 50),
        COALESCE(cfg.significant_threshold, 25),
        CASE WHEN isu.total_votes > 0 AND (ah.votes / isu.total_votes) * 100 > COALESCE(cfg.control_threshold, 50) THEN true ELSE false END,
        CASE WHEN isu.total_votes > 0 AND (ah.votes / isu.total_votes) * 100 > COALESCE(cfg.significant_threshold, 25) THEN true ELSE false END,
        COALESCE(hr.board_seats, 0) > 0,
        COALESCE(hr.board_seats, 0)::INTEGER
    FROM all_holders ah
    CROSS JOIN issuer_supply isu
    LEFT JOIN config cfg ON true
    LEFT JOIN holder_rights hr ON hr.rights_holder_entity_id = ah.holder_entity_id
    JOIN "ob-poc".entities ie ON ie.entity_id = p_issuer_entity_id
    JOIN "ob-poc".entities he ON he.entity_id = ah.holder_entity_id
    LEFT JOIN "ob-poc".entity_types het ON he.entity_type_id = het.entity_type_id
    ORDER BY ah.votes DESC;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_holder_control_position IS
    'Compute holder control positions including class-level board appointment rights flowing to holders.';

-- =============================================================================
-- 5. FUNCTION: Primary governance controller (single deterministic winner per issuer)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_primary_governance_controller(
    p_issuer_entity_id UUID,
    p_as_of DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    issuer_entity_id UUID,
    primary_controller_entity_id UUID,
    governance_controller_entity_id UUID,
    basis TEXT,
    board_seats INTEGER,
    voting_pct NUMERIC,
    economic_pct NUMERIC,
    has_control BOOLEAN,
    has_significant_influence BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    WITH ranked AS (
        SELECT
            hcp.*,
            ROW_NUMBER() OVER (
                ORDER BY
                    -- 1) board appointment rights first
                    hcp.has_board_rights DESC,
                    hcp.board_seats DESC,
                    -- 2) then voting control flags
                    hcp.has_control DESC,
                    hcp.has_significant_influence DESC,
                    -- 3) then raw % (stable)
                    hcp.voting_pct DESC,
                    hcp.economic_pct DESC,
                    hcp.holder_entity_id ASC
            ) AS rn
        FROM kyc.fn_holder_control_position(p_issuer_entity_id, p_as_of, 'VOTES') hcp
        WHERE (hcp.has_board_rights = true)
           OR (hcp.has_control = true)
           OR (hcp.has_significant_influence = true)
    ),
    winner AS (
        SELECT *
        FROM ranked
        WHERE rn = 1
    ),
    role_profile AS (
        SELECT rp.group_container_entity_id
        FROM kyc.investor_role_profiles rp
        JOIN winner w ON w.holder_entity_id = rp.holder_entity_id
        WHERE rp.issuer_entity_id = p_issuer_entity_id
          AND rp.effective_from <= p_as_of
          AND (rp.effective_to IS NULL OR rp.effective_to > p_as_of)
        ORDER BY rp.effective_from DESC
        LIMIT 1
    )
    SELECT
        p_issuer_entity_id,
        w.holder_entity_id AS primary_controller_entity_id,
        COALESCE(rp.group_container_entity_id, w.holder_entity_id) AS governance_controller_entity_id,
        CASE
            WHEN w.has_board_rights THEN 'BOARD_APPOINTMENT'
            WHEN w.has_control THEN 'VOTING_CONTROL'
            WHEN w.has_significant_influence THEN 'SIGNIFICANT_INFLUENCE'
            ELSE 'NONE'
        END AS basis,
        w.board_seats,
        w.voting_pct,
        w.economic_pct,
        w.has_control,
        w.has_significant_influence
    FROM winner w
    LEFT JOIN role_profile rp ON true;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION kyc.fn_primary_governance_controller IS
    'Return single deterministic primary governance controller for an issuer based on board rights > voting control > significant influence.';

-- =============================================================================
-- 6. FUNCTION: Compute holding control links (FIXED - correct denominator calculation)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_compute_control_links(
    p_issuer_entity_id UUID DEFAULT NULL,
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER := 0;
BEGIN
    DELETE FROM kyc.holding_control_links
    WHERE as_of_date = p_as_of_date
      AND (p_issuer_entity_id IS NULL OR issuer_entity_id = p_issuer_entity_id);

    INSERT INTO kyc.holding_control_links (
        holder_entity_id,
        issuer_entity_id,
        share_class_id,
        total_units,
        voting_pct,
        economic_pct,
        control_type,
        threshold_pct,
        is_direct,
        chain_depth,
        source_holding_ids,
        as_of_date
    )
    WITH issuer_thresholds AS (
        SELECT
            icc.issuer_entity_id,
            COALESCE(icc.control_threshold_pct, 50.00) AS control_threshold,
            COALESCE(icc.significant_threshold_pct, 25.00) AS significant_threshold,
            COALESCE(icc.material_threshold_pct, 10.00) AS material_threshold,
            COALESCE(icc.disclosure_threshold_pct, 5.00) AS disclosure_threshold
        FROM kyc.issuer_control_config icc
        WHERE (icc.effective_to IS NULL OR icc.effective_to > p_as_of_date)
          AND icc.effective_from <= p_as_of_date
    ),
    issuer_denoms AS (
        SELECT
            sc.issuer_entity_id,
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.votes_per_unit, 1)) AS total_votes,
            SUM(COALESCE(scs.issued_units, 0)
                * COALESCE(sc.economic_per_unit, 1)) AS total_economic
        FROM kyc.share_classes sc
        LEFT JOIN LATERAL (
            SELECT scs2.*
            FROM kyc.share_class_supply scs2
            WHERE scs2.share_class_id = sc.id
              AND scs2.as_of_date <= p_as_of_date
            ORDER BY scs2.as_of_date DESC
            LIMIT 1
        ) scs ON true
        WHERE (p_issuer_entity_id IS NULL OR sc.issuer_entity_id = p_issuer_entity_id)
        GROUP BY sc.issuer_entity_id
    ),
    holder_totals AS (
        SELECT
            h.investor_entity_id AS holder_entity_id,
            sc.issuer_entity_id,
            h.share_class_id,
            SUM(h.units) AS total_units,
            SUM(h.units * COALESCE(sc.votes_per_unit, 1)) AS holder_votes,
            SUM(h.units * COALESCE(sc.economic_per_unit, 1)) AS holder_economic,
            ARRAY_AGG(h.id) AS source_holding_ids
        FROM kyc.holdings h
        JOIN kyc.share_classes sc ON sc.id = h.share_class_id
        WHERE h.status = 'active'
          AND (p_issuer_entity_id IS NULL OR sc.issuer_entity_id = p_issuer_entity_id)
        GROUP BY h.investor_entity_id, sc.issuer_entity_id, h.share_class_id
    )
    SELECT
        ht.holder_entity_id,
        ht.issuer_entity_id,
        ht.share_class_id,
        ht.total_units,
        CASE WHEN COALESCE(id.total_votes, 0) > 0
             THEN (ht.holder_votes / id.total_votes) * 100 ELSE 0 END AS voting_pct,
        CASE WHEN COALESCE(id.total_economic, 0) > 0
             THEN (ht.holder_economic / id.total_economic) * 100 ELSE 0 END AS economic_pct,
        CASE
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.control_threshold, 50) THEN 'CONTROLLING'
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.significant_threshold, 25) THEN 'SIGNIFICANT_INFLUENCE'
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.material_threshold, 10) THEN 'MATERIAL'
            WHEN COALESCE(
                CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
                CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
                0
            ) >= COALESCE(it.disclosure_threshold, 5) THEN 'NOTIFIABLE'
            ELSE 'MINORITY'
        END AS control_type,
        COALESCE(it.control_threshold, 50.00),
        true,
        1,
        ht.source_holding_ids,
        p_as_of_date
    FROM holder_totals ht
    JOIN issuer_denoms id ON id.issuer_entity_id = ht.issuer_entity_id
    LEFT JOIN issuer_thresholds it ON it.issuer_entity_id = ht.issuer_entity_id
    WHERE COALESCE(
        CASE WHEN COALESCE(id.total_votes, 0) > 0 THEN (ht.holder_votes / id.total_votes) * 100 END,
        CASE WHEN COALESCE(id.total_economic, 0) > 0 THEN (ht.holder_economic / id.total_economic) * 100 END,
        0
    ) >= COALESCE(it.disclosure_threshold, 5);

    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_compute_control_links IS
    'Compute control links from holdings with correct as-of denominator calculation.';

-- =============================================================================
-- 7. VIEW: ManCo Group Summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_manco_group_summary AS
SELECT
    g.group_id,
    g.group_name,
    g.group_code,
    g.group_type,
    g.manco_entity_id,
    me.name AS manco_name,
    g.jurisdiction,
    g.ultimate_parent_entity_id,
    upe.name AS ultimate_parent_name,
    COUNT(DISTINCT gm.cbu_id) AS cbu_count,
    ARRAY_AGG(DISTINCT c.name ORDER BY c.name) AS cbu_names,
    g.effective_from,
    g.is_auto_derived
FROM "ob-poc".cbu_groups g
JOIN "ob-poc".entities me ON me.entity_id = g.manco_entity_id
LEFT JOIN "ob-poc".entities upe ON upe.entity_id = g.ultimate_parent_entity_id
LEFT JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id
    AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE)
LEFT JOIN "ob-poc".cbus c ON c.cbu_id = gm.cbu_id
WHERE g.effective_to IS NULL
GROUP BY g.group_id, g.group_name, g.group_code, g.group_type, g.manco_entity_id, me.name,
         g.jurisdiction, g.ultimate_parent_entity_id, upe.name,
         g.effective_from, g.is_auto_derived;

COMMENT ON VIEW "ob-poc".v_manco_group_summary IS
    'Summary of governance controller groups with CBU counts and names.';

-- =============================================================================
-- 8. VIEW: CBUs by controller with control chain
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cbus_by_manco AS
SELECT
    g.manco_entity_id,
    me.name AS manco_name,
    c.cbu_id,
    c.name AS cbu_name,
    c.cbu_category,
    c.jurisdiction,
    gm.source AS membership_source,
    -- Get controlling shareholder of the CBU's fund entity (if any)
    hcl.holder_entity_id AS controlling_holder_id,
    che.name AS controlling_holder_name,
    hcl.voting_pct AS controlling_voting_pct,
    hcl.control_type
FROM "ob-poc".cbu_groups g
JOIN "ob-poc".entities me ON me.entity_id = g.manco_entity_id
JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id
    AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE)
JOIN "ob-poc".cbus c ON c.cbu_id = gm.cbu_id
-- Get the fund entity for the CBU (commercial_client_entity_id)
LEFT JOIN kyc.holding_control_links hcl ON hcl.issuer_entity_id = c.commercial_client_entity_id
    AND hcl.control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')
    AND hcl.as_of_date = (SELECT MAX(as_of_date) FROM kyc.holding_control_links)
LEFT JOIN "ob-poc".entities che ON che.entity_id = hcl.holder_entity_id
WHERE g.effective_to IS NULL
ORDER BY g.manco_entity_id, c.name;

COMMENT ON VIEW "ob-poc".v_cbus_by_manco IS
    'All CBUs grouped by governance controller with controlling shareholder information.';

-- =============================================================================
-- 9. FUNCTION: Get all CBUs in a controller group
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_get_manco_group_cbus(
    p_manco_entity_id UUID
)
RETURNS TABLE (
    cbu_id UUID,
    cbu_name TEXT,
    cbu_category TEXT,
    jurisdiction VARCHAR(10),
    fund_entity_id UUID,
    fund_entity_name TEXT,
    membership_source VARCHAR(30)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        c.cbu_id,
        c.name::TEXT,
        c.cbu_category::TEXT,
        c.jurisdiction,
        c.commercial_client_entity_id,
        fe.name::TEXT,
        gm.source
    FROM "ob-poc".cbu_groups g
    JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id
        AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE)
    JOIN "ob-poc".cbus c ON c.cbu_id = gm.cbu_id
    LEFT JOIN "ob-poc".entities fe ON fe.entity_id = c.commercial_client_entity_id
    WHERE g.manco_entity_id = p_manco_entity_id
      AND (g.effective_to IS NULL OR g.effective_to > CURRENT_DATE)
    ORDER BY c.name;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".fn_get_manco_group_cbus IS
    'Get all CBUs managed by a specific governance controller.';

-- =============================================================================
-- 10. FUNCTION: Find controller for a CBU
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_get_cbu_manco(
    p_cbu_id UUID
)
RETURNS TABLE (
    manco_entity_id UUID,
    manco_name TEXT,
    manco_lei VARCHAR(20),
    group_id UUID,
    group_name TEXT,
    group_type TEXT,
    source VARCHAR(30)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        g.manco_entity_id,
        me.name::TEXT,
        em.lei,
        g.group_id,
        g.group_name::TEXT,
        g.group_type::TEXT,
        gm.source
    FROM "ob-poc".cbu_group_members gm
    JOIN "ob-poc".cbu_groups g ON g.group_id = gm.group_id
        AND (g.effective_to IS NULL OR g.effective_to > CURRENT_DATE)
    JOIN "ob-poc".entities me ON me.entity_id = g.manco_entity_id
    LEFT JOIN "ob-poc".entity_manco em ON em.entity_id = g.manco_entity_id
    WHERE gm.cbu_id = p_cbu_id
      AND (gm.effective_to IS NULL OR gm.effective_to > CURRENT_DATE);
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".fn_get_cbu_manco IS
    'Get the governance controller for a specific CBU.';

-- =============================================================================
-- 11. FUNCTION: Get control chain for a controller group
-- =============================================================================
-- Trace the shareholding control chain upward to find ultimate controller.

CREATE OR REPLACE FUNCTION "ob-poc".fn_manco_group_control_chain(
    p_manco_entity_id UUID,
    p_max_depth INTEGER DEFAULT 5
)
RETURNS TABLE (
    depth INTEGER,
    entity_id UUID,
    entity_name TEXT,
    entity_type TEXT,
    controlled_by_entity_id UUID,
    controlled_by_name TEXT,
    control_type VARCHAR(30),
    voting_pct NUMERIC(8,4),
    is_ultimate_controller BOOLEAN
) AS $$
WITH RECURSIVE control_chain AS (
    -- Base: Start with the controller itself
    SELECT
        1 AS depth,
        e.entity_id,
        e.name::TEXT AS entity_name,
        et.type_code::TEXT AS entity_type,
        NULL::UUID AS controlled_by_entity_id,
        NULL::TEXT AS controlled_by_name,
        NULL::VARCHAR(30) AS control_type,
        NULL::NUMERIC(8,4) AS voting_pct
    FROM "ob-poc".entities e
    LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
    WHERE e.entity_id = p_manco_entity_id

    UNION ALL

    -- Recursive: Find who controls each entity via shareholding
    SELECT
        cc.depth + 1,
        hcl.holder_entity_id,
        he.name::TEXT,
        het.type_code::TEXT,
        cc.entity_id,
        cc.entity_name,
        hcl.control_type,
        hcl.voting_pct
    FROM control_chain cc
    JOIN kyc.holding_control_links hcl ON hcl.issuer_entity_id = cc.entity_id
        AND hcl.control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')
        AND hcl.as_of_date = (SELECT MAX(as_of_date) FROM kyc.holding_control_links)
    JOIN "ob-poc".entities he ON he.entity_id = hcl.holder_entity_id
    LEFT JOIN "ob-poc".entity_types het ON het.entity_type_id = he.entity_type_id
    WHERE cc.depth < p_max_depth
)
SELECT
    cc.depth,
    cc.entity_id,
    cc.entity_name,
    cc.entity_type,
    cc.controlled_by_entity_id,
    cc.controlled_by_name,
    cc.control_type,
    cc.voting_pct,
    -- Is ultimate controller if no one controls this entity
    NOT EXISTS (
        SELECT 1 FROM kyc.holding_control_links hcl2
        WHERE hcl2.issuer_entity_id = cc.entity_id
          AND hcl2.control_type IN ('CONTROLLING', 'SIGNIFICANT_INFLUENCE')
    ) AS is_ultimate_controller
FROM control_chain cc
ORDER BY cc.depth;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION "ob-poc".fn_manco_group_control_chain IS
    'Trace the shareholding control chain upward from a controller to find ultimate parent.';

-- =============================================================================
-- 12. FUNCTION: Derive CBU groups from governance controller (with MANCO_ROLE fallback)
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_derive_cbu_groups(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    groups_created INTEGER,
    memberships_created INTEGER
) AS $$
DECLARE
    v_groups_created INTEGER := 0;
    v_memberships_created INTEGER := 0;
BEGIN
    -- Create temp table for chosen anchors (avoids repeating CTE)
    CREATE TEMP TABLE IF NOT EXISTS _chosen_anchors ON COMMIT DROP AS
    WITH cbu_issuer AS (
        -- Get issuer entity from SICAV role (the fund entity), or share_classes, or commercial_client
        SELECT
            c.cbu_id,
            c.jurisdiction,
            COALESCE(
                -- Priority 1: SICAV role = the fund entity that is the issuer
                (SELECT cer.entity_id FROM "ob-poc".cbu_entity_roles cer
                 JOIN "ob-poc".roles r ON r.role_id = cer.role_id AND r.name = 'SICAV'
                 WHERE cer.cbu_id = c.cbu_id
                   AND (cer.effective_to IS NULL OR cer.effective_to > p_as_of_date)
                 ORDER BY cer.effective_from DESC LIMIT 1),
                -- Priority 2: share_class issuer_entity_id
                (SELECT sc.issuer_entity_id FROM kyc.share_classes sc
                 WHERE sc.cbu_id = c.cbu_id AND sc.issuer_entity_id IS NOT NULL
                 ORDER BY sc.issuer_entity_id LIMIT 1),
                -- Priority 3: commercial_client_entity_id (fallback)
                c.commercial_client_entity_id
            ) AS issuer_entity_id
        FROM "ob-poc".cbus c
    ),
    computed_controller AS (
        SELECT
            ci.cbu_id,
            ci.jurisdiction,
            pc.governance_controller_entity_id AS anchor_entity_id,
            'GOVERNANCE_CONTROLLER'::varchar AS source,
            1 AS precedence
        FROM cbu_issuer ci
        JOIN LATERAL kyc.fn_primary_governance_controller(ci.issuer_entity_id, p_as_of_date) pc ON true
        WHERE pc.governance_controller_entity_id IS NOT NULL
          AND pc.basis <> 'NONE'
    ),
    manco_role AS (
        SELECT
            cer.cbu_id,
            c.jurisdiction,
            cer.entity_id AS anchor_entity_id,
            'MANCO_ROLE'::varchar AS source,
            2 AS precedence
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
        WHERE r.name = 'MANAGEMENT_COMPANY'
          AND (cer.effective_to IS NULL OR cer.effective_to > p_as_of_date)
    ),
    candidates AS (
        SELECT * FROM computed_controller
        UNION ALL
        SELECT * FROM manco_role
    )
    SELECT DISTINCT ON (cbu_id)
        cbu_id,
        jurisdiction,
        anchor_entity_id,
        source
    FROM candidates
    ORDER BY cbu_id, precedence ASC, anchor_entity_id ASC;

    -- Create groups (set-based)
    -- Use DISTINCT ON to ensure one group_type per anchor+jurisdiction (prefer GOVERNANCE_CONTROLLER)
    WITH desired_groups AS (
        SELECT DISTINCT ON (ch.anchor_entity_id, ch.jurisdiction)
            ch.anchor_entity_id,
            ch.jurisdiction,
            CASE WHEN ch.source = 'GOVERNANCE_CONTROLLER' THEN 'GOVERNANCE_BOOK' ELSE 'MANCO_BOOK' END AS group_type
        FROM _chosen_anchors ch
        ORDER BY ch.anchor_entity_id, ch.jurisdiction,
                 CASE WHEN ch.source = 'GOVERNANCE_CONTROLLER' THEN 1 ELSE 2 END
    )
    INSERT INTO "ob-poc".cbu_groups (
        manco_entity_id,
        group_name,
        group_code,
        group_type,
        jurisdiction,
        is_auto_derived,
        effective_from
    )
    SELECT
        dg.anchor_entity_id,
        e.name || ' Book',
        UPPER(REPLACE(SUBSTRING(e.name FROM 1 FOR 20), ' ', '_')) || COALESCE('_' || dg.jurisdiction, ''),
        dg.group_type,
        dg.jurisdiction,
        true,
        p_as_of_date
    FROM desired_groups dg
    JOIN "ob-poc".entities e ON e.entity_id = dg.anchor_entity_id
    ON CONFLICT (manco_entity_id, jurisdiction, effective_to)
    DO UPDATE SET updated_at = now();

    GET DIAGNOSTICS v_groups_created = ROW_COUNT;

    -- Close prior active memberships for these CBUs if they point elsewhere
    WITH chosen_groups AS (
        SELECT
            ch.cbu_id,
            ch.source,
            g.group_id
        FROM _chosen_anchors ch
        JOIN "ob-poc".cbu_groups g
          ON g.manco_entity_id = ch.anchor_entity_id
         AND (g.jurisdiction = ch.jurisdiction OR (g.jurisdiction IS NULL AND ch.jurisdiction IS NULL))
         AND g.effective_to IS NULL
    )
    UPDATE "ob-poc".cbu_group_members gm
    SET effective_to = p_as_of_date
    FROM chosen_groups cg
    WHERE gm.cbu_id = cg.cbu_id
      AND gm.effective_to IS NULL
      AND gm.group_id <> cg.group_id;

    -- Insert/refresh active membership (update source if changed)
    INSERT INTO "ob-poc".cbu_group_members (group_id, cbu_id, source, effective_from)
    SELECT
        g.group_id,
        ch.cbu_id,
        ch.source,
        p_as_of_date
    FROM _chosen_anchors ch
    JOIN "ob-poc".cbu_groups g
      ON g.manco_entity_id = ch.anchor_entity_id
     AND (g.jurisdiction = ch.jurisdiction OR (g.jurisdiction IS NULL AND ch.jurisdiction IS NULL))
     AND g.effective_to IS NULL
    ON CONFLICT (group_id, cbu_id, effective_to)
    DO UPDATE SET source = EXCLUDED.source
    WHERE cbu_group_members.source <> EXCLUDED.source;

    GET DIAGNOSTICS v_memberships_created = ROW_COUNT;

    DROP TABLE IF EXISTS _chosen_anchors;

    RETURN QUERY SELECT v_groups_created, v_memberships_created;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".fn_derive_cbu_groups IS
    'Auto-derive CBU groups using governance controller (board appointment/control) with fallback to MANAGEMENT_COMPANY role.';

COMMIT;

-- =============================================================================
-- USAGE EXAMPLES
-- =============================================================================
/*
-- 1. Derive CBU groups from governance controllers (with MANCO_ROLE fallback)
SELECT * FROM "ob-poc".fn_derive_cbu_groups();

-- 2. Compute control links from holdings
SELECT kyc.fn_compute_control_links(NULL, CURRENT_DATE);

-- 3. View all governance controller groups with CBU counts
SELECT * FROM "ob-poc".v_manco_group_summary;

-- 4. Get all CBUs for a specific controller
SELECT * FROM "ob-poc".fn_get_manco_group_cbus('controller-entity-uuid-here');

-- 5. Find which controller manages a CBU
SELECT * FROM "ob-poc".fn_get_cbu_manco('cbu-uuid-here');

-- 6. Get control chain for a controller group (who controls the controller?)
SELECT * FROM "ob-poc".fn_manco_group_control_chain('controller-entity-uuid-here');

-- 7. Get primary governance controller for an issuer
SELECT * FROM kyc.fn_primary_governance_controller('issuer-entity-uuid-here');

-- 8. View all CBUs by controller with controlling shareholders
SELECT * FROM "ob-poc".v_cbus_by_manco
WHERE manco_name ILIKE '%Allianz%'
ORDER BY manco_name, cbu_name;
*/
