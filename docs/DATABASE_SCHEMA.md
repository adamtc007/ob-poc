# OB-POC Database Schema

**Database**: `data_designer`  
**PostgreSQL Version**: 17.x (recommended)  
**Last Updated**: 2025-11-30

## Quick Start

```bash
# Rebuild database from scratch
createdb data_designer
psql -d data_designer -f schema_export.sql
```

## Overview

Two schemas:
- **public**: Runtime API Endpoints System (16 tables)
- **ob-poc**: KYC/AML Onboarding Domain (103 tables)

**Total**: 119 tables, 5 custom functions, 2 views, 4 extensions

## Extensions

```sql
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";   -- UUID generation
CREATE EXTENSION IF NOT EXISTS pg_trgm;       -- Trigram text search
CREATE EXTENSION IF NOT EXISTS vector;        -- pgvector for embeddings
```

## Custom Types

```sql
CREATE TYPE public.action_type_enum AS ENUM (
    'HTTP_API', 'BPMN_WORKFLOW', 'MESSAGE_QUEUE', 
    'DATABASE_OPERATION', 'EXTERNAL_SERVICE'
);

CREATE TYPE public.execution_status_enum AS ENUM (
    'PENDING', 'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED'
);
```

---

## Core Domain Tables (ob-poc schema)

### CBU (Client Business Unit)

```sql
CREATE TABLE "ob-poc".cbus (
    cbu_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    name varchar(255) NOT NULL,
    description text,
    nature_purpose text,
    source_of_funds text,
    client_type varchar(100),          -- FUND, COMPANY, INDIVIDUAL, TRUST
    jurisdiction varchar(50),
    risk_context jsonb DEFAULT '{}',   -- risk_rating, pep_exposure, sanctions_exposure
    onboarding_context jsonb DEFAULT '{}',
    semantic_context jsonb DEFAULT '{}',
    embedding vector(1536),
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now()
);

CREATE UNIQUE INDEX cbus_name_jurisdiction_unique 
    ON "ob-poc".cbus(name, jurisdiction);
```

### Entity Type System (Class Table Inheritance)

```sql
-- Base entity table
CREATE TABLE "ob-poc".entities (
    entity_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_type_id uuid NOT NULL REFERENCES "ob-poc".entity_types(entity_type_id),
    external_id varchar(255),
    name varchar(255) NOT NULL,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now()
);

-- Entity type definitions
CREATE TABLE "ob-poc".entity_types (
    entity_type_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    name varchar(255) NOT NULL,
    type_code varchar(100),
    description text,
    table_name varchar(255) NOT NULL,
    parent_type_id uuid REFERENCES "ob-poc".entity_types(entity_type_id),
    type_hierarchy_path text[],
    semantic_context jsonb DEFAULT '{}',
    embedding vector(768),
    created_at timestamptz DEFAULT now()
);

-- Natural persons
CREATE TABLE "ob-poc".entity_proper_persons (
    proper_person_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    first_name varchar(255) NOT NULL,
    last_name varchar(255) NOT NULL,
    middle_names varchar(255),
    date_of_birth date,
    nationality varchar(100),
    residence_address text,
    id_document_type varchar(100),
    id_document_number varchar(100),
    search_name text GENERATED ALWAYS AS (
        COALESCE(first_name, '') || ' ' || COALESCE(last_name, '')
    ) STORED,
    created_at timestamptz DEFAULT now()
);

-- Limited companies
CREATE TABLE "ob-poc".entity_limited_companies (
    limited_company_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    company_name varchar(255) NOT NULL,
    registration_number varchar(100),
    jurisdiction varchar(100),
    incorporation_date date,
    registered_address text,
    business_nature text,
    created_at timestamptz DEFAULT now()
);

-- Partnerships
CREATE TABLE "ob-poc".entity_partnerships (
    partnership_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    partnership_name varchar(255) NOT NULL,
    partnership_type varchar(100),     -- LP, LLP, GP
    jurisdiction varchar(100),
    formation_date date,
    principal_place_business text,
    created_at timestamptz DEFAULT now()
);

-- Trusts
CREATE TABLE "ob-poc".entity_trusts (
    trust_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    trust_name varchar(255) NOT NULL,
    trust_type varchar(100),           -- Discretionary, Fixed Interest, etc.
    jurisdiction varchar(100) NOT NULL,
    establishment_date date,
    governing_law varchar(100),
    created_at timestamptz DEFAULT now()
);
```

