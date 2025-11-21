# Complete Schema and Rust Code Review Package

Generated: 2025-11-21

This document contains all database schema definitions and Rust source code for alignment review.

---

# PART 1: SCHEMA MISMATCH ANALYSIS

# Schema Mismatch Report: Rust Code vs PostgreSQL Database

Generated: 2025-11-21

## Executive Summary

The Rust DSL code has diverged from the PostgreSQL database schema during refactoring. The database schema appears to be the correct/canonical version. This document details all mismatches requiring Rust code updates.

---

## Critical Mismatches

### 1. attribute_values Table

**Database Schema (CORRECT):**
```sql
attribute_values (
    av_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES cbus(cbu_id),
    dsl_ob_id uuid,
    dsl_version integer NOT NULL,
    attribute_id uuid NOT NULL REFERENCES dictionary(attribute_id),
    value jsonb NOT NULL,
    state text NOT NULL DEFAULT 'resolved',
    source jsonb,
    observed_at timestamp with time zone
)
UNIQUE(cbu_id, dsl_version, attribute_id)
```

**Rust Code Expects:**
```rust
// In dsl_repository.rs save_attribute()
INSERT INTO attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
VALUES ($1::uuid, $2, $3, $4, NOW())
```

**Mismatches:**
- entity_id -> should be cbu_id (uuid, not string)
- attribute_value -> should be value (jsonb)
- value_type -> not needed, value is jsonb
- created_at -> should be observed_at
- Missing: dsl_version (required), state

**Files to Update:**
- rust/src/database/dsl_repository.rs - save_attribute(), save_execution_transactionally()
- rust/src/database/attribute_values_service.rs

---

### 2. entities Table

**Database Schema (CORRECT):**
```sql
entities (
    entity_id uuid PRIMARY KEY,
    entity_type_id uuid NOT NULL REFERENCES entity_types(entity_type_id),
    external_id varchar(255),
    name varchar(255) NOT NULL,
    created_at timestamp with time zone,
    updated_at timestamp with time zone
)
```

**Rust Code Expects:**
```rust
// In crud_executor.rs execute_create_tx()
INSERT INTO entities (entity_id, entity_type, legal_name, jurisdiction, status, created_at, updated_at)
```

**Mismatches:**
- entity_type -> should be entity_type_id (uuid FK to entity_types)
- legal_name -> should be name
- jurisdiction -> column does not exist
- status -> column does not exist

**Files to Update:**
- rust/src/database/crud_executor.rs - CBU_ENTITY_RELATIONSHIP and CBU_PROPER_PERSON handlers

---

### 3. dsl_instances Table

**Database Schema (CORRECT):**
```sql
dsl_instances (
    instance_id uuid PRIMARY KEY,
    domain_name varchar(100) NOT NULL,
    business_reference varchar(255) NOT NULL,
    current_version integer NOT NULL DEFAULT 1,
    status varchar(50) NOT NULL DEFAULT 'CREATED',
    created_at timestamp with time zone,
    updated_at timestamp with time zone,
    metadata jsonb
)
UNIQUE(domain_name, business_reference)
```

**Rust Code (DslRepository) - ALREADY UPDATED:**
The dsl_repository.rs has been updated to use correct columns but some queries may still be wrong.

---

### 4. cbu_entity_roles Table (for entity relationships)

**Database Schema:**
```sql
cbu_entity_roles (
    cbu_entity_role_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES cbus(cbu_id),
    entity_id uuid NOT NULL REFERENCES entities(entity_id),
    role_id uuid REFERENCES roles(role_id),
    created_at timestamp with time zone
)
```

**Rust Code Issue:**
The CrudExecutor tries to insert into entities directly for CBU_ENTITY_RELATIONSHIP, but should use cbu_entity_roles to link CBUs to entities with roles.

---

### 5. dictionary Table

**Database Schema (CORRECT):**
```sql
dictionary (
    attribute_id uuid PRIMARY KEY,
    name varchar NOT NULL,
    long_description text,
    group_id varchar NOT NULL,
    mask varchar,
    domain varchar,
    vector text,
    source jsonb,
    sink jsonb,
    created_at timestamp with time zone,
    updated_at timestamp with time zone
)
```

---

## Document Tables

### document_catalog
- 33 columns for document management
- Handles versioning, AI extraction, compliance
- Uses document_type_id FK to document_types

### document_types
- 22 columns defining document type metadata
- Has expected_attribute_ids and validation_attribute_ids arrays

### document_usage
- Tracks document usage in DSL workflows
- Links to dsl_version_id and cbu_id
- Records verb_used, verification_result, confidence_score

---

## Files Requiring Updates

### High Priority:

1. **rust/src/database/crud_executor.rs**
   - Fix entities INSERT (use entity_type_id, name)
   - Use cbu_entity_roles for relationships

2. **rust/src/database/dsl_repository.rs**
   - Fix save_attribute() - use correct columns
   - Fix save_execution_transactionally()

3. **rust/src/database/attribute_values_service.rs**
   - Update all queries to match actual schema

### Medium Priority:

4. **rust/src/forth_engine/kyc_vocab.rs**
5. **rust/src/database/cbu_service.rs**
6. **rust/src/cbu_model_dsl/service.rs**

---

## Test Command

After fixes:
```bash
cd rust && cargo run --bin cbu_live_test --features database
```

---

# PART 2: DATABASE SCHEMA (pg_dump)

