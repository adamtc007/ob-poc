-- Migration 044: Agent teaching mechanism
-- Direct phrase→verb teaching that bypasses candidate staging
--
-- Created: 2026-01-21
-- Purpose: Allow explicit teaching of phrase→verb mappings

-- ============================================================================
-- 1. Teaching function (trusted source, no staging)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.teach_phrase(
    p_phrase TEXT,
    p_verb TEXT,
    p_source TEXT DEFAULT 'direct_teaching'
) RETURNS BOOLEAN AS $$
DECLARE
    v_normalized TEXT;
    v_added BOOLEAN;
    v_word_count INT;
BEGIN
    -- Normalize phrase
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));

    -- Basic validation: not empty
    IF v_normalized = '' OR v_normalized IS NULL THEN
        RAISE EXCEPTION 'Phrase cannot be empty';
    END IF;

    -- Basic validation: verb exists
    IF NOT EXISTS (SELECT 1 FROM "ob-poc".dsl_verbs WHERE full_name = p_verb) THEN
        RAISE EXCEPTION 'Unknown verb: %. Use verbs.list to see available verbs.', p_verb;
    END IF;

    -- Word count check (warn but don't block for teaching)
    v_word_count := array_length(string_to_array(v_normalized, ' '), 1);
    IF v_word_count < 2 THEN
        RAISE WARNING 'Very short phrase (% words) - may cause false positives', v_word_count;
    END IF;

    -- Add to dsl_verbs.intent_patterns using existing function
    SELECT "ob-poc".add_learned_pattern(p_verb, v_normalized) INTO v_added;

    IF v_added THEN
        -- Audit the teaching
        INSERT INTO agent.learning_audit (
            action,
            learning_type,
            actor,
            details
        ) VALUES (
            'taught',
            'invocation_phrase',
            p_source,
            jsonb_build_object(
                'phrase', v_normalized,
                'verb', p_verb,
                'word_count', v_word_count
            )
        );
    END IF;

    RETURN v_added;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.teach_phrase IS
    'Directly teach a phrase→verb mapping. Bypasses candidate staging (trusted source).';

-- ============================================================================
-- 2. Batch teaching function
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.teach_phrases_batch(
    p_phrases JSONB,  -- Array of {phrase, verb} objects
    p_source TEXT DEFAULT 'batch_teaching'
) RETURNS TABLE (
    phrase TEXT,
    verb TEXT,
    success BOOLEAN,
    message TEXT
) AS $$
DECLARE
    v_item JSONB;
    v_phrase TEXT;
    v_verb TEXT;
    v_added BOOLEAN;
BEGIN
    FOR v_item IN SELECT * FROM jsonb_array_elements(p_phrases)
    LOOP
        v_phrase := v_item->>'phrase';
        v_verb := v_item->>'verb';

        BEGIN
            SELECT agent.teach_phrase(v_phrase, v_verb, p_source) INTO v_added;

            phrase := v_phrase;
            verb := v_verb;
            success := v_added;
            message := CASE
                WHEN v_added THEN 'Learned'
                ELSE 'Already exists'
            END;
            RETURN NEXT;

        EXCEPTION WHEN OTHERS THEN
            phrase := v_phrase;
            verb := v_verb;
            success := FALSE;
            message := SQLERRM;
            RETURN NEXT;
        END;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.teach_phrases_batch IS
    'Batch teach multiple phrase→verb mappings from JSON array.';

-- ============================================================================
-- 3. Function: Unteach a pattern (with audit)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.unteach_phrase(
    p_phrase TEXT,
    p_verb TEXT,
    p_reason TEXT DEFAULT NULL,
    p_actor TEXT DEFAULT 'manual'
) RETURNS BOOLEAN AS $$
DECLARE
    v_normalized TEXT;
    v_removed BOOLEAN := FALSE;
BEGIN
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));

    -- Remove from dsl_verbs.intent_patterns
    UPDATE "ob-poc".dsl_verbs
    SET intent_patterns = array_remove(intent_patterns, v_normalized),
        updated_at = NOW()
    WHERE full_name = p_verb
      AND v_normalized = ANY(intent_patterns);

    v_removed := FOUND;

    IF v_removed THEN
        -- Remove from embeddings cache
        DELETE FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = p_verb
          AND pattern_normalized = v_normalized;

        -- Audit
        INSERT INTO agent.learning_audit (
            action, learning_type, actor, details
        ) VALUES (
            'untaught',
            'invocation_phrase',
            p_actor,
            jsonb_build_object(
                'phrase', v_normalized,
                'verb', p_verb,
                'reason', p_reason
            )
        );
    END IF;

    RETURN v_removed;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.unteach_phrase IS
    'Remove a taught pattern (with audit). Use when a pattern causes problems.';

-- ============================================================================
-- 4. View: Recently taught patterns
-- ============================================================================

CREATE OR REPLACE VIEW agent.v_recently_taught AS
SELECT
    la.id,
    la.details->>'phrase' as phrase,
    la.details->>'verb' as verb,
    la.actor as source,
    la.timestamp as taught_at,
    -- Check if embedding exists yet
    EXISTS (
        SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
        WHERE vpe.verb_name = la.details->>'verb'
          AND vpe.pattern_normalized = la.details->>'phrase'
          AND vpe.embedding IS NOT NULL
    ) as has_embedding
FROM agent.learning_audit la
WHERE la.action = 'taught'
  AND la.learning_type = 'invocation_phrase'
ORDER BY la.timestamp DESC
LIMIT 100;

COMMENT ON VIEW agent.v_recently_taught IS
    'Recently taught patterns with embedding status. Run populate_embeddings to activate patterns without embeddings.';

-- ============================================================================
-- 5. Function: Get patterns pending embedding
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_taught_pending_embeddings()
RETURNS TABLE (
    verb TEXT,
    phrase TEXT,
    taught_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        la.details->>'verb' as verb,
        la.details->>'phrase' as phrase,
        la.timestamp as taught_at
    FROM agent.learning_audit la
    WHERE la.action = 'taught'
      AND la.learning_type = 'invocation_phrase'
      AND NOT EXISTS (
          SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
          WHERE vpe.verb_name = la.details->>'verb'
            AND vpe.pattern_normalized = la.details->>'phrase'
            AND vpe.embedding IS NOT NULL
      )
    ORDER BY la.timestamp DESC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.get_taught_pending_embeddings IS
    'Get taught patterns that are awaiting populate_embeddings to create vectors.';

-- ============================================================================
-- 6. Teaching stats view
-- ============================================================================

CREATE OR REPLACE VIEW agent.v_teaching_stats AS
SELECT
    DATE_TRUNC('day', la.timestamp) as day,
    la.actor as source,
    COUNT(*) FILTER (WHERE la.action = 'taught') as patterns_taught,
    COUNT(*) FILTER (WHERE la.action = 'untaught') as patterns_untaught,
    COUNT(DISTINCT la.details->>'verb') as verbs_affected
FROM agent.learning_audit la
WHERE la.action IN ('taught', 'untaught')
  AND la.learning_type = 'invocation_phrase'
  AND la.timestamp > NOW() - INTERVAL '30 days'
GROUP BY 1, 2
ORDER BY 1 DESC, 3 DESC;

COMMENT ON VIEW agent.v_teaching_stats IS
    'Teaching activity over the last 30 days by source.';

-- ============================================================================
-- 7. Grant permissions
-- ============================================================================

GRANT EXECUTE ON FUNCTION agent.teach_phrase TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.teach_phrases_batch TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.unteach_phrase TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.get_taught_pending_embeddings TO PUBLIC;
