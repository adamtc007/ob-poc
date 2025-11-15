-- Migration 007: Agentic DSL CRUD System
-- Purpose: Enable AI-powered natural language to DSL to database operations
-- Date: 2025-11-14

-- Ensure CBU table has source_of_funds field
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS source_of_funds TEXT;

-- Enhance crud_operations table for agentic tracking
ALTER TABLE "ob-poc".crud_operations
ADD COLUMN IF NOT EXISTS cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
ADD COLUMN IF NOT EXISTS parsed_ast JSONB,
ADD COLUMN IF NOT EXISTS natural_language_input TEXT;

-- CBU creation audit log
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_creation_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    nature_purpose TEXT,
    source_of_funds TEXT,
    created_via VARCHAR(50) DEFAULT 'agentic_dsl',
    ai_instruction TEXT,
    generated_dsl TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Entity role connection audit log
CREATE TABLE IF NOT EXISTS "ob-poc".entity_role_connections (
    connection_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    role_id UUID NOT NULL,
    connected_via VARCHAR(50) DEFAULT 'agentic_dsl',
    ai_instruction TEXT,
    generated_dsl TEXT,
    connected_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_cbu_creation_log_cbu ON "ob-poc".cbu_creation_log(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_creation_log_created ON "ob-poc".cbu_creation_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_entity_role_conn_cbu ON "ob-poc".entity_role_connections(cbu_id);
CREATE INDEX IF NOT EXISTS idx_entity_role_conn_entity ON "ob-poc".entity_role_connections(entity_id);
CREATE INDEX IF NOT EXISTS idx_crud_ops_cbu ON "ob-poc".crud_operations(cbu_id) WHERE cbu_id IS NOT NULL;

-- Comments
COMMENT ON TABLE "ob-poc".cbu_creation_log IS 
'Audit log for CBU creations via agentic DSL system. Tracks natural language instructions and generated DSL.';

COMMENT ON TABLE "ob-poc".entity_role_connections IS 
'Tracks entity-role-CBU connections made via agentic DSL. Enables audit trail for relationship creation.';

COMMENT ON COLUMN "ob-poc".crud_operations.natural_language_input IS 
'Original natural language instruction from user (if created via agentic system).';

COMMENT ON COLUMN "ob-poc".crud_operations.parsed_ast IS 
'Parsed Abstract Syntax Tree of the generated DSL for debugging and analysis.';
