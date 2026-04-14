ALTER TABLE "ob-poc".bpmn_pending_dispatches
    ADD COLUMN IF NOT EXISTS session_stack JSONB NOT NULL DEFAULT '{}';
