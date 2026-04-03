-- Cross-Workspace State Consistency: Phase 7 — External Call Log
-- Records every third-party interaction. Enables idempotency on replay.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.4

CREATE TABLE IF NOT EXISTS "ob-poc".external_call_log (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id           UUID NOT NULL,
    verb_fqn            TEXT NOT NULL,
    provider            TEXT NOT NULL,
    operation           TEXT NOT NULL,
    external_ref        TEXT,
    request_hash        BIGINT NOT NULL,
    request_snapshot    JSONB,
    response_snapshot   JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    superseded_by       UUID REFERENCES "ob-poc".external_call_log(id),
    is_current          BOOLEAN NOT NULL DEFAULT true,

    CONSTRAINT chk_external_call_superseded
        CHECK (NOT (is_current AND superseded_by IS NOT NULL))
);

-- Fast idempotency lookup: current call for (entity, verb, provider)
CREATE UNIQUE INDEX IF NOT EXISTS idx_external_call_log_current
    ON "ob-poc".external_call_log (entity_id, verb_fqn, provider)
    WHERE is_current = true;

COMMENT ON TABLE "ob-poc".external_call_log IS
    'Records every third-party system interaction. Enables idempotency checks on constellation replay — same request_hash = skip, different = amend per provider capability.';
