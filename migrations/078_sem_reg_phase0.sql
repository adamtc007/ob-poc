-- Migration 078: Semantic Registry Phase 0 — Snapshot Infrastructure
-- Immutable snapshot data model, core enums, snapshot_sets + snapshots tables.
-- All registry objects share one snapshot table with JSONB definition column.

BEGIN;

-- ============================================================
-- Schema
-- ============================================================

CREATE SCHEMA IF NOT EXISTS sem_reg;

-- ============================================================
-- Enums
-- ============================================================

CREATE TYPE sem_reg.governance_tier AS ENUM ('governed', 'operational');
CREATE TYPE sem_reg.trust_class AS ENUM ('proof', 'decision_support', 'convenience');
CREATE TYPE sem_reg.snapshot_status AS ENUM ('draft', 'active', 'deprecated', 'retired');
CREATE TYPE sem_reg.change_type AS ENUM (
    'created', 'non_breaking', 'breaking',
    'promotion', 'deprecation', 'retirement'
);
CREATE TYPE sem_reg.object_type AS ENUM (
    'attribute_def',
    'entity_type_def',
    'relationship_type_def',
    'verb_contract',
    'taxonomy_def',
    'taxonomy_node',
    'membership_rule',
    'view_def',
    'policy_rule',
    'evidence_requirement',
    'document_type_def',
    'observation_def',
    'derivation_spec'
);

-- ============================================================
-- Classification levels (reference data)
-- ============================================================

CREATE TABLE sem_reg.classification_levels (
    level_id        SERIAL PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    ordinal         INT NOT NULL UNIQUE,   -- Public=0 < Internal=1 < Confidential=2 < Restricted=3
    description     TEXT
);

INSERT INTO sem_reg.classification_levels (name, ordinal, description) VALUES
    ('public',       0, 'No restrictions on disclosure'),
    ('internal',     1, 'Internal use only — not for external distribution'),
    ('confidential', 2, 'Limited distribution — need-to-know basis'),
    ('restricted',   3, 'Highest sensitivity — regulatory or legal constraints');

-- ============================================================
-- Snapshot sets (atomic publish transactions)
-- ============================================================

CREATE TABLE sem_reg.snapshot_sets (
    snapshot_set_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    description     TEXT,
    created_by      TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================
-- Snapshots — the universal immutable registry table
-- ============================================================

CREATE TABLE sem_reg.snapshots (
    snapshot_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_set_id   UUID REFERENCES sem_reg.snapshot_sets(snapshot_set_id),

    -- Identity
    object_type       sem_reg.object_type NOT NULL,
    object_id         UUID NOT NULL,

    -- Versioning
    version_major     INT NOT NULL DEFAULT 1,
    version_minor     INT NOT NULL DEFAULT 0,
    status            sem_reg.snapshot_status NOT NULL DEFAULT 'active',

    -- Governance
    governance_tier   sem_reg.governance_tier NOT NULL DEFAULT 'operational',
    trust_class       sem_reg.trust_class NOT NULL DEFAULT 'convenience',

    -- Security label (JSONB — SecurityLabel struct)
    security_label    JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Temporal — immutable once set
    effective_from    TIMESTAMPTZ NOT NULL DEFAULT now(),
    effective_until   TIMESTAMPTZ,          -- NULL = currently active

    -- Lineage
    predecessor_id    UUID REFERENCES sem_reg.snapshots(snapshot_id),
    change_type       sem_reg.change_type NOT NULL DEFAULT 'created',
    change_rationale  TEXT,
    created_by        TEXT NOT NULL,
    approved_by       TEXT,                 -- 'auto' for operational tier

    -- The full object definition (type-specific JSONB)
    definition        JSONB NOT NULL DEFAULT '{}'::jsonb,

    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- ── Invariant: operational tier cannot have trust_class = proof ──
    CONSTRAINT chk_proof_rule
        CHECK (trust_class != 'proof' OR governance_tier = 'governed')
);

-- ============================================================
-- Indexes
-- ============================================================

-- Fast "resolve current active" — at most one active snapshot per (object_type, object_id)
CREATE UNIQUE INDEX uix_snapshots_active
    ON sem_reg.snapshots (object_type, object_id)
    WHERE effective_until IS NULL AND status = 'active';

-- History lookup — all snapshots for an object ordered by time
CREATE INDEX ix_snapshots_history
    ON sem_reg.snapshots (object_id, effective_from DESC);

-- Point-in-time resolution — find snapshot active at a given timestamp
CREATE INDEX ix_snapshots_temporal
    ON sem_reg.snapshots (object_type, object_id, effective_from, effective_until);

-- Snapshot set membership
CREATE INDEX ix_snapshots_set
    ON sem_reg.snapshots (snapshot_set_id)
    WHERE snapshot_set_id IS NOT NULL;

-- Object type listing
CREATE INDEX ix_snapshots_type_status
    ON sem_reg.snapshots (object_type, status);

-- ============================================================
-- Helper: resolve active snapshot at a point in time
-- ============================================================

CREATE OR REPLACE FUNCTION sem_reg.resolve_active(
    p_object_type sem_reg.object_type,
    p_object_id   UUID,
    p_as_of       TIMESTAMPTZ DEFAULT now()
)
RETURNS sem_reg.snapshots
LANGUAGE sql STABLE
AS $$
    SELECT *
    FROM sem_reg.snapshots
    WHERE object_type = p_object_type
      AND object_id   = p_object_id
      AND status       = 'active'
      AND effective_from <= p_as_of
      AND (effective_until IS NULL OR effective_until > p_as_of)
    ORDER BY effective_from DESC
    LIMIT 1;
$$;

-- ============================================================
-- Helper: count active snapshots by object type
-- ============================================================

CREATE OR REPLACE FUNCTION sem_reg.count_active(
    p_object_type sem_reg.object_type DEFAULT NULL
)
RETURNS TABLE(object_type sem_reg.object_type, active_count BIGINT)
LANGUAGE sql STABLE
AS $$
    SELECT s.object_type, COUNT(*)
    FROM sem_reg.snapshots s
    WHERE s.status = 'active'
      AND s.effective_until IS NULL
      AND (p_object_type IS NULL OR s.object_type = p_object_type)
    GROUP BY s.object_type
    ORDER BY s.object_type;
$$;

COMMIT;
