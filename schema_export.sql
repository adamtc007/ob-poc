--
-- PostgreSQL database dump
--

\restrict sZdH9v5mL2mgep1gJRs6gFooGt56s8brvkwbFjl5tb16yaIZFaohb1JjxkmKwgk

-- Dumped from database version 17.6 (Homebrew)
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
-- Name: public; Type: SCHEMA; Schema: -; Owner: adamtc007
--

-- *not* creating schema, since initdb creates it


ALTER SCHEMA public OWNER TO adamtc007;

--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: adamtc007
--

COMMENT ON SCHEMA public IS 'Runtime API Endpoints System - Phase 1 Foundation';


--
-- Name: pg_trgm; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;


--
-- Name: EXTENSION pg_trgm; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';


--
-- Name: uuid-ossp; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;


--
-- Name: EXTENSION "uuid-ossp"; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION "uuid-ossp" IS 'generate universally unique identifiers (UUIDs)';


--
-- Name: vector; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;


--
-- Name: EXTENSION vector; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION vector IS 'vector data type and ivfflat and hnsw access methods';


--
-- Name: action_type_enum; Type: TYPE; Schema: public; Owner: adamtc007
--

CREATE TYPE public.action_type_enum AS ENUM (
    'HTTP_API',
    'BPMN_WORKFLOW',
    'MESSAGE_QUEUE',
    'DATABASE_OPERATION',
    'EXTERNAL_SERVICE'
);


ALTER TYPE public.action_type_enum OWNER TO adamtc007;

--
-- Name: execution_status_enum; Type: TYPE; Schema: public; Owner: adamtc007
--

CREATE TYPE public.execution_status_enum AS ENUM (
    'PENDING',
    'RUNNING',
    'COMPLETED',
    'FAILED',
    'CANCELLED'
);


ALTER TYPE public.execution_status_enum OWNER TO adamtc007;

--
-- Name: cleanup_demo_data(); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
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


ALTER FUNCTION "ob-poc".cleanup_demo_data() OWNER TO adamtc007;

--
-- Name: get_attribute_value(uuid, text); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
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


ALTER FUNCTION "ob-poc".get_attribute_value(p_entity_id uuid, p_attribute_id text) OWNER TO adamtc007;

--
-- Name: get_next_version_number(character varying); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
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


ALTER FUNCTION "ob-poc".get_next_version_number(domain_name_param character varying) OWNER TO adamtc007;

--
-- Name: invalidate_ast_cache(); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
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


ALTER FUNCTION "ob-poc".invalidate_ast_cache() OWNER TO adamtc007;

--
-- Name: refresh_document_type_similarities(); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
--

CREATE FUNCTION "ob-poc".refresh_document_type_similarities() RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- Delete expired entries
    DELETE FROM "ob-poc".csg_semantic_similarity_cache
    WHERE expires_at < NOW();

    -- Only proceed if vector extension is available
    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        RAISE NOTICE 'pgvector extension not installed, skipping similarity refresh';
        RETURN;
    END IF;

    -- Insert new similarities based on embeddings
    INSERT INTO "ob-poc".csg_semantic_similarity_cache
        (source_type, source_code, target_type, target_code,
         cosine_similarity, relationship_type, computed_at, expires_at)
    SELECT
        'document_type', dt1.type_code,
        'document_type', dt2.type_code,
        1 - (dt1.embedding <=> dt2.embedding) as similarity,
        'alternative',
        NOW(),
        NOW() + INTERVAL '7 days'
    FROM "ob-poc".document_types dt1
    CROSS JOIN "ob-poc".document_types dt2
    WHERE dt1.type_code != dt2.type_code
      AND dt1.embedding IS NOT NULL
      AND dt2.embedding IS NOT NULL
      AND 1 - (dt1.embedding <=> dt2.embedding) > 0.5
    ON CONFLICT (source_type, source_code, target_type, target_code)
    DO UPDATE SET
        cosine_similarity = EXCLUDED.cosine_similarity,
        computed_at = NOW(),
        expires_at = NOW() + INTERVAL '7 days';
END;
$$;


ALTER FUNCTION "ob-poc".refresh_document_type_similarities() OWNER TO adamtc007;

--
-- Name: resolve_semantic_to_uuid(text); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
--

CREATE FUNCTION "ob-poc".resolve_semantic_to_uuid(semantic_id text) RETURNS uuid
    LANGUAGE sql STABLE
    AS $$
    SELECT uuid FROM "ob-poc".attribute_registry WHERE id = semantic_id;
$$;


ALTER FUNCTION "ob-poc".resolve_semantic_to_uuid(semantic_id text) OWNER TO adamtc007;

--
-- Name: resolve_uuid_to_semantic(uuid); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
--

CREATE FUNCTION "ob-poc".resolve_uuid_to_semantic(attr_uuid uuid) RETURNS text
    LANGUAGE sql STABLE
    AS $$
    SELECT id FROM "ob-poc".attribute_registry WHERE uuid = attr_uuid;
$$;


ALTER FUNCTION "ob-poc".resolve_uuid_to_semantic(attr_uuid uuid) OWNER TO adamtc007;

--
-- Name: set_attribute_value(uuid, text, text, numeric, bigint, boolean, date, timestamp with time zone, jsonb, text); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
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


ALTER FUNCTION "ob-poc".set_attribute_value(p_entity_id uuid, p_attribute_id text, p_value_text text, p_value_number numeric, p_value_integer bigint, p_value_boolean boolean, p_value_date date, p_value_datetime timestamp with time zone, p_value_json jsonb, p_created_by text) OWNER TO adamtc007;

--
-- Name: update_attribute_registry_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
--

CREATE FUNCTION "ob-poc".update_attribute_registry_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW() AT TIME ZONE 'utc';
    RETURN NEW;
END;
$$;


ALTER FUNCTION "ob-poc".update_attribute_registry_timestamp() OWNER TO adamtc007;

--
-- Name: update_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: adamtc007
--

CREATE FUNCTION "ob-poc".update_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


ALTER FUNCTION "ob-poc".update_timestamp() OWNER TO adamtc007;

--
-- Name: ensure_entity_exists(character varying, character varying, character varying); Type: FUNCTION; Schema: public; Owner: adamtc007
--

CREATE FUNCTION public.ensure_entity_exists(p_entity_type_name character varying, p_entity_name character varying, p_external_id character varying DEFAULT NULL::character varying) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    entity_type_uuid UUID;
    entity_uuid UUID;
BEGIN
    -- Get entity type UUID
    SELECT entity_type_id INTO entity_type_uuid
    FROM "ob-poc".entity_types
    WHERE name = p_entity_type_name;

    IF entity_type_uuid IS NULL THEN
        RAISE EXCEPTION 'Entity type % not found', p_entity_type_name;
    END IF;

    -- Check if entity already exists
    SELECT entity_id INTO entity_uuid
    FROM "ob-poc".entities
    WHERE name = p_entity_name AND entity_type_id = entity_type_uuid;

    -- Create entity if it doesn't exist
    IF entity_uuid IS NULL THEN
        INSERT INTO "ob-poc".entities (entity_type_id, name, external_id)
        VALUES (entity_type_uuid, p_entity_name, p_external_id)
        RETURNING entity_id INTO entity_uuid;
    END IF;

    RETURN entity_uuid;
END;
$$;


ALTER FUNCTION public.ensure_entity_exists(p_entity_type_name character varying, p_entity_name character varying, p_external_id character varying) OWNER TO adamtc007;

--
-- Name: generate_correlation_id(text, uuid, uuid, text); Type: FUNCTION; Schema: public; Owner: adamtc007
--

CREATE FUNCTION public.generate_correlation_id(template text, cbu_id_val uuid, action_id_val uuid, resource_type_name text) RETURNS text
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    RETURN replace(
        replace(
            replace(template, '{{cbu_id}}', cbu_id_val::text),
            '{{action_id}}', action_id_val::text
        ),
        '{{resource_type}}', resource_type_name
    );
END;
$$;


ALTER FUNCTION public.generate_correlation_id(template text, cbu_id_val uuid, action_id_val uuid, resource_type_name text) OWNER TO adamtc007;

--
-- Name: generate_idempotency_key(text, text, text, uuid, uuid, uuid); Type: FUNCTION; Schema: public; Owner: adamtc007
--

CREATE FUNCTION public.generate_idempotency_key(template text, resource_type_name text, environment_name text, cbu_id_val uuid, action_id_val uuid, dsl_version_id_val uuid) RETURNS text
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    RETURN replace(
        replace(
            replace(
                replace(
                    replace(template, '{{resource_type}}', resource_type_name),
                    '{{environment}}', environment_name
                ),
                '{{cbu_id}}', cbu_id_val::text
            ),
            '{{action_id}}', action_id_val::text
        ),
        '{{dsl_version_id}}', dsl_version_id_val::text
    );
END;
$$;


ALTER FUNCTION public.generate_idempotency_key(template text, resource_type_name text, environment_name text, cbu_id_val uuid, action_id_val uuid, dsl_version_id_val uuid) OWNER TO adamtc007;

--
-- Name: get_resource_endpoint_url(text, text, text); Type: FUNCTION; Schema: public; Owner: adamtc007
--

CREATE FUNCTION public.get_resource_endpoint_url(resource_type_name text, lifecycle_action text, environment_name text DEFAULT 'production'::text) RETURNS text
    LANGUAGE plpgsql
    AS $_$
DECLARE
    endpoint_url TEXT;
BEGIN
    SELECT rte.endpoint_url INTO endpoint_url
    FROM resource_type_endpoints rte
    JOIN resource_types rt ON rte.resource_type_id = rt.resource_type_id
    WHERE rt.resource_type_name = $1
    AND rte.lifecycle_action = $2
    AND rte.environment = $3
    AND rt.active = true;

    RETURN endpoint_url;
END;
$_$;


ALTER FUNCTION public.get_resource_endpoint_url(resource_type_name text, lifecycle_action text, environment_name text) OWNER TO adamtc007;

--
-- Name: update_updated_at_column(); Type: FUNCTION; Schema: public; Owner: adamtc007
--

CREATE FUNCTION public.update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$;


ALTER FUNCTION public.update_updated_at_column() OWNER TO adamtc007;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: cbus; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".cbus (
    cbu_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    nature_purpose text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    source_of_funds text,
    client_type character varying(100),
    jurisdiction character varying(50),
    risk_context jsonb DEFAULT '{}'::jsonb,
    onboarding_context jsonb DEFAULT '{}'::jsonb,
    semantic_context jsonb DEFAULT '{}'::jsonb,
    embedding public.vector(1536),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone
);


ALTER TABLE "ob-poc".cbus OWNER TO adamtc007;

--
-- Name: COLUMN cbus.risk_context; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".cbus.risk_context IS 'Risk-related context: risk_rating, pep_exposure, sanctions_exposure, industry_codes[]';


--
-- Name: COLUMN cbus.onboarding_context; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".cbus.onboarding_context IS 'Onboarding state: stage, completed_steps[], pending_requirements[], override_rules[]';


--
-- Name: COLUMN cbus.semantic_context; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".cbus.semantic_context IS 'Rich semantic metadata: business_description, industry_keywords[], related_entities[]';


--
-- Name: kyc_investigations; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".kyc_investigations (
    investigation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid,
    investigation_type character varying(50) NOT NULL,
    risk_rating character varying(20),
    regulatory_framework jsonb,
    ubo_threshold numeric(5,2) DEFAULT 10.0,
    investigation_depth integer DEFAULT 5,
    status character varying(50) DEFAULT 'INITIATED'::character varying,
    deadline date,
    outcome character varying(50),
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone
);


ALTER TABLE "ob-poc".kyc_investigations OWNER TO adamtc007;

--
-- Name: active_investigations; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".active_investigations AS
 SELECT i.investigation_id,
    i.cbu_id,
    c.name AS cbu_name,
    i.investigation_type,
    i.status,
    i.risk_rating,
    i.deadline,
    i.created_at,
    EXTRACT(day FROM (now() - i.created_at)) AS days_open
   FROM ("ob-poc".kyc_investigations i
     JOIN "ob-poc".cbus c ON ((i.cbu_id = c.cbu_id)))
  WHERE ((i.status)::text <> 'COMPLETE'::text);


ALTER VIEW "ob-poc".active_investigations OWNER TO adamtc007;

--
-- Name: attribute_dictionary; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".attribute_dictionary (
    attribute_id uuid DEFAULT gen_random_uuid() NOT NULL,
    attr_id character varying(100) NOT NULL,
    attr_name character varying(255) NOT NULL,
    domain character varying(50) NOT NULL,
    data_type character varying(50) DEFAULT 'STRING'::character varying NOT NULL,
    description text,
    validation_pattern character varying(255),
    is_required boolean DEFAULT false,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".attribute_dictionary OWNER TO adamtc007;

--
-- Name: attribute_registry; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    applicability jsonb DEFAULT '{}'::jsonb,
    embedding public.vector(1536),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone,
    CONSTRAINT check_category CHECK ((category = ANY (ARRAY['identity'::text, 'financial'::text, 'compliance'::text, 'document'::text, 'risk'::text, 'contact'::text, 'address'::text, 'tax'::text, 'employment'::text, 'product'::text, 'entity'::text, 'ubo'::text, 'isda'::text]))),
    CONSTRAINT check_value_type CHECK ((value_type = ANY (ARRAY['string'::text, 'integer'::text, 'number'::text, 'boolean'::text, 'date'::text, 'datetime'::text, 'email'::text, 'phone'::text, 'address'::text, 'currency'::text, 'percentage'::text, 'tax_id'::text, 'json'::text])))
);


ALTER TABLE "ob-poc".attribute_registry OWNER TO adamtc007;

--
-- Name: TABLE attribute_registry; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".attribute_registry IS 'Type-safe attribute registry with string-based identifiers following the AttributeID-as-Type pattern';


--
-- Name: COLUMN attribute_registry.id; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".attribute_registry.id IS 'Attribute identifier in format attr.{category}.{name} (e.g., attr.identity.first_name)';


--
-- Name: COLUMN attribute_registry.validation_rules; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".attribute_registry.validation_rules IS 'JSON object containing validation rules: {required, min_value, max_value, min_length, max_length, pattern, allowed_values}';


--
-- Name: COLUMN attribute_registry.applicability; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".attribute_registry.applicability IS 'CSG applicability rules: entity_types[], required_for[], source_documents[], depends_on[]';


--
-- Name: attribute_uuid_map; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".attribute_uuid_map AS
 SELECT id AS semantic_id,
    uuid,
    display_name,
    category,
    value_type
   FROM "ob-poc".attribute_registry
  ORDER BY id;


ALTER VIEW "ob-poc".attribute_uuid_map OWNER TO adamtc007;

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
-- Name: TABLE attribute_values; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".attribute_values IS 'Attribute values with enforced dictionary and CBU referential integrity';


--
-- Name: attribute_values_typed; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".attribute_values_typed OWNER TO adamtc007;

--
-- Name: TABLE attribute_values_typed; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".attribute_values_typed IS 'Type-safe attribute values with proper column typing based on value_type';


--
-- Name: CONSTRAINT check_single_value ON attribute_values_typed; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON CONSTRAINT check_single_value ON "ob-poc".attribute_values_typed IS 'Ensures exactly one value column is populated per row';


--
-- Name: attribute_values_typed_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: adamtc007
--

CREATE SEQUENCE "ob-poc".attribute_values_typed_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE "ob-poc".attribute_values_typed_id_seq OWNER TO adamtc007;

--
-- Name: attribute_values_typed_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: adamtc007
--

ALTER SEQUENCE "ob-poc".attribute_values_typed_id_seq OWNED BY "ob-poc".attribute_values_typed.id;


--
-- Name: decision_conditions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".decision_conditions (
    condition_id uuid DEFAULT gen_random_uuid() NOT NULL,
    decision_id uuid NOT NULL,
    condition_type character varying(50) NOT NULL,
    description text,
    frequency character varying(50),
    due_date date,
    threshold numeric(20,2),
    currency character varying(3),
    assigned_to character varying(255),
    status character varying(50) DEFAULT 'PENDING'::character varying,
    satisfied_by character varying(255),
    satisfied_at timestamp with time zone,
    satisfaction_evidence text,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".decision_conditions OWNER TO adamtc007;

--
-- Name: kyc_decisions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".kyc_decisions (
    decision_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    investigation_id uuid,
    decision character varying(50) NOT NULL,
    decision_authority character varying(100),
    rationale text,
    decided_by character varying(255),
    decided_at timestamp with time zone DEFAULT now(),
    effective_date date DEFAULT CURRENT_DATE,
    review_date date
);


ALTER TABLE "ob-poc".kyc_decisions OWNER TO adamtc007;

--
-- Name: blocking_conditions; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".blocking_conditions AS
 SELECT dc.condition_id,
    dc.decision_id,
    kd.cbu_id,
    c.name AS cbu_name,
    dc.condition_type,
    dc.description,
    dc.due_date,
    dc.status,
        CASE
            WHEN (dc.due_date < CURRENT_DATE) THEN 'OVERDUE'::text
            WHEN (dc.due_date = CURRENT_DATE) THEN 'DUE_TODAY'::text
            ELSE 'PENDING'::text
        END AS urgency
   FROM (("ob-poc".decision_conditions dc
     JOIN "ob-poc".kyc_decisions kd ON ((dc.decision_id = kd.decision_id)))
     JOIN "ob-poc".cbus c ON ((kd.cbu_id = c.cbu_id)))
  WHERE ((dc.status)::text = 'PENDING'::text);


ALTER VIEW "ob-poc".blocking_conditions OWNER TO adamtc007;

--
-- Name: cbu_creation_log; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".cbu_creation_log OWNER TO adamtc007;

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
-- Name: cbu_resource_instances; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".cbu_resource_instances (
    instance_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    product_id uuid,
    service_id uuid,
    resource_type_id uuid,
    instance_url character varying(1024) NOT NULL,
    instance_identifier character varying(255),
    instance_name character varying(255),
    instance_config jsonb DEFAULT '{}'::jsonb,
    status character varying(50) DEFAULT 'PENDING'::character varying NOT NULL,
    requested_at timestamp with time zone DEFAULT now(),
    provisioned_at timestamp with time zone,
    activated_at timestamp with time zone,
    decommissioned_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cbu_resource_instances_status_check CHECK (((status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('PROVISIONING'::character varying)::text, ('ACTIVE'::character varying)::text, ('SUSPENDED'::character varying)::text, ('DECOMMISSIONED'::character varying)::text])))
);


ALTER TABLE "ob-poc".cbu_resource_instances OWNER TO adamtc007;

--
-- Name: TABLE cbu_resource_instances; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".cbu_resource_instances IS 'Production resource instances - the actual delivered artifacts for a CBU (accounts, connections, platform access)';


--
-- Name: COLUMN cbu_resource_instances.instance_url; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.instance_url IS 'Unique URL/endpoint for this resource instance (e.g., https://custody.bank.com/accounts/ABC123)';


--
-- Name: crud_operations; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT crud_operations_asset_type_check CHECK (((asset_type)::text = ANY (ARRAY['CBU'::text, 'ENTITY'::text, 'PARTNERSHIP'::text, 'LIMITED_COMPANY'::text, 'PROPER_PERSON'::text, 'TRUST'::text, 'ATTRIBUTE'::text, 'DOCUMENT'::text, 'CBU_ENTITY_ROLE'::text, 'OWNERSHIP'::text, 'DOCUMENT_REQUEST'::text, 'DOCUMENT_LINK'::text, 'INVESTIGATION'::text, 'RISK_ASSESSMENT_CBU'::text, 'RISK_RATING'::text, 'SCREENING_RESULT'::text, 'SCREENING_HIT_RESOLUTION'::text, 'SCREENING_BATCH'::text, 'DECISION'::text, 'DECISION_CONDITION'::text, 'MONITORING_CASE'::text, 'MONITORING_REVIEW'::text, 'MONITORING_ALERT_RULE'::text, 'MONITORING_ACTIVITY'::text, 'ATTRIBUTE_VALUE'::text, 'ATTRIBUTE_VALIDATION'::text]))),
    CONSTRAINT crud_operations_execution_status_check CHECK (((execution_status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('EXECUTING'::character varying)::text, ('COMPLETED'::character varying)::text, ('FAILED'::character varying)::text, ('ROLLED_BACK'::character varying)::text]))),
    CONSTRAINT crud_operations_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
);


ALTER TABLE "ob-poc".crud_operations OWNER TO adamtc007;

--
-- Name: TABLE crud_operations; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".crud_operations IS 'Tracks all CRUD operations generated by the agentic system with AI metadata and execution status';


--
-- Name: COLUMN crud_operations.affected_records; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".crud_operations.affected_records IS 'JSON array of record IDs affected by this operation';


--
-- Name: COLUMN crud_operations.ai_confidence; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".crud_operations.ai_confidence IS 'AI confidence score between 0.0 and 1.0 for the generated DSL';


--
-- Name: csg_rule_overrides; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".csg_rule_overrides (
    override_id uuid DEFAULT gen_random_uuid() NOT NULL,
    rule_id uuid NOT NULL,
    cbu_id uuid NOT NULL,
    override_type character varying(50) NOT NULL,
    override_params jsonb,
    approved_by character varying(255),
    approval_reason text NOT NULL,
    approved_at timestamp with time zone,
    expires_at timestamp with time zone,
    created_by character varying(255),
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT csg_rule_overrides_override_type_check CHECK (((override_type)::text = ANY (ARRAY[('disable'::character varying)::text, ('downgrade'::character varying)::text, ('modify_params'::character varying)::text, ('add_exception'::character varying)::text])))
);


