COPY (
    SELECT
        ar.id,
        ar.display_name,
        ar.category,
        ar.value_type,
        ar.domain,
        ar.uuid,
        ar.is_required,
        ar.metadata,
        CASE
            WHEN ar.id IN (
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
            ) THEN 'B_infrastructure'
            WHEN ar.id LIKE '%_internal'
              OR ar.id LIKE 'sys_%'
              OR ar.id LIKE 'admin_%' THEN 'B_infrastructure'
            WHEN ar.id LIKE 'attr.%'
              AND ar.category IN (
                    'identity', 'financial', 'compliance', 'document',
                    'risk', 'contact', 'address', 'tax', 'entity', 'ubo',
                    'isda', 'resource', 'cbu', 'trust', 'fund', 'partnership'
                ) THEN 'A_legitimate'
            WHEN ar.id LIKE 'entity.%'
              AND ar.category = 'entity' THEN 'A_legitimate'
            ELSE 'C_ambiguous'
        END AS triage_category,
        CASE
            WHEN ar.id LIKE 'attr.%.%' THEN
                REPLACE(SUBSTRING(ar.id FROM 6), '_', '-')
            WHEN ar.id LIKE 'entity.%' THEN
                regexp_replace(
                    REPLACE(ar.id, '_', '-'),
                    '^entity\.(company|person|partnership|trust)\.',
                    'entity.'
                )
            ELSE NULL
        END AS suggested_fqn
    FROM "ob-poc".attribute_registry ar
    WHERE ar.sem_reg_snapshot_id IS NULL
    ORDER BY triage_category, ar.category, ar.id
) TO STDOUT WITH (FORMAT csv, DELIMITER E'\t', HEADER true);
