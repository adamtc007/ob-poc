-- Phase 2: Execution Verb Hashes
-- Track which verb configurations (by compiled_hash) were used in each execution
-- This enables:
--   1. Audit: "What exact verb config was active when this ran?"
--   2. Debugging: Correlate execution failures with specific verb versions
--   3. Reproducibility: Re-run with the same verb definitions

--------------------------------------------------------------------------------
-- Part A: Add verb_hash to dsl_idempotency (per-statement tracking)
--------------------------------------------------------------------------------

-- Add verb_hash column to track which verb config was used for each statement
ALTER TABLE "ob-poc".dsl_idempotency
ADD COLUMN IF NOT EXISTS verb_hash BYTEA DEFAULT NULL;

COMMENT ON COLUMN "ob-poc".dsl_idempotency.verb_hash IS
'SHA256 compiled_hash of the verb config used for this execution. Links to dsl_verbs.compiled_hash.';

-- Create index for finding executions by verb hash
CREATE INDEX IF NOT EXISTS idx_dsl_idempotency_verb_hash
ON "ob-poc".dsl_idempotency (verb_hash) WHERE verb_hash IS NOT NULL;

--------------------------------------------------------------------------------
-- Part B: Add aggregated verb tracking to dsl_execution_log (per-execution)
--------------------------------------------------------------------------------

-- Add verb_hashes column to dsl_execution_log
-- Stores array of compiled_hash values from dsl_verbs table for each verb used
ALTER TABLE "ob-poc".dsl_execution_log
ADD COLUMN IF NOT EXISTS verb_hashes BYTEA[] DEFAULT NULL;

COMMENT ON COLUMN "ob-poc".dsl_execution_log.verb_hashes IS
'Array of compiled_hash values (SHA256) for verbs used in this execution. Links to dsl_verbs.compiled_hash for audit trail.';

-- Add verb_names column for human-readable reference
ALTER TABLE "ob-poc".dsl_execution_log
ADD COLUMN IF NOT EXISTS verb_names TEXT[] DEFAULT NULL;

COMMENT ON COLUMN "ob-poc".dsl_execution_log.verb_names IS
'Array of verb names (domain.verb) used in this execution. Parallel to verb_hashes for readability.';

-- Create index for querying executions by verb hash
-- Useful for "find all executions that used this specific verb config"
CREATE INDEX IF NOT EXISTS idx_dsl_execution_verb_hashes
ON "ob-poc".dsl_execution_log USING GIN (verb_hashes);

--------------------------------------------------------------------------------
-- Part C: Helper functions for verb hash queries
--------------------------------------------------------------------------------

-- Create function to find executions by verb hash
CREATE OR REPLACE FUNCTION "ob-poc".find_executions_by_verb_hash(
    p_verb_hash BYTEA
)
RETURNS TABLE (
    execution_id UUID,
    cbu_id VARCHAR(255),
    status VARCHAR(50),
    started_at TIMESTAMPTZ,
    verb_names TEXT[]
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        execution_id,
        cbu_id,
        status,
        started_at,
        verb_names
    FROM "ob-poc".dsl_execution_log
    WHERE p_verb_hash = ANY(verb_hashes)
    ORDER BY started_at DESC;
$$;

COMMENT ON FUNCTION "ob-poc".find_executions_by_verb_hash(BYTEA) IS
'Find all executions that used a specific verb configuration (by compiled_hash)';

-- Create function to find idempotency records by verb hash
CREATE OR REPLACE FUNCTION "ob-poc".find_idempotency_by_verb_hash(
    p_verb_hash BYTEA,
    p_limit INT DEFAULT 100
)
RETURNS TABLE (
    idempotency_key TEXT,
    execution_id UUID,
    verb TEXT,
    result_type TEXT,
    created_at TIMESTAMPTZ
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        idempotency_key,
        execution_id,
        verb,
        result_type,
        created_at
    FROM "ob-poc".dsl_idempotency
    WHERE verb_hash = p_verb_hash
    ORDER BY created_at DESC
    LIMIT p_limit;
$$;

COMMENT ON FUNCTION "ob-poc".find_idempotency_by_verb_hash(BYTEA, INT) IS
'Find idempotency records that used a specific verb configuration (by compiled_hash)';

-- Create function to get verb config at execution time
CREATE OR REPLACE FUNCTION "ob-poc".get_verb_config_at_execution(
    p_execution_id UUID,
    p_verb_name TEXT
)
RETURNS TABLE (
    verb_name TEXT,
    compiled_hash BYTEA,
    compiled_json JSONB,
    effective_config_json JSONB,
    diagnostics_json JSONB
)
LANGUAGE SQL
STABLE
AS $$
    WITH execution_hash AS (
        SELECT verb_hashes[idx] as hash
        FROM "ob-poc".dsl_execution_log el,
             generate_subscripts(el.verb_names, 1) as idx
        WHERE el.execution_id = p_execution_id
          AND el.verb_names[idx] = p_verb_name
    )
    SELECT
        v.verb_name,
        v.compiled_hash,
        v.compiled_json,
        v.effective_config_json,
        v.diagnostics_json
    FROM "ob-poc".dsl_verbs v
    WHERE v.compiled_hash = (SELECT hash FROM execution_hash LIMIT 1);
$$;

COMMENT ON FUNCTION "ob-poc".get_verb_config_at_execution(UUID, TEXT) IS
'Get the exact verb configuration that was active during a specific execution';

--------------------------------------------------------------------------------
-- Part D: Audit view
--------------------------------------------------------------------------------

-- Create view for execution audit with verb details
CREATE OR REPLACE VIEW "ob-poc".v_execution_verb_audit AS
SELECT
    el.execution_id,
    el.cbu_id,
    el.execution_phase,
    el.status,
    el.started_at,
    el.completed_at,
    el.duration_ms,
    el.executed_by,
    COALESCE(array_length(el.verb_names, 1), 0) as verb_count,
    el.verb_names,
    -- Check if any verb hashes no longer match current config (verb was updated)
    EXISTS (
        SELECT 1
        FROM unnest(el.verb_hashes, el.verb_names) AS t(hash, name)
        JOIN "ob-poc".dsl_verbs v ON v.verb_name = t.name
        WHERE v.compiled_hash IS DISTINCT FROM t.hash
    ) as has_stale_verb_refs
FROM "ob-poc".dsl_execution_log el
WHERE el.verb_hashes IS NOT NULL;

COMMENT ON VIEW "ob-poc".v_execution_verb_audit IS
'Execution log with verb versioning audit info. has_stale_verb_refs=true means verb config changed since execution.';
