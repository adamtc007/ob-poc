-- Migration 047: Client Group Tables
-- Two-stage resolution: nickname → group_id → anchor_entity_id
--
-- Design:
-- - client_group: Virtual entity representing client brand/nickname
-- - client_group_alias: Multiple aliases per group for fuzzy matching
-- - client_group_alias_embedding: Versioned embeddings with model metadata
-- - client_group_anchor: Role-based mapping to real entities

BEGIN;

-- ============================================================================
-- Client Group (virtual entity for nicknames/brands)
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,
    short_code TEXT UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

COMMENT ON TABLE "ob-poc".client_group IS 'Virtual entity representing client brand/nickname groups';

-- ============================================================================
-- Aliases (multiple per group, for fuzzy matching)
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_alias (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    alias_norm TEXT NOT NULL,  -- normalized: lowercase, trimmed
    source TEXT DEFAULT 'manual',
    confidence FLOAT DEFAULT 1.0,
    is_primary BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(group_id, alias_norm)
);

CREATE INDEX IF NOT EXISTS idx_cga_alias_norm ON "ob-poc".client_group_alias(alias_norm);
CREATE INDEX IF NOT EXISTS idx_cga_group_id ON "ob-poc".client_group_alias(group_id);

-- ============================================================================
-- Embeddings with versioning support
-- Composite PK allows multiple embeddings per alias (different models/pooling)
-- Contract: all embeddings are L2-normalized for proper cosine distance
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_alias_embedding (
    alias_id UUID NOT NULL REFERENCES "ob-poc".client_group_alias(id) ON DELETE CASCADE,
    embedder_id TEXT NOT NULL,           -- e.g., 'bge-small-en-v1.5'
    pooling TEXT NOT NULL,               -- e.g., 'cls', 'mean'
    normalize BOOLEAN NOT NULL,          -- should always be true for BGE
    dimension INT NOT NULL,              -- e.g., 384
    embedding vector(384) NOT NULL,      -- L2-normalized vector
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (alias_id, embedder_id)
);

COMMENT ON TABLE "ob-poc".client_group_alias_embedding IS
    'Embeddings must be L2-normalized. Query embeddings must also be normalized for correct cosine distance.';

-- IVFFlat index for approximate nearest neighbor search
-- Note: Run ANALYZE client_group_alias_embedding after bulk inserts for good recall
CREATE INDEX IF NOT EXISTS idx_cgae_embedding ON "ob-poc".client_group_alias_embedding
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 10);

-- ============================================================================
-- Anchor mappings (group → real entities, role-based)
-- Jurisdiction uses empty string '' for "no jurisdiction" to enable unique constraint
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_anchor (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,
    anchor_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    anchor_role TEXT NOT NULL,           -- 'ultimate_parent', 'governance_controller', etc.
    jurisdiction TEXT NOT NULL DEFAULT '',  -- empty string = no jurisdiction filter
    confidence FLOAT DEFAULT 1.0,
    priority INTEGER DEFAULT 0,          -- higher = preferred
    valid_from DATE,
    valid_to DATE,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(group_id, anchor_role, anchor_entity_id, jurisdiction)
);

CREATE INDEX IF NOT EXISTS idx_cga_anchor_group_role ON "ob-poc".client_group_anchor(group_id, anchor_role);
CREATE INDEX IF NOT EXISTS idx_cga_anchor_entity ON "ob-poc".client_group_anchor(anchor_entity_id);

COMMENT ON COLUMN "ob-poc".client_group_anchor.jurisdiction IS
    'Empty string means "applies to all jurisdictions". Specific jurisdiction takes precedence over empty.';

-- ============================================================================
-- Anchor role reference (for documentation/validation)
-- ============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".client_group_anchor_role (
    role_code TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    default_for_domains TEXT[]  -- which verb domains use this role by default
);