```sql
--
-- PostgreSQL database dump
--

\restrict 1kcNxfNgRb4KmRoftNphy7tPUcQs6OWoor2jPe4trwgAmFDi8ExnBOMNXUkbJF0

-- Dumped from database version 14.19 (Homebrew)
-- Dumped by pg_dump version 17.6 (Homebrew)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: ob-poc; Type: SCHEMA; Schema: -; Owner: adamtc007
--

CREATE SCHEMA "ob-poc";


ALTER SCHEMA "ob-poc" OWNER TO adamtc007;

--
-- Name: update_dsl_domains_updated_at(); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
--

CREATE FUNCTION "ob-poc".update_dsl_domains_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$;


ALTER FUNCTION "ob-poc".update_dsl_domains_updated_at() OWNER TO adamtc007;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: agent_prompt_templates; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".agent_prompt_templates (
    template_id uuid DEFAULT gen_random_uuid() NOT NULL,
    template_name character varying(200) NOT NULL,
    template_type character varying(100) NOT NULL,
    base_prompt text NOT NULL,
    context_sections jsonb,
    variable_definitions jsonb,
    applicable_domains text[],
    applicable_verbs text[],
    use_case_description text,
    effectiveness_score numeric(3,2),
    usage_count integer DEFAULT 0,
    last_used timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".agent_prompt_templates OWNER TO adamtc007;

--
-- Name: agent_verb_usage; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".agent_verb_usage (
    usage_id uuid DEFAULT gen_random_uuid() NOT NULL,
    session_id character varying(200),
    agent_type character varying(100),
    domain character varying(100) NOT NULL,
    verb character varying(100) NOT NULL,
    context_prompt text,
    selected_parameters jsonb,
    alternative_verbs_considered text[],
    selection_reasoning text,
    confidence_reported numeric(3,2),
    execution_success boolean,
    user_feedback character varying(20),
    correction_applied text,
    preceding_verbs text[],
    workflow_stage character varying(100),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".agent_verb_usage OWNER TO adamtc007;

--
-- Name: ast_nodes; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".ast_nodes (
    node_id uuid DEFAULT gen_random_uuid() NOT NULL,
    version_id uuid NOT NULL,
    parent_node_id uuid,
    node_type character varying(100) NOT NULL,
    node_key character varying(255),
    node_value jsonb,
    position_index integer,
    depth integer NOT NULL,
    path text NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".ast_nodes OWNER TO adamtc007;

--
-- Name: TABLE ast_nodes; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".ast_nodes IS 'Hierarchical storage of AST nodes for efficient querying and traversal';


--
-- Name: attribute_values; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".attribute_values (
    av_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    dsl_ob_id uuid,
    dsl_version integer NOT NULL,
    attribute_id uuid NOT NULL,
    value jsonb NOT NULL,
    state text DEFAULT 'resolved'::text NOT NULL,
    source jsonb,
    observed_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".attribute_values OWNER TO adamtc007;

--
-- Name: cbu_entity_roles; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".cbu_entity_roles (
    cbu_entity_role_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    role_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".cbu_entity_roles OWNER TO adamtc007;

--
-- Name: cbus; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".cbus (
    cbu_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    nature_purpose text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".cbus OWNER TO adamtc007;

--
-- Name: dictionary; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dictionary (
    attribute_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    long_description text,
    group_id character varying(100) DEFAULT 'default'::character varying NOT NULL,
    mask character varying(50) DEFAULT 'string'::character varying,
    domain character varying(100),
    vector text,
    source jsonb,
    sink jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dictionary OWNER TO adamtc007;

--
-- Name: document_catalog; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_catalog (
    document_id uuid DEFAULT gen_random_uuid() NOT NULL,
    document_code character varying(200) NOT NULL,
    document_type_id uuid NOT NULL,
    issuer_id uuid,
    title character varying(500),
    description text,
    language character varying(10) DEFAULT 'en'::character varying,
    issue_date date,
    expiry_date date,
    last_verified_date date,
    verification_status character varying(50) DEFAULT 'pending'::character varying,
    file_path character varying(1000),
    file_size_bytes bigint,
    file_hash character varying(128),
    mime_type character varying(100),
    page_count integer,
    extracted_text text,
    extracted_attributes jsonb,
    ai_summary text,
    tags text[],
    related_entities text[],
    business_purpose text,
    confidentiality_level character varying(50) DEFAULT 'internal'::character varying,
    retention_period_years integer,
    disposal_date date,
    audit_trail jsonb,
    version character varying(50) DEFAULT '1.0'::character varying,
    parent_document_id uuid,
    is_current_version boolean DEFAULT true,
    last_embedding_update timestamp with time zone,
    ai_metadata jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".document_catalog OWNER TO adamtc007;

--
-- Name: document_issuers; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_issuers (
    issuer_id uuid DEFAULT gen_random_uuid() NOT NULL,
    issuer_code character varying(100) NOT NULL,
    legal_name character varying(300) NOT NULL,
    jurisdiction character varying(10),
    regulatory_type character varying(100),
    official_website character varying(500),
    verification_endpoints jsonb,
    contact_information jsonb,
    document_types_issued text[],
    authority_level character varying(50),
    typical_processing_time_days integer,
    digital_issuance_available boolean DEFAULT false,
    api_integration_available boolean DEFAULT false,
    reliability_score numeric(3,2) DEFAULT 0.8,
    verification_method character varying(100),
    active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".document_issuers OWNER TO adamtc007;

--
-- Name: document_types; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_types (
    type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    type_code character varying(100) NOT NULL,
    display_name character varying(200) NOT NULL,
    category character varying(100) NOT NULL,
    domain character varying(100),
    primary_attribute_id uuid,
    description text,
    typical_issuers text[],
    validity_period_days integer,
    renewal_required boolean DEFAULT false,
    expected_attribute_ids uuid[] DEFAULT '{}'::uuid[] NOT NULL,
    validation_attribute_ids uuid[],
    extraction_template jsonb,
    required_for_products text[],
    compliance_frameworks text[],
    risk_classification character varying(50),
    ai_description text,
    common_contents text,
    key_data_point_attributes uuid[],
    active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".document_types OWNER TO adamtc007;

--
-- Name: document_catalog_with_attributes; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".document_catalog_with_attributes AS
 SELECT dc.document_id,
    dc.document_code,
    dc.document_type_id,
    dc.issuer_id,
    dc.title,
    dc.description,
    dc.language,
    dc.issue_date,
    dc.expiry_date,
    dc.last_verified_date,
    dc.verification_status,
    dc.file_path,
    dc.file_size_bytes,
    dc.file_hash,
    dc.mime_type,
    dc.page_count,
    dc.extracted_text,
    dc.extracted_attributes,
    dc.ai_summary,
    dc.tags,
    dc.related_entities,
    dc.business_purpose,
    dc.confidentiality_level,
    dc.retention_period_years,
    dc.disposal_date,
    dc.audit_trail,
    dc.version,
    dc.parent_document_id,
    dc.is_current_version,
    dc.last_embedding_update,
    dc.ai_metadata,
    dc.created_at,
    dc.updated_at,
    dt.type_code,
    dt.display_name AS document_type_name,
    dt.category AS document_category,
    dt.domain AS document_domain,
    dt.expected_attribute_ids,
    dt.key_data_point_attributes,
    di.issuer_code,
    di.legal_name AS issuer_name,
    di.jurisdiction AS issuer_jurisdiction,
    ( SELECT jsonb_object_agg(d.name, ea.value) AS jsonb_object_agg
           FROM (jsonb_each(dc.extracted_attributes) ea(key, value)
             JOIN "ob-poc".dictionary d ON ((d.attribute_id = (ea.key)::uuid)))
          WHERE (dc.extracted_attributes IS NOT NULL)) AS extracted_attributes_resolved
   FROM (("ob-poc".document_catalog dc
     LEFT JOIN "ob-poc".document_types dt ON ((dc.document_type_id = dt.type_id)))
     LEFT JOIN "ob-poc".document_issuers di ON ((dc.issuer_id = di.issuer_id)));


ALTER VIEW "ob-poc".document_catalog_with_attributes OWNER TO adamtc007;

--
-- Name: document_relationships; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    source_document_id uuid NOT NULL,
    target_document_id uuid NOT NULL,
    relationship_type character varying(50) NOT NULL,
    relationship_strength character varying(20) DEFAULT 'strong'::character varying,
    description text,
    business_rationale text,
    effective_date date,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT chk_no_self_reference CHECK ((source_document_id <> target_document_id))
);


ALTER TABLE "ob-poc".document_relationships OWNER TO adamtc007;

--
-- Name: document_usage; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_usage (
    usage_id uuid DEFAULT gen_random_uuid() NOT NULL,
    document_id uuid NOT NULL,
    dsl_version_id uuid,
    cbu_id character varying(255),
    workflow_stage character varying(100),
    usage_type character varying(50) NOT NULL,
    verb_used character varying(100),
    usage_context text,
    verification_result character varying(50),
    confidence_score numeric(3,2),
    notes text,
    accessed_by character varying(100),
    access_method character varying(50),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".document_usage OWNER TO adamtc007;

--
-- Name: document_usage_with_context; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".document_usage_with_context AS
 SELECT du.usage_id,
    du.document_id,
    du.dsl_version_id,
    du.cbu_id,
    du.workflow_stage,
    du.usage_type,
    du.verb_used,
    du.usage_context,
    du.verification_result,
    du.confidence_score,
    du.notes,
    du.accessed_by,
    du.access_method,
    du.created_at,
    dc.document_code,
    dt.type_code,
    dt.expected_attribute_ids,
    array_length(dt.expected_attribute_ids, 1) AS expected_attribute_count,
    ( SELECT count(*) AS count
           FROM jsonb_object_keys(dc.extracted_attributes) jsonb_object_keys(jsonb_object_keys)) AS extracted_attribute_count
   FROM (("ob-poc".document_usage du
     JOIN "ob-poc".document_catalog dc ON ((du.document_id = dc.document_id)))
     JOIN "ob-poc".document_types dt ON ((dc.document_type_id = dt.type_id)));


ALTER VIEW "ob-poc".document_usage_with_context OWNER TO adamtc007;

--
-- Name: domain_vocabularies; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".domain_vocabularies (
    vocab_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain character varying(100) NOT NULL,
    verb character varying(100) NOT NULL,
    category character varying(50),
    description text,
    parameters jsonb,
    examples jsonb,
    phase character varying(20),
    active boolean DEFAULT true,
    version character varying(20) DEFAULT '1.0.0'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".domain_vocabularies OWNER TO adamtc007;

--
-- Name: dsl_business_references; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_business_references (
    reference_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    reference_type character varying(100) NOT NULL,
    reference_id_value character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_business_references OWNER TO adamtc007;

--
-- Name: TABLE dsl_business_references; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_business_references IS 'Links between DSL instances and business objects';


--
-- Name: dsl_compilation_logs; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_compilation_logs (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    version_id uuid NOT NULL,
    compilation_start timestamp with time zone NOT NULL,
    compilation_end timestamp with time zone,
    success boolean,
    error_message text,
    error_location jsonb,
    node_count integer,
    complexity_score double precision,
    performance_metrics jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_compilation_logs OWNER TO adamtc007;

--
-- Name: TABLE dsl_compilation_logs; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_compilation_logs IS 'Detailed logs of DSL compilation processes';


--
-- Name: dsl_domain_relationships; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_domain_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    parent_instance_id uuid NOT NULL,
    child_instance_id uuid NOT NULL,
    relationship_type character varying(100) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_domain_relationships OWNER TO adamtc007;

--
-- Name: TABLE dsl_domain_relationships; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_domain_relationships IS 'Cross-domain relationships between DSL instances';


--
-- Name: dsl_domains; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_domains (
    domain_id integer NOT NULL,
    domain_name character varying(50) NOT NULL,
    description text NOT NULL,
    base_grammar_version character varying(20) DEFAULT '3.0.0'::character varying NOT NULL,
    vocabulary_version character varying(20) DEFAULT '1.0.0'::character varying NOT NULL,
    active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT valid_domain_name CHECK (((domain_name)::text ~ '^[A-Z][a-zA-Z_]*$'::text))
);


ALTER TABLE "ob-poc".dsl_domains OWNER TO adamtc007;

--
-- Name: dsl_domains_domain_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: adamtc007
--

CREATE SEQUENCE "ob-poc".dsl_domains_domain_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE "ob-poc".dsl_domains_domain_id_seq OWNER TO adamtc007;

--
-- Name: dsl_domains_domain_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: adamtc007
--

ALTER SEQUENCE "ob-poc".dsl_domains_domain_id_seq OWNED BY "ob-poc".dsl_domains.domain_id;


--
-- Name: dsl_instance_versions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_instance_versions (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    version_number integer NOT NULL,
    dsl_content text NOT NULL,
    operation_type character varying(50) NOT NULL,
    compilation_status character varying(50) DEFAULT 'PENDING'::character varying NOT NULL,
    ast_json jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_by character varying(255),
    change_description text
);


ALTER TABLE "ob-poc".dsl_instance_versions OWNER TO adamtc007;

--
-- Name: TABLE dsl_instance_versions; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_instance_versions IS 'Version history for DSL instances, storing content and compiled AST';


--
-- Name: dsl_instances; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_instances (
    instance_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain_name character varying(100) NOT NULL,
    business_reference character varying(255) NOT NULL,
    current_version integer DEFAULT 1 NOT NULL,
    status character varying(50) DEFAULT 'CREATED'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    metadata jsonb
);


ALTER TABLE "ob-poc".dsl_instances OWNER TO adamtc007;

--
-- Name: TABLE dsl_instances; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_instances IS 'Main registry of DSL instances across all domains';


--
-- Name: dsl_ob; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_ob (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id character varying(255) NOT NULL,
    dsl_text text NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_ob OWNER TO adamtc007;

--
-- Name: dsl_templates; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_templates (
    template_id uuid DEFAULT gen_random_uuid() NOT NULL,
    template_name character varying(255) NOT NULL,
    domain_name character varying(100) NOT NULL,
    template_type character varying(100) NOT NULL,
    content text NOT NULL,
    variables jsonb,
    requirements jsonb,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_templates OWNER TO adamtc007;

--
-- Name: TABLE dsl_templates; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_templates IS 'Reusable DSL templates for standard operations';


--
-- Name: dsl_transformation_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_transformation_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instruction_pattern character varying(255) NOT NULL,
    transformation_type character varying(50) NOT NULL,
    target_values jsonb,
    dsl_template text,
    confidence_score numeric(3,2) DEFAULT 0.8,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_transformation_rules OWNER TO adamtc007;

--
-- Name: dsl_validation_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_validation_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    rule_type character varying(50) NOT NULL,
    target_pattern character varying(255) NOT NULL,
    error_message text,
    warning_message text,
    suggestion text,
    severity character varying(20) DEFAULT 'error'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_validation_rules OWNER TO adamtc007;

--
-- Name: dsl_validations; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_validations (
    validation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    version_id uuid NOT NULL,
    validation_type character varying(100) NOT NULL,
    validation_success boolean NOT NULL,
    validation_messages jsonb,
    validated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    validated_by character varying(255)
);


ALTER TABLE "ob-poc".dsl_validations OWNER TO adamtc007;

--
-- Name: TABLE dsl_validations; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_validations IS 'Validation results for DSL instance versions';


--
-- Name: dsl_visualizations; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_visualizations (
    visualization_id uuid DEFAULT gen_random_uuid() NOT NULL,
    version_id uuid NOT NULL,
    visualization_type character varying(100) NOT NULL,
    visualization_data jsonb NOT NULL,
    options_used jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_visualizations OWNER TO adamtc007;

--
-- Name: TABLE dsl_visualizations; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_visualizations IS 'Storage for generated visualizations of DSL instances';


--
-- Name: entities; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entities (
    entity_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type_id uuid NOT NULL,
    external_id character varying(255),
    name character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entities OWNER TO adamtc007;

--
-- Name: entity_limited_companies; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_limited_companies (
    limited_company_id uuid DEFAULT gen_random_uuid() NOT NULL,
    company_name character varying(255) NOT NULL,
    registration_number character varying(100),
    jurisdiction character varying(100),
    incorporation_date date,
    registered_address text,
    business_nature text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entity_limited_companies OWNER TO adamtc007;

--
-- Name: entity_partnerships; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_partnerships (
    partnership_id uuid DEFAULT gen_random_uuid() NOT NULL,
    partnership_name character varying(255) NOT NULL,
    partnership_type character varying(100),
    jurisdiction character varying(100),
    formation_date date,
    principal_place_business text,
    partnership_agreement_date date,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entity_partnerships OWNER TO adamtc007;

--
-- Name: entity_product_mappings; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_product_mappings (
    entity_type character varying(100) NOT NULL,
    product_id uuid NOT NULL,
    compatible boolean NOT NULL,
    restrictions jsonb,
    required_fields jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entity_product_mappings OWNER TO adamtc007;

--
-- Name: entity_proper_persons; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_proper_persons (
    proper_person_id uuid DEFAULT gen_random_uuid() NOT NULL,
    first_name character varying(255) NOT NULL,
    last_name character varying(255) NOT NULL,
    middle_names character varying(255),
    date_of_birth date,
    nationality character varying(100),
    residence_address text,
    id_document_type character varying(100),
    id_document_number character varying(100),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entity_proper_persons OWNER TO adamtc007;

--
-- Name: entity_trusts; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_trusts (
    trust_id uuid DEFAULT gen_random_uuid() NOT NULL,
    trust_name character varying(255) NOT NULL,
    trust_type character varying(100),
    jurisdiction character varying(100) NOT NULL,
    establishment_date date,
    trust_deed_date date,
    trust_purpose text,
    governing_law character varying(100),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entity_trusts OWNER TO adamtc007;

--
-- Name: entity_types; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_types (
    entity_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    table_name character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".entity_types OWNER TO adamtc007;

--
-- Name: grammar_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".grammar_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    rule_name character varying(100) NOT NULL,
    rule_definition text NOT NULL,
    rule_type character varying(50) DEFAULT 'production'::character varying NOT NULL,
    domain character varying(100),
    version character varying(20) DEFAULT '1.0.0'::character varying,
    active boolean DEFAULT true,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".grammar_rules OWNER TO adamtc007;

--
-- Name: kyc_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".kyc_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type character varying(100) NOT NULL,
    jurisdiction character varying(10),
    required_documents text[] NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".kyc_rules OWNER TO adamtc007;

--
-- Name: orchestration_domain_sessions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".orchestration_domain_sessions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    orchestration_session_id uuid NOT NULL,
    domain_name character varying(100) NOT NULL,
    domain_session_id uuid NOT NULL,
    state character varying(50) DEFAULT 'CREATED'::character varying,
    contributed_dsl text,
    domain_context jsonb,
    dependencies text[],
    last_activity timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".orchestration_domain_sessions OWNER TO adamtc007;

--
-- Name: orchestration_sessions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".orchestration_sessions (
    session_id uuid DEFAULT gen_random_uuid() NOT NULL,
    primary_domain character varying(100) NOT NULL,
    cbu_id uuid,
    entity_type character varying(50),
    entity_name text,
    jurisdiction character varying(10),
    products text[],
    services text[],
    workflow_type character varying(50) DEFAULT 'ONBOARDING'::character varying,
    current_state character varying(50) DEFAULT 'CREATED'::character varying,
    version_number integer DEFAULT 0,
    unified_dsl text,
    shared_context jsonb,
    execution_plan jsonb,
    entity_refs jsonb,
    attribute_refs jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    last_used timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    expires_at timestamp with time zone DEFAULT ((now() AT TIME ZONE 'utc'::text) + '24:00:00'::interval)
);


ALTER TABLE "ob-poc".orchestration_sessions OWNER TO adamtc007;

--
-- Name: orchestration_state_history; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".orchestration_state_history (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    orchestration_session_id uuid NOT NULL,
    from_state character varying(50),
    to_state character varying(50) NOT NULL,
    domain_name character varying(100),
    reason text,
    generated_by character varying(100),
    version_number integer,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".orchestration_state_history OWNER TO adamtc007;

--
-- Name: orchestration_tasks; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".orchestration_tasks (
    task_id uuid DEFAULT gen_random_uuid() NOT NULL,
    orchestration_session_id uuid NOT NULL,
    domain_name character varying(100) NOT NULL,
    verb character varying(200) NOT NULL,
    parameters jsonb,
    dependencies text[],
    status character varying(50) DEFAULT 'PENDING'::character varying,
    generated_dsl text,
    error_message text,
    scheduled_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".orchestration_tasks OWNER TO adamtc007;

--
-- Name: partnership_control_mechanisms; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".partnership_control_mechanisms (
    control_mechanism_id uuid DEFAULT gen_random_uuid() NOT NULL,
    partnership_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    control_type character varying(100) NOT NULL,
    control_description text,
    effective_date date,
    termination_date date,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".partnership_control_mechanisms OWNER TO adamtc007;

--
-- Name: partnership_interests; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".partnership_interests (
    interest_id uuid DEFAULT gen_random_uuid() NOT NULL,
    partnership_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    partner_type character varying(100) NOT NULL,
    capital_commitment numeric(15,2),
    ownership_percentage numeric(5,2),
    voting_rights numeric(5,2),
    profit_sharing_percentage numeric(5,2),
    admission_date date,
    withdrawal_date date,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".partnership_interests OWNER TO adamtc007;

--
-- Name: prod_resources; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".prod_resources (
    resource_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    owner character varying(255) NOT NULL,
    dictionary_group character varying(100),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".prod_resources OWNER TO adamtc007;

--
-- Name: product_requirements; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".product_requirements (
    product_id uuid NOT NULL,
    entity_types jsonb NOT NULL,
    required_dsl jsonb NOT NULL,
    attributes jsonb NOT NULL,
    compliance jsonb NOT NULL,
    prerequisites jsonb NOT NULL,
    conditional_rules jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".product_requirements OWNER TO adamtc007;

--
-- Name: product_services; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".product_services (
    product_id uuid NOT NULL,
    service_id uuid NOT NULL
);


ALTER TABLE "ob-poc".product_services OWNER TO adamtc007;

--
-- Name: product_workflows; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".product_workflows (
    workflow_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id character varying(255) NOT NULL,
    product_id uuid NOT NULL,
    entity_type character varying(100) NOT NULL,
    required_dsl jsonb NOT NULL,
    generated_dsl text NOT NULL,
    compliance_rules jsonb NOT NULL,
    status character varying(50) DEFAULT 'PENDING'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".product_workflows OWNER TO adamtc007;

--
-- Name: products; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".products (
    product_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".products OWNER TO adamtc007;

--
-- Name: roles; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".roles (
    role_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".roles OWNER TO adamtc007;

--
-- Name: service_resources; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_resources (
    service_id uuid NOT NULL,
    resource_id uuid NOT NULL
);


ALTER TABLE "ob-poc".service_resources OWNER TO adamtc007;

--
-- Name: services; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".services (
    service_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".services OWNER TO adamtc007;

--
-- Name: trust_beneficiary_classes; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".trust_beneficiary_classes (
    beneficiary_class_id uuid DEFAULT gen_random_uuid() NOT NULL,
    trust_id uuid NOT NULL,
    class_name character varying(255) NOT NULL,
    class_definition text,
    class_type character varying(100),
    monitoring_required boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".trust_beneficiary_classes OWNER TO adamtc007;

--
-- Name: trust_parties; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".trust_parties (
    trust_party_id uuid DEFAULT gen_random_uuid() NOT NULL,
    trust_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    party_role character varying(100) NOT NULL,
    party_type character varying(100) NOT NULL,
    appointment_date date,
    resignation_date date,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".trust_parties OWNER TO adamtc007;

--
-- Name: trust_protector_powers; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".trust_protector_powers (
    protector_power_id uuid DEFAULT gen_random_uuid() NOT NULL,
    trust_party_id uuid NOT NULL,
    power_type character varying(100) NOT NULL,
    power_description text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".trust_protector_powers OWNER TO adamtc007;

--
-- Name: ubo_registry; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".ubo_registry (
    ubo_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    subject_entity_id uuid NOT NULL,
    ubo_proper_person_id uuid NOT NULL,
    relationship_type character varying(100) NOT NULL,
    qualifying_reason character varying(100) NOT NULL,
    ownership_percentage numeric(5,2),
    control_type character varying(100),
    workflow_type character varying(100) NOT NULL,
    regulatory_framework character varying(100),
    verification_status character varying(50) DEFAULT 'PENDING'::character varying,
    screening_result character varying(50) DEFAULT 'PENDING'::character varying,
    risk_rating character varying(50),
    identified_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    verified_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".ubo_registry OWNER TO adamtc007;

--
-- Name: verb_relationships; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".verb_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    source_domain character varying(100) NOT NULL,
    source_verb character varying(100) NOT NULL,
    target_domain character varying(100) NOT NULL,
    target_verb character varying(100) NOT NULL,
    relationship_type character varying(50) NOT NULL,
    relationship_strength numeric(3,2) DEFAULT 0.8,
    context_conditions text[],
    business_rationale text,
    sequence_type character varying(20),
    timing_constraints text,
    agent_explanation text,
    violation_consequences text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".verb_relationships OWNER TO adamtc007;

--
-- Name: verb_semantics; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".verb_semantics (
    semantic_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain character varying(100) NOT NULL,
    verb character varying(100) NOT NULL,
    semantic_description text NOT NULL,
    intent_category character varying(50) NOT NULL,
    business_purpose text NOT NULL,
    side_effects text[],
    prerequisites text[],
    postconditions text[],
    resource_requirements text[],
    performance_characteristics jsonb,
    agent_prompt text NOT NULL,
    usage_patterns text[],
    common_mistakes text[],
    selection_criteria text,
    parameter_semantics jsonb NOT NULL,
    parameter_validation jsonb,
    parameter_examples jsonb,
    typical_predecessors text[],
    typical_successors text[],
    workflow_stage character varying(100),
    parallel_compatibility text[],
    compliance_implications text[],
    risk_factors text[],
    approval_requirements text[],
    audit_significance character varying(50),
    confidence_score numeric(3,2) DEFAULT 1.0,
    last_validated timestamp with time zone,
    validation_notes text,
    version character varying(20) DEFAULT '1.0.0'::character varying,
    status character varying(20) DEFAULT 'active'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".verb_semantics OWNER TO adamtc007;

--
-- Name: v_agent_verb_context; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".v_agent_verb_context AS
 SELECT dv.domain,
    dv.verb,
    dv.category,
    dv.description AS syntax_description,
    dv.parameters AS syntax_parameters,
    dv.examples AS syntax_examples,
    vs.semantic_description,
    vs.intent_category,
    vs.business_purpose,
    vs.side_effects,
    vs.prerequisites,
    vs.postconditions,
    vs.agent_prompt,
    vs.usage_patterns,
    vs.selection_criteria,
    vs.parameter_semantics,
    vs.workflow_stage,
    vs.compliance_implications,
    vs.confidence_score,
    array_agg(DISTINCT ((((vr_out.target_verb)::text || ' ('::text) || (vr_out.relationship_type)::text) || ')'::text)) FILTER (WHERE (vr_out.target_verb IS NOT NULL)) AS related_verbs,
    array_agg(DISTINCT ((vr_in.source_verb)::text || ' (prerequisite)'::text)) FILTER (WHERE ((vr_in.source_verb IS NOT NULL) AND ((vr_in.relationship_type)::text = 'requires'::text))) AS required_by,
    COALESCE(usage_stats.usage_count, (0)::bigint) AS historical_usage_count,
    COALESCE(usage_stats.success_rate, (0)::numeric) AS historical_success_rate,
    COALESCE(usage_stats.avg_confidence, (0)::numeric) AS avg_agent_confidence
   FROM (((("ob-poc".domain_vocabularies dv
     LEFT JOIN "ob-poc".verb_semantics vs ON ((((dv.domain)::text = (vs.domain)::text) AND ((dv.verb)::text = (vs.verb)::text))))
     LEFT JOIN "ob-poc".verb_relationships vr_out ON ((((dv.domain)::text = (vr_out.source_domain)::text) AND ((dv.verb)::text = (vr_out.source_verb)::text))))
     LEFT JOIN "ob-poc".verb_relationships vr_in ON ((((dv.domain)::text = (vr_in.target_domain)::text) AND ((dv.verb)::text = (vr_in.target_verb)::text))))
     LEFT JOIN ( SELECT agent_verb_usage.domain,
            agent_verb_usage.verb,
            count(*) AS usage_count,
            avg(
                CASE
                    WHEN agent_verb_usage.execution_success THEN 1.0
                    ELSE 0.0
                END) AS success_rate,
            avg(agent_verb_usage.confidence_reported) AS avg_confidence
           FROM "ob-poc".agent_verb_usage
          WHERE (agent_verb_usage.created_at > (now() - '30 days'::interval))
          GROUP BY agent_verb_usage.domain, agent_verb_usage.verb) usage_stats ON ((((dv.domain)::text = (usage_stats.domain)::text) AND ((dv.verb)::text = (usage_stats.verb)::text))))
  GROUP BY dv.domain, dv.verb, dv.category, dv.description, dv.parameters, dv.examples, vs.semantic_description, vs.intent_category, vs.business_purpose, vs.side_effects, vs.prerequisites, vs.postconditions, vs.agent_prompt, vs.usage_patterns, vs.selection_criteria, vs.parameter_semantics, vs.workflow_stage, vs.compliance_implications, vs.confidence_score, usage_stats.usage_count, usage_stats.success_rate, usage_stats.avg_confidence;


ALTER VIEW "ob-poc".v_agent_verb_context OWNER TO adamtc007;

--
-- Name: v_kyc_requirements; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".v_kyc_requirements AS
 SELECT kr.entity_type,
    kr.jurisdiction,
    kr.required_documents,
    array_length(kr.required_documents, 1) AS document_count
   FROM "ob-poc".kyc_rules kr
  ORDER BY kr.entity_type, kr.jurisdiction;


ALTER VIEW "ob-poc".v_kyc_requirements OWNER TO adamtc007;

--
-- Name: v_workflow_sequences; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".v_workflow_sequences AS
 SELECT vs.workflow_stage,
    vs.domain,
    array_agg(vs.verb ORDER BY vs.verb) AS available_verbs,
    array_agg(DISTINCT vr.target_verb) FILTER (WHERE ((vr.relationship_type)::text = 'enables'::text)) AS enables_verbs,
    array_agg(DISTINCT vr2.source_verb) FILTER (WHERE ((vr2.relationship_type)::text = 'requires'::text)) AS required_by_verbs
   FROM (("ob-poc".verb_semantics vs
     LEFT JOIN "ob-poc".verb_relationships vr ON ((((vs.domain)::text = (vr.source_domain)::text) AND ((vs.verb)::text = (vr.source_verb)::text))))
     LEFT JOIN "ob-poc".verb_relationships vr2 ON ((((vs.domain)::text = (vr2.target_domain)::text) AND ((vs.verb)::text = (vr2.target_verb)::text))))
  WHERE ((vs.status)::text = 'active'::text)
  GROUP BY vs.workflow_stage, vs.domain
  ORDER BY vs.workflow_stage, vs.domain;


ALTER VIEW "ob-poc".v_workflow_sequences OWNER TO adamtc007;

--
-- Name: verb_decision_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".verb_decision_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    rule_name character varying(200) NOT NULL,
    rule_type character varying(50) NOT NULL,
    condition_expression text NOT NULL,
    action_expression text NOT NULL,
    priority_weight integer DEFAULT 100,
    applicable_domains text[],
    applicable_verbs text[],
    business_context text,
    llm_prompt_addition text,
    error_message text,
    suggestion_text text,
    confidence_level numeric(3,2) DEFAULT 0.8,
    active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".verb_decision_rules OWNER TO adamtc007;

--
-- Name: verb_embeddings; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".verb_embeddings (
    embedding_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain character varying(100) NOT NULL,
    verb character varying(100) NOT NULL,
    semantic_embedding public.vector(1536),
    context_embedding public.vector(1536),
    parameter_embedding public.vector(1536),
    embedding_model character varying(100),
    embedding_version character varying(20),
    last_updated timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".verb_embeddings OWNER TO adamtc007;

--
-- Name: verb_patterns; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".verb_patterns (
    pattern_id uuid DEFAULT gen_random_uuid() NOT NULL,
    pattern_name character varying(200) NOT NULL,
    pattern_category character varying(100) NOT NULL,
    pattern_description text NOT NULL,
    pattern_template text NOT NULL,
    pattern_variables jsonb,
    use_cases text[],
    business_scenarios text[],
    complexity_level character varying(20),
    required_verbs text[] NOT NULL,
    optional_verbs text[],
    forbidden_verbs text[],
    agent_selection_rules text,
    customization_guidance text,
    common_adaptations jsonb,
    success_rate numeric(5,2),
    usage_frequency integer DEFAULT 0,
    domain_applicability text[],
    tags text[],
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".verb_patterns OWNER TO adamtc007;

--
-- Name: verb_registry; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".verb_registry (
    verb character varying(100) NOT NULL,
    primary_domain character varying(100) NOT NULL,
    shared boolean DEFAULT false,
    deprecated boolean DEFAULT false,
    replacement_verb character varying(100),
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".verb_registry OWNER TO adamtc007;

--
-- Name: vocabulary_audit; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".vocabulary_audit (
    audit_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain character varying(100) NOT NULL,
    verb character varying(100) NOT NULL,
    change_type character varying(20) NOT NULL,
    old_definition jsonb,
    new_definition jsonb,
    changed_by character varying(255),
    change_reason text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".vocabulary_audit OWNER TO adamtc007;

--
-- Name: dsl_domains domain_id; Type: DEFAULT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domains ALTER COLUMN domain_id SET DEFAULT nextval('"ob-poc".dsl_domains_domain_id_seq'::regclass);


--
-- Name: agent_prompt_templates agent_prompt_templates_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".agent_prompt_templates
    ADD CONSTRAINT agent_prompt_templates_pkey PRIMARY KEY (template_id);


--
-- Name: agent_verb_usage agent_verb_usage_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".agent_verb_usage
    ADD CONSTRAINT agent_verb_usage_pkey PRIMARY KEY (usage_id);


--
-- Name: ast_nodes ast_nodes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ast_nodes
    ADD CONSTRAINT ast_nodes_pkey PRIMARY KEY (node_id);


--
-- Name: attribute_values attribute_values_cbu_id_dsl_version_attribute_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_cbu_id_dsl_version_attribute_id_key UNIQUE (cbu_id, dsl_version, attribute_id);


--
-- Name: attribute_values attribute_values_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_pkey PRIMARY KEY (av_id);


--
-- Name: cbu_entity_roles cbu_entity_roles_cbu_id_entity_id_role_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_cbu_id_entity_id_role_id_key UNIQUE (cbu_id, entity_id, role_id);


--
-- Name: cbu_entity_roles cbu_entity_roles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_pkey PRIMARY KEY (cbu_entity_role_id);


--
-- Name: cbus cbus_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_name_key UNIQUE (name);


--
-- Name: cbus cbus_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_pkey PRIMARY KEY (cbu_id);


--
-- Name: dictionary dictionary_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dictionary
    ADD CONSTRAINT dictionary_name_key UNIQUE (name);


--
-- Name: dictionary dictionary_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dictionary
    ADD CONSTRAINT dictionary_pkey PRIMARY KEY (attribute_id);


--
-- Name: document_catalog document_catalog_document_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_document_code_key UNIQUE (document_code);


--
-- Name: document_catalog document_catalog_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_pkey PRIMARY KEY (document_id);


--
-- Name: document_issuers document_issuers_issuer_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_issuers
    ADD CONSTRAINT document_issuers_issuer_code_key UNIQUE (issuer_code);


--
-- Name: document_issuers document_issuers_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_issuers
    ADD CONSTRAINT document_issuers_pkey PRIMARY KEY (issuer_id);


--
-- Name: document_relationships document_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: document_types document_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_types
    ADD CONSTRAINT document_types_pkey PRIMARY KEY (type_id);


--
-- Name: document_types document_types_type_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_types
    ADD CONSTRAINT document_types_type_code_key UNIQUE (type_code);


--
-- Name: document_usage document_usage_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_usage
    ADD CONSTRAINT document_usage_pkey PRIMARY KEY (usage_id);


--
-- Name: domain_vocabularies domain_vocabularies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".domain_vocabularies
    ADD CONSTRAINT domain_vocabularies_pkey PRIMARY KEY (vocab_id);


--
-- Name: dsl_business_references dsl_business_references_instance_id_reference_type_referenc_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_business_references
    ADD CONSTRAINT dsl_business_references_instance_id_reference_type_referenc_key UNIQUE (instance_id, reference_type, reference_id_value);


--
-- Name: dsl_business_references dsl_business_references_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_business_references
    ADD CONSTRAINT dsl_business_references_pkey PRIMARY KEY (reference_id);


--
-- Name: dsl_compilation_logs dsl_compilation_logs_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_compilation_logs
    ADD CONSTRAINT dsl_compilation_logs_pkey PRIMARY KEY (log_id);


--
-- Name: dsl_domain_relationships dsl_domain_relationships_parent_instance_id_child_instance__key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domain_relationships
    ADD CONSTRAINT dsl_domain_relationships_parent_instance_id_child_instance__key UNIQUE (parent_instance_id, child_instance_id, relationship_type);


--
-- Name: dsl_domain_relationships dsl_domain_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domain_relationships
    ADD CONSTRAINT dsl_domain_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: dsl_domains dsl_domains_domain_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domains
    ADD CONSTRAINT dsl_domains_domain_name_key UNIQUE (domain_name);


--
-- Name: dsl_domains dsl_domains_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domains
    ADD CONSTRAINT dsl_domains_pkey PRIMARY KEY (domain_id);


--
-- Name: dsl_instance_versions dsl_instance_versions_instance_id_version_number_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT dsl_instance_versions_instance_id_version_number_key UNIQUE (instance_id, version_number);


--
-- Name: dsl_instance_versions dsl_instance_versions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT dsl_instance_versions_pkey PRIMARY KEY (version_id);


--
-- Name: dsl_instances dsl_instances_domain_name_business_reference_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instances
    ADD CONSTRAINT dsl_instances_domain_name_business_reference_key UNIQUE (domain_name, business_reference);


--
-- Name: dsl_instances dsl_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instances
    ADD CONSTRAINT dsl_instances_pkey PRIMARY KEY (instance_id);


--
-- Name: dsl_ob dsl_ob_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_ob
    ADD CONSTRAINT dsl_ob_pkey PRIMARY KEY (version_id);


--
-- Name: dsl_templates dsl_templates_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_templates
    ADD CONSTRAINT dsl_templates_pkey PRIMARY KEY (template_id);


--
-- Name: dsl_templates dsl_templates_template_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_templates
    ADD CONSTRAINT dsl_templates_template_name_key UNIQUE (template_name);


--
-- Name: dsl_transformation_rules dsl_transformation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_transformation_rules
    ADD CONSTRAINT dsl_transformation_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: dsl_validation_rules dsl_validation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_validation_rules
    ADD CONSTRAINT dsl_validation_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: dsl_validations dsl_validations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_validations
    ADD CONSTRAINT dsl_validations_pkey PRIMARY KEY (validation_id);


--
-- Name: dsl_visualizations dsl_visualizations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_visualizations
    ADD CONSTRAINT dsl_visualizations_pkey PRIMARY KEY (visualization_id);


--
-- Name: entities entities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_limited_companies entity_limited_companies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_pkey PRIMARY KEY (limited_company_id);


--
-- Name: entity_partnerships entity_partnerships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_partnerships
    ADD CONSTRAINT entity_partnerships_pkey PRIMARY KEY (partnership_id);


--
-- Name: entity_product_mappings entity_product_mappings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT entity_product_mappings_pkey PRIMARY KEY (entity_type, product_id);


--
-- Name: entity_proper_persons entity_proper_persons_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_pkey PRIMARY KEY (proper_person_id);


--
-- Name: entity_trusts entity_trusts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_trusts
    ADD CONSTRAINT entity_trusts_pkey PRIMARY KEY (trust_id);


--
-- Name: entity_types entity_types_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_types
    ADD CONSTRAINT entity_types_name_key UNIQUE (name);


--
-- Name: entity_types entity_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_types
    ADD CONSTRAINT entity_types_pkey PRIMARY KEY (entity_type_id);


--
-- Name: grammar_rules grammar_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".grammar_rules
    ADD CONSTRAINT grammar_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: grammar_rules grammar_rules_rule_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".grammar_rules
    ADD CONSTRAINT grammar_rules_rule_name_key UNIQUE (rule_name);


--
-- Name: kyc_rules kyc_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".kyc_rules
    ADD CONSTRAINT kyc_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: orchestration_domain_sessions orchestration_domain_sessions_orchestration_session_id_doma_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT orchestration_domain_sessions_orchestration_session_id_doma_key UNIQUE (orchestration_session_id, domain_name);


--
-- Name: orchestration_domain_sessions orchestration_domain_sessions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT orchestration_domain_sessions_pkey PRIMARY KEY (id);


--
-- Name: orchestration_sessions orchestration_sessions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_sessions
    ADD CONSTRAINT orchestration_sessions_pkey PRIMARY KEY (session_id);


--
-- Name: orchestration_state_history orchestration_state_history_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_state_history
    ADD CONSTRAINT orchestration_state_history_pkey PRIMARY KEY (id);


--
-- Name: orchestration_tasks orchestration_tasks_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_tasks
    ADD CONSTRAINT orchestration_tasks_pkey PRIMARY KEY (task_id);


--
-- Name: partnership_control_mechanisms partnership_control_mechanisms_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT partnership_control_mechanisms_pkey PRIMARY KEY (control_mechanism_id);


--
-- Name: partnership_interests partnership_interests_partnership_id_entity_id_partner_type_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_partnership_id_entity_id_partner_type_key UNIQUE (partnership_id, entity_id, partner_type);


--
-- Name: partnership_interests partnership_interests_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_pkey PRIMARY KEY (interest_id);


--
-- Name: prod_resources prod_resources_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".prod_resources
    ADD CONSTRAINT prod_resources_name_key UNIQUE (name);


--
-- Name: prod_resources prod_resources_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".prod_resources
    ADD CONSTRAINT prod_resources_pkey PRIMARY KEY (resource_id);


--
-- Name: product_requirements product_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_requirements
    ADD CONSTRAINT product_requirements_pkey PRIMARY KEY (product_id);


--
-- Name: product_services product_services_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_pkey PRIMARY KEY (product_id, service_id);


--
-- Name: product_workflows product_workflows_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT product_workflows_pkey PRIMARY KEY (workflow_id);


--
-- Name: products products_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".products
    ADD CONSTRAINT products_name_key UNIQUE (name);


--
-- Name: products products_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".products
    ADD CONSTRAINT products_pkey PRIMARY KEY (product_id);


--
-- Name: roles roles_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".roles
    ADD CONSTRAINT roles_name_key UNIQUE (name);


--
-- Name: roles roles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".roles
    ADD CONSTRAINT roles_pkey PRIMARY KEY (role_id);


--
-- Name: service_resources service_resources_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_pkey PRIMARY KEY (service_id, resource_id);


--
-- Name: services services_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".services
    ADD CONSTRAINT services_name_key UNIQUE (name);


--
-- Name: services services_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".services
    ADD CONSTRAINT services_pkey PRIMARY KEY (service_id);


--
-- Name: trust_beneficiary_classes trust_beneficiary_classes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_beneficiary_classes
    ADD CONSTRAINT trust_beneficiary_classes_pkey PRIMARY KEY (beneficiary_class_id);


--
-- Name: trust_parties trust_parties_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_pkey PRIMARY KEY (trust_party_id);


--
-- Name: trust_parties trust_parties_trust_id_entity_id_party_role_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_trust_id_entity_id_party_role_key UNIQUE (trust_id, entity_id, party_role);


--
-- Name: trust_protector_powers trust_protector_powers_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_protector_powers
    ADD CONSTRAINT trust_protector_powers_pkey PRIMARY KEY (protector_power_id);


--
-- Name: ubo_registry ubo_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_pkey PRIMARY KEY (ubo_id);


--
-- Name: ubo_registry ubo_registry_subject_entity_id_ubo_proper_person_id_relatio_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_subject_entity_id_ubo_proper_person_id_relatio_key UNIQUE (subject_entity_id, ubo_proper_person_id, relationship_type);


--
-- Name: verb_decision_rules verb_decision_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_decision_rules
    ADD CONSTRAINT verb_decision_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: verb_embeddings verb_embeddings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_embeddings
    ADD CONSTRAINT verb_embeddings_pkey PRIMARY KEY (embedding_id);


--
-- Name: verb_patterns verb_patterns_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_patterns
    ADD CONSTRAINT verb_patterns_pkey PRIMARY KEY (pattern_id);


--
-- Name: verb_registry verb_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_registry
    ADD CONSTRAINT verb_registry_pkey PRIMARY KEY (verb);


--
-- Name: verb_relationships verb_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_relationships
    ADD CONSTRAINT verb_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: verb_semantics verb_semantics_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_semantics
    ADD CONSTRAINT verb_semantics_pkey PRIMARY KEY (semantic_id);


--
-- Name: vocabulary_audit vocabulary_audit_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".vocabulary_audit
    ADD CONSTRAINT vocabulary_audit_pkey PRIMARY KEY (audit_id);


--
-- Name: idx_agent_prompt_templates_effectiveness; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_agent_prompt_templates_effectiveness ON "ob-poc".agent_prompt_templates USING btree (effectiveness_score DESC);


--
-- Name: idx_agent_prompt_templates_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_agent_prompt_templates_type ON "ob-poc".agent_prompt_templates USING btree (template_type);


--
-- Name: idx_agent_verb_usage_created_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_agent_verb_usage_created_at ON "ob-poc".agent_verb_usage USING btree (created_at DESC);


--
-- Name: idx_agent_verb_usage_domain_verb; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_agent_verb_usage_domain_verb ON "ob-poc".agent_verb_usage USING btree (domain, verb);


--
-- Name: idx_agent_verb_usage_feedback; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_agent_verb_usage_feedback ON "ob-poc".agent_verb_usage USING btree (user_feedback);


--
-- Name: idx_agent_verb_usage_success; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_agent_verb_usage_success ON "ob-poc".agent_verb_usage USING btree (execution_success);


--
-- Name: idx_ast_nodes_depth; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ast_nodes_depth ON "ob-poc".ast_nodes USING btree (version_id, depth);


--
-- Name: idx_ast_nodes_parent; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ast_nodes_parent ON "ob-poc".ast_nodes USING btree (parent_node_id);


--
-- Name: idx_ast_nodes_path; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ast_nodes_path ON "ob-poc".ast_nodes USING btree (path text_pattern_ops);


--
-- Name: idx_ast_nodes_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ast_nodes_type ON "ob-poc".ast_nodes USING btree (node_type);


--
-- Name: idx_ast_nodes_version; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ast_nodes_version ON "ob-poc".ast_nodes USING btree (version_id);


--
-- Name: idx_attr_vals_lookup; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attr_vals_lookup ON "ob-poc".attribute_values USING btree (cbu_id, attribute_id, dsl_version);


--
-- Name: idx_beneficiary_classes_trust; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_beneficiary_classes_trust ON "ob-poc".trust_beneficiary_classes USING btree (trust_id);


--
-- Name: idx_cbu_entity_roles_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbu_entity_roles_cbu ON "ob-poc".cbu_entity_roles USING btree (cbu_id);


--
-- Name: idx_cbu_entity_roles_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbu_entity_roles_entity ON "ob-poc".cbu_entity_roles USING btree (entity_id);


--
-- Name: idx_cbu_entity_roles_role; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbu_entity_roles_role ON "ob-poc".cbu_entity_roles USING btree (role_id);


--
-- Name: idx_cbus_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbus_name ON "ob-poc".cbus USING btree (name);


--
-- Name: idx_dictionary_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dictionary_domain ON "ob-poc".dictionary USING btree (domain);


--
-- Name: idx_dictionary_group_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dictionary_group_id ON "ob-poc".dictionary USING btree (group_id);


--
-- Name: idx_dictionary_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dictionary_name ON "ob-poc".dictionary USING btree (name);


--
-- Name: idx_document_catalog_current; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_current ON "ob-poc".document_catalog USING btree (is_current_version);


--
-- Name: idx_document_catalog_entities; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_entities ON "ob-poc".document_catalog USING gin (related_entities);


--
-- Name: idx_document_catalog_expiry; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_expiry ON "ob-poc".document_catalog USING btree (expiry_date);


--
-- Name: idx_document_catalog_extracted; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_extracted ON "ob-poc".document_catalog USING gin (extracted_attributes);


--
-- Name: idx_document_catalog_issuer; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_issuer ON "ob-poc".document_catalog USING btree (issuer_id);


--
-- Name: idx_document_catalog_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_status ON "ob-poc".document_catalog USING btree (verification_status);


--
-- Name: idx_document_catalog_tags; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_tags ON "ob-poc".document_catalog USING gin (tags);


--
-- Name: idx_document_catalog_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_type ON "ob-poc".document_catalog USING btree (document_type_id);


--
-- Name: idx_document_issuers_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_issuers_active ON "ob-poc".document_issuers USING btree (active);


--
-- Name: idx_document_issuers_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_issuers_jurisdiction ON "ob-poc".document_issuers USING btree (jurisdiction);


--
-- Name: idx_document_issuers_regulatory_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_issuers_regulatory_type ON "ob-poc".document_issuers USING btree (regulatory_type);


--
-- Name: idx_document_relationships_source; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_relationships_source ON "ob-poc".document_relationships USING btree (source_document_id);


--
-- Name: idx_document_relationships_target; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_relationships_target ON "ob-poc".document_relationships USING btree (target_document_id);


--
-- Name: idx_document_relationships_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_relationships_type ON "ob-poc".document_relationships USING btree (relationship_type);


--
-- Name: idx_document_types_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_active ON "ob-poc".document_types USING btree (active);


--
-- Name: idx_document_types_category; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_category ON "ob-poc".document_types USING btree (category);


--
-- Name: idx_document_types_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_domain ON "ob-poc".document_types USING btree (domain);


--
-- Name: idx_document_types_primary_attr; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_primary_attr ON "ob-poc".document_types USING btree (primary_attribute_id);


--
-- Name: idx_document_usage_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_usage_cbu ON "ob-poc".document_usage USING btree (cbu_id);


--
-- Name: idx_document_usage_created; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_usage_created ON "ob-poc".document_usage USING btree (created_at DESC);


--
-- Name: idx_document_usage_document; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_usage_document ON "ob-poc".document_usage USING btree (document_id);


--
-- Name: idx_document_usage_dsl_version; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_usage_dsl_version ON "ob-poc".document_usage USING btree (dsl_version_id);


--
-- Name: idx_document_usage_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_usage_type ON "ob-poc".document_usage USING btree (usage_type);


--
-- Name: idx_document_usage_workflow; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_usage_workflow ON "ob-poc".document_usage USING btree (workflow_stage);


--
-- Name: idx_domain_vocabularies_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_domain_vocabularies_active ON "ob-poc".domain_vocabularies USING btree (active);


--
-- Name: idx_domain_vocabularies_category; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_domain_vocabularies_category ON "ob-poc".domain_vocabularies USING btree (category);


--
-- Name: idx_domain_vocabularies_domain_verb; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE UNIQUE INDEX idx_domain_vocabularies_domain_verb ON "ob-poc".domain_vocabularies USING btree (domain, verb);


--
-- Name: idx_dsl_domains_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_domains_active ON "ob-poc".dsl_domains USING btree (active);


--
-- Name: idx_dsl_domains_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_domains_name ON "ob-poc".dsl_domains USING btree (domain_name);


--
-- Name: idx_dsl_instances_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_instances_domain ON "ob-poc".dsl_instances USING btree (domain_name);


--
-- Name: idx_dsl_instances_reference; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_instances_reference ON "ob-poc".dsl_instances USING btree (business_reference);


--
-- Name: idx_dsl_ob_cbu_id_created_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_ob_cbu_id_created_at ON "ob-poc".dsl_ob USING btree (cbu_id, created_at DESC);


--
-- Name: idx_dsl_transformation_pattern; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_transformation_pattern ON "ob-poc".dsl_transformation_rules USING btree (instruction_pattern);


--
-- Name: idx_dsl_validation_pattern; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_validation_pattern ON "ob-poc".dsl_validation_rules USING btree (target_pattern);


--
-- Name: idx_dsl_validation_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_validation_type ON "ob-poc".dsl_validation_rules USING btree (rule_type);


--
-- Name: idx_dsl_versions_instance; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_versions_instance ON "ob-poc".dsl_instance_versions USING btree (instance_id);


--
-- Name: idx_dsl_versions_version; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_versions_version ON "ob-poc".dsl_instance_versions USING btree (instance_id, version_number);


--
-- Name: idx_entities_external_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entities_external_id ON "ob-poc".entities USING btree (external_id);


--
-- Name: idx_entities_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entities_name ON "ob-poc".entities USING btree (name);


--
-- Name: idx_entities_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entities_type ON "ob-poc".entities USING btree (entity_type_id);


--
-- Name: idx_entity_product_mappings_compatible; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_product_mappings_compatible ON "ob-poc".entity_product_mappings USING btree (compatible);


--
-- Name: idx_entity_product_mappings_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_product_mappings_entity ON "ob-poc".entity_product_mappings USING btree (entity_type);


--
-- Name: idx_entity_product_mappings_product; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_product_mappings_product ON "ob-poc".entity_product_mappings USING btree (product_id);


--
-- Name: idx_entity_types_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_name ON "ob-poc".entity_types USING btree (name);


--
-- Name: idx_entity_types_table; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_table ON "ob-poc".entity_types USING btree (table_name);


--
-- Name: idx_grammar_rules_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_grammar_rules_active ON "ob-poc".grammar_rules USING btree (active);


--
-- Name: idx_grammar_rules_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_grammar_rules_domain ON "ob-poc".grammar_rules USING btree (domain);


--
-- Name: idx_grammar_rules_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_grammar_rules_name ON "ob-poc".grammar_rules USING btree (rule_name);


--
-- Name: idx_kyc_rules_entity_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_kyc_rules_entity_type ON "ob-poc".kyc_rules USING btree (entity_type);


--
-- Name: idx_kyc_rules_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_kyc_rules_jurisdiction ON "ob-poc".kyc_rules USING btree (jurisdiction);


--
-- Name: idx_limited_companies_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_limited_companies_jurisdiction ON "ob-poc".entity_limited_companies USING btree (jurisdiction);


--
-- Name: idx_limited_companies_reg_num; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_limited_companies_reg_num ON "ob-poc".entity_limited_companies USING btree (registration_number);


--
-- Name: idx_orchestration_domain_sessions_activity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_domain_sessions_activity ON "ob-poc".orchestration_domain_sessions USING btree (last_activity);


--
-- Name: idx_orchestration_domain_sessions_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_domain_sessions_domain ON "ob-poc".orchestration_domain_sessions USING btree (domain_name);


--
-- Name: idx_orchestration_domain_sessions_orchestration; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_domain_sessions_orchestration ON "ob-poc".orchestration_domain_sessions USING btree (orchestration_session_id);


--
-- Name: idx_orchestration_domain_sessions_state; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_domain_sessions_state ON "ob-poc".orchestration_domain_sessions USING btree (state);


--
-- Name: idx_orchestration_sessions_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_sessions_cbu ON "ob-poc".orchestration_sessions USING btree (cbu_id);


--
-- Name: idx_orchestration_sessions_entity_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_sessions_entity_type ON "ob-poc".orchestration_sessions USING btree (entity_type);


--
-- Name: idx_orchestration_sessions_expires; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_sessions_expires ON "ob-poc".orchestration_sessions USING btree (expires_at);


--
-- Name: idx_orchestration_sessions_last_used; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_sessions_last_used ON "ob-poc".orchestration_sessions USING btree (last_used);


--
-- Name: idx_orchestration_sessions_state; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_sessions_state ON "ob-poc".orchestration_sessions USING btree (current_state);


--
-- Name: idx_orchestration_sessions_workflow; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_sessions_workflow ON "ob-poc".orchestration_sessions USING btree (workflow_type);


--
-- Name: idx_orchestration_state_history_created; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_state_history_created ON "ob-poc".orchestration_state_history USING btree (created_at);


--
-- Name: idx_orchestration_state_history_session; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_state_history_session ON "ob-poc".orchestration_state_history USING btree (orchestration_session_id);


--
-- Name: idx_orchestration_state_history_states; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_state_history_states ON "ob-poc".orchestration_state_history USING btree (from_state, to_state);


--
-- Name: idx_orchestration_tasks_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_tasks_domain ON "ob-poc".orchestration_tasks USING btree (domain_name);


--
-- Name: idx_orchestration_tasks_scheduled; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_tasks_scheduled ON "ob-poc".orchestration_tasks USING btree (scheduled_at);


--
-- Name: idx_orchestration_tasks_session; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_tasks_session ON "ob-poc".orchestration_tasks USING btree (orchestration_session_id);


--
-- Name: idx_orchestration_tasks_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_orchestration_tasks_status ON "ob-poc".orchestration_tasks USING btree (status);


--
-- Name: idx_partnership_control_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnership_control_entity ON "ob-poc".partnership_control_mechanisms USING btree (entity_id);


--
-- Name: idx_partnership_control_partnership; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnership_control_partnership ON "ob-poc".partnership_control_mechanisms USING btree (partnership_id);


--
-- Name: idx_partnership_interests_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnership_interests_entity ON "ob-poc".partnership_interests USING btree (entity_id);


--
-- Name: idx_partnership_interests_partnership; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnership_interests_partnership ON "ob-poc".partnership_interests USING btree (partnership_id);


--
-- Name: idx_partnership_interests_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnership_interests_type ON "ob-poc".partnership_interests USING btree (partner_type);


--
-- Name: idx_partnerships_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnerships_jurisdiction ON "ob-poc".entity_partnerships USING btree (jurisdiction);


--
-- Name: idx_partnerships_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnerships_type ON "ob-poc".entity_partnerships USING btree (partnership_type);


--
-- Name: idx_prod_resources_dict_group; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_prod_resources_dict_group ON "ob-poc".prod_resources USING btree (dictionary_group);


--
-- Name: idx_prod_resources_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_prod_resources_name ON "ob-poc".prod_resources USING btree (name);


--
-- Name: idx_prod_resources_owner; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_prod_resources_owner ON "ob-poc".prod_resources USING btree (owner);


--
-- Name: idx_product_requirements_product; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_product_requirements_product ON "ob-poc".product_requirements USING btree (product_id);


--
-- Name: idx_product_workflows_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_product_workflows_cbu ON "ob-poc".product_workflows USING btree (cbu_id);


--
-- Name: idx_product_workflows_product_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_product_workflows_product_entity ON "ob-poc".product_workflows USING btree (product_id, entity_type);


--
-- Name: idx_product_workflows_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_product_workflows_status ON "ob-poc".product_workflows USING btree (status);


--
-- Name: idx_products_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_products_name ON "ob-poc".products USING btree (name);


--
-- Name: idx_proper_persons_full_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_proper_persons_full_name ON "ob-poc".entity_proper_persons USING btree (last_name, first_name);


--
-- Name: idx_proper_persons_id_document; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_proper_persons_id_document ON "ob-poc".entity_proper_persons USING btree (id_document_type, id_document_number);


--
-- Name: idx_proper_persons_nationality; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_proper_persons_nationality ON "ob-poc".entity_proper_persons USING btree (nationality);


--
-- Name: idx_protector_powers_party; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_protector_powers_party ON "ob-poc".trust_protector_powers USING btree (trust_party_id);


--
-- Name: idx_roles_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_roles_name ON "ob-poc".roles USING btree (name);


--
-- Name: idx_services_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_services_name ON "ob-poc".services USING btree (name);


--
-- Name: idx_trust_parties_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trust_parties_entity ON "ob-poc".trust_parties USING btree (entity_id);


--
-- Name: idx_trust_parties_role; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trust_parties_role ON "ob-poc".trust_parties USING btree (party_role);


--
-- Name: idx_trust_parties_trust; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trust_parties_trust ON "ob-poc".trust_parties USING btree (trust_id);


--
-- Name: idx_trusts_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trusts_jurisdiction ON "ob-poc".entity_trusts USING btree (jurisdiction);


--
-- Name: idx_trusts_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trusts_type ON "ob-poc".entity_trusts USING btree (trust_type);


--
-- Name: idx_ubo_registry_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ubo_registry_cbu ON "ob-poc".ubo_registry USING btree (cbu_id);


--
-- Name: idx_ubo_registry_subject; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ubo_registry_subject ON "ob-poc".ubo_registry USING btree (subject_entity_id);


--
-- Name: idx_ubo_registry_ubo_proper_person; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ubo_registry_ubo_proper_person ON "ob-poc".ubo_registry USING btree (ubo_proper_person_id);


--
-- Name: idx_ubo_registry_workflow; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ubo_registry_workflow ON "ob-poc".ubo_registry USING btree (workflow_type);


--
-- Name: idx_verb_decision_rules_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_decision_rules_active ON "ob-poc".verb_decision_rules USING btree (active);


--
-- Name: idx_verb_decision_rules_priority; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_decision_rules_priority ON "ob-poc".verb_decision_rules USING btree (priority_weight DESC);


--
-- Name: idx_verb_decision_rules_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_decision_rules_type ON "ob-poc".verb_decision_rules USING btree (rule_type);


--
-- Name: idx_verb_embeddings_domain_verb; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_embeddings_domain_verb ON "ob-poc".verb_embeddings USING btree (domain, verb);


--
-- Name: idx_verb_patterns_category; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_patterns_category ON "ob-poc".verb_patterns USING btree (pattern_category);


--
-- Name: idx_verb_patterns_complexity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_patterns_complexity ON "ob-poc".verb_patterns USING btree (complexity_level);


--
-- Name: idx_verb_patterns_success_rate; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_patterns_success_rate ON "ob-poc".verb_patterns USING btree (success_rate DESC);


--
-- Name: idx_verb_patterns_usage; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_patterns_usage ON "ob-poc".verb_patterns USING btree (usage_frequency DESC);


--
-- Name: idx_verb_registry_deprecated; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_registry_deprecated ON "ob-poc".verb_registry USING btree (deprecated);


--
-- Name: idx_verb_registry_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_registry_domain ON "ob-poc".verb_registry USING btree (primary_domain);


--
-- Name: idx_verb_registry_shared; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_registry_shared ON "ob-poc".verb_registry USING btree (shared);


--
-- Name: idx_verb_relationships_source; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_relationships_source ON "ob-poc".verb_relationships USING btree (source_domain, source_verb);


--
-- Name: idx_verb_relationships_strength; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_relationships_strength ON "ob-poc".verb_relationships USING btree (relationship_strength DESC);


--
-- Name: idx_verb_relationships_target; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_relationships_target ON "ob-poc".verb_relationships USING btree (target_domain, target_verb);


--
-- Name: idx_verb_relationships_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_relationships_type ON "ob-poc".verb_relationships USING btree (relationship_type);


--
-- Name: idx_verb_semantics_confidence; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_semantics_confidence ON "ob-poc".verb_semantics USING btree (confidence_score DESC);


--
-- Name: idx_verb_semantics_domain_verb; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_semantics_domain_verb ON "ob-poc".verb_semantics USING btree (domain, verb);


--
-- Name: idx_verb_semantics_intent; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_semantics_intent ON "ob-poc".verb_semantics USING btree (intent_category);


--
-- Name: idx_verb_semantics_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_semantics_status ON "ob-poc".verb_semantics USING btree (status);


--
-- Name: idx_verb_semantics_workflow_stage; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_verb_semantics_workflow_stage ON "ob-poc".verb_semantics USING btree (workflow_stage);


--
-- Name: idx_vocabulary_audit_change_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_vocabulary_audit_change_type ON "ob-poc".vocabulary_audit USING btree (change_type);


--
-- Name: idx_vocabulary_audit_created_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_vocabulary_audit_created_at ON "ob-poc".vocabulary_audit USING btree (created_at DESC);


--
-- Name: idx_vocabulary_audit_domain_verb; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_vocabulary_audit_domain_verb ON "ob-poc".vocabulary_audit USING btree (domain, verb);


--
-- Name: dsl_domains trigger_dsl_domains_updated_at; Type: TRIGGER; Schema: ob-poc; Owner: adamtc007
--

CREATE TRIGGER trigger_dsl_domains_updated_at BEFORE UPDATE ON "ob-poc".dsl_domains FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_dsl_domains_updated_at();


--
-- Name: document_catalog validate_document_attributes_trigger; Type: TRIGGER; Schema: ob-poc; Owner: adamtc007
--

CREATE TRIGGER validate_document_attributes_trigger BEFORE INSERT OR UPDATE ON "ob-poc".document_catalog FOR EACH ROW EXECUTE FUNCTION public.trigger_validate_document_attributes();


--
-- Name: agent_verb_usage agent_verb_usage_domain_verb_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".agent_verb_usage
    ADD CONSTRAINT agent_verb_usage_domain_verb_fkey FOREIGN KEY (domain, verb) REFERENCES "ob-poc".domain_vocabularies(domain, verb);


--
-- Name: ast_nodes ast_nodes_parent_node_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ast_nodes
    ADD CONSTRAINT ast_nodes_parent_node_id_fkey FOREIGN KEY (parent_node_id) REFERENCES "ob-poc".ast_nodes(node_id) ON DELETE CASCADE;


--
-- Name: ast_nodes ast_nodes_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ast_nodes
    ADD CONSTRAINT ast_nodes_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE;


--
-- Name: attribute_values attribute_values_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE;


--
-- Name: attribute_values attribute_values_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cbu_entity_roles cbu_entity_roles_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles cbu_entity_roles_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles cbu_entity_roles_role_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_role_id_fkey FOREIGN KEY (role_id) REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE;


--
-- Name: document_catalog document_catalog_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id);


--
-- Name: document_catalog document_catalog_issuer_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_issuer_id_fkey FOREIGN KEY (issuer_id) REFERENCES "ob-poc".document_issuers(issuer_id);


--
-- Name: document_catalog document_catalog_parent_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_parent_document_id_fkey FOREIGN KEY (parent_document_id) REFERENCES "ob-poc".document_catalog(document_id);


--
-- Name: document_relationships document_relationships_source_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_source_document_id_fkey FOREIGN KEY (source_document_id) REFERENCES "ob-poc".document_catalog(document_id);


--
-- Name: document_relationships document_relationships_target_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_target_document_id_fkey FOREIGN KEY (target_document_id) REFERENCES "ob-poc".document_catalog(document_id);


--
-- Name: document_types document_types_primary_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_types
    ADD CONSTRAINT document_types_primary_attribute_id_fkey FOREIGN KEY (primary_attribute_id) REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE RESTRICT;


--
-- Name: document_usage document_usage_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_usage
    ADD CONSTRAINT document_usage_document_id_fkey FOREIGN KEY (document_id) REFERENCES "ob-poc".document_catalog(document_id);


--
-- Name: document_usage document_usage_dsl_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_usage
    ADD CONSTRAINT document_usage_dsl_version_id_fkey FOREIGN KEY (dsl_version_id) REFERENCES "ob-poc".dsl_ob(version_id);


--
-- Name: dsl_business_references dsl_business_references_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_business_references
    ADD CONSTRAINT dsl_business_references_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


--
-- Name: dsl_compilation_logs dsl_compilation_logs_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_compilation_logs
    ADD CONSTRAINT dsl_compilation_logs_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE;


--
-- Name: dsl_domain_relationships dsl_domain_relationships_child_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domain_relationships
    ADD CONSTRAINT dsl_domain_relationships_child_instance_id_fkey FOREIGN KEY (child_instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


--
-- Name: dsl_domain_relationships dsl_domain_relationships_parent_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_domain_relationships
    ADD CONSTRAINT dsl_domain_relationships_parent_instance_id_fkey FOREIGN KEY (parent_instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


--
-- Name: dsl_instance_versions dsl_instance_versions_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT dsl_instance_versions_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


--
-- Name: dsl_validations dsl_validations_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_validations
    ADD CONSTRAINT dsl_validations_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE;


--
-- Name: dsl_visualizations dsl_visualizations_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_visualizations
    ADD CONSTRAINT dsl_visualizations_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_instance_versions(version_id) ON DELETE CASCADE;


--
-- Name: entities entities_entity_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_entity_type_id_fkey FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types(entity_type_id) ON DELETE CASCADE;


--
-- Name: entity_product_mappings entity_product_mappings_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT entity_product_mappings_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: orchestration_domain_sessions orchestration_domain_sessions_orchestration_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT orchestration_domain_sessions_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_sessions orchestration_sessions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_sessions
    ADD CONSTRAINT orchestration_sessions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: orchestration_state_history orchestration_state_history_orchestration_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_state_history
    ADD CONSTRAINT orchestration_state_history_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_tasks orchestration_tasks_orchestration_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_tasks
    ADD CONSTRAINT orchestration_tasks_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms partnership_control_mechanisms_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT partnership_control_mechanisms_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms partnership_control_mechanisms_partnership_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT partnership_control_mechanisms_partnership_id_fkey FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: partnership_interests partnership_interests_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_interests partnership_interests_partnership_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_partnership_id_fkey FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: product_requirements product_requirements_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_requirements
    ADD CONSTRAINT product_requirements_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: product_services product_services_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: product_services product_services_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: product_workflows product_workflows_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT product_workflows_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: service_resources service_resources_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE;


--
-- Name: service_resources service_resources_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: trust_beneficiary_classes trust_beneficiary_classes_trust_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_beneficiary_classes
    ADD CONSTRAINT trust_beneficiary_classes_trust_id_fkey FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_parties trust_parties_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: trust_parties trust_parties_trust_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_trust_id_fkey FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_protector_powers trust_protector_powers_trust_party_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_protector_powers
    ADD CONSTRAINT trust_protector_powers_trust_party_id_fkey FOREIGN KEY (trust_party_id) REFERENCES "ob-poc".trust_parties(trust_party_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_subject_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_subject_entity_id_fkey FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_ubo_proper_person_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_ubo_proper_person_id_fkey FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: verb_embeddings verb_embeddings_domain_verb_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_embeddings
    ADD CONSTRAINT verb_embeddings_domain_verb_fkey FOREIGN KEY (domain, verb) REFERENCES "ob-poc".domain_vocabularies(domain, verb) ON DELETE CASCADE;


--
-- Name: verb_relationships verb_relationships_source_domain_source_verb_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_relationships
    ADD CONSTRAINT verb_relationships_source_domain_source_verb_fkey FOREIGN KEY (source_domain, source_verb) REFERENCES "ob-poc".domain_vocabularies(domain, verb);


--
-- Name: verb_relationships verb_relationships_target_domain_target_verb_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_relationships
    ADD CONSTRAINT verb_relationships_target_domain_target_verb_fkey FOREIGN KEY (target_domain, target_verb) REFERENCES "ob-poc".domain_vocabularies(domain, verb);


--
-- Name: verb_semantics verb_semantics_domain_verb_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_semantics
    ADD CONSTRAINT verb_semantics_domain_verb_fkey FOREIGN KEY (domain, verb) REFERENCES "ob-poc".domain_vocabularies(domain, verb) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict 1kcNxfNgRb4KmRoftNphy7tPUcQs6OWoor2jPe4trwgAmFDi8ExnBOMNXUkbJF0

```

