-- Migration 024: Service Intents + SRDEF Enhancement
--
-- Adds:
-- 1. service_intents table - captures what CBU wants (product + service + options)
-- 2. Enhances service_resource_types with SRDEF identity columns
-- 3. Discovery reason tracking for audit trail
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. SERVICE INTENTS TABLE
-- =============================================================================
-- Captures a CBU's subscription to a product/service combination with options.
-- This is the INPUT to the resource discovery engine.

CREATE TABLE IF NOT EXISTS "ob-poc".service_intents (
    intent_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),

    -- Service configuration options (markets, SSI mode, channels, etc.)
    options JSONB NOT NULL DEFAULT '{}',

    -- Lifecycle
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'cancelled')),

    -- Audit
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by TEXT,

    -- One intent per CBU/product/service combination
    UNIQUE(cbu_id, product_id, service_id)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_service_intents_cbu
    ON "ob-poc".service_intents(cbu_id);
CREATE INDEX IF NOT EXISTS idx_service_intents_status
    ON "ob-poc".service_intents(cbu_id, status) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_service_intents_product
    ON "ob-poc".service_intents(product_id);

-- Updated_at trigger
CREATE OR REPLACE FUNCTION "ob-poc".update_service_intents_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_service_intents_updated ON "ob-poc".service_intents;
CREATE TRIGGER trg_service_intents_updated
    BEFORE UPDATE ON "ob-poc".service_intents
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_service_intents_timestamp();

COMMENT ON TABLE "ob-poc".service_intents IS
    'CBU subscription to product/service combinations. Input to resource discovery.';
COMMENT ON COLUMN "ob-poc".service_intents.options IS
    'Service configuration: markets, SSI mode, channels, counterparties, etc.';

-- =============================================================================
-- 2. SRDEF IDENTITY ON SERVICE_RESOURCE_TYPES
-- =============================================================================
-- Add SRDEF (ServiceResourceDefinition) identity columns to existing table.
-- SRDEF ID format: SRDEF::<APP>::<Kind>::<Purpose>

-- Add srdef_id as computed column for canonical identity
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_id TEXT GENERATED ALWAYS AS (
    'SRDEF::' ||
    COALESCE(owner, 'UNKNOWN') || '::' ||
    COALESCE(resource_type, 'Resource') || '::' ||
    COALESCE(resource_code, resource_id::text)
) STORED;

-- Provisioning strategy: how to obtain this resource
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS provisioning_strategy TEXT DEFAULT 'create'
    CHECK (provisioning_strategy IN ('create', 'request', 'discover'));

-- Resource purpose: what this resource is for (more semantic than resource_type)
ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS resource_purpose TEXT;

-- Index on srdef_id for fast lookups
CREATE INDEX IF NOT EXISTS idx_service_resource_types_srdef
    ON "ob-poc".service_resource_types(srdef_id);

COMMENT ON COLUMN "ob-poc".service_resource_types.srdef_id IS
    'Canonical SRDEF identity: SRDEF::<APP>::<Kind>::<Purpose>';
COMMENT ON COLUMN "ob-poc".service_resource_types.provisioning_strategy IS
    'How to obtain: create (we create it), request (ask owner), discover (find existing)';
COMMENT ON COLUMN "ob-poc".service_resource_types.resource_purpose IS
    'Semantic purpose: custody_securities, swift_messaging, iam_access, etc.';

-- =============================================================================
-- 3. DISCOVERY REASONS TABLE
-- =============================================================================
-- Tracks WHY a particular SRDEF was discovered for a CBU.
-- This is the OUTPUT of the resource discovery engine, providing audit trail.