ALTER TABLE "ob-poc".csg_rule_overrides OWNER TO adamtc007;

--
-- Name: csg_semantic_similarity_cache; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".csg_semantic_similarity_cache (
    cache_id uuid DEFAULT gen_random_uuid() NOT NULL,
    source_type character varying(50) NOT NULL,
    source_code character varying(100) NOT NULL,
    target_type character varying(50) NOT NULL,
    target_code character varying(100) NOT NULL,
    cosine_similarity double precision NOT NULL,
    levenshtein_distance integer,
    semantic_relatedness double precision,
    relationship_type character varying(50),
    computed_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone DEFAULT (now() + '7 days'::interval)
);


ALTER TABLE "ob-poc".csg_semantic_similarity_cache OWNER TO adamtc007;

--
-- Name: csg_validation_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".csg_validation_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    rule_code character varying(100) NOT NULL,
    rule_name character varying(255) NOT NULL,
    rule_version integer DEFAULT 1,
    target_type character varying(50) NOT NULL,
    target_code character varying(100),
    rule_type character varying(50) NOT NULL,
    rule_params jsonb NOT NULL,
    error_code character varying(10) NOT NULL,
    error_message_template text NOT NULL,
    suggestion_template text,
    severity character varying(20) DEFAULT 'error'::character varying,
    description text,
    rationale text,
    documentation_url text,
    is_active boolean DEFAULT true,
    effective_from timestamp with time zone DEFAULT now(),
    effective_until timestamp with time zone,
    created_by character varying(255),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT csg_validation_rules_rule_type_check CHECK (((rule_type)::text = ANY (ARRAY[('entity_type_constraint'::character varying)::text, ('jurisdiction_constraint'::character varying)::text, ('client_type_constraint'::character varying)::text, ('prerequisite'::character varying)::text, ('exclusion'::character varying)::text, ('co_occurrence'::character varying)::text, ('sequence'::character varying)::text, ('cardinality'::character varying)::text, ('custom'::character varying)::text]))),
    CONSTRAINT csg_validation_rules_severity_check CHECK (((severity)::text = ANY (ARRAY[('error'::character varying)::text, ('warning'::character varying)::text, ('info'::character varying)::text]))),
    CONSTRAINT csg_validation_rules_target_type_check CHECK (((target_type)::text = ANY (ARRAY[('document_type'::character varying)::text, ('attribute'::character varying)::text, ('entity_type'::character varying)::text, ('verb'::character varying)::text, ('cross_reference'::character varying)::text])))
);


ALTER TABLE "ob-poc".csg_validation_rules OWNER TO adamtc007;

--
-- Name: currencies; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".currencies (
    currency_id uuid DEFAULT gen_random_uuid() NOT NULL,
    iso_code character varying(3) NOT NULL,
    name character varying(100) NOT NULL,
    symbol character varying(10),
    decimal_places integer DEFAULT 2,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".currencies OWNER TO adamtc007;

--
-- Name: decisions; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".decisions AS
 SELECT decision_id,
    investigation_id,
    cbu_id,
        CASE
            WHEN ((decision)::text = 'ACCEPT'::text) THEN 'APPROVE'::character varying
            WHEN ((decision)::text = 'CONDITIONAL_ACCEPTANCE'::text) THEN 'CONDITIONAL_APPROVE'::character varying
            WHEN ((decision)::text = 'REJECT'::text) THEN 'REJECT'::character varying
            WHEN ((decision)::text = 'ESCALATE'::text) THEN 'ESCALATE'::character varying
            ELSE decision
        END AS decision_type,
    rationale,
    (decided_at)::date AS decision_date,
    decision_authority AS approval_level,
    review_date AS next_review_date,
    NULL::character varying AS reason_code,
    false AS is_permanent,
    NULL::date AS reapply_after,
    NULL::character varying AS escalate_to,
    NULL::character varying AS escalation_reason,
    NULL::character varying AS escalation_priority,
    NULL::date AS escalation_due_date,
    rationale AS case_summary,
    NULL::date AS defer_until,
    '[]'::jsonb AS pending_items,
    decided_by,
    decided_at AS created_at,
    decided_at AS updated_at
   FROM "ob-poc".kyc_decisions;


ALTER VIEW "ob-poc".decisions OWNER TO adamtc007;

--
-- Name: VIEW decisions; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON VIEW "ob-poc".decisions IS 'Bridge view: maps kyc_decisions to DECISION crud_asset';


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
-- Name: document_attribute_mappings; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT document_attribute_mappings_extraction_method_check CHECK (((extraction_method)::text = ANY (ARRAY[('OCR'::character varying)::text, ('MRZ'::character varying)::text, ('BARCODE'::character varying)::text, ('QR_CODE'::character varying)::text, ('FORM_FIELD'::character varying)::text, ('TABLE'::character varying)::text, ('CHECKBOX'::character varying)::text, ('SIGNATURE'::character varying)::text, ('PHOTO'::character varying)::text, ('NLP'::character varying)::text, ('AI'::character varying)::text])))
);


ALTER TABLE "ob-poc".document_attribute_mappings OWNER TO adamtc007;

--
-- Name: TABLE document_attribute_mappings; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".document_attribute_mappings IS 'Seeded with common document type to attribute mappings for KYC and onboarding';


--
-- Name: COLUMN document_attribute_mappings.extraction_method; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".document_attribute_mappings.extraction_method IS 'Method used to extract the attribute: OCR, MRZ, BARCODE, FORM_FIELD, etc.';


--
-- Name: COLUMN document_attribute_mappings.confidence_threshold; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".document_attribute_mappings.confidence_threshold IS 'Minimum confidence score (0.0-1.0) required for extraction';


--
-- Name: document_catalog; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_catalog (
    doc_id uuid DEFAULT gen_random_uuid() NOT NULL,
    file_hash_sha256 text,
    storage_key text,
    file_size_bytes bigint,
    mime_type character varying(100),
    extracted_data jsonb,
    extraction_status character varying(50) DEFAULT 'PENDING'::character varying,
    extraction_confidence numeric(5,4),
    last_extracted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    cbu_id uuid,
    document_type_id uuid,
    document_id uuid DEFAULT gen_random_uuid(),
    document_type_code character varying(100),
    document_name character varying(255),
    source_system character varying(100),
    status character varying(50) DEFAULT 'active'::character varying,
    metadata jsonb DEFAULT '{}'::jsonb
);


ALTER TABLE "ob-poc".document_catalog OWNER TO adamtc007;

--
-- Name: TABLE document_catalog; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".document_catalog IS 'Central "fact" table for all document instances. Stores file info and AI extraction results.';


--
-- Name: document_entity_links; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_entity_links (
    link_id uuid DEFAULT gen_random_uuid() NOT NULL,
    doc_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    link_type character varying(50) DEFAULT 'EVIDENCE'::character varying,
    linked_by character varying(255) DEFAULT 'system'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT document_entity_links_link_type_check CHECK (((link_type)::text = ANY (ARRAY[('EVIDENCE'::character varying)::text, ('IDENTITY'::character varying)::text, ('ADDRESS'::character varying)::text, ('FINANCIAL'::character varying)::text, ('REGULATORY'::character varying)::text, ('OTHER'::character varying)::text])))
);


ALTER TABLE "ob-poc".document_entity_links OWNER TO adamtc007;

--
-- Name: TABLE document_entity_links; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".document_entity_links IS 'Links documents to entities with typed relationship (DOCUMENT_LINK crud_asset)';


--
-- Name: document_issuers_backup; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".document_issuers_backup OWNER TO adamtc007;

--
-- Name: document_metadata; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".document_metadata OWNER TO adamtc007;

--
-- Name: TABLE document_metadata; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".document_metadata IS 'EAV table linking documents to their metadata attributes (from the dictionary). This is the critical bridge to the AttributeID-as-Type pattern.';


--
-- Name: document_relationships; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    primary_doc_id uuid NOT NULL,
    related_doc_id uuid NOT NULL,
    relationship_type character varying(100) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".document_relationships OWNER TO adamtc007;

--
-- Name: TABLE document_relationships; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".document_relationships IS 'Models M:N relationships between documents (e.g., amendments, translations).';


--
-- Name: document_requests; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_requests (
    request_id uuid DEFAULT gen_random_uuid() NOT NULL,
    investigation_id uuid NOT NULL,
    document_type character varying(100) NOT NULL,
    requested_from_entity_type character varying(50),
    requested_from_entity_id uuid,
    status character varying(50) DEFAULT 'PENDING'::character varying,
    requested_at timestamp with time zone DEFAULT now(),
    received_at timestamp with time zone,
    doc_id uuid,
    notes text
);


ALTER TABLE "ob-poc".document_requests OWNER TO adamtc007;

--
-- Name: document_types; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    updated_at timestamp with time zone DEFAULT now(),
    applicability jsonb DEFAULT '{}'::jsonb,
    semantic_context jsonb DEFAULT '{}'::jsonb,
    embedding public.vector(768),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone
);


ALTER TABLE "ob-poc".document_types OWNER TO adamtc007;

--
-- Name: COLUMN document_types.applicability; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".document_types.applicability IS 'CSG applicability rules: entity_types[], jurisdictions[], client_types[], required_for[], excludes[]';


--
-- Name: COLUMN document_types.semantic_context; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".document_types.semantic_context IS 'Rich semantic metadata: purpose, synonyms[], related_documents[], extraction_hints{}, keywords[]';


--
-- Name: COLUMN document_types.embedding; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".document_types.embedding IS 'OpenAI ada-002 or equivalent embedding of type description + semantic context';


--
-- Name: document_verifications; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".document_verifications (
    verification_id uuid DEFAULT gen_random_uuid() NOT NULL,
    doc_id uuid NOT NULL,
    verification_method character varying(100) NOT NULL,
    verification_status character varying(50) DEFAULT 'PENDING'::character varying,
    verified_by character varying(255),
    verified_at timestamp with time zone,
    confidence_score numeric(5,4),
    issues_found jsonb DEFAULT '[]'::jsonb,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".document_verifications OWNER TO adamtc007;

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
-- Name: dsl_domains; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".dsl_domains OWNER TO adamtc007;

--
-- Name: dsl_examples; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT dsl_examples_asset_type_check CHECK (((asset_type)::text = ANY (ARRAY['CBU'::text, 'ENTITY'::text, 'PARTNERSHIP'::text, 'LIMITED_COMPANY'::text, 'PROPER_PERSON'::text, 'TRUST'::text, 'ATTRIBUTE'::text, 'DOCUMENT'::text, 'CBU_ENTITY_ROLE'::text, 'OWNERSHIP'::text, 'DOCUMENT_REQUEST'::text, 'DOCUMENT_LINK'::text, 'INVESTIGATION'::text, 'RISK_ASSESSMENT_CBU'::text, 'RISK_RATING'::text, 'SCREENING_RESULT'::text, 'SCREENING_HIT_RESOLUTION'::text, 'SCREENING_BATCH'::text, 'DECISION'::text, 'DECISION_CONDITION'::text, 'MONITORING_CASE'::text, 'MONITORING_REVIEW'::text, 'MONITORING_ALERT_RULE'::text, 'MONITORING_ACTIVITY'::text, 'ATTRIBUTE_VALUE'::text, 'ATTRIBUTE_VALIDATION'::text]))),
    CONSTRAINT dsl_examples_complexity_level_check CHECK (((complexity_level)::text = ANY (ARRAY[('SIMPLE'::character varying)::text, ('MEDIUM'::character varying)::text, ('COMPLEX'::character varying)::text]))),
    CONSTRAINT dsl_examples_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
);


ALTER TABLE "ob-poc".dsl_examples OWNER TO adamtc007;

--
-- Name: TABLE dsl_examples; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_examples IS 'Curated library of natural language to DSL examples for training and context';


--
-- Name: COLUMN dsl_examples.success_rate; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_examples.success_rate IS 'Rate of successful operations when using this example (0.0 to 1.0)';


--
-- Name: dsl_execution_log; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".dsl_execution_log OWNER TO adamtc007;

--
-- Name: dsl_versions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".dsl_versions OWNER TO adamtc007;

--
-- Name: dsl_execution_summary; Type: VIEW; Schema: ob-poc; Owner: adamtc007
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


ALTER VIEW "ob-poc".dsl_execution_summary OWNER TO adamtc007;

--
-- Name: dsl_generation_log; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_generation_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid,
    user_intent text NOT NULL,
    final_valid_dsl text,
    iterations jsonb DEFAULT '[]'::jsonb NOT NULL,
    domain_name character varying(50) NOT NULL,
    session_id uuid,
    cbu_id uuid,
    model_used character varying(100),
    total_attempts integer DEFAULT 1 NOT NULL,
    success boolean DEFAULT false NOT NULL,
    total_latency_ms integer,
    total_input_tokens integer,
    total_output_tokens integer,
    created_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone
);


ALTER TABLE "ob-poc".dsl_generation_log OWNER TO adamtc007;

--
-- Name: TABLE dsl_generation_log; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_generation_log IS 'Captures agent DSL generation iterations for training data extraction and audit trail';


--
-- Name: COLUMN dsl_generation_log.user_intent; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.user_intent IS 'Natural language description of what user wanted - the input side of training pairs';


--
-- Name: COLUMN dsl_generation_log.final_valid_dsl; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.final_valid_dsl IS 'Successfully validated DSL - the output side of training pairs';


--
-- Name: COLUMN dsl_generation_log.iterations; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.iterations IS 'JSONB array of each generation attempt with prompts, responses, and validation results';


--
-- Name: COLUMN dsl_generation_log.domain_name; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.domain_name IS 'Primary domain for this generation: cbu, entity, document, etc.';


--
-- Name: COLUMN dsl_generation_log.model_used; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.model_used IS 'LLM model identifier used for generation';


--
-- Name: COLUMN dsl_generation_log.total_attempts; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.total_attempts IS 'Number of generation attempts before success or failure';


--
-- Name: COLUMN dsl_generation_log.success; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.success IS 'Whether generation ultimately succeeded';


--
-- Name: dsl_instance_versions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_instance_versions (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    version_number integer NOT NULL,
    dsl_content text NOT NULL,
    operation_type character varying(100) NOT NULL,
    compilation_status character varying(50) DEFAULT 'COMPILED'::character varying,
    ast_json jsonb,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".dsl_instance_versions OWNER TO adamtc007;

--
-- Name: dsl_instances; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_instances (
    id integer NOT NULL,
    case_id character varying(255),
    dsl_content text,
    domain character varying(100),
    operation_type character varying(100),
    status character varying(50) DEFAULT 'PROCESSED'::character varying,
    processing_time_ms bigint,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    instance_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain_name character varying(100),
    business_reference character varying(255) NOT NULL,
    current_version integer DEFAULT 1
);


ALTER TABLE "ob-poc".dsl_instances OWNER TO adamtc007;

--
-- Name: dsl_instances_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: adamtc007
--

CREATE SEQUENCE "ob-poc".dsl_instances_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE "ob-poc".dsl_instances_id_seq OWNER TO adamtc007;

--
-- Name: dsl_instances_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: adamtc007
--

ALTER SEQUENCE "ob-poc".dsl_instances_id_seq OWNED BY "ob-poc".dsl_instances.id;


--
-- Name: parsed_asts; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".parsed_asts OWNER TO adamtc007;

--
-- Name: dsl_latest_versions; Type: VIEW; Schema: ob-poc; Owner: adamtc007
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


ALTER VIEW "ob-poc".dsl_latest_versions OWNER TO adamtc007;

--
-- Name: dsl_ob; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".dsl_ob (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    dsl_text text NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text)
);


ALTER TABLE "ob-poc".dsl_ob OWNER TO adamtc007;

--
-- Name: TABLE dsl_ob; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".dsl_ob IS 'DSL documents with enforced CBU referential integrity';


--
-- Name: COLUMN dsl_ob.cbu_id; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".dsl_ob.cbu_id IS 'UUID reference to cbus table primary key';


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
-- Name: entity_crud_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT entity_crud_rules_constraint_type_check CHECK (((constraint_type)::text = ANY (ARRAY[('REQUIRED'::character varying)::text, ('UNIQUE'::character varying)::text, ('FOREIGN_KEY'::character varying)::text, ('VALIDATION'::character varying)::text, ('BUSINESS_RULE'::character varying)::text]))),
    CONSTRAINT entity_crud_rules_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
);


ALTER TABLE "ob-poc".entity_crud_rules OWNER TO adamtc007;

--
-- Name: TABLE entity_crud_rules; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".entity_crud_rules IS 'Entity-specific validation rules and constraints for CRUD operations';


--
-- Name: entity_lifecycle_status; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".entity_lifecycle_status OWNER TO adamtc007;

--
-- Name: TABLE entity_lifecycle_status; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".entity_lifecycle_status IS 'Tracks entity lifecycle states for workflow management';


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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    entity_id uuid
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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    entity_id uuid
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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    search_name text GENERATED ALWAYS AS ((((COALESCE(first_name, ''::character varying))::text || ' '::text) || (COALESCE(last_name, ''::character varying))::text)) STORED,
    entity_id uuid
);


ALTER TABLE "ob-poc".entity_proper_persons OWNER TO adamtc007;

--
-- Name: entity_role_connections; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_role_connections (
    connection_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    role_id uuid NOT NULL,
    connection_type character varying(50) NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE "ob-poc".entity_role_connections OWNER TO adamtc007;

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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    entity_id uuid
);


ALTER TABLE "ob-poc".entity_trusts OWNER TO adamtc007;

--
-- Name: entity_search_view; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".entity_search_view AS
 SELECT entity_proper_persons.proper_person_id AS id,
    'PERSON'::text AS entity_type,
    (((COALESCE(entity_proper_persons.first_name, ''::character varying))::text || ' '::text) || (COALESCE(entity_proper_persons.last_name, ''::character varying))::text) AS display_name,
    entity_proper_persons.nationality AS subtitle_1,
    (entity_proper_persons.date_of_birth)::text AS subtitle_2,
    (((COALESCE(entity_proper_persons.first_name, ''::character varying))::text || ' '::text) || (COALESCE(entity_proper_persons.last_name, ''::character varying))::text) AS search_text
   FROM "ob-poc".entity_proper_persons
  WHERE (entity_proper_persons.proper_person_id IS NOT NULL)
UNION ALL
 SELECT entity_limited_companies.limited_company_id AS id,
    'COMPANY'::text AS entity_type,
    entity_limited_companies.company_name AS display_name,
    entity_limited_companies.jurisdiction AS subtitle_1,
    entity_limited_companies.registration_number AS subtitle_2,
    entity_limited_companies.company_name AS search_text
   FROM "ob-poc".entity_limited_companies
  WHERE (entity_limited_companies.limited_company_id IS NOT NULL)
UNION ALL
 SELECT cbus.cbu_id AS id,
    'CBU'::text AS entity_type,
    cbus.name AS display_name,
    cbus.client_type AS subtitle_1,
    cbus.jurisdiction AS subtitle_2,
    cbus.name AS search_text
   FROM "ob-poc".cbus
  WHERE (cbus.cbu_id IS NOT NULL)
UNION ALL
 SELECT entity_trusts.trust_id AS id,
    'TRUST'::text AS entity_type,
    entity_trusts.trust_name AS display_name,
    entity_trusts.jurisdiction AS subtitle_1,
    NULL::text AS subtitle_2,
    entity_trusts.trust_name AS search_text
   FROM "ob-poc".entity_trusts
  WHERE (entity_trusts.trust_id IS NOT NULL);


ALTER VIEW "ob-poc".entity_search_view OWNER TO adamtc007;

--
-- Name: entity_types; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".entity_types (
    entity_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    table_name character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    type_code character varying(100),
    semantic_context jsonb DEFAULT '{}'::jsonb,
    parent_type_id uuid,
    type_hierarchy_path text[],
    embedding public.vector(768),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone
);


ALTER TABLE "ob-poc".entity_types OWNER TO adamtc007;

--
-- Name: COLUMN entity_types.semantic_context; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".entity_types.semantic_context IS 'Rich semantic metadata: category, parent_type, synonyms[], typical_documents[], typical_attributes[]';


--
-- Name: COLUMN entity_types.type_hierarchy_path; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".entity_types.type_hierarchy_path IS 'Materialized path for efficient ancestor queries, e.g., ["ENTITY", "LEGAL_ENTITY", "LIMITED_COMPANY"]';


--
-- Name: entity_validation_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT entity_validation_rules_severity_check CHECK (((severity)::text = ANY (ARRAY[('ERROR'::character varying)::text, ('WARNING'::character varying)::text, ('INFO'::character varying)::text]))),
    CONSTRAINT entity_validation_rules_validation_type_check CHECK (((validation_type)::text = ANY (ARRAY[('REQUIRED'::character varying)::text, ('FORMAT'::character varying)::text, ('RANGE'::character varying)::text, ('REFERENCE'::character varying)::text, ('CUSTOM'::character varying)::text])))
);


ALTER TABLE "ob-poc".entity_validation_rules OWNER TO adamtc007;

--
-- Name: TABLE entity_validation_rules; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".entity_validation_rules IS 'Defines validation rules for agentic CRUD operations';


--
-- Name: COLUMN entity_validation_rules.validation_rule; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".entity_validation_rules.validation_rule IS 'JSON object defining the validation logic';


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
-- Name: investigation_assignments; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".investigation_assignments (
    assignment_id uuid DEFAULT gen_random_uuid() NOT NULL,
    investigation_id uuid NOT NULL,
    assignee character varying(255) NOT NULL,
    role character varying(50),
    assigned_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".investigation_assignments OWNER TO adamtc007;

