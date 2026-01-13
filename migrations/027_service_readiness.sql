-- Migration 027: Service Readiness (Derived)
--
-- Adds:
-- 1. cbu_service_readiness - derived table tracking "good to transact" status
-- 2. Views for readiness reporting
-- 3. Functions for readiness computation
--
-- This table is DERIVED and can be rebuilt from:
-- - service_intents
-- - srdef_discovery_reasons
-- - cbu_resource_instances
-- - cbu_unified_attr_requirements / cbu_attr_values
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. CBU SERVICE READINESS TABLE
-- =============================================================================
-- Materialized readiness status per CBU/product/service combination.
-- Rebuilt by the readiness computation engine.

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_readiness (
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),

    -- Readiness status
    status TEXT NOT NULL DEFAULT 'blocked'
        CHECK (status IN ('ready', 'blocked', 'partial')),

    -- Why is it blocked? Array of blocking reasons
    blocking_reasons JSONB NOT NULL DEFAULT '[]',

    -- What SRDEFs are required for this service?
    required_srdefs JSONB NOT NULL DEFAULT '[]',

    -- What resource instances are active?
    active_srids JSONB NOT NULL DEFAULT '[]',

    -- Computed at
    as_of TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Recomputation tracking
    last_recomputed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    recomputation_trigger TEXT,  -- what triggered recomputation

    PRIMARY KEY (cbu_id, product_id, service_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbu_service_readiness_status
    ON "ob-poc".cbu_service_readiness(cbu_id, status);
CREATE INDEX IF NOT EXISTS idx_cbu_service_readiness_blocked
    ON "ob-poc".cbu_service_readiness(cbu_id) WHERE status = 'blocked';
CREATE INDEX IF NOT EXISTS idx_cbu_service_readiness_ready
    ON "ob-poc".cbu_service_readiness(cbu_id) WHERE status = 'ready';

COMMENT ON TABLE "ob-poc".cbu_service_readiness IS
    'Derived readiness status per CBU service. Rebuildable from intents + instances + attrs.';

-- =============================================================================
-- 2. BLOCKING REASON SCHEMA
-- =============================================================================
-- Document the structure of blocking_reasons array

COMMENT ON COLUMN "ob-poc".cbu_service_readiness.blocking_reasons IS
$comment$
Array of blocking reasons. Each element:
{
  "type": "missing_srdef" | "pending_provisioning" | "failed_provisioning" |
          "missing_attrs" | "attr_conflict" | "dependency_not_ready",
  "srdef_id": "SRDEF::...",          -- which SRDEF is affected
  "details": {                        -- type-specific details
    // For missing_attrs:
    "missing_attr_ids": ["uuid1", "uuid2"],
    "missing_attr_names": ["market_scope", "settlement_currency"],

    // For pending_provisioning:
    "request_id": "uuid",
    "status": "queued" | "sent" | "ack",
    "owner_system": "CUSTODY",
    "requested_at": "2024-01-10T...",

    // For failed_provisioning:
    "request_id": "uuid",
    "error_message": "...",
    "error_codes": ["..."],

    // For dependency_not_ready:
    "depends_on_srdef": "SRDEF::...",
    "dependency_status": "blocked"
  },
  "explain": "Human-readable explanation"
}
$comment$;

-- =============================================================================
-- 3. VIEW: Readiness Dashboard
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_service_readiness_dashboard AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    r.product_id,
    p.name AS product_name,
    p.product_code,
    r.service_id,
    s.name AS service_name,
    s.service_code,
    r.status,
    jsonb_array_length(r.blocking_reasons) AS blocking_count,
    jsonb_array_length(r.required_srdefs) AS required_srdef_count,
    jsonb_array_length(r.active_srids) AS active_instance_count,
    r.as_of,
    r.last_recomputed_at
FROM "ob-poc".cbu_service_readiness r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
JOIN "ob-poc".products p ON p.product_id = r.product_id
JOIN "ob-poc".services s ON s.service_id = r.service_id
ORDER BY
    r.status DESC,  -- blocked first
    c.name, p.name, s.name;

COMMENT ON VIEW "ob-poc".v_service_readiness_dashboard IS
    'Dashboard view of service readiness across all CBUs.';

-- =============================================================================
-- 4. VIEW: CBU Readiness Summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cbu_readiness_summary AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    COUNT(*) AS total_services,
    COUNT(*) FILTER (WHERE r.status = 'ready') AS ready_count,
    COUNT(*) FILTER (WHERE r.status = 'partial') AS partial_count,
    COUNT(*) FILTER (WHERE r.status = 'blocked') AS blocked_count,
    ROUND(100.0 * COUNT(*) FILTER (WHERE r.status = 'ready') / NULLIF(COUNT(*), 0), 1) AS pct_ready
FROM "ob-poc".cbu_service_readiness r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
GROUP BY r.cbu_id, c.name
ORDER BY pct_ready ASC, c.name;

COMMENT ON VIEW "ob-poc".v_cbu_readiness_summary IS
    'Summary of readiness per CBU across all services.';

-- =============================================================================
-- 5. VIEW: Blocking Reasons Expanded
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_blocking_reasons_expanded AS
SELECT
    r.cbu_id,
    c.name AS cbu_name,
    r.product_id,
    p.name AS product_name,
    r.service_id,
    s.name AS service_name,
    br->>'type' AS blocking_type,
    br->>'srdef_id' AS srdef_id,
    br->>'explain' AS explanation,
    br->'details' AS details
FROM "ob-poc".cbu_service_readiness r
JOIN "ob-poc".cbus c ON c.cbu_id = r.cbu_id
JOIN "ob-poc".products p ON p.product_id = r.product_id
JOIN "ob-poc".services s ON s.service_id = r.service_id
CROSS JOIN LATERAL jsonb_array_elements(r.blocking_reasons) AS br
WHERE r.status IN ('blocked', 'partial')
ORDER BY c.name, p.name, s.name;

COMMENT ON VIEW "ob-poc".v_blocking_reasons_expanded IS
    'Flattened view of all blocking reasons for analysis.';

-- =============================================================================
-- 6. FUNCTION: Compute Service Readiness (Stub)
-- =============================================================================
-- Main computation logic lives in Rust. This is a stub for manual invocation.

CREATE OR REPLACE FUNCTION "ob-poc".compute_service_readiness(p_cbu_id UUID)
RETURNS TABLE(
    product_id UUID,
    service_id UUID,
    status TEXT,
    blocking_count INT
) AS $$
BEGIN
    -- Stub implementation
    -- Real logic is in Rust: ReadinessEngine::compute_for_cbu()

    RETURN QUERY
    SELECT
        si.product_id,
        si.service_id,
        'blocked'::TEXT AS status,
        0 AS blocking_count
    FROM "ob-poc".service_intents si
    WHERE si.cbu_id = p_cbu_id
    AND si.status = 'active';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".compute_service_readiness IS
    'Stub for readiness computation. Actual implementation in Rust ReadinessEngine.';

-- =============================================================================
-- 7. FUNCTION: Check Single SRDEF Readiness
-- =============================================================================
-- Check if a single SRDEF is ready (all attrs satisfied, instance active)

CREATE OR REPLACE FUNCTION "ob-poc".check_srdef_readiness(
    p_cbu_id UUID,
    p_srdef_id TEXT
) RETURNS TABLE(
    is_ready BOOLEAN,
    missing_attrs UUID[],
    instance_status TEXT,
    blocking_reason TEXT
) AS $$
DECLARE
    v_missing UUID[];
    v_instance_status TEXT;
    v_is_ready BOOLEAN := TRUE;
    v_blocking_reason TEXT;
BEGIN
    -- Check for missing required attributes
    SELECT ARRAY_AGG(r.attr_id) INTO v_missing
    FROM "ob-poc".cbu_unified_attr_requirements r
    LEFT JOIN "ob-poc".cbu_attr_values v ON v.cbu_id = r.cbu_id AND v.attr_id = r.attr_id
    WHERE r.cbu_id = p_cbu_id
    AND r.requirement_strength = 'required'
    AND p_srdef_id = ANY(SELECT jsonb_array_elements_text(r.required_by_srdefs))
    AND v.value IS NULL;

    IF v_missing IS NOT NULL AND array_length(v_missing, 1) > 0 THEN
        v_is_ready := FALSE;
        v_blocking_reason := 'Missing required attributes';
    END IF;

    -- Check instance status
    SELECT ri.status INTO v_instance_status
    FROM "ob-poc".cbu_resource_instances ri
    WHERE ri.cbu_id = p_cbu_id
    AND ri.srdef_id = p_srdef_id
    ORDER BY ri.created_at DESC
    LIMIT 1;

    IF v_instance_status IS NULL THEN
        v_is_ready := FALSE;
        v_blocking_reason := COALESCE(v_blocking_reason || '; ', '') || 'No instance provisioned';
    ELSIF v_instance_status != 'ACTIVE' THEN
        v_is_ready := FALSE;
        v_blocking_reason := COALESCE(v_blocking_reason || '; ', '') || 'Instance status: ' || v_instance_status;
    END IF;

    RETURN QUERY SELECT v_is_ready, v_missing, v_instance_status, v_blocking_reason;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".check_srdef_readiness IS
    'Check if a single SRDEF is ready for a CBU. Returns missing attrs and instance status.';

-- =============================================================================
-- 8. TRIGGER: Auto-invalidate readiness on instance change
-- =============================================================================
-- When a resource instance status changes, mark related readiness as stale.

ALTER TABLE "ob-poc".cbu_service_readiness
ADD COLUMN IF NOT EXISTS is_stale BOOLEAN DEFAULT FALSE;

CREATE OR REPLACE FUNCTION "ob-poc".invalidate_readiness_on_instance_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Mark readiness as stale when instance status changes
    IF TG_OP = 'UPDATE' AND OLD.status IS DISTINCT FROM NEW.status THEN
        UPDATE "ob-poc".cbu_service_readiness
        SET is_stale = TRUE
        WHERE cbu_id = NEW.cbu_id;
    ELSIF TG_OP = 'INSERT' THEN
        UPDATE "ob-poc".cbu_service_readiness
        SET is_stale = TRUE
        WHERE cbu_id = NEW.cbu_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_instance_invalidates_readiness ON "ob-poc".cbu_resource_instances;
CREATE TRIGGER trg_instance_invalidates_readiness
    AFTER INSERT OR UPDATE ON "ob-poc".cbu_resource_instances
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".invalidate_readiness_on_instance_change();

COMMENT ON COLUMN "ob-poc".cbu_service_readiness.is_stale IS
    'Set TRUE when underlying data changes. Triggers recomputation.';
