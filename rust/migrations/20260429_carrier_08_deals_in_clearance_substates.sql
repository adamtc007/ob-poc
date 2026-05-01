-- 20260429_carrier_08_deals_in_clearance_substates.sql
-- Phase 5.2 of refactor-todo-2026-04-29.md (D-012).
-- Adds deals.bac_status and deals.kyc_clearance_status — the parallel
-- substate columns under IN_CLEARANCE. Per Q4/Q21 (b): both substates
-- run independently while deal_status='IN_CLEARANCE'; both must reach
-- 'approved' before the IN_CLEARANCE → CONTRACTED transition is
-- allowed (cross_slot_constraints.deal_contracted_requires_bac_approved
-- + cross_workspace_constraints.deal_contracted_requires_kyc_approved).
--
-- Both columns nullable: rows in deal_status NOT IN ('IN_CLEARANCE',
-- 'CONTRACTED') have no substate semantics. Seed values are written
-- by deal.submit-for-bac (sets bac_status='in_review' on entry to
-- IN_CLEARANCE) and by the KYC workspace's update-kyc-clearance hook.

ALTER TABLE "ob-poc".deals
    ADD COLUMN IF NOT EXISTS bac_status text;
ALTER TABLE "ob-poc".deals
    ADD COLUMN IF NOT EXISTS kyc_clearance_status text;

DO $$
BEGIN
    ALTER TABLE "ob-poc".deals
        DROP CONSTRAINT IF EXISTS deals_bac_status_check;
    ALTER TABLE "ob-poc".deals
        ADD CONSTRAINT deals_bac_status_check CHECK (
            bac_status IS NULL
            OR bac_status IN ('pending','in_review','approved','rejected')
        );

    ALTER TABLE "ob-poc".deals
        DROP CONSTRAINT IF EXISTS deals_kyc_clearance_status_check;
    ALTER TABLE "ob-poc".deals
        ADD CONSTRAINT deals_kyc_clearance_status_check CHECK (
            kyc_clearance_status IS NULL
            OR kyc_clearance_status IN ('pending','in_review','approved','rejected')
        );
END$$;

-- Defensive: any deal already in IN_CLEARANCE (pre-migration BAC_APPROVAL
-- or KYC_CLEARANCE rows that carrier_04 collapsed) get conservative
-- substate seeds. Live row count = 0 as of 2026-04-29.
UPDATE "ob-poc".deals
    SET bac_status = COALESCE(bac_status, 'pending'),
        kyc_clearance_status = COALESCE(kyc_clearance_status, 'pending')
    WHERE deal_status = 'IN_CLEARANCE';

-- Materialises: M-045 (substates) · DAG: deal_dag.yaml · Q21 (b) amendment
