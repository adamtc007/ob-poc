-- 20260429_carrier_05_cbus_operational_status_reassert.sql
-- Phase 2.4 of refactor-todo-2026-04-29.md (D-009).
-- Idempotent re-assertion: the cbus.operational_status column AND
-- chk_cbu_operational_status CHECK already exist in the live DB with
-- the exact 8-state set this migration codifies. Migration is shipped
-- for traceability (carrier audit trailer) — it's a no-op against the
-- current live schema but ensures the carrier is locked-in for replay
-- against fresh databases.

ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS operational_status text;

DO $$
BEGIN
    -- Live name in production: chk_cbu_operational_status. The TODO §2.4
    -- proposed a slightly different name (cbus_operational_status_check);
    -- preserve the live name to avoid a no-op constraint rename on
    -- production while still tolerating the proposed name on fresh
    -- databases.
    ALTER TABLE "ob-poc".cbus
        DROP CONSTRAINT IF EXISTS chk_cbu_operational_status;
    ALTER TABLE "ob-poc".cbus
        DROP CONSTRAINT IF EXISTS cbus_operational_status_check;
    ALTER TABLE "ob-poc".cbus
        ADD CONSTRAINT chk_cbu_operational_status CHECK (
            operational_status IS NULL
            OR operational_status IN (
                'dormant','trade_permissioned','actively_trading',
                'restricted','suspended','winding_down','offboarded','archived'
            )
        );
END$$;

-- Materialises: M-032 · DAG: cbu_dag.yaml · Substrate audit: S-3
