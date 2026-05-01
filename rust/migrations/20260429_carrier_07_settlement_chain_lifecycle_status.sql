-- 20260429_carrier_07_settlement_chain_lifecycle_status.sql
-- Phase 2.7 of refactor-todo-2026-04-29.md (D-011).
-- Adds cbu_settlement_chains.lifecycle_status carrier for M-021
-- (cbu.settlement_chain_lifecycle, 7 states). The legacy is_active
-- boolean is preserved for backward compat; new readers use
-- lifecycle_status.

ALTER TABLE "ob-poc".cbu_settlement_chains
    ADD COLUMN IF NOT EXISTS lifecycle_status text;

DO $$
BEGIN
    ALTER TABLE "ob-poc".cbu_settlement_chains
        DROP CONSTRAINT IF EXISTS cbu_settlement_chains_lifecycle_status_check;
    ALTER TABLE "ob-poc".cbu_settlement_chains
        ADD CONSTRAINT cbu_settlement_chains_lifecycle_status_check CHECK (
            lifecycle_status IS NULL
            OR lifecycle_status IN (
                'draft','configured','reviewed','parallel_run',
                'live','suspended','deactivated'
            )
        );
END$$;

-- Backfill: existing rows with is_active = true → lifecycle_status = 'live';
-- existing rows with is_active = false → lifecycle_status = 'deactivated'.
UPDATE "ob-poc".cbu_settlement_chains
    SET lifecycle_status = CASE
        WHEN is_active THEN 'live'
        ELSE 'deactivated'
    END
    WHERE lifecycle_status IS NULL;

-- Materialises: M-021 · DAG: cbu_dag.yaml · Substrate audit: S-12