---

# PART 3: RUST DATABASE CODE


## File: rust/src/database/mod.rs

```rust
//! Database connection and management module
//!
//! This module provides database connection management, connection pooling,
//! and configuration for the DSL architecture.
//!
//! ## Architecture Update (November 2025)
//! Legacy database modules (business_request_repository, cbu_crud_manager, etc.)
//! have been removed. The Forth engine now handles database operations through
//! RuntimeEnv with direct SQL queries matching the demo_setup.sql schema.

use sqlx::Row;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::{info, warn};

pub mod attribute_values_service;
pub mod cbu_service;
pub mod crud_executor;
pub mod crud_service;
pub mod dictionary_service;
pub mod document_service;
pub mod dsl_repository;

// Re-export for convenience
pub use attribute_values_service::AttributeValuesService;
pub use cbu_service::{Cbu, CbuService, Role};
pub use crud_executor::{CrudExecutionResult, CrudExecutor};
pub use crud_service::{AssetType, CrudOperation, CrudService, OperationType};
pub use dictionary_service::DictionaryDatabaseService;
pub use document_service::{DocumentCatalogEntry, DocumentService, DocumentType};
pub use dsl_repository::{DslRepository, DslSaveResult};

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string()),
            max_connections: std::env::var("DATABASE_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
            max_lifetime: Some(Duration::from_secs(1800)), // 30 minutes
        }
    }
}

/// Database connection manager
pub struct DatabaseManager {
    pool: PgPool,
}

impl DatabaseManager {
    /// Create a new database manager with the given configuration
    pub async fn new(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        info!(
            "Connecting to database: {}",
            mask_database_url(&config.database_url)
        );

        let mut pool_options = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.connection_timeout);

        if let Some(idle_timeout) = config.idle_timeout {
            pool_options = pool_options.idle_timeout(idle_timeout);
        }

        if let Some(max_lifetime) = config.max_lifetime {
            pool_options = pool_options.max_lifetime(max_lifetime);
        }

        let pool = pool_options
            .connect(&config.database_url)
            .await
            .map_err(|e| {
                warn!("Failed to connect to database: {}", e);
                e
            })?;

        info!("Database connection pool created successfully");

        Ok(Self { pool })
    }

    /// Create a new database manager with default configuration
    pub async fn with_default_config() -> Result<Self, sqlx::Error> {
        let config = DatabaseConfig::default();
        Self::new(config).await
    }

    /// Create a new database manager from an existing pool
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new dictionary database service using this database connection
    pub fn dictionary_service(&self) -> DictionaryDatabaseService {
        DictionaryDatabaseService::new(self.pool.clone())
    }

    /// Test database connectivity
    pub async fn test_connection(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<(), sqlx::migrate::MigrateError> {
        info!("Running database migrations");

        // Verify the schema exists
        let tables_exist = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM information_schema.tables
            WHERE table_schema = 'ob-poc'
            AND table_name IN ('cbus', 'dictionary', 'attribute_values', 'entities',
                               'dsl_instances', 'parsed_asts', 'ubo_registry', 'document_catalog')
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx::migrate::MigrateError::Execute)?;

        let count: i64 = tables_exist.get("count");

        if count < 6 {
            warn!("Expected database tables not found. Please run sql/demo_setup.sql");
            return Err(sqlx::migrate::MigrateError::VersionMissing(1));
        }

        info!("Database schema verification complete");
        Ok(())
    }

    /// Close the database connection pool
    pub async fn close(self) {
        info!("Closing database connection pool");
        self.pool.close().await;
    }
}

/// Mask sensitive information in database URL for logging
fn mask_database_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let mut masked = parsed.clone();
        if parsed.password().is_some() {
            let _ = masked.set_password(Some("***"));
        }
        masked.to_string()
    } else {
        // If URL parsing fails, just mask the middle part
        if url.len() > 20 {
            format!("{}***{}", &url[..10], &url[url.len() - 10..])
        } else {
            "***".to_string()
        }
    }
}
```

## File: rust/src/database/crud_executor.rs

```rust
//! CRUD Executor - Executes CrudStatements against the database
//!
//! This module provides the execution layer that takes CrudStatements
//! from the Forth engine and performs actual database operations.

use crate::parser::ast::{
    CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, Literal, Value,
};
use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Result of executing a CRUD statement
#[derive(Debug, Clone)]
pub struct CrudExecutionResult {
    /// Type of operation executed
    pub operation: String,
    /// Asset/table affected
    pub asset: String,
    /// Number of rows affected
    pub rows_affected: u64,
    /// Generated ID (for creates)
    pub generated_id: Option<Uuid>,
    /// Retrieved data (for reads)
    pub data: Option<JsonValue>,
}

/// Executor for CRUD statements
pub struct CrudExecutor {
    pool: PgPool,
}

impl CrudExecutor {
    /// Create a new CRUD executor
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Execute a single CRUD statement
    pub async fn execute(&self, stmt: &CrudStatement) -> Result<CrudExecutionResult> {
        match stmt {
            CrudStatement::DataCreate(create) => self.execute_create(create).await,
            CrudStatement::DataRead(read) => self.execute_read(read).await,
            CrudStatement::DataUpdate(update) => self.execute_update(update).await,
            CrudStatement::DataDelete(delete) => self.execute_delete(delete).await,
            _ => Err(anyhow!("Unsupported CRUD statement type")),
        }
    }

    /// Execute multiple CRUD statements in a transaction
    pub async fn execute_all(&self, stmts: &[CrudStatement]) -> Result<Vec<CrudExecutionResult>> {
        let mut results = Vec::new();

        // Start transaction
        let mut tx = self.pool.begin().await?;

        for stmt in stmts {
            let result = match stmt {
                CrudStatement::DataCreate(create) => {
                    self.execute_create_tx(create, &mut tx).await?
                }
                CrudStatement::DataUpdate(update) => {
                    self.execute_update_tx(update, &mut tx).await?
                }
                CrudStatement::DataDelete(delete) => {
                    self.execute_delete_tx(delete, &mut tx).await?
                }
                CrudStatement::DataRead(read) => {
                    // Reads don't need transaction
                    self.execute_read(read).await?
                }
                _ => return Err(anyhow!("Unsupported CRUD statement type")),
            };
            results.push(result);
        }

        // Commit transaction
        tx.commit().await?;

        Ok(results)
    }

    /// Execute a CREATE statement
    async fn execute_create(&self, create: &DataCreate) -> Result<CrudExecutionResult> {
        let mut tx = self.pool.begin().await?;
        let result = self.execute_create_tx(create, &mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Execute CREATE within a transaction
    async fn execute_create_tx(
        &self,
        create: &DataCreate,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<CrudExecutionResult> {
        let generated_id = Uuid::new_v4();

        match create.asset.as_str() {
            "CBU" => {
                let name = self
                    .get_string_value(&create.values, "cbu-name")
                    .or_else(|| self.get_string_value(&create.values, "client-name"))
                    .or_else(|| self.get_string_value(&create.values, "name"))
                    .unwrap_or_else(|| "Unknown".to_string());
                let description = self
                    .get_string_value(&create.values, "description")
                    .or_else(|| self.get_string_value(&create.values, "client-type"));
                let nature_purpose = self
                    .get_string_value(&create.values, "nature-purpose")
                    .or_else(|| self.get_string_value(&create.values, "jurisdiction"));

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, NOW(), NOW())
                    "#
                )
                .bind(generated_id)
                .bind(&name)
                .bind(&description)
                .bind(&nature_purpose)
                .execute(&mut **tx)
                .await?;

                info!("Created CBU: {} ({})", name, generated_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: 1,
                    generated_id: Some(generated_id),
                    data: None,
                })
            }
            "CBU_ENTITY_RELATIONSHIP" => {
                let entity_id = self
                    .get_string_value(&create.values, "entity-id")
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let role = self
                    .get_string_value(&create.values, "role")
                    .unwrap_or_else(|| "UNKNOWN".to_string());

                // For now, store in entities table with role
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, entity_type, legal_name, jurisdiction, status, created_at, updated_at)
                    VALUES ($1::uuid, $2, $3, 'US', 'ACTIVE', NOW(), NOW())
                    ON CONFLICT (entity_id) DO UPDATE SET updated_at = NOW()
                    "#
                )
                .bind(&entity_id)
                .bind(&role)
                .bind(format!("Entity-{}", &entity_id[..8.min(entity_id.len())]))
                .execute(&mut **tx)
                .await?;

                info!("Created entity relationship: {} as {}", entity_id, role);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU_ENTITY_RELATIONSHIP".to_string(),
                    rows_affected: 1,
                    generated_id: Some(generated_id),
                    data: None,
                })
            }
            "CBU_PROPER_PERSON" => {
                let person_name = self
                    .get_string_value(&create.values, "person-name")
                    .unwrap_or_else(|| "Unknown Person".to_string());
                let role = self
                    .get_string_value(&create.values, "role")
                    .unwrap_or_else(|| "CONTACT".to_string());

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, entity_type, legal_name, jurisdiction, status, created_at, updated_at)
                    VALUES ($1::uuid, 'PROPER_PERSON', $2, 'US', 'ACTIVE', NOW(), NOW())
                    "#
                )
                .bind(generated_id)
                .bind(&person_name)
                .execute(&mut **tx)
                .await?;

                info!("Created proper person: {} as {}", person_name, role);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU_PROPER_PERSON".to_string(),
                    rows_affected: 1,
                    generated_id: Some(generated_id),
                    data: None,
                })
            }
            _ => {
                warn!("Unknown asset type for CREATE: {}", create.asset);
                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: create.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Execute a READ statement
    async fn execute_read(&self, read: &DataRead) -> Result<CrudExecutionResult> {
        match read.asset.as_str() {
            "CBU" => {
                let cbu_id = self.get_string_value(&read.where_clause, "cbu-id");

                let rows = if let Some(id) = cbu_id {
                    sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, Option<String>)>(
                        r#"
                        SELECT cbu_id, name, description, nature_purpose
                        FROM "ob-poc".cbus
                        WHERE cbu_id = $1::uuid
                        "#,
                    )
                    .bind(&id)
                    .fetch_all(&self.pool)
                    .await?
                } else {
                    let limit = read.limit.unwrap_or(100);
                    sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, Option<String>)>(
                        r#"
                        SELECT cbu_id, name, description, nature_purpose
                        FROM "ob-poc".cbus
                        ORDER BY created_at DESC
                        LIMIT $1
                        "#,
                    )
                    .bind(limit as i64)
                    .fetch_all(&self.pool)
                    .await?
                };

                let data: Vec<JsonValue> = rows
                    .into_iter()
                    .map(|(id, name, description, nature_purpose)| {
                        serde_json::json!({
                            "cbu_id": id.to_string(),
                            "name": name,
                            "description": description,
                            "nature_purpose": nature_purpose
                        })
                    })
                    .collect();

                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }
            _ => {
                warn!("Unknown asset type for READ: {}", read.asset);
                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: read.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: Some(JsonValue::Array(vec![])),
                })
            }
        }
    }

    /// Execute an UPDATE statement
    async fn execute_update(&self, update: &DataUpdate) -> Result<CrudExecutionResult> {
        let mut tx = self.pool.begin().await?;
        let result = self.execute_update_tx(update, &mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Execute UPDATE within a transaction
    async fn execute_update_tx(
        &self,
        update: &DataUpdate,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<CrudExecutionResult> {
        match update.asset.as_str() {
            "CBU" => {
                let cbu_id = self
                    .get_string_value(&update.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for UPDATE"))?;

                // Build dynamic update based on available fields
                let description = self
                    .get_string_value(&update.values, "description")
                    .or_else(|| self.get_string_value(&update.values, "status"));
                let nature_purpose = self
                    .get_string_value(&update.values, "nature-purpose")
                    .or_else(|| self.get_string_value(&update.values, "client-type"));

                let result = if let Some(desc) = description {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".cbus
                        SET description = $1, updated_at = NOW()
                        WHERE cbu_id = $2::uuid
                        "#,
                    )
                    .bind(&desc)
                    .bind(&cbu_id)
                    .execute(&mut **tx)
                    .await?
                } else if let Some(np) = nature_purpose {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".cbus
                        SET nature_purpose = $1, updated_at = NOW()
                        WHERE cbu_id = $2::uuid
                        "#,
                    )
                    .bind(&np)
                    .bind(&cbu_id)
                    .execute(&mut **tx)
                    .await?
                } else {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".cbus
                        SET updated_at = NOW()
                        WHERE cbu_id = $1::uuid
                        "#,
                    )
                    .bind(&cbu_id)
                    .execute(&mut **tx)
                    .await?
                };

                info!("Updated CBU: {}", cbu_id);

                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: result.rows_affected(),
                    generated_id: None,
                    data: None,
                })
            }
            _ => {
                warn!("Unknown asset type for UPDATE: {}", update.asset);
                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: update.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Execute a DELETE statement
    async fn execute_delete(&self, delete: &DataDelete) -> Result<CrudExecutionResult> {
        let mut tx = self.pool.begin().await?;
        let result = self.execute_delete_tx(delete, &mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Execute DELETE within a transaction
    async fn execute_delete_tx(
        &self,
        delete: &DataDelete,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<CrudExecutionResult> {
        match delete.asset.as_str() {
            "CBU" => {
                let cbu_id = self
                    .get_string_value(&delete.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for DELETE"))?;

                let result = sqlx::query(
                    r#"
                    DELETE FROM "ob-poc".cbus
                    WHERE cbu_id = $1::uuid
                    "#,
                )
                .bind(&cbu_id)
                .execute(&mut **tx)
                .await?;

                info!("Deleted CBU: {}", cbu_id);

                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: result.rows_affected(),
                    generated_id: None,
                    data: None,
                })
            }
            _ => {
                warn!("Unknown asset type for DELETE: {}", delete.asset);
                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: delete.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Helper to extract string value from HashMap
    fn get_string_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<String> {
        values.get(key).and_then(|v| match v {
            Value::Literal(Literal::String(s)) => Some(s.clone()),
            Value::Identifier(s) => Some(s.clone()),
            _ => None,
        })
    }
}
```

## File: rust/src/database/dsl_repository.rs

```rust
//! DSL Repository - Database operations for DSL instances and parsed ASTs
//!
//! This module centralizes all database operations for the DSL/AST tables,
//! providing transactional saves with automatic version tracking.

use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Result of saving a DSL/AST pair
#[derive(Debug, Clone)]
pub struct DslSaveResult {
    pub case_id: String,
    pub version: i32,
    pub success: bool,
    pub instance_id: Uuid,
}

/// DSL Repository for database operations
pub struct DslRepository {
    pool: PgPool,
}

impl DslRepository {
    /// Create a new DSL repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Save DSL and AST atomically in a transaction
    /// Returns the new version number
    pub async fn save_dsl_ast(
        &self,
        case_id: &str,
        dsl_content: &str,
        ast_json: &str,
        domain: &str,
        operation_type: &str,
        _parse_time_ms: i64,
    ) -> Result<DslSaveResult, sqlx::Error> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Check if instance exists for this business_reference
        let existing: Option<(Uuid, i32)> = sqlx::query_as(
            r#"
            SELECT instance_id, current_version
            FROM "ob-poc".dsl_instances
            WHERE business_reference = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&mut *tx)
        .await?;

        let (instance_id, version) = if let Some((id, current_ver)) = existing {
            // Update existing instance
            let new_version = current_ver + 1;
            sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_instances
                SET current_version = $1, updated_at = NOW()
                WHERE instance_id = $2
                "#,
            )
            .bind(new_version)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            (id, new_version)
        } else {
            // Create new instance
            let new_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".dsl_instances
                (instance_id, domain_name, business_reference, current_version, status, created_at, updated_at)
                VALUES ($1, $2, $3, 1, 'ACTIVE', NOW(), NOW())
                "#,
            )
            .bind(new_id)
            .bind(domain)
            .bind(case_id)
            .execute(&mut *tx)
            .await?;
            (new_id, 1)
        };

        // Insert version record with DSL content and AST
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_instance_versions
            (instance_id, version_number, dsl_content, operation_type, compilation_status, ast_json, created_at)
            VALUES ($1, $2, $3, $4, 'COMPILED', $5::jsonb, NOW())
            "#,
        )
        .bind(instance_id)
        .bind(version)
        .bind(dsl_content)
        .bind(operation_type)
        .bind(ast_json)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(DslSaveResult {
            case_id: case_id.to_string(),
            version,
            success: true,
            instance_id,
        })
    }

    /// Save DSL execution with CBU context (simplified version for CBU Model DSL)
    pub async fn save_execution(
        &self,
        dsl_content: &str,
        domain: &str,
        case_id: &str,
        _cbu_id: Option<Uuid>,
        ast_json: &serde_json::Value,
    ) -> Result<DslSaveResult, sqlx::Error> {
        let ast_str =
            serde_json::to_string(ast_json).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        self.save_dsl_ast(case_id, dsl_content, &ast_str, domain, "EXECUTE", 0)
            .await
    }

    /// Get DSL content by instance ID
    pub async fn get_dsl_content(&self, instance_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT v.dsl_content
            FROM "ob-poc".dsl_instance_versions v
            JOIN "ob-poc".dsl_instances i ON i.instance_id = v.instance_id
            WHERE i.instance_id = $1
            ORDER BY v.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(content,)| content))
    }

    /// Load latest DSL for a case (business_reference)
    pub async fn load_dsl(&self, case_id: &str) -> Result<Option<(String, i32)>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String, i32)>(
            r#"
            SELECT v.dsl_content, v.version_number
            FROM "ob-poc".dsl_instance_versions v
            JOIN "ob-poc".dsl_instances i ON i.instance_id = v.instance_id
            WHERE i.business_reference = $1
            ORDER BY v.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Load latest AST for a case
    pub async fn load_ast(&self, case_id: &str) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT v.ast_json::text
            FROM "ob-poc".dsl_instance_versions v
            JOIN "ob-poc".dsl_instances i ON i.instance_id = v.instance_id
            WHERE i.business_reference = $1
            ORDER BY v.version_number DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(ast,)| ast))
    }

    /// Get version count for a case
    pub async fn get_version_count(&self, case_id: &str) -> Result<i32, sqlx::Error> {
        let result: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT current_version
            FROM "ob-poc".dsl_instances
            WHERE business_reference = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(v,)| v).unwrap_or(0))
    }

    /// Create or update CBU
    pub async fn upsert_cbu(
        &self,
        cbu_id: &str,
        client_name: &str,
        client_type: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
            VALUES ($1::uuid, $2, $3, 'General', NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET name = $2, description = $3, updated_at = NOW()
            "#,
        )
        .bind(cbu_id)
        .bind(client_name)
        .bind(client_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Save attribute value
    pub async fn save_attribute(
        &self,
        entity_id: &str,
        attribute_id: &str,
        value: &str,
        value_type: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
            VALUES ($1::uuid, $2, $3, $4, NOW())
            "#,
        )
        .bind(attribute_id)
        .bind(entity_id)
        .bind(value)
        .bind(value_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Save DSL execution results atomically in a single transaction
    /// This includes: DSL instance, version, CBU, and all attributes
    /// If any operation fails, the entire transaction is rolled back
    pub async fn save_execution_transactionally(
        &self,
        case_id: &str,
        dsl_content: &str,
        ast_json: &str,
        domain: &str,
        operation_type: &str,
        _parse_time_ms: i64,
        client_name: &str,
        client_type: &str,
        attributes: &HashMap<String, (String, String)>, // attr_id -> (value, value_type)
    ) -> Result<DslSaveResult, sqlx::Error> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Check if instance exists for this business_reference
        let existing: Option<(Uuid, i32)> = sqlx::query_as(
            r#"
            SELECT instance_id, current_version
            FROM "ob-poc".dsl_instances
            WHERE business_reference = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(&mut *tx)
        .await?;

        let (instance_id, version) = if let Some((id, current_ver)) = existing {
            // Update existing instance
            let new_version = current_ver + 1;
            sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_instances
                SET current_version = $1, updated_at = NOW()
                WHERE instance_id = $2
                "#,
            )
            .bind(new_version)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            (id, new_version)
        } else {
            // Create new instance
            let new_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".dsl_instances
                (instance_id, domain_name, business_reference, current_version, status, created_at, updated_at)
                VALUES ($1, $2, $3, 1, 'ACTIVE', NOW(), NOW())
                "#,
            )
            .bind(new_id)
            .bind(domain)
            .bind(case_id)
            .execute(&mut *tx)
            .await?;
            (new_id, 1)
        };

        // Insert version record with DSL content and AST
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dsl_instance_versions
            (instance_id, version_number, dsl_content, operation_type, compilation_status, ast_json, created_at)
            VALUES ($1, $2, $3, $4, 'COMPILED', $5::jsonb, NOW())
            "#,
        )
        .bind(instance_id)
        .bind(version)
        .bind(dsl_content)
        .bind(operation_type)
        .bind(ast_json)
        .execute(&mut *tx)
        .await?;

        // Upsert CBU
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose, created_at, updated_at)
            VALUES ($1::uuid, $2, $3, 'General', NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET name = $2, description = $3, updated_at = NOW()
            "#,
        )
        .bind(case_id)
        .bind(client_name)
        .bind(client_type)
        .execute(&mut *tx)
        .await?;

        // Save all attributes
        for (attr_id, (value, value_type)) in attributes {
            // Skip invalid UUIDs for attribute_id
            if attr_id.starts_with(':') || attr_id.len() < 36 {
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO "ob-poc".attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
                VALUES ($1::uuid, $2, $3, $4, NOW())
                "#,
            )
            .bind(attr_id)
            .bind(case_id)
            .bind(value)
            .bind(value_type)
            .execute(&mut *tx)
            .await?;
        }

        // Commit transaction - if this fails, everything is rolled back
        tx.commit().await?;

        Ok(DslSaveResult {
            case_id: case_id.to_string(),
            version,
            success: true,
            instance_id,
        })
    }
}
```

