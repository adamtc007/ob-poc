-- Migration: 020_csg_document_types_metadata.sql
-- Purpose: Add CSG (Context-Sensitive Grammar) metadata columns to document_types
-- Part of CSG Linter implementation for business rule validation

BEGIN;

-- ============================================
-- DOCUMENT_TYPES: Add CSG Metadata Columns
-- ============================================
-- Current PK: type_id (uuid)
-- Current columns: type_code, display_name, category, domain, description, required_attributes

-- 1. Applicability Rules (hard constraints for CSG linting)
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS applicability JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".document_types.applicability IS
'CSG applicability rules: entity_types[], jurisdictions[], client_types[], required_for[], excludes[]';

-- 2. Semantic Context (soft/descriptive metadata for AI/suggestions)
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".document_types.semantic_context IS
'Rich semantic metadata: purpose, synonyms[], related_documents[], extraction_hints{}, keywords[]';

-- 3. Vector Embedding for similarity search
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS embedding vector(1536);

COMMENT ON COLUMN "ob-poc".document_types.embedding IS
'OpenAI ada-002 or equivalent embedding of type description + semantic context';

-- 4. Embedding metadata
ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".document_types
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_document_types_applicability
ON "ob-poc".document_types USING GIN (applicability);

CREATE INDEX IF NOT EXISTS idx_document_types_semantic_context
ON "ob-poc".document_types USING GIN (semantic_context);

-- Only create vector index if pgvector extension exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_document_types_embedding
                 ON "ob-poc".document_types USING ivfflat (embedding vector_cosine_ops)
                 WITH (lists = 100)';
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Could not create vector index: %', SQLERRM;
END $$;

COMMIT;
