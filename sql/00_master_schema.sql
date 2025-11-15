--
-- PostgreSQL database dump
--

\restrict N5ML2PEaakwAHI2DSinDAkNMCBQ5pX8rIoIO9I2ZHcP8fhow9jQTRyovL6NuNNO

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
-- Name: ob-poc; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA "ob-poc";


--
-- Name: cleanup_demo_data(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".cleanup_demo_data() RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    DELETE FROM "ob-poc".attribute_values WHERE entity_id LIKE 'DEMO-%' OR entity_id LIKE 'CASE-%';
    DELETE FROM "ob-poc".parsed_asts WHERE case_id LIKE 'CASE-%';
    DELETE FROM "ob-poc".dsl_instances WHERE case_id LIKE 'CASE-%';
    DELETE FROM "ob-poc".ubo_registry WHERE entity_id LIKE 'DEMO-%';
    DELETE FROM "ob-poc".entities WHERE entity_id LIKE 'DEMO-%';
    DELETE FROM "ob-poc".document_catalog WHERE case_id LIKE 'CASE-%';

    RAISE NOTICE 'Demo data cleanup completed';
END;
$$;


--
-- Name: get_attribute_value(uuid, text); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".get_attribute_value(p_entity_id uuid, p_attribute_id text) RETURNS TABLE(value_text text, value_number numeric, value_integer bigint, value_boolean boolean, value_date date, value_datetime timestamp with time zone, value_json jsonb)
    LANGUAGE sql STABLE
    AS $$
    SELECT
        value_text,
        value_number,
        value_integer,
        value_boolean,
        value_date,
        value_datetime,
        value_json
    FROM "ob-poc".attribute_values_typed
    WHERE entity_id = p_entity_id
      AND attribute_id = p_attribute_id
      AND effective_to IS NULL
    ORDER BY effective_from DESC
    LIMIT 1;
$$;


--
-- Name: get_next_version_number(character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".get_next_version_number(domain_name_param character varying) RETURNS integer
    LANGUAGE plpgsql
    AS $$
DECLARE
    next_version INTEGER;
BEGIN
    SELECT COALESCE(MAX(dv.version_number), 0) + 1
    INTO next_version
    FROM "ob-poc".dsl_domains d
    JOIN "ob-poc".dsl_versions dv ON d.domain_id = dv.domain_id
    WHERE d.domain_name = domain_name_param;

    RETURN next_version;
END;
$$;


--
-- Name: invalidate_ast_cache(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".invalidate_ast_cache() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- If DSL source code changed, invalidate the AST
    IF OLD.dsl_source_code IS DISTINCT FROM NEW.dsl_source_code THEN
        UPDATE "ob-poc".parsed_asts
        SET invalidated_at = now()
        WHERE version_id = NEW.version_id;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: resolve_semantic_to_uuid(text); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".resolve_semantic_to_uuid(semantic_id text) RETURNS uuid
    LANGUAGE sql STABLE
    AS $$
    SELECT uuid FROM "ob-poc".attribute_registry WHERE id = semantic_id;
$$;


--
-- Name: resolve_uuid_to_semantic(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".resolve_uuid_to_semantic(attr_uuid uuid) RETURNS text
    LANGUAGE sql STABLE
    AS $$
    SELECT id FROM "ob-poc".attribute_registry WHERE uuid = attr_uuid;
$$;


--
-- Name: set_attribute_value(uuid, text, text, numeric, bigint, boolean, date, timestamp with time zone, jsonb, text); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".set_attribute_value(p_entity_id uuid, p_attribute_id text, p_value_text text DEFAULT NULL::text, p_value_number numeric DEFAULT NULL::numeric, p_value_integer bigint DEFAULT NULL::bigint, p_value_boolean boolean DEFAULT NULL::boolean, p_value_date date DEFAULT NULL::date, p_value_datetime timestamp with time zone DEFAULT NULL::timestamp with time zone, p_value_json jsonb DEFAULT NULL::jsonb, p_created_by text DEFAULT 'system'::text) RETURNS bigint
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_new_id BIGINT;
BEGIN
    -- Expire any existing active values
    UPDATE "ob-poc".attribute_values_typed
    SET effective_to = NOW() AT TIME ZONE 'utc'
    WHERE entity_id = p_entity_id
      AND attribute_id = p_attribute_id
      AND effective_to IS NULL;

    -- Insert new value
    INSERT INTO "ob-poc".attribute_values_typed (
        entity_id,
        attribute_id,
        value_text,
        value_number,
        value_integer,
        value_boolean,
        value_date,
        value_datetime,
        value_json,
        created_by
    )
    VALUES (
        p_entity_id,
        p_attribute_id,
        p_value_text,
        p_value_number,
        p_value_integer,
        p_value_boolean,
        p_value_date,
        p_value_datetime,
        p_value_json,
        p_created_by
    )
    RETURNING id INTO v_new_id;

    RETURN v_new_id;
END;
$$;


--
-- Name: update_attribute_registry_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_attribute_registry_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW() AT TIME ZONE 'utc';
    RETURN NEW;
END;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: attribute_registry; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".attribute_registry (
    id text NOT NULL,
    display_name text NOT NULL,
    category text NOT NULL,
    value_type text NOT NULL,
    validation_rules jsonb DEFAULT '{}'::jsonb,
    metadata jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    uuid uuid NOT NULL,
    CONSTRAINT check_category CHECK ((category = ANY (ARRAY['identity'::text, 'financial'::text, 'compliance'::text, 'document'::text, 'risk'::text, 'contact'::text, 'address'::text, 'tax'::text, 'employment'::text, 'product'::text, 'entity'::text, 'ubo'::text, 'isda'::text]))),
    CONSTRAINT check_value_type CHECK ((value_type = ANY (ARRAY['string'::text, 'integer'::text, 'number'::text, 'boolean'::text, 'date'::text, 'datetime'::text, 'email'::text, 'phone'::text, 'address'::text, 'currency'::text, 'percentage'::text, 'tax_id'::text, 'json'::text])))
);


--
-- Name: TABLE attribute_registry; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".attribute_registry IS 'Type-safe attribute registry with string-based identifiers following the AttributeID-as-Type pattern';


--
-- Name: COLUMN attribute_registry.id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".attribute_registry.id IS 'Attribute identifier in format attr.{category}.{name} (e.g., attr.identity.first_name)';


--
-- Name: COLUMN attribute_registry.validation_rules; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".attribute_registry.validation_rules IS 'JSON object containing validation rules: {required, min_value, max_value, min_length, max_length, pattern, allowed_values}';


--
-- Name: attribute_uuid_map; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".attribute_uuid_map AS
 SELECT attribute_registry.id AS semantic_id,
    attribute_registry.uuid,
    attribute_registry.display_name,
    attribute_registry.category,
    attribute_registry.value_type
   FROM "ob-poc".attribute_registry
  ORDER BY attribute_registry.id;


--
-- Name: attribute_values; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE attribute_values; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".attribute_values IS 'Attribute values with enforced dictionary and CBU referential integrity';


--
-- Name: attribute_values_typed; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".attribute_values_typed (
    id integer NOT NULL,
    entity_id uuid NOT NULL,
    attribute_id text NOT NULL,
    value_text text,
    value_number numeric,
    value_integer bigint,
    value_boolean boolean,
    value_date date,
    value_datetime timestamp with time zone,
    value_json jsonb,
    effective_from timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    effective_to timestamp with time zone,
    source jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_by text DEFAULT 'system'::text,
    attribute_uuid uuid,
    CONSTRAINT check_single_value CHECK ((((((((((value_text IS NOT NULL))::integer + ((value_number IS NOT NULL))::integer) + ((value_integer IS NOT NULL))::integer) + ((value_boolean IS NOT NULL))::integer) + ((value_date IS NOT NULL))::integer) + ((value_datetime IS NOT NULL))::integer) + ((value_json IS NOT NULL))::integer) = 1)),
    CONSTRAINT check_temporal_validity CHECK (((effective_to IS NULL) OR (effective_to > effective_from)))
);


--
-- Name: TABLE attribute_values_typed; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".attribute_values_typed IS 'Type-safe attribute values with proper column typing based on value_type';


--
-- Name: CONSTRAINT check_single_value ON attribute_values_typed; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON CONSTRAINT check_single_value ON "ob-poc".attribute_values_typed IS 'Ensures exactly one value column is populated per row';


--
-- Name: attribute_values_typed_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: -
--

CREATE SEQUENCE "ob-poc".attribute_values_typed_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: attribute_values_typed_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: -
--

ALTER SEQUENCE "ob-poc".attribute_values_typed_id_seq OWNED BY "ob-poc".attribute_values_typed.id;


--
-- Name: cbu_creation_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_creation_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    nature_purpose text,
    source_of_funds text,
    ai_instruction text,
    generated_dsl text,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP
);


--
-- Name: cbu_entity_roles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_entity_roles (
    cbu_entity_role_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    role_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: cbus; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbus (
    cbu_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    nature_purpose text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    source_of_funds text
);


--
-- Name: crud_operations; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".crud_operations (
    operation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    operation_type character varying(20) NOT NULL,
    asset_type character varying(50) NOT NULL,
    entity_table_name character varying(100),
    generated_dsl text NOT NULL,
    ai_instruction text NOT NULL,
    affected_records jsonb DEFAULT '[]'::jsonb NOT NULL,
    execution_status character varying(20) DEFAULT 'PENDING'::character varying NOT NULL,
    ai_confidence numeric(3,2),
    ai_provider character varying(50),
    ai_model character varying(100),
    execution_time_ms integer,
    error_message text,
    created_by character varying(255) DEFAULT 'agentic_system'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    completed_at timestamp with time zone,
    rows_affected integer DEFAULT 0,
    transaction_id uuid,
    parent_operation_id uuid,
    CONSTRAINT crud_operations_ai_confidence_check CHECK (((ai_confidence >= 0.0) AND (ai_confidence <= 1.0))),
    CONSTRAINT crud_operations_asset_type_check CHECK (((asset_type)::text = ANY ((ARRAY['CBU'::character varying, 'ENTITY'::character varying, 'PARTNERSHIP'::character varying, 'LIMITED_COMPANY'::character varying, 'PROPER_PERSON'::character varying, 'TRUST'::character varying, 'ATTRIBUTE'::character varying, 'DOCUMENT'::character varying])::text[]))),
    CONSTRAINT crud_operations_execution_status_check CHECK (((execution_status)::text = ANY ((ARRAY['PENDING'::character varying, 'EXECUTING'::character varying, 'COMPLETED'::character varying, 'FAILED'::character varying, 'ROLLED_BACK'::character varying])::text[]))),
    CONSTRAINT crud_operations_operation_type_check CHECK (((operation_type)::text = ANY ((ARRAY['CREATE'::character varying, 'READ'::character varying, 'UPDATE'::character varying, 'DELETE'::character varying])::text[])))
);


--
-- Name: TABLE crud_operations; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".crud_operations IS 'Tracks all CRUD operations generated by the agentic system with AI metadata and execution status';


--
-- Name: COLUMN crud_operations.affected_records; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".crud_operations.affected_records IS 'JSON array of record IDs affected by this operation';


--
-- Name: COLUMN crud_operations.ai_confidence; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".crud_operations.ai_confidence IS 'AI confidence score between 0.0 and 1.0 for the generated DSL';


--
-- Name: dictionary; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: document_attribute_mappings; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_attribute_mappings (
    mapping_id uuid DEFAULT gen_random_uuid() NOT NULL,
    document_type_id uuid NOT NULL,
    attribute_uuid uuid NOT NULL,
    extraction_method character varying(50) NOT NULL,
    field_location jsonb,
    field_name character varying(255),
    confidence_threshold numeric(3,2) DEFAULT 0.80,
    is_required boolean DEFAULT false,
    validation_pattern text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT document_attribute_mappings_confidence_threshold_check CHECK (((confidence_threshold >= (0)::numeric) AND (confidence_threshold <= (1)::numeric))),
    CONSTRAINT document_attribute_mappings_extraction_method_check CHECK (((extraction_method)::text = ANY ((ARRAY['OCR'::character varying, 'MRZ'::character varying, 'BARCODE'::character varying, 'QR_CODE'::character varying, 'FORM_FIELD'::character varying, 'TABLE'::character varying, 'CHECKBOX'::character varying, 'SIGNATURE'::character varying, 'PHOTO'::character varying, 'NLP'::character varying, 'AI'::character varying])::text[])))
);


--
-- Name: TABLE document_attribute_mappings; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".document_attribute_mappings IS 'Seeded with common document type to attribute mappings for KYC and onboarding';


--
-- Name: COLUMN document_attribute_mappings.extraction_method; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".document_attribute_mappings.extraction_method IS 'Method used to extract the attribute: OCR, MRZ, BARCODE, FORM_FIELD, etc.';


--
-- Name: COLUMN document_attribute_mappings.confidence_threshold; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".document_attribute_mappings.confidence_threshold IS 'Minimum confidence score (0.0-1.0) required for extraction';


--
-- Name: document_catalog; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_catalog (
    doc_id uuid DEFAULT gen_random_uuid() NOT NULL,
    file_hash_sha256 text NOT NULL,
    storage_key text NOT NULL,
    file_size_bytes bigint,
    mime_type character varying(100),
    extracted_data jsonb,
    extraction_status character varying(50) DEFAULT 'PENDING'::character varying,
    extraction_confidence numeric(5,4),
    last_extracted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    cbu_id uuid,
    document_type_id uuid
);


--
-- Name: TABLE document_catalog; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".document_catalog IS 'Central "fact" table for all document instances. Stores file info and AI extraction results.';


--
-- Name: document_issuers_backup; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_issuers_backup (
    issuer_id uuid,
    issuer_code character varying(100),
    legal_name character varying(300),
    jurisdiction character varying(10),
    regulatory_type character varying(100),
    official_website character varying(500),
    verification_endpoint character varying(500),
    trust_level character varying(20),
    created_at timestamp with time zone,
    updated_at timestamp with time zone,
    backup_created_at timestamp with time zone
);


--
-- Name: document_metadata; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_metadata (
    doc_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    value jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    extraction_confidence numeric(3,2),
    extraction_method character varying(50),
    extracted_at timestamp with time zone,
    extraction_metadata jsonb
);


--
-- Name: TABLE document_metadata; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".document_metadata IS 'EAV table linking documents to their metadata attributes (from the dictionary). This is the critical bridge to the AttributeID-as-Type pattern.';


--
-- Name: document_relationships; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    primary_doc_id uuid NOT NULL,
    related_doc_id uuid NOT NULL,
    relationship_type character varying(100) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: TABLE document_relationships; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".document_relationships IS 'Models M:N relationships between documents (e.g., amendments, translations).';


--
-- Name: document_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_types (
    type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    type_code character varying(100) NOT NULL,
    display_name character varying(200) NOT NULL,
    category character varying(100) NOT NULL,
    domain character varying(100),
    description text,
    required_attributes jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: domain_vocabularies; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: dsl_domains; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_domains (
    domain_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain_name character varying(100) NOT NULL,
    description text,
    base_grammar_version character varying(20) DEFAULT '1.0.0'::character varying,
    vocabulary_version character varying(20) DEFAULT '1.0.0'::character varying,
    active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: dsl_examples; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_examples (
    example_id uuid DEFAULT gen_random_uuid() NOT NULL,
    title character varying(255) NOT NULL,
    description text,
    operation_type character varying(20) NOT NULL,
    asset_type character varying(50) NOT NULL,
    entity_table_name character varying(100),
    natural_language_input text NOT NULL,
    example_dsl text NOT NULL,
    expected_outcome text,
    tags text[] DEFAULT ARRAY[]::text[],
    complexity_level character varying(20) DEFAULT 'MEDIUM'::character varying,
    success_rate numeric(3,2) DEFAULT 1.0,
    usage_count integer DEFAULT 0,
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_by character varying(255) DEFAULT 'system'::character varying,
    CONSTRAINT dsl_examples_asset_type_check CHECK (((asset_type)::text = ANY ((ARRAY['CBU'::character varying, 'ENTITY'::character varying, 'PARTNERSHIP'::character varying, 'LIMITED_COMPANY'::character varying, 'PROPER_PERSON'::character varying, 'TRUST'::character varying, 'ATTRIBUTE'::character varying, 'DOCUMENT'::character varying])::text[]))),
    CONSTRAINT dsl_examples_complexity_level_check CHECK (((complexity_level)::text = ANY ((ARRAY['SIMPLE'::character varying, 'MEDIUM'::character varying, 'COMPLEX'::character varying])::text[]))),
    CONSTRAINT dsl_examples_operation_type_check CHECK (((operation_type)::text = ANY ((ARRAY['CREATE'::character varying, 'READ'::character varying, 'UPDATE'::character varying, 'DELETE'::character varying])::text[])))
);


--
-- Name: TABLE dsl_examples; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_examples IS 'Curated library of natural language to DSL examples for training and context';


--
-- Name: COLUMN dsl_examples.success_rate; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_examples.success_rate IS 'Rate of successful operations when using this example (0.0 to 1.0)';


--
-- Name: dsl_execution_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_execution_log (
    execution_id uuid DEFAULT gen_random_uuid() NOT NULL,
    version_id uuid NOT NULL,
    cbu_id character varying(255),
    execution_phase character varying(50) NOT NULL,
    status character varying(50) NOT NULL,
    result_data jsonb,
    error_details jsonb,
    performance_metrics jsonb,
    executed_by character varying(255),
    started_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    completed_at timestamp with time zone,
    duration_ms integer GENERATED ALWAYS AS (
CASE
    WHEN (completed_at IS NOT NULL) THEN (EXTRACT(epoch FROM (completed_at - started_at)) * (1000)::numeric)
    ELSE NULL::numeric
END) STORED
);


--
-- Name: dsl_versions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_versions (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain_id uuid NOT NULL,
    version_number integer NOT NULL,
    functional_state character varying(100),
    dsl_source_code text NOT NULL,
    compilation_status character varying(50) DEFAULT 'DRAFT'::character varying,
    change_description text,
    parent_version_id uuid,
    created_by character varying(255),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    compiled_at timestamp with time zone,
    activated_at timestamp with time zone
);


--
-- Name: dsl_execution_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".dsl_execution_summary AS
 SELECT d.domain_name,
    dv.version_number,
    dv.compilation_status,
    count(del.execution_id) AS total_executions,
    count(
        CASE
            WHEN ((del.status)::text = 'SUCCESS'::text) THEN 1
            ELSE NULL::integer
        END) AS successful_executions,
    count(
        CASE
            WHEN ((del.status)::text = 'FAILED'::text) THEN 1
            ELSE NULL::integer
        END) AS failed_executions,
    avg(del.duration_ms) AS avg_duration_ms,
    max(del.started_at) AS last_execution_at
   FROM (("ob-poc".dsl_domains d
     JOIN "ob-poc".dsl_versions dv ON ((d.domain_id = dv.domain_id)))
     LEFT JOIN "ob-poc".dsl_execution_log del ON ((dv.version_id = del.version_id)))
  GROUP BY d.domain_name, dv.version_number, dv.compilation_status
  ORDER BY d.domain_name, dv.version_number DESC;


--
-- Name: dsl_instances; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_instances (
    id integer NOT NULL,
    case_id character varying(255) NOT NULL,
    dsl_content text NOT NULL,
    domain character varying(100),
    operation_type character varying(100),
    status character varying(50) DEFAULT 'PROCESSED'::character varying,
    processing_time_ms bigint,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: dsl_instances_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: -
--

CREATE SEQUENCE "ob-poc".dsl_instances_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: dsl_instances_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: -
--

ALTER SEQUENCE "ob-poc".dsl_instances_id_seq OWNED BY "ob-poc".dsl_instances.id;


--
-- Name: parsed_asts; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".parsed_asts (
    ast_id uuid DEFAULT gen_random_uuid() NOT NULL,
    version_id uuid NOT NULL,
    ast_json jsonb NOT NULL,
    parse_metadata jsonb,
    grammar_version character varying(20) NOT NULL,
    parser_version character varying(20) NOT NULL,
    ast_hash character varying(64),
    node_count integer,
    complexity_score numeric(10,2),
    parsed_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    invalidated_at timestamp with time zone
);


--
-- Name: dsl_latest_versions; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".dsl_latest_versions AS
 SELECT d.domain_name,
    d.description AS domain_description,
    dv.version_id,
    dv.version_number,
    dv.functional_state,
    dv.compilation_status,
    dv.change_description,
    dv.created_by,
    dv.created_at,
        CASE
            WHEN (pa.ast_id IS NOT NULL) THEN true
            ELSE false
        END AS has_compiled_ast
   FROM (("ob-poc".dsl_domains d
     JOIN "ob-poc".dsl_versions dv ON ((d.domain_id = dv.domain_id)))
     LEFT JOIN "ob-poc".parsed_asts pa ON ((dv.version_id = pa.version_id)))
  WHERE ((dv.version_number = ( SELECT max(dv2.version_number) AS max
           FROM "ob-poc".dsl_versions dv2
          WHERE (dv2.domain_id = dv.domain_id))) AND (d.active = true))
  ORDER BY d.domain_name;


--
-- Name: dsl_ob; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_ob (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    dsl_text text NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: TABLE dsl_ob; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_ob IS 'DSL documents with enforced CBU referential integrity';


--
-- Name: COLUMN dsl_ob.cbu_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_ob.cbu_id IS 'UUID reference to cbus table primary key';


--
-- Name: entities; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entities (
    entity_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type_id uuid NOT NULL,
    external_id character varying(255),
    name character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: entity_crud_rules; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_crud_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_table_name character varying(100) NOT NULL,
    operation_type character varying(20) NOT NULL,
    field_name character varying(100),
    constraint_type character varying(50) NOT NULL,
    constraint_description text NOT NULL,
    validation_pattern character varying(500),
    error_message text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT entity_crud_rules_constraint_type_check CHECK (((constraint_type)::text = ANY ((ARRAY['REQUIRED'::character varying, 'UNIQUE'::character varying, 'FOREIGN_KEY'::character varying, 'VALIDATION'::character varying, 'BUSINESS_RULE'::character varying])::text[]))),
    CONSTRAINT entity_crud_rules_operation_type_check CHECK (((operation_type)::text = ANY ((ARRAY['CREATE'::character varying, 'READ'::character varying, 'UPDATE'::character varying, 'DELETE'::character varying])::text[])))
);


--
-- Name: TABLE entity_crud_rules; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_crud_rules IS 'Entity-specific validation rules and constraints for CRUD operations';


--
-- Name: entity_lifecycle_status; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_lifecycle_status (
    status_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid NOT NULL,
    status_code character varying(50) NOT NULL,
    status_description character varying(200),
    effective_date date NOT NULL,
    end_date date,
    reason_code character varying(100),
    notes text,
    created_by character varying(100) DEFAULT 'system'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE entity_lifecycle_status; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_lifecycle_status IS 'Tracks entity lifecycle states for workflow management';


--
-- Name: entity_limited_companies; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: entity_partnerships; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: entity_product_mappings; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_product_mappings (
    entity_type character varying(100) NOT NULL,
    product_id uuid NOT NULL,
    compatible boolean NOT NULL,
    restrictions jsonb,
    required_fields jsonb,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: entity_proper_persons; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: entity_role_connections; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_role_connections (
    connection_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    role_id uuid NOT NULL,
    connection_type character varying(50) NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP
);


--
-- Name: entity_trusts; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: entity_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_types (
    entity_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    table_name character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: entity_validation_rules; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_validation_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type character varying(50) NOT NULL,
    field_name character varying(100) NOT NULL,
    validation_type character varying(50) NOT NULL,
    validation_rule jsonb NOT NULL,
    error_message character varying(500),
    severity character varying(20) DEFAULT 'ERROR'::character varying,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT entity_validation_rules_severity_check CHECK (((severity)::text = ANY ((ARRAY['ERROR'::character varying, 'WARNING'::character varying, 'INFO'::character varying])::text[]))),
    CONSTRAINT entity_validation_rules_validation_type_check CHECK (((validation_type)::text = ANY ((ARRAY['REQUIRED'::character varying, 'FORMAT'::character varying, 'RANGE'::character varying, 'REFERENCE'::character varying, 'CUSTOM'::character varying])::text[])))
);


--
-- Name: TABLE entity_validation_rules; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_validation_rules IS 'Defines validation rules for agentic CRUD operations';


--
-- Name: COLUMN entity_validation_rules.validation_rule; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_validation_rules.validation_rule IS 'JSON object defining the validation logic';


--
-- Name: grammar_rules; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: master_entity_xref; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".master_entity_xref (
    xref_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid NOT NULL,
    entity_name character varying(500) NOT NULL,
    jurisdiction_code character varying(10),
    entity_status character varying(50) DEFAULT 'ACTIVE'::character varying,
    business_purpose text,
    primary_contact_person uuid,
    regulatory_numbers jsonb DEFAULT '{}'::jsonb,
    additional_metadata jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT master_entity_xref_entity_status_check CHECK (((entity_status)::text = ANY ((ARRAY['ACTIVE'::character varying, 'INACTIVE'::character varying, 'DISSOLVED'::character varying, 'SUSPENDED'::character varying])::text[]))),
    CONSTRAINT master_entity_xref_entity_type_check CHECK (((entity_type)::text = ANY ((ARRAY['PARTNERSHIP'::character varying, 'LIMITED_COMPANY'::character varying, 'PROPER_PERSON'::character varying, 'TRUST'::character varying])::text[])))
);


--
-- Name: TABLE master_entity_xref; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".master_entity_xref IS 'Master cross-reference table linking all entity types with unified metadata';


--
-- Name: COLUMN master_entity_xref.regulatory_numbers; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".master_entity_xref.regulatory_numbers IS 'JSON object storing various regulatory identification numbers';


--
-- Name: master_jurisdictions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".master_jurisdictions (
    jurisdiction_code character varying(10) NOT NULL,
    jurisdiction_name character varying(200) NOT NULL,
    country_code character varying(3) NOT NULL,
    region character varying(100),
    regulatory_framework character varying(100),
    entity_formation_allowed boolean DEFAULT true,
    offshore_jurisdiction boolean DEFAULT false,
    regulatory_authority character varying(300),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE master_jurisdictions; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".master_jurisdictions IS 'Comprehensive jurisdiction lookup table for entity formation and compliance';


--
-- Name: COLUMN master_jurisdictions.offshore_jurisdiction; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".master_jurisdictions.offshore_jurisdiction IS 'TRUE for offshore/tax haven jurisdictions';


--
-- Name: orchestration_domain_sessions; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: orchestration_sessions; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: orchestration_state_history; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: orchestration_tasks; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: partnership_control_mechanisms; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: partnership_interests; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: prod_resources; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: product_requirements; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: product_services; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".product_services (
    product_id uuid NOT NULL,
    service_id uuid NOT NULL
);


--
-- Name: product_workflows; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".product_workflows (
    workflow_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    product_id uuid NOT NULL,
    entity_type character varying(100) NOT NULL,
    required_dsl jsonb NOT NULL,
    generated_dsl text NOT NULL,
    compliance_rules jsonb NOT NULL,
    status character varying(50) DEFAULT 'PENDING'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: COLUMN product_workflows.cbu_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".product_workflows.cbu_id IS 'UUID reference to cbus table primary key';


--
-- Name: products; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".products (
    product_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: rag_embeddings; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".rag_embeddings (
    embedding_id uuid DEFAULT gen_random_uuid() NOT NULL,
    content_type character varying(50) NOT NULL,
    content_text text NOT NULL,
    embedding_data jsonb,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    source_table character varying(100),
    asset_type character varying(50),
    relevance_score numeric(3,2) DEFAULT 1.0,
    usage_count integer DEFAULT 0,
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT rag_embeddings_content_type_check CHECK (((content_type)::text = ANY ((ARRAY['SCHEMA'::character varying, 'EXAMPLE'::character varying, 'ATTRIBUTE'::character varying, 'RULE'::character varying, 'GRAMMAR'::character varying, 'VERB_PATTERN'::character varying])::text[])))
);


--
-- Name: TABLE rag_embeddings; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".rag_embeddings IS 'Vector embeddings for RAG context retrieval in agentic CRUD operations';


--
-- Name: COLUMN rag_embeddings.embedding_data; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".rag_embeddings.embedding_data IS 'Vector embedding stored as JSON until pgvector extension is available';


--
-- Name: referential_integrity_check; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".referential_integrity_check AS
 WITH integrity_issues AS (
         SELECT 'dsl_ob'::text AS table_name,
            'cbu_id'::text AS column_name,
            (d.cbu_id)::text AS orphaned_value,
            'missing CBU reference'::text AS issue
           FROM "ob-poc".dsl_ob d
          WHERE (NOT (EXISTS ( SELECT 1
                   FROM "ob-poc".cbus c
                  WHERE (c.cbu_id = d.cbu_id))))
        UNION ALL
         SELECT 'attribute_values'::text AS table_name,
            'cbu_id'::text AS column_name,
            (av.cbu_id)::text AS orphaned_value,
            'missing CBU reference'::text AS issue
           FROM "ob-poc".attribute_values av
          WHERE (NOT (EXISTS ( SELECT 1
                   FROM "ob-poc".cbus c
                  WHERE (c.cbu_id = av.cbu_id))))
        UNION ALL
         SELECT 'attribute_values'::text AS table_name,
            'attribute_id'::text AS column_name,
            (av.attribute_id)::text AS orphaned_value,
            'missing dictionary reference'::text AS issue
           FROM "ob-poc".attribute_values av
          WHERE (NOT (EXISTS ( SELECT 1
                   FROM "ob-poc".dictionary d
                  WHERE (d.attribute_id = av.attribute_id))))
        )
 SELECT integrity_issues.table_name,
    integrity_issues.column_name,
    integrity_issues.orphaned_value,
    integrity_issues.issue
   FROM integrity_issues;


--
-- Name: roles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".roles (
    role_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: schema_changes; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".schema_changes (
    change_id uuid DEFAULT gen_random_uuid() NOT NULL,
    change_type character varying(50) NOT NULL,
    description text NOT NULL,
    script_name character varying(255),
    applied_at timestamp with time zone DEFAULT now(),
    applied_by character varying(100) DEFAULT CURRENT_USER
);


--
-- Name: service_resources; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".service_resources (
    service_id uuid NOT NULL,
    resource_id uuid NOT NULL
);


--
-- Name: services; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".services (
    service_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: trust_beneficiary_classes; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: trust_parties; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: trust_protector_powers; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".trust_protector_powers (
    protector_power_id uuid DEFAULT gen_random_uuid() NOT NULL,
    trust_party_id uuid NOT NULL,
    power_type character varying(100) NOT NULL,
    power_description text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


--
-- Name: ubo_registry; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE ubo_registry; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_registry IS 'UBO identification results with proper entity referential integrity';


--
-- Name: verb_registry; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: vocabulary_audit; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: attribute_values_typed id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed ALTER COLUMN id SET DEFAULT nextval('"ob-poc".attribute_values_typed_id_seq'::regclass);


--
-- Name: dsl_instances id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instances ALTER COLUMN id SET DEFAULT nextval('"ob-poc".dsl_instances_id_seq'::regclass);


--
-- Name: attribute_registry attribute_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_registry
    ADD CONSTRAINT attribute_registry_pkey PRIMARY KEY (id);


--
-- Name: attribute_values attribute_values_cbu_id_dsl_version_attribute_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_cbu_id_dsl_version_attribute_id_key UNIQUE (cbu_id, dsl_version, attribute_id);


--
-- Name: attribute_values attribute_values_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_pkey PRIMARY KEY (av_id);


--
-- Name: attribute_values_typed attribute_values_typed_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT attribute_values_typed_pkey PRIMARY KEY (id);


--
-- Name: cbu_creation_log cbu_creation_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_creation_log
    ADD CONSTRAINT cbu_creation_log_pkey PRIMARY KEY (log_id);


--
-- Name: cbu_entity_roles cbu_entity_roles_cbu_id_entity_id_role_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_cbu_id_entity_id_role_id_key UNIQUE (cbu_id, entity_id, role_id);


--
-- Name: cbu_entity_roles cbu_entity_roles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_pkey PRIMARY KEY (cbu_entity_role_id);


--
-- Name: cbus cbus_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_name_key UNIQUE (name);


--
-- Name: cbus cbus_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_pkey PRIMARY KEY (cbu_id);


--
-- Name: crud_operations crud_operations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".crud_operations
    ADD CONSTRAINT crud_operations_pkey PRIMARY KEY (operation_id);


--
-- Name: dictionary dictionary_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dictionary
    ADD CONSTRAINT dictionary_name_key UNIQUE (name);


--
-- Name: dictionary dictionary_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dictionary
    ADD CONSTRAINT dictionary_pkey PRIMARY KEY (attribute_id);


--
-- Name: document_attribute_mappings document_attribute_mappings_document_type_id_attribute_uuid_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_document_type_id_attribute_uuid_key UNIQUE (document_type_id, attribute_uuid);


--
-- Name: document_attribute_mappings document_attribute_mappings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_pkey PRIMARY KEY (mapping_id);


--
-- Name: document_catalog document_catalog_file_hash_sha256_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_file_hash_sha256_key UNIQUE (file_hash_sha256);


--
-- Name: document_catalog document_catalog_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_pkey PRIMARY KEY (doc_id);


--
-- Name: document_metadata document_metadata_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_metadata
    ADD CONSTRAINT document_metadata_pkey PRIMARY KEY (doc_id, attribute_id);


--
-- Name: document_relationships document_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: document_relationships document_relationships_primary_doc_id_related_doc_id_relati_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_primary_doc_id_related_doc_id_relati_key UNIQUE (primary_doc_id, related_doc_id, relationship_type);


--
-- Name: document_types document_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_types
    ADD CONSTRAINT document_types_pkey PRIMARY KEY (type_id);


--
-- Name: document_types document_types_type_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_types
    ADD CONSTRAINT document_types_type_code_key UNIQUE (type_code);


--
-- Name: domain_vocabularies domain_vocabularies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".domain_vocabularies
    ADD CONSTRAINT domain_vocabularies_pkey PRIMARY KEY (vocab_id);


--
-- Name: dsl_domains dsl_domains_domain_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_domains
    ADD CONSTRAINT dsl_domains_domain_name_key UNIQUE (domain_name);


--
-- Name: dsl_domains dsl_domains_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_domains
    ADD CONSTRAINT dsl_domains_pkey PRIMARY KEY (domain_id);


--
-- Name: dsl_examples dsl_examples_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_examples
    ADD CONSTRAINT dsl_examples_pkey PRIMARY KEY (example_id);


--
-- Name: dsl_execution_log dsl_execution_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_execution_log
    ADD CONSTRAINT dsl_execution_log_pkey PRIMARY KEY (execution_id);


--
-- Name: dsl_instances dsl_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instances
    ADD CONSTRAINT dsl_instances_pkey PRIMARY KEY (id);


--
-- Name: dsl_ob dsl_ob_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_ob
    ADD CONSTRAINT dsl_ob_pkey PRIMARY KEY (version_id);


--
-- Name: dsl_versions dsl_versions_domain_id_version_number_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_domain_id_version_number_key UNIQUE (domain_id, version_number);


--
-- Name: dsl_versions dsl_versions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_pkey PRIMARY KEY (version_id);


--
-- Name: entities entities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_crud_rules entity_crud_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_crud_rules
    ADD CONSTRAINT entity_crud_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: entity_lifecycle_status entity_lifecycle_status_entity_type_entity_id_status_code_e_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_lifecycle_status
    ADD CONSTRAINT entity_lifecycle_status_entity_type_entity_id_status_code_e_key UNIQUE (entity_type, entity_id, status_code, effective_date);


--
-- Name: entity_lifecycle_status entity_lifecycle_status_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_lifecycle_status
    ADD CONSTRAINT entity_lifecycle_status_pkey PRIMARY KEY (status_id);


--
-- Name: entity_limited_companies entity_limited_companies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_pkey PRIMARY KEY (limited_company_id);


--
-- Name: entity_partnerships entity_partnerships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_partnerships
    ADD CONSTRAINT entity_partnerships_pkey PRIMARY KEY (partnership_id);


--
-- Name: entity_product_mappings entity_product_mappings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT entity_product_mappings_pkey PRIMARY KEY (entity_type, product_id);


--
-- Name: entity_proper_persons entity_proper_persons_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_pkey PRIMARY KEY (proper_person_id);


--
-- Name: entity_role_connections entity_role_connections_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT entity_role_connections_pkey PRIMARY KEY (connection_id);


--
-- Name: entity_trusts entity_trusts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_trusts
    ADD CONSTRAINT entity_trusts_pkey PRIMARY KEY (trust_id);


--
-- Name: entity_types entity_types_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_types
    ADD CONSTRAINT entity_types_name_key UNIQUE (name);


--
-- Name: entity_types entity_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_types
    ADD CONSTRAINT entity_types_pkey PRIMARY KEY (entity_type_id);


--
-- Name: entity_validation_rules entity_validation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_validation_rules
    ADD CONSTRAINT entity_validation_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: grammar_rules grammar_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".grammar_rules
    ADD CONSTRAINT grammar_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: grammar_rules grammar_rules_rule_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".grammar_rules
    ADD CONSTRAINT grammar_rules_rule_name_key UNIQUE (rule_name);


--
-- Name: master_entity_xref master_entity_xref_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".master_entity_xref
    ADD CONSTRAINT master_entity_xref_pkey PRIMARY KEY (xref_id);


--
-- Name: master_jurisdictions master_jurisdictions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".master_jurisdictions
    ADD CONSTRAINT master_jurisdictions_pkey PRIMARY KEY (jurisdiction_code);


--
-- Name: orchestration_domain_sessions orchestration_domain_sessions_orchestration_session_id_doma_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT orchestration_domain_sessions_orchestration_session_id_doma_key UNIQUE (orchestration_session_id, domain_name);


--
-- Name: orchestration_domain_sessions orchestration_domain_sessions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT orchestration_domain_sessions_pkey PRIMARY KEY (id);


--
-- Name: orchestration_sessions orchestration_sessions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_sessions
    ADD CONSTRAINT orchestration_sessions_pkey PRIMARY KEY (session_id);


--
-- Name: orchestration_state_history orchestration_state_history_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_state_history
    ADD CONSTRAINT orchestration_state_history_pkey PRIMARY KEY (id);


--
-- Name: orchestration_tasks orchestration_tasks_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_tasks
    ADD CONSTRAINT orchestration_tasks_pkey PRIMARY KEY (task_id);


--
-- Name: parsed_asts parsed_asts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".parsed_asts
    ADD CONSTRAINT parsed_asts_pkey PRIMARY KEY (ast_id);


--
-- Name: parsed_asts parsed_asts_version_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".parsed_asts
    ADD CONSTRAINT parsed_asts_version_id_key UNIQUE (version_id);


--
-- Name: partnership_control_mechanisms partnership_control_mechanisms_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT partnership_control_mechanisms_pkey PRIMARY KEY (control_mechanism_id);


--
-- Name: partnership_interests partnership_interests_partnership_id_entity_id_partner_type_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_partnership_id_entity_id_partner_type_key UNIQUE (partnership_id, entity_id, partner_type);


--
-- Name: partnership_interests partnership_interests_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_pkey PRIMARY KEY (interest_id);


--
-- Name: prod_resources prod_resources_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".prod_resources
    ADD CONSTRAINT prod_resources_name_key UNIQUE (name);


--
-- Name: prod_resources prod_resources_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".prod_resources
    ADD CONSTRAINT prod_resources_pkey PRIMARY KEY (resource_id);


--
-- Name: product_requirements product_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_requirements
    ADD CONSTRAINT product_requirements_pkey PRIMARY KEY (product_id);


--
-- Name: product_services product_services_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_pkey PRIMARY KEY (product_id, service_id);


--
-- Name: product_workflows product_workflows_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT product_workflows_pkey PRIMARY KEY (workflow_id);


--
-- Name: products products_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".products
    ADD CONSTRAINT products_name_key UNIQUE (name);


--
-- Name: products products_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".products
    ADD CONSTRAINT products_pkey PRIMARY KEY (product_id);


--
-- Name: rag_embeddings rag_embeddings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".rag_embeddings
    ADD CONSTRAINT rag_embeddings_pkey PRIMARY KEY (embedding_id);


--
-- Name: roles roles_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".roles
    ADD CONSTRAINT roles_name_key UNIQUE (name);


--
-- Name: roles roles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".roles
    ADD CONSTRAINT roles_pkey PRIMARY KEY (role_id);


--
-- Name: schema_changes schema_changes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".schema_changes
    ADD CONSTRAINT schema_changes_pkey PRIMARY KEY (change_id);


--
-- Name: service_resources service_resources_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_pkey PRIMARY KEY (service_id, resource_id);


--
-- Name: services services_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".services
    ADD CONSTRAINT services_name_key UNIQUE (name);


--
-- Name: services services_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".services
    ADD CONSTRAINT services_pkey PRIMARY KEY (service_id);


--
-- Name: trust_beneficiary_classes trust_beneficiary_classes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_beneficiary_classes
    ADD CONSTRAINT trust_beneficiary_classes_pkey PRIMARY KEY (beneficiary_class_id);


--
-- Name: trust_parties trust_parties_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_pkey PRIMARY KEY (trust_party_id);


--
-- Name: trust_parties trust_parties_trust_id_entity_id_party_role_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_trust_id_entity_id_party_role_key UNIQUE (trust_id, entity_id, party_role);


--
-- Name: trust_protector_powers trust_protector_powers_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_protector_powers
    ADD CONSTRAINT trust_protector_powers_pkey PRIMARY KEY (protector_power_id);


--
-- Name: ubo_registry ubo_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_pkey PRIMARY KEY (ubo_id);


--
-- Name: ubo_registry ubo_registry_subject_entity_id_ubo_proper_person_id_relatio_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_subject_entity_id_ubo_proper_person_id_relatio_key UNIQUE (subject_entity_id, ubo_proper_person_id, relationship_type);


--
-- Name: attribute_registry uk_attribute_uuid; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_registry
    ADD CONSTRAINT uk_attribute_uuid UNIQUE (uuid);


--
-- Name: verb_registry verb_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verb_registry
    ADD CONSTRAINT verb_registry_pkey PRIMARY KEY (verb);


--
-- Name: vocabulary_audit vocabulary_audit_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".vocabulary_audit
    ADD CONSTRAINT vocabulary_audit_pkey PRIMARY KEY (audit_id);


--
-- Name: idx_attr_uuid; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attr_uuid ON "ob-poc".attribute_registry USING btree (uuid);


--
-- Name: idx_attr_vals_lookup; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attr_vals_lookup ON "ob-poc".attribute_values USING btree (cbu_id, attribute_id, dsl_version);


--
-- Name: idx_attribute_registry_category; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_registry_category ON "ob-poc".attribute_registry USING btree (category);


--
-- Name: idx_attribute_registry_value_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_registry_value_type ON "ob-poc".attribute_registry USING btree (value_type);


--
-- Name: idx_attribute_values_typed_attribute; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_values_typed_attribute ON "ob-poc".attribute_values_typed USING btree (attribute_id);


--
-- Name: idx_attribute_values_typed_effective; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_values_typed_effective ON "ob-poc".attribute_values_typed USING btree (effective_from, effective_to) WHERE (effective_to IS NULL);


--
-- Name: idx_attribute_values_typed_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_values_typed_entity ON "ob-poc".attribute_values_typed USING btree (entity_id);


--
-- Name: idx_attribute_values_typed_entity_attribute; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_values_typed_entity_attribute ON "ob-poc".attribute_values_typed USING btree (entity_id, attribute_id);


--
-- Name: idx_beneficiary_classes_trust; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_beneficiary_classes_trust ON "ob-poc".trust_beneficiary_classes USING btree (trust_id);


--
-- Name: idx_cbu_creation_log_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_creation_log_cbu ON "ob-poc".cbu_creation_log USING btree (cbu_id);


--
-- Name: idx_cbu_entity_roles_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_entity_roles_cbu ON "ob-poc".cbu_entity_roles USING btree (cbu_id);


--
-- Name: idx_cbu_entity_roles_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_entity_roles_entity ON "ob-poc".cbu_entity_roles USING btree (entity_id);


--
-- Name: idx_cbu_entity_roles_role; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_entity_roles_role ON "ob-poc".cbu_entity_roles USING btree (role_id);


--
-- Name: idx_cbus_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_name ON "ob-poc".cbus USING btree (name);


--
-- Name: idx_crud_operations_asset; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_crud_operations_asset ON "ob-poc".crud_operations USING btree (asset_type);


--
-- Name: idx_crud_operations_created; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_crud_operations_created ON "ob-poc".crud_operations USING btree (created_at DESC);


--
-- Name: idx_crud_operations_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_crud_operations_parent ON "ob-poc".crud_operations USING btree (parent_operation_id) WHERE (parent_operation_id IS NOT NULL);


--
-- Name: idx_crud_operations_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_crud_operations_status ON "ob-poc".crud_operations USING btree (execution_status);


--
-- Name: idx_crud_operations_transaction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_crud_operations_transaction ON "ob-poc".crud_operations USING btree (transaction_id) WHERE (transaction_id IS NOT NULL);


--
-- Name: idx_crud_operations_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_crud_operations_type ON "ob-poc".crud_operations USING btree (operation_type);


--
-- Name: idx_dictionary_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dictionary_domain ON "ob-poc".dictionary USING btree (domain);


--
-- Name: idx_dictionary_group_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dictionary_group_id ON "ob-poc".dictionary USING btree (group_id);


--
-- Name: idx_dictionary_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dictionary_name ON "ob-poc".dictionary USING btree (name);


--
-- Name: idx_doc_attr_mappings_attr; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_attr_mappings_attr ON "ob-poc".document_attribute_mappings USING btree (attribute_uuid);


--
-- Name: idx_doc_attr_mappings_doc_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_attr_mappings_doc_type ON "ob-poc".document_attribute_mappings USING btree (document_type_id);


--
-- Name: idx_doc_catalog_hash; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_catalog_hash ON "ob-poc".document_catalog USING btree (file_hash_sha256);


--
-- Name: idx_doc_catalog_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_catalog_status ON "ob-poc".document_catalog USING btree (extraction_status);


--
-- Name: idx_doc_meta_attr_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_meta_attr_id ON "ob-poc".document_metadata USING btree (attribute_id);


--
-- Name: idx_doc_meta_doc_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_meta_doc_id ON "ob-poc".document_metadata USING btree (doc_id);


--
-- Name: idx_doc_meta_value_gin; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_meta_value_gin ON "ob-poc".document_metadata USING gin (value jsonb_path_ops);


--
-- Name: idx_doc_rel_primary; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_rel_primary ON "ob-poc".document_relationships USING btree (primary_doc_id);


--
-- Name: idx_doc_rel_related; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_rel_related ON "ob-poc".document_relationships USING btree (related_doc_id);


--
-- Name: idx_document_catalog_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_catalog_cbu ON "ob-poc".document_catalog USING btree (cbu_id);


--
-- Name: idx_domain_vocabularies_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_domain_vocabularies_active ON "ob-poc".domain_vocabularies USING btree (active);


--
-- Name: idx_domain_vocabularies_category; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_domain_vocabularies_category ON "ob-poc".domain_vocabularies USING btree (category);


--
-- Name: idx_domain_vocabularies_domain_verb; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_domain_vocabularies_domain_verb ON "ob-poc".domain_vocabularies USING btree (domain, verb);


--
-- Name: idx_dsl_domains_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_domains_active ON "ob-poc".dsl_domains USING btree (active);


--
-- Name: idx_dsl_domains_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_domains_name ON "ob-poc".dsl_domains USING btree (domain_name);


--
-- Name: idx_dsl_examples_asset; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_asset ON "ob-poc".dsl_examples USING btree (asset_type);


--
-- Name: idx_dsl_examples_complexity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_complexity ON "ob-poc".dsl_examples USING btree (complexity_level);


--
-- Name: idx_dsl_examples_operation; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_operation ON "ob-poc".dsl_examples USING btree (operation_type);


--
-- Name: idx_dsl_examples_success; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_success ON "ob-poc".dsl_examples USING btree (success_rate DESC);


--
-- Name: idx_dsl_examples_table; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_table ON "ob-poc".dsl_examples USING btree (entity_table_name) WHERE (entity_table_name IS NOT NULL);


--
-- Name: idx_dsl_examples_tags; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_tags ON "ob-poc".dsl_examples USING gin (tags);


--
-- Name: idx_dsl_examples_usage; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_examples_usage ON "ob-poc".dsl_examples USING btree (usage_count DESC);


--
-- Name: idx_dsl_execution_cbu_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_cbu_id ON "ob-poc".dsl_execution_log USING btree (cbu_id);


--
-- Name: idx_dsl_execution_started_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_started_at ON "ob-poc".dsl_execution_log USING btree (started_at DESC);


--
-- Name: idx_dsl_execution_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_status ON "ob-poc".dsl_execution_log USING btree (status);


--
-- Name: idx_dsl_execution_version_phase; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_version_phase ON "ob-poc".dsl_execution_log USING btree (version_id, execution_phase);


--
-- Name: idx_dsl_instances_case_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_instances_case_id ON "ob-poc".dsl_instances USING btree (case_id);


--
-- Name: idx_dsl_ob_cbu_id_created_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_ob_cbu_id_created_at ON "ob-poc".dsl_ob USING btree (cbu_id, created_at DESC);


--
-- Name: idx_dsl_versions_created_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_versions_created_at ON "ob-poc".dsl_versions USING btree (created_at DESC);


--
-- Name: idx_dsl_versions_domain_version; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_versions_domain_version ON "ob-poc".dsl_versions USING btree (domain_id, version_number DESC);


--
-- Name: idx_dsl_versions_functional_state; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_versions_functional_state ON "ob-poc".dsl_versions USING btree (functional_state);


--
-- Name: idx_dsl_versions_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_versions_status ON "ob-poc".dsl_versions USING btree (compilation_status);


--
-- Name: idx_entities_external_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entities_external_id ON "ob-poc".entities USING btree (external_id);


--
-- Name: idx_entities_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entities_name ON "ob-poc".entities USING btree (name);


--
-- Name: idx_entities_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entities_type ON "ob-poc".entities USING btree (entity_type_id);


--
-- Name: idx_entity_crud_rules_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_crud_rules_active ON "ob-poc".entity_crud_rules USING btree (is_active);


--
-- Name: idx_entity_crud_rules_field; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_crud_rules_field ON "ob-poc".entity_crud_rules USING btree (field_name) WHERE (field_name IS NOT NULL);


--
-- Name: idx_entity_crud_rules_operation; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_crud_rules_operation ON "ob-poc".entity_crud_rules USING btree (operation_type);


--
-- Name: idx_entity_crud_rules_table; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_crud_rules_table ON "ob-poc".entity_crud_rules USING btree (entity_table_name);


--
-- Name: idx_entity_lifecycle_effective; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_lifecycle_effective ON "ob-poc".entity_lifecycle_status USING btree (effective_date);


--
-- Name: idx_entity_lifecycle_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_lifecycle_status ON "ob-poc".entity_lifecycle_status USING btree (status_code);


--
-- Name: idx_entity_lifecycle_type_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_lifecycle_type_id ON "ob-poc".entity_lifecycle_status USING btree (entity_type, entity_id);


--
-- Name: idx_entity_product_mappings_compatible; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_product_mappings_compatible ON "ob-poc".entity_product_mappings USING btree (compatible);


--
-- Name: idx_entity_product_mappings_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_product_mappings_entity ON "ob-poc".entity_product_mappings USING btree (entity_type);


--
-- Name: idx_entity_product_mappings_product; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_product_mappings_product ON "ob-poc".entity_product_mappings USING btree (product_id);


--
-- Name: idx_entity_role_connections_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_role_connections_cbu ON "ob-poc".entity_role_connections USING btree (cbu_id);


--
-- Name: idx_entity_role_connections_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_role_connections_entity ON "ob-poc".entity_role_connections USING btree (entity_id);


--
-- Name: idx_entity_role_connections_role; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_role_connections_role ON "ob-poc".entity_role_connections USING btree (role_id);


--
-- Name: idx_entity_types_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_name ON "ob-poc".entity_types USING btree (name);


--
-- Name: idx_entity_types_table; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_table ON "ob-poc".entity_types USING btree (table_name);


--
-- Name: idx_entity_validation_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_validation_active ON "ob-poc".entity_validation_rules USING btree (is_active);


--
-- Name: idx_entity_validation_field; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_validation_field ON "ob-poc".entity_validation_rules USING btree (field_name);


--
-- Name: idx_entity_validation_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_validation_type ON "ob-poc".entity_validation_rules USING btree (entity_type);


--
-- Name: idx_grammar_rules_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_grammar_rules_active ON "ob-poc".grammar_rules USING btree (active);


--
-- Name: idx_grammar_rules_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_grammar_rules_domain ON "ob-poc".grammar_rules USING btree (domain);


--
-- Name: idx_grammar_rules_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_grammar_rules_name ON "ob-poc".grammar_rules USING btree (rule_name);


--
-- Name: idx_limited_companies_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_jurisdiction ON "ob-poc".entity_limited_companies USING btree (jurisdiction);


--
-- Name: idx_limited_companies_reg_num; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_reg_num ON "ob-poc".entity_limited_companies USING btree (registration_number);


--
-- Name: idx_master_entity_xref_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_master_entity_xref_entity_id ON "ob-poc".master_entity_xref USING btree (entity_id);


--
-- Name: idx_master_entity_xref_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_master_entity_xref_jurisdiction ON "ob-poc".master_entity_xref USING btree (jurisdiction_code);


--
-- Name: idx_master_entity_xref_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_master_entity_xref_name ON "ob-poc".master_entity_xref USING gin (to_tsvector('english'::regconfig, (entity_name)::text));


--
-- Name: idx_master_entity_xref_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_master_entity_xref_status ON "ob-poc".master_entity_xref USING btree (entity_status);


--
-- Name: idx_master_entity_xref_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_master_entity_xref_type ON "ob-poc".master_entity_xref USING btree (entity_type);


--
-- Name: idx_orchestration_domain_sessions_activity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_domain_sessions_activity ON "ob-poc".orchestration_domain_sessions USING btree (last_activity);


--
-- Name: idx_orchestration_domain_sessions_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_domain_sessions_domain ON "ob-poc".orchestration_domain_sessions USING btree (domain_name);


--
-- Name: idx_orchestration_domain_sessions_orchestration; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_domain_sessions_orchestration ON "ob-poc".orchestration_domain_sessions USING btree (orchestration_session_id);


--
-- Name: idx_orchestration_domain_sessions_state; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_domain_sessions_state ON "ob-poc".orchestration_domain_sessions USING btree (state);


--
-- Name: idx_orchestration_sessions_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_sessions_cbu ON "ob-poc".orchestration_sessions USING btree (cbu_id);


--
-- Name: idx_orchestration_sessions_entity_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_sessions_entity_type ON "ob-poc".orchestration_sessions USING btree (entity_type);


--
-- Name: idx_orchestration_sessions_expires; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_sessions_expires ON "ob-poc".orchestration_sessions USING btree (expires_at);


--
-- Name: idx_orchestration_sessions_last_used; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_sessions_last_used ON "ob-poc".orchestration_sessions USING btree (last_used);


--
-- Name: idx_orchestration_sessions_state; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_sessions_state ON "ob-poc".orchestration_sessions USING btree (current_state);


--
-- Name: idx_orchestration_sessions_workflow; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_sessions_workflow ON "ob-poc".orchestration_sessions USING btree (workflow_type);


--
-- Name: idx_orchestration_state_history_created; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_state_history_created ON "ob-poc".orchestration_state_history USING btree (created_at);


--
-- Name: idx_orchestration_state_history_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_state_history_session ON "ob-poc".orchestration_state_history USING btree (orchestration_session_id);


--
-- Name: idx_orchestration_state_history_states; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_state_history_states ON "ob-poc".orchestration_state_history USING btree (from_state, to_state);


--
-- Name: idx_orchestration_tasks_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_tasks_domain ON "ob-poc".orchestration_tasks USING btree (domain_name);


--
-- Name: idx_orchestration_tasks_scheduled; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_tasks_scheduled ON "ob-poc".orchestration_tasks USING btree (scheduled_at);


--
-- Name: idx_orchestration_tasks_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_tasks_session ON "ob-poc".orchestration_tasks USING btree (orchestration_session_id);


--
-- Name: idx_orchestration_tasks_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_orchestration_tasks_status ON "ob-poc".orchestration_tasks USING btree (status);


--
-- Name: idx_parsed_asts_grammar_version; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_parsed_asts_grammar_version ON "ob-poc".parsed_asts USING btree (grammar_version);


--
-- Name: idx_parsed_asts_hash; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_parsed_asts_hash ON "ob-poc".parsed_asts USING btree (ast_hash);


--
-- Name: idx_parsed_asts_parsed_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_parsed_asts_parsed_at ON "ob-poc".parsed_asts USING btree (parsed_at DESC);


--
-- Name: idx_parsed_asts_version_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_parsed_asts_version_id ON "ob-poc".parsed_asts USING btree (version_id);


--
-- Name: idx_partnership_control_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnership_control_entity ON "ob-poc".partnership_control_mechanisms USING btree (entity_id);


--
-- Name: idx_partnership_control_partnership; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnership_control_partnership ON "ob-poc".partnership_control_mechanisms USING btree (partnership_id);


--
-- Name: idx_partnership_interests_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnership_interests_entity ON "ob-poc".partnership_interests USING btree (entity_id);


--
-- Name: idx_partnership_interests_partnership; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnership_interests_partnership ON "ob-poc".partnership_interests USING btree (partnership_id);


--
-- Name: idx_partnership_interests_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnership_interests_type ON "ob-poc".partnership_interests USING btree (partner_type);


--
-- Name: idx_partnerships_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnerships_jurisdiction ON "ob-poc".entity_partnerships USING btree (jurisdiction);


--
-- Name: idx_partnerships_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnerships_type ON "ob-poc".entity_partnerships USING btree (partnership_type);


--
-- Name: idx_prod_resources_dict_group; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_prod_resources_dict_group ON "ob-poc".prod_resources USING btree (dictionary_group);


--
-- Name: idx_prod_resources_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_prod_resources_name ON "ob-poc".prod_resources USING btree (name);


--
-- Name: idx_prod_resources_owner; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_prod_resources_owner ON "ob-poc".prod_resources USING btree (owner);


--
-- Name: idx_product_requirements_product; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_product_requirements_product ON "ob-poc".product_requirements USING btree (product_id);


--
-- Name: idx_product_workflows_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_product_workflows_cbu ON "ob-poc".product_workflows USING btree (cbu_id);


--
-- Name: idx_product_workflows_product_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_product_workflows_product_entity ON "ob-poc".product_workflows USING btree (product_id, entity_type);


--
-- Name: idx_product_workflows_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_product_workflows_status ON "ob-poc".product_workflows USING btree (status);


--
-- Name: idx_products_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_products_name ON "ob-poc".products USING btree (name);


--
-- Name: idx_proper_persons_full_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proper_persons_full_name ON "ob-poc".entity_proper_persons USING btree (last_name, first_name);


--
-- Name: idx_proper_persons_id_document; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proper_persons_id_document ON "ob-poc".entity_proper_persons USING btree (id_document_type, id_document_number);


--
-- Name: idx_proper_persons_nationality; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proper_persons_nationality ON "ob-poc".entity_proper_persons USING btree (nationality);


--
-- Name: idx_protector_powers_party; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_protector_powers_party ON "ob-poc".trust_protector_powers USING btree (trust_party_id);


--
-- Name: idx_rag_embeddings_asset; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rag_embeddings_asset ON "ob-poc".rag_embeddings USING btree (asset_type);


--
-- Name: idx_rag_embeddings_relevance; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rag_embeddings_relevance ON "ob-poc".rag_embeddings USING btree (relevance_score DESC);


--
-- Name: idx_rag_embeddings_source; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rag_embeddings_source ON "ob-poc".rag_embeddings USING btree (source_table) WHERE (source_table IS NOT NULL);


--
-- Name: idx_rag_embeddings_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rag_embeddings_type ON "ob-poc".rag_embeddings USING btree (content_type);


--
-- Name: idx_rag_embeddings_usage; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rag_embeddings_usage ON "ob-poc".rag_embeddings USING btree (usage_count DESC);


--
-- Name: idx_roles_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_roles_name ON "ob-poc".roles USING btree (name);


--
-- Name: idx_services_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_services_name ON "ob-poc".services USING btree (name);


--
-- Name: idx_trust_parties_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trust_parties_entity ON "ob-poc".trust_parties USING btree (entity_id);


--
-- Name: idx_trust_parties_role; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trust_parties_role ON "ob-poc".trust_parties USING btree (party_role);


--
-- Name: idx_trust_parties_trust; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trust_parties_trust ON "ob-poc".trust_parties USING btree (trust_id);


--
-- Name: idx_trusts_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trusts_jurisdiction ON "ob-poc".entity_trusts USING btree (jurisdiction);


--
-- Name: idx_trusts_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trusts_type ON "ob-poc".entity_trusts USING btree (trust_type);


--
-- Name: idx_ubo_registry_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_registry_cbu ON "ob-poc".ubo_registry USING btree (cbu_id);


--
-- Name: idx_ubo_registry_subject; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_registry_subject ON "ob-poc".ubo_registry USING btree (subject_entity_id);


--
-- Name: idx_ubo_registry_ubo_proper_person; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_registry_ubo_proper_person ON "ob-poc".ubo_registry USING btree (ubo_proper_person_id);


--
-- Name: idx_ubo_registry_workflow; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_registry_workflow ON "ob-poc".ubo_registry USING btree (workflow_type);


--
-- Name: idx_values_attr_uuid; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_values_attr_uuid ON "ob-poc".attribute_values_typed USING btree (attribute_uuid);


--
-- Name: idx_verb_registry_deprecated; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_registry_deprecated ON "ob-poc".verb_registry USING btree (deprecated);


--
-- Name: idx_verb_registry_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_registry_domain ON "ob-poc".verb_registry USING btree (primary_domain);


--
-- Name: idx_verb_registry_shared; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_registry_shared ON "ob-poc".verb_registry USING btree (shared);


--
-- Name: idx_vocabulary_audit_change_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_vocabulary_audit_change_type ON "ob-poc".vocabulary_audit USING btree (change_type);


--
-- Name: idx_vocabulary_audit_created_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_vocabulary_audit_created_at ON "ob-poc".vocabulary_audit USING btree (created_at DESC);


--
-- Name: idx_vocabulary_audit_domain_verb; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_vocabulary_audit_domain_verb ON "ob-poc".vocabulary_audit USING btree (domain, verb);


--
-- Name: dsl_versions trigger_invalidate_ast_cache; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trigger_invalidate_ast_cache AFTER UPDATE ON "ob-poc".dsl_versions FOR EACH ROW EXECUTE FUNCTION "ob-poc".invalidate_ast_cache();


--
-- Name: attribute_registry trigger_update_attribute_registry_timestamp; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trigger_update_attribute_registry_timestamp BEFORE UPDATE ON "ob-poc".attribute_registry FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_attribute_registry_timestamp();


--
-- Name: attribute_values attribute_values_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE;


--
-- Name: attribute_values attribute_values_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT attribute_values_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: attribute_values_typed attribute_values_typed_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT attribute_values_typed_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(id);


--
-- Name: cbu_entity_roles cbu_entity_roles_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles cbu_entity_roles_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles cbu_entity_roles_role_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_role_id_fkey FOREIGN KEY (role_id) REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE;


--
-- Name: crud_operations crud_operations_parent_operation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".crud_operations
    ADD CONSTRAINT crud_operations_parent_operation_id_fkey FOREIGN KEY (parent_operation_id) REFERENCES "ob-poc".crud_operations(operation_id);


--
-- Name: document_attribute_mappings document_attribute_mappings_attribute_uuid_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_attribute_uuid_fkey FOREIGN KEY (attribute_uuid) REFERENCES "ob-poc".attribute_registry(uuid) ON DELETE CASCADE;


--
-- Name: document_attribute_mappings document_attribute_mappings_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id) ON DELETE CASCADE;


