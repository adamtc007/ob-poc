-- EOP-PLAN-KYCUBO-KIT-T7 §2/T7.1 — capture telemetry for the plain-English
-- ramp, per EOP-DD-KYCUBO-CAPTURE-CHARTER v0.1 (RATIFIED 2026-08-14).
--
-- Deliberately NOT kyc_intent_events. This is proposal telemetry about what
-- the ramp showed an operator and what they did with it — not a governed
-- verb-stream event. It must be independently deletable on the charter's
-- retention schedule (§2: 180 days) without touching kyc_intent_events'
-- append-only immutability guarantees (T4.5 §3 append protocol).
--
-- Columns are exactly the charter §1 field list — no more. Any additional
-- field needs a charter amendment first (I-6: everything captured is
-- governed), not a silent ALTER TABLE.

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_ramp_capture (
    id                  uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Verbatim per the charter's PII ruling (§1) — KYC/UBO utterances name
    -- real persons/entities by domain design; hashing would defeat the only
    -- permitted use (§4: T7.3 training/eval corpus review).
    utterance_text      text        NOT NULL,
    -- Board *identity*, not board contents (full contents are reconstructable
    -- from the substrate at the recorded as_of and would be redundant state).
    placement_set_hash  text        NOT NULL,
    proposal            text        NOT NULL,
    disposition         text        NOT NULL
        CHECK (disposition IN ('select', 'clarify', 'abstain')),
    user_action         text        NOT NULL
        CHECK (user_action IN ('accepted', 'edited', 'rejected')),
    -- Joins back to the governed append-protocol record it produced, without
    -- duplicating that record. NULL unless user_action = 'accepted'.
    staged_move_id      uuid        NULL,
    created_at          timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS kyc_ramp_capture_created_at_idx
    ON "ob-poc".kyc_ramp_capture (created_at);

COMMENT ON TABLE "ob-poc".kyc_ramp_capture IS
    'Plain-English ramp proposal telemetry (T7.1). NOT the verb stream — separately retained/deletable per EOP-DD-KYCUBO-CAPTURE-CHARTER v0.1 §2 (180-day window). EOP-PLAN-KYCUBO-KIT-T7 §2.';
