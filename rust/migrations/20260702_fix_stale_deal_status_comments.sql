-- Phase 1a (state-graph remediation, RW-6): fix stale COMMENTs that no longer
-- match their live CHECK constraints. Comments are documentation only; the
-- CHECK constraints below are unchanged and remain authoritative.

BEGIN;

COMMENT ON COLUMN "ob-poc".deals.deal_status IS
  'PROSPECT | QUALIFYING | NEGOTIATING | IN_CLEARANCE | CONTRACTED | LOST | REJECTED | WITHDRAWN | CANCELLED';

COMMENT ON COLUMN "ob-poc".deal_rate_cards.status IS
  'DRAFT | PENDING_INTERNAL_APPROVAL | APPROVED_INTERNALLY | PROPOSED | COUNTER_PROPOSED | AGREED | SUPERSEDED | CANCELLED';

COMMIT;
