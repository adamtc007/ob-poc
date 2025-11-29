-- Migration: 021_csg_attribute_registry_metadata.sql
-- Purpose: Add CSG metadata columns to attribute_registry
-- Part of CSG Linter implementation for business rule validation

BEGIN;

-- ============================================
-- ATTRIBUTE_REGISTRY: Add CSG Metadata Columns
-- ============================================
-- Current PK: id (text) - semantic ID like 'attr.identity.first_name'
-- Also has: uuid (for FK relationships)
-- Current columns: display_name, category, value_type, validation_rules, metadata

-- 1. Applicability Rules
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS applicability JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".attribute_registry.applicability IS
'CSG applicability rules: entity_types[], required_for[], source_documents[], depends_on[]';

-- 2. Vector Embedding (semantic_context can go in existing metadata column)
ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS embedding vector(1536);

ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".attribute_registry
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_attribute_registry_applicability
ON "ob-poc".attribute_registry USING GIN (applicability);

-- Only create vector index if pgvector extension exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_attribute_registry_embedding
                 ON "ob-poc".attribute_registry USING ivfflat (embedding vector_cosine_ops)
                 WITH (lists = 100)';
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Could not create vector index: %', SQLERRM;
END $$;

COMMIT;
