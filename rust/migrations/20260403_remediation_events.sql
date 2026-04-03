-- Cross-Workspace State Consistency: Phase 6 — Remediation Events
-- Lifecycle entity tracking the resolution of a shared attribute supersession.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §4.7, §6.6

CREATE TABLE IF NOT EXISTS "ob-poc".remediation_events (
    id                              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id                       UUID NOT NULL,
    source_atom_id                  UUID NOT NULL REFERENCES "ob-poc".shared_atom_registry(id),
    source_workspace                TEXT NOT NULL,
    prior_version                   INTEGER NOT NULL,
    new_version                     INTEGER NOT NULL,
    affected_workspace              TEXT NOT NULL,
    affected_constellation_family   TEXT NOT NULL,
    status                          TEXT NOT NULL DEFAULT 'detected',
    failed_at_step                  TEXT,
    failure_reason                  TEXT,
    deferral_reason                 TEXT,
    resolved_at                     TIMESTAMPTZ,
    resolved_by                     UUID,
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT chk_remediation_status
        CHECK (status IN ('detected', 'replaying', 'resolved', 'escalated', 'deferred'))
);

-- Find open remediation events per entity
CREATE INDEX IF NOT EXISTS idx_remediation_events_entity_status
    ON "ob-poc".remediation_events (entity_id, status);

-- Find all open events (for dashboards)
CREATE INDEX IF NOT EXISTS idx_remediation_events_status
    ON "ob-poc".remediation_events (status)
    WHERE status NOT IN ('resolved', 'deferred');

-- FK back-reference from workspace_fact_refs
ALTER TABLE "ob-poc".workspace_fact_refs
    DROP CONSTRAINT IF EXISTS fk_workspace_fact_refs_remediation;

ALTER TABLE "ob-poc".workspace_fact_refs
    ADD CONSTRAINT fk_workspace_fact_refs_remediation
    FOREIGN KEY (remediation_id) REFERENCES "ob-poc".remediation_events(id);

COMMENT ON TABLE "ob-poc".remediation_events IS
    'Lifecycle entity tracking cross-workspace state drift caused by a superseded shared attribute version. FSM: Detected → Replaying → Resolved | Escalated → Resolved | Deferred.';
COMMENT ON COLUMN "ob-poc".remediation_events.status IS
    'detected = staleness found; replaying = constellation replay in progress; resolved = replay complete; escalated = replay failed, human review; deferred = divergence explicitly accepted.';
