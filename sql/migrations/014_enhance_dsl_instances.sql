-- ============================================
-- Enhance DSL Instances for RAG
-- ============================================
BEGIN;

-- Add RAG-specific columns
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    operation_type VARCHAR(50);
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    domain VARCHAR(50);
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    natural_language_input TEXT;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    execution_success BOOLEAN DEFAULT false;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    execution_error TEXT;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    execution_time_ms INTEGER;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    confidence_score NUMERIC(5,2);
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    generation_attempt INTEGER DEFAULT 1;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    generation_method VARCHAR(50) DEFAULT 'template';
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    llm_model VARCHAR(100);
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    llm_tokens_used INTEGER;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    validation_errors JSONB;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    rag_context_used JSONB;
ALTER TABLE "ob-poc".dsl_instances ADD COLUMN IF NOT EXISTS
    tags TEXT[];

-- Indexes for efficient RAG retrieval
CREATE INDEX IF NOT EXISTS idx_dsl_operation_type 
    ON "ob-poc".dsl_instances(operation_type);
    
CREATE INDEX IF NOT EXISTS idx_dsl_domain 
    ON "ob-poc".dsl_instances(domain);
    
CREATE INDEX IF NOT EXISTS idx_dsl_success 
    ON "ob-poc".dsl_instances(execution_success);
    
CREATE INDEX IF NOT EXISTS idx_dsl_confidence 
    ON "ob-poc".dsl_instances(confidence_score DESC);
    
CREATE INDEX IF NOT EXISTS idx_dsl_tags 
    ON "ob-poc".dsl_instances USING GIN(tags);

-- Full-text search index for natural language inputs
CREATE INDEX IF NOT EXISTS idx_dsl_natural_language_fts 
    ON "ob-poc".dsl_instances 
    USING gin(to_tsvector('english', COALESCE(natural_language_input, '')));

COMMIT;
