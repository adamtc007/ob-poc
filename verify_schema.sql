-- Schema Verification Script for OB-POC Database
-- This script verifies the integrity and completeness of the ob-poc schema
-- Run after schema creation to ensure all components are properly installed

\echo '======================================='
\echo 'OB-POC Schema Verification'
\echo '======================================='

-- Check if schema exists
\echo 'Checking schema existence...'
SELECT CASE
    WHEN EXISTS (SELECT 1 FROM information_schema.schemata WHERE schema_name = 'ob-poc')
    THEN 'PASS: ob-poc schema exists'
    ELSE 'FAIL: ob-poc schema missing'
END as schema_check;

-- Count total tables
\echo 'Counting tables...'
SELECT
    count(*) as total_tables,
    CASE
        WHEN count(*) >= 55 THEN 'PASS: Expected 55+ tables'
        ELSE 'FAIL: Missing tables - expected 55+, found ' || count(*)
    END as table_count_check
FROM information_schema.tables
WHERE table_schema = 'ob-poc' AND table_type = 'BASE TABLE';

-- Check core tables exist
\echo 'Verifying core tables...'
WITH core_tables AS (
    SELECT unnest(ARRAY[
        'cbus', 'dictionary', 'attribute_values', 'entities',
        'dsl_ob', 'dsl_versions', 'dsl_domains', 'domain_vocabularies',
        'products', 'services', 'ubo_registry', 'document_catalog'
    ]) as required_table
),
existing_tables AS (
    SELECT table_name
    FROM information_schema.tables
    WHERE table_schema = 'ob-poc'
)
SELECT
    ct.required_table,
    CASE
        WHEN et.table_name IS NOT NULL THEN 'EXISTS'
        ELSE 'MISSING'
    END as status
FROM core_tables ct
LEFT JOIN existing_tables et ON ct.required_table = et.table_name
ORDER BY ct.required_table;

-- Check entity type tables
\echo 'Verifying entity type tables...'
SELECT
    count(*) as entity_tables,
    CASE
        WHEN count(*) >= 4 THEN 'PASS: Entity tables present'
        ELSE 'WARN: Some entity tables missing'
    END as entity_check
FROM information_schema.tables
WHERE table_schema = 'ob-poc'
    AND table_name LIKE 'entity_%'
    AND table_type = 'BASE TABLE';

-- Check document library tables (V3.1)
\echo 'Verifying document library...'
SELECT
    count(*) as document_tables
FROM information_schema.tables
WHERE table_schema = 'ob-poc'
    AND table_name LIKE 'document_%'
    AND table_type = 'BASE TABLE';

-- Check DSL management tables
\echo 'Verifying DSL management...'
WITH dsl_tables AS (
    SELECT unnest(ARRAY[
        'dsl_ob', 'dsl_versions', 'dsl_domains', 'domain_vocabularies',
        'dsl_examples', 'dsl_execution_log', 'parsed_asts'
    ]) as required_table
),
existing_dsl AS (
    SELECT table_name
    FROM information_schema.tables
    WHERE table_schema = 'ob-poc' AND table_name LIKE 'dsl_%'
)
SELECT
    dt.required_table,
    CASE
        WHEN ed.table_name IS NOT NULL THEN 'EXISTS'
        ELSE 'MISSING'
    END as status
FROM dsl_tables dt
LEFT JOIN existing_dsl ed ON dt.required_table = ed.table_name
ORDER BY dt.required_table;

-- Check primary keys
\echo 'Verifying primary keys...'
SELECT
    count(*) as tables_with_pk,
    CASE
        WHEN count(*) >= 50 THEN 'PASS: Most tables have PKs'
        ELSE 'WARN: Some tables missing primary keys'
    END as pk_check
FROM information_schema.table_constraints tc
JOIN information_schema.tables t ON t.table_name = tc.table_name
WHERE tc.constraint_type = 'PRIMARY KEY'
    AND tc.table_schema = 'ob-poc'
    AND t.table_schema = 'ob-poc'
    AND t.table_type = 'BASE TABLE';

-- Check foreign keys
\echo 'Verifying foreign key relationships...'
SELECT
    count(*) as foreign_keys,
    CASE
        WHEN count(*) >= 20 THEN 'PASS: Adequate foreign key relationships'
        ELSE 'WARN: May need more foreign key constraints'
    END as fk_check
FROM information_schema.table_constraints
WHERE constraint_type = 'FOREIGN KEY'
    AND table_schema = 'ob-poc';

-- Check indexes
\echo 'Verifying indexes...'
SELECT
    count(*) as total_indexes,
    CASE
        WHEN count(*) >= 30 THEN 'PASS: Good index coverage'
        ELSE 'WARN: Consider adding more indexes'
    END as index_check
FROM pg_indexes
WHERE schemaname = 'ob-poc';

-- Check UUID extension
\echo 'Verifying UUID support...'
SELECT CASE
    WHEN EXISTS (
        SELECT 1 FROM pg_extension WHERE extname = 'uuid-ossp'
    ) OR EXISTS (
        SELECT 1 FROM pg_proc WHERE proname = 'gen_random_uuid'
    )
    THEN 'PASS: UUID generation available'
    ELSE 'WARN: UUID extension may be missing'
END as uuid_check;

-- Sample data checks (if seed data loaded)
\echo 'Checking sample data...'
SELECT
    'dictionary' as table_name,
    count(*) as row_count,
    CASE
        WHEN count(*) >= 50 THEN 'PASS: Dictionary populated'
        WHEN count(*) > 0 THEN 'PARTIAL: Some dictionary entries'
        ELSE 'EMPTY: No dictionary data'
    END as data_status
FROM "ob-poc".dictionary
UNION ALL
SELECT
    'entity_types' as table_name,
    count(*) as row_count,
    CASE
        WHEN count(*) >= 3 THEN 'PASS: Entity types populated'
        WHEN count(*) > 0 THEN 'PARTIAL: Some entity types'
        ELSE 'EMPTY: No entity types'
    END as data_status
FROM "ob-poc".entity_types
UNION ALL
SELECT
    'products' as table_name,
    count(*) as row_count,
    CASE
        WHEN count(*) > 0 THEN 'POPULATED: Products available'
        ELSE 'EMPTY: No products'
    END as data_status
FROM "ob-poc".products;

-- Schema size summary
\echo 'Schema size summary...'
SELECT
    schemaname,
    count(*) as total_objects,
    sum(CASE WHEN tablename IS NOT NULL THEN 1 ELSE 0 END) as tables,
    count(*) - sum(CASE WHEN tablename IS NOT NULL THEN 1 ELSE 0 END) as other_objects
FROM pg_tables
WHERE schemaname = 'ob-poc'
GROUP BY schemaname;

-- Version info
\echo 'Database version info...'
SELECT version() as postgresql_version;

\echo '======================================='
\echo 'Verification Complete'
\echo '======================================='
\echo 'Next steps:'
\echo '1. Load seed data: psql -f sql/seed_dictionary_attributes.sql'
\echo '2. Load CBU data: psql -f sql/seed_cbus.sql'
\echo '3. Test connection: cargo run --bin test_db_connection'
\echo '======================================='
