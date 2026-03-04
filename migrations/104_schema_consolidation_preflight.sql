-- 104_schema_consolidation_preflight.sql
-- Purpose:
--   Preflight + mapping bootstrap for consolidating kyc/custody/client_portal
--   tables into "ob-poc" schema.

BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".schema_consolidation_table_map (
    source_schema TEXT NOT NULL,
    source_table  TEXT NOT NULL,
    target_table  TEXT NOT NULL,
    moved_at      TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT schema_consolidation_table_map_pk PRIMARY KEY (source_schema, source_table),
    CONSTRAINT schema_consolidation_table_map_source_schema_ck CHECK (
        source_schema IN ('kyc', 'custody', 'client_portal')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS schema_consolidation_table_map_target_uq
    ON "ob-poc".schema_consolidation_table_map (source_schema, target_table);

INSERT INTO "ob-poc".schema_consolidation_table_map (source_schema, source_table, target_table)
SELECT
    t.table_schema,
    t.table_name,
    CASE
        WHEN t.table_schema = 'kyc' AND t.table_name = 'ubo_registry' THEN 'kyc_ubo_registry'
        WHEN t.table_schema = 'kyc' AND t.table_name = 'ubo_evidence' THEN 'kyc_ubo_evidence'
        WHEN t.table_schema = 'client_portal' AND t.table_name = 'sessions' THEN 'client_portal_sessions'
        ELSE t.table_name
    END AS target_table
FROM information_schema.tables t
WHERE t.table_type = 'BASE TABLE'
  AND t.table_schema IN ('kyc', 'custody', 'client_portal')
ON CONFLICT (source_schema, source_table) DO UPDATE
SET target_table = EXCLUDED.target_table;

DO $$
DECLARE
    unresolved_count INTEGER;
BEGIN
    SELECT COUNT(*)
    INTO unresolved_count
    FROM "ob-poc".schema_consolidation_table_map m
    JOIN information_schema.tables o
      ON o.table_schema = 'ob-poc'
     AND o.table_type = 'BASE TABLE'
     AND o.table_name = m.target_table;

    -- Allowed collisions are mapped to renamed targets, so this must be zero.
    IF unresolved_count > 0 THEN
        RAISE EXCEPTION
            'Schema consolidation preflight failed: % target table names still collide with existing "ob-poc" tables.',
            unresolved_count;
    END IF;
END $$;

COMMIT;
