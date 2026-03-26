-- Migration 127: Persist V2 navigation/runbook-plan session state.

ALTER TABLE "ob-poc".repl_sessions_v2
    ADD COLUMN IF NOT EXISTS extended_state JSONB NOT NULL DEFAULT '{}'::jsonb;
