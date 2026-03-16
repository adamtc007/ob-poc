-- Phase 0.1: Fix conflicting CASCADE/SET NULL on ubo_registry.cbu_id
-- Source: P3-A CR-1, P4 RISK-2
-- Problem: ubo_registry.cbu_id has two FK constraints with conflicting delete rules.

-- Drop BOTH conflicting FKs on cbu_id
ALTER TABLE "ob-poc".ubo_registry
  DROP CONSTRAINT IF EXISTS ubo_registry_cbu_id_fkey,
  DROP CONSTRAINT IF EXISTS fk_ubo_registry_cbu_id;

-- Re-add single FK with SET NULL (preserves UBO audit trail on CBU delete)
ALTER TABLE "ob-poc".ubo_registry
  ADD CONSTRAINT ubo_registry_cbu_id_fkey
  FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id)
  ON DELETE SET NULL;

-- ubo_registry.entity_id — drop duplicate, keep CASCADE
ALTER TABLE "ob-poc".ubo_registry
  DROP CONSTRAINT IF EXISTS fk_ubo_registry_entity_id;

-- ubo_registry.ultimate_parent_entity_id — drop duplicate, keep CASCADE
ALTER TABLE "ob-poc".ubo_registry
  DROP CONSTRAINT IF EXISTS fk_ubo_registry_ultimate_parent;
