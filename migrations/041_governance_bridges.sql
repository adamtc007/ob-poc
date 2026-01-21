-- Migration 041: Governance Controller Bridge Functions
--
-- Purpose: Bridge existing data sources into the governance controller model
--
-- Bridges:
--   1. MANCO_ROLE → special_rights BOARD_APPOINTMENT (immediate - we have 525 roles)
--   2. GLEIF IS_FUND_MANAGED_BY → special_rights BOARD_APPOINTMENT (when relationships imported)
--   3. BODS ownership → kyc.holdings (when BODS data imported)
--
-- All bridges are idempotent (safe to re-run)

BEGIN;

-- =============================================================================
-- 1. BRIDGE: MANAGEMENT_COMPANY role → BOARD_APPOINTMENT special rights
-- =============================================================================
-- Rationale: If an entity is assigned MANAGEMENT_COMPANY role on a CBU, they
-- effectively control the fund's governance. This creates a synthetic board
-- appointment right so the governance controller logic fires.

CREATE OR REPLACE FUNCTION kyc.fn_bridge_manco_role_to_board_rights(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    rights_created INTEGER,
    rights_updated INTEGER
) AS $$
DECLARE
    v_created INTEGER := 0;
    v_updated INTEGER := 0;
BEGIN
    -- Insert BOARD_APPOINTMENT rights for MANAGEMENT_COMPANY roles
    -- Issuer = entity with SICAV role (the fund)
    -- Holder = entity with MANAGEMENT_COMPANY role (the ManCo)
    -- Use DISTINCT ON to deduplicate when same manco manages multiple CBUs for same fund
    WITH manco_fund_pairs AS (
        SELECT DISTINCT ON (sicav_cer.entity_id, manco_cer.entity_id)
            manco_cer.cbu_id,
            manco_cer.entity_id AS manco_entity_id,
            sicav_cer.entity_id AS fund_entity_id,
            manco_cer.effective_from,
            manco_cer.effective_to
        FROM "ob-poc".cbu_entity_roles manco_cer
        JOIN "ob-poc".roles manco_r ON manco_r.role_id = manco_cer.role_id AND manco_r.name = 'MANAGEMENT_COMPANY'
        -- Join to SICAV role on same CBU to get the fund entity
        JOIN "ob-poc".cbu_entity_roles sicav_cer ON sicav_cer.cbu_id = manco_cer.cbu_id
        JOIN "ob-poc".roles sicav_r ON sicav_r.role_id = sicav_cer.role_id AND sicav_r.name = 'SICAV'
        WHERE (manco_cer.effective_to IS NULL OR manco_cer.effective_to > p_as_of_date)
          -- Ensure fund and manco are different entities
          AND sicav_cer.entity_id <> manco_cer.entity_id
        ORDER BY sicav_cer.entity_id, manco_cer.entity_id, manco_cer.effective_from
    )
    INSERT INTO kyc.special_rights (
        issuer_entity_id,
        holder_entity_id,
        right_type,
        board_seats,
        effective_from,
        effective_to,
        notes
    )
    SELECT
        mfp.fund_entity_id,
        mfp.manco_entity_id,
        'BOARD_APPOINTMENT',
        1,  -- Default 1 seat for ManCo-derived rights
        COALESCE(mfp.effective_from, p_as_of_date),
        mfp.effective_to,
        'Auto-generated from MANAGEMENT_COMPANY role assignment'
    FROM manco_fund_pairs mfp
    ON CONFLICT (issuer_entity_id, holder_entity_id, right_type, share_class_id)
    WHERE share_class_id IS NULL
    DO UPDATE SET
        effective_to = EXCLUDED.effective_to
    WHERE kyc.special_rights.effective_to IS DISTINCT FROM EXCLUDED.effective_to;

    GET DIAGNOSTICS v_created = ROW_COUNT;

    RETURN QUERY SELECT v_created, v_updated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_bridge_manco_role_to_board_rights IS
    'Bridge MANAGEMENT_COMPANY role assignments to BOARD_APPOINTMENT special rights for governance controller.';

-- =============================================================================
-- 2. BRIDGE: GLEIF fund manager relationships → BOARD_APPOINTMENT special rights
-- =============================================================================
-- When GLEIF IS_FUND_MANAGED_BY relationships are imported, bridge them to
-- board appointment rights.

CREATE OR REPLACE FUNCTION kyc.fn_bridge_gleif_fund_manager_to_board_rights(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    rights_created INTEGER,
    rights_updated INTEGER
) AS $$
DECLARE
    v_created INTEGER := 0;
    v_updated INTEGER := 0;
