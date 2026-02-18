-- Migration 090: Semantic Registry — Evidence Instance Layer
-- Creates observations, document_instances, provenance_edges, retention_policies tables.
-- Also adds immutability trigger on sem_reg.snapshots (missing from Phase 0).
--
-- IMPORTANT: sem_reg.observations is SEPARATE from any attribute_observations table.
-- Different semantics — these track evidence about entities, not attribute values.

BEGIN;

-- ============================================================
-- 1. Immutability trigger on sem_reg.snapshots
--    Matches the compiled_runbooks pattern from migration 089.
-- ============================================================

CREATE OR REPLACE FUNCTION sem_reg.snapshots_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'sem_reg.snapshots is append-only: UPDATE and DELETE are prohibited (INV-1)';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_snapshots_immutable ON sem_reg.snapshots;

CREATE TRIGGER trg_snapshots_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.snapshots
    FOR EACH ROW EXECUTE FUNCTION sem_reg.snapshots_immutable();

COMMENT ON TRIGGER trg_snapshots_immutable ON sem_reg.snapshots IS
    'Enforces INV-1: no in-place updates on snapshots. All changes produce new snapshots.';

-- ============================================================
-- 2. Observations — evidence about entities
--    INSERT-only with linear chain enforcement via supersedes.
-- ============================================================

CREATE TABLE sem_reg.observations (
    observation_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id         UUID NOT NULL REFERENCES sem_reg.snapshots(snapshot_id),
    supersedes          UUID REFERENCES sem_reg.observations(observation_id),
    observed_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    observer_id         TEXT NOT NULL,
    evidence_grade      TEXT NOT NULL CHECK (evidence_grade IN (
        'primary_document', 'secondary_document', 'self_declaration',
        'third_party_attestation', 'system_derived', 'manual_override'
    )),
    raw_payload         JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Linear chain enforcement: no two observations may supersede the same parent.
-- This ensures supersession forms a linear chain, not a tree.
CREATE UNIQUE INDEX uix_observations_supersedes
    ON sem_reg.observations (supersedes)
    WHERE supersedes IS NOT NULL;

-- Fast lookup by snapshot
CREATE INDEX ix_observations_snapshot
    ON sem_reg.observations (snapshot_id, observed_at DESC);

-- Immutability trigger — observations are INSERT-only
CREATE OR REPLACE FUNCTION sem_reg.observations_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'sem_reg.observations is append-only: UPDATE and DELETE are prohibited';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_observations_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.observations
    FOR EACH ROW EXECUTE FUNCTION sem_reg.observations_immutable();

COMMENT ON TABLE sem_reg.observations IS
    'Evidence observations about entities. INSERT-only, linear supersession chain.';

-- ============================================================
-- 3. Document instances — specific document submissions
-- ============================================================

CREATE TABLE sem_reg.document_instances (
    instance_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_snapshot_id   UUID NOT NULL REFERENCES sem_reg.snapshots(snapshot_id),
    entity_id                   UUID NOT NULL,
    status                      TEXT NOT NULL CHECK (status IN (
        'pending', 'received', 'verified', 'rejected', 'expired'
    )),
    verified_at                 TIMESTAMPTZ,
    expires_at                  TIMESTAMPTZ,
    storage_ref                 TEXT,
    metadata                    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ix_document_instances_entity
    ON sem_reg.document_instances (entity_id, status);

CREATE INDEX ix_document_instances_type
    ON sem_reg.document_instances (document_type_snapshot_id);

COMMENT ON TABLE sem_reg.document_instances IS
    'Concrete document submissions linked to document type definitions in the registry.';

-- ============================================================
-- 4. Provenance edges — lineage between evidence artifacts
--    INSERT-only graph of relationships.
-- ============================================================

CREATE TABLE sem_reg.provenance_edges (
    edge_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_type     TEXT NOT NULL,
    source_id       UUID NOT NULL,
    target_type     TEXT NOT NULL,
    target_id       UUID NOT NULL,
    edge_class      TEXT NOT NULL CHECK (edge_class IN (
        'derived_from', 'verified_by', 'superseded_by',
        'attested_by', 'sourced_from', 'contributed_to'
    )),
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Forward traversal: given a source, find all targets
CREATE INDEX ix_provenance_source
    ON sem_reg.provenance_edges (source_type, source_id);

-- Reverse traversal: given a target, find all sources
CREATE INDEX ix_provenance_target
    ON sem_reg.provenance_edges (target_type, target_id);

-- Immutability trigger — provenance edges are INSERT-only
CREATE OR REPLACE FUNCTION sem_reg.provenance_edges_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'sem_reg.provenance_edges is append-only: UPDATE and DELETE are prohibited';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_provenance_edges_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.provenance_edges
    FOR EACH ROW EXECUTE FUNCTION sem_reg.provenance_edges_immutable();

COMMENT ON TABLE sem_reg.provenance_edges IS
    'INSERT-only provenance graph linking evidence artifacts.';

-- ============================================================
-- 5. Retention policies — document lifecycle rules
-- ============================================================

CREATE TABLE sem_reg.retention_policies (
    policy_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_snapshot_id   UUID NOT NULL REFERENCES sem_reg.snapshots(snapshot_id),
    retention_days              INT NOT NULL CHECK (retention_days > 0),
    archive_action              TEXT NOT NULL CHECK (archive_action IN (
        'delete', 'archive', 'anonymize', 'retain'
    )),
    metadata                    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ix_retention_policies_doctype
    ON sem_reg.retention_policies (document_type_snapshot_id);

COMMENT ON TABLE sem_reg.retention_policies IS
    'Retention lifecycle rules linked to document type definitions.';

COMMIT;
