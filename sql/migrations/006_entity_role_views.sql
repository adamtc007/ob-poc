-- Migration: Add views for entity roles with priority scoring
-- Used by visualization to determine slot assignment and primary role display

-- View: Entity with all roles and computed primary role (highest priority)
CREATE OR REPLACE VIEW "ob-poc".v_cbu_entity_with_roles AS
WITH role_priorities AS (
    SELECT 
        cer.cbu_id,
        cer.entity_id,
        e.name as entity_name,
        et.type_code as entity_type,
        COALESCE(
            lc.jurisdiction, 
            p.jurisdiction, 
            t.jurisdiction, 
            pp.nationality
        ) as jurisdiction,
        r.name as role_name,
        -- Role priority for slot assignment (higher = more important for layout)
        CASE r.name
            -- Ownership roles (highest priority)
            WHEN 'ULTIMATE_BENEFICIAL_OWNER' THEN 100
            WHEN 'BENEFICIAL_OWNER' THEN 95
            WHEN 'SHAREHOLDER' THEN 90
            WHEN 'LIMITED_PARTNER' THEN 85
            -- Fund service provider roles
            WHEN 'MANAGEMENT_COMPANY' THEN 75
            WHEN 'INVESTMENT_MANAGER' THEN 74
            WHEN 'AIFM' THEN 73
            -- Trust roles
            WHEN 'SETTLOR' THEN 72
            WHEN 'TRUSTEE' THEN 71
            WHEN 'PROTECTOR' THEN 68
            -- Control roles
            WHEN 'DIRECTOR' THEN 70
            WHEN 'CONDUCTING_OFFICER' THEN 68
            WHEN 'OFFICER' THEN 65
            WHEN 'COMPANY_SECRETARY' THEN 60
            WHEN 'AUTHORIZED_SIGNATORY' THEN 55
            -- Service providers
            WHEN 'DEPOSITARY' THEN 50
            WHEN 'CUSTODIAN' THEN 49
            WHEN 'ADMINISTRATOR' THEN 45
            WHEN 'FUND_ADMIN' THEN 44
            WHEN 'TRANSFER_AGENT' THEN 43
            WHEN 'AUDITOR' THEN 40
            WHEN 'LEGAL_COUNSEL' THEN 35
            WHEN 'PRIME_BROKER' THEN 38
            -- Other roles
            WHEN 'BENEFICIARY' THEN 30
            WHEN 'INVESTOR' THEN 25
            WHEN 'SERVICE_PROVIDER' THEN 20
            WHEN 'NOMINEE' THEN 15
            WHEN 'RELATED_PARTY' THEN 10
            WHEN 'PRINCIPAL' THEN 80
            ELSE 5
        END as role_priority
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
    jurisdiction,
    array_agg(role_name ORDER BY role_priority DESC) as roles,
    (array_agg(role_name ORDER BY role_priority DESC))[1] as primary_role,
    MAX(role_priority) as max_role_priority
FROM role_priorities
GROUP BY cbu_id, entity_id, entity_name, entity_type, jurisdiction;

-- View: Investor aggregation by share class (for collapsed investor groups)
CREATE OR REPLACE VIEW "ob-poc".v_cbu_investor_groups AS
SELECT 
    sc.cbu_id,
    h.share_class_id,
    sc.name as share_class_name,
    sc.currency,
    sc.isin,
    COUNT(DISTINCT h.investor_entity_id) as investor_count,
    SUM(h.units) as total_units,
    SUM(h.cost_basis) as total_value
FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
WHERE h.status = 'active'
GROUP BY sc.cbu_id, h.share_class_id, sc.name, sc.currency, sc.isin;

-- View: Individual investor details (for expanded view)
CREATE OR REPLACE VIEW "ob-poc".v_cbu_investor_details AS
SELECT 
    sc.cbu_id,
    h.share_class_id,
    sc.name as share_class_name,
    h.investor_entity_id,
    e.name as investor_name,
    et.type_code as investor_type,
    h.units,
    h.cost_basis as value,
    COALESCE(lc.jurisdiction, pp.nationality) as jurisdiction
FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
WHERE h.status = 'active';
