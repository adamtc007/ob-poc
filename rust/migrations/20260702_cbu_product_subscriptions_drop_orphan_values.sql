-- Phase 7c (state-graph remediation): cbu_product_subscriptions.status --
-- confirmed only 'ACTIVE' is ever written (batch_control.rs, cbu.rs; no
-- YAML verb targets this table via generic CRUD). PENDING/SUSPENDED/
-- TERMINATED have zero writers and zero live rows.

BEGIN;

ALTER TABLE "ob-poc".cbu_product_subscriptions
  DROP CONSTRAINT cbu_product_subscriptions_status_check;

ALTER TABLE "ob-poc".cbu_product_subscriptions
  ADD CONSTRAINT cbu_product_subscriptions_status_check
  CHECK (status = 'ACTIVE');

COMMIT;
