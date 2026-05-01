-- 20260429_carrier_01_cbu_service_consumption.sql
-- Phase 2.1 of refactor-todo-2026-04-29.md (D-005).
-- The cbu_service_consumption table was scaffolded in
-- 20260424_tranche_2_3_dag_alignment.sql with the M-039 6-state CHECK
-- and the (cbu_id, service_kind) UNIQUE. This migration extends it
-- with the two linkage columns S-15 calls for: service_id (which
-- catalogue service is consumed) and onboarding_request_id (the Deal
-- handoff that provisioned this consumption).

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_consumption (
    consumption_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    service_kind text NOT NULL CHECK (service_kind IN (
        'CUSTODY','TA','FA','SEC_LENDING','FX','TRADING','REPORTING','PRICING','COLLATERAL'
    )),
    status text NOT NULL CHECK (status IN (
        'proposed','provisioned','active','suspended','winding_down','retired'
    )),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (cbu_id, service_kind)
);

-- S-15 linkage columns. NULL-tolerant: pre-existing rows have no FK
-- attribution; new rows from the Deal→Ops handoff carry both.
ALTER TABLE "ob-poc".cbu_service_consumption
    ADD COLUMN IF NOT EXISTS service_id uuid;
ALTER TABLE "ob-poc".cbu_service_consumption
    ADD COLUMN IF NOT EXISTS onboarding_request_id uuid;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
            WHERE conrelid = '"ob-poc".cbu_service_consumption'::regclass
              AND conname = 'cbu_service_consumption_service_id_fkey'
    ) THEN
        ALTER TABLE "ob-poc".cbu_service_consumption
            ADD CONSTRAINT cbu_service_consumption_service_id_fkey
            FOREIGN KEY (service_id)
            REFERENCES "ob-poc".services(service_id)
            ON DELETE SET NULL;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
            WHERE conrelid = '"ob-poc".cbu_service_consumption'::regclass
              AND conname = 'cbu_service_consumption_onboarding_request_id_fkey'
    ) THEN
        ALTER TABLE "ob-poc".cbu_service_consumption
            ADD CONSTRAINT cbu_service_consumption_onboarding_request_id_fkey
            FOREIGN KEY (onboarding_request_id)
            REFERENCES "ob-poc".deal_onboarding_requests(request_id)
            ON DELETE SET NULL;
    END IF;
END$$;

CREATE INDEX IF NOT EXISTS idx_cbu_service_consumption_cbu
    ON "ob-poc".cbu_service_consumption(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_service_consumption_service
    ON "ob-poc".cbu_service_consumption(service_id);
CREATE INDEX IF NOT EXISTS idx_cbu_service_consumption_onboarding_request
    ON "ob-poc".cbu_service_consumption(onboarding_request_id);

COMMENT ON TABLE "ob-poc".cbu_service_consumption IS
    'Operational layer: per-(cbu, service_kind) provisioning lifecycle. State machine M-039, 6 states (proposed, provisioned, active, suspended, winding_down, retired). Distinct from service_intents (M-026) which models intent at the (cbu, product/service) grain. service_id + onboarding_request_id close S-15 (Deal→Ops handoff attribution).';

-- Materialises: M-039 · DAG: cbu_dag.yaml · Substrate audit: S-1, S-15
