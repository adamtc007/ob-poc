-- =============================================================================
-- Contract Versioning for dsl_verbs
-- =============================================================================
-- Adds compiler version tracking to enable:
-- 1. Detection of stale compiled config (recompile when compiler changes)
-- 2. Debugging when behavior differs between environments
-- 3. CI validation that production uses expected compiler version
-- =============================================================================

-- Add compiler_version column - tracks which version of the DSL compiler
-- generated the compiled_json and compiled_hash
ALTER TABLE "ob-poc".dsl_verbs
ADD COLUMN IF NOT EXISTS compiler_version VARCHAR(50);

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiler_version IS
    'Semantic version of the DSL compiler that generated compiled_json (e.g., 0.1.0)';

-- Add compiled_at column - timestamp when the verb was last compiled
-- Distinct from updated_at which tracks any row change
ALTER TABLE "ob-poc".dsl_verbs
ADD COLUMN IF NOT EXISTS compiled_at TIMESTAMPTZ;

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_at IS
    'Timestamp when compiled_json was last generated (NULL if never compiled)';

-- Create index for queries filtering by compiler version
-- Useful for finding verbs compiled with outdated compiler
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_compiler_version
ON "ob-poc".dsl_verbs (compiler_version)
WHERE compiler_version IS NOT NULL;

-- =============================================================================
-- View: Verbs needing recompilation
-- =============================================================================
-- Shows verbs where:
-- 1. compiled_at is older than updated_at (source changed after compile)
-- 2. compiler_version doesn't match current (need recompile with new compiler)
-- 3. compiled_hash is NULL (never compiled)

CREATE OR REPLACE VIEW "ob-poc".v_verbs_needing_recompile AS
SELECT
    verb_id,
    full_name,
    domain,
    verb_name,
    yaml_hash,
    compiled_hash IS NOT NULL AS has_compiled,
    compiler_version,
    compiled_at,
    updated_at,
    CASE
        WHEN compiled_hash IS NULL THEN 'never_compiled'
        WHEN compiled_at IS NULL THEN 'missing_compiled_at'
        WHEN compiled_at < updated_at THEN 'source_changed'
        ELSE 'up_to_date'
    END AS recompile_reason
FROM "ob-poc".dsl_verbs
ORDER BY domain, verb_name;

COMMENT ON VIEW "ob-poc".v_verbs_needing_recompile IS
    'Shows verbs that may need recompilation with current compiler version';
