-- Migration: Resource Dependencies and Onboarding Workflow
-- Date: 2024-12-13
-- Description: Adds Terraform-like resource provisioning with dependency graph support

-- =============================================================================
-- PHASE 1: Resource Dependency Schema
-- =============================================================================

-- Track which resource types depend on other resource types
CREATE TABLE IF NOT EXISTS "ob-poc".resource_dependencies (
    dependency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource_type_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    depends_on_type_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),

    -- Dependency metadata
    dependency_type VARCHAR(20) DEFAULT 'required'
        CHECK (dependency_type IN ('required', 'optional', 'conditional')),

    -- Which argument receives the dependency's URL
    inject_arg VARCHAR(100) NOT NULL,

    -- For conditional dependencies
    condition_expression TEXT,  -- e.g., "product.has_feature('multi_currency')"

    -- Ordering hint for same-level dependencies
    priority INTEGER DEFAULT 100,

    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),

    CONSTRAINT no_self_dependency CHECK (resource_type_id != depends_on_type_id),
    UNIQUE(resource_type_id, depends_on_type_id)
);

CREATE INDEX IF NOT EXISTS idx_resource_deps_type ON "ob-poc".resource_dependencies(resource_type_id);
CREATE INDEX IF NOT EXISTS idx_resource_deps_on ON "ob-poc".resource_dependencies(depends_on_type_id);

COMMENT ON TABLE "ob-poc".resource_dependencies IS
'Resource type dependencies for onboarding. E.g., custody_account depends on cash_account.
The inject_arg specifies which provisioning argument receives the dependency URL.';

-- Track actual instance-level dependencies (post-provisioning)
CREATE TABLE IF NOT EXISTS "ob-poc".resource_instance_dependencies (
    instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    depends_on_instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    dependency_type VARCHAR(20) DEFAULT 'required',
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (instance_id, depends_on_instance_id)
);

-- =============================================================================
-- PHASE 3: Onboarding Workflow Tables
-- =============================================================================

-- Store generated plans
CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_plans (
    plan_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    products TEXT[] NOT NULL,
    generated_dsl TEXT NOT NULL,
    dependency_graph JSONB NOT NULL,
    resource_count INTEGER NOT NULL,
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'modified', 'validated', 'executing', 'complete', 'failed')),
    attribute_overrides JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ DEFAULT (now() + interval '24 hours')
);

CREATE INDEX IF NOT EXISTS idx_onboarding_plans_cbu ON "ob-poc".onboarding_plans(cbu_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_plans_status ON "ob-poc".onboarding_plans(status);

-- Track execution
CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_executions (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES "ob-poc".onboarding_plans(plan_id),
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'complete', 'failed', 'cancelled')),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    result_urls JSONB
);

CREATE INDEX IF NOT EXISTS idx_onboarding_executions_plan ON "ob-poc".onboarding_executions(plan_id);

-- Track per-resource provisioning tasks
CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id UUID NOT NULL REFERENCES "ob-poc".onboarding_executions(execution_id),
    resource_code VARCHAR(50) NOT NULL,
    resource_instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    stage INTEGER NOT NULL,  -- Parallel execution stage
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'complete', 'failed', 'skipped')),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_onboarding_tasks_exec ON "ob-poc".onboarding_tasks(execution_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_tasks_status ON "ob-poc".onboarding_tasks(status);
