-- DSL Persistence Tables for ob-poc
-- These tables store DSL instances, versions, and Abstract Syntax Trees (ASTs)

-- Main table for DSL Instances
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL,  -- e.g. "onboarding", "kyc", "ubo_discovery"
    business_reference VARCHAR(255) NOT NULL,  -- External business identifier (like CBU)
    current_version INTEGER NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'CREATED', -- CREATED, EDITING, COMPILED, FINALIZED, ARCHIVED, FAILED
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    metadata JSONB,
    UNIQUE(domain_name, business_reference)
);

-- Table for versions of a DSL instance
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_instance_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL,
    dsl_content TEXT NOT NULL,
    operation_type VARCHAR(50) NOT NULL, -- CREATE_FROM_TEMPLATE, INCREMENTAL_EDIT, TEMPLATE_ADDITION, MANUAL_EDIT, RECOMPILATION
    compilation_status VARCHAR(50) NOT NULL DEFAULT 'PENDING', -- PENDING, SUCCESS, ERROR
    ast_json JSONB, -- Compiled AST as JSON
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    created_by VARCHAR(255), -- User or system that created this version
    change_description TEXT, -- Description of what changed in this version
    UNIQUE(instance_id, version_number)
);

-- AST node storage for richer querying capabilities
CREATE TABLE IF NOT EXISTS "ob-poc".ast_nodes (
    node_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE,
    parent_node_id UUID REFERENCES "ob-poc".ast_nodes(node_id) ON DELETE CASCADE,
    node_type VARCHAR(100) NOT NULL, -- VERB, ATTRIBUTE, LIST, MAP, VALUE, etc.
    node_key VARCHAR(255), -- For named nodes (attributes, map keys)
    node_value JSONB, -- For leaf nodes
    position_index INTEGER, -- For ordered collections
    depth INTEGER NOT NULL, -- Tree depth for efficient traversal
    path TEXT NOT NULL, -- JSONPath-like path to node
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Create indices for efficient querying
CREATE INDEX idx_dsl_instances_domain ON "ob-poc".dsl_instances(domain_name);
CREATE INDEX idx_dsl_instances_reference ON "ob-poc".dsl_instances(business_reference);
CREATE INDEX idx_dsl_versions_instance ON "ob-poc".dsl_instance_versions(instance_id);
CREATE INDEX idx_dsl_versions_version ON "ob-poc".dsl_instance_versions(instance_id, version_number);
CREATE INDEX idx_ast_nodes_version ON "ob-poc".ast_nodes(version_id);
CREATE INDEX idx_ast_nodes_parent ON "ob-poc".ast_nodes(parent_node_id);
CREATE INDEX idx_ast_nodes_type ON "ob-poc".ast_nodes(node_type);
CREATE INDEX idx_ast_nodes_path ON "ob-poc".ast_nodes(path text_pattern_ops);
CREATE INDEX idx_ast_nodes_depth ON "ob-poc".ast_nodes(version_id, depth);

-- DSL templates for reusable operations
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_name VARCHAR(255) NOT NULL UNIQUE,
    domain_name VARCHAR(100) NOT NULL,
    template_type VARCHAR(100) NOT NULL, -- CREATE_CBU, ADD_PRODUCTS, DISCOVER_SERVICES, etc.
    content TEXT NOT NULL, -- Template content with placeholders
    variables JSONB, -- Metadata about variables that need to be substituted
    requirements JSONB, -- Prerequisites for this template
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Cross-reference between DSL instances and business objects
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_business_references (
    reference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE,
    reference_type VARCHAR(100) NOT NULL, -- CBU, KYC_CASE, ONBOARDING_REQUEST, etc.
    reference_id_value VARCHAR(255) NOT NULL, -- The actual reference ID
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE(instance_id, reference_type, reference_id_value)
);

-- For DSL compilation and execution metrics
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_compilation_logs (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE,
    compilation_start TIMESTAMPTZ NOT NULL,
    compilation_end TIMESTAMPTZ,
    success BOOLEAN,
    error_message TEXT,
    error_location JSONB, -- Line, column, context
    node_count INTEGER,
    complexity_score FLOAT,
    performance_metrics JSONB, -- Detailed compilation performance
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- For linking sub-domain DSL instances to parent instances
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_domain_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_instance_id UUID NOT NULL REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE,
    child_instance_id UUID NOT NULL REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE,
    relationship_type VARCHAR(100) NOT NULL, -- COMPOSITION, REFERENCE, DEPENDENCY, etc.
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE(parent_instance_id, child_instance_id, relationship_type)
);

-- For validation of DSL instances against business rules
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_validations (
    validation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE,
    validation_type VARCHAR(100) NOT NULL, -- SYNTAX, SEMANTIC, BUSINESS_RULES, COMPLIANCE
    validation_success BOOLEAN NOT NULL,
    validation_messages JSONB, -- Array of validation messages
    validated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    validated_by VARCHAR(255)
);

-- Store DSL visualizations
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_visualizations (
    visualization_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE,
    visualization_type VARCHAR(100) NOT NULL, -- AST, DOMAIN_ENHANCED, WORKFLOW, UBO_GRAPH, etc.
    visualization_data JSONB NOT NULL, -- The complete visualization structure
    options_used JSONB, -- Visualization options that were applied
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

COMMENT ON TABLE "ob-poc".dsl_instances IS 'Main registry of DSL instances across all domains';
COMMENT ON TABLE "ob-poc".dsl_instance_versions IS 'Version history for DSL instances, storing content and compiled AST';
COMMENT ON TABLE "ob-poc".ast_nodes IS 'Hierarchical storage of AST nodes for efficient querying and traversal';
COMMENT ON TABLE "ob-poc".dsl_templates IS 'Reusable DSL templates for standard operations';
COMMENT ON TABLE "ob-poc".dsl_business_references IS 'Links between DSL instances and business objects';
COMMENT ON TABLE "ob-poc".dsl_compilation_logs IS 'Detailed logs of DSL compilation processes';
COMMENT ON TABLE "ob-poc".dsl_domain_relationships IS 'Cross-domain relationships between DSL instances';
COMMENT ON TABLE "ob-poc".dsl_validations IS 'Validation results for DSL instance versions';
COMMENT ON TABLE "ob-poc".dsl_visualizations IS 'Storage for generated visualizations of DSL instances';
