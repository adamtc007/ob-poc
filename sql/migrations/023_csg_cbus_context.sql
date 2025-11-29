-- Migration: 023_csg_cbus_context.sql
-- Purpose: Add CSG context columns to cbus for risk and onboarding state
-- Part of CSG Linter implementation for business rule validation

BEGIN;

-- ============================================
-- CBUS: Add CSG Context Columns
-- ============================================
-- Current PK: cbu_id (uuid)
-- ALREADY HAS: client_type, jurisdiction (no migration needed for these!)

-- 1. Risk Context (for risk-aware validation)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS risk_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".cbus.risk_context IS
'Risk-related context: risk_rating, pep_exposure, sanctions_exposure, industry_codes[]';

-- 2. Onboarding Context (for state-aware validation)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS onboarding_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".cbus.onboarding_context IS
'Onboarding state: stage, completed_steps[], pending_requirements[], override_rules[]';

-- 3. Semantic Context (for AI-assisted operations)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS semantic_context JSONB DEFAULT '{}'::jsonb;

COMMENT ON COLUMN "ob-poc".cbus.semantic_context IS
'Rich semantic metadata: business_description, industry_keywords[], related_entities[]';

-- 4. Vector Embedding (for similarity search across CBUs)
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS embedding vector(1536);

ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS embedding_model VARCHAR(100);

ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS embedding_updated_at TIMESTAMPTZ;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cbus_risk_context ON "ob-poc".cbus USING GIN (risk_context);
CREATE INDEX IF NOT EXISTS idx_cbus_onboarding_context ON "ob-poc".cbus USING GIN (onboarding_context);
CREATE INDEX IF NOT EXISTS idx_cbus_semantic_context ON "ob-poc".cbus USING GIN (semantic_context);

-- Only create vector index if pgvector extension exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_cbus_embedding
                 ON "ob-poc".cbus USING ivfflat (embedding vector_cosine_ops)
                 WITH (lists = 100)';
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Could not create vector index: %', SQLERRM;
END $$;

COMMIT;
