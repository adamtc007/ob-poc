-- Cross-Workspace State Consistency: Phase 3 — Workspace Fact Refs
-- Consumption-state projection (INV-2). Tracks which version of a shared
-- fact each consuming workspace last acknowledged or built against.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.3

CREATE TABLE IF NOT EXISTS "ob-poc".workspace_fact_refs (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    atom_id             UUID NOT NULL REFERENCES "ob-poc".shared_atom_registry(id),
    entity_id           UUID NOT NULL,
    consumer_workspace  TEXT NOT NULL,
    held_version        INTEGER NOT NULL,
    status              TEXT NOT NULL DEFAULT 'current',
    stale_since         TIMESTAMPTZ,
    remediation_id      UUID,

    CONSTRAINT chk_workspace_fact_ref_status
        CHECK (status IN ('current', 'stale', 'deferred'))
);

-- One ref per atom × entity × workspace
CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_fact_refs_unique
    ON "ob-poc".workspace_fact_refs (atom_id, entity_id, consumer_workspace);

-- Fast stale-ref queries (for pre-REPL check and narration)
CREATE INDEX IF NOT EXISTS idx_workspace_fact_refs_stale
    ON "ob-poc".workspace_fact_refs (consumer_workspace, status)
    WHERE status = 'stale';

-- Entity-scoped queries
CREATE INDEX IF NOT EXISTS idx_workspace_fact_refs_entity
    ON "ob-poc".workspace_fact_refs (entity_id, status);

COMMENT ON TABLE "ob-poc".workspace_fact_refs IS
    'Consumption-state projection — tracks which shared fact version each consuming workspace last acknowledged. A stale row means the consumer is operating against a superseded attribute version.';
COMMENT ON COLUMN "ob-poc".workspace_fact_refs.held_version IS
    'The version number from shared_fact_versions that this workspace last operated against.';
COMMENT ON COLUMN "ob-poc".workspace_fact_refs.status IS
    'current = up to date; stale = superseded attribute exists; deferred = divergence explicitly accepted.';
