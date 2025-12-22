-- =============================================================================
-- TRADING MATRIX STORAGE ARCHITECTURE - SCHEMA MIGRATION
-- =============================================================================
-- Date: December 22, 2025
-- Description: Core tables for Traded Instruments Matrix, SLA Framework,
--              and Service Resource traceability
-- =============================================================================

BEGIN;

-- =============================================================================
-- PART 1: DOCUMENT TYPES FOR TRADING LIFECYCLE
-- =============================================================================

INSERT INTO "ob-poc".document_types (type_code, display_name, category)
VALUES
  -- Primary trading documents
  ('INVESTMENT_MANDATE', 'Investment Management Agreement / IMA', 'OPERATIONAL'),
  ('TRADING_PROFILE', 'Trading Profile Configuration', 'OPERATIONAL'),
  ('TRADING_AUTHORITY', 'Trading Authority Matrix', 'OPERATIONAL'),

  -- Settlement documents
  ('SSI_TEMPLATE', 'Standing Settlement Instructions', 'OPERATIONAL'),
  ('SUBCUSTODIAN_AGREEMENT', 'Subcustodian Network Agreement', 'OPERATIONAL'),

  -- Connectivity documents
  ('SWIFT_CONFIGURATION', 'SWIFT Gateway Configuration', 'TECHNICAL'),
  ('CTM_ENROLLMENT', 'CTM/ALERT Enrollment Form', 'TECHNICAL'),
  ('FIX_SESSION_CONFIG', 'FIX Session Configuration', 'TECHNICAL'),

  -- OTC documents
  ('ISDA_MASTER', 'ISDA Master Agreement', 'LEGAL'),
  ('CSA_ANNEX', 'Credit Support Annex', 'LEGAL'),
  ('ISDA_SCHEDULE', 'ISDA Schedule', 'LEGAL'),

  -- SLA documents
  ('SERVICE_AGREEMENT', 'Service Level Agreement', 'LEGAL'),
  ('OLA_INTERNAL', 'Operational Level Agreement (Internal)', 'OPERATIONAL'),

  -- Pricing documents
  ('PRICING_AGREEMENT', 'Pricing Source Agreement', 'OPERATIONAL'),
  ('VALUATION_POLICY', 'Fund Valuation Policy', 'REGULATORY')
ON CONFLICT (type_code) DO NOTHING;

-- =============================================================================
-- PART 2: TRADING PROFILE EXTENSIONS
-- =============================================================================

-- Document-Profile linkage table
CREATE TABLE IF NOT EXISTS "ob-poc".trading_profile_documents (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id) ON DELETE CASCADE,
    doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),
    profile_section VARCHAR(50) NOT NULL,
    extraction_status VARCHAR(20) DEFAULT 'PENDING',
    extracted_at TIMESTAMPTZ,
    extraction_notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_profile_section CHECK (profile_section IN (
        'universe', 'investment_managers', 'isda_agreements', 'settlement_config',
        'booking_rules', 'standing_instructions', 'pricing_matrix', 'valuation_config',
        'constraints', 'cash_sweep_config', 'sla_commitments'
    )),
    CONSTRAINT valid_extraction_status CHECK (extraction_status IN (
        'PENDING', 'IN_PROGRESS', 'COMPLETE', 'FAILED', 'PARTIAL'
    ))
);

COMMENT ON TABLE "ob-poc".trading_profile_documents IS
'Links source documents (IMA, ISDA, SSI forms) to trading profile sections they populate.
Enables audit trail: "Where did this config come from?" → traces to source document.';

CREATE INDEX IF NOT EXISTS idx_tpd_profile ON "ob-poc".trading_profile_documents(profile_id);
CREATE INDEX IF NOT EXISTS idx_tpd_doc ON "ob-poc".trading_profile_documents(doc_id);
CREATE INDEX IF NOT EXISTS idx_tpd_section ON "ob-poc".trading_profile_documents(profile_section);

-- Extend trading profiles with materialization tracking
ALTER TABLE "ob-poc".cbu_trading_profiles
  ADD COLUMN IF NOT EXISTS source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  ADD COLUMN IF NOT EXISTS materialization_status VARCHAR(20) DEFAULT 'PENDING',
  ADD COLUMN IF NOT EXISTS materialized_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS materialization_hash TEXT,
  ADD COLUMN IF NOT EXISTS sla_profile_id UUID;

