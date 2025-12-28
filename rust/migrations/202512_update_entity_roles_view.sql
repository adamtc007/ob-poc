-- Update v_cbu_entity_with_roles to include taxonomy columns
CREATE OR REPLACE VIEW "ob-poc".v_cbu_entity_with_roles AS
WITH role_priorities AS (
    SELECT
        cer.cbu_id,
        cer.entity_id,
        e.name AS entity_name,
        et.type_code AS entity_type,
        et.entity_category,
        COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) AS jurisdiction,
        r.name AS role_name,
        r.role_category,
        r.layout_category,
        r.ubo_treatment,
        r.kyc_obligation,
        -- Use display_priority from roles table, fallback to category-based priority
        COALESCE(r.display_priority,
            CASE r.role_category
                WHEN 'OWNERSHIP_CHAIN' THEN 100
                WHEN 'OWNERSHIP_CONTROL' THEN 100
                WHEN 'CONTROL_CHAIN' THEN 90
                WHEN 'TRUST_ROLES' THEN 85
                WHEN 'FUND_STRUCTURE' THEN 80
                WHEN 'FUND_MANAGEMENT' THEN 70
                WHEN 'INVESTOR_CHAIN' THEN 60
                WHEN 'BOTH' THEN 50
                WHEN 'SERVICE_PROVIDER' THEN 30
                WHEN 'TRADING_EXECUTION' THEN 20
                WHEN 'FUND_OPERATIONS' THEN 20
                WHEN 'DISTRIBUTION' THEN 15
                WHEN 'FINANCING' THEN 15
                WHEN 'RELATED_PARTY' THEN 10
                ELSE 5
            END
        ) AS role_priority
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
    LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
    LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
    LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
)
SELECT
    cbu_id,
    entity_id,
    entity_name,
    entity_type,
    entity_category,
    jurisdiction,
    array_agg(role_name ORDER BY role_priority DESC) AS roles,
    array_agg(DISTINCT role_category) AS role_categories,
    (array_agg(role_name ORDER BY role_priority DESC))[1] AS primary_role,
    max(role_priority) AS max_role_priority,
    -- New taxonomy columns: take from the highest priority role
    (array_agg(role_category ORDER BY role_priority DESC))[1] AS primary_role_category,
    (array_agg(layout_category ORDER BY role_priority DESC))[1] AS primary_layout_category,
    (array_agg(ubo_treatment ORDER BY role_priority DESC))[1] AS effective_ubo_treatment,
    (array_agg(kyc_obligation ORDER BY role_priority DESC))[1] AS effective_kyc_obligation
FROM role_priorities
GROUP BY cbu_id, entity_id, entity_name, entity_type, entity_category, jurisdiction;
