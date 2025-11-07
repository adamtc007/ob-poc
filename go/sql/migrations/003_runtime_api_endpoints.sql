-- Migration: Runtime API Endpoints & Resource Creation System
-- Version: 003
-- Description: Adds support for DSL-to-API runtime execution with resource types,
--              action definitions, execution tracking, and idempotency support.

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create enums for action types and execution status
CREATE TYPE action_type_enum AS ENUM (
    'HTTP_API',
    'BPMN_WORKFLOW',
    'MESSAGE_QUEUE',
    'DATABASE_OPERATION',
    'EXTERNAL_SERVICE'
);

CREATE TYPE execution_status_enum AS ENUM (
    'PENDING',
    'RUNNING',
    'COMPLETED',
    'FAILED',
    'CANCELLED'
);

-- =============================================================================
-- Resource Types and Resource Dictionary
-- =============================================================================

-- Resource Types: Define concrete runtime resources (e.g., CustodyAccount)
CREATE TABLE resource_types (
    resource_type_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    resource_type_name VARCHAR(200) NOT NULL,
    description TEXT,
    active BOOLEAN DEFAULT true,
    version INTEGER DEFAULT 1,
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Ensure unique resource type names per environment/version
CREATE UNIQUE INDEX idx_resource_types_name_env_ver
ON resource_types(resource_type_name, environment, version);

-- Resource Type Attributes: Subset of main dictionary per resource type
CREATE TABLE resource_type_attributes (
    resource_type_id UUID REFERENCES resource_types(resource_type_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL, -- References "dsl-ob-poc".dictionary.attribute_id
    required BOOLEAN DEFAULT false,
    constraints JSONB, -- Resource-specific constraints
    transformation VARCHAR(100), -- Optional default transform key
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (resource_type_id, attribute_id)
);

-- Resource Type Endpoints: Lifecycle actions (create, activate, etc.)
CREATE TABLE resource_type_endpoints (
    endpoint_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    resource_type_id UUID REFERENCES resource_types(resource_type_id) ON DELETE CASCADE,
    lifecycle_action VARCHAR(50) NOT NULL, -- e.g., 'create', 'activate', 'suspend'
    endpoint_url TEXT NOT NULL,
    method VARCHAR(10) DEFAULT 'POST',
    authentication JSONB, -- credentials_ref, type, etc.
    timeout_seconds INTEGER DEFAULT 300,
    retry_config JSONB,
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(resource_type_id, lifecycle_action, environment)
);

-- =============================================================================
-- Action Registry and Execution System
-- =============================================================================

-- Actions Registry: DSL verb to API endpoint mappings
CREATE TABLE actions_registry (
    action_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    action_name VARCHAR(255) NOT NULL,
    verb_pattern VARCHAR(100) NOT NULL, -- e.g., "resources.create"
    action_type action_type_enum NOT NULL,
    resource_type_id UUID REFERENCES resource_types(resource_type_id),
    domain VARCHAR(100), -- Optional domain filter
    trigger_conditions JSONB,
    execution_config JSONB NOT NULL,
    attribute_mapping JSONB NOT NULL,
    success_criteria JSONB,
    failure_handling JSONB,
    active BOOLEAN DEFAULT true,
    version INTEGER DEFAULT 1,
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for action lookup performance
CREATE INDEX idx_actions_verb_pattern ON actions_registry(verb_pattern);
CREATE INDEX idx_actions_domain ON actions_registry(domain);
CREATE INDEX idx_actions_active ON actions_registry(active);
CREATE INDEX idx_actions_resource_type ON actions_registry(resource_type_id);

-- Action Executions: Track individual execution attempts
CREATE TABLE action_executions (
    execution_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    action_id UUID REFERENCES actions_registry(action_id),
    cbu_id UUID REFERENCES "dsl-ob-poc".cbus(cbu_id),
    dsl_version_id UUID REFERENCES "dsl-ob-poc".dsl_ob(version_id),
    execution_status execution_status_enum NOT NULL DEFAULT 'PENDING',

    -- Execution context and results
    trigger_context JSONB, -- DSL state snapshot that triggered the action
    request_payload JSONB, -- API request that was sent
    response_payload JSONB, -- API response received
    result_attributes JSONB, -- Mapped attributes from response
    error_details JSONB, -- Error information if failed

    -- Timing and retry information
    execution_duration_ms INTEGER,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    retry_count INTEGER DEFAULT 0,
    next_retry_at TIMESTAMPTZ,

    -- Idempotency and observability
    idempotency_key TEXT,
    correlation_id TEXT,
    trace_id TEXT,
    span_id TEXT,

    -- HTTP-specific metadata
    http_status INTEGER,
    endpoint TEXT,
    headers JSONB
);

-- Indexes for execution tracking and monitoring
CREATE INDEX idx_executions_status ON action_executions(execution_status);
CREATE INDEX idx_executions_cbu ON action_executions(cbu_id);
CREATE INDEX idx_executions_action ON action_executions(action_id);
CREATE INDEX idx_executions_started_at ON action_executions(started_at);
CREATE INDEX idx_executions_idempotency ON action_executions(idempotency_key);

-- Unique constraint to prevent duplicate executions (idempotency)
CREATE UNIQUE INDEX uq_action_dedupe
ON action_executions(action_id, cbu_id, idempotency_key)
WHERE idempotency_key IS NOT NULL;

-- Action Execution Attempts: Detailed retry tracking
CREATE TABLE action_execution_attempts (
    attempt_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id UUID REFERENCES action_executions(execution_id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    status execution_status_enum NOT NULL,

    -- Attempt-specific data
    request_payload JSONB,
    response_payload JSONB,
    error_details JSONB,
    http_status INTEGER,
    duration_ms INTEGER,

    -- HTTP-specific details
    endpoint_url TEXT,
    request_headers JSONB,
    response_headers JSONB
);

-- Ensure unique attempt sequence per execution
CREATE UNIQUE INDEX uq_attempt_seq ON action_execution_attempts(execution_id, attempt_no);
CREATE INDEX idx_attempts_execution ON action_execution_attempts(execution_id);
CREATE INDEX idx_attempts_status ON action_execution_attempts(status);

-- =============================================================================
-- Credentials Management
-- =============================================================================

-- Credentials Vault: Secure storage for API authentication
CREATE TABLE credentials_vault (
    credential_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    credential_name VARCHAR(255) UNIQUE NOT NULL,
    credential_type VARCHAR(50) NOT NULL, -- oauth2, api_key, basic_auth, etc.
    encrypted_data BYTEA NOT NULL, -- Encrypted credential data
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    active BOOLEAN DEFAULT true
);

CREATE INDEX idx_credentials_environment ON credentials_vault(environment);
CREATE INDEX idx_credentials_active ON credentials_vault(active);
CREATE INDEX idx_credentials_expires ON credentials_vault(expires_at);

-- =============================================================================
-- Insert Sample Data for Development
-- =============================================================================

-- Sample Resource Type: Custody Account
INSERT INTO resource_types (resource_type_name, description, environment) VALUES
('CustodyAccount', 'Client custody account for asset segregation and safekeeping', 'development'),
('FundAccountingSetup', 'Fund accounting infrastructure and reporting setup', 'development'),
('TradingAccount', 'Trading account with market access and settlement capabilities', 'development');

-- Sample Resource Type Attributes (referencing existing dictionary attributes)
-- Note: These would reference actual attribute_ids from the dictionary table
-- For now, using placeholder UUIDs that would be replaced with real dictionary references

-- Custody Account required attributes
DO $$
DECLARE
    custody_resource_id UUID;
BEGIN
    SELECT resource_type_id INTO custody_resource_id
    FROM resource_types
    WHERE resource_type_name = 'CustodyAccount' AND environment = 'development';

    -- Add sample attribute associations (would use real dictionary attribute_ids)
    INSERT INTO resource_type_attributes (resource_type_id, attribute_id, required, constraints) VALUES
    (custody_resource_id, uuid_generate_v4(), true, '{"validation": "required", "format": "uppercase"}'),
    (custody_resource_id, uuid_generate_v4(), true, '{"validation": "required", "format": "iso_currency"}'),
    (custody_resource_id, uuid_generate_v4(), false, '{"validation": "optional"}');
END $$;

-- Sample Resource Type Endpoints
DO $$
DECLARE
    custody_resource_id UUID;
BEGIN
    SELECT resource_type_id INTO custody_resource_id
    FROM resource_types
    WHERE resource_type_name = 'CustodyAccount' AND environment = 'development';

    INSERT INTO resource_type_endpoints (
        resource_type_id,
        lifecycle_action,
        endpoint_url,
        method,
        authentication,
        timeout_seconds,
        retry_config,
        environment
    ) VALUES (
        custody_resource_id,
        'create',
        'http://localhost:8080/api/v1/custody/accounts',
        'POST',
        '{"type": "api_key", "credentials_ref": "custody_api_key"}',
        300,
        '{"max_retries": 3, "backoff_strategy": "exponential", "base_delay_ms": 1000}',
        'development'
    );
END $$;

-- Sample Action Definition for Custody Account Creation
DO $$
DECLARE
    custody_resource_id UUID;
    action_uuid UUID;
BEGIN
    SELECT resource_type_id INTO custody_resource_id
    FROM resource_types
    WHERE resource_type_name = 'CustodyAccount' AND environment = 'development';

    action_uuid := uuid_generate_v4();

    INSERT INTO actions_registry (
        action_id,
        action_name,
        verb_pattern,
        action_type,
        resource_type_id,
        domain,
        trigger_conditions,
        execution_config,
        attribute_mapping,
        success_criteria,
        failure_handling,
        environment
    ) VALUES (
        action_uuid,
        'Create Custody Account',
        'resources.create',
        'HTTP_API',
        custody_resource_id,
        'onboarding',
        '{"state": "DISCOVER_RESOURCES", "attribute_requirements": ["custody.account_type", "settlement.currency"]}',
        '{"endpoint_url": "LOOKUP:resource_type.create", "method": "POST", "timeout_seconds": 300, "idempotency": {"header": "Idempotency-Key", "key_template": "{{resource_type}}:{{environment}}:{{cbu_id}}:{{action_id}}:{{dsl_version_id}}", "dedupe_ttl_seconds": 86400}, "telemetry": {"correlation_id_template": "{{cbu_id}}:{{action_id}}:{{resource_type}}", "propagate_trace": true}}',
        '{"input_mapping": [{"dsl_attribute_id": "custody.account_type", "api_parameter": "accountType", "transformation": "uppercase"}, {"dsl_attribute_id": "settlement.currency", "api_parameter": "baseCurrency", "transformation": "iso_currency_code"}], "output_mapping": [{"api_response_path": "$.account.id", "dsl_attribute_id": "custody.account_id", "attribute_name": "custody.account_id"}, {"api_response_path": "$.account.url", "dsl_attribute_id": "custody.account_url", "attribute_name": "custody.account_url"}]}',
        '{"http_status_codes": [200, 201, 202], "response_validation": "$.status == ''CREATED''", "required_outputs": ["custody.account_id", "custody.account_url"]}',
        '{"retry_on_codes": [500, 502, 503, 504], "fallback_action": "manual_intervention", "notification_channels": ["ops_team_slack", "onboarding_manager_email"]}',
        'development'
    );
END $$;

-- Sample Credential (placeholder - would be encrypted in production)
INSERT INTO credentials_vault (
    credential_name,
    credential_type,
    encrypted_data,
    environment,
    active
) VALUES (
    'custody_api_key',
    'api_key',
    'encrypted_api_key_data_here'::bytea,
    'development',
    true
);

-- =============================================================================
-- Views for easier querying
-- =============================================================================

-- View: Complete action definitions with resource type information
CREATE VIEW v_action_definitions AS
SELECT
    a.action_id,
    a.action_name,
    a.verb_pattern,
    a.action_type,
    a.domain,
    rt.resource_type_name,
    rt.description as resource_description,
    a.trigger_conditions,
    a.execution_config,
    a.attribute_mapping,
    a.success_criteria,
    a.failure_handling,
    a.active,
    a.environment,
    a.created_at,
    a.updated_at
FROM actions_registry a
LEFT JOIN resource_types rt ON a.resource_type_id = rt.resource_type_id;

-- View: Execution summary with action details
CREATE VIEW v_execution_summary AS
SELECT
    e.execution_id,
    e.cbu_id,
    a.action_name,
    a.verb_pattern,
    rt.resource_type_name,
    e.execution_status,
    e.started_at,
    e.completed_at,
    e.execution_duration_ms,
    e.retry_count,
    e.http_status,
    e.idempotency_key,
    e.correlation_id
FROM action_executions e
JOIN actions_registry a ON e.action_id = a.action_id
LEFT JOIN resource_types rt ON a.resource_type_id = rt.resource_type_id;

-- =============================================================================
-- Functions for common operations
-- =============================================================================

-- Function: Generate idempotency key based on template
CREATE OR REPLACE FUNCTION generate_idempotency_key(
    template TEXT,
    resource_type_name TEXT,
    environment_name TEXT,
    cbu_id_val UUID,
    action_id_val UUID,
    dsl_version_id_val UUID
) RETURNS TEXT AS $$
BEGIN
    RETURN replace(
        replace(
            replace(
                replace(
                    replace(template, '{{resource_type}}', resource_type_name),
                    '{{environment}}', environment_name
                ),
                '{{cbu_id}}', cbu_id_val::text
            ),
            '{{action_id}}', action_id_val::text
        ),
        '{{dsl_version_id}}', dsl_version_id_val::text
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function: Generate correlation ID based on template
CREATE OR REPLACE FUNCTION generate_correlation_id(
    template TEXT,
    cbu_id_val UUID,
    action_id_val UUID,
    resource_type_name TEXT
) RETURNS TEXT AS $$
BEGIN
    RETURN replace(
        replace(
            replace(template, '{{cbu_id}}', cbu_id_val::text),
            '{{action_id}}', action_id_val::text
        ),
        '{{resource_type}}', resource_type_name
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function: Get resource type endpoint URL for lifecycle action
CREATE OR REPLACE FUNCTION get_resource_endpoint_url(
    resource_type_name TEXT,
    lifecycle_action TEXT,
    environment_name TEXT DEFAULT 'production'
) RETURNS TEXT AS $$
DECLARE
    endpoint_url TEXT;
BEGIN
    SELECT rte.endpoint_url INTO endpoint_url
    FROM resource_type_endpoints rte
    JOIN resource_types rt ON rte.resource_type_id = rt.resource_type_id
    WHERE rt.resource_type_name = $1
    AND rte.lifecycle_action = $2
    AND rte.environment = $3
    AND rt.active = true;

    RETURN endpoint_url;
END;
$$ LANGUAGE plpgsql;

COMMENT ON SCHEMA public IS 'Runtime API Endpoints System - Phase 1 Foundation';