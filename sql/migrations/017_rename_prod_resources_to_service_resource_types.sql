-- Migration 017: Rename prod_resources to service_resource_types
-- This consolidates lifecycle-resource and resource domains into service-resource
--
-- Note: service_resources already exists as a junction table linking services to resources
-- So we use service_resource_types for the main resource type definitions table
--
-- Run with: psql -d data_designer -f sql/migrations/017_rename_prod_resources_to_service_resource_types.sql

BEGIN;

-- 1. Rename the main table
ALTER TABLE "ob-poc".prod_resources RENAME TO service_resource_types;

-- 2. Rename indexes
ALTER INDEX "ob-poc".idx_prod_resources_dict_group RENAME TO idx_service_resource_types_dict_group;
ALTER INDEX "ob-poc".idx_prod_resources_is_active RENAME TO idx_service_resource_types_is_active;
ALTER INDEX "ob-poc".idx_prod_resources_name RENAME TO idx_service_resource_types_name;
ALTER INDEX "ob-poc".idx_prod_resources_owner RENAME TO idx_service_resource_types_owner;
ALTER INDEX "ob-poc".idx_prod_resources_resource_code RENAME TO idx_service_resource_types_resource_code;

-- 3. Rename constraints
ALTER TABLE "ob-poc".service_resource_types RENAME CONSTRAINT prod_resources_pkey TO service_resource_types_pkey;
ALTER TABLE "ob-poc".service_resource_types RENAME CONSTRAINT prod_resources_name_key TO service_resource_types_name_key;

-- Note: Foreign keys pointing TO this table are automatically updated by PostgreSQL
-- when the table is renamed. The following tables have FKs to this table:
--   - cbu_resource_instances (resource_type_id)
--   - service_resources junction table (resource_id)
--   - onboarding_resource_allocations (resource_id)
--   - resource_attribute_requirements (resource_id)
--   - service_resource_capabilities (resource_id)

COMMIT;

-- Verify the rename
SELECT table_name FROM information_schema.tables
WHERE table_schema = 'ob-poc' AND table_name IN ('prod_resources', 'service_resource_types', 'service_resources');
