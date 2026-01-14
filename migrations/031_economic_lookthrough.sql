-- Migration 031: Economic Look-Through Function
--
-- Purpose: Bounded recursive computation of economic exposure through fund chains.
-- Features:
-- - Configurable depth limit (default 6)
-- - Minimum percentage threshold (stops when ownership drops below threshold)
-- - Cycle detection (prevents infinite loops in malformed data)
-- - Respects role profiles (stops at END_INVESTOR, looks through INTERMEDIARY_FOF, etc.)
-- - Explicit stop condition precedence for debugging
--
-- Design rationale: Store direct edges only, compute look-through on-demand.
-- Materializing implied edges is unmaintainable at scale (1K investors × 200 SPVs = explosion).

-- =============================================================================
-- VIEW: Direct economic edges (single-hop ownership for recursive base case)
-- =============================================================================

CREATE OR REPLACE VIEW kyc.v_economic_edges_direct AS
SELECT
    os.owner_entity_id AS from_entity_id,
    os.issuer_entity_id AS to_entity_id,
    os.percentage AS pct_of_to,
    COALESCE(sc.instrument_type, 'SHARES') AS instrument_type,
    os.share_class_id,
    fv.vehicle_type,
    os.basis,
    'OWNERSHIP_SNAPSHOT' AS source,
    os.as_of_date
FROM kyc.ownership_snapshots os
LEFT JOIN kyc.share_classes sc ON os.share_class_id = sc.id
LEFT JOIN kyc.fund_vehicles fv ON os.issuer_entity_id = fv.fund_entity_id
WHERE os.basis = 'ECONOMIC'
  AND os.is_direct = true
  AND os.superseded_at IS NULL;

COMMENT ON VIEW kyc.v_economic_edges_direct IS
'Direct economic ownership edges (single-hop). Base case for recursive look-through.';

-- =============================================================================
-- FUNCTION: Bounded economic look-through with cycle detection
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_compute_economic_exposure(
    p_root_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_max_depth INT DEFAULT 6,
    p_min_pct NUMERIC DEFAULT 0.0001,
    p_max_rows INT DEFAULT 200,
    -- Explicit stop condition config
    p_stop_on_no_bo_data BOOLEAN DEFAULT true,
    p_stop_on_policy_none BOOLEAN DEFAULT true
) RETURNS TABLE (
    root_entity_id UUID,
    leaf_entity_id UUID,
    leaf_name TEXT,
    cumulative_pct NUMERIC,
    depth INT,
    path_entities UUID[],
    path_names TEXT[],
    stopped_reason TEXT  -- Why traversal stopped at this leaf
) AS $$
WITH RECURSIVE exposure_tree AS (
    -- Base case: direct holdings from root
    SELECT
        p_root_entity_id AS root_id,
        e.to_entity_id AS current_id,
        ent.name AS current_name,
        e.pct_of_to::NUMERIC AS cumulative_pct,  -- Cast to generic NUMERIC for recursive union
        1 AS depth,
        ARRAY[p_root_entity_id, e.to_entity_id] AS path,
        ARRAY[
            (SELECT name FROM "ob-poc".entities WHERE entity_id = p_root_entity_id)::TEXT,
            ent.name::TEXT
        ]::TEXT[] AS path_names,
        CASE
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            WHEN rp.role_type = 'END_INVESTOR' THEN 'END_INVESTOR'
            ELSE NULL
        END AS stop_reason
    FROM kyc.v_economic_edges_direct e
    JOIN "ob-poc".entities ent ON e.to_entity_id = ent.entity_id
    LEFT JOIN kyc.investor_role_profiles rp
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE e.from_entity_id = p_root_entity_id
      AND e.as_of_date <= p_as_of_date

    UNION ALL

    -- Recursive case: traverse deeper
    SELECT
        t.root_id,
        e.to_entity_id,
        ent.name,
        (t.cumulative_pct * (e.pct_of_to::NUMERIC / 100.0))::NUMERIC,
        t.depth + 1,
        t.path || e.to_entity_id,
        t.path_names || ent.name::TEXT,
        CASE
            -- STOP CONDITION PRECEDENCE:
            -- 1. Cycle detection (highest priority - prevents infinite loops)
            WHEN e.to_entity_id = ANY(t.path) THEN 'CYCLE_DETECTED'
            -- 2. Depth limit
            WHEN t.depth + 1 >= p_max_depth THEN 'MAX_DEPTH'
            -- 3. Percentage threshold
            WHEN (t.cumulative_pct * (e.pct_of_to::NUMERIC / 100.0)) < p_min_pct THEN 'BELOW_MIN_PCT'
            -- 4. End investor role
            WHEN rp.role_type = 'END_INVESTOR' THEN 'END_INVESTOR'
            -- 5. Lookthrough policy
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            -- 6. BO data availability
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            ELSE NULL
        END AS stop_reason
    FROM exposure_tree t
    JOIN kyc.v_economic_edges_direct e ON e.from_entity_id = t.current_id
    JOIN "ob-poc".entities ent ON e.to_entity_id = ent.entity_id
    LEFT JOIN kyc.investor_role_profiles rp
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE t.stop_reason IS NULL  -- Only continue if not stopped
      AND t.depth < p_max_depth
      AND t.cumulative_pct >= p_min_pct
      AND e.as_of_date <= p_as_of_date
      -- CYCLE DETECTION: prevent visiting same node twice in path
      AND NOT (e.to_entity_id = ANY(t.path))
)
SELECT
    root_id AS root_entity_id,
    current_id AS leaf_entity_id,
    current_name AS leaf_name,
    cumulative_pct,
    depth,
    path AS path_entities,
    path_names,
    COALESCE(stop_reason, 'LEAF_NODE') AS stopped_reason
