-- Instrument Lifecycle Taxonomy Schema
-- =============================================================================
-- Parallel structure to Product → Service → Service Resource:
--   Instrument Class → Lifecycle → Lifecycle Resource Type
-- =============================================================================

-- =============================================================================
-- LIFECYCLES (analogous to services)
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".lifecycles (
    lifecycle_id uuid DEFAULT gen_random_uuid() NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    category character varying(100) NOT NULL,  -- SETTLEMENT, ASSET_SERVICING, PRICING, OTC_LIFECYCLE, COLLATERAL, TAX, REGULATORY
    owner character varying(100) NOT NULL,      -- CUSTODY, DERIVATIVES, FUND_ACCOUNTING, etc.
    regulatory_driver character varying(100),   -- UMR, EMIR_MIFID_DFA, FATCA_CRS, etc.
    sla_definition jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT lifecycles_pkey PRIMARY KEY (lifecycle_id),
    CONSTRAINT lifecycles_code_key UNIQUE (code)
);

CREATE INDEX idx_lifecycles_category ON "ob-poc".lifecycles(category);
CREATE INDEX idx_lifecycles_owner ON "ob-poc".lifecycles(owner);

COMMENT ON TABLE "ob-poc".lifecycles IS 'Operational lifecycles/services that instruments require (analogous to services table)';

-- =============================================================================
-- LIFECYCLE RESOURCE TYPES (analogous to service_resource_types)
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".lifecycle_resource_types (
    resource_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    resource_type character varying(100) NOT NULL,  -- ACCOUNT, SSI, AGREEMENT, CONNECTIVITY, DATA_FEED, PROCESS, CLASSIFICATION
    owner character varying(100) NOT NULL,
    location_type character varying(100),           -- CSD_OR_CUSTODIAN, BANK, TRI_PARTY_AGENT, etc.
    per_currency boolean DEFAULT false,
    per_counterparty boolean DEFAULT false,
    per_market boolean DEFAULT false,
    vendor_options jsonb,                           -- Array of valid vendors
    provisioning_verb character varying(100),       -- DSL verb to provision this resource
    provisioning_args jsonb,                        -- Default args for provisioning verb
    depends_on jsonb,                               -- Array of resource codes this depends on
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT lifecycle_resource_types_pkey PRIMARY KEY (resource_type_id),
    CONSTRAINT lifecycle_resource_types_code_key UNIQUE (code)
);

CREATE INDEX idx_lifecycle_resource_types_type ON "ob-poc".lifecycle_resource_types(resource_type);
CREATE INDEX idx_lifecycle_resource_types_owner ON "ob-poc".lifecycle_resource_types(owner);

COMMENT ON TABLE "ob-poc".lifecycle_resource_types IS 'Resource types that lifecycles require (analogous to service_resource_types)';

-- =============================================================================
-- INSTRUMENT LIFECYCLES (analogous to product_services)
-- Junction table: which lifecycles apply to which instrument classes
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".instrument_lifecycles (
    instrument_lifecycle_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instrument_class_id uuid NOT NULL,              -- FK to custody.instrument_classes
    lifecycle_id uuid NOT NULL,                     -- FK to lifecycles
    is_mandatory boolean DEFAULT true,
    requires_isda boolean DEFAULT false,
    display_order integer DEFAULT 100,
    configuration jsonb,                            -- Instrument-specific lifecycle config
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT instrument_lifecycles_pkey PRIMARY KEY (instrument_lifecycle_id),
    CONSTRAINT instrument_lifecycles_unique UNIQUE (instrument_class_id, lifecycle_id),
    CONSTRAINT instrument_lifecycles_lifecycle_fk FOREIGN KEY (lifecycle_id) 
        REFERENCES "ob-poc".lifecycles(lifecycle_id)
);

CREATE INDEX idx_instrument_lifecycles_class ON "ob-poc".instrument_lifecycles(instrument_class_id);
CREATE INDEX idx_instrument_lifecycles_lifecycle ON "ob-poc".instrument_lifecycles(lifecycle_id);

COMMENT ON TABLE "ob-poc".instrument_lifecycles IS 'Junction: which lifecycles apply to which instrument classes (analogous to product_services)';

-- =============================================================================
-- LIFECYCLE RESOURCE CAPABILITIES (analogous to service_resource_capabilities)
-- Junction table: which resources each lifecycle requires
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".lifecycle_resource_capabilities (
    capability_id uuid DEFAULT gen_random_uuid() NOT NULL,
    lifecycle_id uuid NOT NULL,
    resource_type_id uuid NOT NULL,
    is_required boolean DEFAULT true,
    priority integer DEFAULT 100,                   -- For fallback ordering
    supported_options jsonb,                        -- Lifecycle-specific resource options
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT lifecycle_resource_capabilities_pkey PRIMARY KEY (capability_id),
    CONSTRAINT lifecycle_resource_capabilities_unique UNIQUE (lifecycle_id, resource_type_id),
    CONSTRAINT lifecycle_resource_capabilities_lifecycle_fk FOREIGN KEY (lifecycle_id) 
        REFERENCES "ob-poc".lifecycles(lifecycle_id),
    CONSTRAINT lifecycle_resource_capabilities_resource_fk FOREIGN KEY (resource_type_id) 
        REFERENCES "ob-poc".lifecycle_resource_types(resource_type_id)
);

