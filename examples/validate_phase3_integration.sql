-- validate_phase3_integration.sql
-- Comprehensive validation script for Phase 3 Grammar & Examples
--
-- This script validates the complete integration of Document Library and ISDA DSL domains
-- Tests database schema, data integrity, and cross-domain relationships

-- ============================================================================
-- PHASE 3 VALIDATION: DATABASE STATE VERIFICATION
-- ============================================================================

\echo '=== PHASE 3 VALIDATION: Document Library & ISDA DSL Integration ==='
\echo ''

-- ============================================================================
-- 1. DOMAIN INFRASTRUCTURE VALIDATION
-- ============================================================================

\echo '1. DOMAIN INFRASTRUCTURE VALIDATION'
\echo '------------------------------------'

-- Check all domains are operational
SELECT
    '1.1 Domain Registry:' as test_section,
    COUNT(*) as total_domains,
    STRING_AGG(domain_name, ', ' ORDER BY domain_name) as active_domains
FROM "ob-poc".dsl_domains
WHERE active = true;

-- Verify expected 7 domains exist
SELECT
    '1.2 Expected Domains:' as test_section,
    CASE
        WHEN COUNT(*) = 7 THEN '✅ PASS - All 7 domains present'
        ELSE '❌ FAIL - Expected 7 domains, found ' || COUNT(*)::text
    END as validation_result
FROM "ob-poc".dsl_domains
WHERE domain_name IN ('Document', 'ISDA', 'KYC', 'UBO', 'Onboarding', 'Compliance', 'Graph');

\echo ''

-- ============================================================================
-- 2. ATTRIBUTEID DICTIONARY VALIDATION
-- ============================================================================

\echo '2. ATTRIBUTEID DICTIONARY VALIDATION'
\echo '-------------------------------------'

-- Count total AttributeIDs by domain
SELECT
    '2.1 AttributeID Distribution:' as test_section,
    domain,
    COUNT(*) as attribute_count
FROM "ob-poc".dictionary
GROUP BY domain
ORDER BY COUNT(*) DESC;

-- Verify ISDA AttributeIDs
SELECT
    '2.2 ISDA AttributeIDs:' as test_section,
    CASE
        WHEN COUNT(*) >= 50 THEN '✅ PASS - ' || COUNT(*)::text || ' ISDA attributes found'
        ELSE '❌ FAIL - Expected 50+ ISDA attributes, found ' || COUNT(*)::text
    END as validation_result
FROM "ob-poc".dictionary
WHERE name LIKE 'isda.%';

-- Verify Document AttributeIDs
SELECT
    '2.3 Document AttributeIDs:' as test_section,
    CASE
        WHEN COUNT(*) >= 20 THEN '✅ PASS - ' || COUNT(*)::text || ' document attributes found'
        ELSE '❌ FAIL - Expected 20+ document attributes, found ' || COUNT(*)::text
    END as validation_result
FROM "ob-poc".dictionary
WHERE name LIKE 'document.%';

\echo ''

-- ============================================================================
-- 3. DOCUMENT LIBRARY INFRASTRUCTURE
-- ============================================================================

\echo '3. DOCUMENT LIBRARY INFRASTRUCTURE'
\echo '-----------------------------------'

-- Check document tables exist
SELECT
    '3.1 Document Tables:' as test_section,
    table_name,
    'EXISTS' as status
FROM information_schema.tables
WHERE table_schema = 'ob-poc'
  AND table_name LIKE '%document%'
ORDER BY table_name;

-- Verify document types
SELECT
    '3.2 Document Types:' as test_section,
    COUNT(*) as total_types,
    COUNT(CASE WHEN type_name LIKE 'ISDA_%' THEN 1 END) as isda_types,
    CASE
        WHEN COUNT(*) >= 10 THEN '✅ PASS - ' || COUNT(*)::text || ' document types available'
        ELSE '⚠️  WARN - Only ' || COUNT(*)::text || ' document types found'
    END as validation_result
FROM "ob-poc".document_types;

-- Check document issuers
SELECT
    '3.3 Document Issuers:' as test_section,
    COUNT(*) as total_issuers,
    CASE
        WHEN COUNT(*) >= 10 THEN '✅ PASS - ' || COUNT(*)::text || ' document issuers registered'
        ELSE '⚠️  WARN - Only ' || COUNT(*)::text || ' document issuers found'
    END as validation_result
