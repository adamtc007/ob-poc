-- Agent Learning Infrastructure
-- Enables continuous improvement from user interactions

-- Schema for agent learning
CREATE SCHEMA IF NOT EXISTS agent;

-- =============================================================================
-- LEARNED ENTITY ALIASES
-- =============================================================================
-- When users refer to entities by non-canonical names, learn the mapping
-- e.g., "Barclays" → "Barclays PLC", "DB" → "Deutsche Bank AG"

CREATE TABLE agent.entity_aliases (
    id              BIGSERIAL PRIMARY KEY,
    alias           TEXT NOT NULL,                    -- User's term ("Barclays")
    canonical_name  TEXT NOT NULL,                    -- System name ("Barclays PLC")
    entity_id       UUID REFERENCES "ob-poc".entities(entity_id),
    confidence      DECIMAL(3,2) DEFAULT 1.0,         -- 0.00-1.00
    occurrence_count INT DEFAULT 1,                   -- Times seen
    source          TEXT DEFAULT 'user_correction',  -- user_correction, threshold_auto, manual
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(alias, canonical_name)
);

CREATE INDEX idx_entity_aliases_alias ON agent.entity_aliases(LOWER(alias));
CREATE INDEX idx_entity_aliases_entity ON agent.entity_aliases(entity_id);

-- =============================================================================
-- LEARNED LEXICON TOKENS
-- =============================================================================
-- New vocabulary learned from user inputs
-- e.g., "counterparty" → EntityType, "ISDA" → ProductType

CREATE TABLE agent.lexicon_tokens (
    id              BIGSERIAL PRIMARY KEY,
    token           TEXT NOT NULL,                    -- The word/phrase
    token_type      TEXT NOT NULL,                    -- Verb, Entity, Prep, etc.
    token_subtype   TEXT,                             -- More specific classification
    occurrence_count INT DEFAULT 1,
    confidence      DECIMAL(3,2) DEFAULT 1.0,
    source          TEXT DEFAULT 'user_correction',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(token, token_type)
);

CREATE INDEX idx_lexicon_tokens_token ON agent.lexicon_tokens(LOWER(token));

-- =============================================================================
-- LEARNED INVOCATION PHRASES
-- =============================================================================
-- Natural language phrases that map to DSL verbs
-- e.g., "set up an ISDA" → isda.create

CREATE TABLE agent.invocation_phrases (
    id              BIGSERIAL PRIMARY KEY,
    phrase          TEXT NOT NULL,                    -- User's phrase
    verb            TEXT NOT NULL,                    -- DSL verb (domain.verb)
    confidence      DECIMAL(3,2) DEFAULT 1.0,
    occurrence_count INT DEFAULT 1,
    source          TEXT DEFAULT 'user_correction',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(phrase, verb)
);

CREATE INDEX idx_invocation_phrases_phrase ON agent.invocation_phrases
    USING gin(to_tsvector('english', phrase));

-- =============================================================================
-- AGENT EVENTS (for analysis, not hot path)
-- =============================================================================
-- Captures intent resolution flow for learning analysis

CREATE TABLE agent.events (
    id              BIGSERIAL PRIMARY KEY,
    session_id      UUID,
    timestamp       TIMESTAMPTZ DEFAULT NOW(),
    event_type      TEXT NOT NULL,                    -- prompt_sent, intent_extracted, etc.

    -- Event-specific payload
    user_message    TEXT,                             -- Original user input
    parsed_intents  JSONB,                            -- Extracted intents
    selected_verb   TEXT,                             -- Chosen DSL verb
    generated_dsl   TEXT,                             -- DSL output

    -- Correction tracking
    was_corrected   BOOLEAN DEFAULT FALSE,
    corrected_dsl   TEXT,                             -- User's correction
    correction_type TEXT,                             -- verb_change, entity_change, arg_change

    -- Resolution details
    entities_resolved JSONB,                          -- Entity resolution attempts
    resolution_failures JSONB,                        -- What failed to resolve

    -- Outcome
    execution_success BOOLEAN,
    error_message   TEXT,

    -- Metadata
    duration_ms     INT,
    llm_model       TEXT,
    llm_tokens_used INT
);

CREATE INDEX idx_agent_events_session ON agent.events(session_id);
CREATE INDEX idx_agent_events_timestamp ON agent.events(timestamp);
CREATE INDEX idx_agent_events_type ON agent.events(event_type);
CREATE INDEX idx_agent_events_corrected ON agent.events(was_corrected) WHERE was_corrected = TRUE;
CREATE INDEX idx_agent_events_verb ON agent.events(selected_verb);

-- =============================================================================
-- LEARNING CANDIDATES (queued for review or auto-apply)
-- =============================================================================