CREATE INDEX idx_lifecycle_resource_capabilities_lifecycle ON "ob-poc".lifecycle_resource_capabilities(lifecycle_id);
CREATE INDEX idx_lifecycle_resource_capabilities_resource ON "ob-poc".lifecycle_resource_capabilities(resource_type_id);

COMMENT ON TABLE "ob-poc".lifecycle_resource_capabilities IS 'Junction: which resources each lifecycle requires (analogous to service_resource_capabilities)';

-- =============================================================================
-- CBU LIFECYCLE INSTANCES (analogous to cbu_resource_instances)
-- Provisioned lifecycle resources for a CBU
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_lifecycle_instances (
    instance_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    resource_type_id uuid NOT NULL,
    instance_identifier character varying(255),     -- e.g., "XLON-Securities-BNY"
    instance_url character varying(500),            -- Unique resource URL for dependency tracking
    -- Context scoping
    market_id uuid,                                 -- If per_market
    currency character varying(3),                  -- If per_currency
    counterparty_entity_id uuid,                    -- If per_counterparty
    -- Status
    status character varying(50) DEFAULT 'PENDING',
    -- Provider details
    provider_code character varying(50),            -- BNYM, BLOOMBERG, MARKITWIRE, etc.
    provider_account character varying(100),
    provider_bic character varying(11),
    -- Configuration
    config jsonb,
    -- Dependencies
    depends_on_urls jsonb,                          -- Array of instance_urls this depends on
    -- Lifecycle
    provisioned_at timestamp with time zone,
    activated_at timestamp with time zone,
    suspended_at timestamp with time zone,
    decommissioned_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cbu_lifecycle_instances_pkey PRIMARY KEY (instance_id),
    CONSTRAINT cbu_lifecycle_instances_url_key UNIQUE (instance_url),
    CONSTRAINT cbu_lifecycle_instances_cbu_fk FOREIGN KEY (cbu_id) 
        REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT cbu_lifecycle_instances_resource_fk FOREIGN KEY (resource_type_id) 
        REFERENCES "ob-poc".lifecycle_resource_types(resource_type_id),
    CONSTRAINT cbu_lifecycle_instances_status_check CHECK (
        status IN ('PENDING', 'PROVISIONING', 'PROVISIONED', 'ACTIVE', 'SUSPENDED', 'DECOMMISSIONED')
    )
);

CREATE INDEX idx_cbu_lifecycle_instances_cbu ON "ob-poc".cbu_lifecycle_instances(cbu_id);
CREATE INDEX idx_cbu_lifecycle_instances_resource ON "ob-poc".cbu_lifecycle_instances(resource_type_id);
CREATE INDEX idx_cbu_lifecycle_instances_status ON "ob-poc".cbu_lifecycle_instances(status);
CREATE INDEX idx_cbu_lifecycle_instances_market ON "ob-poc".cbu_lifecycle_instances(market_id) WHERE market_id IS NOT NULL;
CREATE INDEX idx_cbu_lifecycle_instances_counterparty ON "ob-poc".cbu_lifecycle_instances(counterparty_entity_id) WHERE counterparty_entity_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".cbu_lifecycle_instances IS 'Provisioned lifecycle resources for a CBU (analogous to cbu_resource_instances)';

