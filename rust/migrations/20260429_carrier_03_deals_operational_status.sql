-- 20260429_carrier_03_deals_operational_status.sql
-- Phase 2.5 of refactor-todo-2026-04-29.md (D-007).
-- Adds deals.operational_status carrier for M-046 (deal operational
-- dual_lifecycle, 5 states). Includes B1 backfill: existing rows in
-- deals.deal_status = 'ONBOARDING' are migrated to
-- deal_status = 'CONTRACTED', operational_status = 'ONBOARDING'
-- BEFORE the deals_status_check tightening in carrier_04.
--
-- This sequencing is required: B1 (live DB has 2 rows in ONBOARDING
-- as of 2026-04-29) cannot be retained under the new 9-state
-- commercial CHECK that drops ONBOARDING.

ALTER TABLE "ob-poc".deals
    ADD COLUMN IF NOT EXISTS operational_status text;

DO $$
BEGIN
    ALTER TABLE "ob-poc".deals
        DROP CONSTRAINT IF EXISTS deals_operational_status_check;
    ALTER TABLE "ob-poc".deals
        ADD CONSTRAINT deals_operational_status_check CHECK (
            operational_status IS NULL
            OR operational_status IN (
                'ONBOARDING','ACTIVE','SUSPENDED','WINDING_DOWN','OFFBOARDED'
            )
        );
END$$;

-- B1 backfill: move ONBOARDING rows from deal_status to operational_status.
UPDATE "ob-poc".deals
    SET deal_status = 'CONTRACTED',
        operational_status = 'ONBOARDING'
    WHERE deal_status = 'ONBOARDING';

-- Defensive: any remaining commercial-terminal rows that imply operational
-- progression get operational_status seeded. ACTIVE / SUSPENDED /
-- WINDING_DOWN / OFFBOARDED rows in deal_status should be similarly moved.
UPDATE "ob-poc".deals
    SET deal_status = 'CONTRACTED',
        operational_status = deal_status
    WHERE deal_status IN ('ACTIVE','SUSPENDED','WINDING_DOWN','OFFBOARDED')
      AND operational_status IS NULL;

-- BAC_APPROVAL / KYC_CLEARANCE rows (none in live DB as of 2026-04-29)
-- are folded into IN_CLEARANCE in carrier_04, where the deals_status_check
-- is rewritten to include IN_CLEARANCE. Doing it here would violate the
-- still-active 15-state CHECK that doesn't yet allow IN_CLEARANCE.

-- Materialises: M-046 · DAG: deal_dag.yaml · Substrate audit: S-9 (+ B1 backfill)
