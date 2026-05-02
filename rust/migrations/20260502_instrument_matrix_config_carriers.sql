-- Carrier tables for instrument-matrix config lifecycles declared in
-- instrument_matrix_dag.yaml and rust/config/verbs/*.yaml.

BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_reconciliation_configs (
    config_id uuid PRIMARY KEY DEFAULT uuidv7(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    stream text NOT NULL,
    sor text NOT NULL,
    tolerance numeric,
    status text NOT NULL DEFAULT 'draft',
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by text,
    CONSTRAINT cbu_reconciliation_configs_status_check CHECK (
        status IN ('draft', 'active', 'suspended', 'retired')
    )
);

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_collateral_management (
    collateral_id uuid PRIMARY KEY DEFAULT uuidv7(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    csa_reference uuid,
    threshold numeric,
    minimum_transfer_amount numeric,
    triparty_agent text,
    status text NOT NULL DEFAULT 'configured',
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by text,
    CONSTRAINT cbu_collateral_management_status_check CHECK (
        status IN ('configured', 'active', 'suspended', 'terminated')
    )
);

CREATE TABLE IF NOT EXISTS "ob-poc".corporate_action_events (
    event_id uuid PRIMARY KEY DEFAULT uuidv7(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    external_event_id text,
    event_type text NOT NULL,
    election_option text,
    record_date date,
    payable_date date,
    status text NOT NULL DEFAULT 'election_pending',
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by text,
    CONSTRAINT corporate_action_events_status_check CHECK (
        status IN ('election_pending', 'elected', 'default_applied')
    )
);

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_gateway_connectivity (
    connectivity_id uuid PRIMARY KEY DEFAULT uuidv7(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    gateway_id uuid,
    status text NOT NULL DEFAULT 'PENDING',
    connectivity_resource_id uuid REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    credentials_reference text,
    effective_date date,
    activated_at timestamp with time zone,
    suspended_at timestamp with time zone,
    gateway_config jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cbu_gateway_connectivity_status_check CHECK (
        status IN ('PENDING', 'TESTING', 'ACTIVE', 'SUSPENDED', 'DECOMMISSIONED')
    )
);

COMMIT;

