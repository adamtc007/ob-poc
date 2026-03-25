WITH infrastructure_ids AS (
    SELECT ar.id
    FROM "ob-poc".attribute_registry ar
    WHERE ar.sem_reg_snapshot_id IS NULL
      AND (
        ar.id IN (
            'access_level',
            'api_key',
            'api_secret',
            'internal_reference',
            'system_status',
            'sync_status',
            'import_batch_id',
            'etl_timestamp',
            'record_version',
            'last_modified_by',
            'workflow_state'
        )
        OR ar.id LIKE '%_internal'
        OR ar.id LIKE 'sys_%'
        OR ar.id LIKE 'admin_%'
      )
)
UPDATE "ob-poc".attribute_registry ar
SET metadata = jsonb_set(
        COALESCE(ar.metadata, '{}'::jsonb),
        '{sem_os}',
        COALESCE(ar.metadata->'sem_os', '{}'::jsonb) || jsonb_build_object(
            'reconciliation_status', 'out_of_scope',
            'triage_category', 'B_infrastructure',
            'triage_reason', 'System/infrastructure field — not a governed KYC/onboarding data attribute'
        ),
        true
    ),
    updated_at = NOW()
FROM infrastructure_ids ids
WHERE ar.id = ids.id;
