/*
v3: Refactors the attributes table to be the central dictionary.
- Uses JSONB to store rich, complex metadata for sources and sinks.
- Renames to 'dictionary' as it's the master table.
- Removes the old 'dictionaries' and 'dictionary_attributes' tables,
  as an attribute's 'dictionary_id' (now 'group_id') is just a string for grouping.
- **Sets main schema to "ob-poc"**
*/
CREATE SCHEMA IF NOT EXISTS "ob-poc";

-- Table to store immutable, versioned DSL files
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_ob (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id VARCHAR(255) NOT NULL,
    dsl_text TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_dsl_ob_cbu_id_created_at
ON "ob-poc".dsl_ob (cbu_id, created_at DESC);

-- CBU table: Client Business Unit definitions
CREATE TABLE IF NOT EXISTS "ob-poc".cbus (
    cbu_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    nature_purpose TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_cbus_name ON "ob-poc".cbus (name);

-- Products table: Core product definitions
CREATE TABLE IF NOT EXISTS "ob-poc".products (
    product_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_products_name ON "ob-poc".products (name);

-- Services table: Services that can be offered with or without products
CREATE TABLE IF NOT EXISTS "ob-poc".services (
    service_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_services_name ON "ob-poc".services (name);

-- Product <-> Service Join Table
CREATE TABLE IF NOT EXISTS "ob-poc".product_services (
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE,
    service_id UUID NOT NULL REFERENCES "ob-poc".services (service_id) ON DELETE CASCADE,
    PRIMARY KEY (product_id, service_id)
);

-- ============================================================================
-- PRODUCT REQUIREMENTS TABLES (PHASE 5 IMPLEMENTATION)
-- ============================================================================

-- Product Requirements table: DSL operations and attributes required per product
CREATE TABLE IF NOT EXISTS "ob-poc".product_requirements (
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE,
    entity_types JSONB NOT NULL,           -- Array of supported entity types ["TRUST", "CORPORATION", etc.]
    required_dsl JSONB NOT NULL,           -- Array of required DSL verbs for this product
    attributes JSONB NOT NULL,             -- Array of required attribute IDs
    compliance JSONB NOT NULL,             -- Array of compliance rules (ProductComplianceRule objects)
    prerequisites JSONB NOT NULL,          -- Array of prerequisite operations that must complete first
    conditional_rules JSONB NOT NULL,      -- Array of conditional rules (ProductConditionalRule objects)
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    PRIMARY KEY (product_id)
);

-- Entity Product Mappings table: Compatibility matrix for entity types and products
CREATE TABLE IF NOT EXISTS "ob-poc".entity_product_mappings (
    entity_type VARCHAR(100) NOT NULL,     -- TRUST, CORPORATION, PARTNERSHIP, PROPER_PERSON
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id) ON DELETE CASCADE,
    compatible BOOLEAN NOT NULL,           -- Whether this entity type can use this product
    restrictions JSONB,                    -- Array of restriction descriptions
    required_fields JSONB,                 -- Array of additional fields required for this combination
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    PRIMARY KEY (entity_type, product_id)
);

-- Product Workflows table: Generated workflows for specific product-entity combinations
CREATE TABLE IF NOT EXISTS "ob-poc".product_workflows (
    workflow_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id VARCHAR(255) NOT NULL,
    product_id UUID NOT NULL REFERENCES "ob-poc".products (product_id),
    entity_type VARCHAR(100) NOT NULL,
    required_dsl JSONB NOT NULL,           -- Array of DSL verbs for this workflow
    generated_dsl TEXT NOT NULL,           -- Complete generated DSL document
    compliance_rules JSONB NOT NULL,       -- Array of applicable compliance rules
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING', -- PENDING, GENERATING, READY, EXECUTING, COMPLETED, FAILED
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Indexes for product requirements tables
CREATE INDEX IF NOT EXISTS idx_product_requirements_product ON "ob-poc".product_requirements (product_id);
CREATE INDEX IF NOT EXISTS idx_entity_product_mappings_entity ON "ob-poc".entity_product_mappings (entity_type);
CREATE INDEX IF NOT EXISTS idx_entity_product_mappings_product ON "ob-poc".entity_product_mappings (product_id);
CREATE INDEX IF NOT EXISTS idx_entity_product_mappings_compatible ON "ob-poc".entity_product_mappings (compatible);
CREATE INDEX IF NOT EXISTS idx_product_workflows_cbu ON "ob-poc".product_workflows (cbu_id);
CREATE INDEX IF NOT EXISTS idx_product_workflows_status ON "ob-poc".product_workflows (status);
CREATE INDEX IF NOT EXISTS idx_product_workflows_product_entity ON "ob-poc".product_workflows (product_id, entity_type);

-- ============================================================================
-- GRAMMAR AND VOCABULARY TABLES (PHASE 4 MIGRATION)
-- ============================================================================

-- DSL Grammar Rules - Database-stored EBNF grammar definitions
CREATE TABLE IF NOT EXISTS "ob-poc".grammar_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_name VARCHAR(100) NOT NULL UNIQUE, -- e.g., "s_expression", "verb_call", "attribute_ref"
    rule_definition TEXT NOT NULL,          -- EBNF rule definition
    rule_type VARCHAR(50) NOT NULL DEFAULT 'production', -- 'production', 'terminal', 'lexical'
    domain VARCHAR(100),                    -- Domain this rule applies to, NULL for universal
    version VARCHAR(20) DEFAULT '1.0.0',   -- Grammar version
    active BOOLEAN DEFAULT true,           -- Enable/disable rules
    description TEXT,                      -- Human-readable description
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_grammar_rules_name ON "ob-poc".grammar_rules (rule_name);
CREATE INDEX IF NOT EXISTS idx_grammar_rules_domain ON "ob-poc".grammar_rules (domain);
CREATE INDEX IF NOT EXISTS idx_grammar_rules_active ON "ob-poc".grammar_rules (active);

-- Domain Vocabularies - Database-stored DSL verbs by domain
CREATE TABLE IF NOT EXISTS "ob-poc".domain_vocabularies (
    vocab_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,          -- e.g., "onboarding", "kyc", "hedge-fund-investor"
    verb VARCHAR(100) NOT NULL,            -- e.g., "case.create", "kyc.start", "investor.create-opportunity"
    category VARCHAR(50),                  -- e.g., "case_management", "compliance", "workflow"
    description TEXT,                      -- Human-readable description
    parameters JSONB,                      -- Parameter definitions as JSON
    examples JSONB,                        -- Usage examples as JSON array
    phase VARCHAR(20),                     -- Implementation phase (for migration tracking)
    active BOOLEAN DEFAULT true,          -- Enable/disable verbs
    version VARCHAR(20) DEFAULT '1.0.0',  -- Vocabulary version
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_domain_vocabularies_domain_verb ON "ob-poc".domain_vocabularies (domain, verb);
CREATE INDEX IF NOT EXISTS idx_domain_vocabularies_category ON "ob-poc".domain_vocabularies (category);
CREATE INDEX IF NOT EXISTS idx_domain_vocabularies_active ON "ob-poc".domain_vocabularies (active);

-- Cross-Domain Verb Registry - Global verb registry for conflict detection
CREATE TABLE IF NOT EXISTS "ob-poc".verb_registry (
    verb VARCHAR(100) PRIMARY KEY,        -- The actual verb (e.g., "case.create")
    primary_domain VARCHAR(100) NOT NULL, -- Domain that "owns" this verb
    shared BOOLEAN DEFAULT false,         -- Can be used by multiple domains
    deprecated BOOLEAN DEFAULT false,     -- Mark for deprecation
    replacement_verb VARCHAR(100),        -- If deprecated, what replaces it
    description TEXT,                     -- Global description
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_verb_registry_domain ON "ob-poc".verb_registry (primary_domain);
CREATE INDEX IF NOT EXISTS idx_verb_registry_shared ON "ob-poc".verb_registry (shared);
CREATE INDEX IF NOT EXISTS idx_verb_registry_deprecated ON "ob-poc".verb_registry (deprecated);

-- Vocabulary Change Audit - Track all vocabulary changes for compliance
CREATE TABLE IF NOT EXISTS "ob-poc".vocabulary_audit (
    audit_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain VARCHAR(100) NOT NULL,
    verb VARCHAR(100) NOT NULL,
    change_type VARCHAR(20) NOT NULL,     -- 'CREATE', 'UPDATE', 'DELETE', 'DEPRECATE'
    old_definition JSONB,                 -- Previous state (for UPDATE/DELETE)
    new_definition JSONB,                 -- New state (for CREATE/UPDATE)
    changed_by VARCHAR(255),              -- User/system that made the change
    change_reason TEXT,                   -- Why the change was made
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_vocabulary_audit_domain_verb ON "ob-poc".vocabulary_audit (domain, verb);
CREATE INDEX IF NOT EXISTS idx_vocabulary_audit_change_type ON "ob-poc".vocabulary_audit (change_type);
CREATE INDEX IF NOT EXISTS idx_vocabulary_audit_created_at ON "ob-poc".vocabulary_audit (created_at DESC);

-- ============================================================================
-- DICTIONARY AND RESOURCE TABLES (REFACTORED)
-- ============================================================================

-- Master Data Dictionary (Attributes table)
-- This is the central pillar.
CREATE TABLE IF NOT EXISTS "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The unique "variable name" for the DSL, e.g., "entity.legal_name"
    name VARCHAR(255) NOT NULL UNIQUE,

    -- Description for AI agent discovery and human readability
    long_description TEXT,

    -- The "dictionary" this attribute belongs to (e.g., "KYC", "Settlement")
    -- This replaces the old 'dictionaries' table.
    group_id VARCHAR(100) NOT NULL DEFAULT 'default',

    -- Metadata
    mask VARCHAR(50) DEFAULT 'string', -- 'string', 'ssn', 'date'
    domain VARCHAR(100), -- 'KYC', 'AML', 'Trading', 'Settlement'
    vector TEXT,         -- For AI semantic search

    -- Rich metadata stored as JSON
    source JSONB,        -- See SourceMetadata struct in Go
    sink JSONB,          -- See SinkMetadata struct in Go

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_dictionary_name ON "ob-poc".dictionary (name);
CREATE INDEX IF NOT EXISTS idx_dictionary_group_id ON "ob-poc".dictionary (group_id);
CREATE INDEX IF NOT EXISTS idx_dictionary_domain ON "ob-poc".dictionary (domain);

-- Attribute Values table: Runtime values for onboarding instances
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values (
    av_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id        UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    dsl_ob_id     UUID,                  -- optional: reference precise DSL row, if you store dsl_ob.id
    dsl_version   INTEGER NOT NULL,      -- tie values to the exact runbook snapshot
    attribute_id  UUID NOT NULL REFERENCES "ob-poc".dictionary (attribute_id) ON DELETE CASCADE,
    value         JSONB NOT NULL,
    state         TEXT NOT NULL DEFAULT 'resolved', -- 'pending' | 'resolved' | 'invalid'
    source        JSONB,                 -- provenance (table/column/system/collector)
    observed_at   TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (cbu_id, dsl_version, attribute_id)
);

CREATE INDEX IF NOT EXISTS idx_attr_vals_lookup ON "ob-poc".attribute_values (cbu_id, attribute_id, dsl_version);

-- Production Resources table
CREATE TABLE IF NOT EXISTS "ob-poc".prod_resources (
    resource_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    owner VARCHAR(255) NOT NULL,

    -- A resource is now defined by its "dictionary_group"
    -- This replaces the foreign key to the old 'dictionaries' table.
    -- e.g., "CustodyAccount" resource uses the "CustodyAccount" group_id.
    dictionary_group VARCHAR(100),

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_prod_resources_name ON "ob-poc".prod_resources (name);
CREATE INDEX IF NOT EXISTS idx_prod_resources_owner ON "ob-poc".prod_resources (owner);
CREATE INDEX IF NOT EXISTS idx_prod_resources_dict_group ON "ob-poc".prod_resources (dictionary_group);


-- Service <-> Resource Join Table
CREATE TABLE IF NOT EXISTS "ob-poc".service_resources (
    service_id UUID NOT NULL REFERENCES "ob-poc".services (service_id) ON DELETE CASCADE,
    resource_id UUID NOT NULL REFERENCES "ob-poc".prod_resources (resource_id) ON DELETE CASCADE,
    PRIMARY KEY (service_id, resource_id)
);

-- ============================================================================
-- ENTITY RELATIONSHIP MODEL
-- ============================================================================

-- Roles table: Defines roles entities can play within a CBU
CREATE TABLE IF NOT EXISTS "ob-poc".roles (
    role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_roles_name ON "ob-poc".roles (name);

-- Entity Types table: Defines the different types of entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_types (
    entity_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    table_name VARCHAR(255) NOT NULL, -- Points to specific entity type table
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_entity_types_name ON "ob-poc".entity_types (name);
CREATE INDEX IF NOT EXISTS idx_entity_types_table ON "ob-poc".entity_types (table_name);

-- Entities table: Central entity registry
CREATE TABLE IF NOT EXISTS "ob-poc".entities (
    entity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type_id UUID NOT NULL REFERENCES "ob-poc".entity_types (entity_type_id) ON DELETE CASCADE,
    external_id VARCHAR(255), -- Reference to the specific entity type table
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_entities_type ON "ob-poc".entities (entity_type_id);
CREATE INDEX IF NOT EXISTS idx_entities_external_id ON "ob-poc".entities (external_id);
CREATE INDEX IF NOT EXISTS idx_entities_name ON "ob-poc".entities (name);

-- CBU Entity Roles table: Links CBUs to entities through roles
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_entity_roles (
    cbu_entity_role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES "ob-poc".roles (role_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (cbu_id, entity_id, role_id)
);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_cbu ON "ob-poc".cbu_entity_roles (cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_entity ON "ob-poc".cbu_entity_roles (entity_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_role ON "ob-poc".cbu_entity_roles (role_id);

-- ============================================================================
-- ENTITY TYPE TABLES
-- ============================================================================

-- Limited Company entity type
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
CREATE INDEX IF NOT EXISTS idx_limited_companies_reg_num ON "ob-poc".entity_limited_companies (registration_number);
CREATE INDEX IF NOT EXISTS idx_limited_companies_jurisdiction ON "ob-poc".entity_limited_companies (jurisdiction);

-- Partnership entity type
CREATE TABLE IF NOT EXISTS "ob-poc".entity_partnerships (
    partnership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_name VARCHAR(255) NOT NULL,
    partnership_type VARCHAR(100), -- 'General', 'Limited', 'Limited Liability'
    jurisdiction VARCHAR(100),
    formation_date DATE,
    principal_place_business TEXT,
    partnership_agreement_date DATE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_partnerships_type ON "ob-poc".entity_partnerships (partnership_type);
CREATE INDEX IF NOT EXISTS idx_partnerships_jurisdiction ON "ob-poc".entity_partnerships (jurisdiction);

-- Proper Person entity type
CREATE TABLE IF NOT EXISTS "ob-poc".entity_proper_persons (
    proper_person_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    first_name VARCHAR(255) NOT NULL,
    last_name VARCHAR(255) NOT NULL,
    middle_names VARCHAR(255),
    date_of_birth DATE,
    nationality VARCHAR(100),
    residence_address TEXT,
    id_document_type VARCHAR(100), -- 'Passport', 'National ID', 'Driving License'
    id_document_number VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_proper_persons_full_name ON "ob-poc".entity_proper_persons (last_name, first_name);
CREATE INDEX IF NOT EXISTS idx_proper_persons_nationality ON "ob-poc".entity_proper_persons (nationality);
CREATE INDEX IF NOT EXISTS idx_proper_persons_id_document ON "ob-poc".entity_proper_persons (id_document_type, id_document_number);

-- Trust entity type
CREATE TABLE IF NOT EXISTS "ob-poc".entity_trusts (
    trust_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_name VARCHAR(255) NOT NULL,
    trust_type VARCHAR(100), -- 'Discretionary', 'Fixed Interest', 'Unit Trust', 'Charitable'
    jurisdiction VARCHAR(100) NOT NULL,
    establishment_date DATE,
    trust_deed_date DATE,
    trust_purpose TEXT,
    governing_law VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_trusts_type ON "ob-poc".entity_trusts (trust_type);
CREATE INDEX IF NOT EXISTS idx_trusts_jurisdiction ON "ob-poc".entity_trusts (jurisdiction);

-- ============================================================================
-- TRUST PARTY RELATIONSHIPS (Trust-Specific UBO Structure)
-- ============================================================================

-- Trust Parties table: Defines the different roles within a trust
CREATE TABLE IF NOT EXISTS "ob-poc".trust_parties (
    trust_party_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts (trust_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    party_role VARCHAR(100) NOT NULL, -- 'SETTLOR', 'TRUSTEE', 'BENEFICIARY', 'PROTECTOR'
    party_type VARCHAR(100) NOT NULL, -- 'PROPER_PERSON', 'CORPORATE_TRUSTEE', 'BENEFICIARY_CLASS'
    appointment_date DATE,
    resignation_date DATE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (trust_id, entity_id, party_role)
);
CREATE INDEX IF NOT EXISTS idx_trust_parties_trust ON "ob-poc".trust_parties (trust_id);
CREATE INDEX IF NOT EXISTS idx_trust_parties_entity ON "ob-poc".trust_parties (entity_id);
CREATE INDEX IF NOT EXISTS idx_trust_parties_role ON "ob-poc".trust_parties (party_role);

-- Trust Beneficiary Classes table: For class beneficiaries (e.g., "all grandchildren")
CREATE TABLE IF NOT EXISTS "ob-poc".trust_beneficiary_classes (
    beneficiary_class_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_id UUID NOT NULL REFERENCES "ob-poc".entity_trusts (trust_id) ON DELETE CASCADE,
    class_name VARCHAR(255) NOT NULL, -- "All grandchildren of John Smith"
    class_definition TEXT, -- Detailed definition of the class
    class_type VARCHAR(100), -- 'DESCENDANTS', 'SPOUSE_FAMILY', 'CHARITABLE_CLASS'
    monitoring_required BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_beneficiary_classes_trust ON "ob-poc".trust_beneficiary_classes (trust_id);

-- Trust Protector Powers table: Powers held by trust protectors
CREATE TABLE IF NOT EXISTS "ob-poc".trust_protector_powers (
    protector_power_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trust_party_id UUID NOT NULL REFERENCES "ob-poc".trust_parties (trust_party_id) ON DELETE CASCADE,
    power_type VARCHAR(100) NOT NULL, -- 'TRUSTEE_APPOINTMENT', 'TRUSTEE_REMOVAL', 'DISTRIBUTION_VETO'
    power_description TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_protector_powers_party ON "ob-poc".trust_protector_powers (trust_party_id);

-- ============================================================================
-- PARTNERSHIP STRUCTURE (Partnership-Specific UBO Structure)
-- ============================================================================

-- Partnership Interests table: Ownership and control structure for partnerships
CREATE TABLE IF NOT EXISTS "ob-poc".partnership_interests (
    interest_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL REFERENCES "ob-poc".entity_partnerships (partnership_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    partner_type VARCHAR(100) NOT NULL, -- 'GENERAL_PARTNER', 'LIMITED_PARTNER', 'MANAGING_PARTNER'
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
CREATE INDEX IF NOT EXISTS idx_partnership_interests_partnership ON "ob-poc".partnership_interests (partnership_id);
CREATE INDEX IF NOT EXISTS idx_partnership_interests_entity ON "ob-poc".partnership_interests (entity_id);
CREATE INDEX IF NOT EXISTS idx_partnership_interests_type ON "ob-poc".partnership_interests (partner_type);

-- Partnership Control Mechanisms table: How control is exercised
CREATE TABLE IF NOT EXISTS "ob-poc".partnership_control_mechanisms (
    control_mechanism_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partnership_id UUID NOT NULL REFERENCES "ob-poc".entity_partnerships (partnership_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    control_type VARCHAR(100) NOT NULL, -- 'MANAGEMENT_AGREEMENT', 'GP_CONTROL', 'INVESTMENT_COMMITTEE'
    control_description TEXT,
    effective_date DATE,
    termination_date DATE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
CREATE INDEX IF NOT EXISTS idx_partnership_control_partnership ON "ob-poc".partnership_control_mechanisms (partnership_id);
CREATE INDEX IF NOT EXISTS idx_partnership_control_entity ON "ob-poc".partnership_control_mechanisms (entity_id);

-- ============================================================================
-- UBO IDENTIFICATION RESULTS (Entity-Type-Agnostic UBO Storage)
-- ============================================================================

-- UBO Registry table: Results of UBO identification across all entity types
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_registry (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus (cbu_id) ON DELETE CASCADE,
    subject_entity_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    ubo_proper_person_id UUID NOT NULL REFERENCES "ob-poc".entities (entity_id) ON DELETE CASCADE,
    relationship_type VARCHAR(100) NOT NULL, -- 'DIRECT_OWNERSHIP', 'TRUST_SETTLOR', 'PARTNERSHIP_GP_CONTROL'
    qualifying_reason VARCHAR(100) NOT NULL, -- 'OWNERSHIP_THRESHOLD', 'TRUST_CREATOR', 'ULTIMATE_CONTROL'
    ownership_percentage DECIMAL(5,2),
    control_type VARCHAR(100),
    workflow_type VARCHAR(100) NOT NULL, -- 'STANDARD_CORPORATE', 'TRUST_SPECIFIC', 'PARTNERSHIP_DUAL_PRONG'
    regulatory_framework VARCHAR(100), -- 'EU_5MLD', 'FATF_TRUST_GUIDANCE', 'US_CDD'
    verification_status VARCHAR(50) DEFAULT 'PENDING', -- 'PENDING', 'VERIFIED', 'FAILED'
    screening_result VARCHAR(50) DEFAULT 'PENDING', -- 'CLEARED', 'FLAGGED', 'BLOCKED'
    risk_rating VARCHAR(50), -- 'LOW', 'MEDIUM', 'HIGH', 'VERY_HIGH'
    identified_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    UNIQUE (subject_entity_id, ubo_proper_person_id, relationship_type)
);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_cbu ON "ob-poc".ubo_registry (cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_subject ON "ob-poc".ubo_registry (subject_entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_ubo_proper_person ON "ob-poc".ubo_registry (ubo_proper_person_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_workflow ON "ob-poc".ubo_registry (workflow_type);

-- ============================================================================
-- MULTI-DSL ORCHESTRATION SESSIONS (Phase 1 Persistent Storage)
-- ============================================================================

-- Orchestration Sessions table: Persistent multi-domain workflow sessions
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_sessions (
    session_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    primary_domain VARCHAR(100) NOT NULL,

    -- Entity and workflow context
    cbu_id UUID REFERENCES "ob-poc".cbus (cbu_id),
    entity_type VARCHAR(50),
    entity_name TEXT,
    jurisdiction VARCHAR(10),
    products TEXT[], -- Array of product names
    services TEXT[], -- Array of service names
    workflow_type VARCHAR(50) DEFAULT 'ONBOARDING',

    -- Session state
    current_state VARCHAR(50) DEFAULT 'CREATED',
    version_number INTEGER DEFAULT 0,

    -- DSL accumulation (DSL-as-State pattern)
    unified_dsl TEXT, -- The complete accumulated DSL document

    -- Cross-domain context and execution plan (stored as JSON)
    shared_context JSONB, -- SharedContext struct as JSON
    execution_plan JSONB, -- ExecutionPlan struct as JSON
    entity_refs JSONB, -- Cross-domain entity references
    attribute_refs JSONB, -- Cross-domain attribute references

    -- Session lifecycle
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    last_used TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    expires_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc' + INTERVAL '24 hours')
);

CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_cbu ON "ob-poc".orchestration_sessions (cbu_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_entity_type ON "ob-poc".orchestration_sessions (entity_type);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_workflow ON "ob-poc".orchestration_sessions (workflow_type);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_state ON "ob-poc".orchestration_sessions (current_state);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_last_used ON "ob-poc".orchestration_sessions (last_used);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_expires ON "ob-poc".orchestration_sessions (expires_at);

-- Domain Sessions table: Specific domain sessions within orchestration
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_domain_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    domain_name VARCHAR(100) NOT NULL,
    domain_session_id UUID NOT NULL, -- Domain's internal session ID

    -- Domain state
    state VARCHAR(50) DEFAULT 'CREATED',
    contributed_dsl TEXT, -- DSL contributed by this domain
    domain_context JSONB, -- Domain-specific context
    dependencies TEXT[], -- Array of domain names this depends on

    -- Activity tracking
    last_activity TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    UNIQUE (orchestration_session_id, domain_name)
);

CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_orchestration ON "ob-poc".orchestration_domain_sessions (orchestration_session_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_domain ON "ob-poc".orchestration_domain_sessions (domain_name);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_state ON "ob-poc".orchestration_domain_sessions (state);
CREATE INDEX IF NOT EXISTS idx_orchestration_domain_sessions_activity ON "ob-poc".orchestration_domain_sessions (last_activity);

-- Orchestration Tasks table: Track workflow tasks and dependencies
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,
    domain_name VARCHAR(100) NOT NULL,

    -- Task definition
    verb VARCHAR(200) NOT NULL, -- DSL verb to execute
    parameters JSONB, -- Verb parameters as JSON
    dependencies TEXT[], -- Array of task IDs this depends on

    -- Task state
    status VARCHAR(50) DEFAULT 'PENDING', -- PENDING, SCHEDULED, RUNNING, COMPLETED, FAILED, SKIPPED
    generated_dsl TEXT, -- DSL generated by this task
    error_message TEXT, -- Error if task failed

    -- Timing
    scheduled_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_session ON "ob-poc".orchestration_tasks (orchestration_session_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_domain ON "ob-poc".orchestration_tasks (domain_name);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_status ON "ob-poc".orchestration_tasks (status);
CREATE INDEX IF NOT EXISTS idx_orchestration_tasks_scheduled ON "ob-poc".orchestration_tasks (scheduled_at);

-- State History table: Track orchestration session state transitions
CREATE TABLE IF NOT EXISTS "ob-poc".orchestration_state_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orchestration_session_id UUID NOT NULL REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE,

    -- State transition
    from_state VARCHAR(50),
    to_state VARCHAR(50) NOT NULL,
    domain_name VARCHAR(100), -- Domain that triggered the transition
    reason TEXT,
    generated_by VARCHAR(100), -- 'USER', 'AI_AGENT', 'SYSTEM'

    -- Additional context
    version_number INTEGER, -- Session version at time of transition
    metadata JSONB, -- Additional transition metadata

    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_orchestration_state_history_session ON "ob-poc".orchestration_state_history (orchestration_session_id);
CREATE INDEX IF NOT EXISTS idx_orchestration_state_history_states ON "ob-poc".orchestration_state_history (from_state, to_state);
CREATE INDEX IF NOT EXISTS idx_orchestration_state_history_created ON "ob-poc".orchestration_state_history (created_at);