--
-- Name: investigations; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".investigations AS
 SELECT investigation_id,
    cbu_id,
    investigation_type,
    status,
    risk_rating,
    ubo_threshold,
    deadline,
    outcome,
    outcome AS outcome_rationale,
    notes AS assigned_to,
    created_at,
    updated_at,
    completed_at
   FROM "ob-poc".kyc_investigations;


ALTER VIEW "ob-poc".investigations OWNER TO adamtc007;

--
-- Name: VIEW investigations; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON VIEW "ob-poc".investigations IS 'Bridge view: maps kyc_investigations to INVESTIGATION crud_asset';


--
-- Name: master_jurisdictions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".master_jurisdictions OWNER TO adamtc007;

--
-- Name: TABLE master_jurisdictions; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".master_jurisdictions IS 'Comprehensive jurisdiction lookup table for entity formation and compliance';


--
-- Name: COLUMN master_jurisdictions.offshore_jurisdiction; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".master_jurisdictions.offshore_jurisdiction IS 'TRUE for offshore/tax haven jurisdictions';


--
-- Name: jurisdictions; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".jurisdictions AS
 SELECT jurisdiction_code AS iso_code,
    jurisdiction_name AS name,
    region,
    regulatory_framework AS description
   FROM "ob-poc".master_jurisdictions;


ALTER VIEW "ob-poc".jurisdictions OWNER TO adamtc007;

--
-- Name: master_entity_xref; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT master_entity_xref_entity_status_check CHECK (((entity_status)::text = ANY (ARRAY[('ACTIVE'::character varying)::text, ('INACTIVE'::character varying)::text, ('DISSOLVED'::character varying)::text, ('SUSPENDED'::character varying)::text]))),
    CONSTRAINT master_entity_xref_entity_type_check CHECK (((entity_type)::text = ANY (ARRAY[('PARTNERSHIP'::character varying)::text, ('LIMITED_COMPANY'::character varying)::text, ('PROPER_PERSON'::character varying)::text, ('TRUST'::character varying)::text])))
);


ALTER TABLE "ob-poc".master_entity_xref OWNER TO adamtc007;

--
-- Name: TABLE master_entity_xref; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".master_entity_xref IS 'Master cross-reference table linking all entity types with unified metadata';


--
-- Name: COLUMN master_entity_xref.regulatory_numbers; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".master_entity_xref.regulatory_numbers IS 'JSON object storing various regulatory identification numbers';


--
-- Name: monitoring_activities; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".monitoring_activities (
    activity_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    cbu_id uuid NOT NULL,
    activity_type character varying(30) NOT NULL,
    description text NOT NULL,
    reference_id character varying(255),
    reference_type character varying(50),
    recorded_by character varying(255) NOT NULL,
    recorded_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT monitoring_activities_activity_type_check CHECK (((activity_type)::text = ANY (ARRAY[('CLIENT_CONTACT'::character varying)::text, ('DOCUMENT_UPDATE'::character varying)::text, ('SCREENING_RUN'::character varying)::text, ('TRANSACTION_REVIEW'::character varying)::text, ('RISK_ASSESSMENT'::character varying)::text, ('INTERNAL_NOTE'::character varying)::text, ('OTHER'::character varying)::text])))
);


ALTER TABLE "ob-poc".monitoring_activities OWNER TO adamtc007;

--
-- Name: TABLE monitoring_activities; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".monitoring_activities IS 'Activity log for monitoring cases (MONITORING_ACTIVITY crud_asset)';


--
-- Name: monitoring_alert_rules; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".monitoring_alert_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    rule_type character varying(30) NOT NULL,
    rule_name character varying(255) NOT NULL,
    description text,
    threshold jsonb NOT NULL,
    is_active boolean DEFAULT true,
    last_triggered_at timestamp with time zone,
    trigger_count integer DEFAULT 0,
    created_by character varying(255) DEFAULT 'system'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT monitoring_alert_rules_rule_type_check CHECK (((rule_type)::text = ANY (ARRAY[('TRANSACTION_VOLUME'::character varying)::text, ('TRANSACTION_VALUE'::character varying)::text, ('JURISDICTION_ACTIVITY'::character varying)::text, ('COUNTERPARTY_TYPE'::character varying)::text, ('PATTERN_DEVIATION'::character varying)::text, ('CUSTOM'::character varying)::text])))
);


ALTER TABLE "ob-poc".monitoring_alert_rules OWNER TO adamtc007;

--
-- Name: TABLE monitoring_alert_rules; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".monitoring_alert_rules IS 'Custom alert rules for ongoing monitoring (MONITORING_ALERT_RULE crud_asset)';


--
-- Name: monitoring_cases; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".monitoring_cases (
    case_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_type character varying(30) NOT NULL,
    status character varying(30) DEFAULT 'OPEN'::character varying,
    close_reason character varying(40),
    close_notes text,
    retention_period_years integer DEFAULT 7,
    closed_at timestamp with time zone,
    closed_by character varying(255),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT monitoring_cases_case_type_check CHECK (((case_type)::text = ANY (ARRAY[('ONGOING_MONITORING'::character varying)::text, ('TRIGGERED_REVIEW'::character varying)::text, ('PERIODIC_REVIEW'::character varying)::text]))),
    CONSTRAINT monitoring_cases_close_reason_check CHECK (((close_reason)::text = ANY (ARRAY[('ACCOUNT_CLOSED'::character varying)::text, ('CLIENT_EXITED'::character varying)::text, ('RELATIONSHIP_TERMINATED'::character varying)::text, ('MERGED_WITH_OTHER'::character varying)::text, ('REGULATORY_ORDER'::character varying)::text, ('OTHER'::character varying)::text]))),
    CONSTRAINT monitoring_cases_retention_period_years_check CHECK (((retention_period_years >= 5) AND (retention_period_years <= 25))),
    CONSTRAINT monitoring_cases_status_check CHECK (((status)::text = ANY (ARRAY[('OPEN'::character varying)::text, ('UNDER_REVIEW'::character varying)::text, ('ESCALATED'::character varying)::text, ('CLOSED'::character varying)::text])))
);


ALTER TABLE "ob-poc".monitoring_cases OWNER TO adamtc007;

--
-- Name: TABLE monitoring_cases; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".monitoring_cases IS 'Ongoing monitoring cases for CBUs (MONITORING_CASE crud_asset)';


--
-- Name: monitoring_events; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".monitoring_events (
    event_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    event_type character varying(50) NOT NULL,
    description text,
    severity character varying(20),
    requires_review boolean DEFAULT false,
    reviewed_by character varying(255),
    reviewed_at timestamp with time zone,
    review_outcome character varying(50),
    review_notes text,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".monitoring_events OWNER TO adamtc007;

--
-- Name: monitoring_reviews; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".monitoring_reviews (
    review_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    cbu_id uuid NOT NULL,
    review_type character varying(30) NOT NULL,
    trigger_type character varying(30),
    trigger_reference_id character varying(255),
    due_date date NOT NULL,
    risk_based_frequency character varying(20),
    scope jsonb DEFAULT '["FULL"]'::jsonb,
    status character varying(30) DEFAULT 'SCHEDULED'::character varying,
    outcome character varying(30),
    findings text,
    next_review_date date,
    actions jsonb DEFAULT '[]'::jsonb,
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    completed_by character varying(255),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT monitoring_reviews_outcome_check CHECK (((outcome)::text = ANY (ARRAY[('NO_CHANGE'::character varying)::text, ('RISK_INCREASED'::character varying)::text, ('RISK_DECREASED'::character varying)::text, ('ESCALATED'::character varying)::text, ('EXIT_RECOMMENDED'::character varying)::text, ('ENHANCED_MONITORING'::character varying)::text]))),
    CONSTRAINT monitoring_reviews_review_type_check CHECK (((review_type)::text = ANY (ARRAY[('PERIODIC'::character varying)::text, ('ANNUAL'::character varying)::text, ('ENHANCED_PERIODIC'::character varying)::text, ('SIMPLIFIED_PERIODIC'::character varying)::text]))),
    CONSTRAINT monitoring_reviews_risk_based_frequency_check CHECK (((risk_based_frequency)::text = ANY (ARRAY[('ANNUAL'::character varying)::text, ('BIANNUAL'::character varying)::text, ('QUARTERLY'::character varying)::text, ('MONTHLY'::character varying)::text]))),
    CONSTRAINT monitoring_reviews_status_check CHECK (((status)::text = ANY (ARRAY[('SCHEDULED'::character varying)::text, ('IN_PROGRESS'::character varying)::text, ('COMPLETED'::character varying)::text, ('OVERDUE'::character varying)::text, ('CANCELLED'::character varying)::text]))),
    CONSTRAINT monitoring_reviews_trigger_type_check CHECK (((trigger_type)::text = ANY (ARRAY[('ADVERSE_MEDIA'::character varying)::text, ('SANCTIONS_ALERT'::character varying)::text, ('TRANSACTION_ALERT'::character varying)::text, ('OWNERSHIP_CHANGE'::character varying)::text, ('REGULATORY_CHANGE'::character varying)::text, ('CLIENT_REQUEST'::character varying)::text, ('INTERNAL_REFERRAL'::character varying)::text, ('SCREENING_HIT'::character varying)::text, ('OTHER'::character varying)::text])))
);


ALTER TABLE "ob-poc".monitoring_reviews OWNER TO adamtc007;

--
-- Name: TABLE monitoring_reviews; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".monitoring_reviews IS 'Periodic and triggered reviews for ongoing monitoring (MONITORING_REVIEW crud_asset)';


--
-- Name: monitoring_setup; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".monitoring_setup (
    setup_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    monitoring_level character varying(50) NOT NULL,
    components jsonb,
    active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".monitoring_setup OWNER TO adamtc007;

--
-- Name: onboarding_products; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".onboarding_products (
    onboarding_product_id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id uuid NOT NULL,
    product_id uuid NOT NULL,
    selection_order integer,
    selected_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".onboarding_products OWNER TO adamtc007;

--
-- Name: onboarding_requests; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".onboarding_requests (
    request_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    request_state character varying(50) DEFAULT 'draft'::character varying NOT NULL,
    dsl_draft text,
    dsl_version integer DEFAULT 1,
    current_phase character varying(100),
    phase_metadata jsonb,
    validation_errors jsonb,
    created_by character varying(255),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone,
    CONSTRAINT onboarding_requests_request_state_check CHECK (((request_state)::text = ANY (ARRAY[('draft'::character varying)::text, ('products_selected'::character varying)::text, ('services_discovered'::character varying)::text, ('services_configured'::character varying)::text, ('resources_allocated'::character varying)::text, ('complete'::character varying)::text])))
);


ALTER TABLE "ob-poc".onboarding_requests OWNER TO adamtc007;

--
-- Name: onboarding_resource_allocations; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".onboarding_resource_allocations (
    allocation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id uuid NOT NULL,
    service_id uuid NOT NULL,
    resource_id uuid NOT NULL,
    handles_options jsonb,
    required_attributes uuid[],
    allocation_status character varying(50) DEFAULT 'pending'::character varying,
    allocated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".onboarding_resource_allocations OWNER TO adamtc007;

--
-- Name: onboarding_service_configs; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".onboarding_service_configs (
    config_id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id uuid NOT NULL,
    service_id uuid NOT NULL,
    option_selections jsonb NOT NULL,
    is_valid boolean DEFAULT false,
    validation_messages jsonb,
    configured_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".onboarding_service_configs OWNER TO adamtc007;

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
-- Name: overdue_reviews; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".overdue_reviews AS
 SELECT mr.review_id,
    mr.cbu_id,
    c.name AS cbu_name,
    mr.review_type,
    mr.due_date,
    mr.status,
    (CURRENT_DATE - mr.due_date) AS days_overdue
   FROM ("ob-poc".monitoring_reviews mr
     JOIN "ob-poc".cbus c ON ((mr.cbu_id = c.cbu_id)))
  WHERE ((mr.due_date < CURRENT_DATE) AND ((mr.status)::text = ANY (ARRAY[('SCHEDULED'::character varying)::text, ('IN_PROGRESS'::character varying)::text])));


ALTER VIEW "ob-poc".overdue_reviews OWNER TO adamtc007;

--
-- Name: ownership_relationships; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".ownership_relationships (
    ownership_id uuid DEFAULT gen_random_uuid() NOT NULL,
    owner_entity_id uuid NOT NULL,
    owned_entity_id uuid NOT NULL,
    ownership_type character varying(30) NOT NULL,
    ownership_percent numeric(5,2) NOT NULL,
    effective_from date DEFAULT CURRENT_DATE,
    effective_to date,
    evidence_doc_id uuid,
    notes text,
    created_by character varying(255) DEFAULT 'system'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT ownership_not_self CHECK ((owner_entity_id <> owned_entity_id)),
    CONSTRAINT ownership_relationships_ownership_percent_check CHECK (((ownership_percent >= (0)::numeric) AND (ownership_percent <= (100)::numeric))),
    CONSTRAINT ownership_relationships_ownership_type_check CHECK (((ownership_type)::text = ANY (ARRAY[('DIRECT'::character varying)::text, ('INDIRECT'::character varying)::text, ('BENEFICIAL'::character varying)::text]))),
    CONSTRAINT ownership_temporal CHECK (((effective_to IS NULL) OR (effective_to > effective_from)))
);


ALTER TABLE "ob-poc".ownership_relationships OWNER TO adamtc007;

--
-- Name: TABLE ownership_relationships; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".ownership_relationships IS 'Tracks ownership relationships between entities for UBO analysis (OWNERSHIP crud_asset)';


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
    service_id uuid NOT NULL,
    is_mandatory boolean DEFAULT false,
    is_default boolean DEFAULT false,
    display_order integer,
    configuration jsonb
);


ALTER TABLE "ob-poc".product_services OWNER TO adamtc007;

--
-- Name: product_workflows; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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


ALTER TABLE "ob-poc".product_workflows OWNER TO adamtc007;

--
-- Name: COLUMN product_workflows.cbu_id; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".product_workflows.cbu_id IS 'UUID reference to cbus table primary key';


--
-- Name: products; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".products (
    product_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    product_code character varying(50),
    product_category character varying(100),
    regulatory_framework character varying(100),
    min_asset_requirement numeric(20,2),
    is_active boolean DEFAULT true,
    metadata jsonb
);


ALTER TABLE "ob-poc".products OWNER TO adamtc007;

--
-- Name: rag_embeddings; Type: TABLE; Schema: ob-poc; Owner: adamtc007
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
    CONSTRAINT rag_embeddings_content_type_check CHECK (((content_type)::text = ANY (ARRAY[('SCHEMA'::character varying)::text, ('EXAMPLE'::character varying)::text, ('ATTRIBUTE'::character varying)::text, ('RULE'::character varying)::text, ('GRAMMAR'::character varying)::text, ('VERB_PATTERN'::character varying)::text])))
);


ALTER TABLE "ob-poc".rag_embeddings OWNER TO adamtc007;

--
-- Name: TABLE rag_embeddings; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".rag_embeddings IS 'Vector embeddings for RAG context retrieval in agentic CRUD operations';


--
-- Name: COLUMN rag_embeddings.embedding_data; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".rag_embeddings.embedding_data IS 'Vector embedding stored as JSON until pgvector extension is available';


--
-- Name: referential_integrity_check; Type: VIEW; Schema: ob-poc; Owner: adamtc007
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
 SELECT table_name,
    column_name,
    orphaned_value,
    issue
   FROM integrity_issues;


ALTER VIEW "ob-poc".referential_integrity_check OWNER TO adamtc007;

--
-- Name: resource_attribute_requirements; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".resource_attribute_requirements (
    requirement_id uuid DEFAULT gen_random_uuid() NOT NULL,
    resource_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    resource_field_name character varying(255),
    is_mandatory boolean DEFAULT true,
    transformation_rule jsonb,
    validation_override jsonb,
    default_value text,
    display_order integer DEFAULT 0
);


ALTER TABLE "ob-poc".resource_attribute_requirements OWNER TO adamtc007;

--
-- Name: resource_instance_attributes; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".resource_instance_attributes (
    value_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    value_text character varying,
    value_number numeric,
    value_boolean boolean,
    value_date date,
    value_timestamp timestamp with time zone,
    value_json jsonb,
    state character varying(50) DEFAULT 'proposed'::character varying,
    source jsonb,
    observed_at timestamp with time zone DEFAULT now(),
    CONSTRAINT resource_instance_attributes_state_check CHECK (((state)::text = ANY (ARRAY[('proposed'::character varying)::text, ('confirmed'::character varying)::text, ('derived'::character varying)::text, ('system'::character varying)::text])))
);


ALTER TABLE "ob-poc".resource_instance_attributes OWNER TO adamtc007;

--
-- Name: TABLE resource_instance_attributes; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".resource_instance_attributes IS 'Attribute values for resource instances - dense storage (row exists = value set)';


--
-- Name: risk_assessments; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".risk_assessments (
    assessment_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid,
    entity_id uuid,
    investigation_id uuid,
    assessment_type character varying(50) NOT NULL,
    rating character varying(20),
    factors jsonb,
    methodology character varying(50),
    rationale text,
    assessed_by character varying(255),
    assessed_at timestamp with time zone DEFAULT now(),
    CONSTRAINT risk_assessments_check CHECK (((cbu_id IS NOT NULL) OR (entity_id IS NOT NULL)))
);


ALTER TABLE "ob-poc".risk_assessments OWNER TO adamtc007;

--
-- Name: risk_flags; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".risk_flags (
    flag_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid,
    entity_id uuid,
    investigation_id uuid,
    flag_type character varying(50) NOT NULL,
    description text,
    status character varying(50) DEFAULT 'ACTIVE'::character varying,
    flagged_by character varying(255),
    flagged_at timestamp with time zone DEFAULT now(),
    resolved_by character varying(255),
    resolved_at timestamp with time zone,
    resolution_notes text,
    CONSTRAINT risk_flags_check CHECK (((cbu_id IS NOT NULL) OR (entity_id IS NOT NULL)))
);


ALTER TABLE "ob-poc".risk_flags OWNER TO adamtc007;

--
-- Name: risk_rating_changes; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".risk_rating_changes (
    change_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    review_id uuid,
    previous_rating character varying(20),
    new_rating character varying(20) NOT NULL,
    change_reason character varying(30) NOT NULL,
    rationale text NOT NULL,
    effective_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    changed_by character varying(255) NOT NULL,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT risk_rating_changes_change_reason_check CHECK (((change_reason)::text = ANY (ARRAY[('PERIODIC_REVIEW'::character varying)::text, ('TRIGGER_EVENT'::character varying)::text, ('OWNERSHIP_CHANGE'::character varying)::text, ('JURISDICTION_CHANGE'::character varying)::text, ('PRODUCT_CHANGE'::character varying)::text, ('SCREENING_RESULT'::character varying)::text, ('TRANSACTION_PATTERN'::character varying)::text, ('REGULATORY_CHANGE'::character varying)::text, ('OTHER'::character varying)::text]))),
    CONSTRAINT risk_rating_changes_new_rating_check CHECK (((new_rating)::text = ANY (ARRAY[('LOW'::character varying)::text, ('MEDIUM'::character varying)::text, ('MEDIUM_HIGH'::character varying)::text, ('HIGH'::character varying)::text, ('VERY_HIGH'::character varying)::text, ('PROHIBITED'::character varying)::text]))),
    CONSTRAINT risk_rating_changes_previous_rating_check CHECK (((previous_rating)::text = ANY (ARRAY[('LOW'::character varying)::text, ('MEDIUM'::character varying)::text, ('MEDIUM_HIGH'::character varying)::text, ('HIGH'::character varying)::text, ('VERY_HIGH'::character varying)::text, ('PROHIBITED'::character varying)::text])))
);


ALTER TABLE "ob-poc".risk_rating_changes OWNER TO adamtc007;

--
-- Name: TABLE risk_rating_changes; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".risk_rating_changes IS 'Audit trail of risk rating changes during monitoring';


--
-- Name: risk_ratings; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".risk_ratings (
    rating_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    rating character varying(20) NOT NULL,
    previous_rating character varying(20),
    rationale text,
    assessment_id uuid,
    effective_from timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    effective_to timestamp with time zone,
    set_by character varying(255) DEFAULT 'system'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT risk_ratings_previous_rating_check CHECK (((previous_rating)::text = ANY (ARRAY[('LOW'::character varying)::text, ('MEDIUM'::character varying)::text, ('MEDIUM_HIGH'::character varying)::text, ('HIGH'::character varying)::text, ('VERY_HIGH'::character varying)::text, ('PROHIBITED'::character varying)::text]))),
    CONSTRAINT risk_ratings_rating_check CHECK (((rating)::text = ANY (ARRAY[('LOW'::character varying)::text, ('MEDIUM'::character varying)::text, ('MEDIUM_HIGH'::character varying)::text, ('HIGH'::character varying)::text, ('VERY_HIGH'::character varying)::text, ('PROHIBITED'::character varying)::text])))
);


ALTER TABLE "ob-poc".risk_ratings OWNER TO adamtc007;

--
-- Name: TABLE risk_ratings; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".risk_ratings IS 'Historical record of risk ratings assigned to CBUs (RISK_RATING crud_asset)';


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
-- Name: scheduled_reviews; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".scheduled_reviews (
    review_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    review_type character varying(50) NOT NULL,
    due_date date NOT NULL,
    assigned_to character varying(255),
    status character varying(50) DEFAULT 'SCHEDULED'::character varying,
    completed_by character varying(255),
    completed_at timestamp with time zone,
    completion_notes text,
    next_review_id uuid,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".scheduled_reviews OWNER TO adamtc007;