CREATE TABLE agent.learning_candidates (
    id              BIGSERIAL PRIMARY KEY,
    fingerprint     TEXT NOT NULL UNIQUE,             -- Dedup key
    learning_type   TEXT NOT NULL,                    -- entity_alias, lexicon_token, invocation_phrase, prompt_change

    -- What to learn
    input_pattern   TEXT NOT NULL,                    -- What user said
    suggested_output TEXT NOT NULL,                   -- What we should map to

    -- Evidence
    occurrence_count INT DEFAULT 1,
    first_seen      TIMESTAMPTZ DEFAULT NOW(),
    last_seen       TIMESTAMPTZ DEFAULT NOW(),
    example_events  BIGINT[],                         -- References to agent.events

    -- Risk assessment
    risk_level      TEXT DEFAULT 'low',               -- low, medium, high
    auto_applicable BOOLEAN DEFAULT FALSE,

    -- Status
    status          TEXT DEFAULT 'pending',           -- pending, approved, rejected, applied
    reviewed_by     TEXT,
    reviewed_at     TIMESTAMPTZ,
    applied_at      TIMESTAMPTZ,

    -- Metadata
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_learning_candidates_status ON agent.learning_candidates(status);
CREATE INDEX idx_learning_candidates_type ON agent.learning_candidates(learning_type);
CREATE INDEX idx_learning_candidates_auto ON agent.learning_candidates(auto_applicable)
    WHERE auto_applicable = TRUE AND status = 'pending';

-- =============================================================================
-- LEARNING AUDIT LOG
-- =============================================================================

CREATE TABLE agent.learning_audit (
    id              BIGSERIAL PRIMARY KEY,
    timestamp       TIMESTAMPTZ DEFAULT NOW(),
    action          TEXT NOT NULL,                    -- applied, rejected, reverted
    learning_type   TEXT NOT NULL,
    learning_id     BIGINT,                           -- Reference to applied learning
    candidate_id    BIGINT REFERENCES agent.learning_candidates(id),
    actor           TEXT NOT NULL,                    -- system_auto, system_threshold, user:xxx
    details         JSONB,

    -- For rollback
    previous_state  JSONB,
    can_rollback    BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_learning_audit_timestamp ON agent.learning_audit(timestamp);
CREATE INDEX idx_learning_audit_type ON agent.learning_audit(learning_type);

-- =============================================================================
-- HELPER FUNCTIONS
-- =============================================================================

-- Increment occurrence count or insert new alias
CREATE OR REPLACE FUNCTION agent.upsert_entity_alias(
    p_alias TEXT,
    p_canonical_name TEXT,
    p_entity_id UUID DEFAULT NULL,
    p_source TEXT DEFAULT 'user_correction'
) RETURNS BIGINT AS $$
DECLARE
    v_id BIGINT;
BEGIN
    INSERT INTO agent.entity_aliases (alias, canonical_name, entity_id, source)
    VALUES (LOWER(TRIM(p_alias)), p_canonical_name, p_entity_id, p_source)
    ON CONFLICT (alias, canonical_name) DO UPDATE SET
        occurrence_count = agent.entity_aliases.occurrence_count + 1,
        updated_at = NOW()
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Increment occurrence count or insert new lexicon token
CREATE OR REPLACE FUNCTION agent.upsert_lexicon_token(
    p_token TEXT,
    p_token_type TEXT,
    p_token_subtype TEXT DEFAULT NULL,
    p_source TEXT DEFAULT 'user_correction'
) RETURNS BIGINT AS $$
DECLARE
    v_id BIGINT;
BEGIN
    INSERT INTO agent.lexicon_tokens (token, token_type, token_subtype, source)
    VALUES (LOWER(TRIM(p_token)), p_token_type, p_token_subtype, p_source)
    ON CONFLICT (token, token_type) DO UPDATE SET
        occurrence_count = agent.lexicon_tokens.occurrence_count + 1,
        updated_at = NOW()
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Get learning candidates ready for auto-apply (3+ occurrences, low risk)
CREATE OR REPLACE FUNCTION agent.get_auto_applicable_candidates()
RETURNS TABLE (
    id BIGINT,
    learning_type TEXT,
    input_pattern TEXT,
    suggested_output TEXT,
    occurrence_count INT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        lc.id,
        lc.learning_type,
        lc.input_pattern,
        lc.suggested_output,
        lc.occurrence_count
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.auto_applicable = TRUE
      AND lc.occurrence_count >= 3
      AND lc.risk_level = 'low'
    ORDER BY lc.occurrence_count DESC;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE agent.entity_aliases IS 'Learned mappings from user terms to canonical entity names';
COMMENT ON TABLE agent.lexicon_tokens IS 'Vocabulary learned from user inputs for intent parsing';
COMMENT ON TABLE agent.invocation_phrases IS 'Natural language phrases mapped to DSL verbs';
COMMENT ON TABLE agent.events IS 'Agent interaction events for learning analysis';
COMMENT ON TABLE agent.learning_candidates IS 'Pending learnings awaiting approval or auto-apply';
COMMENT ON TABLE agent.learning_audit IS 'Audit trail of all learning applications and reversions';

COMMENT ON SCHEMA agent IS 'Continuous learning infrastructure for agent intent resolution';
