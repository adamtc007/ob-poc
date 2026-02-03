-- Migration: 073_entity_linking_support.sql
-- Purpose: Add supporting tables for entity linking service
-- Author: Claude Code
-- Date: 2026-02-03

-- ============================================================================
-- Entity concept links (for disambiguation via industry/domain)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_concept_link (
    entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    concept_id  TEXT NOT NULL,
    relation    TEXT NOT NULL DEFAULT 'related',
    weight      REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
    provenance  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_id, concept_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_ecl_concept ON "ob-poc".entity_concept_link(concept_id);
CREATE INDEX IF NOT EXISTS idx_ecl_entity ON "ob-poc".entity_concept_link(entity_id);

COMMENT ON TABLE "ob-poc".entity_concept_link IS 'Links entities to lexicon concepts for disambiguation';
COMMENT ON COLUMN "ob-poc".entity_concept_link.concept_id IS 'Concept identifier (e.g., industry:banking, domain:custody)';
COMMENT ON COLUMN "ob-poc".entity_concept_link.relation IS 'Relationship type (related, primary_industry, etc.)';

-- ============================================================================
-- Token features for fuzzy matching (populated by compiler)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_feature (
    entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    token_norm  TEXT NOT NULL,
    weight      REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
    source      TEXT NOT NULL DEFAULT 'canonical_name',
    PRIMARY KEY (entity_id, token_norm)
);

CREATE INDEX IF NOT EXISTS idx_ef_token ON "ob-poc".entity_feature(token_norm);
CREATE INDEX IF NOT EXISTS idx_ef_entity ON "ob-poc".entity_feature(entity_id);

COMMENT ON TABLE "ob-poc".entity_feature IS 'Token features for fuzzy entity matching';
COMMENT ON COLUMN "ob-poc".entity_feature.token_norm IS 'Normalized token (lowercase, stripped)';
COMMENT ON COLUMN "ob-poc".entity_feature.source IS 'Source of token (canonical_name, alias, manual)';

-- ============================================================================
-- Add normalized name column to entities if missing
-- ============================================================================

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
        AND table_name = 'entities'
        AND column_name = 'name_norm'
    ) THEN
        ALTER TABLE "ob-poc".entities ADD COLUMN name_norm TEXT;
    END IF;
END $$;

-- Populate name_norm from name (basic normalization)
UPDATE "ob-poc".entities
SET name_norm = LOWER(TRIM(REGEXP_REPLACE(name, '[^a-zA-Z0-9 ]', ' ', 'g')))
WHERE name_norm IS NULL OR name_norm = '';

-- Create index on name_norm
CREATE INDEX IF NOT EXISTS idx_entities_name_norm ON "ob-poc".entities(name_norm);

-- ============================================================================
-- Trigger to maintain name_norm and updated_at
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_entity_name_norm()
RETURNS TRIGGER AS $$
BEGIN
    NEW.name_norm = LOWER(TRIM(REGEXP_REPLACE(NEW.name, '[^a-zA-Z0-9 ]', ' ', 'g')));
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_entity_name_norm ON "ob-poc".entities;
CREATE TRIGGER trg_entity_name_norm
    BEFORE INSERT OR UPDATE OF name ON "ob-poc".entities
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_entity_name_norm();

-- ============================================================================
-- View for entity linking data (convenient for compiler)
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_entity_linking_data AS
SELECT
    e.entity_id,
    et.name as entity_kind,
    e.name as canonical_name,
    COALESCE(e.name_norm, LOWER(e.name)) as canonical_name_norm,
    e.created_at,
    e.updated_at
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id;

COMMENT ON VIEW "ob-poc".v_entity_linking_data IS 'Flattened entity data for linking service';

-- ============================================================================
-- View for all entity aliases (union of entity_names and agent.entity_aliases)
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_entity_aliases AS
SELECT
    e.entity_id,
    en.name as alias,
    LOWER(TRIM(REGEXP_REPLACE(en.name, '[^a-zA-Z0-9 ]', ' ', 'g'))) as alias_norm,
    CASE en.name_type
        WHEN 'LEGAL' THEN 1.0
        WHEN 'TRADING' THEN 0.95
        WHEN 'SHORT' THEN 0.9
        WHEN 'ALTERNATIVE' THEN 0.85
        WHEN 'HISTORICAL' THEN 0.7
        ELSE 0.8
    END::REAL as weight,
    'entity_names' as source
FROM "ob-poc".entities e
JOIN "ob-poc".entity_names en ON e.entity_id = en.entity_id
UNION ALL
SELECT
    ea.entity_id,
    ea.alias,
    LOWER(TRIM(REGEXP_REPLACE(ea.alias, '[^a-zA-Z0-9 ]', ' ', 'g'))) as alias_norm,
    COALESCE(ea.confidence, 1.0)::REAL as weight,
    'agent_aliases' as source
FROM agent.entity_aliases ea
WHERE ea.entity_id IS NOT NULL;

COMMENT ON VIEW "ob-poc".v_entity_aliases IS 'Unified view of all entity aliases for linking';

-- ============================================================================
-- Stats view for monitoring
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_entity_linking_stats AS
SELECT
    (SELECT COUNT(*) FROM "ob-poc".entities) as total_entities,
    (SELECT COUNT(*) FROM "ob-poc".entity_names) as total_names,
    (SELECT COUNT(*) FROM agent.entity_aliases WHERE entity_id IS NOT NULL) as total_agent_aliases,
    (SELECT COUNT(*) FROM "ob-poc".entity_concept_link) as total_concept_links,
    (SELECT COUNT(*) FROM "ob-poc".entity_feature) as total_features,
    (SELECT COUNT(DISTINCT entity_id) FROM "ob-poc".entity_concept_link) as entities_with_concepts,
    (SELECT COUNT(DISTINCT entity_id) FROM "ob-poc".entity_feature) as entities_with_features;

COMMENT ON VIEW "ob-poc".v_entity_linking_stats IS 'Statistics for entity linking data';
