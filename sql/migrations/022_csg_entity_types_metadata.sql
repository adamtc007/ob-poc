-- Migration: 022_csg_entity_types_metadata.sql
-- Purpose: Add CSG metadata columns to entity_types for hierarchy and semantic context
-- Part of CSG Linter implementation for business rule validation

BEGIN;

-- ============================================
-- ENTITY_TYPES: Add CSG Metadata Columns
-- ============================================
-- Current PK: entity_type_id (uuid)
-- Current columns: name, description, table_name

-- 1. Type Code (normalized identifier for rules matching)
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS type_code VARCHAR(100);

-- Populate type_code from name (uppercase, underscores)
UPDATE "ob-poc".entity_types
SET type_code = UPPER(REPLACE(REPLACE(name, ' ', '_'), '-', '_'))
WHERE type_code IS NULL;

-- Make it unique after population (use IF NOT EXISTS pattern)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'ob-poc'
        AND indexname = 'idx_entity_types_type_code'
    ) THEN
        CREATE UNIQUE INDEX idx_entity_types_type_code
        ON "ob-poc".entity_types(type_code);
    END IF;
END $$;

-- 2. Semantic Context
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".entity_types.semantic_context IS
'Rich semantic metadata: category, parent_type, synonyms[], typical_documents[], typical_attributes[]';

-- 3. Type Hierarchy (for wildcard matching)
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS parent_type_id UUID REFERENCES "ob-poc".entity_types(entity_type_id);

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS type_hierarchy_path TEXT[];

COMMENT ON COLUMN "ob-poc".entity_types.type_hierarchy_path IS
'Materialized path for efficient ancestor queries, e.g., ["ENTITY", "LEGAL_ENTITY", "LIMITED_COMPANY"]';

-- 4. Vector Embedding
ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS embedding vector(1536);

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".entity_types
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_entity_types_semantic_context
ON "ob-poc".entity_types USING GIN (semantic_context);

CREATE INDEX IF NOT EXISTS idx_entity_types_parent
ON "ob-poc".entity_types (parent_type_id);

CREATE INDEX IF NOT EXISTS idx_entity_types_hierarchy
ON "ob-poc".entity_types USING GIN (type_hierarchy_path);

-- Only create vector index if pgvector extension exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_entity_types_embedding
                 ON "ob-poc".entity_types USING ivfflat (embedding vector_cosine_ops)
                 WITH (lists = 50)';
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Could not create vector index: %', SQLERRM;
END $$;

COMMIT;
