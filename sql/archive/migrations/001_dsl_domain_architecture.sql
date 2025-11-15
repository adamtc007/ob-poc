-- ============================================================================
-- Migration 001: DSL Domain Architecture
-- ============================================================================
-- This migration transforms the current single-table DSL storage into a
-- domain-based architecture with proper versioning, AST storage, and
-- execution tracking.
--
-- Changes:
-- 1. Create DSL domains table for multi-domain support
-- 2. Restructure DSL storage with domain context and sequential versioning
-- 3. Add AST storage table for compiled DSL caching
-- 4. Add execution tracking for the compilation/execution pipeline
-- 5. Migrate existing data from dsl_ob to new structure
-- ============================================================================

BEGIN;

-- ============================================================================
-- STEP 1: CREATE NEW DOMAIN-BASED TABLES
-- ============================================================================

-- DSL Domain Management Table
-- Supports: Onboarding, KYC, KYC_UBO, Account_Opening, etc.
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_domains (
    domain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL UNIQUE, -- 'Onboarding', 'KYC', 'KYC_UBO', 'Account_Opening'
    description TEXT,
    base_grammar_version VARCHAR(20) DEFAULT '1.0.0',
    vocabulary_version VARCHAR(20) DEFAULT '1.0.0', -- Domain-specific vocabulary version
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Create indexes for domains
CREATE INDEX IF NOT EXISTS idx_dsl_domains_name ON "ob-poc".dsl_domains (domain_name);
CREATE INDEX IF NOT EXISTS idx_dsl_domains_active ON "ob-poc".dsl_domains (active);

-- DSL Versions Table (replaces/extends dsl_ob)
-- Each domain has sequential versions with complete change tracking
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains (domain_id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL, -- Sequential: 1, 2, 3, etc.
    functional_state VARCHAR(100), -- 'Create_Case', 'Generate_UBO', 'Review_Edit', 'Confirm_Compile', 'Run'
    dsl_source_code TEXT NOT NULL,
    compilation_status VARCHAR(50) DEFAULT 'DRAFT', -- 'DRAFT', 'COMPILING', 'COMPILED', 'ACTIVE', 'DEPRECATED', 'ERROR'
    change_description TEXT, -- Change log entry describing what changed
    parent_version_id UUID REFERENCES "ob-poc".dsl_versions (version_id), -- For branching/merging
    created_by VARCHAR(255), -- User who created this version
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    compiled_at TIMESTAMPTZ, -- When compilation completed
    activated_at TIMESTAMPTZ, -- When version became active
    UNIQUE (domain_id, version_number)
);

-- Create indexes for dsl_versions
CREATE INDEX IF NOT EXISTS idx_dsl_versions_domain_version ON "ob-poc".dsl_versions (domain_id, version_number DESC);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_status ON "ob-poc".dsl_versions (compilation_status);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_functional_state ON "ob-poc".dsl_versions (functional_state);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_created_at ON "ob-poc".dsl_versions (created_at DESC);

-- Parsed AST Storage Table
-- Stores compiled AST with metadata for fast execution
CREATE TABLE IF NOT EXISTS "ob-poc".parsed_asts (
    ast_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_versions (version_id) ON DELETE CASCADE,
    ast_json JSONB NOT NULL, -- Complete AST structure from NOM parser
    parse_metadata JSONB, -- Parser timing, validation results, warnings
    grammar_version VARCHAR(20) NOT NULL, -- eBNF grammar version used for parsing
    parser_version VARCHAR(20) NOT NULL, -- Version of NOM parser used
    ast_hash VARCHAR(64), -- SHA-256 hash of AST for change detection
    node_count INTEGER, -- Number of AST nodes (for statistics)
    complexity_score DECIMAL(10,2), -- Calculated complexity metric
    parsed_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    invalidated_at TIMESTAMPTZ, -- When AST was invalidated (for cache management)
    UNIQUE (version_id)
);

-- Create indexes for parsed_asts
CREATE INDEX IF NOT EXISTS idx_parsed_asts_version_id ON "ob-poc".parsed_asts (version_id);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_parsed_at ON "ob-poc".parsed_asts (parsed_at DESC);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_grammar_version ON "ob-poc".parsed_asts (grammar_version);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_hash ON "ob-poc".parsed_asts (ast_hash);

-- DSL Execution Log Table
-- Tracks the complete compilation and execution pipeline
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_execution_log (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_versions (version_id) ON DELETE CASCADE,
    cbu_id VARCHAR(255), -- Client Business Unit context (when executed)
    execution_phase VARCHAR(50) NOT NULL, -- 'PARSE', 'COMPILE', 'VALIDATE', 'EXECUTE', 'COMPLETE'
    status VARCHAR(50) NOT NULL, -- 'SUCCESS', 'FAILED', 'IN_PROGRESS', 'CANCELLED'
    result_data JSONB, -- Execution results, entity IDs created, etc.
    error_details JSONB, -- Error information if failed (stack trace, error codes)
    performance_metrics JSONB, -- Timing, memory usage, etc.
    executed_by VARCHAR(255), -- User or system that triggered execution
    started_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    completed_at TIMESTAMPTZ,
    duration_ms INTEGER GENERATED ALWAYS AS (
        CASE
            WHEN completed_at IS NOT NULL
            THEN EXTRACT(EPOCH FROM (completed_at - started_at)) * 1000
            ELSE NULL
        END
    ) STORED
);

-- Create indexes for execution log
CREATE INDEX IF NOT EXISTS idx_dsl_execution_version_phase ON "ob-poc".dsl_execution_log (version_id, execution_phase);
CREATE INDEX IF NOT EXISTS idx_dsl_execution_status ON "ob-poc".dsl_execution_log (status);
CREATE INDEX IF NOT EXISTS idx_dsl_execution_started_at ON "ob-poc".dsl_execution_log (started_at DESC);
CREATE INDEX IF NOT EXISTS idx_dsl_execution_cbu_id ON "ob-poc".dsl_execution_log (cbu_id);

-- ============================================================================
-- STEP 2: INSERT DEFAULT DOMAINS
-- ============================================================================

-- Insert the standard DSL domains
INSERT INTO "ob-poc".dsl_domains (domain_name, description, base_grammar_version, vocabulary_version)
VALUES
    ('Legacy', 'Migrated DSL content from original dsl_ob table', '1.0.0', '1.0.0'),
    ('Onboarding', 'Client onboarding workflows and processes', '1.0.0', '1.0.0'),
    ('KYC', 'Know Your Customer compliance and verification workflows', '1.0.0', '1.0.0'),
    ('KYC_UBO', 'Ultimate Beneficial Owner identification (embedded in KYC)', '1.0.0', '1.0.0'),
    ('Account_Opening', 'Account setup and approval processes', '1.0.0', '1.0.0'),
    ('Compliance', 'General compliance checking and monitoring', '1.0.0', '1.0.0'),
    ('Risk_Assessment', 'Risk evaluation and scoring workflows', '1.0.0', '1.0.0')
ON CONFLICT (domain_name) DO NOTHING;

-- ============================================================================
-- STEP 3: MIGRATE EXISTING DATA FROM dsl_ob
-- ============================================================================

-- Check if dsl_ob table exists and has data
DO $migration$
DECLARE
    legacy_domain_id UUID;
    dsl_record RECORD;
    version_counter INTEGER := 1;
BEGIN
    -- Get the Legacy domain ID
    SELECT domain_id INTO legacy_domain_id
    FROM "ob-poc".dsl_domains
    WHERE domain_name = 'Legacy';

    -- Only migrate if dsl_ob table exists and has data
    IF EXISTS (SELECT 1 FROM information_schema.tables
               WHERE table_schema = 'ob-poc'
               AND table_name = 'dsl_ob') THEN

        -- Migrate each record from dsl_ob to dsl_versions
        FOR dsl_record IN
            SELECT version_id, cbu_id, dsl_text, created_at
            FROM "ob-poc".dsl_ob
            ORDER BY created_at ASC
        LOOP
            INSERT INTO "ob-poc".dsl_versions (
                version_id,
                domain_id,
                version_number,
                functional_state,
                dsl_source_code,
                compilation_status,
                change_description,
                created_by,
                created_at
            ) VALUES (
                dsl_record.version_id, -- Keep original UUID
                legacy_domain_id,
                version_counter,
                'Migrated', -- Functional state for migrated records
                dsl_record.dsl_text,
                'DRAFT', -- Default status for migrated content
                format('Migrated from dsl_ob table. Original CBU: %s', dsl_record.cbu_id),
                'MIGRATION_SCRIPT',
                dsl_record.created_at
            ) ON CONFLICT (version_id) DO NOTHING;

            version_counter := version_counter + 1;
        END LOOP;

        -- Log migration results
        RAISE NOTICE 'Migrated % records from dsl_ob to dsl_versions', version_counter - 1;
    ELSE
        RAISE NOTICE 'No dsl_ob table found - skipping data migration';
    END IF;
END $migration$;

-- ============================================================================
-- STEP 4: CREATE USEFUL VIEWS FOR COMMON QUERIES
-- ============================================================================

-- View for latest version of each domain
CREATE OR REPLACE VIEW "ob-poc".dsl_latest_versions AS
SELECT
    d.domain_name,
    d.description as domain_description,
    dv.version_id,
    dv.version_number,
    dv.functional_state,
    dv.compilation_status,
    dv.change_description,
    dv.created_by,
    dv.created_at,
    CASE WHEN pa.ast_id IS NOT NULL THEN true ELSE false END as has_compiled_ast
FROM "ob-poc".dsl_domains d
JOIN "ob-poc".dsl_versions dv ON d.domain_id = dv.domain_id
LEFT JOIN "ob-poc".parsed_asts pa ON dv.version_id = pa.version_id
WHERE dv.version_number = (
    SELECT MAX(version_number)
    FROM "ob-poc".dsl_versions dv2
    WHERE dv2.domain_id = dv.domain_id
)
AND d.active = true
ORDER BY d.domain_name;

-- View for execution status summary
CREATE OR REPLACE VIEW "ob-poc".dsl_execution_summary AS
SELECT
    d.domain_name,
    dv.version_number,
    dv.compilation_status,
    COUNT(del.execution_id) as total_executions,
    COUNT(CASE WHEN del.status = 'SUCCESS' THEN 1 END) as successful_executions,
    COUNT(CASE WHEN del.status = 'FAILED' THEN 1 END) as failed_executions,
    AVG(del.duration_ms) as avg_duration_ms,
    MAX(del.started_at) as last_execution_at
FROM "ob-poc".dsl_domains d
JOIN "ob-poc".dsl_versions dv ON d.domain_id = dv.domain_id
LEFT JOIN "ob-poc".dsl_execution_log del ON dv.version_id = del.version_id
GROUP BY d.domain_name, dv.version_number, dv.compilation_status
ORDER BY d.domain_name, dv.version_number DESC;

-- ============================================================================
-- STEP 5: CREATE HELPER FUNCTIONS
-- ============================================================================

-- Function to get next version number for a domain
CREATE OR REPLACE FUNCTION "ob-poc".get_next_version_number(domain_name_param VARCHAR)
RETURNS INTEGER AS $$
DECLARE
    next_version INTEGER;
BEGIN
    SELECT COALESCE(MAX(dv.version_number), 0) + 1
    INTO next_version
    FROM "ob-poc".dsl_domains d
    JOIN "ob-poc".dsl_versions dv ON d.domain_id = dv.domain_id
    WHERE d.domain_name = domain_name_param;

    RETURN next_version;
END;
$$ LANGUAGE plpgsql;

-- Function to invalidate AST cache when DSL source changes
CREATE OR REPLACE FUNCTION "ob-poc".invalidate_ast_cache()
RETURNS TRIGGER AS $$
BEGIN
    -- If DSL source code changed, invalidate the AST
    IF OLD.dsl_source_code IS DISTINCT FROM NEW.dsl_source_code THEN
        UPDATE "ob-poc".parsed_asts
        SET invalidated_at = now()
        WHERE version_id = NEW.version_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Ensure trigger can be re-applied idempotently
DROP TRIGGER IF EXISTS trigger_invalidate_ast_cache ON "ob-poc".dsl_versions;
-- Create trigger for AST cache invalidation
CREATE TRIGGER trigger_invalidate_ast_cache
    AFTER UPDATE ON "ob-poc".dsl_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".invalidate_ast_cache();

-- ============================================================================
-- STEP 6: GRANT APPROPRIATE PERMISSIONS
-- ============================================================================

-- Grant permissions to application role (adjust as needed)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA "dsl-ob-poc" TO dsl_app_role;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA "dsl-ob-poc" TO dsl_app_role;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA "dsl-ob-poc" TO dsl_app_role;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;

-- Show summary of what was created
SELECT 'Migration 001 completed successfully' as status;
SELECT
    schemaname,
    tablename,
    tableowner
FROM pg_tables
WHERE schemaname = 'ob-poc'
    AND tablename IN ('dsl_domains', 'dsl_versions', 'parsed_asts', 'dsl_execution_log')
ORDER BY tablename;