-- =============================================================================
-- PART 3: INVESTMENT MANAGER ASSIGNMENTS
-- =============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_im_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    profile_id UUID REFERENCES "ob-poc".cbu_trading_profiles(profile_id),

    -- Manager identification
    manager_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    manager_lei VARCHAR(20),
    manager_bic VARCHAR(11),
    manager_name VARCHAR(255),

    -- Role and priority
    manager_role VARCHAR(30) NOT NULL DEFAULT 'INVESTMENT_MANAGER',
    priority INTEGER NOT NULL DEFAULT 100,

    -- Scope definition
    scope_all BOOLEAN DEFAULT FALSE,
    scope_markets TEXT[],
    scope_instrument_classes TEXT[],
    scope_currencies TEXT[],
    scope_isda_asset_classes TEXT[],

    -- Instruction method and resource link
    instruction_method VARCHAR(20) NOT NULL,
    instruction_resource_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    -- Permissions
    can_trade BOOLEAN DEFAULT TRUE,
    can_settle BOOLEAN DEFAULT TRUE,
    can_affirm BOOLEAN DEFAULT FALSE,

    -- Lifecycle
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_im_role CHECK (manager_role IN (
        'INVESTMENT_MANAGER', 'SUB_ADVISOR', 'OVERLAY_MANAGER',
        'TRANSITION_MANAGER', 'EXECUTION_BROKER'
    )),
    CONSTRAINT valid_instruction_method CHECK (instruction_method IN (
        'SWIFT', 'CTM', 'FIX', 'API', 'MANUAL', 'ALERT'
    )),
    CONSTRAINT valid_im_status CHECK (status IN ('ACTIVE', 'SUSPENDED', 'TERMINATED'))
);

COMMENT ON TABLE custody.cbu_im_assignments IS
'Investment Manager assignments with trading scope. Materialized from trading profile.
Links IM to instruction delivery resource for traceability.';

