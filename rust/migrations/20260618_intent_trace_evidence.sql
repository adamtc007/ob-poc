-- Intent Trace evidence capture (Option C — measure both reductions).
--
-- Extends the canonical telemetry table "ob-poc".intent_events (the target of
-- agent/telemetry/store.rs::insert_intent_event) with the evidence fields the
-- Intent Trace work requires. Idempotent and forward-only:
--   * CREATE TABLE IF NOT EXISTS reconstructs the base table on DBs that never
--     materialised it (the live data_designer is one such — telemetry inserts
--     have been silently no-oping). DDL mirrors migrations/master-schema.sql.
--   * ADD COLUMN IF NOT EXISTS adds the five new evidence columns.
--
-- Net-new columns:
--   surface_full_count        FilterSummary.total_registry  (verbs before pack collapse)
--   surface_pack_scoped_count FilterSummary.after_semreg     (verbs after pack collapse)
--   soft_stage_flow           candidate counts entering/leaving search boundaries
--   state_observer            eval-mode read-only [{verb,state_reachable,failing_predicate}]
--   entity_confidence         resolution confidence already flowing into ContextResolutionRequest
--
-- NOT added: board_status (Confirmed/Provisional). ABSENT in the codebase (research B3);
-- it is Option-A surface, out of scope for Option C.

CREATE TABLE IF NOT EXISTS "ob-poc".intent_events (
    event_id uuid NOT NULL,
    ts timestamp with time zone DEFAULT now() NOT NULL,
    session_id uuid NOT NULL,
    actor_id text NOT NULL,
    entrypoint text NOT NULL,
    utterance_hash text NOT NULL,
    utterance_preview text,
    scope text,
    subject_ref_type text,
    subject_ref_id uuid,
    semreg_mode text NOT NULL,
    semreg_denied_verbs jsonb,
    verb_candidates_pre jsonb,
    verb_candidates_post jsonb,
    chosen_verb_fqn text,
    selection_source text,
    forced_verb_fqn text,
    outcome text NOT NULL,
    dsl_hash text,
    run_sheet_entry_id uuid,
    macro_semreg_checked boolean DEFAULT false NOT NULL,
    macro_denied_verbs jsonb,
    prompt_version text,
    error_code text,
    dominant_entity_id uuid,
    dominant_entity_kind text,
    entity_kind_filtered boolean DEFAULT false NOT NULL,
    allowed_verbs_fingerprint character varying(70),
    pruned_verbs_count integer,
    toctou_recheck_performed boolean DEFAULT false,
    toctou_result character varying(30),
    toctou_new_fingerprint character varying(70)
);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'intent_events_pkey'
          AND conrelid = '"ob-poc".intent_events'::regclass
    ) THEN
        ALTER TABLE "ob-poc".intent_events
            ADD CONSTRAINT intent_events_pkey PRIMARY KEY (event_id);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS ob_poc_intent_events_chosen_verb_idx
    ON "ob-poc".intent_events USING btree (chosen_verb_fqn);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_dominant_entity_idx
    ON "ob-poc".intent_events USING btree (dominant_entity_id) WHERE (dominant_entity_id IS NOT NULL);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_session_idx
    ON "ob-poc".intent_events USING btree (session_id, ts);
CREATE INDEX IF NOT EXISTS ob_poc_intent_events_ts_idx
    ON "ob-poc".intent_events USING btree (ts);

-- ── Intent Trace evidence columns ──────────────────────────────────────────
ALTER TABLE "ob-poc".intent_events
    ADD COLUMN IF NOT EXISTS surface_full_count        integer,
    ADD COLUMN IF NOT EXISTS surface_pack_scoped_count integer,
    ADD COLUMN IF NOT EXISTS soft_stage_flow           jsonb,
    ADD COLUMN IF NOT EXISTS state_observer            jsonb,
    ADD COLUMN IF NOT EXISTS entity_confidence         real;

COMMENT ON COLUMN "ob-poc".intent_events.surface_full_count IS
    'Verb registry size before pack-scope collapse (FilterSummary.total_registry).';
COMMENT ON COLUMN "ob-poc".intent_events.surface_pack_scoped_count IS
    'Verb count after pack-scope collapse (FilterSummary.after_semreg). The classification reducer.';
COMMENT ON COLUMN "ob-poc".intent_events.soft_stage_flow IS
    'Candidate counts entering/leaving the meaningful search boundaries (scenario/macro, lexicon+exact, semantic, post-normalize).';
COMMENT ON COLUMN "ob-poc".intent_events.state_observer IS
    'Eval-mode read-only state reachability: [{verb,state_reachable,failing_predicate}] over ranked/allowed. Does NOT filter.';
COMMENT ON COLUMN "ob-poc".intent_events.entity_confidence IS
    'Entity/context resolution confidence (already flows into ContextResolutionRequest).';
