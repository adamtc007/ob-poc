ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".entities
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_cbus_active
    ON "ob-poc".cbus (cbu_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_entities_active
    ON "ob-poc".entities (entity_id)
    WHERE deleted_at IS NULL;