### CBU-Entity Relationships

```sql
CREATE TABLE "ob-poc".roles (
    role_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    name varchar(255) NOT NULL UNIQUE,
    description text,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".cbu_entity_roles (
    cbu_entity_role_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    role_id uuid NOT NULL REFERENCES "ob-poc".roles(role_id),
    created_at timestamptz DEFAULT now(),
    UNIQUE(cbu_id, entity_id, role_id)
);
```

### Documents

```sql
CREATE TABLE "ob-poc".document_types (
    type_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    type_code varchar(100) NOT NULL UNIQUE,
    display_name varchar(200) NOT NULL,
    category varchar(100) NOT NULL,
    domain varchar(100),
    description text,
    required_attributes jsonb DEFAULT '{}',
    applicability jsonb DEFAULT '{}',  -- entity_types[], jurisdictions[]
    semantic_context jsonb DEFAULT '{}',
    embedding vector(768),
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".document_catalog (
    doc_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid REFERENCES "ob-poc".cbus(cbu_id),
    document_type_id uuid REFERENCES "ob-poc".document_types(type_id),
    document_type_code varchar(100),
    document_name varchar(255),
    file_hash_sha256 text,
    storage_key text,
    file_size_bytes bigint,
    mime_type varchar(100),
    status varchar(50) DEFAULT 'active',
    extraction_status varchar(50) DEFAULT 'PENDING',
    extracted_data jsonb,
    extraction_confidence numeric(5,4),
    metadata jsonb DEFAULT '{}',
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".document_entity_links (
    link_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    doc_id uuid NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),
    entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    link_type varchar(50) DEFAULT 'EVIDENCE'
        CHECK (link_type IN ('EVIDENCE','IDENTITY','ADDRESS','FINANCIAL','REGULATORY','OTHER')),
    created_at timestamptz DEFAULT now()
);
```

### Screening

```sql
CREATE TABLE "ob-poc".screenings (
    screening_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    screening_type varchar(50) NOT NULL,  -- PEP, SANCTIONS, ADVERSE_MEDIA
    status varchar(50) DEFAULT 'PENDING',
    result varchar(50),                    -- CLEAR, MATCH, POSSIBLE_MATCH
    match_count integer DEFAULT 0,
    match_details jsonb,
    screened_at timestamptz DEFAULT now(),
    provider varchar(100),
    created_at timestamptz DEFAULT now()
);
```

### KYC Investigations & Decisions

```sql
CREATE TABLE "ob-poc".kyc_investigations (
    investigation_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid REFERENCES "ob-poc".cbus(cbu_id),
    investigation_type varchar(50) NOT NULL,
    risk_rating varchar(20),
    regulatory_framework jsonb,
    ubo_threshold numeric(5,2) DEFAULT 10.0,
    status varchar(50) DEFAULT 'INITIATED',
    deadline date,
    outcome varchar(50),
    notes text,
    created_at timestamptz DEFAULT now(),
    completed_at timestamptz
);

CREATE TABLE "ob-poc".kyc_decisions (
    decision_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_id uuid REFERENCES "ob-poc".kyc_investigations(investigation_id),
    decision varchar(50) NOT NULL,         -- ACCEPT, REJECT, CONDITIONAL_ACCEPTANCE
    decision_authority varchar(100),
    rationale text,
    decided_by varchar(255),
    decided_at timestamptz DEFAULT now(),
    effective_date date DEFAULT CURRENT_DATE,
    review_date date
);

CREATE TABLE "ob-poc".decision_conditions (
    condition_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    decision_id uuid NOT NULL REFERENCES "ob-poc".kyc_decisions(decision_id),
    condition_type varchar(50) NOT NULL,
    description text,
    due_date date,
    status varchar(50) DEFAULT 'PENDING',
    satisfied_at timestamptz,
    created_at timestamptz DEFAULT now()
);
```

