-- ============================================
-- OB-POC Master Schema - Consolidated
-- ============================================
-- Version: 3.0 (Consolidated from production database)
-- Date: 2025-11-16
-- Description: Complete schema for ob-poc including all production tables
--
-- This file represents the CURRENT STATE of the database schema
-- Generated from live database and manually organized for clarity
-- ============================================

BEGIN;

-- Create schema if not exists
CREATE SCHEMA IF NOT EXISTS "ob-poc";

-- ============================================
-- SECTION 1: CORE CBU (Client Business Unit)
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbus (
    cbu_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    nature_purpose TEXT,
    source_of_funds TEXT,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_cbus_name ON "ob-poc".cbus(name);

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_creation_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    created_by VARCHAR(255),
    creation_method VARCHAR(100),
    ai_model_used VARCHAR(100),
    processing_time_ms INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".roles (
    role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_entity_roles (
    cbu_entity_role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE,
    role_start_date DATE,
    role_end_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(cbu_id, entity_id, role_id)
);

-- ============================================
-- SECTION 2: ATTRIBUTE DICTIONARY & VALUES
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    semantic_id VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    data_type VARCHAR(50) NOT NULL,
    validation_rules JSONB,
    pii_classification VARCHAR(50),
    business_domain VARCHAR(100),
    description TEXT,
    source_system VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_dictionary_semantic_id ON "ob-poc".dictionary(semantic_id);
CREATE INDEX IF NOT EXISTS idx_dictionary_domain ON "ob-poc".dictionary(business_domain);

CREATE TABLE IF NOT EXISTS "ob-poc".attribute_registry (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    semantic_id VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    data_type VARCHAR(50) NOT NULL,
    validation_schema JSONB,
    pii_level VARCHAR(50),
    domain VARCHAR(100),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values (
    value_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    value_text TEXT,
    value_numeric NUMERIC,
    value_date DATE,
    value_boolean BOOLEAN,
    value_json JSONB,
    source VARCHAR(255),
    confidence_score NUMERIC(5,2),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_attribute_values_cbu ON "ob-poc".attribute_values(cbu_id);
CREATE INDEX IF NOT EXISTS idx_attribute_values_attribute ON "ob-poc".attribute_values(attribute_id);

CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values_typed (
    value_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    value_text TEXT,
    value_numeric NUMERIC,
    value_date DATE,
    value_boolean BOOLEAN,
    value_json JSONB,
    source_type VARCHAR(100),
    source_reference VARCHAR(255),
    confidence_score NUMERIC(5,2),
    extraction_method VARCHAR(100),
    verified BOOLEAN DEFAULT false,
    verified_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CHECK ((cbu_id IS NOT NULL) OR (entity_id IS NOT NULL))
);

-- ============================================
-- SECTION 3: ENTITIES & ENTITY TYPES
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_types (
    entity_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(50) UNIQUE NOT NULL,
    type_name VARCHAR(255) NOT NULL,
    description TEXT,
    is_active BOOLEAN DEFAULT true
);

CREATE TABLE IF NOT EXISTS "ob-poc".entities (
    entity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type_id UUID NOT NULL REFERENCES "ob-poc".entity_types(entity_type_id),
    name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(10),
    registration_number VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON "ob-poc".entities(entity_type_id);
CREATE INDEX IF NOT EXISTS idx_entities_name ON "ob-poc".entities(name);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_proper_persons (
    person_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL UNIQUE REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    first_name VARCHAR(100),
    middle_name VARCHAR(100),
    last_name VARCHAR(100),
    date_of_birth DATE,
    nationality VARCHAR(10),
    tax_id VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_limited_companies (
    company_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL UNIQUE REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    incorporation_date DATE,
    company_number VARCHAR(100),
    registered_office_address TEXT,
    share_capital NUMERIC(20,2),
    currency VARCHAR(3),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_partnerships (
    partnership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL UNIQUE REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    partnership_type VARCHAR(50),
    formation_date DATE,
    partnership_agreement_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".partnership_interests (
    interest_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE,
    partner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    interest_percentage NUMERIC(5,2),
    interest_class VARCHAR(100),
    capital_contribution NUMERIC(20,2),
    effective_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".partnership_control_mechanisms (
    mechanism_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE,
    mechanism_type VARCHAR(100),
    description TEXT,
    controlling_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_trusts (
    trust_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL UNIQUE REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    trust_type VARCHAR(50),
    formation_date DATE,
    governing_law VARCHAR(50),
    trust_deed_date DATE,
    is_revocable BOOLEAN,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".trust_parties (
    party_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE,
    party_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    party_role VARCHAR(50),
    appointment_date DATE,
    removal_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".trust_beneficiary_classes (
    class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE,
    class_name VARCHAR(255),
    class_description TEXT,
    distribution_rules JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".trust_protector_powers (
    power_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE,
    power_description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_role_connections (
    connection_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    relationship_type VARCHAR(100),
    ownership_percentage NUMERIC(5,2),
    control_type VARCHAR(100),
    effective_date DATE,
    end_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_lifecycle_status (
    status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL,
    status_date TIMESTAMPTZ NOT NULL,
    status_reason TEXT,
    changed_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_crud_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type_id UUID NOT NULL REFERENCES "ob-poc".entity_types(entity_type_id),
    operation VARCHAR(20) NOT NULL,
    required_attributes JSONB,
    validation_rules JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_validation_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type_id UUID NOT NULL REFERENCES "ob-poc".entity_types(entity_type_id),
    field_name VARCHAR(255) NOT NULL,
    validation_type VARCHAR(50),
    validation_params JSONB,
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- SECTION 4: UBO (Ultimate Beneficial Ownership)
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_registry (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    total_ownership_percentage NUMERIC(5,2),
    control_mechanism VARCHAR(100),
    is_direct BOOLEAN,
    ownership_path JSONB,
    calculated_at TIMESTAMPTZ DEFAULT NOW(),
    verified BOOLEAN DEFAULT false,
    verified_by VARCHAR(255),
    verified_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_ubo_registry_cbu ON "ob-poc".ubo_registry(cbu_id);

-- ============================================
-- SECTION 5: PRODUCTS & SERVICES
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".products (
    product_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    product_code VARCHAR(50) UNIQUE,
    product_category VARCHAR(100),
    regulatory_framework VARCHAR(100),
    min_asset_requirement NUMERIC(20,2),
    is_active BOOLEAN DEFAULT true,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_products_name ON "ob-poc".products(name);
CREATE INDEX IF NOT EXISTS idx_products_product_code ON "ob-poc".products(product_code);
CREATE INDEX IF NOT EXISTS idx_products_is_active ON "ob-poc".products(is_active);

CREATE TABLE IF NOT EXISTS "ob-poc".services (
    service_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    service_code VARCHAR(50) UNIQUE,
    service_category VARCHAR(100),
    sla_definition JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_services_name ON "ob-poc".services(name);
CREATE INDEX IF NOT EXISTS idx_services_service_code ON "ob-poc".services(service_code);
CREATE INDEX IF NOT EXISTS idx_services_is_active ON "ob-poc".services(is_active);

CREATE TABLE IF NOT EXISTS "ob-poc".product_services (
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE,
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    is_mandatory BOOLEAN DEFAULT false,
    is_default BOOLEAN DEFAULT false,
    display_order INTEGER,
    configuration JSONB,
    PRIMARY KEY (product_id, service_id)
);

CREATE TABLE IF NOT EXISTS "ob-poc".product_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    is_mandatory BOOLEAN DEFAULT true,
    validation_rules JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".product_workflows (
    workflow_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    workflow_name VARCHAR(255),
    workflow_definition JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entity_product_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE,
    mapped_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_id, product_id)
);

-- ============================================
-- SECTION 6: PRODUCTION RESOURCES
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".prod_resources (
    resource_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    owner VARCHAR(255) NOT NULL,
    dictionary_group VARCHAR(100),
    resource_code VARCHAR(50) UNIQUE,
    resource_type VARCHAR(100),
    vendor VARCHAR(255),
    version VARCHAR(50),
    api_endpoint TEXT,
    api_version VARCHAR(20),
    authentication_method VARCHAR(50),
    authentication_config JSONB,
    capabilities JSONB,
    capacity_limits JSONB,
    maintenance_windows JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_prod_resources_name ON "ob-poc".prod_resources(name);
CREATE INDEX IF NOT EXISTS idx_prod_resources_owner ON "ob-poc".prod_resources(owner);
CREATE INDEX IF NOT EXISTS idx_prod_resources_dict_group ON "ob-poc".prod_resources(dictionary_group);
CREATE INDEX IF NOT EXISTS idx_prod_resources_resource_code ON "ob-poc".prod_resources(resource_code);
CREATE INDEX IF NOT EXISTS idx_prod_resources_is_active ON "ob-poc".prod_resources(is_active);

CREATE TABLE IF NOT EXISTS "ob-poc".service_resources (
    service_id UUID NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    resource_id UUID NOT NULL REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE,
    PRIMARY KEY (service_id, resource_id)
);

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

CREATE INDEX IF NOT EXISTS idx_service_capabilities_service ON "ob-poc".service_resource_capabilities(service_id);
CREATE INDEX IF NOT EXISTS idx_service_capabilities_resource ON "ob-poc".service_resource_capabilities(resource_id);

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

CREATE INDEX IF NOT EXISTS idx_resource_requirements_resource ON "ob-poc".resource_attribute_requirements(resource_id);
CREATE INDEX IF NOT EXISTS idx_resource_requirements_attribute ON "ob-poc".resource_attribute_requirements(attribute_id);

-- ============================================
-- SECTION 7: SERVICE OPTIONS
-- ============================================

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

CREATE INDEX IF NOT EXISTS idx_service_options_service ON "ob-poc".service_option_definitions(service_id);

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

CREATE INDEX IF NOT EXISTS idx_option_choices_def ON "ob-poc".service_option_choices(option_def_id);

-- ============================================
-- SECTION 8: ONBOARDING WORKFLOW
-- ============================================

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

CREATE INDEX IF NOT EXISTS idx_onboarding_request_cbu ON "ob-poc".onboarding_requests(cbu_id);
CREATE INDEX IF NOT EXISTS idx_onboarding_request_state ON "ob-poc".onboarding_requests(request_state);

CREATE TABLE IF NOT EXISTS "ob-poc".onboarding_products (
    onboarding_product_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    selection_order INTEGER,
    selected_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(request_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_onboarding_products_request ON "ob-poc".onboarding_products(request_id);

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

CREATE INDEX IF NOT EXISTS idx_onboarding_configs_request ON "ob-poc".onboarding_service_configs(request_id);

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

CREATE INDEX IF NOT EXISTS idx_onboarding_allocations_request ON "ob-poc".onboarding_resource_allocations(request_id);

CREATE TABLE IF NOT EXISTS "ob-poc".service_discovery_cache (
    discovery_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID REFERENCES "ob-poc".products(product_id),
    discovered_at TIMESTAMPTZ DEFAULT NOW(),
    services_available JSONB,
    resource_availability JSONB,
    ttl_seconds INTEGER DEFAULT 3600
);

-- ============================================
-- SECTION 9: DOCUMENTS
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_types (
    document_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(50) UNIQUE NOT NULL,
    type_name VARCHAR(255) NOT NULL,
    description TEXT,
    required_attributes JSONB,
    retention_period_days INTEGER,
    is_active BOOLEAN DEFAULT true
);

CREATE TABLE IF NOT EXISTS "ob-poc".document_catalog (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(document_type_id),
    document_name VARCHAR(255) NOT NULL,
    file_path TEXT,
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),
    upload_date TIMESTAMPTZ DEFAULT NOW(),
    issuer VARCHAR(255),
    issue_date DATE,
    expiry_date DATE,
    status VARCHAR(50) DEFAULT 'active',
    verification_status VARCHAR(50),
    verified_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    metadata JSONB
);

CREATE INDEX IF NOT EXISTS idx_document_catalog_cbu ON "ob-poc".document_catalog(cbu_id);
CREATE INDEX IF NOT EXISTS idx_document_catalog_type ON "ob-poc".document_catalog(document_type_id);

CREATE TABLE IF NOT EXISTS "ob-poc".document_issuers_backup (
    issuer_id UUID PRIMARY KEY,
    issuer_name VARCHAR(255),
    issuer_type VARCHAR(100),
    jurisdiction VARCHAR(10),
    created_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS "ob-poc".document_metadata (
    metadata_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(document_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    extracted_value TEXT,
    confidence_score NUMERIC(5,2),
    extraction_method VARCHAR(100),
    extracted_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(document_id, attribute_id)
);

CREATE TABLE IF NOT EXISTS "ob-poc".document_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(document_id) ON DELETE CASCADE,
    target_document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(document_id) ON DELETE CASCADE,
    relationship_type VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".document_attribute_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(document_type_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE,
    field_name VARCHAR(255),
    extraction_pattern TEXT,
    is_required BOOLEAN DEFAULT false,
    default_value TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(document_type_id, attribute_id)
);

-- ============================================
-- SECTION 10: DSL MANAGEMENT
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_domains (
    domain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) UNIQUE NOT NULL,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains(domain_id),
    version_number INTEGER NOT NULL,
    dsl_content TEXT NOT NULL,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(domain_id, version_number)
);

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains(domain_id),
    instance_name VARCHAR(255),
    dsl_content TEXT NOT NULL,
    status VARCHAR(50) DEFAULT 'draft',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".parsed_asts (
    ast_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".dsl_instances(instance_id),
    ast_json JSONB NOT NULL,
    parsed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_execution_log (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID REFERENCES "ob-poc".dsl_instances(instance_id),
    execution_status VARCHAR(50),
    execution_result JSONB,
    error_message TEXT,
    executed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_examples (
    example_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains(domain_id),
    example_name VARCHAR(255),
    dsl_content TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_ob (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255),
    content TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- SECTION 11: VOCABULARIES & GRAMMAR
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".domain_vocabularies (
    vocab_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    description TEXT,
    parameter_schema JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(domain_name, verb)
);

CREATE TABLE IF NOT EXISTS "ob-poc".verb_registry (
    verb_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    verb_name VARCHAR(100) UNIQUE NOT NULL,
    domain VARCHAR(100),
    description TEXT,
    parameter_schema JSONB,
    is_active BOOLEAN DEFAULT true
);

CREATE TABLE IF NOT EXISTS "ob-poc".vocabulary_audit (
    audit_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vocab_id UUID REFERENCES "ob-poc".domain_vocabularies(vocab_id),
    change_type VARCHAR(50),
    old_value JSONB,
    new_value JSONB,
    changed_by VARCHAR(255),
    changed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".grammar_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_name VARCHAR(255) NOT NULL,
    rule_pattern TEXT NOT NULL,
    rule_type VARCHAR(50),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- SECTION 12: ORCHESTRATION
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_sessions (
    session_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    session_type VARCHAR(100),
    status VARCHAR(50) DEFAULT 'active',
    context JSONB,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_domain_sessions (
    domain_session_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains(domain_id),
    sequence_number INTEGER,
    status VARCHAR(50),
    started_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    task_type VARCHAR(100),
    task_payload JSONB,
    status VARCHAR(50) DEFAULT 'pending',
    result JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_state_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    state_snapshot JSONB,
    recorded_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- SECTION 13: MASTER REFERENCE DATA
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".master_jurisdictions (
    jurisdiction_code VARCHAR(10) PRIMARY KEY,
    jurisdiction_name VARCHAR(255) NOT NULL,
    region VARCHAR(100),
    regulatory_framework VARCHAR(255),
    tax_treaty_countries TEXT[],
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".master_entity_xref (
    xref_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    internal_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    external_system VARCHAR(100),
    external_entity_id VARCHAR(255),
    sync_status VARCHAR(50),
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(internal_entity_id, external_system)
);

-- ============================================
-- SECTION 14: CRUD OPERATIONS LOG
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".crud_operations (
    operation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(100),
    entity_id UUID,
    operation_type VARCHAR(20),
    operation_payload JSONB,
    operation_result JSONB,
    performed_by VARCHAR(255),
    performed_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- SECTION 15: MISCELLANEOUS
-- ============================================

CREATE TABLE IF NOT EXISTS "ob-poc".rag_embeddings (
    embedding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content TEXT NOT NULL,
    embedding vector(1536),
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".schema_changes (
    change_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    change_description TEXT,
    applied_at TIMESTAMPTZ DEFAULT NOW(),
    applied_by VARCHAR(255)
);

COMMIT;

-- ============================================
-- END OF MASTER SCHEMA
-- ============================================
