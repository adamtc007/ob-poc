-- EOP-PLAN-KYCUBO-KIT-001 T1 — closes KIT-1 (S-expression source of truth).
--
-- Adds the resolved DSL S-expression text alongside the structured JSONB
-- payload already captured. Nullable for now: existing rows predate the
-- renderer and have no source text to backfill without a separate one-time
-- backfill pass (T1 scope note — the backfill migration is a distinct,
-- later artifact, provenance-marked `backfilled`, per the plan's T0.2
-- ratification). New appends populate it unconditionally from
-- `render_intent_event_to_sexpr` — enforced at the application layer
-- (`append_captures_source` gate test), not yet a DB-level NOT NULL
-- constraint (that follows once the backfill completes).

ALTER TABLE "ob-poc".kyc_intent_events
    ADD COLUMN IF NOT EXISTS source_text text NULL;

COMMENT ON COLUMN "ob-poc".kyc_intent_events.source_text IS
    'Resolved DSL S-expression for this event, UUIDs resolved at capture (KIT-1). NULL only for pre-T1 rows pending backfill. EOP-PLAN-KYCUBO-KIT-001 T1.';
