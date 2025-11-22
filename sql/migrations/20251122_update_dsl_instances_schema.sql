-- Migration: Update dsl_instances to match DslRepository code
-- This adds the columns expected by the Rust DslRepository

-- Step 1: Add new columns to dsl_instances
ALTER TABLE "ob-poc".dsl_instances
ADD COLUMN IF NOT EXISTS instance_id UUID DEFAULT gen_random_uuid(),
ADD COLUMN IF NOT EXISTS domain_name VARCHAR(100),
ADD COLUMN IF NOT EXISTS business_reference VARCHAR(255),
ADD COLUMN IF NOT EXISTS current_version INTEGER DEFAULT 1;

-- Step 2: Migrate existing data
-- Map old columns to new columns
UPDATE "ob-poc".dsl_instances
SET 
    instance_id = COALESCE(instance_id, gen_random_uuid()),
    domain_name = COALESCE(domain_name, domain),
    business_reference = COALESCE(business_reference, case_id),
    current_version = COALESCE(current_version, 1);

-- Step 3: Make business_reference NOT NULL after data migration
ALTER TABLE "ob-poc".dsl_instances
ALTER COLUMN instance_id SET NOT NULL,
ALTER COLUMN business_reference SET NOT NULL;

-- Step 4: Add unique constraint on instance_id if not exists
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'dsl_instances_instance_id_key' 
        AND conrelid = '"ob-poc".dsl_instances'::regclass
    ) THEN
        ALTER TABLE "ob-poc".dsl_instances ADD CONSTRAINT dsl_instances_instance_id_key UNIQUE (instance_id);
    END IF;
END $$;

-- Step 5: Create dsl_instance_versions table if not exists
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_instance_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL,
    version_number INTEGER NOT NULL,
    dsl_content TEXT NOT NULL,
    operation_type VARCHAR(100) NOT NULL,
    compilation_status VARCHAR(50) DEFAULT 'COMPILED',
    ast_json JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_instance FOREIGN KEY (instance_id) 
        REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE
);

-- Step 6: Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_dsl_instances_business_reference 
    ON "ob-poc".dsl_instances(business_reference);
CREATE INDEX IF NOT EXISTS idx_dsl_instance_versions_instance_id 
    ON "ob-poc".dsl_instance_versions(instance_id);
CREATE INDEX IF NOT EXISTS idx_dsl_instance_versions_version_number 
    ON "ob-poc".dsl_instance_versions(instance_id, version_number);

-- Done
