-- 110_drop_legacy_schemas.sql
-- Final hard cutover: remove compatibility views and drop legacy schemas.

BEGIN;

DO $$
DECLARE
    r RECORD;
BEGIN
    -- Drop compatibility views created in migration 108.
    FOR r IN
        SELECT source_schema, source_table
        FROM "ob-poc".schema_consolidation_table_map
        ORDER BY source_schema, source_table
    LOOP
        EXECUTE format('DROP VIEW IF EXISTS %I.%I', r.source_schema, r.source_table);
    END LOOP;
END $$;

DROP SCHEMA IF EXISTS kyc CASCADE;
DROP SCHEMA IF EXISTS custody CASCADE;
DROP SCHEMA IF EXISTS client_portal CASCADE;

COMMIT;
