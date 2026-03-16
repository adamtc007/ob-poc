-- Phase 0.3: Drop all duplicate FK constraint pairs
-- Source: P3-A CR-3
-- Problem: 6 duplicate FK constraint pairs across entities and cbu_entity_roles.

-- entities: drop duplicate FK on entity_type_id (keep entities_entity_type_id_fkey)
ALTER TABLE "ob-poc".entities
  DROP CONSTRAINT IF EXISTS fk_entities_entity_type;

-- cbu_entity_roles: drop duplicate FKs (keep the *_fkey variants)
ALTER TABLE "ob-poc".cbu_entity_roles
  DROP CONSTRAINT IF EXISTS fk_cbu_entity_roles_cbu_id;
ALTER TABLE "ob-poc".cbu_entity_roles
  DROP CONSTRAINT IF EXISTS fk_cbu_entity_roles_entity_id;
ALTER TABLE "ob-poc".cbu_entity_roles
  DROP CONSTRAINT IF EXISTS fk_cbu_entity_roles_role_id;

-- Drop redundant unique index on cbu_entity_roles
DROP INDEX IF EXISTS "ob-poc".idx_cbu_entity_roles_unique;

-- Drop redundant unique index on entity_funds
DROP INDEX IF EXISTS "ob-poc".entity_funds_entity_id_key;
