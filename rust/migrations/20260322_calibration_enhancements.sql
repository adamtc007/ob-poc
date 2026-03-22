CREATE TABLE IF NOT EXISTS "ob-poc".calibration_fixture_transitions (
    transition_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES "ob-poc".calibration_runs(run_id),
    utterance_id UUID NOT NULL REFERENCES "ob-poc".calibration_utterances(utterance_id),
    trace_id UUID NOT NULL REFERENCES "ob-poc".utterance_traces(trace_id),
    fixture_state JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_cal_fixture_run
    ON "ob-poc".calibration_fixture_transitions (run_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_cal_fixture_trace
    ON "ob-poc".calibration_fixture_transitions (trace_id);
