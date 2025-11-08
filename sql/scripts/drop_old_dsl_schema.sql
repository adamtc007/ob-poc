-- Drop the legacy schema and lock down search_path to only use "ob-poc".
-- This script is intentionally conservative: it uses RESTRICT so any
-- dangling dependencies will cause an error instead of being dropped.

-- Usage:
--   psql -d ob-poc -v ON_ERROR_STOP=1 -f sql/scripts/drop_old_dsl_schema.sql
--   # If this fails with dependency errors, run the diagnostics below.

BEGIN;

-- Ensure we’re in the right database context by locking search_path afterwards
-- (applies for new sessions once committed)
ALTER DATABASE "ob-poc" SET search_path = "ob-poc", public;

-- Attempt to drop the legacy schema. This will ERROR if any objects in other
-- schemas depend on objects in "dsl-ob-poc".
DROP SCHEMA IF EXISTS "dsl-ob-poc" RESTRICT;

COMMIT;

-- Diagnostics (optional): list objects that still reference "dsl-ob-poc"
-- Views referencing the legacy schema
-- SELECT n.nspname AS dependent_schema,
--        c.relname  AS view_name
-- FROM pg_class c
-- JOIN pg_namespace n ON n.oid = c.relnamespace
-- WHERE c.relkind = 'v'
--   AND pg_get_viewdef(c.oid) LIKE '%"dsl-ob-poc"%';

-- Functions referencing the legacy schema
-- SELECT n.nspname AS dependent_schema,
--        p.proname  AS function_name
-- FROM pg_proc p
-- JOIN pg_namespace n ON n.oid = p.pronamespace
-- WHERE pg_get_functiondef(p.oid) LIKE '%"dsl-ob-poc"%';

-- General dependency graph pointing at the legacy schema
-- SELECT n2.nspname AS dependent_schema,
--        c2.relkind  AS kind,
--        c2.relname  AS name
-- FROM pg_depend d
-- JOIN pg_class  c1 ON d.refobjid = c1.oid
-- JOIN pg_namespace n1 ON n1.oid = c1.relnamespace
-- JOIN pg_class  c2 ON d.objid   = c2.oid
-- JOIN pg_namespace n2 ON n2.oid = c2.relnamespace
-- WHERE n1.nspname = 'dsl-ob-poc'
-- ORDER BY 1, 2, 3;