--
-- Name: document_catalog document_catalog_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id);


--
-- Name: document_metadata document_metadata_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_metadata
    ADD CONSTRAINT document_metadata_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE;


--
-- Name: document_metadata document_metadata_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_metadata
    ADD CONSTRAINT document_metadata_doc_id_fkey FOREIGN KEY (doc_id) REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE;


--
-- Name: document_relationships document_relationships_primary_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_primary_doc_id_fkey FOREIGN KEY (primary_doc_id) REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE;


--
-- Name: document_relationships document_relationships_related_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_related_doc_id_fkey FOREIGN KEY (related_doc_id) REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE;


--
-- Name: dsl_execution_log dsl_execution_log_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_execution_log
    ADD CONSTRAINT dsl_execution_log_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE;


--
-- Name: dsl_versions dsl_versions_domain_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES "ob-poc".dsl_domains(domain_id) ON DELETE CASCADE;


--
-- Name: dsl_versions dsl_versions_parent_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_parent_version_id_fkey FOREIGN KEY (parent_version_id) REFERENCES "ob-poc".dsl_versions(version_id);


--
-- Name: entities entities_entity_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_entity_type_id_fkey FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types(entity_type_id) ON DELETE CASCADE;


--
-- Name: entity_product_mappings entity_product_mappings_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT entity_product_mappings_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: attribute_values_typed fk_attribute_uuid; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT fk_attribute_uuid FOREIGN KEY (attribute_uuid) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: attribute_values fk_attribute_values_dsl_ob_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT fk_attribute_values_dsl_ob_id FOREIGN KEY (dsl_ob_id) REFERENCES "ob-poc".dsl_ob(version_id) ON DELETE SET NULL;


