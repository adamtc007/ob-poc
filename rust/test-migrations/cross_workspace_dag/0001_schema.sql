-- Cross-workspace DAG test-harness schema (Phase 1 MVP).
--
-- Curated subset of production tables touched by the cross-workspace
-- runtime tests. Each test gets a fresh ephemeral DB via #[sqlx::test];
-- this single migration runs once per test.
--
-- Conventions:
--   - All UUIDs supplied explicitly by the seed engine (no uuidv7() / gen_random_uuid()
--     defaults — keeps tests deterministic and avoids extension dependencies).
--   - CHECK constraints on `status` columns are KEPT verbatim from production —
--     they validate that test data declares legal lifecycle states.
--   - FK constraints are DROPPED in the test schema. The seed engine inserts
--     in the order declared in scenario YAML; the cross-workspace runtime
--     under test doesn't depend on FK enforcement.
--   - Embedding columns, JSONB defaults, and audit timestamps are kept as
--     nullable / DEFAULT NOW() so test inserts only need to supply a few
--     columns (entity_id, status, FK-bridge UUIDs).
--
-- Coverage as of Phase 1 MVP: 4 tables sufficient to exercise
--   - deal_contracted_requires_kyc_approved
--   - deal_contracted_requires_bp_approved
--   - cbu_validated_requires_kyc_case_approved
--
-- Phase 2+ adds: trading_profiles, services, capability_bindings,
--   application_instances, cbu_evidence, cbu_service_consumption.

CREATE SCHEMA IF NOT EXISTS "ob-poc";

-- ─────────────────────────────────────────────────────────────────
-- cbus  (cbu workspace, slot=cbu, state column = status)
-- ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS "ob-poc".cbus (
    cbu_id uuid PRIMARY KEY,
    name varchar(255) NOT NULL,
    description text,
    primary_client_group_id uuid,
    cbu_category varchar(50),
    status varchar(30) DEFAULT 'DISCOVERED',
    operational_status varchar(30),
    disposition_status varchar(30) DEFAULT 'active',
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_cbu_status CHECK (
        status IN ('DISCOVERED','VALIDATION_PENDING','VALIDATED',
                   'UPDATE_PENDING_PROOF','VALIDATION_FAILED')
    )
);

-- ─────────────────────────────────────────────────────────────────
-- cases  (kyc workspace, slot=kyc_case, state column = status)
-- ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS "ob-poc".cases (
    case_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL,
    case_ref varchar(30) NOT NULL,
    sponsor_cbu_id uuid,
    client_group_id uuid,
    deal_id uuid,
    status varchar(30) DEFAULT 'INTAKE' NOT NULL,
    case_type varchar(30) DEFAULT 'NEW_CLIENT',
    escalation_level varchar(30) DEFAULT 'STANDARD' NOT NULL,
    risk_rating varchar(20),
    opened_at timestamp with time zone DEFAULT now() NOT NULL,
    closed_at timestamp with time zone,
    notes text,
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cases_chk_case_status CHECK (
        status IN ('INTAKE','DISCOVERY','ASSESSMENT','REVIEW','APPROVED',
                   'REJECTED','BLOCKED','WITHDRAWN','DO_NOT_ONBOARD')
    )
);

-- ─────────────────────────────────────────────────────────────────
-- deals  (deal workspace, slot=deal, state column = deal_status)
-- ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS "ob-poc".deals (
    deal_id uuid PRIMARY KEY,
    deal_name varchar(255) NOT NULL,
    deal_reference varchar(100),
    primary_client_group_id uuid NOT NULL,
    deal_status varchar(50) DEFAULT 'PROSPECT' NOT NULL,
    estimated_revenue numeric(18,2),
    currency_code varchar(3) DEFAULT 'USD',
    opened_at timestamp with time zone DEFAULT now() NOT NULL,
    contracted_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT deals_status_check CHECK (
        deal_status IN ('PROSPECT','QUALIFYING','NEGOTIATING','BAC_APPROVAL',
                        'KYC_CLEARANCE','CONTRACTED','ONBOARDING','ACTIVE',
                        'SUSPENDED','WINDING_DOWN','OFFBOARDED','CANCELLED',
                        'LOST','REJECTED','WITHDRAWN')
    )
);

