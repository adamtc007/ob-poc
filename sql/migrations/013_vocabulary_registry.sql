-- ============================================
-- Vocabulary Registry for DSL Verbs
-- ============================================
BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".vocabulary_registry (
    vocab_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    verb_name VARCHAR(100) UNIQUE NOT NULL,
    domain VARCHAR(50) NOT NULL,
    action VARCHAR(50) NOT NULL,
    signature TEXT NOT NULL,
    description TEXT,
    operation_types TEXT[] NOT NULL,
    parameter_schema JSONB,
    examples JSONB,
    related_attributes UUID[],
    is_active BOOLEAN DEFAULT true,
    is_shared BOOLEAN DEFAULT false,
    deprecated_at TIMESTAMPTZ,
    usage_count INTEGER DEFAULT 0,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_vocab_verb_name ON "ob-poc".vocabulary_registry(verb_name);
CREATE INDEX idx_vocab_domain ON "ob-poc".vocabulary_registry(domain);
CREATE INDEX idx_vocab_active ON "ob-poc".vocabulary_registry(is_active) WHERE is_active = true;
CREATE INDEX idx_vocab_operation_types ON "ob-poc".vocabulary_registry USING GIN(operation_types);

COMMENT ON TABLE "ob-poc".vocabulary_registry IS 
'Central registry of all DSL verbs with their signatures, domains, and usage patterns for RAG-based generation';

COMMIT;