BEGIN
    -- Bridge from entity_parent_relationships where relationship_type indicates fund management
    WITH gleif_fund_managers AS (
        SELECT
            epr.child_entity_id AS fund_entity_id,  -- The fund
            epr.parent_entity_id AS manager_entity_id,  -- The manager
            epr.created_at::date AS effective_from
        FROM "ob-poc".entity_parent_relationships epr
        WHERE epr.relationship_type IN ('IS_FUND_MANAGED_BY', 'IS_FUND-MANAGED_BY', 'FUND_MANAGER')
          AND epr.parent_entity_id IS NOT NULL
          AND epr.source = 'GLEIF'
    )
    INSERT INTO kyc.special_rights (
        issuer_entity_id,
        holder_entity_id,
        right_type,
        board_seats,
        effective_from,
        notes
    )
    SELECT
        gfm.fund_entity_id,
        gfm.manager_entity_id,
        'BOARD_APPOINTMENT',
        1,
        COALESCE(gfm.effective_from, p_as_of_date),
        'Auto-generated from GLEIF IS_FUND_MANAGED_BY relationship'
    FROM gleif_fund_managers gfm
    WHERE gfm.fund_entity_id IS NOT NULL
      AND gfm.manager_entity_id IS NOT NULL
    ON CONFLICT (issuer_entity_id, holder_entity_id, right_type, share_class_id)
    WHERE share_class_id IS NULL
    DO NOTHING;

    GET DIAGNOSTICS v_created = ROW_COUNT;

    RETURN QUERY SELECT v_created, v_updated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_bridge_gleif_fund_manager_to_board_rights IS
    'Bridge GLEIF IS_FUND_MANAGED_BY relationships to BOARD_APPOINTMENT special rights.';

-- =============================================================================
-- 3. BRIDGE: BODS ownership statements → kyc.holdings
-- =============================================================================
-- When BODS data is imported, convert ownership statements into holdings.
-- This enables the governance controller to compute control from actual ownership %.

CREATE OR REPLACE FUNCTION kyc.fn_bridge_bods_to_holdings(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    holdings_created INTEGER,
    holdings_updated INTEGER,
    entities_linked INTEGER
) AS $$
DECLARE
    v_created INTEGER := 0;
    v_updated INTEGER := 0;
    v_linked INTEGER := 0;
BEGIN
    -- First, link BODS entity statements to our entities via LEI
    INSERT INTO "ob-poc".entity_bods_links (entity_id, bods_entity_statement_id, match_method, match_confidence)
    SELECT DISTINCT
        elc.entity_id,
        bes.statement_id,
        'LEI',
        1.0
    FROM "ob-poc".bods_entity_statements bes
    JOIN "ob-poc".entity_limited_companies elc ON elc.lei = bes.lei
    WHERE bes.lei IS NOT NULL
    ON CONFLICT (entity_id, bods_entity_statement_id) DO NOTHING;

    GET DIAGNOSTICS v_linked = ROW_COUNT;

    -- Now bridge BODS ownership statements to holdings
    -- This requires:
    --   1. Subject entity linked to our entities
    --   2. Interested party entity linked to our entities
    --   3. A share class exists for the subject entity
    WITH bods_ownership AS (
        SELECT
            bos.statement_id,
            subj_link.entity_id AS issuer_entity_id,
            party_link.entity_id AS holder_entity_id,
            COALESCE(bos.share_exact, (bos.share_min + bos.share_max) / 2) AS ownership_pct,
            bos.share_min,
            bos.share_max,
            bos.is_direct,
            bos.start_date,
            bos.end_date
        FROM "ob-poc".bods_ownership_statements bos
        -- Link subject to our entity
        JOIN "ob-poc".entity_bods_links subj_link
            ON subj_link.bods_entity_statement_id = bos.subject_entity_statement_id
        -- Link interested party to our entity
        JOIN "ob-poc".entity_bods_links party_link
            ON party_link.bods_entity_statement_id = bos.interested_party_statement_id
        WHERE bos.ownership_type IN ('shareholding', 'voting-rights', 'ownership-of-shares')
          AND (bos.end_date IS NULL OR bos.end_date > p_as_of_date)
    ),
    -- Find or create default share class for each issuer
    issuer_share_classes AS (
        SELECT DISTINCT ON (bo.issuer_entity_id)
            bo.issuer_entity_id,
            sc.id AS share_class_id,
            COALESCE(scs.outstanding_units, scs.issued_units, 1000000) AS total_units
        FROM bods_ownership bo
        JOIN kyc.share_classes sc ON sc.issuer_entity_id = bo.issuer_entity_id
        LEFT JOIN LATERAL (
            SELECT * FROM kyc.share_class_supply scs
            WHERE scs.share_class_id = sc.id
            ORDER BY scs.as_of_date DESC
            LIMIT 1
        ) scs ON true
        ORDER BY bo.issuer_entity_id, sc.created_at
    )
    INSERT INTO kyc.holdings (
        share_class_id,
        investor_entity_id,
        units,
        cost_basis,
        acquisition_date,
        status,
        source,
        notes
    )
    SELECT
        isc.share_class_id,
        bo.holder_entity_id,
        -- Convert ownership % to units based on total supply
        (bo.ownership_pct / 100.0) * isc.total_units,
        0,  -- No cost basis from BODS
        COALESCE(bo.start_date, p_as_of_date),
        'active',
        'BODS_BRIDGE',
        'Auto-generated from BODS ownership statement ' || bo.statement_id
    FROM bods_ownership bo
    JOIN issuer_share_classes isc ON isc.issuer_entity_id = bo.issuer_entity_id
    ON CONFLICT (share_class_id, investor_entity_id)
    DO UPDATE SET
        units = EXCLUDED.units
    WHERE kyc.holdings.units <> EXCLUDED.units;

    GET DIAGNOSTICS v_created = ROW_COUNT;

    RETURN QUERY SELECT v_created, v_updated, v_linked;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_bridge_bods_to_holdings IS
    'Bridge BODS ownership statements to kyc.holdings for governance controller computation.';

