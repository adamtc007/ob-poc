-- ob-poc Database Schema
-- Generated from existing database schema on 2025-11-12
-- Complete CREATE TABLE statements with all constraints, indexes, and foreign keys

-- Create the schema if it doesn't exist
CREATE SCHEMA IF NOT EXISTS "ob-poc";

-- ====================================
-- TABLE DEFINITIONS
-- ====================================

CREATE TABLE "ob-poc".attribute_values (
    av_id UUID NOT NULL DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    dsl_ob_id UUID,
    dsl_version INTEGER NOT NULL,
    attribute_id UUID NOT NULL,
    value JSONB NOT NULL,
    state TEXT NOT NULL DEFAULT 'resolved'::text,
    source JSONB,
    observed_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".cbu_entity_roles (
    cbu_entity_role_id UUID NOT NULL DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    role_id UUID NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".cbus (
    cbu_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    nature_purpose TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".crud_operations (
    operation_id UUID NOT NULL DEFAULT gen_random_uuid(),
    operation_type VARCHAR(20) NOT NULL,
    asset_type VARCHAR(50) NOT NULL,
    entity_table_name VARCHAR(100),
    generated_dsl TEXT NOT NULL,
    ai_instruction TEXT NOT NULL,
    affected_records JSONB NOT NULL DEFAULT '[]'::jsonb,
    execution_status VARCHAR(20) NOT NULL DEFAULT 'PENDING'::character varying,
    ai_confidence NUMERIC(3,2),
    ai_provider VARCHAR(50),
    ai_model VARCHAR(100),
    execution_time_ms INTEGER,
    error_message TEXT,
    created_by VARCHAR(255) DEFAULT 'agentic_system'::character varying,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    completed_at TIMESTAMPTZ,
    rows_affected INTEGER DEFAULT 0,
    transaction_id UUID,
    parent_operation_id UUID
);

CREATE TABLE "ob-poc".dictionary (
    attribute_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    long_description TEXT,
    group_id VARCHAR(100) NOT NULL DEFAULT 'default'::character varying,
    mask VARCHAR(50) DEFAULT 'string'::character varying,
    domain VARCHAR(100),
    vector TEXT,
    source JSONB,
    sink JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".document_catalog (
    doc_id UUID NOT NULL DEFAULT gen_random_uuid(),
    file_hash_sha256 TEXT NOT NULL,
    storage_key TEXT NOT NULL,
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),
    extracted_data JSONB,
    extraction_status VARCHAR(50) DEFAULT 'PENDING'::character varying,
    extraction_confidence NUMERIC(5,4),
    last_extracted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".document_issuers_backup (
    issuer_id UUID,
    issuer_code VARCHAR(100),
    legal_name VARCHAR(300),
    jurisdiction VARCHAR(10),
    regulatory_type VARCHAR(100),
    official_website VARCHAR(500),
    verification_endpoint VARCHAR(500),
    trust_level VARCHAR(20),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    backup_created_at TIMESTAMPTZ
);