--
-- Name: schema_changes; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".schema_changes (
    change_id uuid DEFAULT gen_random_uuid() NOT NULL,
    change_type character varying(50) NOT NULL,
    description text NOT NULL,
    script_name character varying(255),
    applied_at timestamp with time zone DEFAULT now(),
    applied_by character varying(100) DEFAULT CURRENT_USER
);


ALTER TABLE "ob-poc".schema_changes OWNER TO adamtc007;

--
-- Name: screening_batch_results; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".screening_batch_results (
    batch_id uuid NOT NULL,
    screening_id uuid NOT NULL
);


ALTER TABLE "ob-poc".screening_batch_results OWNER TO adamtc007;

--
-- Name: screening_batches; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".screening_batches (
    batch_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid,
    investigation_id uuid,
    screen_types jsonb DEFAULT '["PEP", "SANCTIONS"]'::jsonb NOT NULL,
    entity_count integer DEFAULT 0,
    completed_count integer DEFAULT 0,
    hit_count integer DEFAULT 0,
    status character varying(30) DEFAULT 'PENDING'::character varying,
    match_threshold numeric(5,2) DEFAULT 85.0,
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    error_message text,
    created_by character varying(255) DEFAULT 'system'::character varying,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT screening_batches_status_check CHECK (((status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('IN_PROGRESS'::character varying)::text, ('COMPLETED'::character varying)::text, ('FAILED'::character varying)::text, ('CANCELLED'::character varying)::text])))
);


ALTER TABLE "ob-poc".screening_batches OWNER TO adamtc007;

--
-- Name: TABLE screening_batches; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".screening_batches IS 'Batch screening jobs for multiple entities (SCREENING_BATCH crud_asset)';


--
-- Name: screening_hit_resolutions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".screening_hit_resolutions (
    resolution_id uuid DEFAULT gen_random_uuid() NOT NULL,
    screening_id uuid NOT NULL,
    hit_reference character varying(255),
    ubo_id uuid,
    resolution character varying(30) NOT NULL,
    dismiss_reason character varying(30),
    rationale text NOT NULL,
    evidence_refs jsonb DEFAULT '[]'::jsonb,
    notes text,
    resolved_by character varying(255) NOT NULL,
    resolved_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    reviewed_by character varying(255),
    reviewed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT screening_hit_resolutions_dismiss_reason_check CHECK (((dismiss_reason)::text = ANY (ARRAY[('NAME_ONLY_MATCH'::character varying)::text, ('DIFFERENT_DOB'::character varying)::text, ('DIFFERENT_NATIONALITY'::character varying)::text, ('DIFFERENT_JURISDICTION'::character varying)::text, ('DECEASED'::character varying)::text, ('DELISTED'::character varying)::text, ('OTHER'::character varying)::text]))),
    CONSTRAINT screening_hit_resolutions_resolution_check CHECK (((resolution)::text = ANY (ARRAY[('TRUE_MATCH'::character varying)::text, ('FALSE_POSITIVE'::character varying)::text, ('INCONCLUSIVE'::character varying)::text, ('ESCALATE'::character varying)::text])))
);


ALTER TABLE "ob-poc".screening_hit_resolutions OWNER TO adamtc007;

--
-- Name: TABLE screening_hit_resolutions; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".screening_hit_resolutions IS 'Resolution decisions for screening hits (SCREENING_HIT_RESOLUTION crud_asset)';


--
-- Name: screening_lists; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".screening_lists (
    screening_list_id uuid DEFAULT gen_random_uuid() NOT NULL,
    list_code character varying(50) NOT NULL,
    list_name character varying(255) NOT NULL,
    list_type character varying(50) NOT NULL,
    provider character varying(100),
    description text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE "ob-poc".screening_lists OWNER TO adamtc007;

--
-- Name: screenings; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".screenings (
    screening_id uuid DEFAULT gen_random_uuid() NOT NULL,
    investigation_id uuid,
    entity_id uuid NOT NULL,
    screening_type character varying(50) NOT NULL,
    databases jsonb,
    lists jsonb,
    include_rca boolean DEFAULT false,
    search_depth character varying(20),
    languages jsonb,
    status character varying(50) DEFAULT 'PENDING'::character varying,
    result character varying(50),
    match_details jsonb,
    resolution character varying(50),
    resolution_rationale text,
    screened_at timestamp with time zone DEFAULT now(),
    reviewed_by character varying(255),
    resolved_by character varying(255),
    resolved_at timestamp with time zone
);


ALTER TABLE "ob-poc".screenings OWNER TO adamtc007;

--
-- Name: screening_results; Type: VIEW; Schema: ob-poc; Owner: adamtc007
--

CREATE VIEW "ob-poc".screening_results AS
 SELECT screening_id AS result_id,
    entity_id,
    screening_type AS screen_type,
        CASE
            WHEN (databases IS NOT NULL) THEN (databases ->> 0)
            ELSE 'INTERNAL'::text
        END AS provider,
    85.0 AS match_threshold,
        CASE
            WHEN ((result)::text = 'MATCH'::text) THEN 1
            WHEN ((result)::text = 'POTENTIAL_MATCH'::text) THEN 1
            ELSE 0
        END AS hit_count,
    NULL::numeric AS highest_match_score,
    match_details AS raw_response,
    '[]'::jsonb AS categories,
    NULL::integer AS lookback_months,
    screened_at,
    NULL::timestamp with time zone AS expires_at,
    screened_at AS created_at
   FROM "ob-poc".screenings;


ALTER VIEW "ob-poc".screening_results OWNER TO adamtc007;

--
-- Name: VIEW screening_results; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON VIEW "ob-poc".screening_results IS 'Bridge view: maps screenings to SCREENING_RESULT crud_asset';


--
-- Name: service_delivery_map; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_delivery_map (
    delivery_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    product_id uuid NOT NULL,
    service_id uuid NOT NULL,
    instance_id uuid,
    service_config jsonb DEFAULT '{}'::jsonb,
    delivery_status character varying(50) DEFAULT 'PENDING'::character varying,
    requested_at timestamp with time zone DEFAULT now(),
    started_at timestamp with time zone,
    delivered_at timestamp with time zone,
    failed_at timestamp with time zone,
    failure_reason text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT service_delivery_map_delivery_status_check CHECK (((delivery_status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('IN_PROGRESS'::character varying)::text, ('DELIVERED'::character varying)::text, ('FAILED'::character varying)::text, ('CANCELLED'::character varying)::text])))
);


ALTER TABLE "ob-poc".service_delivery_map OWNER TO adamtc007;

--
-- Name: TABLE service_delivery_map; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".service_delivery_map IS 'Tracks service delivery for CBU onboarding - links CBU -> Product -> Service -> Instance';


--
-- Name: service_discovery_cache; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_discovery_cache (
    discovery_id uuid DEFAULT gen_random_uuid() NOT NULL,
    product_id uuid,
    discovered_at timestamp with time zone DEFAULT now(),
    services_available jsonb,
    resource_availability jsonb,
    ttl_seconds integer DEFAULT 3600
);


ALTER TABLE "ob-poc".service_discovery_cache OWNER TO adamtc007;

--
-- Name: service_option_choices; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_option_choices (
    choice_id uuid DEFAULT gen_random_uuid() NOT NULL,
    option_def_id uuid NOT NULL,
    choice_value character varying(255) NOT NULL,
    choice_label character varying(255),
    choice_metadata jsonb,
    is_default boolean DEFAULT false,
    is_active boolean DEFAULT true,
    display_order integer,
    requires_options jsonb,
    excludes_options jsonb
);


ALTER TABLE "ob-poc".service_option_choices OWNER TO adamtc007;

--
-- Name: service_option_definitions; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_option_definitions (
    option_def_id uuid DEFAULT gen_random_uuid() NOT NULL,
    service_id uuid NOT NULL,
    option_key character varying(100) NOT NULL,
    option_label character varying(255),
    option_type character varying(50) NOT NULL,
    validation_rules jsonb,
    is_required boolean DEFAULT false,
    display_order integer,
    help_text text,
    CONSTRAINT service_option_definitions_option_type_check CHECK (((option_type)::text = ANY (ARRAY[('single_select'::character varying)::text, ('multi_select'::character varying)::text, ('numeric'::character varying)::text, ('boolean'::character varying)::text, ('text'::character varying)::text])))
);


ALTER TABLE "ob-poc".service_option_definitions OWNER TO adamtc007;

--
-- Name: service_resource_capabilities; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_resource_capabilities (
    capability_id uuid DEFAULT gen_random_uuid() NOT NULL,
    service_id uuid NOT NULL,
    resource_id uuid NOT NULL,
    supported_options jsonb NOT NULL,
    priority integer DEFAULT 100,
    cost_factor numeric(10,4) DEFAULT 1.0,
    performance_rating integer,
    resource_config jsonb,
    is_active boolean DEFAULT true,
    CONSTRAINT service_resource_capabilities_performance_rating_check CHECK (((performance_rating >= 1) AND (performance_rating <= 5)))
);


ALTER TABLE "ob-poc".service_resource_capabilities OWNER TO adamtc007;

--
-- Name: service_resource_types; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".service_resource_types (
    resource_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    owner character varying(255) NOT NULL,
    dictionary_group character varying(100),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    resource_code character varying(50),
    resource_type character varying(100),
    vendor character varying(255),
    version character varying(50),
    api_endpoint text,
    api_version character varying(20),
    authentication_method character varying(50),
    authentication_config jsonb,
    capabilities jsonb,
    capacity_limits jsonb,
    maintenance_windows jsonb,
    is_active boolean DEFAULT true
);


ALTER TABLE "ob-poc".service_resource_types OWNER TO adamtc007;

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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    service_code character varying(50),
    service_category character varying(100),
    sla_definition jsonb,
    is_active boolean DEFAULT true
);


ALTER TABLE "ob-poc".services OWNER TO adamtc007;

--
-- Name: taxonomy_audit_log; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".taxonomy_audit_log (
    audit_id uuid DEFAULT gen_random_uuid() NOT NULL,
    operation character varying(100) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid NOT NULL,
    user_id character varying(255) NOT NULL,
    before_state jsonb,
    after_state jsonb,
    metadata jsonb,
    success boolean DEFAULT true NOT NULL,
    error_message text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE "ob-poc".taxonomy_audit_log OWNER TO adamtc007;

--
-- Name: TABLE taxonomy_audit_log; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".taxonomy_audit_log IS 'Audit trail for all taxonomy operations including product, service, and resource management';


--
-- Name: COLUMN taxonomy_audit_log.operation; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.operation IS 'Type of operation performed (e.g., create_product, configure_service, allocate_resource)';


--
-- Name: COLUMN taxonomy_audit_log.entity_type; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.entity_type IS 'Type of entity being operated on (e.g., product, service, onboarding_request)';


--
-- Name: COLUMN taxonomy_audit_log.before_state; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.before_state IS 'State of the entity before the operation (null for create operations)';


--
-- Name: COLUMN taxonomy_audit_log.after_state; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.after_state IS 'State of the entity after the operation (null for delete operations)';


--
-- Name: COLUMN taxonomy_audit_log.metadata; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_audit_log.metadata IS 'Additional context about the operation';


--
-- Name: taxonomy_crud_log; Type: TABLE; Schema: ob-poc; Owner: adamtc007
--

CREATE TABLE "ob-poc".taxonomy_crud_log (
    operation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    operation_type character varying(20) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid,
    natural_language_input text,
    parsed_dsl text,
    execution_result jsonb,
    success boolean DEFAULT false,
    error_message text,
    user_id character varying(255),
    created_at timestamp with time zone DEFAULT now(),
    execution_time_ms integer
);


ALTER TABLE "ob-poc".taxonomy_crud_log OWNER TO adamtc007;

--
-- Name: TABLE taxonomy_crud_log; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".taxonomy_crud_log IS 'Audit log for taxonomy CRUD operations';


--
-- Name: COLUMN taxonomy_crud_log.operation_type; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_crud_log.operation_type IS 'CREATE, READ, UPDATE, DELETE';


--
-- Name: COLUMN taxonomy_crud_log.entity_type; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON COLUMN "ob-poc".taxonomy_crud_log.entity_type IS 'product, service, resource, onboarding';


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
-- Name: TABLE ubo_registry; Type: COMMENT; Schema: ob-poc; Owner: adamtc007
--

COMMENT ON TABLE "ob-poc".ubo_registry IS 'UBO identification results with proper entity referential integrity';


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
-- Name: action_execution_attempts; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.action_execution_attempts (
    attempt_id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    execution_id uuid,
    attempt_no integer NOT NULL,
    started_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone,
    status public.execution_status_enum NOT NULL,
    request_payload jsonb,
    response_payload jsonb,
    error_details jsonb,
    http_status integer,
    duration_ms integer,
    endpoint_url text,
    request_headers jsonb,
    response_headers jsonb
);


ALTER TABLE public.action_execution_attempts OWNER TO adamtc007;

--
-- Name: action_executions; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.action_executions (
    execution_id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    action_id uuid,
    cbu_id uuid,
    dsl_version_id uuid,
    execution_status public.execution_status_enum DEFAULT 'PENDING'::public.execution_status_enum NOT NULL,
    trigger_context jsonb,
    request_payload jsonb,
    response_payload jsonb,
    result_attributes jsonb,
    error_details jsonb,
    execution_duration_ms integer,
    started_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone,
    retry_count integer DEFAULT 0,
    next_retry_at timestamp with time zone,
    idempotency_key text,
    correlation_id text,
    trace_id text,
    span_id text,
    http_status integer,
    endpoint text,
    headers jsonb
);


ALTER TABLE public.action_executions OWNER TO adamtc007;

