COPY (
    SELECT
        '(attribute.define'
        || ' :id "' || ar.id || '"'
        || ' :display-name "' || REPLACE(ar.display_name, '"', '\\"') || '"'
        || ' :category "' || ar.category || '"'
        || ' :value-type "' || ar.value_type || '"'
        || ' :domain "' || COALESCE(ar.domain, ar.category) || '"'
        || ' :evidence-grade "none"'
        || ')'
        AS dsl_call
    FROM "ob-poc".attribute_registry ar
    WHERE ar.sem_reg_snapshot_id IS NULL
      AND COALESCE(ar.metadata #>> '{sem_os,reconciliation_status}', '') <> 'out_of_scope'
      AND (
          (ar.id LIKE 'attr.%' AND ar.category IN (
              'identity', 'financial', 'compliance', 'document',
              'risk', 'contact', 'address', 'tax', 'entity', 'ubo',
              'isda', 'resource', 'cbu', 'trust', 'fund', 'partnership'
          ))
          OR (ar.id LIKE 'entity.%' AND ar.category = 'entity')
      )
    ORDER BY ar.category, ar.id
) TO STDOUT WITH (FORMAT text);
