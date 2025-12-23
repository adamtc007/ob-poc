-- Product Domain Taxonomy Alignment Migration
-- Aligns Product/Service/Resource schema with Instrument/Lifecycle/Resource pattern

-- ============================================================================
-- STEP 1: Add missing columns to service_resource_types
-- ============================================================================

-- Context scoping columns (matching lifecycle_resource_types)
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS per_market boolean DEFAULT false;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS per_currency boolean DEFAULT false;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS per_counterparty boolean DEFAULT false;

-- Provisioning and dependency columns
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS provisioning_verb varchar(100);

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS provisioning_args jsonb;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS depends_on jsonb;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS location_type varchar(50);

COMMENT ON COLUMN "ob-poc".service_resource_types.per_market IS 'Resource requires market context (e.g., settlement account per exchange)';
COMMENT ON COLUMN "ob-poc".service_resource_types.per_currency IS 'Resource requires currency context (e.g., cash account per currency)';
COMMENT ON COLUMN "ob-poc".service_resource_types.per_counterparty IS 'Resource requires counterparty context (e.g., ISDA per counterparty)';
COMMENT ON COLUMN "ob-poc".service_resource_types.provisioning_verb IS 'DSL verb to provision this resource type';
COMMENT ON COLUMN "ob-poc".service_resource_types.depends_on IS 'Array of resource_codes this resource depends on';
COMMENT ON COLUMN "ob-poc".service_resource_types.location_type IS 'INTERNAL, EXTERNAL, HYBRID';

-- ============================================================================
-- STEP 2: Add is_required column to service_resource_capabilities
-- ============================================================================

ALTER TABLE "ob-poc".service_resource_capabilities
ADD COLUMN IF NOT EXISTS is_required boolean DEFAULT true;

COMMENT ON COLUMN "ob-poc".service_resource_capabilities.is_required IS 'Whether this resource is required for the service to function';

-- ============================================================================
-- STEP 3: Add context scoping to cbu_resource_instances
-- ============================================================================

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS market_id uuid REFERENCES custody.markets(market_id);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS currency varchar(3);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS counterparty_entity_id uuid REFERENCES "ob-poc".entities(entity_id);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS provider_code varchar(50);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS provider_config jsonb;

-- Create indexes for context lookups
CREATE INDEX IF NOT EXISTS idx_cbu_resource_instances_market
ON "ob-poc".cbu_resource_instances(market_id) WHERE market_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_cbu_resource_instances_currency
ON "ob-poc".cbu_resource_instances(currency) WHERE currency IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_cbu_resource_instances_counterparty
ON "ob-poc".cbu_resource_instances(counterparty_entity_id) WHERE counterparty_entity_id IS NOT NULL;

-- ============================================================================
-- STEP 4: Create v_cbu_service_gaps view (mirrors v_cbu_lifecycle_gaps)
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cbu_service_gaps AS
WITH cbu_products AS (
    -- Get products associated with each CBU via onboarding or direct assignment
    SELECT DISTINCT
        c.cbu_id,
        c.name AS cbu_name,
        p.product_id,
        p.product_code,
        p.name AS product_name
    FROM "ob-poc".cbus c
    -- Join via CBU's primary product
    LEFT JOIN "ob-poc".products p ON p.product_id = c.product_id
    WHERE p.product_id IS NOT NULL AND p.is_active = true

    UNION

    -- Also include products from onboarding requests
    SELECT DISTINCT
        c.cbu_id,
        c.name AS cbu_name,
        p.product_id,
        p.product_code,
        p.name AS product_name
    FROM "ob-poc".cbus c
    JOIN "ob-poc".onboarding_requests orq ON orq.cbu_id = c.cbu_id
    JOIN "ob-poc".onboarding_products op ON op.request_id = orq.request_id
    JOIN "ob-poc".products p ON p.product_id = op.product_id
    WHERE p.is_active = true
),
required_resources AS (
    -- Get required resources for each CBU's products
    SELECT
        cp.cbu_id,
        cp.cbu_name,
        cp.product_code,
        cp.product_name,
        s.service_id,
        s.service_code,
        s.name AS service_name,
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
    FROM cbu_products cp
    JOIN "ob-poc".product_services ps ON ps.product_id = cp.product_id
    JOIN "ob-poc".services s ON s.service_id = ps.service_id AND s.is_active = true
    JOIN "ob-poc".service_resource_capabilities src ON src.service_id = s.service_id AND src.is_active = true
    JOIN "ob-poc".service_resource_types srt ON srt.resource_id = src.resource_id AND srt.is_active = true
    WHERE COALESCE(src.is_required, true) = true  -- Only required resources
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
    rr.per_counterparty
FROM required_resources rr
WHERE NOT EXISTS (
    -- Check if resource is already provisioned for this CBU
    SELECT 1
    FROM "ob-poc".cbu_resource_instances cri
    WHERE cri.cbu_id = rr.cbu_id
      AND cri.resource_type_id = rr.resource_type_id
      AND cri.status IN ('PENDING', 'ACTIVE', 'PROVISIONED')
)
ORDER BY rr.cbu_name, rr.product_code, rr.service_code, rr.resource_code;

COMMENT ON VIEW "ob-poc".v_cbu_service_gaps IS 'Shows missing required service resources for each CBU based on their products';

-- ============================================================================
-- STEP 5: Update existing resource types with provisioning verbs
-- ============================================================================

UPDATE "ob-poc".service_resource_types SET
    provisioning_verb = 'service-resource.provision',
    location_type = CASE
        WHEN resource_type IN ('PLATFORM', 'SYSTEM') THEN 'INTERNAL'
        WHEN resource_type IN ('CONNECTION', 'FEED') THEN 'EXTERNAL'
        ELSE 'HYBRID'
    END
WHERE provisioning_verb IS NULL;

-- ============================================================================
-- STEP 6: Create index for gap view performance
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_cbu_resource_instances_lookup
ON "ob-poc".cbu_resource_instances(cbu_id, resource_type_id, status);