CREATE INDEX IF NOT EXISTS idx_cbu_im_cbu ON custody.cbu_im_assignments(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_im_profile ON custody.cbu_im_assignments(profile_id);
CREATE INDEX IF NOT EXISTS idx_cbu_im_manager ON custody.cbu_im_assignments(manager_entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_im_method ON custody.cbu_im_assignments(instruction_method);
CREATE INDEX IF NOT EXISTS idx_cbu_im_active ON custody.cbu_im_assignments(cbu_id) WHERE status = 'ACTIVE';

-- =============================================================================
-- PART 4: PRICING CONFIGURATION
-- =============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_pricing_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    profile_id UUID REFERENCES "ob-poc".cbu_trading_profiles(profile_id),

    -- Scope
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    currency VARCHAR(3),

    -- Source hierarchy
    priority INTEGER NOT NULL DEFAULT 1,
    source VARCHAR(30) NOT NULL,
    price_type VARCHAR(20) NOT NULL DEFAULT 'CLOSING',
    fallback_source VARCHAR(30),

    -- Validation parameters
    max_age_hours INTEGER DEFAULT 24,
    tolerance_pct NUMERIC(5,2) DEFAULT 5.0,
    stale_action VARCHAR(20) DEFAULT 'WARN',

    -- Service resource linkage
    pricing_resource_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_price_source CHECK (source IN (
        'BLOOMBERG', 'REUTERS', 'MARKIT', 'REFINITIV', 'ICE',
        'MODEL', 'INTERNAL', 'VENDOR', 'COUNTERPARTY'
    )),
    CONSTRAINT valid_price_type CHECK (price_type IN (
        'CLOSING', 'MID', 'BID', 'ASK', 'VWAP', 'OFFICIAL'
    )),
    CONSTRAINT valid_stale_action CHECK (stale_action IN (
        'WARN', 'BLOCK', 'USE_FALLBACK', 'ESCALATE'
    ))
);

COMMENT ON TABLE custody.cbu_pricing_config IS
'Pricing source configuration by instrument class. Materialized from trading profile.
Links to provisioned pricing feed resource for traceability.';

CREATE INDEX IF NOT EXISTS idx_cbu_pricing_cbu ON custody.cbu_pricing_config(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_pricing_class ON custody.cbu_pricing_config(instrument_class_id);

-- =============================================================================
-- PART 5: CASH SWEEP CONFIGURATION
-- =============================================================================

CREATE TABLE IF NOT EXISTS custody.cbu_cash_sweep_config (
    sweep_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    profile_id UUID REFERENCES "ob-poc".cbu_trading_profiles(profile_id),

    -- Currency and threshold
    currency VARCHAR(3) NOT NULL,
    threshold_amount NUMERIC(18,2) NOT NULL,

    -- Sweep vehicle
    vehicle_type VARCHAR(20) NOT NULL,
    vehicle_id VARCHAR(50),
    vehicle_name VARCHAR(255),

    -- Timing
    sweep_time TIME NOT NULL,
    sweep_timezone VARCHAR(50) NOT NULL,
    sweep_frequency VARCHAR(20) DEFAULT 'DAILY',

    -- Interest handling
    interest_allocation VARCHAR(20) DEFAULT 'ACCRUED',
    interest_account_id UUID,

    -- Service resource
    sweep_resource_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    is_active BOOLEAN DEFAULT TRUE,
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_vehicle_type CHECK (vehicle_type IN (
        'STIF', 'MMF', 'DEPOSIT', 'OVERNIGHT_REPO', 'TRI_PARTY_REPO', 'MANUAL'
    )),
    CONSTRAINT valid_sweep_frequency CHECK (sweep_frequency IN (
        'INTRADAY', 'DAILY', 'WEEKLY', 'MONTHLY'
    )),
    CONSTRAINT valid_interest_allocation CHECK (interest_allocation IN (
        'ACCRUED', 'MONTHLY', 'QUARTERLY', 'REINVEST'
    )),
    UNIQUE(cbu_id, currency)
);

COMMENT ON TABLE custody.cbu_cash_sweep_config IS
'Cash sweep configuration for idle cash management. STIFs, MMFs, overnight deposits.';

CREATE INDEX IF NOT EXISTS idx_cbu_sweep_cbu ON custody.cbu_cash_sweep_config(cbu_id);

-- =============================================================================
-- PART 6: RESOURCE-PROFILE LINKAGE
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".resource_profile_sources (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE,
    profile_id UUID NOT NULL REFERENCES "ob-poc".cbu_trading_profiles(profile_id) ON DELETE CASCADE,
    profile_section VARCHAR(50) NOT NULL,
    profile_path TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW()
);

COMMENT ON TABLE "ob-poc".resource_profile_sources IS
'Links provisioned service resources back to their source in the trading profile.
Enables: "Why was this SWIFT gateway provisioned?" → "investment_managers[0].instruction_method = SWIFT"';

CREATE INDEX IF NOT EXISTS idx_rps_instance ON "ob-poc".resource_profile_sources(instance_id);
CREATE INDEX IF NOT EXISTS idx_rps_profile ON "ob-poc".resource_profile_sources(profile_id);

-- =============================================================================
-- PART 7: SLA FRAMEWORK
-- =============================================================================

-- SLA metric types (reference data)
CREATE TABLE IF NOT EXISTS "ob-poc".sla_metric_types (
    metric_code VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    metric_category VARCHAR(30) NOT NULL,
    unit VARCHAR(20) NOT NULL,
    aggregation_method VARCHAR(20) DEFAULT 'AVERAGE',
    higher_is_better BOOLEAN DEFAULT TRUE,
    is_active BOOLEAN DEFAULT TRUE,

    CONSTRAINT valid_metric_category CHECK (metric_category IN (
        'TIMELINESS', 'ACCURACY', 'AVAILABILITY', 'VOLUME', 'QUALITY'
    )),
    CONSTRAINT valid_unit CHECK (unit IN (
        'PERCENT', 'HOURS', 'MINUTES', 'SECONDS', 'COUNT', 'CURRENCY', 'BASIS_POINTS'
    )),
    CONSTRAINT valid_aggregation CHECK (aggregation_method IN (
        'AVERAGE', 'SUM', 'MIN', 'MAX', 'MEDIAN', 'P95', 'P99'
    ))
);

-- Seed SLA metric types
INSERT INTO "ob-poc".sla_metric_types (metric_code, name, metric_category, unit, higher_is_better) VALUES
  ('SETTLEMENT_RATE', 'On-time Settlement Rate', 'ACCURACY', 'PERCENT', TRUE),
  ('SETTLEMENT_FAIL_RATE', 'Settlement Failure Rate', 'ACCURACY', 'PERCENT', FALSE),
  ('INSTRUCTION_LATENCY', 'Instruction Processing Latency', 'TIMELINESS', 'MINUTES', FALSE),
  ('NAV_DELIVERY_TIME', 'NAV Delivery Time', 'TIMELINESS', 'HOURS', FALSE),
  ('PRICE_AVAILABILITY', 'Price Availability Rate', 'AVAILABILITY', 'PERCENT', TRUE),
  ('PRICE_STALENESS', 'Average Price Age', 'TIMELINESS', 'HOURS', FALSE),
  ('MATCH_RATE', 'Trade Match Rate', 'ACCURACY', 'PERCENT', TRUE),
  ('AFFIRMATION_TIME', 'Average Affirmation Time', 'TIMELINESS', 'HOURS', FALSE),
  ('MARGIN_CALL_TIMELINESS', 'Margin Call Processing Time', 'TIMELINESS', 'HOURS', FALSE),
  ('COLLATERAL_ELIGIBILITY_ACCURACY', 'Collateral Validation Accuracy', 'ACCURACY', 'PERCENT', TRUE),
  ('SWEEP_EXECUTION_RATE', 'Sweep Execution Success Rate', 'ACCURACY', 'PERCENT', TRUE),
  ('INTEREST_ALLOCATION_ACCURACY', 'Interest Allocation Accuracy', 'ACCURACY', 'PERCENT', TRUE)
ON CONFLICT (metric_code) DO NOTHING;

-- SLA Templates
CREATE TABLE IF NOT EXISTS "ob-poc".sla_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,

    -- What this SLA applies to
    applies_to_type VARCHAR(30) NOT NULL,
    applies_to_code VARCHAR(50),

    -- Commitment
    metric_code VARCHAR(50) NOT NULL REFERENCES "ob-poc".sla_metric_types(metric_code),
    target_value NUMERIC(10,4) NOT NULL,
    warning_threshold NUMERIC(10,4),
    measurement_period VARCHAR(20) DEFAULT 'MONTHLY',

    -- Response commitments
    response_time_hours NUMERIC(5,2),
    escalation_path TEXT,

    -- Metadata
    regulatory_requirement BOOLEAN DEFAULT FALSE,
    regulatory_reference TEXT,

    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_applies_to CHECK (applies_to_type IN (
        'SERVICE', 'RESOURCE_TYPE', 'ISDA', 'CSA', 'PRODUCT'
    )),
    CONSTRAINT valid_measurement_period CHECK (measurement_period IN (
        'DAILY', 'WEEKLY', 'MONTHLY', 'QUARTERLY', 'ANNUAL'
    ))
);

