-- ============================================
-- Rename prod_resources to lifecycle_resources
-- ============================================
BEGIN;

-- Rename table
ALTER TABLE IF EXISTS "ob-poc".prod_resources RENAME TO lifecycle_resources;

-- Rename indexes
ALTER INDEX IF EXISTS "ob-poc".idx_prod_resources_name RENAME TO idx_lifecycle_resources_name;
ALTER INDEX IF EXISTS "ob-poc".idx_prod_resources_owner RENAME TO idx_lifecycle_resources_owner;
ALTER INDEX IF EXISTS "ob-poc".idx_prod_resources_dict_group RENAME TO idx_lifecycle_resources_dict_group;
ALTER INDEX IF EXISTS "ob-poc".idx_prod_resources_resource_code RENAME TO idx_lifecycle_resources_resource_code;
ALTER INDEX IF EXISTS "ob-poc".idx_prod_resources_is_active RENAME TO idx_lifecycle_resources_is_active;

-- Update any sequences if they exist
-- ALTER SEQUENCE IF EXISTS "ob-poc".prod_resources_resource_id_seq RENAME TO lifecycle_resources_resource_id_seq;

COMMIT;
