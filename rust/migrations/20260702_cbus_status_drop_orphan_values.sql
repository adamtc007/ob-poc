-- Phase 7a (state-graph remediation, ratified 7.4): remove SUSPENDED and
-- ARCHIVED from cbus.status. Confirmed zero writers (grep: no verb, no op
-- writes cbus.status to either value) and zero live rows in either state.
-- Suspension/archival already live on cbus.operational_status
-- (suspended/archived, via cbu.suspend / the backend archival scheduler);
-- these were duplicate vocabulary on the wrong (validation-lifecycle)
-- column.

BEGIN;

ALTER TABLE "ob-poc".cbus DROP CONSTRAINT chk_cbu_status;

ALTER TABLE "ob-poc".cbus
  ADD CONSTRAINT chk_cbu_status
  CHECK (status IN (
    'DISCOVERED', 'VALIDATION_PENDING', 'VALIDATED', 'UPDATE_PENDING_PROOF', 'VALIDATION_FAILED'
  ));

COMMIT;