### Ownership & UBO

```sql
CREATE TABLE "ob-poc".ownership_relationships (
    ownership_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    owner_entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    owned_entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ownership_type varchar(30) NOT NULL
        CHECK (ownership_type IN ('DIRECT','INDIRECT','BENEFICIAL')),
    ownership_percent numeric(5,2) NOT NULL CHECK (ownership_percent BETWEEN 0 AND 100),
    effective_from date DEFAULT CURRENT_DATE,
    effective_to date,
    evidence_doc_id uuid REFERENCES "ob-poc".document_catalog(doc_id),
    created_at timestamptz DEFAULT now(),
    CONSTRAINT ownership_not_self CHECK (owner_entity_id <> owned_entity_id)
);

CREATE TABLE "ob-poc".ubo_registry (
    ubo_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id),
    total_ownership_percent numeric(5,2),
    is_pep boolean DEFAULT false,
    is_sanctioned boolean DEFAULT false,
    verification_status varchar(50) DEFAULT 'PENDING',
    verified_at timestamptz,
    created_at timestamptz DEFAULT now()
);
```

### Risk Management

```sql
CREATE TABLE "ob-poc".risk_ratings (
    rating_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    rating varchar(20) NOT NULL
        CHECK (rating IN ('LOW','MEDIUM','MEDIUM_HIGH','HIGH','VERY_HIGH','PROHIBITED')),
    previous_rating varchar(20),
    rationale text,
    effective_from timestamptz DEFAULT now(),
    effective_to timestamptz,
    set_by varchar(255) DEFAULT 'system',
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".risk_assessments (
    assessment_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid,
    entity_id uuid,
    investigation_id uuid,
    assessment_type varchar(50) NOT NULL,
    rating varchar(20),
    factors jsonb,
    methodology varchar(50),
    rationale text,
    assessed_by varchar(255),
    assessed_at timestamptz DEFAULT now(),
    CONSTRAINT risk_target CHECK (cbu_id IS NOT NULL OR entity_id IS NOT NULL)
);
```

### Products, Services & Resources

```sql
CREATE TABLE "ob-poc".products (
    product_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    product_code varchar(50) UNIQUE,
    name varchar(255) NOT NULL,
    description text,
    product_category varchar(100),
    regulatory_framework varchar(100),
    min_asset_requirement numeric(20,2),
    is_active boolean DEFAULT true,
    metadata jsonb,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".services (
    service_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    service_code varchar(50) UNIQUE,
    name varchar(255) NOT NULL,
    description text,
    service_type varchar(100),
    is_active boolean DEFAULT true,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".product_services (
    product_id uuid NOT NULL REFERENCES "ob-poc".products(product_id),
    service_id uuid NOT NULL REFERENCES "ob-poc".services(service_id),
    is_mandatory boolean DEFAULT false,
    is_default boolean DEFAULT false,
    display_order integer,
    PRIMARY KEY (product_id, service_id)
);

CREATE TABLE "ob-poc".prod_resources (
    resource_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    resource_code varchar(50),
    name varchar(255) NOT NULL,
    description text,
    resource_type varchar(100),
    owner varchar(255) NOT NULL,
    vendor varchar(255),
    api_endpoint text,
    capabilities jsonb,
    is_active boolean DEFAULT true,
    created_at timestamptz DEFAULT now()
);
```

### Resource Instances (Delivered Artifacts)

