-- Migration 038: Split YAML and learned intent patterns
--
-- PROBLEM: sync_invocation_phrases overwrites intent_patterns on startup,
--          which deletes learned patterns added by the learning loop.
--
-- SOLUTION: Add yaml_intent_patterns column for YAML-sourced patterns.
--           Learning loop continues to use intent_patterns (which becomes learned-only).
--           View v_verb_intent_patterns unions both for embedding.
--
-- FLOW:
--   YAML invocation_phrases → dsl_verbs.yaml_intent_patterns (startup sync)
--   Learning loop → dsl_verbs.intent_patterns (learned patterns)
--   v_verb_intent_patterns → UNION of both → populate_embeddings

-- 1. Add yaml_intent_patterns column
ALTER TABLE "ob-poc".dsl_verbs
ADD COLUMN IF NOT EXISTS yaml_intent_patterns text[] DEFAULT ARRAY[]::text[];

COMMENT ON COLUMN "ob-poc".dsl_verbs.yaml_intent_patterns IS
    'Intent patterns from YAML invocation_phrases - synced on startup, safe to overwrite';

COMMENT ON COLUMN "ob-poc".dsl_verbs.intent_patterns IS
    'Learned intent patterns from feedback loop - NOT overwritten on startup';

-- 2. Migrate existing intent_patterns to yaml_intent_patterns (one-time)
-- This preserves current patterns as the YAML baseline
UPDATE "ob-poc".dsl_verbs
SET yaml_intent_patterns = COALESCE(intent_patterns, ARRAY[]::text[])
WHERE yaml_intent_patterns IS NULL OR yaml_intent_patterns = ARRAY[]::text[];

-- 3. Dedupe: remove from intent_patterns anything that's now in yaml_intent_patterns
-- This preserves only "true learned deltas" in intent_patterns
UPDATE "ob-poc".dsl_verbs
SET intent_patterns = (
  SELECT COALESCE(array_agg(DISTINCT p), ARRAY[]::text[])
  FROM unnest(COALESCE(intent_patterns, ARRAY[]::text[])) p
  WHERE NOT (p = ANY(COALESCE(yaml_intent_patterns, ARRAY[]::text[])))
)
WHERE intent_patterns IS NOT NULL
  AND array_length(intent_patterns, 1) > 0;

-- 4. Drop and recreate view to UNION both pattern sources
DROP VIEW IF EXISTS "ob-poc".v_verb_intent_patterns CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_verb_embedding_stats CASCADE;

CREATE VIEW "ob-poc".v_verb_intent_patterns AS
SELECT
    v.full_name as verb_full_name,
    pattern,
    v.category,
    CASE
        WHEN v.category IN ('investigation', 'screening', 'kyc_workflow') THEN true
        ELSE false
    END as is_agent_bound,
    1 as priority,
    'yaml' as source
FROM "ob-poc".dsl_verbs v
CROSS JOIN LATERAL unnest(v.yaml_intent_patterns) as pattern
WHERE v.yaml_intent_patterns IS NOT NULL
  AND array_length(v.yaml_intent_patterns, 1) > 0

UNION ALL

SELECT
    v.full_name as verb_full_name,
    pattern,
    v.category,
    CASE
        WHEN v.category IN ('investigation', 'screening', 'kyc_workflow') THEN true
        ELSE false
    END as is_agent_bound,
    2 as priority,  -- Learned patterns get higher priority
    'learned' as source
FROM "ob-poc".dsl_verbs v
CROSS JOIN LATERAL unnest(v.intent_patterns) as pattern
WHERE v.intent_patterns IS NOT NULL
  AND array_length(v.intent_patterns, 1) > 0;

COMMENT ON VIEW "ob-poc".v_verb_intent_patterns IS
    'Flattened view of all intent patterns (YAML + learned) for embedding population';

-- 5. Recreate stats view with new columns
CREATE VIEW "ob-poc".v_verb_embedding_stats AS
SELECT
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs) as total_verbs,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE (yaml_intent_patterns IS NOT NULL AND array_length(yaml_intent_patterns, 1) > 0)
        OR (intent_patterns IS NOT NULL AND array_length(intent_patterns, 1) > 0)) as verbs_with_patterns,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE yaml_intent_patterns IS NOT NULL AND array_length(yaml_intent_patterns, 1) > 0) as verbs_with_yaml_patterns,
    (SELECT COUNT(*) FROM "ob-poc".dsl_verbs
     WHERE intent_patterns IS NOT NULL AND array_length(intent_patterns, 1) > 0) as verbs_with_learned_patterns,
    (SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings WHERE embedding IS NOT NULL) as total_embeddings,
    (SELECT COUNT(DISTINCT verb_name) FROM "ob-poc".verb_pattern_embeddings) as unique_verbs_embedded;

COMMENT ON VIEW "ob-poc".v_verb_embedding_stats IS
    'Statistics for verb embedding coverage - split by YAML vs learned patterns';

-- 6. Update add_learned_pattern to ensure it goes to intent_patterns (not yaml)
-- (Already correct - appends to intent_patterns)

-- 7. Add index for yaml_intent_patterns lookups
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_yaml_patterns
ON "ob-poc".dsl_verbs USING GIN (yaml_intent_patterns)
WHERE yaml_intent_patterns IS NOT NULL;
