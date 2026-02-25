-- Migration 094: sem_reg_pub schema — read-only projection tables for ob-poc consumption.
-- These are populated by the outbox dispatcher from sem_reg.outbox_events.

CREATE SCHEMA IF NOT EXISTS sem_reg_pub;

-- Active verb contracts (flattened from JSONB snapshots)
CREATE TABLE sem_reg_pub.active_verb_contracts (
    snapshot_set_id  UUID NOT NULL,
    snapshot_id      UUID NOT NULL,
    fqn              TEXT NOT NULL,
    verb_name        TEXT NOT NULL,
    payload          JSONB NOT NULL,
    published_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (snapshot_set_id, fqn)
);

-- Active entity type definitions
CREATE TABLE sem_reg_pub.active_entity_types (
    snapshot_set_id  UUID NOT NULL,
    snapshot_id      UUID NOT NULL,
    fqn              TEXT NOT NULL,
    payload          JSONB NOT NULL,
    published_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (snapshot_set_id, fqn)
);

-- Active taxonomies
CREATE TABLE sem_reg_pub.active_taxonomies (
    snapshot_set_id  UUID NOT NULL,
    snapshot_id      UUID NOT NULL,
    fqn              TEXT NOT NULL,
    payload          JSONB NOT NULL,
    published_at     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (snapshot_set_id, fqn)
);

-- Projection watermark — tracks how far the dispatcher has processed
CREATE TABLE sem_reg_pub.projection_watermark (
    projection_name  TEXT PRIMARY KEY,     -- 'active_snapshot_set'
    last_outbox_seq  BIGINT,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed the watermark row
INSERT INTO sem_reg_pub.projection_watermark (projection_name, last_outbox_seq)
VALUES ('active_snapshot_set', NULL)
ON CONFLICT DO NOTHING;