FROM "ob-poc".document_issuers;

\echo ''

-- ============================================================================
-- 4. DSL VERB VALIDATION
-- ============================================================================

\echo '4. DSL VERB VALIDATION'
\echo '----------------------'

-- Count verbs by domain
SELECT
    '4.1 Verb Distribution:' as test_section,
    domain,
    COUNT(*) as verb_count
FROM "ob-poc".domain_vocabularies
WHERE active = true
GROUP BY domain
ORDER BY COUNT(*) DESC;

-- Verify Document domain verbs
SELECT
    '4.2 Document Verbs:' as test_section,
    CASE
        WHEN COUNT(*) >= 8 THEN '✅ PASS - ' || COUNT(*)::text || ' document verbs found'
        ELSE '❌ FAIL - Expected 8 document verbs, found ' || COUNT(*)::text
    END as validation_result,
    STRING_AGG(verb, ', ' ORDER BY verb) as available_verbs
FROM "ob-poc".domain_vocabularies
WHERE domain = 'document' AND active = true;

-- Verify ISDA domain verbs
SELECT
    '4.3 ISDA Verbs:' as test_section,
    CASE
        WHEN COUNT(*) >= 12 THEN '✅ PASS - ' || COUNT(*)::text || ' ISDA verbs found'
        ELSE '❌ FAIL - Expected 12 ISDA verbs, found ' || COUNT(*)::text
    END as validation_result
FROM "ob-poc".domain_vocabularies
WHERE domain = 'isda' AND active = true;

-- Check verb registry consistency
SELECT
    '4.4 Verb Registry Consistency:' as test_section,
    CASE
        WHEN dv_count = vr_count THEN '✅ PASS - All verbs registered consistently'
        ELSE '❌ FAIL - Inconsistency: ' || dv_count::text || ' domain verbs vs ' || vr_count::text || ' registry entries'
    END as validation_result
FROM (
    SELECT
        COUNT(*) as dv_count
    FROM "ob-poc".domain_vocabularies
    WHERE active = true
) dv,
(
    SELECT
        COUNT(*) as vr_count
    FROM "ob-poc".verb_registry
) vr;

\echo ''

-- ============================================================================
-- 5. SEMANTIC METADATA VALIDATION
-- ============================================================================

\echo '5. SEMANTIC METADATA VALIDATION'
\echo '--------------------------------'

-- Check semantic metadata coverage
SELECT
    '5.1 Semantic Coverage:' as test_section,
    vs.domain,
    COUNT(*) as verbs_with_semantics,
    dv.total_verbs,
    ROUND(100.0 * COUNT(*) / dv.total_verbs, 1) || '%' as coverage_percentage
FROM "ob-poc".verb_semantics vs
JOIN (
    SELECT domain, COUNT(*) as total_verbs
    FROM "ob-poc".domain_vocabularies
    WHERE active = true
    GROUP BY domain
) dv ON vs.domain = dv.domain
GROUP BY vs.domain, dv.total_verbs
ORDER BY COUNT(*) DESC;

-- Check ISDA semantic metadata
SELECT
    '5.2 ISDA Semantic Metadata:' as test_section,
    COUNT(*) as isda_semantic_count,
    CASE
        WHEN COUNT(*) >= 4 THEN '✅ PASS - ' || COUNT(*)::text || ' ISDA verbs have semantic metadata'
        ELSE '⚠️  WARN - Only ' || COUNT(*)::text || ' ISDA verbs have semantic metadata'
    END as validation_result
FROM "ob-poc".verb_semantics
WHERE domain = 'isda';

\echo ''

-- ============================================================================
-- 6. CROSS-DOMAIN RELATIONSHIP VALIDATION
-- ============================================================================

\echo '6. CROSS-DOMAIN RELATIONSHIP VALIDATION'
\echo '----------------------------------------'

-- Check verb relationships exist
SELECT
    '6.1 Cross-Domain Relationships:' as test_section,
    COUNT(*) as total_relationships,
    COUNT(DISTINCT source_domain) as source_domains,
    COUNT(DISTINCT target_domain) as target_domains,
    CASE
        WHEN COUNT(*) >= 10 THEN '✅ PASS - ' || COUNT(*)::text || ' cross-domain relationships defined'
        ELSE '⚠️  WARN - Only ' || COUNT(*)::text || ' cross-domain relationships found'
    END as validation_result
