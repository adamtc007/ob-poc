-- Migration: Standardize CBU ID as UUID everywhere in schema
-- This migration fixes the inconsistency where some tables use VARCHAR(255) for cbu_id
-- while others use UUID, ensuring all CBU references are proper UUIDs with foreign key constraints

BEGIN;

-- 1. Convert dsl_ob.cbu_id from VARCHAR to UUID in-place and add FK
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='ob-poc' AND table_name='dsl_ob' AND column_name='cbu_id' AND data_type <> 'uuid'
  ) THEN
    -- Convert values to UUID where possible
    ALTER TABLE "ob-poc".dsl_ob
      ALTER COLUMN cbu_id TYPE UUID
      USING (CASE WHEN cbu_id::text ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN cbu_id::uuid ELSE NULL END);
  END IF;
  -- Ensure NOT NULL
  ALTER TABLE "ob-poc".dsl_ob ALTER COLUMN cbu_id SET NOT NULL;
  -- Ensure FK exists
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.table_constraints
    WHERE table_schema='ob-poc' AND table_name='dsl_ob' AND constraint_name='fk_dsl_ob_cbu_id'
  ) THEN
    ALTER TABLE "ob-poc".dsl_ob
      ADD CONSTRAINT fk_dsl_ob_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;
  END IF;
END $$;

-- 2. Convert product_workflows.cbu_id from VARCHAR to UUID in-place and add FK
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='ob-poc' AND table_name='product_workflows' AND column_name='cbu_id' AND data_type <> 'uuid'
  ) THEN
    ALTER TABLE "ob-poc".product_workflows
      ALTER COLUMN cbu_id TYPE UUID
      USING (CASE WHEN cbu_id::text ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN cbu_id::uuid ELSE NULL END);
  END IF;
  ALTER TABLE "ob-poc".product_workflows ALTER COLUMN cbu_id SET NOT NULL;
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.table_constraints
    WHERE table_schema='ob-poc' AND table_name='product_workflows' AND constraint_name='fk_product_workflows_cbu_id'
  ) THEN
    ALTER TABLE "ob-poc".product_workflows
      ADD CONSTRAINT fk_product_workflows_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;
  END IF;
END $$;

-- 3. Ensure orchestration_sessions.cbu_id FK exists (column already UUID)
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.table_constraints
    WHERE table_schema='ob-poc' AND table_name='orchestration_sessions' AND constraint_name='fk_orchestration_sessions_cbu_id'
  ) THEN
    ALTER TABLE "ob-poc".orchestration_sessions
      ADD CONSTRAINT fk_orchestration_sessions_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;
  END IF;
END $$;

-- 4. Refresh indexes (idempotent)
DROP INDEX IF EXISTS "ob-poc".idx_dsl_ob_cbu_id_created_at;
CREATE INDEX idx_dsl_ob_cbu_id_created_at ON "ob-poc".dsl_ob (cbu_id, created_at DESC);

DROP INDEX IF EXISTS "ob-poc".idx_product_workflows_cbu;
CREATE INDEX idx_product_workflows_cbu ON "ob-poc".product_workflows (cbu_id);

-- 5. Update comments
COMMENT ON COLUMN "ob-poc".dsl_ob.cbu_id IS 'UUID reference to cbus table primary key';
COMMENT ON COLUMN "ob-poc".product_workflows.cbu_id IS 'UUID reference to cbus table primary key';

COMMIT;
