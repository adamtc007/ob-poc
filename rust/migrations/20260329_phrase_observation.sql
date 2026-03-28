-- Migration 20260329: Phrase observation infrastructure — watermark tracking for batch observation.

-- Watermark tracking for batch observation
CREATE TABLE IF NOT EXISTS "ob-poc".phrase_observation_state (
    id                          SERIAL PRIMARY KEY,
    last_observed_sequence      BIGINT NOT NULL DEFAULT 0,
    last_run_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    patterns_found              INT NOT NULL DEFAULT 0,
    wrong_match_patterns_found  INT NOT NULL DEFAULT 0,
    next_run_at                 TIMESTAMPTZ
);

-- Seed initial row
INSERT INTO "ob-poc".phrase_observation_state (id) VALUES (1) ON CONFLICT DO NOTHING;

COMMENT ON TABLE "ob-poc".phrase_observation_state IS 'Watermark state for phrase observation batch runs';