-- Seed SLA templates
INSERT INTO "ob-poc".sla_templates
(template_code, name, applies_to_type, applies_to_code, metric_code, target_value, warning_threshold, measurement_period)
VALUES
  ('CUSTODY_SETTLE_DVP', 'DVP Settlement Rate', 'SERVICE', 'CUSTODY', 'SETTLEMENT_RATE', 99.5, 98.0, 'MONTHLY'),
  ('CUSTODY_SETTLE_INSTR', 'Instruction Processing', 'SERVICE', 'CUSTODY', 'INSTRUCTION_LATENCY', 30, 60, 'DAILY'),
  ('FA_NAV_DELIVERY', 'NAV Delivery Time', 'SERVICE', 'FUND_ACCOUNTING', 'NAV_DELIVERY_TIME', 18, 19, 'DAILY'),
  ('FA_PRICE_AVAIL', 'Price Availability', 'SERVICE', 'FUND_ACCOUNTING', 'PRICE_AVAILABILITY', 99.0, 97.0, 'DAILY'),
  ('TC_MATCH_RATE', 'Trade Match Rate', 'RESOURCE_TYPE', 'CTM_CONNECTION', 'MATCH_RATE', 98.0, 95.0, 'MONTHLY'),
  ('TC_AFFIRM_TIME', 'Affirmation Turnaround', 'RESOURCE_TYPE', 'CTM_CONNECTION', 'AFFIRMATION_TIME', 4, 8, 'DAILY'),
  ('PRICE_BLOOMBERG', 'Bloomberg Feed Availability', 'RESOURCE_TYPE', 'BLOOMBERG_TERMINAL', 'PRICE_AVAILABILITY', 99.9, 99.5, 'MONTHLY'),
  ('CSA_MARGIN_CALL', 'Margin Call Timeliness', 'CSA', NULL, 'MARGIN_CALL_TIMELINESS', 2, 4, 'DAILY'),
  ('CSA_COLLATERAL_VAL', 'Collateral Validation', 'CSA', NULL, 'COLLATERAL_ELIGIBILITY_ACCURACY', 100.0, 99.0, 'DAILY')