--
-- Name: actions_registry; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.actions_registry (
    action_id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    action_name character varying(255) NOT NULL,
    verb_pattern character varying(100) NOT NULL,
    action_type public.action_type_enum NOT NULL,
    resource_type_id uuid,
    domain character varying(100),
    trigger_conditions jsonb,
    execution_config jsonb NOT NULL,
    attribute_mapping jsonb NOT NULL,
    success_criteria jsonb,
    failure_handling jsonb,
    active boolean DEFAULT true,
    version integer DEFAULT 1,
    environment character varying(50) DEFAULT 'production'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.actions_registry OWNER TO adamtc007;

--
-- Name: attribute_sources; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.attribute_sources (
    id integer NOT NULL,
    source_key character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    trust_level character varying(20),
    requires_validation boolean DEFAULT false,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT attribute_sources_trust_level_check CHECK (((trust_level)::text = ANY (ARRAY[('high'::character varying)::text, ('medium'::character varying)::text, ('low'::character varying)::text])))
);


ALTER TABLE public.attribute_sources OWNER TO adamtc007;

--
-- Name: attribute_sources_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.attribute_sources_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.attribute_sources_id_seq OWNER TO adamtc007;

--
-- Name: attribute_sources_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.attribute_sources_id_seq OWNED BY public.attribute_sources.id;


--
-- Name: business_attributes; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.business_attributes (
    id integer NOT NULL,
    entity_name character varying(100) NOT NULL,
    attribute_name character varying(100) NOT NULL,
    full_path character varying(200) GENERATED ALWAYS AS ((((entity_name)::text || '.'::text) || (attribute_name)::text)) STORED,
    data_type character varying(50) NOT NULL,
    sql_type character varying(100),
    rust_type character varying(100),
    format_mask character varying(100),
    validation_pattern text,
    domain_id integer,
    source_id integer,
    required boolean DEFAULT false,
    editable boolean DEFAULT true,
    min_value numeric,
    max_value numeric,
    min_length integer,
    max_length integer,
    description text,
    metadata jsonb,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.business_attributes OWNER TO adamtc007;

--
-- Name: business_attributes_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.business_attributes_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.business_attributes_id_seq OWNER TO adamtc007;

--
-- Name: business_attributes_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.business_attributes_id_seq OWNED BY public.business_attributes.id;


--
-- Name: credentials_vault; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.credentials_vault (
    credential_id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    credential_name character varying(255) NOT NULL,
    credential_type character varying(50) NOT NULL,
    encrypted_data bytea NOT NULL,
    environment character varying(50) DEFAULT 'production'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone,
    active boolean DEFAULT true
);


ALTER TABLE public.credentials_vault OWNER TO adamtc007;

--
-- Name: data_domains; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.data_domains (
    id integer NOT NULL,
    domain_name character varying(100) NOT NULL,
    "values" jsonb NOT NULL,
    description text,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.data_domains OWNER TO adamtc007;

--
-- Name: data_domains_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.data_domains_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.data_domains_id_seq OWNER TO adamtc007;

--
-- Name: data_domains_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.data_domains_id_seq OWNED BY public.data_domains.id;


--
-- Name: derived_attributes; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.derived_attributes (
    id integer NOT NULL,
    entity_name character varying(100) NOT NULL,
    attribute_name character varying(100) NOT NULL,
    full_path character varying(200) GENERATED ALWAYS AS ((((entity_name)::text || '.'::text) || (attribute_name)::text)) STORED,
    data_type character varying(50) NOT NULL,
    sql_type character varying(100),
    rust_type character varying(100),
    domain_id integer,
    description text,
    metadata jsonb,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.derived_attributes OWNER TO adamtc007;

--
-- Name: derived_attributes_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.derived_attributes_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.derived_attributes_id_seq OWNER TO adamtc007;

--
-- Name: derived_attributes_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.derived_attributes_id_seq OWNED BY public.derived_attributes.id;


--
-- Name: resource_type_attributes; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.resource_type_attributes (
    resource_type_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    required boolean DEFAULT false,
    constraints jsonb,
    transformation character varying(100),
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.resource_type_attributes OWNER TO adamtc007;

--
-- Name: resource_type_endpoints; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.resource_type_endpoints (
    endpoint_id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    resource_type_id uuid,
    lifecycle_action character varying(50) NOT NULL,
    endpoint_url text NOT NULL,
    method character varying(10) DEFAULT 'POST'::character varying,
    authentication jsonb,
    timeout_seconds integer DEFAULT 300,
    retry_config jsonb,
    environment character varying(50) DEFAULT 'production'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.resource_type_endpoints OWNER TO adamtc007;

--
-- Name: resource_types; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.resource_types (
    resource_type_id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    resource_type_name character varying(200) NOT NULL,
    description text,
    active boolean DEFAULT true,
    version integer DEFAULT 1,
    environment character varying(50) DEFAULT 'production'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.resource_types OWNER TO adamtc007;

--
-- Name: rule_categories; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.rule_categories (
    id integer NOT NULL,
    category_key character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    color character varying(7),
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.rule_categories OWNER TO adamtc007;

--
-- Name: rule_categories_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.rule_categories_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.rule_categories_id_seq OWNER TO adamtc007;

--
-- Name: rule_categories_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.rule_categories_id_seq OWNED BY public.rule_categories.id;


--
-- Name: rule_dependencies; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.rule_dependencies (
    id integer NOT NULL,
    rule_id integer,
    attribute_id integer,
    dependency_type character varying(20) DEFAULT 'input'::character varying,
    CONSTRAINT rule_dependencies_dependency_type_check CHECK (((dependency_type)::text = ANY (ARRAY[('input'::character varying)::text, ('lookup'::character varying)::text, ('reference'::character varying)::text])))
);


ALTER TABLE public.rule_dependencies OWNER TO adamtc007;

--
-- Name: rule_dependencies_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.rule_dependencies_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.rule_dependencies_id_seq OWNER TO adamtc007;

--
-- Name: rule_dependencies_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.rule_dependencies_id_seq OWNED BY public.rule_dependencies.id;


--
-- Name: rule_executions; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.rule_executions (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    rule_id integer,
    execution_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    input_data jsonb,
    output_value jsonb,
    execution_duration_ms integer,
    success boolean,
    error_message text,
    context jsonb
);


ALTER TABLE public.rule_executions OWNER TO adamtc007;

--
-- Name: rule_versions; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.rule_versions (
    id integer NOT NULL,
    rule_id integer,
    version integer NOT NULL,
    rule_definition text NOT NULL,
    change_description text,
    created_by character varying(100),
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.rule_versions OWNER TO adamtc007;

--
-- Name: rule_versions_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.rule_versions_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.rule_versions_id_seq OWNER TO adamtc007;

--
-- Name: rule_versions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.rule_versions_id_seq OWNED BY public.rule_versions.id;


--
-- Name: rules; Type: TABLE; Schema: public; Owner: adamtc007
--

CREATE TABLE public.rules (
    id integer NOT NULL,
    rule_id character varying(50) NOT NULL,
    rule_name character varying(200) NOT NULL,
    description text,
    category_id integer,
    target_attribute_id integer,
    rule_definition text NOT NULL,
    parsed_ast jsonb,
    status character varying(20) DEFAULT 'draft'::character varying,
    version integer DEFAULT 1,
    tags text[],
    performance_metrics jsonb,
    embedding_data jsonb,
    created_by character varying(100),
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    updated_by character varying(100),
    updated_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    search_vector tsvector GENERATED ALWAYS AS (((setweight(to_tsvector('english'::regconfig, (COALESCE(rule_name, ''::character varying))::text), 'A'::"char") || setweight(to_tsvector('english'::regconfig, COALESCE(description, ''::text)), 'B'::"char")) || setweight(to_tsvector('english'::regconfig, COALESCE(rule_definition, ''::text)), 'C'::"char"))) STORED,
    embedding public.vector(1536),
    CONSTRAINT rules_status_check CHECK (((status)::text = ANY (ARRAY[('draft'::character varying)::text, ('active'::character varying)::text, ('inactive'::character varying)::text, ('deprecated'::character varying)::text])))
);


ALTER TABLE public.rules OWNER TO adamtc007;

--
-- Name: rules_id_seq; Type: SEQUENCE; Schema: public; Owner: adamtc007
--

CREATE SEQUENCE public.rules_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.rules_id_seq OWNER TO adamtc007;

--
-- Name: rules_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: adamtc007
--

ALTER SEQUENCE public.rules_id_seq OWNED BY public.rules.id;


--
-- Name: v_action_definitions; Type: VIEW; Schema: public; Owner: adamtc007
--

CREATE VIEW public.v_action_definitions AS
 SELECT a.action_id,
    a.action_name,
    a.verb_pattern,
    a.action_type,
    a.domain,
    rt.resource_type_name,
    rt.description AS resource_description,
    a.trigger_conditions,
    a.execution_config,
    a.attribute_mapping,
    a.success_criteria,
    a.failure_handling,
    a.active,
    a.environment,
    a.created_at,
    a.updated_at
   FROM (public.actions_registry a
     LEFT JOIN public.resource_types rt ON ((a.resource_type_id = rt.resource_type_id)));


ALTER VIEW public.v_action_definitions OWNER TO adamtc007;

--
-- Name: v_execution_summary; Type: VIEW; Schema: public; Owner: adamtc007
--

CREATE VIEW public.v_execution_summary AS
 SELECT e.execution_id,
    e.cbu_id,
    a.action_name,
    a.verb_pattern,
    rt.resource_type_name,
    e.execution_status,
    e.started_at,
    e.completed_at,
    e.execution_duration_ms,
    e.retry_count,
    e.http_status,
    e.idempotency_key,
    e.correlation_id
   FROM ((public.action_executions e
     JOIN public.actions_registry a ON ((e.action_id = a.action_id)))
     LEFT JOIN public.resource_types rt ON ((a.resource_type_id = rt.resource_type_id)));


ALTER VIEW public.v_execution_summary OWNER TO adamtc007;

--
-- Name: attribute_values_typed id; Type: DEFAULT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed ALTER COLUMN id SET DEFAULT nextval('"ob-poc".attribute_values_typed_id_seq'::regclass);


--
-- Name: dsl_instances id; Type: DEFAULT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instances ALTER COLUMN id SET DEFAULT nextval('"ob-poc".dsl_instances_id_seq'::regclass);


--
-- Name: attribute_sources id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.attribute_sources ALTER COLUMN id SET DEFAULT nextval('public.attribute_sources_id_seq'::regclass);


--
-- Name: business_attributes id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.business_attributes ALTER COLUMN id SET DEFAULT nextval('public.business_attributes_id_seq'::regclass);


--
-- Name: data_domains id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.data_domains ALTER COLUMN id SET DEFAULT nextval('public.data_domains_id_seq'::regclass);


--
-- Name: derived_attributes id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.derived_attributes ALTER COLUMN id SET DEFAULT nextval('public.derived_attributes_id_seq'::regclass);


--
-- Name: rule_categories id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_categories ALTER COLUMN id SET DEFAULT nextval('public.rule_categories_id_seq'::regclass);


--
-- Name: rule_dependencies id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_dependencies ALTER COLUMN id SET DEFAULT nextval('public.rule_dependencies_id_seq'::regclass);


--
-- Name: rule_versions id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_versions ALTER COLUMN id SET DEFAULT nextval('public.rule_versions_id_seq'::regclass);


--
-- Name: rules id; Type: DEFAULT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rules ALTER COLUMN id SET DEFAULT nextval('public.rules_id_seq'::regclass);


--
-- Name: attribute_dictionary attribute_dictionary_attr_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_dictionary
    ADD CONSTRAINT attribute_dictionary_attr_id_key UNIQUE (attr_id);


--
-- Name: attribute_dictionary attribute_dictionary_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_dictionary
    ADD CONSTRAINT attribute_dictionary_pkey PRIMARY KEY (attribute_id);


--
-- Name: attribute_registry attribute_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_registry
    ADD CONSTRAINT attribute_registry_pkey PRIMARY KEY (id);


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
-- Name: attribute_values_typed attribute_values_typed_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT attribute_values_typed_pkey PRIMARY KEY (id);


--
-- Name: cbu_creation_log cbu_creation_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_creation_log
    ADD CONSTRAINT cbu_creation_log_pkey PRIMARY KEY (log_id);


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
-- Name: cbu_resource_instances cbu_resource_instances_cbu_id_resource_type_id_instance_ide_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_cbu_id_resource_type_id_instance_ide_key UNIQUE (cbu_id, resource_type_id, instance_identifier);


--
-- Name: cbu_resource_instances cbu_resource_instances_instance_url_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_instance_url_key UNIQUE (instance_url);


--
-- Name: cbu_resource_instances cbu_resource_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_pkey PRIMARY KEY (instance_id);


--
-- Name: cbus cbus_name_jurisdiction_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_name_jurisdiction_key UNIQUE (name, jurisdiction);


--
-- Name: cbus cbus_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_pkey PRIMARY KEY (cbu_id);


--
-- Name: crud_operations crud_operations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".crud_operations
    ADD CONSTRAINT crud_operations_pkey PRIMARY KEY (operation_id);


--
-- Name: csg_rule_overrides csg_rule_overrides_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_rule_overrides
    ADD CONSTRAINT csg_rule_overrides_pkey PRIMARY KEY (override_id);


--
-- Name: csg_rule_overrides csg_rule_overrides_rule_id_cbu_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_rule_overrides
    ADD CONSTRAINT csg_rule_overrides_rule_id_cbu_id_key UNIQUE (rule_id, cbu_id);


--
-- Name: csg_semantic_similarity_cache csg_semantic_similarity_cache_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_semantic_similarity_cache
    ADD CONSTRAINT csg_semantic_similarity_cache_pkey PRIMARY KEY (cache_id);


--
-- Name: csg_semantic_similarity_cache csg_semantic_similarity_cache_source_type_source_code_targe_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_semantic_similarity_cache
    ADD CONSTRAINT csg_semantic_similarity_cache_source_type_source_code_targe_key UNIQUE (source_type, source_code, target_type, target_code);


--
-- Name: csg_validation_rules csg_validation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_validation_rules
    ADD CONSTRAINT csg_validation_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: csg_validation_rules csg_validation_rules_rule_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_validation_rules
    ADD CONSTRAINT csg_validation_rules_rule_code_key UNIQUE (rule_code);


--
-- Name: currencies currencies_iso_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".currencies
    ADD CONSTRAINT currencies_iso_code_key UNIQUE (iso_code);


--
-- Name: currencies currencies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".currencies
    ADD CONSTRAINT currencies_pkey PRIMARY KEY (currency_id);


--
-- Name: decision_conditions decision_conditions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".decision_conditions
    ADD CONSTRAINT decision_conditions_pkey PRIMARY KEY (condition_id);


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
-- Name: document_attribute_mappings document_attribute_mappings_document_type_id_attribute_uuid_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_document_type_id_attribute_uuid_key UNIQUE (document_type_id, attribute_uuid);


--
-- Name: document_attribute_mappings document_attribute_mappings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_pkey PRIMARY KEY (mapping_id);


--
-- Name: document_catalog document_catalog_file_hash_sha256_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_file_hash_sha256_key UNIQUE (file_hash_sha256);


--
-- Name: document_catalog document_catalog_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_pkey PRIMARY KEY (doc_id);


--
-- Name: document_entity_links document_entity_links_doc_id_entity_id_link_type_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_entity_links
    ADD CONSTRAINT document_entity_links_doc_id_entity_id_link_type_key UNIQUE (doc_id, entity_id, link_type);


--
-- Name: document_entity_links document_entity_links_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_entity_links
    ADD CONSTRAINT document_entity_links_pkey PRIMARY KEY (link_id);


--
-- Name: document_metadata document_metadata_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_metadata
    ADD CONSTRAINT document_metadata_pkey PRIMARY KEY (doc_id, attribute_id);


--
-- Name: document_relationships document_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: document_relationships document_relationships_primary_doc_id_related_doc_id_relati_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_primary_doc_id_related_doc_id_relati_key UNIQUE (primary_doc_id, related_doc_id, relationship_type);


--
-- Name: document_requests document_requests_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_requests
    ADD CONSTRAINT document_requests_pkey PRIMARY KEY (request_id);


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
-- Name: document_verifications document_verifications_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_verifications
    ADD CONSTRAINT document_verifications_pkey PRIMARY KEY (verification_id);


--
-- Name: domain_vocabularies domain_vocabularies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".domain_vocabularies
    ADD CONSTRAINT domain_vocabularies_pkey PRIMARY KEY (vocab_id);


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
-- Name: dsl_examples dsl_examples_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_examples
    ADD CONSTRAINT dsl_examples_pkey PRIMARY KEY (example_id);


--
-- Name: dsl_execution_log dsl_execution_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_execution_log
    ADD CONSTRAINT dsl_execution_log_pkey PRIMARY KEY (execution_id);


--
-- Name: dsl_generation_log dsl_generation_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_generation_log
    ADD CONSTRAINT dsl_generation_log_pkey PRIMARY KEY (log_id);


--
-- Name: dsl_instance_versions dsl_instance_versions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT dsl_instance_versions_pkey PRIMARY KEY (version_id);


--
-- Name: dsl_instances dsl_instances_instance_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instances
    ADD CONSTRAINT dsl_instances_instance_id_key UNIQUE (instance_id);


--
-- Name: dsl_instances dsl_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instances
    ADD CONSTRAINT dsl_instances_pkey PRIMARY KEY (id);


--
-- Name: dsl_ob dsl_ob_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_ob
    ADD CONSTRAINT dsl_ob_pkey PRIMARY KEY (version_id);


--
-- Name: dsl_versions dsl_versions_domain_id_version_number_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_domain_id_version_number_key UNIQUE (domain_id, version_number);


--
-- Name: dsl_versions dsl_versions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_pkey PRIMARY KEY (version_id);


--
-- Name: entities entities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_crud_rules entity_crud_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_crud_rules
    ADD CONSTRAINT entity_crud_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: entity_lifecycle_status entity_lifecycle_status_entity_type_entity_id_status_code_e_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_lifecycle_status
    ADD CONSTRAINT entity_lifecycle_status_entity_type_entity_id_status_code_e_key UNIQUE (entity_type, entity_id, status_code, effective_date);


--
-- Name: entity_lifecycle_status entity_lifecycle_status_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_lifecycle_status
    ADD CONSTRAINT entity_lifecycle_status_pkey PRIMARY KEY (status_id);


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
-- Name: entity_role_connections entity_role_connections_natural_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT entity_role_connections_natural_key UNIQUE (cbu_id, entity_id, role_id);


--
-- Name: entity_role_connections entity_role_connections_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT entity_role_connections_pkey PRIMARY KEY (connection_id);


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
-- Name: entity_validation_rules entity_validation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_validation_rules
    ADD CONSTRAINT entity_validation_rules_pkey PRIMARY KEY (rule_id);


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
-- Name: investigation_assignments investigation_assignments_investigation_id_assignee_role_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".investigation_assignments
    ADD CONSTRAINT investigation_assignments_investigation_id_assignee_role_key UNIQUE (investigation_id, assignee, role);


--
-- Name: investigation_assignments investigation_assignments_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".investigation_assignments
    ADD CONSTRAINT investigation_assignments_pkey PRIMARY KEY (assignment_id);


--
-- Name: kyc_decisions kyc_decisions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".kyc_decisions
    ADD CONSTRAINT kyc_decisions_pkey PRIMARY KEY (decision_id);


--
-- Name: kyc_investigations kyc_investigations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".kyc_investigations
    ADD CONSTRAINT kyc_investigations_pkey PRIMARY KEY (investigation_id);


--
-- Name: master_entity_xref master_entity_xref_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".master_entity_xref
    ADD CONSTRAINT master_entity_xref_pkey PRIMARY KEY (xref_id);


--
-- Name: master_jurisdictions master_jurisdictions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".master_jurisdictions
    ADD CONSTRAINT master_jurisdictions_pkey PRIMARY KEY (jurisdiction_code);


--
-- Name: monitoring_activities monitoring_activities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_activities
    ADD CONSTRAINT monitoring_activities_pkey PRIMARY KEY (activity_id);


--
-- Name: monitoring_alert_rules monitoring_alert_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_alert_rules
    ADD CONSTRAINT monitoring_alert_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: monitoring_cases monitoring_cases_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_cases
    ADD CONSTRAINT monitoring_cases_pkey PRIMARY KEY (case_id);


--
-- Name: monitoring_events monitoring_events_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_events
    ADD CONSTRAINT monitoring_events_pkey PRIMARY KEY (event_id);


--
-- Name: monitoring_reviews monitoring_reviews_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_reviews
    ADD CONSTRAINT monitoring_reviews_pkey PRIMARY KEY (review_id);


--
-- Name: monitoring_setup monitoring_setup_cbu_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_setup
    ADD CONSTRAINT monitoring_setup_cbu_id_key UNIQUE (cbu_id);


--
-- Name: monitoring_setup monitoring_setup_cbu_unique; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_setup
    ADD CONSTRAINT monitoring_setup_cbu_unique UNIQUE (cbu_id);


--
-- Name: monitoring_setup monitoring_setup_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_setup
    ADD CONSTRAINT monitoring_setup_pkey PRIMARY KEY (setup_id);


--
-- Name: onboarding_products onboarding_products_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_pkey PRIMARY KEY (onboarding_product_id);


--
-- Name: onboarding_products onboarding_products_request_id_product_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_request_id_product_id_key UNIQUE (request_id, product_id);


--
-- Name: onboarding_requests onboarding_requests_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_requests
    ADD CONSTRAINT onboarding_requests_pkey PRIMARY KEY (request_id);


--
-- Name: onboarding_resource_allocations onboarding_resource_allocations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_resource_allocations
    ADD CONSTRAINT onboarding_resource_allocations_pkey PRIMARY KEY (allocation_id);


--
-- Name: onboarding_service_configs onboarding_service_configs_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_service_configs
    ADD CONSTRAINT onboarding_service_configs_pkey PRIMARY KEY (config_id);


--
-- Name: onboarding_service_configs onboarding_service_configs_request_id_service_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_service_configs
    ADD CONSTRAINT onboarding_service_configs_request_id_service_id_key UNIQUE (request_id, service_id);


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
-- Name: ownership_relationships ownership_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_pkey PRIMARY KEY (ownership_id);


--
-- Name: parsed_asts parsed_asts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".parsed_asts
    ADD CONSTRAINT parsed_asts_pkey PRIMARY KEY (ast_id);


--
-- Name: parsed_asts parsed_asts_version_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".parsed_asts
    ADD CONSTRAINT parsed_asts_version_id_key UNIQUE (version_id);


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
-- Name: service_resource_types prod_resources_resource_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_types
    ADD CONSTRAINT prod_resources_resource_code_key UNIQUE (resource_code);


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
-- Name: products products_product_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".products
    ADD CONSTRAINT products_product_code_key UNIQUE (product_code);


--
-- Name: rag_embeddings rag_embeddings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".rag_embeddings
    ADD CONSTRAINT rag_embeddings_pkey PRIMARY KEY (embedding_id);


--
-- Name: resource_attribute_requirements resource_attribute_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_pkey PRIMARY KEY (requirement_id);


--
-- Name: resource_attribute_requirements resource_attribute_requirements_resource_id_attribute_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_resource_id_attribute_id_key UNIQUE (resource_id, attribute_id);


--
-- Name: resource_instance_attributes resource_instance_attributes_instance_id_attribute_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_instance_id_attribute_id_key UNIQUE (instance_id, attribute_id);


--
-- Name: resource_instance_attributes resource_instance_attributes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_pkey PRIMARY KEY (value_id);


--
-- Name: risk_assessments risk_assessments_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_assessments
    ADD CONSTRAINT risk_assessments_pkey PRIMARY KEY (assessment_id);


--
-- Name: risk_flags risk_flags_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_flags
    ADD CONSTRAINT risk_flags_pkey PRIMARY KEY (flag_id);


--
-- Name: risk_rating_changes risk_rating_changes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_rating_changes
    ADD CONSTRAINT risk_rating_changes_pkey PRIMARY KEY (change_id);


--
-- Name: risk_ratings risk_ratings_cbu_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_ratings
    ADD CONSTRAINT risk_ratings_cbu_id_key UNIQUE (cbu_id);


--
-- Name: risk_ratings risk_ratings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_ratings
    ADD CONSTRAINT risk_ratings_pkey PRIMARY KEY (rating_id);


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
-- Name: scheduled_reviews scheduled_reviews_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".scheduled_reviews
    ADD CONSTRAINT scheduled_reviews_pkey PRIMARY KEY (review_id);


--
-- Name: schema_changes schema_changes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".schema_changes
    ADD CONSTRAINT schema_changes_pkey PRIMARY KEY (change_id);


--
-- Name: screening_batch_results screening_batch_results_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_batch_results
    ADD CONSTRAINT screening_batch_results_pkey PRIMARY KEY (batch_id, screening_id);


--
-- Name: screening_batches screening_batches_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_batches
    ADD CONSTRAINT screening_batches_pkey PRIMARY KEY (batch_id);


--
-- Name: screening_hit_resolutions screening_hit_resolutions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_hit_resolutions
    ADD CONSTRAINT screening_hit_resolutions_pkey PRIMARY KEY (resolution_id);


--
-- Name: screening_lists screening_lists_list_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_lists
    ADD CONSTRAINT screening_lists_list_code_key UNIQUE (list_code);


--
-- Name: screening_lists screening_lists_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_lists
    ADD CONSTRAINT screening_lists_pkey PRIMARY KEY (screening_list_id);


--
-- Name: screenings screenings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screenings
    ADD CONSTRAINT screenings_pkey PRIMARY KEY (screening_id);


--
-- Name: service_delivery_map service_delivery_map_cbu_id_product_id_service_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_cbu_id_product_id_service_id_key UNIQUE (cbu_id, product_id, service_id);


--
-- Name: service_delivery_map service_delivery_map_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_pkey PRIMARY KEY (delivery_id);


--
-- Name: service_discovery_cache service_discovery_cache_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_discovery_cache
    ADD CONSTRAINT service_discovery_cache_pkey PRIMARY KEY (discovery_id);


--
-- Name: service_option_choices service_option_choices_option_def_id_choice_value_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_option_choices
    ADD CONSTRAINT service_option_choices_option_def_id_choice_value_key UNIQUE (option_def_id, choice_value);


--
-- Name: service_option_choices service_option_choices_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_option_choices
    ADD CONSTRAINT service_option_choices_pkey PRIMARY KEY (choice_id);


--
-- Name: service_option_definitions service_option_definitions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_option_definitions
    ADD CONSTRAINT service_option_definitions_pkey PRIMARY KEY (option_def_id);


--
-- Name: service_option_definitions service_option_definitions_service_id_option_key_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_option_definitions
    ADD CONSTRAINT service_option_definitions_service_id_option_key_key UNIQUE (service_id, option_key);


--
-- Name: service_resource_capabilities service_resource_capabilities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_pkey PRIMARY KEY (capability_id);


--
-- Name: service_resource_capabilities service_resource_capabilities_service_id_resource_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_service_id_resource_id_key UNIQUE (service_id, resource_id);


--
-- Name: service_resource_types service_resource_types_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_types
    ADD CONSTRAINT service_resource_types_name_key UNIQUE (name);


--
-- Name: service_resource_types service_resource_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_types
    ADD CONSTRAINT service_resource_types_pkey PRIMARY KEY (resource_id);


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
-- Name: services services_service_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".services
    ADD CONSTRAINT services_service_code_key UNIQUE (service_code);


--
-- Name: taxonomy_audit_log taxonomy_audit_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".taxonomy_audit_log
    ADD CONSTRAINT taxonomy_audit_log_pkey PRIMARY KEY (audit_id);


--
-- Name: taxonomy_crud_log taxonomy_crud_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".taxonomy_crud_log
    ADD CONSTRAINT taxonomy_crud_log_pkey PRIMARY KEY (operation_id);


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
-- Name: attribute_registry uk_attribute_uuid; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_registry
    ADD CONSTRAINT uk_attribute_uuid UNIQUE (uuid);


--
-- Name: verb_registry verb_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".verb_registry
    ADD CONSTRAINT verb_registry_pkey PRIMARY KEY (verb);


--
-- Name: vocabulary_audit vocabulary_audit_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".vocabulary_audit
    ADD CONSTRAINT vocabulary_audit_pkey PRIMARY KEY (audit_id);


--
-- Name: action_execution_attempts action_execution_attempts_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.action_execution_attempts
    ADD CONSTRAINT action_execution_attempts_pkey PRIMARY KEY (attempt_id);


--
-- Name: action_executions action_executions_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.action_executions
    ADD CONSTRAINT action_executions_pkey PRIMARY KEY (execution_id);


--
-- Name: actions_registry actions_registry_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.actions_registry
    ADD CONSTRAINT actions_registry_pkey PRIMARY KEY (action_id);


--
-- Name: attribute_sources attribute_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.attribute_sources
    ADD CONSTRAINT attribute_sources_pkey PRIMARY KEY (id);


--
-- Name: attribute_sources attribute_sources_source_key_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.attribute_sources
    ADD CONSTRAINT attribute_sources_source_key_key UNIQUE (source_key);


--
-- Name: business_attributes business_attributes_entity_name_attribute_name_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_entity_name_attribute_name_key UNIQUE (entity_name, attribute_name);


--
-- Name: business_attributes business_attributes_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_pkey PRIMARY KEY (id);


--
-- Name: credentials_vault credentials_vault_credential_name_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.credentials_vault
    ADD CONSTRAINT credentials_vault_credential_name_key UNIQUE (credential_name);


--
-- Name: credentials_vault credentials_vault_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.credentials_vault
    ADD CONSTRAINT credentials_vault_pkey PRIMARY KEY (credential_id);


--
-- Name: data_domains data_domains_domain_name_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.data_domains
    ADD CONSTRAINT data_domains_domain_name_key UNIQUE (domain_name);


--
-- Name: data_domains data_domains_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.data_domains
    ADD CONSTRAINT data_domains_pkey PRIMARY KEY (id);


--
-- Name: derived_attributes derived_attributes_entity_name_attribute_name_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.derived_attributes
    ADD CONSTRAINT derived_attributes_entity_name_attribute_name_key UNIQUE (entity_name, attribute_name);


--
-- Name: derived_attributes derived_attributes_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.derived_attributes
    ADD CONSTRAINT derived_attributes_pkey PRIMARY KEY (id);


--
-- Name: resource_type_attributes resource_type_attributes_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.resource_type_attributes
    ADD CONSTRAINT resource_type_attributes_pkey PRIMARY KEY (resource_type_id, attribute_id);


--
-- Name: resource_type_endpoints resource_type_endpoints_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.resource_type_endpoints
    ADD CONSTRAINT resource_type_endpoints_pkey PRIMARY KEY (endpoint_id);


--
-- Name: resource_type_endpoints resource_type_endpoints_resource_type_id_lifecycle_action_e_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.resource_type_endpoints
    ADD CONSTRAINT resource_type_endpoints_resource_type_id_lifecycle_action_e_key UNIQUE (resource_type_id, lifecycle_action, environment);


--
-- Name: resource_types resource_types_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.resource_types
    ADD CONSTRAINT resource_types_pkey PRIMARY KEY (resource_type_id);


--
-- Name: rule_categories rule_categories_category_key_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_categories
    ADD CONSTRAINT rule_categories_category_key_key UNIQUE (category_key);


--
-- Name: rule_categories rule_categories_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_categories
    ADD CONSTRAINT rule_categories_pkey PRIMARY KEY (id);


--
-- Name: rule_dependencies rule_dependencies_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_pkey PRIMARY KEY (id);


--
-- Name: rule_dependencies rule_dependencies_rule_id_attribute_id_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_rule_id_attribute_id_key UNIQUE (rule_id, attribute_id);


--
-- Name: rule_executions rule_executions_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_executions
    ADD CONSTRAINT rule_executions_pkey PRIMARY KEY (id);


--
-- Name: rule_versions rule_versions_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_versions
    ADD CONSTRAINT rule_versions_pkey PRIMARY KEY (id);


--
-- Name: rule_versions rule_versions_rule_id_version_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_versions
    ADD CONSTRAINT rule_versions_rule_id_version_key UNIQUE (rule_id, version);


--
-- Name: rules rules_pkey; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_pkey PRIMARY KEY (id);


--
-- Name: rules rules_rule_id_key; Type: CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_rule_id_key UNIQUE (rule_id);


--
-- Name: entity_limited_companies_reg_jurisdiction_uniq; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE UNIQUE INDEX entity_limited_companies_reg_jurisdiction_uniq ON "ob-poc".entity_limited_companies USING btree (registration_number, jurisdiction) WHERE ((registration_number IS NOT NULL) AND (jurisdiction IS NOT NULL));


--
-- Name: entity_proper_persons_id_doc_uniq; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE UNIQUE INDEX entity_proper_persons_id_doc_uniq ON "ob-poc".entity_proper_persons USING btree (id_document_type, id_document_number) WHERE ((id_document_type IS NOT NULL) AND (id_document_number IS NOT NULL));


--
-- Name: idx_attr_uuid; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attr_uuid ON "ob-poc".attribute_registry USING btree (uuid);


--
-- Name: idx_attr_vals_lookup; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attr_vals_lookup ON "ob-poc".attribute_values USING btree (cbu_id, attribute_id, dsl_version);


--
-- Name: idx_attribute_dictionary_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_dictionary_active ON "ob-poc".attribute_dictionary USING btree (is_active);


--
-- Name: idx_attribute_dictionary_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_dictionary_domain ON "ob-poc".attribute_dictionary USING btree (domain);


--
-- Name: idx_attribute_registry_applicability; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_registry_applicability ON "ob-poc".attribute_registry USING gin (applicability);


--
-- Name: idx_attribute_registry_category; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_registry_category ON "ob-poc".attribute_registry USING btree (category);


--
-- Name: idx_attribute_registry_embedding; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_registry_embedding ON "ob-poc".attribute_registry USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='100');


--
-- Name: idx_attribute_registry_value_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_registry_value_type ON "ob-poc".attribute_registry USING btree (value_type);


--
-- Name: idx_attribute_values_typed_attribute; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_values_typed_attribute ON "ob-poc".attribute_values_typed USING btree (attribute_id);


--
-- Name: idx_attribute_values_typed_effective; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_values_typed_effective ON "ob-poc".attribute_values_typed USING btree (effective_from, effective_to) WHERE (effective_to IS NULL);


--
-- Name: idx_attribute_values_typed_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_values_typed_entity ON "ob-poc".attribute_values_typed USING btree (entity_id);


--
-- Name: idx_attribute_values_typed_entity_attribute; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_attribute_values_typed_entity_attribute ON "ob-poc".attribute_values_typed USING btree (entity_id, attribute_id);


--
-- Name: idx_beneficiary_classes_trust; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_beneficiary_classes_trust ON "ob-poc".trust_beneficiary_classes USING btree (trust_id);


--
-- Name: idx_cbu_creation_log_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbu_creation_log_cbu ON "ob-poc".cbu_creation_log USING btree (cbu_id);


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
-- Name: idx_cbu_name_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbu_name_trgm ON "ob-poc".cbus USING gin (name public.gin_trgm_ops);


--
-- Name: idx_cbus_embedding; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbus_embedding ON "ob-poc".cbus USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='100');


--
-- Name: idx_cbus_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbus_name ON "ob-poc".cbus USING btree (name);


--
-- Name: idx_cbus_onboarding_context; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbus_onboarding_context ON "ob-poc".cbus USING gin (onboarding_context);


--
-- Name: idx_cbus_risk_context; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbus_risk_context ON "ob-poc".cbus USING gin (risk_context);


--
-- Name: idx_cbus_semantic_context; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cbus_semantic_context ON "ob-poc".cbus USING gin (semantic_context);


--
-- Name: idx_companies_name_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_companies_name_trgm ON "ob-poc".entity_limited_companies USING gin (company_name public.gin_trgm_ops);


--
-- Name: idx_companies_reg_number; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_companies_reg_number ON "ob-poc".entity_limited_companies USING btree (registration_number);


--
-- Name: idx_conditions_decision; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_conditions_decision ON "ob-poc".decision_conditions USING btree (decision_id);


--
-- Name: idx_conditions_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_conditions_status ON "ob-poc".decision_conditions USING btree (status);


--
-- Name: idx_cri_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cri_cbu ON "ob-poc".cbu_resource_instances USING btree (cbu_id);


--
-- Name: idx_cri_resource_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cri_resource_type ON "ob-poc".cbu_resource_instances USING btree (resource_type_id);


--
-- Name: idx_cri_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cri_status ON "ob-poc".cbu_resource_instances USING btree (status);


--
-- Name: idx_cri_url; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_cri_url ON "ob-poc".cbu_resource_instances USING btree (instance_url);


--
-- Name: idx_crud_operations_asset; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_crud_operations_asset ON "ob-poc".crud_operations USING btree (asset_type);


--
-- Name: idx_crud_operations_created; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_crud_operations_created ON "ob-poc".crud_operations USING btree (created_at DESC);


--
-- Name: idx_crud_operations_parent; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_crud_operations_parent ON "ob-poc".crud_operations USING btree (parent_operation_id) WHERE (parent_operation_id IS NOT NULL);


--
-- Name: idx_crud_operations_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_crud_operations_status ON "ob-poc".crud_operations USING btree (execution_status);


--
-- Name: idx_crud_operations_transaction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_crud_operations_transaction ON "ob-poc".crud_operations USING btree (transaction_id) WHERE (transaction_id IS NOT NULL);


--
-- Name: idx_crud_operations_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_crud_operations_type ON "ob-poc".crud_operations USING btree (operation_type);


--
-- Name: idx_csg_overrides_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_csg_overrides_cbu ON "ob-poc".csg_rule_overrides USING btree (cbu_id);


--
-- Name: idx_csg_overrides_rule; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_csg_overrides_rule ON "ob-poc".csg_rule_overrides USING btree (rule_id);


--
-- Name: idx_csg_rules_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_csg_rules_active ON "ob-poc".csg_validation_rules USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_csg_rules_params; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_csg_rules_params ON "ob-poc".csg_validation_rules USING gin (rule_params);


--
-- Name: idx_csg_rules_target; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_csg_rules_target ON "ob-poc".csg_validation_rules USING btree (target_type, target_code);


--
-- Name: idx_csg_rules_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_csg_rules_type ON "ob-poc".csg_validation_rules USING btree (rule_type);


--
-- Name: idx_currencies_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_currencies_active ON "ob-poc".currencies USING btree (is_active);


--
-- Name: idx_dam_attribute_uuid; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dam_attribute_uuid ON "ob-poc".document_attribute_mappings USING btree (attribute_uuid);


--
-- Name: idx_dam_document_type_attribute; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dam_document_type_attribute ON "ob-poc".document_attribute_mappings USING btree (document_type_id, attribute_uuid);


--
-- Name: idx_dam_document_type_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dam_document_type_id ON "ob-poc".document_attribute_mappings USING btree (document_type_id);


--
-- Name: idx_decisions_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_decisions_cbu ON "ob-poc".kyc_decisions USING btree (cbu_id);


--
-- Name: idx_decisions_decision; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_decisions_decision ON "ob-poc".kyc_decisions USING btree (decision);


--
-- Name: idx_decisions_investigation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_decisions_investigation ON "ob-poc".kyc_decisions USING btree (investigation_id);


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
-- Name: idx_doc_attr_mappings_attr; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_attr_mappings_attr ON "ob-poc".document_attribute_mappings USING btree (attribute_uuid);


--
-- Name: idx_doc_attr_mappings_doc_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_attr_mappings_doc_type ON "ob-poc".document_attribute_mappings USING btree (document_type_id);


--
-- Name: idx_doc_catalog_hash; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_catalog_hash ON "ob-poc".document_catalog USING btree (file_hash_sha256);


--
-- Name: idx_doc_catalog_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_catalog_status ON "ob-poc".document_catalog USING btree (extraction_status);


--
-- Name: idx_doc_meta_attr_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_meta_attr_id ON "ob-poc".document_metadata USING btree (attribute_id);


--
-- Name: idx_doc_meta_doc_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_meta_doc_id ON "ob-poc".document_metadata USING btree (doc_id);


--
-- Name: idx_doc_meta_value_gin; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_meta_value_gin ON "ob-poc".document_metadata USING gin (value jsonb_path_ops);


--
-- Name: idx_doc_rel_primary; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_rel_primary ON "ob-poc".document_relationships USING btree (primary_doc_id);


--
-- Name: idx_doc_rel_related; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_rel_related ON "ob-poc".document_relationships USING btree (related_doc_id);


--
-- Name: idx_doc_requests_investigation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_requests_investigation ON "ob-poc".document_requests USING btree (investigation_id);


--
-- Name: idx_doc_requests_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_requests_status ON "ob-poc".document_requests USING btree (status);


--
-- Name: idx_doc_verifications_doc; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_doc_verifications_doc ON "ob-poc".document_verifications USING btree (doc_id);


--
-- Name: idx_document_catalog_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_cbu ON "ob-poc".document_catalog USING btree (cbu_id);


--
-- Name: idx_document_catalog_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_type ON "ob-poc".document_catalog USING btree (document_type_id);


--
-- Name: idx_document_catalog_type_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_catalog_type_status ON "ob-poc".document_catalog USING btree (document_type_id, extraction_status);


--
-- Name: idx_document_entity_links_doc; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_entity_links_doc ON "ob-poc".document_entity_links USING btree (doc_id);


--
-- Name: idx_document_entity_links_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_entity_links_entity ON "ob-poc".document_entity_links USING btree (entity_id);


--
-- Name: idx_document_metadata_doc_attr; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_metadata_doc_attr ON "ob-poc".document_metadata USING btree (doc_id, attribute_id);


--
-- Name: idx_document_types_applicability; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_applicability ON "ob-poc".document_types USING gin (applicability);


--
-- Name: idx_document_types_embedding; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_embedding ON "ob-poc".document_types USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='100');


--
-- Name: idx_document_types_semantic_context; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_document_types_semantic_context ON "ob-poc".document_types USING gin (semantic_context);


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
-- Name: idx_dsl_examples_asset; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_asset ON "ob-poc".dsl_examples USING btree (asset_type);


--
-- Name: idx_dsl_examples_complexity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_complexity ON "ob-poc".dsl_examples USING btree (complexity_level);


--
-- Name: idx_dsl_examples_operation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_operation ON "ob-poc".dsl_examples USING btree (operation_type);


--
-- Name: idx_dsl_examples_success; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_success ON "ob-poc".dsl_examples USING btree (success_rate DESC);


--
-- Name: idx_dsl_examples_table; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_table ON "ob-poc".dsl_examples USING btree (entity_table_name) WHERE (entity_table_name IS NOT NULL);


--
-- Name: idx_dsl_examples_tags; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_tags ON "ob-poc".dsl_examples USING gin (tags);


--
-- Name: idx_dsl_examples_usage; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_examples_usage ON "ob-poc".dsl_examples USING btree (usage_count DESC);


--
-- Name: idx_dsl_execution_cbu_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_execution_cbu_id ON "ob-poc".dsl_execution_log USING btree (cbu_id);


--
-- Name: idx_dsl_execution_started_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_execution_started_at ON "ob-poc".dsl_execution_log USING btree (started_at DESC);


--
-- Name: idx_dsl_execution_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_execution_status ON "ob-poc".dsl_execution_log USING btree (status);


--
-- Name: idx_dsl_execution_version_phase; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_execution_version_phase ON "ob-poc".dsl_execution_log USING btree (version_id, execution_phase);


--
-- Name: idx_dsl_instance_versions_instance_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_instance_versions_instance_id ON "ob-poc".dsl_instance_versions USING btree (instance_id);


--
-- Name: idx_dsl_instance_versions_version_number; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_instance_versions_version_number ON "ob-poc".dsl_instance_versions USING btree (instance_id, version_number);


--
-- Name: idx_dsl_instances_business_reference; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_instances_business_reference ON "ob-poc".dsl_instances USING btree (business_reference);


--
-- Name: idx_dsl_instances_case_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_instances_case_id ON "ob-poc".dsl_instances USING btree (case_id);


--
-- Name: idx_dsl_ob_cbu_id_created_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_ob_cbu_id_created_at ON "ob-poc".dsl_ob USING btree (cbu_id, created_at DESC);


--
-- Name: idx_dsl_versions_created_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_versions_created_at ON "ob-poc".dsl_versions USING btree (created_at DESC);


--
-- Name: idx_dsl_versions_domain_version; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_versions_domain_version ON "ob-poc".dsl_versions USING btree (domain_id, version_number DESC);


--
-- Name: idx_dsl_versions_functional_state; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_versions_functional_state ON "ob-poc".dsl_versions USING btree (functional_state);


--
-- Name: idx_dsl_versions_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_dsl_versions_status ON "ob-poc".dsl_versions USING btree (compilation_status);


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
-- Name: idx_entity_crud_rules_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_crud_rules_active ON "ob-poc".entity_crud_rules USING btree (is_active);


--
-- Name: idx_entity_crud_rules_field; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_crud_rules_field ON "ob-poc".entity_crud_rules USING btree (field_name) WHERE (field_name IS NOT NULL);


--
-- Name: idx_entity_crud_rules_operation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_crud_rules_operation ON "ob-poc".entity_crud_rules USING btree (operation_type);


--
-- Name: idx_entity_crud_rules_table; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_crud_rules_table ON "ob-poc".entity_crud_rules USING btree (entity_table_name);


--
-- Name: idx_entity_lifecycle_effective; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_lifecycle_effective ON "ob-poc".entity_lifecycle_status USING btree (effective_date);


--
-- Name: idx_entity_lifecycle_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_lifecycle_status ON "ob-poc".entity_lifecycle_status USING btree (status_code);


--
-- Name: idx_entity_lifecycle_type_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_lifecycle_type_id ON "ob-poc".entity_lifecycle_status USING btree (entity_type, entity_id);


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
-- Name: idx_entity_role_connections_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_role_connections_cbu ON "ob-poc".entity_role_connections USING btree (cbu_id);


--
-- Name: idx_entity_role_connections_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_role_connections_entity ON "ob-poc".entity_role_connections USING btree (entity_id);


--
-- Name: idx_entity_role_connections_role; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_role_connections_role ON "ob-poc".entity_role_connections USING btree (role_id);


--
-- Name: idx_entity_types_embedding; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_embedding ON "ob-poc".entity_types USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='50');


--
-- Name: idx_entity_types_hierarchy; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_hierarchy ON "ob-poc".entity_types USING gin (type_hierarchy_path);


--
-- Name: idx_entity_types_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_name ON "ob-poc".entity_types USING btree (name);


--
-- Name: idx_entity_types_parent; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_parent ON "ob-poc".entity_types USING btree (parent_type_id);


--
-- Name: idx_entity_types_semantic_context; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_semantic_context ON "ob-poc".entity_types USING gin (semantic_context);


--
-- Name: idx_entity_types_table; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_types_table ON "ob-poc".entity_types USING btree (table_name);


--
-- Name: idx_entity_types_type_code; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE UNIQUE INDEX idx_entity_types_type_code ON "ob-poc".entity_types USING btree (type_code);


--
-- Name: idx_entity_validation_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_validation_active ON "ob-poc".entity_validation_rules USING btree (is_active);


--
-- Name: idx_entity_validation_field; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_validation_field ON "ob-poc".entity_validation_rules USING btree (field_name);


--
-- Name: idx_entity_validation_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_entity_validation_type ON "ob-poc".entity_validation_rules USING btree (entity_type);


--
-- Name: idx_gen_log_created; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_created ON "ob-poc".dsl_generation_log USING btree (created_at DESC);


--
-- Name: idx_gen_log_domain; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_domain ON "ob-poc".dsl_generation_log USING btree (domain_name);


--
-- Name: idx_gen_log_instance; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_instance ON "ob-poc".dsl_generation_log USING btree (instance_id) WHERE (instance_id IS NOT NULL);


--
-- Name: idx_gen_log_intent_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_intent_trgm ON "ob-poc".dsl_generation_log USING gin (user_intent public.gin_trgm_ops);


--
-- Name: idx_gen_log_iterations; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_iterations ON "ob-poc".dsl_generation_log USING gin (iterations);


--
-- Name: idx_gen_log_session; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_session ON "ob-poc".dsl_generation_log USING btree (session_id) WHERE (session_id IS NOT NULL);


--
-- Name: idx_gen_log_success; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_gen_log_success ON "ob-poc".dsl_generation_log USING btree (success) WHERE (success = true);


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
-- Name: idx_investigations_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_investigations_cbu ON "ob-poc".kyc_investigations USING btree (cbu_id);


--
-- Name: idx_investigations_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_investigations_status ON "ob-poc".kyc_investigations USING btree (status);


--
-- Name: idx_limited_companies_entity_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_limited_companies_entity_id ON "ob-poc".entity_limited_companies USING btree (entity_id);


--
-- Name: idx_limited_companies_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_limited_companies_jurisdiction ON "ob-poc".entity_limited_companies USING btree (jurisdiction);


--
-- Name: idx_limited_companies_reg_num; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_limited_companies_reg_num ON "ob-poc".entity_limited_companies USING btree (registration_number);


--
-- Name: idx_master_entity_xref_entity_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_master_entity_xref_entity_id ON "ob-poc".master_entity_xref USING btree (entity_id);


--
-- Name: idx_master_entity_xref_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_master_entity_xref_jurisdiction ON "ob-poc".master_entity_xref USING btree (jurisdiction_code);


--
-- Name: idx_master_entity_xref_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_master_entity_xref_name ON "ob-poc".master_entity_xref USING gin (to_tsvector('english'::regconfig, (entity_name)::text));


--
-- Name: idx_master_entity_xref_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_master_entity_xref_status ON "ob-poc".master_entity_xref USING btree (entity_status);


--
-- Name: idx_master_entity_xref_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_master_entity_xref_type ON "ob-poc".master_entity_xref USING btree (entity_type);


--
-- Name: idx_monitoring_activities_case; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_activities_case ON "ob-poc".monitoring_activities USING btree (case_id);


--
-- Name: idx_monitoring_activities_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_activities_cbu ON "ob-poc".monitoring_activities USING btree (cbu_id);


--
-- Name: idx_monitoring_activities_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_activities_type ON "ob-poc".monitoring_activities USING btree (activity_type);


--
-- Name: idx_monitoring_alert_rules_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_alert_rules_active ON "ob-poc".monitoring_alert_rules USING btree (cbu_id, is_active) WHERE (is_active = true);


--
-- Name: idx_monitoring_alert_rules_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_alert_rules_cbu ON "ob-poc".monitoring_alert_rules USING btree (cbu_id);


--
-- Name: idx_monitoring_cases_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_cases_cbu ON "ob-poc".monitoring_cases USING btree (cbu_id);


--
-- Name: idx_monitoring_cases_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_cases_status ON "ob-poc".monitoring_cases USING btree (status);


--
-- Name: idx_monitoring_events_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_events_cbu ON "ob-poc".monitoring_events USING btree (cbu_id);


--
-- Name: idx_monitoring_events_review; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_events_review ON "ob-poc".monitoring_events USING btree (requires_review) WHERE (requires_review = true);


--
-- Name: idx_monitoring_events_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_events_type ON "ob-poc".monitoring_events USING btree (event_type);


--
-- Name: idx_monitoring_reviews_case; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_reviews_case ON "ob-poc".monitoring_reviews USING btree (case_id);


--
-- Name: idx_monitoring_reviews_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_reviews_cbu ON "ob-poc".monitoring_reviews USING btree (cbu_id);


--
-- Name: idx_monitoring_reviews_due; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_reviews_due ON "ob-poc".monitoring_reviews USING btree (due_date) WHERE ((status)::text = ANY (ARRAY[('SCHEDULED'::character varying)::text, ('OVERDUE'::character varying)::text]));


--
-- Name: idx_monitoring_setup_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_monitoring_setup_cbu ON "ob-poc".monitoring_setup USING btree (cbu_id);


--
-- Name: idx_onboarding_allocations_request; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_onboarding_allocations_request ON "ob-poc".onboarding_resource_allocations USING btree (request_id);


--
-- Name: idx_onboarding_configs_request; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_onboarding_configs_request ON "ob-poc".onboarding_service_configs USING btree (request_id);


--
-- Name: idx_onboarding_products_request; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_onboarding_products_request ON "ob-poc".onboarding_products USING btree (request_id);


--
-- Name: idx_onboarding_request_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_onboarding_request_cbu ON "ob-poc".onboarding_requests USING btree (cbu_id);


--
-- Name: idx_onboarding_request_state; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_onboarding_request_state ON "ob-poc".onboarding_requests USING btree (request_state);


--
-- Name: idx_option_choices_def; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_option_choices_def ON "ob-poc".service_option_choices USING btree (option_def_id);


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
-- Name: idx_ownership_owned; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ownership_owned ON "ob-poc".ownership_relationships USING btree (owned_entity_id);


--
-- Name: idx_ownership_owner; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ownership_owner ON "ob-poc".ownership_relationships USING btree (owner_entity_id);


--
-- Name: idx_ownership_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ownership_type ON "ob-poc".ownership_relationships USING btree (ownership_type);


--
-- Name: idx_parsed_asts_grammar_version; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_parsed_asts_grammar_version ON "ob-poc".parsed_asts USING btree (grammar_version);


--
-- Name: idx_parsed_asts_hash; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_parsed_asts_hash ON "ob-poc".parsed_asts USING btree (ast_hash);


--
-- Name: idx_parsed_asts_parsed_at; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_parsed_asts_parsed_at ON "ob-poc".parsed_asts USING btree (parsed_at DESC);


--
-- Name: idx_parsed_asts_version_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_parsed_asts_version_id ON "ob-poc".parsed_asts USING btree (version_id);


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
-- Name: idx_partnerships_entity_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnerships_entity_id ON "ob-poc".entity_partnerships USING btree (entity_id);


--
-- Name: idx_partnerships_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnerships_jurisdiction ON "ob-poc".entity_partnerships USING btree (jurisdiction);


--
-- Name: idx_partnerships_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_partnerships_type ON "ob-poc".entity_partnerships USING btree (partnership_type);


--
-- Name: idx_persons_first_name_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_persons_first_name_trgm ON "ob-poc".entity_proper_persons USING gin (first_name public.gin_trgm_ops);


--
-- Name: idx_persons_last_name_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_persons_last_name_trgm ON "ob-poc".entity_proper_persons USING gin (last_name public.gin_trgm_ops);


--
-- Name: idx_persons_search_name_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_persons_search_name_trgm ON "ob-poc".entity_proper_persons USING gin (search_name public.gin_trgm_ops);


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
-- Name: idx_products_is_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_products_is_active ON "ob-poc".products USING btree (is_active);


--
-- Name: idx_products_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_products_name ON "ob-poc".products USING btree (name);


--
-- Name: idx_products_product_code; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_products_product_code ON "ob-poc".products USING btree (product_code);


--
-- Name: idx_proper_persons_entity_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_proper_persons_entity_id ON "ob-poc".entity_proper_persons USING btree (entity_id);


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
-- Name: idx_rag_embeddings_asset; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_rag_embeddings_asset ON "ob-poc".rag_embeddings USING btree (asset_type);


--
-- Name: idx_rag_embeddings_relevance; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_rag_embeddings_relevance ON "ob-poc".rag_embeddings USING btree (relevance_score DESC);


--
-- Name: idx_rag_embeddings_source; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_rag_embeddings_source ON "ob-poc".rag_embeddings USING btree (source_table) WHERE (source_table IS NOT NULL);


--
-- Name: idx_rag_embeddings_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_rag_embeddings_type ON "ob-poc".rag_embeddings USING btree (content_type);


--
-- Name: idx_rag_embeddings_usage; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_rag_embeddings_usage ON "ob-poc".rag_embeddings USING btree (usage_count DESC);


--
-- Name: idx_resource_requirements_resource; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_resource_requirements_resource ON "ob-poc".resource_attribute_requirements USING btree (resource_id);


--
-- Name: idx_ria_attribute; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ria_attribute ON "ob-poc".resource_instance_attributes USING btree (attribute_id);


--
-- Name: idx_ria_instance; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_ria_instance ON "ob-poc".resource_instance_attributes USING btree (instance_id);


--
-- Name: idx_risk_assessments_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_assessments_cbu ON "ob-poc".risk_assessments USING btree (cbu_id);


--
-- Name: idx_risk_assessments_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_assessments_entity ON "ob-poc".risk_assessments USING btree (entity_id);


--
-- Name: idx_risk_assessments_rating; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_assessments_rating ON "ob-poc".risk_assessments USING btree (rating);


--
-- Name: idx_risk_flags_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_flags_cbu ON "ob-poc".risk_flags USING btree (cbu_id);


--
-- Name: idx_risk_flags_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_flags_entity ON "ob-poc".risk_flags USING btree (entity_id);


--
-- Name: idx_risk_flags_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_flags_status ON "ob-poc".risk_flags USING btree (status);


--
-- Name: idx_risk_rating_changes_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_rating_changes_cbu ON "ob-poc".risk_rating_changes USING btree (cbu_id);


--
-- Name: idx_risk_ratings_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_ratings_cbu ON "ob-poc".risk_ratings USING btree (cbu_id);


--
-- Name: idx_risk_ratings_current; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_ratings_current ON "ob-poc".risk_ratings USING btree (cbu_id, effective_to) WHERE (effective_to IS NULL);


--
-- Name: idx_risk_ratings_rating; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_risk_ratings_rating ON "ob-poc".risk_ratings USING btree (rating);


--
-- Name: idx_roles_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_roles_name ON "ob-poc".roles USING btree (name);


--
-- Name: idx_scheduled_reviews_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_scheduled_reviews_cbu ON "ob-poc".scheduled_reviews USING btree (cbu_id);


--
-- Name: idx_scheduled_reviews_due; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_scheduled_reviews_due ON "ob-poc".scheduled_reviews USING btree (due_date);


--
-- Name: idx_scheduled_reviews_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_scheduled_reviews_status ON "ob-poc".scheduled_reviews USING btree (status);


--
-- Name: idx_screening_batches_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screening_batches_cbu ON "ob-poc".screening_batches USING btree (cbu_id);


--
-- Name: idx_screening_batches_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screening_batches_status ON "ob-poc".screening_batches USING btree (status);


--
-- Name: idx_screening_hit_resolutions_resolution; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screening_hit_resolutions_resolution ON "ob-poc".screening_hit_resolutions USING btree (resolution);


--
-- Name: idx_screening_hit_resolutions_screening; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screening_hit_resolutions_screening ON "ob-poc".screening_hit_resolutions USING btree (screening_id);


--
-- Name: idx_screening_lists_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screening_lists_type ON "ob-poc".screening_lists USING btree (list_type);


--
-- Name: idx_screenings_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screenings_entity ON "ob-poc".screenings USING btree (entity_id);


--
-- Name: idx_screenings_investigation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screenings_investigation ON "ob-poc".screenings USING btree (investigation_id);


--
-- Name: idx_screenings_result; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screenings_result ON "ob-poc".screenings USING btree (result);


--
-- Name: idx_screenings_type; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_screenings_type ON "ob-poc".screenings USING btree (screening_type);


--
-- Name: idx_sdm_cbu; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_sdm_cbu ON "ob-poc".service_delivery_map USING btree (cbu_id);


--
-- Name: idx_sdm_product; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_sdm_product ON "ob-poc".service_delivery_map USING btree (product_id);


--
-- Name: idx_sdm_service; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_sdm_service ON "ob-poc".service_delivery_map USING btree (service_id);


--
-- Name: idx_sdm_status; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_sdm_status ON "ob-poc".service_delivery_map USING btree (delivery_status);


--
-- Name: idx_service_capabilities_resource; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_capabilities_resource ON "ob-poc".service_resource_capabilities USING btree (resource_id);


--
-- Name: idx_service_capabilities_service; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_capabilities_service ON "ob-poc".service_resource_capabilities USING btree (service_id);


--
-- Name: idx_service_options_service; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_options_service ON "ob-poc".service_option_definitions USING btree (service_id);


--
-- Name: idx_service_resource_types_dict_group; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_resource_types_dict_group ON "ob-poc".service_resource_types USING btree (dictionary_group);


--
-- Name: idx_service_resource_types_is_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_resource_types_is_active ON "ob-poc".service_resource_types USING btree (is_active);


--
-- Name: idx_service_resource_types_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_resource_types_name ON "ob-poc".service_resource_types USING btree (name);


--
-- Name: idx_service_resource_types_owner; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_resource_types_owner ON "ob-poc".service_resource_types USING btree (owner);


--
-- Name: idx_service_resource_types_resource_code; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_service_resource_types_resource_code ON "ob-poc".service_resource_types USING btree (resource_code);


--
-- Name: idx_services_is_active; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_services_is_active ON "ob-poc".services USING btree (is_active);


--
-- Name: idx_services_name; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_services_name ON "ob-poc".services USING btree (name);


--
-- Name: idx_services_service_code; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_services_service_code ON "ob-poc".services USING btree (service_code);


--
-- Name: idx_similarity_expires; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_similarity_expires ON "ob-poc".csg_semantic_similarity_cache USING btree (expires_at);


--
-- Name: idx_similarity_score; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_similarity_score ON "ob-poc".csg_semantic_similarity_cache USING btree (cosine_similarity DESC);


--
-- Name: idx_similarity_source; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_similarity_source ON "ob-poc".csg_semantic_similarity_cache USING btree (source_type, source_code);


--
-- Name: idx_similarity_target; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_similarity_target ON "ob-poc".csg_semantic_similarity_cache USING btree (target_type, target_code);


--
-- Name: idx_taxonomy_audit_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_audit_entity ON "ob-poc".taxonomy_audit_log USING btree (entity_id, created_at DESC);


--
-- Name: idx_taxonomy_audit_operation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_audit_operation ON "ob-poc".taxonomy_audit_log USING btree (operation, created_at DESC);


--
-- Name: idx_taxonomy_audit_timestamp; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_audit_timestamp ON "ob-poc".taxonomy_audit_log USING btree (created_at DESC);


--
-- Name: idx_taxonomy_audit_user; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_audit_user ON "ob-poc".taxonomy_audit_log USING btree (user_id, created_at DESC);


--
-- Name: idx_taxonomy_crud_entity; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_crud_entity ON "ob-poc".taxonomy_crud_log USING btree (entity_type, entity_id);


--
-- Name: idx_taxonomy_crud_operation; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_crud_operation ON "ob-poc".taxonomy_crud_log USING btree (operation_type);


--
-- Name: idx_taxonomy_crud_time; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_crud_time ON "ob-poc".taxonomy_crud_log USING btree (created_at);


--
-- Name: idx_taxonomy_crud_user; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_taxonomy_crud_user ON "ob-poc".taxonomy_crud_log USING btree (user_id);


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
-- Name: idx_trusts_entity_id; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trusts_entity_id ON "ob-poc".entity_trusts USING btree (entity_id);


--
-- Name: idx_trusts_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trusts_jurisdiction ON "ob-poc".entity_trusts USING btree (jurisdiction);


--
-- Name: idx_trusts_name_trgm; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_trusts_name_trgm ON "ob-poc".entity_trusts USING gin (trust_name public.gin_trgm_ops);


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
-- Name: idx_values_attr_uuid; Type: INDEX; Schema: ob-poc; Owner: adamtc007
--

CREATE INDEX idx_values_attr_uuid ON "ob-poc".attribute_values_typed USING btree (attribute_uuid);


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
-- Name: idx_actions_active; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_actions_active ON public.actions_registry USING btree (active);


--
-- Name: idx_actions_domain; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_actions_domain ON public.actions_registry USING btree (domain);


--
-- Name: idx_actions_resource_type; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_actions_resource_type ON public.actions_registry USING btree (resource_type_id);


--
-- Name: idx_actions_verb_pattern; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_actions_verb_pattern ON public.actions_registry USING btree (verb_pattern);


--
-- Name: idx_attempts_execution; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_attempts_execution ON public.action_execution_attempts USING btree (execution_id);


--
-- Name: idx_attempts_status; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_attempts_status ON public.action_execution_attempts USING btree (status);


--
-- Name: idx_business_attrs_entity; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_business_attrs_entity ON public.business_attributes USING btree (entity_name);


--
-- Name: idx_credentials_active; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_credentials_active ON public.credentials_vault USING btree (active);


--
-- Name: idx_credentials_environment; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_credentials_environment ON public.credentials_vault USING btree (environment);


--
-- Name: idx_credentials_expires; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_credentials_expires ON public.credentials_vault USING btree (expires_at);


--
-- Name: idx_derived_attrs_entity; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_derived_attrs_entity ON public.derived_attributes USING btree (entity_name);


--
-- Name: idx_executions_action; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_action ON public.action_executions USING btree (action_id);


--
-- Name: idx_executions_cbu; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_cbu ON public.action_executions USING btree (cbu_id);


--
-- Name: idx_executions_idempotency; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_idempotency ON public.action_executions USING btree (idempotency_key);


--
-- Name: idx_executions_rule; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_rule ON public.rule_executions USING btree (rule_id);


--
-- Name: idx_executions_started_at; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_started_at ON public.action_executions USING btree (started_at);


--
-- Name: idx_executions_status; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_status ON public.action_executions USING btree (execution_status);


--
-- Name: idx_executions_time; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_executions_time ON public.rule_executions USING btree (execution_time);


--
-- Name: idx_resource_types_name_env_ver; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE UNIQUE INDEX idx_resource_types_name_env_ver ON public.resource_types USING btree (resource_type_name, environment, version);


--
-- Name: idx_rule_deps_attr; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rule_deps_attr ON public.rule_dependencies USING btree (attribute_id);


--
-- Name: idx_rule_deps_rule; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rule_deps_rule ON public.rule_dependencies USING btree (rule_id);


--
-- Name: idx_rules_category; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rules_category ON public.rules USING btree (category_id);


--
-- Name: idx_rules_embedding; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rules_embedding ON public.rules USING hnsw (embedding public.vector_cosine_ops);


--
-- Name: idx_rules_search; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rules_search ON public.rules USING gin (search_vector);


--
-- Name: idx_rules_status; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rules_status ON public.rules USING btree (status);


--
-- Name: idx_rules_target; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE INDEX idx_rules_target ON public.rules USING btree (target_attribute_id);


--
-- Name: uq_action_dedupe; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE UNIQUE INDEX uq_action_dedupe ON public.action_executions USING btree (action_id, cbu_id, idempotency_key) WHERE (idempotency_key IS NOT NULL);


--
-- Name: uq_attempt_seq; Type: INDEX; Schema: public; Owner: adamtc007
--

CREATE UNIQUE INDEX uq_attempt_seq ON public.action_execution_attempts USING btree (execution_id, attempt_no);


--
-- Name: cbu_resource_instances trg_cri_updated; Type: TRIGGER; Schema: ob-poc; Owner: adamtc007
--

CREATE TRIGGER trg_cri_updated BEFORE UPDATE ON "ob-poc".cbu_resource_instances FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();


--
-- Name: service_delivery_map trg_sdm_updated; Type: TRIGGER; Schema: ob-poc; Owner: adamtc007
--

CREATE TRIGGER trg_sdm_updated BEFORE UPDATE ON "ob-poc".service_delivery_map FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();


--
-- Name: dsl_versions trigger_invalidate_ast_cache; Type: TRIGGER; Schema: ob-poc; Owner: adamtc007
--

CREATE TRIGGER trigger_invalidate_ast_cache AFTER UPDATE ON "ob-poc".dsl_versions FOR EACH ROW EXECUTE FUNCTION "ob-poc".invalidate_ast_cache();


--
-- Name: attribute_registry trigger_update_attribute_registry_timestamp; Type: TRIGGER; Schema: ob-poc; Owner: adamtc007
--

CREATE TRIGGER trigger_update_attribute_registry_timestamp BEFORE UPDATE ON "ob-poc".attribute_registry FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_attribute_registry_timestamp();


--
-- Name: business_attributes update_business_attributes_updated_at; Type: TRIGGER; Schema: public; Owner: adamtc007
--

CREATE TRIGGER update_business_attributes_updated_at BEFORE UPDATE ON public.business_attributes FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: derived_attributes update_derived_attributes_updated_at; Type: TRIGGER; Schema: public; Owner: adamtc007
--

CREATE TRIGGER update_derived_attributes_updated_at BEFORE UPDATE ON public.derived_attributes FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: rules update_rules_updated_at; Type: TRIGGER; Schema: public; Owner: adamtc007
--

CREATE TRIGGER update_rules_updated_at BEFORE UPDATE ON public.rules FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


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
-- Name: attribute_values_typed attribute_values_typed_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT attribute_values_typed_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(id);


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
-- Name: cbu_resource_instances cbu_resource_instances_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_resource_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_resource_type_id_fkey FOREIGN KEY (resource_type_id) REFERENCES "ob-poc".service_resource_types(resource_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id);


--
-- Name: crud_operations crud_operations_parent_operation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".crud_operations
    ADD CONSTRAINT crud_operations_parent_operation_id_fkey FOREIGN KEY (parent_operation_id) REFERENCES "ob-poc".crud_operations(operation_id);


--
-- Name: csg_rule_overrides csg_rule_overrides_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_rule_overrides
    ADD CONSTRAINT csg_rule_overrides_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: csg_rule_overrides csg_rule_overrides_rule_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".csg_rule_overrides
    ADD CONSTRAINT csg_rule_overrides_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES "ob-poc".csg_validation_rules(rule_id) ON DELETE CASCADE;


--
-- Name: decision_conditions decision_conditions_decision_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".decision_conditions
    ADD CONSTRAINT decision_conditions_decision_id_fkey FOREIGN KEY (decision_id) REFERENCES "ob-poc".kyc_decisions(decision_id) ON DELETE CASCADE;


--
-- Name: document_attribute_mappings document_attribute_mappings_attribute_uuid_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_attribute_uuid_fkey FOREIGN KEY (attribute_uuid) REFERENCES "ob-poc".attribute_registry(uuid) ON DELETE CASCADE;


--
-- Name: document_attribute_mappings document_attribute_mappings_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_attribute_mappings
    ADD CONSTRAINT document_attribute_mappings_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id) ON DELETE CASCADE;


--
-- Name: document_catalog document_catalog_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id);


--
-- Name: document_entity_links document_entity_links_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_entity_links
    ADD CONSTRAINT document_entity_links_doc_id_fkey FOREIGN KEY (doc_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: document_entity_links document_entity_links_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_entity_links
    ADD CONSTRAINT document_entity_links_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: document_metadata document_metadata_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_metadata
    ADD CONSTRAINT document_metadata_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE;


--
-- Name: document_metadata document_metadata_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_metadata
    ADD CONSTRAINT document_metadata_doc_id_fkey FOREIGN KEY (doc_id) REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE;


--
-- Name: document_relationships document_relationships_primary_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_primary_doc_id_fkey FOREIGN KEY (primary_doc_id) REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE;


--
-- Name: document_relationships document_relationships_related_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_relationships
    ADD CONSTRAINT document_relationships_related_doc_id_fkey FOREIGN KEY (related_doc_id) REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE;


--
-- Name: document_requests document_requests_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_requests
    ADD CONSTRAINT document_requests_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id);


--
-- Name: dsl_execution_log dsl_execution_log_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_execution_log
    ADD CONSTRAINT dsl_execution_log_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE;


--
-- Name: dsl_generation_log dsl_generation_log_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_generation_log
    ADD CONSTRAINT dsl_generation_log_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id);


--
-- Name: dsl_versions dsl_versions_domain_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES "ob-poc".dsl_domains(domain_id) ON DELETE CASCADE;


--
-- Name: dsl_versions dsl_versions_parent_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_versions
    ADD CONSTRAINT dsl_versions_parent_version_id_fkey FOREIGN KEY (parent_version_id) REFERENCES "ob-poc".dsl_versions(version_id);


--
-- Name: entities entities_entity_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_entity_type_id_fkey FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types(entity_type_id) ON DELETE CASCADE;


--
-- Name: entity_limited_companies entity_limited_companies_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_partnerships entity_partnerships_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_partnerships
    ADD CONSTRAINT entity_partnerships_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_product_mappings entity_product_mappings_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT entity_product_mappings_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: entity_proper_persons entity_proper_persons_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_trusts entity_trusts_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_trusts
    ADD CONSTRAINT entity_trusts_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_types entity_types_parent_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_types
    ADD CONSTRAINT entity_types_parent_type_id_fkey FOREIGN KEY (parent_type_id) REFERENCES "ob-poc".entity_types(entity_type_id);


--
-- Name: attribute_values_typed fk_attribute_uuid; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT fk_attribute_uuid FOREIGN KEY (attribute_uuid) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: attribute_values fk_attribute_values_dsl_ob_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".attribute_values
    ADD CONSTRAINT fk_attribute_values_dsl_ob_id FOREIGN KEY (dsl_ob_id) REFERENCES "ob-poc".dsl_ob(version_id) ON DELETE SET NULL;


--
-- Name: cbu_creation_log fk_cbu_creation_log_cbu; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_creation_log
    ADD CONSTRAINT fk_cbu_creation_log_cbu FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles fk_cbu_entity_roles_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT fk_cbu_entity_roles_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles fk_cbu_entity_roles_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT fk_cbu_entity_roles_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: cbu_entity_roles fk_cbu_entity_roles_role_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT fk_cbu_entity_roles_role_id FOREIGN KEY (role_id) REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE;


--
-- Name: document_catalog fk_document_catalog_cbu; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT fk_document_catalog_cbu FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: dsl_ob fk_dsl_ob_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_ob
    ADD CONSTRAINT fk_dsl_ob_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: entities fk_entities_entity_type_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT fk_entities_entity_type_id FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types(entity_type_id) ON DELETE CASCADE;


--
-- Name: entity_product_mappings fk_entity_product_mappings_product_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_product_mappings
    ADD CONSTRAINT fk_entity_product_mappings_product_id FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: entity_role_connections fk_entity_role_connections_cbu; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT fk_entity_role_connections_cbu FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: entity_role_connections fk_entity_role_connections_entity; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".entity_role_connections
    ADD CONSTRAINT fk_entity_role_connections_entity FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: dsl_instance_versions fk_instance; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT fk_instance FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


--
-- Name: orchestration_domain_sessions fk_orchestration_domain_sessions_orchestration_session_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_domain_sessions
    ADD CONSTRAINT fk_orchestration_domain_sessions_orchestration_session_id FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_sessions fk_orchestration_sessions_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_sessions
    ADD CONSTRAINT fk_orchestration_sessions_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;


--
-- Name: orchestration_state_history fk_orchestration_state_history_orchestration_session_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_state_history
    ADD CONSTRAINT fk_orchestration_state_history_orchestration_session_id FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: orchestration_tasks fk_orchestration_tasks_orchestration_session_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".orchestration_tasks
    ADD CONSTRAINT fk_orchestration_tasks_orchestration_session_id FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms fk_partnership_control_mechanisms_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT fk_partnership_control_mechanisms_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_control_mechanisms fk_partnership_control_mechanisms_partnership_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_control_mechanisms
    ADD CONSTRAINT fk_partnership_control_mechanisms_partnership_id FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: partnership_interests fk_partnership_interests_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT fk_partnership_interests_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: partnership_interests fk_partnership_interests_partnership_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".partnership_interests
    ADD CONSTRAINT fk_partnership_interests_partnership_id FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;


--
-- Name: product_requirements fk_product_requirements_product_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_requirements
    ADD CONSTRAINT fk_product_requirements_product_id FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: product_workflows fk_product_workflows_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT fk_product_workflows_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: product_workflows fk_product_workflows_product_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".product_workflows
    ADD CONSTRAINT fk_product_workflows_product_id FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;


--
-- Name: service_resources fk_service_resources_resource_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT fk_service_resources_resource_id FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE;


--
-- Name: service_resources fk_service_resources_service_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT fk_service_resources_service_id FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: trust_beneficiary_classes fk_trust_beneficiary_classes_trust_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_beneficiary_classes
    ADD CONSTRAINT fk_trust_beneficiary_classes_trust_id FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_parties fk_trust_parties_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT fk_trust_parties_entity_id FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: trust_parties fk_trust_parties_trust_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_parties
    ADD CONSTRAINT fk_trust_parties_trust_id FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;


--
-- Name: trust_protector_powers fk_trust_protector_powers_trust_party_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".trust_protector_powers
    ADD CONSTRAINT fk_trust_protector_powers_trust_party_id FOREIGN KEY (trust_party_id) REFERENCES "ob-poc".trust_parties(trust_party_id) ON DELETE CASCADE;


--
-- Name: ubo_registry fk_ubo_registry_cbu_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_cbu_id FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;


--
-- Name: ubo_registry fk_ubo_registry_subject_entity_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_subject_entity_id FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: ubo_registry fk_ubo_registry_ubo_proper_person_id; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_ubo_proper_person_id FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: investigation_assignments investigation_assignments_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".investigation_assignments
    ADD CONSTRAINT investigation_assignments_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE;


--
-- Name: kyc_decisions kyc_decisions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".kyc_decisions
    ADD CONSTRAINT kyc_decisions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: kyc_decisions kyc_decisions_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".kyc_decisions
    ADD CONSTRAINT kyc_decisions_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE;


--
-- Name: kyc_investigations kyc_investigations_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".kyc_investigations
    ADD CONSTRAINT kyc_investigations_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: master_entity_xref master_entity_xref_jurisdiction_code_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".master_entity_xref
    ADD CONSTRAINT master_entity_xref_jurisdiction_code_fkey FOREIGN KEY (jurisdiction_code) REFERENCES "ob-poc".master_jurisdictions(jurisdiction_code);


--
-- Name: monitoring_activities monitoring_activities_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_activities
    ADD CONSTRAINT monitoring_activities_case_id_fkey FOREIGN KEY (case_id) REFERENCES "ob-poc".monitoring_cases(case_id);


--
-- Name: monitoring_activities monitoring_activities_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_activities
    ADD CONSTRAINT monitoring_activities_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: monitoring_alert_rules monitoring_alert_rules_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_alert_rules
    ADD CONSTRAINT monitoring_alert_rules_case_id_fkey FOREIGN KEY (case_id) REFERENCES "ob-poc".monitoring_cases(case_id);


--
-- Name: monitoring_alert_rules monitoring_alert_rules_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_alert_rules
    ADD CONSTRAINT monitoring_alert_rules_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: monitoring_cases monitoring_cases_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_cases
    ADD CONSTRAINT monitoring_cases_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: monitoring_events monitoring_events_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_events
    ADD CONSTRAINT monitoring_events_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: monitoring_reviews monitoring_reviews_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_reviews
    ADD CONSTRAINT monitoring_reviews_case_id_fkey FOREIGN KEY (case_id) REFERENCES "ob-poc".monitoring_cases(case_id);


--
-- Name: monitoring_reviews monitoring_reviews_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_reviews
    ADD CONSTRAINT monitoring_reviews_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: monitoring_setup monitoring_setup_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".monitoring_setup
    ADD CONSTRAINT monitoring_setup_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: onboarding_products onboarding_products_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: onboarding_products onboarding_products_request_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_request_id_fkey FOREIGN KEY (request_id) REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE;


--
-- Name: onboarding_requests onboarding_requests_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_requests
    ADD CONSTRAINT onboarding_requests_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: onboarding_resource_allocations onboarding_resource_allocations_request_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_resource_allocations
    ADD CONSTRAINT onboarding_resource_allocations_request_id_fkey FOREIGN KEY (request_id) REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE;


--
-- Name: onboarding_resource_allocations onboarding_resource_allocations_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_resource_allocations
    ADD CONSTRAINT onboarding_resource_allocations_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id);


--
-- Name: onboarding_resource_allocations onboarding_resource_allocations_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_resource_allocations
    ADD CONSTRAINT onboarding_resource_allocations_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id);


