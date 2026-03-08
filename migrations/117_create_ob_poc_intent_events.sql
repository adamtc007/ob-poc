CREATE TABLE IF NOT EXISTS "ob-poc".intent_events (
  event_id            uuid PRIMARY KEY,
  ts                  timestamptz NOT NULL DEFAULT now(),
  session_id          uuid NOT NULL,
  actor_id            text NOT NULL,
  entrypoint          text NOT NULL,
  utterance_hash      text NOT NULL,
  utterance_preview   text NULL,
  scope               text NULL,
  subject_ref_type    text NULL,
  subject_ref_id      uuid NULL,
  semreg_mode         text NOT NULL,
  semreg_denied_verbs jsonb NULL,
  verb_candidates_pre  jsonb NULL,
  verb_candidates_post jsonb NULL,
  chosen_verb_fqn     text NULL,
  selection_source    text NULL,
  forced_verb_fqn     text NULL,
  outcome             text NOT NULL,
  dsl_hash            text NULL,
  run_sheet_entry_id  uuid NULL,
  macro_semreg_checked bool NOT NULL DEFAULT false,
  macro_denied_verbs   jsonb NULL,
  prompt_version      text NULL,
  error_code          text NULL,
  dominant_entity_id  uuid NULL,
  dominant_entity_kind text NULL,
  entity_kind_filtered boolean NOT NULL DEFAULT false,
  allowed_verbs_fingerprint varchar(70),
  pruned_verbs_count integer,
  toctou_recheck_performed boolean DEFAULT false,
  toctou_result varchar(30),
  toctou_new_fingerprint varchar(70)
);

CREATE INDEX IF NOT EXISTS ob_poc_intent_events_ts_idx ON "ob-poc".intent_events(ts);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_session_idx ON "ob-poc".intent_events(session_id, ts);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_utter_hash_idx ON "ob-poc".intent_events(utterance_hash);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_chosen_verb_idx ON "ob-poc".intent_events(chosen_verb_fqn);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_dominant_entity_idx
    ON "ob-poc".intent_events (dominant_entity_id)
    WHERE dominant_entity_id IS NOT NULL;