CREATE TABLE "ob-poc".document_metadata (
    doc_id UUID NOT NULL,
    attribute_id UUID NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".document_relationships (
    relationship_id UUID NOT NULL DEFAULT gen_random_uuid(),
    primary_doc_id UUID NOT NULL,
    related_doc_id UUID NOT NULL,
    relationship_type VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".document_types (
    type_id UUID NOT NULL DEFAULT gen_random_uuid(),
    type_code VARCHAR(100) NOT NULL,
    display_name VARCHAR(200) NOT NULL,
    category VARCHAR(100) NOT NULL,
    domain VARCHAR(100),
    description TEXT,
    required_attributes JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE "ob-poc".domain_vocabularies (
    vocab_id UUID NOT NULL DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    category VARCHAR(50),
    description TEXT,
    parameters JSONB,
    examples JSONB,
    phase VARCHAR(20),
    active BOOLEAN DEFAULT true,
    version VARCHAR(20) DEFAULT '1.0.0'::character varying,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".dsl_domains (
    domain_id UUID NOT NULL DEFAULT gen_random_uuid(),
    domain_name VARCHAR(100) NOT NULL,
    description TEXT,
    base_grammar_version VARCHAR(20) DEFAULT '1.0.0'::character varying,
    vocabulary_version VARCHAR(20) DEFAULT '1.0.0'::character varying,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".dsl_examples (
    example_id UUID NOT NULL DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    operation_type VARCHAR(20) NOT NULL,
    asset_type VARCHAR(50) NOT NULL,
    entity_table_name VARCHAR(100),
    natural_language_input TEXT NOT NULL,
    example_dsl TEXT NOT NULL,
    expected_outcome TEXT,
    tags TEXT[] DEFAULT ARRAY[]::text[],
    complexity_level VARCHAR(20) DEFAULT 'MEDIUM'::character varying,
    success_rate NUMERIC(3,2) DEFAULT 1.0,
    usage_count INTEGER DEFAULT 0,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_by VARCHAR(255) DEFAULT 'system'::character varying
);

CREATE TABLE "ob-poc".dsl_execution_log (
    execution_id UUID NOT NULL DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL,
    cbu_id VARCHAR(255),
    execution_phase VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    result_data JSONB,
    error_details JSONB,
    performance_metrics JSONB,
    executed_by VARCHAR(255),
    started_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    completed_at TIMESTAMPTZ,
    duration_ms INTEGER
);

CREATE TABLE "ob-poc".dsl_execution_summary (
    domain_name VARCHAR(100),
    version_number INTEGER,
    compilation_status VARCHAR(50),
    total_executions BIGINT,
    successful_executions BIGINT,
    failed_executions BIGINT,
    last_execution_at TIMESTAMPTZ
);

CREATE TABLE "ob-poc".dsl_latest_versions (
    domain_name VARCHAR(100),
    domain_description TEXT,
    version_id UUID,
    version_number INTEGER,
    functional_state VARCHAR(100),
    compilation_status VARCHAR(50),
    change_description TEXT,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ,
    has_compiled_ast BOOLEAN
);

CREATE TABLE "ob-poc".dsl_ob (
    version_id UUID NOT NULL DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    dsl_text TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".dsl_versions (
    version_id UUID NOT NULL DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL,
    version_number INTEGER NOT NULL,
    functional_state VARCHAR(100),
    dsl_source_code TEXT NOT NULL,
    compilation_status VARCHAR(50) DEFAULT 'DRAFT'::character varying,
    change_description TEXT,
    parent_version_id UUID,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    compiled_at TIMESTAMPTZ,
    activated_at TIMESTAMPTZ
);

CREATE TABLE "ob-poc".entities (
    entity_id UUID NOT NULL DEFAULT gen_random_uuid(),
    entity_type_id UUID NOT NULL,
    external_id VARCHAR(255),
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_crud_rules (
    rule_id UUID NOT NULL DEFAULT gen_random_uuid(),
    entity_table_name VARCHAR(100) NOT NULL,
    operation_type VARCHAR(20) NOT NULL,
    field_name VARCHAR(100),
    constraint_type VARCHAR(50) NOT NULL,
    constraint_description TEXT NOT NULL,
    validation_pattern VARCHAR(500),
    error_message TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_lifecycle_status (
    status_id UUID NOT NULL DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    status_code VARCHAR(50) NOT NULL,
    status_description VARCHAR(200),
    effective_date DATE NOT NULL,
    end_date DATE,
    reason_code VARCHAR(100),
    notes TEXT,
    created_by VARCHAR(100) DEFAULT 'system'::character varying,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE "ob-poc".entity_limited_companies (
    limited_company_id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_name VARCHAR(255) NOT NULL,
    registration_number VARCHAR(100),
    jurisdiction VARCHAR(100),
    incorporation_date DATE,
    registered_address TEXT,
    business_nature TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_partnerships (
    partnership_id UUID NOT NULL DEFAULT gen_random_uuid(),
    partnership_name VARCHAR(255) NOT NULL,
    partnership_type VARCHAR(100),
    jurisdiction VARCHAR(100),
    formation_date DATE,
    principal_place_business TEXT,
    partnership_agreement_date DATE,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_product_mappings (
    entity_type VARCHAR(100) NOT NULL,
    product_id UUID NOT NULL,
    compatible BOOLEAN NOT NULL,
    restrictions JSONB,
    required_fields JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_proper_persons (
    proper_person_id UUID NOT NULL DEFAULT gen_random_uuid(),
    first_name VARCHAR(255) NOT NULL,
    last_name VARCHAR(255) NOT NULL,
    middle_names VARCHAR(255),
    date_of_birth DATE,
    nationality VARCHAR(100),
    residence_address TEXT,
    id_document_type VARCHAR(100),
    id_document_number VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_trusts (
    trust_id UUID NOT NULL DEFAULT gen_random_uuid(),
    trust_name VARCHAR(255) NOT NULL,
    trust_type VARCHAR(100),
    jurisdiction VARCHAR(100) NOT NULL,
    establishment_date DATE,
    trust_deed_date DATE,
    trust_purpose TEXT,
    governing_law VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_types (
    entity_type_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    table_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".entity_validation_rules (
    rule_id UUID NOT NULL DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    field_name VARCHAR(100) NOT NULL,
    validation_type VARCHAR(50) NOT NULL,
    validation_rule JSONB NOT NULL,
    error_message VARCHAR(500),
    severity VARCHAR(20) DEFAULT 'ERROR'::character varying,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE "ob-poc".grammar_rules (
    rule_id UUID NOT NULL DEFAULT gen_random_uuid(),
    rule_name VARCHAR(100) NOT NULL,
    rule_definition TEXT NOT NULL,
    rule_type VARCHAR(50) NOT NULL DEFAULT 'production'::character varying,
    domain VARCHAR(100),
    version VARCHAR(20) DEFAULT '1.0.0'::character varying,
    active BOOLEAN DEFAULT true,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".master_entity_xref (
    xref_id UUID NOT NULL DEFAULT gen_random_uuid(),
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    entity_name VARCHAR(500) NOT NULL,
    jurisdiction_code VARCHAR(10),
    entity_status VARCHAR(50) DEFAULT 'ACTIVE'::character varying,
    business_purpose TEXT,
    primary_contact_person UUID,
    regulatory_numbers JSONB DEFAULT '{}'::jsonb,
    additional_metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE "ob-poc".master_jurisdictions (
    jurisdiction_code VARCHAR(10) NOT NULL,
    jurisdiction_name VARCHAR(200) NOT NULL,
    country_code VARCHAR(3) NOT NULL,
    region VARCHAR(100),
    regulatory_framework VARCHAR(100),
    entity_formation_allowed BOOLEAN DEFAULT true,
    offshore_jurisdiction BOOLEAN DEFAULT false,
    regulatory_authority VARCHAR(300),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE "ob-poc".orchestration_domain_sessions (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL,
    domain_name VARCHAR(100) NOT NULL,
    domain_session_id UUID NOT NULL,
    state VARCHAR(50) DEFAULT 'CREATED'::character varying,
    contributed_dsl TEXT,
    domain_context JSONB,
    dependencies TEXT[],
    last_activity TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".orchestration_sessions (
    session_id UUID NOT NULL DEFAULT gen_random_uuid(),
    primary_domain VARCHAR(100) NOT NULL,
    cbu_id UUID,
    entity_type VARCHAR(50),
    entity_name TEXT,
    jurisdiction VARCHAR(10),
    products TEXT[],
    services TEXT[],
    workflow_type VARCHAR(50) DEFAULT 'ONBOARDING'::character varying,
    current_state VARCHAR(50) DEFAULT 'CREATED'::character varying,
    version_number INTEGER DEFAULT 0,
    unified_dsl TEXT,
    shared_context JSONB,
    execution_plan JSONB,
    entity_refs JSONB,
    attribute_refs JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    last_used TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    expires_at TIMESTAMPTZ DEFAULT ((now() AT TIME ZONE 'utc'::text) + '24:00:00'::interval)
);

CREATE TABLE "ob-poc".orchestration_state_history (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL,
    from_state VARCHAR(50),
    to_state VARCHAR(50) NOT NULL,
    domain_name VARCHAR(100),
    reason TEXT,
    generated_by VARCHAR(100),
    version_number INTEGER,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".orchestration_tasks (
    task_id UUID NOT NULL DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL,
    domain_name VARCHAR(100) NOT NULL,
    verb VARCHAR(200) NOT NULL,
    parameters JSONB,
    dependencies TEXT[],
    status VARCHAR(50) DEFAULT 'PENDING'::character varying,
    generated_dsl TEXT,
    error_message TEXT,
    scheduled_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".parsed_asts (
    ast_id UUID NOT NULL DEFAULT gen_random_uuid(),
    version_id UUID NOT NULL,
    ast_json JSONB NOT NULL,
    parse_metadata JSONB,
    grammar_version VARCHAR(20) NOT NULL,
    parser_version VARCHAR(20) NOT NULL,
    ast_hash VARCHAR(64),
    node_count INTEGER,
    complexity_score NUMERIC(10,2),
    parsed_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    invalidated_at TIMESTAMPTZ
);

CREATE TABLE "ob-poc".partnership_control_mechanisms (
    control_mechanism_id UUID NOT NULL DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    control_type VARCHAR(100) NOT NULL,
    control_description TEXT,
    effective_date DATE,
    termination_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".partnership_interests (
    interest_id UUID NOT NULL DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    partner_type VARCHAR(100) NOT NULL,
    capital_commitment NUMERIC(15,2),
    ownership_percentage NUMERIC(5,2),
    voting_rights NUMERIC(5,2),
    profit_sharing_percentage NUMERIC(5,2),
    admission_date DATE,
    withdrawal_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".prod_resources (
    resource_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    owner VARCHAR(255) NOT NULL,
    dictionary_group VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".product_requirements (
    product_id UUID NOT NULL,
    entity_types JSONB NOT NULL,
    required_dsl JSONB NOT NULL,
    attributes JSONB NOT NULL,
    compliance JSONB NOT NULL,
    prerequisites JSONB NOT NULL,
    conditional_rules JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".product_services (
    product_id UUID NOT NULL,
    service_id UUID NOT NULL
);

CREATE TABLE "ob-poc".product_workflows (
    workflow_id UUID NOT NULL DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    product_id UUID NOT NULL,
    entity_type VARCHAR(100) NOT NULL,
    required_dsl JSONB NOT NULL,
    generated_dsl TEXT NOT NULL,
    compliance_rules JSONB NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING'::character varying,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".products (
    product_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".rag_embeddings (
    embedding_id UUID NOT NULL DEFAULT gen_random_uuid(),
    content_type VARCHAR(50) NOT NULL,
    content_text TEXT NOT NULL,
    embedding_data JSONB,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    source_table VARCHAR(100),
    asset_type VARCHAR(50),
    relevance_score NUMERIC(3,2) DEFAULT 1.0,
    usage_count INTEGER DEFAULT 0,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".referential_integrity_check (
    table_name TEXT,
    column_name TEXT,
    orphaned_value TEXT,
    issue TEXT
);

CREATE TABLE "ob-poc".roles (
    role_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".schema_changes (
    change_id UUID NOT NULL DEFAULT gen_random_uuid(),
    change_type VARCHAR(50) NOT NULL,
    description TEXT NOT NULL,
    script_name VARCHAR(255),
    applied_at TIMESTAMPTZ DEFAULT now(),
    applied_by VARCHAR(100) DEFAULT CURRENT_USER
);

CREATE TABLE "ob-poc".service_resources (
    service_id UUID NOT NULL,
    resource_id UUID NOT NULL
);

CREATE TABLE "ob-poc".services (
    service_id UUID NOT NULL DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".trust_beneficiary_classes (
    beneficiary_class_id UUID NOT NULL DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL,
    class_name VARCHAR(255) NOT NULL,
    class_definition TEXT,
    class_type VARCHAR(100),
    monitoring_required BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".trust_parties (
    trust_party_id UUID NOT NULL DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL,
    entity_id UUID NOT NULL,
    party_role VARCHAR(100) NOT NULL,
    party_type VARCHAR(100) NOT NULL,
    appointment_date DATE,
    resignation_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".trust_protector_powers (
    protector_power_id UUID NOT NULL DEFAULT gen_random_uuid(),
    trust_party_id UUID NOT NULL,
    power_type VARCHAR(100) NOT NULL,
    power_description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".ubo_registry (
    ubo_id UUID NOT NULL DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    subject_entity_id UUID NOT NULL,
    ubo_proper_person_id UUID NOT NULL,
    relationship_type VARCHAR(100) NOT NULL,
    qualifying_reason VARCHAR(100) NOT NULL,
    ownership_percentage NUMERIC(5,2),
    control_type VARCHAR(100),
    workflow_type VARCHAR(100) NOT NULL,
    regulatory_framework VARCHAR(100),
    verification_status VARCHAR(50) DEFAULT 'PENDING'::character varying,
    screening_result VARCHAR(50) DEFAULT 'PENDING'::character varying,
    risk_rating VARCHAR(50),
    identified_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".verb_registry (
    verb VARCHAR(100) NOT NULL,
    primary_domain VARCHAR(100) NOT NULL,
    shared BOOLEAN DEFAULT false,
    deprecated BOOLEAN DEFAULT false,
    replacement_verb VARCHAR(100),
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

CREATE TABLE "ob-poc".vocabulary_audit (
    audit_id UUID NOT NULL DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    change_type VARCHAR(20) NOT NULL,
    old_definition JSONB,
    new_definition JSONB,
    changed_by VARCHAR(255),
    change_reason TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'utc'::text)
);

-- ====================================
-- PRIMARY KEY CONSTRAINTS
-- ====================================

ALTER TABLE "ob-poc".attribute_values ADD CONSTRAINT attribute_values_pkey PRIMARY KEY (av_id);
ALTER TABLE "ob-poc".cbu_entity_roles ADD CONSTRAINT cbu_entity_roles_pkey PRIMARY KEY (cbu_entity_role_id);
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT cbus_pkey PRIMARY KEY (cbu_id);
ALTER TABLE "ob-poc".crud_operations ADD CONSTRAINT crud_operations_pkey PRIMARY KEY (operation_id);
ALTER TABLE "ob-poc".dictionary ADD CONSTRAINT dictionary_pkey PRIMARY KEY (attribute_id);
ALTER TABLE "ob-poc".document_catalog ADD CONSTRAINT document_catalog_pkey PRIMARY KEY (doc_id);
ALTER TABLE "ob-poc".document_metadata ADD CONSTRAINT document_metadata_pkey PRIMARY KEY (doc_id, attribute_id);
ALTER TABLE "ob-poc".document_relationships ADD CONSTRAINT document_relationships_pkey PRIMARY KEY (relationship_id);
ALTER TABLE "ob-poc".document_types ADD CONSTRAINT document_types_pkey PRIMARY KEY (type_id);
ALTER TABLE "ob-poc".domain_vocabularies ADD CONSTRAINT domain_vocabularies_pkey PRIMARY KEY (vocab_id);
ALTER TABLE "ob-poc".dsl_domains ADD CONSTRAINT dsl_domains_pkey PRIMARY KEY (domain_id);
ALTER TABLE "ob-poc".dsl_examples ADD CONSTRAINT dsl_examples_pkey PRIMARY KEY (example_id);
ALTER TABLE "ob-poc".dsl_execution_log ADD CONSTRAINT dsl_execution_log_pkey PRIMARY KEY (execution_id);
ALTER TABLE "ob-poc".dsl_ob ADD CONSTRAINT dsl_ob_pkey PRIMARY KEY (version_id);
ALTER TABLE "ob-poc".dsl_versions ADD CONSTRAINT dsl_versions_pkey PRIMARY KEY (version_id);
ALTER TABLE "ob-poc".entities ADD CONSTRAINT entities_pkey PRIMARY KEY (entity_id);
ALTER TABLE "ob-poc".entity_crud_rules ADD CONSTRAINT entity_crud_rules_pkey PRIMARY KEY (rule_id);
ALTER TABLE "ob-poc".entity_lifecycle_status ADD CONSTRAINT entity_lifecycle_status_pkey PRIMARY KEY (status_id);
ALTER TABLE "ob-poc".entity_limited_companies ADD CONSTRAINT entity_limited_companies_pkey PRIMARY KEY (limited_company_id);
ALTER TABLE "ob-poc".entity_partnerships ADD CONSTRAINT entity_partnerships_pkey PRIMARY KEY (partnership_id);
ALTER TABLE "ob-poc".entity_product_mappings ADD CONSTRAINT entity_product_mappings_pkey PRIMARY KEY (product_id, entity_type);
ALTER TABLE "ob-poc".entity_proper_persons ADD CONSTRAINT entity_proper_persons_pkey PRIMARY KEY (proper_person_id);
ALTER TABLE "ob-poc".entity_trusts ADD CONSTRAINT entity_trusts_pkey PRIMARY KEY (trust_id);
ALTER TABLE "ob-poc".entity_types ADD CONSTRAINT entity_types_pkey PRIMARY KEY (entity_type_id);
ALTER TABLE "ob-poc".entity_validation_rules ADD CONSTRAINT entity_validation_rules_pkey PRIMARY KEY (rule_id);
ALTER TABLE "ob-poc".grammar_rules ADD CONSTRAINT grammar_rules_pkey PRIMARY KEY (rule_id);
ALTER TABLE "ob-poc".master_entity_xref ADD CONSTRAINT master_entity_xref_pkey PRIMARY KEY (xref_id);
ALTER TABLE "ob-poc".master_jurisdictions ADD CONSTRAINT master_jurisdictions_pkey PRIMARY KEY (jurisdiction_code);
ALTER TABLE "ob-poc".orchestration_domain_sessions ADD CONSTRAINT orchestration_domain_sessions_pkey PRIMARY KEY (id);
ALTER TABLE "ob-poc".orchestration_sessions ADD CONSTRAINT orchestration_sessions_pkey PRIMARY KEY (session_id);
ALTER TABLE "ob-poc".orchestration_state_history ADD CONSTRAINT orchestration_state_history_pkey PRIMARY KEY (id);
ALTER TABLE "ob-poc".orchestration_tasks ADD CONSTRAINT orchestration_tasks_pkey PRIMARY KEY (task_id);
ALTER TABLE "ob-poc".parsed_asts ADD CONSTRAINT parsed_asts_pkey PRIMARY KEY (ast_id);
ALTER TABLE "ob-poc".partnership_control_mechanisms ADD CONSTRAINT partnership_control_mechanisms_pkey PRIMARY KEY (control_mechanism_id);
ALTER TABLE "ob-poc".partnership_interests ADD CONSTRAINT partnership_interests_pkey PRIMARY KEY (interest_id);
ALTER TABLE "ob-poc".prod_resources ADD CONSTRAINT prod_resources_pkey PRIMARY KEY (resource_id);
ALTER TABLE "ob-poc".product_requirements ADD CONSTRAINT product_requirements_pkey PRIMARY KEY (product_id);
ALTER TABLE "ob-poc".product_services ADD CONSTRAINT product_services_pkey PRIMARY KEY (product_id, service_id);
ALTER TABLE "ob-poc".product_workflows ADD CONSTRAINT product_workflows_pkey PRIMARY KEY (workflow_id);
ALTER TABLE "ob-poc".products ADD CONSTRAINT products_pkey PRIMARY KEY (product_id);
ALTER TABLE "ob-poc".rag_embeddings ADD CONSTRAINT rag_embeddings_pkey PRIMARY KEY (embedding_id);
ALTER TABLE "ob-poc".roles ADD CONSTRAINT roles_pkey PRIMARY KEY (role_id);
ALTER TABLE "ob-poc".schema_changes ADD CONSTRAINT schema_changes_pkey PRIMARY KEY (change_id);
ALTER TABLE "ob-poc".service_resources ADD CONSTRAINT service_resources_pkey PRIMARY KEY (service_id, resource_id);
ALTER TABLE "ob-poc".services ADD CONSTRAINT services_pkey PRIMARY KEY (service_id);
ALTER TABLE "ob-poc".trust_beneficiary_classes ADD CONSTRAINT trust_beneficiary_classes_pkey PRIMARY KEY (beneficiary_class_id);
ALTER TABLE "ob-poc".trust_parties ADD CONSTRAINT trust_parties_pkey PRIMARY KEY (trust_party_id);
ALTER TABLE "ob-poc".trust_protector_powers ADD CONSTRAINT trust_protector_powers_pkey PRIMARY KEY (protector_power_id);
ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT ubo_registry_pkey PRIMARY KEY (ubo_id);
ALTER TABLE "ob-poc".verb_registry ADD CONSTRAINT verb_registry_pkey PRIMARY KEY (verb);
ALTER TABLE "ob-poc".vocabulary_audit ADD CONSTRAINT vocabulary_audit_pkey PRIMARY KEY (audit_id);

-- ====================================
-- UNIQUE CONSTRAINTS
-- ====================================

ALTER TABLE "ob-poc".attribute_values ADD CONSTRAINT attribute_values_cbu_id_dsl_version_attribute_id_key UNIQUE (cbu_id, dsl_version, attribute_id);
ALTER TABLE "ob-poc".cbu_entity_roles ADD CONSTRAINT cbu_entity_roles_cbu_id_entity_id_role_id_key UNIQUE (cbu_id, entity_id, role_id);
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT cbus_name_key UNIQUE (name);
ALTER TABLE "ob-poc".dictionary ADD CONSTRAINT dictionary_name_key UNIQUE (name);
ALTER TABLE "ob-poc".document_catalog ADD CONSTRAINT document_catalog_file_hash_sha256_key UNIQUE (file_hash_sha256);
ALTER TABLE "ob-poc".document_relationships ADD CONSTRAINT document_relationships_primary_doc_id_related_doc_id_relati_key UNIQUE (primary_doc_id, related_doc_id, relationship_type);
ALTER TABLE "ob-poc".document_types ADD CONSTRAINT document_types_type_code_key UNIQUE (type_code);
ALTER TABLE "ob-poc".dsl_domains ADD CONSTRAINT dsl_domains_domain_name_key UNIQUE (domain_name);
ALTER TABLE "ob-poc".dsl_versions ADD CONSTRAINT dsl_versions_domain_id_version_number_key UNIQUE (domain_id, version_number);
ALTER TABLE "ob-poc".entity_lifecycle_status ADD CONSTRAINT entity_lifecycle_status_entity_type_entity_id_status_code_e_key UNIQUE (entity_type, entity_id, status_code, effective_date);
ALTER TABLE "ob-poc".entity_types ADD CONSTRAINT entity_types_name_key UNIQUE (name);
ALTER TABLE "ob-poc".grammar_rules ADD CONSTRAINT grammar_rules_rule_name_key UNIQUE (rule_name);
ALTER TABLE "ob-poc".orchestration_domain_sessions ADD CONSTRAINT orchestration_domain_sessions_orchestration_session_id_doma_key UNIQUE (orchestration_session_id, domain_name);
ALTER TABLE "ob-poc".parsed_asts ADD CONSTRAINT parsed_asts_version_id_key UNIQUE (version_id);
ALTER TABLE "ob-poc".partnership_interests ADD CONSTRAINT partnership_interests_partnership_id_entity_id_partner_type_key UNIQUE (partnership_id, entity_id, partner_type);
ALTER TABLE "ob-poc".prod_resources ADD CONSTRAINT prod_resources_name_key UNIQUE (name);
ALTER TABLE "ob-poc".products ADD CONSTRAINT products_name_key UNIQUE (name);
ALTER TABLE "ob-poc".roles ADD CONSTRAINT roles_name_key UNIQUE (name);
ALTER TABLE "ob-poc".services ADD CONSTRAINT services_name_key UNIQUE (name);
ALTER TABLE "ob-poc".trust_parties ADD CONSTRAINT trust_parties_trust_id_entity_id_party_role_key UNIQUE (trust_id, entity_id, party_role);
ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT ubo_registry_subject_entity_id_ubo_proper_person_id_relatio_key UNIQUE (subject_entity_id, ubo_proper_person_id, relationship_type);

-- ====================================
-- FOREIGN KEY CONSTRAINTS
-- ====================================

ALTER TABLE "ob-poc".attribute_values ADD CONSTRAINT attribute_values_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary (attribute_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".attribute_values ADD CONSTRAINT attribute_values_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus (cbu_id);
ALTER TABLE "ob-poc".attribute_values ADD CONSTRAINT fk_attribute_values_dsl_ob_id FOREIGN KEY (dsl_ob_id) REFERENCES "ob-poc".dsl_ob (version_id) ON DELETE SET NULL;
ALTER TABLE "ob-poc".cbu_entity_roles ADD CONSTRAINT cbu_entity_roles_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".cbu_entity_roles ADD CONSTRAINT cbu_entity_roles_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".cbu_entity_roles ADD CONSTRAINT cbu_entity_roles_role_id_fkey FOREIGN KEY (role_id) REFERENCES "ob-poc".roles (role_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".crud_operations ADD CONSTRAINT crud_operations_parent_operation_id_fkey FOREIGN KEY (parent_operation_id) REFERENCES "ob-poc".crud_operations (operation_id);
ALTER TABLE "ob-poc".document_metadata ADD CONSTRAINT document_metadata_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary (attribute_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".document_metadata ADD CONSTRAINT document_metadata_doc_id_fkey FOREIGN KEY (doc_id) REFERENCES "ob-poc".document_catalog (doc_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".document_relationships ADD CONSTRAINT document_relationships_primary_doc_id_fkey FOREIGN KEY (primary_doc_id) REFERENCES "ob-poc".document_catalog (doc_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".document_relationships ADD CONSTRAINT document_relationships_related_doc_id_fkey FOREIGN KEY (related_doc_id) REFERENCES "ob-poc".document_catalog (doc_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".dsl_execution_log ADD CONSTRAINT dsl_execution_log_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions (version_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".dsl_ob ADD CONSTRAINT fk_dsl_ob_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".dsl_versions ADD CONSTRAINT dsl_versions_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES "ob-poc".dsl_domains (domain_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".dsl_versions ADD CONSTRAINT dsl_versions_parent_version_id_fkey FOREIGN KEY (parent_version_id) REFERENCES "ob-poc".dsl_versions (version_id);
ALTER TABLE "ob-poc".entities ADD CONSTRAINT entities_entity_type_id_fkey FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types (entity_type_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".entity_product_mappings ADD CONSTRAINT entity_product_mappings_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".master_entity_xref ADD CONSTRAINT master_entity_xref_jurisdiction_code_fkey FOREIGN KEY (jurisdiction_code) REFERENCES "ob-poc".master_jurisdictions (jurisdiction_code);
ALTER TABLE "ob-poc".orchestration_domain_sessions ADD CONSTRAINT orchestration_domain_sessions_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions (session_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".orchestration_sessions ADD CONSTRAINT orchestration_sessions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus (cbu_id);
ALTER TABLE "ob-poc".orchestration_state_history ADD CONSTRAINT orchestration_state_history_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions (session_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".orchestration_tasks ADD CONSTRAINT orchestration_tasks_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions (session_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".parsed_asts ADD CONSTRAINT parsed_asts_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions (version_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".partnership_control_mechanisms ADD CONSTRAINT partnership_control_mechanisms_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".partnership_control_mechanisms ADD CONSTRAINT partnership_control_mechanisms_partnership_id_fkey FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships (partnership_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".partnership_interests ADD CONSTRAINT partnership_interests_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".partnership_interests ADD CONSTRAINT partnership_interests_partnership_id_fkey FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships (partnership_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".product_requirements ADD CONSTRAINT product_requirements_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".product_services ADD CONSTRAINT product_services_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".product_services ADD CONSTRAINT product_services_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services (service_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".product_workflows ADD CONSTRAINT product_workflows_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".product_workflows ADD CONSTRAINT product_workflows_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products (product_id);
ALTER TABLE "ob-poc".service_resources ADD CONSTRAINT service_resources_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".prod_resources (resource_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".service_resources ADD CONSTRAINT service_resources_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services (service_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".trust_beneficiary_classes ADD CONSTRAINT trust_beneficiary_classes_trust_id_fkey FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts (trust_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".trust_parties ADD CONSTRAINT trust_parties_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".trust_parties ADD CONSTRAINT trust_parties_trust_id_fkey FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts (trust_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".trust_protector_powers ADD CONSTRAINT trust_protector_powers_trust_party_id_fkey FOREIGN KEY (trust_party_id) REFERENCES "ob-poc".trust_parties (trust_party_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT ubo_registry_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT ubo_registry_subject_entity_id_fkey FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE;
ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT ubo_registry_ubo_proper_person_id_fkey FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE;

-- ====================================
-- CHECK CONSTRAINTS
-- ====================================

-- CRUD Operations Constraints
ALTER TABLE "ob-poc".crud_operations ADD CONSTRAINT crud_operations_ai_confidence_check CHECK ((ai_confidence >= 0.0 AND ai_confidence <= 1.0));
ALTER TABLE "ob-poc".crud_operations ADD CONSTRAINT crud_operations_asset_type_check CHECK (asset_type IN ('CBU', 'ENTITY', 'PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 'TRUST', 'ATTRIBUTE', 'DOCUMENT'));
ALTER TABLE "ob-poc".crud_operations ADD CONSTRAINT crud_operations_execution_status_check CHECK (execution_status IN ('PENDING', 'EXECUTING', 'COMPLETED', 'FAILED', 'ROLLED_BACK'));
ALTER TABLE "ob-poc".crud_operations ADD CONSTRAINT crud_operations_operation_type_check CHECK (operation_type IN ('CREATE', 'READ', 'UPDATE', 'DELETE'));

-- DSL Examples Constraints
ALTER TABLE "ob-poc".dsl_examples ADD CONSTRAINT dsl_examples_complexity_level_check CHECK (complexity_level IN ('SIMPLE', 'MEDIUM', 'COMPLEX'));
ALTER TABLE "ob-poc".dsl_examples ADD CONSTRAINT dsl_examples_asset_type_check CHECK (asset_type IN ('CBU', 'ENTITY', 'PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 'TRUST', 'ATTRIBUTE', 'DOCUMENT'));
ALTER TABLE "ob-poc".dsl_examples ADD CONSTRAINT dsl_examples_operation_type_check CHECK (operation_type IN ('CREATE', 'READ', 'UPDATE', 'DELETE'));

-- Entity CRUD Rules Constraints
ALTER TABLE "ob-poc".entity_crud_rules ADD CONSTRAINT entity_crud_rules_operation_type_check CHECK (operation_type IN ('CREATE', 'READ', 'UPDATE', 'DELETE'));
ALTER TABLE "ob-poc".entity_crud_rules ADD CONSTRAINT entity_crud_rules_constraint_type_check CHECK (constraint_type IN ('REQUIRED', 'UNIQUE', 'FOREIGN_KEY', 'VALIDATION', 'BUSINESS_RULE'));

-- Entity Validation Rules Constraints
ALTER TABLE "ob-poc".entity_validation_rules ADD CONSTRAINT entity_validation_rules_validation_type_check CHECK (validation_type IN ('REQUIRED', 'FORMAT', 'RANGE', 'REFERENCE', 'CUSTOM'));
ALTER TABLE "ob-poc".entity_validation_rules ADD CONSTRAINT entity_validation_rules_severity_check CHECK (severity IN ('ERROR', 'WARNING', 'INFO'));

-- Master Entity Cross-Reference Constraints
ALTER TABLE "ob-poc".master_entity_xref ADD CONSTRAINT master_entity_xref_entity_type_check CHECK (entity_type IN ('PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 'TRUST'));
ALTER TABLE "ob-poc".master_entity_xref ADD CONSTRAINT master_entity_xref_entity_status_check CHECK (entity_status IN ('ACTIVE', 'INACTIVE', 'DISSOLVED', 'SUSPENDED'));

-- RAG Embeddings Constraints
ALTER TABLE "ob-poc".rag_embeddings ADD CONSTRAINT rag_embeddings_content_type_check CHECK (content_type IN ('SCHEMA', 'EXAMPLE', 'ATTRIBUTE', 'RULE', 'GRAMMAR', 'VERB_PATTERN'));

-- ====================================
-- CREATE INDEXES
-- ====================================

-- Attribute values indexes
CREATE INDEX IF NOT EXISTS idx_attr_vals_lookup ON "ob-poc".attribute_values (cbu_id, attribute_id, dsl_version);

-- CBU entity roles indexes
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_cbu ON "ob-poc".cbu_entity_roles (cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_entity ON "ob-poc".cbu_entity_roles (entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_role ON "ob-poc".cbu_entity_roles (role_id);

-- CBUs indexes
CREATE INDEX IF NOT EXISTS idx_cbus_name ON "ob-poc".cbus (name);

-- CRUD operations indexes
CREATE INDEX IF NOT EXISTS idx_crud_operations_asset ON "ob-poc".crud_operations (asset_type);
CREATE INDEX IF NOT EXISTS idx_crud_operations_created ON "ob-poc".crud_operations (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_crud_operations_parent ON "ob-poc".crud_operations (parent_operation_id) WHERE parent_operation_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_crud_operations_status ON "ob-poc".crud_operations (execution_status);
CREATE INDEX IF NOT EXISTS idx_crud_operations_transaction ON "ob-poc".crud_operations (transaction_id) WHERE transaction_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_crud_operations_type ON "ob-poc".crud_operations (operation_type);

-- Dictionary indexes
CREATE INDEX IF NOT EXISTS idx_dictionary_domain ON "ob-poc".dictionary (domain);
CREATE INDEX IF NOT EXISTS idx_dictionary_group_id ON "ob-poc".dictionary (group_id);
CREATE INDEX IF NOT EXISTS idx_dictionary_name ON "ob-poc".dictionary (name);

-- ====================================
-- COMMENTS
-- ====================================

COMMENT ON SCHEMA "ob-poc" IS 'OB-POC Ultimate Beneficial Ownership and onboarding system schema';
COMMENT ON TABLE "ob-poc".cbus IS 'Client Business Units - primary entities in the system';
COMMENT ON TABLE "ob-poc".dictionary IS 'Universal attribute dictionary with AttributeID-as-Type pattern';
COMMENT ON TABLE "ob-poc".attribute_values IS 'Runtime attribute values linked to dictionary';
COMMENT ON TABLE "ob-poc".entities IS 'Entity registry for all types (partnerships, companies, persons, trusts)';
COMMENT ON TABLE "ob-poc".dsl_ob IS 'DSL onboarding documents storage';
COMMENT ON TABLE "ob-poc".ubo_registry IS 'Ultimate beneficial ownership registry';
COMMENT ON TABLE "ob-poc".crud_operations IS 'Agentic CRUD operations tracking and auditing';

-- ====================================
-- END OF SCHEMA
-- ====================================