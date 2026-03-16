-- =============================================================
-- table_inventory.sql
-- Full table inventory with estimated row counts and column counts
-- Use for scoping: which tables go into which P3 session
-- =============================================================

SELECT
    t.table_name,
    -- Estimated row count from pg_stat (avoids full table scans)
    COALESCE(s.n_live_tup, 0) AS estimated_rows,
    -- Column count
    (SELECT COUNT(*)
     FROM information_schema.columns c
     WHERE c.table_schema = t.table_schema
       AND c.table_name = t.table_name
    ) AS column_count,
    -- Has PK?
    EXISTS (
        SELECT 1 FROM information_schema.table_constraints tc
        WHERE tc.table_schema = t.table_schema
          AND tc.table_name = t.table_name
          AND tc.constraint_type = 'PRIMARY KEY'
    ) AS has_pk,
    -- FK count (as child)
    (SELECT COUNT(*) FROM information_schema.table_constraints tc
     WHERE tc.table_schema = t.table_schema
       AND tc.table_name = t.table_name
       AND tc.constraint_type = 'FOREIGN KEY'
    ) AS fk_count,
    -- Index count
    (SELECT COUNT(DISTINCT indexname)
     FROM pg_indexes pi
     WHERE pi.schemaname = t.table_schema
       AND pi.tablename = t.table_name
    ) AS index_count
FROM information_schema.tables t
LEFT JOIN pg_stat_user_tables s
    ON s.schemaname = t.table_schema
    AND s.relname = t.table_name
WHERE t.table_schema = 'public'  -- adjust if using a named schema
    AND t.table_type = 'BASE TABLE'
ORDER BY t.table_name;
