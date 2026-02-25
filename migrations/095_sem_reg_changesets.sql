-- Migration 095: Semantic Registry Changesets
-- Stage 3.1 — Draft workflow: changesets, entries, reviews.
-- Changesets are the sole draft payload store (no draft_snapshots table).

CREATE TABLE sem_reg.changesets (
    changeset_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status          TEXT NOT NULL CHECK (status IN ('draft','in_review','approved','published','rejected')),
    owner_actor_id  TEXT NOT NULL,
    scope           TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sem_reg.changeset_entries (
    entry_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    changeset_id     UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
    object_fqn       TEXT NOT NULL,
    object_type      TEXT NOT NULL,
    change_kind      TEXT NOT NULL CHECK (change_kind IN ('add','modify','remove')),
    draft_payload    JSONB NOT NULL,
    base_snapshot_id UUID,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sem_reg.changeset_reviews (
    review_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    changeset_id    UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
    actor_id        TEXT NOT NULL,
    verdict         TEXT NOT NULL CHECK (verdict IN ('approved','rejected','requested_changes')),
    comment         TEXT,
    reviewed_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes for common queries
CREATE INDEX idx_changesets_status ON sem_reg.changesets (status);
CREATE INDEX idx_changesets_owner ON sem_reg.changesets (owner_actor_id);
CREATE INDEX idx_changeset_entries_changeset ON sem_reg.changeset_entries (changeset_id);
CREATE INDEX idx_changeset_reviews_changeset ON sem_reg.changeset_reviews (changeset_id);
