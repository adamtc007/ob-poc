-- G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3, EOP-DESIGN-CONTROLPLANE-
-- G5-GATE-APPLICABILITY-MATRIX-001): extends shadow-decision rows with the
-- ExecutionPath they were evaluated under, so the E3 probe can be
-- amended to a per-(gate, path) breakdown instead of the path-blind
-- per-gate-only view T2.7/T7.2 originally built (Path A was the only
-- producer of these rows before G5).
--
-- Single-letter codes match ob_poc_types::ExecutionPath::as_letter():
-- 'A' RunbookSequencer, 'B' DslDirect, 'C' WorkflowDispatched,
-- 'D' BusFederated. DEFAULT 'A' is correct as a backfill value for every
-- pre-G5 row, not a guess: every row inserted before this migration came
-- from exactly one call site — sequencer.rs's phase5_runtime_recheck,
-- Path A's own shadow-recheck (T2.7/T9.x). No other call site existed
-- until this tranche.
ALTER TABLE "ob-poc".control_plane_shadow_decisions
    ADD COLUMN execution_path TEXT NOT NULL DEFAULT 'A';

ALTER TABLE "ob-poc".control_plane_shadow_decisions
    ADD CONSTRAINT control_plane_shadow_decisions_execution_path_valid
    CHECK (execution_path IN ('A', 'B', 'C', 'D'));

CREATE INDEX idx_control_plane_shadow_decisions_execution_path
    ON "ob-poc".control_plane_shadow_decisions (execution_path, decided_at DESC);

COMMENT ON COLUMN "ob-poc".control_plane_shadow_decisions.execution_path IS
    'G5: which of the 4 RR-2 ExecutionPath variants this shadow decision was evaluated under (A=RunbookSequencer, B=DslDirect, C=WorkflowDispatched, D=BusFederated). Default A is a correct historical backfill, not a guess -- every pre-G5 row came from Path A''s sole shadow call site.';
