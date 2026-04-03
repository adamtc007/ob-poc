-- Cross-Workspace State Consistency: Phase 9 — Compensation Records
-- Regulatory audit trail for every external correction triggered by replay.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.5

CREATE TABLE IF NOT EXISTS "ob-poc".compensation_records (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    remediation_id      UUID NOT NULL REFERENCES "ob-poc".remediation_events(id),
    entity_id           UUID NOT NULL,
    provider            TEXT NOT NULL,
    original_call_id    UUID REFERENCES "ob-poc".external_call_log(id),
    correction_call_id  UUID REFERENCES "ob-poc".external_call_log(id),
    correction_type     TEXT NOT NULL,
    changed_fields      JSONB,
    outcome             TEXT NOT NULL DEFAULT 'pending',
    confirmed_at        TIMESTAMPTZ,
    confirmed_by        TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT chk_correction_type
        CHECK (correction_type IN ('amend', 'cancel_recreate', 'correction_filing', 'manual')),
    CONSTRAINT chk_compensation_outcome
        CHECK (outcome IN ('success', 'pending', 'failed'))
);

-- Find compensation records for a remediation event
CREATE INDEX IF NOT EXISTS idx_compensation_records_remediation
    ON "ob-poc".compensation_records (remediation_id);

-- Find by entity
CREATE INDEX IF NOT EXISTS idx_compensation_records_entity
    ON "ob-poc".compensation_records (entity_id);

COMMENT ON TABLE "ob-poc".compensation_records IS
    'Regulatory audit trail for every external correction triggered by constellation replay. Primary evidence table for compliance review.';
