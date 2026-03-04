-- 106_move_custody_tables_to_ob_poc.sql
-- Moves all custody base tables into "ob-poc" using precomputed mapping.

BEGIN;

DO $$
DECLARE
    r RECORD;
    seq_name TEXT;
    rel_name TEXT;
    new_name TEXT;
BEGIN
    FOR r IN
        SELECT source_table, target_table
        FROM "ob-poc".schema_consolidation_table_map
        WHERE source_schema = 'custody'
        ORDER BY source_table
    LOOP
        IF EXISTS (
            SELECT 1
            FROM information_schema.tables t
            WHERE t.table_schema = 'custody'
              AND t.table_type = 'BASE TABLE'
              AND t.table_name = r.source_table
        ) AND r.source_table <> r.target_table THEN
            EXECUTE format('ALTER TABLE custody.%I RENAME TO %I', r.source_table, r.target_table);
        END IF;

        IF EXISTS (
            SELECT 1
            FROM information_schema.tables t
            WHERE t.table_schema = 'custody'
              AND t.table_type = 'BASE TABLE'
              AND t.table_name = r.target_table
        ) THEN
            FOR rel_name IN
                SELECT c.conname
                FROM pg_constraint c
                WHERE c.conrelid = format('custody.%I', r.target_table)::regclass
            LOOP
                new_name := rel_name;
                IF position(r.source_table IN new_name) > 0 THEN
                    new_name := replace(new_name, r.source_table, r.target_table);
                END IF;
                IF new_name = rel_name AND EXISTS (
                    SELECT 1
                    FROM pg_class pc
                    JOIN pg_namespace pn ON pn.oid = pc.relnamespace
                    WHERE pn.nspname = 'ob-poc'
                      AND pc.relname = rel_name
                ) THEN
                    new_name := r.target_table || '_' || rel_name;
                END IF;
                new_name := lower(substr(new_name, 1, 63));
                IF new_name <> rel_name THEN
                    IF EXISTS (
                        SELECT 1
                        FROM pg_class pc
                        JOIN pg_namespace pn ON pn.oid = pc.relnamespace
                        WHERE pn.nspname IN ('custody', 'ob-poc')
                          AND pc.relname = new_name
                          AND NOT (pn.nspname = 'custody' AND pc.relname = rel_name)
                    ) THEN
                        new_name := substr(new_name, 1, 54) || '_' || substr(md5(r.target_table || '_' || rel_name), 1, 8);
                    END IF;
                    EXECUTE format('ALTER TABLE custody.%I RENAME CONSTRAINT %I TO %I', r.target_table, rel_name, new_name);
                END IF;
            END LOOP;

            FOR rel_name IN
                SELECT i.indexname
                FROM pg_indexes i
                WHERE i.schemaname = 'custody'
                  AND i.tablename = r.target_table
                  AND NOT EXISTS (
                      SELECT 1
                      FROM pg_constraint c
                      WHERE c.conrelid = format('custody.%I', r.target_table)::regclass
                        AND c.conname = i.indexname
                  )
            LOOP
                new_name := rel_name;
                IF position(r.source_table IN new_name) > 0 THEN
                    new_name := replace(new_name, r.source_table, r.target_table);
                END IF;
                IF new_name = rel_name AND EXISTS (
                    SELECT 1
                    FROM pg_class pc
                    JOIN pg_namespace pn ON pn.oid = pc.relnamespace
                    WHERE pn.nspname = 'ob-poc'
                      AND pc.relname = rel_name
                ) THEN
                    new_name := r.target_table || '_' || rel_name;
                END IF;
                new_name := lower(substr(new_name, 1, 63));
                IF new_name <> rel_name THEN
                    IF EXISTS (
                        SELECT 1
                        FROM pg_class pc
                        JOIN pg_namespace pn ON pn.oid = pc.relnamespace
                        WHERE pn.nspname IN ('custody', 'ob-poc')
                          AND pc.relname = new_name
                          AND NOT (pn.nspname = 'custody' AND pc.relname = rel_name)
                    ) THEN
                        new_name := substr(new_name, 1, 54) || '_' || substr(md5(r.target_table || '_' || rel_name), 1, 8);
                    END IF;
                    EXECUTE format('ALTER INDEX custody.%I RENAME TO %I', rel_name, new_name);
                END IF;
            END LOOP;

            EXECUTE format('ALTER TABLE custody.%I SET SCHEMA "ob-poc"', r.target_table);
            UPDATE "ob-poc".schema_consolidation_table_map
            SET moved_at = now()
            WHERE source_schema = 'custody'
              AND source_table = r.source_table;
        END IF;
    END LOOP;

    FOR seq_name IN
        SELECT s.sequence_name
        FROM information_schema.sequences s
        WHERE s.sequence_schema = 'custody'
    LOOP
        EXECUTE format('ALTER SEQUENCE custody.%I SET SCHEMA "ob-poc"', seq_name);
    END LOOP;
END $$;

COMMIT;
