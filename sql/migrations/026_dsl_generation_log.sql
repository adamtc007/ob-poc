-- DSL Generation Log
-- Captures agent prompt/response iterations for training data extraction and audit trail
-- Migration: 026_dsl_generation_log.sql

-- Enable pg_trgm extension for similarity search if not already enabled
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Main table for capturing generation iterations
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_generation_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Link to persisted DSL (nullable - might fail before persisting)
    instance_id UUID REFERENCES "ob-poc".dsl_instances(instance_id),

    -- === THE GOLD PAIR FOR TRAINING ===
    user_intent TEXT NOT NULL,           -- Natural language: "Create hedge fund with John as director"
    final_valid_dsl TEXT,                -- The DSL that passed validation (NULL if never succeeded)

    -- === ITERATION HISTORY ===
    -- JSONB array capturing each attempt
    -- Structure:
    -- [
    --   {
    --     "attempt": 1,
    --     "timestamp": "2025-01-15T10:30:00Z",
    --     "prompt_template": "cbu_create_v2",
    --     "prompt_text": "Given the vocabulary...",
    --     "raw_response": "I'll create a CBU...",
    --     "extracted_dsl": "(cbu.create :name ...)",
    --     "parse_result": {"success": true, "error": null},
    --     "lint_result": {"valid": false, "errors": ["Unknown verb"], "warnings": []},
    --     "compile_result": {"success": false, "error": "Unknown verb", "step_count": 0},
    --     "latency_ms": 1500,
    --     "input_tokens": 500,
    --     "output_tokens": 200
    --   }
    -- ]
    iterations JSONB NOT NULL DEFAULT '[]',

    -- === CONTEXT ===
    domain_name VARCHAR(50) NOT NULL,           -- "cbu", "entity", "document"
    session_id UUID,                            -- Link to agent session if applicable
    cbu_id UUID,                                -- Target CBU if applicable

    -- === METRICS ===
    model_used VARCHAR(100),                    -- "claude-sonnet-4-20250514"
    total_attempts INT NOT NULL DEFAULT 1,
    success BOOLEAN NOT NULL DEFAULT false,
    total_latency_ms INT,                       -- Sum of all attempts
    total_input_tokens INT,
    total_output_tokens INT,

    -- === TIMESTAMPS ===
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- Indexes for training data extraction
CREATE INDEX IF NOT EXISTS idx_gen_log_success ON "ob-poc".dsl_generation_log(success) WHERE success = true;
CREATE INDEX IF NOT EXISTS idx_gen_log_domain ON "ob-poc".dsl_generation_log(domain_name);
CREATE INDEX IF NOT EXISTS idx_gen_log_created ON "ob-poc".dsl_generation_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_gen_log_instance ON "ob-poc".dsl_generation_log(instance_id) WHERE instance_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_gen_log_session ON "ob-poc".dsl_generation_log(session_id) WHERE session_id IS NOT NULL;

-- GIN index for JSONB queries on iterations
CREATE INDEX IF NOT EXISTS idx_gen_log_iterations ON "ob-poc".dsl_generation_log USING GIN (iterations);

-- Trigram index for similarity search on user_intent
CREATE INDEX IF NOT EXISTS idx_gen_log_intent_trgm ON "ob-poc".dsl_generation_log USING GIN (user_intent gin_trgm_ops);

-- Comments
COMMENT ON TABLE "ob-poc".dsl_generation_log IS 'Captures agent DSL generation iterations for training data extraction and audit trail';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.user_intent IS 'Natural language description of what user wanted - the input side of training pairs';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.final_valid_dsl IS 'Successfully validated DSL - the output side of training pairs';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.iterations IS 'JSONB array of each generation attempt with prompts, responses, and validation results';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.domain_name IS 'Primary domain for this generation: cbu, entity, document, etc.';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.model_used IS 'LLM model identifier used for generation';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.total_attempts IS 'Number of generation attempts before success or failure';
COMMENT ON COLUMN "ob-poc".dsl_generation_log.success IS 'Whether generation ultimately succeeded';