-- =============================================================================
-- CBU LIFECYCLE COVERAGE (view for gap analysis)
-- Shows which lifecycles are fully provisioned for each CBU universe entry
-- =============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_cbu_lifecycle_coverage AS
SELECT 
    u.cbu_id,
    u.universe_id,
    ic.code AS instrument_class,
    m.mic AS market,
    u.counterparty_entity_id,
    l.code AS lifecycle_code,
    l.name AS lifecycle_name,
    il.is_mandatory,
    il.requires_isda,
    -- Count required resources for this lifecycle
    (SELECT COUNT(*) 
     FROM "ob-poc".lifecycle_resource_capabilities lrc 
     WHERE lrc.lifecycle_id = l.lifecycle_id AND lrc.is_required = true
    ) AS required_resource_count,
    -- Count provisioned resources for this CBU/context
    (SELECT COUNT(*) 
     FROM "ob-poc".lifecycle_resource_capabilities lrc 
     JOIN "ob-poc".cbu_lifecycle_instances cli 
       ON cli.resource_type_id = lrc.resource_type_id 
       AND cli.cbu_id = u.cbu_id
       AND cli.status IN ('PROVISIONED', 'ACTIVE')
       AND (cli.market_id IS NULL OR cli.market_id = u.market_id)
       AND (cli.counterparty_entity_id IS NULL OR cli.counterparty_entity_id = u.counterparty_entity_id)
     WHERE lrc.lifecycle_id = l.lifecycle_id AND lrc.is_required = true
    ) AS provisioned_resource_count,
    -- Is this lifecycle fully provisioned?
    CASE 
        WHEN (SELECT COUNT(*) FROM "ob-poc".lifecycle_resource_capabilities lrc 
              WHERE lrc.lifecycle_id = l.lifecycle_id AND lrc.is_required = true) = 
             (SELECT COUNT(*) FROM "ob-poc".lifecycle_resource_capabilities lrc 
              JOIN "ob-poc".cbu_lifecycle_instances cli 
                ON cli.resource_type_id = lrc.resource_type_id 
                AND cli.cbu_id = u.cbu_id
                AND cli.status IN ('PROVISIONED', 'ACTIVE')
              WHERE lrc.lifecycle_id = l.lifecycle_id AND lrc.is_required = true)
        THEN true 
        ELSE false 
    END AS is_fully_provisioned
FROM custody.cbu_instrument_universe u
JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
LEFT JOIN custody.markets m ON m.market_id = u.market_id
JOIN "ob-poc".instrument_lifecycles il ON il.instrument_class_id = u.instrument_class_id
JOIN "ob-poc".lifecycles l ON l.lifecycle_id = il.lifecycle_id
WHERE il.is_active = true AND l.is_active = true;

COMMENT ON VIEW "ob-poc".v_cbu_lifecycle_coverage IS 'Shows lifecycle coverage status for each CBU universe entry';

-- =============================================================================
-- CBU LIFECYCLE GAPS (view for gap analysis)
-- Shows missing resources for CBU universe entries
-- =============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_cbu_lifecycle_gaps AS
SELECT 
    u.cbu_id,
    c.name AS cbu_name,
    ic.code AS instrument_class,
    m.mic AS market,
    e.name AS counterparty_name,
    l.code AS lifecycle_code,
    l.name AS lifecycle_name,
    il.is_mandatory,
    lrt.code AS missing_resource_code,
    lrt.name AS missing_resource_name,
    lrt.provisioning_verb,
    lrt.location_type,
    lrt.per_market,
    lrt.per_currency,
    lrt.per_counterparty
FROM custody.cbu_instrument_universe u
JOIN "ob-poc".cbus c ON c.cbu_id = u.cbu_id
JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
LEFT JOIN custody.markets m ON m.market_id = u.market_id
LEFT JOIN "ob-poc".entities e ON e.entity_id = u.counterparty_entity_id
JOIN "ob-poc".instrument_lifecycles il ON il.instrument_class_id = u.instrument_class_id
JOIN "ob-poc".lifecycles l ON l.lifecycle_id = il.lifecycle_id
JOIN "ob-poc".lifecycle_resource_capabilities lrc ON lrc.lifecycle_id = l.lifecycle_id AND lrc.is_required = true
JOIN "ob-poc".lifecycle_resource_types lrt ON lrt.resource_type_id = lrc.resource_type_id
WHERE il.is_active = true 
  AND l.is_active = true
  AND NOT EXISTS (
    SELECT 1 FROM "ob-poc".cbu_lifecycle_instances cli
    WHERE cli.cbu_id = u.cbu_id
      AND cli.resource_type_id = lrt.resource_type_id
      AND cli.status IN ('PROVISIONED', 'ACTIVE')
      AND (cli.market_id IS NULL OR cli.market_id = u.market_id OR NOT lrt.per_market)
      AND (cli.counterparty_entity_id IS NULL OR cli.counterparty_entity_id = u.counterparty_entity_id OR NOT lrt.per_counterparty)
  );

COMMENT ON VIEW "ob-poc".v_cbu_lifecycle_gaps IS 'Shows missing lifecycle resources for CBU universe entries';

-- =============================================================================
-- LOCATION MAPPINGS TABLE
-- CSD/Custodian lookup by market
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".market_csd_mappings (
    mapping_id uuid DEFAULT gen_random_uuid() NOT NULL,
    market_id uuid NOT NULL,
    csd_code character varying(50) NOT NULL,
    csd_bic character varying(11) NOT NULL,
    csd_name character varying(255),
    is_primary boolean DEFAULT true,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT market_csd_mappings_pkey PRIMARY KEY (mapping_id),
    CONSTRAINT market_csd_mappings_market_csd_key UNIQUE (market_id, csd_code)
);

CREATE INDEX idx_market_csd_mappings_market ON "ob-poc".market_csd_mappings(market_id);

COMMENT ON TABLE "ob-poc".market_csd_mappings IS 'Maps markets to their CSDs for safekeeping account provisioning';