--
-- Name: cbu_creation_log fk_cbu_creation_log_cbu; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_creation_log
    ADD CONSTRAINT fk_cbu_creation_log_cbu FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles fk_cbu_entity_roles_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT fk_cbu_entity_roles_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles fk_cbu_entity_roles_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT fk_cbu_entity_roles_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles fk_cbu_entity_roles_role_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT fk_cbu_entity_roles_role_id FOREIGN KEY (role_id) REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE;


--
-- Name: document_catalog fk_document_catalog_cbu; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT fk_document_catalog_cbu FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: dsl_ob fk_dsl_ob_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_ob
    ADD CONSTRAINT fk_dsl_ob_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: entities fk_entities_entity_type_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT fk_entities_entity_type_id FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types(entity_type_id) ON DELETE CASCADE;


--
-- Name: entity_product_mappings fk_entity_product_mappings_product_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT fk_entity_product_mappings_product_id FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: entity_role_connections fk_entity_role_connections_cbu; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT fk_entity_role_connections_cbu FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: entity_role_connections fk_entity_role_connections_entity; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT fk_entity_role_connections_entity FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: orchestration_domain_sessions fk_orchestration_domain_sessions_orchestration_session_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT fk_orchestration_domain_sessions_orchestration_session_id FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_sessions fk_orchestration_sessions_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_sessions
    ADD CONSTRAINT fk_orchestration_sessions_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;


