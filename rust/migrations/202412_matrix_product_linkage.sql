-- Migration: Matrix-Product Linkage
-- Purpose: Link Trading Matrix entries to Product subscriptions
-- Architecture: Products ADD attributes to matrix entries, they don't define them

-- ============================================================================
-- 1. CBU Product Subscriptions (if not exists)
-- ============================================================================
-- Tracks which products a CBU subscribes to

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_product_subscriptions (
    subscription_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id uuid NOT NULL REFERENCES "ob-poc".products(product_id),
    status varchar(20) NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('PENDING', 'ACTIVE', 'SUSPENDED', 'TERMINATED')),
    effective_from date NOT NULL DEFAULT CURRENT_DATE,
    effective_to date,
    config jsonb DEFAULT '{}',
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),

    UNIQUE (cbu_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_product_subscriptions_cbu
    ON "ob-poc".cbu_product_subscriptions(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_product_subscriptions_product
    ON "ob-poc".cbu_product_subscriptions(product_id);

-- ============================================================================
-- 2. Matrix-Product Overlay Table
-- ============================================================================
-- Links matrix entries (instrument universe) to product-specific attributes
-- Key insight: Products don't define the matrix - they add attributes to it

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_matrix_product_overlay (
    overlay_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- Matrix context (what this overlay applies to)
    -- NULL means "applies to all" for that dimension
    instrument_class_id uuid REFERENCES custody.instrument_classes(class_id),
    market_id uuid REFERENCES custody.markets(market_id),
    currency varchar(3),
    counterparty_entity_id uuid REFERENCES "ob-poc".entities(entity_id),

    -- Product providing the overlay
    subscription_id uuid NOT NULL REFERENCES "ob-poc".cbu_product_subscriptions(subscription_id) ON DELETE CASCADE,

    -- Attributes this product adds to the matrix entry
    additional_services jsonb DEFAULT '[]',       -- Service codes added by this product
    additional_slas jsonb DEFAULT '[]',           -- SLA template codes
    additional_resources jsonb DEFAULT '[]',      -- Extra resource requirements
    product_specific_config jsonb DEFAULT '{}',   -- Product-specific settings

    -- Status
    status varchar(20) NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('PENDING', 'ACTIVE', 'SUSPENDED')),
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),

    -- Unique per CBU/subscription/context combination
    UNIQUE NULLS NOT DISTINCT (cbu_id, subscription_id, instrument_class_id, market_id, currency, counterparty_entity_id)
);

CREATE INDEX IF NOT EXISTS idx_matrix_overlay_cbu
    ON "ob-poc".cbu_matrix_product_overlay(cbu_id);
CREATE INDEX IF NOT EXISTS idx_matrix_overlay_subscription
    ON "ob-poc".cbu_matrix_product_overlay(subscription_id);
CREATE INDEX IF NOT EXISTS idx_matrix_overlay_instrument
    ON "ob-poc".cbu_matrix_product_overlay(instrument_class_id);

-- ============================================================================
-- 3. View: Effective Matrix with Product Overlays
-- ============================================================================
-- Joins trading matrix with all applicable product overlays

CREATE OR REPLACE VIEW "ob-poc".v_cbu_matrix_effective AS
WITH matrix_base AS (
    SELECT
        u.universe_id,
        u.cbu_id,
        c.name AS cbu_name,
        u.instrument_class_id,
        ic.code AS instrument_class,
        ic.name AS instrument_class_name,
        u.market_id,
        m.mic AS market,
        m.name AS market_name,
        u.currencies,
        u.counterparty_entity_id,
        e.name AS counterparty_name,
        u.is_held,
        u.is_traded,
        u.is_active
    FROM custody.cbu_instrument_universe u
    JOIN "ob-poc".cbus c ON c.cbu_id = u.cbu_id
    JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
    LEFT JOIN custody.markets m ON m.market_id = u.market_id
    LEFT JOIN "ob-poc".entities e ON e.entity_id = u.counterparty_entity_id
    WHERE u.is_active = true
),
product_overlays AS (
    SELECT
        o.cbu_id,
        o.instrument_class_id,
        o.market_id,
        o.currency,
        o.counterparty_entity_id,
        p.product_id,
        p.product_code,
        p.name AS product_name,
        o.additional_services,
        o.additional_slas,
        o.additional_resources,
        o.product_specific_config,
        o.status AS overlay_status
    FROM "ob-poc".cbu_matrix_product_overlay o
    JOIN "ob-poc".cbu_product_subscriptions ps ON ps.subscription_id = o.subscription_id
    JOIN "ob-poc".products p ON p.product_id = ps.product_id
    WHERE o.status = 'ACTIVE' AND ps.status = 'ACTIVE'
)
SELECT
    mb.universe_id,
    mb.cbu_id,
    mb.cbu_name,
    mb.instrument_class_id,
    mb.instrument_class,
    mb.instrument_class_name,
    mb.market_id,
    mb.market,
    mb.market_name,
    mb.currencies,
    mb.counterparty_entity_id,
    mb.counterparty_name,
    mb.is_held,
    mb.is_traded,
    -- Aggregate all applicable product overlays
    COALESCE(
        jsonb_agg(
            jsonb_build_object(
                'product_code', po.product_code,
                'product_name', po.product_name,
                'additional_services', po.additional_services,
                'additional_slas', po.additional_slas,
                'additional_resources', po.additional_resources,
                'config', po.product_specific_config
            )
        ) FILTER (WHERE po.product_code IS NOT NULL),
        '[]'::jsonb
    ) AS product_overlays,
    -- Count of products overlaying this entry
    COUNT(po.product_code) AS overlay_count