-- =============================================================================
-- 4. MASTER BRIDGE: Run all bridges in sequence
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_run_governance_bridges(
    p_as_of_date DATE DEFAULT CURRENT_DATE
)
RETURNS TABLE (
    bridge_name TEXT,
    records_affected INTEGER
) AS $$
DECLARE
    v_manco_created INTEGER;
    v_manco_updated INTEGER;
    v_gleif_created INTEGER;
    v_gleif_updated INTEGER;
    v_bods_created INTEGER;
    v_bods_updated INTEGER;
    v_bods_linked INTEGER;
BEGIN
    -- Run ManCo role bridge
    SELECT * INTO v_manco_created, v_manco_updated
    FROM kyc.fn_bridge_manco_role_to_board_rights(p_as_of_date);

    RETURN QUERY SELECT 'manco_role_to_board_rights'::TEXT, v_manco_created;

    -- Run GLEIF fund manager bridge
    SELECT * INTO v_gleif_created, v_gleif_updated
    FROM kyc.fn_bridge_gleif_fund_manager_to_board_rights(p_as_of_date);

    RETURN QUERY SELECT 'gleif_fund_manager_to_board_rights'::TEXT, v_gleif_created;

    -- Run BODS bridge
    SELECT * INTO v_bods_created, v_bods_updated, v_bods_linked
    FROM kyc.fn_bridge_bods_to_holdings(p_as_of_date);

    RETURN QUERY SELECT 'bods_to_holdings'::TEXT, v_bods_created;
    RETURN QUERY SELECT 'bods_entity_links'::TEXT, v_bods_linked;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.fn_run_governance_bridges IS
    'Run all governance bridges: ManCo roles, GLEIF fund managers, BODS ownership.';

-- =============================================================================
-- 5. Add unique constraint to special_rights if missing
-- =============================================================================
-- Need this for ON CONFLICT to work

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_special_rights_holder_issuer_type_class'
    ) THEN
        ALTER TABLE kyc.special_rights
        ADD CONSTRAINT uq_special_rights_holder_issuer_type_class
        UNIQUE NULLS NOT DISTINCT (issuer_entity_id, holder_entity_id, right_type, share_class_id);
    END IF;
END $$;

COMMIT;

-- =============================================================================
-- USAGE
-- =============================================================================
/*
-- Run all bridges (idempotent - safe to re-run)
SELECT * FROM kyc.fn_run_governance_bridges();

-- Then derive CBU groups (now governance controller should fire)
SELECT * FROM "ob-poc".fn_derive_cbu_groups();

-- Check results
SELECT * FROM "ob-poc".v_manco_group_summary;

-- Verify special_rights were created
SELECT notes, COUNT(*) FROM kyc.special_rights GROUP BY notes;
*/
