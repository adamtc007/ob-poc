-- Phase 2c (state-graph remediation, RW-1, ratified 1.3): add 'CALCULATING' to
-- fee_billing_periods.calc_status. The billing pipeline transitions PENDING ->
-- CALCULATING while a calculation run is in flight, but the live CHECK
-- constraint never declared it as legal.

BEGIN;

ALTER TABLE "ob-poc".fee_billing_periods DROP CONSTRAINT fee_billing_periods_calc_status_check;

ALTER TABLE "ob-poc".fee_billing_periods
  ADD CONSTRAINT fee_billing_periods_calc_status_check
  CHECK (calc_status IN ('PENDING', 'CALCULATING', 'CALCULATED', 'REVIEWED', 'APPROVED', 'DISPUTED', 'INVOICED'));

COMMIT;
