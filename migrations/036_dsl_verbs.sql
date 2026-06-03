-- DSL Verbs Registry
-- Persistent verb metadata for RAG discovery and agent context
-- Synced from YAML on startup with hash-based change detection
--
-- IDEMPOTENT: This migration can be run multiple times safely.

-- =============================================================================
-- VERB DEFINITIONS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_verbs (
    verb_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain          TEXT NOT NULL,
    verb_name       TEXT NOT NULL,
    full_name       TEXT GENERATED ALWAYS AS (domain || '.' || verb_name) STORED,
    description     TEXT,

    -- Behavior type
    behavior        TEXT NOT NULL DEFAULT 'crud',  -- 'crud', 'plugin', 'graph_query'

    -- RAG metadata for agent discovery
    category        TEXT,                          -- 'entity_management', 'kyc_workflow', etc.
    search_text     TEXT,                          -- semantic search corpus (description + patterns)
    intent_patterns TEXT[],                        -- natural language patterns that match this verb
    workflow_phases TEXT[],                        -- which KYC phases this verb applies to
    graph_contexts  TEXT[],                        -- graph positions where verb is relevant

    -- Example for agent suggestions
    example_short   TEXT,                          -- brief description of example
    example_dsl     TEXT,                          -- actual DSL example

    -- Suggested next verbs (workflow hints)
    typical_next    TEXT[],                        -- verbs that often follow this one

    -- Dataflow metadata (from produces/consumes)
    produces_type   TEXT,                          -- entity type this verb creates
    produces_subtype TEXT,                         -- subtype if applicable
    consumes        JSONB DEFAULT '[]',            -- [{binding_type, required, arg_name}]

    -- Lifecycle constraints
    lifecycle_entity_arg TEXT,                     -- argument containing entity ID for lifecycle constraints
    requires_states TEXT[],                        -- valid pre-states for execution
    transitions_to  TEXT,                          -- state after execution

    -- Source tracking for sync
    source          TEXT NOT NULL DEFAULT 'yaml', -- 'yaml', 'dynamic', 'user_defined'
    yaml_hash       TEXT,                          -- SHA256 of YAML definition for drift detection

    -- Timestamps
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE(domain, verb_name)
);

-- Idempotent column renames: handle migration from old column names
-- These DO NOTHING if the old columns don't exist (already migrated)
DO $$
BEGIN
    -- Rename lifecycle_entity_type -> lifecycle_entity_arg if old name exists
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc' AND table_name = 'dsl_verbs'
        AND column_name = 'lifecycle_entity_type'
    ) THEN
        ALTER TABLE "ob-poc".dsl_verbs RENAME COLUMN lifecycle_entity_type TO lifecycle_entity_arg;
    END IF;

    -- Rename target_state -> transitions_to if old name exists
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc' AND table_name = 'dsl_verbs'
        AND column_name = 'target_state'
    ) THEN
        ALTER TABLE "ob-poc".dsl_verbs RENAME COLUMN target_state TO transitions_to;
    END IF;
END $$;

-- Ensure columns exist with correct names (for fresh installs)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc' AND table_name = 'dsl_verbs'
        AND column_name = 'lifecycle_entity_arg'
    ) THEN
        ALTER TABLE "ob-poc".dsl_verbs ADD COLUMN lifecycle_entity_arg TEXT;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc' AND table_name = 'dsl_verbs'
        AND column_name = 'transitions_to'
    ) THEN
        ALTER TABLE "ob-poc".dsl_verbs ADD COLUMN transitions_to TEXT;
    END IF;
END $$;

-- Full-text search index on search_text
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_search
    ON "ob-poc".dsl_verbs USING gin(to_tsvector('english', coalesce(search_text, '')));

-- Index for category filtering
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_category
    ON "ob-poc".dsl_verbs(category);

-- Index for workflow phase queries
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_workflow
    ON "ob-poc".dsl_verbs USING gin(workflow_phases);

-- Index for graph context queries
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_graph_ctx
    ON "ob-poc".dsl_verbs USING gin(graph_contexts);

-- Index for domain queries
CREATE INDEX IF NOT EXISTS idx_dsl_verbs_domain
    ON "ob-poc".dsl_verbs(domain);

