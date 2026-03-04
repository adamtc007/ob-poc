-- 109_code_cutover_cleanup_constraints.sql
-- Post-move validation and hardening before schema removal.

BEGIN;

DO $$
DECLARE
    remaining_tables INTEGER;
    missing_targets INTEGER;
BEGIN
    -- No base tables should remain in legacy schemas.
    SELECT COUNT(*)
      INTO remaining_tables
      FROM information_schema.tables t
     WHERE t.table_type = 'BASE TABLE'
       AND t.table_schema IN ('kyc', 'custody', 'client_portal');

    IF remaining_tables > 0 THEN
        RAISE EXCEPTION
            'Schema consolidation check failed: % base tables remain in legacy schemas',
            remaining_tables;
    END IF;

    -- Every mapped target must exist in "ob-poc".
    SELECT COUNT(*)
      INTO missing_targets
      FROM "ob-poc".schema_consolidation_table_map m
 LEFT JOIN information_schema.tables t
        ON t.table_schema = 'ob-poc'
       AND t.table_type = 'BASE TABLE'
       AND t.table_name = m.target_table
     WHERE t.table_name IS NULL;

    IF missing_targets > 0 THEN
        RAISE EXCEPTION
            'Schema consolidation check failed: % mapped target tables missing in "ob-poc"',
            missing_targets;
    END IF;

END $$;

COMMIT;
