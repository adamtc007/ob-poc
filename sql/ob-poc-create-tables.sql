-- ============================================================================
-- OB-POC COMPLETE SCHEMA - CONSOLIDATED CREATE TABLES
-- ============================================================================
--
-- Comprehensive CREATE TABLE statements for the ob-poc Ultimate Beneficial
-- Ownership and Onboarding system with DSL-as-State architecture.
--
-- Generated: 2025-11-11
-- Schema: "ob-poc" (Canonical PostgreSQL schema)
-- Architecture: DSL-as-State + AttributeID-as-Type + AI Integration
-- ============================================================================

-- Create the main schema
CREATE SCHEMA IF NOT EXISTS "ob-poc";

-- ============================================================================
-- CORE BUSINESS TABLES
-- ============================================================================

-- Client Business Units (CBUs) - Core entity registry
CREATE TABLE IF NOT EXISTS "ob-poc".cbus (
    cbu_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    nature_purpose TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Products - Core product definitions
CREATE TABLE IF NOT EXISTS "ob-poc".products (
    product_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Services - Services that can be offered with or without products
CREATE TABLE IF NOT EXISTS "ob-poc".services (
    service_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Product <-> Service Join Table
CREATE TABLE IF NOT EXISTS "ob-poc".product_services (
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE,
    service_id UUID NOT NULL REFERENCES "ob-poc".services (service_id) ON DELETE CASCADE,
    PRIMARY KEY (product_id, service_id)
);

-- ============================================================================
-- DSL INFRASTRUCTURE
-- ============================================================================

-- DSL Storage - Immutable, versioned DSL files
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_ob (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id VARCHAR(255) NOT NULL,
    dsl_text TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- DSL Domain Registry
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_domains (
    domain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    base_grammar_version VARCHAR(20) DEFAULT '1.0.0',
    vocabulary_version VARCHAR(20) DEFAULT '1.0.0',
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- DSL Version Management
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains(domain_id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL,
    functional_state VARCHAR(100),
    dsl_source_code TEXT NOT NULL,
    compilation_status VARCHAR(50) DEFAULT 'DRAFT',
    change_description TEXT,
    parent_version_id UUID REFERENCES "ob-poc".dsl_versions(version_id),
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    compiled_at TIMESTAMPTZ,
    activated_at TIMESTAMPTZ,
    UNIQUE (domain_id, version_number)
);

-- Parsed Abstract Syntax Trees
CREATE TABLE IF NOT EXISTS "ob-poc".parsed_asts (
    ast_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL UNIQUE REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE,
    ast_json JSONB NOT NULL,
    parse_metadata JSONB,
    grammar_version VARCHAR(20) NOT NULL,
    parser_version VARCHAR(20) NOT NULL,
    ast_hash VARCHAR(64),
    node_count INTEGER,
    complexity_score NUMERIC(10,2),
    parsed_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    invalidated_at TIMESTAMPTZ
);

-- DSL Execution Logging
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_execution_log (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE,
    cbu_id VARCHAR(255),
    execution_phase VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    result_data JSONB,
    error_details JSONB,
    performance_metrics JSONB,
    executed_by VARCHAR(255),
    started_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    completed_at TIMESTAMPTZ,
    duration_ms INTEGER GENERATED ALWAYS AS (
        CASE
            WHEN completed_at IS NOT NULL THEN EXTRACT(epoch FROM completed_at - started_at) * 1000
            ELSE NULL
        END
    ) STORED
);

-- ============================================================================
-- GRAMMAR AND VOCABULARY MANAGEMENT
-- ============================================================================

-- DSL Grammar Rules - Database-stored EBNF grammar definitions
CREATE TABLE IF NOT EXISTS "ob-poc".grammar_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_name VARCHAR(100) NOT NULL UNIQUE,
    rule_definition TEXT NOT NULL,
    rule_type VARCHAR(50) NOT NULL DEFAULT 'production',
    domain VARCHAR(100),
    version VARCHAR(20) DEFAULT '1.0.0',
    active BOOLEAN DEFAULT true,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Domain Vocabularies - Database-stored DSL verbs by domain
CREATE TABLE IF NOT EXISTS "ob-poc".domain_vocabularies (
    vocab_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    category VARCHAR(50),
    description TEXT,
    parameters JSONB,
    examples JSONB,
    phase VARCHAR(20),
    active BOOLEAN DEFAULT true,
    version VARCHAR(20) DEFAULT '1.0.0',
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Cross-Domain Verb Registry - Global verb registry
CREATE TABLE IF NOT EXISTS "ob-poc".verb_registry (
    verb VARCHAR(100) PRIMARY KEY,
    primary_domain VARCHAR(100) NOT NULL,
    shared BOOLEAN DEFAULT false,
    deprecated BOOLEAN DEFAULT false,
    replacement_verb VARCHAR(100),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Vocabulary Change Audit - Track vocabulary changes
CREATE TABLE IF NOT EXISTS "ob-poc".vocabulary_audit (
    audit_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    change_type VARCHAR(20) NOT NULL,
    old_definition JSONB,
    new_definition JSONB,
    changed_by VARCHAR(255),
    change_reason TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- ATTRIBUTE DICTIONARY AND VALUES
-- ============================================================================

-- Master Data Dictionary (Central pillar of AttributeID-as-Type pattern)
CREATE TABLE IF NOT EXISTS "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    long_description TEXT,
    group_id VARCHAR(100) NOT NULL DEFAULT 'default',
    mask VARCHAR(50) DEFAULT 'string',
    domain VARCHAR(100),
    vector TEXT,
    source JSONB,
    sink JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Attribute Values - Runtime values for onboarding instances
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values (
    av_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    dsl_ob_id UUID,
    dsl_version INTEGER NOT NULL,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary (attribute_id) ON DELETE CASCADE,
    value JSONB NOT NULL,
    state TEXT NOT NULL DEFAULT 'resolved',
    source JSONB,
    observed_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (cbu_id, dsl_version, attribute_id)
);

-- ============================================================================
-- PRODUCT REQUIREMENTS AND WORKFLOWS
-- ============================================================================

-- Product Requirements - DSL operations and attributes required per product
CREATE TABLE IF NOT EXISTS "ob-poc".product_requirements (
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE,
    entity_types JSONB NOT NULL,
    required_dsl JSONB NOT NULL,
    attributes JSONB NOT NULL,
    compliance JSONB NOT NULL,
    prerequisites JSONB NOT NULL,
    conditional_rules JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    PRIMARY KEY (product_id)
);

-- Entity Product Mappings - Compatibility matrix
CREATE TABLE IF NOT EXISTS "ob-poc".entity_product_mappings (
    entity_type VARCHAR(100) NOT NULL,
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE,
    compatible BOOLEAN NOT NULL,
    restrictions JSONB,
    required_fields JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    PRIMARY KEY (entity_type, product_id)
);

-- Product Workflows - Generated workflows for product-entity combinations
CREATE TABLE IF NOT EXISTS "ob-poc".product_workflows (
    workflow_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id VARCHAR(255) NOT NULL,
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id),
    entity_type VARCHAR(100) NOT NULL,
    required_dsl JSONB NOT NULL,
    generated_dsl TEXT NOT NULL,
    compliance_rules JSONB NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- PRODUCTION RESOURCES
-- ============================================================================

-- Production Resources table
CREATE TABLE IF NOT EXISTS "ob-poc".prod_resources (
    resource_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    owner VARCHAR(255) NOT NULL,
    dictionary_group VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Service <-> Resource Join Table
CREATE TABLE IF NOT EXISTS "ob-poc".service_resources (
    service_id UUID NOT NULL REFERENCES "ob-poc".services (service_id) ON DELETE CASCADE,
    resource_id UUID NOT NULL REFERENCES "ob-poc".prod_resources (resource_id) ON DELETE CASCADE,
    PRIMARY KEY (service_id, resource_id)
);

-- ============================================================================
-- ENTITY RELATIONSHIP MODEL
-- ============================================================================

-- Roles - Defines roles entities can play within a CBU
CREATE TABLE IF NOT EXISTS "ob-poc".roles (
    role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Entity Types - Different types of entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_types (
    entity_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    table_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Entities - Central entity registry
CREATE TABLE IF NOT EXISTS "ob-poc".entities (
    entity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type_id UUID NOT NULL REFERENCES "ob-poc".entity_types (entity_type_id) ON DELETE CASCADE,
    external_id VARCHAR(255),
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- CBU Entity Roles - Links CBUs to entities through roles
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_entity_roles (
    cbu_entity_role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES "ob-poc".roles (role_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (cbu_id, entity_id, role_id)
);

-- ============================================================================
-- ENTITY TYPE IMPLEMENTATIONS
-- ============================================================================

-- Limited Company entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_limited_companies (
    limited_company_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_name VARCHAR(255) NOT NULL,
    registration_number VARCHAR(100),
    jurisdiction VARCHAR(100),
    incorporation_date DATE,
    registered_address TEXT,
    business_nature TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Partnership entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_partnerships (
    partnership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_name VARCHAR(255) NOT NULL,
    partnership_type VARCHAR(100),
    jurisdiction VARCHAR(100),
    formation_date DATE,
    principal_place_business TEXT,
    partnership_agreement_date DATE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Proper Person entities (individuals)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_proper_persons (
    proper_person_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    first_name VARCHAR(255) NOT NULL,
    last_name VARCHAR(255) NOT NULL,
    middle_names VARCHAR(255),
    date_of_birth DATE,
    nationality VARCHAR(100),
    residence_address TEXT,
    id_document_type VARCHAR(100),
    id_document_number VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Trust entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_trusts (
    trust_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_name VARCHAR(255) NOT NULL,
    trust_type VARCHAR(100),
    jurisdiction VARCHAR(100) NOT NULL,
    establishment_date DATE,
    trust_deed_date DATE,
    trust_purpose TEXT,
    governing_law VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- TRUST-SPECIFIC STRUCTURES
-- ============================================================================

-- Trust Parties - Roles within trusts
CREATE TABLE IF NOT EXISTS "ob-poc".trust_parties (
    trust_party_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts (trust_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    party_role VARCHAR(100) NOT NULL,
    party_type VARCHAR(100) NOT NULL,
    appointment_date DATE,
    resignation_date DATE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (trust_id, entity_id, party_role)
);

-- Trust Beneficiary Classes
CREATE TABLE IF NOT EXISTS "ob-poc".trust_beneficiary_classes (
    beneficiary_class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts (trust_id) ON DELETE CASCADE,
    class_name VARCHAR(255) NOT NULL,
    class_definition TEXT,
    class_type VARCHAR(100),
    monitoring_required BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Trust Protector Powers
CREATE TABLE IF NOT EXISTS "ob-poc".trust_protector_powers (
    protector_power_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_party_id UUID NOT NULL REFERENCES "ob-poc".trust_parties (trust_party_id) ON DELETE CASCADE,
    power_type VARCHAR(100) NOT NULL,
    power_description TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- PARTNERSHIP-SPECIFIC STRUCTURES
-- ============================================================================

-- Partnership Interests - Ownership and control
CREATE TABLE IF NOT EXISTS "ob-poc".partnership_interests (
    interest_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL REFERENCES "ob-poc".entity_partnerships (partnership_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    partner_type VARCHAR(100) NOT NULL,
    capital_commitment DECIMAL(15,2),
    ownership_percentage DECIMAL(5,2),
    voting_rights DECIMAL(5,2),
    profit_sharing_percentage DECIMAL(5,2),
    admission_date DATE,
    withdrawal_date DATE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (partnership_id, entity_id, partner_type)
);

-- Partnership Control Mechanisms
CREATE TABLE IF NOT EXISTS "ob-poc".partnership_control_mechanisms (
    control_mechanism_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL REFERENCES "ob-poc".entity_partnerships (partnership_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    control_type VARCHAR(100) NOT NULL,
    control_description TEXT,
    effective_date DATE,
    termination_date DATE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- UBO IDENTIFICATION RESULTS
-- ============================================================================

-- UBO Registry - Results of UBO identification
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_registry (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE,
    subject_entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    ubo_proper_person_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    relationship_type VARCHAR(100) NOT NULL,
    qualifying_reason VARCHAR(100) NOT NULL,
    ownership_percentage DECIMAL(5,2),
    control_type VARCHAR(100),
    workflow_type VARCHAR(100) NOT NULL,
    regulatory_framework VARCHAR(100),
    verification_status VARCHAR(50) DEFAULT 'PENDING',
    screening_result VARCHAR(50) DEFAULT 'PENDING',
    risk_rating VARCHAR(50),
    identified_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (subject_entity_id, ubo_proper_person_id, relationship_type)
);

-- ============================================================================
-- ORCHESTRATION SYSTEM
-- ============================================================================

-- Orchestration Sessions - Multi-domain workflow sessions
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_sessions (
    session_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    primary_domain VARCHAR(100) NOT NULL,
    cbu_id UUID REFERENCES "ob-poc".cbus (cbu_id),
    entity_type VARCHAR(50),
    entity_name TEXT,
    jurisdiction VARCHAR(10),
    products TEXT[],
    services TEXT[],
    workflow_type VARCHAR(50) DEFAULT 'ONBOARDING',
    current_state VARCHAR(50) DEFAULT 'CREATED',
    version_number INTEGER DEFAULT 0,
    unified_dsl TEXT,
    shared_context JSONB,
    execution_plan JSONB,
    entity_refs JSONB,
    attribute_refs JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    last_used TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    expires_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc' + INTERVAL '24 hours')
);

-- Domain Sessions within orchestration
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_domain_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    domain_name VARCHAR(100) NOT NULL,
    domain_session_id UUID NOT NULL,
    state VARCHAR(50) DEFAULT 'CREATED',
    contributed_dsl TEXT,
    domain_context JSONB,
    dependencies TEXT[],
    last_activity TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (orchestration_session_id, domain_name)
);

-- Orchestration Tasks - Track workflow tasks and dependencies
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    domain_name VARCHAR(100) NOT NULL,
    verb VARCHAR(200) NOT NULL,
    parameters JSONB,
    dependencies TEXT[],
    status VARCHAR(50) DEFAULT 'PENDING',
    generated_dsl TEXT,
    error_message TEXT,
    scheduled_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- State History - Track orchestration session state transitions
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_state_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    from_state VARCHAR(50),
    to_state VARCHAR(50) NOT NULL,
    domain_name VARCHAR(100),
    reason TEXT,
    generated_by VARCHAR(100),
    version_number INTEGER,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Core business table indexes
CREATE INDEX IF NOT EXISTS idx_cbus_name ON "ob-poc".cbus (name);
CREATE INDEX IF NOT EXISTS idx_products_name ON "ob-poc".products (name);
CREATE INDEX IF NOT EXISTS idx_services_name ON "ob-poc".services (name);

-- DSL table indexes
CREATE INDEX IF NOT EXISTS idx_dsl_ob_cbu_id_created_at ON "ob-poc".dsl_ob (cbu_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dsl_domains_name ON "ob-poc".dsl_domains (domain_name);
CREATE INDEX IF NOT EXISTS idx_dsl_domains_active ON "ob-poc".dsl_domains (active);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_domain_version ON "ob-poc".dsl_versions (domain_id, version_number DESC);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_status ON "ob-poc".dsl_versions (compilation_status);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_functional_state ON "ob-poc".dsl_versions (functional_state);
CREATE INDEX IF NOT EXISTS idx_dsl_versions_created_at ON "ob-poc".dsl_versions (created_at DESC);

-- Parsed AST indexes
CREATE INDEX IF NOT EXISTS idx_parsed_asts_version_id ON "ob-poc".parsed_asts (version_id);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_hash ON "ob-poc".parsed_asts (ast_hash);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_grammar_version ON "ob-poc".parsed_asts (grammar_version);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_parsed_at ON "ob-poc".parsed_asts (parsed_at DESC);

-- DSL execution indexes
CREATE INDEX IF NOT EXISTS idx_dsl_execution_version_phase ON "ob-poc".dsl_execution_log (version_id, execution_phase);
CREATE INDEX IF NOT EXISTS idx_dsl_execution_cbu_id ON "ob-poc".dsl_execution_log (cbu_id);
CREATE INDEX IF NOT EXISTS idx_dsl_execution_status ON "ob-poc".dsl_execution_log (status);
CREATE INDEX IF NOT EXISTS idx_dsl_execution_started_at ON "ob-poc".dsl_execution_log (started_at DESC);

-- Grammar and vocabulary indexes
CREATE INDEX IF NOT EXISTS idx_grammar_rules_name ON "ob-poc".grammar_rules (rule_name);
CREATE INDEX IF NOT EXISTS idx_grammar_rules_domain ON "ob-poc".grammar_rules (domain);
CREATE INDEX IF NOT EXISTS idx_grammar_rules_active ON "ob-poc".grammar_rules (active);
CREATE UNIQUE INDEX IF NOT EXISTS idx_domain_vocabularies_domain_verb ON "ob-poc".domain_vocabularies (domain, verb);
CREATE INDEX IF NOT EXISTS idx_domain_vocabularies_category ON "ob-poc".domain_vocabularies (category);
CREATE INDEX IF NOT EXISTS idx_domain_vocabularies_active ON "ob-poc".domain_vocabularies (active);

-- Verb registry indexes
CREATE INDEX IF NOT EXISTS idx_verb_registry_domain ON "ob-poc".verb_registry (primary_domain);
CREATE INDEX IF NOT EXISTS idx_verb_registry_shared ON "ob-poc".verb_registry (shared);
CREATE INDEX IF NOT EXISTS idx_verb_registry_deprecated ON "ob-poc".verb_registry (deprecated);

-- Vocabulary audit indexes
CREATE INDEX IF NOT EXISTS idx_vocabulary_audit_domain_verb ON "ob-poc".vocabulary_audit (domain, verb);
CREATE INDEX IF NOT EXISTS idx_vocabulary_audit_change_type ON "ob-poc".vocabulary_audit (change_type);
CREATE INDEX IF NOT EXISTS idx_vocabulary_audit_created_at ON "ob-poc".vocabulary_audit (created_at DESC);

-- Dictionary and attribute indexes
CREATE INDEX IF NOT EXISTS idx_dictionary_name ON "ob-poc".dictionary (name);
CREATE INDEX IF NOT EXISTS idx_dictionary_group_id ON "ob-poc".dictionary (group_id);
CREATE INDEX IF NOT EXISTS idx_dictionary_domain ON "ob-poc".dictionary (domain);
CREATE INDEX IF NOT EXISTS idx_attr_vals_lookup ON "ob-poc".attribute_values (cbu_id, attribute_id, dsl_version);

-- Product requirement indexes
CREATE INDEX IF NOT EXISTS idx_product_requirements_product ON "ob-poc".product_requirements (product_id);
CREATE INDEX IF NOT EXISTS idx_entity_product_mappings_entity ON "ob-poc".entity_product_mappings (entity_type);
CREATE INDEX IF NOT EXISTS idx_entity_product_mappings_product ON "ob-poc".entity_product_mappings (product_id);
CREATE INDEX IF NOT EXISTS idx_entity_product_mappings_compatible ON "ob-poc".entity_product_mappings (compatible);
CREATE INDEX IF NOT EXISTS idx_product_workflows_cbu ON "ob-poc".product_workflows (cbu_id);
CREATE INDEX IF NOT EXISTS idx_product_workflows_status ON "ob-poc".product_workflows (status);
CREATE INDEX IF NOT EXISTS idx_product_workflows_product_entity ON "ob-poc".product_workflows (product_id, entity_type);

-- Resource indexes
CREATE INDEX IF NOT EXISTS idx_prod_resources_name ON "ob-poc".prod_resources (name);
CREATE INDEX IF NOT EXISTS idx_prod_resources_owner ON "ob-poc".prod_resources (owner);
CREATE INDEX IF NOT EXISTS idx_prod_resources_dict_group ON "ob-poc".prod_resources (dictionary_group);

-- Entity relationship indexes
CREATE INDEX IF NOT EXISTS idx_roles_name ON "ob-poc".roles (name);
CREATE INDEX IF NOT EXISTS idx_entity_types_name ON "ob-poc".entity_types (name);
CREATE INDEX IF NOT EXISTS idx_entity_types_table ON "ob-poc".entity_types (table_name);
CREATE INDEX IF NOT EXISTS idx_entities_type ON "ob-poc".entities (entity_type_id);
CREATE INDEX IF NOT EXISTS idx_entities_external_id ON "ob-poc".entities (external_id);
CREATE INDEX IF NOT EXISTS idx_entities_name ON "ob-poc".entities (name);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_cbu ON "ob-poc".cbu_entity_roles (cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_entity ON "ob-poc".cbu_entity_roles (entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_role ON "ob-poc".cbu_entity_roles (role_id);

-- Entity type specific indexes
CREATE INDEX IF NOT EXISTS idx_limited_companies_reg_num ON "ob-poc".entity_limited_companies (registration_number);
CREATE INDEX IF NOT EXISTS idx_limited_companies_jurisdiction ON "ob-poc".entity_limited_companies (jurisdiction);
CREATE INDEX IF NOT EXISTS idx_partnerships_type ON "ob-poc".entity_partnerships (partnership_type);
CREATE INDEX IF NOT EXISTS idx_partnerships_jurisdiction ON "ob-poc".entity_partnerships (jurisdiction);
CREATE INDEX IF NOT EXISTS idx_proper_persons_full_name ON "ob-poc".entity_proper_persons (last_name, first_name);
CREATE INDEX IF NOT EXISTS idx_proper_persons_nationality ON "ob-poc".entity_proper_persons (nationality);
CREATE INDEX IF NOT EXISTS idx_proper_persons_id_document ON "ob-poc".entity_proper_persons (id_document_type, id_document_number);
CREATE INDEX IF NOT EXISTS idx_trusts_type ON "ob-poc".entity_trusts (trust_type);
CREATE INDEX IF NOT EXISTS idx_trusts_jurisdiction ON "ob-poc".entity_trusts (jurisdiction);

-- Trust structure indexes
CREATE INDEX IF NOT EXISTS idx_trust_parties_trust ON "ob-poc".trust_parties (trust_id);
CREATE INDEX IF NOT EXISTS idx_trust_parties_entity ON "ob-poc".trust_parties (entity_id);
CREATE INDEX IF NOT EXISTS idx_trust_parties_role ON "ob-poc".trust_parties (party_role);
CREATE INDEX IF NOT EXISTS idx_beneficiary_classes_trust ON "ob-poc".trust_beneficiary_classes (trust_id);
CREATE INDEX IF NOT EXISTS idx_protector_powers_party ON "ob-poc".trust_protector_powers (trust_party_id);

-- Partnership structure indexes
CREATE INDEX IF NOT EXISTS idx_partnership_interests_partnership ON "ob-poc".partnership_interests (partnership_id);
CREATE INDEX IF NOT EXISTS idx_partnership_interests_entity ON "ob-poc".partnership_interests (entity_id);
CREATE INDEX IF NOT EXISTS idx_partnership_interests_type ON "ob-poc".partnership_interests (partner_type);
CREATE INDEX IF NOT EXISTS idx_partnership_control_partnership ON "ob-poc".partnership_control_mechanisms (partnership_id);
CREATE INDEX IF NOT EXISTS idx_partnership_control_entity ON "ob-poc".partnership_control_mechanisms (entity_id);

-- UBO registry indexes
CREATE INDEX IF NOT EXISTS idx_ubo_registry_cbu ON "ob-poc".ubo_registry (cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_subject ON "ob-poc".ubo_registry (subject_entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_ubo_proper_person ON "ob-poc".ubo_registry (ubo_proper_person_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_workflow ON "ob-poc".ubo_registry (workflow_type);

-- Orchestration indexes
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_cbu ON "ob-poc".orchestration_sessions (cbu_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_entity_type ON "ob-poc".orchestration_sessions (entity_type);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_workflow ON "ob-poc".orchestration_sessions (workflow_type);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_state ON "ob-poc".orchestration_sessions (current_state);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_last_used ON "ob-poc".orchestration_sessions (last_used);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_expires ON "ob-poc".orchestration_sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_orchestration ON "ob-poc".orchestration_domain_sessions (orchestration_session_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_domain ON "ob-poc".orchestration_domain_sessions (domain_name);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_state ON "ob-poc".orchestration_domain_sessions (state);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_activity ON "ob-poc".orchestration_domain_sessions (last_activity);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_session ON "ob-poc".orchestration_tasks (orchestration_session_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_domain ON "ob-poc".orchestration_tasks (domain_name);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_status ON "ob-poc".orchestration_tasks (status);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_scheduled ON "ob-poc".orchestration_tasks (scheduled_at);
CREATE INDEX IF NOT EXISTS idx_orchestration_state_history_session ON "ob-poc".orchestration_state_history (orchestration_session_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_state_history_states ON "ob-poc".orchestration_state_history (from_state, to_state);
CREATE INDEX IF NOT EXISTS idx_orchestration_state_history_created ON "ob-poc".orchestration_state_history (created_at);

-- ============================================================================
-- TRIGGERS AND FUNCTIONS
-- ============================================================================

-- AST Cache Invalidation Function
CREATE OR REPLACE FUNCTION "ob-poc".invalidate_ast_cache()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE "ob-poc".parsed_asts
    SET invalidated_at = now() at time zone 'utc'
    WHERE version_id = NEW.version_id
    AND invalidated_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for AST cache invalidation
CREATE OR REPLACE TRIGGER trigger_invalidate_ast_cache
    AFTER UPDATE ON "ob-poc".dsl_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".invalidate_ast_cache();

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON SCHEMA "ob-poc" IS 'Ultimate Beneficial Ownership and Onboarding system with DSL-as-State architecture';
COMMENT ON TABLE "ob-poc".cbus IS 'Client Business Units - Core entity registry for onboarding cases';
COMMENT ON TABLE "ob-poc".dictionary IS 'Master attribute dictionary - central pillar of AttributeID-as-Type pattern';
COMMENT ON TABLE "ob-poc".dsl_ob IS 'Immutable storage for DSL documents - implements DSL-as-State pattern';
COMMENT ON TABLE "ob-poc".dsl_domains IS 'Registry of business domains with grammar and vocabulary versions';
COMMENT ON TABLE "ob-poc".dsl_versions IS 'Version management for DSL instances within domains';
COMMENT ON TABLE "ob-poc".parsed_asts IS 'Compiled Abstract Syntax Trees with caching and invalidation';
COMMENT ON TABLE "ob-poc".dsl_execution_log IS 'Performance tracking and execution results for DSL operations';
COMMENT ON TABLE "ob-poc".entities IS 'Central entity registry supporting polymorphic entity types';
COMMENT ON TABLE "ob-poc".ubo_registry IS 'Results of Ultimate Beneficial Ownership identification';
COMMENT ON TABLE "ob-poc".orchestration_sessions IS 'Multi-domain workflow coordination with DSL accumulation';