--
-- Name: onboarding_service_configs onboarding_service_configs_request_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_service_configs
    ADD CONSTRAINT onboarding_service_configs_request_id_fkey FOREIGN KEY (request_id) REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE;


--
-- Name: onboarding_service_configs onboarding_service_configs_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".onboarding_service_configs
    ADD CONSTRAINT onboarding_service_configs_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id);


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
-- Name: ownership_relationships ownership_relationships_evidence_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_evidence_doc_id_fkey FOREIGN KEY (evidence_doc_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: ownership_relationships ownership_relationships_owned_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_owned_entity_id_fkey FOREIGN KEY (owned_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: ownership_relationships ownership_relationships_owner_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_owner_entity_id_fkey FOREIGN KEY (owner_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: parsed_asts parsed_asts_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".parsed_asts
    ADD CONSTRAINT parsed_asts_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE;


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
-- Name: resource_attribute_requirements resource_attribute_requirements_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary(attribute_id);


--
-- Name: resource_attribute_requirements resource_attribute_requirements_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE;


--
-- Name: resource_instance_attributes resource_instance_attributes_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".dictionary(attribute_id);


--
-- Name: resource_instance_attributes resource_instance_attributes_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE;


--
-- Name: risk_assessments risk_assessments_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_assessments
    ADD CONSTRAINT risk_assessments_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: risk_assessments risk_assessments_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_assessments
    ADD CONSTRAINT risk_assessments_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: risk_assessments risk_assessments_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_assessments
    ADD CONSTRAINT risk_assessments_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE;


--
-- Name: risk_flags risk_flags_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_flags
    ADD CONSTRAINT risk_flags_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: risk_flags risk_flags_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_flags
    ADD CONSTRAINT risk_flags_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: risk_flags risk_flags_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_flags
    ADD CONSTRAINT risk_flags_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE;


--
-- Name: risk_rating_changes risk_rating_changes_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_rating_changes
    ADD CONSTRAINT risk_rating_changes_case_id_fkey FOREIGN KEY (case_id) REFERENCES "ob-poc".monitoring_cases(case_id);


--
-- Name: risk_rating_changes risk_rating_changes_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_rating_changes
    ADD CONSTRAINT risk_rating_changes_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: risk_rating_changes risk_rating_changes_review_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_rating_changes
    ADD CONSTRAINT risk_rating_changes_review_id_fkey FOREIGN KEY (review_id) REFERENCES "ob-poc".monitoring_reviews(review_id);


--
-- Name: risk_ratings risk_ratings_assessment_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_ratings
    ADD CONSTRAINT risk_ratings_assessment_id_fkey FOREIGN KEY (assessment_id) REFERENCES "ob-poc".risk_assessments(assessment_id);


--
-- Name: risk_ratings risk_ratings_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".risk_ratings
    ADD CONSTRAINT risk_ratings_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: scheduled_reviews scheduled_reviews_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".scheduled_reviews
    ADD CONSTRAINT scheduled_reviews_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: screening_batch_results screening_batch_results_batch_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_batch_results
    ADD CONSTRAINT screening_batch_results_batch_id_fkey FOREIGN KEY (batch_id) REFERENCES "ob-poc".screening_batches(batch_id);


--
-- Name: screening_batch_results screening_batch_results_screening_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_batch_results
    ADD CONSTRAINT screening_batch_results_screening_id_fkey FOREIGN KEY (screening_id) REFERENCES "ob-poc".screenings(screening_id);


--
-- Name: screening_batches screening_batches_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_batches
    ADD CONSTRAINT screening_batches_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: screening_batches screening_batches_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_batches
    ADD CONSTRAINT screening_batches_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id);


--
-- Name: screening_hit_resolutions screening_hit_resolutions_screening_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_hit_resolutions
    ADD CONSTRAINT screening_hit_resolutions_screening_id_fkey FOREIGN KEY (screening_id) REFERENCES "ob-poc".screenings(screening_id);


--
-- Name: screening_hit_resolutions screening_hit_resolutions_ubo_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screening_hit_resolutions
    ADD CONSTRAINT screening_hit_resolutions_ubo_id_fkey FOREIGN KEY (ubo_id) REFERENCES "ob-poc".ubo_registry(ubo_id);


--
-- Name: screenings screenings_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screenings
    ADD CONSTRAINT screenings_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: screenings screenings_investigation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".screenings
    ADD CONSTRAINT screenings_investigation_id_fkey FOREIGN KEY (investigation_id) REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE;


--
-- Name: service_delivery_map service_delivery_map_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: service_delivery_map service_delivery_map_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: service_delivery_map service_delivery_map_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: service_delivery_map service_delivery_map_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id);


--
-- Name: service_discovery_cache service_discovery_cache_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_discovery_cache
    ADD CONSTRAINT service_discovery_cache_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: service_option_choices service_option_choices_option_def_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_option_choices
    ADD CONSTRAINT service_option_choices_option_def_id_fkey FOREIGN KEY (option_def_id) REFERENCES "ob-poc".service_option_definitions(option_def_id) ON DELETE CASCADE;


--
-- Name: service_option_definitions service_option_definitions_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_option_definitions
    ADD CONSTRAINT service_option_definitions_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: service_resource_capabilities service_resource_capabilities_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE;


--
-- Name: service_resource_capabilities service_resource_capabilities_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: service_resources service_resources_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: adamtc007
--

ALTER TABLE ONLY "ob-poc".service_resources
    ADD CONSTRAINT service_resources_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE;


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
-- Name: action_execution_attempts action_execution_attempts_execution_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.action_execution_attempts
    ADD CONSTRAINT action_execution_attempts_execution_id_fkey FOREIGN KEY (execution_id) REFERENCES public.action_executions(execution_id) ON DELETE CASCADE;


--
-- Name: action_executions action_executions_action_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.action_executions
    ADD CONSTRAINT action_executions_action_id_fkey FOREIGN KEY (action_id) REFERENCES public.actions_registry(action_id);


--
-- Name: action_executions action_executions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.action_executions
    ADD CONSTRAINT action_executions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: action_executions action_executions_dsl_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.action_executions
    ADD CONSTRAINT action_executions_dsl_version_id_fkey FOREIGN KEY (dsl_version_id) REFERENCES "ob-poc".dsl_ob(version_id);


--
-- Name: actions_registry actions_registry_resource_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.actions_registry
    ADD CONSTRAINT actions_registry_resource_type_id_fkey FOREIGN KEY (resource_type_id) REFERENCES public.resource_types(resource_type_id);


--
-- Name: business_attributes business_attributes_domain_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES public.data_domains(id);


--
-- Name: business_attributes business_attributes_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_source_id_fkey FOREIGN KEY (source_id) REFERENCES public.attribute_sources(id);


--
-- Name: derived_attributes derived_attributes_domain_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.derived_attributes
    ADD CONSTRAINT derived_attributes_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES public.data_domains(id);


--
-- Name: resource_type_attributes resource_type_attributes_resource_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.resource_type_attributes
    ADD CONSTRAINT resource_type_attributes_resource_type_id_fkey FOREIGN KEY (resource_type_id) REFERENCES public.resource_types(resource_type_id) ON DELETE CASCADE;


--
-- Name: resource_type_endpoints resource_type_endpoints_resource_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.resource_type_endpoints
    ADD CONSTRAINT resource_type_endpoints_resource_type_id_fkey FOREIGN KEY (resource_type_id) REFERENCES public.resource_types(resource_type_id) ON DELETE CASCADE;


--
-- Name: rule_dependencies rule_dependencies_attribute_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES public.business_attributes(id);


--
-- Name: rule_dependencies rule_dependencies_rule_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;


--
-- Name: rule_executions rule_executions_rule_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_executions
    ADD CONSTRAINT rule_executions_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id);