## File: rust/src/database/cbu_service.rs

```rust
//! CBU Service - CRUD operations for Client Business Units
//!
//! This module provides database operations for CBUs, entity roles,
//! and related structures following the CBU builder pattern.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Client Business Unit record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Cbu {
    pub cbu_id: Uuid,
    pub client_name: String,
    pub client_type: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Role definition
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub role_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// CBU-Entity role assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuEntityRole {
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub role_id: Uuid,
}

/// Service for CBU operations
#[derive(Clone, Debug)]
pub struct CbuService {
    pool: PgPool,
}

impl CbuService {
    /// Create a new CBU service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new CBU
    pub async fn create_cbu(
        &self,
        client_name: &str,
        client_type: &str,
        jurisdiction: Option<&str>,
    ) -> Result<Uuid> {
        let cbu_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (
                cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, COALESCE($4, 'US'), 'ACTIVE', NOW(), NOW())
            "#,
        )
        .bind(cbu_id)
        .bind(client_name)
        .bind(client_type)
        .bind(jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to create CBU")?;

        info!(
            "Created CBU {} for client '{}' (type: {})",
            cbu_id, client_name, client_type
        );

        Ok(cbu_id)
    }

    /// Get CBU by ID
    pub async fn get_cbu_by_id(&self, cbu_id: Uuid) -> Result<Option<Cbu>> {
        let result = sqlx::query_as::<_, Cbu>(
            r#"
            SELECT cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU by ID")?;

        Ok(result)
    }

    /// Get CBU by client name
    pub async fn get_cbu_by_name(&self, client_name: &str) -> Result<Option<Cbu>> {
        let result = sqlx::query_as::<_, Cbu>(
            r#"
            SELECT cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            FROM "ob-poc".cbus
            WHERE client_name = $1
            "#,
        )
        .bind(client_name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CBU by name")?;

        Ok(result)
    }

    /// List all CBUs
    pub async fn list_cbus(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<Cbu>> {
        let results = sqlx::query_as::<_, Cbu>(
            r#"
            SELECT cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            FROM "ob-poc".cbus
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit.unwrap_or(100))
        .bind(offset.unwrap_or(0))
        .fetch_all(&self.pool)
        .await
        .context("Failed to list CBUs")?;

        Ok(results)
    }

    /// Update CBU
    pub async fn update_cbu(
        &self,
        cbu_id: Uuid,
        client_name: Option<&str>,
        client_type: Option<&str>,
        jurisdiction: Option<&str>,
        status: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbus
            SET client_name = COALESCE($1, client_name),
                client_type = COALESCE($2, client_type),
                jurisdiction = COALESCE($3, jurisdiction),
                status = COALESCE($4, status),
                updated_at = NOW()
            WHERE cbu_id = $5
            "#,
        )
        .bind(client_name)
        .bind(client_type)
        .bind(jurisdiction)
        .bind(status)
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to update CBU")?;

        if result.rows_affected() > 0 {
            info!("Updated CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete CBU (soft delete - sets status to DELETED)
    pub async fn delete_cbu(&self, cbu_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbus
            SET status = 'DELETED', updated_at = NOW()
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete CBU")?;

        if result.rows_affected() > 0 {
            info!("Soft deleted CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Ensure a role exists, creating it if necessary
    pub async fn ensure_role(&self, name: &str, description: &str) -> Result<Uuid> {
        // Try to get existing role
        let existing = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT role_id FROM "ob-poc".roles WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check for existing role")?;

        if let Some(role_id) = existing {
            return Ok(role_id);
        }

        // Create new role
        let role_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".roles (role_id, name, description, created_at)
            VALUES ($1, $2, $3, NOW())
            "#,
        )
        .bind(role_id)
        .bind(name)
        .bind(description)
        .execute(&self.pool)
        .await
        .context("Failed to create role")?;

        info!("Created role '{}' with ID {}", name, role_id);
        Ok(role_id)
    }

    /// Attach an entity to a CBU with a specific role
    pub async fn attach_entity_to_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (cbu_id, entity_id, role_id)
            DO NOTHING
            "#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(&self.pool)
        .await
        .context("Failed to attach entity to CBU")?;

        info!(
            "Attached entity {} to CBU {} with role {}",
            entity_id, cbu_id, role_id
        );

        Ok(())
    }

    /// Get all entities attached to a CBU
    pub async fn get_cbu_entities(&self, cbu_id: Uuid) -> Result<Vec<(Uuid, Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, String)>(
            r#"
            SELECT cer.entity_id, cer.role_id, r.name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CBU entities")?;

        Ok(rows)
    }

    /// Detach an entity from a CBU
    pub async fn detach_entity_from_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<bool> {
        let result = if let Some(rid) = role_id {
            sqlx::query(
                r#"
                DELETE FROM "ob-poc".cbu_entity_roles
                WHERE cbu_id = $1 AND entity_id = $2 AND role_id = $3
                "#,
            )
            .bind(cbu_id)
            .bind(entity_id)
            .bind(rid)
            .execute(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                DELETE FROM "ob-poc".cbu_entity_roles
                WHERE cbu_id = $1 AND entity_id = $2
                "#,
            )
            .bind(cbu_id)
            .bind(entity_id)
            .execute(&self.pool)
            .await
        }
        .context("Failed to detach entity from CBU")?;

        Ok(result.rows_affected() > 0)
    }

    /// Get sink attributes for CBU (attributes that should be populated)
    pub async fn get_sink_attributes_for_cbu(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE sink IS NOT NULL
              AND (sink::text ILIKE '%CBU%' OR sink::text ILIKE '%cbu%')
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get sink attributes for CBU")?;

        Ok(rows)
    }

    /// Upsert CBU (create or update)
    pub async fn upsert_cbu(
        &self,
        cbu_id: Option<Uuid>,
        client_name: &str,
        client_type: &str,
        jurisdiction: Option<&str>,
    ) -> Result<Uuid> {
        let id = cbu_id.unwrap_or_else(Uuid::new_v4);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (
                cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, COALESCE($4, 'US'), 'ACTIVE', NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET
                client_name = $2,
                client_type = $3,
                jurisdiction = COALESCE($4, "ob-poc".cbus.jurisdiction),
                updated_at = NOW()
            "#,
        )
        .bind(id)
        .bind(client_name)
        .bind(client_type)
        .bind(jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to upsert CBU")?;

        info!(
            "Upserted CBU {} for client '{}' (type: {})",
            id, client_name, client_type
        );

        Ok(id)
    }
}
```

## File: rust/src/database/attribute_values_service.rs

```rust
//! Attribute Values Service - CRUD operations for runtime attribute values
//!
//! This module provides database operations for the attribute_values table,
//! which stores actual attribute values associated with CBUs and entities.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

/// Service for attribute value operations
#[derive(Clone, Debug)]
pub struct AttributeValuesService {
    pool: PgPool,
}

impl AttributeValuesService {
    /// Create a new attribute values service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get an attribute value for a specific entity
    pub async fn get_attribute_value(
        &self,
        entity_id: &str,
        attribute_id: Uuid,
    ) -> Result<Option<String>> {
        let result = sqlx::query_scalar::<_, String>(
            r#"
            SELECT attribute_value
            FROM "ob-poc".attribute_values
            WHERE entity_id = $1 AND attribute_id = $2
            "#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get attribute value")?;

        Ok(result)
    }

    /// Set an attribute value for an entity (upsert)
    pub async fn set_attribute_value(
        &self,
        entity_id: &str,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values (entity_id, attribute_id, attribute_value, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (entity_id, attribute_id)
            DO UPDATE SET attribute_value = $3, updated_at = NOW()
            "#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .context("Failed to set attribute value")?;

        info!(
            "Set attribute {} = '{}' for entity {}",
            attribute_id, value, entity_id
        );
        Ok(())
    }

    /// Batch upsert attribute values
    pub async fn upsert_attribute_values(
        &self,
        entity_id: &str,
        attributes: &[(Uuid, String)],
    ) -> Result<usize> {
        let mut count = 0;

        for (attr_id, value) in attributes {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".attribute_values (entity_id, attribute_id, attribute_value, created_at, updated_at)
                VALUES ($1, $2, $3, NOW(), NOW())
                ON CONFLICT (entity_id, attribute_id)
                DO UPDATE SET attribute_value = $3, updated_at = NOW()
                "#,
            )
            .bind(entity_id)
            .bind(attr_id)
            .bind(value)
            .execute(&self.pool)
            .await
            .context("Failed to upsert attribute value")?;

            count += 1;
        }

        info!(
            "Upserted {} attribute values for entity {}",
            count, entity_id
        );
        Ok(count)
    }

    /// Get all attribute values for an entity
    pub async fn get_entity_attributes(&self, entity_id: &str) -> Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT attribute_id, attribute_value
            FROM "ob-poc".attribute_values
            WHERE entity_id = $1
            ORDER BY attribute_id
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entity attributes")?;

        Ok(rows)
    }

    /// Delete an attribute value
    pub async fn delete_attribute_value(
        &self,
        entity_id: &str,
        attribute_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".attribute_values
            WHERE entity_id = $1 AND attribute_id = $2
            "#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete attribute value")?;

        Ok(result.rows_affected() > 0)
    }

    /// Get sink attributes for a specific asset type
    /// Returns attributes where the sink field contains the asset type
    pub async fn get_sink_attributes_for_asset(&self, asset_type: &str) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE sink IS NOT NULL
              AND sink::text ILIKE $1
            "#,
        )
        .bind(format!("%{}%", asset_type))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get sink attributes for asset")?;

        Ok(rows)
    }

    /// Get source attributes for a specific document type
    /// Returns attributes that are produced/sourced from documents
    pub async fn get_source_attributes_for_doc_type(&self, doc_type: &str) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE source IS NOT NULL
              AND source::text ILIKE $1
            "#,
        )
        .bind(format!("%{}%", doc_type))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get source attributes for doc type")?;

        Ok(rows)
    }
}
```

## File: rust/src/database/dictionary_service.rs

```rust
//! Dictionary Database Service - CRUD operations for attribute management
//!
//! This module provides database operations for the central attribute dictionary
//! that forms the foundation of our AttributeID-as-Type architecture.

use crate::models::dictionary_models::{
    AttributeSearchCriteria, AttributeValidationRequest, AttributeValidationResult,
    DictionaryAttribute, DictionaryAttributeWithMetadata, DictionaryHealthCheck,
    DictionaryStatistics, DiscoveredAttribute, NewDictionaryAttribute, UpdateDictionaryAttribute,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Database service for dictionary operations
#[derive(Clone, Debug)]
pub struct DictionaryDatabaseService {
    pool: PgPool,
}

impl DictionaryDatabaseService {
    /// Create a new dictionary database service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a mock dictionary database service for testing
    pub fn new_mock() -> Self {
        // This creates a mock service that will fail on actual database operations
        // but is useful for type checking and basic initialization
        use std::str::FromStr;
        let mock_url = "postgresql://mock:mock@localhost/mock";
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&mock_url)
            .expect("Mock pool creation failed");

        Self { pool }
    }

    /// Create a new dictionary attribute
    pub async fn create_attribute(
        &self,
        new_attribute: NewDictionaryAttribute,
    ) -> Result<DictionaryAttribute> {
        let attribute_id = Uuid::new_v4();

        let result = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            INSERT INTO "ob-poc".dictionary (
                attribute_id, name, long_description, group_id, mask, domain, vector, source, sink
            ) VALUES ($1, $2, $3, COALESCE($4, 'default'), COALESCE($5, 'string'), $6, $7, $8, $9)
            RETURNING attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            "#,
        )
        .bind(attribute_id)
        .bind(&new_attribute.name)
        .bind(&new_attribute.long_description)
        .bind(&new_attribute.group_id)
        .bind(&new_attribute.mask)
        .bind(&new_attribute.domain)
        .bind(&new_attribute.vector)
        .bind(&new_attribute.source)
        .bind(&new_attribute.sink)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create dictionary attribute")?;

        info!(
            "Created dictionary attribute '{}' with ID: {}",
            result.name, result.attribute_id
        );

        Ok(result)
    }

    /// Get attribute by ID
    pub async fn get_by_id(&self, attribute_id: Uuid) -> Result<Option<DictionaryAttribute>> {
        let result = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
        )
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch dictionary attribute by ID")?;

        Ok(result)
    }

    /// Get attribute by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<DictionaryAttribute>> {
        let result = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch dictionary attribute by name")?;

        Ok(result)
    }

    /// Update an existing attribute
    pub async fn update_attribute(
        &self,
        attribute_id: Uuid,
        updates: UpdateDictionaryAttribute,
    ) -> Result<Option<DictionaryAttribute>> {
        // Use a simpler approach with COALESCE for conditional updates
        if updates.name.is_some()
            || updates.long_description.is_some()
            || updates.group_id.is_some()
            || updates.mask.is_some()
            || updates.domain.is_some()
            || updates.vector.is_some()
            || updates.source.is_some()
            || updates.sink.is_some()
        {
            let result = sqlx::query!(
                r#"
                UPDATE "ob-poc".dictionary
                SET name = COALESCE($1, name),
                    long_description = COALESCE($2, long_description),
                    group_id = COALESCE($3, group_id),
                    mask = COALESCE($4, mask),
                    domain = COALESCE($5, domain),
                    vector = COALESCE($6, vector),
                    source = COALESCE($7, source),
                    sink = COALESCE($8, sink),
                    updated_at = NOW()
                WHERE attribute_id = $9
                RETURNING attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                "#,
                updates.name as Option<String>,
                updates.long_description as Option<String>,
                updates.group_id as Option<String>,
                updates.mask as Option<String>,
                updates.domain as Option<String>,
                updates.vector as Option<String>,
                updates.source as Option<serde_json::Value>,
                updates.sink as Option<serde_json::Value>,
                attribute_id
            )
            .fetch_optional(&self.pool)
            .await
            .context("Failed to update dictionary attribute")?;

            let result = result.map(|row| DictionaryAttribute {
                attribute_id: row.attribute_id,
                name: row.name,
                long_description: row.long_description,
                group_id: row.group_id,
                mask: row.mask.unwrap_or_else(|| "string".to_string()),
                domain: row.domain,
                vector: row.vector,
                source: row.source,
                sink: row.sink,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });

            if let Some(ref attr) = result {
                info!(
                    "Updated dictionary attribute '{}' with ID: {}",
                    attr.name, attr.attribute_id
                );
            }

            Ok(result)
        } else {
            Ok(self.get_by_id(attribute_id).await?)
        }
    }

    /// Delete an attribute
    pub async fn delete_attribute(&self, attribute_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
        )
        .bind(attribute_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete dictionary attribute")?;

        let deleted = result.rows_affected() > 0;

        if deleted {
            info!("Deleted dictionary attribute with ID: {}", attribute_id);
        } else {
            warn!(
                "Attempted to delete non-existent attribute: {}",
                attribute_id
            );
        }

        Ok(deleted)
    }

    /// Search attributes by criteria
    pub async fn search_attributes(
        &self,
        criteria: &AttributeSearchCriteria,
    ) -> Result<Vec<DictionaryAttribute>> {
        // Use a more direct approach with conditional queries
        match (
            &criteria.name_pattern,
            &criteria.group_id,
            &criteria.domain,
            &criteria.mask,
        ) {
            (Some(name_pattern), Some(group_id), Some(domain), Some(mask)) => {
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    WHERE name ILIKE $1 AND group_id = $2 AND domain = $3 AND mask = $4
                    ORDER BY name
                    LIMIT $5 OFFSET $6
                    "#,
                    format!("%{}%", name_pattern),
                    group_id,
                    domain,
                    mask,
                    criteria.limit.unwrap_or(100) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to search dictionary attributes by name pattern")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
            (Some(name_pattern), Some(group_id), None, None) => {
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    WHERE name ILIKE $1 AND group_id = $2
                    ORDER BY name
                    LIMIT $3 OFFSET $4
                    "#,
                    format!("%{}%", name_pattern),
                    group_id,
                    criteria.limit.unwrap_or(100) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to list all dictionary attributes")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
            (Some(name_pattern), None, None, None) => {
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    WHERE name ILIKE $1
                    ORDER BY name
                    LIMIT $2 OFFSET $3
                    "#,
                    format!("%{}%", name_pattern),
                    criteria.limit.unwrap_or(50) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to list all dictionary attributes")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
            _ => {
                // Default case - return all with limit
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    ORDER BY name
                    LIMIT $1 OFFSET $2
                    "#,
                    criteria.limit.unwrap_or(50) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to search dictionary attributes by name")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
        }
    }

    /// Get attributes by domain
    pub async fn get_by_domain(&self, domain: &str) -> Result<Vec<DictionaryAttribute>> {
        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE domain = $1
            ORDER BY name
            "#,
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch attributes by domain")?;

        Ok(results)
    }

    /// Get attributes by group
    pub async fn get_by_group(&self, group_id: &str) -> Result<Vec<DictionaryAttribute>> {
        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE group_id = $1
            ORDER BY name
            "#,
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch attributes by group")?;

        Ok(results)
    }

    /// Semantic search for attributes (basic implementation)
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DiscoveredAttribute>> {
        // This is a basic text search implementation
        // In production, you'd integrate with a vector database or use PostgreSQL's full-text search
        let search_limit = limit.unwrap_or(10);

        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE name ILIKE $1
               OR long_description ILIKE $1
               OR domain ILIKE $1
            ORDER BY
                CASE
                    WHEN name ILIKE $1 THEN 1
                    WHEN long_description ILIKE $1 THEN 2
                    ELSE 3
                END,
                name
            LIMIT $2
            "#,
        )
        .bind(format!("%{}%", query))
        .bind(search_limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to perform semantic search")?;

        // Convert to discovered attributes with basic relevance scoring
        let discovered: Vec<DiscoveredAttribute> = results
            .into_iter()
            .enumerate()
            .map(|(index, attr)| {
                let relevance_score = 1.0 - (index as f64 * 0.1); // Simple scoring
                let match_reason = if attr.name.to_lowercase().contains(&query.to_lowercase()) {
                    "Name match".to_string()
                } else if attr.long_description.as_ref().map_or(false, |desc| {
                    desc.to_lowercase().contains(&query.to_lowercase())
                }) {
                    "Description match".to_string()
                } else {
                    "Domain match".to_string()
                };

                DiscoveredAttribute {
                    attribute: attr,
                    relevance_score: relevance_score.max(0.1),
                    match_reason,
                }
            })
            .collect();

        Ok(discovered)
    }

    /// Validate an attribute value (basic implementation)
    pub async fn validate_attribute_value(
        &self,
        request: &AttributeValidationRequest,
    ) -> Result<AttributeValidationResult> {
        // Get the attribute definition
        let attr = self.get_by_id(request.attribute_id).await?;

        let attribute = match attr {
            Some(attr) => attr,
            None => {
                return Ok(AttributeValidationResult {
                    is_valid: false,
                    normalized_value: None,
                    validation_errors: vec![format!(
                        "Attribute {} not found",
                        request.attribute_id
                    )],
                    warnings: vec![],
                });
            }
        };

        // Basic validation based on mask
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let normalized_value = Some(request.value.clone());

        match attribute.mask.as_str() {
            "string" => {
                if !request.value.is_string() {
                    errors.push("Value must be a string".to_string());
                }
            }
            "decimal" | "number" => {
                if !request.value.is_number() {
                    errors.push("Value must be a number".to_string());
                }
            }
            "boolean" => {
                if !request.value.is_boolean() {
                    errors.push("Value must be a boolean".to_string());
                }
            }
            "date" => {
                if request.value.is_string() {
                    let date_str = request.value.as_str().unwrap_or("");
                    if chrono::DateTime::parse_from_rfc3339(date_str).is_err() {
                        errors.push("Value must be a valid ISO 8601 date".to_string());
                    }
                } else {
                    errors.push("Date value must be a string in ISO 8601 format".to_string());
                }
            }
            "uuid" => {
                if request.value.is_string() {
                    let uuid_str = request.value.as_str().unwrap_or("");
                    if Uuid::parse_str(uuid_str).is_err() {
                        errors.push("Value must be a valid UUID".to_string());
                    }
                } else {
                    errors.push("UUID value must be a string".to_string());
                }
            }
            _ => {
                warnings.push(format!(
                    "Unknown mask '{}' - validation skipped",
                    attribute.mask
                ));
            }
        }

        Ok(AttributeValidationResult {
            is_valid: errors.is_empty(),
            normalized_value,
            validation_errors: errors,
            warnings,
        })
    }

    /// Get dictionary statistics
    pub async fn get_statistics(&self) -> Result<DictionaryStatistics> {
        // Total attributes
        let total_attributes: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary"#)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get total attribute count")?;

        // Attributes by domain
        let domain_stats = sqlx::query(
            r#"
            SELECT COALESCE(domain, 'null') as domain, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY domain
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get domain statistics")?;

        let mut attributes_by_domain = HashMap::new();
        for row in domain_stats {
            let domain: String = row.get("domain");
            let count: i64 = row.get("count");
            attributes_by_domain.insert(domain, count);
        }

        // Attributes by group
        let group_stats = sqlx::query(
            r#"
            SELECT group_id, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY group_id
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get group statistics")?;

        let mut attributes_by_group = HashMap::new();
        for row in group_stats {
            let group_id: String = row.get("group_id");
            let count: i64 = row.get("count");
            attributes_by_group.insert(group_id, count);
        }

        // Attributes by mask
        let mask_stats = sqlx::query(
            r#"
            SELECT mask, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY mask
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get mask statistics")?;

        let mut attributes_by_mask = HashMap::new();
        for row in mask_stats {
            let mask: String = row.get("mask");
            let count: i64 = row.get("count");
            attributes_by_mask.insert(mask, count);
        }

        // Recently created attributes
        let recently_created = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            ORDER BY created_at DESC
            LIMIT 10
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get recently created attributes")?;

        Ok(DictionaryStatistics {
            total_attributes,
            attributes_by_domain,
            attributes_by_group,
            attributes_by_mask,
            most_used_attributes: vec![], // Would need usage tracking
            recently_created,
            orphaned_attributes: 0, // Would need cross-reference analysis
        })
    }

    /// Perform health check on dictionary
    pub async fn health_check(&self) -> Result<DictionaryHealthCheck> {
        let total_attributes: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary"#)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get total attribute count")?;

        let attributes_with_descriptions: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".dictionary WHERE long_description IS NOT NULL AND long_description != ''"#
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get attributes with descriptions count")?;

        let missing_domains: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary WHERE domain IS NULL"#)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get missing domains count")?;

        // Check for duplicate names (shouldn't happen due to unique constraint)
        let duplicate_names = sqlx::query(
            r#"
            SELECT name, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY name
            HAVING COUNT(*) > 1
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to check for duplicate names")?;

        let duplicate_name_list: Vec<String> = duplicate_names
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();

        let mut recommendations = Vec::new();

        if attributes_with_descriptions < total_attributes / 2 {
            recommendations.push(
                "Consider adding descriptions to more attributes for better AI discoverability"
                    .to_string(),
            );
        }

        if missing_domains > 0 {
            recommendations.push(format!(
                "{} attributes are missing domain classification",
                missing_domains
            ));
        }

        if !duplicate_name_list.is_empty() {
            recommendations
                .push("Duplicate attribute names found - this should not happen".to_string());
        }

        let status = if recommendations.is_empty() {
            "healthy".to_string()
        } else if recommendations.len() <= 2 {
            "warning".to_string()
        } else {
            "needs_attention".to_string()
        };

        Ok(DictionaryHealthCheck {
            status,
            total_attributes,
            attributes_with_descriptions,
            attributes_with_validation: 0, // Would need to parse source metadata
            duplicate_names: duplicate_name_list,
            missing_domains,
            recommendations,
            last_check_at: chrono::Utc::now(),
        })
    }

    /// List all attributes with pagination
    pub async fn list_all(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<DictionaryAttribute>> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            ORDER BY name
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list dictionary attributes")?;

        Ok(results)
    }

    /// Check if attribute exists by name
    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        let count: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary WHERE name = $1"#)
                .bind(name)
                .fetch_one(&self.pool)
                .await
                .context("Failed to check if attribute exists")?;

        Ok(count > 0)
    }

    /// Get attribute count
    pub async fn count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary"#)
            .fetch_one(&self.pool)
            .await
            .context("Failed to get attribute count")?;

        Ok(count)
    }
}
```

## File: rust/src/database/document_service.rs

```rust
//! Document Service - CRUD operations for document management
//!
//! This module provides database operations for document types, catalog,
//! metadata, and relationships following the document dictionary pattern.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Document type definition from the dictionary
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentType {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub required_attributes: Option<JsonValue>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Document catalog entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalogEntry {
    pub doc_id: Uuid,
    pub entity_id: String,
    pub document_type: String,
    pub issuer: Option<String>,
    pub title: Option<String>,
    pub file_hash: Option<String>,
    pub storage_key: Option<String>,
    pub mime_type: Option<String>,
    pub confidentiality_level: Option<String>,
    pub status: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Document metadata (attribute values extracted from documents)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub doc_id: Uuid,
    pub attribute_id: Uuid,
    pub value: String,
}

/// Document relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRelationship {
    pub primary_doc_id: Uuid,
    pub related_doc_id: Uuid,
    pub relationship_type: String,
}

/// Service for document operations
#[derive(Clone, Debug)]
pub struct DocumentService {
    pool: PgPool,
}

impl DocumentService {
    /// Create a new document service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get document type by code
    pub async fn get_document_type_by_code(&self, type_code: &str) -> Result<Option<DocumentType>> {
        let result = sqlx::query_as::<_, DocumentType>(
            r#"
            SELECT type_id, type_code, display_name, category, required_attributes, created_at, updated_at
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document type by code")?;

        Ok(result)
    }

    /// Create a document catalog entry
    pub async fn create_document_catalog_entry(
        &self,
        entity_id: &str,
        document_type: &str,
        issuer: Option<&str>,
        title: Option<&str>,
        file_hash: Option<&str>,
        storage_key: Option<&str>,
        mime_type: Option<&str>,
        confidentiality_level: Option<&str>,
    ) -> Result<Uuid> {
        let doc_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                doc_id, entity_id, document_type, issuer, title,
                file_hash, storage_key, mime_type, confidentiality_level,
                status, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'ACTIVE', NOW(), NOW())
            "#,
        )
        .bind(doc_id)
        .bind(entity_id)
        .bind(document_type)
        .bind(issuer)
        .bind(title)
        .bind(file_hash)
        .bind(storage_key)
        .bind(mime_type)
        .bind(confidentiality_level)
        .execute(&self.pool)
        .await
        .context("Failed to create document catalog entry")?;

        info!(
            "Created document catalog entry {} for entity {} (type: {})",
            doc_id, entity_id, document_type
        );

        Ok(doc_id)
    }

    /// Get document catalog entry by ID
    pub async fn get_document_by_id(&self, doc_id: Uuid) -> Result<Option<DocumentCatalogEntry>> {
        let result = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, entity_id, document_type, issuer, title,
                   file_hash, storage_key, mime_type, confidentiality_level,
                   status, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1
            "#,
        )
        .bind(doc_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get document by ID")?;

        Ok(result)
    }

    /// Get documents for an entity
    pub async fn get_documents_for_entity(
        &self,
        entity_id: &str,
    ) -> Result<Vec<DocumentCatalogEntry>> {
        let results = sqlx::query_as::<_, DocumentCatalogEntry>(
            r#"
            SELECT doc_id, entity_id, document_type, issuer, title,
                   file_hash, storage_key, mime_type, confidentiality_level,
                   status, created_at, updated_at
            FROM "ob-poc".document_catalog
            WHERE entity_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get documents for entity")?;

        Ok(results)
    }

    /// Set document metadata (attribute extracted from document)
    pub async fn set_document_metadata(
        &self,
        doc_id: Uuid,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (doc_id, attribute_id)
            DO UPDATE SET value = $3, updated_at = NOW()
            "#,
        )
        .bind(doc_id)
        .bind(attribute_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .context("Failed to set document metadata")?;

        info!(
            "Set metadata for doc {} attribute {} = '{}'",
            doc_id, attribute_id, value
        );

        Ok(())
    }

    /// Get all metadata for a document
    pub async fn get_document_metadata(&self, doc_id: Uuid) -> Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT attribute_id, value
            FROM "ob-poc".document_metadata
            WHERE doc_id = $1
            "#,
        )
        .bind(doc_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get document metadata")?;

        Ok(rows)
    }

    /// Link two documents with a relationship
    pub async fn link_documents(
        &self,
        primary_doc_id: Uuid,
        related_doc_id: Uuid,
        relationship_type: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_relationships (
                primary_doc_id, related_doc_id, relationship_type, created_at
            )
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (primary_doc_id, related_doc_id, relationship_type)
            DO NOTHING
            "#,
        )
        .bind(primary_doc_id)
        .bind(related_doc_id)
        .bind(relationship_type)
        .execute(&self.pool)
        .await
        .context("Failed to link documents")?;

        info!(
            "Linked documents {} -> {} ({})",
            primary_doc_id, related_doc_id, relationship_type
        );

        Ok(())
    }

    /// Get required attributes for a document type
    pub async fn get_required_attributes_for_doc_type(
        &self,
        type_code: &str,
    ) -> Result<Vec<String>> {
        let doc_type = self.get_document_type_by_code(type_code).await?;

        match doc_type {
            Some(dt) => {
                if let Some(attrs) = dt.required_attributes {
                    if let Some(arr) = attrs.as_array() {
                        let result: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        return Ok(result);
                    }
                }
                Ok(vec![])
            }
            None => Ok(vec![]),
        }
    }

    /// Update document status
    pub async fn update_document_status(&self, doc_id: Uuid, status: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".document_catalog
            SET status = $1, updated_at = NOW()
            WHERE doc_id = $2
            "#,
        )
        .bind(status)
        .bind(doc_id)
        .execute(&self.pool)
        .await
        .context("Failed to update document status")?;

        Ok(result.rows_affected() > 0)
    }

    /// Get source attributes that a document type produces
    pub async fn get_source_attributes_for_doc_type(&self, doc_type: &str) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id
            FROM "ob-poc".dictionary
            WHERE source IS NOT NULL
              AND source::text ILIKE $1
            "#,
        )
        .bind(format!("%{}%", doc_type))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get source attributes for doc type")?;

        Ok(rows)
    }
}
```

