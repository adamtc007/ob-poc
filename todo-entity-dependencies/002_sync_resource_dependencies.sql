-- =============================================================================
-- Migration: Sync resource_dependencies → entity_type_dependencies
-- =============================================================================
--
-- This migration ensures all existing resource dependencies are captured
-- in the new unified table. Run AFTER 001_entity_type_dependencies.sql
--
-- Safe to run multiple times (uses ON CONFLICT DO NOTHING)
--
-- =============================================================================

-- Sync any resource dependencies not already in entity_type_dependencies
INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind, priority)
SELECT DISTINCT
    'resource_instance' as from_type,
    rt_from.resource_code as from_subtype,
    'resource_instance' as to_type,
    rt_to.resource_code as to_subtype,
    rd.inject_arg as via_arg,
    CASE rd.dependency_type 
        WHEN 'optional' THEN 'optional'
        ELSE 'required'
    END as dependency_kind,
    100 as priority
FROM "ob-poc".resource_dependencies rd
JOIN "ob-poc".service_resource_types rt_from ON rt_from.resource_id = rd.resource_type_id
JOIN "ob-poc".service_resource_types rt_to ON rt_to.resource_id = rd.depends_on_type_id
WHERE rd.is_active = true
ON CONFLICT (from_type, from_subtype, to_type, to_subtype) DO NOTHING;

-- Report results
WITH sync_stats AS (
    SELECT 
        (SELECT count(*) FROM "ob-poc".resource_dependencies WHERE is_active = true) as legacy_count,
        (SELECT count(*) FROM "ob-poc".entity_type_dependencies 
         WHERE from_type = 'resource_instance' AND is_active = true) as unified_count
)
SELECT 
    'Sync complete: ' || legacy_count || ' legacy deps, ' || unified_count || ' unified deps' as status
FROM sync_stats;

-- =============================================================================
-- VALIDATION: Verify all legacy deps are now in unified table
-- =============================================================================

DO $$
DECLARE
    missing_count INTEGER;
BEGIN
    SELECT count(*) INTO missing_count
    FROM "ob-poc".resource_dependencies rd
    JOIN "ob-poc".service_resource_types rt_from ON rt_from.resource_id = rd.resource_type_id
    JOIN "ob-poc".service_resource_types rt_to ON rt_to.resource_id = rd.depends_on_type_id
    WHERE rd.is_active = true
    AND NOT EXISTS (
        SELECT 1 FROM "ob-poc".entity_type_dependencies etd
        WHERE etd.from_type = 'resource_instance'
        AND etd.from_subtype = rt_from.resource_code
        AND etd.to_type = 'resource_instance'
        AND etd.to_subtype = rt_to.resource_code
        AND etd.is_active = true
    );
    
    IF missing_count > 0 THEN
        RAISE WARNING 'Found % resource dependencies not in unified table!', missing_count;
    ELSE
        RAISE NOTICE 'All resource dependencies successfully migrated to unified table';
    END IF;
END $$;
