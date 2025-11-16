-- Taxonomy CRUD Support Migration
-- Adds CRUD tracking and logging for taxonomy operations

BEGIN;

-- Taxonomy CRUD operations log
CREATE TABLE IF NOT EXISTS "ob-poc".taxonomy_crud_log (
    operation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation_type VARCHAR(20) NOT NULL,
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID,
    natural_language_input TEXT,
    parsed_dsl TEXT,
    execution_result JSONB,
    success BOOLEAN DEFAULT false,
    error_message TEXT,
    user_id VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    execution_time_ms INTEGER
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_taxonomy_crud_entity ON "ob-poc".taxonomy_crud_log(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_taxonomy_crud_user ON "ob-poc".taxonomy_crud_log(user_id);
CREATE INDEX IF NOT EXISTS idx_taxonomy_crud_time ON "ob-poc".taxonomy_crud_log(created_at);
CREATE INDEX IF NOT EXISTS idx_taxonomy_crud_operation ON "ob-poc".taxonomy_crud_log(operation_type);

-- Comments
COMMENT ON TABLE "ob-poc".taxonomy_crud_log IS 'Audit log for taxonomy CRUD operations';
COMMENT ON COLUMN "ob-poc".taxonomy_crud_log.operation_type IS 'CREATE, READ, UPDATE, DELETE';
COMMENT ON COLUMN "ob-poc".taxonomy_crud_log.entity_type IS 'product, service, resource, onboarding';

COMMIT;