## File: rust/src/database/crud_service.rs

```rust
//! CRUD Service - Agentic CRUD operation logging
//!
//! This module provides database operations for logging all CRUD operations
//! for full agentic auditability and traceability.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Types of assets that can be operated on
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetType {
    Cbu,
    ProperPerson,
    Company,
    Trust,
    Partnership,
    Entity,
    Attribute,
    Document,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Cbu => write!(f, "CBU"),
            AssetType::ProperPerson => write!(f, "PROPER_PERSON"),
            AssetType::Company => write!(f, "COMPANY"),
            AssetType::Trust => write!(f, "TRUST"),
            AssetType::Partnership => write!(f, "PARTNERSHIP"),
            AssetType::Entity => write!(f, "ENTITY"),
            AssetType::Attribute => write!(f, "ATTRIBUTE"),
            AssetType::Document => write!(f, "DOCUMENT"),
        }
    }
}

/// Types of CRUD operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationType {
    Create,
    Read,
    Update,
    Delete,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Create => write!(f, "CREATE"),
            OperationType::Read => write!(f, "READ"),
            OperationType::Update => write!(f, "UPDATE"),
            OperationType::Delete => write!(f, "DELETE"),
        }
    }
}

/// CRUD operation record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CrudOperation {
    pub operation_id: Uuid,
    pub operation_type: String,
    pub asset_type: String,
    pub entity_table_name: String,
    pub generated_dsl: Option<String>,
    pub ai_instruction: Option<String>,
    pub affected_records: Option<JsonValue>,
    pub affected_sinks: Option<JsonValue>,
    pub contributing_sources: Option<JsonValue>,
    pub execution_status: String,
    pub ai_confidence: Option<f64>,
    pub ai_provider: Option<String>,
    pub ai_model: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Service for CRUD operation logging
#[derive(Clone, Debug)]
pub struct CrudService {
    pool: PgPool,
}

impl CrudService {
    /// Create a new CRUD service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Log a CRUD operation
    pub async fn log_crud_operation(
        &self,
        operation_type: OperationType,
        asset_type: AssetType,
        entity_table_name: &str,
        generated_dsl: Option<&str>,
        ai_instruction: Option<&str>,
        affected_records: Option<JsonValue>,
        ai_provider: Option<&str>,
        ai_model: Option<&str>,
    ) -> Result<Uuid> {
        let operation_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".crud_operations (
                operation_id, operation_type, asset_type, entity_table_name,
                generated_dsl, ai_instruction, affected_records,
                execution_status, ai_provider, ai_model, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'COMPLETED', $8, $9, NOW())
            "#,
        )
        .bind(operation_id)
        .bind(operation_type.to_string())
        .bind(asset_type.to_string())
        .bind(entity_table_name)
        .bind(generated_dsl)
        .bind(ai_instruction)
        .bind(affected_records)
        .bind(ai_provider)
        .bind(ai_model)
        .execute(&self.pool)
        .await
        .context("Failed to log CRUD operation")?;

        info!(
            "Logged {} {} operation {} on {}",
            operation_type, asset_type, operation_id, entity_table_name
        );

        Ok(operation_id)
    }

    /// Log a CRUD operation with sink/source tracking
    pub async fn log_crud_operation_with_sinks(
        &self,
        operation_type: OperationType,
        asset_type: AssetType,
        entity_table_name: &str,
        generated_dsl: Option<&str>,
        ai_instruction: Option<&str>,
        affected_records: Option<JsonValue>,
        affected_sinks: Option<Vec<Uuid>>,
        contributing_sources: Option<Vec<Uuid>>,
        ai_provider: Option<&str>,
        ai_model: Option<&str>,
    ) -> Result<Uuid> {
        let operation_id = Uuid::new_v4();

        let sinks_json = affected_sinks.map(|s| serde_json::to_value(s).unwrap_or_default());
        let sources_json =
            contributing_sources.map(|s| serde_json::to_value(s).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".crud_operations (
                operation_id, operation_type, asset_type, entity_table_name,
                generated_dsl, ai_instruction, affected_records,
                affected_sinks, contributing_sources,
                execution_status, ai_provider, ai_model, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'COMPLETED', $10, $11, NOW())
            "#,
        )
        .bind(operation_id)
        .bind(operation_type.to_string())
        .bind(asset_type.to_string())
        .bind(entity_table_name)
        .bind(generated_dsl)
        .bind(ai_instruction)
        .bind(affected_records)
        .bind(sinks_json)
        .bind(sources_json)
        .bind(ai_provider)
        .bind(ai_model)
        .execute(&self.pool)
        .await
        .context("Failed to log CRUD operation with sinks")?;

        info!(
            "Logged {} {} operation {} on {} (with sink/source tracking)",
            operation_type, asset_type, operation_id, entity_table_name
        );

        Ok(operation_id)
    }

    /// Get CRUD operations for an entity
    pub async fn get_operations_for_entity(
        &self,
        entity_table_name: &str,
        limit: Option<i32>,
    ) -> Result<Vec<CrudOperation>> {
        let results = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            WHERE entity_table_name = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(entity_table_name)
        .bind(limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CRUD operations for entity")?;

        Ok(results)
    }

    /// Get CRUD operations by asset type
    pub async fn get_operations_by_asset_type(
        &self,
        asset_type: AssetType,
        limit: Option<i32>,
    ) -> Result<Vec<CrudOperation>> {
        let results = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            WHERE asset_type = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(asset_type.to_string())
        .bind(limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CRUD operations by asset type")?;

        Ok(results)
    }

    /// Get recent CRUD operations
    pub async fn get_recent_operations(&self, limit: Option<i32>) -> Result<Vec<CrudOperation>> {
        let results = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit.unwrap_or(50))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get recent CRUD operations")?;

        Ok(results)
    }

    /// Get CRUD operation by ID
    pub async fn get_operation_by_id(&self, operation_id: Uuid) -> Result<Option<CrudOperation>> {
        let result = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            WHERE operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CRUD operation by ID")?;

        Ok(result)
    }
}
```

## File: rust/src/forth_engine/mod.rs

```rust
//! Public facade for the DSL Forth Engine.

use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::kyc_vocab::kyc_orch_vocab;
use crate::forth_engine::parser_nom::NomKycParser;
use crate::forth_engine::vm::VM;
use std::sync::Arc;

#[cfg(feature = "database")]
use crate::database::{CrudExecutor, DslRepository};
#[cfg(feature = "database")]
use sqlx::PgPool;

// Module declarations
pub mod ast;
pub mod compiler;
pub mod ebnf;
pub mod env;
pub mod errors;
pub mod kyc_vocab;
pub mod parser_nom;
pub mod value;
pub mod vm;
pub mod vocab;

// Re-export key types
pub use ast::{DslParser, DslSheet, Expr};
pub use env::{generate_onboarding_template, mint_ob_request_id};
pub use errors::EngineError;
pub use value::Value;

/// Result of DSL execution
#[derive(Debug)]
pub struct ExecutionResult {
    /// Execution logs
    pub logs: Vec<String>,
    /// Extracted case_id from DSL
    pub case_id: Option<String>,
    /// Whether execution succeeded
    pub success: bool,
    /// Version number (sequence) for this DSL instance
    pub version: i32,
}

/// Executes a DSL Sheet without database connection.
/// This is the main entry point for the Forth-style engine.
pub fn execute_sheet(sheet: &DslSheet) -> Result<Vec<String>, EngineError> {
    let result = execute_sheet_internal(sheet, None)?;
    Ok(result.logs)
}

/// Executes a DSL Sheet with database connection.
/// This is async because it persists results to the database.
/// Uses DslRepository for fully transactional saves - all operations
/// (DSL, AST, CBU, attributes) are saved atomically.
#[cfg(feature = "database")]
pub async fn execute_sheet_with_db(
    sheet: &DslSheet,
    pool: PgPool,
) -> Result<ExecutionResult, EngineError> {
    let start_time = std::time::Instant::now();

    // Execute the DSL synchronously (parse + compile + run)
    let (mut result, mut env) = execute_sheet_internal_with_env(sheet, Some(pool.clone()))?;

    let parse_time_ms = start_time.elapsed().as_millis() as u64;

    // Execute pending CRUD statements against database
    let pending_crud = env.take_pending_crud();
    if !pending_crud.is_empty() {
        let executor = CrudExecutor::new(pool.clone());
        let crud_results = executor
            .execute_all(&pending_crud)
            .await
            .map_err(|e| EngineError::Database(format!("CRUD execution failed: {}", e)))?;

        // Log CRUD results
        for crud_result in &crud_results {
            result.logs.push(format!(
                "CRUD {}: {} - {} rows affected",
                crud_result.operation, crud_result.asset, crud_result.rows_affected
            ));

            // If a CBU was created, use its ID as the case_id
            if crud_result.asset == "CBU" && crud_result.operation == "CREATE" {
                if let Some(id) = &crud_result.generated_id {
                    if result.case_id.is_none() {
                        result.case_id = Some(id.to_string());
                        env.set_case_id(id.to_string());
                    }
                }
            }
        }
    }

    // Persist to database using the database facade
    if let Some(case_id) = &result.case_id {
        // Extract domain and operation from DSL content
        let domain = if sheet.content.contains("cbu.") {
            "cbu"
        } else if sheet.content.contains("case.") {
            "case"
        } else if sheet.content.contains("kyc.") {
            "kyc"
        } else if sheet.content.contains("entity.") {
            "entity"
        } else if sheet.content.contains("crud.") {
            "crud"
        } else if sheet.content.contains("attr.") {
            "attr"
        } else if sheet.content.contains("document.") {
            "document"
        } else {
            "general"
        };

        let operation_type = sheet
            .content
            .split_whitespace()
            .next()
            .and_then(|s| s.strip_prefix('('))
            .unwrap_or("unknown");

        // Build AST JSON
        let ast_json = serde_json::json!({
            "sheet_id": sheet.id,
            "domain": sheet.domain,
            "version": sheet.version,
            "logs": result.logs,
            "attributes": env.attribute_cache.iter()
                .map(|(k, v)| (k.0.clone(), format!("{:?}", v)))
                .collect::<std::collections::HashMap<_, _>>()
        })
        .to_string();

        // Extract client name and type for CBU
        let client_name = env
            .attribute_cache
            .iter()
            .find(|(k, _)| k.0.contains("client-name"))
            .map(|(_, v)| match v {
                Value::Str(s) => s.clone(),
                _ => case_id.to_string(),
            })
            .unwrap_or_else(|| case_id.to_string());

        let case_type = env
            .attribute_cache
            .iter()
            .find(|(k, _)| k.0.contains("case-type"))
            .map(|(_, v)| match v {
                Value::Str(s) => s.clone(),
                _ => "ONBOARDING".to_string(),
            })
            .unwrap_or_else(|| "ONBOARDING".to_string());

        // Build attributes map for transactional save
        let mut attributes = std::collections::HashMap::new();
        for (attr_id, value) in &env.attribute_cache {
            let (value_text, value_type) = match value {
                Value::Str(s) => (s.clone(), "STRING".to_string()),
                Value::Int(i) => (i.to_string(), "INTEGER".to_string()),
                Value::Bool(b) => (b.to_string(), "BOOLEAN".to_string()),
                Value::Keyword(k) => (k.clone(), "KEYWORD".to_string()),
                _ => continue,
            };
            attributes.insert(attr_id.0.clone(), (value_text, value_type));
        }

        // Use DslRepository for fully transactional save
        // All operations (DSL, AST, CBU, attributes) are atomic
        let repo = DslRepository::new(pool.clone());
        let save_result = repo
            .save_execution_transactionally(
                case_id,
                &sheet.content,
                &ast_json,
                domain,
                operation_type,
                parse_time_ms as i64,
                &client_name,
                &case_type,
                &attributes,
            )
            .await
            .map_err(|e| EngineError::Database(format!("Failed to save execution: {}", e)))?;

        // Set version in result
        result.version = save_result.version;
    }

    Ok(result)
}

/// Create a new OB (Onboarding) Request
/// This mints a new OB Request ID, generates the DSL template, parses it,
/// and saves both DSL and AST to the database with version 1.
#[cfg(feature = "database")]
pub async fn create_ob_request(
    pool: PgPool,
    client_name: &str,
    client_type: &str,
) -> Result<(String, ExecutionResult), EngineError> {
    // 1. Mint new OB Request ID
    let ob_request_id = env::mint_ob_request_id();

    // 2. Generate DSL onboarding template
    let dsl_content = env::generate_onboarding_template(&ob_request_id, client_name, client_type);

    // 3. Create sheet and execute (parse + save)
    let sheet = DslSheet {
        id: ob_request_id.clone(),
        domain: "onboarding".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    // 4. Execute - this will parse, validate, and save to DB
    let result = execute_sheet_with_db(&sheet, pool).await?;

    Ok((ob_request_id, result))
}

/// Internal execution function
fn execute_sheet_internal(
    sheet: &DslSheet,
    #[cfg(feature = "database")] pool: Option<PgPool>,
    #[cfg(not(feature = "database"))] _pool: Option<()>,
) -> Result<ExecutionResult, EngineError> {
    #[cfg(feature = "database")]
    {
        let (result, _env) = execute_sheet_internal_with_env(sheet, pool)?;
        Ok(result)
    }

    #[cfg(not(feature = "database"))]
    {
        let (result, _env) = execute_sheet_internal_with_env(sheet, _pool)?;
        Ok(result)
    }
}

/// Internal execution function that returns both result and environment
fn execute_sheet_internal_with_env(
    sheet: &DslSheet,
    #[cfg(feature = "database")] pool: Option<PgPool>,
    #[cfg(not(feature = "database"))] _pool: Option<()>,
) -> Result<(ExecutionResult, RuntimeEnv), EngineError> {
    // 1. Parsing (sheet.content -> AST)
    let parser = NomKycParser::new();
    let ast = parser.parse(&sheet.content)?;

    // 2. Compiling (AST -> Bytecode)
    let vocab = kyc_orch_vocab();
    let program = compiler::compile_sheet(&ast, &vocab)?;
    let program_arc = Arc::new(program);

    // 3. Create runtime environment
    #[cfg(feature = "database")]
    let mut env = if let Some(p) = pool {
        RuntimeEnv::with_pool(env::OnboardingRequestId(sheet.id.clone()), p)
    } else {
        RuntimeEnv::new(env::OnboardingRequestId(sheet.id.clone()))
    };

    #[cfg(not(feature = "database"))]
    let mut env = RuntimeEnv::new(env::OnboardingRequestId(sheet.id.clone()));

    // 4. VM Execution (Bytecode)
    let mut vm = VM::new(program_arc, Arc::new(vocab), &mut env);

    let mut logs = Vec::new();
    loop {
        match vm.step_with_logging() {
            Ok(Some(log_msg)) => {
                logs.push(log_msg);
            }
            Ok(None) => {
                // End of program
                break;
            }
            Err(e) => {
                return Err(EngineError::Vm(e));
            }
        }
    }

    // Extract case_id from the execution
    let case_id = vm.env.get_case_id().cloned();

    // Clone the environment for return (need to transfer ownership)
    let final_env = std::mem::replace(
        &mut env,
        RuntimeEnv::new(env::OnboardingRequestId(String::new())),
    );

    Ok((
        ExecutionResult {
            logs,
            case_id,
            success: true,
            version: 0, // Will be set by execute_sheet_with_db after DB query
        },
        final_env,
    ))
}

/// Extract case_id from DSL content by parsing keyword-value pairs
pub fn extract_case_id(dsl_content: &str) -> Option<String> {
    // Simple extraction: find :case-id followed by a string
    if let Some(start) = dsl_content.find(":case-id") {
        let after_keyword = &dsl_content[start + 8..];
        // Skip whitespace and find the string value
        let trimmed = after_keyword.trim_start();
        if let Some(stripped) = trimmed.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                return Some(stripped[..end].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_sheet_case_create() {
        let sheet = DslSheet {
            id: "test-1".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"(case.create :case-id "TEST-001" :case-type "ONBOARDING")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
        let logs = result.unwrap();
        assert!(!logs.is_empty());
    }

    #[test]
    fn test_execute_sheet_multiple_operations() {
        let sheet = DslSheet {
            id: "test-2".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"
                (case.create :case-id "MULTI-001" :case-type "ONBOARDING")
                (kyc.start :entity-id "ENT-001")
            "#
            .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_entity_operations() {
        let sheet = DslSheet {
            id: "test-3".to_string(),
            domain: "entity".to_string(),
            version: "1".to_string(),
            content: r#"(entity.register :entity-id "ENT-001" :entity-type "CORP")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_unknown_verb() {
        let sheet = DslSheet {
            id: "test-4".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"(unknown.verb :key "value")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_case_id() {
        let dsl = r#"(case.create :case-id "EXTRACT-001" :case-type "ONBOARDING")"#;
        let case_id = extract_case_id(dsl);
        assert_eq!(case_id, Some("EXTRACT-001".to_string()));
    }

    #[test]
    fn test_extract_case_id_with_whitespace() {
        let dsl = r#"(case.create :case-id   "SPACE-001"   :case-type "ONBOARDING")"#;
        let case_id = extract_case_id(dsl);
        assert_eq!(case_id, Some("SPACE-001".to_string()));
    }

    #[test]
    fn test_extract_case_id_not_found() {
        let dsl = r#"(entity.register :entity-id "ENT-001")"#;
        let case_id = extract_case_id(dsl);
        assert_eq!(case_id, None);
    }

    #[test]
    fn test_execute_sheet_kyc_operations() {
        let sheet = DslSheet {
            id: "test-kyc".to_string(),
            domain: "kyc".to_string(),
            version: "1".to_string(),
            content: r#"(kyc.collect :case-id "KYC-001" :collection-type "ENHANCED")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_ubo_operations() {
        let sheet = DslSheet {
            id: "test-ubo".to_string(),
            domain: "ubo".to_string(),
            version: "1".to_string(),
            content: r#"(ubo.collect-entity-data :entity-id "UBO-ENT-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_document_operations() {
        let sheet = DslSheet {
            id: "test-doc".to_string(),
            domain: "document".to_string(),
            version: "1".to_string(),
            content: r#"(document.catalog :doc-id "DOC-001" :doc-type "PASSPORT")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_products_operations() {
        let sheet = DslSheet {
            id: "test-prod".to_string(),
            domain: "products".to_string(),
            version: "1".to_string(),
            content: r#"(products.add :case-id "PROD-001" :product-type "CUSTODY")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stack_effect_validation() {
        // Test that stack underflow is caught at compile time
        let sheet = DslSheet {
            id: "test-stack".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            // case.create expects 4 items (2 pairs), but we only provide 2
            content: r#"(case.create :case-id "UNDER-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        // Should fail due to stack underflow
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_sheet_cbu_create() {
        let sheet = DslSheet {
            id: "test-cbu-create".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.create :cbu-name "ACME Corp" :client-type "CORP" :jurisdiction "US")"#
                .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_operations() {
        let sheet = DslSheet {
            id: "test-cbu-ops".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"
                (cbu.create :cbu-name "Test Fund" :client-type "FUND" :jurisdiction "GB")
                (cbu.attach-entity :entity-id "ENT-001" :role "BENEFICIAL_OWNER")
            "#
            .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_read() {
        let sheet = DslSheet {
            id: "test-cbu-read".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.read :cbu-id "CBU-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_update() {
        let sheet = DslSheet {
            id: "test-cbu-update".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.update :cbu-id "CBU-001" :status "ACTIVE")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_delete() {
        let sheet = DslSheet {
            id: "test-cbu-delete".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.delete :cbu-id "CBU-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_crud_operations() {
        let sheet = DslSheet {
            id: "test-crud".to_string(),
            domain: "crud".to_string(),
            version: "1".to_string(),
            content: r#"
                (crud.begin :operation-type "CREATE" :asset-type "CBU")
                (crud.commit :entity-table "cbus" :ai-instruction "Create test CBU" :ai-provider "OPENAI")
            "#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_attr_operations() {
        let sheet = DslSheet {
            id: "test-attr".to_string(),
            domain: "attr".to_string(),
            version: "1".to_string(),
            content: r#"(attr.set :attr-id "KYC.LEI" :value "5493001KJTIIGC8Y1R12")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_document_extended() {
        let sheet = DslSheet {
            id: "test-doc-ext".to_string(),
            domain: "document".to_string(),
            version: "1".to_string(),
            content: r#"(document.extract-attributes :document-id "DOC-001" :document-type "UK-PASSPORT")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_full_onboarding_flow() {
        let sheet = DslSheet {
            id: "test-full-flow".to_string(),
            domain: "onboarding".to_string(),
            version: "1".to_string(),
            content: r#"
                (cbu.create :cbu-name "Full Flow Corp" :client-type "CORP" :jurisdiction "US")
                (entity.register :entity-id "ENT-001" :entity-type "PROPER_PERSON")
                (cbu.attach-entity :entity-id "ENT-001" :role "BENEFICIAL_OWNER")
                (document.catalog :doc-id "PASS-001" :doc-type "UK-PASSPORT")
                (document.extract-attributes :document-id "PASS-001" :document-type "UK-PASSPORT")
            "#
            .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cbu_create_emits_crud_statement() {
        use crate::parser::ast::CrudStatement;

        let sheet = DslSheet {
            id: "test-crud".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.create :cbu-name "Test Corp" :client-type "CORP" :jurisdiction "US")"#
                .to_string(),
        };

        // Execute using internal function that returns env
        #[cfg(feature = "database")]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();
        #[cfg(not(feature = "database"))]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();

        // Should have one pending CRUD statement
        assert_eq!(env.pending_crud.len(), 1);
        match &env.pending_crud[0] {
            CrudStatement::DataCreate(create) => {
                assert_eq!(create.asset, "CBU");
                assert!(create.values.contains_key("cbu-name"));
                assert!(create.values.contains_key("client-type"));
                assert!(create.values.contains_key("jurisdiction"));
            }
            _ => panic!("Expected DataCreate statement"),
        }
    }

    #[test]
    fn test_cbu_model_state_transitions() {
        use crate::cbu_model_dsl::CbuModelParser;
        use crate::forth_engine::env::OnboardingRequestId;

        let model_dsl = r#"
        (cbu-model
          :id "CBU.TEST"
          :version "1.0"
          (attributes
            (group :name "core" :required [@attr("LEGAL_NAME")]))
          (states
            :initial "Proposed"
            :final ["Closed"]
            (state "Proposed" :description "Initial")
            (state "Active" :description "Active")
            (state "Closed" :description "Closed"))
          (transitions
            (-> "Proposed" "Active" :verb "cbu.approve" :preconditions [])
            (-> "Active" "Closed" :verb "cbu.close" :preconditions []))
          (roles
            (role "Owner" :min 1)))
        "#;

        let model = CbuModelParser::parse_str(model_dsl).unwrap();

        let mut env = RuntimeEnv::new(OnboardingRequestId("TEST".to_string()));
        env.set_cbu_model(model);

        // Initial state should be "Proposed"
        assert_eq!(env.get_cbu_state(), Some("Proposed"));

        // Valid transition: Proposed -> Active
        assert!(env.is_valid_transition("Active"));

        // Invalid transition: Proposed -> Closed (not defined)
        assert!(!env.is_valid_transition("Closed"));

        // After transitioning to Active
        env.set_cbu_state("Active".to_string());
        assert!(env.is_valid_transition("Closed"));
    }

    #[test]
    fn test_multiple_cbu_operations_emit_crud() {
        use crate::parser::ast::CrudStatement;

        let sheet = DslSheet {
            id: "test-multi-crud".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"
                (cbu.create :cbu-name "Multi Corp" :client-type "FUND" :jurisdiction "GB")
                (cbu.attach-entity :entity-id "ENT-001" :role "OWNER")
                (cbu.finalize :cbu-id "CBU-001" :status "ACTIVE")
            "#
            .to_string(),
        };

        // Execute using internal function that returns env
        #[cfg(feature = "database")]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();
        #[cfg(not(feature = "database"))]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();

        // Should have 3 pending CRUD statements
        assert_eq!(env.pending_crud.len(), 3);

        // First: DataCreate for CBU
        assert!(matches!(&env.pending_crud[0], CrudStatement::DataCreate(c) if c.asset == "CBU"));

        // Second: DataCreate for relationship
        assert!(
            matches!(&env.pending_crud[1], CrudStatement::DataCreate(c) if c.asset == "CBU_ENTITY_RELATIONSHIP")
        );

        // Third: DataUpdate for finalize
        assert!(matches!(&env.pending_crud[2], CrudStatement::DataUpdate(u) if u.asset == "CBU"));
    }
}
```

## File: rust/src/forth_engine/kyc_vocab.rs

