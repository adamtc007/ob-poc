DROP VIEW IF EXISTS "ob-poc".v_attribute_reconciliation_summary;

CREATE OR REPLACE VIEW "ob-poc".v_attribute_reconciliation_summary AS
SELECT
    COUNT(*)::bigint AS total_attributes,
    COUNT(*) FILTER (WHERE sem_reg_snapshot_id IS NOT NULL)::bigint AS reconciled,
    COUNT(*) FILTER (
        WHERE sem_reg_snapshot_id IS NULL
          AND metadata #>> '{sem_os,reconciliation_status}' = 'out_of_scope'
    )::bigint AS out_of_scope,
    COUNT(*) FILTER (
        WHERE sem_reg_snapshot_id IS NULL
          AND COALESCE(metadata #>> '{sem_os,reconciliation_status}', '') <> 'out_of_scope'
    )::bigint AS pending_manual,
    COUNT(*) FILTER (WHERE is_derived)::bigint AS derived,
    COUNT(*) FILTER (WHERE NOT is_derived)::bigint AS primary_attributes,
    COUNT(*) FILTER (WHERE evidence_grade <> 'none')::bigint AS evidence_graded,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE sem_reg_snapshot_id IS NOT NULL)
        / NULLIF(
            COUNT(*) - COUNT(*) FILTER (
                WHERE metadata #>> '{sem_os,reconciliation_status}' = 'out_of_scope'
            ),
            0
        ),
        1
    ) AS pct_reconciled
FROM "ob-poc".attribute_registry;

COMMENT ON VIEW "ob-poc".v_attribute_reconciliation_summary IS
    'Dashboard: attribute reconciliation progress. pct_reconciled excludes out_of_scope from denominator.';
