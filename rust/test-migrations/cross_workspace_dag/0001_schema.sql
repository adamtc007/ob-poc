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
