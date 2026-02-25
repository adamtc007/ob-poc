-- Migration 093: Bootstrap audit table for idempotent seed operations.
-- Tracks in-progress and completed bootstrap runs to prevent duplicate seeding.

CREATE TABLE IF NOT EXISTS sem_reg.bootstrap_audit (
    bundle_hash        TEXT PRIMARY KEY,
    origin_actor_id    TEXT NOT NULL,
    bundle_counts      JSONB NOT NULL,
    snapshot_set_id    UUID,
    status             TEXT NOT NULL CHECK (status IN ('in_progress','published','failed')),
    started_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at       TIMESTAMPTZ,
    error              TEXT
);
