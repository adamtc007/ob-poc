-- Migration 123: publish document governance objects and add runtime policy bindings.
--
-- Extends sem_reg_pub with published read models for:
--   - requirement_profile_def
--   - proof_obligation_def
--   - evidence_strategy_def
--
-- Adds a dedicated runtime binding table so document/runtime rows can stamp the
-- SemOS snapshot set and policy object snapshots they were computed against.

CREATE TABLE IF NOT EXISTS sem_reg_pub.active_requirement_profiles (
    snapshot_set_id  UUID NOT NULL,
    snapshot_id      UUID NOT NULL,
    fqn              TEXT NOT NULL,
    payload          JSONB NOT NULL,
    published_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (snapshot_set_id, fqn)
);

CREATE TABLE IF NOT EXISTS sem_reg_pub.active_proof_obligations (
    snapshot_set_id  UUID NOT NULL,
    snapshot_id      UUID NOT NULL,
    fqn              TEXT NOT NULL,
    payload          JSONB NOT NULL,
    published_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (snapshot_set_id, fqn)
);

CREATE TABLE IF NOT EXISTS sem_reg_pub.active_evidence_strategies (
    snapshot_set_id  UUID NOT NULL,
    snapshot_id      UUID NOT NULL,
    fqn              TEXT NOT NULL,
    payload          JSONB NOT NULL,
    published_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (snapshot_set_id, fqn)
);

CREATE TABLE IF NOT EXISTS "ob-poc".policy_version_bindings (
    binding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    subject_kind TEXT NOT NULL,
    subject_id UUID NOT NULL,

    semos_snapshot_set_id UUID NOT NULL REFERENCES sem_reg.snapshot_sets(snapshot_set_id),

    requirement_profile_fqn TEXT,
    requirement_profile_snapshot_id UUID REFERENCES sem_reg.snapshots(snapshot_id),

    verification_rule_fqn TEXT,
    verification_rule_snapshot_id UUID REFERENCES sem_reg.snapshots(snapshot_id),

    acceptance_policy_fqn TEXT,
    acceptance_policy_snapshot_id UUID REFERENCES sem_reg.snapshots(snapshot_id),

    document_type_registry_version TEXT,
    extraction_model_version TEXT,
    policy_effective_at TIMESTAMPTZ,

    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    computed_by TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_policy_version_bindings_subject
    ON "ob-poc".policy_version_bindings(subject_kind, subject_id, computed_at DESC);

CREATE INDEX IF NOT EXISTS idx_policy_version_bindings_snapshot_set
    ON "ob-poc".policy_version_bindings(semos_snapshot_set_id, computed_at DESC);

CREATE INDEX IF NOT EXISTS idx_policy_version_bindings_requirement_profile
    ON "ob-poc".policy_version_bindings(requirement_profile_snapshot_id)
    WHERE requirement_profile_snapshot_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".policy_version_bindings IS
'Dedicated runtime policy binding table for document polymorphism. Stores the published SemOS snapshot set and resolved policy object snapshots used when computing runtime document decisions.';