CREATE TABLE IF NOT EXISTS "ob-poc".srdef_discovery_reasons (
    discovery_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    srdef_id TEXT NOT NULL,
    resource_type_id UUID REFERENCES "ob-poc".service_resource_types(resource_id),

    -- Which intent(s) triggered this discovery
    triggered_by_intents JSONB NOT NULL DEFAULT '[]',  -- array of intent_ids

    -- Discovery reasoning
    discovery_rule TEXT NOT NULL,  -- rule name that matched
    discovery_reason JSONB NOT NULL DEFAULT '{}',  -- detailed explanation

    -- For parameterized resources (per-market, per-currency, etc.)
    parameters JSONB DEFAULT '{}',

    -- Lifecycle
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    superseded_at TIMESTAMPTZ  -- set when re-discovery replaces this
);

-- Partial unique index for active discoveries (only one active per CBU/SRDEF/params)
CREATE UNIQUE INDEX IF NOT EXISTS idx_srdef_discovery_active
    ON "ob-poc".srdef_discovery_reasons(cbu_id, srdef_id, parameters)
    WHERE superseded_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_srdef_discovery_cbu
    ON "ob-poc".srdef_discovery_reasons(cbu_id) WHERE superseded_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_srdef_discovery_srdef
    ON "ob-poc".srdef_discovery_reasons(srdef_id);

COMMENT ON TABLE "ob-poc".srdef_discovery_reasons IS
    'Audit trail: why each SRDEF was discovered for a CBU. Output of discovery engine.';
COMMENT ON COLUMN "ob-poc".srdef_discovery_reasons.parameters IS
    'For parameterized resources: {market_id: ..., currency: ...}';

-- =============================================================================
-- 4. ENHANCE RESOURCE_ATTRIBUTE_REQUIREMENTS
-- =============================================================================
-- Add source policy and constraint columns for SRDEF attribute profiles

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS requirement_type TEXT DEFAULT 'required'
    CHECK (requirement_type IN ('required', 'optional', 'conditional'));

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS source_policy JSONB DEFAULT '["derived", "entity", "cbu", "document", "manual"]';

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS constraints JSONB DEFAULT '{}';

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS evidence_policy JSONB DEFAULT '{}';

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS condition_expression TEXT;

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.requirement_type IS
    'required=must have, optional=nice to have, conditional=depends on condition_expression';
COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.source_policy IS
    'Ordered list of acceptable sources for this attribute value';
COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.constraints IS
    'Type/range/regex/enum constraints for validation';
COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.evidence_policy IS
    'What evidence is required: {requires_document: true, min_confidence: 0.9}';

-- =============================================================================
-- 5. SERVICE INTENT OPTIONS SCHEMA (for reference)
-- =============================================================================
-- Document expected structure of service_intents.options JSONB

COMMENT ON COLUMN "ob-poc".service_intents.options IS
$comment$
Service configuration options. Expected structure varies by service:

Custody/Settlement:
{
  "markets": ["XNAS", "XNYS", "XLON"],
  "currencies": ["USD", "GBP", "EUR"],
  "ssi_mode": "standing" | "per_trade",
  "counterparties": ["uuid1", "uuid2"]
}

Trading:
{
  "instrument_classes": ["equity", "fixed_income"],
  "execution_venues": ["XNAS", "XNYS"],
  "order_types": ["market", "limit"]
}

Reporting:
{
  "report_types": ["position", "transaction", "valuation"],
  "frequency": "daily" | "weekly" | "monthly",
  "format": "pdf" | "csv" | "xml"
}
$comment$;

-- =============================================================================
-- 6. VIEW: Active Service Intents with Product/Service Names
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_service_intents_active AS
SELECT
    si.intent_id,
    si.cbu_id,
    c.name AS cbu_name,
    si.product_id,
    p.name AS product_name,
    p.product_code,
    si.service_id,
    s.name AS service_name,
    s.service_code,
    si.options,
    si.status,
    si.created_at,
    si.updated_at
FROM "ob-poc".service_intents si
JOIN "ob-poc".cbus c ON c.cbu_id = si.cbu_id
JOIN "ob-poc".products p ON p.product_id = si.product_id
JOIN "ob-poc".services s ON s.service_id = si.service_id
WHERE si.status = 'active';

COMMENT ON VIEW "ob-poc".v_service_intents_active IS
    'Active service intents with resolved product/service names';
