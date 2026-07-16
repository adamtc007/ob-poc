-- T2.7 (EOP-PLAN-CONTROLPLANE-001): shadow-mode control-plane decisions,
-- recorded beside sem_reg.decision_records per the plan's design note
-- (C-044). Shadow mode never gates dispatch — this table exists purely so
-- a divergence between the control plane's shadow decision and the legacy
-- Phase 5 recheck outcome is visible and triageable per gate, ahead of any
-- gate individually graduating to enforce mode.
CREATE TABLE "ob-poc".control_plane_shadow_decisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    decided_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    session_id UUID NOT NULL,
    entry_id UUID NOT NULL,
    verb_fqn TEXT NOT NULL,
    -- One row per GateId in gate::GateId::ALL, e.g. "success" | "failure" |
    -- "not_evaluated" | "not_implemented", keyed by gate name.
    gate_results JSONB NOT NULL,
    -- Whether the control-plane shadow evaluation agreed with the legacy
    -- Phase 5 recheck's block/allow outcome for this entry.
    legacy_outcome_blocked BOOLEAN NOT NULL,
    shadow_intent_admission_blocked BOOLEAN NOT NULL,
    diverged BOOLEAN NOT NULL
);

CREATE INDEX idx_control_plane_shadow_decisions_session
    ON "ob-poc".control_plane_shadow_decisions (session_id, decided_at DESC);

CREATE INDEX idx_control_plane_shadow_decisions_diverged
    ON "ob-poc".control_plane_shadow_decisions (diverged, decided_at DESC)
    WHERE diverged;

COMMENT ON TABLE "ob-poc".control_plane_shadow_decisions IS
    'T2.7 shadow-mode ob-poc-control-plane decisions vs legacy Phase 5 recheck outcome (EOP-PLAN-CONTROLPLANE-001). Never gates dispatch; divergence triage input for per-gate enforce-mode graduation.';
