-- Phase 2.5: Phrase Bank Materialization
--
-- 1. Materialization trigger: SemOS phrase_mapping snapshots → phrase_bank
-- 2. Bulk migration of legacy invocation_phrases → phrase_bank
-- 3. Bulk migration of YAML invocation patterns → phrase_bank

-- ============================================================================
-- 1. Materialization trigger for phrase_mapping snapshots
-- ============================================================================

CREATE OR REPLACE FUNCTION sem_reg.materialize_phrase_mapping_to_bank()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    v_phrase    text;
    v_verb_fqn  text;
    v_workspace text;
    v_source    text;
    v_risk_tier text;
BEGIN
    IF NEW.object_type::text <> 'phrase_mapping'
       OR NEW.status::text <> 'active'
       OR NEW.effective_until IS NOT NULL
    THEN
        RETURN NEW;
    END IF;

    v_phrase    := NEW.definition ->> 'phrase';
    v_verb_fqn  := NEW.definition ->> 'verb_fqn';
    v_workspace := NEW.definition ->> 'workspace';
    v_source    := COALESCE(NEW.definition ->> 'source', 'governed');
    v_risk_tier := COALESCE(NEW.definition ->> 'risk_tier', 'elevated');

    IF v_phrase IS NULL OR v_verb_fqn IS NULL THEN
        RETURN NEW;
    END IF;

    -- Deactivate any existing active row for this (phrase, workspace)
    UPDATE "ob-poc".phrase_bank
    SET active = FALSE
    WHERE phrase = v_phrase
      AND COALESCE(workspace, '__global__') = COALESCE(v_workspace, '__global__')
      AND active = TRUE;

    -- Insert new active row
    INSERT INTO "ob-poc".phrase_bank (
        phrase, verb_fqn, workspace, source, risk_tier,
        sem_reg_snapshot_id, active
    ) VALUES (
        v_phrase, v_verb_fqn, v_workspace, v_source, v_risk_tier,
        NEW.snapshot_id, TRUE
    );

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_materialize_phrase_mapping ON sem_reg.snapshots;

CREATE TRIGGER trg_materialize_phrase_mapping
    AFTER INSERT ON sem_reg.snapshots
    FOR EACH ROW
    EXECUTE FUNCTION sem_reg.materialize_phrase_mapping_to_bank();

-- ============================================================================
-- 2. Bulk migration: invocation_phrases (legacy learned) → phrase_bank
-- ============================================================================

INSERT INTO "ob-poc".phrase_bank (phrase, verb_fqn, workspace, source, risk_tier, active)
SELECT
    ip.phrase,
    ip.verb,
    NULL,           -- global (no workspace context in legacy data)
    'legacy',
    'elevated',     -- default risk tier
    TRUE
FROM "ob-poc".invocation_phrases ip
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".phrase_bank pb
    WHERE pb.phrase = ip.phrase
      AND pb.active = TRUE
)
ON CONFLICT DO NOTHING;

-- ============================================================================
-- 3. Bulk migration: dsl_verbs.yaml_intent_patterns → phrase_bank
-- ============================================================================

INSERT INTO "ob-poc".phrase_bank (phrase, verb_fqn, workspace, source, risk_tier, active)
SELECT
    LOWER(TRIM(pattern)) as phrase,
    dv.full_name as verb_fqn,
    NULL,           -- global (YAML patterns are not workspace-qualified)
    'yaml',
    'standard',     -- YAML patterns are low-risk (developer-curated)
    TRUE
FROM "ob-poc".dsl_verbs dv
CROSS JOIN LATERAL unnest(
    COALESCE(dv.yaml_intent_patterns, '{}'::text[])
) AS pattern
WHERE pattern IS NOT NULL
  AND TRIM(pattern) <> ''
  AND NOT EXISTS (
    SELECT 1 FROM "ob-poc".phrase_bank pb
    WHERE pb.phrase = LOWER(TRIM(pattern))
      AND COALESCE(pb.workspace, '__global__') = '__global__'
      AND pb.active = TRUE
  )
ON CONFLICT DO NOTHING;
