-- Phase 7b (state-graph remediation, ratified 7.6): cbu_trading_activity
-- has zero rows and zero writers (confirmed: no Rust op touches it). Table
-- drop is fenced (rides the W1-proper cutover); COMMENT ON TABLE marks it
-- deprecated instead, matching the trading_activity DAG slot's own note
-- (instrument_matrix_dag.yaml).

BEGIN;

COMMENT ON TABLE "ob-poc".cbu_trading_activity IS
  'DEPRECATED (state-graph remediation Phase 7b, 2026-07-02): zero writers, '
  'zero live rows. The trading_activity DAG slot (instrument_matrix_dag.yaml) '
  'that referenced it as source_entity has been removed. Table retained '
  '(not dropped) pending the W1-proper cutover.';

COMMIT;
