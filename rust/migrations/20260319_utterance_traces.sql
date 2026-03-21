CREATE TABLE IF NOT EXISTS "ob-poc".utterance_traces (
    trace_id                 UUID PRIMARY KEY,
    utterance_id             UUID NOT NULL,
    session_id               UUID NOT NULL,
    correlation_id           UUID NULL,
    trace_kind               TEXT NOT NULL,
    parent_trace_id          UUID NULL REFERENCES "ob-poc".utterance_traces(trace_id),
    timestamp                TIMESTAMPTZ NOT NULL DEFAULT now(),
    raw_utterance            TEXT NOT NULL,
    outcome                  TEXT NOT NULL,
    halt_reason_code         TEXT NULL,
    halt_phase               SMALLINT NULL,
    resolved_verb            TEXT NULL,
    plane                    TEXT NULL,
    polarity                 TEXT NULL,
    execution_shape_kind     TEXT NULL,
    fallback_invoked         BOOLEAN NOT NULL DEFAULT false,
    situation_signature_hash BIGINT NULL,
    template_id              TEXT NULL,
    template_version         TEXT NULL,
    surface_versions         JSONB NOT NULL DEFAULT '{}'::jsonb,
    trace_payload            JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT utterance_traces_trace_kind_check
        CHECK (
            trace_kind IN (
                'original',
                'clarification_prompt',
                'clarification_response',
                'resumed_execution'
            )
        ),
    CONSTRAINT utterance_traces_outcome_check
        CHECK (
            outcome IN (
                'in_progress',
                'executed_successfully',
                'executed_with_correction',
                'halted_at_phase',
                'clarification_triggered',
                'no_match'
            )
        )
);

CREATE INDEX IF NOT EXISTS idx_utterance_traces_session_ts
    ON "ob-poc".utterance_traces (session_id, timestamp);

CREATE INDEX IF NOT EXISTS idx_utterance_traces_parent
    ON "ob-poc".utterance_traces (parent_trace_id)
    WHERE parent_trace_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_utterance_traces_outcome
    ON "ob-poc".utterance_traces (outcome);

CREATE INDEX IF NOT EXISTS idx_utterance_traces_resolved_verb
    ON "ob-poc".utterance_traces (resolved_verb)
    WHERE resolved_verb IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_utterance_traces_fallback
    ON "ob-poc".utterance_traces (timestamp)
    WHERE fallback_invoked = true;

CREATE INDEX IF NOT EXISTS idx_utterance_traces_signature
    ON "ob-poc".utterance_traces (situation_signature_hash)
    WHERE situation_signature_hash IS NOT NULL;
