-- Migration 100: Archive table for authoring pipeline retention.
-- Used by cleanup.rs to move terminal/orphan ChangeSets out of the hot path.

-- 1. Archive table mirrors the extended changeset columns
CREATE TABLE IF NOT EXISTS sem_reg_authoring.change_sets_archive (
  changeset_id                       UUID PRIMARY KEY,
  status                             TEXT NOT NULL,
  scope                              TEXT,
  owner_id                           UUID,
  title                              TEXT,
  rationale                          TEXT,
  content_hash                       TEXT,
  hash_version                       TEXT,
  supersedes_change_set_id           UUID,
  superseded_by                      UUID,
  superseded_at                      TIMESTAMPTZ,
  depends_on                         UUID[],
  evaluated_against_snapshot_set_id  UUID,
  created_at                         TIMESTAMPTZ NOT NULL,
  archived_at                        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_archive_status
  ON sem_reg_authoring.change_sets_archive(status);

CREATE INDEX IF NOT EXISTS idx_archive_archived_at
  ON sem_reg_authoring.change_sets_archive(archived_at DESC);

-- 2. Archive table for artifacts of archived ChangeSets
CREATE TABLE IF NOT EXISTS sem_reg_authoring.change_set_artifacts_archive (
  artifact_id    UUID PRIMARY KEY,
  change_set_id  UUID NOT NULL REFERENCES sem_reg_authoring.change_sets_archive(changeset_id),
  artifact_type  TEXT NOT NULL,
  ordinal        INT NOT NULL DEFAULT 0,
  path           TEXT,
  content        TEXT NOT NULL,
  content_hash   TEXT NOT NULL,
  metadata       JSONB,
  created_at     TIMESTAMPTZ,
  archived_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_artifacts_archive_cs
  ON sem_reg_authoring.change_set_artifacts_archive(change_set_id);
