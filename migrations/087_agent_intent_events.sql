-- Migration 087: agent.intent_events telemetry table
-- Append-only telemetry for the single orchestrator pipeline.
-- PII-safe: stores utterance hash + optional redacted preview, not raw input.

CREATE SCHEMA IF NOT EXISTS agent;

CREATE TABLE IF NOT EXISTS agent.intent_events (
  event_id            uuid PRIMARY KEY,
  ts                  timestamptz NOT NULL DEFAULT now(),

  session_id          uuid NOT NULL,
  actor_id            text NOT NULL,
  entrypoint          text NOT NULL,        -- chat|mcp|repl

  utterance_hash      text NOT NULL,        -- sha256(normalized)
  utterance_preview   text NULL,            -- redacted/trimmed (optional, max 80 chars)
  scope               text NULL,

  subject_ref_type    text NULL,            -- entity|case|none
  subject_ref_id      uuid NULL,

  semreg_mode         text NOT NULL,        -- strict|permissive|fail_open
  semreg_denied_verbs jsonb NULL,

  verb_candidates_pre  jsonb NULL,          -- top N pre-SemReg [(verb, score), ...]
  verb_candidates_post jsonb NULL,          -- post-SemReg filter

  chosen_verb_fqn     text NULL,
  selection_source    text NULL,            -- discovery|user_choice|semreg|macro
  forced_verb_fqn     text NULL,

  outcome             text NOT NULL,        -- ready|needs_clarification|no_match|no_allowed_verbs|scope_resolved|direct_dsl_denied|macro_expanded|error
  dsl_hash            text NULL,
  run_sheet_entry_id  uuid NULL,

  macro_semreg_checked bool NOT NULL DEFAULT false,
  macro_denied_verbs   jsonb NULL,

  prompt_version      text NULL,
  error_code          text NULL
);

CREATE INDEX IF NOT EXISTS intent_events_ts_idx ON agent.intent_events(ts);
CREATE INDEX IF NOT EXISTS intent_events_session_idx ON agent.intent_events(session_id, ts);
CREATE INDEX IF NOT EXISTS intent_events_utter_hash_idx ON agent.intent_events(utterance_hash);
CREATE INDEX IF NOT EXISTS intent_events_chosen_verb_idx ON agent.intent_events(chosen_verb_fqn);

-- Review views

CREATE OR REPLACE VIEW agent.v_intent_top_clarify_verbs AS
SELECT
  chosen_verb_fqn,
  count(*) AS clarify_count,
  min(ts) AS first_seen,
  max(ts) AS last_seen
FROM agent.intent_events
WHERE outcome = 'needs_clarification'
  AND chosen_verb_fqn IS NOT NULL
GROUP BY chosen_verb_fqn
ORDER BY clarify_count DESC
LIMIT 50;

CREATE OR REPLACE VIEW agent.v_intent_semreg_overrides AS
SELECT
  event_id, ts, session_id,
  chosen_verb_fqn, forced_verb_fqn, selection_source, semreg_mode
FROM agent.intent_events
WHERE forced_verb_fqn IS NOT NULL
  AND selection_source = 'semreg'
ORDER BY ts DESC
LIMIT 100;

CREATE OR REPLACE VIEW agent.v_intent_semreg_denies AS
SELECT
  jsonb_array_elements_text(semreg_denied_verbs) AS denied_verb,
  count(*) AS deny_count,
  max(ts) AS last_denied
FROM agent.intent_events
WHERE semreg_denied_verbs IS NOT NULL
  AND jsonb_array_length(semreg_denied_verbs) > 0
GROUP BY denied_verb
ORDER BY deny_count DESC
LIMIT 50;

CREATE OR REPLACE VIEW agent.v_intent_macro_denies AS
SELECT
  jsonb_array_elements_text(macro_denied_verbs) AS denied_verb,
  count(*) AS deny_count,
  max(ts) AS last_denied
FROM agent.intent_events
WHERE macro_denied_verbs IS NOT NULL
  AND jsonb_array_length(macro_denied_verbs) > 0
GROUP BY denied_verb
ORDER BY deny_count DESC
LIMIT 50;

CREATE OR REPLACE VIEW agent.v_intent_failure_modes AS
SELECT
  outcome,
  error_code,
  count(*) AS event_count,
  min(ts) AS first_seen,
  max(ts) AS last_seen
FROM agent.intent_events
WHERE outcome NOT IN ('ready', 'scope_resolved', 'macro_expanded')
GROUP BY outcome, error_code
ORDER BY event_count DESC
LIMIT 50;
