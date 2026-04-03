-- Cross-Workspace State Consistency: Phase 4 — Staleness Propagation
-- Three-stage post-commit propagation from superseded shared attribute versions
-- through consumer references.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §4.2, §5.1
--
-- Extends the existing derived attribute staleness pattern:
--   propagate_derived_chain_staleness() → propagate_shared_fact_staleness()

-- Stage 2: Mark consumer references stale when a new fact version is inserted.
-- Stage 1 (attribute superseded) is handled by trg_shared_fact_version_supersede
-- in the shared_fact_versions migration.
-- Stage 3 (remediation event creation) is handled in Rust application code.

CREATE OR REPLACE FUNCTION "ob-poc".propagate_shared_fact_staleness()
RETURNS TRIGGER AS $$
DECLARE
    v_atom_lifecycle TEXT;
    v_stale_count INTEGER;
BEGIN
    -- Only propagate for Active or Deprecated atoms
    SELECT lifecycle_status INTO v_atom_lifecycle
    FROM "ob-poc".shared_atom_registry
    WHERE id = NEW.atom_id;

    IF v_atom_lifecycle IS NULL OR v_atom_lifecycle NOT IN ('active', 'deprecated') THEN
        RETURN NEW;
    END IF;

    -- Mark all consumer refs stale where held_version < new version
    UPDATE "ob-poc".workspace_fact_refs
    SET status = 'stale',
        stale_since = COALESCE(stale_since, now())
    WHERE atom_id = NEW.atom_id
      AND entity_id = NEW.entity_id
      AND status = 'current'
      AND held_version < NEW.version;

    GET DIAGNOSTICS v_stale_count = ROW_COUNT;

    -- Log propagation (useful for debugging, low overhead at O(50) atoms)
    IF v_stale_count > 0 THEN
        RAISE NOTICE 'Shared fact staleness propagated: atom_id=%, entity_id=%, new_version=%, stale_consumers=%',
            NEW.atom_id, NEW.entity_id, NEW.version, v_stale_count;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger: fires AFTER INSERT on shared_fact_versions (after the supersede trigger)
DROP TRIGGER IF EXISTS trg_propagate_shared_fact_staleness
    ON "ob-poc".shared_fact_versions;

CREATE TRIGGER trg_propagate_shared_fact_staleness
    AFTER INSERT ON "ob-poc".shared_fact_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".propagate_shared_fact_staleness();

COMMENT ON FUNCTION "ob-poc".propagate_shared_fact_staleness() IS
    'Stage 2 of three-stage staleness propagation: marks workspace_fact_refs as stale when a new shared fact version supersedes a prior version. Only fires for Active/Deprecated atoms.';