--
-- Name: rule_versions rule_versions_rule_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rule_versions
    ADD CONSTRAINT rule_versions_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;


--
-- Name: rules rules_category_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_category_id_fkey FOREIGN KEY (category_id) REFERENCES public.rule_categories(id);


--
-- Name: rules rules_target_attribute_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: adamtc007
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_target_attribute_id_fkey FOREIGN KEY (target_attribute_id) REFERENCES public.derived_attributes(id);


--
-- Name: SCHEMA "ob-poc"; Type: ACL; Schema: -; Owner: adamtc007
--

GRANT USAGE ON SCHEMA "ob-poc" TO PUBLIC;


--
-- Name: SCHEMA public; Type: ACL; Schema: -; Owner: adamtc007
--

REVOKE USAGE ON SCHEMA public FROM PUBLIC;
GRANT ALL ON SCHEMA public TO PUBLIC;


--
-- Name: TABLE cbus; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".cbus TO PUBLIC;


--
-- Name: TABLE attribute_values; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".attribute_values TO PUBLIC;


--
-- Name: TABLE cbu_entity_roles; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".cbu_entity_roles TO PUBLIC;


--
-- Name: TABLE crud_operations; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".crud_operations TO PUBLIC;


--
-- Name: TABLE dictionary; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dictionary TO PUBLIC;


