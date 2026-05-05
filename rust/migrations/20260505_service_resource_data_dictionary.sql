-- Service-resource data dictionary tranche 1.
--
-- Adds the onboarding-scoped data-request artefacts, dispatch linkage,
-- and delivery/result state values needed to close the resource-owner loop.

ALTER TABLE "ob-poc".deal_onboarding_requests
ALTER COLUMN request_status SET DEFAULT 'PENDING';

ALTER TABLE "ob-poc".cbu_resource_instances
DROP CONSTRAINT IF EXISTS cbu_resource_instances_status_check;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD CONSTRAINT cbu_resource_instances_status_check CHECK (
    status IN (
        'PENDING',
        'PROVISIONING',
        'AWAITING_OWNER',
        'ACTIVE',
        'FAILED',
        'CANCELLED',
        'SUSPENDED',
        'DECOMMISSIONED'
    )
);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS resource_locator JSONB;

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.resource_locator IS
    'Structured owner-returned locator: {kind, value, identifier, owner_ticket_id}. resource_url remains a compatibility column.';

ALTER TABLE "ob-poc".provisioning_events
DROP CONSTRAINT IF EXISTS provisioning_events_kind_check;

ALTER TABLE "ob-poc".provisioning_events
ADD CONSTRAINT provisioning_events_kind_check CHECK (
    kind IN (
        'REQUEST_PREPARED',
        'DISPATCHED',
        'STAND_DOWN',
        'ACK',
        'RESULT',
        'ERROR',
        'STATUS',
        'RETRY',
        'REQUEST_SENT'
    )
);

COMMENT ON TABLE "ob-poc".provisioning_requests IS
    'Provisioning request row with mutable workflow status plus append-only provisioning_events.';

ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS onboarding_request_id UUID
    REFERENCES "ob-poc".deal_onboarding_requests(request_id) ON DELETE SET NULL;

ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS onboarding_data_request_id UUID;

ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS onboarding_data_request_slice_id UUID;

ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS owner_principal_fqn TEXT;

ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS dispatch_idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_provisioning_requests_dispatch_idem
    ON "ob-poc".provisioning_requests(dispatch_idempotency_key)
    WHERE dispatch_idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_data_requests (
    data_request_id UUID PRIMARY KEY DEFAULT uuidv7(),
    onboarding_request_id UUID NOT NULL UNIQUE
        REFERENCES "ob-poc".deal_onboarding_requests(request_id) ON DELETE CASCADE,
    deal_id UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    contract_id UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    request_status TEXT NOT NULL DEFAULT 'collecting'
        CHECK (request_status IN (
            'collecting',
            'ready_for_dispatch',
            'dispatching',
            'awaiting_owner',
            'completed',
            'blocked',
            'cancelled'
        )),
    compiled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    blocking_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_data_request_discoveries (
    discovery_snapshot_id UUID PRIMARY KEY DEFAULT uuidv7(),
    data_request_id UUID NOT NULL
        REFERENCES "ob-poc".onboarding_data_requests(data_request_id) ON DELETE CASCADE,
    source_discovery_id UUID REFERENCES "ob-poc".srdef_discovery_reasons(discovery_id),
    srdef_id TEXT NOT NULL,
    resource_type_id UUID REFERENCES "ob-poc".service_resource_types(resource_id),
    parameters JSONB NOT NULL DEFAULT '{}',
    discovery_snapshot JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (data_request_id, srdef_id, parameters)
);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_data_request_slices (
    slice_id UUID PRIMARY KEY DEFAULT uuidv7(),
    data_request_id UUID NOT NULL
        REFERENCES "ob-poc".onboarding_data_requests(data_request_id) ON DELETE CASCADE,
    discovery_snapshot_id UUID
        REFERENCES "ob-poc".onboarding_data_request_discoveries(discovery_snapshot_id)
        ON DELETE SET NULL,
    onboarding_request_id UUID NOT NULL
        REFERENCES "ob-poc".deal_onboarding_requests(request_id) ON DELETE CASCADE,
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    srdef_id TEXT NOT NULL,
    resource_type_id UUID REFERENCES "ob-poc".service_resource_types(resource_id),
    srdef_lineage TEXT NOT NULL DEFAULT 'yaml',
    srdef_snapshot_id UUID,
    parameters JSONB NOT NULL DEFAULT '{}',
    owner_system TEXT,
    owner_principal_fqn TEXT,
    application_id UUID,
    application_instance_id UUID,
    cbu_resource_instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    provisioning_request_id UUID REFERENCES "ob-poc".provisioning_requests(request_id),
    slice_status TEXT NOT NULL DEFAULT 'collecting'
        CHECK (slice_status IN (
            'collecting',
            'ready',
            'dispatched',
            'awaiting_owner',
            'activated',
            'blocked',
            'failed',
            'cancelled'
        )),
    blocking_reason TEXT,
    ready_at TIMESTAMPTZ,
    dispatched_at TIMESTAMPTZ,
    activated_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (data_request_id, srdef_id, parameters)
);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_data_request_attrs (
    slice_id UUID NOT NULL
        REFERENCES "ob-poc".onboarding_data_request_slices(slice_id) ON DELETE CASCADE,
    attr_id UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid),
    attr_code TEXT,
    requirement_strength TEXT NOT NULL DEFAULT 'required'
        CHECK (requirement_strength IN ('required', 'optional', 'conditional')),
    condition_expression TEXT,
    condition_status TEXT NOT NULL DEFAULT 'unconditional'
        CHECK (condition_status IN ('unconditional', 'pending', 'satisfied', 'not_applicable')),
    source_policy JSONB NOT NULL DEFAULT '[]',
    evidence_policy JSONB NOT NULL DEFAULT '{}',
    merged_constraints JSONB NOT NULL DEFAULT '{}',
    default_value JSONB,
    value_status TEXT NOT NULL DEFAULT 'missing'
        CHECK (value_status IN ('missing', 'present', 'not_applicable')),
    value_id UUID,
    value_ref JSONB,
    value_observed_at TIMESTAMPTZ,
    blocking_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (slice_id, attr_id)
);