INSERT INTO "ob-poc".client_group_anchor_role (role_code, description, default_for_domains) VALUES
    ('ultimate_parent', 'UBO top-level parent (ownership apex)', ARRAY['ubo', 'ownership']),
    ('governance_controller', 'Operational/board control entity (ManCo equivalent)', ARRAY['session', 'cbu', 'view']),
    ('book_controller', 'Regional book controller', ARRAY['view']),
    ('operating_controller', 'Day-to-day operations controller', ARRAY['contract', 'service']),
    ('regulatory_anchor', 'Primary regulated entity for compliance', ARRAY['kyc', 'screening'])
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================================
-- Helper view for alias search with group info
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_client_group_aliases AS
SELECT
    cga.id AS alias_id,
    cga.alias,
    cga.alias_norm,
    cga.is_primary,
    cga.confidence AS alias_confidence,
    cg.id AS group_id,
    cg.canonical_name,
    cg.short_code
FROM "ob-poc".client_group_alias cga
JOIN "ob-poc".client_group cg ON cg.id = cga.group_id;

-- ============================================================================
-- Helper view for anchor resolution with deterministic ordering
-- ============================================================================
CREATE OR REPLACE VIEW "ob-poc".v_client_group_anchors AS
SELECT
    cga.group_id,
    cga.anchor_role,
    cga.anchor_entity_id,
    cga.jurisdiction,
    cga.confidence,
    cga.priority,
    cga.valid_from,
    cga.valid_to,
    e.name AS entity_name,
    e.entity_type_id
FROM "ob-poc".client_group_anchor cga
JOIN "ob-poc".entities e ON e.entity_id = cga.anchor_entity_id
WHERE (cga.valid_from IS NULL OR cga.valid_from <= CURRENT_DATE)
  AND (cga.valid_to IS NULL OR cga.valid_to >= CURRENT_DATE);

-- ============================================================================
-- Function: Resolve client group to anchor entity
-- Uses deterministic ordering: exact jurisdiction → global → priority → confidence → uuid
-- ============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".resolve_client_group_anchor(
    p_group_id UUID,
    p_anchor_role TEXT,
    p_jurisdiction TEXT DEFAULT ''
) RETURNS TABLE (
    anchor_entity_id UUID,
    entity_name TEXT,
    jurisdiction TEXT,
    confidence FLOAT,
    match_type TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        cga.anchor_entity_id,
        e.name::TEXT AS entity_name,
        cga.jurisdiction,
        cga.confidence,
        CASE
            WHEN cga.jurisdiction = p_jurisdiction AND p_jurisdiction != '' THEN 'exact_jurisdiction'
            WHEN cga.jurisdiction = '' THEN 'global_fallback'
            ELSE 'other'
        END AS match_type
    FROM "ob-poc".client_group_anchor cga
    JOIN "ob-poc".entities e ON e.entity_id = cga.anchor_entity_id
    WHERE cga.group_id = p_group_id
      AND cga.anchor_role = p_anchor_role
      AND (cga.valid_from IS NULL OR cga.valid_from <= CURRENT_DATE)
      AND (cga.valid_to IS NULL OR cga.valid_to >= CURRENT_DATE)
      AND (
          cga.jurisdiction = p_jurisdiction  -- exact match
          OR (p_jurisdiction = '' AND cga.jurisdiction = '')  -- no jurisdiction requested, match global
          OR (p_jurisdiction != '' AND cga.jurisdiction = '')  -- specific requested, fallback to global
      )
    ORDER BY
        CASE WHEN cga.jurisdiction = p_jurisdiction AND p_jurisdiction != '' THEN 0 ELSE 1 END,  -- exact jurisdiction first
        cga.priority DESC,                                    -- then priority
        cga.confidence DESC,                                  -- then confidence
        cga.anchor_entity_id                                  -- stable tie-breaker
    LIMIT 1;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".resolve_client_group_anchor IS
    'Resolve client group to anchor entity with deterministic ordering';

COMMIT;
