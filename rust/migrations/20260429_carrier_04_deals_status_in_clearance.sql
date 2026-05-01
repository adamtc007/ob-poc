-- 20260429_carrier_04_deals_status_in_clearance.sql
-- Phase 2.3 of refactor-todo-2026-04-29.md (D-008).
-- Tightens deals_status_check to the 9-state commercial-only set per
-- the IN_CLEARANCE compound substate amendment (Q4/Q21 (b)).
--
-- Pre-existing live state set (15): PROSPECT, QUALIFYING, NEGOTIATING,
-- BAC_APPROVAL, KYC_CLEARANCE, CONTRACTED, ONBOARDING, ACTIVE, SUSPENDED,
-- WINDING_DOWN, OFFBOARDED, CANCELLED, LOST, REJECTED, WITHDRAWN.
--
-- New target state set (9, commercial only): PROSPECT, QUALIFYING,
-- NEGOTIATING, IN_CLEARANCE, CONTRACTED, LOST, REJECTED, WITHDRAWN,
-- CANCELLED. Operational states moved to deals.operational_status
-- (carrier_03). BAC and KYC clearance become parallel substates on
-- deals.bac_status / deals.kyc_clearance_status (carrier_08).
--
-- Order of operations within this migration is critical:
--   (1) drop the old CHECK
--   (2) backfill BAC_APPROVAL / KYC_CLEARANCE rows to IN_CLEARANCE
--   (3) install the new 9-state CHECK
-- Reversing (1) and (2) would leave rows that violate the old CHECK
-- briefly; reversing (2) and (3) would fail validation.

DO $$
BEGIN
    ALTER TABLE "ob-poc".deals
        DROP CONSTRAINT IF EXISTS deals_status_check;
END$$;

-- Backfill BAC_APPROVAL / KYC_CLEARANCE → IN_CLEARANCE.
-- Live row count = 0 as of 2026-04-29; defensive in case any creep in.
UPDATE "ob-poc".deals
    SET deal_status = 'IN_CLEARANCE'
    WHERE deal_status IN ('BAC_APPROVAL','KYC_CLEARANCE');

DO $$
BEGIN
    ALTER TABLE "ob-poc".deals
        ADD CONSTRAINT deals_status_check CHECK (deal_status IN (
            'PROSPECT','QUALIFYING','NEGOTIATING',
            'IN_CLEARANCE','CONTRACTED',
            'LOST','REJECTED','WITHDRAWN','CANCELLED'
        ));
END$$;

-- Materialises: M-045 · DAG: deal_dag.yaml · Substrate audit: S-2, S-8
