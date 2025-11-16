-- Migration 011: Add taxonomy audit log table
-- Tracks all taxonomy operations for compliance and debugging

CREATE TABLE IF NOT EXISTS "ob-poc".taxonomy_audit_log (
    audit_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation VARCHAR(100) NOT NULL,
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    before_state JSONB,
    after_state JSONB,
    metadata JSONB,
    success BOOLEAN NOT NULL DEFAULT true,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_taxonomy_audit_entity 
    ON "ob-poc".taxonomy_audit_log(entity_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_taxonomy_audit_operation 
    ON "ob-poc".taxonomy_audit_log(operation, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_taxonomy_audit_user 
    ON "ob-poc".taxonomy_audit_log(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_taxonomy_audit_timestamp 
    ON "ob-poc".taxonomy_audit_log(created_at DESC);

-- Comments
COMMENT ON TABLE "ob-poc".taxonomy_audit_log IS 
    'Audit trail for all taxonomy operations including product, service, and resource management';

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.operation IS 
    'Type of operation performed (e.g., create_product, configure_service, allocate_resource)';

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.entity_type IS 
    'Type of entity being operated on (e.g., product, service, onboarding_request)';

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.before_state IS 
    'State of the entity before the operation (null for create operations)';

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.after_state IS 
    'State of the entity after the operation (null for delete operations)';

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.metadata IS 
    'Additional context about the operation';