```sql
CREATE TABLE "ob-poc".cbu_resource_instances (
    instance_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id uuid REFERENCES "ob-poc".products(product_id),
    service_id uuid REFERENCES "ob-poc".services(service_id),
    resource_type_id uuid REFERENCES "ob-poc".prod_resources(resource_id),
    instance_url varchar(1024) NOT NULL,
    instance_identifier varchar(255),
    instance_name varchar(255),
    instance_config jsonb DEFAULT '{}',
    status varchar(50) DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','PROVISIONING','ACTIVE','SUSPENDED','DECOMMISSIONED')),
    requested_at timestamptz DEFAULT now(),
    provisioned_at timestamptz,
    activated_at timestamptz,
    decommissioned_at timestamptz,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".resource_instance_attributes (
    value_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    instance_id uuid NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    attribute_id uuid NOT NULL REFERENCES "ob-poc".dictionary(attribute_id),
    value_text varchar,
    value_number numeric,
    value_boolean boolean,
    value_date date,
    value_timestamp timestamptz,
    value_json jsonb,
    state varchar(50) DEFAULT 'proposed'
        CHECK (state IN ('proposed','confirmed','derived','system')),
    source jsonb,
    observed_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".service_delivery_map (
    delivery_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id uuid REFERENCES "ob-poc".products(product_id),
    service_id uuid REFERENCES "ob-poc".services(service_id),
    instance_id uuid REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    service_config jsonb,
    delivery_status varchar(50) DEFAULT 'PENDING'
        CHECK (delivery_status IN ('PENDING','IN_PROGRESS','DELIVERED','FAILED','CANCELLED')),
    requested_at timestamptz DEFAULT now(),
    delivered_at timestamptz,
    failure_reason text
);
```

### Attribute System

```sql
CREATE TABLE "ob-poc".dictionary (
    attribute_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    name varchar(255) NOT NULL,
    long_description text,
    group_id varchar(100) DEFAULT 'default',
    mask varchar(50) DEFAULT 'string',
    domain varchar(100),
    source jsonb,
    sink jsonb,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".attribute_registry (
    id text PRIMARY KEY,               -- attr.{category}.{name}
    uuid uuid NOT NULL UNIQUE,
    display_name text NOT NULL,
    category text NOT NULL
        CHECK (category IN ('identity','financial','compliance','document',
                           'risk','contact','address','tax','employment',
                           'product','entity','ubo','isda')),
    value_type text NOT NULL
        CHECK (value_type IN ('string','integer','number','boolean','date',
                             'datetime','email','phone','address','currency',
                             'percentage','tax_id','json')),
    validation_rules jsonb DEFAULT '{}',
    applicability jsonb DEFAULT '{}',
    embedding vector(1536),
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".attribute_values_typed (
    id serial PRIMARY KEY,
    entity_id uuid NOT NULL,
    attribute_id text NOT NULL,
    attribute_uuid uuid,
    value_text text,
    value_number numeric,
    value_integer bigint,
    value_boolean boolean,
    value_date date,
    value_datetime timestamptz,
    value_json jsonb,
    effective_from timestamptz DEFAULT now(),
    effective_to timestamptz,
    source jsonb,
    created_at timestamptz DEFAULT now(),
    CONSTRAINT check_single_value CHECK (
        (value_text IS NOT NULL)::int +
        (value_number IS NOT NULL)::int +
        (value_integer IS NOT NULL)::int +
        (value_boolean IS NOT NULL)::int +
        (value_date IS NOT NULL)::int +
        (value_datetime IS NOT NULL)::int +
        (value_json IS NOT NULL)::int = 1
    )
);
```

### DSL Storage & Execution

```sql
CREATE TABLE "ob-poc".dsl_ob (
    version_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    dsl_text text NOT NULL,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".dsl_instances (
    instance_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    business_reference varchar(255) NOT NULL,
    domain_name varchar(100),
    current_version integer DEFAULT 1,
    status varchar(50) DEFAULT 'PROCESSED',
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".dsl_generation_log (
    log_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    instance_id uuid,
    user_intent text NOT NULL,
    final_valid_dsl text,
    iterations jsonb DEFAULT '[]',
    domain_name varchar(50) NOT NULL,
    session_id uuid,
    cbu_id uuid,
    model_used varchar(100),
    total_attempts integer DEFAULT 1,
    success boolean DEFAULT false,
    total_latency_ms integer,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".dsl_execution_log (
    execution_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    version_id uuid NOT NULL,
    cbu_id varchar(255),
    execution_phase varchar(50) NOT NULL,
    status varchar(50) NOT NULL,
    result_data jsonb,
    error_details jsonb,
    performance_metrics jsonb,
    executed_by varchar(255),
    started_at timestamptz DEFAULT now(),
    completed_at timestamptz,
    duration_ms integer GENERATED ALWAYS AS (
        CASE WHEN completed_at IS NOT NULL 
        THEN EXTRACT(epoch FROM (completed_at - started_at)) * 1000 
        END
    ) STORED
);
```

