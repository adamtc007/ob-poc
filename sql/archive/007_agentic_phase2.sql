-- Migration 007: Agentic Phase 2 - CBU creation log and entity role connections

-- Add source_of_funds column to cbus table if it doesn't exist
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'cbus' 
        AND column_name = 'source_of_funds'
    ) THEN
        ALTER TABLE "ob-poc".cbus 
        ADD COLUMN source_of_funds TEXT;
    END IF;
END $$;

-- Create cbu_creation_log table for audit trail
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_creation_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    nature_purpose TEXT,
    source_of_funds TEXT,
    ai_instruction TEXT,
    generated_dsl TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_cbu_creation_log_cbu FOREIGN KEY (cbu_id) 
        REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE
);

-- Create index on cbu_id for faster lookups
CREATE INDEX IF NOT EXISTS idx_cbu_creation_log_cbu ON "ob-poc".cbu_creation_log(cbu_id);

-- Create entity_role_connections table for relationship tracking
CREATE TABLE IF NOT EXISTS "ob-poc".entity_role_connections (
    connection_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    role_id UUID NOT NULL,
    connection_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_entity_role_connections_cbu FOREIGN KEY (cbu_id) 
        REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    CONSTRAINT fk_entity_role_connections_entity FOREIGN KEY (entity_id) 
        REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_entity_role_connections_cbu ON "ob-poc".entity_role_connections(cbu_id);
CREATE INDEX IF NOT EXISTS idx_entity_role_connections_entity ON "ob-poc".entity_role_connections(entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_role_connections_role ON "ob-poc".entity_role_connections(role_id);
