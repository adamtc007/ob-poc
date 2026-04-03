-- Cross-Workspace State Consistency: Phase 2 — Shared Fact Versions
-- Versioned fact store for shared atoms. One row per entity × atom × version.
-- This is the AUTHORITATIVE source of truth (INV-1, INV-2).
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.2

CREATE TABLE IF NOT EXISTS "ob-poc".shared_fact_versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    atom_id         UUID NOT NULL REFERENCES "ob-poc".shared_atom_registry(id),
    entity_id       UUID NOT NULL,
    version         INTEGER NOT NULL DEFAULT 1,
    value           JSONB NOT NULL,
    mutated_by_verb TEXT,
    mutated_by_user UUID,
    mutated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    is_current      BOOLEAN NOT NULL DEFAULT true,

    CONSTRAINT uq_shared_fact_version
        UNIQUE (atom_id, entity_id, version)
);

-- Fast current-version lookup (only one row per atom × entity is current)
CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_fact_versions_current
    ON "ob-poc".shared_fact_versions (atom_id, entity_id)
    WHERE is_current = true;

-- Entity-scoped queries (e.g., "all shared facts for entity X")
CREATE INDEX IF NOT EXISTS idx_shared_fact_versions_entity
    ON "ob-poc".shared_fact_versions (entity_id);

-- Auto-supersede: when a new version is inserted, clear is_current on prior versions.
-- This trigger ensures exactly one current version per (atom_id, entity_id).
CREATE OR REPLACE FUNCTION "ob-poc".trg_shared_fact_version_supersede()
RETURNS TRIGGER AS $$
BEGIN
    -- Clear is_current on all prior versions for the same atom × entity
    UPDATE "ob-poc".shared_fact_versions
    SET is_current = false
    WHERE atom_id = NEW.atom_id
      AND entity_id = NEW.entity_id
      AND id != NEW.id
      AND is_current = true;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_shared_fact_version_supersede
    ON "ob-poc".shared_fact_versions;

CREATE TRIGGER trg_shared_fact_version_supersede
    AFTER INSERT ON "ob-poc".shared_fact_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".trg_shared_fact_version_supersede();

COMMENT ON TABLE "ob-poc".shared_fact_versions IS
    'Versioned fact store for shared atoms. One row per entity × atom × version. The is_current flag denotes the latest version; superseded versions are retained for audit.';
COMMENT ON COLUMN "ob-poc".shared_fact_versions.is_current IS
    'Denormalized flag: true for the latest version only. Maintained by trigger.';
COMMENT ON COLUMN "ob-poc".shared_fact_versions.version IS
    'Monotonically increasing version number per (atom_id, entity_id) pair.';