--
-- Name: orchestration_state_history fk_orchestration_state_history_orchestration_session_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_state_history
    ADD CONSTRAINT fk_orchestration_state_history_orchestration_session_id FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_tasks fk_orchestration_tasks_orchestration_session_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_tasks
    ADD CONSTRAINT fk_orchestration_tasks_orchestration_session_id FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms fk_partnership_control_mechanisms_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT fk_partnership_control_mechanisms_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms fk_partnership_control_mechanisms_partnership_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT fk_partnership_control_mechanisms_partnership_id FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: partnership_interests fk_partnership_interests_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT fk_partnership_interests_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_interests fk_partnership_interests_partnership_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT fk_partnership_interests_partnership_id FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: product_requirements fk_product_requirements_product_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_requirements
    ADD CONSTRAINT fk_product_requirements_product_id FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: product_workflows fk_product_workflows_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT fk_product_workflows_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: product_workflows fk_product_workflows_product_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT fk_product_workflows_product_id FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: service_resources fk_service_resources_resource_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT fk_service_resources_resource_id FOREIGN KEY (resource_id) REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE;


--
-- Name: service_resources fk_service_resources_service_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT fk_service_resources_service_id FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: trust_beneficiary_classes fk_trust_beneficiary_classes_trust_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_beneficiary_classes
    ADD CONSTRAINT fk_trust_beneficiary_classes_trust_id FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_parties fk_trust_parties_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT fk_trust_parties_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: trust_parties fk_trust_parties_trust_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT fk_trust_parties_trust_id FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_protector_powers fk_trust_protector_powers_trust_party_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_protector_powers
    ADD CONSTRAINT fk_trust_protector_powers_trust_party_id FOREIGN KEY (trust_party_id) REFERENCES "ob-poc".trust_parties(trust_party_id) ON DELETE CASCADE;


