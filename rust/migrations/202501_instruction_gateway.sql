-- =============================================================================
-- Phase 1: Instruction Profile & Gateway Routing
-- Trading Matrix Implementation
-- =============================================================================

-- =============================================================================
-- INSTRUCTION PROFILE TABLES
-- =============================================================================

-- Message type definitions (reference data)
CREATE TABLE IF NOT EXISTS custody.instruction_message_types (
    message_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lifecycle_event VARCHAR(50) NOT NULL,
    message_standard VARCHAR(20) NOT NULL,
    message_type VARCHAR(50) NOT NULL,
    direction VARCHAR(10) NOT NULL,
    description TEXT,
    schema_version VARCHAR(20),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(lifecycle_event, message_standard, message_type)
);

COMMENT ON TABLE custody.instruction_message_types IS 'Reference data: instruction message types (MT540, sese.023, etc.)';
COMMENT ON COLUMN custody.instruction_message_types.lifecycle_event IS 'TRADE_INSTRUCTION, SETTLEMENT_INSTRUCTION, CONFIRMATION, CA_INSTRUCTION, etc.';
COMMENT ON COLUMN custody.instruction_message_types.message_standard IS 'MT, MX, FIX, FPML, PROPRIETARY';

-- Instruction templates
CREATE TABLE IF NOT EXISTS custody.instruction_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_code VARCHAR(50) NOT NULL UNIQUE,
    template_name VARCHAR(255) NOT NULL,
    message_type_id UUID NOT NULL REFERENCES custody.instruction_message_types(message_type_id),
    base_template JSONB NOT NULL,
    field_mappings JSONB,
    validation_rules JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.instruction_templates IS 'Reusable instruction message templates with field mappings';

-- CBU template assignments (which template for which instrument/market/event)
CREATE TABLE IF NOT EXISTS custody.cbu_instruction_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    template_id UUID NOT NULL REFERENCES custody.instruction_templates(template_id),
    lifecycle_event VARCHAR(50) NOT NULL,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    priority INTEGER NOT NULL DEFAULT 50,
    effective_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id, lifecycle_event, counterparty_entity_id)
);

COMMENT ON TABLE custody.cbu_instruction_assignments IS 'CBU-specific template assignments for instruction generation';

-- Field-level overrides
CREATE TABLE IF NOT EXISTS custody.cbu_instruction_field_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    assignment_id UUID NOT NULL REFERENCES custody.cbu_instruction_assignments(assignment_id) ON DELETE CASCADE,
    field_path VARCHAR(255) NOT NULL,
    override_type VARCHAR(20) NOT NULL,
    override_value TEXT,
    derivation_rule JSONB,
    reason TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(assignment_id, field_path)
);

COMMENT ON TABLE custody.cbu_instruction_field_overrides IS 'Per-assignment field overrides (static values, derivation rules)';
COMMENT ON COLUMN custody.cbu_instruction_field_overrides.override_type IS 'STATIC, DERIVED, CONDITIONAL, SUPPRESS';

