-- Migration: Standardize CBU ID as UUID everywhere in schema
-- This migration fixes the inconsistency where some tables use VARCHAR(255) for cbu_id
-- while others use UUID, ensuring all CBU references are proper UUIDs with foreign key constraints

BEGIN;

-- 1. First, update the dsl_ob table to use proper UUID and add foreign key constraint
-- Note: This assumes existing cbu_id values can be converted to UUID format
-- If they're not valid UUIDs, you'll need to handle the data conversion first

-- Add a temporary column to hold UUID values
ALTER TABLE "ob-poc".dsl_ob ADD COLUMN cbu_id_uuid UUID;

-- Convert existing VARCHAR cbu_id values to UUID (assuming they're already UUID strings)
-- If they're not UUID format, you'll need a different conversion strategy
UPDATE "ob-poc".dsl_ob
SET cbu_id_uuid = CASE
    WHEN cbu_id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'::text
    THEN cbu_id::UUID
    ELSE NULL
END;

-- Drop the old VARCHAR column and rename the new UUID column
ALTER TABLE "ob-poc".dsl_ob DROP COLUMN cbu_id;
ALTER TABLE "ob-poc".dsl_ob RENAME COLUMN cbu_id_uuid TO cbu_id;
ALTER TABLE "ob-poc".dsl_ob ALTER COLUMN cbu_id SET NOT NULL;

-- Add foreign key constraint to reference cbus table
ALTER TABLE "ob-poc".dsl_ob
ADD CONSTRAINT fk_dsl_ob_cbu_id
FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;

-- 2. Update product_workflows table cbu_id to be UUID with foreign key
ALTER TABLE "ob-poc".product_workflows ADD COLUMN cbu_id_uuid UUID;

UPDATE "ob-poc".product_workflows
SET cbu_id_uuid = CASE
    WHEN cbu_id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'::text
    THEN cbu_id::UUID
    ELSE NULL
END;

ALTER TABLE "ob-poc".product_workflows DROP COLUMN cbu_id;
ALTER TABLE "ob-poc".product_workflows RENAME COLUMN cbu_id_uuid TO cbu_id;
ALTER TABLE "ob-poc".product_workflows ALTER COLUMN cbu_id SET NOT NULL;

-- Add foreign key constraint
ALTER TABLE "ob-poc".product_workflows
ADD CONSTRAINT fk_product_workflows_cbu_id
FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;

-- 3. Update orchestration_sessions table to ensure proper UUID handling
-- This table already has the correct type but let's ensure foreign key constraint
ALTER TABLE "ob-poc".orchestration_sessions
ADD CONSTRAINT fk_orchestration_sessions_cbu_id
FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;

-- 4. Recreate indexes that may have been affected
DROP INDEX IF EXISTS "ob-poc".idx_dsl_ob_cbu_id_created_at;
CREATE INDEX idx_dsl_ob_cbu_id_created_at
ON "ob-poc".dsl_ob (cbu_id, created_at DESC);

DROP INDEX IF EXISTS "ob-poc".idx_product_workflows_cbu;
CREATE INDEX idx_product_workflows_cbu
ON "ob-poc".product_workflows (cbu_id);

-- 5. Update any application code comments that reference VARCHAR cbu_id
COMMENT ON COLUMN "ob-poc".dsl_ob.cbu_id IS 'UUID reference to cbus table primary key';
COMMENT ON COLUMN "ob-poc".product_workflows.cbu_id IS 'UUID reference to cbus table primary key';

COMMIT;
