-- v1.2 Tranche 3 Phase 3.B — Catalogue workspace carrier tables (2026-04-27).
--
-- Adds the `catalogue_proposals` and supporting tables for the Catalogue
-- workspace per docs/governance/tranche-3-design-2026-04-26.md.
--
-- The state machine: DRAFT → STAGED → COMMITTED | ROLLED_BACK | REJECTED.
-- Per Phase 3.F, COMMITTED is the architectural drift gate; once Stage 4
-- forward-discipline activates, the YAML catalogue is loaded exclusively
-- from `catalogue_committed_verbs`.
--
-- Two-eye rule: `committed_by` MUST be different from `proposed_by`. The
-- ABAC layer enforces this at the verb-dispatch boundary; the schema
-- documents the invariant in a CHECK that compares principal IDs at insert
-- time on the COMMITTED-row write.

-- Catalogue proposals — primary entity for the workspace.
CREATE TABLE IF NOT EXISTS "ob-poc".catalogue_proposals (
    proposal_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The verb FQN being authored / amended (e.g. "deal.cancel").
    verb_fqn                 TEXT NOT NULL,

    -- Full proposed verb declaration as JSON. The `state_effect`,
    -- `external_effects`, `consequence`, `transition_args`, etc. are all
    -- inside this blob; the validator runs over it on stage transition.
    proposed_declaration     JSONB NOT NULL,

    -- Optional rationale supplied at propose-time. Surfaced in audit trail.
    rationale                TEXT,

    -- State machine.
    status                   TEXT NOT NULL DEFAULT 'DRAFT'
                             CHECK (status IN ('DRAFT', 'STAGED', 'COMMITTED', 'ROLLED_BACK', 'REJECTED')),

    -- Audit trail principals.
    proposed_by              TEXT NOT NULL,
    staged_at                TIMESTAMPTZ,
    committed_by             TEXT,
    committed_at             TIMESTAMPTZ,
    rolled_back_by           TEXT,
    rolled_back_at           TIMESTAMPTZ,
    rolled_back_reason       TEXT,
    rejected_by              TEXT,
    rejected_at              TIMESTAMPTZ,
    rejected_reason          TEXT,

    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Two-eye rule: when COMMITTED, the principal must differ from proposer.
    CONSTRAINT catalogue_two_eye_rule CHECK (
        status != 'COMMITTED' OR (committed_by IS NOT NULL AND committed_by != proposed_by)
    )
);

-- Index for proposal lookups by status + verb_fqn.
CREATE INDEX IF NOT EXISTS catalogue_proposals_status_idx
    ON "ob-poc".catalogue_proposals (status, verb_fqn);

CREATE INDEX IF NOT EXISTS catalogue_proposals_verb_fqn_idx
    ON "ob-poc".catalogue_proposals (verb_fqn);

CREATE INDEX IF NOT EXISTS catalogue_proposals_proposed_by_idx
    ON "ob-poc".catalogue_proposals (proposed_by);


-- Validator-output snapshot for each proposal at stage time. Records the
-- validator's structural / well-formedness / warnings counts so the
-- audit trail captures *why* a proposal was clean (or not) at stage.
CREATE TABLE IF NOT EXISTS "ob-poc".catalogue_proposal_validator_runs (
    run_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id              UUID NOT NULL REFERENCES "ob-poc".catalogue_proposals(proposal_id) ON DELETE CASCADE,
    structural_errors        INTEGER NOT NULL DEFAULT 0,
    well_formedness_errors   INTEGER NOT NULL DEFAULT 0,
    policy_warnings          INTEGER NOT NULL DEFAULT 0,
    error_detail             JSONB,                          -- list of human-readable errors
    ran_at                   TIMESTAMPTZ NOT NULL DEFAULT now(),
    is_clean                 BOOLEAN NOT NULL
);

CREATE INDEX IF NOT EXISTS catalogue_proposal_validator_runs_proposal_idx
    ON "ob-poc".catalogue_proposal_validator_runs (proposal_id, ran_at DESC);


-- Committed verb declarations. Once Phase 3.F Stage 4 activates, this
-- table is the AUTHORITATIVE catalogue — YAML loading is removed and the
-- runtime hydrates from this table.
--
-- Until Stage 4, this table is a *projection* of the committed proposals;
-- YAML remains the source of truth and the table is reconciled at boot.
CREATE TABLE IF NOT EXISTS "ob-poc".catalogue_committed_verbs (
    verb_fqn                 TEXT PRIMARY KEY,
    declaration              JSONB NOT NULL,                 -- full verb declaration
    committed_proposal_id    UUID NOT NULL REFERENCES "ob-poc".catalogue_proposals(proposal_id),
    committed_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS catalogue_committed_verbs_committed_at_idx
    ON "ob-poc".catalogue_committed_verbs (committed_at DESC);


-- Updated-at trigger on catalogue_proposals.
CREATE OR REPLACE FUNCTION "ob-poc".catalogue_proposals_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS catalogue_proposals_updated_at_trg
    ON "ob-poc".catalogue_proposals;
CREATE TRIGGER catalogue_proposals_updated_at_trg
    BEFORE UPDATE ON "ob-poc".catalogue_proposals
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".catalogue_proposals_set_updated_at();