-- Indexes for instruction profile
CREATE INDEX IF NOT EXISTS idx_cbu_instruction_assignments_cbu ON custody.cbu_instruction_assignments(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_instruction_assignments_lookup ON custody.cbu_instruction_assignments(cbu_id, lifecycle_event, instrument_class_id, market_id);

-- =============================================================================
-- TRADE GATEWAY TABLES
-- =============================================================================

-- Gateway definitions (reference data)
CREATE TABLE IF NOT EXISTS custody.trade_gateways (
    gateway_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    gateway_code VARCHAR(50) NOT NULL UNIQUE,
    gateway_name VARCHAR(255) NOT NULL,
    gateway_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(20) NOT NULL,
    provider VARCHAR(50),
    supported_events TEXT[] NOT NULL,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE custody.trade_gateways IS 'Reference data: available trade gateways (SWIFT, FIX, Omgeo, etc.)';
COMMENT ON COLUMN custody.trade_gateways.gateway_type IS 'SWIFT_FIN, SWIFT_INTERACT, FIX, OMGEO_CTM, OMGEO_ALERT, BLOOMBERG_TOMS, etc.';
COMMENT ON COLUMN custody.trade_gateways.protocol IS 'MT, MX, FIX_4_2, FIX_4_4, FIX_5_0, FPML, REST, SOAP, FILE, MANUAL';

-- CBU gateway connectivity
CREATE TABLE IF NOT EXISTS custody.cbu_gateway_connectivity (
    connectivity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    connectivity_resource_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    credentials_reference VARCHAR(255),
    effective_date DATE,
    activated_at TIMESTAMPTZ,
    suspended_at TIMESTAMPTZ,
    gateway_config JSONB,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, gateway_id)
);

COMMENT ON TABLE custody.cbu_gateway_connectivity IS 'CBU connectivity to trade gateways';
COMMENT ON COLUMN custody.cbu_gateway_connectivity.status IS 'PENDING, TESTING, ACTIVE, SUSPENDED, DECOMMISSIONED';

-- Gateway routing rules
CREATE TABLE IF NOT EXISTS custody.cbu_gateway_routing (
    routing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    lifecycle_event VARCHAR(50) NOT NULL,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    priority INTEGER NOT NULL DEFAULT 50,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, gateway_id, lifecycle_event, instrument_class_id, market_id, counterparty_entity_id)
);

COMMENT ON TABLE custody.cbu_gateway_routing IS 'Priority-based gateway routing rules (similar to SSI booking rules)';

-- Gateway fallback configuration
CREATE TABLE IF NOT EXISTS custody.cbu_gateway_fallbacks (
    fallback_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    primary_gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    fallback_gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    lifecycle_event VARCHAR(50),
    trigger_conditions TEXT[] NOT NULL,
    priority INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, primary_gateway_id, lifecycle_event)
);

COMMENT ON TABLE custody.cbu_gateway_fallbacks IS 'Fallback gateway chain for resilience';
COMMENT ON COLUMN custody.cbu_gateway_fallbacks.trigger_conditions IS 'TIMEOUT, ERROR, REJECTION, MANUAL';

