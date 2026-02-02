-- Migration: 069_uuidv7_defaults.sql
-- Purpose: Update all UUID column defaults from gen_random_uuid() to uuidv7()
-- PostgreSQL 18 introduces native uuidv7() function for time-ordered UUIDs
--
-- UUIDv7 benefits:
-- - Time-ordered: natural chronological sorting
-- - Better index locality: reduces random I/O
-- - Includes millisecond timestamp: useful for debugging
-- - K-sortable: efficient for distributed systems

BEGIN;

-- Dynamically update all columns that use gen_random_uuid() as default
DO $$
DECLARE
    rec RECORD;
    alter_sql TEXT;
    updated_count INTEGER := 0;
BEGIN
    FOR rec IN
        SELECT table_schema, table_name, column_name
        FROM information_schema.columns
        WHERE column_default LIKE '%gen_random_uuid()%'
        ORDER BY table_schema, table_name, column_name
    LOOP
        alter_sql := format(
            'ALTER TABLE %I.%I ALTER COLUMN %I SET DEFAULT uuidv7()',
            rec.table_schema,
            rec.table_name,
            rec.column_name
        );

        BEGIN
            EXECUTE alter_sql;
            updated_count := updated_count + 1;
            RAISE NOTICE 'Updated: %.%.%', rec.table_schema, rec.table_name, rec.column_name;
        EXCEPTION WHEN OTHERS THEN
            RAISE WARNING 'Failed to update %.%.%: %',
                rec.table_schema, rec.table_name, rec.column_name, SQLERRM;
        END;
    END LOOP;

    RAISE NOTICE 'Total columns updated to uuidv7(): %', updated_count;
END;
$$;

-- Verify the migration
DO $$
DECLARE
    remaining INTEGER;
BEGIN
    SELECT COUNT(*) INTO remaining
    FROM information_schema.columns
    WHERE column_default LIKE '%gen_random_uuid()%';

    IF remaining > 0 THEN
        RAISE WARNING '% columns still using gen_random_uuid()', remaining;
    ELSE
        RAISE NOTICE 'All UUID columns now using uuidv7()';
    END IF;
END;
$$;

COMMIT;
