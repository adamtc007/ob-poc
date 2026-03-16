CREATE TABLE IF NOT EXISTS "ob-poc".cbu_entity_roles_history (
    history_id UUID PRIMARY KEY DEFAULT uuidv7(),
    cbu_entity_role_id UUID NOT NULL,
    cbu_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    role_id UUID NOT NULL,
    target_entity_id UUID,
    ownership_percentage NUMERIC(5,2),
    effective_from DATE,
    effective_to DATE,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    operation TEXT NOT NULL CHECK (operation IN ('UPDATE', 'DELETE')),
    changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_history_cbu_id
    ON "ob-poc".cbu_entity_roles_history (cbu_id);

CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_history_entity_id
    ON "ob-poc".cbu_entity_roles_history (entity_id);

CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_history_changed_at
    ON "ob-poc".cbu_entity_roles_history (changed_at DESC);