--
-- Name: ubo_registry fk_ubo_registry_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;


--
-- Name: ubo_registry fk_ubo_registry_subject_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_subject_entity_id FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: ubo_registry fk_ubo_registry_ubo_proper_person_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_ubo_proper_person_id FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: master_entity_xref master_entity_xref_jurisdiction_code_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".master_entity_xref
    ADD CONSTRAINT master_entity_xref_jurisdiction_code_fkey FOREIGN KEY (jurisdiction_code) REFERENCES "ob-poc".master_jurisdictions(jurisdiction_code);


--
-- Name: orchestration_domain_sessions orchestration_domain_sessions_orchestration_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT orchestration_domain_sessions_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_sessions orchestration_sessions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_sessions
    ADD CONSTRAINT orchestration_sessions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: orchestration_state_history orchestration_state_history_orchestration_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_state_history
    ADD CONSTRAINT orchestration_state_history_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_tasks orchestration_tasks_orchestration_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".orchestration_tasks
    ADD CONSTRAINT orchestration_tasks_orchestration_session_id_fkey FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: parsed_asts parsed_asts_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".parsed_asts
    ADD CONSTRAINT parsed_asts_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms partnership_control_mechanisms_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT partnership_control_mechanisms_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms partnership_control_mechanisms_partnership_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT partnership_control_mechanisms_partnership_id_fkey FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: partnership_interests partnership_interests_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_interests partnership_interests_partnership_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT partnership_interests_partnership_id_fkey FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: product_requirements product_requirements_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_requirements
    ADD CONSTRAINT product_requirements_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: product_services product_services_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: product_services product_services_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: product_workflows product_workflows_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT product_workflows_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: service_resources service_resources_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE;


--
-- Name: service_resources service_resources_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: trust_beneficiary_classes trust_beneficiary_classes_trust_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_beneficiary_classes
    ADD CONSTRAINT trust_beneficiary_classes_trust_id_fkey FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_parties trust_parties_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: trust_parties trust_parties_trust_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT trust_parties_trust_id_fkey FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_protector_powers trust_protector_powers_trust_party_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trust_protector_powers
    ADD CONSTRAINT trust_protector_powers_trust_party_id_fkey FOREIGN KEY (trust_party_id) REFERENCES "ob-poc".trust_parties(trust_party_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_subject_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_subject_entity_id_fkey FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_ubo_proper_person_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_ubo_proper_person_id_fkey FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict N5ML2PEaakwAHI2DSinDAkNMCBQ5pX8rIoIO9I2ZHcP8fhow9jQTRyovL6NuNNO

