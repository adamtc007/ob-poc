-- =============================================================================
-- Clearstream Investor Register Schema Verification Script
-- =============================================================================
-- Purpose: Verify that the ob-poc schema supports Clearstream CASCADE-RS/Vestima
--          data ingestion and BODS mapping
-- Run with: psql -d data_designer -f scripts/verify_clearstream_schema.sql
-- =============================================================================

\echo '=============================================='
\echo 'Clearstream Schema Verification Report'
\echo '=============================================='
\echo ''

-- -----------------------------------------------------------------------------
-- 1. Core Tables Existence Check
-- -----------------------------------------------------------------------------
\echo '1. CORE TABLES CHECK'
\echo '--------------------'

SELECT 
    table_schema,
    table_name,
    CASE 
        WHEN table_name IS NOT NULL THEN '✓ EXISTS'
        ELSE '✗ MISSING'
    END as status
FROM information_schema.tables
WHERE (table_schema = 'kyc' AND table_name IN ('share_classes', 'holdings', 'movements'))
   OR (table_schema = 'ob-poc' AND table_name IN ('entities', 'entity_identifiers', 'entity_relationships'))
ORDER BY table_schema, table_name;

-- -----------------------------------------------------------------------------
-- 2. Share Classes Table Structure (Clearstream Fund Data)
-- -----------------------------------------------------------------------------
\echo ''
\echo '2. SHARE_CLASSES TABLE (Clearstream Fund Master)'
\echo '-------------------------------------------------'

SELECT 
    column_name,
    data_type,
    character_maximum_length,
    is_nullable,
    column_default
FROM information_schema.columns
WHERE table_schema = 'kyc' AND table_name = 'share_classes'
ORDER BY ordinal_position;

-- -----------------------------------------------------------------------------
-- 3. Holdings Table Structure (Clearstream Register Positions)
-- -----------------------------------------------------------------------------
\echo ''
\echo '3. HOLDINGS TABLE (Clearstream_Register_Positions)'
\echo '---------------------------------------------------'

SELECT 
    column_name,
    data_type,
    character_maximum_length,
    is_nullable,
    column_default
FROM information_schema.columns
WHERE table_schema = 'kyc' AND table_name = 'holdings'
ORDER BY ordinal_position;

-- -----------------------------------------------------------------------------
-- 4. Movements Table Structure (Clearstream Movement Log)
-- -----------------------------------------------------------------------------
\echo ''
\echo '4. MOVEMENTS TABLE (Clearstream_Movement_Log)'
\echo '----------------------------------------------'

SELECT 
    column_name,
    data_type,
    character_maximum_length,
    is_nullable,
    column_default
FROM information_schema.columns
WHERE table_schema = 'kyc' AND table_name = 'movements'
ORDER BY ordinal_position;

-- -----------------------------------------------------------------------------
-- 5. Entity Identifiers Table (LEI Spine + Clearstream KV)
-- -----------------------------------------------------------------------------
\echo ''
\echo '5. ENTITY_IDENTIFIERS TABLE (LEI + Clearstream IDs)'
\echo '----------------------------------------------------'

SELECT 
    column_name,
    data_type,
    character_maximum_length,
    is_nullable,
    column_default
FROM information_schema.columns
WHERE table_schema = 'ob-poc' AND table_name = 'entity_identifiers'
ORDER BY ordinal_position;

-- -----------------------------------------------------------------------------
-- 6. Foreign Key Relationships
-- -----------------------------------------------------------------------------
\echo ''
\echo '6. FOREIGN KEY RELATIONSHIPS'
\echo '----------------------------'

SELECT 
    tc.table_schema,
    tc.table_name,
    kcu.column_name,
    ccu.table_schema AS foreign_schema,
    ccu.table_name AS foreign_table,
    ccu.column_name AS foreign_column
FROM information_schema.table_constraints AS tc
JOIN information_schema.key_column_usage AS kcu
    ON tc.constraint_name = kcu.constraint_name
    AND tc.table_schema = kcu.table_schema
JOIN information_schema.constraint_column_usage AS ccu
    ON ccu.constraint_name = tc.constraint_name