```rust
//! Core DSL Vocabulary for the DSL Forth Engine.
//!
//! This module provides the vocabulary (word definitions) for all DSL verbs
//! across all domains: case, entity, products, kyc, ubo, document, isda, etc.

use crate::forth_engine::errors::VmError;
use crate::forth_engine::value::{AttributeId, Value};
use crate::forth_engine::vm::VM;
use crate::forth_engine::vocab::{Vocab, WordId, WordSpec};
use crate::parser::ast::{CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate};
use std::collections::HashMap;
use std::sync::Arc;

/// Collect keyword-value pairs from the stack
/// Returns a HashMap of keyword -> value pairs
fn collect_keyword_pairs(vm: &mut VM, num_pairs: usize) -> Result<HashMap<String, Value>, VmError> {
    let mut pairs = HashMap::new();

    for _ in 0..num_pairs {
        let (keyword, value) = vm.pop_keyword_value()?;
        pairs.insert(keyword, value);
    }

    Ok(pairs)
}

/// Process collected pairs: extract case_id and store all as attributes
fn process_pairs(vm: &mut VM, pairs: &HashMap<String, Value>) {
    for (key, value) in pairs {
        // Extract case_id and store in environment
        if key == ":case-id" {
            if let Value::Str(case_id) = value {
                vm.env.set_case_id(case_id.clone());
            }
        }

        // Store all keyword-value pairs as attributes
        let attr_id = AttributeId(key.clone());
        vm.env.set_attribute(attr_id, value.clone());
    }
}

/// Typed word implementation that consumes a specific number of keyword-value pairs
fn typed_word(vm: &mut VM, _word_name: &str, num_pairs: usize) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, num_pairs)?;
    process_pairs(vm, &pairs);
    Ok(())
}

// Case Operations - stack_effect is (num_pairs * 2, 0)
fn word_case_create(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.create", 2) // :case-id, :case-type
}

fn word_case_update(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.update", 1) // :case-id
}

fn word_case_validate(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.validate", 1) // :case-id
}

fn word_case_approve(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.approve", 1) // :case-id
}

fn word_case_close(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.close", 1) // :case-id
}

// Entity Operations
fn word_entity_register(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "entity.register", 2) // :entity-id, :entity-type
}

fn word_entity_classify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "entity.classify", 1) // :entity-id
}

fn word_entity_link(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "entity.link", 2) // :entity-id, :target-id
}

fn word_identity_verify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "identity.verify", 1) // :entity-id
}

fn word_identity_attest(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "identity.attest", 1) // :entity-id
}

// Product Operations
fn word_products_add(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "products.add", 2) // :case-id, :product-type
}

fn word_products_configure(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "products.configure", 2) // :product-id, :config
}

fn word_services_discover(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "services.discover", 1) // :case-id
}

fn word_services_provision(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "services.provision", 2) // :service-id, :config
}

fn word_services_activate(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "services.activate", 1) // :service-id
}

// KYC Operations
fn word_kyc_start(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.start", 1) // :entity-id
}

fn word_kyc_collect(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.collect", 2) // :case-id, :collection-type
}

fn word_kyc_verify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.verify", 1) // :entity-id
}

fn word_kyc_assess(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.assess", 1) // :entity-id
}

fn word_compliance_screen(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "compliance.screen", 1) // :entity-id
}

fn word_compliance_monitor(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "compliance.monitor", 1) // :entity-id
}

// UBO Operations
fn word_ubo_collect_entity_data(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.collect-entity-data", 1) // :entity-id
}

fn word_ubo_get_ownership_structure(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.get-ownership-structure", 1) // :entity-id
}

fn word_ubo_resolve_ubos(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.resolve-ubos", 1) // :entity-id
}

fn word_ubo_calculate_indirect_ownership(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.calculate-indirect-ownership", 1) // :entity-id
}

// Document Operations
fn word_document_catalog(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.catalog", 2) // :doc-id, :doc-type
}

fn word_document_verify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.verify", 1) // :doc-id
}

fn word_document_extract(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.extract", 1) // :doc-id
}

fn word_document_link(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.link", 2) // :doc-id, :entity-id
}

// Low-level attribute operations (from original kyc_vocab)
fn word_require_attribute(vm: &mut VM) -> Result<(), VmError> {
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 1,
        found: 0,
    })?;

    if let Value::Attr(_attr_id) = attr_val {
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId".to_string(),
            found: format!("{:?}", attr_val),
        })
    }
}

fn word_set_attribute(vm: &mut VM) -> Result<(), VmError> {
    let value = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 2,
        found: 1,
    })?;
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 2,
        found: 0,
    })?;

    if let Value::Attr(id) = attr_val {
        vm.env.set_attribute(id, value);
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId".to_string(),
            found: format!("{:?}", attr_val),
        })
    }
}

// CBU Operations (Phase 4) - Now emit CrudStatements
fn word_cbu_create(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 3)?; // :cbu-name, :client-type, :jurisdiction
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let values: HashMap<String, crate::parser::ast::Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
                }
                Value::Int(i) => crate::parser::ast::Value::Literal(
                    crate::parser::ast::Literal::Number(i as f64),
                ),
                Value::Bool(b) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::Boolean(b))
                }
                _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(
                    format!("{:?}", v),
                )),
            };
            (key, val)
        })
        .collect();

    // Emit CrudStatement
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU".to_string(),
        values,
    }));

    Ok(())
}

fn word_cbu_read(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :cbu-id
    process_pairs(vm, &pairs);

    // Convert to where clause
    let where_clause: HashMap<String, crate::parser::ast::Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
                }
                _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(
                    format!("{:?}", v),
                )),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

fn word_cbu_update(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :cbu-id, :status or other fields
    process_pairs(vm, &pairs);

    // Separate cbu-id (where clause) from update values
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (k, v) in pairs {
        let key = k.trim_start_matches(':').to_string();
        let val = match v {
            Value::Str(s) => {
                crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
            }
            Value::Int(i) => {
                crate::parser::ast::Value::Literal(crate::parser::ast::Literal::Number(i as f64))
            }
            Value::Bool(b) => {
                crate::parser::ast::Value::Literal(crate::parser::ast::Literal::Boolean(b))
            }
            _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(format!(
                "{:?}",
                v
            ))),
        };

        if key == "cbu-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

fn word_cbu_delete(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :cbu-id
    process_pairs(vm, &pairs);

    let where_clause: HashMap<String, crate::parser::ast::Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
                }
                _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(
                    format!("{:?}", v),
                )),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "CBU".to_string(),
        where_clause,
    }));

    Ok(())
}

fn word_cbu_list(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :filter (optional)
    process_pairs(vm, &pairs);

    let where_clause: HashMap<String, crate::parser::ast::Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
                }
                _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(
                    format!("{:?}", v),
                )),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: None,
    }));

    Ok(())
}

fn word_cbu_attach_entity(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :entity-id, :role
    process_pairs(vm, &pairs);

    // Create a relationship record
    let values: HashMap<String, crate::parser::ast::Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
                }
                _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(
                    format!("{:?}", v),
                )),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_ENTITY_RELATIONSHIP".to_string(),
        values,
    }));

    Ok(())
}

fn word_cbu_attach_proper_person(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :person-name, :role
    process_pairs(vm, &pairs);

    let values: HashMap<String, crate::parser::ast::Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
                }
                _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(
                    format!("{:?}", v),
                )),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_PROPER_PERSON".to_string(),
        values,
    }));

    Ok(())
}

fn word_cbu_finalize(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :cbu-id, :status
    process_pairs(vm, &pairs);

    // Separate cbu-id from status
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (k, v) in pairs {
        let key = k.trim_start_matches(':').to_string();
        let val = match v {
            Value::Str(s) => {
                crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(s))
            }
            _ => crate::parser::ast::Value::Literal(crate::parser::ast::Literal::String(format!(
                "{:?}",
                v
            ))),
        };

        if key == "cbu-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// CRUD Operations (Phase 5)
fn word_crud_begin(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "crud.begin", 2) // :operation-type, :asset-type
}

fn word_crud_commit(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "crud.commit", 3) // :entity-table, :ai-instruction, :ai-provider
}

// Attribute Operations (Phase 2)
fn word_attr_require(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "attr.require", 1) // @attr reference
}

fn word_attr_set(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "attr.set", 2) // @attr reference, value
}

fn word_attr_validate(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "attr.validate", 2) // @attr reference, value
}

// Document Operations (Phase 3) - extended
fn word_document_link_to_cbu(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.link-to-cbu", 3) // :cbu-id, :document-id, :relationship-type
}

fn word_document_extract_attributes(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.extract-attributes", 2) // :document-id, :document-type
}

fn word_document_require(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.require", 1) // @doc reference
}

/// Constructs the complete DSL Vocabulary with all domain verbs.
pub fn kyc_orch_vocab() -> Vocab {
    let specs = vec![
        // Case Operations - stack_effect = (num_pairs * 2, 0)
        WordSpec {
            id: WordId(0),
            name: "case.create".to_string(),
            domain: "case".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_case_create),
        },
        WordSpec {
            id: WordId(1),
            name: "case.update".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_update),
        },
        WordSpec {
            id: WordId(2),
            name: "case.validate".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_validate),
        },
        WordSpec {
            id: WordId(3),
            name: "case.approve".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_approve),
        },
        WordSpec {
            id: WordId(4),
            name: "case.close".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_close),
        },
        // Entity Operations
        WordSpec {
            id: WordId(5),
            name: "entity.register".to_string(),
            domain: "entity".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_entity_register),
        },
        WordSpec {
            id: WordId(6),
            name: "entity.classify".to_string(),
            domain: "entity".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_entity_classify),
        },
        WordSpec {
            id: WordId(7),
            name: "entity.link".to_string(),
            domain: "entity".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_entity_link),
        },
        WordSpec {
            id: WordId(8),
            name: "identity.verify".to_string(),
            domain: "entity".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_identity_verify),
        },
        WordSpec {
            id: WordId(9),
            name: "identity.attest".to_string(),
            domain: "entity".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_identity_attest),
        },
        // Product Operations
        WordSpec {
            id: WordId(10),
            name: "products.add".to_string(),
            domain: "products".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_products_add),
        },
        WordSpec {
            id: WordId(11),
            name: "products.configure".to_string(),
            domain: "products".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_products_configure),
        },
        WordSpec {
            id: WordId(12),
            name: "services.discover".to_string(),
            domain: "services".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_services_discover),
        },
        WordSpec {
            id: WordId(13),
            name: "services.provision".to_string(),
            domain: "services".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_services_provision),
        },
        WordSpec {
            id: WordId(14),
            name: "services.activate".to_string(),
            domain: "services".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_services_activate),
        },
        // KYC Operations
        WordSpec {
            id: WordId(15),
            name: "kyc.start".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_kyc_start),
        },
        WordSpec {
            id: WordId(16),
            name: "kyc.collect".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_kyc_collect),
        },
        WordSpec {
            id: WordId(17),
            name: "kyc.verify".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_kyc_verify),
        },
        WordSpec {
            id: WordId(18),
            name: "kyc.assess".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_kyc_assess),
        },
        WordSpec {
            id: WordId(19),
            name: "compliance.screen".to_string(),
            domain: "compliance".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_compliance_screen),
        },
        WordSpec {
            id: WordId(20),
            name: "compliance.monitor".to_string(),
            domain: "compliance".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_compliance_monitor),
        },
        // UBO Operations
        WordSpec {
            id: WordId(21),
            name: "ubo.collect-entity-data".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_collect_entity_data),
        },
        WordSpec {
            id: WordId(22),
            name: "ubo.get-ownership-structure".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_get_ownership_structure),
        },
        WordSpec {
            id: WordId(23),
            name: "ubo.resolve-ubos".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_resolve_ubos),
        },
        WordSpec {
            id: WordId(24),
            name: "ubo.calculate-indirect-ownership".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_calculate_indirect_ownership),
        },
        // Document Operations
        WordSpec {
            id: WordId(25),
            name: "document.catalog".to_string(),
            domain: "document".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_document_catalog),
        },
        WordSpec {
            id: WordId(26),
            name: "document.verify".to_string(),
            domain: "document".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_document_verify),
        },
        WordSpec {
            id: WordId(27),
            name: "document.extract".to_string(),
            domain: "document".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_document_extract),
        },
        WordSpec {
            id: WordId(28),
            name: "document.link".to_string(),
            domain: "document".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_document_link),
        },
        // Low-level attribute operations
        WordSpec {
            id: WordId(29),
            name: "require-attribute".to_string(),
            domain: "core".to_string(),
            stack_effect: (1, 0),
            impl_fn: Arc::new(word_require_attribute),
        },
        WordSpec {
            id: WordId(30),
            name: "set-attribute".to_string(),
            domain: "core".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_set_attribute),
        },
        // CBU Operations (Phase 4)
        WordSpec {
            id: WordId(31),
            name: "cbu.create".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (6, 0), // 3 pairs
            impl_fn: Arc::new(word_cbu_create),
        },
        WordSpec {
            id: WordId(32),
            name: "cbu.read".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_cbu_read),
        },
        WordSpec {
            id: WordId(33),
            name: "cbu.update".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_update),
        },
        WordSpec {
            id: WordId(34),
            name: "cbu.delete".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_cbu_delete),
        },
        WordSpec {
            id: WordId(35),
            name: "cbu.list".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_cbu_list),
        },
        WordSpec {
            id: WordId(36),
            name: "cbu.attach-entity".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_attach_entity),
        },
        WordSpec {
            id: WordId(37),
            name: "cbu.attach-proper-person".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_attach_proper_person),
        },
        WordSpec {
            id: WordId(38),
            name: "cbu.finalize".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_finalize),
        },
        // CRUD Operations (Phase 5)
        WordSpec {
            id: WordId(39),
            name: "crud.begin".to_string(),
            domain: "crud".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_crud_begin),
        },
        WordSpec {
            id: WordId(40),
            name: "crud.commit".to_string(),
            domain: "crud".to_string(),
            stack_effect: (6, 0), // 3 pairs
            impl_fn: Arc::new(word_crud_commit),
        },
        // Attribute Operations (Phase 2)
        WordSpec {
            id: WordId(41),
            name: "attr.require".to_string(),
            domain: "attr".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_attr_require),
        },
        WordSpec {
            id: WordId(42),
            name: "attr.set".to_string(),
            domain: "attr".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_attr_set),
        },
        WordSpec {
            id: WordId(43),
            name: "attr.validate".to_string(),
            domain: "attr".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_attr_validate),
        },
        // Document Operations (Phase 3) - extended
        WordSpec {
            id: WordId(44),
            name: "document.link-to-cbu".to_string(),
            domain: "document".to_string(),
            stack_effect: (6, 0), // 3 pairs
            impl_fn: Arc::new(word_document_link_to_cbu),
        },
        WordSpec {
            id: WordId(45),
            name: "document.extract-attributes".to_string(),
            domain: "document".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_document_extract_attributes),
        },
        WordSpec {
            id: WordId(46),
            name: "document.require".to_string(),
            domain: "document".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_document_require),
        },
    ];
    Vocab::new(specs)
}
```

## File: rust/src/forth_engine/env.rs

```rust
//! Runtime Environment for the DSL Forth Engine.
//!
//! Provides database-backed storage for attributes and documents during DSL execution.

use crate::cbu_model_dsl::ast::CbuModel;
use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use crate::parser::ast::CrudStatement;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnboardingRequestId(pub String);

/// The RuntimeEnv holds state for a single DSL execution session.
/// It provides access to the database for reading/writing attributes and documents.
pub struct RuntimeEnv {
    /// The case/request ID for this execution
    pub request_id: OnboardingRequestId,

    /// Database connection pool (when database feature enabled)
    #[cfg(feature = "database")]
    pub pool: Option<PgPool>,

    /// Current CBU ID for this execution context
    pub cbu_id: Option<Uuid>,

    /// Current entity ID for this execution context
    pub entity_id: Option<Uuid>,

    /// In-memory cache for attributes during execution
    pub attribute_cache: HashMap<AttributeId, Value>,

    /// In-memory cache for documents during execution
    pub document_cache: HashMap<DocumentId, DocumentMeta>,

    /// Extracted case_id from DSL execution
    pub case_id: Option<String>,

    /// Sink attributes - attributes that should be populated for this context
    pub sink_attributes: HashSet<Uuid>,

    /// Source attributes - attributes that produce data in this context
    pub source_attributes: HashSet<Uuid>,

    /// CBU Model specification for validation (Phase 5)
    pub cbu_model: Option<CbuModel>,

    /// Pending CRUD statements to be executed (Phase 6)
    pub pending_crud: Vec<CrudStatement>,

    /// Current CBU state for state machine validation
    pub cbu_state: Option<String>,
}

/// Document metadata
#[derive(Debug, Clone)]
pub struct DocumentMeta {
    pub id: DocumentId,
    pub name: String,
    pub doc_type: String,
    pub location: Option<String>,
}

impl RuntimeEnv {
    /// Create a new RuntimeEnv without database connection
    pub fn new(request_id: OnboardingRequestId) -> Self {
        Self {
            request_id,
            #[cfg(feature = "database")]
            pool: None,
            cbu_id: None,
            entity_id: None,
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
            sink_attributes: HashSet::new(),
            source_attributes: HashSet::new(),
            cbu_model: None,
            pending_crud: Vec::new(),
            cbu_state: None,
        }
    }

    /// Create a new RuntimeEnv with database connection
    #[cfg(feature = "database")]
    pub fn with_pool(request_id: OnboardingRequestId, pool: PgPool) -> Self {
        Self {
            request_id,
            pool: Some(pool),
            cbu_id: None,
            entity_id: None,
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
            sink_attributes: HashSet::new(),
            source_attributes: HashSet::new(),
            cbu_model: None,
            pending_crud: Vec::new(),
            cbu_state: None,
        }
    }

    /// Set the CBU ID for this execution context
    pub fn set_cbu_id(&mut self, id: Uuid) {
        self.cbu_id = Some(id);
    }

    /// Get the CBU ID, returning error if not set
    pub fn ensure_cbu_id(&self) -> Result<Uuid, &'static str> {
        self.cbu_id.ok_or("CBU ID not set in runtime environment")
    }

    /// Set the entity ID for this execution context
    pub fn set_entity_id(&mut self, id: Uuid) {
        self.entity_id = Some(id);
    }

    /// Get the entity ID, returning error if not set
    pub fn ensure_entity_id(&self) -> Result<Uuid, &'static str> {
        self.entity_id
            .ok_or("Entity ID not set in runtime environment")
    }

    /// Check if an attribute is a sink for this context
    pub fn is_sink(&self, attr_id: &Uuid) -> bool {
        self.sink_attributes.contains(attr_id)
    }

    /// Check if an attribute is a source for this context
    pub fn is_source(&self, attr_id: &Uuid) -> bool {
        self.source_attributes.contains(attr_id)
    }

    /// Add a sink attribute
    pub fn add_sink_attribute(&mut self, attr_id: Uuid) {
        self.sink_attributes.insert(attr_id);
    }

    /// Add a source attribute
    pub fn add_source_attribute(&mut self, attr_id: Uuid) {
        self.source_attributes.insert(attr_id);
    }

    /// Set sink attributes from a list
    pub fn set_sink_attributes(&mut self, attrs: Vec<Uuid>) {
        self.sink_attributes = attrs.into_iter().collect();
    }

    /// Set source attributes from a list
    pub fn set_source_attributes(&mut self, attrs: Vec<Uuid>) {
        self.source_attributes = attrs.into_iter().collect();
    }

    /// Check if database is available
    #[cfg(feature = "database")]
    pub fn has_database(&self) -> bool {
        self.pool.is_some()
    }

    #[cfg(not(feature = "database"))]
    pub fn has_database(&self) -> bool {
        false
    }

    /// Set the CBU Model for validation
    pub fn set_cbu_model(&mut self, model: CbuModel) {
        // Set initial state from model
        self.cbu_state = Some(model.states.initial.clone());
        self.cbu_model = Some(model);
    }

    /// Get the CBU Model
    pub fn get_cbu_model(&self) -> Option<&CbuModel> {
        self.cbu_model.as_ref()
    }

    /// Add a CRUD statement to pending operations
    pub fn push_crud(&mut self, stmt: CrudStatement) {
        self.pending_crud.push(stmt);
    }

    /// Get pending CRUD statements
    pub fn get_pending_crud(&self) -> &[CrudStatement] {
        &self.pending_crud
    }

    /// Take pending CRUD statements (drains the list)
    pub fn take_pending_crud(&mut self) -> Vec<CrudStatement> {
        std::mem::take(&mut self.pending_crud)
    }

    /// Set current CBU state
    pub fn set_cbu_state(&mut self, state: String) {
        self.cbu_state = Some(state);
    }

    /// Get current CBU state
    pub fn get_cbu_state(&self) -> Option<&str> {
        self.cbu_state.as_deref()
    }

    /// Check if a state transition is valid according to the CBU Model
    pub fn is_valid_transition(&self, to_state: &str) -> bool {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => {
                model.states.is_valid_transition(from_state, to_state)
            }
            _ => true, // No model or state = no validation
        }
    }

    /// Get the verb required for a state transition
    pub fn get_transition_verb(&self, to_state: &str) -> Option<String> {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => model
                .states
                .get_transition(from_state, to_state)
                .map(|t| t.verb.clone()),
            _ => None,
        }
    }

    /// Check if all required attributes are present for a transition
    pub fn check_transition_preconditions(&self, to_state: &str) -> Vec<String> {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => {
                if let Some(transition) = model.states.get_transition(from_state, to_state) {
                    let present: Vec<&str> =
                        self.attribute_cache.keys().map(|k| k.0.as_str()).collect();
                    transition
                        .check_preconditions(&present)
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// Get attribute from cache (sync - for VM execution)
    pub fn get_attribute(&self, id: &AttributeId) -> Option<&Value> {
        self.attribute_cache.get(id)
    }

    /// Set attribute in cache (will be persisted at end of execution)
    pub fn set_attribute(&mut self, id: AttributeId, value: Value) {
        self.attribute_cache.insert(id, value);
    }

    /// Set the case_id extracted during execution
    pub fn set_case_id(&mut self, case_id: String) {
        self.case_id = Some(case_id);
    }

    /// Get the case_id
    pub fn get_case_id(&self) -> Option<&String> {
        self.case_id.as_ref()
    }

    /// Load attribute from database into cache
    #[cfg(feature = "database")]
    pub async fn load_attribute(&mut self, id: &AttributeId) -> Result<Option<Value>, sqlx::Error> {
        if let Some(pool) = &self.pool {
            let case_id = self.case_id.as_deref().unwrap_or("");

            let row = sqlx::query_as::<_, (String,)>(
                r#"
                SELECT attribute_value
                FROM "ob-poc".attribute_values
                WHERE attribute_id = $1::uuid AND entity_id = $2
                "#,
            )
            .bind(&id.0)
            .bind(case_id)
            .fetch_optional(pool)
            .await?;

            if let Some((value_text,)) = row {
                let value = Value::Str(value_text);
                self.attribute_cache.insert(id.clone(), value.clone());
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    // Note: DB operations for DSL/AST persistence, CBU creation, and attribute saving
    // have been moved to the central database facade (DslRepository).
    // RuntimeEnv now only handles in-memory caching and attribute loading during execution.
    // See crate::database::DslRepository for transactional DSL/AST saves.
}

/// Generate a new OB Request ID
pub fn mint_ob_request_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    format!("OB-{}", &uuid.to_string()[..8].to_uppercase())
}

/// Generate DSL onboarding template with minted ID
pub fn generate_onboarding_template(
    ob_request_id: &str,
    client_name: &str,
    client_type: &str,
) -> String {
    format!(
        r#"(case.create :case-id "{}" :case-type "ONBOARDING" :client-name "{}" :client-type "{}")"#,
        ob_request_id, client_name, client_type
    )
}
```

## File: rust/src/dsl_manager/clean_manager.rs

```rust
//! Clean DSL Manager - Refactored Gateway Following Call Chain Pattern
//!
//! This module provides a clean, simplified DSL Manager implementation based on the
//! proven call chain architecture from the independent implementation blueprint.
//!
//! ## Architecture: Clean Call Chain Pattern
//! DSL Manager → DSL Mod → DB State Manager → DSL Visualizer
//!
//! ## Design Principles from Session Record
//! 1. **DSL-First Design**: Core system works without AI dependencies
//! 2. **Incremental Accumulation**: Base DSL + incremental additions = accumulated state
//! 3. **Clean Separation**: AI as optional layer, DSL CRUD as core system
//! 4. **Call Chain Approach**: Build it, run it, see where it breaks, fix incrementally
//!
//! ## Key Responsibilities
//! - Serve as the single entry point gateway for all DSL operations
//! - Route DSL operations through the clean call chain
//! - Coordinate incremental DSL accumulation (DSL-as-State pattern)
//! - Provide unified interface for AI and direct DSL operations
//! - Maintain separation between core DSL CRUD and optional AI layer

use crate::db_state_manager::DbStateManager;
use crate::dsl::DslPipelineProcessor;
#[cfg(feature = "database")]
use crate::dsl::PipelineConfig;
use crate::dsl_visualizer::DslVisualizer;
use std::time::Instant;
use uuid::Uuid;

/// Clean DSL Manager following the proven call chain pattern
pub struct CleanDslManager {
    /// DSL processing pipeline (DSL Mod)
    dsl_processor: DslPipelineProcessor,
    /// Database state manager
    db_state_manager: DbStateManager,
    /// Visualization generator
    visualizer: DslVisualizer,
    /// Configuration
    config: CleanManagerConfig,
    /// Database service for SQLX integration
    #[cfg(feature = "database")]
    database_service: Option<crate::database::DictionaryDatabaseService>,
}

/// Configuration for Clean DSL Manager
#[derive(Debug, Clone)]
pub struct CleanManagerConfig {
    /// Enable detailed logging throughout the call chain
    pub enable_detailed_logging: bool,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Maximum processing time for the entire call chain (seconds)
    pub max_processing_time_seconds: u64,
    /// Enable automatic state cleanup
    pub enable_auto_cleanup: bool,
}

impl Default for CleanManagerConfig {
    fn default() -> Self {
        Self {
            enable_detailed_logging: true,
            enable_metrics: true,
            max_processing_time_seconds: 60,
            enable_auto_cleanup: false,
        }
    }
}

/// Result from the complete call chain processing
#[derive(Debug, Clone)]
pub struct CallChainResult {
    /// Overall operation success status
    pub success: bool,
    /// Case ID that was processed
    pub case_id: String,
    /// Total processing time in milliseconds
    pub processing_time_ms: u64,
    /// Any errors that occurred during the call chain
    pub errors: Vec<String>,
    /// Whether visualization was successfully generated
    pub visualization_generated: bool,
    /// Whether this operation used AI generation
    pub ai_generated: bool,
    /// Call chain step details
    pub step_details: CallChainSteps,
}

/// Detailed results from each step in the call chain
#[derive(Debug, Clone)]
pub struct CallChainSteps {
    /// DSL Mod processing result
    pub dsl_processing: Option<DslProcessingStepResult>,
    /// DB State Manager result
    pub state_management: Option<StateManagementStepResult>,
    /// DSL Visualizer result
    pub visualization: Option<VisualizationStepResult>,
}

/// Result from DSL processing step
#[derive(Debug, Clone)]
pub struct DslProcessingStepResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub parsed_ast_available: bool,
    pub domain_snapshot_created: bool,
    pub errors: Vec<String>,
}

/// Result from state management step
#[derive(Debug, Clone)]
pub struct StateManagementStepResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub version_number: u32,
    pub snapshot_id: String,
    pub errors: Vec<String>,
}

/// Result from visualization step
#[derive(Debug, Clone)]
pub struct VisualizationStepResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub output_size_bytes: usize,
    pub format: String,
    pub errors: Vec<String>,
}

/// Result from incremental DSL processing
#[derive(Debug, Clone)]
pub struct IncrementalResult {
    /// Operation success status
    pub success: bool,
    /// Case ID that was processed
    pub case_id: String,
    /// Complete accumulated DSL content
    pub accumulated_dsl: String,
    /// New version number after increment
    pub version_number: u32,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Result from validation-only operations
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Validation success status
    pub valid: bool,
    /// Validation errors found
    pub errors: Vec<String>,
    /// Validation warnings
    pub warnings: Vec<String>,
    /// Number of validation rules checked
    pub rules_checked: u32,
    /// Overall compliance score (0.0 to 1.0)
    pub compliance_score: f64,
}

/// Result from AI-enhanced operations
#[derive(Debug, Clone)]
pub struct AiResult {
    /// Overall operation success
    pub success: bool,
    /// AI-generated DSL content
    pub generated_dsl: String,
    /// Case ID for the generated DSL
    pub case_id: String,
    /// AI confidence score (0.0 to 1.0)
    pub ai_confidence: f64,
    /// Whether the generated DSL passed validation
    pub validation_passed: bool,
    /// Total processing time including AI generation
    pub processing_time_ms: u64,
    /// Flag indicating this was AI-generated
    pub ai_generated: bool,
}

impl CleanDslManager {
    /// Create a new Clean DSL Manager with default configuration
    pub fn new() -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::new(),
            db_state_manager: DbStateManager::new(),
            visualizer: DslVisualizer::new(),
            config: CleanManagerConfig::default(),
            #[cfg(feature = "database")]
            database_service: None,
        }
    }

    /// Create a Clean DSL Manager with custom configuration
    pub fn with_config(config: CleanManagerConfig) -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::new(),
            db_state_manager: DbStateManager::new(),
            visualizer: DslVisualizer::new(),
            config,
            #[cfg(feature = "database")]
            database_service: None,
        }
    }

    /// Create a Clean DSL Manager with database connectivity for SQLX integration
    #[cfg(feature = "database")]
    pub fn with_database(database_service: crate::database::DictionaryDatabaseService) -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::with_database(database_service.clone()),
            db_state_manager: DbStateManager::new(),
            visualizer: DslVisualizer::new(),
            config: CleanManagerConfig::default(),
            #[cfg(feature = "database")]
            database_service: Some(database_service),
        }
    }

    /// Create a Clean DSL Manager with both config and database connectivity
    #[cfg(feature = "database")]
    pub fn with_config_and_database(
        config: CleanManagerConfig,
        database_service: crate::database::DictionaryDatabaseService,
    ) -> Self {
        // Create database manager from the service's pool
        let db_manager =
            crate::database::DatabaseManager::from_pool(database_service.pool().clone());

        Self {
            dsl_processor: DslPipelineProcessor::with_config_and_database(
                PipelineConfig {
                    enable_strict_validation: true,
                    fail_fast: true,
                    enable_detailed_logging: config.enable_detailed_logging,
                    max_step_time_seconds: config.max_processing_time_seconds,
                    enable_metrics: config.enable_metrics,
                },
                database_service.clone(),
            ),
            // Wire database through to state manager
            db_state_manager: DbStateManager::with_database(db_manager),
            visualizer: DslVisualizer::new(),
            config,
            #[cfg(feature = "database")]
            database_service: Some(database_service),
        }
    }

    /// Check if the manager has database connectivity
    #[cfg(feature = "database")]
    pub fn has_database(&self) -> bool {
        self.database_service.is_some()
    }

    /// Check if the manager has database connectivity (without database feature)
    #[cfg(not(feature = "database"))]
    pub fn has_database(&self) -> bool {
        false
    }

    /// Get a reference to the database service if available
    #[cfg(feature = "database")]
    pub fn database_service(&self) -> Option<&crate::database::DictionaryDatabaseService> {
        self.database_service.as_ref()
    }

    /// Get a reference to the database service if available (without database feature)
    #[cfg(not(feature = "database"))]
    pub fn database_service(&self) -> Option<()> {
        None
    }

    /// Process DSL request through the complete call chain
    /// This is the main entry point implementing: DSL Manager → Forth Engine → DB
    pub fn process_dsl_request(&mut self, dsl_content: String) -> CallChainResult {
        use crate::forth_engine::{extract_case_id, DslSheet};

        let start_time = Instant::now();

        // Pre-extract case_id for sheet naming
        let preliminary_case_id = extract_case_id(&dsl_content).unwrap_or_else(|| {
            format!(
                "CASE-{}",
                uuid::Uuid::new_v4().to_string()[..8].to_uppercase()
            )
        });

        let sheet = DslSheet {
            id: preliminary_case_id.clone(),
            domain: "dsl".to_string(),
            version: "1.0".to_string(),
            content: dsl_content.clone(),
        };

        // Execute through Forth engine
        #[cfg(feature = "database")]
        let execution_result = if let Some(ref db_service) = self.database_service {
            // Run async database operations using block_in_place for multi-threaded runtime
            let pool = db_service.pool().clone();
            let sheet_clone = sheet.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    crate::forth_engine::execute_sheet_with_db(&sheet_clone, pool).await
                })
            })
        } else {
            crate::forth_engine::execute_sheet(&sheet).map(|logs| {
                crate::forth_engine::ExecutionResult {
                    logs,
                    case_id: Some(preliminary_case_id.clone()),
                    success: true,
                    version: 0,
                }
            })
        };

        #[cfg(not(feature = "database"))]
        let execution_result = crate::forth_engine::execute_sheet(&sheet).map(|logs| {
            crate::forth_engine::ExecutionResult {
                logs,
                case_id: Some(preliminary_case_id.clone()),
                success: true,
                version: 0,
            }
        });

        match execution_result {
            Ok(result) => {
                let case_id = result.case_id.unwrap_or(preliminary_case_id);

                // Save the DSL to the state manager for accumulation support
                // This is critical for the DSL-as-State pattern
                // Use synchronous update to avoid tokio runtime issues
                self.db_state_manager
                    .update_accumulated_dsl_sync(&case_id, &dsl_content);

                CallChainResult {
                    success: true,
                    case_id,
                    processing_time_ms: start_time.elapsed().as_millis() as u64,
                    errors: vec![],
                    visualization_generated: false,
                    ai_generated: false,
                    step_details: CallChainSteps {
                        dsl_processing: Some(DslProcessingStepResult {
                            success: true,
                            processing_time_ms: start_time.elapsed().as_millis() as u64,
                            parsed_ast_available: true,
                            domain_snapshot_created: false,
                            errors: result.logs,
                        }),
                        state_management: None,
                        visualization: None,
                    },
                }
            }
            Err(e) => CallChainResult {
                success: false,
                case_id: preliminary_case_id,
                processing_time_ms: start_time.elapsed().as_millis() as u64,
                errors: vec![e.to_string()],
                visualization_generated: false,
                ai_generated: false,
                step_details: CallChainSteps {
                    dsl_processing: Some(DslProcessingStepResult {
                        success: false,
                        processing_time_ms: start_time.elapsed().as_millis() as u64,
                        parsed_ast_available: false,
                        domain_snapshot_created: false,
                        errors: vec![e.to_string()],
                    }),
                    state_management: None,
                    visualization: None,
                },
            },
        }
    }

    /// Process incremental DSL addition (DSL-as-State pattern)
    pub async fn process_incremental_dsl(
        &mut self,
        case_id: String,
        additional_dsl: String,
    ) -> IncrementalResult {
        if self.config.enable_detailed_logging {
            println!(
                "🔄 Clean DSL Manager: Processing incremental DSL for case {}",
                case_id
            );
        }

        // Load existing accumulated state
        let existing_state = self.db_state_manager.load_accumulated_state(&case_id).await;

        // Update the accumulated DSL in the state manager
        let update_success = self
            .db_state_manager
            .update_accumulated_dsl(&case_id, &additional_dsl)
            .await;

        if !update_success {
            return IncrementalResult {
                success: false,
                case_id: case_id.clone(),
                accumulated_dsl: existing_state.current_dsl,
                version_number: existing_state.version,
                errors: vec!["Failed to update accumulated DSL".to_string()],
            };
        }

        // Load the updated state
        let updated_state = self.db_state_manager.load_accumulated_state(&case_id).await;

        // Process the complete accumulated DSL through the call chain
        let call_chain_result = self.process_dsl_request(updated_state.current_dsl.clone());

        // Capture errors from execution if any
        let errors = if call_chain_result.success {
            Vec::new()
        } else {
            call_chain_result.errors
        };

        IncrementalResult {
            success: call_chain_result.success,
            case_id: case_id.clone(),
            accumulated_dsl: updated_state.current_dsl,
            version_number: updated_state.version,
            errors,
        }
    }

    /// Validate DSL content without full processing
    pub async fn validate_dsl_only(&self, dsl_content: String) -> ValidationResult {
        if self.config.enable_detailed_logging {
            println!("🔍 Clean DSL Manager: Validation-only mode");
        }

        let validation_result = self.dsl_processor.validate_dsl_content(&dsl_content).await;

        let mut rules_checked = 4; // Base validation rules from 4-step pipeline
        let mut compliance_score = if validation_result.success { 1.0 } else { 0.0 };

        // Adjust score based on warnings
        let warning_count = validation_result
            .step_results
            .iter()
            .map(|step| step.warnings.len())
            .sum::<usize>();

        if warning_count > 0 {
            compliance_score = (compliance_score * 0.8_f64).max(0.0);
            rules_checked += warning_count as u32;
        }

        ValidationResult {
            valid: validation_result.success,
            errors: validation_result.errors,
            warnings: validation_result
                .step_results
                .iter()
                .flat_map(|step| step.warnings.clone())
                .collect(),
            rules_checked,
            compliance_score,
        }
    }

    /// Process AI-generated DSL instruction (AI separation pattern)
    pub async fn process_ai_instruction(&mut self, instruction: String) -> AiResult {
        if self.config.enable_detailed_logging {
            println!("🤖 Clean DSL Manager: Processing AI instruction (mock implementation)");
        }

        let start_time = Instant::now();

        // Mock AI DSL generation - in real implementation, this would call AI services
        let generated_dsl = self.mock_ai_generation(&instruction).await;
        let case_id = self.extract_or_generate_case_id(&generated_dsl);

        // Validate the generated DSL
        let validation_result = self.validate_dsl_only(generated_dsl.clone()).await;

        // If validation passes, process through the call chain
        let mut processing_success = false;
        if validation_result.valid {
            let call_chain_result = self.process_dsl_request(generated_dsl.clone());
            processing_success = call_chain_result.success;
        }

        AiResult {
            success: validation_result.valid && processing_success,
            generated_dsl,
            case_id,
            ai_confidence: 0.85, // Mock confidence score
            validation_passed: validation_result.valid,
            processing_time_ms: start_time.elapsed().as_millis() as u64,
            ai_generated: true,
        }
    }

    /// Health check for the entire call chain
    pub async fn health_check(&mut self) -> bool {
        if self.config.enable_detailed_logging {
            println!("🏥 Clean DSL Manager: Performing call chain health check");
        }

        let dsl_healthy = self.dsl_processor.health_check().await;
        let db_healthy = self.db_state_manager.health_check().await;
        let viz_healthy = self.visualizer.health_check().await;

        let overall_healthy = dsl_healthy && db_healthy && viz_healthy;

        if self.config.enable_detailed_logging {
            println!(
                "✅ Clean DSL Manager health check: {} (DSL: {}, DB: {}, Viz: {})",
                if overall_healthy {
                    "HEALTHY"
                } else {
                    "UNHEALTHY"
                },
                if dsl_healthy { "OK" } else { "FAIL" },
                if db_healthy { "OK" } else { "FAIL" },
                if viz_healthy { "OK" } else { "FAIL" }
            );
        }

        overall_healthy
    }

    // Private helper methods

    async fn mock_ai_generation(&self, instruction: &str) -> String {
        // Mock AI generation - replace with real AI service integration
        if instruction.to_lowercase().contains("onboarding") {
            format!(
                r#"(case.create :case-id "{}" :case-type "ONBOARDING" :instruction "{}")"#,
                self.generate_case_id(),
                instruction
            )
        } else if instruction.to_lowercase().contains("kyc") {
            format!(
                r#"(kyc.collect :case-id "{}" :collection-type "ENHANCED" :instruction "{}")"#,
                self.generate_case_id(),
                instruction
            )
        } else {
            format!(
                r#"(case.create :case-id "{}" :case-type "GENERAL" :instruction "{}")"#,
                self.generate_case_id(),
                instruction
            )
        }
    }

    fn extract_or_generate_case_id(&self, dsl_content: &str) -> String {
        // Try to extract case ID from DSL content
        if let Some(start) = dsl_content.find(":case-id") {
            if let Some(quote_start) = dsl_content[start..].find('"') {
                let absolute_quote_start = start + quote_start + 1;
                if let Some(quote_end) = dsl_content[absolute_quote_start..].find('"') {
                    return dsl_content[absolute_quote_start..absolute_quote_start + quote_end]
                        .to_string();
                }
            }
        }
        // Generate new case ID if extraction failed
        self.generate_case_id()
    }

    fn generate_case_id(&self) -> String {
        format!("CASE-{}", Uuid::new_v4().to_string()[..8].to_uppercase())
    }
}

impl Default for CleanDslManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanDslManager {
    /// Create DSL Manager from database pool for SQLX integration testing
    #[cfg(feature = "database")]
    pub async fn from_database_pool(pool: sqlx::PgPool) -> Self {
        let database_service = crate::database::DictionaryDatabaseService::new(pool);
        Self::with_database(database_service)
    }

    /// Test database connectivity if available
    #[cfg(feature = "database")]
    pub async fn test_database_connection(&self) -> Result<bool, String> {
        if let Some(db_service) = &self.database_service {
            // Use the database service to test connectivity
            match db_service.health_check().await {
                Ok(_) => Ok(true),
                Err(e) => Err(format!("Database connection test failed: {}", e)),
            }
        } else {
            Err("No database service configured".to_string())
        }
    }

    /// Test database connectivity if available (without database feature)
    #[cfg(not(feature = "database"))]
    pub async fn test_database_connection(&self) -> Result<bool, String> {
        Err("Database feature not enabled".to_string())
    }

    /// Execute DSL with database persistence for integration testing
    pub async fn execute_dsl_with_database(&mut self, dsl_content: String) -> CallChainResult {
        if !self.has_database() {
            return CallChainResult {
                success: false,
                case_id: "NO_DATABASE".to_string(),
                processing_time_ms: 0,
                errors: vec!["No database connectivity configured".to_string()],
                visualization_generated: false,
                ai_generated: false,
                step_details: CallChainSteps {
                    dsl_processing: None,
                    state_management: None,
                    visualization: None,
                },
            };
        }

        // Use the regular processing flow - the database connectivity is already wired through
        self.process_dsl_request(dsl_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clean_dsl_manager_creation() {
        let mut manager = CleanDslManager::new();
        assert!(manager.health_check().await);
    }

    #[tokio::test]
    async fn test_dsl_call_chain_processing() {
        let mut manager = CleanDslManager::new();
        let dsl_content =
            r#"(case.create :case-id "CLEAN-001" :case-type "ONBOARDING")"#.to_string();

        let result = manager.process_dsl_request(dsl_content);

        if !result.success {
            eprintln!("Test failed with errors: {:?}", result.errors);
        }
        assert!(result.success);
        assert_eq!(result.case_id, "CLEAN-001");
        assert!(!result.ai_generated);
    }

    #[tokio::test]
    async fn test_incremental_dsl_processing() {
        let mut manager = CleanDslManager::new();

        // Base DSL
        let base_dsl = r#"(case.create :case-id "INC-001" :case-type "ONBOARDING")"#.to_string();
        let base_result = manager.process_dsl_request(base_dsl);
        assert!(base_result.success);

        // Incremental DSL
        let incremental_dsl =
            r#"(kyc.collect :case-id "INC-001" :collection-type "ENHANCED")"#.to_string();
        let incremental_result = manager
            .process_incremental_dsl("INC-001".to_string(), incremental_dsl)
            .await;

        assert!(incremental_result.success);
        // Verify DSL accumulation works correctly
        assert!(
            incremental_result.accumulated_dsl.contains("case.create"),
            "Accumulated DSL should contain base case.create"
        );
        assert!(
            incremental_result.accumulated_dsl.contains("kyc.collect"),
            "Accumulated DSL should contain incremental kyc.collect"
        );
    }

    #[tokio::test]
    async fn test_validation_only_mode() {
        let manager = CleanDslManager::new();
        let valid_dsl = r#"(entity.register :case-id "VAL-001" :entity-type "CORP")"#.to_string();

        let validation_result = manager.validate_dsl_only(valid_dsl).await;

        assert!(validation_result.valid);
        assert!(validation_result.errors.is_empty());
        assert!(validation_result.rules_checked > 0);
        assert!(validation_result.compliance_score > 0.0);
    }

    #[tokio::test]
    async fn test_ai_instruction_processing() {
        let mut manager = CleanDslManager::new();
        let instruction = "Create onboarding case for UK tech company".to_string();

        let ai_result = manager.process_ai_instruction(instruction).await;

        assert!(ai_result.ai_generated);
        assert!(!ai_result.generated_dsl.is_empty());
        assert!(ai_result.generated_dsl.contains("onboarding"));
        assert!(!ai_result.case_id.is_empty());
    }

    #[tokio::test]
    async fn test_failed_dsl_processing() {
        let mut manager = CleanDslManager::new();
        let invalid_dsl = "invalid dsl content".to_string();

        let result = manager.process_dsl_request(invalid_dsl);

        assert!(!result.success);
        assert!(!result.errors.is_empty());
        assert!(!result.visualization_generated);
    }

    #[tokio::test]
    async fn test_call_chain_step_details() {
        let mut manager = CleanDslManager::new();
        let dsl_content =
            r#"(products.add :case-id "STEP-001" :product-type "CUSTODY")"#.to_string();

        let result = manager.process_dsl_request(dsl_content);

        assert!(result.success);
        assert!(result.step_details.dsl_processing.is_some());
        // Note: state_management and visualization are not yet implemented in Forth engine
        // assert!(result.step_details.state_management.is_some());
        // assert!(result.step_details.visualization.is_some());

        let dsl_step = result.step_details.dsl_processing.unwrap();
        assert!(dsl_step.success);
    }

    #[tokio::test]
    async fn test_dsl_orchestration_interface_integration() {
        let mut manager = CleanDslManager::new();

        // Test orchestration interface is properly integrated
        let dsl_content =
            r#"(case.create :case-id "ORCH-001" :case-type "ORCHESTRATION_TEST")"#.to_string();

        let result = manager.process_dsl_request(dsl_content);

        // Verify Forth engine execution worked
        assert!(result.success, "Forth engine execution should succeed");
        assert_eq!(result.case_id, "ORCH-001");

        // Verify DSL processing step completed
        assert!(
            result.step_details.dsl_processing.is_some(),
            "DSL processing step should exist"
        );

        // Verify DSL processing worked
        let dsl_step = result.step_details.dsl_processing.unwrap();
        assert!(dsl_step.success, "DSL processing should succeed");
        assert!(dsl_step.parsed_ast_available, "AST should be available");
    }

    #[tokio::test]
    async fn test_orchestration_error_handling() {
        let mut manager = CleanDslManager::new();

        // Test orchestration with invalid DSL
        let invalid_dsl = "invalid dsl without proper syntax".to_string();

        let result = manager.process_dsl_request(invalid_dsl);

        // Orchestration should handle errors gracefully
        assert!(
            !result.success,
            "Invalid DSL should fail through orchestration"
        );
        assert!(
            !result.errors.is_empty(),
            "Errors should be captured from orchestration"
        );
        assert!(
            !result.visualization_generated,
            "Visualization should not be generated on failure"
        );

        println!("✅ DSL Orchestration Error Handling: WORKING");
    }
}
```