FROM exposure_tree
WHERE stop_reason IS NOT NULL  -- Only return nodes where traversal stopped
   OR NOT EXISTS (  -- Or true leaf nodes with no further edges
        SELECT 1 FROM kyc.v_economic_edges_direct e2
        WHERE e2.from_entity_id = exposure_tree.current_id
          AND e2.as_of_date <= p_as_of_date
   )
ORDER BY cumulative_pct DESC
LIMIT p_max_rows;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.fn_compute_economic_exposure IS
'Bounded look-through computation for economic exposure.

Parameters:
- p_root_entity_id: Starting entity (typically a fund)
- p_as_of_date: Point-in-time query date
- p_max_depth: Maximum traversal depth (default 6)
- p_min_pct: Stop when cumulative ownership drops below this (default 0.01%)
- p_max_rows: Limit result set size (default 200)
- p_stop_on_no_bo_data: Stop at holders without BO data
- p_stop_on_policy_none: Stop at holders with NONE lookthrough policy

Stop condition precedence (debugging aid):
1. CYCLE_DETECTED - prevents infinite loops in malformed data
2. MAX_DEPTH - hard depth limit
3. BELOW_MIN_PCT - percentage threshold
4. END_INVESTOR - role profile marks as end of chain
5. POLICY_NONE - role profile says no lookthrough
6. NO_BO_DATA - beneficial owner data unavailable
7. LEAF_NODE - no further edges to traverse';

-- =============================================================================
-- FUNCTION: Get economic exposure summary for an issuer (aggregated view)
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.fn_economic_exposure_summary(
    p_issuer_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_threshold_pct NUMERIC DEFAULT 5.0
) RETURNS TABLE (
    investor_entity_id UUID,
    investor_name TEXT,
    direct_pct NUMERIC,
    lookthrough_pct NUMERIC,
    is_above_threshold BOOLEAN,
    role_type VARCHAR(50),
    depth INT,
    stop_reason TEXT
) AS $$
WITH direct_holdings AS (
    -- Direct ownership from ownership_snapshots
    SELECT
        os.owner_entity_id,
        e.name AS owner_name,
        os.percentage AS direct_pct,
        rp.role_type
    FROM kyc.ownership_snapshots os
    JOIN "ob-poc".entities e ON os.owner_entity_id = e.entity_id
    LEFT JOIN kyc.investor_role_profiles rp
        ON rp.holder_entity_id = os.owner_entity_id
        AND rp.issuer_entity_id = os.issuer_entity_id
        AND rp.effective_to IS NULL
    WHERE os.issuer_entity_id = p_issuer_entity_id
      AND os.basis = 'ECONOMIC'
      AND os.is_direct = true
      AND os.as_of_date <= p_as_of_date
      AND os.superseded_at IS NULL
),
lookthrough AS (
    -- Look-through from each direct holder
    SELECT
        dh.owner_entity_id AS top_holder_id,
        lt.leaf_entity_id,
        lt.leaf_name,
        lt.cumulative_pct * (dh.direct_pct / 100.0) AS effective_pct,
        lt.depth,
        lt.stopped_reason
    FROM direct_holdings dh
    CROSS JOIN LATERAL kyc.fn_compute_economic_exposure(
        dh.owner_entity_id,
        p_as_of_date,
        6,     -- max_depth
        0.01,  -- min_pct (0.01%)
        50     -- max_rows per holder
    ) lt
    WHERE dh.role_type IN ('INTERMEDIARY_FOF', 'MASTER_POOL', 'OMNIBUS', 'NOMINEE')
       OR dh.role_type IS NULL  -- Default: try look-through
)
SELECT
    COALESCE(lt.leaf_entity_id, dh.owner_entity_id) AS investor_entity_id,
    COALESCE(lt.leaf_name, dh.owner_name) AS investor_name,
    dh.direct_pct,
    COALESCE(lt.effective_pct, dh.direct_pct) AS lookthrough_pct,
    COALESCE(lt.effective_pct, dh.direct_pct) >= p_threshold_pct AS is_above_threshold,
    dh.role_type,
    COALESCE(lt.depth, 0) AS depth,
    lt.stopped_reason AS stop_reason
FROM direct_holdings dh
LEFT JOIN lookthrough lt ON lt.top_holder_id = dh.owner_entity_id
ORDER BY COALESCE(lt.effective_pct, dh.direct_pct) DESC;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.fn_economic_exposure_summary IS
'Aggregated economic exposure view for an issuer. Shows both direct and look-through percentages.
Use p_threshold_pct to filter for significant holders (default 5%).';

-- =============================================================================
-- VIEW: Issuer control thresholds configuration
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.issuer_control_config (
    issuer_entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    disclosure_threshold_pct NUMERIC NOT NULL DEFAULT 5.0,
    material_threshold_pct NUMERIC NOT NULL DEFAULT 10.0,
    significant_threshold_pct NUMERIC NOT NULL DEFAULT 25.0,
    control_threshold_pct NUMERIC NOT NULL DEFAULT 50.0,
    lookthrough_depth_limit INT NOT NULL DEFAULT 6,
    lookthrough_min_pct NUMERIC NOT NULL DEFAULT 0.01,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE kyc.issuer_control_config IS
'Per-issuer configuration for control thresholds and look-through parameters.
Defaults match common regulatory requirements (5% disclosure, 25% significant, 50% control).';

-- Trigger for updated_at
CREATE OR REPLACE FUNCTION kyc.update_issuer_control_config_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_issuer_control_config_updated ON kyc.issuer_control_config;
CREATE TRIGGER trg_issuer_control_config_updated
    BEFORE UPDATE ON kyc.issuer_control_config
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_issuer_control_config_timestamp();
