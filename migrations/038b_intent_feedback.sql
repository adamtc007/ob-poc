-- Intent feedback capture for ML continuous learning
-- Append-only: no updates (except outcome), batch analysis only

-- ============================================================================
-- Main feedback capture table
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".intent_feedback (
    id BIGSERIAL PRIMARY KEY,

    -- Session context
    session_id UUID NOT NULL,
    interaction_id UUID NOT NULL DEFAULT gen_random_uuid(),

    -- User input (sanitized - no PII/client names)
    user_input TEXT NOT NULL,
    user_input_hash TEXT NOT NULL,  -- For dedup without storing raw text long-term
    input_source TEXT NOT NULL DEFAULT 'chat',  -- 'chat', 'voice', 'command'

    -- Match result
    matched_verb TEXT,
    match_score REAL,
    match_confidence TEXT,  -- 'high', 'medium', 'low', 'none'
    semantic_score REAL,
    phonetic_score REAL,
    alternatives JSONB,  -- Top-5 alternatives: [{"verb": "...", "score": 0.72}, ...]

    -- Outcome (updated when user action is known)
    outcome TEXT,  -- 'executed', 'selected_alt', 'corrected', 'rephrased', 'abandoned', NULL (pending)
    outcome_verb TEXT,  -- The verb that was actually executed (may differ from matched_verb)
    correction_input TEXT,  -- If user rephrased, what did they say?
    time_to_outcome_ms INTEGER,  -- How long between match and outcome

    -- Context at time of interaction
    graph_context TEXT,
    workflow_phase TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_source CHECK (input_source IN ('chat', 'voice', 'command')),
    CONSTRAINT valid_confidence CHECK (match_confidence IN ('high', 'medium', 'low', 'none') OR match_confidence IS NULL),
    CONSTRAINT valid_outcome CHECK (outcome IN ('executed', 'selected_alt', 'corrected', 'rephrased', 'abandoned') OR outcome IS NULL)
);

-- Indexes for batch analysis queries
CREATE INDEX IF NOT EXISTS idx_feedback_created ON "ob-poc".intent_feedback(created_at);
CREATE INDEX IF NOT EXISTS idx_feedback_outcome ON "ob-poc".intent_feedback(outcome) WHERE outcome IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_feedback_verb ON "ob-poc".intent_feedback(matched_verb) WHERE matched_verb IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_feedback_session ON "ob-poc".intent_feedback(session_id);
CREATE INDEX IF NOT EXISTS idx_feedback_input_hash ON "ob-poc".intent_feedback(user_input_hash);
CREATE INDEX IF NOT EXISTS idx_feedback_confidence ON "ob-poc".intent_feedback(match_confidence);

-- Partial index for pending outcomes (need to be resolved)
CREATE INDEX IF NOT EXISTS idx_feedback_pending ON "ob-poc".intent_feedback(interaction_id)
WHERE outcome IS NULL;

COMMENT ON TABLE "ob-poc".intent_feedback IS
'ML feedback capture for intent matching continuous learning. Append-only, batch analysis.';

-- ============================================================================
-- Analysis summary table (materialized results from batch jobs)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".intent_feedback_analysis (
    id SERIAL PRIMARY KEY,
    analysis_type TEXT NOT NULL,  -- 'pattern_discovery', 'confusion_pair', 'gap', 'low_score_success'
    analysis_date DATE NOT NULL DEFAULT CURRENT_DATE,

    -- Analysis payload
    data JSONB NOT NULL,

    -- Status
    reviewed BOOLEAN DEFAULT FALSE,
    applied BOOLEAN DEFAULT FALSE,
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(analysis_type, analysis_date, data)
);

CREATE INDEX IF NOT EXISTS idx_analysis_type_date ON "ob-poc".intent_feedback_analysis(analysis_type, analysis_date);
CREATE INDEX IF NOT EXISTS idx_analysis_pending ON "ob-poc".intent_feedback_analysis(reviewed) WHERE NOT reviewed;

COMMENT ON TABLE "ob-poc".intent_feedback_analysis IS
'Materialized analysis results from batch feedback analysis. Reviewed by humans, applied to patterns.';
