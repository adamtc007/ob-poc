-- Migration 099: Research → Governed Change Boundary (v0.4 spec §6.1)
-- Creates sem_reg_authoring schema and extends sem_reg.changesets for authoring pipeline.

-- 0. Schema
CREATE SCHEMA IF NOT EXISTS sem_reg_authoring;

-- 1. Extend sem_reg.changesets with authoring columns
ALTER TABLE sem_reg.changesets
  ADD COLUMN IF NOT EXISTS content_hash TEXT,
  ADD COLUMN IF NOT EXISTS hash_version TEXT DEFAULT 'v1',
  ADD COLUMN IF NOT EXISTS title TEXT,
  ADD COLUMN IF NOT EXISTS rationale TEXT,
  ADD COLUMN IF NOT EXISTS supersedes_change_set_id UUID
    REFERENCES sem_reg.changesets(changeset_id),
  ADD COLUMN IF NOT EXISTS superseded_by UUID
    REFERENCES sem_reg.changesets(changeset_id),
  ADD COLUMN IF NOT EXISTS superseded_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS depends_on UUID[],
  ADD COLUMN IF NOT EXISTS evaluated_against_snapshot_set_id UUID;

-- 2. Widen status CHECK to include authoring statuses
ALTER TABLE sem_reg.changesets
  DROP CONSTRAINT IF EXISTS changesets_status_check;
ALTER TABLE sem_reg.changesets
  ADD CONSTRAINT changesets_status_check
  CHECK (status IN (
    'draft','under_review','approved','published','rejected',
    'validated','dry_run_passed','dry_run_failed','superseded'
  ));

-- 3. Idempotent propose: UNIQUE on (hash_version, content_hash) excluding terminal states
CREATE UNIQUE INDEX IF NOT EXISTS uq_changeset_content_hash
  ON sem_reg.changesets(hash_version, content_hash)
  WHERE content_hash IS NOT NULL
    AND status NOT IN ('rejected','superseded');

-- 4. Extend changeset_entries for artifact metadata
ALTER TABLE sem_reg.changeset_entries
  ADD COLUMN IF NOT EXISTS content_hash TEXT,
  ADD COLUMN IF NOT EXISTS path TEXT,
  ADD COLUMN IF NOT EXISTS artifact_type TEXT,
  ADD COLUMN IF NOT EXISTS ordinal INT DEFAULT 0,
  ADD COLUMN IF NOT EXISTS entry_metadata JSONB;

-- 5. Change set artifacts (standalone table for bundle content)
CREATE TABLE IF NOT EXISTS sem_reg_authoring.change_set_artifacts (
  artifact_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  change_set_id  UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  artifact_type  TEXT NOT NULL
    CHECK (artifact_type IN (
      'migration_sql','migration_down_sql','verb_yaml',
      'attribute_json','taxonomy_json','doc_json'
    )),
  ordinal        INT NOT NULL DEFAULT 0,
  path           TEXT,
  content        TEXT NOT NULL,
  content_hash   TEXT NOT NULL,
  metadata       JSONB,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_artifacts_changeset
  ON sem_reg_authoring.change_set_artifacts(change_set_id, ordinal);

-- 6. Validation reports (append-only)
CREATE TABLE IF NOT EXISTS sem_reg_authoring.validation_reports (
  report_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  change_set_id  UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  stage          TEXT NOT NULL CHECK (stage IN ('validate','dry_run')),
  ran_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  ok             BOOLEAN NOT NULL,
  report         JSONB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_val_reports_cs
  ON sem_reg_authoring.validation_reports(change_set_id, stage, ran_at DESC);

-- 7. Governance audit log (permanent, append-only)
CREATE TABLE IF NOT EXISTS sem_reg_authoring.governance_audit_log (
  entry_id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  ts                      TIMESTAMPTZ NOT NULL DEFAULT now(),
  verb                    TEXT NOT NULL,
  agent_session_id        UUID,
  agent_mode              TEXT,
  change_set_id           UUID,
  snapshot_set_id         UUID,
  active_snapshot_set_id  UUID NOT NULL,
  result                  JSONB NOT NULL,
  duration_ms             BIGINT NOT NULL,
  metadata                JSONB
);
CREATE INDEX IF NOT EXISTS idx_gov_audit_ts
  ON sem_reg_authoring.governance_audit_log(ts DESC);
CREATE INDEX IF NOT EXISTS idx_gov_audit_cs
  ON sem_reg_authoring.governance_audit_log(change_set_id);

-- 8. Publish batches
CREATE TABLE IF NOT EXISTS sem_reg_authoring.publish_batches (
  batch_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  change_set_ids   UUID[] NOT NULL,
  snapshot_set_id  UUID NOT NULL,
  published_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  publisher        TEXT NOT NULL
);

-- 9. Active snapshot set pointer for authoring pipeline
-- Tracks the most recently published snapshot set (the "current" state).
-- Used by publish for drift detection and by plan_publish for stale dry-run checks.
CREATE TABLE IF NOT EXISTS sem_reg_pub.active_snapshot_set (
  singleton        BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton = true),
  active_snapshot_set_id  UUID NOT NULL REFERENCES sem_reg.snapshot_sets(snapshot_set_id),
  updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
