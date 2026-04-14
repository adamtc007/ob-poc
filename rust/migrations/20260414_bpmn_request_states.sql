CREATE TABLE IF NOT EXISTS "ob-poc".bpmn_request_states (
    request_key TEXT PRIMARY KEY,
    correlation_key TEXT NOT NULL UNIQUE,
    session_id UUID NOT NULL,
    runbook_id UUID NOT NULL,
    entry_id UUID NOT NULL,
    process_key TEXT NOT NULL,
    process_instance_id UUID,
    status TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    killed_at TIMESTAMPTZ,
    last_error TEXT,
    CONSTRAINT bpmn_request_states_status_check CHECK (
        status = ANY (ARRAY[
            'requested'::text,
            'dispatch_pending'::text,
            'in_progress'::text,
            'returned'::text,
            'killed'::text,
            'failed'::text
        ])
    )
);

CREATE INDEX IF NOT EXISTS idx_bpmn_request_states_status
    ON "ob-poc".bpmn_request_states (status);

CREATE INDEX IF NOT EXISTS idx_bpmn_request_states_entry
    ON "ob-poc".bpmn_request_states (entry_id, requested_at DESC);
