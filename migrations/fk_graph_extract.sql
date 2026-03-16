-- =============================================================
-- fk_graph_extract.sql
-- Extracts the FK dependency graph for ob-poc schema review
-- Feed the output to Sonnet for session P3-E (topology only)
-- =============================================================

-- 1. FK RELATIONSHIPS (the core graph)
-- One row per FK constraint: parent → child with cascade policies
SELECT
    ccu.table_schema  AS parent_schema,
    ccu.table_name    AS parent_table,
    ccu.column_name   AS parent_column,
    tc.table_schema   AS child_schema,
    tc.table_name     AS child_table,
    kcu.column_name   AS child_column,
    tc.constraint_name,
    rc.delete_rule    AS on_delete,
    rc.update_rule    AS on_update
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu
    ON tc.constraint_name = kcu.constraint_name
    AND tc.table_schema = kcu.table_schema
JOIN information_schema.constraint_column_usage ccu
    ON ccu.constraint_name = tc.constraint_name
    AND ccu.table_schema = tc.table_schema
JOIN information_schema.referential_constraints rc
    ON rc.constraint_name = tc.constraint_name
    AND rc.constraint_schema = tc.table_schema
WHERE tc.constraint_type = 'FOREIGN KEY'
    AND tc.table_schema = 'public'  -- adjust if using a named schema
ORDER BY parent_table, child_table, child_column;


-- 2. ORPHAN DETECTION
-- Tables with zero FK references (neither parent nor child of any FK)
WITH fk_participants AS (
    SELECT DISTINCT ccu.table_name AS tbl
    FROM information_schema.constraint_column_usage ccu
    JOIN information_schema.table_constraints tc
        ON tc.constraint_name = ccu.constraint_name
        AND tc.table_schema = ccu.table_schema
    WHERE tc.constraint_type = 'FOREIGN KEY'
        AND tc.table_schema = 'public'
    UNION
    SELECT DISTINCT tc.table_name
    FROM information_schema.table_constraints tc
    WHERE tc.constraint_type = 'FOREIGN KEY'
        AND tc.table_schema = 'public'
)
SELECT t.table_name AS orphan_table
FROM information_schema.tables t
LEFT JOIN fk_participants fp ON fp.tbl = t.table_name
WHERE t.table_schema = 'public'
    AND t.table_type = 'BASE TABLE'
    AND fp.tbl IS NULL
ORDER BY t.table_name;


-- 3. HUB TABLES (highest FK fan-in)
-- Tables most referenced by other tables via FKs
SELECT
    ccu.table_name    AS parent_table,
    COUNT(DISTINCT tc.table_name) AS referencing_table_count,
    COUNT(*)          AS total_fk_references
FROM information_schema.table_constraints tc
JOIN information_schema.constraint_column_usage ccu
    ON ccu.constraint_name = tc.constraint_name
    AND ccu.table_schema = tc.table_schema
WHERE tc.constraint_type = 'FOREIGN KEY'
    AND tc.table_schema = 'public'
GROUP BY ccu.table_name
ORDER BY referencing_table_count DESC
LIMIT 20;


-- 4. CASCADE CHAINS (multi-hop DELETE CASCADE paths)
-- First hop: direct CASCADE relationships
-- Feed this to Sonnet and ask it to trace chains > 2 hops
SELECT
    ccu.table_name  AS parent_table,
    tc.table_name   AS child_table,
    rc.delete_rule
FROM information_schema.table_constraints tc
JOIN information_schema.constraint_column_usage ccu
    ON ccu.constraint_name = tc.constraint_name
    AND ccu.table_schema = tc.table_schema
JOIN information_schema.referential_constraints rc
    ON rc.constraint_name = tc.constraint_name
    AND rc.constraint_schema = tc.table_schema
WHERE tc.constraint_type = 'FOREIGN KEY'
    AND tc.table_schema = 'public'
    AND rc.delete_rule = 'CASCADE'
ORDER BY parent_table, child_table;
