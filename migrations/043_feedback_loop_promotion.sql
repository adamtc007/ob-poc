-- Migration 043: Feedback loop promotion infrastructure
-- Adds success tracking, quality gates, and collision detection for pattern learning
--
-- Created: 2026-01-21
-- Purpose: Implement staged promotion pipeline with quality guardrails

-- ============================================================================
-- 1. Add success tracking columns to learning_candidates
-- ============================================================================

ALTER TABLE agent.learning_candidates
ADD COLUMN IF NOT EXISTS success_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS total_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_success_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS domain_hint TEXT,
ADD COLUMN IF NOT EXISTS collision_safe BOOLEAN,
ADD COLUMN IF NOT EXISTS collision_check_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS collision_verb TEXT;

COMMENT ON COLUMN agent.learning_candidates.success_count IS
    'Count of successful outcomes (executed + DSL succeeded, or user selected this verb)';
COMMENT ON COLUMN agent.learning_candidates.total_count IS
    'Total signals received (success + failure)';
COMMENT ON COLUMN agent.learning_candidates.collision_safe IS
    'Whether pattern passed semantic collision check (NULL = not checked)';
COMMENT ON COLUMN agent.learning_candidates.collision_verb IS
    'If collision detected, which verb it conflicts with';

-- ============================================================================
-- 2. Stopwords table for quality filtering
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent.stopwords (
    word TEXT PRIMARY KEY,
    category TEXT DEFAULT 'generic'  -- 'generic', 'polite', 'filler'
);

COMMENT ON TABLE agent.stopwords IS 'Common words that should not dominate learning patterns';

-- Seed common stopwords
INSERT INTO agent.stopwords (word, category) VALUES
    ('the', 'generic'), ('a', 'generic'), ('an', 'generic'),
    ('please', 'polite'), ('can', 'polite'), ('could', 'polite'),
    ('you', 'polite'), ('would', 'polite'), ('help', 'polite'),
    ('me', 'generic'), ('i', 'generic'), ('my', 'generic'),
    ('want', 'filler'), ('need', 'filler'), ('like', 'filler'),
    ('to', 'generic'), ('for', 'generic'), ('with', 'generic'),
    ('this', 'generic'), ('that', 'generic'), ('it', 'generic'),
    ('do', 'filler'), ('make', 'filler'), ('get', 'filler'),
    ('just', 'filler'), ('now', 'filler'), ('here', 'filler'),
    ('of', 'generic'), ('in', 'generic'), ('on', 'generic'),
    ('at', 'generic'), ('by', 'generic'), ('is', 'generic'),
    ('are', 'generic'), ('was', 'generic'), ('be', 'generic'),
    ('have', 'generic'), ('has', 'generic'), ('had', 'generic'),
    ('show', 'filler'), ('give', 'filler'), ('tell', 'filler')
ON CONFLICT (word) DO NOTHING;

-- ============================================================================
-- 3. Function: Record learning signal with success tracking
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.record_learning_signal(
    p_phrase TEXT,
    p_verb TEXT,
    p_is_success BOOLEAN,
    p_signal_type TEXT,  -- 'executed', 'selected_alt', 'corrected'
    p_domain_hint TEXT DEFAULT NULL
) RETURNS BIGINT AS $$
DECLARE
    v_normalized TEXT;
    v_fingerprint TEXT;
    v_word_count INT;
    v_stopword_ratio REAL;
    v_id BIGINT;