--
-- Name: TABLE document_catalog; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".document_catalog TO PUBLIC;


--
-- Name: TABLE document_issuers_backup; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".document_issuers_backup TO PUBLIC;


--
-- Name: TABLE document_metadata; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".document_metadata TO PUBLIC;


--
-- Name: TABLE document_relationships; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".document_relationships TO PUBLIC;


--
-- Name: TABLE document_types; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".document_types TO PUBLIC;


--
-- Name: TABLE domain_vocabularies; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".domain_vocabularies TO PUBLIC;


--
-- Name: TABLE dsl_domains; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_domains TO PUBLIC;


--
-- Name: TABLE dsl_examples; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_examples TO PUBLIC;


--
-- Name: TABLE dsl_execution_log; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_execution_log TO PUBLIC;


--
-- Name: TABLE dsl_versions; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_versions TO PUBLIC;


--
-- Name: TABLE dsl_execution_summary; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_execution_summary TO PUBLIC;


--
-- Name: TABLE dsl_instances; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_instances TO PUBLIC;


--
-- Name: SEQUENCE dsl_instances_id_seq; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,USAGE ON SEQUENCE "ob-poc".dsl_instances_id_seq TO PUBLIC;


--
-- Name: TABLE parsed_asts; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".parsed_asts TO PUBLIC;


--
-- Name: TABLE dsl_latest_versions; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_latest_versions TO PUBLIC;


--
-- Name: TABLE dsl_ob; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".dsl_ob TO PUBLIC;


--
-- Name: TABLE entities; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entities TO PUBLIC;


--
-- Name: TABLE entity_crud_rules; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_crud_rules TO PUBLIC;


--
-- Name: TABLE entity_lifecycle_status; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_lifecycle_status TO PUBLIC;


--
-- Name: TABLE entity_limited_companies; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_limited_companies TO PUBLIC;


--
-- Name: TABLE entity_partnerships; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_partnerships TO PUBLIC;


--
-- Name: TABLE entity_product_mappings; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_product_mappings TO PUBLIC;


--
-- Name: TABLE entity_proper_persons; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_proper_persons TO PUBLIC;


--
-- Name: TABLE entity_trusts; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_trusts TO PUBLIC;


--
-- Name: TABLE entity_types; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_types TO PUBLIC;


--
-- Name: TABLE entity_validation_rules; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".entity_validation_rules TO PUBLIC;


--
-- Name: TABLE grammar_rules; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".grammar_rules TO PUBLIC;


--
-- Name: TABLE master_jurisdictions; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".master_jurisdictions TO PUBLIC;


--
-- Name: TABLE master_entity_xref; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".master_entity_xref TO PUBLIC;


--
-- Name: TABLE orchestration_domain_sessions; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".orchestration_domain_sessions TO PUBLIC;


--
-- Name: TABLE orchestration_sessions; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".orchestration_sessions TO PUBLIC;


--
-- Name: TABLE orchestration_state_history; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".orchestration_state_history TO PUBLIC;


--
-- Name: TABLE orchestration_tasks; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".orchestration_tasks TO PUBLIC;


--
-- Name: TABLE partnership_control_mechanisms; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".partnership_control_mechanisms TO PUBLIC;


--
-- Name: TABLE partnership_interests; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".partnership_interests TO PUBLIC;


--
-- Name: TABLE product_requirements; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".product_requirements TO PUBLIC;


--
-- Name: TABLE product_services; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".product_services TO PUBLIC;


--
-- Name: TABLE product_workflows; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".product_workflows TO PUBLIC;


--
-- Name: TABLE products; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".products TO PUBLIC;


--
-- Name: TABLE rag_embeddings; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".rag_embeddings TO PUBLIC;


--
-- Name: TABLE referential_integrity_check; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".referential_integrity_check TO PUBLIC;


--
-- Name: TABLE roles; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".roles TO PUBLIC;


--
-- Name: TABLE schema_changes; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".schema_changes TO PUBLIC;


--
-- Name: TABLE service_resource_types; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".service_resource_types TO PUBLIC;


--
-- Name: TABLE service_resources; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".service_resources TO PUBLIC;


--
-- Name: TABLE services; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".services TO PUBLIC;


--
-- Name: TABLE trust_beneficiary_classes; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".trust_beneficiary_classes TO PUBLIC;


--
-- Name: TABLE trust_parties; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".trust_parties TO PUBLIC;


--
-- Name: TABLE trust_protector_powers; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".trust_protector_powers TO PUBLIC;


--
-- Name: TABLE ubo_registry; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".ubo_registry TO PUBLIC;


--
-- Name: TABLE verb_registry; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".verb_registry TO PUBLIC;


--
-- Name: TABLE vocabulary_audit; Type: ACL; Schema: ob-poc; Owner: adamtc007
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE "ob-poc".vocabulary_audit TO PUBLIC;


--
-- PostgreSQL database dump complete
--

\unrestrict sZdH9v5mL2mgep1gJRs6gFooGt56s8brvkwbFjl5tb16yaIZFaohb1JjxkmKwgk