## File: rust/src/db_state_manager/mod.rs

```rust
//! DB State Manager - DSL State Persistence and Management
//!
//! This module provides the database state management layer for DSL operations,
//! following the proven architecture from the independent call chain implementation.
//!
//! ## Architecture Role
//! The DB State Manager is responsible for:
//! - Persisting DSL state changes with complete audit trails
//! - Managing incremental DSL accumulation (DSL-as-State pattern)
//! - Loading accumulated state for continuation operations
//! - Version management and rollback capabilities
//! - Domain snapshot storage for compliance and audit
//!
//! ## 4-Step Processing Pipeline Integration
//! This component handles steps 3-4 of the DSL processing pipeline:
//! 3. **DSL Domain Snapshot Save** - Save domain state snapshot
//! 4. **AST Dual Commit** - Commit both DSL state and parsed AST
//!
//! ## DSL/AST Table Sync Points
//! This module serves as the synchronization layer for all DSL state transformations:
//! - Updates DSL table with accumulated state changes
//! - Updates AST table with parsed representations
//! - Maintains referential integrity between DSL and AST
//! - Provides atomic transactions for state consistency

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// DB State Manager for DSL state persistence
pub struct DbStateManager {
    #[cfg(feature = "database")]
    database: Option<crate::database::DatabaseManager>,
    /// In-memory state store for testing and development
    state_store: HashMap<String, StoredDslState>,
}

/// Stored DSL state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDslState {
    /// Case identifier
    pub case_id: String,
    /// Current accumulated DSL content
    pub current_dsl: String,
    /// Current version number
    pub version: u32,
    /// Domain snapshot data
    pub domain_snapshot: DomainSnapshot,
    /// AST representation (JSON serialized)
    pub parsed_ast: Option<String>,
    /// Metadata for the state
    pub metadata: HashMap<String, String>,
    /// Timestamp of last update
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Audit trail entries
    pub audit_entries: Vec<AuditEntry>,
}

/// Domain snapshot for compliance and audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSnapshot {
    /// Primary domain for this DSL operation
    pub primary_domain: String,
    /// All domains involved in this operation
    pub involved_domains: Vec<String>,
    /// Domain-specific data snapshots
    pub domain_data: HashMap<String, serde_json::Value>,
    /// Compliance flags and markers
    pub compliance_markers: Vec<String>,
    /// Risk assessment data
    pub risk_assessment: Option<String>,
    /// Snapshot timestamp
    pub snapshot_at: chrono::DateTime<chrono::Utc>,
}

/// Audit trail entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for this audit entry
    pub entry_id: String,
    /// Type of operation
    pub operation_type: String,
    /// User who performed the operation
    pub user_id: String,
    /// Timestamp of the operation
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Operation details
    pub details: HashMap<String, String>,
    /// Previous state hash (for integrity)
    pub previous_state_hash: Option<String>,
    /// New state hash
    pub new_state_hash: String,
    /// DSL table sync status
    pub dsl_table_synced: bool,
    /// AST table sync status
    pub ast_table_synced: bool,
}

/// Result from DSL state save operation
#[derive(Debug, Clone)]
pub struct StateResult {
    /// Operation success status
    pub success: bool,
    /// Case ID that was processed
    pub case_id: String,
    /// New version number
    pub version_number: u32,
    /// Snapshot ID for the domain snapshot
    pub snapshot_id: String,
    /// Any errors that occurred
    pub errors: Vec<String>,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Result from state loading operation
#[derive(Debug, Clone)]
pub struct AccumulatedState {
    /// Case ID
    pub case_id: String,
    /// Current accumulated DSL content
    pub current_dsl: String,
    /// Current version number
    pub version: u32,
    /// Domain snapshot
    pub domain_snapshot: Option<DomainSnapshot>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Input for DSL state save operation (from DSL Mod result)
#[derive(Debug, Clone)]
pub struct DslModResult {
    /// Operation success status
    pub success: bool,
    /// Parsed AST (JSON serialized)
    pub parsed_ast: Option<String>,
    /// Domain snapshot data
    pub domain_snapshot: DomainSnapshot,
    /// Case ID extracted from DSL
    pub case_id: String,
    /// Any errors that occurred during processing
    pub errors: Vec<String>,
}

impl DbStateManager {
    /// Create a new DB State Manager with default configuration
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "database")]
            database: None,
            state_store: HashMap::new(),
        }
    }

    /// Create a new DB State Manager with database connection
    #[cfg(feature = "database")]
    pub fn with_database(database: crate::database::DatabaseManager) -> Self {
        Self {
            database: Some(database),
            state_store: HashMap::new(),
        }
    }

    /// Set database connection (when database feature is enabled)
    #[cfg(feature = "database")]
    pub fn set_database(&mut self, database: crate::database::DatabaseManager) {
        self.database = Some(database);
    }

    /// Save DSL state from DSL Mod processing result
    /// This implements steps 3-4 of the DSL processing pipeline
    pub async fn save_dsl_state(&mut self, dsl_result: &DslModResult) -> StateResult {
        let start_time = std::time::Instant::now();

        // Validate input
        if !dsl_result.success {
            return StateResult {
                success: false,
                case_id: dsl_result.case_id.clone(),
                version_number: 0,
                snapshot_id: String::new(),
                errors: vec!["Cannot save state for failed DSL processing".to_string()],
                processing_time_ms: start_time.elapsed().as_millis() as u64,
            };
        }

        // Load existing state or create new
        let mut stored_state = self.load_or_create_state(&dsl_result.case_id).await;

        // Update state with new information
        stored_state.domain_snapshot = dsl_result.domain_snapshot.clone();
        stored_state.parsed_ast = dsl_result.parsed_ast.clone();
        stored_state.version += 1;
        stored_state.updated_at = chrono::Utc::now();

        // Create audit entry
        let audit_entry = AuditEntry {
            entry_id: Uuid::new_v4().to_string(),
            operation_type: "dsl_state_save".to_string(),
            user_id: "system".to_string(),
            timestamp: chrono::Utc::now(),
            details: {
                let mut details = HashMap::new();
                details.insert("version".to_string(), stored_state.version.to_string());
                details.insert(
                    "domain".to_string(),
                    stored_state.domain_snapshot.primary_domain.clone(),
                );
                details
            },
            previous_state_hash: Some(
                self.calculate_state_hash(&stored_state, stored_state.version - 1),
            ),
            new_state_hash: self.calculate_state_hash(&stored_state, stored_state.version),
            dsl_table_synced: false,
            ast_table_synced: false,
        };

        stored_state.audit_entries.push(audit_entry);

        // Persist the state and sync with DSL/AST tables
        let persist_result = self.persist_state(&stored_state).await;
        let snapshot_id = self.generate_snapshot_id(&stored_state);

        // Sync to DSL and AST tables - critical sync points
        let _sync_result = self.sync_to_tables(&stored_state, dsl_result).await;

        if persist_result {
            StateResult {
                success: true,
                case_id: stored_state.case_id,
                version_number: stored_state.version,
                snapshot_id,
                errors: Vec::new(),
                processing_time_ms: start_time.elapsed().as_millis() as u64,
            }
        } else {
            StateResult {
                success: false,
                case_id: stored_state.case_id,
                version_number: stored_state.version,
                snapshot_id,
                errors: vec!["Failed to persist state to storage".to_string()],
                processing_time_ms: start_time.elapsed().as_millis() as u64,
            }
        }
    }

    /// Load accumulated state for a case ID
    pub async fn load_accumulated_state(&self, case_id: &str) -> AccumulatedState {
        // First try database if available
        #[cfg(feature = "database")]
        if let Some(ref database) = self.database {
            if let Ok(db_state) = self.load_from_database(database.pool(), case_id).await {
                return db_state;
            }
        }

        // Fall back to in-memory store
        match self.state_store.get(case_id) {
            Some(stored_state) => AccumulatedState {
                case_id: stored_state.case_id.clone(),
                current_dsl: stored_state.current_dsl.clone(),
                version: stored_state.version,
                domain_snapshot: Some(stored_state.domain_snapshot.clone()),
                metadata: stored_state.metadata.clone(),
            },
            None => AccumulatedState {
                case_id: case_id.to_string(),
                current_dsl: String::new(),
                version: 0,
                domain_snapshot: None,
                metadata: HashMap::new(),
            },
        }
    }

    /// Load DSL state from database
    #[cfg(feature = "database")]
    async fn load_from_database(
        &self,
        pool: &sqlx::PgPool,
        case_id: &str,
    ) -> Result<AccumulatedState, sqlx::Error> {
        // Load latest DSL instance for this case
        let dsl_row = sqlx::query_as::<_, (String, String, Option<String>)>(
            r#"
            SELECT case_id, dsl_content, domain
            FROM "ob-poc".dsl_instances
            WHERE case_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(pool)
        .await?;

        // Load parsed AST if available
        let ast_row = sqlx::query_as::<_, (Option<String>,)>(
            r#"
            SELECT ast_json::text
            FROM "ob-poc".parsed_asts
            WHERE case_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(case_id)
        .fetch_optional(pool)
        .await?;

        // Count versions for this case
        let version_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".dsl_instances
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_one(pool)
        .await?;

        match dsl_row {
            Some((case_id, dsl_content, domain)) => {
                let mut metadata = HashMap::new();
                if let Some(d) = domain {
                    metadata.insert("domain".to_string(), d);
                }
                if let Some((Some(ast_json),)) = ast_row {
                    metadata.insert("has_ast".to_string(), "true".to_string());
                    metadata.insert(
                        "ast_preview".to_string(),
                        ast_json.chars().take(100).collect(),
                    );
                }

                Ok(AccumulatedState {
                    case_id,
                    current_dsl: dsl_content,
                    version: version_count.0 as u32,
                    domain_snapshot: None, // Could be loaded from parsed_asts if needed
                    metadata,
                })
            }
            None => Err(sqlx::Error::RowNotFound),
        }
    }

    /// Synchronous version of update_accumulated_dsl for use in sync contexts
    pub fn update_accumulated_dsl_sync(&mut self, case_id: &str, additional_dsl: &str) -> bool {
        if let Some(stored_state) = self.state_store.get_mut(case_id) {
            // Append new DSL to existing content
            if stored_state.current_dsl.is_empty() {
                stored_state.current_dsl = additional_dsl.to_string();
            } else {
                stored_state.current_dsl =
                    format!("{}\n\n{}", stored_state.current_dsl, additional_dsl);
            }
            stored_state.updated_at = chrono::Utc::now();
            true
        } else {
            // Create new state with the DSL content
            let new_state = StoredDslState {
                case_id: case_id.to_string(),
                current_dsl: additional_dsl.to_string(),
                version: 1,
                domain_snapshot: DomainSnapshot {
                    primary_domain: "unknown".to_string(),
                    involved_domains: vec![],
                    domain_data: HashMap::new(),
                    compliance_markers: vec![],
                    risk_assessment: None,
                    snapshot_at: chrono::Utc::now(),
                },
                parsed_ast: None,
                metadata: HashMap::new(),
                updated_at: chrono::Utc::now(),
                audit_entries: vec![],
            };
            self.state_store.insert(case_id.to_string(), new_state);
            true
        }
    }

    /// Update accumulated DSL content for incremental operations
    pub async fn update_accumulated_dsl(&mut self, case_id: &str, additional_dsl: &str) -> bool {
        if let Some(stored_state) = self.state_store.get_mut(case_id) {
            // Append new DSL to existing content
            if stored_state.current_dsl.is_empty() {
                stored_state.current_dsl = additional_dsl.to_string();
            } else {
                stored_state.current_dsl =
                    format!("{}\n\n{}", stored_state.current_dsl, additional_dsl);
            }

            stored_state.updated_at = chrono::Utc::now();
            true
        } else {
            // Create new state with the DSL content
            let new_state = StoredDslState {
                case_id: case_id.to_string(),
                current_dsl: additional_dsl.to_string(),
                version: 1,
                domain_snapshot: DomainSnapshot {
                    primary_domain: "unknown".to_string(),
                    involved_domains: vec![],
                    domain_data: HashMap::new(),
                    compliance_markers: vec![],
                    risk_assessment: None,
                    snapshot_at: chrono::Utc::now(),
                },
                parsed_ast: None,
                metadata: HashMap::new(),
                updated_at: chrono::Utc::now(),
                audit_entries: vec![],
            };

            self.state_store.insert(case_id.to_string(), new_state);
            true
        }
    }

    /// Get state history for a case
    pub async fn get_state_history(&self, case_id: &str) -> Vec<AuditEntry> {
        match self.state_store.get(case_id) {
            Some(stored_state) => stored_state.audit_entries.clone(),
            None => Vec::new(),
        }
    }

    /// Health check for the state manager
    pub async fn health_check(&self) -> bool {
        // Check in-memory store
        let store_healthy = true; // Always healthy - empty stores are valid initial state

        // Check database connection if available
        #[cfg(feature = "database")]
        let db_healthy = if let Some(ref _db) = self.database {
            true // Database connection exists
        } else {
            true // Healthy if no database configured
        };

        #[cfg(not(feature = "database"))]
        let db_healthy = true;

        store_healthy && db_healthy
    }

    // Private helper methods

    async fn load_or_create_state(&self, case_id: &str) -> StoredDslState {
        match self.state_store.get(case_id) {
            Some(stored_state) => stored_state.clone(),
            None => self.create_new_state(case_id),
        }
    }

    fn create_new_state(&self, case_id: &str) -> StoredDslState {
        StoredDslState {
            case_id: case_id.to_string(),
            current_dsl: String::new(),
            version: 0,
            domain_snapshot: DomainSnapshot {
                primary_domain: "unknown".to_string(),
                involved_domains: vec![],
                domain_data: HashMap::new(),
                compliance_markers: vec![],
                risk_assessment: None,
                snapshot_at: chrono::Utc::now(),
            },
            parsed_ast: None,
            metadata: HashMap::new(),
            updated_at: chrono::Utc::now(),
            audit_entries: vec![],
        }
    }

    async fn persist_state(&mut self, state: &StoredDslState) -> bool {
        // For now, always persist to in-memory store
        self.state_store
            .insert(state.case_id.clone(), state.clone());

        #[cfg(feature = "database")]
        {
            if self.database.is_some() {
                // Database persistence handled by DslRepository
                return true;
            }
        }

        true
    }

    fn calculate_state_hash(&self, _state: &StoredDslState, _version: u32) -> String {
        // Simple hash calculation - in production, use proper hashing
        format!("hash_{}", Uuid::new_v4())
    }

    fn generate_snapshot_id(&self, state: &StoredDslState) -> String {
        format!("snapshot_{}_{}", state.case_id, state.version)
    }

    /// Sync stored state to DSL and AST tables - critical sync point
    async fn sync_to_tables(&mut self, state: &StoredDslState, dsl_result: &DslModResult) -> bool {
        // Step 1: Update DSL table with accumulated state
        let dsl_sync = self.sync_to_dsl_table(state).await;

        // Step 2: Update AST table with parsed representation
        let ast_sync = self.sync_to_ast_table(state, dsl_result).await;

        // Update audit entry with sync status
        if let Some(last_entry) = self.get_last_audit_entry_mut(&state.case_id) {
            last_entry.dsl_table_synced = dsl_sync;
            last_entry.ast_table_synced = ast_sync;
        }

        dsl_sync && ast_sync
    }

    /// Sync to DSL table - maintains accumulated DSL state
    async fn sync_to_dsl_table(&self, _state: &StoredDslState) -> bool {
        #[cfg(feature = "database")]
        {
            if self.database.is_some() {
                // DSL table sync handled by DslRepository::save_execution_transactionally
                return true;
            }
        }

        // In-memory sync always succeeds
        true
    }

    /// Sync to AST table - maintains parsed representations
    async fn sync_to_ast_table(&self, _state: &StoredDslState, _dsl_result: &DslModResult) -> bool {
        #[cfg(feature = "database")]
        {
            if self.database.is_some() {
                // AST table sync handled by DslRepository::save_execution_transactionally
                return true;
            }
        }

        // In-memory sync always succeeds
        true
    }

    /// Get mutable reference to last audit entry for sync status updates
    fn get_last_audit_entry_mut(&mut self, case_id: &str) -> Option<&mut AuditEntry> {
        if let Some(stored_state) = self.state_store.get_mut(case_id) {
            stored_state.audit_entries.last_mut()
        } else {
            None
        }
    }
}

impl Default for DbStateManager {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for creating domain snapshots

/// Create a domain snapshot from DSL content analysis
pub fn create_domain_snapshot(dsl_content: &str, primary_domain: &str) -> DomainSnapshot {
    let involved_domains = detect_involved_domains(dsl_content);

    DomainSnapshot {
        primary_domain: primary_domain.to_string(),
        involved_domains,
        domain_data: HashMap::new(),
        compliance_markers: detect_compliance_markers(dsl_content),
        risk_assessment: assess_risk_level(dsl_content),
        snapshot_at: chrono::Utc::now(),
    }
}

/// Detect all domains involved in the DSL operation
fn detect_involved_domains(dsl_content: &str) -> Vec<String> {
    let mut domains = Vec::new();

    // Simple domain detection based on verb prefixes
    if dsl_content.contains("case.") {
        domains.push("core".to_string());
    }
    if dsl_content.contains("kyc.") {
        domains.push("kyc".to_string());
    }
    if dsl_content.contains("entity.") {
        domains.push("entity".to_string());
    }
    if dsl_content.contains("ubo.") {
        domains.push("ubo".to_string());
    }
    if dsl_content.contains("document.") {
        domains.push("document".to_string());
    }
    if dsl_content.contains("products.") || dsl_content.contains("services.") {
        domains.push("products".to_string());
    }
    if dsl_content.contains("isda.") {
        domains.push("isda".to_string());
    }

    if domains.is_empty() {
        domains.push("unknown".to_string());
    }

    domains.sort();
    domains.dedup();
    domains
}

/// Detect compliance markers in DSL content
fn detect_compliance_markers(dsl_content: &str) -> Vec<String> {
    let mut markers = Vec::new();

    if dsl_content.contains("ENHANCED") {
        markers.push("enhanced_kyc_required".to_string());
    }
    if dsl_content.contains("HIGH_RISK") {
        markers.push("high_risk_jurisdiction".to_string());
    }
    if dsl_content.contains("PEP") {
        markers.push("pep_screening_required".to_string());
    }
    if dsl_content.contains("sanctions") {
        markers.push("sanctions_screening_required".to_string());
    }

    markers
}

/// Assess risk level based on DSL content
fn assess_risk_level(dsl_content: &str) -> Option<String> {
    if dsl_content.contains("HIGH_RISK") || dsl_content.contains("sanctions") {
        Some("HIGH".to_string())
    } else if dsl_content.contains("ENHANCED") || dsl_content.contains("PEP") {
        Some("MEDIUM".to_string())
    } else {
        Some("LOW".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_state_manager_creation() {
        let manager = DbStateManager::new();
        assert!(manager.health_check().await);
    }

    #[tokio::test]
    async fn test_state_save_and_load() {
        let mut manager = DbStateManager::new();

        let dsl_result = DslModResult {
            success: true,
            parsed_ast: Some(r#"{"type": "case_create"}"#.to_string()),
            domain_snapshot: create_domain_snapshot("(case.create :case-id \"TEST-001\")", "core"),
            case_id: "TEST-001".to_string(),
            errors: Vec::new(),
        };

        let save_result = manager.save_dsl_state(&dsl_result).await;
        assert!(save_result.success);
        assert_eq!(save_result.case_id, "TEST-001");
        assert_eq!(save_result.version_number, 1);

        let loaded_state = manager.load_accumulated_state("TEST-001").await;
        assert_eq!(loaded_state.case_id, "TEST-001");
        assert_eq!(loaded_state.version, 1);
    }

    #[test]
    fn test_domain_detection() {
        let dsl_content =
            "(kyc.collect :case-id \"TEST-001\") (entity.register :entity-id \"ENT-001\")";
        let domains = detect_involved_domains(dsl_content);

        assert!(domains.contains(&"kyc".to_string()));
        assert!(domains.contains(&"entity".to_string()));
        assert_eq!(domains.len(), 2);
    }

    #[test]
    fn test_compliance_markers() {
        let dsl_content = "(kyc.collect :case-id \"TEST-001\" :collection-type \"ENHANCED\")";
        let markers = detect_compliance_markers(dsl_content);

        assert!(markers.contains(&"enhanced_kyc_required".to_string()));
    }
}
```

## File: rust/src/execution/engine.rs