-- =============================================================================
-- VERB CATEGORIES (Reference data for grouping)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_verb_categories (
    category_code   TEXT PRIMARY KEY,
    label           TEXT NOT NULL,
    description     TEXT,
    display_order   INTEGER DEFAULT 100,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed categories (idempotent via ON CONFLICT)
INSERT INTO "ob-poc".dsl_verb_categories (category_code, label, description, display_order)
VALUES
    ('entity_management', 'Entity Management', 'Create, update, and manage legal entities', 10),
    ('cbu_operations', 'CBU Operations', 'Client Business Unit lifecycle and role management', 20),
    ('ownership_control', 'Ownership & Control', 'Ownership chains, UBO determination, and control relationships', 30),
    ('document_management', 'Document Management', 'Document catalog, extraction, and requests', 40),
    ('kyc_workflow', 'KYC Workflow', 'KYC case management, workstreams, and approvals', 50),
    ('screening', 'Screening', 'Sanctions, PEP, and adverse media screening', 60),
    ('graph_visualization', 'Graph Visualization', 'View and navigate entity graphs', 70),
    ('custody_settlement', 'Custody & Settlement', 'Trading universe, SSIs, and booking rules', 80),
    ('products_services', 'Products & Services', 'Product assignment and service resource management', 90),
    ('fund_structure', 'Fund Structure', 'Fund hierarchy, share classes, and investor registry', 100),
    ('verification', 'Verification', 'Adversarial verification and confidence scoring', 110),
    ('reference_data', 'Reference Data', 'Static reference data management', 120)
ON CONFLICT (category_code) DO UPDATE SET
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    display_order = EXCLUDED.display_order;

-- =============================================================================
-- WORKFLOW PHASES (Reference data for KYC lifecycle)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_workflow_phases (
    phase_code      TEXT PRIMARY KEY,
    label           TEXT NOT NULL,
    description     TEXT,
    phase_order     INTEGER NOT NULL,
    transitions_to  TEXT[],  -- valid next phases
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed workflow phases (idempotent via ON CONFLICT)
INSERT INTO "ob-poc".dsl_workflow_phases (phase_code, label, description, phase_order, transitions_to)
VALUES
    ('intake', 'Intake', 'Initial client data collection', 10, ARRAY['entity_collection']),
    ('entity_collection', 'Entity Collection', 'Gathering all related entities and roles', 20, ARRAY['screening']),
    ('screening', 'Screening', 'Running sanctions, PEP, and adverse media checks', 30, ARRAY['document_collection']),
    ('document_collection', 'Document Collection', 'Collecting and verifying required documents', 40, ARRAY['ubo_determination']),
    ('ubo_determination', 'UBO Determination', 'Calculating and verifying beneficial ownership', 50, ARRAY['review']),
    ('review', 'Review', 'Final review and decision', 60, ARRAY['approved', 'rejected']),
    ('approved', 'Approved', 'Client approved, ready for products', 70, NULL),
    ('rejected', 'Rejected', 'Client rejected', 80, NULL)
ON CONFLICT (phase_code) DO UPDATE SET
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    phase_order = EXCLUDED.phase_order,
    transitions_to = EXCLUDED.transitions_to;

-- =============================================================================
-- GRAPH CONTEXTS (Reference data for cursor positions)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_graph_contexts (
    context_code    TEXT PRIMARY KEY,
    label           TEXT NOT NULL,
    description     TEXT,
    priority        INTEGER DEFAULT 50,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed graph contexts (idempotent via ON CONFLICT)
INSERT INTO "ob-poc".dsl_graph_contexts (context_code, label, description, priority)
VALUES
    ('cursor_on_cbu', 'Cursor on CBU', 'When cursor is on a CBU node', 100),
    ('cursor_on_entity', 'Cursor on Entity', 'When cursor is on an entity (person or company)', 95),
    ('cursor_on_person', 'Cursor on Person', 'When cursor is on a natural person', 90),
    ('cursor_on_company', 'Cursor on Company', 'When cursor is on a company entity', 90),
    ('cursor_on_ownership', 'Cursor on Ownership', 'When cursor is on an ownership edge', 85),
    ('cursor_on_product', 'Cursor on Product', 'When cursor is on a product node', 80),
    ('layer_ubo', 'UBO Layer Active', 'When viewing UBO layer', 90),
    ('layer_kyc', 'KYC Layer Active', 'When viewing KYC layer', 90),
    ('layer_services', 'Services Layer Active', 'When viewing service layer', 85),
    ('layer_custody', 'Custody Layer Active', 'When viewing custody layer', 85)
ON CONFLICT (context_code) DO UPDATE SET
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    priority = EXCLUDED.priority;

-- =============================================================================
-- VERB SYNC LOG (Track sync history)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_verb_sync_log (
    sync_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    synced_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    verbs_added     INTEGER NOT NULL DEFAULT 0,
    verbs_updated   INTEGER NOT NULL DEFAULT 0,
    verbs_unchanged INTEGER NOT NULL DEFAULT 0,
    verbs_removed   INTEGER NOT NULL DEFAULT 0,
    source_hash     TEXT,  -- overall hash of all YAML files
    duration_ms     INTEGER,
    error_message   TEXT
);

-- =============================================================================
-- HELPER FUNCTION: Update search_text from other fields
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_verb_search_text()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_text := coalesce(NEW.description, '') || ' ' ||
                       coalesce(array_to_string(NEW.intent_patterns, ' '), '') || ' ' ||
                       coalesce(NEW.example_short, '');
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-update search_text (idempotent via DROP IF EXISTS)
DROP TRIGGER IF EXISTS trg_verb_search_text ON "ob-poc".dsl_verbs;
CREATE TRIGGER trg_verb_search_text
    BEFORE INSERT OR UPDATE ON "ob-poc".dsl_verbs
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_verb_search_text();

-- =============================================================================
-- VIEW: Verb discovery with category info
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_verb_discovery AS
SELECT
    v.verb_id,
    v.domain,
    v.verb_name,
    v.full_name,
    v.description,
    v.behavior,
    v.category,
    c.label as category_label,
    v.intent_patterns,
    v.workflow_phases,
    v.graph_contexts,
    v.example_short,
    v.example_dsl,
    v.typical_next,
    v.produces_type,
    v.consumes,
    v.source,
    v.updated_at
FROM "ob-poc".dsl_verbs v
LEFT JOIN "ob-poc".dsl_verb_categories c ON v.category = c.category_code
ORDER BY c.display_order NULLS LAST, v.domain, v.verb_name;

COMMENT ON TABLE "ob-poc".dsl_verbs IS 'DSL verb definitions synced from YAML, with RAG metadata for agent discovery';
COMMENT ON TABLE "ob-poc".dsl_verb_categories IS 'Verb category reference data for grouping';
COMMENT ON TABLE "ob-poc".dsl_workflow_phases IS 'KYC workflow phase reference data';
COMMENT ON TABLE "ob-poc".dsl_graph_contexts IS 'Graph cursor context reference data';
