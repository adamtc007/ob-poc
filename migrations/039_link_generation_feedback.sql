-- Migration 039: Link dsl_generation_log to intent_feedback for learning loop
--
-- Purpose: Enable the learning loop to correlate verb matches with DSL execution outcomes
--
-- Architecture:
--   intent_feedback: captures phrase → verb match (learning signal)
--   dsl_generation_log: captures LLM → DSL generation (audit trail)
--
--   This migration adds:
--   1. FK from dsl_generation_log → intent_feedback (optional link)
--   2. Execution outcome columns to dsl_generation_log
--   3. Index for learning queries that join both tables

-- ============================================================================
-- 1. Add intent_feedback_id FK to dsl_generation_log
-- ============================================================================

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS intent_feedback_id BIGINT;

-- FK constraint (nullable - not all DSL generations come from chat)
ALTER TABLE "ob-poc".dsl_generation_log
ADD CONSTRAINT fk_generation_log_feedback
FOREIGN KEY (intent_feedback_id)
REFERENCES "ob-poc".intent_feedback(id)
ON DELETE SET NULL;

COMMENT ON COLUMN "ob-poc".dsl_generation_log.intent_feedback_id IS
'Links to intent_feedback for learning loop. NULL for direct DSL execution without chat.';

-- ============================================================================
-- 2. Add execution outcome columns to dsl_generation_log
-- ============================================================================

-- Execution status enum
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'execution_status') THEN
        CREATE TYPE "ob-poc".execution_status AS ENUM (
            'pending',      -- DSL generated, not yet executed
            'executed',     -- Successfully executed
            'failed',       -- Execution error
            'cancelled',    -- User cancelled before execution
            'skipped'       -- Skipped (e.g., dependency failed)
        );
    END IF;
END
$$;

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS execution_status "ob-poc".execution_status DEFAULT 'pending';

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS execution_error TEXT;

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS executed_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".dsl_generation_log
ADD COLUMN IF NOT EXISTS affected_entity_ids UUID[];

COMMENT ON COLUMN "ob-poc".dsl_generation_log.execution_status IS
'Outcome of DSL execution: pending, executed, failed, cancelled, skipped';

COMMENT ON COLUMN "ob-poc".dsl_generation_log.execution_error IS
'Error message if execution_status = failed';

COMMENT ON COLUMN "ob-poc".dsl_generation_log.executed_at IS
'Timestamp when DSL was executed (NULL if pending/cancelled)';

COMMENT ON COLUMN "ob-poc".dsl_generation_log.affected_entity_ids IS
'Entity UUIDs created/modified by this DSL execution';

-- ============================================================================
-- 3. Indexes for learning queries
-- ============================================================================

-- Index for joining feedback to generation log
CREATE INDEX IF NOT EXISTS idx_generation_log_feedback
ON "ob-poc".dsl_generation_log(intent_feedback_id)
WHERE intent_feedback_id IS NOT NULL;

-- Index for execution status queries
CREATE INDEX IF NOT EXISTS idx_generation_log_exec_status
ON "ob-poc".dsl_generation_log(execution_status);

-- Index for failed executions (high-value learning signal)
CREATE INDEX IF NOT EXISTS idx_generation_log_failures
ON "ob-poc".dsl_generation_log(created_at)
WHERE execution_status = 'failed';

-- ============================================================================
-- 4. Learning analysis view
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_learning_feedback AS
SELECT
    f.id as feedback_id,
    f.interaction_id,
    f.session_id,
    f.user_input,
    f.input_source,
    f.matched_verb,
    f.match_score,
    f.match_confidence,
    f.alternatives,
    f.outcome as feedback_outcome,
    f.outcome_verb,
    f.created_at as feedback_at,

    g.log_id as generation_log_id,
    g.user_intent as generation_intent,
    g.final_valid_dsl,
    g.model_used,
    g.total_attempts,
    g.success as generation_success,
    g.execution_status,
    g.execution_error,
    g.executed_at,
    g.affected_entity_ids,
    g.total_latency_ms,
    g.total_input_tokens,
    g.total_output_tokens,

    -- Learning signals
    CASE
        WHEN f.outcome = 'executed' AND g.execution_status = 'executed' THEN 'success'
        WHEN f.outcome = 'executed' AND g.execution_status = 'failed' THEN 'false_positive'
        WHEN f.outcome = 'selected_alt' THEN 'wrong_match'
        WHEN f.outcome = 'corrected' THEN 'correction_needed'
        WHEN f.outcome = 'abandoned' THEN 'no_match'
        ELSE 'pending'
    END as learning_signal,

    -- Time metrics
    EXTRACT(EPOCH FROM (g.executed_at - f.created_at)) * 1000 as phrase_to_execution_ms

FROM "ob-poc".intent_feedback f
LEFT JOIN "ob-poc".dsl_generation_log g ON g.intent_feedback_id = f.id
ORDER BY f.created_at DESC;

COMMENT ON VIEW "ob-poc".v_learning_feedback IS
'Unified view joining feedback capture with DSL generation outcomes for learning analysis';

-- ============================================================================
-- 5. Learning summary stats view
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_learning_stats AS
SELECT
    DATE(created_at) as date,
    COUNT(*) as total_interactions,
    COUNT(*) FILTER (WHERE outcome = 'executed') as executed,
    COUNT(*) FILTER (WHERE outcome = 'selected_alt') as selected_alternative,
    COUNT(*) FILTER (WHERE outcome = 'corrected') as corrected,
    COUNT(*) FILTER (WHERE outcome = 'abandoned') as abandoned,
    COUNT(*) FILTER (WHERE outcome IS NULL) as pending,

    -- Match quality
    AVG(match_score) FILTER (WHERE outcome = 'executed') as avg_success_score,
    AVG(match_score) FILTER (WHERE outcome IN ('selected_alt', 'corrected')) as avg_failure_score,

    -- Confidence distribution
    COUNT(*) FILTER (WHERE match_confidence = 'high') as high_confidence,
    COUNT(*) FILTER (WHERE match_confidence = 'medium') as medium_confidence,
    COUNT(*) FILTER (WHERE match_confidence = 'low') as low_confidence,
    COUNT(*) FILTER (WHERE match_confidence = 'none') as no_match

FROM "ob-poc".intent_feedback
GROUP BY DATE(created_at)
ORDER BY date DESC;

COMMENT ON VIEW "ob-poc".v_learning_stats IS
'Daily learning statistics for monitoring feedback loop health';
