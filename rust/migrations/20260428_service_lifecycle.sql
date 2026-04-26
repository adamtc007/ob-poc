-- Service lifecycle promotion (Tranche 4 R2 — 2026-04-26).
--
-- Backfills the schema for the service + service_version state machines
-- declared in product_service_taxonomy_dag.yaml §2 (R2 promotion of the
-- previously stateless service slot to stateful, modelled after
-- attribute_def_lifecycle).
--
-- Context.
--   Pre-R2: the service catalogue (services table) was stateless reference
--   data. cbu.service_consumption could transition proposed → provisioned
--   without checking whether the underlying service was published in the
--   catalogue.
--
--   R2 promotes service to stateful (5-state lifecycle: ungoverned →
--   draft → active → deprecated → retired) and adds a per-version
--   companion table (service_versions) with its own 5-state lifecycle
--   (drafted → reviewed → published → superseded → retired). The new
--   cross-workspace constraint service_consumption_requires_active_service
--   in cbu_dag.yaml gates provisioning on lifecycle_status = 'active'.
--
-- Design.
--   - lifecycle_status added to "ob-poc".services (the canonical service
--     catalogue table; product_services is a junction table without a
--     surrogate id and is not the right carrier for service state).
--   - service_versions is a new table; FK references services(service_id).
--   - Existing service rows backfill to 'ungoverned' (default).
--     Tranche 4 R6 governance pass will progress them via changesets.
--
-- Forward-only. No data migration beyond the column default.
--
-- Parent docs:
--   docs/todo/onboarding-dag-remediation-plan-2026-04-26.md §"Slice R2"
--   product_service_taxonomy_dag.yaml §2 (R2 stateful slot promotion)

BEGIN;

-- 1. service lifecycle column on the canonical services catalogue table.
ALTER TABLE "ob-poc".services
  ADD COLUMN IF NOT EXISTS lifecycle_status varchar(20)
    NOT NULL DEFAULT 'ungoverned';

ALTER TABLE "ob-poc".services
  DROP CONSTRAINT IF EXISTS services_lifecycle_status_check;

ALTER TABLE "ob-poc".services
  ADD CONSTRAINT services_lifecycle_status_check
  CHECK (lifecycle_status IN (
    'ungoverned', 'draft', 'active', 'deprecated', 'retired'
  ));

CREATE INDEX IF NOT EXISTS idx_services_lifecycle_status
  ON "ob-poc".services(lifecycle_status);

COMMENT ON COLUMN "ob-poc".services.lifecycle_status IS
    'Service catalogue lifecycle (R2). 5 states: ungoverned (entry), '
    'draft, active (published in changeset), deprecated, retired '
    '(terminal). Cross-workspace constraint in cbu_dag.yaml requires '
    'active for cbu.service_consumption.proposed → provisioned.';

-- 2. service_versions — per-version lifecycle keyed to a parent service.
CREATE TABLE IF NOT EXISTS "ob-poc".service_versions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id uuid NOT NULL
        REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    version varchar(20) NOT NULL,
    lifecycle_status varchar(20) NOT NULL DEFAULT 'drafted'
        CHECK (lifecycle_status IN (
            'drafted', 'reviewed', 'published', 'superseded', 'retired'
        )),
    spec jsonb,
    drafted_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text),
    reviewed_at timestamptz,
    published_at timestamptz,
    superseded_at timestamptz,
    retired_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamptz DEFAULT (now() AT TIME ZONE 'utc'::text),
    UNIQUE (service_id, version)
);

CREATE INDEX IF NOT EXISTS idx_service_versions_status
  ON "ob-poc".service_versions(lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_service_versions_service_id
  ON "ob-poc".service_versions(service_id);

COMMENT ON TABLE "ob-poc".service_versions IS
    'Per-version lifecycle for service catalogue entries (R2). Each '
    'service may have many versions; each version progresses through '
    'drafted → reviewed → published → superseded → retired. The current '
    'published version is the one consumed by downstream workspaces.';

COMMENT ON COLUMN "ob-poc".service_versions.lifecycle_status IS
    'Per-version lifecycle: drafted (entry) → reviewed → published → '
    'superseded → retired (terminal).';

COMMIT;

-- Verification (run manually after migration):
--   SELECT column_name, data_type, column_default
--     FROM information_schema.columns
--    WHERE table_schema = 'ob-poc'
--      AND table_name = 'services'
--      AND column_name = 'lifecycle_status';
--
--   SELECT table_name FROM information_schema.tables
--    WHERE table_schema = 'ob-poc' AND table_name = 'service_versions';
--
--   SELECT lifecycle_status, COUNT(*) FROM "ob-poc".services
--    GROUP BY lifecycle_status;