WHERE tc.constraint_type = 'FOREIGN KEY'
  AND tc.table_schema IN ('kyc', 'ob-poc')
  AND tc.table_name IN ('share_classes', 'holdings', 'movements', 'entity_identifiers')
ORDER BY tc.table_schema, tc.table_name;

-- -----------------------------------------------------------------------------
-- 7. Unique Constraints (Idempotency Keys)
-- -----------------------------------------------------------------------------
\echo ''
\echo '7. UNIQUE CONSTRAINTS (Idempotency)'
\echo '------------------------------------'

SELECT 
    tc.table_schema,
    tc.table_name,
    tc.constraint_name,
    string_agg(kcu.column_name, ', ' ORDER BY kcu.ordinal_position) as columns
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu 
    ON tc.constraint_name = kcu.constraint_name 
    AND tc.table_schema = kcu.table_schema
WHERE tc.constraint_type = 'UNIQUE'
  AND tc.table_schema IN ('kyc', 'ob-poc')
  AND tc.table_name IN ('share_classes', 'holdings', 'movements', 'entity_identifiers')
GROUP BY tc.table_schema, tc.table_name, tc.constraint_name
ORDER BY tc.table_schema, tc.table_name;

-- -----------------------------------------------------------------------------
-- 8. Check Constraints (Data Validation)
-- -----------------------------------------------------------------------------
\echo ''
\echo '8. CHECK CONSTRAINTS (Validation Rules)'
\echo '----------------------------------------'

SELECT 
    tc.table_schema,
    tc.table_name,
    tc.constraint_name,
    cc.check_clause
FROM information_schema.table_constraints tc
JOIN information_schema.check_constraints cc 
    ON tc.constraint_name = cc.constraint_name
WHERE tc.constraint_type = 'CHECK'
  AND tc.table_schema IN ('kyc', 'ob-poc')
  AND tc.table_name IN ('share_classes', 'holdings', 'movements', 'entity_identifiers')
  AND tc.constraint_name NOT LIKE '%_not_null'
ORDER BY tc.table_schema, tc.table_name;

-- -----------------------------------------------------------------------------
-- 9. Indexes
-- -----------------------------------------------------------------------------
\echo ''
\echo '9. INDEXES'
\echo '----------'

SELECT 
    schemaname,
    tablename,
    indexname,
    indexdef
FROM pg_indexes
WHERE schemaname IN ('kyc', 'ob-poc')
  AND tablename IN ('share_classes', 'holdings', 'movements', 'entity_identifiers')
ORDER BY schemaname, tablename, indexname;

-- -----------------------------------------------------------------------------
-- 10. Sample Data Counts
-- -----------------------------------------------------------------------------
\echo ''
\echo '10. DATA COUNTS'
\echo '---------------'

SELECT 'kyc.share_classes' as table_name, COUNT(*) as row_count FROM kyc.share_classes
UNION ALL
SELECT 'kyc.holdings', COUNT(*) FROM kyc.holdings
UNION ALL
SELECT 'kyc.movements', COUNT(*) FROM kyc.movements
UNION ALL
SELECT 'ob-poc.entities', COUNT(*) FROM "ob-poc".entities
UNION ALL
SELECT 'ob-poc.entity_identifiers', COUNT(*) FROM "ob-poc".entity_identifiers
ORDER BY table_name;

-- -----------------------------------------------------------------------------
-- 11. Identifier Scheme Distribution
-- -----------------------------------------------------------------------------
\echo ''
\echo '11. IDENTIFIER SCHEMES IN USE'
\echo '------------------------------'

SELECT 
    scheme,
    COUNT(*) as count,
    COUNT(CASE WHEN is_validated THEN 1 END) as validated_count
FROM "ob-poc".entity_identifiers
GROUP BY scheme
ORDER BY count DESC;

-- -----------------------------------------------------------------------------
-- 12. BODS Interest Types Check
-- -----------------------------------------------------------------------------
\echo ''
\echo '12. BODS INTEREST TYPES (for ownership mapping)'
\echo '------------------------------------------------'

SELECT 
    type_code,
    category,
    display_name
FROM "ob-poc".bods_interest_types
ORDER BY display_order
LIMIT 10;

\echo ''
\echo '=============================================='
\echo 'Verification Complete'
\echo '=============================================='