FROM "ob-poc".verb_relationships;

-- Check Document-ISDA integration relationships
SELECT
    '6.2 Document-ISDA Integration:' as test_section,
    COUNT(*) as document_isda_relationships,
    CASE
        WHEN COUNT(*) >= 3 THEN '✅ PASS - Document-ISDA integration relationships found'
        ELSE '⚠️  WARN - Limited Document-ISDA integration relationships'
    END as validation_result
FROM "ob-poc".verb_relationships
WHERE (source_domain = 'document' AND target_domain = 'isda')
   OR (source_domain = 'isda' AND target_domain = 'document');

\echo ''

-- ============================================================================
-- 7. REFERENTIAL INTEGRITY VALIDATION
-- ============================================================================

\echo '7. REFERENTIAL INTEGRITY VALIDATION'
\echo '------------------------------------'

-- Check for orphaned verb references in relationships
SELECT
    '7.1 Orphaned Verb References:' as test_section,
    CASE
        WHEN COUNT(*) = 0 THEN '✅ PASS - No orphaned verb references found'
        ELSE '❌ FAIL - ' || COUNT(*)::text || ' orphaned verb references found'
    END as validation_result,
    CASE
        WHEN COUNT(*) > 0 THEN STRING_AGG(source_domain || '.' || source_verb, ', ')
        ELSE NULL
    END as orphaned_verbs
FROM "ob-poc".verb_relationships vr
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".domain_vocabularies dv
    WHERE dv.domain = vr.source_domain AND dv.verb = vr.source_verb
);

-- Check AttributeID referential integrity in dictionary
SELECT
    '7.2 AttributeID UUID Format:' as test_section,
    COUNT(*) as total_attributes,
    COUNT(CASE WHEN attribute_id::text ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN 1 END) as valid_uuids,
    CASE
        WHEN COUNT(*) = COUNT(CASE WHEN attribute_id::text ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN 1 END)
        THEN '✅ PASS - All AttributeIDs have valid UUID format'
        ELSE '❌ FAIL - Some AttributeIDs have invalid UUID format'
    END as validation_result
FROM "ob-poc".dictionary;

\echo ''

-- ============================================================================
-- 8. PERFORMANCE & INDEXING VALIDATION
-- ============================================================================

\echo '8. PERFORMANCE & INDEXING VALIDATION'
\echo '-------------------------------------'

-- Check critical indexes exist
SELECT
    '8.1 Critical Indexes:' as test_section,
    schemaname,
    tablename,
    indexname,
    'EXISTS' as status
FROM pg_indexes
WHERE schemaname = 'ob-poc'
  AND (tablename LIKE '%document%'
       OR tablename IN ('dictionary', 'domain_vocabularies', 'verb_registry', 'verb_semantics'))
ORDER BY tablename, indexname;

-- Check GIN indexes for JSONB columns
SELECT
    '8.2 JSONB GIN Indexes:' as test_section,
    COUNT(*) as gin_indexes_count,
    CASE
        WHEN COUNT(*) >= 2 THEN '✅ PASS - JSONB GIN indexes found'
        ELSE '⚠️  WARN - Limited JSONB GIN indexes may affect performance'
    END as validation_result
FROM pg_indexes
WHERE schemaname = 'ob-poc'
  AND indexdef LIKE '%gin%'
  AND indexdef LIKE '%jsonb%';

\echo ''

-- ============================================================================
-- 9. SAMPLE DATA VALIDATION
-- ============================================================================

\echo '9. SAMPLE DATA VALIDATION'
\echo '--------------------------'

-- Check if sample documents exist
SELECT
    '9.1 Sample Documents:' as test_section,
    COUNT(*) as sample_count,
    CASE
        WHEN COUNT(*) >= 2 THEN '✅ PASS - ' || COUNT(*)::text || ' sample documents found'
        ELSE '⚠️  WARN - Only ' || COUNT(*)::text || ' sample documents found'
    END as validation_result
FROM "ob-poc".document_catalog;

-- Check document type usage
SELECT
    '9.2 Document Type Usage:' as test_section,
    dt.type_name,
    COALESCE(dc.usage_count, 0) as documents_cataloged
