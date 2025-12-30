-- ============================================================================
-- ALLIANZ INCORRECT DATA CLEANUP
-- Generated: 2024-12-30
-- 
-- This script removes incorrectly loaded Allianz data that was flattened
-- by Python scripts instead of loaded via proper DSL hierarchy.
--
-- PRESERVES:
--   - GLEIF-sourced entities (have LEI codes in entity_funds)
--   - UBO Test entities
--   - Any non-Allianz data
--
-- RUN WITH CAUTION - Creates backup temp tables for review
-- ============================================================================

BEGIN;

-- ============================================================================
-- STEP 1: IDENTIFY WHAT WILL BE DELETED
-- ============================================================================

-- Create temp table of CBUs to delete (the incorrectly created sub-fund CBUs)
CREATE TEMP TABLE cbus_to_delete AS
SELECT cbu_id, name 
FROM "ob-poc".cbus 
WHERE name ILIKE '%allianz%'
  -- Exclude any that might be the correct group CBU if it exists
  AND name NOT ILIKE '%allianz global investors (group)%'
  AND name NOT ILIKE '%allianz global investors group%';

-- Create temp table of entities to delete
-- Only LIMITED_COMPANY_PRIVATE that don't have LEI (not GLEIF sourced)
CREATE TEMP TABLE entities_to_delete AS
SELECT e.entity_id, e.name
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
WHERE e.name ILIKE '%allianz%'
  AND et.name = 'LIMITED_COMPANY_PRIVATE'
  AND (ef.lei IS NULL OR ef.entity_id IS NULL)  -- Preserve GLEIF-sourced entities
  AND e.name NOT ILIKE 'UBO Test:%';  -- Exclude test entities

-- ============================================================================
-- STEP 2: REPORT WHAT WILL BE DELETED
-- ============================================================================

\echo '=========================================='
\echo 'CLEANUP PREVIEW - WHAT WILL BE DELETED'
\echo '=========================================='

SELECT 'CBUs to delete:' as item, COUNT(*) as count FROM cbus_to_delete
UNION ALL
SELECT 'Entities to delete:', COUNT(*) FROM entities_to_delete
UNION ALL
SELECT 'cbu_entity_roles affected:', COUNT(*) 
FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);

\echo ''
\echo 'Sample CBUs being deleted (first 10):'
SELECT name FROM cbus_to_delete ORDER BY name LIMIT 10;

\echo ''
\echo 'Sample entities being deleted (first 10):'
SELECT name FROM entities_to_delete ORDER BY name LIMIT 10;

\echo ''
\echo 'GLEIF entities being PRESERVED:'
SELECT e.name, ef.lei
FROM "ob-poc".entities e
JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
WHERE e.name ILIKE '%allianz%' AND ef.lei IS NOT NULL;

-- ============================================================================
-- STEP 3: DELETE IN CORRECT ORDER (respecting FK constraints)
-- ============================================================================

\echo ''
\echo '=========================================='
\echo 'EXECUTING DELETES'
\echo '=========================================='

-- Delete dependent records from CBUs
DELETE FROM "ob-poc".cbu_entity_roles 
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted cbu_entity_roles'

DELETE FROM "kyc".cases 
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted kyc.cases'

DELETE FROM "ob-poc".cbu_evidence
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted cbu_evidence'

DELETE FROM "ob-poc".cbu_change_log
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted cbu_change_log'

DELETE FROM "ob-poc".dsl_sessions
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted dsl_sessions'

DELETE FROM "ob-poc".ubo_registry
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted ubo_registry'

DELETE FROM "ob-poc".service_delivery_map
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted service_delivery_map'

DELETE FROM "ob-poc".cbu_resource_instances
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted cbu_resource_instances'

DELETE FROM "ob-poc".client_allegations
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted client_allegations'

DELETE FROM "ob-poc".onboarding_requests
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted onboarding_requests'

DELETE FROM "kyc".share_classes
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted kyc.share_classes'

-- Delete the CBUs
DELETE FROM "ob-poc".cbus 
WHERE cbu_id IN (SELECT cbu_id FROM cbus_to_delete);
\echo 'Deleted CBUs'

-- Delete entity-related records
DELETE FROM "ob-poc".entity_relationships
WHERE from_entity_id IN (SELECT entity_id FROM entities_to_delete)
   OR to_entity_id IN (SELECT entity_id FROM entities_to_delete);
\echo 'Deleted entity_relationships'

DELETE FROM "ob-poc".entity_funds
WHERE entity_id IN (SELECT entity_id FROM entities_to_delete);
\echo 'Deleted entity_funds'

DELETE FROM "ob-poc".entity_share_classes
WHERE entity_id IN (SELECT entity_id FROM entities_to_delete);
\echo 'Deleted entity_share_classes'

DELETE FROM "ob-poc".fund_structure
WHERE parent_entity_id IN (SELECT entity_id FROM entities_to_delete)
   OR child_entity_id IN (SELECT entity_id FROM entities_to_delete);
\echo 'Deleted fund_structure'

-- Delete the entities
DELETE FROM "ob-poc".entities
WHERE entity_id IN (SELECT entity_id FROM entities_to_delete);
\echo 'Deleted entities'

-- ============================================================================
-- STEP 4: VERIFICATION
-- ============================================================================

\echo ''
\echo '=========================================='
\echo 'POST-CLEANUP VERIFICATION'
\echo '=========================================='

SELECT 'Remaining Allianz CBUs:' as check, COUNT(*) as count
FROM "ob-poc".cbus WHERE name ILIKE '%allianz%'
UNION ALL
SELECT 'Remaining Allianz entities:', COUNT(*) 
FROM "ob-poc".entities WHERE name ILIKE '%allianz%'
UNION ALL
SELECT 'GLEIF entities preserved:', COUNT(*) 
FROM "ob-poc".entities e
JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
WHERE e.name ILIKE '%allianz%' AND ef.lei IS NOT NULL;

-- ============================================================================
-- COMMIT OR ROLLBACK
-- ============================================================================

-- Cleanup temp tables
DROP TABLE cbus_to_delete;
DROP TABLE entities_to_delete;

-- UNCOMMENT ONE OF THESE:
-- COMMIT;  -- Run this to make changes permanent
ROLLBACK;   -- Run this first to preview without making changes

\echo ''
\echo 'Script completed. If ROLLBACK was executed, no changes were made.'
\echo 'Edit script to use COMMIT when ready to make permanent changes.'
