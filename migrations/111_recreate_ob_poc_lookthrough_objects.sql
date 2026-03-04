-- Migration 111: Recreate look-through views/functions/triggers in "ob-poc"
--
-- Context:
-- Legacy kyc/custody/client_portal schemas were removed in migration 110.
-- This migration recreates economic look-through projections and timestamp
-- triggers in the consolidated "ob-poc" schema.

-- =============================================================================
-- VIEW: Fund vehicle summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_fund_vehicle_summary AS
SELECT
    fv.fund_entity_id,
    e.name AS fund_name,
    fv.vehicle_type,
    fv.is_umbrella,
    fv.domicile_country,
    umbrella.name AS umbrella_name,
    manager.name AS manager_name,
    (
        SELECT COUNT(*)
        FROM "ob-poc".fund_compartments fc
        WHERE fc.umbrella_fund_entity_id = fv.fund_entity_id
    ) AS compartment_count,
    (
        SELECT COUNT(*)
        FROM "ob-poc".share_classes sc
        WHERE sc.entity_id = fv.fund_entity_id
    ) AS share_class_count,
    fv.meta,
    fv.created_at
FROM "ob-poc".fund_vehicles fv
JOIN "ob-poc".entities e ON fv.fund_entity_id = e.entity_id
LEFT JOIN "ob-poc".entities umbrella ON fv.umbrella_entity_id = umbrella.entity_id
LEFT JOIN "ob-poc".entities manager ON fv.manager_entity_id = manager.entity_id;

COMMENT ON VIEW "ob-poc".v_fund_vehicle_summary IS
'Fund vehicles with resolved entity names and aggregate counts';

-- =============================================================================
-- VIEW: Direct economic edges (base case)
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_economic_edges_direct AS
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
FROM "ob-poc".ownership_snapshots os
LEFT JOIN "ob-poc".share_classes sc ON os.share_class_id = sc.id
LEFT JOIN "ob-poc".fund_vehicles fv ON os.issuer_entity_id = fv.fund_entity_id
WHERE os.basis = 'ECONOMIC'
  AND os.is_direct = true
  AND os.superseded_at IS NULL;

COMMENT ON VIEW "ob-poc".v_economic_edges_direct IS
'Direct economic ownership edges (single-hop). Base case for recursive look-through.';

-- =============================================================================
-- FUNCTION: Bounded economic look-through with cycle detection
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_compute_economic_exposure(
    p_root_entity_id UUID,
    p_as_of_date DATE DEFAULT CURRENT_DATE,
    p_max_depth INT DEFAULT 6,
    p_min_pct NUMERIC DEFAULT 0.0001,
    p_max_rows INT DEFAULT 200,
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
    stopped_reason TEXT
) AS $$
WITH RECURSIVE exposure_tree AS (
    SELECT
        p_root_entity_id AS root_id,
        e.to_entity_id AS current_id,
        ent.name AS current_name,
        e.pct_of_to::NUMERIC AS cumulative_pct,
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
    FROM "ob-poc".v_economic_edges_direct e
    JOIN "ob-poc".entities ent ON e.to_entity_id = ent.entity_id
    LEFT JOIN "ob-poc".investor_role_profiles rp
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE e.from_entity_id = p_root_entity_id
      AND e.as_of_date <= p_as_of_date

    UNION ALL

    SELECT
        t.root_id,
        e.to_entity_id,
        ent.name,
        (t.cumulative_pct * (e.pct_of_to::NUMERIC / 100.0))::NUMERIC,
        t.depth + 1,
        t.path || e.to_entity_id,
        t.path_names || ent.name::TEXT,
        CASE
            WHEN e.to_entity_id = ANY(t.path) THEN 'CYCLE_DETECTED'
            WHEN t.depth + 1 >= p_max_depth THEN 'MAX_DEPTH'
            WHEN (t.cumulative_pct * (e.pct_of_to::NUMERIC / 100.0)) < p_min_pct THEN 'BELOW_MIN_PCT'
            WHEN rp.role_type = 'END_INVESTOR' THEN 'END_INVESTOR'
            WHEN rp.lookthrough_policy = 'NONE' AND p_stop_on_policy_none THEN 'POLICY_NONE'
            WHEN rp.beneficial_owner_data_available = false AND p_stop_on_no_bo_data THEN 'NO_BO_DATA'
            ELSE NULL
        END AS stop_reason
    FROM exposure_tree t
    JOIN "ob-poc".v_economic_edges_direct e ON e.from_entity_id = t.current_id
    JOIN "ob-poc".entities ent ON e.to_entity_id = ent.entity_id
    LEFT JOIN "ob-poc".investor_role_profiles rp
        ON rp.holder_entity_id = e.from_entity_id
        AND rp.issuer_entity_id = e.to_entity_id
        AND rp.effective_to IS NULL
    WHERE t.stop_reason IS NULL
      AND t.depth < p_max_depth
      AND t.cumulative_pct >= p_min_pct
      AND e.as_of_date <= p_as_of_date
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
WHERE stop_reason IS NOT NULL
   OR NOT EXISTS (
        SELECT 1 FROM "ob-poc".v_economic_edges_direct e2
        WHERE e2.from_entity_id = exposure_tree.current_id
          AND e2.as_of_date <= p_as_of_date
   )
ORDER BY cumulative_pct DESC
LIMIT p_max_rows;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION "ob-poc".fn_compute_economic_exposure IS
'Bounded look-through computation for economic exposure.';

-- =============================================================================
-- FUNCTION: Economic exposure summary
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".fn_economic_exposure_summary(
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
    SELECT
        os.owner_entity_id,
        e.name AS owner_name,
        os.percentage AS direct_pct,
        rp.role_type
    FROM "ob-poc".ownership_snapshots os
    JOIN "ob-poc".entities e ON os.owner_entity_id = e.entity_id
    LEFT JOIN "ob-poc".investor_role_profiles rp
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
    SELECT
        dh.owner_entity_id AS top_holder_id,
        lt.leaf_entity_id,
        lt.leaf_name,
        lt.cumulative_pct * (dh.direct_pct / 100.0) AS effective_pct,
        lt.depth,
        lt.stopped_reason
    FROM direct_holdings dh
    CROSS JOIN LATERAL "ob-poc".fn_compute_economic_exposure(
        dh.owner_entity_id,
        p_as_of_date,
        6,
        0.01,
        50
    ) lt
    WHERE dh.role_type IN ('INTERMEDIARY_FOF', 'MASTER_POOL', 'OMNIBUS', 'NOMINEE')
       OR dh.role_type IS NULL
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

COMMENT ON FUNCTION "ob-poc".fn_economic_exposure_summary IS
'Aggregated economic exposure view for an issuer.';

-- =============================================================================
-- Trigger helpers recreated in consolidated schema
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_fund_vehicle_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_fund_vehicle_updated ON "ob-poc".fund_vehicles;
CREATE TRIGGER trg_fund_vehicle_updated
    BEFORE UPDATE ON "ob-poc".fund_vehicles
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_fund_vehicle_timestamp();

DROP TRIGGER IF EXISTS trg_fund_compartment_updated ON "ob-poc".fund_compartments;
CREATE TRIGGER trg_fund_compartment_updated
    BEFORE UPDATE ON "ob-poc".fund_compartments
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_fund_vehicle_timestamp();

CREATE OR REPLACE FUNCTION "ob-poc".update_issuer_control_config_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_issuer_control_config_updated ON "ob-poc".issuer_control_config;
CREATE TRIGGER trg_issuer_control_config_updated
    BEFORE UPDATE ON "ob-poc".issuer_control_config
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_issuer_control_config_timestamp();