### Monitoring & Ongoing Due Diligence

```sql
CREATE TABLE "ob-poc".monitoring_cases (
    case_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_type varchar(30) NOT NULL
        CHECK (case_type IN ('ONGOING_MONITORING','TRIGGERED_REVIEW','PERIODIC_REVIEW')),
    status varchar(30) DEFAULT 'OPEN'
        CHECK (status IN ('OPEN','UNDER_REVIEW','ESCALATED','CLOSED')),
    retention_period_years integer DEFAULT 7,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE "ob-poc".monitoring_reviews (
    review_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    case_id uuid NOT NULL REFERENCES "ob-poc".monitoring_cases(case_id),
    cbu_id uuid NOT NULL,
    review_type varchar(30) NOT NULL,
    due_date date NOT NULL,
    status varchar(30) DEFAULT 'SCHEDULED',
    outcome varchar(30),
    findings text,
    next_review_date date,
    completed_at timestamptz,
    created_at timestamptz DEFAULT now()
);
```

---

## Public Schema Tables (Runtime API)

```sql
CREATE TABLE public.resource_types (
    resource_type_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    resource_type_name varchar(100) NOT NULL UNIQUE,
    description text,
    version varchar(20) DEFAULT '1.0',
    environment varchar(50) DEFAULT 'production',
    active boolean DEFAULT true,
    created_at timestamptz DEFAULT now()
);

CREATE TABLE public.resource_type_endpoints (
    endpoint_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    resource_type_id uuid NOT NULL REFERENCES public.resource_types(resource_type_id),
    lifecycle_action varchar(50) NOT NULL,
    endpoint_url text NOT NULL,
    method varchar(10) NOT NULL,
    authentication jsonb,
    retry_config jsonb,
    environment varchar(50) DEFAULT 'production',
    UNIQUE(resource_type_id, lifecycle_action, environment)
);

CREATE TABLE public.actions_registry (
    action_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    action_name varchar(100) NOT NULL,
    verb_pattern varchar(200),
    action_type public.action_type_enum NOT NULL,
    resource_type_id uuid REFERENCES public.resource_types(resource_type_id),
    domain varchar(50),
    trigger_conditions jsonb,
    execution_config jsonb NOT NULL,
    attribute_mapping jsonb,
    success_criteria jsonb,
    failure_handling jsonb,
    active boolean DEFAULT true,
    environment varchar(50) DEFAULT 'production',
    created_at timestamptz DEFAULT now()
);

CREATE TABLE public.action_executions (
    execution_id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    action_id uuid NOT NULL REFERENCES public.actions_registry(action_id),
    cbu_id uuid REFERENCES "ob-poc".cbus(cbu_id),
    dsl_version_id uuid,
    execution_status public.execution_status_enum DEFAULT 'PENDING',
    request_payload jsonb,
    response_payload jsonb,
    retry_count integer DEFAULT 0,
    http_status integer,
    idempotency_key varchar(255),
    correlation_id varchar(255),
    started_at timestamptz DEFAULT now(),
    completed_at timestamptz
);

CREATE TABLE public.rules (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    rule_id varchar(100) NOT NULL UNIQUE,
    rule_name varchar(255) NOT NULL,
    category_id uuid,
    target_attribute_id uuid,
    rule_definition text NOT NULL,
    parsed_ast jsonb,
    status varchar(50) DEFAULT 'DRAFT',
    description text,
    embedding vector(1536),
    search_vector tsvector GENERATED ALWAYS AS (
        setweight(to_tsvector('english', COALESCE(rule_name, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(description, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(rule_definition, '')), 'C')
    ) STORED,
    created_at timestamptz DEFAULT now()
);

-- Vector similarity search index
CREATE INDEX rules_embedding_hnsw ON public.rules 
    USING hnsw (embedding vector_cosine_ops);

-- Full-text search index
CREATE INDEX rules_search_vector_gin ON public.rules 
    USING gin (search_vector);
```

