-- Cross-Workspace State Consistency: Phase 1 — Shared Atom Registry
-- Declares shared atoms and their ownership/lifecycle state.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.1

CREATE TABLE IF NOT EXISTS "ob-poc".shared_atom_registry (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    atom_path       TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    owner_workspace TEXT NOT NULL,
    owner_constellation_family TEXT NOT NULL,
    lifecycle_status TEXT NOT NULL DEFAULT 'draft',
    validation_rule JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    activated_at    TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT chk_lifecycle_status
        CHECK (lifecycle_status IN ('draft', 'active', 'deprecated', 'retired'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_atom_registry_path
    ON "ob-poc".shared_atom_registry (atom_path);

CREATE INDEX IF NOT EXISTS idx_shared_atom_registry_active
    ON "ob-poc".shared_atom_registry (lifecycle_status)
    WHERE lifecycle_status = 'active';

COMMENT ON TABLE "ob-poc".shared_atom_registry IS
    'Declares shared atoms whose values are owned by one workspace but consumed by others. Governed SemOS entity with lifecycle FSM (Draft → Active → Deprecated → Retired).';
COMMENT ON COLUMN "ob-poc".shared_atom_registry.atom_path IS
    'Dot-notation attribute path, e.g. entity.lei, entity.jurisdiction';
COMMENT ON COLUMN "ob-poc".shared_atom_registry.lifecycle_status IS
    'Draft = declared but not enforced; Active = full propagation; Deprecated = enforced but no new consumers; Retired = historical only';
COMMENT ON COLUMN "ob-poc".shared_atom_registry.validation_rule IS
    'Optional validation: {format, allowed_values, gleif_verification, ...}. Always read/written through typed SharedAtomValidation struct.';
