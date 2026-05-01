-- 20260429_carrier_06_deal_slas_status.sql
-- Phase 2.6 of refactor-todo-2026-04-29.md (D-010).
-- Adds deal_slas.sla_status carrier for M-052 (deal SLA lifecycle,
-- 6 states). Live row count = 0; nullable for backward compat.

ALTER TABLE "ob-poc".deal_slas
    ADD COLUMN IF NOT EXISTS sla_status text;

DO $$
BEGIN
    ALTER TABLE "ob-poc".deal_slas
        DROP CONSTRAINT IF EXISTS deal_slas_sla_status_check;
    ALTER TABLE "ob-poc".deal_slas
        ADD CONSTRAINT deal_slas_sla_status_check CHECK (
            sla_status IS NULL
            OR sla_status IN (
                'NEGOTIATED','ACTIVE','BREACHED','IN_REMEDIATION','RESOLVED','WAIVED'
            )
        );
END$$;

-- Materialises: M-052 · DAG: deal_dag.yaml · Substrate audit: S-7
