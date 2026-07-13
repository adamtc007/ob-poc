-- G1 item 2 (EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001 §2.1): the
-- correlation carrier between Path A's seal site (`phase5_runtime_recheck`)
-- and its consume site (`VerbExecutionPortStepExecutor::execute_step`).
-- `CompiledStep.step_id` already equals the originating `RunbookEntry.id`
-- (`rust/src/runbook/types.rs:139`), so a sealed row tagged with the same
-- id lets the consume site look up "the envelope sealed for THIS step" by
-- `(session_id, entry_id)` instead of the ambiguous `(session_id, verb_fqn)`
-- pair (two entries dispatching the same verb FQN would otherwise be
-- indistinguishable).
--
-- Nullable for backward compatibility with any pre-existing row — the
-- table is shadow-sealing-only in production today (no consumer reads
-- `entry_id` until this diff), so no backfill is required or attempted.
ALTER TABLE "ob-poc".control_plane_envelopes
    ADD COLUMN entry_id UUID;

CREATE INDEX idx_control_plane_envelopes_session_entry_status
    ON "ob-poc".control_plane_envelopes (session_id, entry_id, status);

COMMENT ON COLUMN "ob-poc".control_plane_envelopes.entry_id IS
    'G1 item 2 (EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001 §2.1): the RunbookEntry/CompiledStep.step_id that produced this sealed envelope. NULL for any envelope sealed before this column existed (none in production as of 2026-07-13 — shadow-sealing-only). The consume site looks up the freshest sealed row for (session_id, entry_id).';