BEGIN
    -- Normalize phrase
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));
    v_fingerprint := md5(v_normalized || '|' || p_verb);

    -- Quality gate: word count (3-15 words)
    v_word_count := array_length(string_to_array(v_normalized, ' '), 1);
    IF v_word_count IS NULL OR v_word_count < 3 OR v_word_count > 15 THEN
        RETURN NULL;  -- Reject too short or too long
    END IF;

    -- Quality gate: stopword ratio (reject if >70% stopwords)
    SELECT
        COALESCE(COUNT(*) FILTER (WHERE s.word IS NOT NULL)::real / NULLIF(v_word_count, 0), 0)
    INTO v_stopword_ratio
    FROM unnest(string_to_array(v_normalized, ' ')) AS w(word)
    LEFT JOIN agent.stopwords s ON s.word = w.word;

    IF v_stopword_ratio > 0.70 THEN
        RETURN NULL;  -- Reject (too generic)
    END IF;

    -- Upsert candidate
    INSERT INTO agent.learning_candidates (
        fingerprint,
        learning_type,
        input_pattern,
        suggested_output,
        occurrence_count,
        success_count,
        total_count,
        first_seen,
        last_seen,
        last_success_at,
        domain_hint,
        status
    ) VALUES (
        v_fingerprint,
        'invocation_phrase',
        v_normalized,
        p_verb,
        1,
        CASE WHEN p_is_success THEN 1 ELSE 0 END,
        1,
        NOW(),
        NOW(),
        CASE WHEN p_is_success THEN NOW() ELSE NULL END,
        COALESCE(p_domain_hint, (
            SELECT category FROM "ob-poc".dsl_verbs WHERE full_name = p_verb
        )),
        'pending'
    )
    ON CONFLICT (fingerprint) DO UPDATE SET
        occurrence_count = agent.learning_candidates.occurrence_count + 1,
        success_count = agent.learning_candidates.success_count +
            CASE WHEN p_is_success THEN 1 ELSE 0 END,
        total_count = agent.learning_candidates.total_count + 1,
        last_seen = NOW(),
        last_success_at = CASE
            WHEN p_is_success THEN NOW()
            ELSE agent.learning_candidates.last_success_at
        END,
        -- Reset collision check if we get new signals (may have changed)
        collision_safe = NULL,
        collision_check_at = NULL
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.record_learning_signal IS
    'Record a learning signal with quality gates. Returns NULL if phrase rejected.';

