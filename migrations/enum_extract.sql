-- =============================================================
-- enum_extract.sql
-- Extracts all PostgreSQL enum types and their values
-- Feed output to Sonnet for P1-B (FSM cross-check vs DB enums)
-- =============================================================

SELECT
    t.typname    AS enum_name,
    e.enumlabel  AS enum_value,
    e.enumsortorder AS sort_order
FROM pg_type t
JOIN pg_enum e ON t.oid = e.enumtypid
JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
WHERE n.nspname = 'public'  -- adjust if using a named schema
ORDER BY t.typname, e.enumsortorder;
