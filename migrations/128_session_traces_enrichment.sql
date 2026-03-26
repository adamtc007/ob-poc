-- Migration 128: Add enrichment columns to session_traces for round-trip persistence.

ALTER TABLE "ob-poc".session_traces
    ADD COLUMN IF NOT EXISTS verb_resolved TEXT,
    ADD COLUMN IF NOT EXISTS execution_result JSONB;
