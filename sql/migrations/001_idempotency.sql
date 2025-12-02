-- Migration: Idempotency Support for DSL Execution
-- Purpose: Enable re-runnable DSL programs without side effects

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_idempotency (
    idempotency_key VARCHAR(64) PRIMARY KEY,
    execution_id UUID NOT NULL,
    statement_index INTEGER NOT NULL,
    verb VARCHAR(100) NOT NULL,
    args_hash VARCHAR(64) NOT NULL,
    result_type VARCHAR(20) NOT NULL,
    result_id UUID,
    result_json JSONB,
    result_affected BIGINT,
    created_at TIMESTAMPTZ DEFAULT now() NOT NULL,
    CONSTRAINT uq_execution_statement UNIQUE (execution_id, statement_index)
);

CREATE INDEX IF NOT EXISTS idx_idempotency_execution 
ON "ob-poc".dsl_idempotency(execution_id);

CREATE INDEX IF NOT EXISTS idx_idempotency_verb 
ON "ob-poc".dsl_idempotency(verb);

-- Cleanup function
CREATE OR REPLACE FUNCTION "ob-poc".cleanup_idempotency(days_old INTEGER DEFAULT 30)
RETURNS INTEGER AS $func$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM "ob-poc".dsl_idempotency
    WHERE created_at < NOW() - (days_old || ' days')::INTERVAL;
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$func$ LANGUAGE plpgsql;
