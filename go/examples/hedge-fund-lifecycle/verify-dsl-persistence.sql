-- ============================================================================
-- DSL PERSISTENCE VERIFICATION SCRIPT
-- ============================================================================
--
-- This script verifies that the hedge fund investor DSL persistence layer
-- is correctly configured and operational.
--
-- Table: "hf-investor".hf_dsl_executions
-- Purpose: Store all DSL operations for audit trail and replay capability
--
-- Usage:
--   psql "$DB_CONN_STRING" -f verify-dsl-persistence.sql
-- ============================================================================

\echo '═══════════════════════════════════════════════════════════════════════'
\echo 'DSL PERSISTENCE VERIFICATION'
\echo '═══════════════════════════════════════════════════════════════════════'
\echo ''

-- ----------------------------------------------------------------------------
-- 1. VERIFY SCHEMA EXISTS
-- ----------------------------------------------------------------------------
\echo '1. Checking if hf-investor schema exists...'
\echo ''

SELECT
    CASE
        WHEN EXISTS (
            SELECT 1
            FROM information_schema.schemata
            WHERE schema_name = 'hf-investor'
        )
        THEN '✓ Schema "hf-investor" exists'
        ELSE '✗ Schema "hf-investor" NOT FOUND - Run migration first!'
    END as status;

\echo ''

-- ----------------------------------------------------------------------------
-- 2. VERIFY TABLE EXISTS
-- ----------------------------------------------------------------------------
\echo '2. Checking if hf_dsl_executions table exists...'
\echo ''

SELECT
    CASE
        WHEN EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'hf-investor'
              AND table_name = 'hf_dsl_executions'
        )
        THEN '✓ Table "hf-investor".hf_dsl_executions exists'
        ELSE '✗ Table NOT FOUND - Run: psql "$DB_URL" -f sql/init.sql'
    END as status;

\echo ''

-- ----------------------------------------------------------------------------
-- 3. SHOW TABLE STRUCTURE
-- ----------------------------------------------------------------------------
\echo '3. Table Structure:'
\echo ''

\d "hf-investor".hf_dsl_executions

\echo ''

-- ----------------------------------------------------------------------------
-- 4. VERIFY INDEXES
-- ----------------------------------------------------------------------------
\echo '4. Checking indexes for optimal query performance...'
\echo ''

SELECT
    indexname as index_name,
    indexdef as definition
FROM pg_indexes
WHERE schemaname = 'hf-investor'
  AND tablename = 'hf_dsl_executions'
ORDER BY indexname;

\echo ''

-- ----------------------------------------------------------------------------
-- 5. CHECK CURRENT DSL EXECUTION COUNT
-- ----------------------------------------------------------------------------
\echo '5. Current DSL Execution Statistics:'
\echo ''

SELECT
    COUNT(*) as total_executions,
    COUNT(DISTINCT investor_id) as unique_investors,
    COUNT(CASE WHEN execution_status = 'COMPLETED' THEN 1 END) as completed,
    COUNT(CASE WHEN execution_status = 'FAILED' THEN 1 END) as failed,
    COUNT(CASE WHEN execution_status = 'PENDING' THEN 1 END) as pending,
    MIN(created_at) as earliest_execution,
    MAX(created_at) as latest_execution
FROM "hf-investor".hf_dsl_executions;

\echo ''

-- ----------------------------------------------------------------------------
-- 6. SAMPLE DSL OPERATIONS (Latest 10)
-- ----------------------------------------------------------------------------
\echo '6. Latest DSL Operations (Last 10):'
\echo ''

SELECT
    execution_id,
    LEFT(dsl_text, 60) || '...' as operation,
    execution_status,
    execution_time_ms,
    triggered_by,
    TO_CHAR(completed_at, 'YYYY-MM-DD HH24:MI:SS') as completed_at
FROM "hf-investor".hf_dsl_executions
ORDER BY created_at DESC
LIMIT 10;

\echo ''

-- ----------------------------------------------------------------------------
-- 7. DSL OPERATIONS BY VERB TYPE
-- ----------------------------------------------------------------------------
\echo '7. DSL Operations by Verb Type:'
\echo ''

WITH verb_extract AS (
    SELECT
        SUBSTRING(dsl_text FROM '^\(([a-z]+\.[a-z\-]+)') as verb,
        execution_status,
        execution_time_ms
    FROM "hf-investor".hf_dsl_executions
)
SELECT
    verb,
    COUNT(*) as usage_count,
    COUNT(CASE WHEN execution_status = 'COMPLETED' THEN 1 END) as successful,
    ROUND(AVG(execution_time_ms), 2) as avg_exec_time_ms
FROM verb_extract
WHERE verb IS NOT NULL
GROUP BY verb
ORDER BY usage_count DESC;

\echo ''

-- ----------------------------------------------------------------------------
-- 8. SAMPLE QUERY: RETRIEVE INVESTOR LIFECYCLE
-- ----------------------------------------------------------------------------
\echo '8. Sample Query - Retrieve Complete Investor Lifecycle:'
\echo ''
\echo '   Usage: Replace <investor_id> with actual UUID'
\echo ''
\echo '   SELECT'
\echo '     execution_id,'
\echo '     dsl_text,'
\echo '     execution_status,'
\echo '     triggered_by,'
\echo '     completed_at'
\echo '   FROM "hf-investor".hf_dsl_executions'
\echo '   WHERE investor_id = ''<investor_id>'''
\echo '   ORDER BY created_at ASC;'
\echo ''

