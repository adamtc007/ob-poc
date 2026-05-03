-- Product -> service -> service-resource lifecycle alignment.
--
-- The old resource-instance uniqueness collapsed parameterized resources:
-- a CBU could only have one (product, service, resource_type) instance, even
-- when the SRDEF declared per-market, per-currency, or per-counterparty scope.
-- This migration makes the dimensional grain explicit and rewires the service
-- gap view to active product subscriptions/service intents instead of legacy
-- cbus.product_id assignment.

ALTER TABLE "ob-poc".cbu_resource_instances
    DROP CONSTRAINT IF EXISTS cbu_resource_instances_cbu_product_service_resource_key;

CREATE UNIQUE INDEX IF NOT EXISTS cbu_resource_instances_cbu_product_service_resource_dim_key
    ON "ob-poc".cbu_resource_instances
    (cbu_id, product_id, service_id, resource_type_id, market_id, currency, counterparty_entity_id)
    NULLS NOT DISTINCT
    WHERE product_id IS NOT NULL
      AND service_id IS NOT NULL
      AND resource_type_id IS NOT NULL;

CREATE OR REPLACE VIEW "ob-poc".v_cbu_service_gaps AS
WITH active_intents AS (
    SELECT DISTINCT
        si.cbu_id,
        c.name AS cbu_name,
        si.product_id,
        p.product_code,
        p.name AS product_name,
        si.service_id,
        s.service_code,
        s.name AS service_name
    FROM "ob-poc".service_intents si
    JOIN "ob-poc".cbus c ON c.cbu_id = si.cbu_id
    JOIN "ob-poc".products p ON p.product_id = si.product_id
    JOIN "ob-poc".services s ON s.service_id = si.service_id
    WHERE si.status = 'active'
      AND p.is_active = true
      AND s.is_active = true
),
required_resources AS (
    SELECT
        ai.cbu_id,
        ai.cbu_name,
        ai.product_id,
        ai.product_code,
        ai.product_name,
        ai.service_id,
        ai.service_code,
        ai.service_name,
        ps.is_mandatory,
        srt.resource_id AS resource_type_id,
        srt.resource_code,
        srt.name AS resource_name,
        srt.provisioning_verb,
        srt.location_type,
        srt.per_market,
        srt.per_currency,
        srt.per_counterparty,
        COALESCE(src.is_required, true) AS is_required
    FROM active_intents ai
    JOIN "ob-poc".product_services ps
      ON ps.product_id = ai.product_id
     AND ps.service_id = ai.service_id
    JOIN "ob-poc".service_resource_capabilities src
      ON src.service_id = ai.service_id
     AND src.is_active = true
    JOIN "ob-poc".service_resource_types srt
      ON srt.resource_id = src.resource_id
     AND srt.is_active = true
    WHERE COALESCE(src.is_required, true) = true
)
SELECT
    rr.cbu_id,
    rr.cbu_name,
    rr.product_code,
    rr.product_name,
    rr.service_code,
    rr.service_name,
    rr.is_mandatory,
    rr.resource_code AS missing_resource_code,
    rr.resource_name AS missing_resource_name,
    rr.provisioning_verb,
    rr.location_type,
    rr.per_market,
    rr.per_currency,
    rr.per_counterparty,
    rr.is_required
FROM required_resources rr
WHERE NOT EXISTS (
    SELECT 1
    FROM "ob-poc".cbu_resource_instances cri
    WHERE cri.cbu_id = rr.cbu_id
      AND cri.product_id = rr.product_id
      AND cri.service_id = rr.service_id
      AND cri.resource_type_id = rr.resource_type_id
      AND cri.status IN ('PENDING', 'PROVISIONING', 'ACTIVE')
)
ORDER BY rr.cbu_name, rr.product_code, rr.service_code, rr.resource_code;

COMMENT ON VIEW "ob-poc".v_cbu_service_gaps IS
    'Shows missing required service resources for active CBU product service intents.';
