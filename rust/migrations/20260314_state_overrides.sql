CREATE TABLE IF NOT EXISTS "ob-poc".state_overrides (
    id UUID PRIMARY KEY,
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_id UUID NULL REFERENCES "ob-poc".cases(case_id),
    constellation_type VARCHAR(255) NOT NULL,
    slot_path TEXT NOT NULL,
    computed_state VARCHAR(255) NOT NULL,
    override_state VARCHAR(255) NOT NULL,
    justification TEXT NOT NULL,
    authority VARCHAR(255) NOT NULL,
    conditions TEXT NULL,
    reducer_revision VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL,
    revoked_by VARCHAR(255) NULL,
    revoke_reason TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_state_overrides_cbu_id
    ON "ob-poc".state_overrides(cbu_id);

CREATE INDEX IF NOT EXISTS idx_state_overrides_case_id
    ON "ob-poc".state_overrides(case_id);

CREATE UNIQUE INDEX IF NOT EXISTS uq_active_state_override
    ON "ob-poc".state_overrides(
        cbu_id,
        COALESCE(case_id, '00000000-0000-0000-0000-000000000000'::uuid),
        slot_path
    )
    WHERE revoked_at IS NULL;
