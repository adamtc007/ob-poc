-- Phase 3c (state-graph remediation, RW-2): deal_slas is missing
-- updated_at, but SimpleStatusOp (src/domain_ops/simple_status_op.rs) has
-- always unconditionally written `updated_at = $2` for every table it
-- manages, including the SLA trio (deal.start-sla-remediation,
-- deal.resolve-sla-breach, deal.waive-sla-breach). Every one of those three
-- verbs has been failing on every invocation with "column updated_at does
-- not exist" -- discovered while verifying Phase 3c's new from-state
-- enforcement end-to-end (the GREEN control case never succeeded).

BEGIN;

ALTER TABLE "ob-poc".deal_slas
  ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

COMMIT;
