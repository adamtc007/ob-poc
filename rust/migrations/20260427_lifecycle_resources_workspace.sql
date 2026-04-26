-- Lifecycle Resources workspace (Tranche 4 R1, 2026-04-26).
--
-- Models BNY application instances and their binding to product services.
-- Closes the Layer 4 gap identified by the deep review:
--   docs/todo/onboarding-dag-deep-review-2026-04-26.md
--
-- Layer chain:
--   Layer 1 Deal → Layer 2 CBU → Layer 3 Product Service Taxonomy →
--   Layer 4 Lifecycle Resources (this migration)
--
-- Forward-only. No data backfill required (this is greenfield).
--
-- Parent docs:
--   docs/todo/onboarding-dag-remediation-plan-2026-04-26.md (Slice R1)
--   rust/config/sem_os_seeds/dag_taxonomies/lifecycle_resources_dag.yaml
--   rust/config/verbs/{application,application-instance,capability-binding}.yaml
--
-- Note: capability_bindings.service_id is a plain uuid in this slice.
-- R2 (service lifecycle) will add an FK constraint to product_services(id)
-- once that table acquires its stateful lifecycle and authoritative pkey.

BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".applications (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name varchar(255) NOT NULL,
    vendor varchar(255),
    owner_team varchar(255),
    description text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (name)
);

COMMENT ON TABLE "ob-poc".applications IS
    'Layer 4 application registry — catalogue card per BNY application '
    '(vendor + owner team + description). Stateless reference data. '
    'Lifecycle is at the application_instance level, not here.';

CREATE TABLE IF NOT EXISTS "ob-poc".application_instances (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id uuid NOT NULL REFERENCES "ob-poc".applications(id),
    environment varchar(50) NOT NULL,
    instance_label varchar(255) NOT NULL,
    lifecycle_status varchar(40) NOT NULL DEFAULT 'PROVISIONED'
        CHECK (lifecycle_status IN (
            'PROVISIONED',
            'ACTIVE',
            'MAINTENANCE_WINDOW',
            'DEGRADED',
            'OFFLINE',
            'DECOMMISSIONED'
        )),
    last_health_check_at timestamptz,
    health_check_status varchar(20),
    decommissioned_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (application_id, environment, instance_label)
);

COMMENT ON TABLE "ob-poc".application_instances IS
    'Layer 4 per-instance lifecycle. Tracks which BNY application instance '
    'in which environment is provisioned, active, in maintenance, degraded, '
    'offline, or decommissioned. State machine: '
    'application_instance_lifecycle in lifecycle_resources_dag.yaml.';

COMMENT ON COLUMN "ob-poc".application_instances.environment IS
    'Deployment environment label, e.g. prod-eu / prod-us / uat / dev.';

COMMENT ON COLUMN "ob-poc".application_instances.lifecycle_status IS
    'Operational state: PROVISIONED (entry) → ACTIVE → '
    'MAINTENANCE_WINDOW / DEGRADED / OFFLINE → DECOMMISSIONED (terminal).';

CREATE INDEX IF NOT EXISTS idx_application_instances_status
    ON "ob-poc".application_instances(lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_application_instances_app
    ON "ob-poc".application_instances(application_id);

CREATE TABLE IF NOT EXISTS "ob-poc".capability_bindings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    application_instance_id uuid NOT NULL
        REFERENCES "ob-poc".application_instances(id),
    service_id uuid NOT NULL,
    binding_status varchar(20) NOT NULL DEFAULT 'DRAFT'
        CHECK (binding_status IN (
            'DRAFT',
            'PILOT',
            'LIVE',
            'DEPRECATED',
            'RETIRED'
        )),
    pilot_started_at timestamptz,
    promoted_live_at timestamptz,
    deprecated_at timestamptz,
    retired_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (application_instance_id, service_id)
);

COMMENT ON TABLE "ob-poc".capability_bindings IS
    'Layer 4 per-(application_instance, service) binding lifecycle. '
    'Parent slot in DAG: application_instance. Cascade: parent '
    'DECOMMISSIONED forces child binding to RETIRED. service_id is a '
    'plain uuid in this slice (R2 will add FK to product_services).';

COMMENT ON COLUMN "ob-poc".capability_bindings.binding_status IS
    'Binding state: DRAFT (entry) → PILOT → LIVE → DEPRECATED → '
    'RETIRED (terminal). Bindings only enable downstream service '
    'consumption when LIVE on an ACTIVE application_instance.';

CREATE INDEX IF NOT EXISTS idx_capability_bindings_status
    ON "ob-poc".capability_bindings(binding_status);

CREATE INDEX IF NOT EXISTS idx_capability_bindings_instance
    ON "ob-poc".capability_bindings(application_instance_id);

CREATE INDEX IF NOT EXISTS idx_capability_bindings_service
    ON "ob-poc".capability_bindings(service_id);

COMMIT;

-- Verification (run manually after migration):
--   SELECT table_name FROM information_schema.tables
--     WHERE table_schema = 'ob-poc'
--       AND table_name IN (
--         'applications',
--         'application_instances',
--         'capability_bindings'
--       );
--
--   SELECT conname, pg_get_constraintdef(oid)
--     FROM pg_constraint
--     WHERE conname IN (
--       'application_instances_lifecycle_status_check',
--       'capability_bindings_binding_status_check'
--     );