-- ─────────────────────────────────────────────────────────────────
-- booking_principal_clearances  (booking_principal workspace, slot=clearance)
-- state column = clearance_status
-- ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS "ob-poc".booking_principal_clearances (
    id uuid PRIMARY KEY,
    booking_principal_id uuid NOT NULL,
    deal_id uuid,
    cbu_id uuid,
    clearance_status varchar(20) DEFAULT 'PENDING' NOT NULL,
    screening_started_at timestamp with time zone,
    approved_at timestamp with time zone,
    rejected_at timestamp with time zone,
    rejection_reason text,
    activated_at timestamp with time zone,
    suspended_at timestamp with time zone,
    revoked_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT booking_principal_clearances_status_check CHECK (
        clearance_status IN (
            'PENDING','SCREENING','APPROVED','REJECTED',
            'ACTIVE','SUSPENDED','REVOKED'
        )
    )
);

-- ─────────────────────────────────────────────────────────────────
-- Phase 2 additions (2026-04-26)
-- ─────────────────────────────────────────────────────────────────

-- cbu_trading_profiles  (instrument_matrix workspace, slot=trading_profile)
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_trading_profiles (
    profile_id uuid PRIMARY KEY,
    cbu_id uuid,
    version integer DEFAULT 1 NOT NULL,
    status varchar(20) DEFAULT 'DRAFT' NOT NULL,
    document jsonb DEFAULT '{}'::jsonb NOT NULL,
    document_hash text DEFAULT 'test-hash' NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    activated_at timestamp with time zone,
    notes text,
    CONSTRAINT chk_trading_profile_status CHECK (
        status IN ('DRAFT','SUBMITTED','APPROVED','PARALLEL_RUN','ACTIVE',
                   'SUSPENDED','REJECTED','SUPERSEDED','ARCHIVED')
    )
);

-- cbu_service_consumption  (cbu workspace, slot=service_consumption)
--
-- Test-schema NOTE: client_group_id is added here as a denormalized
-- bridging column so the simple-equality SqlPredicateResolver can
-- evaluate `deals.primary_client_group_id = this_consumption_cbu.client_group_id`
-- without following a join. Production cbu_service_consumption does NOT
-- have this column — the predicate would silently fail to resolve in
-- prod (separate tech-debt issue: production resolver needs join support
-- for this constraint to actually fire). Tracked as follow-on work.
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_consumption (
    consumption_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL,
    service_kind varchar(40) DEFAULT 'CUSTODY' NOT NULL,
    service_id uuid,
    client_group_id uuid,
    status varchar(30) DEFAULT 'proposed' NOT NULL,
    provisioned_at timestamp with time zone,
    activated_at timestamp with time zone,
    retired_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cbu_service_consumption_status_check CHECK (
        status IN ('proposed','provisioned','active','suspended',
                   'winding_down','retired')
    ),
    CONSTRAINT cbu_service_consumption_service_kind_check CHECK (
        service_kind IN ('CUSTODY','TA','FA','SEC_LENDING','FX',
                          'TRADING','REPORTING','PRICING','COLLATERAL')
    )
);

-- services  (product_maintenance workspace, slot=service)
CREATE TABLE IF NOT EXISTS "ob-poc".services (
    service_id uuid PRIMARY KEY,
    name varchar(255) NOT NULL,
    description text,
    service_code varchar(50),
    service_category varchar(100),
    is_active boolean DEFAULT true,
    lifecycle_status varchar(20) DEFAULT 'ungoverned' NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT services_lifecycle_status_check CHECK (
        lifecycle_status IN ('ungoverned','draft','active','deprecated','retired')
    )
);

-- application_instances  (lifecycle_resources workspace, slot=application_instance)
CREATE TABLE IF NOT EXISTS "ob-poc".application_instances (
    id uuid PRIMARY KEY,
    application_id uuid,
    environment varchar(50) DEFAULT 'test' NOT NULL,
    instance_label varchar(255) DEFAULT 'test-instance' NOT NULL,
    lifecycle_status varchar(40) DEFAULT 'PROVISIONED' NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT application_instances_lifecycle_status_check CHECK (
        lifecycle_status IN (
            'PROVISIONED','ACTIVE','MAINTENANCE_WINDOW','DEGRADED',
            'OFFLINE','DECOMMISSIONED'
        )
    )
);

-- capability_bindings  (lifecycle_resources workspace, slot=capability_binding)
CREATE TABLE IF NOT EXISTS "ob-poc".capability_bindings (
    id uuid PRIMARY KEY,
    application_instance_id uuid NOT NULL,
    service_id uuid NOT NULL,
    binding_status varchar(20) DEFAULT 'DRAFT' NOT NULL,
    pilot_started_at timestamp with time zone,
    promoted_live_at timestamp with time zone,
    deprecated_at timestamp with time zone,
    retired_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT capability_bindings_status_check CHECK (
        binding_status IN ('DRAFT','PILOT','LIVE','DEPRECATED','RETIRED')
    )
);
