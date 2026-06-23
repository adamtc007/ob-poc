-- Phase 1 catalogue governance: align products and services with the
-- service_resource_types owner/governance pattern.

ALTER TABLE "ob-poc".products
    ADD COLUMN IF NOT EXISTS owner_principal_fqn text,
    ADD COLUMN IF NOT EXISTS governance_status text NOT NULL DEFAULT 'active',
    ADD COLUMN IF NOT EXISTS created_by text,
    ADD COLUMN IF NOT EXISTS created_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text),
    ADD COLUMN IF NOT EXISTS updated_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text);

ALTER TABLE "ob-poc".products
    DROP CONSTRAINT IF EXISTS products_owner_principal_fkey;

ALTER TABLE "ob-poc".products
    ADD CONSTRAINT products_owner_principal_fkey
    FOREIGN KEY (owner_principal_fqn)
    REFERENCES "ob-poc".resource_owner_principals(owner_principal_fqn)
    ON DELETE SET NULL;

ALTER TABLE "ob-poc".products
    DROP CONSTRAINT IF EXISTS products_governance_status_check;

ALTER TABLE "ob-poc".products
    ADD CONSTRAINT products_governance_status_check
    CHECK (governance_status IN ('draft', 'active', 'deprecated', 'retired'));

CREATE INDEX IF NOT EXISTS idx_products_owner_principal
    ON "ob-poc".products(owner_principal_fqn)
    WHERE owner_principal_fqn IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_products_governance_status
    ON "ob-poc".products(governance_status);

COMMENT ON COLUMN "ob-poc".products.owner_principal_fqn IS
    'Resource-owner principal that governs this product catalogue entry.';

COMMENT ON COLUMN "ob-poc".products.governance_status IS
    'Catalogue governance status, matching service_resource_types.governance_status.';

ALTER TABLE "ob-poc".services
    ADD COLUMN IF NOT EXISTS owner_principal_fqn text,
    ADD COLUMN IF NOT EXISTS governance_status text NOT NULL DEFAULT 'active',
    ADD COLUMN IF NOT EXISTS created_by text,
    ADD COLUMN IF NOT EXISTS created_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text),
    ADD COLUMN IF NOT EXISTS updated_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text);

ALTER TABLE "ob-poc".services
    DROP CONSTRAINT IF EXISTS services_owner_principal_fkey;

ALTER TABLE "ob-poc".services
    ADD CONSTRAINT services_owner_principal_fkey
    FOREIGN KEY (owner_principal_fqn)
    REFERENCES "ob-poc".resource_owner_principals(owner_principal_fqn)
    ON DELETE SET NULL;

ALTER TABLE "ob-poc".services
    DROP CONSTRAINT IF EXISTS services_governance_status_check;

ALTER TABLE "ob-poc".services
    ADD CONSTRAINT services_governance_status_check
    CHECK (governance_status IN ('draft', 'active', 'deprecated', 'retired'));

CREATE INDEX IF NOT EXISTS idx_services_owner_principal
    ON "ob-poc".services(owner_principal_fqn)
    WHERE owner_principal_fqn IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_services_governance_status
    ON "ob-poc".services(governance_status);

COMMENT ON COLUMN "ob-poc".services.owner_principal_fqn IS
    'Resource-owner principal that governs this service catalogue entry.';

COMMENT ON COLUMN "ob-poc".services.governance_status IS
    'Catalogue governance status, matching service_resource_types.governance_status.';