ON CONFLICT (template_code) DO NOTHING;

-- CBU-Specific SLA Commitments
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_sla_commitments (
    commitment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    profile_id UUID REFERENCES "ob-poc".cbu_trading_profiles(profile_id),
    template_id UUID REFERENCES "ob-poc".sla_templates(template_id),

    -- Override targets
    override_target_value NUMERIC(10,4),
    override_warning_threshold NUMERIC(10,4),

    -- Bindings
    bound_service_id UUID REFERENCES "ob-poc".services(service_id),
    bound_resource_instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    bound_isda_id UUID,
    bound_csa_id UUID,

    -- Scope restrictions
    scope_instrument_classes TEXT[],
    scope_markets TEXT[],
    scope_currencies TEXT[],
    scope_counterparties UUID[],

    -- Commercial terms
    penalty_structure JSONB,
    incentive_structure JSONB,

    -- Lifecycle
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    negotiated_by VARCHAR(255),
    negotiated_date DATE,

    -- Source tracking
    source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_sla_status CHECK (status IN ('DRAFT', 'ACTIVE', 'SUSPENDED', 'TERMINATED'))
);

COMMENT ON TABLE "ob-poc".cbu_sla_commitments IS
'CBU-specific SLA commitments. Links trading profile sections, service resources,
and ISDA/CSA agreements to measurable SLA targets.';

