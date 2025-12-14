-- Workflow Definitions Cache
-- Stores workflow definitions loaded from YAML for fast querying

CREATE TABLE IF NOT EXISTS "ob-poc".workflow_definitions (
    workflow_id VARCHAR(100) PRIMARY KEY,
    version INTEGER NOT NULL,
    description TEXT,
    definition_json JSONB NOT NULL,
    content_hash VARCHAR(64) NOT NULL,  -- SHA-256 of YAML content
    loaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_workflow_version UNIQUE (workflow_id, version)
);

-- Index for listing workflows by load time
CREATE INDEX IF NOT EXISTS idx_workflow_defs_loaded
ON "ob-poc".workflow_definitions (loaded_at DESC);

-- View for easy querying
CREATE OR REPLACE VIEW "ob-poc".v_workflow_summary AS
SELECT
    workflow_id,
    version,
    description,
    jsonb_object_keys(definition_json->'states') as state_count,
    jsonb_array_length(definition_json->'transitions') as transition_count,
    loaded_at
FROM "ob-poc".workflow_definitions;

COMMENT ON TABLE "ob-poc".workflow_definitions IS 'Cached workflow definitions loaded from YAML files on startup';