FROM "ob-poc".document_types dt
LEFT JOIN (
    SELECT document_type_id, COUNT(*) as usage_count
    FROM "ob-poc".document_catalog
    GROUP BY document_type_id
) dc ON dt.type_id = dc.document_type_id
ORDER BY COALESCE(dc.usage_count, 0) DESC;

\echo ''

-- ============================================================================
-- 10. OVERALL SYSTEM READINESS
-- ============================================================================

\echo '10. OVERALL SYSTEM READINESS ASSESSMENT'
\echo '========================================='

-- Comprehensive readiness check
WITH readiness_metrics AS (
    SELECT
        'Domains' as component,
        CASE WHEN COUNT(*) >= 7 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".dsl_domains WHERE active = true

    UNION ALL

    SELECT
        'AttributeIDs' as component,
        CASE WHEN COUNT(*) >= 70 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".dictionary

    UNION ALL

    SELECT
        'Document Infrastructure' as component,
        CASE WHEN COUNT(*) >= 5 THEN 1 ELSE 0 END as ready
    FROM information_schema.tables
    WHERE table_schema = 'ob-poc' AND table_name LIKE '%document%'

    UNION ALL

    SELECT
        'DSL Verbs' as component,
        CASE WHEN COUNT(*) >= 20 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".domain_vocabularies WHERE active = true

    UNION ALL

    SELECT
        'ISDA Verbs' as component,
        CASE WHEN COUNT(*) >= 12 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".domain_vocabularies WHERE domain = 'isda' AND active = true

    UNION ALL

    SELECT
        'Semantic Metadata' as component,
        CASE WHEN COUNT(*) >= 10 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".verb_semantics
)
SELECT
    'PHASE 3 READINESS SCORE:' as assessment,
    SUM(ready)::text || '/6' as score,
    ROUND(100.0 * SUM(ready) / COUNT(*), 1) || '%' as percentage,
    CASE
        WHEN SUM(ready) = COUNT(*) THEN '🎉 PHASE 3 COMPLETE - ALL SYSTEMS OPERATIONAL'
        WHEN SUM(ready) >= COUNT(*) * 0.8 THEN '✅ PHASE 3 MOSTLY COMPLETE - READY FOR PHASE 4'
        WHEN SUM(ready) >= COUNT(*) * 0.6 THEN '⚠️  PHASE 3 IN PROGRESS - SOME COMPONENTS MISSING'
        ELSE '❌ PHASE 3 INCOMPLETE - MAJOR COMPONENTS MISSING'
    END as status
FROM readiness_metrics;

-- Detailed component status
SELECT
    '   Component Status:' as detail,
    component,
    CASE WHEN ready = 1 THEN '✅ READY' ELSE '❌ NOT READY' END as status
FROM (
    SELECT
        'Domains' as component,
        CASE WHEN COUNT(*) >= 7 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".dsl_domains WHERE active = true

    UNION ALL

    SELECT
        'AttributeIDs' as component,
        CASE WHEN COUNT(*) >= 70 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".dictionary

    UNION ALL

    SELECT
        'Document Infrastructure' as component,
        CASE WHEN COUNT(*) >= 5 THEN 1 ELSE 0 END as ready
    FROM information_schema.tables
    WHERE table_schema = 'ob-poc' AND table_name LIKE '%document%'

    UNION ALL

    SELECT
        'DSL Verbs' as component,
        CASE WHEN COUNT(*) >= 20 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".domain_vocabularies WHERE active = true

    UNION ALL

    SELECT
        'ISDA Verbs' as component,
        CASE WHEN COUNT(*) >= 12 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".domain_vocabularies WHERE domain = 'isda' AND active = true

    UNION ALL

    SELECT
        'Semantic Metadata' as component,
        CASE WHEN COUNT(*) >= 10 THEN 1 ELSE 0 END as ready
    FROM "ob-poc".verb_semantics
) component_check
ORDER BY component;

\echo ''
\echo '========================================================================='
\echo 'PHASE 3 VALIDATION COMPLETE'
\echo ''
\echo 'Next Steps:'
\echo '- Review any failed validations above'
\echo '- Test workflow examples with database'
\echo '- Proceed to Phase 4: Integration & Testing'
\echo '========================================================================='
