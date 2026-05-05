-- Service-resource data dictionary tranche 2.
--
-- Promotes the tranche-1 operational loop into SemOS-visible governed
-- surfaces: addressable resource-owner principals and lifecycle metadata on
-- the SRDEF substrate carried by service_resource_types.

CREATE TABLE IF NOT EXISTS "ob-poc".resource_owner_principals (
    owner_principal_fqn TEXT PRIMARY KEY,
    owner_system TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    dispatch_endpoint TEXT,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'retired')),
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE "ob-poc".resource_owner_principals IS
    'Addressable service-resource owner principals used by onboarding data-request dispatch.';

INSERT INTO "ob-poc".resource_owner_principals
    (owner_principal_fqn, owner_system, display_name)
SELECT DISTINCT
    'resource_owner:' || owner,
    owner,
    owner
FROM "ob-poc".service_resource_types
WHERE owner IS NOT NULL
ON CONFLICT (owner_principal_fqn) DO UPDATE
SET owner_system = EXCLUDED.owner_system,
    display_name = EXCLUDED.display_name,
    updated_at = now();

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS lifecycle_status TEXT NOT NULL DEFAULT 'unsynced'
    CHECK (lifecycle_status IN ('unsynced', 'synced', 'gaps_found', 'complete'));

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_lineage TEXT NOT NULL DEFAULT 'yaml';

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_snapshot JSONB NOT NULL DEFAULT '{}';

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS srdef_snapshot_id UUID;

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS owner_principal_fqn TEXT;

UPDATE "ob-poc".service_resource_types
SET owner_principal_fqn = 'resource_owner:' || owner
WHERE owner IS NOT NULL
  AND owner_principal_fqn IS NULL;

ALTER TABLE "ob-poc".service_resource_types
DROP CONSTRAINT IF EXISTS service_resource_types_owner_principal_fkey;

ALTER TABLE "ob-poc".service_resource_types
ADD CONSTRAINT service_resource_types_owner_principal_fkey
FOREIGN KEY (owner_principal_fqn)
REFERENCES "ob-poc".resource_owner_principals(owner_principal_fqn)
ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_service_resource_types_lifecycle_status
    ON "ob-poc".service_resource_types(lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_service_resource_types_owner_principal
    ON "ob-poc".service_resource_types(owner_principal_fqn)
    WHERE owner_principal_fqn IS NOT NULL;

ALTER TABLE "ob-poc".onboarding_data_request_slices
DROP CONSTRAINT IF EXISTS onboarding_data_request_slices_owner_principal_fkey;

ALTER TABLE "ob-poc".onboarding_data_request_slices
ADD CONSTRAINT onboarding_data_request_slices_owner_principal_fkey
FOREIGN KEY (owner_principal_fqn)
REFERENCES "ob-poc".resource_owner_principals(owner_principal_fqn)
ON DELETE SET NULL;

ALTER TABLE "ob-poc".provisioning_requests
DROP CONSTRAINT IF EXISTS provisioning_requests_owner_principal_fkey;

ALTER TABLE "ob-poc".provisioning_requests
ADD CONSTRAINT provisioning_requests_owner_principal_fkey
FOREIGN KEY (owner_principal_fqn)
REFERENCES "ob-poc".resource_owner_principals(owner_principal_fqn)
ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_onboarding_data_slices_owner_principal
    ON "ob-poc".onboarding_data_request_slices(owner_principal_fqn)
    WHERE owner_principal_fqn IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_provisioning_requests_owner_principal
    ON "ob-poc".provisioning_requests(owner_principal_fqn)
    WHERE owner_principal_fqn IS NOT NULL;