CREATE INDEX IF NOT EXISTS idx_cbu_sla_cbu ON "ob-poc".cbu_sla_commitments(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_sla_profile ON "ob-poc".cbu_sla_commitments(profile_id);
CREATE INDEX IF NOT EXISTS idx_cbu_sla_resource ON "ob-poc".cbu_sla_commitments(bound_resource_instance_id);
CREATE INDEX IF NOT EXISTS idx_cbu_sla_active ON "ob-poc".cbu_sla_commitments(cbu_id) WHERE status = 'ACTIVE';

-- SLA Measurements
CREATE TABLE IF NOT EXISTS "ob-poc".sla_measurements (
    measurement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commitment_id UUID NOT NULL REFERENCES "ob-poc".cbu_sla_commitments(commitment_id) ON DELETE CASCADE,

    -- Measurement period
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,

    -- Actual performance
    measured_value NUMERIC(10,4) NOT NULL,
    sample_size INTEGER,

    -- Status
    status VARCHAR(20) NOT NULL,
    variance_pct NUMERIC(6,2),

    -- Details
    measurement_notes TEXT,
    measurement_method VARCHAR(50),

    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_measurement_status CHECK (status IN ('MET', 'WARNING', 'BREACH')),
    CONSTRAINT valid_measurement_method CHECK (measurement_method IS NULL OR measurement_method IN (
        'AUTOMATED', 'MANUAL', 'ESTIMATED', 'SYSTEM'
    ))
);

CREATE INDEX IF NOT EXISTS idx_sla_meas_commitment ON "ob-poc".sla_measurements(commitment_id);
CREATE INDEX IF NOT EXISTS idx_sla_meas_period ON "ob-poc".sla_measurements(period_start, period_end);
CREATE INDEX IF NOT EXISTS idx_sla_meas_breach ON "ob-poc".sla_measurements(commitment_id) WHERE status = 'BREACH';

-- SLA Breaches
CREATE TABLE IF NOT EXISTS "ob-poc".sla_breaches (
    breach_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    measurement_id UUID NOT NULL REFERENCES "ob-poc".sla_measurements(measurement_id),
    commitment_id UUID NOT NULL REFERENCES "ob-poc".cbu_sla_commitments(commitment_id),

    -- Breach details
    breach_severity VARCHAR(20) NOT NULL,
    breach_date DATE NOT NULL,
    detected_at TIMESTAMPTZ DEFAULT NOW(),

    -- Root cause
    root_cause_category VARCHAR(50),
    root_cause_description TEXT,

    -- Remediation
    remediation_status VARCHAR(20) DEFAULT 'OPEN',
    remediation_plan TEXT,
    remediation_due_date DATE,
    remediation_completed_at TIMESTAMPTZ,

    -- Financial impact
    penalty_applied BOOLEAN DEFAULT FALSE,
    penalty_amount NUMERIC(18,2),
    penalty_currency VARCHAR(3),

    -- Escalation
    escalated_to VARCHAR(255),
    escalated_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_breach_severity CHECK (breach_severity IN ('MINOR', 'MAJOR', 'CRITICAL')),
    CONSTRAINT valid_root_cause CHECK (root_cause_category IS NULL OR root_cause_category IN (
        'SYSTEM', 'VENDOR', 'MARKET', 'CLIENT', 'INTERNAL', 'EXTERNAL', 'UNKNOWN'
    )),
    CONSTRAINT valid_remediation_status CHECK (remediation_status IN (
        'OPEN', 'IN_PROGRESS', 'RESOLVED', 'WAIVED', 'ESCALATED'
    ))
);

CREATE INDEX IF NOT EXISTS idx_sla_breach_commitment ON "ob-poc".sla_breaches(commitment_id);
CREATE INDEX IF NOT EXISTS idx_sla_breach_open ON "ob-poc".sla_breaches(commitment_id)
    WHERE remediation_status IN ('OPEN', 'IN_PROGRESS');

-- =============================================================================
-- PART 8: NEW SERVICE RESOURCE TYPES
-- =============================================================================

INSERT INTO "ob-poc".service_resource_types
(resource_code, name, resource_type, owner, description, capabilities)
VALUES
  -- Trade Instruction Capture
  ('SWIFT_GATEWAY', 'SWIFT Message Gateway', 'CONNECTIVITY', 'BNY',
   'SWIFT message routing for settlement instructions',
   '{"message_types": ["MT540", "MT541", "MT542", "MT543", "MT544", "MT545", "MT546", "MT547"],
     "directions": ["SEND", "RECEIVE"],
     "protocols": ["FIN", "FileAct"]}'::jsonb),

  ('CTM_CONNECTION', 'CTM Trade Matching', 'CONNECTIVITY', 'DTCC',
   'Omgeo CTM for automated trade matching and confirmation',
   '{"services": ["MATCH", "AFFIRM", "CONFIRM"],
     "asset_classes": ["EQUITY", "FIXED_INCOME"]}'::jsonb),

  ('ALERT_CONNECTION', 'ALERT SSI Enrichment', 'CONNECTIVITY', 'DTCC',
   'Omgeo ALERT for SSI lookup and enrichment',
   '{"services": ["SSI_LOOKUP", "ENRICHMENT", "VALIDATION"]}'::jsonb),

  ('FIX_SESSION', 'FIX Protocol Session', 'CONNECTIVITY', 'CLIENT',
   'FIX session for electronic trade capture',
   '{"versions": ["4.2", "4.4", "5.0SP2"],
     "message_types": ["NewOrderSingle", "ExecutionReport", "AllocationInstruction"]}'::jsonb),

  ('API_ENDPOINT', 'REST/gRPC API Endpoint', 'CONNECTIVITY', 'CLIENT',
   'API integration for trade and instruction delivery',
   '{"protocols": ["REST", "gRPC", "GraphQL"],
     "auth_methods": ["OAuth2", "mTLS", "API_KEY"]}'::jsonb),

  -- Pricing Resources
  ('BLOOMBERG_TERMINAL', 'Bloomberg Terminal Feed', 'PRICING', 'BLOOMBERG',
   'Bloomberg pricing and reference data',
   '{"data_types": ["PRICE", "REFERENCE", "CORPORATE_ACTIONS"],
     "asset_classes": ["EQUITY", "FIXED_INCOME", "FX", "COMMODITIES"]}'::jsonb),

  ('BLOOMBERG_BVAL', 'Bloomberg BVAL', 'PRICING', 'BLOOMBERG',
   'Bloomberg evaluated pricing for OTC and illiquid instruments',
   '{"data_types": ["EVALUATED_PRICE", "YIELD", "SPREAD"],
     "asset_classes": ["FIXED_INCOME", "DERIVATIVES", "STRUCTURED"]}'::jsonb),

  ('REFINITIV_FEED', 'Refinitiv Real-Time Feed', 'PRICING', 'REFINITIV',
   'Refinitiv (formerly Reuters) pricing data',
   '{"data_types": ["PRICE", "REFERENCE"],
     "delivery": ["REAL_TIME", "EOD"]}'::jsonb),

  ('MARKIT_PRICING', 'Markit Pricing Service', 'PRICING', 'MARKIT',
   'Markit evaluated pricing for derivatives and credit',
   '{"data_types": ["PRICE", "SPREAD", "CURVE"],
     "asset_classes": ["CDS", "CDX", "LOANS", "BONDS"]}'::jsonb),

  ('ICE_PRICING', 'ICE Data Services', 'PRICING', 'ICE',
   'ICE pricing for fixed income and derivatives',
   '{"data_types": ["PRICE", "ANALYTICS"],
     "asset_classes": ["FIXED_INCOME", "DERIVATIVES"]}'::jsonb),

  -- Cash Management Resources
  ('CASH_SWEEP_ENGINE', 'Cash Sweep Engine', 'CASH_MANAGEMENT', 'BNY',
   'Automated cash sweep processing',
   '{"sweep_types": ["STIF", "MMF", "DEPOSIT", "REPO"],
     "frequencies": ["INTRADAY", "DAILY"]}'::jsonb),

  ('STIF_ACCOUNT', 'Short-Term Investment Fund', 'CASH_MANAGEMENT', 'BNY',
   'BNY institutional cash fund',
   '{"fund_types": ["GOVT", "PRIME", "TREASURY"],
     "nav_frequency": "DAILY"}'::jsonb),

  -- Settlement Resources
  ('SETTLEMENT_INSTRUCTION_ENGINE', 'Settlement Instruction Generator', 'SETTLEMENT', 'BNY',
   'Generates SWIFT/ISO20022 settlement messages',
   '{"message_standards": ["SWIFT", "ISO20022"],
     "instruction_types": ["DVP", "FOP", "RVP", "DFP"]}'::jsonb),

  ('CSD_GATEWAY', 'CSD Direct Connection', 'SETTLEMENT', 'BNY',
   'Direct connectivity to Central Securities Depositories',
   '{"csds": ["DTCC", "Euroclear", "Clearstream", "JASDEC", "CCASS"]}'::jsonb)
ON CONFLICT (resource_code) DO NOTHING;

-- =============================================================================
-- PART 9: STIF INSTRUMENT CLASS
-- =============================================================================

INSERT INTO custody.instrument_classes
(code, name, default_settlement_cycle, swift_message_family, cfi_category, requires_isda)
VALUES
  ('STIF', 'Short-Term Investment Fund', 'T+0', 'FUND', 'C', FALSE),
  ('MMF', 'Money Market Fund', 'T+0', 'FUND', 'C', FALSE),
  ('REPO', 'Repurchase Agreement', 'T+0', 'REPO', 'D', FALSE)
ON CONFLICT (code) DO NOTHING;

COMMIT;

-- =============================================================================
-- VERIFICATION QUERIES
-- =============================================================================

-- Verify new tables
SELECT table_schema, table_name
FROM information_schema.tables
WHERE table_name IN (
    'trading_profile_documents', 'cbu_im_assignments', 'cbu_pricing_config',
    'cbu_cash_sweep_config', 'resource_profile_sources', 'sla_metric_types',
    'sla_templates', 'cbu_sla_commitments', 'sla_measurements', 'sla_breaches'
)
ORDER BY table_schema, table_name;

-- Verify new resource types
SELECT resource_code, name, resource_type
FROM "ob-poc".service_resource_types
WHERE resource_code IN (
    'SWIFT_GATEWAY', 'CTM_CONNECTION', 'BLOOMBERG_TERMINAL', 'CASH_SWEEP_ENGINE'
);

-- Verify SLA templates
SELECT template_code, name, metric_code, target_value
FROM "ob-poc".sla_templates;
