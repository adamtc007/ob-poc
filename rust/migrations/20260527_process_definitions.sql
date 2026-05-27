-- Process definitions registry.
-- Stores bpmn-lite DSL source keyed by process name.
-- Loaded at startup by ProcessRegistry::load_all; compiled into RuntimeEngine
-- instances keyed by name. Served by workflow.start-process verb.

CREATE TABLE process_definitions (
    name        TEXT PRIMARY KEY,
    dsl_source  TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Add payload column to pending waits so form_data is a point-read on the row,
-- not a journey-log replay. The column is untyped and wait-kind-specific; future
-- timer/message waits share this slot.
ALTER TABLE dsl_pending_wait ADD COLUMN payload JSONB;
