-- Migration 091: Semantic Registry — Evidence Model Fixes
-- Addresses D2 (entity-centric observations), D3 (document_instances lifecycle guard).
--
-- D2: Adds attribute_observations table for entity-centric evidence queries.
--     The existing observations table is retained as a snapshot-centric event log.
--     Option 1b: keep both, let callers choose by use case.
--
-- D3: Adds lifecycle guard trigger on document_instances.

BEGIN;

-- ============================================================
-- 1. D2: attribute_observations — entity-centric evidence
--    Answers: "What evidence exists for entity X, attribute A?"
--    Separate from snapshot-centric observations table.
-- ============================================================

CREATE TABLE sem_reg.attribute_observations (
    observation_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_ref     UUID NOT NULL,
    attribute_fqn   TEXT NOT NULL,
    snapshot_id     UUID REFERENCES sem_reg.snapshots(snapshot_id),
    confidence      REAL NOT NULL DEFAULT 1.0
                    CHECK (confidence >= 0.0 AND confidence <= 1.0),
    observed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    observer_id     TEXT NOT NULL DEFAULT 'system',
    evidence_grade  TEXT NOT NULL DEFAULT 'secondary_document'
                    CHECK (evidence_grade IN (
                        'primary_document', 'secondary_document', 'self_declaration',
                        'third_party_attestation', 'system_derived', 'manual_override'
                    )),
    raw_payload     JSONB,
    supersedes      UUID REFERENCES sem_reg.attribute_observations(observation_id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Primary lookup: "all observations for entity X, attribute A"
CREATE INDEX idx_attr_obs_subject
    ON sem_reg.attribute_observations (subject_ref, attribute_fqn);

-- Freshness lookup: "most recent observation for entity X, attribute A"
CREATE INDEX idx_attr_obs_freshness
    ON sem_reg.attribute_observations (subject_ref, attribute_fqn, observed_at DESC);

-- Linear chain: no two observations supersede the same parent
CREATE UNIQUE INDEX uix_attr_obs_supersedes
    ON sem_reg.attribute_observations (supersedes)
    WHERE supersedes IS NOT NULL;

-- Immutability trigger — attribute_observations are INSERT-only
CREATE OR REPLACE FUNCTION sem_reg.attr_observations_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'sem_reg.attribute_observations is append-only: UPDATE and DELETE are prohibited';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_attr_observations_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.attribute_observations
    FOR EACH ROW EXECUTE FUNCTION sem_reg.attr_observations_immutable();

COMMENT ON TABLE sem_reg.attribute_observations IS
    'Entity-centric evidence observations. Answers "what evidence for entity X, attribute A?" INSERT-only.';

-- ============================================================
-- 2. D3: document_instances lifecycle guard
--    Allows status updates, prevents immutable field changes and DELETE.
-- ============================================================

CREATE OR REPLACE FUNCTION sem_reg.guard_document_instance_mutation()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'DELETE not allowed on document_instances';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF OLD.instance_id != NEW.instance_id THEN
            RAISE EXCEPTION 'Cannot change immutable field instance_id on document_instances';
        END IF;
        IF OLD.document_type_snapshot_id != NEW.document_type_snapshot_id THEN
            RAISE EXCEPTION 'Cannot change immutable field document_type_snapshot_id on document_instances';
        END IF;
        IF OLD.entity_id != NEW.entity_id THEN
            RAISE EXCEPTION 'Cannot change immutable field entity_id on document_instances';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_document_instance_guard
    BEFORE UPDATE OR DELETE ON sem_reg.document_instances
    FOR EACH ROW EXECUTE FUNCTION sem_reg.guard_document_instance_mutation();

COMMENT ON TRIGGER trg_document_instance_guard ON sem_reg.document_instances IS
    'D3: Lifecycle guard — allows status updates, prevents immutable field changes and DELETE.';

COMMIT;
