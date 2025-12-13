-- =============================================================================
-- Migration: Unified Entity Type Dependencies
-- =============================================================================
--
-- Creates the entity_type_dependencies table which provides a single source
-- of truth for all entity/resource dependency relationships.
--
-- This replaces the resource-specific resource_dependencies table with a
-- generalized model that supports:
--   - CBU → Case → Workstream hierarchy
--   - Fund structures (umbrella → sub-fund → share class)
--   - Service resources (SETTLE → CUSTODY → SWIFT, etc.)
--   - Any future entity type relationships
--
-- Consumers:
--   - Compiler (topo_sort) - execution ordering
--   - Linter/LSP - type validation and suggestions
--   - Onboarding - resource provisioning order
--
-- =============================================================================

-- Drop if exists (for idempotent migrations)
DROP TABLE IF EXISTS "ob-poc".entity_type_dependencies CASCADE;

-- =============================================================================
-- TABLE: entity_type_dependencies
-- =============================================================================

CREATE TABLE "ob-poc".entity_type_dependencies (
    dependency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Source: what entity type has the dependency
    -- e.g., "resource_instance", "entity", "case", "workstream"
    from_type VARCHAR(50) NOT NULL,
    
    -- Source subtype (optional)
    -- e.g., "CUSTODY_ACCT", "fund_sub", "fund_umbrella"
    from_subtype VARCHAR(50),
    
    -- Target: what it depends on
    to_type VARCHAR(50) NOT NULL,
    
    -- Target subtype (optional)
    to_subtype VARCHAR(50),
    
    -- DSL argument that carries this dependency reference
    -- e.g., "cbu-id", "umbrella-id", "settlement-account-url"
    via_arg VARCHAR(100),
    
    -- Dependency characteristics
    dependency_kind VARCHAR(20) NOT NULL DEFAULT 'required'
        CHECK (dependency_kind IN ('required', 'optional', 'lifecycle')),
    
    -- For conditional dependencies (future use)
    condition_expr TEXT,
    
    -- Ordering hint for same-level dependencies (lower = higher priority)
    priority INTEGER NOT NULL DEFAULT 100,
    
    -- Soft delete
    is_active BOOLEAN NOT NULL DEFAULT true,
    
    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    -- Prevent duplicate edges
    UNIQUE NULLS NOT DISTINCT (from_type, from_subtype, to_type, to_subtype)
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Primary lookup: what does X depend on?
CREATE INDEX idx_entity_deps_from 
    ON "ob-poc".entity_type_dependencies(from_type, from_subtype)
    WHERE is_active = true;

-- Reverse lookup: what depends on X?
CREATE INDEX idx_entity_deps_to 
    ON "ob-poc".entity_type_dependencies(to_type, to_subtype)
    WHERE is_active = true;

-- For priority ordering
CREATE INDEX idx_entity_deps_priority
    ON "ob-poc".entity_type_dependencies(from_type, from_subtype, priority)
    WHERE is_active = true;

-- =============================================================================
-- COMMENTS
-- =============================================================================

COMMENT ON TABLE "ob-poc".entity_type_dependencies IS 
'Unified entity/resource dependency graph. Drives compiler ordering, linter validation, and onboarding workflows.
from_type/subtype depends on to_type/subtype. via_arg indicates which DSL argument carries the reference.';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.from_type IS 
'Entity type that has the dependency (e.g., resource_instance, entity, case, workstream)';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.from_subtype IS 
'Subtype qualifier (e.g., CUSTODY_ACCT for resources, fund_sub for entities)';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.to_type IS 
'Entity type that is depended upon';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.to_subtype IS 
'Subtype qualifier for the dependency target';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.via_arg IS 
'DSL argument name that carries this dependency (for linter validation)';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.dependency_kind IS 
'required = must exist before creation, optional = may be linked, lifecycle = state transition dependency';

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.priority IS 
'Ordering hint when multiple dependencies exist (lower = higher priority)';

-- =============================================================================
-- TRIGGER: updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_entity_deps_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_entity_deps_updated_at
    BEFORE UPDATE ON "ob-poc".entity_type_dependencies
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_entity_deps_timestamp();

-- =============================================================================
-- SEED DATA: Structural Dependencies (type-level)
-- =============================================================================

INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind, priority) VALUES

