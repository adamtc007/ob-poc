-- Migration 046: Remove client_label columns
-- 
-- Rationale: client_label was a "magic string" shortcut that doesn't scale.
-- Instead, use proper GROUP entity taxonomy with entity resolution.
-- The session bootstrap flow + trigger phrases handle disambiguation.

BEGIN;

-- Drop dependent views first
DROP VIEW IF EXISTS "ob-poc".entity_search_view CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_cbu_subscriptions CASCADE;

-- Drop client_label from entities
ALTER TABLE "ob-poc".entities DROP COLUMN IF EXISTS client_label;

-- Drop client_label from CBUs  
ALTER TABLE "ob-poc".cbus DROP COLUMN IF EXISTS client_label;

-- Drop any indexes on client_label (may not exist)
DROP INDEX IF EXISTS "ob-poc".idx_entities_client_label;
DROP INDEX IF EXISTS "ob-poc".idx_cbus_client_label;

-- Recreate entity_search_view without client_label
CREATE OR REPLACE VIEW "ob-poc".entity_search_view AS
SELECT 
    entity_id,
    name,
    entity_type_id,
    external_id,
    bods_entity_type,
    bods_entity_subtype,
    founding_date,
    dissolution_date,
    is_publicly_listed,
    created_at,
    updated_at,
    -- Search vector for full-text search
    to_tsvector('english', 
        COALESCE(name, '') || ' ' || 
        COALESCE(external_id, '')
    ) AS search_vector
FROM "ob-poc".entities;

-- Recreate v_cbu_subscriptions without client_label (use contract join)
CREATE OR REPLACE VIEW "ob-poc".v_cbu_subscriptions AS
SELECT 
    s.cbu_id,
    c.name AS cbu_name,
    lc.client_label AS contract_client,
    lc.contract_id,
    s.product_code,
    s.subscribed_at,
    cp.rate_card_id,
    rc.name AS rate_card_name,
    rc.currency AS rate_card_currency
FROM "ob-poc".cbu_subscriptions s
JOIN "ob-poc".cbus c ON s.cbu_id = c.cbu_id
JOIN "ob-poc".legal_contracts lc ON s.contract_id = lc.contract_id
JOIN "ob-poc".contract_products cp ON s.contract_id = cp.contract_id AND s.product_code = cp.product_code
LEFT JOIN "ob-poc".rate_cards rc ON cp.rate_card_id = rc.rate_card_id;

COMMENT ON TABLE "ob-poc".entities IS 'Entities use GROUP taxonomy for client hierarchy. Resolution via EntityGateway.';
COMMENT ON TABLE "ob-poc".cbus IS 'CBUs link to ManCo entities via share_links. Scope via GROUP apex entity.';

COMMIT;
