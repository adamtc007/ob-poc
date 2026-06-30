-- EOP-DD-KYCUBO-002 §5 — the first stream projection (pure-stream, no reference joins).
--
-- A DISPOSABLE, REBUILDABLE fold of the control edges in the verb stream (K-34).
-- Written ONLY by the projector (rebuild_projection / the future outbox drainer),
-- NEVER by a direct verb write. The verb stream (kyc_intent_events) is the system
-- of record; this table is a materialized fold and can be dropped + rebuilt from
-- the stream at any time with no data loss.
--
-- Status is the fold's DERIVED epistemic status (K-11) — Asserted/Evidenced/
-- Verified/Superseded — never set directly. Additive; no FK to authoritative data
-- (it IS derived).

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_control_edge_projection (
    subject_root         uuid             NOT NULL,
    edge_id              uuid             NOT NULL,
    edge_kind            jsonb            NOT NULL,   -- EdgeKind (data-carrying enum; e.g. TrustRole(..))
    from_entity_id       uuid             NOT NULL,
    to_entity_id         uuid             NOT NULL,
    percentage           double precision NULL,        -- only meaningful for economic edges
    status               text             NOT NULL,     -- derived EdgeStatus (K-11)
    evidence_event_id    uuid             NULL,
    originating_event_id uuid             NOT NULL,     -- K-35 traceability into the stream
    projected_at         timestamptz      NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (subject_root, edge_id)
);

CREATE INDEX IF NOT EXISTS kyc_control_edge_projection_status_idx
    ON "ob-poc".kyc_control_edge_projection (subject_root, status);

COMMENT ON TABLE "ob-poc".kyc_control_edge_projection IS
    'Disposable fold of control edges from kyc_intent_events (K-34). Written only by the projector; rebuildable from the stream. EOP-DD-KYCUBO-002 §5.';