-- Case/Workstream hierarchy
('case', NULL, 'cbu', NULL, 'cbu-id', 'required', 100),
('workstream', NULL, 'case', NULL, 'case-id', 'required', 100),

-- Documents can attach to entities or CBUs
('document', NULL, 'entity', NULL, 'entity-id', 'optional', 100),
('document', NULL, 'cbu', NULL, 'cbu-id', 'optional', 100),

-- Observations attach to entities
('observation', NULL, 'entity', NULL, 'entity-id', 'required', 100),

-- KYC case depends on CBU
('kyc_case', NULL, 'cbu', NULL, 'cbu-id', 'required', 100),

-- Fund depends on CBU
('fund', NULL, 'cbu', NULL, 'cbu-id', 'required', 100);

-- =============================================================================
-- SEED DATA: Fund Hierarchy (subtype-level)
-- =============================================================================

INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind, priority) VALUES

-- Umbrella fund needs a legal entity
('entity', 'fund_umbrella', 'entity', NULL, 'legal-entity-id', 'required', 100),

-- Sub-fund needs umbrella
('entity', 'fund_sub', 'entity', 'fund_umbrella', 'umbrella-id', 'required', 100),

-- Share class needs sub-fund
('entity', 'share_class', 'entity', 'fund_sub', 'sub-fund-id', 'required', 100),

-- Master-feeder structure
('entity', 'fund_master', 'entity', 'fund_umbrella', 'umbrella-id', 'required', 100),
('entity', 'fund_feeder', 'entity', 'fund_master', 'master-fund-id', 'required', 100);

-- =============================================================================
-- SEED DATA: Service Resources (subtype = resource_code)
-- =============================================================================
-- These mirror the existing resource_dependencies table

INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind, priority) VALUES

-- SETTLE_ACCT is root (no dependencies)
-- CUSTODY_ACCT depends on SETTLE_ACCT
('resource_instance', 'CUSTODY_ACCT', 'resource_instance', 'SETTLE_ACCT', 'settlement-account-url', 'required', 100),

-- These depend on CUSTODY_ACCT
('resource_instance', 'SWIFT_CONN', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100),
('resource_instance', 'NAV_ENGINE', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100),
('resource_instance', 'CA_PLATFORM', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100),
('resource_instance', 'REPORTING', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100),
('resource_instance', 'PERF_ANALYTICS', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100),
('resource_instance', 'COLLATERAL_MGMT', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100),
('resource_instance', 'SEC_LENDING', 'resource_instance', 'CUSTODY_ACCT', 'custody-account-url', 'required', 100);

-- =============================================================================
-- VERIFICATION QUERIES
-- =============================================================================

-- View all dependencies for resources
-- SELECT from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind
-- FROM "ob-poc".entity_type_dependencies
-- WHERE from_type = 'resource_instance' AND is_active = true
-- ORDER BY from_subtype, priority;

-- Find root types (no dependencies)
-- SELECT DISTINCT from_type, from_subtype
-- FROM "ob-poc".entity_type_dependencies
-- WHERE (to_type, to_subtype) NOT IN (
--     SELECT from_type, from_subtype 
--     FROM "ob-poc".entity_type_dependencies
--     WHERE from_subtype IS NOT NULL
-- );

-- Compare with legacy resource_dependencies (should match)
-- SELECT 
--     rt1.resource_code as from_code, 
--     rt2.resource_code as to_code, 
--     rd.inject_arg,
--     CASE WHEN etd.dependency_id IS NOT NULL THEN '✓' ELSE '✗' END as migrated
-- FROM "ob-poc".resource_dependencies rd
-- JOIN "ob-poc".service_resource_types rt1 ON rt1.resource_id = rd.resource_type_id
-- JOIN "ob-poc".service_resource_types rt2 ON rt2.resource_id = rd.depends_on_type_id
-- LEFT JOIN "ob-poc".entity_type_dependencies etd 
--     ON etd.from_type = 'resource_instance' 
--     AND etd.from_subtype = rt1.resource_code
--     AND etd.to_type = 'resource_instance'
--     AND etd.to_subtype = rt2.resource_code
-- WHERE rd.is_active = true;

-- =============================================================================
-- DONE
-- =============================================================================

SELECT 'entity_type_dependencies created with ' || count(*) || ' seed rows' as status
FROM "ob-poc".entity_type_dependencies;
