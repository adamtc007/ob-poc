-- 108_create_legacy_schema_compat_views.sql
-- Creates compatibility views in legacy schemas so old SQL can continue to
-- reference kyc./custody./client_portal. during rollout.

BEGIN;

DO $$
DECLARE
    r RECORD;
    relkind CHAR;
BEGIN
    FOR r IN
        SELECT source_schema, source_table, target_table
        FROM "ob-poc".schema_consolidation_table_map
        ORDER BY source_schema, source_table
    LOOP
        SELECT c.relkind
          INTO relkind
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = r.source_schema
           AND c.relname = r.source_table
         LIMIT 1;

        -- If a legacy relation still exists as a base table, this migration is out of order.
        IF relkind = 'r' THEN
            RAISE EXCEPTION
                'Cannot create compatibility view %.%: base table still exists in legacy schema',
                r.source_schema, r.source_table;
        END IF;

        -- Drop existing view and recreate deterministically.
        IF relkind = 'v' THEN
            EXECUTE format('DROP VIEW %I.%I', r.source_schema, r.source_table);
        END IF;

        EXECUTE format(
            'CREATE VIEW %I.%I AS SELECT * FROM "ob-poc".%I',
            r.source_schema,
            r.source_table,
            r.target_table
        );
    END LOOP;
END $$;

COMMIT;