-- ============================================================================
-- 4. Function: Check collision with existing patterns (basic, semantic done in Rust)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.check_pattern_collision_basic(
    p_candidate_id BIGINT
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
BEGIN
    -- Get candidate details
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id;

    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Check if phrase already exists for this verb (exact match)
    IF EXISTS (
        SELECT 1 FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = v_verb
          AND pattern_normalized = v_phrase
    ) THEN
        -- Already a pattern, mark as duplicate
        UPDATE agent.learning_candidates
        SET status = 'duplicate',
            collision_safe = FALSE,
            collision_check_at = NOW()
        WHERE id = p_candidate_id;
        RETURN FALSE;
    END IF;

    -- Basic check passed; semantic check done in Rust
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 5. Function: Get promotable candidates
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_promotable_candidates(
    p_min_occurrences INT DEFAULT 5,
    p_min_success_rate REAL DEFAULT 0.80,
    p_min_age_hours INT DEFAULT 24,
    p_limit INT DEFAULT 50
)
RETURNS TABLE (
    id BIGINT,
    phrase TEXT,
    verb TEXT,
    occurrence_count INT,
    success_count INT,
    total_count INT,
    success_rate REAL,
    domain_hint TEXT,
    first_seen TIMESTAMPTZ,
    age_hours REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        lc.id,
        lc.input_pattern as phrase,
        lc.suggested_output as verb,
        lc.occurrence_count,
        lc.success_count,
        lc.total_count,
        (lc.success_count::real / NULLIF(lc.total_count, 0))::real as success_rate,
        lc.domain_hint,
        lc.first_seen,
        (EXTRACT(EPOCH FROM (NOW() - lc.first_seen)) / 3600)::real as age_hours
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.learning_type = 'invocation_phrase'
      -- Occurrence threshold
      AND lc.occurrence_count >= p_min_occurrences
      -- Success rate threshold (avoid division by zero)
      AND lc.total_count > 0
      AND (lc.success_count::real / lc.total_count) >= p_min_success_rate
      -- Age threshold (cool-down)
      AND lc.first_seen < NOW() - make_interval(hours => p_min_age_hours)
      -- Collision check passed (or not yet checked - Rust will check)
      AND (lc.collision_safe IS NULL OR lc.collision_safe = TRUE)
      -- Not blocklisted
      AND NOT EXISTS (
          SELECT 1 FROM agent.phrase_blocklist bl
          WHERE bl.blocked_verb = lc.suggested_output
            AND lower(bl.phrase) = lc.input_pattern
            AND (bl.expires_at IS NULL OR bl.expires_at > NOW())
      )
    ORDER BY
        lc.occurrence_count DESC,
        (lc.success_count::real / NULLIF(lc.total_count, 0)) DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.get_promotable_candidates IS
    'Get candidates ready for automatic promotion (meet all quality thresholds)';

-- ============================================================================
-- 6. Function: Get candidates needing manual review
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_review_candidates(
    p_min_occurrences INT DEFAULT 3,
    p_min_age_days INT DEFAULT 7,
    p_limit INT DEFAULT 100
)
RETURNS TABLE (
    id BIGINT,
    phrase TEXT,
    verb TEXT,
    occurrence_count INT,
    success_count INT,
    total_count INT,
    success_rate REAL,
    domain_hint TEXT,
    first_seen TIMESTAMPTZ,
    last_seen TIMESTAMPTZ,
    collision_verb TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        lc.id,
        lc.input_pattern as phrase,
        lc.suggested_output as verb,
        lc.occurrence_count,
        lc.success_count,
        lc.total_count,
        CASE WHEN lc.total_count > 0
             THEN (lc.success_count::real / lc.total_count)
             ELSE 0 END as success_rate,
        lc.domain_hint,
        lc.first_seen,
        lc.last_seen,
        lc.collision_verb
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.learning_type = 'invocation_phrase'
      AND lc.occurrence_count >= p_min_occurrences
      AND lc.first_seen < NOW() - make_interval(days => p_min_age_days)
      -- Either failed auto-promotion criteria or collision detected
      AND (
          -- Low success rate
          (lc.total_count > 0 AND (lc.success_count::real / lc.total_count) < 0.80)
          -- Collision detected
          OR lc.collision_safe = FALSE
          -- Not enough occurrences for auto-promote but old enough for review
          OR lc.occurrence_count < 5
      )
    ORDER BY lc.occurrence_count DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.get_review_candidates IS
    'Get candidates that need manual review (failed auto-promotion but have signal)';

-- ============================================================================
-- 7. Function: Apply promotion (with audit)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.apply_promotion(
    p_candidate_id BIGINT,
    p_actor TEXT DEFAULT 'system_auto'
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
    v_added BOOLEAN;
BEGIN
    -- Get candidate
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id
      AND status = 'pending';

    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Add to dsl_verbs.intent_patterns using existing function
    SELECT "ob-poc".add_learned_pattern(v_verb, v_phrase) INTO v_added;

    IF v_added THEN
        -- Update candidate status
        UPDATE agent.learning_candidates
        SET status = 'applied',
            applied_at = NOW()
        WHERE id = p_candidate_id;

        -- Audit log
        INSERT INTO agent.learning_audit (
            action, learning_type, candidate_id, actor, details
        ) VALUES (
            'applied',
            'invocation_phrase',
            p_candidate_id,
            p_actor,
            jsonb_build_object(
                'phrase', v_phrase,
                'verb', v_verb
            )
        );

        RETURN TRUE;
    END IF;

    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.apply_promotion IS
    'Promote a learning candidate to dsl_verbs.intent_patterns with audit trail';

-- ============================================================================
-- 8. Function: Reject a candidate (add to blocklist)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.reject_candidate(
    p_candidate_id BIGINT,
    p_reason TEXT,
    p_actor TEXT DEFAULT 'manual_review'
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
BEGIN
    -- Get candidate
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id
      AND status = 'pending';

    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Add to blocklist
    INSERT INTO agent.phrase_blocklist (phrase, blocked_verb, reason)
    VALUES (v_phrase, v_verb, p_reason)
    ON CONFLICT DO NOTHING;

    -- Update candidate status
    UPDATE agent.learning_candidates
    SET status = 'rejected',
        reviewed_by = p_actor,
        reviewed_at = NOW()
    WHERE id = p_candidate_id;

    -- Audit log
    INSERT INTO agent.learning_audit (
        action, learning_type, candidate_id, actor, details
    ) VALUES (
        'rejected',
        'invocation_phrase',
        p_candidate_id,
        p_actor,
        jsonb_build_object(
            'phrase', v_phrase,
            'verb', v_verb,
            'reason', p_reason
        )
    );

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.reject_candidate IS
    'Reject a learning candidate and add to blocklist';

-- ============================================================================
-- 9. Function: Expire pending outcomes
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.expire_pending_outcomes(
    p_older_than_minutes INT DEFAULT 30
) RETURNS INT AS $$
DECLARE
    v_count INT;
BEGIN
    UPDATE "ob-poc".intent_feedback
    SET outcome = 'abandoned'
    WHERE outcome IS NULL
      AND created_at < NOW() - make_interval(mins => p_older_than_minutes);

    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION agent.expire_pending_outcomes IS
    'Mark stale pending outcomes as abandoned (no user action after N minutes)';

-- ============================================================================
-- 10. Metrics views
-- ============================================================================

-- Weekly learning health dashboard
CREATE OR REPLACE VIEW agent.v_learning_health_weekly AS
SELECT
    DATE_TRUNC('week', f.created_at) as week,
    COUNT(*) as total_interactions,
    COUNT(*) FILTER (WHERE v.learning_signal = 'success') as successes,
    COUNT(*) FILTER (WHERE v.learning_signal IN ('wrong_match', 'correction_needed')) as corrections,
    COUNT(*) FILTER (WHERE v.learning_signal = 'no_match') as no_matches,
    COUNT(*) FILTER (WHERE v.learning_signal = 'false_positive') as false_positives,

    -- Hit rates
    ROUND(100.0 * COUNT(*) FILTER (WHERE v.learning_signal = 'success') /
          NULLIF(COUNT(*), 0), 1) as top1_hit_rate_pct,

    -- Scores
    ROUND(AVG(f.match_score) FILTER (WHERE v.learning_signal = 'success')::numeric, 3) as avg_success_score,
    ROUND(AVG(f.match_score) FILTER (WHERE v.learning_signal IN ('wrong_match', 'correction_needed'))::numeric, 3) as avg_correction_score,

    -- Confidence distribution
    COUNT(*) FILTER (WHERE f.match_confidence = 'high') as high_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'medium') as medium_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'low') as low_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'none') as no_match_confidence

FROM "ob-poc".intent_feedback f
LEFT JOIN "ob-poc".v_learning_feedback v ON v.feedback_id = f.id
WHERE f.created_at > NOW() - INTERVAL '12 weeks'
GROUP BY 1
ORDER BY 1 DESC;

COMMENT ON VIEW agent.v_learning_health_weekly IS
    'Weekly metrics for learning pipeline health monitoring';

-- Candidate pipeline status summary
CREATE OR REPLACE VIEW agent.v_candidate_pipeline AS
SELECT
    status,
    COUNT(*) as count,
    ROUND(AVG(occurrence_count)::numeric, 1) as avg_occurrences,
    ROUND(AVG(CASE WHEN total_count > 0
              THEN success_count::real / total_count
              ELSE 0 END)::numeric, 2) as avg_success_rate,
    MIN(first_seen) as oldest,
    MAX(last_seen) as newest
FROM agent.learning_candidates
WHERE learning_type = 'invocation_phrase'
GROUP BY status
ORDER BY count DESC;

COMMENT ON VIEW agent.v_candidate_pipeline IS
    'Summary of candidate statuses in the learning pipeline';

-- Top pending candidates (for quick review)
CREATE OR REPLACE VIEW agent.v_top_pending_candidates AS
SELECT
    id,
    input_pattern as phrase,
    suggested_output as verb,
    occurrence_count,
    success_count,
    total_count,
    CASE WHEN total_count > 0
         THEN ROUND((success_count::real / total_count)::numeric, 2)
         ELSE 0 END as success_rate,
    domain_hint,
    collision_safe,
    collision_verb,
    first_seen,
    last_seen,
    EXTRACT(DAY FROM (NOW() - first_seen))::int as age_days
FROM agent.learning_candidates
WHERE status = 'pending'
  AND learning_type = 'invocation_phrase'
ORDER BY occurrence_count DESC, success_rate DESC
LIMIT 100;

COMMENT ON VIEW agent.v_top_pending_candidates IS
    'Top 100 pending candidates by occurrence count';

-- ============================================================================
-- 11. Indexes for performance
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_learning_candidates_promotable
ON agent.learning_candidates(occurrence_count DESC, status)
WHERE status = 'pending' AND learning_type = 'invocation_phrase';

CREATE INDEX IF NOT EXISTS idx_learning_candidates_fingerprint
ON agent.learning_candidates(fingerprint);

CREATE INDEX IF NOT EXISTS idx_learning_candidates_success_tracking
ON agent.learning_candidates(total_count, success_count)
WHERE status = 'pending';

-- ============================================================================
-- 12. Grant permissions (if using role-based access)
-- ============================================================================

-- Ensure functions are accessible
GRANT EXECUTE ON FUNCTION agent.record_learning_signal TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.get_promotable_candidates TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.get_review_candidates TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.apply_promotion TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.reject_candidate TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.expire_pending_outcomes TO PUBLIC;
GRANT EXECUTE ON FUNCTION agent.check_pattern_collision_basic TO PUBLIC;

-- Done!