FROM matrix_base mb
LEFT JOIN product_overlays po ON
    po.cbu_id = mb.cbu_id
    AND (po.instrument_class_id IS NULL OR po.instrument_class_id = mb.instrument_class_id)
    AND (po.market_id IS NULL OR po.market_id = mb.market_id)
    AND (po.counterparty_entity_id IS NULL OR po.counterparty_entity_id = mb.counterparty_entity_id)
GROUP BY
    mb.universe_id, mb.cbu_id, mb.cbu_name,
    mb.instrument_class_id, mb.instrument_class, mb.instrument_class_name,
    mb.market_id, mb.market, mb.market_name,
    mb.currencies, mb.counterparty_entity_id, mb.counterparty_name,
    mb.is_held, mb.is_traded;

-- ============================================================================
-- 4. View: Unified Gaps (Both Domains)
-- ============================================================================
-- Shows ALL missing resources from both lifecycle (trading matrix) and service (products)

CREATE OR REPLACE VIEW "ob-poc".v_cbu_unified_gaps AS
-- Gaps from Lifecycle domain (instrument-driven)
SELECT
    g.cbu_id,
    g.cbu_name,
    'LIFECYCLE' AS gap_source,
    g.instrument_class,
    g.market,
    g.counterparty_name,
    NULL AS product_code,
    g.lifecycle_code AS operation_code,
    g.lifecycle_name AS operation_name,
    g.missing_resource_code,
    g.missing_resource_name,
    g.provisioning_verb,
    g.location_type,
    g.per_market,
    g.per_currency,
    g.per_counterparty,
    g.is_mandatory AS is_required
FROM "ob-poc".v_cbu_lifecycle_gaps g

UNION ALL

-- Gaps from Service domain (product-driven)
SELECT
    g.cbu_id,
    g.cbu_name,
    'SERVICE' AS gap_source,
    NULL AS instrument_class,
    NULL AS market,
    NULL AS counterparty_name,
    g.product_code,
    g.service_code AS operation_code,
    g.service_name AS operation_name,
    g.missing_resource_code,
    g.missing_resource_name,
    g.provisioning_verb,
    g.location_type,
    g.per_market,
    g.per_currency,
    g.per_counterparty,
    g.is_mandatory AS is_required
FROM "ob-poc".v_cbu_service_gaps g;

-- ============================================================================
-- 5. View: CBU Product Summary
-- ============================================================================
-- Summary of products subscribed by each CBU

CREATE OR REPLACE VIEW "ob-poc".v_cbu_products AS
SELECT
    ps.subscription_id,
    ps.cbu_id,
    c.name AS cbu_name,
    ps.product_id,
    p.product_code,
    p.name AS product_name,
    p.product_category,
    ps.status,
    ps.effective_from,
    ps.effective_to,
    ps.config,
    -- Count of overlay entries for this subscription
    (SELECT COUNT(*) FROM "ob-poc".cbu_matrix_product_overlay o
     WHERE o.subscription_id = ps.subscription_id) AS overlay_count
FROM "ob-poc".cbu_product_subscriptions ps
JOIN "ob-poc".cbus c ON c.cbu_id = ps.cbu_id
JOIN "ob-poc".products p ON p.product_id = ps.product_id;

-- ============================================================================
-- 6. Trigger: Auto-create overlay when product subscribed
-- ============================================================================
-- When a CBU subscribes to a product, create a global overlay (applies to all matrix entries)

CREATE OR REPLACE FUNCTION "ob-poc".fn_auto_create_product_overlay()
RETURNS TRIGGER AS $$
BEGIN
    -- Create a global overlay (NULL context = applies to all matrix entries)
    INSERT INTO "ob-poc".cbu_matrix_product_overlay (
        cbu_id,
        subscription_id,
        instrument_class_id,
        market_id,
        currency,
        counterparty_entity_id,
        status
    ) VALUES (
        NEW.cbu_id,
        NEW.subscription_id,
        NULL,  -- applies to all instruments
        NULL,  -- applies to all markets
        NULL,  -- applies to all currencies
        NULL,  -- applies to all counterparties
        'ACTIVE'
    )
    ON CONFLICT DO NOTHING;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_auto_create_product_overlay ON "ob-poc".cbu_product_subscriptions;
CREATE TRIGGER trg_auto_create_product_overlay
    AFTER INSERT ON "ob-poc".cbu_product_subscriptions
    FOR EACH ROW
    WHEN (NEW.status = 'ACTIVE')
    EXECUTE FUNCTION "ob-poc".fn_auto_create_product_overlay();

-- ============================================================================
-- Done
-- ============================================================================