---

## Custom Functions

```sql
-- Auto-update timestamps
CREATE FUNCTION public.update_updated_at_column() RETURNS trigger AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Get or create entity
CREATE FUNCTION public.ensure_entity_exists(
    p_entity_type_name varchar,
    p_entity_name varchar,
    p_external_id varchar DEFAULT NULL
) RETURNS uuid AS $$
DECLARE
    entity_type_uuid UUID;
    entity_uuid UUID;
BEGIN
    SELECT entity_type_id INTO entity_type_uuid
    FROM "ob-poc".entity_types WHERE name = p_entity_type_name;
    
    IF entity_type_uuid IS NULL THEN
        RAISE EXCEPTION 'Entity type % not found', p_entity_type_name;
    END IF;
    
    SELECT entity_id INTO entity_uuid
    FROM "ob-poc".entities
    WHERE name = p_entity_name AND entity_type_id = entity_type_uuid;
    
    IF entity_uuid IS NULL THEN
        INSERT INTO "ob-poc".entities (entity_type_id, name, external_id)
        VALUES (entity_type_uuid, p_entity_name, p_external_id)
        RETURNING entity_id INTO entity_uuid;
    END IF;
    
    RETURN entity_uuid;
END;
$$ LANGUAGE plpgsql;

-- Generate correlation ID
CREATE FUNCTION public.generate_correlation_id(
    template text,
    cbu_id_val uuid,
    action_id_val uuid,
    resource_type_name text
) RETURNS text AS $$
BEGIN
    RETURN replace(
        replace(
            replace(template, '{{cbu_id}}', cbu_id_val::text),
            '{{action_id}}', action_id_val::text
        ),
        '{{resource_type}}', resource_type_name
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Get resource endpoint URL
CREATE FUNCTION public.get_resource_endpoint_url(
    resource_type_name text,
    lifecycle_action text,
    environment_name text DEFAULT 'production'
) RETURNS text AS $$
DECLARE
    endpoint_url TEXT;
BEGIN
    SELECT rte.endpoint_url INTO endpoint_url
    FROM resource_type_endpoints rte
    JOIN resource_types rt ON rte.resource_type_id = rt.resource_type_id
    WHERE rt.resource_type_name = resource_type_name
    AND rte.lifecycle_action = lifecycle_action
    AND rte.environment = environment_name
    AND rt.active = true;
    RETURN endpoint_url;
END;
$$ LANGUAGE plpgsql;
```

---

## Key Relationships

```
cbus (1) ─────< (N) cbu_entity_roles >───── (N) entities
                         │
                         └──────────────── (N) roles

entities (1) ──< entity_proper_persons
           ──< entity_limited_companies
           ──< entity_partnerships
           ──< entity_trusts

cbus (1) ──────< (N) document_catalog >──── (N) document_types
cbus (1) ──────< (N) cbu_resource_instances
cbus (1) ──────< (N) service_delivery_map
cbus (1) ──────< (N) kyc_investigations ──< (N) kyc_decisions
cbus (1) ──────< (N) screenings
cbus (1) ──────< (N) risk_ratings
cbus (1) ──────< (N) monitoring_cases ───< (N) monitoring_reviews

entities (1) ──< (N) ownership_relationships (owner)
           ──< (N) ownership_relationships (owned)

products (1) ──< (N) product_services >──── (N) services
```

---

## Rebuild Instructions

The complete schema DDL is in `/schema_export.sql`. To rebuild:

```bash
# Drop and recreate
dropdb data_designer
createdb data_designer
psql -d data_designer -f schema_export.sql

# Or restore from backup
pg_restore -d data_designer backup.dump
```

For seed data, run the SQL files in `sql/seeds/` after schema creation.