-- ----------------------------------------------------------------------------
-- 9. SAMPLE QUERY: FAILED OPERATIONS
-- ----------------------------------------------------------------------------
\echo '9. Failed Operations (if any):'
\echo ''

SELECT
    execution_id,
    LEFT(dsl_text, 50) || '...' as operation,
    error_details,
    triggered_by,
    TO_CHAR(started_at, 'YYYY-MM-DD HH24:MI:SS') as failed_at
FROM "hf-investor".hf_dsl_executions
WHERE execution_status = 'FAILED'
ORDER BY started_at DESC
LIMIT 10;

\echo ''

-- ----------------------------------------------------------------------------
-- 10. PERFORMANCE METRICS
-- ----------------------------------------------------------------------------
\echo '10. Execution Performance Metrics:'
\echo ''

SELECT
    COUNT(*) as total_ops,
    MIN(execution_time_ms) as fastest_ms,
    MAX(execution_time_ms) as slowest_ms,
    ROUND(AVG(execution_time_ms), 2) as avg_ms,
    ROUND(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY execution_time_ms), 2) as median_ms,
    ROUND(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms), 2) as p95_ms
FROM "hf-investor".hf_dsl_executions
WHERE execution_status = 'COMPLETED'
  AND execution_time_ms IS NOT NULL;

\echo ''

-- ----------------------------------------------------------------------------
-- 11. IDEMPOTENCY CHECK
-- ----------------------------------------------------------------------------
\echo '11. Idempotency Key Usage:'
\echo ''

SELECT
    COUNT(*) as total_executions,
    COUNT(DISTINCT idempotency_key) as unique_keys,
    COUNT(idempotency_key) as executions_with_keys,
    COUNT(*) - COUNT(idempotency_key) as executions_without_keys,
    CASE
        WHEN COUNT(idempotency_key) > COUNT(DISTINCT idempotency_key)
        THEN 'WARNING: Duplicate idempotency keys found!'
        ELSE '✓ All idempotency keys unique'
    END as idempotency_status
FROM "hf-investor".hf_dsl_executions;

\echo ''

-- ----------------------------------------------------------------------------
-- 12. SAMPLE INSERT STATEMENT
-- ----------------------------------------------------------------------------
\echo '12. Sample Insert Statement for Testing:'
\echo ''
\echo '   -- Insert a test DSL execution'
\echo '   INSERT INTO "hf-investor".hf_dsl_executions ('
\echo '     execution_id,'
\echo '     investor_id,'
\echo '     dsl_text,'
\echo '     execution_status,'
\echo '     idempotency_key,'
\echo '     triggered_by,'
\echo '     execution_engine,'
\echo '     affected_entities,'
\echo '     execution_time_ms,'
\echo '     started_at,'
\echo '     completed_at'
\echo '   ) VALUES ('
\echo '     gen_random_uuid(),'
\echo '     ''a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d''::uuid,'
\echo '     ''(investor.start-opportunity'
\echo '       :legal-name "Test Investor LP"'
\echo '       :type "CORPORATE"'
\echo '       :domicile "US")'''
\echo '     ''COMPLETED'','
\echo '     ''test-key-'' || gen_random_uuid()::text,'
\echo '     ''test@example.com'','
\echo '     ''hedge-fund-dsl-v1'','
\echo '     ''{"investor_code": "INV-TEST-001"}''::jsonb,'
\echo '     42,'
\echo '     now(),'
\echo '     now()'
\echo '   );'
\echo ''

-- ----------------------------------------------------------------------------
-- 13. TABLE SIZE AND STATISTICS
-- ----------------------------------------------------------------------------
\echo '13. Table Size and Statistics:'
\echo ''

SELECT
    pg_size_pretty(pg_total_relation_size('"hf-investor".hf_dsl_executions')) as total_size,
    pg_size_pretty(pg_relation_size('"hf-investor".hf_dsl_executions')) as table_size,
    pg_size_pretty(pg_indexes_size('"hf-investor".hf_dsl_executions')) as indexes_size,
    (SELECT COUNT(*) FROM "hf-investor".hf_dsl_executions) as row_count;

\echo ''

-- ----------------------------------------------------------------------------
-- 14. VERIFICATION SUMMARY
-- ----------------------------------------------------------------------------
\echo '═══════════════════════════════════════════════════════════════════════'
\echo 'VERIFICATION SUMMARY'
\echo '═══════════════════════════════════════════════════════════════════════'
\echo ''

SELECT
    CASE
        WHEN EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'hf-investor'
              AND table_name = 'hf_dsl_executions'
        )
        THEN '✓ DSL Persistence Layer is OPERATIONAL'
        ELSE '✗ DSL Persistence Layer NOT FOUND'
    END as status;

\echo ''
\echo 'Next Steps:'
\echo '  1. Execute hedge fund CLI commands to generate DSL operations'
\echo '  2. Query hf_dsl_executions to see stored operations'
\echo '  3. Review examples/hedge-fund-lifecycle/ for sample workflows'
\echo ''
\echo 'Related Files:'
\echo '  - SQL Schema: hedge-fund-investor-source/sql/migration_hedge_fund_investor.sql'
\echo '  - DSL Examples: examples/hedge-fund-lifecycle/complete-lifecycle-example.dsl'
\echo '  - JSON Plan: examples/hedge-fund-lifecycle/lifecycle-plan.json'
\echo '  - README: examples/hedge-fund-lifecycle/README.md'
\echo ''
\echo '═══════════════════════════════════════════════════════════════════════'
