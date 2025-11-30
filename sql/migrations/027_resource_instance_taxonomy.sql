-- ============================================
-- Resource Instance Taxonomy Migration
-- Purpose: Add instance-level resource tracking for CBU onboarding
-- ============================================

BEGIN;

-- =============================================================================
-- 1. CBU RESOURCE INSTANCES - The actual "things" created for a CBU
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_resource_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Ownership & Lineage
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id UUID REFERENCES "ob-poc".products(product_id),
    service_id UUID REFERENCES "ob-poc".services(service_id),
    resource_type_id UUID REFERENCES "ob-poc".prod_resources(resource_id),

    -- THE UNIQUE THING (the URL)
    instance_url VARCHAR(1024) NOT NULL,
    instance_identifier VARCHAR(255),
    instance_name VARCHAR(255),

    -- Configuration
    instance_config JSONB DEFAULT '{}',

    -- Lifecycle
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'PROVISIONING', 'ACTIVE', 'SUSPENDED', 'DECOMMISSIONED')),

    -- Audit
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    provisioned_at TIMESTAMPTZ,
    activated_at TIMESTAMPTZ,
    decommissioned_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Constraints
    UNIQUE(instance_url),
    UNIQUE(cbu_id, resource_type_id, instance_identifier)
);

COMMENT ON TABLE "ob-poc".cbu_resource_instances IS
'Production resource instances - the actual delivered artifacts for a CBU (accounts, connections, platform access)';

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.instance_url IS
'Unique URL/endpoint for this resource instance (e.g., https://custody.bank.com/accounts/ABC123)';

-- =============================================================================
-- 2. RESOURCE INSTANCE ATTRIBUTES - Dense table, no sparse matrix
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".resource_instance_attributes (
    value_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),

    -- Typed values (use one based on dictionary.mask)
    value_text VARCHAR,
    value_number NUMERIC,
    value_boolean BOOLEAN,
    value_date DATE,
    value_timestamp TIMESTAMPTZ,
    value_json JSONB,

    -- Provenance
    state VARCHAR(50) DEFAULT 'proposed'
        CHECK (state IN ('proposed', 'confirmed', 'derived', 'system')),
    source JSONB,

    observed_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(instance_id, attribute_id)
);

COMMENT ON TABLE "ob-poc".resource_instance_attributes IS
'Attribute values for resource instances - dense storage (row exists = value set)';

-- =============================================================================
-- 3. SERVICE DELIVERY MAP - The persisted "what was delivered" record
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".service_delivery_map (
    delivery_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),
    instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    -- Service Configuration (options selected during onboarding)
    service_config JSONB DEFAULT '{}',

    -- Status
    delivery_status VARCHAR(50) DEFAULT 'PENDING'
        CHECK (delivery_status IN ('PENDING', 'IN_PROGRESS', 'DELIVERED', 'FAILED', 'CANCELLED')),

    -- Audit
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    failure_reason TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(cbu_id, product_id, service_id)
);

COMMENT ON TABLE "ob-poc".service_delivery_map IS
'Tracks service delivery for CBU onboarding - links CBU -> Product -> Service -> Instance';

-- =============================================================================
-- 4. RESOURCE TYPE ATTRIBUTES - Add missing columns if needed
-- =============================================================================

ALTER TABLE "ob-poc".resource_attribute_requirements
    ADD COLUMN IF NOT EXISTS default_value TEXT,
    ADD COLUMN IF NOT EXISTS display_order INTEGER DEFAULT 0;

-- =============================================================================
-- 5. INDEXES
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_cri_cbu ON "ob-poc".cbu_resource_instances(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cri_status ON "ob-poc".cbu_resource_instances(status);
CREATE INDEX IF NOT EXISTS idx_cri_resource_type ON "ob-poc".cbu_resource_instances(resource_type_id);
CREATE INDEX IF NOT EXISTS idx_cri_url ON "ob-poc".cbu_resource_instances(instance_url);

CREATE INDEX IF NOT EXISTS idx_ria_instance ON "ob-poc".resource_instance_attributes(instance_id);
CREATE INDEX IF NOT EXISTS idx_ria_attribute ON "ob-poc".resource_instance_attributes(attribute_id);

CREATE INDEX IF NOT EXISTS idx_sdm_cbu ON "ob-poc".service_delivery_map(cbu_id);
CREATE INDEX IF NOT EXISTS idx_sdm_product ON "ob-poc".service_delivery_map(product_id);
CREATE INDEX IF NOT EXISTS idx_sdm_service ON "ob-poc".service_delivery_map(service_id);
CREATE INDEX IF NOT EXISTS idx_sdm_status ON "ob-poc".service_delivery_map(delivery_status);

-- =============================================================================
-- 6. UPDATE TRIGGER for updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cri_updated ON "ob-poc".cbu_resource_instances;
CREATE TRIGGER trg_cri_updated
    BEFORE UPDATE ON "ob-poc".cbu_resource_instances
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();

DROP TRIGGER IF EXISTS trg_sdm_updated ON "ob-poc".service_delivery_map;
CREATE TRIGGER trg_sdm_updated
    BEFORE UPDATE ON "ob-poc".service_delivery_map
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();

COMMIT;

-- =============================================================================
-- VERIFICATION
-- =============================================================================

SELECT 'cbu_resource_instances' as table_name, COUNT(*) as columns
FROM information_schema.columns
WHERE table_schema = 'ob-poc' AND table_name = 'cbu_resource_instances'
UNION ALL
SELECT 'resource_instance_attributes', COUNT(*)
FROM information_schema.columns
WHERE table_schema = 'ob-poc' AND table_name = 'resource_instance_attributes'
UNION ALL
SELECT 'service_delivery_map', COUNT(*)
FROM information_schema.columns
WHERE table_schema = 'ob-poc' AND table_name = 'service_delivery_map';