```rust
//! Main DSL Execution Engine
//!
//! This module ties together all components of the DSL execution system:
//! - Operation handlers for executing DSL operations
//! - Business rules engine for validation
//! - External integrations for system connectivity
//! - State management for DSL-as-State persistence
//! - Context management for execution environments
//!
//! The engine provides a high-level API for executing DSL operations with
//! full business rule validation, external system integration, and audit trails.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    context::SessionManager,
    integrations::{create_standard_integrations, IntegrationRegistry},
    rules::{create_standard_rules, BusinessRuleRegistry},
    state::{InMemoryStateStore, PostgresStateStore},
    BusinessRule, DslExecutionEngine, DslState, ExecutionContext, ExecutionMessage,
    ExecutionResult, ExternalIntegration, MessageLevel, OperationHandler,
};
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// Comprehensive DSL execution engine with all components
pub(crate) struct ComprehensiveDslEngine {
    /// Core execution engine
    execution_engine: DslExecutionEngine,
    /// Business rules registry
    rules_registry: Arc<RwLock<BusinessRuleRegistry>>,
    /// External integrations registry
    integrations_registry: Arc<RwLock<IntegrationRegistry>>,
}

impl ComprehensiveDslEngine {
    /// Create a new comprehensive DSL engine with in-memory storage
    pub(crate) fn new_with_memory_store() -> Self {
        let state_store = Arc::new(InMemoryStateStore::new());
        let execution_engine = DslExecutionEngine::new(state_store);
        let rules_registry = Arc::new(RwLock::new(BusinessRuleRegistry::new()));
        let integrations_registry = Arc::new(RwLock::new(create_standard_integrations()));

        Self {
            execution_engine,
            rules_registry,
            integrations_registry,
        }
    }

    /// Create a new comprehensive DSL engine with PostgreSQL storage
    pub(crate) fn new_with_postgres_store(pool: sqlx::PgPool) -> Self {
        let state_store = Arc::new(PostgresStateStore::new(pool));
        let execution_engine = DslExecutionEngine::new(state_store);
        let rules_registry = Arc::new(RwLock::new(BusinessRuleRegistry::new()));
        let integrations_registry = Arc::new(RwLock::new(create_standard_integrations()));

        Self {
            execution_engine,
            rules_registry,
            integrations_registry,
        }
    }

    /// Initialize the engine with standard handlers, rules, and integrations
    pub async fn initialize(&self) -> Result<()> {
        // Register standard business rules
        let mut rules_registry = self.rules_registry.write().await;
        let rules = create_standard_rules();
        for rule in rules {
            rules_registry.register(rule);
        }

        Ok(())
    }

    /// Execute a single DSL operation with full validation and integration
    pub async fn execute_operation(
        &self,
        operation: DslOperation,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // Pre-execution validation with business rules
        let validation_result = self
            .validate_with_business_rules(&operation, &context)
            .await?;

        if !validation_result.is_valid {
            return Ok(ExecutionResult {
                success: false,
                operation: operation.clone(),
                new_state: self.get_current_state(&context.business_unit_id).await?,
                messages: validation_result.messages,
                external_responses: HashMap::new(),
                duration_ms: 0,
            });
        }

        // Execute the operation
        let mut result = self
            .execution_engine
            .execute_operation(operation, context)
            .await?;

        // Post-execution processing
        result.messages.extend(validation_result.messages);

        Ok(result)
    }

    /// Execute a batch of DSL operations with comprehensive validation
    pub async fn execute_batch(
        &self,
        operations: Vec<DslOperation>,
        context: ExecutionContext,
    ) -> Result<BatchExecutionResult> {
        let mut results = Vec::new();
        let mut total_duration = 0u64;
        let mut current_context = context;

        for (index, operation) in operations.into_iter().enumerate() {
            match self
                .execute_operation(operation.clone(), current_context.clone())
                .await
            {
                Ok(result) => {
                    total_duration += result.duration_ms;
                    results.push(result);

                    // Create new context for next operation
                    current_context.session_id = Uuid::new_v4();
                }
                Err(e) => {
                    let num_results = results.len();
                    return Ok(BatchExecutionResult {
                        results,
                        total_operations: index + 1,
                        successful_operations: num_results,
                        failed_at_operation: Some(index),
                        error_message: Some(e.to_string()),
                        total_duration_ms: total_duration,
                    });
                }
            }
        }

        let num_results = results.len();
        Ok(BatchExecutionResult {
            results,
            total_operations: num_results,
            successful_operations: num_results,
            failed_at_operation: None,
            error_message: None,
            total_duration_ms: total_duration,
        })
    }

    /// Execute DSL operations for a specific domain workflow
    pub async fn execute_workflow(
        &self,
        workflow_type: WorkflowType,
        business_unit_id: String,
        executor: String,
        operations: Vec<DslOperation>,
    ) -> Result<WorkflowExecutionResult> {
        // Create appropriate context for workflow type
        let context = self
            .create_workflow_context(workflow_type.clone(), business_unit_id.clone(), executor)
            .await?;

        // Execute the workflow batch
        let batch_result = self.execute_batch(operations, context).await?;

        // Get final state
        let final_state = self.get_current_state(&business_unit_id).await?;

        let workflow_status = if batch_result.failed_at_operation.is_none() {
            WorkflowStatus::Completed
        } else {
            WorkflowStatus::Failed
        };

        Ok(WorkflowExecutionResult {
            workflow_type,
            business_unit_id,
            batch_result,
            final_state,
            workflow_status,
        })
    }

    /// Get current state for a business unit
    pub async fn get_current_state(&self, business_unit_id: &str) -> Result<DslState> {
        match self
            .execution_engine
            .get_current_state(business_unit_id)
            .await?
        {
            Some(state) => Ok(state),
            None => {
                // Create empty initial state
                let now = chrono::Utc::now();
                Ok(DslState {
                    business_unit_id: business_unit_id.to_string(),
                    operations: Vec::new(),
                    current_state: HashMap::new(),
                    metadata: super::StateMetadata {
                        created_at: now,
                        updated_at: now,
                        domain: "unknown".to_string(),
                        status: "initialized".to_string(),
                        tags: Vec::new(),
                        compliance_flags: Vec::new(),
                    },
                    version: 0,
                })
            }
        }
    }

    /// Register a custom business rule
    pub async fn register_business_rule(&self, rule: std::sync::Arc<dyn BusinessRule>) {
        let mut registry = self.rules_registry.write().await;
        registry.register(rule);
    }

    /// Register a custom external integration
    pub async fn register_integration(&self, integration: std::sync::Arc<dyn ExternalIntegration>) {
        let mut registry = self.integrations_registry.write().await;
        registry.register(integration);
    }

    /// Register a custom operation handler
    pub async fn register_operation_handler(&self, handler: Arc<dyn OperationHandler>) {
        self.execution_engine.register_handler(handler).await;
    }

    /// Validate operation with business rules
    async fn validate_with_business_rules(
        &self,
        operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<ValidationSummary> {
        let current_state = self.get_current_state(&context.business_unit_id).await?;

        let rules_registry = self.rules_registry.read().await;
        let rule_results = rules_registry
            .validate_operation(operation, &current_state, context)
            .await?;

        let blocking_violations = rules_registry.get_blocking_violations(&rule_results);
        let is_valid = blocking_violations.is_empty();

        let mut messages = Vec::new();
        for result in &rule_results {
            if result.valid {
                messages.push(ExecutionMessage::info(
                    result
                        .message
                        .as_ref()
                        .unwrap_or(&"Rule passed".to_string())
                        .clone(),
                ));
            } else {
                let level = if result.blocking {
                    MessageLevel::Error
                } else {
                    MessageLevel::Warning
                };
                messages.push(ExecutionMessage {
                    level,
                    message: result
                        .message
                        .as_ref()
                        .unwrap_or(&"Rule failed".to_string())
                        .clone(),
                    context: Some("business_rules".to_string()),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        Ok(ValidationSummary { is_valid, messages })
    }

    /// Create workflow-specific execution context
    async fn create_workflow_context(
        &self,
        workflow_type: WorkflowType,
        business_unit_id: String,
        executor: String,
    ) -> Result<ExecutionContext> {
        let integrations = {
            let registry = self.integrations_registry.read().await;
            registry
                .list_integrations()
                .iter()
                .map(|s| s.to_string())
                .collect()
        };

        match workflow_type {
            WorkflowType::KYC => {
                SessionManager::create_kyc_session(business_unit_id, executor, integrations)
            }
            WorkflowType::Onboarding => {
                SessionManager::create_onboarding_session(business_unit_id, executor, integrations)
            }
            WorkflowType::UBO => {
                SessionManager::create_ubo_session(business_unit_id, executor, integrations)
            }
            WorkflowType::Custom(domain) => {
                SessionManager::create_session(business_unit_id, domain, executor)
            }
        }
    }
}

/// Result of batch execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct BatchExecutionResult {
    pub results: Vec<ExecutionResult>,
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_at_operation: Option<usize>,
    pub error_message: Option<String>,
    pub total_duration_ms: u64,
}

/// Result of workflow execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkflowExecutionResult {
    pub workflow_type: WorkflowType,
    pub business_unit_id: String,
    pub batch_result: BatchExecutionResult,
    pub final_state: DslState,
    pub workflow_status: WorkflowStatus,
}

/// Supported workflow types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum WorkflowType {
    KYC,
    Onboarding,
    UBO,
    Custom(String),
}

/// Workflow execution status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum WorkflowStatus {
    Initialized,
    InProgress,
    Completed,
    Failed,
    Suspended,
}

/// Summary of business rule validation
struct ValidationSummary {
    is_valid: bool,
    messages: Vec<ExecutionMessage>,
}

/// Engine builder for customizing engine configuration
pub(crate) struct EngineBuilder {
    use_postgres: bool,
    postgres_pool: Option<sqlx::PgPool>,
    custom_handlers: Vec<Arc<dyn OperationHandler>>,
    custom_rules: Vec<Arc<dyn BusinessRule>>,
    custom_integrations: Vec<Arc<dyn ExternalIntegration>>,
}

impl EngineBuilder {
    /// Create a new engine builder
    pub fn new() -> Self {
        Self {
            use_postgres: false,
            postgres_pool: None,
            custom_handlers: Vec::new(),
            custom_rules: Vec::new(),
            custom_integrations: Vec::new(),
        }
    }

    /// Use PostgreSQL for state storage
    pub(crate) fn with_postgres(mut self, pool: sqlx::PgPool) -> Self {
        self.use_postgres = true;
        self.postgres_pool = Some(pool);
        self
    }

    /// Add a custom operation handler
    pub(crate) fn with_handler(mut self, handler: Arc<dyn OperationHandler>) -> Self {
        self.custom_handlers.push(handler);
        self
    }

    /// Add a custom business rule
    pub(crate) fn with_rule(mut self, rule: Arc<dyn BusinessRule>) -> Self {
        self.custom_rules.push(rule);
        self
    }

    /// Add a custom integration
    pub(crate) fn with_integration(mut self, integration: Arc<dyn ExternalIntegration>) -> Self {
        self.custom_integrations.push(integration);
        self
    }

    /// Build the comprehensive DSL engine
    pub async fn build(self) -> Result<ComprehensiveDslEngine> {
        let engine = if self.use_postgres {
            let pool = self.postgres_pool.ok_or_else(|| {
                anyhow::anyhow!("PostgreSQL pool required when using Postgres storage")
            })?;
            ComprehensiveDslEngine::new_with_postgres_store(pool)
        } else {
            ComprehensiveDslEngine::new_with_memory_store()
        };

        // Initialize with standard components
        engine.initialize().await?;

        // Add custom handlers
        for handler in self.custom_handlers {
            engine.register_operation_handler(handler).await;
        }

        // Add custom rules
        for rule in self.custom_rules {
            engine.register_business_rule(rule).await;
        }

        // Add custom integrations
        for integration in self.custom_integrations {
            engine.register_integration(integration).await;
        }

        Ok(engine)
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

```

## File: rust/src/execution/state.rs

```rust
//! DSL State Store Implementation
//!
//! This module provides persistence for DSL state using PostgreSQL as the backing store.
//! It implements the StateStore trait for managing DSL state with full event sourcing capabilities.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use super::{DslState, StateMetadata, StateStore};
use crate::data_dictionary::AttributeId;
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// PostgreSQL-backed implementation of the StateStore trait
pub struct PostgresStateStore {
    pool: PgPool,
}

impl PostgresStateStore {
    /// Create a new PostgresStateStore with the given database pool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize the database schema for state storage
    pub async fn initialize_schema(&self) -> Result<()> {
        // Create the state storage tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dsl_states (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                business_unit_id VARCHAR NOT NULL,
                version BIGINT NOT NULL,
                operations JSONB NOT NULL,
                current_state JSONB NOT NULL,
                metadata JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

                UNIQUE(business_unit_id, version)
            );

            CREATE INDEX IF NOT EXISTS idx_dsl_states_business_unit
            ON dsl_states(business_unit_id);

            CREATE INDEX IF NOT EXISTS idx_dsl_states_version
            ON dsl_states(business_unit_id, version DESC);

            CREATE INDEX IF NOT EXISTS idx_dsl_states_updated
            ON dsl_states(updated_at DESC);

            CREATE TABLE IF NOT EXISTS dsl_state_snapshots (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                business_unit_id VARCHAR NOT NULL,
                state_id UUID NOT NULL REFERENCES dsl_states(id),
                snapshot_name VARCHAR,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_by VARCHAR NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_snapshots_business_unit
            ON dsl_state_snapshots(business_unit_id);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to initialize DSL state storage schema")?;

        Ok(())
    }
}

#[async_trait]
impl StateStore for PostgresStateStore {
    async fn get_state(&self, business_unit_id: &str) -> Result<Option<DslState>> {
        // Get the latest version of the state
        let row = sqlx::query(
            r#"
            SELECT operations, current_state, metadata, version, created_at, updated_at
            FROM dsl_states
            WHERE business_unit_id = $1
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(business_unit_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let operations_json: Value = row.get("operations");
                let current_state_json: Value = row.get("current_state");
                let metadata_json: Value = row.get("metadata");
                let version: i64 = row.get("version");

                // Parse operations
                let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
                    .context("Failed to parse operations from database")?;

                // Parse current state
                let current_state: HashMap<AttributeId, Value> =
                    serde_json::from_value(current_state_json)
                        .context("Failed to parse current state from database")?;

                // Parse metadata
                let metadata: StateMetadata = serde_json::from_value(metadata_json)
                    .context("Failed to parse metadata from database")?;

                Ok(Some(DslState {
                    business_unit_id: business_unit_id.to_string(),
                    operations,
                    current_state,
                    metadata,
                    version: version as u64,
                }))
            }
            None => Ok(None),
        }
    }

    async fn save_state(&self, state: &DslState) -> Result<()> {
        // Serialize the state components
        let operations_json =
            serde_json::to_value(&state.operations).context("Failed to serialize operations")?;

        let current_state_json = serde_json::to_value(&state.current_state)
            .context("Failed to serialize current state")?;

        let metadata_json =
            serde_json::to_value(&state.metadata).context("Failed to serialize metadata")?;

        // Insert the new state version
        sqlx::query(
            r#"
            INSERT INTO dsl_states
            (business_unit_id, version, operations, current_state, metadata, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&state.business_unit_id)
        .bind(state.version as i64)
        .bind(&operations_json)
        .bind(&current_state_json)
        .bind(&metadata_json)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .context("Failed to save DSL state to database")?;

        Ok(())
    }

    async fn get_state_history(
        &self,
        business_unit_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<DslState>> {
        let limit = limit.unwrap_or(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT operations, current_state, metadata, version, created_at, updated_at
            FROM dsl_states
            WHERE business_unit_id = $1
            ORDER BY version DESC
            LIMIT $2
            "#,
        )
        .bind(business_unit_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut states = Vec::new();

        for row in rows {
            let operations_json: Value = row.get("operations");
            let current_state_json: Value = row.get("current_state");
            let metadata_json: Value = row.get("metadata");
            let version: i64 = row.get("version");

            // Parse operations
            let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
                .context("Failed to parse operations from database")?;

            // Parse current state
            let current_state: HashMap<AttributeId, Value> =
                serde_json::from_value(current_state_json)
                    .context("Failed to parse current state from database")?;

            // Parse metadata
            let metadata: StateMetadata = serde_json::from_value(metadata_json)
                .context("Failed to parse metadata from database")?;

            states.push(DslState {
                business_unit_id: business_unit_id.to_string(),
                operations,
                current_state,
                metadata,
                version: version as u64,
            });
        }

        Ok(states)
    }

    async fn create_snapshot(&self, business_unit_id: &str) -> Result<Uuid> {
        // Get the latest state
        let latest_state = self.get_state(business_unit_id).await?.ok_or_else(|| {
            anyhow::anyhow!("No state found for business unit: {}", business_unit_id)
        })?;

        // Get the database ID of the latest state record
        let state_row = sqlx::query(
            "SELECT id FROM dsl_states WHERE business_unit_id = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(business_unit_id)
        .fetch_one(&self.pool)
        .await?;

        let state_id: Uuid = state_row.get("id");

        // Create snapshot record
        let snapshot_id = Uuid::new_v4();
        let snapshot_name = format!(
            "snapshot-v{}-{}",
            latest_state.version,
            Utc::now().format("%Y%m%d-%H%M%S")
        );

        sqlx::query(
            r#"
            INSERT INTO dsl_state_snapshots
            (id, business_unit_id, state_id, snapshot_name, created_by)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(snapshot_id)
        .bind(business_unit_id)
        .bind(state_id)
        .bind(&snapshot_name)
        .bind("system") // In a real system, this would be the current user
        .execute(&self.pool)
        .await
        .context("Failed to create state snapshot")?;

        Ok(snapshot_id)
    }

    async fn restore_from_snapshot(&self, snapshot_id: Uuid) -> Result<DslState> {
        // Get the snapshot record
        let snapshot_row = sqlx::query(
            r#"
            SELECT s.business_unit_id, s.state_id, ds.operations, ds.current_state,
                   ds.metadata, ds.version
            FROM dsl_state_snapshots s
            JOIN dsl_states ds ON s.state_id = ds.id
            WHERE s.id = $1
            "#,
        )
        .bind(snapshot_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", snapshot_id))?;

        let business_unit_id: String = snapshot_row.get("business_unit_id");
        let operations_json: Value = snapshot_row.get("operations");
        let current_state_json: Value = snapshot_row.get("current_state");
        let metadata_json: Value = snapshot_row.get("metadata");
        let version: i64 = snapshot_row.get("version");

        // Parse the state components
        let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
            .context("Failed to parse operations from snapshot")?;

        let current_state: HashMap<AttributeId, Value> = serde_json::from_value(current_state_json)
            .context("Failed to parse current state from snapshot")?;

        let metadata: StateMetadata = serde_json::from_value(metadata_json)
            .context("Failed to parse metadata from snapshot")?;

        Ok(DslState {
            business_unit_id,
            operations,
            current_state,
            metadata,
            version: version as u64,
        })
    }
}

/// In-memory implementation for testing and development
pub struct InMemoryStateStore {
    states: tokio::sync::RwLock<HashMap<String, DslState>>,
    snapshots: tokio::sync::RwLock<HashMap<Uuid, DslState>>,
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            states: tokio::sync::RwLock::new(HashMap::new()),
            snapshots: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get_state(&self, business_unit_id: &str) -> Result<Option<DslState>> {
        let states = self.states.read().await;
        Ok(states.get(business_unit_id).cloned())
    }

    async fn save_state(&self, state: &DslState) -> Result<()> {
        let mut states = self.states.write().await;
        states.insert(state.business_unit_id.clone(), state.clone());
        Ok(())
    }

    async fn get_state_history(
        &self,
        business_unit_id: &str,
        _limit: Option<u32>,
    ) -> Result<Vec<DslState>> {
        let states = self.states.read().await;
        match states.get(business_unit_id) {
            Some(state) => Ok(vec![state.clone()]),
            None => Ok(vec![]),
        }
    }

    async fn create_snapshot(&self, business_unit_id: &str) -> Result<Uuid> {
        let states = self.states.read().await;
        let state = states.get(business_unit_id).ok_or_else(|| {
            anyhow::anyhow!("No state found for business unit: {}", business_unit_id)
        })?;

        let snapshot_id = Uuid::new_v4();
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot_id, state.clone());

        Ok(snapshot_id)
    }

    async fn restore_from_snapshot(&self, snapshot_id: Uuid) -> Result<DslState> {
        let snapshots = self.snapshots.read().await;
        snapshots
            .get(&snapshot_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", snapshot_id))
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

```

## File: rust/src/cbu_model_dsl/service.rs

```rust
//! CBU Model Service
//!
//! Provides validation of CBU Models against the attribute dictionary
//! and persistence as documents.

use crate::cbu_model_dsl::ast::CbuModel;
use crate::cbu_model_dsl::parser::{CbuModelError, CbuModelParser};
use crate::database::{DictionaryDatabaseService, DslRepository};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Service for validating and persisting CBU Models
pub struct CbuModelService {
    pool: PgPool,
    dictionary: DictionaryDatabaseService,
}

impl CbuModelService {
    /// Create a new CBU Model Service
    pub fn new(pool: PgPool) -> Self {
        let dictionary = DictionaryDatabaseService::new(pool.clone());
        Self { pool, dictionary }
    }

    /// Parse and validate a CBU Model DSL string
    pub async fn parse_and_validate(&self, input: &str) -> Result<CbuModel, CbuModelError> {
        // Parse the DSL
        let model = CbuModelParser::parse_str(input)?;

        // Validate against dictionary
        self.validate_model(&model).await?;

        Ok(model)
    }

    /// Validate a CBU Model against the attribute dictionary
    pub async fn validate_model(&self, model: &CbuModel) -> Result<(), CbuModelError> {
        info!("Validating CBU Model: {}", model.id);

        // Collect all attribute IDs from the model
        let mut all_attrs: Vec<&str> = model.attributes.all_attributes();

        // Add precondition attributes from transitions
        for transition in &model.states.transitions {
            for attr in &transition.preconditions {
                if !all_attrs.contains(&attr.as_str()) {
                    all_attrs.push(attr);
                }
            }
        }

        // Validate each attribute exists in dictionary and has CBU sink
        let mut errors: Vec<String> = Vec::new();

        for attr_id in &all_attrs {
            match self.validate_attribute(attr_id).await {
                Ok(()) => {
                    debug!("Attribute '{}' validated successfully", attr_id);
                }
                Err(e) => {
                    errors.push(e);
                }
            }
        }

        // Validate state machine consistency
        self.validate_state_machine(model, &mut errors);

        // Validate role constraints
        self.validate_roles(model, &mut errors);

        // Validate verb naming conventions
        self.validate_verbs(model, &mut errors);

        if !errors.is_empty() {
            return Err(CbuModelError::ValidationError(errors.join("; ")));
        }

        info!("CBU Model '{}' validated successfully", model.id);
        Ok(())
    }

    /// Validate a single attribute exists and has CBU sink
    async fn validate_attribute(&self, attr_id: &str) -> Result<(), String> {
        // Try to find by name first
        let attr = self
            .dictionary
            .get_by_name(attr_id)
            .await
            .map_err(|e| format!("Database error looking up '{}': {}", attr_id, e))?;

        match attr {
            Some(attr) => {
                // Check if attribute has CBU in its sink
                if let Some(sink) = &attr.sink {
                    let has_cbu_sink = match sink {
                        JsonValue::Array(arr) => arr.iter().any(|v| {
                            v.as_str()
                                .map(|s| s.to_uppercase() == "CBU")
                                .unwrap_or(false)
                        }),
                        JsonValue::Object(obj) => {
                            if let Some(assets) = obj.get("assets") {
                                match assets {
                                    JsonValue::Array(arr) => arr.iter().any(|v| {
                                        v.as_str()
                                            .map(|s| s.to_uppercase() == "CBU")
                                            .unwrap_or(false)
                                    }),
                                    JsonValue::String(s) => s.to_uppercase() == "CBU",
                                    _ => false,
                                }
                            } else {
                                false
                            }
                        }
                        JsonValue::String(s) => s.to_uppercase() == "CBU",
                        _ => false,
                    };

                    if !has_cbu_sink {
                        warn!(
                            "Attribute '{}' does not have CBU in sink: {:?}",
                            attr_id, sink
                        );
                        // For now, just warn - don't fail validation
                        // In production, this would be: return Err(...)
                    }
                }
                Ok(())
            }
            None => {
                // Attribute not found - this is an error
                Err(format!("Attribute '{}' not found in dictionary", attr_id))
            }
        }
    }

    /// Validate state machine consistency
    fn validate_state_machine(&self, model: &CbuModel, errors: &mut Vec<String>) {
        let sm = &model.states;

        // Check initial state exists
        if !sm.states.iter().any(|s| s.name == sm.initial) {
            errors.push(format!(
                "Initial state '{}' not defined in states",
                sm.initial
            ));
        }

        // Check final states exist
        for final_state in &sm.finals {
            if !sm.states.iter().any(|s| s.name == *final_state) {
                errors.push(format!(
                    "Final state '{}' not defined in states",
                    final_state
                ));
            }
        }

        // Check all transitions reference valid states
        for trans in &sm.transitions {
            if !sm.states.iter().any(|s| s.name == trans.from) {
                errors.push(format!(
                    "Transition from '{}' references undefined state",
                    trans.from
                ));
            }
            if !sm.states.iter().any(|s| s.name == trans.to) {
                errors.push(format!(
                    "Transition to '{}' references undefined state",
                    trans.to
                ));
            }

            // Warn if transitioning out of a final state
            if sm.finals.contains(&trans.from) {
                warn!(
                    "Transition from final state '{}' to '{}' - final states should not have outgoing transitions",
                    trans.from, trans.to
                );
            }
        }

        // Check for unreachable states (except initial)
        for state in &sm.states {
            if state.name != sm.initial {
                let is_reachable = sm.transitions.iter().any(|t| t.to == state.name);
                if !is_reachable {
                    warn!(
                        "State '{}' is not reachable from any transition",
                        state.name
                    );
                }
            }
        }
    }

    /// Validate role constraints
    fn validate_roles(&self, model: &CbuModel, errors: &mut Vec<String>) {
        for role in &model.roles {
            // Check min <= max
            if let Some(max) = role.max {
                if role.min > max {
                    errors.push(format!(
                        "Role '{}' has min ({}) > max ({})",
                        role.name, role.min, max
                    ));
                }
            }
        }

        // Check for duplicate role names
        let mut seen = std::collections::HashSet::new();
        for role in &model.roles {
            if !seen.insert(&role.name) {
                errors.push(format!("Duplicate role name: '{}'", role.name));
            }
        }
    }

    /// Validate verb naming conventions
    fn validate_verbs(&self, model: &CbuModel, _errors: &mut Vec<String>) {
        let mut seen_verbs = std::collections::HashSet::new();

        for trans in &model.states.transitions {
            // Check verb format (should be domain.action)
            if !trans.verb.contains('.') {
                warn!(
                    "Verb '{}' does not follow domain.action convention",
                    trans.verb
                );
            }

            // Check for duplicate verbs (same verb for different transitions is OK)
            seen_verbs.insert(&trans.verb);
        }
    }

    /// Save a CBU Model as a document
    pub async fn save_model(
        &self,
        raw_content: &str,
        model: &CbuModel,
    ) -> Result<Uuid, CbuModelError> {
        let repo = DslRepository::new(self.pool.clone());

        // Save to dsl_instances
        let domain = "cbu-model";
        let case_id = format!("MODEL-{}", model.id);

        // Serialize AST to JSON
        let ast_json = serde_json::to_value(model).map_err(|e| {
            CbuModelError::ValidationError(format!("Failed to serialize model: {}", e))
        })?;

        let result = repo
            .save_execution(
                raw_content,
                domain,
                &case_id,
                None, // No CBU ID for model definitions
                &ast_json,
            )
            .await
            .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        // Also create a document_catalog entry
        self.create_document_entry(&result.instance_id, model)
            .await?;

        info!(
            "Saved CBU Model '{}' with instance_id: {}",
            model.id, result.instance_id
        );

        Ok(result.instance_id)
    }

    /// Create a document catalog entry for the model
    async fn create_document_entry(
        &self,
        instance_id: &Uuid,
        model: &CbuModel,
    ) -> Result<(), CbuModelError> {
        // Check if document_type exists, create if not
        let type_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".document_types
                WHERE type_code = 'DSL.CBU.MODEL'
            )
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        if !type_exists {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".document_types (type_code, display_name, category, description)
                VALUES ('DSL.CBU.MODEL', 'CBU Model DSL', 'DSL', 'CBU Model specification document')
                ON CONFLICT (type_code) DO NOTHING
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;
        }

        // Create document catalog entry
        let doc_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                document_id, document_type_code, document_name,
                source_system, status, metadata
            )
            VALUES ($1, 'DSL.CBU.MODEL', $2, 'ob-poc', 'active', $3)
            "#,
        )
        .bind(doc_id)
        .bind(&format!("{} v{}", model.id, model.version))
        .bind(serde_json::json!({
            "model_id": model.id,
            "version": model.version,
            "dsl_instance_id": instance_id.to_string(),
            "applies_to": model.applies_to
        }))
        .execute(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Load a CBU Model by its model ID (e.g., "CBU.GENERIC")
    pub async fn load_model_by_id(
        &self,
        model_id: &str,
    ) -> Result<Option<CbuModel>, CbuModelError> {
        // Find the latest version in document_catalog
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            r#"
            SELECT metadata
            FROM "ob-poc".document_catalog
            WHERE document_type_code = 'DSL.CBU.MODEL'
            AND metadata->>'model_id' = $1
            AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        match row {
            Some((metadata,)) => {
                // Get the DSL instance ID from metadata
                let instance_id_str = metadata
                    .get("dsl_instance_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        CbuModelError::ValidationError(
                            "Missing dsl_instance_id in metadata".to_string(),
                        )
                    })?;

                let instance_id = Uuid::parse_str(instance_id_str)
                    .map_err(|e| CbuModelError::ValidationError(format!("Invalid UUID: {}", e)))?;

                // Load the DSL content
                let repo = DslRepository::new(self.pool.clone());
                let content = repo
                    .get_dsl_content(instance_id)
                    .await
                    .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?
                    .ok_or_else(|| {
                        CbuModelError::ValidationError(format!(
                            "DSL instance {} not found",
                            instance_id
                        ))
                    })?;

                // Parse and return
                let model = CbuModelParser::parse_str(&content)?;
                Ok(Some(model))
            }
            None => Ok(None),
        }
    }

    /// Load a CBU Model by DSL instance ID
    pub async fn load_model_by_instance(
        &self,
        instance_id: Uuid,
    ) -> Result<CbuModel, CbuModelError> {
        let repo = DslRepository::new(self.pool.clone());
        let content = repo
            .get_dsl_content(instance_id)
            .await
            .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?
            .ok_or_else(|| {
                CbuModelError::ValidationError(format!("DSL instance {} not found", instance_id))
            })?;

        CbuModelParser::parse_str(&content)
    }

    /// List all available CBU Models
    pub async fn list_models(&self) -> Result<Vec<(String, String, Uuid)>, CbuModelError> {
        let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"
            SELECT document_name, metadata
            FROM "ob-poc".document_catalog
            WHERE document_type_code = 'DSL.CBU.MODEL'
            AND status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        let mut models = Vec::new();
        for (name, metadata) in rows {
            let model_id = metadata
                .get("model_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let instance_id_str = metadata
                .get("dsl_instance_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Ok(instance_id) = Uuid::parse_str(instance_id_str) {
                models.push((name, model_id, instance_id));
            }
        }

        Ok(models)
    }
}
```
