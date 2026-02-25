-- Migration 097: Stewardship Phase 0 tables
-- Source spec: docs/stewardship-agent-architecture-v1.0.1.md §8, §9.1-9.7
-- Constraint: no changes to sem_reg.snapshots or other kernel tables
-- (only the partial UNIQUE index on snapshots, which is additive)

-- 0. Schema: stewardship tables in their own schema for boundary visibility
CREATE SCHEMA IF NOT EXISTS stewardship;

-- 1. ALTER changeset_entries: add missing columns per spec §9.1
ALTER TABLE sem_reg.changeset_entries
  ADD COLUMN IF NOT EXISTS action VARCHAR(20) NOT NULL DEFAULT 'add'
    CHECK (action IN ('add','modify','promote','deprecate','alias')),
  ADD COLUMN IF NOT EXISTS predecessor_id UUID REFERENCES sem_reg.snapshots(snapshot_id),
  ADD COLUMN IF NOT EXISTS revision INT NOT NULL DEFAULT 1,
  ADD COLUMN IF NOT EXISTS reasoning TEXT,
  ADD COLUMN IF NOT EXISTS guardrail_log JSONB NOT NULL DEFAULT '[]';

-- 2. Draft uniqueness invariant (spec §9.1)
--    object_id (not fqn) — kernel has no fqn column; fqn is derived from object_id
CREATE UNIQUE INDEX IF NOT EXISTS idx_draft_uniqueness
  ON sem_reg.snapshots (snapshot_set_id, object_type, object_id)
  WHERE status = 'draft' AND effective_until IS NULL;

-- 3. Update changeset status to spec naming (§9.1)
--    Migrate existing 'in_review' rows then drop old value
UPDATE sem_reg.changesets SET status = 'under_review' WHERE status = 'in_review';
ALTER TABLE sem_reg.changesets
  DROP CONSTRAINT IF EXISTS changesets_status_check;
ALTER TABLE sem_reg.changesets
  ADD CONSTRAINT changesets_status_check
    CHECK (status IN ('draft','under_review','approved','published','rejected'));

-- 4. Stewardship event log — immutable, append-only (spec §9.4)
CREATE TABLE IF NOT EXISTS stewardship.events (
  event_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id   UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  event_type     VARCHAR(60) NOT NULL
    CHECK (event_type IN (
      'changeset_created',
      'item_added', 'item_removed', 'item_refined',
      'basis_attached',
      'guardrail_fired',
      'gate_prechecked',
      'submitted_for_review',
      'review_note_added',
      'review_decision_recorded',
      'focus_changed',
      'published',
      'rejected'
    )),
  actor_id       VARCHAR(200) NOT NULL,
  payload        JSONB NOT NULL DEFAULT '{}',
  viewport_manifest_id UUID,  -- optional FK to viewport_manifests for audit
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_stew_events_changeset
  ON stewardship.events (changeset_id, created_at);

-- 5. Basis records (spec §9.3)
CREATE TABLE IF NOT EXISTS stewardship.basis_records (
  basis_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id   UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  entry_id       UUID REFERENCES sem_reg.changeset_entries(entry_id),
  kind           VARCHAR(40) NOT NULL
    CHECK (kind IN ('regulatory_fact','market_practice','platform_convention',
                    'client_requirement','precedent')),
  title          TEXT NOT NULL,
  narrative      TEXT,
  created_by     VARCHAR(200) NOT NULL,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_basis_changeset
  ON stewardship.basis_records (changeset_id);

-- 6. Basis claims (spec §9.3)
CREATE TABLE IF NOT EXISTS stewardship.basis_claims (
  claim_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  basis_id       UUID NOT NULL REFERENCES stewardship.basis_records(basis_id),
  claim_text     TEXT NOT NULL,
  reference_uri  TEXT,
  excerpt        TEXT,
  confidence     DOUBLE PRECISION CHECK (confidence BETWEEN 0.0 AND 1.0),
  flagged_as_open_question BOOLEAN NOT NULL DEFAULT false,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_claims_basis
  ON stewardship.basis_claims (basis_id);

-- 7. Conflict records (spec §9.6)
CREATE TABLE IF NOT EXISTS stewardship.conflict_records (
  conflict_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id            UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  competing_changeset_id  UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  fqn                     VARCHAR(300) NOT NULL,
  detected_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
  resolution_strategy     VARCHAR(20)
    CHECK (resolution_strategy IN ('merge','rebase','supersede')),
  resolution_rationale    TEXT,
  resolved_by             VARCHAR(200),
  resolved_at             TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_conflicts_changeset
  ON stewardship.conflict_records (changeset_id);

-- 8. Templates (spec §9.5) — versioned stewardship objects
CREATE TABLE IF NOT EXISTS stewardship.templates (
  template_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  fqn            VARCHAR(300) NOT NULL,
  display_name   VARCHAR(200) NOT NULL,
  version_major  INT NOT NULL DEFAULT 1,
  version_minor  INT NOT NULL DEFAULT 0,
  version_patch  INT NOT NULL DEFAULT 0,
  domain         VARCHAR(100) NOT NULL,
  scope          JSONB NOT NULL DEFAULT '[]',    -- Vec<EntityType>
  items          JSONB NOT NULL DEFAULT '[]',    -- Vec<TemplateItem>
  steward        VARCHAR(200) NOT NULL,
  basis_ref      UUID,
  status         VARCHAR(20) NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft','active','deprecated')),
  created_by     VARCHAR(200) NOT NULL,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Uniqueness: one active version per FQN
CREATE UNIQUE INDEX IF NOT EXISTS idx_template_fqn_active
  ON stewardship.templates (fqn)
  WHERE status = 'active';

-- 9. Verb implementation bindings (spec §9.7)
CREATE TABLE IF NOT EXISTS stewardship.verb_implementation_bindings (
  binding_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  verb_fqn       VARCHAR(300) NOT NULL,
  binding_kind   VARCHAR(40) NOT NULL
    CHECK (binding_kind IN ('rust_handler','bpmn_process','remote_http','macro_expansion')),
  binding_ref    TEXT NOT NULL,
  exec_modes     JSONB NOT NULL DEFAULT '[]',    -- Vec<ExecMode>
  status         VARCHAR(20) NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft','active','deprecated')),
  last_verified_at TIMESTAMPTZ,
  notes          TEXT,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_verb_binding_active
  ON stewardship.verb_implementation_bindings (verb_fqn)
  WHERE status = 'active';

-- 10. Idempotency tracking for mutating tools (spec §6.2)
CREATE TABLE IF NOT EXISTS stewardship.idempotency_keys (
  client_request_id UUID PRIMARY KEY,
  tool_name         VARCHAR(100) NOT NULL,
  result            JSONB NOT NULL,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Auto-expire old keys (optional: pg_cron or application-level)
CREATE INDEX IF NOT EXISTS idx_idempotency_created
  ON stewardship.idempotency_keys (created_at);
