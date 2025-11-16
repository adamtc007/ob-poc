-- ============================================
-- Complete Product-Service-Resource Taxonomy Migration
-- Version: 1.0.0
-- ============================================

BEGIN;

-- Enhance products table
ALTER TABLE "ob-poc".products 
    ADD COLUMN IF NOT EXISTS product_code VARCHAR(50),
    ADD COLUMN IF NOT EXISTS product_category VARCHAR(100),
    ADD COLUMN IF NOT EXISTS regulatory_framework VARCHAR(100),
    ADD COLUMN IF NOT EXISTS min_asset_requirement NUMERIC(20,2),
    ADD COLUMN IF NOT EXISTS is_active BOOLEAN DEFAULT true,
    ADD COLUMN IF NOT EXISTS metadata JSONB;

-- Enhance services table
ALTER TABLE "ob-poc".services 
    ADD COLUMN IF NOT EXISTS service_code VARCHAR(50),
    ADD COLUMN IF NOT EXISTS service_category VARCHAR(100),
    ADD COLUMN IF NOT EXISTS sla_definition JSONB,
    ADD COLUMN IF NOT EXISTS is_active BOOLEAN DEFAULT true;

-- Enhance prod_resources table
ALTER TABLE "ob-poc".prod_resources 
    ADD COLUMN IF NOT EXISTS resource_code VARCHAR(50),
    ADD COLUMN IF NOT EXISTS resource_type VARCHAR(100),
    ADD COLUMN IF NOT EXISTS vendor VARCHAR(255),
    ADD COLUMN IF NOT EXISTS version VARCHAR(50),
    ADD COLUMN IF NOT EXISTS api_endpoint TEXT,
    ADD COLUMN IF NOT EXISTS api_version VARCHAR(20),
    ADD COLUMN IF NOT EXISTS authentication_method VARCHAR(50),
    ADD COLUMN IF NOT EXISTS authentication_config JSONB,
    ADD COLUMN IF NOT EXISTS capabilities JSONB,
    ADD COLUMN IF NOT EXISTS capacity_limits JSONB,
    ADD COLUMN IF NOT EXISTS maintenance_windows JSONB,
    ADD COLUMN IF NOT EXISTS is_active BOOLEAN DEFAULT true;

-- Enhance product_services table
ALTER TABLE "ob-poc".product_services 
    ADD COLUMN IF NOT EXISTS is_mandatory BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS is_default BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS display_order INTEGER,
    ADD COLUMN IF NOT EXISTS configuration JSONB;

-- Service option definitions
CREATE TABLE IF NOT EXISTS "ob-poc".service_option_definitions (
    option_def_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    option_key VARCHAR(100) NOT NULL,
    option_label VARCHAR(255),
    option_type VARCHAR(50) NOT NULL CHECK (option_type IN ('single_select', 'multi_select', 'numeric', 'boolean', 'text')),
    validation_rules JSONB,
    is_required BOOLEAN DEFAULT false,
    display_order INTEGER,
    help_text TEXT,
    UNIQUE(service_id, option_key)
);

-- Service option choices
CREATE TABLE IF NOT EXISTS "ob-poc".service_option_choices (
    choice_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    option_def_id UUID NOT NULL REFERENCES "ob-poc".service_option_definitions(option_def_id) ON DELETE CASCADE,
    choice_value VARCHAR(255) NOT NULL,
    choice_label VARCHAR(255),
    choice_metadata JSONB,
    is_default BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true,
    display_order INTEGER,
    requires_options JSONB,
    excludes_options JSONB,
    UNIQUE(option_def_id, choice_value)
);

-- Service-Resource Capabilities
CREATE TABLE IF NOT EXISTS "ob-poc".service_resource_capabilities (
    capability_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    resource_id UUID NOT NULL REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE,
    supported_options JSONB NOT NULL,
    priority INTEGER DEFAULT 100,
    cost_factor NUMERIC(10,4) DEFAULT 1.0,
    performance_rating INTEGER CHECK (performance_rating BETWEEN 1 AND 5),
    resource_config JSONB,
    is_active BOOLEAN DEFAULT true,
    UNIQUE(service_id, resource_id)
);

-- Resource Attribute Requirements
CREATE TABLE IF NOT EXISTS "ob-poc".resource_attribute_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource_id UUID NOT NULL REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    resource_field_name VARCHAR(255),
    is_mandatory BOOLEAN DEFAULT true,
    transformation_rule JSONB,
    validation_override JSONB,
    UNIQUE(resource_id, attribute_id)
);

-- Onboarding Request Workflow
CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    request_state VARCHAR(50) NOT NULL DEFAULT 'draft' 
        CHECK (request_state IN ('draft', 'products_selected', 'services_discovered', 
                                 'services_configured', 'resources_allocated', 'complete')),
    dsl_draft TEXT,
    dsl_version INTEGER DEFAULT 1,
    current_phase VARCHAR(100),
    phase_metadata JSONB,
    validation_errors JSONB,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_products (
    onboarding_product_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    selection_order INTEGER,
    selected_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(request_id, product_id)
);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_service_configs (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE,
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),
    option_selections JSONB NOT NULL,
    is_valid BOOLEAN DEFAULT false,
    validation_messages JSONB,
    configured_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(request_id, service_id)
);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_resource_allocations (
    allocation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE,
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id),
    resource_id UUID NOT NULL REFERENCES "ob-poc".prod_resources(resource_id),
    handles_options JSONB,
    required_attributes UUID[],
    allocation_status VARCHAR(50) DEFAULT 'pending',
    allocated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".service_discovery_cache (
    discovery_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID REFERENCES "ob-poc".products(product_id),
    discovered_at TIMESTAMPTZ DEFAULT NOW(),
    services_available JSONB,
    resource_availability JSONB,
    ttl_seconds INTEGER DEFAULT 3600
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_products_product_code ON "ob-poc".products(product_code);
CREATE INDEX IF NOT EXISTS idx_products_is_active ON "ob-poc".products(is_active);
CREATE INDEX IF NOT EXISTS idx_services_service_code ON "ob-poc".services(service_code);
CREATE INDEX IF NOT EXISTS idx_services_is_active ON "ob-poc".services(is_active);
CREATE INDEX IF NOT EXISTS idx_prod_resources_resource_code ON "ob-poc".prod_resources(resource_code);
CREATE INDEX IF NOT EXISTS idx_prod_resources_is_active ON "ob-poc".prod_resources(is_active);
CREATE INDEX IF NOT EXISTS idx_service_options_service ON "ob-poc".service_option_definitions(service_id);
CREATE INDEX IF NOT EXISTS idx_option_choices_def ON "ob-poc".service_option_choices(option_def_id);
CREATE INDEX IF NOT EXISTS idx_service_capabilities_service ON "ob-poc".service_resource_capabilities(service_id);
CREATE INDEX IF NOT EXISTS idx_service_capabilities_resource ON "ob-poc".service_resource_capabilities(resource_id);
CREATE INDEX IF NOT EXISTS idx_resource_requirements_resource ON "ob-poc".resource_attribute_requirements(resource_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_request_cbu ON "ob-poc".onboarding_requests(cbu_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_request_state ON "ob-poc".onboarding_requests(request_state);
CREATE INDEX IF NOT EXISTS idx_onboarding_products_request ON "ob-poc".onboarding_products(request_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_configs_request ON "ob-poc".onboarding_service_configs(request_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_allocations_request ON "ob-poc".onboarding_resource_allocations(request_id);

COMMIT;
