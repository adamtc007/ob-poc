-- T5.3 (EOP-PLAN-CONTROLPLANE-001): write-set attestation records — one
-- row per `PgTransactionScope::commit_attested` call that had an expected
-- `WriteSetProof` attached. Audit trail only; the enforcement mechanism
-- itself is the transaction rollback in `commit_attested` (T5.2), not this
-- table — a persistence failure here never masks the commit/rollback
-- outcome (see `sequencer_tx.rs`).
CREATE TABLE "ob-poc".control_plane_write_attestations (
    attestation_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    attested_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    scope_id UUID NOT NULL,
    session_id UUID,
    verb_fqn TEXT,
    bounded BOOLEAN NOT NULL,
    captured_writes JSONB NOT NULL,
    excess_writes JSONB NOT NULL
);

CREATE INDEX idx_control_plane_write_attestations_scope
    ON "ob-poc".control_plane_write_attestations (scope_id);

CREATE INDEX idx_control_plane_write_attestations_breaches
    ON "ob-poc".control_plane_write_attestations (attested_at DESC)
    WHERE NOT bounded;

COMMENT ON TABLE "ob-poc".control_plane_write_attestations IS
    'T5.3 write-set attestation audit trail (EOP-PLAN-CONTROLPLANE-001). Enforcement (rollback on breach) happens at PgTransactionScope::commit_attested before this row is written, not here.';
