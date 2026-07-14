-- G11 join fix (EOP-SESSION-CONTROLPLANE-G11-JOIN-FIX-001): DD-4(ii)'s
-- re-derivation join (`control_plane_audit.rs::replay_grade_for_decision`)
-- fetched `gate_results` via `WHERE entry_id = $1 LIMIT 1`, with no
-- `ORDER BY`/tiebreaker. `entry_id` is NOT a per-shadow-eval-attempt
-- unique key -- it is the RunbookEntry/CompiledStep's own stable id
-- (`sequencer.rs::phase5_runtime_recheck`'s `entry_id` parameter), reused
-- across every retry/re-check of the SAME runbook step. A step rejected
-- on attempt 1 (e.g. missing evidence) and approved on attempt 2 (after
-- the gap is fixed) both insert rows into this table sharing one
-- `entry_id` but carrying different `gate_results`. The unordered
-- `LIMIT 1` join could non-deterministically return either attempt's row
-- when grading either attempt's `DecisionEvaluated` audit event,
-- corrupting the G11/AuditReplay metric with false Success or false
-- Failure grades.
--
-- `decision_id` is the real per-attempt-unique value: it already exists
-- as `control_plane_audit`'s own correlating column, minted at the same
-- call site the shadow row is built -- `envelope.id()` for `ApprovedStp`
-- decisions, a fresh `Uuid::new_v4()` otherwise (see the `decision_id`
-- local variable's own doc comment above the `DecisionEvaluated`
-- audit-event construction in `sequencer.rs`). This migration threads
-- that SAME value onto the shadow-decisions row so the two can be
-- correlated by construction, not by coincidence -- the DD-4(ii) join
-- now filters on `decision_id` instead of `entry_id`.
--
-- Nullable, no backfill -- same posture as G1's `entry_id` addition to
-- `control_plane_envelopes` (`20260713_control_plane_envelopes_entry_id.sql`)
-- and G5's `execution_path` addition to this same table
-- (`20260713_control_plane_shadow_decisions_execution_path.sql`):
-- existing rows predate this fix and have no real `decision_id` to
-- backfill (their originating audit event, if any, cannot be
-- retroactively identified from the row alone). DD-4(ii) treats a NULL
-- `decision_id` on either side of the join the same way it already
-- treats a nil `entry_id` today -- inconclusive, graded on completeness
-- alone, never a manufactured failure.
ALTER TABLE "ob-poc".control_plane_shadow_decisions
    ADD COLUMN decision_id UUID;

CREATE INDEX idx_control_plane_shadow_decisions_decision_id
    ON "ob-poc".control_plane_shadow_decisions (decision_id)
    WHERE decision_id IS NOT NULL;

COMMENT ON COLUMN "ob-poc".control_plane_shadow_decisions.decision_id IS
    'G11 join fix: the same per-attempt-unique decision_id control_plane_audit rows for this shadow evaluation are keyed by (envelope.id() for ApprovedStp, a fresh Uuid::new_v4() otherwise) -- the correct DD-4(ii) re-derivation join key. entry_id alone is NOT unique (reused across retries of the same runbook step) and must never be used for that join. NULL for pre-this-migration rows and for shadow rows with no corresponding control_plane_audit DecisionEvaluated event (e.g. Path D/bus_runtime.rs, which does not emit one at all).';
