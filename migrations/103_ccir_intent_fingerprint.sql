-- Migration 103: CCIR intent fingerprint + pruned verb count
-- Adds ContextEnvelope fingerprint and pruned verb count to intent telemetry.
-- Part of Phase 2C: ContextEnvelope replaces SemRegVerbPolicy.

ALTER TABLE agent.intent_events
  ADD COLUMN IF NOT EXISTS allowed_verbs_fingerprint VARCHAR(70),
  ADD COLUMN IF NOT EXISTS pruned_verbs_count INTEGER,
  ADD COLUMN IF NOT EXISTS toctou_recheck_performed BOOLEAN DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS toctou_result VARCHAR(30),
  ADD COLUMN IF NOT EXISTS toctou_new_fingerprint VARCHAR(70);

COMMENT ON COLUMN agent.intent_events.allowed_verbs_fingerprint IS
  'SHA-256 fingerprint of sorted allowed verb FQN set from ContextEnvelope (format: v1:<hex>)';
COMMENT ON COLUMN agent.intent_events.pruned_verbs_count IS
  'Number of verbs pruned by SemReg context resolution';
COMMENT ON COLUMN agent.intent_events.toctou_recheck_performed IS
  'Whether a TOCTOU recheck was performed before DSL execution';
COMMENT ON COLUMN agent.intent_events.toctou_result IS
  'TOCTOU recheck result: still_allowed, allowed_but_drifted, denied, or NULL';
COMMENT ON COLUMN agent.intent_events.toctou_new_fingerprint IS
  'New fingerprint from TOCTOU recheck (populated on drift or denial)';