-- Indexes for gateway tables
CREATE INDEX IF NOT EXISTS idx_cbu_gateway_connectivity_cbu ON custody.cbu_gateway_connectivity(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_gateway_routing_cbu ON custody.cbu_gateway_routing(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_gateway_routing_lookup ON custody.cbu_gateway_routing(cbu_id, lifecycle_event, instrument_class_id, market_id);

-- =============================================================================
-- SEED REFERENCE DATA
-- =============================================================================

-- Common message types
INSERT INTO custody.instruction_message_types (lifecycle_event, message_standard, message_type, direction, description, schema_version)
VALUES
    -- SWIFT MT (Legacy)
    ('TRADE_INSTRUCTION', 'MT', 'MT502', 'SEND', 'Order to buy or sell', 'SR2023'),
    ('SETTLEMENT_INSTRUCTION', 'MT', 'MT540', 'SEND', 'Receive free of payment', 'SR2023'),
    ('SETTLEMENT_INSTRUCTION', 'MT', 'MT541', 'SEND', 'Receive against payment', 'SR2023'),
    ('SETTLEMENT_INSTRUCTION', 'MT', 'MT542', 'SEND', 'Deliver free of payment', 'SR2023'),
    ('SETTLEMENT_INSTRUCTION', 'MT', 'MT543', 'SEND', 'Deliver against payment', 'SR2023'),
    ('CONFIRMATION', 'MT', 'MT544', 'RECEIVE', 'Receive free confirmation', 'SR2023'),
    ('CONFIRMATION', 'MT', 'MT545', 'RECEIVE', 'Receive against payment confirmation', 'SR2023'),
    ('CONFIRMATION', 'MT', 'MT546', 'RECEIVE', 'Deliver free confirmation', 'SR2023'),
    ('CONFIRMATION', 'MT', 'MT547', 'RECEIVE', 'Deliver against payment confirmation', 'SR2023'),
    ('CA_INSTRUCTION', 'MT', 'MT565', 'SEND', 'Corporate action instruction', 'SR2023'),
    ('CA_RESPONSE', 'MT', 'MT566', 'RECEIVE', 'Corporate action confirmation', 'SR2023'),
    -- SWIFT MX (ISO 20022)
    ('SETTLEMENT_INSTRUCTION', 'MX', 'sese.023.001.09', 'SEND', 'Securities settlement instruction', '2023'),
    ('CONFIRMATION', 'MX', 'sese.024.001.10', 'RECEIVE', 'Securities settlement status', '2023'),
    ('CA_INSTRUCTION', 'MX', 'seev.033.001.12', 'SEND', 'Corporate action instruction', '2023'),
    -- FIX Protocol
    ('TRADE_INSTRUCTION', 'FIX', 'NewOrderSingle', 'SEND', 'FIX new order single', '5.0'),
    ('CONFIRMATION', 'FIX', 'ExecutionReport', 'RECEIVE', 'FIX execution report', '5.0'),
    ('ALLOCATION', 'FIX', 'AllocationInstruction', 'SEND', 'FIX allocation instruction', '5.0'),
    ('AFFIRMATION', 'FIX', 'Confirmation', 'BOTH', 'FIX confirmation/affirmation', '5.0')
ON CONFLICT DO NOTHING;

-- Common gateways
INSERT INTO custody.trade_gateways (gateway_code, gateway_name, gateway_type, protocol, provider, supported_events)
VALUES
    ('SWIFT_FIN', 'SWIFT FIN Network', 'SWIFT_FIN', 'MT', 'SWIFT', ARRAY['TRADE_INSTRUCTION', 'SETTLEMENT_INSTRUCTION', 'CONFIRMATION', 'CA_INSTRUCTION', 'CA_RESPONSE']),
    ('SWIFT_MX', 'SWIFT ISO 20022', 'SWIFT_INTERACT', 'MX', 'SWIFT', ARRAY['SETTLEMENT_INSTRUCTION', 'CONFIRMATION', 'CA_INSTRUCTION', 'CA_RESPONSE']),
    ('OMGEO_CTM', 'Omgeo CTM', 'OMGEO_CTM', 'REST', 'DTCC', ARRAY['TRADE_INSTRUCTION', 'ALLOCATION', 'AFFIRMATION', 'CONFIRMATION']),
    ('OMGEO_ALERT', 'Omgeo ALERT', 'OMGEO_ALERT', 'REST', 'DTCC', ARRAY['SETTLEMENT_INSTRUCTION']),
    ('BLOOMBERG_TOMS', 'Bloomberg TOMS', 'BLOOMBERG_TOMS', 'FIX_4_4', 'BLOOMBERG', ARRAY['TRADE_INSTRUCTION', 'CONFIRMATION']),
    ('FIX_DIRECT', 'Direct FIX Connection', 'FIX', 'FIX_5_0', 'DIRECT', ARRAY['TRADE_INSTRUCTION', 'CONFIRMATION', 'ALLOCATION']),
    ('MARKITWIRE', 'MarkitWire', 'MARKITWIRE', 'FPML', 'IHS_MARKIT', ARRAY['TRADE_INSTRUCTION', 'CONFIRMATION', 'COLLATERAL_CALL']),
    ('TRADEWEB', 'Tradeweb', 'TRADEWEB', 'FIX_4_4', 'TRADEWEB', ARRAY['TRADE_INSTRUCTION', 'CONFIRMATION']),
    ('DTCC_GTR', 'DTCC GTR', 'DTCC_GTR', 'FPML', 'DTCC', ARRAY['TRADE_INSTRUCTION']),
    ('MANUAL', 'Manual Processing', 'MANUAL', 'MANUAL', NULL, ARRAY['TRADE_INSTRUCTION', 'SETTLEMENT_INSTRUCTION', 'CA_INSTRUCTION'])
ON CONFLICT DO NOTHING;
