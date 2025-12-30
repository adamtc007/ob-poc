-- Fix entity_search_view to include entity_funds (umbrellas, subfunds, share classes)
-- This is required for the DSL lookup service to find fund entities by name

DROP VIEW IF EXISTS "ob-poc".entity_search_view;

CREATE VIEW "ob-poc".entity_search_view AS
-- Persons
SELECT entity_proper_persons.proper_person_id AS id,
    'PERSON'::text AS entity_type,
    (COALESCE(entity_proper_persons.first_name, '')::text || ' '::text) || COALESCE(entity_proper_persons.last_name, '')::text AS display_name,
    entity_proper_persons.nationality AS subtitle_1,
    entity_proper_persons.date_of_birth::text AS subtitle_2,
    (COALESCE(entity_proper_persons.first_name, '')::text || ' '::text) || COALESCE(entity_proper_persons.last_name, '')::text AS search_text
FROM "ob-poc".entity_proper_persons
WHERE entity_proper_persons.proper_person_id IS NOT NULL

UNION ALL

-- Companies
SELECT entity_limited_companies.limited_company_id AS id,
    'COMPANY'::text AS entity_type,
    entity_limited_companies.company_name AS display_name,
    entity_limited_companies.jurisdiction AS subtitle_1,
    entity_limited_companies.registration_number AS subtitle_2,
    entity_limited_companies.company_name AS search_text
FROM "ob-poc".entity_limited_companies
WHERE entity_limited_companies.limited_company_id IS NOT NULL

UNION ALL

-- CBUs
SELECT cbus.cbu_id AS id,
    'CBU'::text AS entity_type,
    cbus.name AS display_name,
    cbus.client_type AS subtitle_1,
    cbus.jurisdiction AS subtitle_2,
    cbus.name AS search_text
FROM "ob-poc".cbus
WHERE cbus.cbu_id IS NOT NULL

UNION ALL

-- Trusts
SELECT entity_trusts.trust_id AS id,
    'TRUST'::text AS entity_type,
    entity_trusts.trust_name AS display_name,
    entity_trusts.jurisdiction AS subtitle_1,
    NULL::text AS subtitle_2,
    entity_trusts.trust_name AS search_text
FROM "ob-poc".entity_trusts
WHERE entity_trusts.trust_id IS NOT NULL

UNION ALL

-- Fund entities (umbrellas, subfunds, share classes, etc.)
SELECT ef.entity_id AS id,
    COALESCE(et.type_code, 'FUND')::text AS entity_type,
    e.name AS display_name,
    ef.jurisdiction AS subtitle_1,
    ef.fund_structure_type AS subtitle_2,
    e.name AS search_text
FROM "ob-poc".entity_funds ef
JOIN "ob-poc".entities e ON ef.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
WHERE ef.entity_id IS NOT NULL;
