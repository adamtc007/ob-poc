-- Service-resource data dictionary tranche 3.
--
-- Completes the v0.4 tranche-2 items: governed SRDEF sync metadata,
-- richer requirement evaluation state, resource-owner principal capability
-- projection, and the L4 capability-binding reconciliation surface.

ALTER TYPE sem_reg.object_type ADD VALUE IF NOT EXISTS 'service_resource_def';

CREATE TABLE IF NOT EXISTS "ob-poc".applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    vendor VARCHAR(255),
    owner_team VARCHAR(255),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (name)
);

CREATE TABLE IF NOT EXISTS "ob-poc".application_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES "ob-poc".applications(id),
    environment VARCHAR(50) NOT NULL,
    instance_label VARCHAR(255) NOT NULL,
    lifecycle_status VARCHAR(40) NOT NULL DEFAULT 'PROVISIONED'
        CHECK (lifecycle_status IN (
            'PROVISIONED',
            'ACTIVE',
            'MAINTENANCE_WINDOW',
            'DEGRADED',
            'OFFLINE',
            'DECOMMISSIONED'
        )),
    last_health_check_at TIMESTAMPTZ,
    health_check_status VARCHAR(20),
    decommissioned_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (application_id, environment, instance_label)
);

CREATE TABLE IF NOT EXISTS "ob-poc".capability_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_instance_id UUID NOT NULL
        REFERENCES "ob-poc".application_instances(id),
    service_id UUID NOT NULL,
    binding_status VARCHAR(20) NOT NULL DEFAULT 'DRAFT'
        CHECK (binding_status IN (
            'DRAFT',
            'PILOT',
            'LIVE',
            'DEPRECATED',
            'RETIRED'
        )),
    pilot_started_at TIMESTAMPTZ,
    promoted_live_at TIMESTAMPTZ,
    deprecated_at TIMESTAMPTZ,
    retired_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (application_instance_id, service_id)
);

CREATE INDEX IF NOT EXISTS idx_application_instances_status
    ON "ob-poc".application_instances(lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_capability_bindings_status
    ON "ob-poc".capability_bindings(binding_status);

CREATE INDEX IF NOT EXISTS idx_capability_bindings_service
    ON "ob-poc".capability_bindings(service_id);

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_snapshot_hash TEXT;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_synced_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS attribute_gap_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS attribute_conflict_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS binding_policy JSONB NOT NULL DEFAULT '{}';

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS l4_binding_required BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS bound_application_id UUID;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS bound_application_instance_id UUID;

ALTER TABLE "ob-poc".resource_owner_principals
ADD COLUMN IF NOT EXISTS principal_kind TEXT NOT NULL DEFAULT 'resource_owner'
    CHECK (principal_kind IN ('resource_owner'));

ALTER TABLE "ob-poc".resource_owner_principals
ADD COLUMN IF NOT EXISTS principal_capabilities JSONB NOT NULL DEFAULT '["resource_owner"]';

ALTER TABLE "ob-poc".resource_owner_principals
ADD COLUMN IF NOT EXISTS dispatch_enabled BOOLEAN NOT NULL DEFAULT TRUE;

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS requirement_status TEXT NOT NULL DEFAULT 'synced'
    CHECK (requirement_status IN ('synced', 'missing_attribute_def', 'conflict'));

ALTER TABLE "ob-poc".resource_attribute_requirements
ADD COLUMN IF NOT EXISTS conflict_reason TEXT;

ALTER TABLE "ob-poc".onboarding_data_request_slices
ADD COLUMN IF NOT EXISTS l4_binding_required BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE "ob-poc".onboarding_data_request_slices
ADD COLUMN IF NOT EXISTS l4_binding_status TEXT NOT NULL DEFAULT 'not_required'
    CHECK (l4_binding_status IN ('not_required', 'resolved', 'missing_live_binding'));

ALTER TABLE "ob-poc".onboarding_data_request_slices
ADD COLUMN IF NOT EXISTS l4_blocking_reason TEXT;

ALTER TABLE "ob-poc".onboarding_data_request_attrs
ADD COLUMN IF NOT EXISTS constraint_status TEXT NOT NULL DEFAULT 'not_evaluated'
    CHECK (constraint_status IN ('not_evaluated', 'valid', 'invalid'));

ALTER TABLE "ob-poc".onboarding_data_request_attrs
ADD COLUMN IF NOT EXISTS evidence_status TEXT NOT NULL DEFAULT 'not_evaluated'
    CHECK (evidence_status IN ('not_evaluated', 'not_required', 'required_missing', 'provided'));

ALTER TABLE "ob-poc".onboarding_data_request_attrs
ADD COLUMN IF NOT EXISTS evaluation_detail JSONB NOT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS idx_service_resource_types_srdef_snapshot_hash
    ON "ob-poc".service_resource_types(srdef_snapshot_hash)
    WHERE srdef_snapshot_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_service_resource_types_l4_binding
    ON "ob-poc".service_resource_types(l4_binding_required, bound_application_instance_id)
    WHERE l4_binding_required;

CREATE INDEX IF NOT EXISTS idx_onboarding_data_slices_l4_status
    ON "ob-poc".onboarding_data_request_slices(l4_binding_status)
    WHERE l4_binding_status <> 'not_required';

COMMENT ON COLUMN "ob-poc".service_resource_types.srdef_snapshot_hash IS
    'Deterministic content hash of the governed SRDEF snapshot projected from YAML/SemOS source.';

COMMENT ON COLUMN "ob-poc".service_resource_types.lifecycle_status IS
    'SemOS-visible SRDEF lifecycle: unsynced, synced, gaps_found, complete.';

COMMENT ON COLUMN "ob-poc".resource_owner_principals.principal_capabilities IS
    'SemOS principal capability projection. Resource-owner principals carry resource_owner.';

COMMENT ON COLUMN "ob-poc".onboarding_data_request_slices.l4_binding_status IS
    'L4 reconciliation status for request SRDEFs that bind to an application capability.';