CREATE INDEX IF NOT EXISTS idx_onboarding_data_requests_cbu_status
    ON "ob-poc".onboarding_data_requests(cbu_id, request_status);

CREATE INDEX IF NOT EXISTS idx_onboarding_data_slices_request_status
    ON "ob-poc".onboarding_data_request_slices(data_request_id, slice_status);

CREATE INDEX IF NOT EXISTS idx_onboarding_data_slices_provisioning_request
    ON "ob-poc".onboarding_data_request_slices(provisioning_request_id)
    WHERE provisioning_request_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_onboarding_data_attrs_status
    ON "ob-poc".onboarding_data_request_attrs(slice_id, value_status);

ALTER TABLE "ob-poc".provisioning_requests
ADD CONSTRAINT provisioning_requests_data_request_fkey
FOREIGN KEY (onboarding_data_request_id)
REFERENCES "ob-poc".onboarding_data_requests(data_request_id)
ON DELETE SET NULL;

ALTER TABLE "ob-poc".provisioning_requests
ADD CONSTRAINT provisioning_requests_data_request_slice_fkey
FOREIGN KEY (onboarding_data_request_slice_id)
REFERENCES "ob-poc".onboarding_data_request_slices(slice_id)
ON DELETE SET NULL;

COMMENT ON TABLE "ob-poc".onboarding_data_requests IS
    'Frozen per-onboarding service-resource data dictionary request.';

COMMENT ON TABLE "ob-poc".onboarding_data_request_slices IS
    'Per-SRDEF/per-parameter slice of an onboarding data request.';

COMMENT ON TABLE "ob-poc".onboarding_data_request_attrs IS
    'Frozen attribute dictionary rows for an onboarding data request slice.';

