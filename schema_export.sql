--
-- PostgreSQL database dump
--

\restrict pf3wukrb5CpdDVMpSX0oXeGguoJxqUBr2OLDvKfrIllBTuOTtyT8Gbzalkv5wGV

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
-- Name: client_portal; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA client_portal;


--
-- Name: custody; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA custody;


--
-- Name: kyc; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA kyc;


--
-- Name: ob-poc; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA "ob-poc";


--
-- Name: SCHEMA "ob-poc"; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON SCHEMA "ob-poc" IS 'OB-POC schema with config-driven visualization. Phase 2 adds layout persistence and caching.';


--
-- Name: ob_kyc; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA ob_kyc;


--
-- Name: ob_ref; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA ob_ref;


--
-- Name: public; Type: SCHEMA; Schema: -; Owner: -
--

-- *not* creating schema, since initdb creates it


--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON SCHEMA public IS 'Runtime API Endpoints System - Phase 1 Foundation';


--
-- Name: teams; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA teams;


--
-- Name: fuzzystrmatch; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS fuzzystrmatch WITH SCHEMA public;


--
-- Name: EXTENSION fuzzystrmatch; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION fuzzystrmatch IS 'determine similarities and distance between strings';


--
-- Name: pg_trgm; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;


--
-- Name: EXTENSION pg_trgm; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';


--
-- Name: uuid-ossp; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;


--
-- Name: EXTENSION "uuid-ossp"; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION "uuid-ossp" IS 'generate universally unique identifiers (UUIDs)';


--
-- Name: vector; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;


--
-- Name: EXTENSION vector; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION vector IS 'vector data type and ivfflat and hnsw access methods';


--
-- Name: action_type_enum; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.action_type_enum AS ENUM (
    'HTTP_API',
    'BPMN_WORKFLOW',
    'MESSAGE_QUEUE',
    'DATABASE_OPERATION',
    'EXTERNAL_SERVICE'
);


--
-- Name: execution_status_enum; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.execution_status_enum AS ENUM (
    'PENDING',
    'RUNNING',
    'COMPLETED',
    'FAILED',
    'CANCELLED'
);


--
-- Name: find_ssi_for_trade(uuid, uuid, uuid, uuid, character varying, character varying, uuid); Type: FUNCTION; Schema: custody; Owner: -
--

CREATE FUNCTION custody.find_ssi_for_trade(p_cbu_id uuid, p_instrument_class_id uuid, p_security_type_id uuid, p_market_id uuid, p_currency character varying, p_settlement_type character varying, p_counterparty_entity_id uuid DEFAULT NULL::uuid) RETURNS TABLE(ssi_id uuid, ssi_name character varying, rule_id uuid, rule_name character varying, rule_priority integer, specificity_score integer)
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.ssi_id,
        s.ssi_name,
        r.rule_id,
        r.rule_name,
        r.priority,
        r.specificity_score
    FROM custody.ssi_booking_rules r
    JOIN custody.cbu_ssi s ON r.ssi_id = s.ssi_id
    WHERE r.cbu_id = p_cbu_id
      AND r.is_active = true
      AND s.status = 'ACTIVE'
      AND (r.expiry_date IS NULL OR r.expiry_date > CURRENT_DATE)
      -- Match criteria (NULL = wildcard)
      AND (r.instrument_class_id IS NULL OR r.instrument_class_id = p_instrument_class_id)
      AND (r.security_type_id IS NULL OR r.security_type_id = p_security_type_id)
      AND (r.market_id IS NULL OR r.market_id = p_market_id)
      AND (r.currency IS NULL OR r.currency = p_currency)
      AND (r.settlement_type IS NULL OR r.settlement_type = p_settlement_type)
      AND (r.counterparty_entity_id IS NULL OR r.counterparty_entity_id = p_counterparty_entity_id)
    ORDER BY r.priority ASC
    LIMIT 1;
END;
$$;


--
-- Name: FUNCTION find_ssi_for_trade(p_cbu_id uuid, p_instrument_class_id uuid, p_security_type_id uuid, p_market_id uuid, p_currency character varying, p_settlement_type character varying, p_counterparty_entity_id uuid); Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON FUNCTION custody.find_ssi_for_trade(p_cbu_id uuid, p_instrument_class_id uuid, p_security_type_id uuid, p_market_id uuid, p_currency character varying, p_settlement_type character varying, p_counterparty_entity_id uuid) IS 'ALERT-style SSI lookup. Returns the first matching SSI based on booking rule priority.';


--
-- Name: sync_counterparty_key(); Type: FUNCTION; Schema: custody; Owner: -
--

CREATE FUNCTION custody.sync_counterparty_key() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
  NEW.counterparty_key := COALESCE(NEW.counterparty_entity_id, '00000000-0000-0000-0000-000000000000'::uuid);
  RETURN NEW;
END;
$$;


--
-- Name: update_updated_at_column(); Type: FUNCTION; Schema: custody; Owner: -
--

CREATE FUNCTION custody.update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;


--
-- Name: check_case_doc_completion(uuid); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.check_case_doc_completion(p_case_id uuid) RETURNS TABLE(total_requests integer, pending_requests integer, received_requests integer, verified_requests integer, mandatory_pending integer, all_mandatory_complete boolean)
    LANGUAGE sql STABLE
    AS $$
SELECT 
    COUNT(*)::INTEGER as total_requests,
    COUNT(*) FILTER (WHERE dr.status IN ('REQUIRED', 'REQUESTED'))::INTEGER as pending_requests,
    COUNT(*) FILTER (WHERE dr.status = 'RECEIVED')::INTEGER as received_requests,
    COUNT(*) FILTER (WHERE dr.status = 'VERIFIED')::INTEGER as verified_requests,
    COUNT(*) FILTER (WHERE dr.status IN ('REQUIRED', 'REQUESTED') AND dr.is_mandatory)::INTEGER as mandatory_pending,
    NOT EXISTS (
        SELECT 1 FROM kyc.doc_requests dr2
        JOIN kyc.entity_workstreams w2 ON w2.workstream_id = dr2.workstream_id
        WHERE w2.case_id = p_case_id
        AND dr2.is_mandatory = true
        AND dr2.status NOT IN ('VERIFIED', 'WAIVED')
    ) as all_mandatory_complete
FROM kyc.doc_requests dr
JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
WHERE w.case_id = p_case_id;
$$;


--
-- Name: generate_doc_requests_from_threshold(uuid, character varying); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.generate_doc_requests_from_threshold(p_case_id uuid, p_batch_reference character varying DEFAULT NULL::character varying) RETURNS TABLE(batch_id uuid, requests_created integer, entities_processed integer)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_batch_id UUID := gen_random_uuid();
    v_batch_ref VARCHAR(50);
    v_cbu_id UUID;
    v_risk_band VARCHAR(20);
    v_requests_created INTEGER := 0;
    v_entities_processed INTEGER := 0;
    v_workstream RECORD;
    v_requirement RECORD;
    v_request_id UUID;
BEGIN
    -- Get CBU ID from case
    SELECT cbu_id INTO v_cbu_id FROM kyc.cases WHERE case_id = p_case_id;
    IF v_cbu_id IS NULL THEN
        RAISE EXCEPTION 'Case not found: %', p_case_id;
    END IF;
    
    -- Generate batch reference if not provided
    v_batch_ref := COALESCE(p_batch_reference, 
        'RFI-' || TO_CHAR(NOW(), 'YYYYMMDD') || '-' || LEFT(v_batch_id::TEXT, 8));
    
    -- Get risk band for CBU
    SELECT COALESCE(
        (SELECT risk_band FROM "ob-poc".compute_cbu_risk_score(v_cbu_id)),
        'MEDIUM'
    ) INTO v_risk_band;
    
    -- Process each workstream in the case
    FOR v_workstream IN 
        SELECT w.workstream_id, w.entity_id, e.name as entity_name,
               array_agg(DISTINCT r.name) FILTER (WHERE r.name IS NOT NULL) as roles
        FROM kyc.entity_workstreams w
        JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
        LEFT JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = w.entity_id AND cer.cbu_id = v_cbu_id
        LEFT JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        WHERE w.case_id = p_case_id
        GROUP BY w.workstream_id, w.entity_id, e.name
    LOOP
        v_entities_processed := v_entities_processed + 1;
        
        -- Get missing requirements for this entity's roles
        FOR v_requirement IN
            SELECT tr.requirement_id, tr.requirement_type, tr.entity_role,
                   tr.document_count_required, tr.is_mandatory
            FROM "ob-poc".threshold_requirements tr
            JOIN "ob-poc".risk_bands rb ON tr.risk_band_id = rb.risk_band_id
            WHERE rb.band_code = v_risk_band 
            AND tr.entity_role = ANY(v_workstream.roles)
            AND tr.is_mandatory = true
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".document_catalog dc
                JOIN "ob-poc".requirement_acceptable_docs rad ON rad.document_type_id = dc.document_type_id
                WHERE rad.requirement_id = tr.requirement_id
                AND dc.entity_id = v_workstream.entity_id
                AND dc.status = 'active'
            )
            AND NOT EXISTS (
                -- Don't create duplicate requests
                SELECT 1 FROM kyc.doc_requests dr
                WHERE dr.workstream_id = v_workstream.workstream_id
                AND dr.doc_type = tr.requirement_type
                AND dr.status NOT IN ('VERIFIED', 'WAIVED', 'REJECTED', 'EXPIRED')
            )
        LOOP
            -- Create doc_request
            INSERT INTO kyc.doc_requests (
                workstream_id, doc_type, status, is_mandatory, priority,
                batch_id, batch_reference, generation_source,
                due_date
            ) VALUES (
                v_workstream.workstream_id,
                v_requirement.requirement_type,
                'REQUIRED',
                v_requirement.is_mandatory,
                CASE WHEN v_requirement.is_mandatory THEN 'HIGH' ELSE 'NORMAL' END,
                v_batch_id,
                v_batch_ref,
                'THRESHOLD',
                CURRENT_DATE + INTERVAL '14 days'
            ) RETURNING request_id INTO v_request_id;
            
            -- Link acceptable document types
            INSERT INTO kyc.doc_request_acceptable_types (request_id, document_type_id)
            SELECT v_request_id, rad.document_type_id
            FROM "ob-poc".requirement_acceptable_docs rad
            WHERE rad.requirement_id = v_requirement.requirement_id;
            
            v_requests_created := v_requests_created + 1;
        END LOOP;
    END LOOP;
    
    RETURN QUERY SELECT v_batch_id, v_requests_created, v_entities_processed;
END;
$$;


--
-- Name: FUNCTION generate_doc_requests_from_threshold(p_case_id uuid, p_batch_reference character varying); Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON FUNCTION kyc.generate_doc_requests_from_threshold(p_case_id uuid, p_batch_reference character varying) IS 'Generates doc_requests based on threshold requirements for all workstreams in a case';


--
-- Name: is_valid_case_transition(character varying, character varying); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.is_valid_case_transition(p_from_status character varying, p_to_status character varying) RETURNS boolean
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    IF p_from_status = p_to_status THEN RETURN true; END IF;
    RETURN CASE p_from_status
        WHEN 'INTAKE' THEN p_to_status IN ('DISCOVERY', 'WITHDRAWN')
        WHEN 'DISCOVERY' THEN p_to_status IN ('ASSESSMENT', 'BLOCKED', 'WITHDRAWN')
        WHEN 'ASSESSMENT' THEN p_to_status IN ('REVIEW', 'BLOCKED', 'WITHDRAWN')
        WHEN 'REVIEW' THEN p_to_status IN ('APPROVED', 'REJECTED', 'BLOCKED', 'REFER_TO_REGULATOR', 'DO_NOT_ONBOARD')
        WHEN 'BLOCKED' THEN p_to_status IN ('DISCOVERY', 'ASSESSMENT', 'REVIEW', 'WITHDRAWN', 'DO_NOT_ONBOARD')
        WHEN 'REFER_TO_REGULATOR' THEN p_to_status IN ('REVIEW', 'DO_NOT_ONBOARD', 'APPROVED', 'REJECTED')
        ELSE false
    END;
END;
$$;


--
-- Name: is_valid_doc_request_transition(character varying, character varying); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.is_valid_doc_request_transition(p_from_status character varying, p_to_status character varying) RETURNS boolean
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    IF p_from_status = p_to_status THEN RETURN true; END IF;
    RETURN CASE p_from_status
        WHEN 'DRAFT' THEN p_to_status IN ('REQUIRED', 'REQUESTED', 'WAIVED')
        WHEN 'REQUIRED' THEN p_to_status IN ('REQUESTED', 'WAIVED', 'EXPIRED')
        WHEN 'REQUESTED' THEN p_to_status IN ('RECEIVED', 'WAIVED', 'EXPIRED')
        WHEN 'RECEIVED' THEN p_to_status IN ('UNDER_REVIEW', 'VERIFIED', 'REJECTED')
        WHEN 'UNDER_REVIEW' THEN p_to_status IN ('VERIFIED', 'REJECTED')
        WHEN 'REJECTED' THEN p_to_status IN ('REQUESTED')
        ELSE false
    END;
END;
$$;


--
-- Name: is_valid_workstream_transition(character varying, character varying); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.is_valid_workstream_transition(p_from_status character varying, p_to_status character varying) RETURNS boolean
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    IF p_from_status = p_to_status THEN RETURN true; END IF;
    RETURN CASE p_from_status
        WHEN 'PENDING' THEN p_to_status IN ('COLLECT', 'BLOCKED')
        WHEN 'COLLECT' THEN p_to_status IN ('VERIFY', 'BLOCKED')
        WHEN 'VERIFY' THEN p_to_status IN ('SCREEN', 'BLOCKED', 'ENHANCED_DD')
        WHEN 'SCREEN' THEN p_to_status IN ('ASSESS', 'BLOCKED', 'ENHANCED_DD', 'REFERRED', 'PROHIBITED')
        WHEN 'ASSESS' THEN p_to_status IN ('COMPLETE', 'BLOCKED', 'ENHANCED_DD')
        WHEN 'ENHANCED_DD' THEN p_to_status IN ('ASSESS', 'COMPLETE', 'BLOCKED', 'REFERRED', 'PROHIBITED')
        WHEN 'BLOCKED' THEN p_to_status IN ('COLLECT', 'VERIFY', 'SCREEN', 'ASSESS', 'PROHIBITED')
        WHEN 'REFERRED' THEN p_to_status IN ('SCREEN', 'ASSESS', 'COMPLETE', 'PROHIBITED')
        ELSE false
    END;
END;
$$;


--
-- Name: update_outstanding_request_timestamp(); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.update_outstanding_request_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_workstream_blocked_days(); Type: FUNCTION; Schema: kyc; Owner: -
--

CREATE FUNCTION kyc.update_workstream_blocked_days() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- When transitioning from BLOCKED to another status, calculate total blocked days
    IF OLD.status = 'BLOCKED' AND NEW.status != 'BLOCKED' THEN
        NEW.blocked_days_total = COALESCE(OLD.blocked_days_total, 0) +
            EXTRACT(DAY FROM NOW() - COALESCE(OLD.blocked_at, NOW()))::INTEGER;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: abort_hung_sessions(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".abort_hung_sessions() RETURNS integer
    LANGUAGE plpgsql
    AS $$
DECLARE
    aborted INTEGER;
BEGIN
    UPDATE "ob-poc".dsl_sessions s
    SET status = 'error',
        last_error = 'Session timed out during operation: ' || l.operation,
        last_error_at = now()
    FROM "ob-poc".dsl_session_locks l
    WHERE s.session_id = l.session_id
      AND l.lock_timeout_at < now()
      AND s.status = 'active';
    GET DIAGNOSTICS aborted = ROW_COUNT;
    DELETE FROM "ob-poc".dsl_session_locks WHERE lock_timeout_at < now();
    RETURN aborted;
END;
$$;


--
-- Name: apply_case_decision(uuid, character varying, character varying, text); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".apply_case_decision(p_case_id uuid, p_decision character varying, p_decided_by character varying, p_notes text DEFAULT NULL::text) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_current_status VARCHAR(30);
    v_latest_eval RECORD;
    v_new_status VARCHAR(30);
BEGIN
    -- Get current case status
    SELECT status INTO v_current_status
    FROM kyc.cases WHERE case_id = p_case_id;
    
    -- Get latest evaluation
    SELECT * INTO v_latest_eval
    FROM "ob-poc".case_evaluation_snapshots
    WHERE case_id = p_case_id
    ORDER BY evaluated_at DESC
    LIMIT 1;
    
    -- Validate decision against recommendation
    IF v_latest_eval.has_hard_stop AND p_decision NOT IN ('DO_NOT_ONBOARD', 'REJECT', 'REFER_TO_REGULATOR') THEN
        RAISE EXCEPTION 'Cannot approve case with unresolved hard stops. Recommended: %', v_latest_eval.recommended_action;
    END IF;
    
    -- Map decision to case status
    v_new_status := CASE p_decision
        WHEN 'APPROVE' THEN 'APPROVED'
        WHEN 'APPROVE_WITH_CONDITIONS' THEN 'APPROVED'
        WHEN 'REJECT' THEN 'REJECTED'
        WHEN 'DO_NOT_ONBOARD' THEN 'DO_NOT_ONBOARD'
        WHEN 'REFER_TO_REGULATOR' THEN 'REFER_TO_REGULATOR'
        WHEN 'ESCALATE' THEN 'REVIEW'  -- Stay in review but escalate
        ELSE v_current_status
    END;
    
    -- Update evaluation snapshot with decision
    UPDATE "ob-poc".case_evaluation_snapshots
    SET decision_made = p_decision,
        decision_made_at = now(),
        decision_made_by = p_decided_by,
        decision_notes = p_notes
    WHERE snapshot_id = v_latest_eval.snapshot_id;
    
    -- Update case status if changed
    IF v_new_status != v_current_status THEN
        UPDATE kyc.cases
        SET status = v_new_status,
            last_activity_at = now()
        WHERE case_id = p_case_id;
        
        -- If closing, set closed_at
        IF v_new_status IN ('APPROVED', 'REJECTED', 'DO_NOT_ONBOARD') THEN
            UPDATE kyc.cases
            SET closed_at = now()
            WHERE case_id = p_case_id;
        END IF;
    END IF;
    
    -- Log case event
    INSERT INTO kyc.case_events (
        case_id, event_type, event_data, actor_type, comment
    ) VALUES (
        p_case_id, 
        'DECISION_APPLIED',
        jsonb_build_object(
            'decision', p_decision,
            'previous_status', v_current_status,
            'new_status', v_new_status,
            'evaluation_snapshot_id', v_latest_eval.snapshot_id,
            'total_score', v_latest_eval.total_score,
            'has_hard_stop', v_latest_eval.has_hard_stop
        ),
        'USER',
        p_notes
    );
    
    RETURN true;
END;
$$;


--
-- Name: FUNCTION apply_case_decision(p_case_id uuid, p_decision character varying, p_decided_by character varying, p_notes text); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".apply_case_decision(p_case_id uuid, p_decision character varying, p_decided_by character varying, p_notes text) IS 'Applies decision to case with validation';


--
-- Name: can_prove_ubo(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".can_prove_ubo(p_ubo_id uuid) RETURNS TABLE(can_prove boolean, has_identity_proof boolean, has_ownership_proof boolean, missing_evidence text[], verified_evidence_count integer, pending_evidence_count integer)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_has_identity BOOLEAN;
    v_has_ownership BOOLEAN;
    v_verified_count INTEGER;
    v_pending_count INTEGER;
    v_missing TEXT[] := ARRAY[]::TEXT[];
BEGIN
    -- Check for identity proof
    SELECT EXISTS (
        SELECT 1 FROM "ob-poc".ubo_evidence
        WHERE ubo_id = p_ubo_id
          AND evidence_role = 'IDENTITY_PROOF'
          AND verification_status = 'VERIFIED'
    ) INTO v_has_identity;
    
    -- Check for ownership proof
    SELECT EXISTS (
        SELECT 1 FROM "ob-poc".ubo_evidence
        WHERE ubo_id = p_ubo_id
          AND evidence_role IN ('OWNERSHIP_PROOF', 'CHAIN_LINK')
          AND verification_status = 'VERIFIED'
    ) INTO v_has_ownership;
    
    -- Count evidence
    SELECT 
        COUNT(*) FILTER (WHERE verification_status = 'VERIFIED'),
        COUNT(*) FILTER (WHERE verification_status = 'PENDING')
    INTO v_verified_count, v_pending_count
    FROM "ob-poc".ubo_evidence
    WHERE ubo_id = p_ubo_id;
    
    -- Build missing list
    IF NOT v_has_identity THEN
        v_missing := array_append(v_missing, 'IDENTITY_PROOF');
    END IF;
    IF NOT v_has_ownership THEN
        v_missing := array_append(v_missing, 'OWNERSHIP_PROOF');
    END IF;
    
    RETURN QUERY SELECT 
        (v_has_identity AND v_has_ownership),
        v_has_identity,
        v_has_ownership,
        v_missing,
        v_verified_count,
        v_pending_count;
END;
$$;


--
-- Name: FUNCTION can_prove_ubo(p_ubo_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".can_prove_ubo(p_ubo_id uuid) IS 'Checks if UBO has sufficient evidence to be proven';


--
-- Name: capture_ubo_snapshot(uuid, uuid, character varying, character varying, character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".capture_ubo_snapshot(p_cbu_id uuid, p_case_id uuid DEFAULT NULL::uuid, p_snapshot_type character varying DEFAULT 'MANUAL'::character varying, p_reason character varying DEFAULT NULL::character varying, p_captured_by character varying DEFAULT NULL::character varying) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_snapshot_id UUID;
    v_ubos JSONB;
    v_chains JSONB;
    v_controls JSONB;
    v_completeness RECORD;
BEGIN
    -- Get current UBOs
    SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'ubo_id', ur.ubo_id,
        'subject_entity_id', ur.subject_entity_id,
        'ubo_person_id', ur.ubo_person_id,
        'relationship_type', ur.relationship_type,
        'qualifying_reason', ur.qualifying_reason,
        'ownership_percentage', ur.ownership_percentage,
        'verification_status', ur.verification_status,
        'risk_rating', ur.risk_rating
    )), '[]'::JSONB)
    INTO v_ubos
    FROM "ob-poc".ubo_registry ur
    WHERE ur.cbu_id = p_cbu_id
      AND ur.superseded_at IS NULL
      AND ur.closed_at IS NULL;
    
    -- Get ownership chains
    SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'ubo_person_id', chain.ubo_person_id,
        'ubo_name', chain.ubo_name,
        'path_entities', chain.path_entities,
        'path_names', chain.path_names,
        'ownership_percentages', chain.ownership_percentages,
        'effective_ownership', chain.effective_ownership,
        'chain_depth', chain.chain_depth
    )), '[]'::JSONB)
    INTO v_chains
    FROM "ob-poc".compute_ownership_chains(p_cbu_id) chain;
    
    -- Get control relationships
    SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'control_id', cr.control_id,
        'controller_entity_id', cr.controller_entity_id,
        'controlled_entity_id', cr.controlled_entity_id,
        'control_type', cr.control_type,
        'description', cr.description
    )), '[]'::JSONB)
    INTO v_controls
    FROM "ob-poc".control_relationships cr
    JOIN "ob-poc".cbu_entity_roles cer ON cr.controlled_entity_id = cer.entity_id
    WHERE cer.cbu_id = p_cbu_id
      AND cr.is_active = true;
    
    -- Check completeness
    SELECT * INTO v_completeness
    FROM "ob-poc".check_ubo_completeness(p_cbu_id);
    
    -- Insert snapshot
    INSERT INTO "ob-poc".ubo_snapshots (
        cbu_id, case_id, snapshot_type, snapshot_reason,
        ubos, ownership_chains, control_relationships,
        total_identified_ownership, has_gaps, gap_summary,
        captured_by
    ) VALUES (
        p_cbu_id, p_case_id, p_snapshot_type, p_reason,
        v_ubos, v_chains, v_controls,
        v_completeness.total_identified_ownership,
        NOT v_completeness.is_complete,
        CASE WHEN NOT v_completeness.is_complete 
             THEN v_completeness.issues::TEXT 
             ELSE NULL END,
        p_captured_by
    ) RETURNING snapshot_id INTO v_snapshot_id;
    
    RETURN v_snapshot_id;
END;
$$;


--
-- Name: FUNCTION capture_ubo_snapshot(p_cbu_id uuid, p_case_id uuid, p_snapshot_type character varying, p_reason character varying, p_captured_by character varying); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".capture_ubo_snapshot(p_cbu_id uuid, p_case_id uuid, p_snapshot_type character varying, p_reason character varying, p_captured_by character varying) IS 'Captures current UBO state as a snapshot';


--
-- Name: cbu_entity_roles_history_trigger(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".cbu_entity_roles_history_trigger() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO "ob-poc".cbu_entity_roles_history (
            cbu_entity_role_id, cbu_id, entity_id, role_id,
            target_entity_id, ownership_percentage,
            effective_from, effective_to, created_at, updated_at,
            operation, changed_at
        ) VALUES (
            OLD.cbu_entity_role_id, OLD.cbu_id, OLD.entity_id, OLD.role_id,
            OLD.target_entity_id, OLD.ownership_percentage,
            OLD.effective_from, OLD.effective_to, OLD.created_at, OLD.updated_at,
            'DELETE', NOW()
        );
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO "ob-poc".cbu_entity_roles_history (
            cbu_entity_role_id, cbu_id, entity_id, role_id,
            target_entity_id, ownership_percentage,
            effective_from, effective_to, created_at, updated_at,
            operation, changed_at
        ) VALUES (
            OLD.cbu_entity_role_id, OLD.cbu_id, OLD.entity_id, OLD.role_id,
            OLD.target_entity_id, OLD.ownership_percentage,
            OLD.effective_from, OLD.effective_to, OLD.created_at, OLD.updated_at,
            'UPDATE', NOW()
        );
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$;


--
-- Name: cbu_relationships_as_of(uuid, date); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".cbu_relationships_as_of(p_cbu_id uuid, p_as_of_date date DEFAULT CURRENT_DATE) RETURNS TABLE(relationship_id uuid, from_entity_id uuid, from_entity_name character varying, to_entity_id uuid, to_entity_name character varying, relationship_type character varying, percentage numeric, ownership_type character varying, control_type character varying, trust_role character varying, effective_from date, effective_to date)
    LANGUAGE plpgsql STABLE
    AS $$
BEGIN
    RETURN QUERY
    WITH cbu_entities AS (
        -- Get all entities linked to this CBU via roles
        SELECT DISTINCT cer.entity_id
        FROM "ob-poc".cbu_entity_roles cer
        WHERE cer.cbu_id = p_cbu_id
    )
    SELECT
        r.relationship_id,
        r.from_entity_id,
        e_from.name AS from_entity_name,
        r.to_entity_id,
        e_to.name AS to_entity_name,
        r.relationship_type,
        r.percentage,
        r.ownership_type,
        r.control_type,
        r.trust_role,
        r.effective_from,
        r.effective_to
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".entities e_from ON r.from_entity_id = e_from.entity_id
    JOIN "ob-poc".entities e_to ON r.to_entity_id = e_to.entity_id
    WHERE (r.from_entity_id IN (SELECT entity_id FROM cbu_entities)
           OR r.to_entity_id IN (SELECT entity_id FROM cbu_entities))
      AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
      AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date);
END;
$$;


--
-- Name: cbu_roles_as_of(uuid, date); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".cbu_roles_as_of(p_cbu_id uuid, p_as_of_date date DEFAULT CURRENT_DATE) RETURNS TABLE(entity_id uuid, entity_name character varying, entity_type character varying, role_name character varying, effective_from date, effective_to date)
    LANGUAGE plpgsql STABLE
    AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.entity_id,
        e.name AS entity_name,
        et.type_code AS entity_type,
        r.name AS role_name,
        cer.effective_from,
        cer.effective_to
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    WHERE cer.cbu_id = p_cbu_id
      AND (cer.effective_from IS NULL OR cer.effective_from <= p_as_of_date)
      AND (cer.effective_to IS NULL OR cer.effective_to > p_as_of_date);
END;
$$;


--
-- Name: cbu_state_at_approval(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".cbu_state_at_approval(p_cbu_id uuid) RETURNS TABLE(case_id uuid, approved_at timestamp with time zone, entity_id uuid, entity_name character varying, role_name character varying, ownership_from uuid, ownership_percentage numeric)
    LANGUAGE plpgsql STABLE
    AS $$
DECLARE
    v_approval_date DATE;
    v_case_id UUID;
BEGIN
    -- Find the most recent approved case
    SELECT c.case_id, c.closed_at::DATE
    INTO v_case_id, v_approval_date
    FROM kyc.cases c
    WHERE c.cbu_id = p_cbu_id
      AND c.status = 'APPROVED'
    ORDER BY c.closed_at DESC
    LIMIT 1;

    IF v_case_id IS NULL THEN
        RETURN; -- No approved case found
    END IF;

    RETURN QUERY
    SELECT
        v_case_id,
        (SELECT c.closed_at FROM kyc.cases c WHERE c.case_id = v_case_id),
        e.entity_id,
        e.name AS entity_name,
        r.name AS role_name,
        rel.from_entity_id AS ownership_from,
        rel.percentage AS ownership_percentage
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    LEFT JOIN "ob-poc".entity_relationships rel
        ON rel.to_entity_id = e.entity_id
        AND rel.relationship_type = 'ownership'
        AND (rel.effective_from IS NULL OR rel.effective_from <= v_approval_date)
        AND (rel.effective_to IS NULL OR rel.effective_to > v_approval_date)
    WHERE cer.cbu_id = p_cbu_id
      AND (cer.effective_from IS NULL OR cer.effective_from <= v_approval_date)
      AND (cer.effective_to IS NULL OR cer.effective_to > v_approval_date);
END;
$$;


--
-- Name: check_cbu_evidence_completeness(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".check_cbu_evidence_completeness(p_cbu_id uuid) RETURNS TABLE(is_complete boolean, missing_categories text[], verified_count integer, pending_count integer, rejected_count integer)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_required_categories TEXT[] := ARRAY['IDENTITY', 'OWNERSHIP', 'REGULATORY'];
    v_verified_categories TEXT[];
    v_verified_count INTEGER;
    v_pending_count INTEGER;
    v_rejected_count INTEGER;
BEGIN
    -- Count evidence by status
    SELECT 
        COUNT(*) FILTER (WHERE verification_status = 'VERIFIED'),
        COUNT(*) FILTER (WHERE verification_status = 'PENDING'),
        COUNT(*) FILTER (WHERE verification_status = 'REJECTED')
    INTO v_verified_count, v_pending_count, v_rejected_count
    FROM "ob-poc".cbu_evidence
    WHERE cbu_id = p_cbu_id;
    
    -- Get verified categories
    SELECT ARRAY_AGG(DISTINCT evidence_category)
    INTO v_verified_categories
    FROM "ob-poc".cbu_evidence
    WHERE cbu_id = p_cbu_id
      AND verification_status = 'VERIFIED'
      AND evidence_category IS NOT NULL;
    
    -- Handle NULL array
    IF v_verified_categories IS NULL THEN
        v_verified_categories := ARRAY[]::TEXT[];
    END IF;
    
    RETURN QUERY SELECT 
        v_required_categories <@ v_verified_categories,  -- All required present in verified
        ARRAY(
            SELECT unnest(v_required_categories)
            EXCEPT
            SELECT unnest(v_verified_categories)
        ),
        v_verified_count,
        v_pending_count,
        v_rejected_count;
END;
$$;


--
-- Name: FUNCTION check_cbu_evidence_completeness(p_cbu_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".check_cbu_evidence_completeness(p_cbu_id uuid) IS 'Checks if CBU has all required evidence categories verified';


--
-- Name: check_cbu_invariants(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".check_cbu_invariants() RETURNS TABLE(cbu_id uuid, cbu_name character varying, violation_type character varying, violation_detail text)
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- Check 1: commercial_client_entity_id without matching role
    RETURN QUERY
    SELECT 
        c.cbu_id,
        c.name,
        'COMMERCIAL_CLIENT_ROLE_MISSING'::VARCHAR,
        'commercial_client_entity_id set but no COMMERCIAL_CLIENT role in cbu_entity_roles'::TEXT
    FROM "ob-poc".cbus c
    WHERE c.commercial_client_entity_id IS NOT NULL
      AND NOT EXISTS (
          SELECT 1 FROM "ob-poc".cbu_entity_roles cer
          JOIN "ob-poc".roles r ON cer.role_id = r.role_id
          WHERE cer.cbu_id = c.cbu_id 
            AND cer.entity_id = c.commercial_client_entity_id
            AND r.name = 'COMMERCIAL_CLIENT'
      );
    
    -- Check 2: CBU with no cbu_category
    RETURN QUERY
    SELECT 
        c.cbu_id,
        c.name,
        'MISSING_CATEGORY'::VARCHAR,
        'cbu_category is NULL'::TEXT
    FROM "ob-poc".cbus c
    WHERE c.cbu_category IS NULL;
    
    -- Check 3: CBU with no jurisdiction
    RETURN QUERY
    SELECT 
        c.cbu_id,
        c.name,
        'MISSING_JURISDICTION'::VARCHAR,
        'jurisdiction is NULL'::TEXT
    FROM "ob-poc".cbus c
    WHERE c.jurisdiction IS NULL;
    
    -- Check 4: Active CBU with no entities (has KYC case but no entity roles)
    RETURN QUERY
    SELECT 
        c.cbu_id,
        c.name,
        'NO_ENTITIES_ASSIGNED'::VARCHAR,
        'CBU has KYC case but no entities assigned via cbu_entity_roles'::TEXT
    FROM "ob-poc".cbus c
    WHERE EXISTS (SELECT 1 FROM kyc.cases kc WHERE kc.cbu_id = c.cbu_id)
      AND NOT EXISTS (SELECT 1 FROM "ob-poc".cbu_entity_roles cer WHERE cer.cbu_id = c.cbu_id);
    
END;
$$;


--
-- Name: FUNCTION check_cbu_invariants(); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".check_cbu_invariants() IS 'Checks CBU data integrity. Run periodically or before major operations. Returns violations.';


--
-- Name: check_cbu_role_requirements(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".check_cbu_role_requirements(p_cbu_id uuid) RETURNS TABLE(requirement_type character varying, requiring_role character varying, required_role character varying, is_satisfied boolean, message text)
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN QUERY
    WITH cbu_roles AS (
        SELECT DISTINCT r.name AS role_name
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON cer.role_id = r.role_id
        WHERE cer.cbu_id = p_cbu_id
    )
    SELECT 
        rr.requirement_type,
        rr.requiring_role,
        rr.required_role,
        EXISTS (SELECT 1 FROM cbu_roles WHERE role_name = rr.required_role) AS is_satisfied,
        CASE 
            WHEN EXISTS (SELECT 1 FROM cbu_roles WHERE role_name = rr.required_role)
            THEN format('Requirement satisfied: %s present', rr.required_role)
            ELSE format('Missing required role %s for %s: %s', 
                        rr.required_role, rr.requiring_role, rr.condition_description)
        END AS message
    FROM "ob-poc".role_requirements rr
    WHERE rr.scope = 'SAME_CBU'
      AND EXISTS (SELECT 1 FROM cbu_roles WHERE role_name = rr.requiring_role);
END;
$$;


--
-- Name: FUNCTION check_cbu_role_requirements(p_cbu_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".check_cbu_role_requirements(p_cbu_id uuid) IS 'Checks if all role requirements are satisfied for a CBU (e.g., feeder needs master).';


--
-- Name: check_ubo_completeness(uuid, numeric); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".check_ubo_completeness(p_cbu_id uuid, p_threshold numeric DEFAULT 25.0) RETURNS TABLE(is_complete boolean, total_identified_ownership numeric, gap_percentage numeric, missing_chains integer, ubos_above_threshold integer, issues jsonb)
    LANGUAGE plpgsql STABLE
    AS $$
DECLARE
    v_total_ownership NUMERIC;
    v_issues JSONB := '[]'::JSONB;
    v_ubos_count INTEGER;
    v_incomplete_chains INTEGER;
BEGIN
    -- Calculate total identified ownership
    SELECT COALESCE(SUM(DISTINCT effective_ownership), 0)
    INTO v_total_ownership
    FROM "ob-poc".compute_ownership_chains(p_cbu_id);
    
    -- Count UBOs above threshold
    SELECT COUNT(DISTINCT ubo_person_id)
    INTO v_ubos_count
    FROM "ob-poc".compute_ownership_chains(p_cbu_id)
    WHERE effective_ownership >= p_threshold;
    
    -- Check for incomplete chains (entities with no further ownership but not persons)
    SELECT COUNT(*)
    INTO v_incomplete_chains
    FROM "ob-poc".entity_ownership o
    JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
    LEFT JOIN "ob-poc".entity_ownership parent ON o.owner_entity_id = parent.owned_entity_id
    JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE cer.cbu_id = p_cbu_id
      AND o.is_active = true
      AND parent.ownership_id IS NULL
      AND et.type_code != 'proper_person';
    
    -- Build issues array
    IF v_total_ownership < 100 THEN
        v_issues := v_issues || jsonb_build_object(
            'type', 'OWNERSHIP_GAP',
            'message', format('Only %.2f%% ownership identified', v_total_ownership),
            'gap', 100 - v_total_ownership
        );
    END IF;
    
    IF v_incomplete_chains > 0 THEN
        v_issues := v_issues || jsonb_build_object(
            'type', 'INCOMPLETE_CHAIN',
            'message', format('%s ownership chains end at non-person entities', v_incomplete_chains),
            'count', v_incomplete_chains
        );
    END IF;
    
    RETURN QUERY SELECT
        (v_total_ownership >= 100 AND v_incomplete_chains = 0),
        v_total_ownership,
        GREATEST(0, 100 - v_total_ownership),
        v_incomplete_chains,
        v_ubos_count,
        v_issues;
END;
$$;


--
-- Name: FUNCTION check_ubo_completeness(p_cbu_id uuid, p_threshold numeric); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".check_ubo_completeness(p_cbu_id uuid, p_threshold numeric) IS 'Validates UBO determination completeness for a CBU';


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
-- Name: cleanup_expired_sessions(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".cleanup_expired_sessions() RETURNS integer
    LANGUAGE plpgsql
    AS $$
DECLARE
    cleaned INTEGER;
BEGIN
    UPDATE "ob-poc".dsl_sessions
    SET status = 'expired'
    WHERE status = 'active' AND expires_at < now();
    GET DIAGNOSTICS cleaned = ROW_COUNT;
    RETURN cleaned;
END;
$$;


--
-- Name: compute_case_redflag_score(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".compute_case_redflag_score(p_case_id uuid) RETURNS TABLE(soft_count integer, escalate_count integer, hard_stop_count integer, soft_score integer, escalate_score integer, has_hard_stop boolean, total_score integer, open_flags integer, mitigated_flags integer, waived_flags integer)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_soft_weight INTEGER;
    v_escalate_weight INTEGER;
BEGIN
    -- Get weights from config
    SELECT weight INTO v_soft_weight FROM "ob-poc".redflag_score_config WHERE severity = 'SOFT';
    SELECT weight INTO v_escalate_weight FROM "ob-poc".redflag_score_config WHERE severity = 'ESCALATE';
    
    -- Default weights if not configured
    v_soft_weight := COALESCE(v_soft_weight, 1);
    v_escalate_weight := COALESCE(v_escalate_weight, 2);
    
    RETURN QUERY
    SELECT 
        COUNT(*) FILTER (WHERE rf.severity = 'SOFT')::INTEGER as soft_count,
        COUNT(*) FILTER (WHERE rf.severity = 'ESCALATE')::INTEGER as escalate_count,
        COUNT(*) FILTER (WHERE rf.severity = 'HARD_STOP')::INTEGER as hard_stop_count,
        (COUNT(*) FILTER (WHERE rf.severity = 'SOFT' AND rf.status = 'OPEN') * v_soft_weight)::INTEGER as soft_score,
        (COUNT(*) FILTER (WHERE rf.severity = 'ESCALATE' AND rf.status = 'OPEN') * v_escalate_weight)::INTEGER as escalate_score,
        (COUNT(*) FILTER (WHERE rf.severity = 'HARD_STOP' AND rf.status IN ('OPEN', 'BLOCKING')) > 0) as has_hard_stop,
        (
            COUNT(*) FILTER (WHERE rf.severity = 'SOFT' AND rf.status = 'OPEN') * v_soft_weight +
            COUNT(*) FILTER (WHERE rf.severity = 'ESCALATE' AND rf.status = 'OPEN') * v_escalate_weight +
            CASE WHEN COUNT(*) FILTER (WHERE rf.severity = 'HARD_STOP' AND rf.status IN ('OPEN', 'BLOCKING')) > 0 
                 THEN 1000 ELSE 0 END
        )::INTEGER as total_score,
        COUNT(*) FILTER (WHERE rf.status = 'OPEN')::INTEGER as open_flags,
        COUNT(*) FILTER (WHERE rf.status = 'MITIGATED')::INTEGER as mitigated_flags,
        COUNT(*) FILTER (WHERE rf.status = 'WAIVED')::INTEGER as waived_flags
    FROM kyc.red_flags rf
    WHERE rf.case_id = p_case_id;
END;
$$;


--
-- Name: FUNCTION compute_case_redflag_score(p_case_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".compute_case_redflag_score(p_case_id uuid) IS 'Computes aggregated red-flag scores for a case';


--
-- Name: compute_cbu_risk_score(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".compute_cbu_risk_score(target_cbu_id uuid) RETURNS TABLE(risk_score integer, risk_band character varying, factors jsonb)
    LANGUAGE plpgsql STABLE
    AS $$
DECLARE
    v_score INTEGER := 0;
    v_factors JSONB := '[]'::JSONB;
    v_cbu RECORD;
    v_factor RECORD;
    v_product_risk INTEGER;
BEGIN
    SELECT client_type, jurisdiction, nature_purpose, source_of_funds
    INTO v_cbu
    FROM "ob-poc".cbus
    WHERE cbu_id = target_cbu_id;

    IF NOT FOUND THEN
        RETURN;
    END IF;

    -- CBU type factor
    IF v_cbu.client_type IS NOT NULL THEN
        SELECT * INTO v_factor
        FROM "ob-poc".threshold_factors
        WHERE factor_type = 'CBU_TYPE' AND factor_code = v_cbu.client_type AND is_active = true;
        
        IF FOUND THEN
            v_score := v_score + v_factor.risk_weight;
            v_factors := v_factors || jsonb_build_object('type', v_factor.factor_type, 'code', v_factor.factor_code, 'weight', v_factor.risk_weight);
        END IF;
    END IF;

    -- Source of funds factor
    IF v_cbu.source_of_funds IS NOT NULL THEN
        SELECT * INTO v_factor
        FROM "ob-poc".threshold_factors
        WHERE factor_type = 'SOURCE_OF_FUNDS' AND factor_code = v_cbu.source_of_funds AND is_active = true;
        
        IF FOUND THEN
            v_score := v_score + v_factor.risk_weight;
            v_factors := v_factors || jsonb_build_object('type', v_factor.factor_type, 'code', v_factor.factor_code, 'weight', v_factor.risk_weight);
        END IF;
    END IF;

    -- Nature/purpose factor
    IF v_cbu.nature_purpose IS NOT NULL THEN
        SELECT * INTO v_factor
        FROM "ob-poc".threshold_factors
        WHERE factor_type = 'NATURE_PURPOSE' AND factor_code = v_cbu.nature_purpose AND is_active = true;
        
        IF FOUND THEN
            v_score := v_score + v_factor.risk_weight;
            v_factors := v_factors || jsonb_build_object('type', v_factor.factor_type, 'code', v_factor.factor_code, 'weight', v_factor.risk_weight);
        END IF;
    END IF;

    -- Product risk (MAX from service_delivery_map)
    SELECT COALESCE(MAX(tf.risk_weight), 0) INTO v_product_risk
    FROM "ob-poc".service_delivery_map sdm
    JOIN "ob-poc".products p ON sdm.product_id = p.product_id
    JOIN "ob-poc".threshold_factors tf ON tf.factor_code = p.product_code
    WHERE sdm.cbu_id = target_cbu_id AND tf.factor_type = 'PRODUCT_RISK' AND tf.is_active = true;

    IF v_product_risk > 0 THEN
        v_score := v_score + v_product_risk;
        v_factors := v_factors || jsonb_build_object('type', 'PRODUCT_RISK', 'code', 'MAX_PRODUCT', 'weight', v_product_risk);
    END IF;

    -- Map to risk band
    RETURN QUERY
    SELECT v_score, rb.band_code, v_factors
    FROM "ob-poc".risk_bands rb
    WHERE v_score >= rb.min_score AND v_score <= rb.max_score
    LIMIT 1;
END;
$$;


--
-- Name: compute_ownership_chains(uuid, uuid, integer, date); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".compute_ownership_chains(p_cbu_id uuid, p_target_entity_id uuid DEFAULT NULL::uuid, p_max_depth integer DEFAULT 10, p_as_of_date date DEFAULT CURRENT_DATE) RETURNS TABLE(chain_id integer, ubo_person_id uuid, ubo_name text, path_entities uuid[], path_names text[], ownership_percentages numeric[], effective_ownership numeric, chain_depth integer, is_complete boolean, relationship_types text[], has_control_path boolean)
    LANGUAGE sql STABLE
    AS $$
WITH RECURSIVE ownership_chain AS (
    -- Base case: direct relationships from entities to CBU-linked entities
    -- Now uses entity_relationships with relationship_type discriminator
    SELECT
        ROW_NUMBER() OVER ()::INTEGER as chain_id,
        base.parent_entity_id as current_entity,
        base.child_entity_id as target_entity,
        ARRAY[base.parent_entity_id] as path,
        ARRAY[base.entity_name] as names,
        ARRAY[base.ownership_pct] as percentages,
        base.ownership_pct as effective_pct,
        1 as depth,
        base.is_person as owner_is_person,
        ARRAY[base.rel_type] as rel_types,
        base.is_control as has_control
    FROM (
        -- All relationships from entity_relationships (ownership, control, trust_role)
        SELECT
            r.from_entity_id as parent_entity_id,
            r.to_entity_id as child_entity_id,
            COALESCE(e.name, 'Unknown')::text as entity_name,
            r.percentage::NUMERIC as ownership_pct,
            et.type_code = 'proper_person' as is_person,
            UPPER(r.relationship_type)::text as rel_type,
            r.relationship_type != 'ownership' as is_control
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        JOIN "ob-poc".cbu_entity_roles cer ON r.to_entity_id = cer.entity_id
        WHERE cer.cbu_id = p_cbu_id
          -- Temporal filtering using the as_of_date parameter
          AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
          AND (r.effective_to IS NULL OR r.effective_to >= p_as_of_date)
          -- Also filter cbu_entity_roles temporally
          AND (cer.effective_from IS NULL OR cer.effective_from <= p_as_of_date)
          AND (cer.effective_to IS NULL OR cer.effective_to >= p_as_of_date)
          AND (p_target_entity_id IS NULL OR r.to_entity_id = p_target_entity_id)
    ) base

    UNION ALL

    -- Recursive case: follow chain upward
    SELECT
        oc.chain_id,
        combined.parent_entity_id,
        oc.target_entity,
        oc.path || combined.parent_entity_id,
        oc.names || combined.entity_name,
        oc.percentages || combined.ownership_pct,
        CASE
            WHEN oc.effective_pct IS NOT NULL AND combined.ownership_pct IS NOT NULL
            THEN (oc.effective_pct * combined.ownership_pct / 100)::NUMERIC
            ELSE oc.effective_pct
        END,
        oc.depth + 1,
        combined.is_person,
        oc.rel_types || combined.rel_type,
        oc.has_control OR combined.is_control
    FROM ownership_chain oc
    CROSS JOIN LATERAL (
        SELECT
            r.from_entity_id as parent_entity_id,
            COALESCE(e.name, 'Unknown')::text as entity_name,
            r.percentage::NUMERIC as ownership_pct,
            et.type_code = 'proper_person' as is_person,
            UPPER(r.relationship_type)::text as rel_type,
            r.relationship_type != 'ownership' as is_control
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE r.to_entity_id = oc.current_entity
          -- Temporal filtering for recursive steps
          AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
          AND (r.effective_to IS NULL OR r.effective_to >= p_as_of_date)
          AND NOT (r.from_entity_id = ANY(oc.path))  -- Prevent cycles
    ) combined
    WHERE oc.depth < p_max_depth
      AND NOT oc.owner_is_person  -- Stop when we hit a person
)
SELECT
    oc.chain_id,
    oc.current_entity as ubo_person_id,
    oc.names[array_length(oc.names, 1)] as ubo_name,
    oc.path as path_entities,
    oc.names as path_names,
    oc.percentages as ownership_percentages,
    oc.effective_pct as effective_ownership,
    oc.depth as chain_depth,
    oc.owner_is_person as is_complete,
    oc.rel_types as relationship_types,
    oc.has_control as has_control_path
FROM ownership_chain oc
WHERE oc.owner_is_person  -- Only return complete chains ending at persons
   OR oc.depth = p_max_depth  -- Or chains that hit max depth
ORDER BY oc.effective_pct DESC NULLS LAST, oc.chain_id;
$$;


--
-- Name: FUNCTION compute_ownership_chains(p_cbu_id uuid, p_target_entity_id uuid, p_max_depth integer, p_as_of_date date); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".compute_ownership_chains(p_cbu_id uuid, p_target_entity_id uuid, p_max_depth integer, p_as_of_date date) IS 'Computes ownership and control chains from CBU entities to natural persons.
Supports point-in-time queries via p_as_of_date parameter (defaults to today).
Returns chains with effective ownership percentages and relationship types.';


--
-- Name: entity_relationships_history_trigger(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".entity_relationships_history_trigger() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO "ob-poc".entity_relationships_history (
            relationship_id, from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, control_type, trust_role, interest_type,
            effective_from, effective_to, source, source_document_ref, notes,
            created_at, created_by, updated_at,
            trust_interest_type, trust_class_description, is_regulated, regulatory_jurisdiction,
            operation, changed_at
        ) VALUES (
            OLD.relationship_id, OLD.from_entity_id, OLD.to_entity_id, OLD.relationship_type,
            OLD.percentage, OLD.ownership_type, OLD.control_type, OLD.trust_role, OLD.interest_type,
            OLD.effective_from, OLD.effective_to, OLD.source, OLD.source_document_ref, OLD.notes,
            OLD.created_at, OLD.created_by, OLD.updated_at,
            OLD.trust_interest_type, OLD.trust_class_description, OLD.is_regulated, OLD.regulatory_jurisdiction,
            'DELETE', NOW()
        );
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO "ob-poc".entity_relationships_history (
            relationship_id, from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, control_type, trust_role, interest_type,
            effective_from, effective_to, source, source_document_ref, notes,
            created_at, created_by, updated_at,
            trust_interest_type, trust_class_description, is_regulated, regulatory_jurisdiction,
            operation, changed_at, superseded_by
        ) VALUES (
            OLD.relationship_id, OLD.from_entity_id, OLD.to_entity_id, OLD.relationship_type,
            OLD.percentage, OLD.ownership_type, OLD.control_type, OLD.trust_role, OLD.interest_type,
            OLD.effective_from, OLD.effective_to, OLD.source, OLD.source_document_ref, OLD.notes,
            OLD.created_at, OLD.created_by, OLD.updated_at,
            OLD.trust_interest_type, OLD.trust_class_description, OLD.is_regulated, OLD.regulatory_jurisdiction,
            'UPDATE', NOW(), NEW.relationship_id
        );
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$;


--
-- Name: evaluate_case_decision(uuid, character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".evaluate_case_decision(p_case_id uuid, p_evaluator character varying DEFAULT NULL::character varying) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_scores RECORD;
    v_threshold RECORD;
    v_snapshot_id UUID;
BEGIN
    -- Get current scores
    SELECT * INTO v_scores
    FROM "ob-poc".compute_case_redflag_score(p_case_id);
    
    -- Find matching threshold (priority: hard_stop > escalate > score-based)
    IF v_scores.has_hard_stop THEN
        SELECT * INTO v_threshold
        FROM "ob-poc".case_decision_thresholds
        WHERE has_hard_stop = true AND is_active = true
        LIMIT 1;
    ELSIF v_scores.escalate_count > 0 THEN
        SELECT * INTO v_threshold
        FROM "ob-poc".case_decision_thresholds
        WHERE threshold_name = 'escalate_flags' AND is_active = true
        LIMIT 1;
    ELSE
        SELECT * INTO v_threshold
        FROM "ob-poc".case_decision_thresholds
        WHERE is_active = true
          AND has_hard_stop = false
          AND (min_score IS NULL OR v_scores.total_score >= min_score)
          AND (max_score IS NULL OR v_scores.total_score <= max_score)
        ORDER BY COALESCE(min_score, 0) DESC
        LIMIT 1;
    END IF;
    
    -- Create evaluation snapshot
    INSERT INTO "ob-poc".case_evaluation_snapshots (
        case_id,
        soft_count, escalate_count, hard_stop_count,
        soft_score, escalate_score, has_hard_stop, total_score,
        open_flags, mitigated_flags, waived_flags,
        matched_threshold_id, recommended_action, required_escalation_level,
        evaluated_by
    ) VALUES (
        p_case_id,
        v_scores.soft_count, v_scores.escalate_count, v_scores.hard_stop_count,
        v_scores.soft_score, v_scores.escalate_score, v_scores.has_hard_stop, v_scores.total_score,
        v_scores.open_flags, v_scores.mitigated_flags, v_scores.waived_flags,
        v_threshold.threshold_id, v_threshold.recommended_action, v_threshold.escalation_level,
        p_evaluator
    ) RETURNING snapshot_id INTO v_snapshot_id;
    
    RETURN v_snapshot_id;
END;
$$;


--
-- Name: FUNCTION evaluate_case_decision(p_case_id uuid, p_evaluator character varying); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".evaluate_case_decision(p_case_id uuid, p_evaluator character varying) IS 'Evaluates case and creates recommendation snapshot';


--
-- Name: find_executions_by_verb_hash(bytea); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".find_executions_by_verb_hash(p_verb_hash bytea) RETURNS TABLE(execution_id uuid, cbu_id character varying, status character varying, started_at timestamp with time zone, verb_names text[])
    LANGUAGE sql STABLE
    AS $$
    SELECT
        execution_id,
        cbu_id,
        status,
        started_at,
        verb_names
    FROM "ob-poc".dsl_execution_log
    WHERE p_verb_hash = ANY(verb_hashes)
    ORDER BY started_at DESC;
$$;


--
-- Name: FUNCTION find_executions_by_verb_hash(p_verb_hash bytea); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".find_executions_by_verb_hash(p_verb_hash bytea) IS 'Find all executions that used a specific verb configuration (by compiled_hash)';


--
-- Name: find_idempotency_by_verb_hash(bytea, integer); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".find_idempotency_by_verb_hash(p_verb_hash bytea, p_limit integer DEFAULT 100) RETURNS TABLE(idempotency_key text, execution_id uuid, verb text, result_type text, created_at timestamp with time zone)
    LANGUAGE sql STABLE
    AS $$
    SELECT
        idempotency_key,
        execution_id,
        verb,
        result_type,
        created_at
    FROM "ob-poc".dsl_idempotency
    WHERE verb_hash = p_verb_hash
    ORDER BY created_at DESC
    LIMIT p_limit;
$$;


--
-- Name: FUNCTION find_idempotency_by_verb_hash(p_verb_hash bytea, p_limit integer); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".find_idempotency_by_verb_hash(p_verb_hash bytea, p_limit integer) IS 'Find idempotency records that used a specific verb configuration (by compiled_hash)';


--
-- Name: find_phonetic_matches(text[], integer); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".find_phonetic_matches(query_phonetic_codes text[], top_k integer DEFAULT 5) RETURNS TABLE(verb_name character varying, pattern_phrase text, category character varying, is_agent_bound boolean, priority integer, matching_codes text[])
    LANGUAGE sql STABLE
    AS $$
    SELECT 
        vpe.verb_name,
        vpe.pattern_phrase,
        vpe.category,
        vpe.is_agent_bound,
        vpe.priority,
        ARRAY(SELECT unnest(vpe.phonetic_codes) INTERSECT SELECT unnest(query_phonetic_codes)) AS matching_codes
    FROM "ob-poc".verb_pattern_embeddings vpe
    WHERE vpe.phonetic_codes && query_phonetic_codes
    ORDER BY 
        array_length(ARRAY(SELECT unnest(vpe.phonetic_codes) INTERSECT SELECT unnest(query_phonetic_codes)), 1) DESC NULLS LAST,
        vpe.priority
    LIMIT top_k;
$$;


--
-- Name: find_similar_patterns(public.vector, integer, real, character varying, boolean); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".find_similar_patterns(query_embedding public.vector, top_k integer DEFAULT 5, min_similarity real DEFAULT 0.5, category_filter character varying DEFAULT NULL::character varying, agent_bound_filter boolean DEFAULT NULL::boolean) RETURNS TABLE(verb_name character varying, pattern_phrase text, similarity real, category character varying, is_agent_bound boolean, priority integer)
    LANGUAGE sql STABLE
    AS $$
    SELECT
        vpe.verb_name,
        vpe.pattern_phrase,
        1 - (vpe.embedding <=> query_embedding) AS similarity,
        vpe.category,
        vpe.is_agent_bound,
        vpe.priority
    FROM "ob-poc".verb_pattern_embeddings vpe
    WHERE
        (category_filter IS NULL OR vpe.category = category_filter)
        AND (agent_bound_filter IS NULL OR vpe.is_agent_bound = agent_bound_filter)
        AND 1 - (vpe.embedding <=> query_embedding) >= min_similarity
    ORDER BY
        vpe.embedding <=> query_embedding,
        vpe.priority
    LIMIT top_k;
$$;


--
-- Name: FUNCTION find_similar_patterns(query_embedding public.vector, top_k integer, min_similarity real, category_filter character varying, agent_bound_filter boolean); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".find_similar_patterns(query_embedding public.vector, top_k integer, min_similarity real, category_filter character varying, agent_bound_filter boolean) IS 'Find top-k verb patterns most semantically similar to query embedding. Uses cosine similarity.';


--
-- Name: fn_auto_create_product_overlay(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".fn_auto_create_product_overlay() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- Create a global overlay (NULL context = applies to all matrix entries)
    INSERT INTO "ob-poc".cbu_matrix_product_overlay (
        cbu_id,
        subscription_id,
        instrument_class_id,
        market_id,
        currency,
        counterparty_entity_id,
        status
    ) VALUES (
        NEW.cbu_id,
        NEW.subscription_id,
        NULL,  -- applies to all instruments
        NULL,  -- applies to all markets
        NULL,  -- applies to all currencies
        NULL,  -- applies to all counterparties
        'ACTIVE'
    )
    ON CONFLICT DO NOTHING;

    RETURN NEW;
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
-- Name: get_layout_config(character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".get_layout_config(p_key character varying) RETURNS jsonb
    LANGUAGE sql STABLE
    AS $$
    SELECT config_value FROM "ob-poc".layout_config WHERE config_key = p_key;
$$;


--
-- Name: FUNCTION get_layout_config(p_key character varying); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".get_layout_config(p_key character varying) IS 'Get a layout configuration value by key. Returns JSONB.';


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
-- Name: get_verb_config_at_execution(uuid, text); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".get_verb_config_at_execution(p_execution_id uuid, p_verb_name text) RETURNS TABLE(verb_name text, compiled_hash bytea, compiled_json jsonb, effective_config_json jsonb, diagnostics_json jsonb)
    LANGUAGE sql STABLE
    AS $$
    WITH execution_hash AS (
        SELECT verb_hashes[idx] as hash
        FROM "ob-poc".dsl_execution_log el,
             generate_subscripts(el.verb_names, 1) as idx
        WHERE el.execution_id = p_execution_id
          AND el.verb_names[idx] = p_verb_name
    )
    SELECT
        v.verb_name,
        v.compiled_hash,
        v.compiled_json,
        v.effective_config_json,
        v.diagnostics_json
    FROM "ob-poc".dsl_verbs v
    WHERE v.compiled_hash = (SELECT hash FROM execution_hash LIMIT 1);
$$;


--
-- Name: FUNCTION get_verb_config_at_execution(p_execution_id uuid, p_verb_name text); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".get_verb_config_at_execution(p_execution_id uuid, p_verb_name text) IS 'Get the exact verb configuration that was active during a specific execution';


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
-- Name: invalidate_layout_cache(uuid, character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".invalidate_layout_cache(p_cbu_id uuid, p_view_mode character varying DEFAULT NULL::character varying) RETURNS integer
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_count INTEGER;
BEGIN
    IF p_view_mode IS NULL THEN
        DELETE FROM "ob-poc".layout_cache WHERE cbu_id = p_cbu_id;
    ELSE
        DELETE FROM "ob-poc".layout_cache
        WHERE cbu_id = p_cbu_id AND view_mode = p_view_mode;
    END IF;
    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$;


--
-- Name: FUNCTION invalidate_layout_cache(p_cbu_id uuid, p_view_mode character varying); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".invalidate_layout_cache(p_cbu_id uuid, p_view_mode character varying) IS 'Invalidate layout cache for a CBU. Pass view_mode to invalidate only that view, or NULL for all views.';


--
-- Name: is_natural_person(uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".is_natural_person(entity_id uuid) RETURNS boolean
    LANGUAGE plpgsql STABLE
    AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE e.entity_id = is_natural_person.entity_id
          AND et.entity_category = 'PERSON'
    );
END;
$$;


--
-- Name: FUNCTION is_natural_person(entity_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".is_natural_person(entity_id uuid) IS 'Returns true if entity is a natural person (PERSON category)';


--
-- Name: is_valid_cbu_transition(character varying, character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".is_valid_cbu_transition(p_from_status character varying, p_to_status character varying) RETURNS boolean
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    -- Same status is always valid (no-op)
    IF p_from_status = p_to_status THEN
        RETURN true;
    END IF;
    
    RETURN CASE p_from_status
        WHEN 'DISCOVERED' THEN 
            p_to_status IN ('VALIDATION_PENDING', 'VALIDATION_FAILED')
        WHEN 'VALIDATION_PENDING' THEN 
            p_to_status IN ('VALIDATED', 'VALIDATION_FAILED', 'DISCOVERED')
        WHEN 'VALIDATED' THEN 
            p_to_status IN ('UPDATE_PENDING_PROOF')  -- Material change triggers re-validation
        WHEN 'UPDATE_PENDING_PROOF' THEN 
            p_to_status IN ('VALIDATED', 'VALIDATION_FAILED')
        WHEN 'VALIDATION_FAILED' THEN 
            p_to_status IN ('VALIDATION_PENDING', 'DISCOVERED')  -- Retry or start over
        ELSE 
            false
    END;
END;
$$;


--
-- Name: FUNCTION is_valid_cbu_transition(p_from_status character varying, p_to_status character varying); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".is_valid_cbu_transition(p_from_status character varying, p_to_status character varying) IS 'Validates CBU status transitions';


--
-- Name: is_valid_ubo_transition(character varying, character varying); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".is_valid_ubo_transition(p_from_status character varying, p_to_status character varying) RETURNS boolean
    LANGUAGE plpgsql IMMUTABLE
    AS $$
BEGIN
    -- Same status is always valid (no-op)
    IF p_from_status = p_to_status THEN
        RETURN true;
    END IF;
    
    -- Handle NULL (new record) - can start as SUSPECTED or PENDING
    IF p_from_status IS NULL THEN
        RETURN p_to_status IN ('SUSPECTED', 'PENDING');
    END IF;
    
    RETURN CASE p_from_status
        WHEN 'SUSPECTED' THEN 
            p_to_status IN ('PROVEN', 'PENDING', 'FAILED', 'REMOVED')
        WHEN 'PENDING' THEN 
            p_to_status IN ('PROVEN', 'VERIFIED', 'FAILED', 'DISPUTED', 'REMOVED')
        WHEN 'PROVEN' THEN 
            p_to_status IN ('VERIFIED', 'DISPUTED', 'REMOVED')
        WHEN 'VERIFIED' THEN 
            p_to_status IN ('DISPUTED', 'REMOVED')  -- Can be challenged or ownership changes
        WHEN 'FAILED' THEN 
            p_to_status IN ('SUSPECTED', 'PENDING')  -- Retry
        WHEN 'DISPUTED' THEN 
            p_to_status IN ('PROVEN', 'VERIFIED', 'REMOVED', 'FAILED')  -- Resolution
        WHEN 'REMOVED' THEN 
            false  -- Terminal state
        ELSE 
            false
    END;
END;
$$;


--
-- Name: FUNCTION is_valid_ubo_transition(p_from_status character varying, p_to_status character varying); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".is_valid_ubo_transition(p_from_status character varying, p_to_status character varying) IS 'Validates UBO verification status transitions';


--
-- Name: log_cbu_status_change(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".log_cbu_status_change() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status IS DISTINCT FROM NEW.status THEN
        INSERT INTO "ob-poc".cbu_change_log (
            cbu_id, change_type, field_name, old_value, new_value, changed_at
        ) VALUES (
            NEW.cbu_id, 
            'STATUS_CHANGE', 
            'status',
            to_jsonb(OLD.status),
            to_jsonb(NEW.status),
            now()
        );
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: migrate_trading_profile_to_ast(uuid, jsonb, uuid, text, integer); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".migrate_trading_profile_to_ast(p_profile_id uuid, p_old_document jsonb, p_cbu_id uuid, p_cbu_name text, p_version integer) RETURNS jsonb
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_new_doc JSONB;
    v_children JSONB := '[]'::jsonb;
    v_universe_category JSONB;
    v_ssi_category JSONB;
    v_isda_category JSONB;
    v_universe_children JSONB := '[]'::jsonb;
    v_ssi_children JSONB := '[]'::jsonb;
    v_isda_children JSONB := '[]'::jsonb;
    v_instrument_class RECORD;
    v_market RECORD;
    v_ssi RECORD;
    v_booking_rule RECORD;
    v_isda RECORD;
    v_node JSONB;
    v_market_node JSONB;
    v_ssi_node JSONB;
    v_rule_node JSONB;
    v_class_code TEXT;
    v_class_children JSONB;
    v_market_children JSONB;
    v_mic TEXT;
BEGIN
    -- ==========================================================================
    -- Build Trading Universe Category
    -- ==========================================================================

    -- Process instrument classes
    IF p_old_document->'universe'->'instrument_classes' IS NOT NULL THEN
        FOR v_instrument_class IN
            SELECT * FROM jsonb_array_elements(p_old_document->'universe'->'instrument_classes')
        LOOP
            v_class_code := v_instrument_class.value->>'class_code';
            v_class_children := '[]'::jsonb;

            -- Find markets for this instrument class
            IF p_old_document->'universe'->'allowed_markets' IS NOT NULL THEN
                FOR v_market IN
                    SELECT * FROM jsonb_array_elements(p_old_document->'universe'->'allowed_markets')
                LOOP
                    v_mic := v_market.value->>'mic';
                    v_market_children := '[]'::jsonb;

                    -- Create universe entry node for this market
                    v_node := jsonb_build_object(
                        'id', jsonb_build_array('_UNIVERSE', v_class_code, v_mic, gen_random_uuid()::text),
                        'node_type', jsonb_build_object(
                            'type', 'universe_entry',
                            'universe_id', gen_random_uuid()::text,
                            'currencies', COALESCE(v_market.value->'currencies', '["USD"]'::jsonb),
                            'settlement_types', COALESCE(v_market.value->'settlement_types', '["DVP"]'::jsonb),
                            'is_held', COALESCE((v_instrument_class.value->>'is_held')::boolean, true),
                            'is_traded', COALESCE((v_instrument_class.value->>'is_traded')::boolean, true)
                        ),
                        'label', 'Universe Entry',
                        'sublabel', array_to_string(ARRAY(SELECT jsonb_array_elements_text(COALESCE(v_market.value->'currencies', '["USD"]'::jsonb))), ', '),
                        'children', '[]'::jsonb,
                        'status_color', 'green',
                        'is_loaded', true,
                        'leaf_count', 1
                    );
                    v_market_children := v_market_children || v_node;

                    -- Create market node
                    v_market_node := jsonb_build_object(
                        'id', jsonb_build_array('_UNIVERSE', v_class_code, v_mic),
                        'node_type', jsonb_build_object(
                            'type', 'market',
                            'mic', v_mic,
                            'market_name', v_mic,
                            'country_code', CASE
                                WHEN v_mic LIKE 'X%' THEN
                                    CASE substring(v_mic from 2 for 2)
                                        WHEN 'NY' THEN 'US'
                                        WHEN 'NA' THEN 'US'
                                        WHEN 'LO' THEN 'GB'
                                        WHEN 'ET' THEN 'DE'
                                        WHEN 'PA' THEN 'FR'
                                        WHEN 'SW' THEN 'CH'
                                        WHEN 'HK' THEN 'HK'
                                        WHEN 'TK' THEN 'JP'
                                        ELSE 'XX'
                                    END
                                ELSE 'XX'
                            END
                        ),
                        'label', v_mic,
                        'children', v_market_children,
                        'status_color', 'green',
                        'is_loaded', true,
                        'leaf_count', jsonb_array_length(v_market_children)
                    );
                    v_class_children := v_class_children || v_market_node;
                END LOOP;
            END IF;

            -- Create instrument class node
            v_node := jsonb_build_object(
                'id', jsonb_build_array('_UNIVERSE', v_class_code),
                'node_type', jsonb_build_object(
                    'type', 'instrument_class',
                    'class_code', v_class_code,
                    'cfi_prefix', NULL,
                    'is_otc', v_class_code LIKE 'OTC%'
                ),
                'label', v_class_code,
                'children', v_class_children,
                'status_color', 'green',
                'is_loaded', true,
                'leaf_count', GREATEST(jsonb_array_length(v_class_children), 1)
            );
            v_universe_children := v_universe_children || v_node;
        END LOOP;
    END IF;

    -- Create Trading Universe category
    v_universe_category := jsonb_build_object(
        'id', jsonb_build_array('_UNIVERSE'),
        'node_type', jsonb_build_object('type', 'category', 'name', 'Trading Universe'),
        'label', 'Trading Universe',
        'children', v_universe_children,
        'status_color', CASE WHEN jsonb_array_length(v_universe_children) > 0 THEN 'green' ELSE 'gray' END,
        'is_loaded', true,
        'leaf_count', COALESCE((SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_universe_children) c), 0)
    );
    v_children := v_children || v_universe_category;

    -- ==========================================================================
    -- Build Settlement Instructions Category
    -- ==========================================================================

    -- Process SSIs from standing_instructions.SECURITIES
    IF p_old_document->'standing_instructions'->'SECURITIES' IS NOT NULL THEN
        FOR v_ssi IN
            SELECT * FROM jsonb_array_elements(p_old_document->'standing_instructions'->'SECURITIES')
        LOOP
            v_market_children := '[]'::jsonb;

            -- Find booking rules that reference this SSI
            IF p_old_document->'booking_rules' IS NOT NULL THEN
                FOR v_booking_rule IN
                    SELECT * FROM jsonb_array_elements(p_old_document->'booking_rules')
                    WHERE (value->>'ssi_ref') = (v_ssi.value->>'name')
                LOOP
                    v_rule_node := jsonb_build_object(
                        'id', jsonb_build_array('_SSI', v_ssi.value->>'name', 'rule_' || (v_booking_rule.value->>'name')),
                        'node_type', jsonb_build_object(
                            'type', 'booking_rule',
                            'rule_id', gen_random_uuid()::text,
                            'rule_name', v_booking_rule.value->>'name',
                            'priority', COALESCE((v_booking_rule.value->>'priority')::int, 50),
                            'specificity_score', CASE
                                WHEN v_booking_rule.value->'match'->>'instrument_class' IS NOT NULL THEN 1 ELSE 0
                            END + CASE
                                WHEN v_booking_rule.value->'match'->>'mic' IS NOT NULL THEN 1 ELSE 0
                            END + CASE
                                WHEN v_booking_rule.value->'match'->>'currency' IS NOT NULL THEN 1 ELSE 0
                            END + CASE
                                WHEN v_booking_rule.value->'match'->>'settlement_type' IS NOT NULL THEN 1 ELSE 0
                            END,
                            'is_active', true,
                            'match_criteria', jsonb_build_object(
                                'instrument_class', v_booking_rule.value->'match'->>'instrument_class',
                                'mic', v_booking_rule.value->'match'->>'mic',
                                'currency', v_booking_rule.value->'match'->>'currency',
                                'settlement_type', v_booking_rule.value->'match'->>'settlement_type',
                                'security_type', v_booking_rule.value->'match'->>'security_type',
                                'counterparty_entity_id', v_booking_rule.value->'match'->>'counterparty'
                            )
                        ),
                        'label', v_booking_rule.value->>'name',
                        'sublabel', 'Priority ' || COALESCE(v_booking_rule.value->>'priority', '50'),
                        'children', '[]'::jsonb,
                        'status_color', 'green',
                        'is_loaded', true,
                        'leaf_count', 1
                    );
                    v_market_children := v_market_children || v_rule_node;
                END LOOP;
            END IF;

            -- Create SSI node
            v_ssi_node := jsonb_build_object(
                'id', jsonb_build_array('_SSI', v_ssi.value->>'name'),
                'node_type', jsonb_build_object(
                    'type', 'ssi',
                    'ssi_id', gen_random_uuid()::text,
                    'ssi_name', v_ssi.value->>'name',
                    'ssi_type', 'SECURITIES',
                    'status', 'ACTIVE',
                    'safekeeping_account', v_ssi.value->>'custody_account',
                    'safekeeping_bic', v_ssi.value->>'custody_bic',
                    'cash_account', v_ssi.value->>'cash_account',
                    'cash_bic', v_ssi.value->>'cash_bic',
                    'pset_bic', NULL,
                    'cash_currency', v_ssi.value->>'currency'
                ),
                'label', v_ssi.value->>'name',
                'sublabel', COALESCE(v_ssi.value->>'custody_bic', '') || ' / ' || COALESCE(v_ssi.value->>'currency', ''),
                'children', v_market_children,
                'status_color', 'green',
                'is_loaded', true,
                'leaf_count', GREATEST(jsonb_array_length(v_market_children), 1)
            );
            v_ssi_children := v_ssi_children || v_ssi_node;
        END LOOP;
    END IF;

    -- Create Settlement Instructions category
    v_ssi_category := jsonb_build_object(
        'id', jsonb_build_array('_SSI'),
        'node_type', jsonb_build_object('type', 'category', 'name', 'Settlement Instructions'),
        'label', 'Settlement Instructions',
        'children', v_ssi_children,
        'status_color', CASE WHEN jsonb_array_length(v_ssi_children) > 0 THEN 'green' ELSE 'gray' END,
        'is_loaded', true,
        'leaf_count', COALESCE((SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_ssi_children) c), 0)
    );
    v_children := v_children || v_ssi_category;

    -- ==========================================================================
    -- Build ISDA Agreements Category
    -- ==========================================================================

    IF p_old_document->'isda_agreements' IS NOT NULL AND
       jsonb_array_length(p_old_document->'isda_agreements') > 0 THEN
        FOR v_isda IN
            SELECT * FROM jsonb_array_elements(p_old_document->'isda_agreements')
        LOOP
            v_node := jsonb_build_object(
                'id', jsonb_build_array('_ISDA', COALESCE(v_isda.value->>'counterparty_name', 'Unknown')),
                'node_type', jsonb_build_object(
                    'type', 'isda_agreement',
                    'isda_id', gen_random_uuid()::text,
                    'counterparty_name', COALESCE(v_isda.value->>'counterparty_name', v_isda.value->'counterparty'->>'name'),
                    'governing_law', v_isda.value->>'governing_law',
                    'agreement_date', v_isda.value->>'agreement_date',
                    'counterparty_entity_id', v_isda.value->'counterparty'->>'entity_id',
                    'counterparty_lei', v_isda.value->'counterparty'->>'lei'
                ),
                'label', COALESCE(v_isda.value->>'counterparty_name', v_isda.value->'counterparty'->>'name', 'Unknown'),
                'sublabel', COALESCE(v_isda.value->>'governing_law', 'NY') || ' Law',
                'children', '[]'::jsonb,  -- CSAs would be children here
                'status_color', 'green',
                'is_loaded', true,
                'leaf_count', 1
            );
            v_isda_children := v_isda_children || v_node;
        END LOOP;
    END IF;

    -- Create ISDA Agreements category
    v_isda_category := jsonb_build_object(
        'id', jsonb_build_array('_ISDA'),
        'node_type', jsonb_build_object('type', 'category', 'name', 'ISDA Agreements'),
        'label', 'ISDA Agreements',
        'children', v_isda_children,
        'status_color', CASE WHEN jsonb_array_length(v_isda_children) > 0 THEN 'green' ELSE 'gray' END,
        'is_loaded', true,
        'leaf_count', COALESCE((SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_isda_children) c), 0)
    );
    v_children := v_children || v_isda_category;

    -- ==========================================================================
    -- Build final document
    -- ==========================================================================

    v_new_doc := jsonb_build_object(
        'cbu_id', p_cbu_id::text,
        'cbu_name', p_cbu_name,
        'version', p_version,
        'status', 'DRAFT',
        'children', v_children,
        'total_leaf_count', COALESCE(
            (SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_children) c),
            0
        ),
        'metadata', jsonb_build_object(
            'source', 'migration',
            'source_ref', 'migration_20260106',
            'notes', 'Migrated from flat structure to AST format',
            'regulatory_framework', p_old_document->'metadata'->>'regulatory_framework'
        ),
        'created_at', to_char(now(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
        'updated_at', to_char(now(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
    );

    RETURN v_new_doc;
END;
$$;


--
-- Name: needs_ast_migration(jsonb); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".needs_ast_migration(p_document jsonb) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- If document has 'universe' as an object (not in children), it's old format
    IF p_document ? 'universe' AND
       jsonb_typeof(p_document->'universe') = 'object' AND
       NOT (p_document ? 'children') THEN
        RETURN TRUE;
    END IF;

    -- If document has 'standing_instructions' at top level, it's old format
    IF p_document ? 'standing_instructions' AND
       NOT (p_document ? 'children') THEN
        RETURN TRUE;
    END IF;

    RETURN FALSE;
END;
$$;


--
-- Name: ownership_as_of(uuid, date); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".ownership_as_of(p_entity_id uuid, p_as_of_date date DEFAULT CURRENT_DATE) RETURNS TABLE(relationship_id uuid, from_entity_id uuid, from_entity_name character varying, to_entity_id uuid, to_entity_name character varying, percentage numeric, ownership_type character varying, effective_from date, effective_to date)
    LANGUAGE plpgsql STABLE
    AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.relationship_id,
        r.from_entity_id,
        e_from.name AS from_entity_name,
        r.to_entity_id,
        e_to.name AS to_entity_name,
        r.percentage,
        r.ownership_type,
        r.effective_from,
        r.effective_to
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".entities e_from ON r.from_entity_id = e_from.entity_id
    JOIN "ob-poc".entities e_to ON r.to_entity_id = e_to.entity_id
    WHERE r.relationship_type = 'ownership'
      AND (r.from_entity_id = p_entity_id OR r.to_entity_id = p_entity_id)
      AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
      AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date);
END;
$$;


--
-- Name: record_execution_with_view_state(text, uuid, integer, character varying, character varying, character varying, uuid, jsonb, bigint, bytea, character varying, uuid, uuid, character varying, uuid, jsonb, uuid[], jsonb, integer, jsonb); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".record_execution_with_view_state(p_idempotency_key text, p_execution_id uuid, p_statement_index integer, p_verb character varying, p_args_hash character varying, p_result_type character varying, p_result_id uuid DEFAULT NULL::uuid, p_result_json jsonb DEFAULT NULL::jsonb, p_result_affected bigint DEFAULT NULL::bigint, p_verb_hash bytea DEFAULT NULL::bytea, p_source character varying DEFAULT 'unknown'::character varying, p_request_id uuid DEFAULT NULL::uuid, p_actor_id uuid DEFAULT NULL::uuid, p_actor_type character varying DEFAULT 'user'::character varying, p_session_id uuid DEFAULT NULL::uuid, p_view_taxonomy_context jsonb DEFAULT NULL::jsonb, p_view_selection uuid[] DEFAULT NULL::uuid[], p_view_refinements jsonb DEFAULT NULL::jsonb, p_view_stack_depth integer DEFAULT NULL::integer, p_view_state_snapshot jsonb DEFAULT NULL::jsonb) RETURNS TABLE(idempotency_key text, view_state_change_id uuid, was_cached boolean)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_existing_key TEXT;
    v_change_id UUID;
BEGIN
    -- Check if already executed (idempotency check)
    SELECT i.idempotency_key INTO v_existing_key
    FROM "ob-poc".dsl_idempotency i
    WHERE i.idempotency_key = p_idempotency_key;

    IF v_existing_key IS NOT NULL THEN
        -- Already executed - return cached indicator
        RETURN QUERY SELECT v_existing_key, NULL::UUID, TRUE;
        RETURN;
    END IF;

    -- Record idempotency entry with source attribution
    INSERT INTO "ob-poc".dsl_idempotency (
        idempotency_key,
        execution_id,
        statement_index,
        verb,
        args_hash,
        result_type,
        result_id,
        result_json,
        result_affected,
        verb_hash,
        source,
        request_id,
        actor_id,
        actor_type
    ) VALUES (
        p_idempotency_key,
        p_execution_id,
        p_statement_index,
        p_verb,
        p_args_hash,
        p_result_type,
        p_result_id,
        p_result_json,
        p_result_affected,
        p_verb_hash,
        p_source,
        p_request_id,
        p_actor_id,
        p_actor_type
    );

    -- If view state provided, record it atomically
    IF p_view_state_snapshot IS NOT NULL THEN
        INSERT INTO "ob-poc".dsl_view_state_changes (
            idempotency_key,
            session_id,
            verb_name,
            taxonomy_context,
            selection,
            refinements,
            stack_depth,
            view_state_snapshot,
            source,
            request_id,
            audit_user_id
        ) VALUES (
            p_idempotency_key,
            p_session_id,
            p_verb,
            p_view_taxonomy_context,
            COALESCE(p_view_selection, '{}'),
            COALESCE(p_view_refinements, '[]'::jsonb),
            COALESCE(p_view_stack_depth, 1),
            p_view_state_snapshot,
            p_source,
            p_request_id,
            p_actor_id
        ) RETURNING change_id INTO v_change_id;

        -- Also update session's current view state
        IF p_session_id IS NOT NULL THEN
            UPDATE "ob-poc".dsl_sessions
            SET current_view_state = p_view_state_snapshot,
                view_updated_at = now()
            WHERE session_id = p_session_id;
        END IF;
    END IF;

    RETURN QUERY SELECT p_idempotency_key, v_change_id, FALSE;
END;
$$;


--
-- Name: FUNCTION record_execution_with_view_state(p_idempotency_key text, p_execution_id uuid, p_statement_index integer, p_verb character varying, p_args_hash character varying, p_result_type character varying, p_result_id uuid, p_result_json jsonb, p_result_affected bigint, p_verb_hash bytea, p_source character varying, p_request_id uuid, p_actor_id uuid, p_actor_type character varying, p_session_id uuid, p_view_taxonomy_context jsonb, p_view_selection uuid[], p_view_refinements jsonb, p_view_stack_depth integer, p_view_state_snapshot jsonb); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".record_execution_with_view_state(p_idempotency_key text, p_execution_id uuid, p_statement_index integer, p_verb character varying, p_args_hash character varying, p_result_type character varying, p_result_id uuid, p_result_json jsonb, p_result_affected bigint, p_verb_hash bytea, p_source character varying, p_request_id uuid, p_actor_id uuid, p_actor_type character varying, p_session_id uuid, p_view_taxonomy_context jsonb, p_view_selection uuid[], p_view_refinements jsonb, p_view_stack_depth integer, p_view_state_snapshot jsonb) IS 'Atomically records execution result and view state change in single transaction';


--
-- Name: record_view_state_change(text, uuid, character varying, jsonb, uuid[], jsonb, integer, jsonb, uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".record_view_state_change(p_idempotency_key text, p_session_id uuid, p_verb_name character varying, p_taxonomy_context jsonb, p_selection uuid[], p_refinements jsonb, p_stack_depth integer, p_view_state_snapshot jsonb, p_audit_user_id uuid DEFAULT NULL::uuid) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_change_id UUID;
BEGIN
    INSERT INTO "ob-poc".dsl_view_state_changes (
        idempotency_key,
        session_id,
        verb_name,
        taxonomy_context,
        selection,
        refinements,
        stack_depth,
        view_state_snapshot,
        audit_user_id
    ) VALUES (
        p_idempotency_key,
        p_session_id,
        p_verb_name,
        p_taxonomy_context,
        p_selection,
        p_refinements,
        p_stack_depth,
        p_view_state_snapshot,
        p_audit_user_id
    ) RETURNING change_id INTO v_change_id;

    -- Also update the session's current view state
    IF p_session_id IS NOT NULL THEN
        UPDATE "ob-poc".dsl_sessions
        SET current_view_state = p_view_state_snapshot,
            view_updated_at = now()
        WHERE session_id = p_session_id;
    END IF;

    RETURN v_change_id;
END;
$$;


--
-- Name: FUNCTION record_view_state_change(p_idempotency_key text, p_session_id uuid, p_verb_name character varying, p_taxonomy_context jsonb, p_selection uuid[], p_refinements jsonb, p_stack_depth integer, p_view_state_snapshot jsonb, p_audit_user_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".record_view_state_change(p_idempotency_key text, p_session_id uuid, p_verb_name character varying, p_taxonomy_context jsonb, p_selection uuid[], p_refinements jsonb, p_stack_depth integer, p_view_state_snapshot jsonb, p_audit_user_id uuid) IS 'Atomically records view state change and updates session - called from DSL pipeline';


--
-- Name: refresh_document_type_similarities(); Type: FUNCTION; Schema: ob-poc; Owner: -
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


--
-- Name: reset_layout_overrides(uuid, character varying, uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".reset_layout_overrides(p_cbu_id uuid, p_view_mode character varying DEFAULT NULL::character varying, p_user_id uuid DEFAULT NULL::uuid) RETURNS integer
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_count INTEGER;
BEGIN
    DELETE FROM "ob-poc".layout_overrides
    WHERE cbu_id = p_cbu_id
      AND (p_view_mode IS NULL OR view_mode = p_view_mode)
      AND (p_user_id IS NULL OR user_id = p_user_id);
    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$;


--
-- Name: FUNCTION reset_layout_overrides(p_cbu_id uuid, p_view_mode character varying, p_user_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".reset_layout_overrides(p_cbu_id uuid, p_view_mode character varying, p_user_id uuid) IS 'Reset layout overrides for a CBU. Pass view_mode and/or user_id to limit scope.';


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
-- Name: sync_commercial_client_role(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".sync_commercial_client_role() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- If commercial_client_entity_id is being set/changed
    IF NEW.commercial_client_entity_id IS NOT NULL THEN
        -- Ensure role exists (upsert)
        INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
        SELECT 
            NEW.cbu_id,
            NEW.commercial_client_entity_id,
            r.role_id
        FROM "ob-poc".roles r
        WHERE r.name = 'COMMERCIAL_CLIENT'
        ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING;
    END IF;
    
    -- If commercial_client_entity_id is being cleared
    IF OLD IS NOT NULL 
       AND OLD.commercial_client_entity_id IS NOT NULL 
       AND (NEW.commercial_client_entity_id IS NULL OR NEW.commercial_client_entity_id != OLD.commercial_client_entity_id) THEN
        -- Remove old role
        DELETE FROM "ob-poc".cbu_entity_roles 
        WHERE cbu_id = NEW.cbu_id 
          AND entity_id = OLD.commercial_client_entity_id
          AND role_id = (SELECT role_id FROM "ob-poc".roles WHERE name = 'COMMERCIAL_CLIENT');
    END IF;
    
    RETURN NEW;
END;
$$;


--
-- Name: FUNCTION sync_commercial_client_role(); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".sync_commercial_client_role() IS 'Maintains invariant: commercial_client_entity_id always has matching COMMERCIAL_CLIENT role in cbu_entity_roles';


--
-- Name: trigger_invalidate_layout_cache(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".trigger_invalidate_layout_cache() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- For entity_relationships changes, find affected CBUs
    IF TG_TABLE_NAME = 'entity_relationships' THEN
        -- Find CBUs that include either entity
        DELETE FROM "ob-poc".layout_cache lc
        WHERE lc.cbu_id IN (
            SELECT DISTINCT cer.cbu_id
            FROM "ob-poc".cbu_entity_roles cer
            WHERE cer.entity_id IN (
                COALESCE(NEW.from_entity_id, OLD.from_entity_id),
                COALESCE(NEW.to_entity_id, OLD.to_entity_id)
            )
        );
    END IF;

    -- For cbu_entity_roles changes
    IF TG_TABLE_NAME = 'cbu_entity_roles' THEN
        DELETE FROM "ob-poc".layout_cache
        WHERE cbu_id = COALESCE(NEW.cbu_id, OLD.cbu_id);
    END IF;

    -- For cbu_products changes
    IF TG_TABLE_NAME = 'cbu_products' THEN
        DELETE FROM "ob-poc".layout_cache
        WHERE cbu_id = COALESCE(NEW.cbu_id, OLD.cbu_id);
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$;


--
-- Name: ubo_chain_as_of(uuid, date, numeric); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".ubo_chain_as_of(p_entity_id uuid, p_as_of_date date DEFAULT CURRENT_DATE, p_threshold numeric DEFAULT 25.0) RETURNS TABLE(chain_path uuid[], chain_names text[], ultimate_owner_id uuid, ultimate_owner_name character varying, ultimate_owner_type character varying, effective_percentage numeric, chain_length integer)
    LANGUAGE plpgsql STABLE
    AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE ownership_chain AS (
        -- Base case: direct owners of the target entity
        SELECT
            ARRAY[r.from_entity_id] AS path,
            ARRAY[e.name::TEXT] AS names,
            r.from_entity_id AS current_entity,
            r.percentage AS cumulative_pct,
            1 AS depth,
            et.type_code AS entity_type
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE r.to_entity_id = p_entity_id
          AND r.relationship_type = 'ownership'
          AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
          AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date)

        UNION ALL

        -- Recursive case: follow the chain upward
        SELECT
            oc.path || r.from_entity_id,
            oc.names || e.name::TEXT,
            r.from_entity_id,
            (oc.cumulative_pct * r.percentage / 100.0)::NUMERIC(10,4),
            oc.depth + 1,
            et.type_code
        FROM ownership_chain oc
        JOIN "ob-poc".entity_relationships r ON r.to_entity_id = oc.current_entity
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE r.relationship_type = 'ownership'
          AND (r.effective_from IS NULL OR r.effective_from <= p_as_of_date)
          AND (r.effective_to IS NULL OR r.effective_to > p_as_of_date)
          AND oc.depth < 10 -- Prevent infinite loops
          AND NOT (r.from_entity_id = ANY(oc.path)) -- Prevent cycles
    )
    -- Return chains that end at natural persons (UBOs)
    SELECT
        oc.path,
        oc.names,
        oc.current_entity AS ultimate_owner_id,
        e.name AS ultimate_owner_name,
        et.type_code AS ultimate_owner_type,
        oc.cumulative_pct AS effective_percentage,
        oc.depth AS chain_length
    FROM ownership_chain oc
    JOIN "ob-poc".entities e ON oc.current_entity = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE et.entity_category = 'PERSON'
      AND oc.cumulative_pct >= p_threshold
    ORDER BY oc.cumulative_pct DESC;
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


--
-- Name: update_entity_deps_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_entity_deps_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;


--
-- Name: update_proofs_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_proofs_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_ubo_edges_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_ubo_edges_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_verb_search_text(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_verb_search_text() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.search_text := coalesce(NEW.description, '') || ' ' ||
                       coalesce(array_to_string(NEW.intent_patterns, ' '), '') || ' ' ||
                       coalesce(NEW.example_short, '');
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;


--
-- Name: update_workflow_timestamp(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".update_workflow_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: upsert_role(character varying, text, character varying, character varying, character varying, boolean, boolean, boolean, jsonb, integer, character varying, integer); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".upsert_role(p_name character varying, p_description text, p_role_category character varying, p_layout_category character varying, p_ubo_treatment character varying, p_requires_percentage boolean, p_natural_person_only boolean, p_legal_entity_only boolean, p_compatible_entity_categories jsonb, p_display_priority integer, p_kyc_obligation character varying, p_sort_order integer) RETURNS uuid
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_role_id UUID;
BEGIN
    INSERT INTO "ob-poc".roles (
        name, description, role_category, layout_category, ubo_treatment,
        requires_percentage, natural_person_only, legal_entity_only,
        compatible_entity_categories, display_priority, kyc_obligation,
        sort_order, is_active, created_at, updated_at
    ) VALUES (
        UPPER(p_name), p_description, p_role_category, p_layout_category, p_ubo_treatment,
        p_requires_percentage, p_natural_person_only, p_legal_entity_only,
        p_compatible_entity_categories, p_display_priority, p_kyc_obligation,
        p_sort_order, TRUE, NOW(), NOW()
    )
    ON CONFLICT (name) DO UPDATE SET
        description = EXCLUDED.description,
        role_category = EXCLUDED.role_category,
        layout_category = EXCLUDED.layout_category,
        ubo_treatment = EXCLUDED.ubo_treatment,
        requires_percentage = EXCLUDED.requires_percentage,
        natural_person_only = EXCLUDED.natural_person_only,
        legal_entity_only = EXCLUDED.legal_entity_only,
        compatible_entity_categories = EXCLUDED.compatible_entity_categories,
        display_priority = EXCLUDED.display_priority,
        kyc_obligation = EXCLUDED.kyc_obligation,
        sort_order = EXCLUDED.sort_order,
        updated_at = NOW()
    RETURNING role_id INTO v_role_id;
    
    RETURN v_role_id;
END;
$$;


--
-- Name: validate_role_assignment(uuid, character varying, uuid); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".validate_role_assignment(p_entity_id uuid, p_role_name character varying, p_cbu_id uuid) RETURNS TABLE(is_valid boolean, error_code character varying, error_message text)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_role RECORD;
    v_entity RECORD;
    v_existing_roles TEXT[];
    v_incompatible RECORD;
BEGIN
    -- Get role details
    SELECT * INTO v_role FROM "ob-poc".roles WHERE name = UPPER(p_role_name);
    IF NOT FOUND THEN
        RETURN QUERY SELECT FALSE, 'ROLE_NOT_FOUND'::VARCHAR(50), 
            format('Role %s does not exist', p_role_name);
        RETURN;
    END IF;
    
    -- Get entity details
    SELECT e.entity_id, e.name, et.entity_category, et.type_code
    INTO v_entity
    FROM "ob-poc".entities e
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE e.entity_id = p_entity_id;
    
    IF NOT FOUND THEN
        RETURN QUERY SELECT FALSE, 'ENTITY_NOT_FOUND'::VARCHAR(50),
            format('Entity %s does not exist', p_entity_id);
        RETURN;
    END IF;
    
    -- Check natural person constraint
    IF v_role.natural_person_only AND v_entity.entity_category != 'PERSON' THEN
        RETURN QUERY SELECT FALSE, 'NATURAL_PERSON_REQUIRED'::VARCHAR(50),
            format('Role %s can only be assigned to natural persons, but %s is %s',
                   p_role_name, v_entity.name, v_entity.entity_category);
        RETURN;
    END IF;
    
    -- Check legal entity constraint
    IF v_role.legal_entity_only AND v_entity.entity_category = 'PERSON' THEN
        RETURN QUERY SELECT FALSE, 'LEGAL_ENTITY_REQUIRED'::VARCHAR(50),
            format('Role %s can only be assigned to legal entities, but %s is a person',
                   p_role_name, v_entity.name);
        RETURN;
    END IF;
    
    -- Check entity category compatibility
    IF v_role.compatible_entity_categories IS NOT NULL THEN
        IF NOT (v_role.compatible_entity_categories ? v_entity.entity_category) THEN
            RETURN QUERY SELECT FALSE, 'INCOMPATIBLE_ENTITY_TYPE'::VARCHAR(50),
                format('Role %s is not compatible with entity category %s. Compatible: %s',
                       p_role_name, v_entity.entity_category, v_role.compatible_entity_categories);
            RETURN;
        END IF;
    END IF;
    
    -- Get existing roles for this entity in this CBU
    SELECT array_agg(r.name) INTO v_existing_roles
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id
    WHERE cer.entity_id = p_entity_id AND cer.cbu_id = p_cbu_id;
    
    -- Check for incompatible role combinations
    FOR v_incompatible IN
        SELECT ri.role_a, ri.role_b, ri.reason, ri.exception_allowed
        FROM "ob-poc".role_incompatibilities ri
        WHERE (ri.role_a = UPPER(p_role_name) OR ri.role_b = UPPER(p_role_name))
    LOOP
        IF v_incompatible.role_a = UPPER(p_role_name) THEN
            IF v_incompatible.role_b = ANY(v_existing_roles) THEN
                IF NOT v_incompatible.exception_allowed THEN
                    RETURN QUERY SELECT FALSE, 'INCOMPATIBLE_ROLES'::VARCHAR(50),
                        format('Role %s is incompatible with existing role %s: %s',
                               p_role_name, v_incompatible.role_b, v_incompatible.reason);
                    RETURN;
                END IF;
            END IF;
        ELSE
            IF v_incompatible.role_a = ANY(v_existing_roles) THEN
                IF NOT v_incompatible.exception_allowed THEN
                    RETURN QUERY SELECT FALSE, 'INCOMPATIBLE_ROLES'::VARCHAR(50),
                        format('Role %s is incompatible with existing role %s: %s',
                               p_role_name, v_incompatible.role_a, v_incompatible.reason);
                    RETURN;
                END IF;
            END IF;
        END IF;
    END LOOP;
    
    -- All checks passed
    RETURN QUERY SELECT TRUE, NULL::VARCHAR(50), NULL::TEXT;
END;
$$;


--
-- Name: FUNCTION validate_role_assignment(p_entity_id uuid, p_role_name character varying, p_cbu_id uuid); Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON FUNCTION "ob-poc".validate_role_assignment(p_entity_id uuid, p_role_name character varying, p_cbu_id uuid) IS 'Validates that a role can be assigned to an entity, checking entity type compatibility and role conflicts.';


--
-- Name: validate_ubo_status_transition(); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".validate_ubo_status_transition() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.verification_status IS DISTINCT FROM NEW.verification_status THEN
        IF NOT "ob-poc".is_valid_ubo_transition(OLD.verification_status, NEW.verification_status) THEN
            RAISE EXCEPTION 'Invalid UBO status transition from % to %', 
                OLD.verification_status, NEW.verification_status;
        END IF;
        
        -- If transitioning to PROVEN, check evidence requirements
        IF NEW.verification_status = 'PROVEN' THEN
            DECLARE
                v_can_prove BOOLEAN;
            BEGIN
                SELECT can_prove INTO v_can_prove
                FROM "ob-poc".can_prove_ubo(NEW.ubo_id);
                
                IF NOT v_can_prove THEN
                    RAISE WARNING 'UBO % marked as PROVEN without complete evidence', NEW.ubo_id;
                    -- Note: Warning only, not blocking - allows override
                END IF;
            END;
        END IF;
        
        -- Set proof_date when transitioning to PROVEN
        IF NEW.verification_status = 'PROVEN' AND NEW.proof_date IS NULL THEN
            NEW.proof_date := now();
        END IF;
    END IF;
    
    RETURN NEW;
END;
$$;


--
-- Name: entity_allows_simplified_dd(uuid); Type: FUNCTION; Schema: ob_kyc; Owner: -
--

CREATE FUNCTION ob_kyc.entity_allows_simplified_dd(p_entity_id uuid) RETURNS boolean
    LANGUAGE plpgsql STABLE
    AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM ob_kyc.entity_regulatory_registrations r
        JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
        JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
        WHERE r.entity_id = p_entity_id
          AND r.status = 'ACTIVE'
          AND r.registration_verified = TRUE
          AND rt.allows_simplified_dd = TRUE
    );
END;
$$;


--
-- Name: ensure_entity_exists(character varying, character varying, character varying); Type: FUNCTION; Schema: public; Owner: -
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


--
-- Name: generate_correlation_id(text, uuid, uuid, text); Type: FUNCTION; Schema: public; Owner: -
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


--
-- Name: generate_idempotency_key(text, text, text, uuid, uuid, uuid); Type: FUNCTION; Schema: public; Owner: -
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


--
-- Name: get_resource_endpoint_url(text, text, text); Type: FUNCTION; Schema: public; Owner: -
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


--
-- Name: update_updated_at_column(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$;


--
-- Name: generate_attestation_signature(uuid, uuid, uuid[], text, timestamp with time zone); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.generate_attestation_signature(p_attester_id uuid, p_campaign_id uuid, p_item_ids uuid[], p_attestation_text text, p_timestamp timestamp with time zone) RETURNS text
    LANGUAGE plpgsql IMMUTABLE
    AS $$
DECLARE
    v_input TEXT;
BEGIN
    v_input := p_attester_id::text || '|' ||
               p_campaign_id::text || '|' ||
               array_to_string(p_item_ids, ',') || '|' ||
               p_attestation_text || '|' ||
               p_timestamp::text;

    RETURN 'sha256:' || encode(sha256(v_input::bytea), 'hex');
END;
$$;


--
-- Name: get_user_access_domains(uuid); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.get_user_access_domains(p_user_id uuid) RETURNS character varying[]
    LANGUAGE sql STABLE
    AS $$
    SELECT array_agg(DISTINCT unnest_domain)
    FROM teams.v_effective_memberships m
    CROSS JOIN LATERAL unnest(COALESCE(m.access_domains, ARRAY[]::varchar[])) as unnest_domain
    WHERE m.user_id = p_user_id;
$$;


--
-- Name: get_user_cbu_access(uuid); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.get_user_cbu_access(p_user_id uuid) RETURNS TABLE(cbu_id uuid, cbu_name character varying, access_domains character varying[], via_teams uuid[], roles character varying[])
    LANGUAGE sql STABLE
    AS $$
    SELECT cbu_id, cbu_name, access_domains, via_teams, roles
    FROM teams.v_user_cbu_access
    WHERE user_id = p_user_id;
$$;


--
-- Name: log_membership_change(); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.log_membership_change() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO teams.membership_history
            (membership_id, team_id, user_id, action, new_role_key, changed_by_user_id)
        VALUES
            (NEW.membership_id, NEW.team_id, NEW.user_id, 'ADDED', NEW.role_key, NEW.delegated_by_user_id);
    ELSIF TG_OP = 'UPDATE' THEN
        IF OLD.role_key != NEW.role_key THEN
            INSERT INTO teams.membership_history
                (membership_id, team_id, user_id, action, old_role_key, new_role_key)
            VALUES
                (NEW.membership_id, NEW.team_id, NEW.user_id, 'UPDATED', OLD.role_key, NEW.role_key);
        END IF;
        IF NEW.effective_to IS NOT NULL AND OLD.effective_to IS NULL THEN
            INSERT INTO teams.membership_history
                (membership_id, team_id, user_id, action, old_role_key)
            VALUES
                (NEW.membership_id, NEW.team_id, NEW.user_id, 'REMOVED', OLD.role_key);
        END IF;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: update_timestamp(); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.update_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: user_can_access_cbu(uuid, uuid); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.user_can_access_cbu(p_user_id uuid, p_cbu_id uuid) RETURNS boolean
    LANGUAGE sql STABLE
    AS $$
    SELECT EXISTS (
        SELECT 1 FROM teams.v_user_cbu_access
        WHERE user_id = p_user_id AND cbu_id = p_cbu_id
    );
$$;


--
-- Name: user_has_domain(uuid, character varying); Type: FUNCTION; Schema: teams; Owner: -
--

CREATE FUNCTION teams.user_has_domain(p_user_id uuid, p_domain character varying) RETURNS boolean
    LANGUAGE sql STABLE
    AS $$
    SELECT p_domain = ANY(COALESCE(teams.get_user_access_domains(p_user_id), ARRAY[]::varchar[]));
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: clients; Type: TABLE; Schema: client_portal; Owner: -
--

CREATE TABLE client_portal.clients (
    client_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    email character varying(255) NOT NULL,
    accessible_cbus uuid[] DEFAULT '{}'::uuid[] NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_login_at timestamp with time zone,
    employer_entity_id uuid,
    identity_provider character varying(50) DEFAULT 'local'::character varying,
    status character varying(50) DEFAULT 'ACTIVE'::character varying,
    offboarded_at timestamp with time zone,
    offboard_reason character varying(50),
    CONSTRAINT chk_identity_provider CHECK (((identity_provider)::text = ANY ((ARRAY['local'::character varying, 'saml'::character varying, 'oidc'::character varying])::text[]))),
    CONSTRAINT chk_offboard_reason CHECK (((offboard_reason IS NULL) OR ((offboard_reason)::text = ANY ((ARRAY['resigned'::character varying, 'terminated'::character varying, 'retired'::character varying, 'deceased'::character varying, 'other'::character varying])::text[])))),
    CONSTRAINT chk_user_status CHECK (((status)::text = ANY ((ARRAY['ACTIVE'::character varying, 'SUSPENDED'::character varying, 'OFFBOARDED'::character varying])::text[])))
);


--
-- Name: commitments; Type: TABLE; Schema: client_portal; Owner: -
--

CREATE TABLE client_portal.commitments (
    commitment_id uuid DEFAULT gen_random_uuid() NOT NULL,
    client_id uuid NOT NULL,
    request_id uuid NOT NULL,
    commitment_text text NOT NULL,
    expected_date date,
    reminder_date date,
    reminder_sent_at timestamp with time zone,
    status character varying(20) DEFAULT 'PENDING'::character varying NOT NULL,
    fulfilled_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT commitments_status_check CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'FULFILLED'::character varying, 'OVERDUE'::character varying, 'CANCELLED'::character varying])::text[])))
);


--
-- Name: credentials; Type: TABLE; Schema: client_portal; Owner: -
--

CREATE TABLE client_portal.credentials (
    credential_id uuid DEFAULT gen_random_uuid() NOT NULL,
    client_id uuid NOT NULL,
    credential_hash text NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone
);


--
-- Name: escalations; Type: TABLE; Schema: client_portal; Owner: -
--

CREATE TABLE client_portal.escalations (
    escalation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    client_id uuid NOT NULL,
    session_id uuid,
    cbu_id uuid,
    reason text,
    preferred_contact character varying(20),
    conversation_context jsonb,
    assigned_to_user_id uuid,
    assigned_at timestamp with time zone,
    status character varying(20) DEFAULT 'OPEN'::character varying NOT NULL,
    resolution_notes text,
    resolved_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT escalations_preferred_contact_check CHECK (((preferred_contact)::text = ANY ((ARRAY['CALL'::character varying, 'EMAIL'::character varying, 'VIDEO'::character varying])::text[]))),
    CONSTRAINT escalations_status_check CHECK (((status)::text = ANY ((ARRAY['OPEN'::character varying, 'ASSIGNED'::character varying, 'IN_PROGRESS'::character varying, 'RESOLVED'::character varying, 'CLOSED'::character varying])::text[])))
);


--
-- Name: sessions; Type: TABLE; Schema: client_portal; Owner: -
--

CREATE TABLE client_portal.sessions (
    session_id uuid DEFAULT gen_random_uuid() NOT NULL,
    client_id uuid NOT NULL,
    active_cbu_id uuid,
    collection_state jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_active_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '24:00:00'::interval) NOT NULL
);


--
-- Name: submissions; Type: TABLE; Schema: client_portal; Owner: -
--

CREATE TABLE client_portal.submissions (
    submission_id uuid DEFAULT gen_random_uuid() NOT NULL,
    client_id uuid NOT NULL,
    request_id uuid NOT NULL,
    submission_type character varying(50) NOT NULL,
    document_type character varying(100),
    file_reference text,
    file_name character varying(255),
    file_size_bytes bigint,
    mime_type character varying(100),
    info_type character varying(100),
    info_data jsonb,
    note_text text,
    status character varying(20) DEFAULT 'SUBMITTED'::character varying NOT NULL,
    review_notes text,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    cataloged_document_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT submissions_status_check CHECK (((status)::text = ANY ((ARRAY['SUBMITTED'::character varying, 'UNDER_REVIEW'::character varying, 'ACCEPTED'::character varying, 'REJECTED'::character varying, 'SUPERSEDED'::character varying])::text[]))),
    CONSTRAINT submissions_submission_type_check CHECK (((submission_type)::text = ANY ((ARRAY['DOCUMENT'::character varying, 'INFORMATION'::character varying, 'NOTE'::character varying, 'CLARIFICATION'::character varying])::text[])))
);


--
-- Name: users; Type: VIEW; Schema: client_portal; Owner: -
--

CREATE VIEW client_portal.users AS
 SELECT client_id AS user_id,
    name,
    email,
    accessible_cbus,
    is_active,
    created_at,
    updated_at,
    last_login_at,
    employer_entity_id,
    identity_provider,
    status,
    offboarded_at,
    offboard_reason
   FROM client_portal.clients;


--
-- Name: outstanding_requests; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.outstanding_requests (
    request_id uuid DEFAULT gen_random_uuid() NOT NULL,
    subject_type character varying(50) NOT NULL,
    subject_id uuid NOT NULL,
    workstream_id uuid,
    case_id uuid,
    cbu_id uuid,
    entity_id uuid,
    request_type character varying(50) NOT NULL,
    request_subtype character varying(100) NOT NULL,
    request_details jsonb DEFAULT '{}'::jsonb,
    requested_from_type character varying(50),
    requested_from_entity_id uuid,
    requested_from_label character varying(255),
    requested_by_user_id uuid,
    requested_by_agent boolean DEFAULT false,
    requested_at timestamp with time zone DEFAULT now(),
    due_date date,
    grace_period_days integer DEFAULT 3,
    last_reminder_at timestamp with time zone,
    reminder_count integer DEFAULT 0,
    max_reminders integer DEFAULT 3,
    communication_log jsonb DEFAULT '[]'::jsonb,
    status character varying(50) DEFAULT 'PENDING'::character varying,
    status_reason text,
    fulfilled_at timestamp with time zone,
    fulfilled_by_user_id uuid,
    fulfillment_type character varying(50),
    fulfillment_reference_type character varying(50),
    fulfillment_reference_id uuid,
    fulfillment_notes text,
    escalated_at timestamp with time zone,
    escalation_level integer DEFAULT 0,
    escalation_reason character varying(255),
    escalated_to_user_id uuid,
    blocks_subject boolean DEFAULT true,
    blocker_message character varying(500),
    created_by_verb character varying(100),
    created_by_execution_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    reason_for_request text,
    compliance_context text,
    acceptable_alternatives text[],
    client_visible boolean DEFAULT true NOT NULL,
    client_notes text,
    CONSTRAINT chk_oreq_fulfillment_type CHECK (((fulfillment_type IS NULL) OR ((fulfillment_type)::text = ANY ((ARRAY['DOCUMENT_UPLOAD'::character varying, 'MANUAL_ENTRY'::character varying, 'API_RESPONSE'::character varying, 'WAIVER'::character varying])::text[])))),
    CONSTRAINT chk_oreq_request_type CHECK (((request_type)::text = ANY ((ARRAY['DOCUMENT'::character varying, 'INFORMATION'::character varying, 'VERIFICATION'::character varying, 'APPROVAL'::character varying, 'SIGNATURE'::character varying])::text[]))),
    CONSTRAINT chk_oreq_requested_from_type CHECK (((requested_from_type IS NULL) OR ((requested_from_type)::text = ANY ((ARRAY['CLIENT'::character varying, 'ENTITY'::character varying, 'EXTERNAL_PROVIDER'::character varying, 'INTERNAL'::character varying])::text[])))),
    CONSTRAINT chk_oreq_status CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'FULFILLED'::character varying, 'PARTIAL'::character varying, 'CANCELLED'::character varying, 'ESCALATED'::character varying, 'EXPIRED'::character varying, 'WAIVED'::character varying])::text[]))),
    CONSTRAINT chk_oreq_subject_type CHECK (((subject_type)::text = ANY ((ARRAY['WORKSTREAM'::character varying, 'KYC_CASE'::character varying, 'ENTITY'::character varying, 'CBU'::character varying])::text[])))
);


--
-- Name: TABLE outstanding_requests; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.outstanding_requests IS 'Fire-and-forget operations awaiting response (document requests, verifications, etc.)';


--
-- Name: COLUMN outstanding_requests.subject_type; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.subject_type IS 'What is this request attached to: WORKSTREAM, KYC_CASE, ENTITY, CBU';


--
-- Name: COLUMN outstanding_requests.request_type; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.request_type IS 'Category of request: DOCUMENT, INFORMATION, VERIFICATION, APPROVAL, SIGNATURE';


--
-- Name: COLUMN outstanding_requests.request_subtype; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.request_subtype IS 'Specific type within category, e.g., SOURCE_OF_WEALTH, ID_DOCUMENT';


--
-- Name: COLUMN outstanding_requests.grace_period_days; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.grace_period_days IS 'Days after due_date before auto-escalation';


--
-- Name: COLUMN outstanding_requests.blocks_subject; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.blocks_subject IS 'Whether this pending request blocks the subject from progressing';


--
-- Name: COLUMN outstanding_requests.reason_for_request; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.reason_for_request IS 'Plain English explanation of why this is needed';


--
-- Name: COLUMN outstanding_requests.compliance_context; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.compliance_context IS 'Regulatory/legal basis for the request';


--
-- Name: COLUMN outstanding_requests.acceptable_alternatives; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.acceptable_alternatives IS 'Alternative document types that would satisfy this request';


--
-- Name: COLUMN outstanding_requests.client_visible; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.client_visible IS 'Whether this request should be shown to the client';


--
-- Name: COLUMN outstanding_requests.client_notes; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.outstanding_requests.client_notes IS 'Notes from the client about this request';


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
-- Name: v_client_outstanding; Type: VIEW; Schema: client_portal; Owner: -
--

CREATE VIEW client_portal.v_client_outstanding AS
 SELECT r.request_id,
    r.cbu_id,
    r.entity_id,
    e.name AS entity_name,
    r.request_type,
    r.request_subtype,
    r.reason_for_request,
    r.compliance_context,
    r.acceptable_alternatives,
    r.status,
    r.due_date,
    r.client_notes,
    r.created_at,
    r.updated_at,
    ( SELECT count(*) AS count
           FROM client_portal.submissions s
          WHERE (s.request_id = r.request_id)) AS submission_count
   FROM (kyc.outstanding_requests r
     LEFT JOIN "ob-poc".entities e ON ((r.entity_id = e.entity_id)))
  WHERE ((r.client_visible = true) AND ((r.status)::text <> ALL ((ARRAY['FULFILLED'::character varying, 'CANCELLED'::character varying, 'WAIVED'::character varying])::text[])));


--
-- Name: cbu_cash_sweep_config; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_cash_sweep_config (
    sweep_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    profile_id uuid,
    currency character varying(3) NOT NULL,
    threshold_amount numeric(18,2) NOT NULL,
    vehicle_type character varying(20) NOT NULL,
    vehicle_id character varying(50),
    vehicle_name character varying(255),
    sweep_time time without time zone NOT NULL,
    sweep_timezone character varying(50) NOT NULL,
    sweep_frequency character varying(20) DEFAULT 'DAILY'::character varying,
    interest_allocation character varying(20) DEFAULT 'ACCRUED'::character varying,
    interest_account_id uuid,
    sweep_resource_id uuid,
    is_active boolean DEFAULT true,
    effective_date date DEFAULT CURRENT_DATE NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_interest_allocation CHECK (((interest_allocation)::text = ANY ((ARRAY['ACCRUED'::character varying, 'MONTHLY'::character varying, 'QUARTERLY'::character varying, 'REINVEST'::character varying])::text[]))),
    CONSTRAINT valid_sweep_frequency CHECK (((sweep_frequency)::text = ANY ((ARRAY['INTRADAY'::character varying, 'DAILY'::character varying, 'WEEKLY'::character varying, 'MONTHLY'::character varying])::text[]))),
    CONSTRAINT valid_vehicle_type CHECK (((vehicle_type)::text = ANY ((ARRAY['STIF'::character varying, 'MMF'::character varying, 'DEPOSIT'::character varying, 'OVERNIGHT_REPO'::character varying, 'TRI_PARTY_REPO'::character varying, 'MANUAL'::character varying])::text[])))
);


--
-- Name: TABLE cbu_cash_sweep_config; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_cash_sweep_config IS 'Cash sweep configuration for idle cash management. STIFs, MMFs, overnight deposits.';


--
-- Name: cbu_cross_border_config; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_cross_border_config (
    config_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    source_market_id uuid NOT NULL,
    target_market_id uuid NOT NULL,
    settlement_method character varying(20) NOT NULL,
    bridge_location_id uuid,
    preferred_currency character varying(3),
    fx_timing character varying(20),
    additional_days integer DEFAULT 0,
    special_instructions text,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cbu_cross_border_config_fx_timing_check CHECK (((fx_timing)::text = ANY ((ARRAY['PRE_SETTLEMENT'::character varying, 'ON_SETTLEMENT'::character varying, 'POST_SETTLEMENT'::character varying])::text[]))),
    CONSTRAINT cbu_cross_border_config_settlement_method_check CHECK (((settlement_method)::text = ANY ((ARRAY['BRIDGE'::character varying, 'DIRECT'::character varying, 'VIA_ICSD'::character varying])::text[])))
);


--
-- Name: TABLE cbu_cross_border_config; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_cross_border_config IS 'Cross-border settlement routing configuration';


--
-- Name: cbu_im_assignments; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_im_assignments (
    assignment_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    profile_id uuid,
    manager_entity_id uuid,
    manager_lei character varying(20),
    manager_bic character varying(11),
    manager_name character varying(255),
    manager_role character varying(30) DEFAULT 'INVESTMENT_MANAGER'::character varying NOT NULL,
    priority integer DEFAULT 100 NOT NULL,
    scope_all boolean DEFAULT false,
    scope_markets text[],
    scope_instrument_classes text[],
    scope_currencies text[],
    scope_isda_asset_classes text[],
    instruction_method character varying(20) NOT NULL,
    instruction_resource_id uuid,
    can_trade boolean DEFAULT true,
    can_settle boolean DEFAULT true,
    can_affirm boolean DEFAULT false,
    effective_date date DEFAULT CURRENT_DATE NOT NULL,
    termination_date date,
    status character varying(20) DEFAULT 'ACTIVE'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_im_role CHECK (((manager_role)::text = ANY ((ARRAY['INVESTMENT_MANAGER'::character varying, 'SUB_ADVISOR'::character varying, 'OVERLAY_MANAGER'::character varying, 'TRANSITION_MANAGER'::character varying, 'EXECUTION_BROKER'::character varying])::text[]))),
    CONSTRAINT valid_im_status CHECK (((status)::text = ANY ((ARRAY['ACTIVE'::character varying, 'SUSPENDED'::character varying, 'TERMINATED'::character varying])::text[]))),
    CONSTRAINT valid_instruction_method CHECK (((instruction_method)::text = ANY ((ARRAY['SWIFT'::character varying, 'CTM'::character varying, 'FIX'::character varying, 'API'::character varying, 'MANUAL'::character varying, 'ALERT'::character varying])::text[])))
);


--
-- Name: TABLE cbu_im_assignments; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_im_assignments IS 'Investment Manager assignments with trading scope. Materialized from trading profile.
Links IM to instruction delivery resource for traceability.';


--
-- Name: cbu_instrument_universe; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_instrument_universe (
    universe_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    instrument_class_id uuid NOT NULL,
    market_id uuid,
    currencies character varying(3)[] DEFAULT '{}'::character varying[] NOT NULL,
    settlement_types character varying(10)[] DEFAULT '{DVP}'::character varying[],
    counterparty_entity_id uuid,
    is_held boolean DEFAULT true,
    is_traded boolean DEFAULT true,
    is_active boolean DEFAULT true,
    effective_date date DEFAULT CURRENT_DATE NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    counterparty_key uuid DEFAULT '00000000-0000-0000-0000-000000000000'::uuid NOT NULL
);


--
-- Name: TABLE cbu_instrument_universe; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_instrument_universe IS 'Layer 1: Declares what instrument classes, markets, currencies a CBU trades. Drives SSI completeness checks.';


--
-- Name: cbu_pricing_config; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_pricing_config (
    config_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    profile_id uuid,
    instrument_class_id uuid,
    market_id uuid,
    currency character varying(3),
    priority integer DEFAULT 1 NOT NULL,
    source character varying(30) NOT NULL,
    price_type character varying(20) DEFAULT 'CLOSING'::character varying NOT NULL,
    fallback_source character varying(30),
    max_age_hours integer DEFAULT 24,
    tolerance_pct numeric(5,2) DEFAULT 5.0,
    stale_action character varying(20) DEFAULT 'WARN'::character varying,
    pricing_resource_id uuid,
    effective_date date DEFAULT CURRENT_DATE NOT NULL,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_price_source CHECK (((source)::text = ANY ((ARRAY['BLOOMBERG'::character varying, 'REUTERS'::character varying, 'MARKIT'::character varying, 'REFINITIV'::character varying, 'ICE'::character varying, 'MODEL'::character varying, 'INTERNAL'::character varying, 'VENDOR'::character varying, 'COUNTERPARTY'::character varying])::text[]))),
    CONSTRAINT valid_price_type CHECK (((price_type)::text = ANY ((ARRAY['CLOSING'::character varying, 'MID'::character varying, 'BID'::character varying, 'ASK'::character varying, 'VWAP'::character varying, 'OFFICIAL'::character varying])::text[]))),
    CONSTRAINT valid_stale_action CHECK (((stale_action)::text = ANY ((ARRAY['WARN'::character varying, 'BLOCK'::character varying, 'USE_FALLBACK'::character varying, 'ESCALATE'::character varying])::text[])))
);


--
-- Name: TABLE cbu_pricing_config; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_pricing_config IS 'Pricing source configuration by instrument class. Materialized from trading profile.
Links to provisioned pricing feed resource for traceability.';


--
-- Name: cbu_settlement_chains; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_settlement_chains (
    chain_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    chain_name character varying(100) NOT NULL,
    market_id uuid,
    instrument_class_id uuid,
    currency character varying(3),
    settlement_type character varying(10),
    is_default boolean DEFAULT false NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    effective_date date,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE cbu_settlement_chains; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_settlement_chains IS 'Settlement chain definitions per CBU';


--
-- Name: cbu_settlement_location_preferences; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_settlement_location_preferences (
    preference_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    market_id uuid,
    instrument_class_id uuid,
    preferred_location_id uuid NOT NULL,
    priority integer DEFAULT 50 NOT NULL,
    reason text,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE cbu_settlement_location_preferences; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_settlement_location_preferences IS 'Preferred settlement locations per CBU/market/instrument';


--
-- Name: cbu_ssi; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_ssi (
    ssi_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    ssi_name character varying(100) NOT NULL,
    ssi_type character varying(20) NOT NULL,
    safekeeping_account character varying(35),
    safekeeping_bic character varying(11),
    safekeeping_account_name character varying(100),
    cash_account character varying(35),
    cash_account_bic character varying(11),
    cash_currency character varying(3),
    collateral_account character varying(35),
    collateral_account_bic character varying(11),
    pset_bic character varying(11),
    receiving_agent_bic character varying(11),
    delivering_agent_bic character varying(11),
    status character varying(20) DEFAULT 'PENDING'::character varying,
    effective_date date NOT NULL,
    expiry_date date,
    source character varying(20) DEFAULT 'MANUAL'::character varying,
    source_reference character varying(100),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    created_by character varying(100),
    market_id uuid
);


--
-- Name: TABLE cbu_ssi; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_ssi IS 'Layer 2: Pure SSI account data. No routing logic - just the accounts themselves.';


--
-- Name: cbu_ssi_agent_override; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_ssi_agent_override (
    override_id uuid DEFAULT gen_random_uuid() NOT NULL,
    ssi_id uuid NOT NULL,
    agent_role character varying(10) NOT NULL,
    agent_bic character varying(11) NOT NULL,
    agent_account character varying(35),
    agent_name character varying(100),
    sequence_order integer NOT NULL,
    reason character varying(255),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: cbu_tax_reclaim_config; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_tax_reclaim_config (
    config_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    source_jurisdiction_id uuid NOT NULL,
    reclaim_method character varying(20) NOT NULL,
    service_provider_entity_id uuid,
    minimum_reclaim_amount numeric(15,2),
    minimum_reclaim_currency character varying(3),
    batch_frequency character varying(20),
    expected_recovery_days integer,
    fee_structure jsonb,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cbu_tax_reclaim_config_batch_frequency_check CHECK (((batch_frequency)::text = ANY ((ARRAY['IMMEDIATE'::character varying, 'WEEKLY'::character varying, 'MONTHLY'::character varying, 'QUARTERLY'::character varying])::text[]))),
    CONSTRAINT cbu_tax_reclaim_config_reclaim_method_check CHECK (((reclaim_method)::text = ANY ((ARRAY['AUTOMATIC'::character varying, 'MANUAL'::character varying, 'OUTSOURCED'::character varying, 'NO_RECLAIM'::character varying])::text[])))
);


--
-- Name: TABLE cbu_tax_reclaim_config; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_tax_reclaim_config IS 'Tax reclaim processing configuration per CBU/jurisdiction';


--
-- Name: cbu_tax_reporting; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_tax_reporting (
    reporting_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    reporting_regime character varying(20) NOT NULL,
    reporting_jurisdiction_id uuid NOT NULL,
    reporting_status character varying(20) DEFAULT 'REQUIRED'::character varying,
    giin character varying(30),
    registration_date date,
    reporting_entity_id uuid,
    sponsor_entity_id uuid,
    notes text,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cbu_tax_reporting_reporting_regime_check CHECK (((reporting_regime)::text = ANY ((ARRAY['FATCA'::character varying, 'CRS'::character varying, 'DAC6'::character varying, 'UK_CDOT'::character varying, 'QI'::character varying, '871M'::character varying])::text[]))),
    CONSTRAINT cbu_tax_reporting_reporting_status_check CHECK (((reporting_status)::text = ANY ((ARRAY['REQUIRED'::character varying, 'EXEMPT'::character varying, 'PARTICIPATING'::character varying, 'PENDING'::character varying])::text[])))
);


--
-- Name: TABLE cbu_tax_reporting; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_tax_reporting IS 'Tax reporting obligations (FATCA, CRS, etc.) per CBU';


--
-- Name: cbu_tax_status; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cbu_tax_status (
    status_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    tax_jurisdiction_id uuid NOT NULL,
    investor_type character varying(20) NOT NULL,
    tax_exempt boolean DEFAULT false NOT NULL,
    exempt_reason text,
    documentation_status character varying(20) DEFAULT 'PENDING'::character varying,
    documentation_expiry date,
    applicable_treaty_rate numeric(5,3),
    qualified_intermediary boolean DEFAULT false NOT NULL,
    qi_ein character varying(20),
    fatca_status character varying(20),
    crs_status character varying(20),
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cbu_tax_status_documentation_status_check CHECK (((documentation_status)::text = ANY ((ARRAY['PENDING'::character varying, 'SUBMITTED'::character varying, 'VALIDATED'::character varying, 'EXPIRED'::character varying])::text[]))),
    CONSTRAINT cbu_tax_status_fatca_status_check CHECK (((fatca_status)::text = ANY ((ARRAY['EXEMPT'::character varying, 'PARTICIPATING'::character varying, 'NON_PARTICIPATING'::character varying])::text[]))),
    CONSTRAINT cbu_tax_status_investor_type_check CHECK (((investor_type)::text = ANY ((ARRAY['PENSION'::character varying, 'SOVEREIGN'::character varying, 'CHARITY'::character varying, 'CORPORATE'::character varying, 'INDIVIDUAL'::character varying, 'FUND'::character varying])::text[])))
);


--
-- Name: TABLE cbu_tax_status; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_tax_status IS 'CBU tax status per jurisdiction';


--
-- Name: cfi_codes; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.cfi_codes (
    cfi_code character(6) NOT NULL,
    category character(1) NOT NULL,
    category_name character varying(50),
    group_code character(2) NOT NULL,
    group_name character varying(50),
    attribute_1 character(1),
    attribute_2 character(1),
    attribute_3 character(1),
    attribute_4 character(1),
    class_id uuid,
    security_type_id uuid,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE cfi_codes; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cfi_codes IS 'ISO 10962 CFI code registry. Maps incoming security CFI to our classification.';


--
-- Name: csa_agreements; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.csa_agreements (
    csa_id uuid DEFAULT gen_random_uuid() NOT NULL,
    isda_id uuid NOT NULL,
    csa_type character varying(20) NOT NULL,
    threshold_amount numeric(18,2),
    threshold_currency character varying(3),
    minimum_transfer_amount numeric(18,2),
    rounding_amount numeric(18,2),
    collateral_ssi_id uuid,
    is_active boolean DEFAULT true,
    effective_date date NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_settlement_identity; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.entity_settlement_identity (
    identity_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    primary_bic character varying(11) NOT NULL,
    lei character varying(20),
    alert_participant_id character varying(50),
    ctm_participant_id character varying(50),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_ssi; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.entity_ssi (
    entity_ssi_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    instrument_class_id uuid,
    security_type_id uuid,
    market_id uuid,
    currency character varying(3),
    counterparty_bic character varying(11) NOT NULL,
    safekeeping_account character varying(35),
    source character varying(20) DEFAULT 'ALERT'::character varying,
    source_reference character varying(100),
    status character varying(20) DEFAULT 'ACTIVE'::character varying,
    effective_date date NOT NULL,
    expiry_date date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: instruction_paths; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.instruction_paths (
    path_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instrument_class_id uuid,
    market_id uuid,
    currency character varying(3),
    instruction_type_id uuid NOT NULL,
    resource_id uuid NOT NULL,
    routing_priority integer DEFAULT 1,
    enrichment_sources jsonb DEFAULT '["SUBCUST_NETWORK", "CLIENT_SSI"]'::jsonb,
    validation_rules jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: instruction_types; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.instruction_types (
    type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    type_code character varying(30) NOT NULL,
    name character varying(100) NOT NULL,
    direction character varying(10) NOT NULL,
    payment_type character varying(10) NOT NULL,
    swift_mt_code character varying(10),
    iso20022_msg_type character varying(50),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: instrument_classes; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.instrument_classes (
    class_id uuid DEFAULT gen_random_uuid() NOT NULL,
    code character varying(20) NOT NULL,
    name character varying(100) NOT NULL,
    default_settlement_cycle character varying(10) NOT NULL,
    swift_message_family character varying(10),
    requires_isda boolean DEFAULT false,
    requires_collateral boolean DEFAULT false,
    cfi_category character(1),
    cfi_group character(2),
    smpg_group character varying(20),
    isda_asset_class character varying(30),
    parent_class_id uuid,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE instrument_classes; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.instrument_classes IS 'Canonical instrument classification. Maps to CFI, SMPG/ALERT, and ISDA taxonomies.';


--
-- Name: isda_agreements; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.isda_agreements (
    isda_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    counterparty_entity_id uuid NOT NULL,
    agreement_date date NOT NULL,
    governing_law character varying(20),
    is_active boolean DEFAULT true,
    effective_date date NOT NULL,
    termination_date date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: isda_product_coverage; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.isda_product_coverage (
    coverage_id uuid DEFAULT gen_random_uuid() NOT NULL,
    isda_id uuid NOT NULL,
    instrument_class_id uuid NOT NULL,
    isda_taxonomy_id uuid,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: isda_product_taxonomy; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.isda_product_taxonomy (
    taxonomy_id uuid DEFAULT gen_random_uuid() NOT NULL,
    asset_class character varying(30) NOT NULL,
    base_product character varying(50),
    sub_product character varying(50),
    taxonomy_code character varying(100) NOT NULL,
    upi_template character varying(50),
    class_id uuid,
    cfi_pattern character varying(6),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE isda_product_taxonomy; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.isda_product_taxonomy IS 'ISDA OTC derivatives taxonomy. Used for regulatory reporting and ISDA/CSA linking.';


--
-- Name: markets; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.markets (
    market_id uuid DEFAULT gen_random_uuid() NOT NULL,
    mic character varying(4) NOT NULL,
    name character varying(255) NOT NULL,
    country_code character varying(2) NOT NULL,
    operating_mic character varying(4),
    primary_currency character varying(3) NOT NULL,
    supported_currencies character varying(3)[] DEFAULT '{}'::character varying[],
    csd_bic character varying(11),
    timezone character varying(50) NOT NULL,
    cut_off_time time without time zone,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: security_types; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.security_types (
    security_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    class_id uuid NOT NULL,
    code character varying(4) NOT NULL,
    name character varying(100) NOT NULL,
    cfi_pattern character varying(6),
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE security_types; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.security_types IS 'SMPG/ALERT security type codes. Used for granular booking rule matching.';


--
-- Name: settlement_chain_hops; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.settlement_chain_hops (
    hop_id uuid DEFAULT gen_random_uuid() NOT NULL,
    chain_id uuid NOT NULL,
    hop_sequence integer NOT NULL,
    role character varying(20) NOT NULL,
    intermediary_entity_id uuid,
    intermediary_bic character varying(11),
    intermediary_name character varying(200),
    account_number character varying(50),
    ssi_id uuid,
    instructions text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT settlement_chain_hops_role_check CHECK (((role)::text = ANY ((ARRAY['CUSTODIAN'::character varying, 'SUBCUSTODIAN'::character varying, 'AGENT'::character varying, 'CSD'::character varying, 'ICSD'::character varying])::text[])))
);


--
-- Name: TABLE settlement_chain_hops; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.settlement_chain_hops IS 'Individual hops/intermediaries in a settlement chain';


--
-- Name: settlement_locations; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.settlement_locations (
    location_id uuid DEFAULT gen_random_uuid() NOT NULL,
    location_code character varying(20) NOT NULL,
    location_name character varying(200) NOT NULL,
    location_type character varying(20) NOT NULL,
    country_code character varying(2),
    bic character varying(11),
    operating_hours jsonb,
    settlement_cycles jsonb,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT settlement_locations_location_type_check CHECK (((location_type)::text = ANY ((ARRAY['CSD'::character varying, 'ICSD'::character varying, 'CUSTODIAN'::character varying])::text[])))
);


--
-- Name: TABLE settlement_locations; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.settlement_locations IS 'Reference data for CSDs, ICSDs, and custodian locations';


--
-- Name: ssi_booking_rules; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.ssi_booking_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    ssi_id uuid NOT NULL,
    rule_name character varying(100) NOT NULL,
    priority integer DEFAULT 50 NOT NULL,
    instrument_class_id uuid,
    security_type_id uuid,
    market_id uuid,
    currency character varying(3),
    settlement_type character varying(10),
    counterparty_entity_id uuid,
    isda_asset_class character varying(30),
    isda_base_product character varying(50),
    specificity_score integer GENERATED ALWAYS AS ((((((
CASE
    WHEN (counterparty_entity_id IS NOT NULL) THEN 32
    ELSE 0
END +
CASE
    WHEN (instrument_class_id IS NOT NULL) THEN 16
    ELSE 0
END) +
CASE
    WHEN (security_type_id IS NOT NULL) THEN 8
    ELSE 0
END) +
CASE
    WHEN (market_id IS NOT NULL) THEN 4
    ELSE 0
END) +
CASE
    WHEN (currency IS NOT NULL) THEN 2
    ELSE 0
END) +
CASE
    WHEN (settlement_type IS NOT NULL) THEN 1
    ELSE 0
END)) STORED,
    is_active boolean DEFAULT true,
    effective_date date DEFAULT CURRENT_DATE NOT NULL,
    expiry_date date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE ssi_booking_rules; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.ssi_booking_rules IS 'Layer 3: ALERT-style booking rules. Priority-based matching with wildcards (NULL = any).';


--
-- Name: subcustodian_network; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.subcustodian_network (
    network_id uuid DEFAULT gen_random_uuid() NOT NULL,
    market_id uuid NOT NULL,
    currency character varying(3) NOT NULL,
    subcustodian_bic character varying(11) NOT NULL,
    subcustodian_name character varying(255),
    local_agent_bic character varying(11),
    local_agent_name character varying(255),
    local_agent_account character varying(35),
    csd_participant_id character varying(35),
    place_of_settlement_bic character varying(11) NOT NULL,
    is_primary boolean DEFAULT true,
    effective_date date NOT NULL,
    expiry_date date,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: tax_jurisdictions; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.tax_jurisdictions (
    jurisdiction_id uuid DEFAULT gen_random_uuid() NOT NULL,
    jurisdiction_code character varying(10) NOT NULL,
    jurisdiction_name character varying(200) NOT NULL,
    country_code character varying(2) NOT NULL,
    default_withholding_rate numeric(5,3),
    reclaim_available boolean DEFAULT true NOT NULL,
    reclaim_deadline_days integer,
    tax_authority_name character varying(200),
    tax_authority_code character varying(50),
    documentation_requirements jsonb,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE tax_jurisdictions; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.tax_jurisdictions IS 'Tax jurisdiction reference data with withholding rates';


--
-- Name: tax_treaty_rates; Type: TABLE; Schema: custody; Owner: -
--

CREATE TABLE custody.tax_treaty_rates (
    treaty_id uuid DEFAULT gen_random_uuid() NOT NULL,
    source_jurisdiction_id uuid NOT NULL,
    investor_jurisdiction_id uuid NOT NULL,
    income_type character varying(20) NOT NULL,
    instrument_class_id uuid,
    standard_rate numeric(5,3) NOT NULL,
    treaty_rate numeric(5,3) NOT NULL,
    beneficial_owner_required boolean DEFAULT true NOT NULL,
    documentation_codes text[],
    effective_date date NOT NULL,
    expiry_date date,
    treaty_reference character varying(100),
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tax_treaty_rates_income_type_check CHECK (((income_type)::text = ANY ((ARRAY['DIVIDEND'::character varying, 'INTEREST'::character varying, 'ROYALTY'::character varying, 'CAPITAL_GAIN'::character varying])::text[])))
);


--
-- Name: TABLE tax_treaty_rates; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.tax_treaty_rates IS 'Bilateral tax treaty rates between jurisdictions';


--
-- Name: approval_requests; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.approval_requests (
    approval_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    workstream_id uuid,
    request_type character varying(50) NOT NULL,
    requested_by character varying(255),
    requested_at timestamp with time zone DEFAULT now() NOT NULL,
    approver character varying(255),
    decision character varying(20),
    decision_at timestamp with time zone,
    comments text
);


--
-- Name: case_events; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.case_events (
    event_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    workstream_id uuid,
    event_type character varying(50) NOT NULL,
    event_data jsonb DEFAULT '{}'::jsonb,
    actor_id uuid,
    actor_type character varying(20) DEFAULT 'USER'::character varying,
    occurred_at timestamp with time zone DEFAULT now() NOT NULL,
    comment text
);


--
-- Name: TABLE case_events; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.case_events IS 'Audit log of all case activities';


--
-- Name: cases; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.cases (
    case_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    status character varying(30) DEFAULT 'INTAKE'::character varying NOT NULL,
    escalation_level character varying(30) DEFAULT 'STANDARD'::character varying NOT NULL,
    risk_rating character varying(20),
    assigned_analyst_id uuid,
    assigned_reviewer_id uuid,
    opened_at timestamp with time zone DEFAULT now() NOT NULL,
    closed_at timestamp with time zone,
    sla_deadline timestamp with time zone,
    last_activity_at timestamp with time zone DEFAULT now(),
    case_type character varying(30) DEFAULT 'NEW_CLIENT'::character varying,
    notes text,
    updated_at timestamp with time zone DEFAULT now(),
    service_context character varying(50),
    sponsor_cbu_id uuid,
    service_agreement_id uuid,
    kyc_standard character varying(50),
    subject_entity_id uuid,
    CONSTRAINT chk_case_status CHECK (((status)::text = ANY ((ARRAY['INTAKE'::character varying, 'DISCOVERY'::character varying, 'ASSESSMENT'::character varying, 'REVIEW'::character varying, 'APPROVED'::character varying, 'REJECTED'::character varying, 'BLOCKED'::character varying, 'WITHDRAWN'::character varying, 'EXPIRED'::character varying, 'REFER_TO_REGULATOR'::character varying, 'DO_NOT_ONBOARD'::character varying])::text[]))),
    CONSTRAINT chk_case_type CHECK (((case_type)::text = ANY ((ARRAY['NEW_CLIENT'::character varying, 'PERIODIC_REVIEW'::character varying, 'EVENT_DRIVEN'::character varying, 'REMEDIATION'::character varying])::text[]))),
    CONSTRAINT chk_escalation_level CHECK (((escalation_level)::text = ANY ((ARRAY['STANDARD'::character varying, 'SENIOR_COMPLIANCE'::character varying, 'EXECUTIVE'::character varying, 'BOARD'::character varying])::text[]))),
    CONSTRAINT chk_risk_rating CHECK (((risk_rating IS NULL) OR ((risk_rating)::text = ANY ((ARRAY['LOW'::character varying, 'MEDIUM'::character varying, 'HIGH'::character varying, 'VERY_HIGH'::character varying, 'PROHIBITED'::character varying])::text[]))))
);


--
-- Name: TABLE cases; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.cases IS 'KYC cases for client onboarding and periodic review';


--
-- Name: doc_request_acceptable_types; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.doc_request_acceptable_types (
    link_id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id uuid NOT NULL,
    document_type_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE doc_request_acceptable_types; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.doc_request_acceptable_types IS 'Document types that can satisfy a doc_request';


--
-- Name: doc_requests; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.doc_requests (
    request_id uuid DEFAULT gen_random_uuid() NOT NULL,
    workstream_id uuid NOT NULL,
    doc_type character varying(50) NOT NULL,
    status character varying(20) DEFAULT 'REQUIRED'::character varying NOT NULL,
    required_at timestamp with time zone DEFAULT now() NOT NULL,
    requested_at timestamp with time zone,
    due_date date,
    received_at timestamp with time zone,
    reviewed_at timestamp with time zone,
    verified_at timestamp with time zone,
    document_id uuid,
    reviewer_id uuid,
    rejection_reason text,
    verification_notes text,
    is_mandatory boolean DEFAULT true,
    priority character varying(10) DEFAULT 'NORMAL'::character varying,
    batch_id uuid,
    batch_reference character varying(50),
    generation_source character varying(30) DEFAULT 'MANUAL'::character varying,
    CONSTRAINT chk_doc_status CHECK (((status)::text = ANY ((ARRAY['DRAFT'::character varying, 'REQUIRED'::character varying, 'REQUESTED'::character varying, 'RECEIVED'::character varying, 'UNDER_REVIEW'::character varying, 'VERIFIED'::character varying, 'REJECTED'::character varying, 'WAIVED'::character varying, 'EXPIRED'::character varying])::text[])))
);


--
-- Name: TABLE doc_requests; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.doc_requests IS 'Document requirements and collection tracking';


--
-- Name: COLUMN doc_requests.batch_id; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.doc_requests.batch_id IS 'Groups doc_requests generated together';


--
-- Name: COLUMN doc_requests.batch_reference; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.doc_requests.batch_reference IS 'Human-readable batch reference (e.g., RFI-20241204-abc123)';


--
-- Name: COLUMN doc_requests.generation_source; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.doc_requests.generation_source IS 'How request was created: MANUAL, THRESHOLD, PERIODIC_REVIEW';


--
-- Name: entity_workstreams; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.entity_workstreams (
    workstream_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    status character varying(30) DEFAULT 'PENDING'::character varying NOT NULL,
    discovery_source_workstream_id uuid,
    discovery_reason character varying(100),
    risk_rating character varying(20),
    risk_factors jsonb DEFAULT '[]'::jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    blocked_at timestamp with time zone,
    blocked_reason text,
    requires_enhanced_dd boolean DEFAULT false,
    is_ubo boolean DEFAULT false,
    ownership_percentage numeric(5,2),
    discovery_depth integer DEFAULT 1,
    updated_at timestamp with time zone DEFAULT now(),
    blocker_type character varying(50),
    blocker_request_id uuid,
    blocker_message character varying(500),
    blocked_days_total integer DEFAULT 0,
    CONSTRAINT chk_workstream_status CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'COLLECT'::character varying, 'VERIFY'::character varying, 'SCREEN'::character varying, 'ASSESS'::character varying, 'COMPLETE'::character varying, 'BLOCKED'::character varying, 'ENHANCED_DD'::character varying, 'REFERRED'::character varying, 'PROHIBITED'::character varying])::text[]))),
    CONSTRAINT chk_ws_blocker_type CHECK (((blocker_type IS NULL) OR ((blocker_type)::text = ANY ((ARRAY['AWAITING_DOCUMENT'::character varying, 'AWAITING_INFORMATION'::character varying, 'AWAITING_VERIFICATION'::character varying, 'AWAITING_APPROVAL'::character varying, 'AWAITING_SIGNATURE'::character varying, 'SCREENING_HIT'::character varying, 'MANUAL_BLOCK'::character varying])::text[]))))
);


--
-- Name: TABLE entity_workstreams; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.entity_workstreams IS 'Per-entity work items within a KYC case';


--
-- Name: COLUMN entity_workstreams.blocker_type; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.entity_workstreams.blocker_type IS 'Type of blocker: AWAITING_DOCUMENT, AWAITING_VERIFICATION, etc.';


--
-- Name: COLUMN entity_workstreams.blocker_request_id; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.entity_workstreams.blocker_request_id IS 'FK to outstanding_requests if blocked by a pending request';


--
-- Name: COLUMN entity_workstreams.blocker_message; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.entity_workstreams.blocker_message IS 'Human-readable description of what is blocking progress';


--
-- Name: COLUMN entity_workstreams.blocked_days_total; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.entity_workstreams.blocked_days_total IS 'Cumulative days spent in BLOCKED status (for SLA tracking)';


--
-- Name: holdings; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.holdings (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    share_class_id uuid NOT NULL,
    investor_entity_id uuid NOT NULL,
    units numeric(20,6) DEFAULT 0 NOT NULL,
    cost_basis numeric(20,2),
    acquisition_date date,
    status character varying(50) DEFAULT 'active'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE holdings; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.holdings IS 'Investor positions (units held) in fund share classes';


--
-- Name: movements; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.movements (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    holding_id uuid NOT NULL,
    movement_type character varying(50) NOT NULL,
    units numeric(20,6) NOT NULL,
    price_per_unit numeric(20,6),
    amount numeric(20,2),
    currency character(3) DEFAULT 'EUR'::bpchar NOT NULL,
    trade_date date NOT NULL,
    settlement_date date,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    reference character varying(100),
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT movements_movement_type_check CHECK (((movement_type)::text = ANY ((ARRAY['subscription'::character varying, 'redemption'::character varying, 'transfer_in'::character varying, 'transfer_out'::character varying, 'dividend'::character varying, 'adjustment'::character varying])::text[]))),
    CONSTRAINT movements_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'confirmed'::character varying, 'settled'::character varying, 'cancelled'::character varying, 'failed'::character varying])::text[])))
);


--
-- Name: TABLE movements; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.movements IS 'Subscription, redemption, and transfer transactions';


--
-- Name: red_flags; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.red_flags (
    red_flag_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    workstream_id uuid,
    flag_type character varying(50) NOT NULL,
    severity character varying(20) NOT NULL,
    status character varying(20) DEFAULT 'OPEN'::character varying NOT NULL,
    description text NOT NULL,
    source character varying(50),
    source_reference text,
    raised_at timestamp with time zone DEFAULT now() NOT NULL,
    raised_by uuid,
    reviewed_at timestamp with time zone,
    reviewed_by uuid,
    resolved_at timestamp with time zone,
    resolved_by uuid,
    resolution_type character varying(30),
    resolution_notes text,
    waiver_approved_by uuid,
    waiver_justification text,
    CONSTRAINT chk_flag_severity CHECK (((severity)::text = ANY ((ARRAY['SOFT'::character varying, 'ESCALATE'::character varying, 'HARD_STOP'::character varying])::text[]))),
    CONSTRAINT chk_flag_status CHECK (((status)::text = ANY ((ARRAY['OPEN'::character varying, 'UNDER_REVIEW'::character varying, 'MITIGATED'::character varying, 'WAIVED'::character varying, 'BLOCKING'::character varying, 'CLOSED'::character varying])::text[])))
);


--
-- Name: TABLE red_flags; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.red_flags IS 'Risk indicators and issues found during KYC review';


--
-- Name: rule_executions; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.rule_executions (
    execution_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    workstream_id uuid,
    rule_name character varying(100) NOT NULL,
    trigger_event character varying(50) NOT NULL,
    condition_matched boolean NOT NULL,
    actions_executed jsonb DEFAULT '[]'::jsonb,
    context_snapshot jsonb DEFAULT '{}'::jsonb,
    executed_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: screenings; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.screenings (
    screening_id uuid DEFAULT gen_random_uuid() NOT NULL,
    workstream_id uuid NOT NULL,
    screening_type character varying(30) NOT NULL,
    provider character varying(50),
    status character varying(20) DEFAULT 'PENDING'::character varying NOT NULL,
    requested_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone,
    expires_at timestamp with time zone,
    result_summary character varying(100),
    result_data jsonb,
    match_count integer DEFAULT 0,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    review_notes text,
    red_flag_id uuid,
    CONSTRAINT chk_screening_status CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'RUNNING'::character varying, 'CLEAR'::character varying, 'HIT_PENDING_REVIEW'::character varying, 'HIT_CONFIRMED'::character varying, 'HIT_DISMISSED'::character varying, 'ERROR'::character varying, 'EXPIRED'::character varying])::text[]))),
    CONSTRAINT chk_screening_type CHECK (((screening_type)::text = ANY ((ARRAY['SANCTIONS'::character varying, 'PEP'::character varying, 'ADVERSE_MEDIA'::character varying, 'CREDIT'::character varying, 'CRIMINAL'::character varying, 'REGULATORY'::character varying, 'CONSOLIDATED'::character varying])::text[])))
);


--
-- Name: TABLE screenings; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.screenings IS 'Screening results from various providers';


--
-- Name: share_classes; Type: TABLE; Schema: kyc; Owner: -
--

CREATE TABLE kyc.share_classes (
    id uuid DEFAULT public.uuid_generate_v4() NOT NULL,
    cbu_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    isin character varying(12),
    currency character(3) DEFAULT 'EUR'::bpchar NOT NULL,
    nav_per_share numeric(20,6),
    nav_date date,
    management_fee_bps integer,
    performance_fee_bps integer,
    subscription_frequency character varying(50),
    redemption_frequency character varying(50),
    redemption_notice_days integer,
    minimum_investment numeric(20,2),
    status character varying(50) DEFAULT 'active'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    fund_type character varying(50),
    fund_structure character varying(50),
    investor_eligibility character varying(50),
    lock_up_period_months integer,
    gate_percentage numeric(5,2),
    high_water_mark boolean DEFAULT false,
    hurdle_rate numeric(5,2),
    entity_id uuid,
    class_category character varying(20) DEFAULT 'FUND'::character varying,
    issuer_entity_id uuid,
    CONSTRAINT chk_class_category CHECK (((class_category)::text = ANY ((ARRAY['CORPORATE'::character varying, 'FUND'::character varying])::text[])))
);


--
-- Name: TABLE share_classes; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.share_classes IS 'Fund share class master data with NAV, fees, and liquidity terms';


--
-- Name: COLUMN share_classes.fund_type; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.fund_type IS 'HEDGE_FUND, UCITS, AIFMD, etc.';


--
-- Name: COLUMN share_classes.fund_structure; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.fund_structure IS 'OPEN_ENDED, CLOSED_ENDED';


--
-- Name: COLUMN share_classes.investor_eligibility; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.investor_eligibility IS 'RETAIL, PROFESSIONAL, QUALIFIED';


--
-- Name: COLUMN share_classes.lock_up_period_months; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.lock_up_period_months IS 'Lock-up period for hedge funds';


--
-- Name: COLUMN share_classes.gate_percentage; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.gate_percentage IS 'Redemption gate percentage';


--
-- Name: COLUMN share_classes.high_water_mark; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.high_water_mark IS 'Performance fee uses high water mark';


--
-- Name: COLUMN share_classes.hurdle_rate; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.hurdle_rate IS 'Hurdle rate for performance fee';


--
-- Name: COLUMN share_classes.entity_id; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.entity_id IS 'The share class entity itself (optional - links to entity ontology)';


--
-- Name: COLUMN share_classes.class_category; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.class_category IS 'CORPORATE = company ownership shares, FUND = investment fund shares';


--
-- Name: COLUMN share_classes.issuer_entity_id; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.issuer_entity_id IS 'The sub-fund/fund entity that issues these shares';


--
-- Name: v_case_summary; Type: VIEW; Schema: kyc; Owner: -
--

CREATE VIEW kyc.v_case_summary AS
SELECT
    NULL::uuid AS case_id,
    NULL::uuid AS cbu_id,
    NULL::character varying(30) AS case_type,
    NULL::character varying(30) AS status,
    NULL::character varying(20) AS risk_rating,
    NULL::timestamp with time zone AS opened_at,
    NULL::timestamp with time zone AS sla_deadline,
    NULL::timestamp with time zone AS closed_at,
    NULL::bigint AS workstream_count,
    NULL::bigint AS completed_workstreams,
    NULL::bigint AS open_red_flags,
    NULL::bigint AS pending_docs;


--
-- Name: v_workstream_detail; Type: VIEW; Schema: kyc; Owner: -
--

CREATE VIEW kyc.v_workstream_detail AS
SELECT
    NULL::uuid AS workstream_id,
    NULL::uuid AS case_id,
    NULL::uuid AS entity_id,
    NULL::character varying(255) AS entity_name,
    NULL::character varying(255) AS entity_type,
    NULL::character varying(30) AS status,
    NULL::character varying(20) AS risk_rating,
    NULL::integer AS discovery_depth,
    NULL::boolean AS is_ubo,
    NULL::numeric(5,2) AS ownership_percentage,
    NULL::boolean AS requires_enhanced_dd,
    NULL::timestamp with time zone AS started_at,
    NULL::timestamp with time zone AS completed_at,
    NULL::bigint AS pending_docs,
    NULL::bigint AS pending_screenings,
    NULL::bigint AS open_flags;


--
-- Name: attribute_dictionary; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: attribute_observations; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".attribute_observations (
    observation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    value_text text,
    value_number numeric,
    value_boolean boolean,
    value_date date,
    value_datetime timestamp with time zone,
    value_json jsonb,
    source_type character varying(30) NOT NULL,
    source_document_id uuid,
    source_workstream_id uuid,
    source_screening_id uuid,
    source_reference text,
    source_metadata jsonb DEFAULT '{}'::jsonb,
    confidence numeric(3,2) DEFAULT 0.50,
    is_authoritative boolean DEFAULT false,
    extraction_method character varying(50),
    observed_at timestamp with time zone DEFAULT now() NOT NULL,
    observed_by text,
    status character varying(30) DEFAULT 'ACTIVE'::character varying,
    superseded_by uuid,
    superseded_at timestamp with time zone,
    effective_from date,
    effective_to date,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT attribute_observations_confidence_check CHECK (((confidence >= (0)::numeric) AND (confidence <= (1)::numeric))),
    CONSTRAINT check_obs_document_source CHECK ((((source_type)::text <> 'DOCUMENT'::text) OR (source_document_id IS NOT NULL))),
    CONSTRAINT check_obs_single_value CHECK (((((((
CASE
    WHEN (value_text IS NOT NULL) THEN 1
    ELSE 0
END +
CASE
    WHEN (value_number IS NOT NULL) THEN 1
    ELSE 0
END) +
CASE
    WHEN (value_boolean IS NOT NULL) THEN 1
    ELSE 0
END) +
CASE
    WHEN (value_date IS NOT NULL) THEN 1
    ELSE 0
END) +
CASE
    WHEN (value_datetime IS NOT NULL) THEN 1
    ELSE 0
END) +
CASE
    WHEN (value_json IS NOT NULL) THEN 1
    ELSE 0
END) = 1)),
    CONSTRAINT check_obs_source_type CHECK (((source_type)::text = ANY ((ARRAY['ALLEGATION'::character varying, 'DOCUMENT'::character varying, 'SCREENING'::character varying, 'THIRD_PARTY'::character varying, 'SYSTEM'::character varying, 'DERIVED'::character varying, 'MANUAL'::character varying])::text[]))),
    CONSTRAINT check_obs_status CHECK (((status)::text = ANY ((ARRAY['ACTIVE'::character varying, 'SUPERSEDED'::character varying, 'DISPUTED'::character varying, 'WITHDRAWN'::character varying, 'REJECTED'::character varying])::text[])))
);


--
-- Name: TABLE attribute_observations; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".attribute_observations IS 'Observation-based attribute storage. Multiple observations per attribute per entity, each with source provenance.';


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
    applicability jsonb DEFAULT '{}'::jsonb,
    embedding public.vector(1536),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone,
    domain character varying(100),
    is_required boolean DEFAULT false,
    default_value text,
    group_id character varying(100),
    reconciliation_rules jsonb DEFAULT '{}'::jsonb,
    acceptable_variation_threshold numeric(3,2),
    requires_authoritative_source boolean DEFAULT false,
    CONSTRAINT check_category CHECK ((category = ANY (ARRAY['identity'::text, 'financial'::text, 'compliance'::text, 'document'::text, 'risk'::text, 'contact'::text, 'address'::text, 'tax'::text, 'employment'::text, 'product'::text, 'entity'::text, 'ubo'::text, 'isda'::text, 'resource'::text, 'cbu'::text, 'trust'::text, 'fund'::text, 'partnership'::text]))),
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
-- Name: COLUMN attribute_registry.applicability; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".attribute_registry.applicability IS 'CSG applicability rules: entity_types[], required_for[], source_documents[], depends_on[]';


--
-- Name: COLUMN attribute_registry.reconciliation_rules; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".attribute_registry.reconciliation_rules IS 'Rules for comparing observations: {"allow_spelling_variation": true, "date_tolerance_days": 0}';


--
-- Name: COLUMN attribute_registry.acceptable_variation_threshold; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".attribute_registry.acceptable_variation_threshold IS 'Similarity threshold (0-1) for acceptable string variations';


--
-- Name: COLUMN attribute_registry.requires_authoritative_source; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".attribute_registry.requires_authoritative_source IS 'If true, at least one observation must be from an authoritative source';


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
-- Name: bods_entity_statements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".bods_entity_statements (
    statement_id character varying(100) NOT NULL,
    entity_type character varying(50),
    name text,
    jurisdiction character varying(10),
    lei character varying(20),
    company_number character varying(100),
    opencorporates_id character varying(200),
    identifiers jsonb,
    source_register character varying(100),
    statement_date date,
    source_url text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_entity_type CHECK (((entity_type)::text = ANY ((ARRAY['registeredEntity'::character varying, 'legalEntity'::character varying, 'arrangement'::character varying, 'anonymousEntity'::character varying, 'unknownEntity'::character varying, 'state'::character varying, 'stateBody'::character varying])::text[])))
);


--
-- Name: TABLE bods_entity_statements; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".bods_entity_statements IS 'BODS entity statements from beneficial ownership registers (UK PSC, etc.)';


--
-- Name: COLUMN bods_entity_statements.lei; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".bods_entity_statements.lei IS 'LEI identifier if present - join key to GLEIF data';


--
-- Name: bods_ownership_statements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".bods_ownership_statements (
    statement_id character varying(100) NOT NULL,
    subject_entity_statement_id character varying(100),
    subject_lei character varying(20),
    subject_name text,
    interested_party_type character varying(20),
    interested_party_statement_id character varying(100),
    interested_party_name text,
    ownership_type character varying(50),
    share_min numeric,
    share_max numeric,
    share_exact numeric,
    is_direct boolean,
    control_types character varying(50)[],
    start_date date,
    end_date date,
    source_register character varying(100),
    statement_date date,
    source_description text,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE bods_ownership_statements; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".bods_ownership_statements IS 'BODS ownership/control statements linking persons to entities';


--
-- Name: bods_person_statements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".bods_person_statements (
    statement_id character varying(100) NOT NULL,
    person_type character varying(50),
    full_name text,
    given_name character varying(200),
    family_name character varying(200),
    names jsonb,
    birth_date date,
    birth_date_precision character varying(20),
    death_date date,
    nationalities character varying(10)[],
    country_of_residence character varying(10),
    addresses jsonb,
    tax_residencies character varying(10)[],
    source_register character varying(100),
    statement_date date,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_person_type CHECK (((person_type)::text = ANY ((ARRAY['knownPerson'::character varying, 'anonymousPerson'::character varying, 'unknownPerson'::character varying])::text[])))
);


--
-- Name: TABLE bods_person_statements; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".bods_person_statements IS 'BODS person statements - natural persons who are UBOs';


--
-- Name: COLUMN bods_person_statements.birth_date_precision; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".bods_person_statements.birth_date_precision IS 'Precision of birth date: exact, month, or year';


--
-- Name: case_decision_thresholds; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".case_decision_thresholds (
    threshold_id uuid DEFAULT gen_random_uuid() NOT NULL,
    threshold_name character varying(100) NOT NULL,
    min_score integer,
    max_score integer,
    has_hard_stop boolean DEFAULT false,
    escalation_level character varying(30),
    recommended_action character varying(50) NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_recommended_action CHECK (((recommended_action)::text = ANY ((ARRAY['APPROVE'::character varying, 'APPROVE_WITH_CONDITIONS'::character varying, 'ESCALATE'::character varying, 'REFER_TO_REGULATOR'::character varying, 'DO_NOT_ONBOARD'::character varying, 'REJECT'::character varying])::text[])))
);


--
-- Name: TABLE case_decision_thresholds; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".case_decision_thresholds IS 'Thresholds mapping scores to recommended actions';


--
-- Name: case_evaluation_snapshots; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".case_evaluation_snapshots (
    snapshot_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    soft_count integer DEFAULT 0 NOT NULL,
    escalate_count integer DEFAULT 0 NOT NULL,
    hard_stop_count integer DEFAULT 0 NOT NULL,
    soft_score integer DEFAULT 0 NOT NULL,
    escalate_score integer DEFAULT 0 NOT NULL,
    has_hard_stop boolean DEFAULT false NOT NULL,
    total_score integer DEFAULT 0 NOT NULL,
    open_flags integer DEFAULT 0 NOT NULL,
    mitigated_flags integer DEFAULT 0 NOT NULL,
    waived_flags integer DEFAULT 0 NOT NULL,
    matched_threshold_id uuid,
    recommended_action character varying(50),
    required_escalation_level character varying(30),
    evaluated_at timestamp with time zone DEFAULT now() NOT NULL,
    evaluated_by character varying(255),
    notes text,
    decision_made character varying(50),
    decision_made_at timestamp with time zone,
    decision_made_by character varying(255),
    decision_notes text
);


--
-- Name: TABLE case_evaluation_snapshots; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".case_evaluation_snapshots IS 'Audit trail of case evaluations and decisions';


--
-- Name: case_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".case_types (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: cbu_change_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_change_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    change_type character varying(50) NOT NULL,
    field_name character varying(100),
    old_value jsonb,
    new_value jsonb,
    evidence_ids uuid[],
    changed_at timestamp with time zone DEFAULT now(),
    changed_by character varying(255),
    reason text,
    case_id uuid,
    CONSTRAINT chk_change_type CHECK (((change_type)::text = ANY ((ARRAY['STATUS_CHANGE'::character varying, 'FIELD_UPDATE'::character varying, 'EVIDENCE_ADDED'::character varying, 'EVIDENCE_VERIFIED'::character varying, 'ROLE_CHANGE'::character varying, 'UBO_CHANGE'::character varying, 'PRODUCT_CHANGE'::character varying])::text[])))
);


--
-- Name: TABLE cbu_change_log; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".cbu_change_log IS 'Audit trail of all CBU changes';


--
-- Name: cbu_relationship_verification; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_relationship_verification (
    verification_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    relationship_id uuid NOT NULL,
    alleged_percentage numeric(5,2),
    alleged_at timestamp with time zone,
    alleged_by uuid,
    allegation_source character varying(100),
    proof_document_id uuid,
    observed_percentage numeric(5,2),
    status character varying(20) DEFAULT 'unverified'::character varying NOT NULL,
    discrepancy_notes text,
    resolved_at timestamp with time zone,
    resolved_by uuid,
    resolution_notes text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_crv_status CHECK (((status)::text = ANY ((ARRAY['unverified'::character varying, 'alleged'::character varying, 'pending'::character varying, 'proven'::character varying, 'disputed'::character varying, 'waived'::character varying])::text[])))
);


--
-- Name: cbu_convergence_status; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".cbu_convergence_status AS
 SELECT cbu_id,
    count(*) AS total_relationships,
    count(*) FILTER (WHERE ((status)::text = 'proven'::text)) AS proven_count,
    count(*) FILTER (WHERE ((status)::text = 'alleged'::text)) AS alleged_count,
    count(*) FILTER (WHERE ((status)::text = 'pending'::text)) AS pending_count,
    count(*) FILTER (WHERE ((status)::text = 'disputed'::text)) AS disputed_count,
    count(*) FILTER (WHERE ((status)::text = 'unverified'::text)) AS unverified_count,
    count(*) FILTER (WHERE ((status)::text = 'waived'::text)) AS waived_count,
    (count(*) FILTER (WHERE ((status)::text = ANY ((ARRAY['proven'::character varying, 'waived'::character varying])::text[]))) = count(*)) AS is_converged
   FROM "ob-poc".cbu_relationship_verification
  GROUP BY cbu_id;


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
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    authority_limit numeric(18,2),
    authority_currency character varying(3) DEFAULT 'USD'::character varying,
    requires_co_signatory boolean DEFAULT false,
    target_entity_id uuid,
    ownership_percentage numeric(5,2),
    effective_from date,
    effective_to date,
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: cbu_entity_roles_history; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_entity_roles_history (
    history_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_entity_role_id uuid NOT NULL,
    cbu_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    role_id uuid NOT NULL,
    target_entity_id uuid,
    ownership_percentage numeric(5,2),
    effective_from date,
    effective_to date,
    created_at timestamp with time zone,
    updated_at timestamp with time zone,
    operation character varying(10) NOT NULL,
    changed_at timestamp with time zone DEFAULT now() NOT NULL,
    changed_by uuid
);


--
-- Name: cbu_evidence; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_evidence (
    evidence_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    document_id uuid,
    attestation_ref character varying(255),
    evidence_type character varying(50) NOT NULL,
    evidence_category character varying(50),
    description text,
    attached_at timestamp with time zone DEFAULT now(),
    attached_by character varying(255),
    verified_at timestamp with time zone,
    verified_by character varying(255),
    verification_status character varying(30) DEFAULT 'PENDING'::character varying,
    verification_notes text,
    CONSTRAINT chk_evidence_source CHECK (((document_id IS NOT NULL) OR (attestation_ref IS NOT NULL))),
    CONSTRAINT chk_evidence_type CHECK (((evidence_type)::text = ANY ((ARRAY['DOCUMENT'::character varying, 'ATTESTATION'::character varying, 'SCREENING'::character varying, 'REGISTRY_CHECK'::character varying, 'MANUAL_VERIFICATION'::character varying])::text[]))),
    CONSTRAINT chk_evidence_verification_status CHECK (((verification_status)::text = ANY ((ARRAY['PENDING'::character varying, 'VERIFIED'::character varying, 'REJECTED'::character varying, 'EXPIRED'::character varying])::text[])))
);


--
-- Name: TABLE cbu_evidence; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".cbu_evidence IS 'Evidence/documentation attached to CBUs for validation';


--
-- Name: cbu_layout_overrides; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_layout_overrides (
    cbu_id uuid NOT NULL,
    user_id uuid NOT NULL,
    view_mode text NOT NULL,
    positions jsonb DEFAULT '[]'::jsonb NOT NULL,
    sizes jsonb DEFAULT '[]'::jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: cbu_lifecycle_instances; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_lifecycle_instances (
    instance_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    resource_type_id uuid NOT NULL,
    instance_identifier character varying(255),
    instance_url character varying(500),
    market_id uuid,
    currency character varying(3),
    counterparty_entity_id uuid,
    status character varying(50) DEFAULT 'PENDING'::character varying,
    provider_code character varying(50),
    provider_account character varying(100),
    provider_bic character varying(11),
    config jsonb,
    depends_on_urls jsonb,
    provisioned_at timestamp with time zone,
    activated_at timestamp with time zone,
    suspended_at timestamp with time zone,
    decommissioned_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cbu_lifecycle_instances_status_check CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'PROVISIONING'::character varying, 'PROVISIONED'::character varying, 'ACTIVE'::character varying, 'SUSPENDED'::character varying, 'DECOMMISSIONED'::character varying])::text[])))
);


--
-- Name: TABLE cbu_lifecycle_instances; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".cbu_lifecycle_instances IS 'Provisioned lifecycle resources for a CBU (analogous to cbu_resource_instances)';


--
-- Name: cbu_matrix_product_overlay; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_matrix_product_overlay (
    overlay_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    instrument_class_id uuid,
    market_id uuid,
    currency character varying(3),
    counterparty_entity_id uuid,
    subscription_id uuid NOT NULL,
    additional_services jsonb DEFAULT '[]'::jsonb,
    additional_slas jsonb DEFAULT '[]'::jsonb,
    additional_resources jsonb DEFAULT '[]'::jsonb,
    product_specific_config jsonb DEFAULT '{}'::jsonb,
    status character varying(20) DEFAULT 'ACTIVE'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cbu_matrix_product_overlay_status_check CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'ACTIVE'::character varying, 'SUSPENDED'::character varying])::text[])))
);


--
-- Name: entity_relationships; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    from_entity_id uuid NOT NULL,
    to_entity_id uuid NOT NULL,
    relationship_type character varying(30) NOT NULL,
    percentage numeric(5,2),
    ownership_type character varying(30),
    control_type character varying(30),
    trust_role character varying(30),
    interest_type character varying(20),
    effective_from date,
    effective_to date,
    source character varying(100),
    source_document_ref character varying(255),
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    created_by uuid,
    updated_at timestamp with time zone DEFAULT now(),
    trust_interest_type character varying(30),
    trust_class_description text,
    is_regulated boolean DEFAULT true,
    regulatory_jurisdiction character varying(20),
    CONSTRAINT chk_er_no_self_reference CHECK ((from_entity_id <> to_entity_id)),
    CONSTRAINT chk_er_ownership_has_percentage CHECK ((((relationship_type)::text <> 'ownership'::text) OR (percentage IS NOT NULL))),
    CONSTRAINT chk_er_relationship_type CHECK (((relationship_type)::text = ANY ((ARRAY['ownership'::character varying, 'control'::character varying, 'trust_role'::character varying, 'employment'::character varying, 'management'::character varying])::text[]))),
    CONSTRAINT chk_er_temporal_valid CHECK (((effective_to IS NULL) OR (effective_from IS NULL) OR (effective_from <= effective_to)))
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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    type_code character varying(100),
    semantic_context jsonb DEFAULT '{}'::jsonb,
    parent_type_id uuid,
    type_hierarchy_path text[],
    embedding public.vector(768),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone,
    entity_category character varying(20),
    deprecated boolean DEFAULT false,
    deprecation_note text
);


--
-- Name: COLUMN entity_types.semantic_context; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_types.semantic_context IS 'Rich semantic metadata: category, parent_type, synonyms[], typical_documents[], typical_attributes[]';


--
-- Name: COLUMN entity_types.type_hierarchy_path; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_types.type_hierarchy_path IS 'Materialized path for efficient ancestor queries, e.g., ["ENTITY", "LEGAL_ENTITY", "LIMITED_COMPANY"]';


--
-- Name: cbu_ownership_graph; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".cbu_ownership_graph AS
 SELECT v.cbu_id,
    r.relationship_id,
    r.from_entity_id,
    e_from.name AS from_entity_name,
    et_from.entity_category AS from_entity_category,
    r.to_entity_id,
    e_to.name AS to_entity_name,
    et_to.entity_category AS to_entity_category,
    r.relationship_type,
    r.percentage,
    r.ownership_type,
    r.control_type,
    r.trust_role,
    r.interest_type,
    v.status AS verification_status,
    v.alleged_percentage,
    v.observed_percentage,
    v.proof_document_id,
    r.effective_from,
    r.effective_to
   FROM ((((("ob-poc".entity_relationships r
     JOIN "ob-poc".cbu_relationship_verification v ON ((v.relationship_id = r.relationship_id)))
     LEFT JOIN "ob-poc".entities e_from ON ((e_from.entity_id = r.from_entity_id)))
     LEFT JOIN "ob-poc".entity_types et_from ON ((et_from.entity_type_id = e_from.entity_type_id)))
     LEFT JOIN "ob-poc".entities e_to ON ((e_to.entity_id = r.to_entity_id)))
     LEFT JOIN "ob-poc".entity_types et_to ON ((et_to.entity_type_id = e_to.entity_type_id)))
  WHERE ((r.effective_to IS NULL) OR (r.effective_to > CURRENT_DATE));


--
-- Name: cbu_product_subscriptions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_product_subscriptions (
    subscription_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    product_id uuid NOT NULL,
    status character varying(20) DEFAULT 'ACTIVE'::character varying NOT NULL,
    effective_from date DEFAULT CURRENT_DATE NOT NULL,
    effective_to date,
    config jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT cbu_product_subscriptions_status_check CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'ACTIVE'::character varying, 'SUSPENDED'::character varying, 'TERMINATED'::character varying])::text[])))
);


--
-- Name: cbu_resource_instances; Type: TABLE; Schema: ob-poc; Owner: -
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
    market_id uuid,
    currency character varying(3),
    counterparty_entity_id uuid,
    provider_code character varying(50),
    provider_config jsonb,
    CONSTRAINT cbu_resource_instances_status_check CHECK (((status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('PROVISIONING'::character varying)::text, ('ACTIVE'::character varying)::text, ('SUSPENDED'::character varying)::text, ('DECOMMISSIONED'::character varying)::text])))
);


--
-- Name: TABLE cbu_resource_instances; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".cbu_resource_instances IS 'Production resource instances - the actual delivered artifacts for a CBU (accounts, connections, platform access)';


--
-- Name: COLUMN cbu_resource_instances.instance_url; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.instance_url IS 'Unique URL/endpoint for this resource instance (e.g., https://custody.bank.com/accounts/ABC123)';


--
-- Name: cbu_service_contexts; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_service_contexts (
    cbu_id uuid NOT NULL,
    service_context character varying(50) NOT NULL,
    effective_date date DEFAULT CURRENT_DATE
);


--
-- Name: cbu_sla_commitments; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_sla_commitments (
    commitment_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    profile_id uuid,
    template_id uuid,
    override_target_value numeric(10,4),
    override_warning_threshold numeric(10,4),
    bound_service_id uuid,
    bound_resource_instance_id uuid,
    bound_isda_id uuid,
    bound_csa_id uuid,
    scope_instrument_classes text[],
    scope_markets text[],
    scope_currencies text[],
    scope_counterparties uuid[],
    penalty_structure jsonb,
    incentive_structure jsonb,
    effective_date date DEFAULT CURRENT_DATE NOT NULL,
    termination_date date,
    status character varying(20) DEFAULT 'ACTIVE'::character varying,
    negotiated_by character varying(255),
    negotiated_date date,
    source_document_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_sla_status CHECK (((status)::text = ANY ((ARRAY['DRAFT'::character varying, 'ACTIVE'::character varying, 'SUSPENDED'::character varying, 'TERMINATED'::character varying])::text[])))
);


--
-- Name: TABLE cbu_sla_commitments; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".cbu_sla_commitments IS 'CBU-specific SLA commitments. Links trading profile sections, service resources,
and ISDA/CSA agreements to measurable SLA targets.';


--
-- Name: cbu_trading_profiles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".cbu_trading_profiles (
    profile_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    status character varying(20) DEFAULT 'DRAFT'::character varying NOT NULL,
    document jsonb NOT NULL,
    document_hash text NOT NULL,
    created_by character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    activated_at timestamp with time zone,
    activated_by character varying(255),
    notes text,
    source_document_id uuid,
    materialization_status character varying(20) DEFAULT 'PENDING'::character varying,
    materialized_at timestamp with time zone,
    materialization_hash text,
    sla_profile_id uuid,
    validated_at timestamp with time zone,
    validated_by character varying(255),
    submitted_at timestamp with time zone,
    submitted_by character varying(255),
    rejected_at timestamp with time zone,
    rejected_by character varying(255),
    rejection_reason text,
    superseded_at timestamp with time zone,
    superseded_by_version integer,
    CONSTRAINT cbu_trading_profiles_status_check CHECK (((status)::text = ANY ((ARRAY['DRAFT'::character varying, 'VALIDATED'::character varying, 'PENDING_REVIEW'::character varying, 'ACTIVE'::character varying, 'SUPERSEDED'::character varying, 'ARCHIVED'::character varying])::text[])))
);


--
-- Name: TABLE cbu_trading_profiles; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".cbu_trading_profiles IS 'Versioned trading profile documents - single source of truth for CBU trading configuration.
Documents are materialized to operational tables (cbu_ssi, ssi_booking_rules, etc.) via the
trading-profile.materialize verb.';


--
-- Name: COLUMN cbu_trading_profiles.document; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.document IS 'JSONB document containing: universe, investment_managers, isda_agreements, settlement_config,
booking_rules, standing_instructions, pricing_matrix, valuation_config, constraints';


--
-- Name: COLUMN cbu_trading_profiles.document_hash; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.document_hash IS 'SHA-256 hash of document for change detection and idempotency';


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
    source_of_funds text,
    client_type character varying(100),
    jurisdiction character varying(50),
    risk_context jsonb DEFAULT '{}'::jsonb,
    onboarding_context jsonb DEFAULT '{}'::jsonb,
    semantic_context jsonb DEFAULT '{}'::jsonb,
    embedding public.vector(1536),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone,
    commercial_client_entity_id uuid,
    cbu_category character varying(50),
    product_id uuid,
    status character varying(30) DEFAULT 'DISCOVERED'::character varying,
    kyc_scope_template character varying(50),
    CONSTRAINT chk_cbu_category CHECK (((cbu_category IS NULL) OR ((cbu_category)::text = ANY ((ARRAY['FUND_MANDATE'::character varying, 'CORPORATE_GROUP'::character varying, 'INSTITUTIONAL_ACCOUNT'::character varying, 'RETAIL_CLIENT'::character varying, 'FAMILY_TRUST'::character varying, 'CORRESPONDENT_BANK'::character varying, 'INTERNAL_TEST'::character varying])::text[])))),
    CONSTRAINT chk_cbu_status CHECK (((status)::text = ANY ((ARRAY['DISCOVERED'::character varying, 'VALIDATION_PENDING'::character varying, 'VALIDATED'::character varying, 'UPDATE_PENDING_PROOF'::character varying, 'VALIDATION_FAILED'::character varying])::text[])))
);


--
-- Name: COLUMN cbus.risk_context; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbus.risk_context IS 'Risk-related context: risk_rating, pep_exposure, sanctions_exposure, industry_codes[]';


--
-- Name: COLUMN cbus.onboarding_context; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbus.onboarding_context IS 'Onboarding state: stage, completed_steps[], pending_requirements[], override_rules[]';


--
-- Name: COLUMN cbus.semantic_context; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbus.semantic_context IS 'Rich semantic metadata: business_description, industry_keywords[], related_entities[]';


--
-- Name: COLUMN cbus.commercial_client_entity_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbus.commercial_client_entity_id IS 'Head office entity that contracted with the bank (e.g., Blackrock Inc). Convenience field - actual ownership is in holdings chain.';


--
-- Name: COLUMN cbus.cbu_category; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".cbus.cbu_category IS 'Template discriminator for visualization layout: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST, CORRESPONDENT_BANK, INTERNAL_TEST';


--
-- Name: client_allegations; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".client_allegations (
    allegation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    workstream_id uuid,
    entity_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    alleged_value jsonb NOT NULL,
    alleged_value_display text,
    alleged_at timestamp with time zone DEFAULT now() NOT NULL,
    alleged_by text,
    allegation_source character varying(50) NOT NULL,
    allegation_reference text,
    verification_status character varying(30) DEFAULT 'PENDING'::character varying,
    verified_by_observation_id uuid,
    verification_result character varying(30),
    verification_notes text,
    verified_at timestamp with time zone,
    verified_by text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT check_allegation_source CHECK (((allegation_source)::text = ANY ((ARRAY['ONBOARDING_FORM'::character varying, 'KYC_QUESTIONNAIRE'::character varying, 'EMAIL'::character varying, 'VERBAL'::character varying, 'API'::character varying, 'DOCUMENT'::character varying, 'PRIOR_CASE'::character varying])::text[]))),
    CONSTRAINT check_verification_result CHECK (((verification_result IS NULL) OR ((verification_result)::text = ANY ((ARRAY['EXACT_MATCH'::character varying, 'ACCEPTABLE_VARIATION'::character varying, 'MATERIAL_DISCREPANCY'::character varying, 'CONTRADICTION'::character varying, 'INCONCLUSIVE'::character varying])::text[])))),
    CONSTRAINT check_verification_status CHECK (((verification_status)::text = ANY ((ARRAY['PENDING'::character varying, 'IN_PROGRESS'::character varying, 'VERIFIED'::character varying, 'CONTRADICTED'::character varying, 'PARTIAL'::character varying, 'UNVERIFIABLE'::character varying, 'WAIVED'::character varying])::text[])))
);


--
-- Name: TABLE client_allegations; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".client_allegations IS 'Client allegations - the unverified claims that form the starting point of KYC verification.';


--
-- Name: client_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".client_types (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0,
    CONSTRAINT client_type_code_uppercase CHECK (((code)::text = upper((code)::text)))
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
    CONSTRAINT crud_operations_asset_type_check CHECK (((asset_type)::text = ANY (ARRAY['CBU'::text, 'ENTITY'::text, 'PARTNERSHIP'::text, 'LIMITED_COMPANY'::text, 'PROPER_PERSON'::text, 'TRUST'::text, 'ATTRIBUTE'::text, 'DOCUMENT'::text, 'CBU_ENTITY_ROLE'::text, 'OWNERSHIP'::text, 'DOCUMENT_REQUEST'::text, 'DOCUMENT_LINK'::text, 'INVESTIGATION'::text, 'RISK_ASSESSMENT_CBU'::text, 'RISK_RATING'::text, 'SCREENING_RESULT'::text, 'SCREENING_HIT_RESOLUTION'::text, 'SCREENING_BATCH'::text, 'DECISION'::text, 'DECISION_CONDITION'::text, 'MONITORING_CASE'::text, 'MONITORING_REVIEW'::text, 'MONITORING_ALERT_RULE'::text, 'MONITORING_ACTIVITY'::text, 'ATTRIBUTE_VALUE'::text, 'ATTRIBUTE_VALIDATION'::text]))),
    CONSTRAINT crud_operations_execution_status_check CHECK (((execution_status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('EXECUTING'::character varying)::text, ('COMPLETED'::character varying)::text, ('FAILED'::character varying)::text, ('ROLLED_BACK'::character varying)::text]))),
    CONSTRAINT crud_operations_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
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
-- Name: csg_validation_rules; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: currencies; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".currencies (
    currency_id uuid DEFAULT gen_random_uuid() NOT NULL,
    iso_code character varying(3) NOT NULL,
    name character varying(100) NOT NULL,
    symbol character varying(10),
    decimal_places integer DEFAULT 2,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT currency_code_uppercase CHECK (((iso_code)::text = upper((iso_code)::text)))
);


--
-- Name: delegation_relationships; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".delegation_relationships (
    delegation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    delegator_entity_id uuid NOT NULL,
    delegate_entity_id uuid NOT NULL,
    delegation_scope text NOT NULL,
    delegation_description text,
    applies_to_cbu_id uuid,
    regulatory_notification_date date,
    regulatory_approval_required boolean DEFAULT false,
    regulatory_approval_date date,
    contract_doc_id uuid,
    effective_from date NOT NULL,
    effective_to date,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: detected_patterns; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".detected_patterns (
    pattern_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    pattern_type character varying(50) NOT NULL,
    severity character varying(20) NOT NULL,
    description text NOT NULL,
    involved_entities uuid[] NOT NULL,
    evidence jsonb,
    status character varying(20) DEFAULT 'DETECTED'::character varying NOT NULL,
    detected_at timestamp with time zone DEFAULT now() NOT NULL,
    resolved_at timestamp with time zone,
    resolved_by character varying(100),
    resolution_notes text,
    CONSTRAINT detected_patterns_pattern_type_check CHECK (((pattern_type)::text = ANY ((ARRAY['CIRCULAR_OWNERSHIP'::character varying, 'LAYERING'::character varying, 'NOMINEE_USAGE'::character varying, 'OPACITY_JURISDICTION'::character varying, 'REGISTRY_MISMATCH'::character varying, 'OWNERSHIP_GAPS'::character varying, 'RECENT_RESTRUCTURING'::character varying, 'ROLE_CONCENTRATION'::character varying])::text[]))),
    CONSTRAINT detected_patterns_severity_check CHECK (((severity)::text = ANY ((ARRAY['INFO'::character varying, 'LOW'::character varying, 'MEDIUM'::character varying, 'HIGH'::character varying, 'CRITICAL'::character varying])::text[]))),
    CONSTRAINT detected_patterns_status_check CHECK (((status)::text = ANY ((ARRAY['DETECTED'::character varying, 'INVESTIGATING'::character varying, 'RESOLVED'::character varying, 'FALSE_POSITIVE'::character varying])::text[])))
);


--
-- Name: TABLE detected_patterns; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".detected_patterns IS 'Audit trail for adversarial pattern detection (circular ownership, layering, nominee usage, etc.)';


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
-- Name: document_attribute_links; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_attribute_links (
    link_id uuid DEFAULT gen_random_uuid() NOT NULL,
    document_type_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    direction character varying(10) NOT NULL,
    extraction_method character varying(50),
    extraction_field_path jsonb,
    extraction_confidence_default numeric(3,2) DEFAULT 0.80,
    extraction_hints jsonb DEFAULT '{}'::jsonb,
    is_authoritative boolean DEFAULT false,
    proof_strength character varying(20),
    alternative_doc_types uuid[],
    entity_types text[],
    jurisdictions text[],
    client_types text[],
    is_active boolean DEFAULT true,
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT check_dal_direction CHECK (((direction)::text = ANY ((ARRAY['SOURCE'::character varying, 'SINK'::character varying, 'BOTH'::character varying])::text[]))),
    CONSTRAINT check_dal_extraction_config CHECK ((((direction)::text = 'SINK'::text) OR (extraction_method IS NOT NULL))),
    CONSTRAINT check_dal_proof_strength CHECK (((proof_strength IS NULL) OR ((proof_strength)::text = ANY ((ARRAY['PRIMARY'::character varying, 'SECONDARY'::character varying, 'SUPPORTING'::character varying])::text[]))))
);


--
-- Name: TABLE document_attribute_links; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".document_attribute_links IS 'Bidirectional links between document types and attributes. SOURCE = document provides attribute value. SINK = attribute requires document as proof.';


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
    CONSTRAINT document_attribute_mappings_extraction_method_check CHECK (((extraction_method)::text = ANY (ARRAY[('OCR'::character varying)::text, ('MRZ'::character varying)::text, ('BARCODE'::character varying)::text, ('QR_CODE'::character varying)::text, ('FORM_FIELD'::character varying)::text, ('TABLE'::character varying)::text, ('CHECKBOX'::character varying)::text, ('SIGNATURE'::character varying)::text, ('PHOTO'::character varying)::text, ('NLP'::character varying)::text, ('AI'::character varying)::text])))
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
    metadata jsonb DEFAULT '{}'::jsonb,
    entity_id uuid
);


--
-- Name: TABLE document_catalog; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".document_catalog IS 'Central "fact" table for all document instances. Stores file info and AI extraction results.';


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
    updated_at timestamp with time zone DEFAULT now(),
    applicability jsonb DEFAULT '{}'::jsonb,
    semantic_context jsonb DEFAULT '{}'::jsonb,
    embedding public.vector(768),
    embedding_model character varying(100),
    embedding_updated_at timestamp with time zone,
    CONSTRAINT document_type_code_uppercase CHECK (((type_code)::text = upper((type_code)::text)))
);


--
-- Name: COLUMN document_types.applicability; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".document_types.applicability IS 'CSG applicability rules: entity_types[], jurisdictions[], client_types[], required_for[], excludes[]';


--
-- Name: COLUMN document_types.semantic_context; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".document_types.semantic_context IS 'Rich semantic metadata: purpose, synonyms[], related_documents[], extraction_hints{}, keywords[]';


--
-- Name: COLUMN document_types.embedding; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".document_types.embedding IS 'OpenAI ada-002 or equivalent embedding of type description + semantic context';


--
-- Name: document_validity_rules; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".document_validity_rules (
    rule_id uuid DEFAULT gen_random_uuid() NOT NULL,
    document_type_id uuid NOT NULL,
    rule_type character varying(50) NOT NULL,
    rule_value integer,
    rule_unit character varying(20),
    rule_parameters jsonb,
    applies_to_jurisdictions text[],
    applies_to_entity_types text[],
    warning_days integer DEFAULT 30,
    is_hard_requirement boolean DEFAULT true,
    regulatory_source character varying(200),
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT document_validity_rules_rule_type_check CHECK (((rule_type)::text = ANY ((ARRAY['MAX_AGE_DAYS'::character varying, 'MAX_AGE_MONTHS'::character varying, 'CHECK_EXPIRY'::character varying, 'MIN_REMAINING_VALIDITY'::character varying, 'ANNUAL_RENEWAL'::character varying, 'VALIDITY_YEARS'::character varying, 'EXPIRES_YEAR_END'::character varying, 'NO_EXPIRY'::character varying, 'SUPERSEDED_BY_EVENT'::character varying])::text[]))),
    CONSTRAINT document_validity_rules_rule_unit_check CHECK (((rule_unit)::text = ANY ((ARRAY['days'::character varying, 'months'::character varying, 'years'::character varying])::text[])))
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
    CONSTRAINT dsl_examples_asset_type_check CHECK (((asset_type)::text = ANY (ARRAY['CBU'::text, 'ENTITY'::text, 'PARTNERSHIP'::text, 'LIMITED_COMPANY'::text, 'PROPER_PERSON'::text, 'TRUST'::text, 'ATTRIBUTE'::text, 'DOCUMENT'::text, 'CBU_ENTITY_ROLE'::text, 'OWNERSHIP'::text, 'DOCUMENT_REQUEST'::text, 'DOCUMENT_LINK'::text, 'INVESTIGATION'::text, 'RISK_ASSESSMENT_CBU'::text, 'RISK_RATING'::text, 'SCREENING_RESULT'::text, 'SCREENING_HIT_RESOLUTION'::text, 'SCREENING_BATCH'::text, 'DECISION'::text, 'DECISION_CONDITION'::text, 'MONITORING_CASE'::text, 'MONITORING_REVIEW'::text, 'MONITORING_ALERT_RULE'::text, 'MONITORING_ACTIVITY'::text, 'ATTRIBUTE_VALUE'::text, 'ATTRIBUTE_VALIDATION'::text]))),
    CONSTRAINT dsl_examples_complexity_level_check CHECK (((complexity_level)::text = ANY (ARRAY[('SIMPLE'::character varying)::text, ('MEDIUM'::character varying)::text, ('COMPLEX'::character varying)::text]))),
    CONSTRAINT dsl_examples_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
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
END) STORED,
    verb_hashes bytea[],
    verb_names text[]
);


--
-- Name: COLUMN dsl_execution_log.verb_hashes; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_execution_log.verb_hashes IS 'Array of compiled_hash values (SHA256) for verbs used in this execution. Links to dsl_verbs.compiled_hash for audit trail.';


--
-- Name: COLUMN dsl_execution_log.verb_names; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_execution_log.verb_names IS 'Array of verb names (domain.verb) used in this execution. Parallel to verb_hashes for readability.';


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
-- Name: dsl_generation_log; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE dsl_generation_log; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_generation_log IS 'Captures agent DSL generation iterations for training data extraction and audit trail';


--
-- Name: COLUMN dsl_generation_log.user_intent; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.user_intent IS 'Natural language description of what user wanted - the input side of training pairs';


--
-- Name: COLUMN dsl_generation_log.final_valid_dsl; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.final_valid_dsl IS 'Successfully validated DSL - the output side of training pairs';


--
-- Name: COLUMN dsl_generation_log.iterations; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.iterations IS 'JSONB array of each generation attempt with prompts, responses, and validation results';


--
-- Name: COLUMN dsl_generation_log.domain_name; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.domain_name IS 'Primary domain for this generation: cbu, entity, document, etc.';


--
-- Name: COLUMN dsl_generation_log.model_used; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.model_used IS 'LLM model identifier used for generation';


--
-- Name: COLUMN dsl_generation_log.total_attempts; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.total_attempts IS 'Number of generation attempts before success or failure';


--
-- Name: COLUMN dsl_generation_log.success; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_generation_log.success IS 'Whether generation ultimately succeeded';


--
-- Name: dsl_graph_contexts; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_graph_contexts (
    context_code text NOT NULL,
    label text NOT NULL,
    description text,
    priority integer DEFAULT 50,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE dsl_graph_contexts; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_graph_contexts IS 'Graph cursor context reference data';


--
-- Name: dsl_idempotency; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_idempotency (
    idempotency_key text NOT NULL,
    execution_id uuid NOT NULL,
    statement_index integer NOT NULL,
    verb text NOT NULL,
    args_hash text NOT NULL,
    result_type text NOT NULL,
    result_id uuid,
    result_json jsonb,
    result_affected bigint,
    created_at timestamp with time zone DEFAULT now(),
    verb_hash bytea,
    input_view_state jsonb,
    input_selection uuid[] DEFAULT '{}'::uuid[],
    output_view_state jsonb,
    source character varying(30) DEFAULT 'unknown'::character varying,
    request_id uuid,
    actor_id uuid,
    actor_type character varying(20) DEFAULT 'user'::character varying
);


--
-- Name: COLUMN dsl_idempotency.verb_hash; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.verb_hash IS 'SHA256 compiled_hash of the verb config used for this execution. Links to dsl_verbs.compiled_hash.';


--
-- Name: COLUMN dsl_idempotency.input_view_state; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.input_view_state IS 'View state snapshot before execution - what selection was targeted';


--
-- Name: COLUMN dsl_idempotency.input_selection; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.input_selection IS 'Selection array before execution - entities affected by batch ops';


--
-- Name: COLUMN dsl_idempotency.output_view_state; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.output_view_state IS 'View state snapshot after execution - result of view.* operations';


--
-- Name: COLUMN dsl_idempotency.source; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.source IS 'Origin of execution: api, cli, mcp, repl, batch, test, migration';


--
-- Name: COLUMN dsl_idempotency.request_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.request_id IS 'Correlation ID for distributed tracing - groups related executions';


--
-- Name: COLUMN dsl_idempotency.actor_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.actor_id IS 'ID of user or system that initiated this execution';


--
-- Name: COLUMN dsl_idempotency.actor_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_idempotency.actor_type IS 'Type of actor: user, system, agent, service';


--
-- Name: dsl_instance_versions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_instance_versions (
    version_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    version_number integer NOT NULL,
    dsl_content text NOT NULL,
    operation_type character varying(100) NOT NULL,
    compilation_status character varying(50) DEFAULT 'COMPILED'::character varying,
    ast_json jsonb,
    created_at timestamp with time zone DEFAULT now(),
    unresolved_count integer DEFAULT 0,
    total_refs integer DEFAULT 0
);


--
-- Name: COLUMN dsl_instance_versions.compilation_status; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_instance_versions.compilation_status IS 'PARSED = syntax OK, needs resolution; PARTIAL = some resolved; RESOLVED = all resolved; EXECUTED = has been run; FAILED = execution failed';


--
-- Name: dsl_instances; Type: TABLE; Schema: ob-poc; Owner: -
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
-- Name: dsl_session_events; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_session_events (
    event_id uuid DEFAULT gen_random_uuid() NOT NULL,
    session_id uuid NOT NULL,
    event_type character varying(30) NOT NULL,
    dsl_source text,
    error_message text,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    occurred_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT dsl_session_events_event_type_check CHECK (((event_type)::text = ANY (ARRAY['created'::text, 'execute_started'::text, 'execute_success'::text, 'execute_failed'::text, 'validation_error'::text, 'timeout'::text, 'aborted'::text, 'expired'::text, 'completed'::text, 'binding_added'::text, 'domain_detected'::text, 'error_recovered'::text, 'parsed'::text, 'resolving'::text])))
);


--
-- Name: dsl_session_locks; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_session_locks (
    session_id uuid NOT NULL,
    locked_at timestamp with time zone DEFAULT now() NOT NULL,
    lock_timeout_at timestamp with time zone DEFAULT (now() + '00:00:30'::interval) NOT NULL,
    operation character varying(50) NOT NULL
);


--
-- Name: dsl_sessions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_sessions (
    session_id uuid DEFAULT gen_random_uuid() NOT NULL,
    status character varying(20) DEFAULT 'active'::character varying NOT NULL,
    primary_domain character varying(30),
    cbu_id uuid,
    kyc_case_id uuid,
    onboarding_request_id uuid,
    named_refs jsonb DEFAULT '{}'::jsonb NOT NULL,
    client_type character varying(50),
    jurisdiction character varying(10),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_activity_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '24:00:00'::interval) NOT NULL,
    completed_at timestamp with time zone,
    error_count integer DEFAULT 0 NOT NULL,
    last_error text,
    last_error_at timestamp with time zone,
    current_view_state jsonb,
    view_updated_at timestamp with time zone,
    CONSTRAINT dsl_sessions_status_check CHECK (((status)::text = ANY ((ARRAY['active'::character varying, 'completed'::character varying, 'aborted'::character varying, 'expired'::character varying, 'error'::character varying])::text[])))
);


--
-- Name: COLUMN dsl_sessions.current_view_state; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_sessions.current_view_state IS 'Current view state for session - enables session restore with full context';


--
-- Name: COLUMN dsl_sessions.view_updated_at; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_sessions.view_updated_at IS 'When view state was last updated';


--
-- Name: dsl_snapshots; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_snapshots (
    snapshot_id uuid DEFAULT gen_random_uuid() NOT NULL,
    session_id uuid NOT NULL,
    version integer NOT NULL,
    dsl_source text NOT NULL,
    dsl_checksum character varying(64) NOT NULL,
    success boolean DEFAULT true NOT NULL,
    bindings_captured jsonb DEFAULT '{}'::jsonb NOT NULL,
    entities_created jsonb DEFAULT '[]'::jsonb NOT NULL,
    domains_used text[] DEFAULT '{}'::text[] NOT NULL,
    executed_at timestamp with time zone DEFAULT now() NOT NULL,
    execution_ms integer
);


--
-- Name: dsl_verb_categories; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_verb_categories (
    category_code text NOT NULL,
    label text NOT NULL,
    description text,
    display_order integer DEFAULT 100,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE dsl_verb_categories; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_verb_categories IS 'Verb category reference data for grouping';


--
-- Name: dsl_verb_sync_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_verb_sync_log (
    sync_id uuid DEFAULT gen_random_uuid() NOT NULL,
    synced_at timestamp with time zone DEFAULT now() NOT NULL,
    verbs_added integer DEFAULT 0 NOT NULL,
    verbs_updated integer DEFAULT 0 NOT NULL,
    verbs_unchanged integer DEFAULT 0 NOT NULL,
    verbs_removed integer DEFAULT 0 NOT NULL,
    source_hash text,
    duration_ms integer,
    error_message text
);


--
-- Name: dsl_verbs; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_verbs (
    verb_id uuid DEFAULT gen_random_uuid() NOT NULL,
    domain text NOT NULL,
    verb_name text NOT NULL,
    full_name text GENERATED ALWAYS AS (((domain || '.'::text) || verb_name)) STORED,
    description text,
    behavior text DEFAULT 'crud'::text NOT NULL,
    category text,
    search_text text,
    intent_patterns text[],
    workflow_phases text[],
    graph_contexts text[],
    example_short text,
    example_dsl text,
    typical_next text[],
    produces_type text,
    produces_subtype text,
    consumes jsonb DEFAULT '[]'::jsonb,
    lifecycle_entity_arg text,
    requires_states text[],
    transitions_to text,
    source text DEFAULT 'yaml'::text NOT NULL,
    yaml_hash text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    compiled_json jsonb,
    effective_config_json jsonb,
    diagnostics_json jsonb DEFAULT '{"errors": [], "warnings": []}'::jsonb,
    compiled_hash bytea,
    compiler_version character varying(50),
    compiled_at timestamp with time zone
);


--
-- Name: TABLE dsl_verbs; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_verbs IS 'DSL verb definitions synced from YAML, with RAG metadata for agent discovery';


--
-- Name: COLUMN dsl_verbs.compiled_json; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_json IS 'Full RuntimeVerb serialized as JSON - the complete compiled contract';


--
-- Name: COLUMN dsl_verbs.effective_config_json; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_verbs.effective_config_json IS 'Expanded configuration with all defaults applied (for debugging)';


--
-- Name: COLUMN dsl_verbs.diagnostics_json; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_verbs.diagnostics_json IS 'Compilation diagnostics: {"errors": [...], "warnings": [...]}';


--
-- Name: COLUMN dsl_verbs.compiled_hash; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_hash IS 'SHA256 of canonical compiled_json for integrity verification';


--
-- Name: COLUMN dsl_verbs.compiler_version; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiler_version IS 'Semantic version of the DSL compiler that generated compiled_json (e.g., 0.1.0)';


--
-- Name: COLUMN dsl_verbs.compiled_at; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_at IS 'Timestamp when compiled_json was last generated (NULL if never compiled)';


--
-- Name: dsl_view_state_changes; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_view_state_changes (
    change_id uuid DEFAULT gen_random_uuid() NOT NULL,
    idempotency_key text NOT NULL,
    session_id uuid,
    verb_name character varying(100) NOT NULL,
    taxonomy_context jsonb NOT NULL,
    selection uuid[] DEFAULT '{}'::uuid[] NOT NULL,
    selection_count integer GENERATED ALWAYS AS (COALESCE(array_length(selection, 1), 0)) STORED,
    refinements jsonb DEFAULT '[]'::jsonb,
    stack_depth integer DEFAULT 1 NOT NULL,
    view_state_snapshot jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    audit_user_id uuid,
    source character varying(30) DEFAULT 'unknown'::character varying,
    request_id uuid
);


--
-- Name: COLUMN dsl_view_state_changes.source; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_view_state_changes.source IS 'Origin of view state change: api, cli, mcp, repl, batch, test';


--
-- Name: COLUMN dsl_view_state_changes.request_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".dsl_view_state_changes.request_id IS 'Correlation ID for distributed tracing';


--
-- Name: dsl_workflow_phases; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".dsl_workflow_phases (
    phase_code text NOT NULL,
    label text NOT NULL,
    description text,
    phase_order integer NOT NULL,
    transitions_to text[],
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE dsl_workflow_phases; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".dsl_workflow_phases IS 'KYC workflow phase reference data';


--
-- Name: edge_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".edge_types (
    edge_type_code character varying(50) NOT NULL,
    display_name character varying(100) NOT NULL,
    description text,
    from_node_types jsonb NOT NULL,
    to_node_types jsonb NOT NULL,
    show_in_ubo_view boolean DEFAULT false,
    show_in_trading_view boolean DEFAULT false,
    show_in_fund_structure_view boolean DEFAULT false,
    show_in_service_view boolean DEFAULT false,
    show_in_product_view boolean DEFAULT false,
    edge_style character varying(30) DEFAULT 'SOLID'::character varying,
    edge_color character varying(30),
    edge_width numeric(3,1) DEFAULT 1.0,
    arrow_style character varying(30) DEFAULT 'SINGLE'::character varying,
    shows_percentage boolean DEFAULT false,
    shows_label boolean DEFAULT true,
    label_template character varying(100),
    label_position character varying(20) DEFAULT 'MIDDLE'::character varying,
    layout_direction character varying(20) DEFAULT 'DOWN'::character varying,
    tier_delta integer DEFAULT 1,
    is_hierarchical boolean DEFAULT true,
    bundle_group character varying(30),
    routing_priority integer DEFAULT 50,
    spring_strength numeric(4,3) DEFAULT 1.0,
    ideal_length numeric(6,1) DEFAULT 100.0,
    sibling_sort_key character varying(30) DEFAULT 'PERCENTAGE_DESC'::character varying,
    source_anchor character varying(20) DEFAULT 'AUTO'::character varying,
    target_anchor character varying(20) DEFAULT 'AUTO'::character varying,
    cycle_break_priority integer DEFAULT 50,
    is_primary_parent_rule character varying(50) DEFAULT 'HIGHEST_PERCENTAGE'::character varying,
    parallel_edge_offset numeric(4,1) DEFAULT 15.0,
    self_loop_radius numeric(4,1) DEFAULT 30.0,
    self_loop_position character varying(20) DEFAULT 'TOP_RIGHT'::character varying,
    z_order integer DEFAULT 50,
    is_ownership boolean DEFAULT false,
    is_control boolean DEFAULT false,
    is_structural boolean DEFAULT false,
    is_service_delivery boolean DEFAULT false,
    is_trading boolean DEFAULT false,
    creates_kyc_obligation boolean DEFAULT false,
    cardinality character varying(10) DEFAULT '1:N'::character varying,
    sort_order integer DEFAULT 100,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE edge_types; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".edge_types IS 'Config-driven edge type definitions with view applicability and layout hints. Replaces hardcoded relationship handling.';


--
-- Name: COLUMN edge_types.tier_delta; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".edge_types.tier_delta IS 'How many tiers down (positive) or up (negative) the target is from source. Used by layout engine.';


--
-- Name: COLUMN edge_types.is_hierarchical; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".edge_types.is_hierarchical IS 'If true, this edge type contributes to tier computation in hierarchical layout.';


--
-- Name: COLUMN edge_types.bundle_group; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".edge_types.bundle_group IS 'Edges in same bundle group are routed together to reduce visual clutter.';


--
-- Name: entity_addresses; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_addresses (
    address_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    address_type character varying(50) NOT NULL,
    language character varying(10),
    address_lines text[],
    city character varying(200),
    region character varying(50),
    country character varying(3) NOT NULL,
    postal_code character varying(50),
    is_primary boolean DEFAULT false,
    source character varying(50),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_address_type CHECK (((address_type)::text = ANY ((ARRAY['LEGAL'::character varying, 'HEADQUARTERS'::character varying, 'BRANCH'::character varying, 'ALTERNATIVE'::character varying, 'TRANSLITERATED'::character varying])::text[])))
);


--
-- Name: TABLE entity_addresses; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_addresses IS 'Structured address data from GLEIF legalAddress, headquartersAddress, otherAddresses';


--
-- Name: entity_bods_links; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_bods_links (
    link_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    bods_entity_statement_id character varying(100),
    match_method character varying(50),
    match_confidence numeric,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE entity_bods_links; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_bods_links IS 'Links our entities to BODS entity statements';


--
-- Name: entity_cooperatives; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_cooperatives (
    cooperative_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid,
    cooperative_name character varying(255) NOT NULL,
    cooperative_type character varying(50),
    jurisdiction character varying(100),
    registration_number character varying(100),
    formation_date date,
    member_count integer,
    registered_address text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
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
    CONSTRAINT entity_crud_rules_constraint_type_check CHECK (((constraint_type)::text = ANY (ARRAY[('REQUIRED'::character varying)::text, ('UNIQUE'::character varying)::text, ('FOREIGN_KEY'::character varying)::text, ('VALIDATION'::character varying)::text, ('BUSINESS_RULE'::character varying)::text]))),
    CONSTRAINT entity_crud_rules_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
);


--
-- Name: TABLE entity_crud_rules; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_crud_rules IS 'Entity-specific validation rules and constraints for CRUD operations';


--
-- Name: entity_foundations; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_foundations (
    foundation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid,
    foundation_name character varying(255) NOT NULL,
    foundation_type character varying(50),
    jurisdiction character varying(100) NOT NULL,
    registration_number character varying(100),
    establishment_date date,
    foundation_purpose text,
    governing_law character varying(100),
    registered_address text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_funds; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_funds (
    entity_id uuid NOT NULL,
    lei character varying(20),
    isin_base character varying(12),
    registration_number character varying(100),
    fund_structure_type text,
    fund_type text,
    regulatory_status text,
    parent_fund_id uuid,
    master_fund_id uuid,
    jurisdiction character varying(10),
    regulator character varying(100),
    authorization_date date,
    investment_objective text,
    base_currency character varying(3),
    incorporation_date date,
    launch_date date,
    financial_year_end character varying(5),
    investor_type text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    gleif_legal_form_id character varying(10),
    gleif_registered_as character varying(100),
    gleif_registered_at character varying(20),
    gleif_category character varying(20),
    gleif_status character varying(20),
    gleif_corroboration_level character varying(30),
    gleif_managing_lou character varying(20),
    gleif_last_update timestamp with time zone,
    legal_address_city character varying(100),
    legal_address_country character varying(2),
    hq_address_city character varying(100),
    hq_address_country character varying(2)
);


--
-- Name: entity_government; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_government (
    government_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid,
    entity_name character varying(255) NOT NULL,
    government_type character varying(50) NOT NULL,
    country_code character varying(3),
    governing_authority character varying(255),
    establishment_date date,
    registered_address text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_identifiers; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_identifiers (
    identifier_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    identifier_type character varying(30) NOT NULL,
    identifier_value character varying(100) NOT NULL,
    issuing_authority character varying(100),
    is_primary boolean DEFAULT false,
    valid_from date,
    valid_until date,
    source character varying(50),
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_identifier_type CHECK (((identifier_type)::text = ANY ((ARRAY['LEI'::character varying, 'BIC'::character varying, 'ISIN'::character varying, 'CIK'::character varying, 'MIC'::character varying, 'REG_NUM'::character varying, 'FIGI'::character varying, 'CUSIP'::character varying, 'SEDOL'::character varying])::text[])))
);


--
-- Name: TABLE entity_identifiers; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_identifiers IS 'Cross-reference identifiers from GLEIF (LEI, BIC mappings, etc.) and other sources';


--
-- Name: entity_lifecycle_events; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_lifecycle_events (
    event_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    event_type character varying(50) NOT NULL,
    event_status character varying(30),
    effective_date date,
    recorded_date date,
    affected_fields jsonb,
    old_values jsonb,
    new_values jsonb,
    successor_lei character varying(20),
    successor_name text,
    validation_documents character varying(50),
    validation_reference text,
    source character varying(50) DEFAULT 'GLEIF'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_event_type CHECK (((event_type)::text = ANY ((ARRAY['CHANGE_LEGAL_NAME'::character varying, 'CHANGE_LEGAL_ADDRESS'::character varying, 'CHANGE_HQ_ADDRESS'::character varying, 'CHANGE_LEGAL_FORM'::character varying, 'MERGER'::character varying, 'SPIN_OFF'::character varying, 'ACQUISITION'::character varying, 'DISSOLUTION'::character varying, 'BANKRUPTCY'::character varying, 'DEREGISTRATION'::character varying, 'RELOCATION'::character varying])::text[])))
);


--
-- Name: TABLE entity_lifecycle_events; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_lifecycle_events IS 'Corporate lifecycle events from GLEIF eventGroups - name changes, mergers, etc.';


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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    entity_id uuid,
    lei character varying(20),
    gleif_status character varying(20),
    gleif_category character varying(50),
    gleif_subcategory character varying(50),
    legal_form_code character varying(10),
    legal_form_text character varying(200),
    gleif_validation_level character varying(30),
    gleif_last_update timestamp with time zone,
    gleif_next_renewal date,
    direct_parent_lei character varying(20),
    ultimate_parent_lei character varying(20),
    entity_creation_date date,
    headquarters_address text,
    headquarters_city character varying(200),
    headquarters_country character varying(3),
    fund_manager_lei character varying(20),
    umbrella_fund_lei character varying(20),
    master_fund_lei character varying(20),
    is_fund boolean DEFAULT false,
    fund_type character varying(30),
    gleif_direct_parent_exception character varying(50),
    gleif_ultimate_parent_exception character varying(50),
    ubo_status character varying(30) DEFAULT 'PENDING'::character varying
);


--
-- Name: COLUMN entity_limited_companies.gleif_direct_parent_exception; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_limited_companies.gleif_direct_parent_exception IS 'GLEIF Level 2 reporting exception for direct parent: NO_KNOWN_PERSON, NATURAL_PERSONS, NON_CONSOLIDATING, etc.';


--
-- Name: COLUMN entity_limited_companies.gleif_ultimate_parent_exception; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_limited_companies.gleif_ultimate_parent_exception IS 'GLEIF Level 2 reporting exception for ultimate parent';


--
-- Name: COLUMN entity_limited_companies.ubo_status; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_limited_companies.ubo_status IS 'UBO discovery status: PENDING, DISCOVERED, PUBLIC_FLOAT, EXEMPT, MANUAL_REQUIRED';


--
-- Name: entity_manco; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_manco (
    entity_id uuid NOT NULL,
    lei character varying(20),
    regulatory_reference character varying(100),
    manco_type text NOT NULL,
    authorized_jurisdiction character varying(10) NOT NULL,
    regulator character varying(100),
    authorization_date date,
    can_manage_ucits boolean DEFAULT false,
    can_manage_aif boolean DEFAULT false,
    passported_jurisdictions text[],
    regulatory_capital_eur numeric(15,2),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_names; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_names (
    name_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    name_type character varying(50) NOT NULL,
    name text NOT NULL,
    language character varying(10),
    is_primary boolean DEFAULT false,
    effective_from date,
    effective_to date,
    source character varying(50),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_name_type CHECK (((name_type)::text = ANY ((ARRAY['LEGAL'::character varying, 'TRADING'::character varying, 'TRANSLITERATED'::character varying, 'HISTORICAL'::character varying, 'ALTERNATIVE'::character varying, 'SHORT'::character varying])::text[])))
);


--
-- Name: TABLE entity_names; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_names IS 'Alternative names for entities from GLEIF otherNames and transliteratedOtherNames fields';


--
-- Name: entity_parent_relationships; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_parent_relationships (
    relationship_id uuid DEFAULT gen_random_uuid() NOT NULL,
    child_entity_id uuid NOT NULL,
    parent_entity_id uuid,
    parent_lei character varying(20),
    parent_name text,
    relationship_type character varying(50) NOT NULL,
    accounting_standard character varying(20),
    relationship_start date,
    relationship_end date,
    relationship_status character varying(30) DEFAULT 'ACTIVE'::character varying,
    validation_source character varying(50),
    validation_reference text,
    source character varying(50) DEFAULT 'GLEIF'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_relationship_type CHECK (((relationship_type)::text = ANY ((ARRAY['DIRECT_PARENT'::character varying, 'ULTIMATE_PARENT'::character varying, 'FUND_MANAGER'::character varying, 'UMBRELLA_FUND'::character varying, 'MASTER_FUND'::character varying, 'BRANCH_OF'::character varying])::text[])))
);


--
-- Name: TABLE entity_parent_relationships; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_parent_relationships IS 'Corporate ownership relationships from GLEIF Level 2 data - direct and ultimate parents';


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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    entity_id uuid
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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    search_name text GENERATED ALWAYS AS ((((COALESCE(first_name, ''::character varying))::text || ' '::text) || (COALESCE(last_name, ''::character varying))::text)) STORED,
    entity_id uuid,
    person_state character varying(20) DEFAULT 'GHOST'::character varying NOT NULL
);


--
-- Name: COLUMN entity_proper_persons.person_state; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_proper_persons.person_state IS 'Person entity state: GHOST (name only), IDENTIFIED (has identifying attributes like DOB/nationality/ID numbers), VERIFIED (confirmed by official documents)';


--
-- Name: entity_regulatory_profiles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_regulatory_profiles (
    entity_id uuid NOT NULL,
    is_regulated boolean DEFAULT false,
    regulator_code character varying(20),
    registration_number character varying(100),
    registration_verified boolean DEFAULT false,
    verification_date date,
    verification_method character varying(50),
    verification_reference character varying(500),
    regulatory_tier character varying(20) DEFAULT 'NONE'::character varying,
    next_verification_due date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_relationships_current; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".entity_relationships_current AS
 SELECT relationship_id,
    from_entity_id,
    to_entity_id,
    relationship_type,
    percentage,
    ownership_type,
    control_type,
    trust_role,
    interest_type,
    effective_from,
    effective_to,
    source,
    source_document_ref,
    notes,
    created_at,
    created_by,
    updated_at
   FROM "ob-poc".entity_relationships
  WHERE ((effective_to IS NULL) OR (effective_to > CURRENT_DATE));


--
-- Name: entity_relationships_history; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_relationships_history (
    history_id uuid DEFAULT gen_random_uuid() NOT NULL,
    relationship_id uuid NOT NULL,
    from_entity_id uuid NOT NULL,
    to_entity_id uuid NOT NULL,
    relationship_type character varying(30) NOT NULL,
    percentage numeric(5,2),
    ownership_type character varying(30),
    control_type character varying(30),
    trust_role character varying(30),
    interest_type character varying(20),
    effective_from date,
    effective_to date,
    source character varying(100),
    source_document_ref character varying(255),
    notes text,
    created_at timestamp with time zone,
    created_by uuid,
    updated_at timestamp with time zone,
    trust_interest_type character varying(30),
    trust_class_description text,
    is_regulated boolean,
    regulatory_jurisdiction character varying(20),
    operation character varying(10) NOT NULL,
    changed_at timestamp with time zone DEFAULT now() NOT NULL,
    changed_by uuid,
    superseded_by uuid,
    change_reason text
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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    entity_id uuid
);


--
-- Name: entity_search_view; Type: VIEW; Schema: ob-poc; Owner: -
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
  WHERE (entity_trusts.trust_id IS NOT NULL)
UNION ALL
 SELECT ef.entity_id AS id,
    (COALESCE(et.type_code, 'FUND'::character varying))::text AS entity_type,
    e.name AS display_name,
    ef.jurisdiction AS subtitle_1,
    ef.fund_structure_type AS subtitle_2,
    e.name AS search_text
   FROM (("ob-poc".entity_funds ef
     JOIN "ob-poc".entities e ON ((ef.entity_id = e.entity_id)))
     LEFT JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
  WHERE (ef.entity_id IS NOT NULL);


--
-- Name: entity_share_classes; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_share_classes (
    entity_id uuid NOT NULL,
    parent_fund_id uuid NOT NULL,
    isin character varying(12),
    share_class_code character varying(20),
    share_class_type text NOT NULL,
    distribution_type text NOT NULL,
    currency character varying(3) NOT NULL,
    is_hedged boolean DEFAULT false,
    management_fee_bps integer,
    performance_fee_pct numeric(5,2),
    minimum_investment numeric(18,2),
    launch_date date,
    soft_close_date date,
    hard_close_date date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: entity_type_dependencies; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_type_dependencies (
    dependency_id uuid DEFAULT gen_random_uuid() NOT NULL,
    from_type character varying(50) NOT NULL,
    from_subtype character varying(50),
    to_type character varying(50) NOT NULL,
    to_subtype character varying(50),
    via_arg character varying(100),
    dependency_kind character varying(20) DEFAULT 'required'::character varying,
    condition_expr text,
    priority integer DEFAULT 100,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT entity_type_dependencies_dependency_kind_check CHECK (((dependency_kind)::text = ANY ((ARRAY['required'::character varying, 'optional'::character varying, 'conditional'::character varying])::text[])))
);


--
-- Name: TABLE entity_type_dependencies; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_type_dependencies IS 'Unified entity/resource dependency graph. Drives compiler ordering, linter validation, and onboarding workflows.
from_type/subtype depends on to_type/subtype. via_arg indicates which DSL argument carries the reference.';


--
-- Name: COLUMN entity_type_dependencies.from_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.from_type IS 'Entity type that has the dependency (e.g., resource_instance, entity, case, workstream)';


--
-- Name: COLUMN entity_type_dependencies.from_subtype; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.from_subtype IS 'Subtype qualifier (e.g., CUSTODY_ACCT for resources, fund_sub for entities)';


--
-- Name: COLUMN entity_type_dependencies.to_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.to_type IS 'Entity type that is depended upon';


--
-- Name: COLUMN entity_type_dependencies.to_subtype; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.to_subtype IS 'Subtype qualifier for the dependency target';


--
-- Name: COLUMN entity_type_dependencies.via_arg; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.via_arg IS 'DSL argument name that carries this dependency (for linter validation)';


--
-- Name: COLUMN entity_type_dependencies.dependency_kind; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.dependency_kind IS 'required = must exist before creation, optional = may be linked, lifecycle = state transition dependency';


--
-- Name: COLUMN entity_type_dependencies.priority; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_type_dependencies.priority IS 'Ordering hint when multiple dependencies exist (lower = higher priority)';


--
-- Name: entity_ubos; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".entity_ubos (
    ubo_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    person_statement_id character varying(100),
    person_name text,
    nationalities character varying(10)[],
    country_of_residence character varying(10),
    ownership_chain jsonb,
    chain_depth integer,
    ownership_min numeric,
    ownership_max numeric,
    ownership_exact numeric,
    control_types character varying(50)[],
    is_direct boolean,
    ubo_type character varying(30),
    confidence_level character varying(20),
    source character varying(50),
    source_register character varying(100),
    discovered_at timestamp with time zone DEFAULT now(),
    verified_at timestamp with time zone,
    verified_by character varying(255),
    CONSTRAINT valid_ubo_type CHECK (((ubo_type)::text = ANY ((ARRAY['NATURAL_PERSON'::character varying, 'PUBLIC_FLOAT'::character varying, 'STATE_OWNED'::character varying, 'WIDELY_HELD'::character varying, 'UNKNOWN'::character varying, 'EXEMPT'::character varying])::text[])))
);


--
-- Name: TABLE entity_ubos; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_ubos IS 'Denormalized UBO summary for quick access';


--
-- Name: COLUMN entity_ubos.ownership_chain; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_ubos.ownership_chain IS 'JSON array of intermediate entities in ownership chain';


--
-- Name: COLUMN entity_ubos.ubo_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".entity_ubos.ubo_type IS 'Type: NATURAL_PERSON, PUBLIC_FLOAT, STATE_OWNED, WIDELY_HELD, UNKNOWN, EXEMPT';


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
    CONSTRAINT entity_validation_rules_severity_check CHECK (((severity)::text = ANY (ARRAY[('ERROR'::character varying)::text, ('WARNING'::character varying)::text, ('INFO'::character varying)::text]))),
    CONSTRAINT entity_validation_rules_validation_type_check CHECK (((validation_type)::text = ANY (ARRAY[('REQUIRED'::character varying)::text, ('FORMAT'::character varying)::text, ('RANGE'::character varying)::text, ('REFERENCE'::character varying)::text, ('CUSTOM'::character varying)::text])))
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
-- Name: fund_investments; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".fund_investments (
    investment_id uuid DEFAULT gen_random_uuid() NOT NULL,
    investor_entity_id uuid NOT NULL,
    investee_entity_id uuid NOT NULL,
    percentage_of_investor_nav numeric(5,2) NOT NULL,
    percentage_of_investee_aum numeric(5,2),
    investment_type text DEFAULT 'DIRECT'::text,
    investment_date date,
    redemption_date date,
    valuation_date date,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: fund_investors; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".fund_investors (
    investor_id uuid DEFAULT gen_random_uuid() NOT NULL,
    fund_cbu_id uuid NOT NULL,
    investor_entity_id uuid NOT NULL,
    investor_type character varying(50) NOT NULL,
    investment_amount numeric(20,2),
    currency character varying(3) DEFAULT 'EUR'::character varying,
    subscription_date date,
    kyc_tier character varying(50),
    kyc_status character varying(50) DEFAULT 'PENDING'::character varying,
    kyc_case_id uuid,
    last_kyc_date date,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: fund_structure; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".fund_structure (
    structure_id uuid DEFAULT gen_random_uuid() NOT NULL,
    parent_entity_id uuid NOT NULL,
    child_entity_id uuid NOT NULL,
    relationship_type text DEFAULT 'CONTAINS'::text NOT NULL,
    effective_from date DEFAULT CURRENT_DATE NOT NULL,
    effective_to date,
    created_at timestamp with time zone DEFAULT now(),
    created_by character varying(100)
);


--
-- Name: gleif_sync_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".gleif_sync_log (
    sync_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid,
    lei character varying(20),
    sync_type character varying(30) NOT NULL,
    sync_status character varying(30) NOT NULL,
    records_fetched integer DEFAULT 0,
    records_updated integer DEFAULT 0,
    records_created integer DEFAULT 0,
    error_message text,
    started_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone,
    CONSTRAINT valid_sync_status CHECK (((sync_status)::text = ANY ((ARRAY['SUCCESS'::character varying, 'FAILED'::character varying, 'PARTIAL'::character varying, 'IN_PROGRESS'::character varying])::text[])))
);


--
-- Name: TABLE gleif_sync_log; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".gleif_sync_log IS 'Audit log for GLEIF data synchronization operations';


--
-- Name: instrument_lifecycles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".instrument_lifecycles (
    instrument_lifecycle_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instrument_class_id uuid NOT NULL,
    lifecycle_id uuid NOT NULL,
    is_mandatory boolean DEFAULT true,
    requires_isda boolean DEFAULT false,
    display_order integer DEFAULT 100,
    configuration jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE instrument_lifecycles; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".instrument_lifecycles IS 'Junction: which lifecycles apply to which instrument classes (analogous to product_services)';


--
-- Name: intent_feedback; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".intent_feedback (
    id bigint NOT NULL,
    session_id uuid NOT NULL,
    interaction_id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_input text NOT NULL,
    user_input_hash text NOT NULL,
    input_source text DEFAULT 'chat'::text NOT NULL,
    matched_verb text,
    match_score real,
    match_confidence text,
    semantic_score real,
    phonetic_score real,
    alternatives jsonb,
    outcome text,
    outcome_verb text,
    correction_input text,
    time_to_outcome_ms integer,
    graph_context text,
    workflow_phase text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT valid_confidence CHECK (((match_confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text, 'none'::text])) OR (match_confidence IS NULL))),
    CONSTRAINT valid_outcome CHECK (((outcome = ANY (ARRAY['executed'::text, 'selected_alt'::text, 'corrected'::text, 'rephrased'::text, 'abandoned'::text])) OR (outcome IS NULL))),
    CONSTRAINT valid_source CHECK ((input_source = ANY (ARRAY['chat'::text, 'voice'::text, 'command'::text])))
);


--
-- Name: TABLE intent_feedback; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".intent_feedback IS 'ML feedback capture for intent matching continuous learning. Append-only, batch analysis.';


--
-- Name: intent_feedback_analysis; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".intent_feedback_analysis (
    id integer NOT NULL,
    analysis_type text NOT NULL,
    analysis_date date DEFAULT CURRENT_DATE NOT NULL,
    data jsonb NOT NULL,
    reviewed boolean DEFAULT false,
    applied boolean DEFAULT false,
    reviewed_by text,
    reviewed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE intent_feedback_analysis; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".intent_feedback_analysis IS 'Materialized analysis results from batch feedback analysis. Reviewed by humans, applied to patterns.';


--
-- Name: intent_feedback_analysis_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: -
--

CREATE SEQUENCE "ob-poc".intent_feedback_analysis_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: intent_feedback_analysis_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: -
--

ALTER SEQUENCE "ob-poc".intent_feedback_analysis_id_seq OWNED BY "ob-poc".intent_feedback_analysis.id;


--
-- Name: intent_feedback_id_seq; Type: SEQUENCE; Schema: ob-poc; Owner: -
--

CREATE SEQUENCE "ob-poc".intent_feedback_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: intent_feedback_id_seq; Type: SEQUENCE OWNED BY; Schema: ob-poc; Owner: -
--

ALTER SEQUENCE "ob-poc".intent_feedback_id_seq OWNED BY "ob-poc".intent_feedback.id;


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
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT jurisdiction_code_uppercase CHECK (((jurisdiction_code)::text = upper((jurisdiction_code)::text)))
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
-- Name: jurisdictions; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".jurisdictions AS
 SELECT jurisdiction_code AS iso_code,
    jurisdiction_name AS name,
    region,
    regulatory_framework AS description
   FROM "ob-poc".master_jurisdictions;


--
-- Name: kyc_case_sponsor_decisions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".kyc_case_sponsor_decisions (
    decision_id uuid DEFAULT gen_random_uuid() NOT NULL,
    case_id uuid NOT NULL,
    our_recommendation character varying(50),
    our_recommendation_date timestamp with time zone,
    our_recommendation_by uuid,
    our_findings jsonb,
    sponsor_decision character varying(50),
    sponsor_decision_date timestamp with time zone,
    sponsor_decision_by character varying(255),
    sponsor_comments text,
    final_status character varying(50),
    effective_date date,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: kyc_decisions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".kyc_decisions (
    decision_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    status character varying(20) NOT NULL,
    conditions text,
    review_interval interval,
    next_review_date date,
    evaluation_snapshot jsonb,
    decided_by uuid NOT NULL,
    decided_at timestamp with time zone DEFAULT now(),
    decision_rationale text,
    dsl_execution_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT kyc_decisions_status_check CHECK (((status)::text = ANY ((ARRAY['CLEARED'::character varying, 'REJECTED'::character varying, 'CONDITIONAL'::character varying, 'PENDING_REVIEW'::character varying])::text[])))
);


--
-- Name: TABLE kyc_decisions; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".kyc_decisions IS 'Final KYC decisions with complete evaluation snapshot';


--
-- Name: kyc_service_agreements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".kyc_service_agreements (
    agreement_id uuid DEFAULT gen_random_uuid() NOT NULL,
    sponsor_cbu_id uuid NOT NULL,
    sponsor_entity_id uuid,
    agreement_reference character varying(100),
    effective_date date NOT NULL,
    termination_date date,
    kyc_standard character varying(50) DEFAULT 'BNY_STANDARD'::character varying NOT NULL,
    auto_accept_threshold character varying(50),
    sponsor_review_required boolean DEFAULT true,
    target_turnaround_days integer DEFAULT 5,
    status character varying(50) DEFAULT 'ACTIVE'::character varying,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: layout_cache; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".layout_cache (
    cache_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    view_mode character varying(30) NOT NULL,
    user_id uuid,
    input_hash character varying(64) NOT NULL,
    algorithm_version character varying(20) DEFAULT 'v1.0'::character varying,
    node_positions jsonb NOT NULL,
    edge_paths jsonb NOT NULL,
    bounding_box jsonb,
    tier_info jsonb,
    computation_time_ms integer,
    node_count integer,
    edge_count integer,
    computed_at timestamp with time zone DEFAULT now(),
    valid_until timestamp with time zone
);


--
-- Name: layout_config; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".layout_config (
    config_key character varying(50) NOT NULL,
    config_value jsonb NOT NULL,
    description text,
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE layout_config; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".layout_config IS 'Global layout configuration settings. Key-value store with JSONB values.';


--
-- Name: lifecycle_resource_capabilities; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".lifecycle_resource_capabilities (
    capability_id uuid DEFAULT gen_random_uuid() NOT NULL,
    lifecycle_id uuid NOT NULL,
    resource_type_id uuid NOT NULL,
    is_required boolean DEFAULT true,
    priority integer DEFAULT 100,
    supported_options jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE lifecycle_resource_capabilities; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".lifecycle_resource_capabilities IS 'Junction: which resources each lifecycle requires (analogous to service_resource_capabilities)';


--
-- Name: lifecycle_resource_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".lifecycle_resource_types (
    resource_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    resource_type character varying(100) NOT NULL,
    owner character varying(100) NOT NULL,
    location_type character varying(100),
    per_currency boolean DEFAULT false,
    per_counterparty boolean DEFAULT false,
    per_market boolean DEFAULT false,
    vendor_options jsonb,
    provisioning_verb character varying(100),
    provisioning_args jsonb,
    depends_on jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE lifecycle_resource_types; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".lifecycle_resource_types IS 'Resource types that lifecycles require (analogous to service_resource_types)';


--
-- Name: lifecycles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".lifecycles (
    lifecycle_id uuid DEFAULT gen_random_uuid() NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    category character varying(100) NOT NULL,
    owner character varying(100) NOT NULL,
    regulatory_driver character varying(100),
    sla_definition jsonb,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE lifecycles; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".lifecycles IS 'Operational lifecycles/services that instruments require (analogous to services table)';


--
-- Name: market_csd_mappings; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".market_csd_mappings (
    mapping_id uuid DEFAULT gen_random_uuid() NOT NULL,
    market_id uuid NOT NULL,
    csd_code character varying(50) NOT NULL,
    csd_bic character varying(11) NOT NULL,
    csd_name character varying(255),
    is_primary boolean DEFAULT true,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE market_csd_mappings; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".market_csd_mappings IS 'Maps markets to their CSDs for safekeeping account provisioning';


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
    CONSTRAINT master_entity_xref_entity_status_check CHECK (((entity_status)::text = ANY (ARRAY[('ACTIVE'::character varying)::text, ('INACTIVE'::character varying)::text, ('DISSOLVED'::character varying)::text, ('SUSPENDED'::character varying)::text]))),
    CONSTRAINT master_entity_xref_entity_type_check CHECK (((entity_type)::text = ANY (ARRAY[('PARTNERSHIP'::character varying)::text, ('LIMITED_COMPANY'::character varying)::text, ('PROPER_PERSON'::character varying)::text, ('TRUST'::character varying)::text])))
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
-- Name: node_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".node_types (
    node_type_code character varying(30) NOT NULL,
    display_name character varying(100) NOT NULL,
    description text,
    show_in_ubo_view boolean DEFAULT false,
    show_in_trading_view boolean DEFAULT false,
    show_in_fund_structure_view boolean DEFAULT false,
    show_in_service_view boolean DEFAULT false,
    show_in_product_view boolean DEFAULT false,
    icon character varying(50),
    default_color character varying(30),
    default_shape character varying(30) DEFAULT 'RECTANGLE'::character varying,
    default_width numeric(6,1) DEFAULT 160.0,
    default_height numeric(6,1) DEFAULT 60.0,
    can_be_container boolean DEFAULT false,
    default_tier integer,
    importance_weight numeric(3,2) DEFAULT 1.0,
    child_layout_mode character varying(20) DEFAULT 'VERTICAL'::character varying,
    container_padding numeric(5,1) DEFAULT 20.0,
    collapse_below_zoom numeric(3,2) DEFAULT 0.3,
    hide_label_below_zoom numeric(3,2) DEFAULT 0.2,
    show_detail_above_zoom numeric(3,2) DEFAULT 0.7,
    max_visible_children integer DEFAULT 20,
    overflow_behavior character varying(20) DEFAULT 'COLLAPSE'::character varying,
    dedupe_mode character varying(20) DEFAULT 'SINGLE'::character varying,
    min_separation numeric(5,1) DEFAULT 20.0,
    z_order integer DEFAULT 100,
    is_kyc_subject boolean DEFAULT false,
    is_structural boolean DEFAULT false,
    is_operational boolean DEFAULT false,
    is_trading boolean DEFAULT false,
    sort_order integer DEFAULT 100,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE node_types; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".node_types IS 'Config-driven node type definitions with view applicability and layout hints. Replaces hardcoded Rust enums.';


--
-- Name: COLUMN node_types.show_in_ubo_view; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".node_types.show_in_ubo_view IS 'If true, nodes of this type appear in UBO/KYC views. Replaces hardcoded is_ubo_relevant().';


--
-- Name: COLUMN node_types.show_in_trading_view; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".node_types.show_in_trading_view IS 'If true, nodes of this type appear in Trading views. Replaces hardcoded is_trading_relevant().';


--
-- Name: observation_discrepancies; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".observation_discrepancies (
    discrepancy_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    attribute_id uuid NOT NULL,
    case_id uuid,
    workstream_id uuid,
    observation_1_id uuid NOT NULL,
    observation_2_id uuid NOT NULL,
    discrepancy_type character varying(30) NOT NULL,
    severity character varying(20) NOT NULL,
    description text NOT NULL,
    value_1_display text,
    value_2_display text,
    resolution_status character varying(30) DEFAULT 'OPEN'::character varying,
    resolution_type character varying(30),
    resolution_notes text,
    resolved_at timestamp with time zone,
    resolved_by text,
    accepted_observation_id uuid,
    red_flag_id uuid,
    detected_at timestamp with time zone DEFAULT now() NOT NULL,
    detected_by text DEFAULT 'SYSTEM'::text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT check_disc_resolution_status CHECK (((resolution_status)::text = ANY ((ARRAY['OPEN'::character varying, 'INVESTIGATING'::character varying, 'RESOLVED'::character varying, 'ESCALATED'::character varying, 'ACCEPTED'::character varying])::text[]))),
    CONSTRAINT check_disc_resolution_type CHECK (((resolution_type IS NULL) OR ((resolution_type)::text = ANY ((ARRAY['ACCEPTABLE_VARIATION'::character varying, 'SOURCE_ERROR'::character varying, 'DATA_ENTRY_ERROR'::character varying, 'LEGITIMATE_CHANGE'::character varying, 'FRAUD_CONFIRMED'::character varying, 'FALSE_POSITIVE'::character varying, 'WAIVED'::character varying])::text[])))),
    CONSTRAINT check_disc_severity CHECK (((severity)::text = ANY ((ARRAY['INFO'::character varying, 'LOW'::character varying, 'MEDIUM'::character varying, 'HIGH'::character varying, 'CRITICAL'::character varying])::text[]))),
    CONSTRAINT check_disc_type CHECK (((discrepancy_type)::text = ANY ((ARRAY['VALUE_MISMATCH'::character varying, 'DATE_MISMATCH'::character varying, 'SPELLING_VARIATION'::character varying, 'FORMAT_DIFFERENCE'::character varying, 'MISSING_VS_PRESENT'::character varying, 'CONTRADICTORY'::character varying])::text[])))
);


--
-- Name: TABLE observation_discrepancies; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".observation_discrepancies IS 'Tracks discrepancies detected between attribute observations during KYC reconciliation.';


--
-- Name: onboarding_executions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".onboarding_executions (
    execution_id uuid DEFAULT gen_random_uuid() NOT NULL,
    plan_id uuid NOT NULL,
    status character varying(20) DEFAULT 'pending'::character varying,
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    error_message text,
    result_urls jsonb,
    CONSTRAINT onboarding_executions_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'running'::character varying, 'complete'::character varying, 'failed'::character varying, 'cancelled'::character varying])::text[])))
);


--
-- Name: onboarding_plans; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".onboarding_plans (
    plan_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    products text[] NOT NULL,
    generated_dsl text NOT NULL,
    dependency_graph jsonb NOT NULL,
    resource_count integer NOT NULL,
    status character varying(20) DEFAULT 'pending'::character varying,
    attribute_overrides jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone DEFAULT (now() + '24:00:00'::interval),
    CONSTRAINT onboarding_plans_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'modified'::character varying, 'validated'::character varying, 'executing'::character varying, 'complete'::character varying, 'failed'::character varying])::text[])))
);


--
-- Name: onboarding_products; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".onboarding_products (
    onboarding_product_id uuid DEFAULT gen_random_uuid() NOT NULL,
    request_id uuid NOT NULL,
    product_id uuid NOT NULL,
    selection_order integer,
    selected_at timestamp with time zone DEFAULT now()
);


--
-- Name: onboarding_requests; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: onboarding_tasks; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".onboarding_tasks (
    task_id uuid DEFAULT gen_random_uuid() NOT NULL,
    execution_id uuid NOT NULL,
    resource_code character varying(50) NOT NULL,
    resource_instance_id uuid,
    stage integer NOT NULL,
    status character varying(20) DEFAULT 'pending'::character varying,
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    error_message text,
    retry_count integer DEFAULT 0,
    CONSTRAINT onboarding_tasks_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'running'::character varying, 'complete'::character varying, 'failed'::character varying, 'skipped'::character varying])::text[])))
);


--
-- Name: product_services; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".product_services (
    product_id uuid NOT NULL,
    service_id uuid NOT NULL,
    is_mandatory boolean DEFAULT false,
    is_default boolean DEFAULT false,
    display_order integer,
    configuration jsonb
);


--
-- Name: products; Type: TABLE; Schema: ob-poc; Owner: -
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
    metadata jsonb,
    kyc_risk_rating character varying(20),
    kyc_context character varying(50),
    requires_kyc boolean DEFAULT true
);


--
-- Name: proofs; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".proofs (
    proof_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    document_id uuid,
    proof_type character varying(50) NOT NULL,
    valid_from date,
    valid_until date,
    status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    marked_dirty_at timestamp with time zone,
    dirty_reason character varying(100),
    uploaded_by uuid,
    uploaded_at timestamp with time zone DEFAULT now(),
    verified_by uuid,
    verified_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE proofs; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".proofs IS 'Evidence documents that prove ownership/control assertions';


--
-- Name: red_flag_severities; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".red_flag_severities (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_blocking boolean DEFAULT false,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: redflag_score_config; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".redflag_score_config (
    config_id uuid DEFAULT gen_random_uuid() NOT NULL,
    severity character varying(20) NOT NULL,
    weight integer NOT NULL,
    is_blocking boolean DEFAULT false,
    description text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_redflag_severity CHECK (((severity)::text = ANY ((ARRAY['SOFT'::character varying, 'ESCALATE'::character varying, 'HARD_STOP'::character varying])::text[])))
);


--
-- Name: TABLE redflag_score_config; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".redflag_score_config IS 'Red-flag severity weights for score calculation';


--
-- Name: regulators; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".regulators (
    regulator_code character varying(20) NOT NULL,
    name character varying(255) NOT NULL,
    jurisdiction character varying(10) NOT NULL,
    tier character varying(20) DEFAULT 'NONE'::character varying NOT NULL,
    registry_url character varying(500),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: regulatory_tiers; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".regulatory_tiers (
    tier_code character varying(20) NOT NULL,
    description text,
    allows_simplified_dd boolean DEFAULT false,
    requires_enhanced_screening boolean DEFAULT false,
    reliance_level character varying(20) DEFAULT 'none'::character varying,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: requirement_acceptable_docs; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".requirement_acceptable_docs (
    requirement_id uuid NOT NULL,
    document_type_code character varying(50) NOT NULL,
    priority integer DEFAULT 1
);


--
-- Name: resource_attribute_requirements; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: resource_dependencies; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".resource_dependencies (
    dependency_id uuid DEFAULT gen_random_uuid() NOT NULL,
    resource_type_id uuid NOT NULL,
    depends_on_type_id uuid NOT NULL,
    dependency_type character varying(20) DEFAULT 'required'::character varying,
    inject_arg character varying(100) NOT NULL,
    condition_expression text,
    priority integer DEFAULT 100,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT no_self_dependency CHECK ((resource_type_id <> depends_on_type_id)),
    CONSTRAINT resource_dependencies_dependency_type_check CHECK (((dependency_type)::text = ANY ((ARRAY['required'::character varying, 'optional'::character varying, 'conditional'::character varying])::text[])))
);


--
-- Name: TABLE resource_dependencies; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".resource_dependencies IS 'Resource type dependencies for onboarding. E.g., custody_account depends on cash_account.
The inject_arg specifies which provisioning argument receives the dependency URL.';


--
-- Name: resource_instance_attributes; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE resource_instance_attributes; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".resource_instance_attributes IS 'Attribute values for resource instances - dense storage (row exists = value set)';


--
-- Name: resource_instance_dependencies; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".resource_instance_dependencies (
    instance_id uuid NOT NULL,
    depends_on_instance_id uuid NOT NULL,
    dependency_type character varying(20) DEFAULT 'required'::character varying,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: resource_profile_sources; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".resource_profile_sources (
    link_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    profile_id uuid NOT NULL,
    profile_section character varying(50) NOT NULL,
    profile_path text,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE resource_profile_sources; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".resource_profile_sources IS 'Links provisioned service resources back to their source in the trading profile.
Enables: "Why was this SWIFT gateway provisioned?" → "investment_managers[0].instruction_method = SWIFT"';


--
-- Name: risk_bands; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".risk_bands (
    band_code character varying(20) NOT NULL,
    min_score integer NOT NULL,
    max_score integer NOT NULL,
    description text,
    escalation_required boolean DEFAULT false,
    review_frequency_months integer DEFAULT 12,
    CONSTRAINT valid_score_range CHECK ((min_score <= max_score))
);


--
-- Name: TABLE risk_bands; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".risk_bands IS 'Risk band definitions mapping composite score to risk level';


--
-- Name: risk_ratings; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".risk_ratings (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    severity_level integer DEFAULT 0,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: role_categories; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".role_categories (
    category_code character varying(30) NOT NULL,
    category_name character varying(100) NOT NULL,
    description text,
    layout_behavior character varying(30) NOT NULL,
    sort_order integer DEFAULT 100,
    show_in_ubo_view boolean DEFAULT true,
    show_in_trading_view boolean DEFAULT false,
    show_in_fund_structure_view boolean DEFAULT false,
    show_in_service_view boolean DEFAULT false
);


--
-- Name: TABLE role_categories; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".role_categories IS 'Reference table for role categories with layout behavior hints for visualization.';


--
-- Name: role_incompatibilities; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".role_incompatibilities (
    incompatibility_id uuid DEFAULT gen_random_uuid() NOT NULL,
    role_a character varying(255) NOT NULL,
    role_b character varying(255) NOT NULL,
    reason text NOT NULL,
    exception_allowed boolean DEFAULT false,
    exception_condition text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_role_order CHECK (((role_a)::text < (role_b)::text))
);


--
-- Name: TABLE role_incompatibilities; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".role_incompatibilities IS 'Defines invalid role combinations that cannot coexist on same entity within same CBU.';


--
-- Name: role_requirements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".role_requirements (
    requirement_id uuid DEFAULT gen_random_uuid() NOT NULL,
    requiring_role character varying(255) NOT NULL,
    required_role character varying(255) NOT NULL,
    requirement_type character varying(30) NOT NULL,
    scope character varying(30) NOT NULL,
    condition_description text,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE role_requirements; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".role_requirements IS 'Defines role dependencies - when one role requires another to be present.';


--
-- Name: role_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".role_types (
    role_code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    triggers_full_kyc boolean DEFAULT false,
    triggers_screening boolean DEFAULT false,
    triggers_id_verification boolean DEFAULT false,
    check_regulatory_status boolean DEFAULT false,
    if_regulated_obligation character varying(50),
    cascade_to_entity_ubos boolean DEFAULT false,
    threshold_based boolean DEFAULT false,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: roles; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".roles (
    role_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    role_category character varying(30),
    layout_category character varying(30),
    ubo_treatment character varying(30),
    requires_percentage boolean DEFAULT false,
    natural_person_only boolean DEFAULT false,
    legal_entity_only boolean DEFAULT false,
    compatible_entity_categories jsonb,
    display_priority integer DEFAULT 50,
    kyc_obligation character varying(30) DEFAULT 'FULL_KYC'::character varying,
    is_active boolean DEFAULT true,
    sort_order integer DEFAULT 100,
    CONSTRAINT role_name_uppercase CHECK (((name)::text = upper((name)::text)))
);


--
-- Name: TABLE roles; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".roles IS 'Master role taxonomy with visualization metadata, UBO treatment rules, and entity compatibility constraints. Version 2.0.';


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
-- Name: screening_lists; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: screening_requirements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".screening_requirements (
    risk_band character varying(20) NOT NULL,
    screening_type character varying(50) NOT NULL,
    is_required boolean DEFAULT true NOT NULL,
    frequency_months integer DEFAULT 12
);


--
-- Name: screening_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".screening_types (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: semantic_match_cache; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".semantic_match_cache (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    transcript_normalized text NOT NULL,
    matched_verb character varying(100) NOT NULL,
    similarity_score real NOT NULL,
    match_method character varying(20) NOT NULL,
    hit_count integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_accessed_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: service_delivery_map; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE service_delivery_map; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".service_delivery_map IS 'Tracks service delivery for CBU onboarding - links CBU -> Product -> Service -> Instance';


--
-- Name: service_option_choices; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: service_option_definitions; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: service_resource_capabilities; Type: TABLE; Schema: ob-poc; Owner: -
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
    is_required boolean DEFAULT true,
    CONSTRAINT service_resource_capabilities_performance_rating_check CHECK (((performance_rating >= 1) AND (performance_rating <= 5)))
);


--
-- Name: COLUMN service_resource_capabilities.is_required; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_capabilities.is_required IS 'Whether this resource is required for the service to function';


--
-- Name: service_resource_types; Type: TABLE; Schema: ob-poc; Owner: -
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
    is_active boolean DEFAULT true,
    per_market boolean DEFAULT false,
    per_currency boolean DEFAULT false,
    per_counterparty boolean DEFAULT false,
    provisioning_verb character varying(100),
    provisioning_args jsonb,
    depends_on jsonb,
    location_type character varying(50)
);


--
-- Name: COLUMN service_resource_types.per_market; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_types.per_market IS 'Resource requires market context (e.g., settlement account per exchange)';


--
-- Name: COLUMN service_resource_types.per_currency; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_types.per_currency IS 'Resource requires currency context (e.g., cash account per currency)';


--
-- Name: COLUMN service_resource_types.per_counterparty; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_types.per_counterparty IS 'Resource requires counterparty context (e.g., ISDA per counterparty)';


--
-- Name: COLUMN service_resource_types.provisioning_verb; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_types.provisioning_verb IS 'DSL verb to provision this resource type';


--
-- Name: COLUMN service_resource_types.depends_on; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_types.depends_on IS 'Array of resource_codes this resource depends on';


--
-- Name: COLUMN service_resource_types.location_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".service_resource_types.location_type IS 'INTERNAL, EXTERNAL, HYBRID';


--
-- Name: services; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: settlement_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".settlement_types (
    code character varying(20) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: sla_breaches; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".sla_breaches (
    breach_id uuid DEFAULT gen_random_uuid() NOT NULL,
    measurement_id uuid NOT NULL,
    commitment_id uuid NOT NULL,
    breach_severity character varying(20) NOT NULL,
    breach_date date NOT NULL,
    detected_at timestamp with time zone DEFAULT now(),
    root_cause_category character varying(50),
    root_cause_description text,
    remediation_status character varying(20) DEFAULT 'OPEN'::character varying,
    remediation_plan text,
    remediation_due_date date,
    remediation_completed_at timestamp with time zone,
    penalty_applied boolean DEFAULT false,
    penalty_amount numeric(18,2),
    penalty_currency character varying(3),
    escalated_to character varying(255),
    escalated_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_breach_severity CHECK (((breach_severity)::text = ANY ((ARRAY['MINOR'::character varying, 'MAJOR'::character varying, 'CRITICAL'::character varying])::text[]))),
    CONSTRAINT valid_remediation_status CHECK (((remediation_status)::text = ANY ((ARRAY['OPEN'::character varying, 'IN_PROGRESS'::character varying, 'RESOLVED'::character varying, 'WAIVED'::character varying, 'ESCALATED'::character varying])::text[]))),
    CONSTRAINT valid_root_cause CHECK (((root_cause_category IS NULL) OR ((root_cause_category)::text = ANY ((ARRAY['SYSTEM'::character varying, 'VENDOR'::character varying, 'MARKET'::character varying, 'CLIENT'::character varying, 'INTERNAL'::character varying, 'EXTERNAL'::character varying, 'UNKNOWN'::character varying])::text[]))))
);


--
-- Name: sla_measurements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".sla_measurements (
    measurement_id uuid DEFAULT gen_random_uuid() NOT NULL,
    commitment_id uuid NOT NULL,
    period_start date NOT NULL,
    period_end date NOT NULL,
    measured_value numeric(10,4) NOT NULL,
    sample_size integer,
    status character varying(20) NOT NULL,
    variance_pct numeric(6,2),
    measurement_notes text,
    measurement_method character varying(50),
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_measurement_method CHECK (((measurement_method IS NULL) OR ((measurement_method)::text = ANY ((ARRAY['AUTOMATED'::character varying, 'MANUAL'::character varying, 'ESTIMATED'::character varying, 'SYSTEM'::character varying])::text[])))),
    CONSTRAINT valid_measurement_status CHECK (((status)::text = ANY ((ARRAY['MET'::character varying, 'WARNING'::character varying, 'BREACH'::character varying])::text[])))
);


--
-- Name: sla_metric_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".sla_metric_types (
    metric_code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    metric_category character varying(30) NOT NULL,
    unit character varying(20) NOT NULL,
    aggregation_method character varying(20) DEFAULT 'AVERAGE'::character varying,
    higher_is_better boolean DEFAULT true,
    is_active boolean DEFAULT true,
    CONSTRAINT valid_aggregation CHECK (((aggregation_method)::text = ANY ((ARRAY['AVERAGE'::character varying, 'SUM'::character varying, 'MIN'::character varying, 'MAX'::character varying, 'MEDIAN'::character varying, 'P95'::character varying, 'P99'::character varying])::text[]))),
    CONSTRAINT valid_metric_category CHECK (((metric_category)::text = ANY ((ARRAY['TIMELINESS'::character varying, 'ACCURACY'::character varying, 'AVAILABILITY'::character varying, 'VOLUME'::character varying, 'QUALITY'::character varying])::text[]))),
    CONSTRAINT valid_unit CHECK (((unit)::text = ANY ((ARRAY['PERCENT'::character varying, 'HOURS'::character varying, 'MINUTES'::character varying, 'SECONDS'::character varying, 'COUNT'::character varying, 'CURRENCY'::character varying, 'BASIS_POINTS'::character varying])::text[])))
);


--
-- Name: sla_templates; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".sla_templates (
    template_id uuid DEFAULT gen_random_uuid() NOT NULL,
    template_code character varying(50) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    applies_to_type character varying(30) NOT NULL,
    applies_to_code character varying(50),
    metric_code character varying(50) NOT NULL,
    target_value numeric(10,4) NOT NULL,
    warning_threshold numeric(10,4),
    measurement_period character varying(20) DEFAULT 'MONTHLY'::character varying,
    response_time_hours numeric(5,2),
    escalation_path text,
    regulatory_requirement boolean DEFAULT false,
    regulatory_reference text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_applies_to CHECK (((applies_to_type)::text = ANY ((ARRAY['SERVICE'::character varying, 'RESOURCE_TYPE'::character varying, 'ISDA'::character varying, 'CSA'::character varying, 'PRODUCT'::character varying])::text[]))),
    CONSTRAINT valid_measurement_period CHECK (((measurement_period)::text = ANY ((ARRAY['DAILY'::character varying, 'WEEKLY'::character varying, 'MONTHLY'::character varying, 'QUARTERLY'::character varying, 'ANNUAL'::character varying])::text[])))
);


--
-- Name: ssi_types; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".ssi_types (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: taxonomy_crud_log; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE taxonomy_crud_log; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".taxonomy_crud_log IS 'Audit log for taxonomy CRUD operations';


--
-- Name: COLUMN taxonomy_crud_log.operation_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".taxonomy_crud_log.operation_type IS 'CREATE, READ, UPDATE, DELETE';


--
-- Name: COLUMN taxonomy_crud_log.entity_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".taxonomy_crud_log.entity_type IS 'product, service, resource, onboarding';


--
-- Name: threshold_factors; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".threshold_factors (
    factor_id uuid DEFAULT gen_random_uuid() NOT NULL,
    factor_type character varying(50) NOT NULL,
    factor_code character varying(50) NOT NULL,
    risk_weight integer DEFAULT 1 NOT NULL,
    description text,
    is_active boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE threshold_factors; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".threshold_factors IS 'Risk factors contributing to overall CBU risk score';


--
-- Name: COLUMN threshold_factors.factor_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".threshold_factors.factor_type IS 'Category: CBU_TYPE, SOURCE_OF_FUNDS, NATURE_PURPOSE, JURISDICTION, PRODUCT_RISK';


--
-- Name: COLUMN threshold_factors.risk_weight; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".threshold_factors.risk_weight IS 'Contribution to composite risk score (higher = riskier)';


--
-- Name: threshold_requirements; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".threshold_requirements (
    requirement_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_role character varying(50) NOT NULL,
    risk_band character varying(20) NOT NULL,
    attribute_code character varying(50) NOT NULL,
    is_required boolean DEFAULT true NOT NULL,
    confidence_min numeric(3,2) DEFAULT 0.85,
    max_age_days integer,
    must_be_authoritative boolean DEFAULT false,
    notes text,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE threshold_requirements; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".threshold_requirements IS 'KYC attribute requirements per entity role and risk band';


--
-- Name: trading_profile_documents; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".trading_profile_documents (
    link_id uuid DEFAULT gen_random_uuid() NOT NULL,
    profile_id uuid NOT NULL,
    doc_id uuid NOT NULL,
    profile_section character varying(50) NOT NULL,
    extraction_status character varying(20) DEFAULT 'PENDING'::character varying,
    extracted_at timestamp with time zone,
    extraction_notes text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT valid_extraction_status CHECK (((extraction_status)::text = ANY ((ARRAY['PENDING'::character varying, 'IN_PROGRESS'::character varying, 'COMPLETE'::character varying, 'FAILED'::character varying, 'PARTIAL'::character varying])::text[]))),
    CONSTRAINT valid_profile_section CHECK (((profile_section)::text = ANY ((ARRAY['universe'::character varying, 'investment_managers'::character varying, 'isda_agreements'::character varying, 'settlement_config'::character varying, 'booking_rules'::character varying, 'standing_instructions'::character varying, 'pricing_matrix'::character varying, 'valuation_config'::character varying, 'constraints'::character varying, 'cash_sweep_config'::character varying, 'sla_commitments'::character varying])::text[])))
);


--
-- Name: TABLE trading_profile_documents; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".trading_profile_documents IS 'Links source documents (IMA, ISDA, SSI forms) to trading profile sections they populate.
Enables audit trail: "Where did this config come from?" → traces to source document.';


--
-- Name: trading_profile_materializations; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".trading_profile_materializations (
    materialization_id uuid DEFAULT gen_random_uuid() NOT NULL,
    profile_id uuid NOT NULL,
    materialized_at timestamp with time zone DEFAULT now() NOT NULL,
    materialized_by character varying(255),
    sections_materialized text[] NOT NULL,
    records_created jsonb DEFAULT '{}'::jsonb NOT NULL,
    records_updated jsonb DEFAULT '{}'::jsonb NOT NULL,
    records_deleted jsonb DEFAULT '{}'::jsonb NOT NULL,
    errors jsonb,
    duration_ms integer
);


--
-- Name: trading_profile_migration_backup; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".trading_profile_migration_backup (
    backup_id uuid DEFAULT gen_random_uuid() NOT NULL,
    profile_id uuid NOT NULL,
    original_document jsonb NOT NULL,
    migrated_at timestamp with time zone DEFAULT now()
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
-- Name: ubo_assertion_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".ubo_assertion_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    dsl_execution_id uuid,
    assertion_type character varying(50) NOT NULL,
    expected_value boolean NOT NULL,
    actual_value boolean NOT NULL,
    passed boolean NOT NULL,
    failure_details jsonb,
    asserted_at timestamp with time zone DEFAULT now(),
    asserted_by uuid
);


--
-- Name: TABLE ubo_assertion_log; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_assertion_log IS 'Audit log of all KYC assertions (declarative gates)';


--
-- Name: ubo_convergence_status; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".ubo_convergence_status AS
 SELECT cbu_id,
    count(*) AS total_edges,
    count(*) FILTER (WHERE ((status)::text = 'proven'::text)) AS proven_edges,
    count(*) FILTER (WHERE ((status)::text = 'alleged'::text)) AS alleged_edges,
    count(*) FILTER (WHERE ((status)::text = 'pending'::text)) AS pending_edges,
    count(*) FILTER (WHERE ((status)::text = 'disputed'::text)) AS disputed_edges,
    (count(*) FILTER (WHERE ((status)::text = 'proven'::text)) = count(*)) AS is_converged
   FROM "ob-poc".cbu_relationship_verification
  GROUP BY cbu_id;


--
-- Name: VIEW ubo_convergence_status; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".ubo_convergence_status IS 'Computed convergence status per CBU from cbu_relationship_verification';


--
-- Name: ubo_evidence; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".ubo_evidence (
    ubo_evidence_id uuid DEFAULT gen_random_uuid() NOT NULL,
    ubo_id uuid NOT NULL,
    document_id uuid,
    attestation_ref character varying(255),
    evidence_type character varying(50) NOT NULL,
    evidence_role character varying(50) NOT NULL,
    description text,
    attached_at timestamp with time zone DEFAULT now(),
    attached_by character varying(255),
    verified_at timestamp with time zone,
    verified_by character varying(255),
    verification_status character varying(30) DEFAULT 'PENDING'::character varying,
    verification_notes text,
    CONSTRAINT chk_ubo_evidence_role CHECK (((evidence_role)::text = ANY ((ARRAY['IDENTITY_PROOF'::character varying, 'OWNERSHIP_PROOF'::character varying, 'CONTROL_PROOF'::character varying, 'ADDRESS_PROOF'::character varying, 'SOURCE_OF_WEALTH'::character varying, 'CHAIN_LINK'::character varying])::text[]))),
    CONSTRAINT chk_ubo_evidence_source CHECK (((document_id IS NOT NULL) OR (attestation_ref IS NOT NULL))),
    CONSTRAINT chk_ubo_evidence_type CHECK (((evidence_type)::text = ANY ((ARRAY['DOCUMENT'::character varying, 'ATTESTATION'::character varying, 'SCREENING'::character varying, 'REGISTRY_LOOKUP'::character varying, 'OWNERSHIP_RECORD'::character varying])::text[]))),
    CONSTRAINT chk_ubo_evidence_verification CHECK (((verification_status)::text = ANY ((ARRAY['PENDING'::character varying, 'VERIFIED'::character varying, 'REJECTED'::character varying, 'EXPIRED'::character varying])::text[])))
);


--
-- Name: TABLE ubo_evidence; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_evidence IS 'Evidence documents and attestations supporting UBO determinations';


--
-- Name: ubo_expired_proofs; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".ubo_expired_proofs AS
 SELECT v.cbu_id,
    v.proof_document_id AS proof_id,
    d.document_type_code AS proof_type,
    d.status AS doc_status,
    v.verification_id AS edge_id,
    r.from_entity_id,
    r.to_entity_id,
    r.relationship_type AS edge_type
   FROM (("ob-poc".cbu_relationship_verification v
     JOIN "ob-poc".entity_relationships r ON ((v.relationship_id = r.relationship_id)))
     JOIN "ob-poc".document_catalog d ON ((v.proof_document_id = d.doc_id)))
  WHERE (((d.status)::text <> ALL ((ARRAY['active'::character varying, 'valid'::character varying])::text[])) OR ((v.status)::text = 'disputed'::text));


--
-- Name: VIEW ubo_expired_proofs; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".ubo_expired_proofs IS 'Relationship verifications with invalid or expired proof documents';


--
-- Name: ubo_missing_proofs; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".ubo_missing_proofs AS
 SELECT v.cbu_id,
    v.verification_id AS edge_id,
    r.from_entity_id,
    f.name AS from_entity_name,
    r.to_entity_id,
    t.name AS to_entity_name,
    r.relationship_type AS edge_type,
    v.status,
    v.alleged_percentage,
        CASE
            WHEN ((r.relationship_type)::text = 'ownership'::text) THEN 'shareholder_register'::text
            WHEN ((r.relationship_type)::text = 'control'::text) THEN 'board_resolution'::text
            WHEN ((r.relationship_type)::text = 'trust_role'::text) THEN 'trust_deed'::text
            ELSE NULL::text
        END AS required_proof_type
   FROM ((("ob-poc".cbu_relationship_verification v
     JOIN "ob-poc".entity_relationships r ON ((v.relationship_id = r.relationship_id)))
     JOIN "ob-poc".entities f ON ((f.entity_id = r.from_entity_id)))
     JOIN "ob-poc".entities t ON ((t.entity_id = r.to_entity_id)))
  WHERE (((v.status)::text = ANY ((ARRAY['alleged'::character varying, 'pending'::character varying, 'unverified'::character varying])::text[])) AND (v.proof_document_id IS NULL));


--
-- Name: VIEW ubo_missing_proofs; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".ubo_missing_proofs IS 'Relationship verifications missing proof documents';


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
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    case_id uuid,
    workstream_id uuid,
    discovery_method character varying(30) DEFAULT 'MANUAL'::character varying,
    superseded_by uuid,
    superseded_at timestamp with time zone,
    closed_at timestamp with time zone,
    closed_reason character varying(100),
    evidence_doc_ids uuid[],
    proof_date timestamp with time zone,
    proof_method character varying(50),
    proof_notes text,
    replacement_ubo_id uuid,
    removal_reason character varying(100),
    CONSTRAINT chk_ubo_discovery_method CHECK (((discovery_method)::text = ANY ((ARRAY['MANUAL'::character varying, 'INFERRED'::character varying, 'DOCUMENT'::character varying, 'REGISTRY'::character varying, 'SCREENING'::character varying])::text[]))),
    CONSTRAINT chk_ubo_proof_method CHECK (((proof_method IS NULL) OR ((proof_method)::text = ANY ((ARRAY['DOCUMENT'::character varying, 'REGISTRY_LOOKUP'::character varying, 'SCREENING_MATCH'::character varying, 'MANUAL_VERIFICATION'::character varying, 'OWNERSHIP_CHAIN'::character varying, 'CLIENT_ATTESTATION'::character varying])::text[])))),
    CONSTRAINT chk_ubo_verification_status CHECK (((verification_status)::text = ANY ((ARRAY['SUSPECTED'::character varying, 'PENDING'::character varying, 'PROVEN'::character varying, 'VERIFIED'::character varying, 'FAILED'::character varying, 'DISPUTED'::character varying, 'REMOVED'::character varying])::text[])))
);


--
-- Name: TABLE ubo_registry; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_registry IS 'DEPRECATED: UBO status now derived from ubo_edges + entity_workstreams';


--
-- Name: ubo_snapshot_comparisons; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".ubo_snapshot_comparisons (
    comparison_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    baseline_snapshot_id uuid NOT NULL,
    current_snapshot_id uuid NOT NULL,
    has_changes boolean DEFAULT false NOT NULL,
    change_summary jsonb DEFAULT '{}'::jsonb NOT NULL,
    added_ubos jsonb DEFAULT '[]'::jsonb,
    removed_ubos jsonb DEFAULT '[]'::jsonb,
    changed_ubos jsonb DEFAULT '[]'::jsonb,
    ownership_changes jsonb DEFAULT '[]'::jsonb,
    control_changes jsonb DEFAULT '[]'::jsonb,
    compared_at timestamp with time zone DEFAULT now() NOT NULL,
    compared_by character varying(255),
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_different_snapshots CHECK ((baseline_snapshot_id <> current_snapshot_id))
);


--
-- Name: TABLE ubo_snapshot_comparisons; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_snapshot_comparisons IS 'Comparisons between UBO snapshots to detect changes';


--
-- Name: ubo_snapshots; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".ubo_snapshots (
    snapshot_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    snapshot_type character varying(30) DEFAULT 'MANUAL'::character varying NOT NULL,
    snapshot_reason character varying(100),
    ubos jsonb DEFAULT '[]'::jsonb NOT NULL,
    ownership_chains jsonb DEFAULT '[]'::jsonb NOT NULL,
    control_relationships jsonb DEFAULT '[]'::jsonb NOT NULL,
    total_identified_ownership numeric(5,2),
    has_gaps boolean DEFAULT false,
    gap_summary text,
    captured_at timestamp with time zone DEFAULT now() NOT NULL,
    captured_by character varying(255),
    notes text,
    created_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_snapshot_type CHECK (((snapshot_type)::text = ANY ((ARRAY['MANUAL'::character varying, 'PERIODIC'::character varying, 'EVENT_DRIVEN'::character varying, 'CASE_OPEN'::character varying, 'CASE_CLOSE'::character varying])::text[])))
);


--
-- Name: TABLE ubo_snapshots; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_snapshots IS 'Point-in-time snapshots of UBO ownership state for a CBU';


--
-- Name: ubo_treatments; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".ubo_treatments (
    treatment_code character varying(30) NOT NULL,
    treatment_name character varying(100) NOT NULL,
    description text,
    terminates_chain boolean DEFAULT false,
    requires_lookthrough boolean DEFAULT false
);


--
-- Name: TABLE ubo_treatments; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ubo_treatments IS 'Reference table for UBO calculation behaviors (terminus, look-through, etc.).';


--
-- Name: v_active_trading_profiles; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_active_trading_profiles AS
 SELECT tp.profile_id,
    tp.cbu_id,
    c.name AS cbu_name,
    tp.version,
    tp.document,
    tp.document_hash,
    tp.created_at,
    tp.activated_at,
    tp.activated_by
   FROM ("ob-poc".cbu_trading_profiles tp
     JOIN "ob-poc".cbus c ON ((c.cbu_id = tp.cbu_id)))
  WHERE ((tp.status)::text = 'ACTIVE'::text);


--
-- Name: v_allegation_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_allegation_summary AS
 SELECT ca.cbu_id,
    ca.entity_id,
    e.name AS entity_name,
    count(*) AS total_allegations,
    count(*) FILTER (WHERE ((ca.verification_status)::text = 'VERIFIED'::text)) AS verified,
    count(*) FILTER (WHERE ((ca.verification_status)::text = 'CONTRADICTED'::text)) AS contradicted,
    count(*) FILTER (WHERE ((ca.verification_status)::text = 'PARTIAL'::text)) AS partial,
    count(*) FILTER (WHERE ((ca.verification_status)::text = 'PENDING'::text)) AS pending,
    count(*) FILTER (WHERE ((ca.verification_status)::text = 'UNVERIFIABLE'::text)) AS unverifiable
   FROM ("ob-poc".client_allegations ca
     JOIN "ob-poc".entities e ON ((ca.entity_id = e.entity_id)))
  GROUP BY ca.cbu_id, ca.entity_id, e.name;


--
-- Name: v_attribute_current; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_attribute_current AS
 SELECT DISTINCT ON (entity_id, attribute_id) entity_id,
    attribute_id,
    observation_id,
    value_text,
    value_number,
    value_boolean,
    value_date,
    value_datetime,
    value_json,
    source_type,
    source_document_id,
    confidence,
    is_authoritative,
    observed_at
   FROM "ob-poc".attribute_observations
  WHERE ((status)::text = 'ACTIVE'::text)
  ORDER BY entity_id, attribute_id, is_authoritative DESC, confidence DESC, observed_at DESC;


--
-- Name: VIEW v_attribute_current; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_attribute_current IS 'Current best value for each attribute - prioritizes authoritative sources, then confidence, then recency';


--
-- Name: v_case_redflag_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_case_redflag_summary AS
 SELECT c.case_id,
    c.cbu_id,
    c.status AS case_status,
    c.escalation_level,
    scores.soft_count,
    scores.escalate_count,
    scores.hard_stop_count,
    scores.total_score,
    scores.has_hard_stop,
    scores.open_flags,
    scores.mitigated_flags,
    scores.waived_flags,
    ( SELECT es.recommended_action
           FROM "ob-poc".case_evaluation_snapshots es
          WHERE (es.case_id = c.case_id)
          ORDER BY es.evaluated_at DESC
         LIMIT 1) AS last_recommendation,
    ( SELECT es.evaluated_at
           FROM "ob-poc".case_evaluation_snapshots es
          WHERE (es.case_id = c.case_id)
          ORDER BY es.evaluated_at DESC
         LIMIT 1) AS last_evaluated_at
   FROM (kyc.cases c
     CROSS JOIN LATERAL "ob-poc".compute_case_redflag_score(c.case_id) scores(soft_count, escalate_count, hard_stop_count, soft_score, escalate_score, has_hard_stop, total_score, open_flags, mitigated_flags, waived_flags));


--
-- Name: VIEW v_case_redflag_summary; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_case_redflag_summary IS 'Summary view of case red-flag status';


--
-- Name: v_cbu_entity_graph; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_entity_graph AS
 SELECT c.cbu_id,
    c.name AS cbu_name,
    c.cbu_category,
    c.client_type,
    c.jurisdiction AS cbu_jurisdiction,
    e.entity_id,
    e.name AS entity_name,
    et.type_code AS entity_type,
    r.name AS role_code,
    r.description AS role_description,
    ew.status AS workstream_status,
    ew.risk_rating AS entity_risk_rating,
    ew.requires_enhanced_dd,
    ew.is_ubo,
    ew.ownership_percentage,
    (c.commercial_client_entity_id = e.entity_id) AS is_commercial_client,
        CASE
            WHEN ((et.type_code)::text = 'PROPER_PERSON_NATURAL'::text) THEN ( SELECT jsonb_build_object('first_name', pp.first_name, 'last_name', pp.last_name, 'nationality', pp.nationality, 'date_of_birth', pp.date_of_birth) AS jsonb_build_object
               FROM "ob-poc".entity_proper_persons pp
              WHERE (pp.entity_id = e.entity_id))
            WHEN ((et.type_code)::text ~~ 'LIMITED_COMPANY%'::text) THEN ( SELECT jsonb_build_object('company_name', lc.company_name, 'registration_number', lc.registration_number, 'jurisdiction', lc.jurisdiction, 'incorporation_date', lc.incorporation_date) AS jsonb_build_object
               FROM "ob-poc".entity_limited_companies lc
              WHERE (lc.entity_id = e.entity_id))
            WHEN ((et.type_code)::text ~~ 'PARTNERSHIP%'::text) THEN ( SELECT jsonb_build_object('partnership_name', p.partnership_name, 'partnership_type', p.partnership_type, 'jurisdiction', p.jurisdiction, 'formation_date', p.formation_date) AS jsonb_build_object
               FROM "ob-poc".entity_partnerships p
              WHERE (p.entity_id = e.entity_id))
            WHEN ((et.type_code)::text ~~ 'TRUST%'::text) THEN ( SELECT jsonb_build_object('trust_name', t.trust_name, 'trust_type', t.trust_type, 'governing_law', t.governing_law, 'establishment_date', t.establishment_date) AS jsonb_build_object
               FROM "ob-poc".entity_trusts t
              WHERE (t.entity_id = e.entity_id))
            ELSE NULL::jsonb
        END AS entity_details
   FROM ((((("ob-poc".cbus c
     JOIN "ob-poc".cbu_entity_roles cer ON ((c.cbu_id = cer.cbu_id)))
     JOIN "ob-poc".entities e ON ((cer.entity_id = e.entity_id)))
     JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
     JOIN "ob-poc".roles r ON ((cer.role_id = r.role_id)))
     LEFT JOIN LATERAL ( SELECT ew2.workstream_id,
            ew2.case_id,
            ew2.entity_id,
            ew2.status,
            ew2.discovery_source_workstream_id,
            ew2.discovery_reason,
            ew2.risk_rating,
            ew2.risk_factors,
            ew2.created_at,
            ew2.started_at,
            ew2.completed_at,
            ew2.blocked_at,
            ew2.blocked_reason,
            ew2.requires_enhanced_dd,
            ew2.is_ubo,
            ew2.ownership_percentage,
            ew2.discovery_depth
           FROM (kyc.cases kc
             JOIN kyc.entity_workstreams ew2 ON ((kc.case_id = ew2.case_id)))
          WHERE ((kc.cbu_id = c.cbu_id) AND (ew2.entity_id = e.entity_id))
          ORDER BY ew2.created_at DESC
         LIMIT 1) ew ON (true));


--
-- Name: VIEW v_cbu_entity_graph; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_entity_graph IS 'Complete CBU entity relationship graph with roles, KYC status, and entity details. Use for visualization and entity queries.';


--
-- Name: v_cbu_entity_with_roles; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_entity_with_roles AS
 WITH role_data AS (
         SELECT cer.cbu_id,
            cer.entity_id,
            e.name AS entity_name,
            et.type_code AS entity_type,
            et.entity_category,
            COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) AS jurisdiction,
            r.name AS role_name,
            r.role_category,
            r.layout_category,
            r.display_priority,
            r.ubo_treatment,
            r.requires_percentage,
            r.kyc_obligation
           FROM ((((((("ob-poc".cbu_entity_roles cer
             JOIN "ob-poc".entities e ON ((cer.entity_id = e.entity_id)))
             JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
             JOIN "ob-poc".roles r ON ((cer.role_id = r.role_id)))
             LEFT JOIN "ob-poc".entity_limited_companies lc ON ((e.entity_id = lc.entity_id)))
             LEFT JOIN "ob-poc".entity_partnerships p ON ((e.entity_id = p.entity_id)))
             LEFT JOIN "ob-poc".entity_trusts t ON ((e.entity_id = t.entity_id)))
             LEFT JOIN "ob-poc".entity_proper_persons pp ON ((e.entity_id = pp.entity_id)))
        )
 SELECT cbu_id,
    entity_id,
    entity_name,
    entity_type,
    entity_category,
    jurisdiction,
    array_agg(role_name ORDER BY display_priority DESC) AS roles,
    array_agg(DISTINCT role_category) FILTER (WHERE (role_category IS NOT NULL)) AS role_categories,
    array_agg(DISTINCT layout_category) FILTER (WHERE (layout_category IS NOT NULL)) AS layout_categories,
    (array_agg(role_name ORDER BY display_priority DESC))[1] AS primary_role,
    (array_agg(role_category ORDER BY display_priority DESC) FILTER (WHERE (role_category IS NOT NULL)))[1] AS primary_role_category,
    (array_agg(layout_category ORDER BY display_priority DESC) FILTER (WHERE (layout_category IS NOT NULL)))[1] AS primary_layout_category,
    max(display_priority) AS max_role_priority,
        CASE
            WHEN ('ALWAYS_UBO'::text = ANY ((array_agg(ubo_treatment))::text[])) THEN 'ALWAYS_UBO'::text
            WHEN ('TERMINUS'::text = ANY ((array_agg(ubo_treatment))::text[])) THEN 'TERMINUS'::text
            WHEN ('CONTROL_PRONG'::text = ANY ((array_agg(ubo_treatment))::text[])) THEN 'CONTROL_PRONG'::text
            WHEN ('BY_PERCENTAGE'::text = ANY ((array_agg(ubo_treatment))::text[])) THEN 'BY_PERCENTAGE'::text
            WHEN ('LOOK_THROUGH'::text = ANY ((array_agg(ubo_treatment))::text[])) THEN 'LOOK_THROUGH'::text
            ELSE 'NOT_APPLICABLE'::text
        END AS effective_ubo_treatment,
        CASE
            WHEN ('FULL_KYC'::text = ANY ((array_agg(kyc_obligation))::text[])) THEN 'FULL_KYC'::text
            WHEN ('SCREEN_AND_ID'::text = ANY ((array_agg(kyc_obligation))::text[])) THEN 'SCREEN_AND_ID'::text
            WHEN ('SIMPLIFIED'::text = ANY ((array_agg(kyc_obligation))::text[])) THEN 'SIMPLIFIED'::text
            WHEN ('SCREEN_ONLY'::text = ANY ((array_agg(kyc_obligation))::text[])) THEN 'SCREEN_ONLY'::text
            ELSE 'RECORD_ONLY'::text
        END AS effective_kyc_obligation
   FROM role_data
  GROUP BY cbu_id, entity_id, entity_name, entity_type, entity_category, jurisdiction;


--
-- Name: VIEW v_cbu_entity_with_roles; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_entity_with_roles IS 'Aggregated view of entities with their roles, categories, and effective KYC/UBO treatment.
Fixed in V2.1: Added primary_role_category, renamed primary_layout to primary_layout_category.';


--
-- Name: v_cbu_investor_details; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_investor_details AS
 SELECT sc.cbu_id,
    h.share_class_id,
    sc.name AS share_class_name,
    h.investor_entity_id,
    e.name AS investor_name,
    et.type_code AS investor_type,
    h.units,
    h.cost_basis AS value,
    COALESCE(lc.jurisdiction, pp.nationality) AS jurisdiction
   FROM (((((kyc.holdings h
     JOIN kyc.share_classes sc ON ((h.share_class_id = sc.id)))
     JOIN "ob-poc".entities e ON ((h.investor_entity_id = e.entity_id)))
     JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
     LEFT JOIN "ob-poc".entity_limited_companies lc ON ((e.entity_id = lc.entity_id)))
     LEFT JOIN "ob-poc".entity_proper_persons pp ON ((e.entity_id = pp.entity_id)))
  WHERE ((h.status)::text = 'active'::text);


--
-- Name: v_cbu_investor_groups; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_investor_groups AS
 SELECT sc.cbu_id,
    h.share_class_id,
    sc.name AS share_class_name,
    sc.currency,
    sc.isin,
    count(DISTINCT h.investor_entity_id) AS investor_count,
    sum(h.units) AS total_units,
    sum(h.cost_basis) AS total_value
   FROM (kyc.holdings h
     JOIN kyc.share_classes sc ON ((h.share_class_id = sc.id)))
  WHERE ((h.status)::text = 'active'::text)
  GROUP BY sc.cbu_id, h.share_class_id, sc.name, sc.currency, sc.isin;


--
-- Name: v_cbu_kyc_scope; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_kyc_scope AS
 SELECT c.cbu_id,
    c.name AS cbu_name,
    c.client_type,
    c.kyc_scope_template,
    cer.entity_id,
    e.name AS entity_name,
    et.name AS entity_type,
    ro.name AS role_name,
    ro.role_id,
    rtypes.role_code,
    rtypes.triggers_full_kyc,
    rtypes.triggers_screening,
    rtypes.triggers_id_verification,
    rtypes.check_regulatory_status,
    rtypes.if_regulated_obligation,
    COALESCE(erp.is_regulated, false) AS is_regulated,
    erp.regulator_code,
    erp.registration_verified,
    erp.regulatory_tier,
    COALESCE(regtier.allows_simplified_dd, false) AS allows_simplified_dd
   FROM ((((((("ob-poc".cbus c
     JOIN "ob-poc".cbu_entity_roles cer ON ((c.cbu_id = cer.cbu_id)))
     JOIN "ob-poc".entities e ON ((cer.entity_id = e.entity_id)))
     JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
     JOIN "ob-poc".roles ro ON ((cer.role_id = ro.role_id)))
     LEFT JOIN "ob-poc".role_types rtypes ON ((upper((ro.name)::text) = (rtypes.role_code)::text)))
     LEFT JOIN "ob-poc".entity_regulatory_profiles erp ON ((e.entity_id = erp.entity_id)))
     LEFT JOIN "ob-poc".regulatory_tiers regtier ON (((erp.regulatory_tier)::text = (regtier.tier_code)::text)));


--
-- Name: v_cbu_kyc_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_kyc_summary AS
 WITH latest_case AS (
         SELECT DISTINCT ON (cases.cbu_id) cases.cbu_id,
            cases.case_id,
            cases.case_type,
            cases.status,
            cases.risk_rating,
            cases.assigned_analyst_id AS assigned_to,
            cases.opened_at,
            cases.last_activity_at AS updated_at
           FROM kyc.cases
          ORDER BY cases.cbu_id, cases.opened_at DESC
        ), entity_summary AS (
         SELECT c_1.cbu_id,
            count(DISTINCT ew.entity_id) AS total_entities,
            count(DISTINCT ew.entity_id) FILTER (WHERE ((ew.status)::text = 'COMPLETE'::text)) AS approved_entities,
            count(DISTINCT ew.entity_id) FILTER (WHERE ((ew.status)::text = ANY ((ARRAY['PENDING'::character varying, 'COLLECT'::character varying, 'VERIFY'::character varying, 'SCREEN'::character varying, 'ASSESS'::character varying, 'ENHANCED_DD'::character varying])::text[]))) AS pending_entities,
            count(DISTINCT ew.entity_id) FILTER (WHERE (((ew.status)::text = 'BLOCKED'::text) OR ((ew.risk_rating)::text = 'PROHIBITED'::text))) AS blocked_entities,
            max(
                CASE ew.risk_rating
                    WHEN 'PROHIBITED'::text THEN 5
                    WHEN 'VERY_HIGH'::text THEN 4
                    WHEN 'HIGH'::text THEN 3
                    WHEN 'MEDIUM'::text THEN 2
                    WHEN 'LOW'::text THEN 1
                    ELSE 0
                END) AS max_risk_score
           FROM (kyc.cases c_1
             JOIN kyc.entity_workstreams ew ON ((c_1.case_id = ew.case_id)))
          GROUP BY c_1.cbu_id
        ), open_cases AS (
         SELECT cases.cbu_id,
            count(*) AS open_case_count
           FROM kyc.cases
          WHERE ((cases.status)::text <> ALL ((ARRAY['APPROVED'::character varying, 'REJECTED'::character varying, 'WITHDRAWN'::character varying, 'EXPIRED'::character varying])::text[]))
          GROUP BY cases.cbu_id
        ), allegation_summary AS (
         SELECT client_allegations.cbu_id,
            count(*) AS total_allegations,
            count(*) FILTER (WHERE ((client_allegations.verification_status)::text = 'PENDING'::text)) AS pending_allegations,
            count(*) FILTER (WHERE ((client_allegations.verification_status)::text = 'CONTRADICTED'::text)) AS contradicted_allegations
           FROM "ob-poc".client_allegations
          GROUP BY client_allegations.cbu_id
        )
 SELECT c.cbu_id,
    c.name AS cbu_name,
    c.cbu_category,
    c.jurisdiction,
        CASE
            WHEN (es.blocked_entities > 0) THEN 'BLOCKED'::text
            WHEN (es.pending_entities > 0) THEN 'PENDING'::text
            WHEN ((es.approved_entities = es.total_entities) AND (es.total_entities > 0)) THEN 'APPROVED'::text
            WHEN (es.total_entities = 0) THEN 'NO_ENTITIES'::text
            ELSE 'PARTIAL'::text
        END AS overall_kyc_status,
        CASE es.max_risk_score
            WHEN 5 THEN 'PROHIBITED'::text
            WHEN 4 THEN 'VERY_HIGH'::text
            WHEN 3 THEN 'HIGH'::text
            WHEN 2 THEN 'MEDIUM'::text
            WHEN 1 THEN 'LOW'::text
            ELSE 'UNRATED'::text
        END AS overall_risk_rating,
    lc.updated_at AS last_kyc_activity,
    lc.case_id AS primary_case_id,
    lc.case_type AS primary_case_type,
    lc.status AS primary_case_status,
    lc.risk_rating AS case_risk_rating,
    lc.assigned_to AS case_owner,
    COALESCE(oc.open_case_count, (0)::bigint) AS open_case_count,
    COALESCE(es.total_entities, (0)::bigint) AS total_entities,
    COALESCE(es.approved_entities, (0)::bigint) AS approved_entities,
    COALESCE(es.pending_entities, (0)::bigint) AS pending_entities,
    COALESCE(es.blocked_entities, (0)::bigint) AS blocked_entities,
    COALESCE(als.total_allegations, (0)::bigint) AS total_allegations,
    COALESCE(als.pending_allegations, (0)::bigint) AS pending_allegations,
    COALESCE(als.contradicted_allegations, (0)::bigint) AS contradicted_allegations,
    (COALESCE(es.blocked_entities, (0)::bigint) > 0) AS has_blocked_entities,
    (COALESCE(als.contradicted_allegations, (0)::bigint) > 0) AS has_contradictions
   FROM (((("ob-poc".cbus c
     LEFT JOIN latest_case lc ON ((c.cbu_id = lc.cbu_id)))
     LEFT JOIN entity_summary es ON ((c.cbu_id = es.cbu_id)))
     LEFT JOIN open_cases oc ON ((c.cbu_id = oc.cbu_id)))
     LEFT JOIN allegation_summary als ON ((c.cbu_id = als.cbu_id)));


--
-- Name: VIEW v_cbu_kyc_summary; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_kyc_summary IS 'KYC-focused CBU summary: overall status, risk rating, entity breakdown. Use for dashboards and compliance queries.';


--
-- Name: v_cbu_lifecycle; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_lifecycle AS
 WITH latest_kyc_case AS (
         SELECT DISTINCT ON (cases.cbu_id) cases.cbu_id,
            cases.case_id,
            cases.case_type,
            cases.status AS case_status,
            cases.risk_rating AS case_risk_rating,
            cases.opened_at AS case_created_at
           FROM kyc.cases
          ORDER BY cases.cbu_id, cases.opened_at DESC
        ), entity_kyc_agg AS (
         SELECT c_1.cbu_id,
            count(DISTINCT ew.entity_id) AS entity_count,
            count(DISTINCT ew.entity_id) FILTER (WHERE ((ew.status)::text = 'COMPLETE'::text)) AS approved_count,
            count(DISTINCT ew.entity_id) FILTER (WHERE ((ew.status)::text = 'BLOCKED'::text)) AS rejected_count,
            count(DISTINCT ew.entity_id) FILTER (WHERE ((ew.status)::text = ANY ((ARRAY['PENDING'::character varying, 'COLLECT'::character varying, 'VERIFY'::character varying, 'SCREEN'::character varying, 'ASSESS'::character varying])::text[]))) AS pending_count,
            count(DISTINCT ew.entity_id) FILTER (WHERE ((ew.risk_rating)::text = 'PROHIBITED'::text)) AS prohibited_count,
            max((ew.risk_rating)::text) AS max_risk_rating
           FROM (kyc.cases c_1
             JOIN kyc.entity_workstreams ew ON ((c_1.case_id = ew.case_id)))
          GROUP BY c_1.cbu_id
        ), service_agg AS (
         SELECT service_delivery_map.cbu_id,
            count(*) AS service_count,
            count(*) FILTER (WHERE ((service_delivery_map.delivery_status)::text = 'DELIVERED'::text)) AS delivered_count,
            count(*) FILTER (WHERE ((service_delivery_map.delivery_status)::text = 'PENDING'::text)) AS pending_count,
            count(*) FILTER (WHERE ((service_delivery_map.delivery_status)::text = 'FAILED'::text)) AS failed_count
           FROM "ob-poc".service_delivery_map
          GROUP BY service_delivery_map.cbu_id
        ), resource_agg AS (
         SELECT cbu_resource_instances.cbu_id,
            count(*) AS resource_count,
            count(*) FILTER (WHERE ((cbu_resource_instances.status)::text = 'ACTIVE'::text)) AS active_count,
            count(*) FILTER (WHERE ((cbu_resource_instances.status)::text = 'PENDING'::text)) AS pending_count,
            count(*) FILTER (WHERE ((cbu_resource_instances.status)::text = 'SUSPENDED'::text)) AS suspended_count
           FROM "ob-poc".cbu_resource_instances
          GROUP BY cbu_resource_instances.cbu_id
        )
 SELECT c.cbu_id,
    c.name,
    c.cbu_category,
    c.client_type,
    c.jurisdiction,
        CASE
            WHEN (((lkc.case_type)::text = 'NEW_CLIENT'::text) AND ((lkc.case_status)::text = ANY ((ARRAY['INTAKE'::character varying, 'DISCOVERY'::character varying])::text[]))) THEN 'ONBOARDING'::text
            WHEN (((lkc.case_type)::text = 'NEW_CLIENT'::text) AND ((lkc.case_status)::text = 'APPROVED'::text)) THEN 'ONBOARDED'::text
            WHEN (((lkc.case_type)::text = 'NEW_CLIENT'::text) AND ((lkc.case_status)::text = 'REJECTED'::text)) THEN 'REJECTED'::text
            WHEN (lkc.case_id IS NULL) THEN 'PROSPECT'::text
            ELSE 'IN_PROGRESS'::text
        END AS onboarding_state,
        CASE
            WHEN (eka.prohibited_count > 0) THEN 'PROHIBITED'::text
            WHEN (eka.rejected_count > 0) THEN 'BLOCKED'::text
            WHEN (eka.pending_count > 0) THEN 'PENDING_KYC'::text
            WHEN ((eka.approved_count = eka.entity_count) AND (eka.entity_count > 0)) THEN 'CLEARED'::text
            WHEN (eka.entity_count = 0) THEN 'NO_ENTITIES'::text
            ELSE 'PARTIAL'::text
        END AS kyc_overall_state,
        CASE
            WHEN (sa.failed_count > 0) THEN 'SERVICE_FAILED'::text
            WHEN (sa.pending_count > 0) THEN 'SERVICES_PENDING'::text
            WHEN ((sa.delivered_count = sa.service_count) AND (sa.service_count > 0)) THEN 'FULLY_SERVICED'::text
            WHEN (sa.service_count = 0) THEN 'NO_SERVICES'::text
            ELSE 'PARTIAL_SERVICES'::text
        END AS service_state,
        CASE
            WHEN (ra.suspended_count > 0) THEN 'SUSPENDED'::text
            WHEN (ra.active_count > 0) THEN 'OPERATIONAL'::text
            WHEN (ra.pending_count > 0) THEN 'PENDING_RESOURCES'::text
            ELSE 'NO_RESOURCES'::text
        END AS resource_state,
        CASE
            WHEN (eka.prohibited_count > 0) THEN 'BLOCKED'::text
            WHEN ((lkc.case_status)::text = 'REJECTED'::text) THEN 'REJECTED'::text
            WHEN (ra.suspended_count > 0) THEN 'SUSPENDED'::text
            WHEN ((ra.active_count > 0) AND (eka.approved_count = eka.entity_count) AND (eka.entity_count > 0)) THEN 'ACTIVE'::text
            WHEN (((lkc.case_type)::text = 'NEW_CLIENT'::text) AND ((lkc.case_status)::text = ANY ((ARRAY['INTAKE'::character varying, 'DISCOVERY'::character varying, 'ASSESSMENT'::character varying])::text[]))) THEN 'ONBOARDING'::text
            WHEN (lkc.case_id IS NULL) THEN 'PROSPECT'::text
            ELSE 'IN_PROGRESS'::text
        END AS composite_lifecycle,
    lkc.case_id AS latest_case_id,
    lkc.case_status AS latest_case_status,
    lkc.case_risk_rating,
    eka.max_risk_rating AS entity_max_risk,
    COALESCE(eka.entity_count, (0)::bigint) AS entity_count,
    COALESCE(sa.service_count, (0)::bigint) AS service_count,
    COALESCE(ra.resource_count, (0)::bigint) AS resource_count,
    COALESCE(ra.active_count, (0)::bigint) AS active_resource_count
   FROM (((("ob-poc".cbus c
     LEFT JOIN latest_kyc_case lkc ON ((c.cbu_id = lkc.cbu_id)))
     LEFT JOIN entity_kyc_agg eka ON ((c.cbu_id = eka.cbu_id)))
     LEFT JOIN service_agg sa ON ((c.cbu_id = sa.cbu_id)))
     LEFT JOIN resource_agg ra ON ((c.cbu_id = ra.cbu_id)));


--
-- Name: VIEW v_cbu_lifecycle; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_lifecycle IS 'Derived CBU lifecycle state - composite of KYC cases/workstreams, services, and resources. Use this instead of storing status on CBU directly.';


--
-- Name: v_cbu_lifecycle_coverage; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_lifecycle_coverage AS
 SELECT u.cbu_id,
    u.universe_id,
    ic.code AS instrument_class,
    m.mic AS market,
    u.counterparty_entity_id,
    l.code AS lifecycle_code,
    l.name AS lifecycle_name,
    il.is_mandatory,
    il.requires_isda,
    ( SELECT count(*) AS count
           FROM "ob-poc".lifecycle_resource_capabilities lrc
          WHERE ((lrc.lifecycle_id = l.lifecycle_id) AND (lrc.is_required = true))) AS required_resource_count,
    ( SELECT count(*) AS count
           FROM ("ob-poc".lifecycle_resource_capabilities lrc
             JOIN "ob-poc".cbu_lifecycle_instances cli ON (((cli.resource_type_id = lrc.resource_type_id) AND (cli.cbu_id = u.cbu_id) AND ((cli.status)::text = ANY ((ARRAY['PROVISIONED'::character varying, 'ACTIVE'::character varying])::text[])) AND ((cli.market_id IS NULL) OR (cli.market_id = u.market_id)) AND ((cli.counterparty_entity_id IS NULL) OR (cli.counterparty_entity_id = u.counterparty_entity_id)))))
          WHERE ((lrc.lifecycle_id = l.lifecycle_id) AND (lrc.is_required = true))) AS provisioned_resource_count,
        CASE
            WHEN (( SELECT count(*) AS count
               FROM "ob-poc".lifecycle_resource_capabilities lrc
              WHERE ((lrc.lifecycle_id = l.lifecycle_id) AND (lrc.is_required = true))) = ( SELECT count(*) AS count
               FROM ("ob-poc".lifecycle_resource_capabilities lrc
                 JOIN "ob-poc".cbu_lifecycle_instances cli ON (((cli.resource_type_id = lrc.resource_type_id) AND (cli.cbu_id = u.cbu_id) AND ((cli.status)::text = ANY ((ARRAY['PROVISIONED'::character varying, 'ACTIVE'::character varying])::text[])))))
              WHERE ((lrc.lifecycle_id = l.lifecycle_id) AND (lrc.is_required = true)))) THEN true
            ELSE false
        END AS is_fully_provisioned
   FROM ((((custody.cbu_instrument_universe u
     JOIN custody.instrument_classes ic ON ((ic.class_id = u.instrument_class_id)))
     LEFT JOIN custody.markets m ON ((m.market_id = u.market_id)))
     JOIN "ob-poc".instrument_lifecycles il ON ((il.instrument_class_id = u.instrument_class_id)))
     JOIN "ob-poc".lifecycles l ON ((l.lifecycle_id = il.lifecycle_id)))
  WHERE ((il.is_active = true) AND (l.is_active = true));


--
-- Name: VIEW v_cbu_lifecycle_coverage; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_lifecycle_coverage IS 'Shows lifecycle coverage status for each CBU universe entry';


--
-- Name: v_cbu_lifecycle_gaps; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_lifecycle_gaps AS
 SELECT u.cbu_id,
    c.name AS cbu_name,
    ic.code AS instrument_class,
    m.mic AS market,
    e.name AS counterparty_name,
    l.code AS lifecycle_code,
    l.name AS lifecycle_name,
    il.is_mandatory,
    lrt.code AS missing_resource_code,
    lrt.name AS missing_resource_name,
    lrt.provisioning_verb,
    lrt.location_type,
    lrt.per_market,
    lrt.per_currency,
    lrt.per_counterparty
   FROM ((((((((custody.cbu_instrument_universe u
     JOIN "ob-poc".cbus c ON ((c.cbu_id = u.cbu_id)))
     JOIN custody.instrument_classes ic ON ((ic.class_id = u.instrument_class_id)))
     LEFT JOIN custody.markets m ON ((m.market_id = u.market_id)))
     LEFT JOIN "ob-poc".entities e ON ((e.entity_id = u.counterparty_entity_id)))
     JOIN "ob-poc".instrument_lifecycles il ON ((il.instrument_class_id = u.instrument_class_id)))
     JOIN "ob-poc".lifecycles l ON ((l.lifecycle_id = il.lifecycle_id)))
     JOIN "ob-poc".lifecycle_resource_capabilities lrc ON (((lrc.lifecycle_id = l.lifecycle_id) AND (lrc.is_required = true))))
     JOIN "ob-poc".lifecycle_resource_types lrt ON ((lrt.resource_type_id = lrc.resource_type_id)))
  WHERE ((il.is_active = true) AND (l.is_active = true) AND (NOT (EXISTS ( SELECT 1
           FROM "ob-poc".cbu_lifecycle_instances cli
          WHERE ((cli.cbu_id = u.cbu_id) AND (cli.resource_type_id = lrt.resource_type_id) AND ((cli.status)::text = ANY ((ARRAY['PROVISIONED'::character varying, 'ACTIVE'::character varying])::text[])) AND ((cli.market_id IS NULL) OR (cli.market_id = u.market_id) OR (NOT lrt.per_market)) AND ((cli.counterparty_entity_id IS NULL) OR (cli.counterparty_entity_id = u.counterparty_entity_id) OR (NOT lrt.per_counterparty)))))));


--
-- Name: VIEW v_cbu_lifecycle_gaps; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_lifecycle_gaps IS 'Shows missing lifecycle resources for CBU universe entries';


--
-- Name: v_cbu_matrix_effective; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_matrix_effective AS
 WITH matrix_base AS (
         SELECT u.universe_id,
            u.cbu_id,
            c.name AS cbu_name,
            u.instrument_class_id,
            ic.code AS instrument_class,
            ic.name AS instrument_class_name,
            u.market_id,
            m.mic AS market,
            m.name AS market_name,
            u.currencies,
            u.counterparty_entity_id,
            e.name AS counterparty_name,
            u.is_held,
            u.is_traded,
            u.is_active
           FROM ((((custody.cbu_instrument_universe u
             JOIN "ob-poc".cbus c ON ((c.cbu_id = u.cbu_id)))
             JOIN custody.instrument_classes ic ON ((ic.class_id = u.instrument_class_id)))
             LEFT JOIN custody.markets m ON ((m.market_id = u.market_id)))
             LEFT JOIN "ob-poc".entities e ON ((e.entity_id = u.counterparty_entity_id)))
          WHERE (u.is_active = true)
        ), product_overlays AS (
         SELECT o.cbu_id,
            o.instrument_class_id,
            o.market_id,
            o.currency,
            o.counterparty_entity_id,
            p.product_id,
            p.product_code,
            p.name AS product_name,
            o.additional_services,
            o.additional_slas,
            o.additional_resources,
            o.product_specific_config,
            o.status AS overlay_status
           FROM (("ob-poc".cbu_matrix_product_overlay o
             JOIN "ob-poc".cbu_product_subscriptions ps ON ((ps.subscription_id = o.subscription_id)))
             JOIN "ob-poc".products p ON ((p.product_id = ps.product_id)))
          WHERE (((o.status)::text = 'ACTIVE'::text) AND ((ps.status)::text = 'ACTIVE'::text))
        )
 SELECT mb.universe_id,
    mb.cbu_id,
    mb.cbu_name,
    mb.instrument_class_id,
    mb.instrument_class,
    mb.instrument_class_name,
    mb.market_id,
    mb.market,
    mb.market_name,
    mb.currencies,
    mb.counterparty_entity_id,
    mb.counterparty_name,
    mb.is_held,
    mb.is_traded,
    COALESCE(jsonb_agg(jsonb_build_object('product_code', po.product_code, 'product_name', po.product_name, 'additional_services', po.additional_services, 'additional_slas', po.additional_slas, 'additional_resources', po.additional_resources, 'config', po.product_specific_config)) FILTER (WHERE (po.product_code IS NOT NULL)), '[]'::jsonb) AS product_overlays,
    count(po.product_code) AS overlay_count
   FROM (matrix_base mb
     LEFT JOIN product_overlays po ON (((po.cbu_id = mb.cbu_id) AND ((po.instrument_class_id IS NULL) OR (po.instrument_class_id = mb.instrument_class_id)) AND ((po.market_id IS NULL) OR (po.market_id = mb.market_id)) AND ((po.counterparty_entity_id IS NULL) OR (po.counterparty_entity_id = mb.counterparty_entity_id)))))
  GROUP BY mb.universe_id, mb.cbu_id, mb.cbu_name, mb.instrument_class_id, mb.instrument_class, mb.instrument_class_name, mb.market_id, mb.market, mb.market_name, mb.currencies, mb.counterparty_entity_id, mb.counterparty_name, mb.is_held, mb.is_traded;


--
-- Name: v_cbu_products; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_products AS
 SELECT ps.subscription_id,
    ps.cbu_id,
    c.name AS cbu_name,
    ps.product_id,
    p.product_code,
    p.name AS product_name,
    p.product_category,
    ps.status,
    ps.effective_from,
    ps.effective_to,
    ps.config,
    ( SELECT count(*) AS count
           FROM "ob-poc".cbu_matrix_product_overlay o
          WHERE (o.subscription_id = ps.subscription_id)) AS overlay_count
   FROM (("ob-poc".cbu_product_subscriptions ps
     JOIN "ob-poc".cbus c ON ((c.cbu_id = ps.cbu_id)))
     JOIN "ob-poc".products p ON ((p.product_id = ps.product_id)));


--
-- Name: v_cbu_service_gaps; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_service_gaps AS
 WITH cbu_products AS (
         SELECT DISTINCT c.cbu_id,
            c.name AS cbu_name,
            p.product_id,
            p.product_code,
            p.name AS product_name
           FROM ("ob-poc".cbus c
             LEFT JOIN "ob-poc".products p ON ((p.product_id = c.product_id)))
          WHERE ((p.product_id IS NOT NULL) AND (p.is_active = true))
        UNION
         SELECT DISTINCT c.cbu_id,
            c.name AS cbu_name,
            p.product_id,
            p.product_code,
            p.name AS product_name
           FROM ((("ob-poc".cbus c
             JOIN "ob-poc".onboarding_requests orq ON ((orq.cbu_id = c.cbu_id)))
             JOIN "ob-poc".onboarding_products op ON ((op.request_id = orq.request_id)))
             JOIN "ob-poc".products p ON ((p.product_id = op.product_id)))
          WHERE (p.is_active = true)
        ), required_resources AS (
         SELECT cp.cbu_id,
            cp.cbu_name,
            cp.product_code,
            cp.product_name,
            s.service_id,
            s.service_code,
            s.name AS service_name,
            ps.is_mandatory,
            srt.resource_id AS resource_type_id,
            srt.resource_code,
            srt.name AS resource_name,
            srt.provisioning_verb,
            srt.location_type,
            srt.per_market,
            srt.per_currency,
            srt.per_counterparty,
            COALESCE(src.is_required, true) AS is_required
           FROM ((((cbu_products cp
             JOIN "ob-poc".product_services ps ON ((ps.product_id = cp.product_id)))
             JOIN "ob-poc".services s ON (((s.service_id = ps.service_id) AND (s.is_active = true))))
             JOIN "ob-poc".service_resource_capabilities src ON (((src.service_id = s.service_id) AND (src.is_active = true))))
             JOIN "ob-poc".service_resource_types srt ON (((srt.resource_id = src.resource_id) AND (srt.is_active = true))))
          WHERE (COALESCE(src.is_required, true) = true)
        )
 SELECT cbu_id,
    cbu_name,
    product_code,
    product_name,
    service_code,
    service_name,
    is_mandatory,
    resource_code AS missing_resource_code,
    resource_name AS missing_resource_name,
    provisioning_verb,
    location_type,
    per_market,
    per_currency,
    per_counterparty
   FROM required_resources rr
  WHERE (NOT (EXISTS ( SELECT 1
           FROM "ob-poc".cbu_resource_instances cri
          WHERE ((cri.cbu_id = rr.cbu_id) AND (cri.resource_type_id = rr.resource_type_id) AND ((cri.status)::text = ANY ((ARRAY['PENDING'::character varying, 'ACTIVE'::character varying, 'PROVISIONED'::character varying])::text[]))))))
  ORDER BY cbu_name, product_code, service_code, resource_code;


--
-- Name: VIEW v_cbu_service_gaps; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_cbu_service_gaps IS 'Shows missing required service resources for each CBU based on their products';


--
-- Name: v_cbu_unified_gaps; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_unified_gaps AS
 SELECT g.cbu_id,
    g.cbu_name,
    'LIFECYCLE'::text AS gap_source,
    g.instrument_class,
    g.market,
    g.counterparty_name,
    NULL::character varying AS product_code,
    g.lifecycle_code AS operation_code,
    g.lifecycle_name AS operation_name,
    g.missing_resource_code,
    g.missing_resource_name,
    g.provisioning_verb,
    g.location_type,
    g.per_market,
    g.per_currency,
    g.per_counterparty,
    g.is_mandatory AS is_required
   FROM "ob-poc".v_cbu_lifecycle_gaps g
UNION ALL
 SELECT g.cbu_id,
    g.cbu_name,
    'SERVICE'::text AS gap_source,
    NULL::character varying AS instrument_class,
    NULL::character varying AS market,
    NULL::character varying AS counterparty_name,
    g.product_code,
    g.service_code AS operation_code,
    g.service_name AS operation_name,
    g.missing_resource_code,
    g.missing_resource_name,
    g.provisioning_verb,
    g.location_type,
    g.per_market,
    g.per_currency,
    g.per_counterparty,
    g.is_mandatory AS is_required
   FROM "ob-poc".v_cbu_service_gaps g;


--
-- Name: v_cbu_validation_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_cbu_validation_summary AS
 SELECT c.cbu_id,
    c.name,
    c.status AS cbu_status,
    c.client_type,
    c.jurisdiction,
    count(e.evidence_id) AS total_evidence,
    count(e.evidence_id) FILTER (WHERE ((e.verification_status)::text = 'VERIFIED'::text)) AS verified_evidence,
    count(e.evidence_id) FILTER (WHERE ((e.verification_status)::text = 'PENDING'::text)) AS pending_evidence,
    count(e.evidence_id) FILTER (WHERE ((e.verification_status)::text = 'REJECTED'::text)) AS rejected_evidence,
    array_agg(DISTINCT e.evidence_category) FILTER (WHERE ((e.verification_status)::text = 'VERIFIED'::text)) AS verified_categories,
    max(e.verified_at) AS last_verification_at,
    ( SELECT count(*) AS count
           FROM "ob-poc".cbu_change_log cl
          WHERE (cl.cbu_id = c.cbu_id)) AS change_count
   FROM ("ob-poc".cbus c
     LEFT JOIN "ob-poc".cbu_evidence e ON ((c.cbu_id = e.cbu_id)))
  GROUP BY c.cbu_id, c.name, c.status, c.client_type, c.jurisdiction;


--
-- Name: v_document_extraction_map; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_document_extraction_map AS
 SELECT dt.type_code AS document_type,
    dt.display_name AS document_name,
    ar.id AS attribute_id,
    ar.display_name AS attribute_name,
    dal.direction,
    dal.extraction_method,
    dal.is_authoritative,
    dal.proof_strength
   FROM (("ob-poc".document_attribute_links dal
     JOIN "ob-poc".document_types dt ON ((dal.document_type_id = dt.type_id)))
     JOIN "ob-poc".attribute_registry ar ON ((dal.attribute_id = ar.uuid)))
  WHERE (dal.is_active = true)
  ORDER BY dt.type_code, dal.direction, ar.id;


--
-- Name: v_entity_regulatory_status; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_entity_regulatory_status AS
 SELECT e.entity_id,
    e.name AS entity_name,
    et.name AS entity_type,
    COALESCE(erp.is_regulated, false) AS is_regulated,
    erp.regulator_code,
    r.name AS regulator_name,
    erp.registration_number,
    erp.registration_verified,
    erp.regulatory_tier,
    rt.allows_simplified_dd,
    rt.requires_enhanced_screening
   FROM (((("ob-poc".entities e
     JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
     LEFT JOIN "ob-poc".entity_regulatory_profiles erp ON ((e.entity_id = erp.entity_id)))
     LEFT JOIN "ob-poc".regulators r ON (((erp.regulator_code)::text = (r.regulator_code)::text)))
     LEFT JOIN "ob-poc".regulatory_tiers rt ON (((erp.regulatory_tier)::text = (rt.tier_code)::text)));


--
-- Name: v_execution_audit_with_view; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_execution_audit_with_view AS
 SELECT e.idempotency_key,
    e.execution_id,
    e.verb_hash,
    e.verb AS verb_name,
    e.result_id,
    e.created_at AS executed_at,
    e.input_selection,
    COALESCE(array_length(e.input_selection, 1), 0) AS selection_count,
    (e.input_view_state ->> 'context'::text) AS view_context,
    (e.output_view_state IS NOT NULL) AS produced_view_state,
    e.source,
    e.request_id,
    e.actor_id,
    e.actor_type,
    v.domain,
    v.description
   FROM ("ob-poc".dsl_idempotency e
     LEFT JOIN "ob-poc".dsl_verbs v ON ((e.verb_hash = v.compiled_hash)))
  ORDER BY e.created_at DESC;


--
-- Name: VIEW v_execution_audit_with_view; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_execution_audit_with_view IS 'Complete execution audit trail with view state and source attribution';


--
-- Name: v_execution_verb_audit; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_execution_verb_audit AS
 SELECT execution_id,
    cbu_id,
    execution_phase,
    status,
    started_at,
    completed_at,
    duration_ms,
    executed_by,
    COALESCE(array_length(verb_names, 1), 0) AS verb_count,
    verb_names,
    (EXISTS ( SELECT 1
           FROM (UNNEST(el.verb_hashes, el.verb_names) t(hash, name)
             JOIN "ob-poc".dsl_verbs v ON ((v.verb_name = t.name)))
          WHERE (v.compiled_hash IS DISTINCT FROM t.hash))) AS has_stale_verb_refs
   FROM "ob-poc".dsl_execution_log el
  WHERE (verb_hashes IS NOT NULL);


--
-- Name: VIEW v_execution_verb_audit; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_execution_verb_audit IS 'Execution log with verb versioning audit info. has_stale_verb_refs=true means verb config changed since execution.';


--
-- Name: v_open_discrepancies; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_open_discrepancies AS
 SELECT od.discrepancy_id,
    od.entity_id,
    od.attribute_id,
    od.case_id,
    od.workstream_id,
    od.observation_1_id,
    od.observation_2_id,
    od.discrepancy_type,
    od.severity,
    od.description,
    od.value_1_display,
    od.value_2_display,
    od.resolution_status,
    od.resolution_type,
    od.resolution_notes,
    od.resolved_at,
    od.resolved_by,
    od.accepted_observation_id,
    od.red_flag_id,
    od.detected_at,
    od.detected_by,
    od.created_at,
    od.updated_at,
    e.name AS entity_name,
    ar.display_name AS attribute_name,
    ar.category AS attribute_category
   FROM (("ob-poc".observation_discrepancies od
     JOIN "ob-poc".entities e ON ((od.entity_id = e.entity_id)))
     JOIN "ob-poc".attribute_registry ar ON ((od.attribute_id = ar.uuid)))
  WHERE ((od.resolution_status)::text = ANY ((ARRAY['OPEN'::character varying, 'INVESTIGATING'::character varying])::text[]))
  ORDER BY
        CASE od.severity
            WHEN 'CRITICAL'::text THEN 1
            WHEN 'HIGH'::text THEN 2
            WHEN 'MEDIUM'::text THEN 3
            WHEN 'LOW'::text THEN 4
            ELSE 5
        END, od.detected_at;


--
-- Name: v_request_execution_trace; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_request_execution_trace AS
 SELECT e.request_id,
    e.idempotency_key,
    e.execution_id,
    e.statement_index,
    e.verb,
    e.result_type,
    e.result_id,
    e.source,
    e.actor_id,
    e.actor_type,
    e.created_at,
    v.change_id AS view_state_change_id,
    v.selection_count AS view_selection_count
   FROM ("ob-poc".dsl_idempotency e
     LEFT JOIN "ob-poc".dsl_view_state_changes v ON ((e.idempotency_key = v.idempotency_key)))
  WHERE (e.request_id IS NOT NULL)
  ORDER BY e.request_id, e.created_at;


--
-- Name: VIEW v_request_execution_trace; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_request_execution_trace IS 'Trace all executions and view state changes for a single request';


--
-- Name: v_role_taxonomy; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_role_taxonomy AS
 SELECT r.role_id,
    r.name AS role_code,
    r.description,
    r.role_category,
    rc.category_name,
    rc.layout_behavior,
    r.layout_category,
    r.ubo_treatment,
    ut.treatment_name,
    ut.terminates_chain,
    ut.requires_lookthrough,
    r.display_priority,
    r.requires_percentage,
    r.natural_person_only,
    r.legal_entity_only,
    r.compatible_entity_categories,
    r.kyc_obligation,
    r.sort_order,
    r.is_active
   FROM (("ob-poc".roles r
     LEFT JOIN "ob-poc".role_categories rc ON (((r.role_category)::text = (rc.category_code)::text)))
     LEFT JOIN "ob-poc".ubo_treatments ut ON (((r.ubo_treatment)::text = (ut.treatment_code)::text)))
  WHERE (r.is_active = true)
  ORDER BY rc.sort_order, r.sort_order, r.display_priority DESC;


--
-- Name: VIEW v_role_taxonomy; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_role_taxonomy IS 'Complete role taxonomy reference with category and treatment details.';


--
-- Name: v_session_view_history; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_session_view_history AS
 SELECT c.session_id,
    c.change_id,
    c.verb_name,
    c.selection_count,
    c.stack_depth,
    (c.taxonomy_context ->> 'node_type'::text) AS node_type,
    (c.taxonomy_context ->> 'label'::text) AS label,
    c.refinements,
    c.created_at,
    s.status AS session_status,
    s.primary_domain
   FROM ("ob-poc".dsl_view_state_changes c
     LEFT JOIN "ob-poc".dsl_sessions s ON ((c.session_id = s.session_id)))
  ORDER BY c.session_id, c.created_at DESC;


--
-- Name: VIEW v_session_view_history; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_session_view_history IS 'View state change history per session - shows navigation path through data';


--
-- Name: v_ubo_candidates; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_ubo_candidates AS
 WITH RECURSIVE ownership_chain AS (
         SELECT v.cbu_id,
            r.to_entity_id AS target_entity_id,
            r.from_entity_id AS owner_entity_id,
            (COALESCE(v.observed_percentage, v.alleged_percentage, r.percentage))::numeric AS effective_percentage,
            1 AS depth,
            ARRAY[r.to_entity_id, r.from_entity_id] AS path
           FROM ("ob-poc".entity_relationships r
             JOIN "ob-poc".cbu_relationship_verification v ON ((v.relationship_id = r.relationship_id)))
          WHERE (((r.relationship_type)::text = 'ownership'::text) AND ((v.status)::text = ANY ((ARRAY['proven'::character varying, 'alleged'::character varying, 'pending'::character varying])::text[])) AND ((r.effective_to IS NULL) OR (r.effective_to > CURRENT_DATE)))
        UNION ALL
         SELECT oc_1.cbu_id,
            oc_1.target_entity_id,
            r.from_entity_id AS owner_entity_id,
            ((oc_1.effective_percentage * (COALESCE(v.observed_percentage, v.alleged_percentage, r.percentage))::numeric) / (100)::numeric) AS effective_percentage,
            (oc_1.depth + 1),
            (oc_1.path || r.from_entity_id)
           FROM ((((ownership_chain oc_1
             JOIN "ob-poc".entities e_1 ON ((e_1.entity_id = oc_1.owner_entity_id)))
             JOIN "ob-poc".entity_types et_1 ON ((et_1.entity_type_id = e_1.entity_type_id)))
             JOIN "ob-poc".entity_relationships r ON ((r.to_entity_id = oc_1.owner_entity_id)))
             JOIN "ob-poc".cbu_relationship_verification v ON (((v.relationship_id = r.relationship_id) AND (v.cbu_id = oc_1.cbu_id))))
          WHERE (((et_1.entity_category)::text = 'SHELL'::text) AND ((r.relationship_type)::text = 'ownership'::text) AND ((v.status)::text = ANY ((ARRAY['proven'::character varying, 'alleged'::character varying, 'pending'::character varying])::text[])) AND ((r.effective_to IS NULL) OR (r.effective_to > CURRENT_DATE)) AND (oc_1.depth < 10) AND (NOT (r.from_entity_id = ANY (oc_1.path))))
        )
 SELECT oc.cbu_id,
    oc.owner_entity_id AS entity_id,
    e.name AS entity_name,
    et.entity_category,
    et.name AS entity_type_name,
    sum(oc.effective_percentage) AS total_effective_percentage,
    ((et.entity_category)::text = 'PERSON'::text) AS is_natural_person,
    (((et.entity_category)::text = 'PERSON'::text) AND (sum(oc.effective_percentage) >= (25)::numeric)) AS is_ubo
   FROM ((ownership_chain oc
     JOIN "ob-poc".entities e ON ((e.entity_id = oc.owner_entity_id)))
     JOIN "ob-poc".entity_types et ON ((et.entity_type_id = e.entity_type_id)))
  GROUP BY oc.cbu_id, oc.owner_entity_id, e.name, et.entity_category, et.name;


--
-- Name: v_ubo_evidence_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_ubo_evidence_summary AS
 SELECT ur.ubo_id,
    ur.cbu_id,
    ur.subject_entity_id,
    ur.ubo_proper_person_id,
    e.name AS ubo_name,
    ur.verification_status,
    ur.proof_date,
    ur.proof_method,
    count(ue.ubo_evidence_id) AS total_evidence,
    count(ue.ubo_evidence_id) FILTER (WHERE ((ue.verification_status)::text = 'VERIFIED'::text)) AS verified_evidence,
    count(ue.ubo_evidence_id) FILTER (WHERE ((ue.verification_status)::text = 'PENDING'::text)) AS pending_evidence,
    array_agg(DISTINCT ue.evidence_role) FILTER (WHERE ((ue.verification_status)::text = 'VERIFIED'::text)) AS proven_roles,
    ( SELECT can_prove_ubo.can_prove
           FROM "ob-poc".can_prove_ubo(ur.ubo_id) can_prove_ubo(can_prove, has_identity_proof, has_ownership_proof, missing_evidence, verified_evidence_count, pending_evidence_count)
         LIMIT 1) AS can_be_proven
   FROM (("ob-poc".ubo_registry ur
     JOIN "ob-poc".entities e ON ((ur.ubo_proper_person_id = e.entity_id)))
     LEFT JOIN "ob-poc".ubo_evidence ue ON ((ur.ubo_id = ue.ubo_id)))
  WHERE ((ur.closed_at IS NULL) AND (ur.superseded_at IS NULL))
  GROUP BY ur.ubo_id, ur.cbu_id, ur.subject_entity_id, ur.ubo_proper_person_id, e.name, ur.verification_status, ur.proof_date, ur.proof_method;


--
-- Name: VIEW v_ubo_evidence_summary; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_ubo_evidence_summary IS 'Summary view of UBO records with evidence status';


--
-- Name: v_verb_discovery; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_verb_discovery AS
 SELECT v.verb_id,
    v.domain,
    v.verb_name,
    v.full_name,
    v.description,
    v.behavior,
    v.category,
    c.label AS category_label,
    v.intent_patterns,
    v.workflow_phases,
    v.graph_contexts,
    v.example_short,
    v.example_dsl,
    v.typical_next,
    v.produces_type,
    v.consumes,
    v.source,
    v.updated_at
   FROM ("ob-poc".dsl_verbs v
     LEFT JOIN "ob-poc".dsl_verb_categories c ON ((v.category = c.category_code)))
  ORDER BY c.display_order, v.domain, v.verb_name;


--
-- Name: v_verbs_needing_recompile; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_verbs_needing_recompile AS
 SELECT verb_id,
    full_name,
    domain,
    verb_name,
    yaml_hash,
    (compiled_hash IS NOT NULL) AS has_compiled,
    compiler_version,
    compiled_at,
    updated_at,
        CASE
            WHEN (compiled_hash IS NULL) THEN 'never_compiled'::text
            WHEN (compiled_at IS NULL) THEN 'missing_compiled_at'::text
            WHEN (compiled_at < updated_at) THEN 'source_changed'::text
            ELSE 'up_to_date'::text
        END AS recompile_reason
   FROM "ob-poc".dsl_verbs
  ORDER BY domain, verb_name;


--
-- Name: VIEW v_verbs_needing_recompile; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON VIEW "ob-poc".v_verbs_needing_recompile IS 'Shows verbs that may need recompilation with current compiler version';


--
-- Name: workflow_definitions; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".workflow_definitions (
    workflow_id character varying(100) NOT NULL,
    version integer NOT NULL,
    description text,
    definition_json jsonb NOT NULL,
    content_hash character varying(64) NOT NULL,
    loaded_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE workflow_definitions; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".workflow_definitions IS 'Cached workflow definitions loaded from YAML files on startup';


--
-- Name: v_workflow_summary; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".v_workflow_summary AS
 SELECT workflow_id,
    version,
    description,
    jsonb_object_keys((definition_json -> 'states'::text)) AS state_count,
    jsonb_array_length((definition_json -> 'transitions'::text)) AS transition_count,
    loaded_at
   FROM "ob-poc".workflow_definitions;


--
-- Name: verb_pattern_embeddings; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".verb_pattern_embeddings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    verb_name character varying(100) NOT NULL,
    pattern_phrase text NOT NULL,
    pattern_normalized text NOT NULL,
    phonetic_codes text[] DEFAULT '{}'::text[] NOT NULL,
    embedding public.vector(384) NOT NULL,
    category character varying(50) DEFAULT 'navigation'::character varying NOT NULL,
    is_agent_bound boolean DEFAULT false NOT NULL,
    priority integer DEFAULT 50 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE verb_pattern_embeddings; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".verb_pattern_embeddings IS 'Pre-computed embeddings for voice command patterns. Used for semantic similarity matching of voice transcripts to DSL verbs.';


--
-- Name: COLUMN verb_pattern_embeddings.phonetic_codes; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".verb_pattern_embeddings.phonetic_codes IS 'Double Metaphone codes for phonetic fallback matching. Handles "enhawnce" → "enhance".';


--
-- Name: COLUMN verb_pattern_embeddings.embedding; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".verb_pattern_embeddings.embedding IS 'all-MiniLM-L6-v2 embedding (384 dimensions). Captures semantic meaning of pattern phrase.';


--
-- Name: verification_challenges; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".verification_challenges (
    challenge_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    entity_id uuid,
    allegation_id uuid,
    observation_id uuid,
    challenge_type character varying(30) NOT NULL,
    challenge_reason text NOT NULL,
    severity character varying(20) NOT NULL,
    status character varying(20) DEFAULT 'OPEN'::character varying NOT NULL,
    response_text text,
    response_evidence_ids uuid[],
    raised_at timestamp with time zone DEFAULT now() NOT NULL,
    raised_by character varying(100),
    responded_at timestamp with time zone,
    resolved_at timestamp with time zone,
    resolved_by character varying(100),
    resolution_type character varying(30),
    resolution_notes text,
    CONSTRAINT verification_challenges_resolution_type_check CHECK (((resolution_type IS NULL) OR ((resolution_type)::text = ANY ((ARRAY['ACCEPTED'::character varying, 'REJECTED'::character varying, 'WAIVED'::character varying, 'ESCALATED'::character varying])::text[])))),
    CONSTRAINT verification_challenges_severity_check CHECK (((severity)::text = ANY ((ARRAY['INFO'::character varying, 'LOW'::character varying, 'MEDIUM'::character varying, 'HIGH'::character varying, 'CRITICAL'::character varying])::text[]))),
    CONSTRAINT verification_challenges_status_check CHECK (((status)::text = ANY ((ARRAY['OPEN'::character varying, 'RESPONDED'::character varying, 'RESOLVED'::character varying, 'ESCALATED'::character varying])::text[]))),
    CONSTRAINT verification_challenges_type_check CHECK (((challenge_type)::text = ANY ((ARRAY['INCONSISTENCY'::character varying, 'LOW_CONFIDENCE'::character varying, 'MISSING_CORROBORATION'::character varying, 'PATTERN_DETECTED'::character varying, 'EVASION_SIGNAL'::character varying, 'REGISTRY_MISMATCH'::character varying])::text[])))
);


--
-- Name: TABLE verification_challenges; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".verification_challenges IS 'Challenge/response workflow for adversarial verification - tracks formal challenges requiring client response';


--
-- Name: verification_escalations; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".verification_escalations (
    escalation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    cbu_id uuid NOT NULL,
    case_id uuid,
    challenge_id uuid,
    escalation_level character varying(30) NOT NULL,
    escalation_reason text NOT NULL,
    risk_indicators jsonb,
    status character varying(20) DEFAULT 'PENDING'::character varying NOT NULL,
    decision character varying(20),
    decision_notes text,
    escalated_at timestamp with time zone DEFAULT now() NOT NULL,
    escalated_by character varying(100),
    decided_at timestamp with time zone,
    decided_by character varying(100),
    CONSTRAINT verification_escalations_decision_check CHECK (((decision IS NULL) OR ((decision)::text = ANY ((ARRAY['APPROVE'::character varying, 'REJECT'::character varying, 'REQUIRE_MORE_INFO'::character varying, 'ESCALATE_FURTHER'::character varying])::text[])))),
    CONSTRAINT verification_escalations_level_check CHECK (((escalation_level)::text = ANY ((ARRAY['SENIOR_ANALYST'::character varying, 'COMPLIANCE_OFFICER'::character varying, 'MLRO'::character varying, 'COMMITTEE'::character varying])::text[]))),
    CONSTRAINT verification_escalations_status_check CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'UNDER_REVIEW'::character varying, 'DECIDED'::character varying])::text[])))
);


--
-- Name: TABLE verification_escalations; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".verification_escalations IS 'Risk-based escalation routing for verification challenges requiring higher authority review';


--
-- Name: view_modes; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".view_modes (
    view_mode_code character varying(30) NOT NULL,
    display_name character varying(100) NOT NULL,
    description text,
    root_identification_rule character varying(50) NOT NULL,
    primary_traversal_direction character varying(10) DEFAULT 'DOWN'::character varying,
    hierarchy_edge_types jsonb NOT NULL,
    overlay_edge_types jsonb DEFAULT '[]'::jsonb,
    default_algorithm character varying(30) DEFAULT 'HIERARCHICAL'::character varying,
    algorithm_params jsonb DEFAULT '{}'::jsonb,
    swim_lane_attribute character varying(50),
    swim_lane_direction character varying(10) DEFAULT 'VERTICAL'::character varying,
    temporal_axis character varying(30),
    temporal_axis_direction character varying(10) DEFAULT 'HORIZONTAL'::character varying,
    snap_to_grid boolean DEFAULT false,
    grid_size_x numeric(5,1) DEFAULT 20.0,
    grid_size_y numeric(5,1) DEFAULT 20.0,
    auto_cluster boolean DEFAULT false,
    cluster_attribute character varying(50),
    cluster_visual_style character varying(20) DEFAULT 'BACKGROUND'::character varying,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE view_modes; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".view_modes IS 'Per-view configuration including root identification, hierarchy edges, and layout algorithm.';


--
-- Name: COLUMN view_modes.root_identification_rule; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".view_modes.root_identification_rule IS 'How to identify root nodes: CBU (CBU node), TERMINUS_ENTITIES (natural persons), APEX_ENTITY (top of chain), UMBRELLA_FUNDS (umbrella funds).';


--
-- Name: COLUMN view_modes.hierarchy_edge_types; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".view_modes.hierarchy_edge_types IS 'Edge types that define the primary hierarchy for layout. Array of edge_type_codes.';


--
-- Name: COLUMN view_modes.overlay_edge_types; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".view_modes.overlay_edge_types IS 'Edge types that overlay on the hierarchy (control, trustee). Not used for tier computation.';


--
-- Name: workflow_audit_log; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".workflow_audit_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    from_state character varying(100),
    to_state character varying(100) NOT NULL,
    transition_type character varying(20) DEFAULT 'auto'::character varying NOT NULL,
    transitioned_at timestamp with time zone DEFAULT now() NOT NULL,
    transitioned_by character varying(255),
    reason text,
    blockers_at_transition jsonb,
    guard_results jsonb
);


--
-- Name: TABLE workflow_audit_log; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".workflow_audit_log IS 'Audit trail of all workflow state transitions';


--
-- Name: workflow_instances; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".workflow_instances (
    instance_id uuid DEFAULT gen_random_uuid() NOT NULL,
    workflow_id character varying(100) NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    subject_type character varying(50) NOT NULL,
    subject_id uuid NOT NULL,
    current_state character varying(100) NOT NULL,
    state_entered_at timestamp with time zone DEFAULT now() NOT NULL,
    history jsonb DEFAULT '[]'::jsonb NOT NULL,
    blockers jsonb DEFAULT '[]'::jsonb NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by character varying(255)
);


--
-- Name: TABLE workflow_instances; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".workflow_instances IS 'Running workflow instances for KYC/onboarding orchestration';


--
-- Name: COLUMN workflow_instances.workflow_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".workflow_instances.workflow_id IS 'Workflow definition ID (e.g., kyc_onboarding)';


--
-- Name: COLUMN workflow_instances.subject_type; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".workflow_instances.subject_type IS 'Type of entity this workflow is for (cbu, entity, case)';


--
-- Name: COLUMN workflow_instances.subject_id; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".workflow_instances.subject_id IS 'UUID of the subject entity';


--
-- Name: COLUMN workflow_instances.current_state; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".workflow_instances.current_state IS 'Current state in the workflow state machine';


--
-- Name: COLUMN workflow_instances.history; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".workflow_instances.history IS 'JSON array of StateTransition records';


--
-- Name: COLUMN workflow_instances.blockers; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON COLUMN "ob-poc".workflow_instances.blockers IS 'JSON array of current Blocker records';


--
-- Name: workstream_statuses; Type: TABLE; Schema: ob-poc; Owner: -
--

CREATE TABLE "ob-poc".workstream_statuses (
    code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    is_terminal boolean DEFAULT false,
    is_active boolean DEFAULT true,
    display_order integer DEFAULT 0
);


--
-- Name: entity_regulatory_registrations; Type: TABLE; Schema: ob_kyc; Owner: -
--

CREATE TABLE ob_kyc.entity_regulatory_registrations (
    registration_id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_id uuid NOT NULL,
    regulator_code character varying(50) NOT NULL,
    registration_number character varying(100),
    registration_type character varying(50) NOT NULL,
    activity_scope text,
    home_regulator_code character varying(50),
    passport_reference character varying(100),
    registration_verified boolean DEFAULT false,
    verification_date date,
    verification_method character varying(50),
    verification_reference character varying(500),
    verification_expires date,
    status character varying(50) DEFAULT 'ACTIVE'::character varying,
    effective_date date DEFAULT CURRENT_DATE,
    expiry_date date,
    created_at timestamp without time zone DEFAULT now(),
    updated_at timestamp without time zone DEFAULT now(),
    created_by uuid,
    updated_by uuid
);


--
-- Name: regulators; Type: TABLE; Schema: ob_ref; Owner: -
--

CREATE TABLE ob_ref.regulators (
    regulator_code character varying(50) NOT NULL,
    regulator_name character varying(255) NOT NULL,
    jurisdiction character varying(2) NOT NULL,
    regulatory_tier character varying(50) NOT NULL,
    regulator_type character varying(50) DEFAULT 'GOVERNMENT'::character varying,
    registry_url character varying(500),
    active boolean DEFAULT true,
    created_at timestamp without time zone DEFAULT now(),
    updated_at timestamp without time zone DEFAULT now()
);


--
-- Name: regulatory_tiers; Type: TABLE; Schema: ob_ref; Owner: -
--

CREATE TABLE ob_ref.regulatory_tiers (
    tier_code character varying(50) NOT NULL,
    description character varying(255) NOT NULL,
    allows_simplified_dd boolean DEFAULT false,
    requires_enhanced_screening boolean DEFAULT false
);


--
-- Name: v_entity_regulatory_summary; Type: VIEW; Schema: ob_kyc; Owner: -
--

CREATE VIEW ob_kyc.v_entity_regulatory_summary AS
 SELECT e.entity_id,
    e.name AS entity_name,
    count(r.registration_id) AS registration_count,
    count(r.registration_id) FILTER (WHERE (r.registration_verified AND ((r.status)::text = 'ACTIVE'::text))) AS verified_count,
    bool_or((r.registration_verified AND ((r.status)::text = 'ACTIVE'::text) AND rt.allows_simplified_dd)) AS allows_simplified_dd,
    array_agg(DISTINCT r.regulator_code) FILTER (WHERE ((r.status)::text = 'ACTIVE'::text)) AS active_regulators,
    array_agg(DISTINCT r.regulator_code) FILTER (WHERE (r.registration_verified AND ((r.status)::text = 'ACTIVE'::text))) AS verified_regulators,
    max(r.verification_date) AS last_verified,
    min(r.verification_expires) FILTER (WHERE (r.verification_expires > CURRENT_DATE)) AS next_expiry
   FROM ((("ob-poc".entities e
     LEFT JOIN ob_kyc.entity_regulatory_registrations r ON ((e.entity_id = r.entity_id)))
     LEFT JOIN ob_ref.regulators reg ON (((r.regulator_code)::text = (reg.regulator_code)::text)))
     LEFT JOIN ob_ref.regulatory_tiers rt ON (((reg.regulatory_tier)::text = (rt.tier_code)::text)))
  GROUP BY e.entity_id, e.name;


--
-- Name: registration_types; Type: TABLE; Schema: ob_ref; Owner: -
--

CREATE TABLE ob_ref.registration_types (
    registration_type character varying(50) NOT NULL,
    description character varying(255) NOT NULL,
    is_primary boolean DEFAULT false,
    allows_reliance boolean DEFAULT true
);


--
-- Name: request_types; Type: TABLE; Schema: ob_ref; Owner: -
--

CREATE TABLE ob_ref.request_types (
    request_type character varying(50) NOT NULL,
    request_subtype character varying(100) NOT NULL,
    description character varying(255),
    default_due_days integer DEFAULT 7,
    default_grace_days integer DEFAULT 3,
    max_reminders integer DEFAULT 3,
    blocks_by_default boolean DEFAULT true,
    fulfillment_sources character varying(50)[] DEFAULT ARRAY['CLIENT'::text, 'USER'::text],
    auto_fulfill_on_upload boolean DEFAULT true,
    escalation_enabled boolean DEFAULT true,
    escalation_after_days integer DEFAULT 10,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE request_types; Type: COMMENT; Schema: ob_ref; Owner: -
--

COMMENT ON TABLE ob_ref.request_types IS 'Configuration for different request types and subtypes';


--
-- Name: COLUMN request_types.fulfillment_sources; Type: COMMENT; Schema: ob_ref; Owner: -
--

COMMENT ON COLUMN ob_ref.request_types.fulfillment_sources IS 'Who can fulfill this request: CLIENT, USER, SYSTEM, EXTERNAL_PROVIDER';


--
-- Name: COLUMN request_types.auto_fulfill_on_upload; Type: COMMENT; Schema: ob_ref; Owner: -
--

COMMENT ON COLUMN ob_ref.request_types.auto_fulfill_on_upload IS 'Whether uploading a matching document auto-fulfills the request';


--
-- Name: COLUMN request_types.escalation_after_days; Type: COMMENT; Schema: ob_ref; Owner: -
--

COMMENT ON COLUMN ob_ref.request_types.escalation_after_days IS 'Days past due date before auto-escalation';


--
-- Name: role_types; Type: TABLE; Schema: ob_ref; Owner: -
--

CREATE TABLE ob_ref.role_types (
    role_type_id uuid DEFAULT gen_random_uuid() NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    category character varying(50),
    triggers_full_kyc boolean DEFAULT false,
    triggers_screening boolean DEFAULT false,
    triggers_id_verification boolean DEFAULT false,
    check_regulatory_status boolean DEFAULT false,
    if_regulated_obligation character varying(50),
    cascade_to_entity_ubos boolean DEFAULT false,
    threshold_based boolean DEFAULT false,
    active boolean DEFAULT true,
    created_at timestamp without time zone DEFAULT now(),
    updated_at timestamp without time zone DEFAULT now()
);


--
-- Name: attribute_sources; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: attribute_sources_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.attribute_sources_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: attribute_sources_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.attribute_sources_id_seq OWNED BY public.attribute_sources.id;


--
-- Name: business_attributes; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: business_attributes_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.business_attributes_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: business_attributes_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.business_attributes_id_seq OWNED BY public.business_attributes.id;


--
-- Name: credentials_vault; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: data_domains; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.data_domains (
    id integer NOT NULL,
    domain_name character varying(100) NOT NULL,
    "values" jsonb NOT NULL,
    description text,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


--
-- Name: data_domains_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.data_domains_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: data_domains_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.data_domains_id_seq OWNED BY public.data_domains.id;


--
-- Name: derived_attributes; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: derived_attributes_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.derived_attributes_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: derived_attributes_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.derived_attributes_id_seq OWNED BY public.derived_attributes.id;


--
-- Name: rule_categories; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.rule_categories (
    id integer NOT NULL,
    category_key character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    color character varying(7),
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


--
-- Name: rule_categories_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.rule_categories_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: rule_categories_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.rule_categories_id_seq OWNED BY public.rule_categories.id;


--
-- Name: rule_dependencies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.rule_dependencies (
    id integer NOT NULL,
    rule_id integer,
    attribute_id integer,
    dependency_type character varying(20) DEFAULT 'input'::character varying,
    CONSTRAINT rule_dependencies_dependency_type_check CHECK (((dependency_type)::text = ANY (ARRAY[('input'::character varying)::text, ('lookup'::character varying)::text, ('reference'::character varying)::text])))
);


--
-- Name: rule_dependencies_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.rule_dependencies_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: rule_dependencies_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.rule_dependencies_id_seq OWNED BY public.rule_dependencies.id;


--
-- Name: rule_executions; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: rule_versions; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: rule_versions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.rule_versions_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: rule_versions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.rule_versions_id_seq OWNED BY public.rule_versions.id;


--
-- Name: rules; Type: TABLE; Schema: public; Owner: -
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


--
-- Name: rules_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.rules_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: rules_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.rules_id_seq OWNED BY public.rules.id;


--
-- Name: access_attestations; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.access_attestations (
    attestation_id uuid DEFAULT gen_random_uuid() NOT NULL,
    campaign_id uuid NOT NULL,
    attester_user_id uuid NOT NULL,
    attester_name character varying(255) NOT NULL,
    attester_email character varying(255) NOT NULL,
    attester_role character varying(100),
    attestation_scope character varying(50) NOT NULL,
    team_id uuid,
    item_ids uuid[],
    items_count integer NOT NULL,
    attestation_text text NOT NULL,
    attestation_version character varying(20) DEFAULT 'v1'::character varying,
    attested_at timestamp with time zone DEFAULT now() NOT NULL,
    signature_hash text NOT NULL,
    signature_input text,
    ip_address inet,
    user_agent text,
    session_id uuid,
    CONSTRAINT chk_attestation_scope CHECK (((attestation_scope)::text = ANY ((ARRAY['FULL_CAMPAIGN'::character varying, 'MY_REVIEWS'::character varying, 'SPECIFIC_TEAM'::character varying, 'SPECIFIC_ITEMS'::character varying])::text[])))
);


--
-- Name: access_domains; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.access_domains (
    domain_code character varying(50) NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    visualizer_views text[] DEFAULT '{}'::text[] NOT NULL,
    is_active boolean DEFAULT true
);


--
-- Name: access_review_campaigns; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.access_review_campaigns (
    campaign_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    review_type character varying(50) NOT NULL,
    scope_type character varying(50) NOT NULL,
    scope_filter jsonb,
    review_period_start date DEFAULT CURRENT_DATE NOT NULL,
    review_period_end date DEFAULT CURRENT_DATE NOT NULL,
    deadline date NOT NULL,
    reminder_days integer[] DEFAULT ARRAY[7, 3, 1],
    status character varying(50) DEFAULT 'DRAFT'::character varying,
    total_items integer DEFAULT 0,
    reviewed_items integer DEFAULT 0,
    confirmed_items integer DEFAULT 0,
    revoked_items integer DEFAULT 0,
    extended_items integer DEFAULT 0,
    escalated_items integer DEFAULT 0,
    pending_items integer DEFAULT 0,
    created_at timestamp with time zone DEFAULT now(),
    created_by_user_id uuid,
    launched_at timestamp with time zone,
    completed_at timestamp with time zone,
    CONSTRAINT chk_campaign_status CHECK (((status)::text = ANY ((ARRAY['DRAFT'::character varying, 'POPULATING'::character varying, 'ACTIVE'::character varying, 'IN_REVIEW'::character varying, 'PAST_DEADLINE'::character varying, 'COMPLETED'::character varying, 'CANCELLED'::character varying])::text[]))),
    CONSTRAINT chk_review_type CHECK (((review_type)::text = ANY ((ARRAY['QUARTERLY'::character varying, 'ANNUAL'::character varying, 'TRIGGERED'::character varying, 'JOINER_MOVER_LEAVER'::character varying])::text[]))),
    CONSTRAINT chk_scope_type CHECK (((scope_type)::text = ANY ((ARRAY['ALL'::character varying, 'BY_TEAM_TYPE'::character varying, 'BY_DELEGATING_ENTITY'::character varying, 'SPECIFIC_TEAMS'::character varying, 'GOVERNANCE_ONLY'::character varying])::text[])))
);


--
-- Name: access_review_items; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.access_review_items (
    item_id uuid DEFAULT gen_random_uuid() NOT NULL,
    campaign_id uuid NOT NULL,
    membership_id uuid NOT NULL,
    user_id uuid NOT NULL,
    team_id uuid NOT NULL,
    role_key character varying(100) NOT NULL,
    user_name character varying(255),
    user_email character varying(255),
    user_employer character varying(255),
    team_name character varying(255),
    team_type character varying(50),
    delegating_entity_name character varying(255),
    access_domains character varying(50)[],
    legal_appointment_id uuid,
    legal_position character varying(100),
    legal_entity_name character varying(255),
    legal_effective_from date,
    legal_effective_to date,
    last_login_at timestamp with time zone,
    days_since_login integer,
    membership_created_at timestamp with time zone,
    membership_age_days integer,
    flag_no_legal_link boolean DEFAULT false,
    flag_legal_expired boolean DEFAULT false,
    flag_legal_expiring_soon boolean DEFAULT false,
    flag_dormant_account boolean DEFAULT false,
    flag_never_logged_in boolean DEFAULT false,
    flag_role_mismatch boolean DEFAULT false,
    flag_orphaned_membership boolean DEFAULT false,
    flags_json jsonb DEFAULT '{}'::jsonb,
    recommendation character varying(50),
    recommendation_reason text,
    risk_score integer DEFAULT 0,
    reviewer_user_id uuid,
    reviewer_email character varying(255),
    reviewer_name character varying(255),
    status character varying(50) DEFAULT 'PENDING'::character varying,
    reviewed_at timestamp with time zone,
    reviewer_notes text,
    extended_to date,
    extension_reason text,
    escalated_to_user_id uuid,
    escalation_reason text,
    escalated_at timestamp with time zone,
    auto_action_at timestamp with time zone,
    auto_action_reason text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT chk_item_status CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'CONFIRMED'::character varying, 'REVOKED'::character varying, 'EXTENDED'::character varying, 'ESCALATED'::character varying, 'AUTO_SUSPENDED'::character varying, 'SKIPPED'::character varying])::text[]))),
    CONSTRAINT chk_recommendation CHECK (((recommendation)::text = ANY ((ARRAY['CONFIRM'::character varying, 'REVOKE'::character varying, 'EXTEND'::character varying, 'REVIEW'::character varying, 'ESCALATE'::character varying])::text[])))
);


--
-- Name: access_review_log; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.access_review_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    campaign_id uuid,
    item_id uuid,
    action character varying(50) NOT NULL,
    action_detail jsonb,
    actor_user_id uuid,
    actor_email character varying(255),
    actor_type character varying(50),
    created_at timestamp with time zone DEFAULT now(),
    ip_address inet
);


--
-- Name: function_domains; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.function_domains (
    function_name character varying(100) NOT NULL,
    access_domains character varying(50)[] NOT NULL,
    description text
);


--
-- Name: membership_audit_log; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.membership_audit_log (
    log_id uuid DEFAULT gen_random_uuid() NOT NULL,
    team_id uuid NOT NULL,
    user_id uuid NOT NULL,
    action character varying(50) NOT NULL,
    reason text,
    performed_at timestamp with time zone DEFAULT now(),
    performed_by_user_id uuid
);


--
-- Name: membership_history; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.membership_history (
    history_id uuid DEFAULT gen_random_uuid() NOT NULL,
    membership_id uuid NOT NULL,
    team_id uuid NOT NULL,
    user_id uuid NOT NULL,
    action character varying(50) NOT NULL,
    old_role_key character varying(100),
    new_role_key character varying(100),
    reason text,
    changed_by_user_id uuid,
    changed_at timestamp with time zone DEFAULT now()
);


--
-- Name: memberships; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.memberships (
    membership_id uuid DEFAULT gen_random_uuid() NOT NULL,
    team_id uuid NOT NULL,
    user_id uuid NOT NULL,
    role_key character varying(100) NOT NULL,
    team_type character varying(50) GENERATED ALWAYS AS (split_part((role_key)::text, '.'::text, 1)) STORED,
    function_name character varying(50) GENERATED ALWAYS AS (split_part(split_part((role_key)::text, '.'::text, 2), ':'::text, 1)) STORED,
    role_level character varying(50) GENERATED ALWAYS AS (split_part((role_key)::text, ':'::text, 2)) STORED,
    effective_from date DEFAULT CURRENT_DATE NOT NULL,
    effective_to date,
    permission_overrides jsonb DEFAULT '{}'::jsonb,
    legal_appointment_id uuid,
    requires_legal_appointment boolean DEFAULT false,
    delegated_by_user_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: COLUMN memberships.legal_appointment_id; Type: COMMENT; Schema: teams; Owner: -
--

COMMENT ON COLUMN teams.memberships.legal_appointment_id IS 'Links portal access to legal appointment (DIRECTOR, CONDUCTING_OFFICER, etc.)';


--
-- Name: COLUMN memberships.requires_legal_appointment; Type: COMMENT; Schema: teams; Owner: -
--

COMMENT ON COLUMN teams.memberships.requires_legal_appointment IS 'If true, warns when no legal appointment linked';


--
-- Name: team_cbu_access; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.team_cbu_access (
    access_id uuid DEFAULT gen_random_uuid() NOT NULL,
    team_id uuid NOT NULL,
    cbu_id uuid NOT NULL,
    access_restrictions jsonb DEFAULT '{}'::jsonb,
    granted_at timestamp with time zone DEFAULT now(),
    granted_by_user_id uuid
);


--
-- Name: team_service_entitlements; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.team_service_entitlements (
    entitlement_id uuid DEFAULT gen_random_uuid() NOT NULL,
    team_id uuid NOT NULL,
    service_code character varying(100) NOT NULL,
    config jsonb DEFAULT '{}'::jsonb,
    granted_at timestamp with time zone DEFAULT now(),
    granted_by_user_id uuid,
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: teams; Type: TABLE; Schema: teams; Owner: -
--

CREATE TABLE teams.teams (
    team_id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    team_type character varying(50) NOT NULL,
    delegating_entity_id uuid NOT NULL,
    authority_type character varying(50) NOT NULL,
    authority_scope jsonb DEFAULT '{}'::jsonb,
    access_mode character varying(50) NOT NULL,
    explicit_cbus uuid[],
    scope_filter jsonb,
    service_entitlements jsonb DEFAULT '{}'::jsonb,
    is_active boolean DEFAULT true,
    archived_at timestamp with time zone,
    archive_reason text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    created_by_user_id uuid,
    CONSTRAINT chk_access_mode CHECK (((access_mode)::text = ANY ((ARRAY['explicit'::character varying, 'by-manco'::character varying, 'by-im'::character varying, 'by-filter'::character varying])::text[]))),
    CONSTRAINT chk_authority_type CHECK (((authority_type)::text = ANY ((ARRAY['operational'::character varying, 'oversight'::character varying, 'trading'::character varying, 'administrative'::character varying, 'governance'::character varying])::text[]))),
    CONSTRAINT chk_team_type CHECK (((team_type)::text = ANY ((ARRAY['fund-ops'::character varying, 'manco-oversight'::character varying, 'im-trading'::character varying, 'spv-admin'::character varying, 'client-service'::character varying, 'accounting'::character varying, 'reporting'::character varying, 'board'::character varying, 'investment-committee'::character varying, 'conducting-officers'::character varying, 'executive'::character varying])::text[])))
);


--
-- Name: v_campaign_dashboard; Type: VIEW; Schema: teams; Owner: -
--

CREATE VIEW teams.v_campaign_dashboard AS
 SELECT campaign_id,
    name,
    review_type,
    scope_type,
    scope_filter,
    review_period_start,
    review_period_end,
    deadline,
    reminder_days,
    status,
    total_items,
    reviewed_items,
    confirmed_items,
    revoked_items,
    extended_items,
    escalated_items,
    pending_items,
    created_at,
    created_by_user_id,
    launched_at,
    completed_at,
    round((((reviewed_items)::numeric / (NULLIF(total_items, 0))::numeric) * (100)::numeric), 1) AS progress_percent,
    (deadline - CURRENT_DATE) AS days_until_deadline,
        CASE
            WHEN ((status)::text = 'COMPLETED'::text) THEN 'COMPLETED'::text
            WHEN (CURRENT_DATE > deadline) THEN 'OVERDUE'::text
            WHEN ((deadline - CURRENT_DATE) <= 3) THEN 'URGENT'::text
            WHEN ((deadline - CURRENT_DATE) <= 7) THEN 'DUE_SOON'::text
            ELSE 'ON_TRACK'::text
        END AS urgency
   FROM teams.access_review_campaigns c;


--
-- Name: v_effective_memberships; Type: VIEW; Schema: teams; Owner: -
--

CREATE VIEW teams.v_effective_memberships AS
 SELECT m.membership_id,
    m.team_id,
    m.user_id,
    m.role_key,
    m.team_type,
    m.function_name,
    m.role_level,
    m.effective_from,
    m.effective_to,
    m.permission_overrides,
    m.legal_appointment_id,
    m.requires_legal_appointment,
    m.delegated_by_user_id,
    m.created_at,
    m.updated_at,
    t.name AS team_name,
    t.delegating_entity_id,
    e.name AS delegating_entity_name,
    c.name AS user_name,
    c.email AS user_email,
    fd.access_domains
   FROM ((((teams.memberships m
     JOIN teams.teams t ON ((m.team_id = t.team_id)))
     JOIN "ob-poc".entities e ON ((t.delegating_entity_id = e.entity_id)))
     JOIN client_portal.clients c ON ((m.user_id = c.client_id)))
     LEFT JOIN teams.function_domains fd ON (((m.function_name)::text = (fd.function_name)::text)))
  WHERE ((t.is_active = true) AND ((c.status)::text = 'ACTIVE'::text) AND (m.effective_from <= CURRENT_DATE) AND ((m.effective_to IS NULL) OR (m.effective_to >= CURRENT_DATE)));


--
-- Name: v_flagged_items_summary; Type: VIEW; Schema: teams; Owner: -
--

CREATE VIEW teams.v_flagged_items_summary AS
 SELECT campaign_id,
    count(*) FILTER (WHERE flag_legal_expired) AS legal_expired_count,
    count(*) FILTER (WHERE flag_legal_expiring_soon) AS legal_expiring_count,
    count(*) FILTER (WHERE flag_no_legal_link) AS no_legal_link_count,
    count(*) FILTER (WHERE flag_dormant_account) AS dormant_count,
    count(*) FILTER (WHERE flag_never_logged_in) AS never_logged_in_count,
    count(*) FILTER (WHERE flag_role_mismatch) AS role_mismatch_count,
    count(*) FILTER (WHERE (risk_score >= 70)) AS high_risk_count,
    count(*) FILTER (WHERE ((risk_score >= 40) AND (risk_score <= 69))) AS medium_risk_count,
    count(*) FILTER (WHERE (risk_score < 40)) AS low_risk_count
   FROM teams.access_review_items
  GROUP BY campaign_id;


--
-- Name: v_governance_access; Type: VIEW; Schema: teams; Owner: -
--

CREATE VIEW teams.v_governance_access AS
 SELECT m.membership_id,
    m.user_id,
    c.name AS user_name,
    c.email AS user_email,
    m.team_id,
    t.name AS team_name,
    t.team_type,
    m.role_key,
    m.function_name,
    m.role_level,
    fd.access_domains,
    m.legal_appointment_id,
        CASE
            WHEN (m.requires_legal_appointment AND (m.legal_appointment_id IS NULL)) THEN true
            ELSE false
        END AS missing_legal_appointment
   FROM (((teams.memberships m
     JOIN teams.teams t ON ((m.team_id = t.team_id)))
     JOIN client_portal.clients c ON ((m.user_id = c.client_id)))
     LEFT JOIN teams.function_domains fd ON (((m.function_name)::text = (fd.function_name)::text)))
  WHERE (((t.team_type)::text = ANY ((ARRAY['board'::character varying, 'investment-committee'::character varying, 'conducting-officers'::character varying, 'executive'::character varying])::text[])) AND (t.is_active = true) AND ((c.status)::text = 'ACTIVE'::text) AND (m.effective_from <= CURRENT_DATE) AND ((m.effective_to IS NULL) OR (m.effective_to >= CURRENT_DATE)));


--
-- Name: v_reviewer_workload; Type: VIEW; Schema: teams; Owner: -
--

CREATE VIEW teams.v_reviewer_workload AS
 SELECT campaign_id,
    reviewer_user_id,
    reviewer_email,
    reviewer_name,
    count(*) AS total_items,
    count(*) FILTER (WHERE ((status)::text = 'PENDING'::text)) AS pending_items,
    count(*) FILTER (WHERE ((status)::text = 'CONFIRMED'::text)) AS confirmed_items,
    count(*) FILTER (WHERE ((status)::text = 'REVOKED'::text)) AS revoked_items,
    count(*) FILTER (WHERE (flag_legal_expired OR flag_no_legal_link OR flag_dormant_account)) AS flagged_items,
    (EXISTS ( SELECT 1
           FROM teams.access_attestations a
          WHERE ((a.campaign_id = i.campaign_id) AND (a.attester_user_id = i.reviewer_user_id)))) AS has_attested
   FROM teams.access_review_items i
  GROUP BY campaign_id, reviewer_user_id, reviewer_email, reviewer_name;


--
-- Name: v_user_cbu_access; Type: VIEW; Schema: teams; Owner: -
--

CREATE VIEW teams.v_user_cbu_access AS
 WITH user_teams AS (
         SELECT m.user_id,
            t.team_id,
            t.name AS team_name,
            m.role_key,
            fd.access_domains,
            t.access_mode,
            t.explicit_cbus,
            t.delegating_entity_id,
            t.scope_filter,
            t.authority_scope
           FROM ((teams.v_effective_memberships m
             JOIN teams.teams t ON ((m.team_id = t.team_id)))
             LEFT JOIN teams.function_domains fd ON (((m.function_name)::text = (fd.function_name)::text)))
        ), resolved_cbus AS (
         SELECT ut.user_id,
            ut.team_id,
            ut.team_name,
            ut.role_key,
            ut.access_domains,
            unnest(ut.explicit_cbus) AS cbu_id
           FROM user_teams ut
          WHERE (((ut.access_mode)::text = 'explicit'::text) AND (ut.explicit_cbus IS NOT NULL))
        UNION ALL
         SELECT ut.user_id,
            ut.team_id,
            ut.team_name,
            ut.role_key,
            ut.access_domains,
            cer.cbu_id
           FROM ((user_teams ut
             JOIN "ob-poc".cbu_entity_roles cer ON ((cer.entity_id = ut.delegating_entity_id)))
             JOIN "ob-poc".roles r ON (((cer.role_id = r.role_id) AND ((r.name)::text = 'MANAGEMENT_COMPANY'::text))))
          WHERE ((ut.access_mode)::text = 'by-manco'::text)
        UNION ALL
         SELECT ut.user_id,
            ut.team_id,
            ut.team_name,
            ut.role_key,
            ut.access_domains,
            a.cbu_id
           FROM (user_teams ut
             JOIN custody.cbu_im_assignments a ON ((a.manager_entity_id = ut.delegating_entity_id)))
          WHERE (((ut.access_mode)::text = 'by-im'::text) AND ((a.status)::text = 'ACTIVE'::text))
        )
 SELECT rc.user_id,
    rc.cbu_id,
    c.name AS cbu_name,
    array_agg(DISTINCT rc.team_id) AS via_teams,
    array_agg(DISTINCT rc.role_key) AS roles,
    array_agg(DISTINCT unnest_domain.unnest_domain) AS access_domains
   FROM ((resolved_cbus rc
     JOIN "ob-poc".cbus c ON ((rc.cbu_id = c.cbu_id)))
     CROSS JOIN LATERAL unnest(COALESCE(rc.access_domains, ARRAY[]::character varying[])) unnest_domain(unnest_domain))
  GROUP BY rc.user_id, rc.cbu_id, c.name;


--
-- Name: attribute_values_typed id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed ALTER COLUMN id SET DEFAULT nextval('"ob-poc".attribute_values_typed_id_seq'::regclass);


--
-- Name: dsl_instances id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instances ALTER COLUMN id SET DEFAULT nextval('"ob-poc".dsl_instances_id_seq'::regclass);


--
-- Name: intent_feedback id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".intent_feedback ALTER COLUMN id SET DEFAULT nextval('"ob-poc".intent_feedback_id_seq'::regclass);


--
-- Name: intent_feedback_analysis id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".intent_feedback_analysis ALTER COLUMN id SET DEFAULT nextval('"ob-poc".intent_feedback_analysis_id_seq'::regclass);


--
-- Name: attribute_sources id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attribute_sources ALTER COLUMN id SET DEFAULT nextval('public.attribute_sources_id_seq'::regclass);


--
-- Name: business_attributes id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.business_attributes ALTER COLUMN id SET DEFAULT nextval('public.business_attributes_id_seq'::regclass);


--
-- Name: data_domains id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_domains ALTER COLUMN id SET DEFAULT nextval('public.data_domains_id_seq'::regclass);


--
-- Name: derived_attributes id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.derived_attributes ALTER COLUMN id SET DEFAULT nextval('public.derived_attributes_id_seq'::regclass);


--
-- Name: rule_categories id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_categories ALTER COLUMN id SET DEFAULT nextval('public.rule_categories_id_seq'::regclass);


--
-- Name: rule_dependencies id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_dependencies ALTER COLUMN id SET DEFAULT nextval('public.rule_dependencies_id_seq'::regclass);


--
-- Name: rule_versions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_versions ALTER COLUMN id SET DEFAULT nextval('public.rule_versions_id_seq'::regclass);


--
-- Name: rules id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rules ALTER COLUMN id SET DEFAULT nextval('public.rules_id_seq'::regclass);


--
-- Name: clients clients_email_key; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.clients
    ADD CONSTRAINT clients_email_key UNIQUE (email);


--
-- Name: clients clients_pkey; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.clients
    ADD CONSTRAINT clients_pkey PRIMARY KEY (client_id);


--
-- Name: commitments commitments_pkey; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.commitments
    ADD CONSTRAINT commitments_pkey PRIMARY KEY (commitment_id);


--
-- Name: credentials credentials_pkey; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.credentials
    ADD CONSTRAINT credentials_pkey PRIMARY KEY (credential_id);


--
-- Name: escalations escalations_pkey; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.escalations
    ADD CONSTRAINT escalations_pkey PRIMARY KEY (escalation_id);


--
-- Name: sessions sessions_pkey; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.sessions
    ADD CONSTRAINT sessions_pkey PRIMARY KEY (session_id);


--
-- Name: submissions submissions_pkey; Type: CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.submissions
    ADD CONSTRAINT submissions_pkey PRIMARY KEY (submission_id);


--
-- Name: cbu_cash_sweep_config cbu_cash_sweep_config_cbu_id_currency_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cash_sweep_config
    ADD CONSTRAINT cbu_cash_sweep_config_cbu_id_currency_key UNIQUE (cbu_id, currency);


--
-- Name: cbu_cash_sweep_config cbu_cash_sweep_config_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cash_sweep_config
    ADD CONSTRAINT cbu_cash_sweep_config_pkey PRIMARY KEY (sweep_id);


--
-- Name: cbu_cross_border_config cbu_cross_border_config_cbu_id_source_market_id_target_mark_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cross_border_config
    ADD CONSTRAINT cbu_cross_border_config_cbu_id_source_market_id_target_mark_key UNIQUE (cbu_id, source_market_id, target_market_id);


--
-- Name: cbu_cross_border_config cbu_cross_border_config_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cross_border_config
    ADD CONSTRAINT cbu_cross_border_config_pkey PRIMARY KEY (config_id);


--
-- Name: cbu_im_assignments cbu_im_assignments_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_im_assignments
    ADD CONSTRAINT cbu_im_assignments_pkey PRIMARY KEY (assignment_id);


--
-- Name: cbu_instrument_universe cbu_instrument_universe_natural_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_natural_key UNIQUE (cbu_id, instrument_class_id, market_id, counterparty_key);


--
-- Name: cbu_instrument_universe cbu_instrument_universe_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_pkey PRIMARY KEY (universe_id);


--
-- Name: cbu_pricing_config cbu_pricing_config_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_pricing_config
    ADD CONSTRAINT cbu_pricing_config_pkey PRIMARY KEY (config_id);


--
-- Name: cbu_settlement_chains cbu_settlement_chains_cbu_id_chain_name_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_chains
    ADD CONSTRAINT cbu_settlement_chains_cbu_id_chain_name_key UNIQUE (cbu_id, chain_name);


--
-- Name: cbu_settlement_chains cbu_settlement_chains_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_chains
    ADD CONSTRAINT cbu_settlement_chains_pkey PRIMARY KEY (chain_id);


--
-- Name: cbu_settlement_location_preferences cbu_settlement_location_prefe_cbu_id_market_id_instrument_c_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_location_preferences
    ADD CONSTRAINT cbu_settlement_location_prefe_cbu_id_market_id_instrument_c_key UNIQUE (cbu_id, market_id, instrument_class_id, preferred_location_id);


--
-- Name: cbu_settlement_location_preferences cbu_settlement_location_preferences_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_location_preferences
    ADD CONSTRAINT cbu_settlement_location_preferences_pkey PRIMARY KEY (preference_id);


--
-- Name: cbu_ssi_agent_override cbu_ssi_agent_override_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi_agent_override
    ADD CONSTRAINT cbu_ssi_agent_override_pkey PRIMARY KEY (override_id);


--
-- Name: cbu_ssi_agent_override cbu_ssi_agent_override_ssi_id_agent_role_sequence_order_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi_agent_override
    ADD CONSTRAINT cbu_ssi_agent_override_ssi_id_agent_role_sequence_order_key UNIQUE (ssi_id, agent_role, sequence_order);


--
-- Name: cbu_ssi cbu_ssi_cbu_id_ssi_name_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi
    ADD CONSTRAINT cbu_ssi_cbu_id_ssi_name_key UNIQUE (cbu_id, ssi_name);


--
-- Name: cbu_ssi cbu_ssi_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi
    ADD CONSTRAINT cbu_ssi_pkey PRIMARY KEY (ssi_id);


--
-- Name: cbu_tax_reclaim_config cbu_tax_reclaim_config_cbu_id_source_jurisdiction_id_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reclaim_config
    ADD CONSTRAINT cbu_tax_reclaim_config_cbu_id_source_jurisdiction_id_key UNIQUE (cbu_id, source_jurisdiction_id);


--
-- Name: cbu_tax_reclaim_config cbu_tax_reclaim_config_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reclaim_config
    ADD CONSTRAINT cbu_tax_reclaim_config_pkey PRIMARY KEY (config_id);


--
-- Name: cbu_tax_reporting cbu_tax_reporting_cbu_id_reporting_regime_reporting_jurisdi_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reporting
    ADD CONSTRAINT cbu_tax_reporting_cbu_id_reporting_regime_reporting_jurisdi_key UNIQUE (cbu_id, reporting_regime, reporting_jurisdiction_id);


--
-- Name: cbu_tax_reporting cbu_tax_reporting_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reporting
    ADD CONSTRAINT cbu_tax_reporting_pkey PRIMARY KEY (reporting_id);


--
-- Name: cbu_tax_status cbu_tax_status_cbu_id_tax_jurisdiction_id_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_status
    ADD CONSTRAINT cbu_tax_status_cbu_id_tax_jurisdiction_id_key UNIQUE (cbu_id, tax_jurisdiction_id);


--
-- Name: cbu_tax_status cbu_tax_status_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_status
    ADD CONSTRAINT cbu_tax_status_pkey PRIMARY KEY (status_id);


--
-- Name: cfi_codes cfi_codes_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cfi_codes
    ADD CONSTRAINT cfi_codes_pkey PRIMARY KEY (cfi_code);


--
-- Name: csa_agreements csa_agreements_isda_id_csa_type_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.csa_agreements
    ADD CONSTRAINT csa_agreements_isda_id_csa_type_key UNIQUE (isda_id, csa_type);


--
-- Name: csa_agreements csa_agreements_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.csa_agreements
    ADD CONSTRAINT csa_agreements_pkey PRIMARY KEY (csa_id);


--
-- Name: entity_settlement_identity entity_settlement_identity_entity_id_primary_bic_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_settlement_identity
    ADD CONSTRAINT entity_settlement_identity_entity_id_primary_bic_key UNIQUE (entity_id, primary_bic);


--
-- Name: entity_settlement_identity entity_settlement_identity_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_settlement_identity
    ADD CONSTRAINT entity_settlement_identity_pkey PRIMARY KEY (identity_id);


--
-- Name: entity_ssi entity_ssi_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_ssi
    ADD CONSTRAINT entity_ssi_pkey PRIMARY KEY (entity_ssi_id);


--
-- Name: instruction_paths instruction_paths_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_paths
    ADD CONSTRAINT instruction_paths_pkey PRIMARY KEY (path_id);


--
-- Name: instruction_types instruction_types_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_types
    ADD CONSTRAINT instruction_types_pkey PRIMARY KEY (type_id);


--
-- Name: instruction_types instruction_types_type_code_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_types
    ADD CONSTRAINT instruction_types_type_code_key UNIQUE (type_code);


--
-- Name: instrument_classes instrument_classes_code_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instrument_classes
    ADD CONSTRAINT instrument_classes_code_key UNIQUE (code);


--
-- Name: instrument_classes instrument_classes_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instrument_classes
    ADD CONSTRAINT instrument_classes_pkey PRIMARY KEY (class_id);


--
-- Name: isda_agreements isda_agreements_cbu_id_counterparty_entity_id_agreement_dat_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_agreements
    ADD CONSTRAINT isda_agreements_cbu_id_counterparty_entity_id_agreement_dat_key UNIQUE (cbu_id, counterparty_entity_id, agreement_date);


--
-- Name: isda_agreements isda_agreements_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_agreements
    ADD CONSTRAINT isda_agreements_pkey PRIMARY KEY (isda_id);


--
-- Name: isda_product_coverage isda_product_coverage_isda_id_instrument_class_id_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_coverage
    ADD CONSTRAINT isda_product_coverage_isda_id_instrument_class_id_key UNIQUE (isda_id, instrument_class_id);


--
-- Name: isda_product_coverage isda_product_coverage_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_coverage
    ADD CONSTRAINT isda_product_coverage_pkey PRIMARY KEY (coverage_id);


--
-- Name: isda_product_taxonomy isda_product_taxonomy_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_taxonomy
    ADD CONSTRAINT isda_product_taxonomy_pkey PRIMARY KEY (taxonomy_id);


--
-- Name: isda_product_taxonomy isda_product_taxonomy_taxonomy_code_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_taxonomy
    ADD CONSTRAINT isda_product_taxonomy_taxonomy_code_key UNIQUE (taxonomy_code);


--
-- Name: markets markets_mic_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.markets
    ADD CONSTRAINT markets_mic_key UNIQUE (mic);


--
-- Name: markets markets_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.markets
    ADD CONSTRAINT markets_pkey PRIMARY KEY (market_id);


--
-- Name: security_types security_types_code_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.security_types
    ADD CONSTRAINT security_types_code_key UNIQUE (code);


--
-- Name: security_types security_types_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.security_types
    ADD CONSTRAINT security_types_pkey PRIMARY KEY (security_type_id);


--
-- Name: settlement_chain_hops settlement_chain_hops_chain_id_hop_sequence_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_chain_hops
    ADD CONSTRAINT settlement_chain_hops_chain_id_hop_sequence_key UNIQUE (chain_id, hop_sequence);


--
-- Name: settlement_chain_hops settlement_chain_hops_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_chain_hops
    ADD CONSTRAINT settlement_chain_hops_pkey PRIMARY KEY (hop_id);


--
-- Name: settlement_locations settlement_locations_location_code_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_locations
    ADD CONSTRAINT settlement_locations_location_code_key UNIQUE (location_code);


--
-- Name: settlement_locations settlement_locations_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_locations
    ADD CONSTRAINT settlement_locations_pkey PRIMARY KEY (location_id);


--
-- Name: ssi_booking_rules ssi_booking_rules_cbu_id_priority_rule_name_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_cbu_id_priority_rule_name_key UNIQUE (cbu_id, priority, rule_name);


--
-- Name: ssi_booking_rules ssi_booking_rules_cbu_rule_name_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_cbu_rule_name_key UNIQUE (cbu_id, rule_name);


--
-- Name: ssi_booking_rules ssi_booking_rules_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: subcustodian_network subcustodian_network_market_id_currency_subcustodian_bic_ef_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.subcustodian_network
    ADD CONSTRAINT subcustodian_network_market_id_currency_subcustodian_bic_ef_key UNIQUE (market_id, currency, subcustodian_bic, effective_date);


--
-- Name: subcustodian_network subcustodian_network_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.subcustodian_network
    ADD CONSTRAINT subcustodian_network_pkey PRIMARY KEY (network_id);


--
-- Name: tax_jurisdictions tax_jurisdictions_jurisdiction_code_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_jurisdictions
    ADD CONSTRAINT tax_jurisdictions_jurisdiction_code_key UNIQUE (jurisdiction_code);


--
-- Name: tax_jurisdictions tax_jurisdictions_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_jurisdictions
    ADD CONSTRAINT tax_jurisdictions_pkey PRIMARY KEY (jurisdiction_id);


--
-- Name: tax_treaty_rates tax_treaty_rates_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_treaty_rates
    ADD CONSTRAINT tax_treaty_rates_pkey PRIMARY KEY (treaty_id);


--
-- Name: tax_treaty_rates tax_treaty_rates_source_jurisdiction_id_investor_jurisdicti_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_treaty_rates
    ADD CONSTRAINT tax_treaty_rates_source_jurisdiction_id_investor_jurisdicti_key UNIQUE (source_jurisdiction_id, investor_jurisdiction_id, income_type, instrument_class_id);


--
-- Name: approval_requests approval_requests_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.approval_requests
    ADD CONSTRAINT approval_requests_pkey PRIMARY KEY (approval_id);


--
-- Name: case_events case_events_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.case_events
    ADD CONSTRAINT case_events_pkey PRIMARY KEY (event_id);


--
-- Name: cases cases_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.cases
    ADD CONSTRAINT cases_pkey PRIMARY KEY (case_id);


--
-- Name: doc_request_acceptable_types doc_request_acceptable_types_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_request_acceptable_types
    ADD CONSTRAINT doc_request_acceptable_types_pkey PRIMARY KEY (link_id);


--
-- Name: doc_request_acceptable_types doc_request_acceptable_types_request_id_document_type_id_key; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_request_acceptable_types
    ADD CONSTRAINT doc_request_acceptable_types_request_id_document_type_id_key UNIQUE (request_id, document_type_id);


--
-- Name: doc_requests doc_requests_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_requests
    ADD CONSTRAINT doc_requests_pkey PRIMARY KEY (request_id);


--
-- Name: doc_requests doc_requests_workstream_doc_type_uniq; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_requests
    ADD CONSTRAINT doc_requests_workstream_doc_type_uniq UNIQUE (workstream_id, doc_type);


--
-- Name: entity_workstreams entity_workstreams_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.entity_workstreams
    ADD CONSTRAINT entity_workstreams_pkey PRIMARY KEY (workstream_id);


--
-- Name: holdings holdings_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.holdings
    ADD CONSTRAINT holdings_pkey PRIMARY KEY (id);


--
-- Name: holdings holdings_share_class_id_investor_entity_id_key; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.holdings
    ADD CONSTRAINT holdings_share_class_id_investor_entity_id_key UNIQUE (share_class_id, investor_entity_id);


--
-- Name: movements movements_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.movements
    ADD CONSTRAINT movements_pkey PRIMARY KEY (id);


--
-- Name: outstanding_requests outstanding_requests_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.outstanding_requests
    ADD CONSTRAINT outstanding_requests_pkey PRIMARY KEY (request_id);


--
-- Name: red_flags red_flags_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.red_flags
    ADD CONSTRAINT red_flags_pkey PRIMARY KEY (red_flag_id);


--
-- Name: rule_executions rule_executions_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.rule_executions
    ADD CONSTRAINT rule_executions_pkey PRIMARY KEY (execution_id);


--
-- Name: screenings screenings_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.screenings
    ADD CONSTRAINT screenings_pkey PRIMARY KEY (screening_id);


--
-- Name: screenings screenings_workstream_type_uniq; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.screenings
    ADD CONSTRAINT screenings_workstream_type_uniq UNIQUE (workstream_id, screening_type);


--
-- Name: share_classes share_classes_cbu_id_isin_key; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.share_classes
    ADD CONSTRAINT share_classes_cbu_id_isin_key UNIQUE (cbu_id, isin);


--
-- Name: share_classes share_classes_pkey; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.share_classes
    ADD CONSTRAINT share_classes_pkey PRIMARY KEY (id);


--
-- Name: entity_workstreams uq_case_entity; Type: CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.entity_workstreams
    ADD CONSTRAINT uq_case_entity UNIQUE (case_id, entity_id);


--
-- Name: attribute_dictionary attribute_dictionary_attr_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_dictionary
    ADD CONSTRAINT attribute_dictionary_attr_id_key UNIQUE (attr_id);


--
-- Name: attribute_dictionary attribute_dictionary_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_dictionary
    ADD CONSTRAINT attribute_dictionary_pkey PRIMARY KEY (attribute_id);


--
-- Name: attribute_observations attribute_observations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_pkey PRIMARY KEY (observation_id);


--
-- Name: attribute_registry attribute_registry_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_registry
    ADD CONSTRAINT attribute_registry_pkey PRIMARY KEY (id);


--
-- Name: attribute_values_typed attribute_values_typed_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT attribute_values_typed_pkey PRIMARY KEY (id);


--
-- Name: bods_entity_statements bods_entity_statements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".bods_entity_statements
    ADD CONSTRAINT bods_entity_statements_pkey PRIMARY KEY (statement_id);


--
-- Name: bods_ownership_statements bods_ownership_statements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".bods_ownership_statements
    ADD CONSTRAINT bods_ownership_statements_pkey PRIMARY KEY (statement_id);


--
-- Name: bods_person_statements bods_person_statements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".bods_person_statements
    ADD CONSTRAINT bods_person_statements_pkey PRIMARY KEY (statement_id);


--
-- Name: case_decision_thresholds case_decision_thresholds_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".case_decision_thresholds
    ADD CONSTRAINT case_decision_thresholds_pkey PRIMARY KEY (threshold_id);


--
-- Name: case_decision_thresholds case_decision_thresholds_threshold_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".case_decision_thresholds
    ADD CONSTRAINT case_decision_thresholds_threshold_name_key UNIQUE (threshold_name);


--
-- Name: case_evaluation_snapshots case_evaluation_snapshots_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".case_evaluation_snapshots
    ADD CONSTRAINT case_evaluation_snapshots_pkey PRIMARY KEY (snapshot_id);


--
-- Name: case_types case_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".case_types
    ADD CONSTRAINT case_types_pkey PRIMARY KEY (code);


--
-- Name: cbu_change_log cbu_change_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_change_log
    ADD CONSTRAINT cbu_change_log_pkey PRIMARY KEY (log_id);


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
-- Name: cbu_entity_roles_history cbu_entity_roles_history_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles_history
    ADD CONSTRAINT cbu_entity_roles_history_pkey PRIMARY KEY (history_id);


--
-- Name: cbu_entity_roles cbu_entity_roles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_pkey PRIMARY KEY (cbu_entity_role_id);


--
-- Name: cbu_evidence cbu_evidence_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_evidence
    ADD CONSTRAINT cbu_evidence_pkey PRIMARY KEY (evidence_id);


--
-- Name: cbu_layout_overrides cbu_layout_overrides_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_layout_overrides
    ADD CONSTRAINT cbu_layout_overrides_pkey PRIMARY KEY (cbu_id, user_id, view_mode);


--
-- Name: cbu_lifecycle_instances cbu_lifecycle_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_lifecycle_instances
    ADD CONSTRAINT cbu_lifecycle_instances_pkey PRIMARY KEY (instance_id);


--
-- Name: cbu_lifecycle_instances cbu_lifecycle_instances_url_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_lifecycle_instances
    ADD CONSTRAINT cbu_lifecycle_instances_url_key UNIQUE (instance_url);


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_cbu_id_subscription_id_instrumen_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_cbu_id_subscription_id_instrumen_key UNIQUE NULLS NOT DISTINCT (cbu_id, subscription_id, instrument_class_id, market_id, currency, counterparty_entity_id);


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_pkey PRIMARY KEY (overlay_id);


--
-- Name: cbu_product_subscriptions cbu_product_subscriptions_cbu_id_product_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_product_subscriptions
    ADD CONSTRAINT cbu_product_subscriptions_cbu_id_product_id_key UNIQUE (cbu_id, product_id);


--
-- Name: cbu_product_subscriptions cbu_product_subscriptions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_product_subscriptions
    ADD CONSTRAINT cbu_product_subscriptions_pkey PRIMARY KEY (subscription_id);


--
-- Name: cbu_relationship_verification cbu_relationship_verification_cbu_id_relationship_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_relationship_verification
    ADD CONSTRAINT cbu_relationship_verification_cbu_id_relationship_id_key UNIQUE (cbu_id, relationship_id);


--
-- Name: cbu_relationship_verification cbu_relationship_verification_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_relationship_verification
    ADD CONSTRAINT cbu_relationship_verification_pkey PRIMARY KEY (verification_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_cbu_id_resource_type_id_instance_ide_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_cbu_id_resource_type_id_instance_ide_key UNIQUE (cbu_id, resource_type_id, instance_identifier);


--
-- Name: cbu_resource_instances cbu_resource_instances_cbu_product_service_resource_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_cbu_product_service_resource_key UNIQUE (cbu_id, product_id, service_id, resource_type_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_instance_url_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_instance_url_key UNIQUE (instance_url);


--
-- Name: cbu_resource_instances cbu_resource_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_pkey PRIMARY KEY (instance_id);


--
-- Name: cbu_service_contexts cbu_service_contexts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_service_contexts
    ADD CONSTRAINT cbu_service_contexts_pkey PRIMARY KEY (cbu_id, service_context);


--
-- Name: cbu_sla_commitments cbu_sla_commitments_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_pkey PRIMARY KEY (commitment_id);


--
-- Name: cbu_trading_profiles cbu_trading_profiles_cbu_id_version_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_trading_profiles
    ADD CONSTRAINT cbu_trading_profiles_cbu_id_version_key UNIQUE (cbu_id, version);


--
-- Name: cbu_trading_profiles cbu_trading_profiles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_trading_profiles
    ADD CONSTRAINT cbu_trading_profiles_pkey PRIMARY KEY (profile_id);


--
-- Name: cbus cbus_name_jurisdiction_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_name_jurisdiction_key UNIQUE (name, jurisdiction);


--
-- Name: cbus cbus_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_pkey PRIMARY KEY (cbu_id);


--
-- Name: client_allegations client_allegations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_pkey PRIMARY KEY (allegation_id);


--
-- Name: client_types client_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_types
    ADD CONSTRAINT client_types_pkey PRIMARY KEY (code);


--
-- Name: crud_operations crud_operations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".crud_operations
    ADD CONSTRAINT crud_operations_pkey PRIMARY KEY (operation_id);


--
-- Name: csg_validation_rules csg_validation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".csg_validation_rules
    ADD CONSTRAINT csg_validation_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: csg_validation_rules csg_validation_rules_rule_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".csg_validation_rules
    ADD CONSTRAINT csg_validation_rules_rule_code_key UNIQUE (rule_code);


--
-- Name: currencies currencies_iso_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".currencies
    ADD CONSTRAINT currencies_iso_code_key UNIQUE (iso_code);


--
-- Name: currencies currencies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".currencies
    ADD CONSTRAINT currencies_pkey PRIMARY KEY (currency_id);


--
-- Name: delegation_relationships delegation_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".delegation_relationships
    ADD CONSTRAINT delegation_relationships_pkey PRIMARY KEY (delegation_id);


--
-- Name: detected_patterns detected_patterns_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".detected_patterns
    ADD CONSTRAINT detected_patterns_pkey PRIMARY KEY (pattern_id);


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
-- Name: document_attribute_links document_attribute_links_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_links
    ADD CONSTRAINT document_attribute_links_pkey PRIMARY KEY (link_id);


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
-- Name: document_validity_rules document_validity_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_validity_rules
    ADD CONSTRAINT document_validity_rules_pkey PRIMARY KEY (rule_id);


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
-- Name: dsl_generation_log dsl_generation_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_generation_log
    ADD CONSTRAINT dsl_generation_log_pkey PRIMARY KEY (log_id);


--
-- Name: dsl_graph_contexts dsl_graph_contexts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_graph_contexts
    ADD CONSTRAINT dsl_graph_contexts_pkey PRIMARY KEY (context_code);


--
-- Name: dsl_idempotency dsl_idempotency_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_idempotency
    ADD CONSTRAINT dsl_idempotency_pkey PRIMARY KEY (idempotency_key);


--
-- Name: dsl_instance_versions dsl_instance_versions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT dsl_instance_versions_pkey PRIMARY KEY (version_id);


--
-- Name: dsl_instances dsl_instances_instance_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instances
    ADD CONSTRAINT dsl_instances_instance_id_key UNIQUE (instance_id);


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
-- Name: dsl_session_events dsl_session_events_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_session_events
    ADD CONSTRAINT dsl_session_events_pkey PRIMARY KEY (event_id);


--
-- Name: dsl_session_locks dsl_session_locks_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_session_locks
    ADD CONSTRAINT dsl_session_locks_pkey PRIMARY KEY (session_id);


--
-- Name: dsl_sessions dsl_sessions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_sessions
    ADD CONSTRAINT dsl_sessions_pkey PRIMARY KEY (session_id);


--
-- Name: dsl_snapshots dsl_snapshots_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_snapshots
    ADD CONSTRAINT dsl_snapshots_pkey PRIMARY KEY (snapshot_id);


--
-- Name: dsl_snapshots dsl_snapshots_session_id_version_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_snapshots
    ADD CONSTRAINT dsl_snapshots_session_id_version_key UNIQUE (session_id, version);


--
-- Name: dsl_verb_categories dsl_verb_categories_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_verb_categories
    ADD CONSTRAINT dsl_verb_categories_pkey PRIMARY KEY (category_code);


--
-- Name: dsl_verb_sync_log dsl_verb_sync_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_verb_sync_log
    ADD CONSTRAINT dsl_verb_sync_log_pkey PRIMARY KEY (sync_id);


--
-- Name: dsl_verbs dsl_verbs_domain_verb_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_verbs
    ADD CONSTRAINT dsl_verbs_domain_verb_name_key UNIQUE (domain, verb_name);


--
-- Name: dsl_verbs dsl_verbs_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_verbs
    ADD CONSTRAINT dsl_verbs_pkey PRIMARY KEY (verb_id);


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
-- Name: dsl_view_state_changes dsl_view_state_changes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_view_state_changes
    ADD CONSTRAINT dsl_view_state_changes_pkey PRIMARY KEY (change_id);


--
-- Name: dsl_workflow_phases dsl_workflow_phases_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_workflow_phases
    ADD CONSTRAINT dsl_workflow_phases_pkey PRIMARY KEY (phase_code);


--
-- Name: edge_types edge_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".edge_types
    ADD CONSTRAINT edge_types_pkey PRIMARY KEY (edge_type_code);


--
-- Name: entities entities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entities
    ADD CONSTRAINT entities_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_addresses entity_addresses_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_addresses
    ADD CONSTRAINT entity_addresses_pkey PRIMARY KEY (address_id);


--
-- Name: entity_bods_links entity_bods_links_entity_id_bods_entity_statement_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_bods_links
    ADD CONSTRAINT entity_bods_links_entity_id_bods_entity_statement_id_key UNIQUE (entity_id, bods_entity_statement_id);


--
-- Name: entity_bods_links entity_bods_links_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_bods_links
    ADD CONSTRAINT entity_bods_links_pkey PRIMARY KEY (link_id);


--
-- Name: entity_cooperatives entity_cooperatives_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_cooperatives
    ADD CONSTRAINT entity_cooperatives_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_cooperatives entity_cooperatives_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_cooperatives
    ADD CONSTRAINT entity_cooperatives_pkey PRIMARY KEY (cooperative_id);


--
-- Name: entity_crud_rules entity_crud_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_crud_rules
    ADD CONSTRAINT entity_crud_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: entity_foundations entity_foundations_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_foundations
    ADD CONSTRAINT entity_foundations_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_foundations entity_foundations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_foundations
    ADD CONSTRAINT entity_foundations_pkey PRIMARY KEY (foundation_id);


--
-- Name: entity_funds entity_funds_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_funds
    ADD CONSTRAINT entity_funds_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_funds entity_funds_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_funds
    ADD CONSTRAINT entity_funds_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_government entity_government_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_government
    ADD CONSTRAINT entity_government_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_government entity_government_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_government
    ADD CONSTRAINT entity_government_pkey PRIMARY KEY (government_id);


--
-- Name: entity_identifiers entity_identifiers_entity_id_identifier_type_identifier_val_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_identifiers
    ADD CONSTRAINT entity_identifiers_entity_id_identifier_type_identifier_val_key UNIQUE (entity_id, identifier_type, identifier_value);


--
-- Name: entity_identifiers entity_identifiers_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_identifiers
    ADD CONSTRAINT entity_identifiers_pkey PRIMARY KEY (identifier_id);


--
-- Name: entity_lifecycle_events entity_lifecycle_events_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_lifecycle_events
    ADD CONSTRAINT entity_lifecycle_events_pkey PRIMARY KEY (event_id);


--
-- Name: entity_limited_companies entity_limited_companies_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_limited_companies entity_limited_companies_lei_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_lei_key UNIQUE (lei);


--
-- Name: entity_limited_companies entity_limited_companies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_pkey PRIMARY KEY (limited_company_id);


--
-- Name: entity_manco entity_manco_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_manco
    ADD CONSTRAINT entity_manco_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_manco entity_manco_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_manco
    ADD CONSTRAINT entity_manco_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_names entity_names_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_names
    ADD CONSTRAINT entity_names_pkey PRIMARY KEY (name_id);


--
-- Name: entity_parent_relationships entity_parent_relationships_child_entity_id_parent_lei_rela_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_parent_relationships
    ADD CONSTRAINT entity_parent_relationships_child_entity_id_parent_lei_rela_key UNIQUE (child_entity_id, parent_lei, relationship_type);


--
-- Name: entity_parent_relationships entity_parent_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_parent_relationships
    ADD CONSTRAINT entity_parent_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: entity_partnerships entity_partnerships_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_partnerships
    ADD CONSTRAINT entity_partnerships_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_partnerships entity_partnerships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_partnerships
    ADD CONSTRAINT entity_partnerships_pkey PRIMARY KEY (partnership_id);


--
-- Name: entity_proper_persons entity_proper_persons_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_proper_persons entity_proper_persons_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_pkey PRIMARY KEY (proper_person_id);


--
-- Name: entity_regulatory_profiles entity_regulatory_profiles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_regulatory_profiles
    ADD CONSTRAINT entity_regulatory_profiles_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_relationships_history entity_relationships_history_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_relationships_history
    ADD CONSTRAINT entity_relationships_history_pkey PRIMARY KEY (history_id);


--
-- Name: entity_relationships entity_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_relationships
    ADD CONSTRAINT entity_relationships_pkey PRIMARY KEY (relationship_id);


--
-- Name: entity_share_classes entity_share_classes_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_share_classes
    ADD CONSTRAINT entity_share_classes_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_share_classes entity_share_classes_isin_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_share_classes
    ADD CONSTRAINT entity_share_classes_isin_key UNIQUE (isin);


--
-- Name: entity_share_classes entity_share_classes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_share_classes
    ADD CONSTRAINT entity_share_classes_pkey PRIMARY KEY (entity_id);


--
-- Name: entity_trusts entity_trusts_entity_id_uniq; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_trusts
    ADD CONSTRAINT entity_trusts_entity_id_uniq UNIQUE (entity_id);


--
-- Name: entity_trusts entity_trusts_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_trusts
    ADD CONSTRAINT entity_trusts_pkey PRIMARY KEY (trust_id);


--
-- Name: entity_type_dependencies entity_type_dependencies_from_type_from_subtype_to_type_to__key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_type_dependencies
    ADD CONSTRAINT entity_type_dependencies_from_type_from_subtype_to_type_to__key UNIQUE (from_type, from_subtype, to_type, to_subtype);


--
-- Name: entity_type_dependencies entity_type_dependencies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_type_dependencies
    ADD CONSTRAINT entity_type_dependencies_pkey PRIMARY KEY (dependency_id);


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
-- Name: entity_ubos entity_ubos_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_ubos
    ADD CONSTRAINT entity_ubos_pkey PRIMARY KEY (ubo_id);


--
-- Name: entity_validation_rules entity_validation_rules_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_validation_rules
    ADD CONSTRAINT entity_validation_rules_pkey PRIMARY KEY (rule_id);


--
-- Name: fund_investments fund_investments_investor_entity_id_investee_entity_id_inve_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investments
    ADD CONSTRAINT fund_investments_investor_entity_id_investee_entity_id_inve_key UNIQUE (investor_entity_id, investee_entity_id, investment_date);


--
-- Name: fund_investments fund_investments_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investments
    ADD CONSTRAINT fund_investments_pkey PRIMARY KEY (investment_id);


--
-- Name: fund_investors fund_investors_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investors
    ADD CONSTRAINT fund_investors_pkey PRIMARY KEY (investor_id);


--
-- Name: fund_structure fund_structure_parent_entity_id_child_entity_id_relationshi_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_structure
    ADD CONSTRAINT fund_structure_parent_entity_id_child_entity_id_relationshi_key UNIQUE (parent_entity_id, child_entity_id, relationship_type, effective_from);


--
-- Name: fund_structure fund_structure_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_structure
    ADD CONSTRAINT fund_structure_pkey PRIMARY KEY (structure_id);


--
-- Name: gleif_sync_log gleif_sync_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".gleif_sync_log
    ADD CONSTRAINT gleif_sync_log_pkey PRIMARY KEY (sync_id);


--
-- Name: instrument_lifecycles instrument_lifecycles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".instrument_lifecycles
    ADD CONSTRAINT instrument_lifecycles_pkey PRIMARY KEY (instrument_lifecycle_id);


--
-- Name: instrument_lifecycles instrument_lifecycles_unique; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".instrument_lifecycles
    ADD CONSTRAINT instrument_lifecycles_unique UNIQUE (instrument_class_id, lifecycle_id);


--
-- Name: intent_feedback_analysis intent_feedback_analysis_analysis_type_analysis_date_data_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".intent_feedback_analysis
    ADD CONSTRAINT intent_feedback_analysis_analysis_type_analysis_date_data_key UNIQUE (analysis_type, analysis_date, data);


--
-- Name: intent_feedback_analysis intent_feedback_analysis_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".intent_feedback_analysis
    ADD CONSTRAINT intent_feedback_analysis_pkey PRIMARY KEY (id);


--
-- Name: intent_feedback intent_feedback_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".intent_feedback
    ADD CONSTRAINT intent_feedback_pkey PRIMARY KEY (id);


--
-- Name: kyc_case_sponsor_decisions kyc_case_sponsor_decisions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_case_sponsor_decisions
    ADD CONSTRAINT kyc_case_sponsor_decisions_pkey PRIMARY KEY (decision_id);


--
-- Name: kyc_decisions kyc_decisions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_decisions
    ADD CONSTRAINT kyc_decisions_pkey PRIMARY KEY (decision_id);


--
-- Name: kyc_service_agreements kyc_service_agreements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_service_agreements
    ADD CONSTRAINT kyc_service_agreements_pkey PRIMARY KEY (agreement_id);


--
-- Name: layout_cache layout_cache_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".layout_cache
    ADD CONSTRAINT layout_cache_pkey PRIMARY KEY (cache_id);


--
-- Name: layout_config layout_config_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".layout_config
    ADD CONSTRAINT layout_config_pkey PRIMARY KEY (config_key);


--
-- Name: lifecycle_resource_capabilities lifecycle_resource_capabilities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycle_resource_capabilities
    ADD CONSTRAINT lifecycle_resource_capabilities_pkey PRIMARY KEY (capability_id);


--
-- Name: lifecycle_resource_capabilities lifecycle_resource_capabilities_unique; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycle_resource_capabilities
    ADD CONSTRAINT lifecycle_resource_capabilities_unique UNIQUE (lifecycle_id, resource_type_id);


--
-- Name: lifecycle_resource_types lifecycle_resource_types_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycle_resource_types
    ADD CONSTRAINT lifecycle_resource_types_code_key UNIQUE (code);


--
-- Name: lifecycle_resource_types lifecycle_resource_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycle_resource_types
    ADD CONSTRAINT lifecycle_resource_types_pkey PRIMARY KEY (resource_type_id);


--
-- Name: lifecycles lifecycles_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycles
    ADD CONSTRAINT lifecycles_code_key UNIQUE (code);


--
-- Name: lifecycles lifecycles_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycles
    ADD CONSTRAINT lifecycles_pkey PRIMARY KEY (lifecycle_id);


--
-- Name: market_csd_mappings market_csd_mappings_market_csd_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".market_csd_mappings
    ADD CONSTRAINT market_csd_mappings_market_csd_key UNIQUE (market_id, csd_code);


--
-- Name: market_csd_mappings market_csd_mappings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".market_csd_mappings
    ADD CONSTRAINT market_csd_mappings_pkey PRIMARY KEY (mapping_id);


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
-- Name: node_types node_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".node_types
    ADD CONSTRAINT node_types_pkey PRIMARY KEY (node_type_code);


--
-- Name: observation_discrepancies observation_discrepancies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_pkey PRIMARY KEY (discrepancy_id);


--
-- Name: onboarding_executions onboarding_executions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_executions
    ADD CONSTRAINT onboarding_executions_pkey PRIMARY KEY (execution_id);


--
-- Name: onboarding_plans onboarding_plans_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_plans
    ADD CONSTRAINT onboarding_plans_pkey PRIMARY KEY (plan_id);


--
-- Name: onboarding_products onboarding_products_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_pkey PRIMARY KEY (onboarding_product_id);


--
-- Name: onboarding_products onboarding_products_request_id_product_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_request_id_product_id_key UNIQUE (request_id, product_id);


--
-- Name: onboarding_requests onboarding_requests_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_requests
    ADD CONSTRAINT onboarding_requests_pkey PRIMARY KEY (request_id);


--
-- Name: onboarding_tasks onboarding_tasks_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_tasks
    ADD CONSTRAINT onboarding_tasks_pkey PRIMARY KEY (task_id);


--
-- Name: service_resource_types prod_resources_resource_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_types
    ADD CONSTRAINT prod_resources_resource_code_key UNIQUE (resource_code);


--
-- Name: product_services product_services_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".product_services
    ADD CONSTRAINT product_services_pkey PRIMARY KEY (product_id, service_id);


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
-- Name: products products_product_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".products
    ADD CONSTRAINT products_product_code_key UNIQUE (product_code);


--
-- Name: proofs proofs_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".proofs
    ADD CONSTRAINT proofs_pkey PRIMARY KEY (proof_id);


--
-- Name: red_flag_severities red_flag_severities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".red_flag_severities
    ADD CONSTRAINT red_flag_severities_pkey PRIMARY KEY (code);


--
-- Name: redflag_score_config redflag_score_config_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".redflag_score_config
    ADD CONSTRAINT redflag_score_config_pkey PRIMARY KEY (config_id);


--
-- Name: regulators regulators_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".regulators
    ADD CONSTRAINT regulators_pkey PRIMARY KEY (regulator_code);


--
-- Name: regulatory_tiers regulatory_tiers_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".regulatory_tiers
    ADD CONSTRAINT regulatory_tiers_pkey PRIMARY KEY (tier_code);


--
-- Name: requirement_acceptable_docs requirement_acceptable_docs_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".requirement_acceptable_docs
    ADD CONSTRAINT requirement_acceptable_docs_pkey PRIMARY KEY (requirement_id, document_type_code);


--
-- Name: resource_attribute_requirements resource_attribute_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_pkey PRIMARY KEY (requirement_id);


--
-- Name: resource_attribute_requirements resource_attribute_requirements_resource_id_attribute_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_resource_id_attribute_id_key UNIQUE (resource_id, attribute_id);


--
-- Name: resource_dependencies resource_dependencies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_dependencies
    ADD CONSTRAINT resource_dependencies_pkey PRIMARY KEY (dependency_id);


--
-- Name: resource_dependencies resource_dependencies_resource_type_id_depends_on_type_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_dependencies
    ADD CONSTRAINT resource_dependencies_resource_type_id_depends_on_type_id_key UNIQUE (resource_type_id, depends_on_type_id);


--
-- Name: resource_instance_attributes resource_instance_attributes_instance_id_attribute_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_instance_id_attribute_id_key UNIQUE (instance_id, attribute_id);


--
-- Name: resource_instance_attributes resource_instance_attributes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_pkey PRIMARY KEY (value_id);


--
-- Name: resource_instance_dependencies resource_instance_dependencies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_dependencies
    ADD CONSTRAINT resource_instance_dependencies_pkey PRIMARY KEY (instance_id, depends_on_instance_id);


--
-- Name: resource_profile_sources resource_profile_sources_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_profile_sources
    ADD CONSTRAINT resource_profile_sources_pkey PRIMARY KEY (link_id);


--
-- Name: risk_bands risk_bands_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".risk_bands
    ADD CONSTRAINT risk_bands_pkey PRIMARY KEY (band_code);


--
-- Name: risk_ratings risk_ratings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".risk_ratings
    ADD CONSTRAINT risk_ratings_pkey PRIMARY KEY (code);


--
-- Name: role_categories role_categories_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_categories
    ADD CONSTRAINT role_categories_pkey PRIMARY KEY (category_code);


--
-- Name: role_incompatibilities role_incompatibilities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_incompatibilities
    ADD CONSTRAINT role_incompatibilities_pkey PRIMARY KEY (incompatibility_id);


--
-- Name: role_requirements role_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_requirements
    ADD CONSTRAINT role_requirements_pkey PRIMARY KEY (requirement_id);


--
-- Name: role_types role_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_types
    ADD CONSTRAINT role_types_pkey PRIMARY KEY (role_code);


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
-- Name: screening_lists screening_lists_list_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".screening_lists
    ADD CONSTRAINT screening_lists_list_code_key UNIQUE (list_code);


--
-- Name: screening_lists screening_lists_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".screening_lists
    ADD CONSTRAINT screening_lists_pkey PRIMARY KEY (screening_list_id);


--
-- Name: screening_requirements screening_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".screening_requirements
    ADD CONSTRAINT screening_requirements_pkey PRIMARY KEY (risk_band, screening_type);


--
-- Name: screening_types screening_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".screening_types
    ADD CONSTRAINT screening_types_pkey PRIMARY KEY (code);


--
-- Name: semantic_match_cache semantic_match_cache_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".semantic_match_cache
    ADD CONSTRAINT semantic_match_cache_pkey PRIMARY KEY (id);


--
-- Name: semantic_match_cache semantic_match_cache_transcript_normalized_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".semantic_match_cache
    ADD CONSTRAINT semantic_match_cache_transcript_normalized_key UNIQUE (transcript_normalized);


--
-- Name: service_delivery_map service_delivery_map_cbu_id_product_id_service_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_cbu_id_product_id_service_id_key UNIQUE (cbu_id, product_id, service_id);


--
-- Name: service_delivery_map service_delivery_map_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_pkey PRIMARY KEY (delivery_id);


--
-- Name: service_option_choices service_option_choices_option_def_id_choice_value_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_option_choices
    ADD CONSTRAINT service_option_choices_option_def_id_choice_value_key UNIQUE (option_def_id, choice_value);


--
-- Name: service_option_choices service_option_choices_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_option_choices
    ADD CONSTRAINT service_option_choices_pkey PRIMARY KEY (choice_id);


--
-- Name: service_option_definitions service_option_definitions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_option_definitions
    ADD CONSTRAINT service_option_definitions_pkey PRIMARY KEY (option_def_id);


--
-- Name: service_option_definitions service_option_definitions_service_id_option_key_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_option_definitions
    ADD CONSTRAINT service_option_definitions_service_id_option_key_key UNIQUE (service_id, option_key);


--
-- Name: service_resource_capabilities service_resource_capabilities_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_pkey PRIMARY KEY (capability_id);


--
-- Name: service_resource_capabilities service_resource_capabilities_service_id_resource_id_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_service_id_resource_id_key UNIQUE (service_id, resource_id);


--
-- Name: service_resource_types service_resource_types_name_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_types
    ADD CONSTRAINT service_resource_types_name_key UNIQUE (name);


--
-- Name: service_resource_types service_resource_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_types
    ADD CONSTRAINT service_resource_types_pkey PRIMARY KEY (resource_id);


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
-- Name: services services_service_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".services
    ADD CONSTRAINT services_service_code_key UNIQUE (service_code);


--
-- Name: settlement_types settlement_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".settlement_types
    ADD CONSTRAINT settlement_types_pkey PRIMARY KEY (code);


--
-- Name: sla_breaches sla_breaches_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_breaches
    ADD CONSTRAINT sla_breaches_pkey PRIMARY KEY (breach_id);


--
-- Name: sla_measurements sla_measurements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_measurements
    ADD CONSTRAINT sla_measurements_pkey PRIMARY KEY (measurement_id);


--
-- Name: sla_metric_types sla_metric_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_metric_types
    ADD CONSTRAINT sla_metric_types_pkey PRIMARY KEY (metric_code);


--
-- Name: sla_templates sla_templates_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_templates
    ADD CONSTRAINT sla_templates_pkey PRIMARY KEY (template_id);


--
-- Name: sla_templates sla_templates_template_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_templates
    ADD CONSTRAINT sla_templates_template_code_key UNIQUE (template_code);


--
-- Name: ssi_types ssi_types_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ssi_types
    ADD CONSTRAINT ssi_types_pkey PRIMARY KEY (code);


--
-- Name: taxonomy_crud_log taxonomy_crud_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".taxonomy_crud_log
    ADD CONSTRAINT taxonomy_crud_log_pkey PRIMARY KEY (operation_id);


--
-- Name: threshold_factors threshold_factors_factor_type_factor_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".threshold_factors
    ADD CONSTRAINT threshold_factors_factor_type_factor_code_key UNIQUE (factor_type, factor_code);


--
-- Name: threshold_factors threshold_factors_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".threshold_factors
    ADD CONSTRAINT threshold_factors_pkey PRIMARY KEY (factor_id);


--
-- Name: threshold_requirements threshold_requirements_entity_role_risk_band_attribute_code_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".threshold_requirements
    ADD CONSTRAINT threshold_requirements_entity_role_risk_band_attribute_code_key UNIQUE (entity_role, risk_band, attribute_code);


--
-- Name: threshold_requirements threshold_requirements_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".threshold_requirements
    ADD CONSTRAINT threshold_requirements_pkey PRIMARY KEY (requirement_id);


--
-- Name: trading_profile_documents trading_profile_documents_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trading_profile_documents
    ADD CONSTRAINT trading_profile_documents_pkey PRIMARY KEY (link_id);


--
-- Name: trading_profile_materializations trading_profile_materializations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trading_profile_materializations
    ADD CONSTRAINT trading_profile_materializations_pkey PRIMARY KEY (materialization_id);


--
-- Name: trading_profile_migration_backup trading_profile_migration_backup_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trading_profile_migration_backup
    ADD CONSTRAINT trading_profile_migration_backup_pkey PRIMARY KEY (backup_id);


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
-- Name: ubo_assertion_log ubo_assertion_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_assertion_log
    ADD CONSTRAINT ubo_assertion_log_pkey PRIMARY KEY (log_id);


--
-- Name: ubo_evidence ubo_evidence_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_evidence
    ADD CONSTRAINT ubo_evidence_pkey PRIMARY KEY (ubo_evidence_id);


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
-- Name: ubo_snapshot_comparisons ubo_snapshot_comparisons_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshot_comparisons
    ADD CONSTRAINT ubo_snapshot_comparisons_pkey PRIMARY KEY (comparison_id);


--
-- Name: ubo_snapshots ubo_snapshots_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshots
    ADD CONSTRAINT ubo_snapshots_pkey PRIMARY KEY (snapshot_id);


--
-- Name: ubo_treatments ubo_treatments_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_treatments
    ADD CONSTRAINT ubo_treatments_pkey PRIMARY KEY (treatment_code);


--
-- Name: attribute_registry uk_attribute_uuid; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_registry
    ADD CONSTRAINT uk_attribute_uuid UNIQUE (uuid);


--
-- Name: document_attribute_links unique_doc_attr_direction; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_links
    ADD CONSTRAINT unique_doc_attr_direction UNIQUE (document_type_id, attribute_id, direction);


--
-- Name: cbu_entity_roles uq_cbu_entity_role; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT uq_cbu_entity_role UNIQUE (cbu_id, entity_id, role_id);


--
-- Name: entity_relationships uq_entity_relationship; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_relationships
    ADD CONSTRAINT uq_entity_relationship UNIQUE (from_entity_id, to_entity_id, relationship_type);


--
-- Name: fund_investors uq_fund_investor; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investors
    ADD CONSTRAINT uq_fund_investor UNIQUE (fund_cbu_id, investor_entity_id);


--
-- Name: redflag_score_config uq_redflag_severity; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".redflag_score_config
    ADD CONSTRAINT uq_redflag_severity UNIQUE (severity);


--
-- Name: role_incompatibilities uq_role_pair; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_incompatibilities
    ADD CONSTRAINT uq_role_pair UNIQUE (role_a, role_b);


--
-- Name: workflow_instances uq_workflow_subject; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workflow_instances
    ADD CONSTRAINT uq_workflow_subject UNIQUE (workflow_id, subject_type, subject_id);


--
-- Name: workflow_definitions uq_workflow_version; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workflow_definitions
    ADD CONSTRAINT uq_workflow_version UNIQUE (workflow_id, version);


--
-- Name: verb_pattern_embeddings verb_pattern_embeddings_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verb_pattern_embeddings
    ADD CONSTRAINT verb_pattern_embeddings_pkey PRIMARY KEY (id);


--
-- Name: verb_pattern_embeddings verb_pattern_embeddings_verb_name_pattern_normalized_key; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verb_pattern_embeddings
    ADD CONSTRAINT verb_pattern_embeddings_verb_name_pattern_normalized_key UNIQUE (verb_name, pattern_normalized);


--
-- Name: verification_challenges verification_challenges_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_challenges
    ADD CONSTRAINT verification_challenges_pkey PRIMARY KEY (challenge_id);


--
-- Name: verification_escalations verification_escalations_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_escalations
    ADD CONSTRAINT verification_escalations_pkey PRIMARY KEY (escalation_id);


--
-- Name: view_modes view_modes_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".view_modes
    ADD CONSTRAINT view_modes_pkey PRIMARY KEY (view_mode_code);


--
-- Name: workflow_audit_log workflow_audit_log_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workflow_audit_log
    ADD CONSTRAINT workflow_audit_log_pkey PRIMARY KEY (log_id);


--
-- Name: workflow_definitions workflow_definitions_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workflow_definitions
    ADD CONSTRAINT workflow_definitions_pkey PRIMARY KEY (workflow_id);


--
-- Name: workflow_instances workflow_instances_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workflow_instances
    ADD CONSTRAINT workflow_instances_pkey PRIMARY KEY (instance_id);


--
-- Name: workstream_statuses workstream_statuses_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workstream_statuses
    ADD CONSTRAINT workstream_statuses_pkey PRIMARY KEY (code);


--
-- Name: entity_regulatory_registrations entity_regulatory_registrations_pkey; Type: CONSTRAINT; Schema: ob_kyc; Owner: -
--

ALTER TABLE ONLY ob_kyc.entity_regulatory_registrations
    ADD CONSTRAINT entity_regulatory_registrations_pkey PRIMARY KEY (registration_id);


--
-- Name: entity_regulatory_registrations uq_entity_regulator; Type: CONSTRAINT; Schema: ob_kyc; Owner: -
--

ALTER TABLE ONLY ob_kyc.entity_regulatory_registrations
    ADD CONSTRAINT uq_entity_regulator UNIQUE (entity_id, regulator_code);


--
-- Name: registration_types registration_types_pkey; Type: CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.registration_types
    ADD CONSTRAINT registration_types_pkey PRIMARY KEY (registration_type);


--
-- Name: regulators regulators_pkey; Type: CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.regulators
    ADD CONSTRAINT regulators_pkey PRIMARY KEY (regulator_code);


--
-- Name: regulatory_tiers regulatory_tiers_pkey; Type: CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.regulatory_tiers
    ADD CONSTRAINT regulatory_tiers_pkey PRIMARY KEY (tier_code);


--
-- Name: request_types request_types_pkey; Type: CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.request_types
    ADD CONSTRAINT request_types_pkey PRIMARY KEY (request_type, request_subtype);


--
-- Name: role_types role_types_code_key; Type: CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.role_types
    ADD CONSTRAINT role_types_code_key UNIQUE (code);


--
-- Name: role_types role_types_pkey; Type: CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.role_types
    ADD CONSTRAINT role_types_pkey PRIMARY KEY (role_type_id);


--
-- Name: attribute_sources attribute_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attribute_sources
    ADD CONSTRAINT attribute_sources_pkey PRIMARY KEY (id);


--
-- Name: attribute_sources attribute_sources_source_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.attribute_sources
    ADD CONSTRAINT attribute_sources_source_key_key UNIQUE (source_key);


--
-- Name: business_attributes business_attributes_entity_name_attribute_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_entity_name_attribute_name_key UNIQUE (entity_name, attribute_name);


--
-- Name: business_attributes business_attributes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_pkey PRIMARY KEY (id);


--
-- Name: credentials_vault credentials_vault_credential_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.credentials_vault
    ADD CONSTRAINT credentials_vault_credential_name_key UNIQUE (credential_name);


--
-- Name: credentials_vault credentials_vault_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.credentials_vault
    ADD CONSTRAINT credentials_vault_pkey PRIMARY KEY (credential_id);


--
-- Name: data_domains data_domains_domain_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_domains
    ADD CONSTRAINT data_domains_domain_name_key UNIQUE (domain_name);


--
-- Name: data_domains data_domains_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_domains
    ADD CONSTRAINT data_domains_pkey PRIMARY KEY (id);


--
-- Name: derived_attributes derived_attributes_entity_name_attribute_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.derived_attributes
    ADD CONSTRAINT derived_attributes_entity_name_attribute_name_key UNIQUE (entity_name, attribute_name);


--
-- Name: derived_attributes derived_attributes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.derived_attributes
    ADD CONSTRAINT derived_attributes_pkey PRIMARY KEY (id);


--
-- Name: rule_categories rule_categories_category_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_categories
    ADD CONSTRAINT rule_categories_category_key_key UNIQUE (category_key);


--
-- Name: rule_categories rule_categories_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_categories
    ADD CONSTRAINT rule_categories_pkey PRIMARY KEY (id);


--
-- Name: rule_dependencies rule_dependencies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_pkey PRIMARY KEY (id);


--
-- Name: rule_dependencies rule_dependencies_rule_id_attribute_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_rule_id_attribute_id_key UNIQUE (rule_id, attribute_id);


--
-- Name: rule_executions rule_executions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_executions
    ADD CONSTRAINT rule_executions_pkey PRIMARY KEY (id);


--
-- Name: rule_versions rule_versions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_versions
    ADD CONSTRAINT rule_versions_pkey PRIMARY KEY (id);


--
-- Name: rule_versions rule_versions_rule_id_version_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_versions
    ADD CONSTRAINT rule_versions_rule_id_version_key UNIQUE (rule_id, version);


--
-- Name: rules rules_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_pkey PRIMARY KEY (id);


--
-- Name: rules rules_rule_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_rule_id_key UNIQUE (rule_id);


--
-- Name: access_attestations access_attestations_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_attestations
    ADD CONSTRAINT access_attestations_pkey PRIMARY KEY (attestation_id);


--
-- Name: access_domains access_domains_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_domains
    ADD CONSTRAINT access_domains_pkey PRIMARY KEY (domain_code);


--
-- Name: access_review_campaigns access_review_campaigns_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_campaigns
    ADD CONSTRAINT access_review_campaigns_pkey PRIMARY KEY (campaign_id);


--
-- Name: access_review_items access_review_items_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_items
    ADD CONSTRAINT access_review_items_pkey PRIMARY KEY (item_id);


--
-- Name: access_review_log access_review_log_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_log
    ADD CONSTRAINT access_review_log_pkey PRIMARY KEY (log_id);


--
-- Name: function_domains function_domains_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.function_domains
    ADD CONSTRAINT function_domains_pkey PRIMARY KEY (function_name);


--
-- Name: membership_audit_log membership_audit_log_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.membership_audit_log
    ADD CONSTRAINT membership_audit_log_pkey PRIMARY KEY (log_id);


--
-- Name: membership_history membership_history_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.membership_history
    ADD CONSTRAINT membership_history_pkey PRIMARY KEY (history_id);


--
-- Name: memberships memberships_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.memberships
    ADD CONSTRAINT memberships_pkey PRIMARY KEY (membership_id);


--
-- Name: memberships memberships_team_id_user_id_role_key_key; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.memberships
    ADD CONSTRAINT memberships_team_id_user_id_role_key_key UNIQUE (team_id, user_id, role_key);


--
-- Name: team_cbu_access team_cbu_access_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_cbu_access
    ADD CONSTRAINT team_cbu_access_pkey PRIMARY KEY (access_id);


--
-- Name: team_cbu_access team_cbu_access_team_id_cbu_id_key; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_cbu_access
    ADD CONSTRAINT team_cbu_access_team_id_cbu_id_key UNIQUE (team_id, cbu_id);


--
-- Name: team_service_entitlements team_service_entitlements_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_service_entitlements
    ADD CONSTRAINT team_service_entitlements_pkey PRIMARY KEY (entitlement_id);


--
-- Name: team_service_entitlements team_service_entitlements_team_id_service_code_key; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_service_entitlements
    ADD CONSTRAINT team_service_entitlements_team_id_service_code_key UNIQUE (team_id, service_code);


--
-- Name: teams teams_pkey; Type: CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.teams
    ADD CONSTRAINT teams_pkey PRIMARY KEY (team_id);


--
-- Name: idx_commitments_client; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_commitments_client ON client_portal.commitments USING btree (client_id);


--
-- Name: idx_commitments_request; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_commitments_request ON client_portal.commitments USING btree (request_id);


--
-- Name: idx_commitments_status; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_commitments_status ON client_portal.commitments USING btree (status) WHERE ((status)::text = 'PENDING'::text);


--
-- Name: idx_credentials_client; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_credentials_client ON client_portal.credentials USING btree (client_id);


--
-- Name: idx_escalations_client; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_escalations_client ON client_portal.escalations USING btree (client_id);


--
-- Name: idx_escalations_status; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_escalations_status ON client_portal.escalations USING btree (status) WHERE ((status)::text <> ALL ((ARRAY['RESOLVED'::character varying, 'CLOSED'::character varying])::text[]));


--
-- Name: idx_sessions_client; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_sessions_client ON client_portal.sessions USING btree (client_id);


--
-- Name: idx_sessions_expires; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_sessions_expires ON client_portal.sessions USING btree (expires_at);


--
-- Name: idx_submissions_client; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_submissions_client ON client_portal.submissions USING btree (client_id);


--
-- Name: idx_submissions_request; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_submissions_request ON client_portal.submissions USING btree (request_id);


--
-- Name: idx_submissions_status; Type: INDEX; Schema: client_portal; Owner: -
--

CREATE INDEX idx_submissions_status ON client_portal.submissions USING btree (status);


--
-- Name: idx_booking_rules_lookup; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_booking_rules_lookup ON custody.ssi_booking_rules USING btree (cbu_id, is_active, priority, instrument_class_id, security_type_id, market_id, currency);


--
-- Name: idx_cbu_cross_border_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_cross_border_cbu ON custody.cbu_cross_border_config USING btree (cbu_id);


--
-- Name: idx_cbu_im_active; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_im_active ON custody.cbu_im_assignments USING btree (cbu_id) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_cbu_im_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_im_cbu ON custody.cbu_im_assignments USING btree (cbu_id);


--
-- Name: idx_cbu_im_manager; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_im_manager ON custody.cbu_im_assignments USING btree (manager_entity_id);


--
-- Name: idx_cbu_im_method; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_im_method ON custody.cbu_im_assignments USING btree (instruction_method);


--
-- Name: idx_cbu_im_profile; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_im_profile ON custody.cbu_im_assignments USING btree (profile_id);


--
-- Name: idx_cbu_pricing_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_pricing_cbu ON custody.cbu_pricing_config USING btree (cbu_id);


--
-- Name: idx_cbu_pricing_class; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_pricing_class ON custody.cbu_pricing_config USING btree (instrument_class_id);


--
-- Name: idx_cbu_settlement_chains_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_settlement_chains_cbu ON custody.cbu_settlement_chains USING btree (cbu_id);


--
-- Name: idx_cbu_settlement_chains_market; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_settlement_chains_market ON custody.cbu_settlement_chains USING btree (market_id);


--
-- Name: idx_cbu_settlement_loc_prefs_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_settlement_loc_prefs_cbu ON custody.cbu_settlement_location_preferences USING btree (cbu_id);


--
-- Name: idx_cbu_ssi_active; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_ssi_active ON custody.cbu_ssi USING btree (cbu_id, status) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_cbu_ssi_lookup; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_ssi_lookup ON custody.cbu_ssi USING btree (cbu_id, status);


--
-- Name: idx_cbu_sweep_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_sweep_cbu ON custody.cbu_cash_sweep_config USING btree (cbu_id);


--
-- Name: idx_cbu_tax_reclaim_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_tax_reclaim_cbu ON custody.cbu_tax_reclaim_config USING btree (cbu_id);


--
-- Name: idx_cbu_tax_reporting_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_tax_reporting_cbu ON custody.cbu_tax_reporting USING btree (cbu_id);


--
-- Name: idx_cbu_tax_reporting_regime; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_tax_reporting_regime ON custody.cbu_tax_reporting USING btree (reporting_regime);


--
-- Name: idx_cbu_tax_status_cbu; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_tax_status_cbu ON custody.cbu_tax_status USING btree (cbu_id);


--
-- Name: idx_cbu_tax_status_jurisdiction; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_tax_status_jurisdiction ON custody.cbu_tax_status USING btree (tax_jurisdiction_id);


--
-- Name: idx_settlement_chain_hops_chain; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_settlement_chain_hops_chain ON custody.settlement_chain_hops USING btree (chain_id);


--
-- Name: idx_settlement_locations_code; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_settlement_locations_code ON custody.settlement_locations USING btree (location_code);


--
-- Name: idx_settlement_locations_type; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_settlement_locations_type ON custody.settlement_locations USING btree (location_type);


--
-- Name: idx_tax_jurisdictions_code; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_tax_jurisdictions_code ON custody.tax_jurisdictions USING btree (jurisdiction_code);


--
-- Name: idx_tax_jurisdictions_country; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_tax_jurisdictions_country ON custody.tax_jurisdictions USING btree (country_code);


--
-- Name: idx_tax_treaty_investor; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_tax_treaty_investor ON custody.tax_treaty_rates USING btree (investor_jurisdiction_id);


--
-- Name: idx_tax_treaty_source; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_tax_treaty_source ON custody.tax_treaty_rates USING btree (source_jurisdiction_id);


--
-- Name: cases_cbu_type_active_uniq; Type: INDEX; Schema: kyc; Owner: -
--

CREATE UNIQUE INDEX cases_cbu_type_active_uniq ON kyc.cases USING btree (cbu_id, case_type) WHERE (closed_at IS NULL);


--
-- Name: idx_case_events_case; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_case_events_case ON kyc.case_events USING btree (case_id);


--
-- Name: idx_case_events_time; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_case_events_time ON kyc.case_events USING btree (occurred_at DESC);


--
-- Name: idx_case_events_type; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_case_events_type ON kyc.case_events USING btree (event_type);


--
-- Name: idx_case_events_workstream; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_case_events_workstream ON kyc.case_events USING btree (workstream_id) WHERE (workstream_id IS NOT NULL);


--
-- Name: idx_cases_analyst; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_cases_analyst ON kyc.cases USING btree (assigned_analyst_id) WHERE (assigned_analyst_id IS NOT NULL);


--
-- Name: idx_cases_cbu; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_cases_cbu ON kyc.cases USING btree (cbu_id);


--
-- Name: idx_cases_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_cases_status ON kyc.cases USING btree (status);


--
-- Name: idx_doc_request_types_request; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_doc_request_types_request ON kyc.doc_request_acceptable_types USING btree (request_id);


--
-- Name: idx_doc_requests_batch; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_doc_requests_batch ON kyc.doc_requests USING btree (batch_id) WHERE (batch_id IS NOT NULL);


--
-- Name: idx_doc_requests_due_date; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_doc_requests_due_date ON kyc.doc_requests USING btree (due_date);


--
-- Name: idx_doc_requests_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_doc_requests_status ON kyc.doc_requests USING btree (status);


--
-- Name: idx_doc_requests_type; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_doc_requests_type ON kyc.doc_requests USING btree (doc_type);


--
-- Name: idx_doc_requests_workstream; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_doc_requests_workstream ON kyc.doc_requests USING btree (workstream_id);


--
-- Name: idx_holdings_investor; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_holdings_investor ON kyc.holdings USING btree (investor_entity_id);


--
-- Name: idx_holdings_share_class; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_holdings_share_class ON kyc.holdings USING btree (share_class_id);


--
-- Name: idx_movements_holding; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_movements_holding ON kyc.movements USING btree (holding_id);


--
-- Name: idx_movements_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_movements_status ON kyc.movements USING btree (status);


--
-- Name: idx_movements_trade_date; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_movements_trade_date ON kyc.movements USING btree (trade_date);


--
-- Name: idx_oreq_case; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_case ON kyc.outstanding_requests USING btree (case_id) WHERE (case_id IS NOT NULL);


--
-- Name: idx_oreq_cbu; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_cbu ON kyc.outstanding_requests USING btree (cbu_id) WHERE (cbu_id IS NOT NULL);


--
-- Name: idx_oreq_entity; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_entity ON kyc.outstanding_requests USING btree (entity_id) WHERE (entity_id IS NOT NULL);


--
-- Name: idx_oreq_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_status ON kyc.outstanding_requests USING btree (status);


--
-- Name: idx_oreq_status_pending; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_status_pending ON kyc.outstanding_requests USING btree (due_date) WHERE ((status)::text = 'PENDING'::text);


--
-- Name: idx_oreq_subject; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_subject ON kyc.outstanding_requests USING btree (subject_type, subject_id);


--
-- Name: idx_oreq_type; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_type ON kyc.outstanding_requests USING btree (request_type, request_subtype);


--
-- Name: idx_oreq_workstream; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_oreq_workstream ON kyc.outstanding_requests USING btree (workstream_id) WHERE (workstream_id IS NOT NULL);


--
-- Name: idx_red_flags_case; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_red_flags_case ON kyc.red_flags USING btree (case_id);


--
-- Name: idx_red_flags_severity; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_red_flags_severity ON kyc.red_flags USING btree (severity);


--
-- Name: idx_red_flags_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_red_flags_status ON kyc.red_flags USING btree (status);


--
-- Name: idx_red_flags_type; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_red_flags_type ON kyc.red_flags USING btree (flag_type);


--
-- Name: idx_red_flags_workstream; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_red_flags_workstream ON kyc.red_flags USING btree (workstream_id) WHERE (workstream_id IS NOT NULL);


--
-- Name: idx_screenings_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_screenings_status ON kyc.screenings USING btree (status);


--
-- Name: idx_screenings_type; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_screenings_type ON kyc.screenings USING btree (screening_type);


--
-- Name: idx_screenings_workstream; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_screenings_workstream ON kyc.screenings USING btree (workstream_id);


--
-- Name: idx_share_classes_cbu; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_share_classes_cbu ON kyc.share_classes USING btree (cbu_id);


--
-- Name: idx_share_classes_entity; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_share_classes_entity ON kyc.share_classes USING btree (entity_id) WHERE (entity_id IS NOT NULL);


--
-- Name: idx_share_classes_isin; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_share_classes_isin ON kyc.share_classes USING btree (isin) WHERE (isin IS NOT NULL);


--
-- Name: idx_workstreams_case; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_workstreams_case ON kyc.entity_workstreams USING btree (case_id);


--
-- Name: idx_workstreams_discovery; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_workstreams_discovery ON kyc.entity_workstreams USING btree (discovery_source_workstream_id) WHERE (discovery_source_workstream_id IS NOT NULL);


--
-- Name: idx_workstreams_entity; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_workstreams_entity ON kyc.entity_workstreams USING btree (entity_id);


--
-- Name: idx_workstreams_status; Type: INDEX; Schema: kyc; Owner: -
--

CREATE INDEX idx_workstreams_status ON kyc.entity_workstreams USING btree (status);


--
-- Name: entities_type_name_uniq; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX entities_type_name_uniq ON "ob-poc".entities USING btree (entity_type_id, name);


--
-- Name: entity_limited_companies_reg_jurisdiction_uniq; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX entity_limited_companies_reg_jurisdiction_uniq ON "ob-poc".entity_limited_companies USING btree (registration_number, jurisdiction) WHERE ((registration_number IS NOT NULL) AND (jurisdiction IS NOT NULL));


--
-- Name: entity_proper_persons_id_doc_uniq; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX entity_proper_persons_id_doc_uniq ON "ob-poc".entity_proper_persons USING btree (id_document_type, id_document_number) WHERE ((id_document_type IS NOT NULL) AND (id_document_number IS NOT NULL));


--
-- Name: idx_alleg_case; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_alleg_case ON "ob-poc".client_allegations USING btree (case_id) WHERE (case_id IS NOT NULL);


--
-- Name: idx_alleg_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_alleg_cbu ON "ob-poc".client_allegations USING btree (cbu_id);


--
-- Name: idx_alleg_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_alleg_entity ON "ob-poc".client_allegations USING btree (entity_id);


--
-- Name: idx_alleg_pending; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_alleg_pending ON "ob-poc".client_allegations USING btree (cbu_id) WHERE ((verification_status)::text = 'PENDING'::text);


--
-- Name: idx_alleg_workstream; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_alleg_workstream ON "ob-poc".client_allegations USING btree (workstream_id) WHERE (workstream_id IS NOT NULL);


--
-- Name: idx_analysis_pending; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_analysis_pending ON "ob-poc".intent_feedback_analysis USING btree (reviewed) WHERE (NOT reviewed);


--
-- Name: idx_analysis_type_date; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_analysis_type_date ON "ob-poc".intent_feedback_analysis USING btree (analysis_type, analysis_date);


--
-- Name: idx_assertion_log_case; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_assertion_log_case ON "ob-poc".ubo_assertion_log USING btree (case_id);


--
-- Name: idx_assertion_log_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_assertion_log_cbu ON "ob-poc".ubo_assertion_log USING btree (cbu_id);


--
-- Name: idx_assertion_log_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_assertion_log_type ON "ob-poc".ubo_assertion_log USING btree (assertion_type, passed);


--
-- Name: idx_attr_uuid; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attr_uuid ON "ob-poc".attribute_registry USING btree (uuid);


--
-- Name: idx_attribute_dictionary_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_dictionary_active ON "ob-poc".attribute_dictionary USING btree (is_active);


--
-- Name: idx_attribute_dictionary_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_dictionary_domain ON "ob-poc".attribute_dictionary USING btree (domain);


--
-- Name: idx_attribute_registry_applicability; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_registry_applicability ON "ob-poc".attribute_registry USING gin (applicability);


--
-- Name: idx_attribute_registry_category; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_registry_category ON "ob-poc".attribute_registry USING btree (category);


--
-- Name: idx_attribute_registry_embedding; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_attribute_registry_embedding ON "ob-poc".attribute_registry USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='100');


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
-- Name: idx_audit_instance; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_audit_instance ON "ob-poc".workflow_audit_log USING btree (instance_id, transitioned_at DESC);


--
-- Name: idx_bods_entity_company_num; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_bods_entity_company_num ON "ob-poc".bods_entity_statements USING btree (company_number) WHERE (company_number IS NOT NULL);


--
-- Name: idx_bods_entity_lei; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_bods_entity_lei ON "ob-poc".bods_entity_statements USING btree (lei) WHERE (lei IS NOT NULL);


--
-- Name: idx_bods_ownership_interested; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_bods_ownership_interested ON "ob-poc".bods_ownership_statements USING btree (interested_party_statement_id);


--
-- Name: idx_bods_ownership_subject; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_bods_ownership_subject ON "ob-poc".bods_ownership_statements USING btree (subject_entity_statement_id);


--
-- Name: idx_bods_ownership_subject_lei; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_bods_ownership_subject_lei ON "ob-poc".bods_ownership_statements USING btree (subject_lei) WHERE (subject_lei IS NOT NULL);


--
-- Name: idx_bods_person_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_bods_person_name ON "ob-poc".bods_person_statements USING gin (to_tsvector('english'::regconfig, full_name));


--
-- Name: idx_case_eval_snapshots_case_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_case_eval_snapshots_case_id ON "ob-poc".case_evaluation_snapshots USING btree (case_id);


--
-- Name: idx_case_eval_snapshots_evaluated_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_case_eval_snapshots_evaluated_at ON "ob-poc".case_evaluation_snapshots USING btree (evaluated_at DESC);


--
-- Name: idx_cbu_change_log_case_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_change_log_case_id ON "ob-poc".cbu_change_log USING btree (case_id);


--
-- Name: idx_cbu_change_log_cbu_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_change_log_cbu_id ON "ob-poc".cbu_change_log USING btree (cbu_id);


--
-- Name: idx_cbu_change_log_changed_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_change_log_changed_at ON "ob-poc".cbu_change_log USING btree (changed_at DESC);


--
-- Name: idx_cbu_change_log_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_change_log_type ON "ob-poc".cbu_change_log USING btree (change_type);


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
-- Name: idx_cbu_evidence_cbu_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_evidence_cbu_id ON "ob-poc".cbu_evidence USING btree (cbu_id);


--
-- Name: idx_cbu_evidence_document_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_evidence_document_id ON "ob-poc".cbu_evidence USING btree (document_id);


--
-- Name: idx_cbu_evidence_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_evidence_status ON "ob-poc".cbu_evidence USING btree (verification_status);


--
-- Name: idx_cbu_lifecycle_instances_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_lifecycle_instances_cbu ON "ob-poc".cbu_lifecycle_instances USING btree (cbu_id);


--
-- Name: idx_cbu_lifecycle_instances_counterparty; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_lifecycle_instances_counterparty ON "ob-poc".cbu_lifecycle_instances USING btree (counterparty_entity_id) WHERE (counterparty_entity_id IS NOT NULL);


--
-- Name: idx_cbu_lifecycle_instances_market; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_lifecycle_instances_market ON "ob-poc".cbu_lifecycle_instances USING btree (market_id) WHERE (market_id IS NOT NULL);


--
-- Name: idx_cbu_lifecycle_instances_resource; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_lifecycle_instances_resource ON "ob-poc".cbu_lifecycle_instances USING btree (resource_type_id);


--
-- Name: idx_cbu_lifecycle_instances_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_lifecycle_instances_status ON "ob-poc".cbu_lifecycle_instances USING btree (status);


--
-- Name: idx_cbu_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_name_trgm ON "ob-poc".cbus USING gin (name public.gin_trgm_ops);


--
-- Name: idx_cbu_product_subscriptions_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_product_subscriptions_cbu ON "ob-poc".cbu_product_subscriptions USING btree (cbu_id);


--
-- Name: idx_cbu_product_subscriptions_product; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_product_subscriptions_product ON "ob-poc".cbu_product_subscriptions USING btree (product_id);


--
-- Name: idx_cbu_rel_verif_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_rel_verif_cbu ON "ob-poc".cbu_relationship_verification USING btree (cbu_id);


--
-- Name: idx_cbu_rel_verif_rel; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_rel_verif_rel ON "ob-poc".cbu_relationship_verification USING btree (relationship_id);


--
-- Name: idx_cbu_rel_verif_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_rel_verif_status ON "ob-poc".cbu_relationship_verification USING btree (cbu_id, status);


--
-- Name: idx_cbu_resource_instances_counterparty; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_resource_instances_counterparty ON "ob-poc".cbu_resource_instances USING btree (counterparty_entity_id) WHERE (counterparty_entity_id IS NOT NULL);


--
-- Name: idx_cbu_resource_instances_currency; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_resource_instances_currency ON "ob-poc".cbu_resource_instances USING btree (currency) WHERE (currency IS NOT NULL);


--
-- Name: idx_cbu_resource_instances_lookup; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_resource_instances_lookup ON "ob-poc".cbu_resource_instances USING btree (cbu_id, resource_type_id, status);


--
-- Name: idx_cbu_resource_instances_market; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_resource_instances_market ON "ob-poc".cbu_resource_instances USING btree (market_id) WHERE (market_id IS NOT NULL);


--
-- Name: idx_cbu_sla_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_sla_active ON "ob-poc".cbu_sla_commitments USING btree (cbu_id) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_cbu_sla_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_sla_cbu ON "ob-poc".cbu_sla_commitments USING btree (cbu_id);


--
-- Name: idx_cbu_sla_profile; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_sla_profile ON "ob-poc".cbu_sla_commitments USING btree (profile_id);


--
-- Name: idx_cbu_sla_resource; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_sla_resource ON "ob-poc".cbu_sla_commitments USING btree (bound_resource_instance_id);


--
-- Name: idx_cbus_embedding; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_embedding ON "ob-poc".cbus USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='100');


--
-- Name: idx_cbus_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_name ON "ob-poc".cbus USING btree (name);


--
-- Name: idx_cbus_onboarding_context; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_onboarding_context ON "ob-poc".cbus USING gin (onboarding_context);


--
-- Name: idx_cbus_product_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_product_id ON "ob-poc".cbus USING btree (product_id);


--
-- Name: idx_cbus_risk_context; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_risk_context ON "ob-poc".cbus USING gin (risk_context);


--
-- Name: idx_cbus_semantic_context; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbus_semantic_context ON "ob-poc".cbus USING gin (semantic_context);


--
-- Name: idx_cer_history_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cer_history_cbu ON "ob-poc".cbu_entity_roles_history USING btree (cbu_id);


--
-- Name: idx_cer_history_changed_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cer_history_changed_at ON "ob-poc".cbu_entity_roles_history USING btree (changed_at);


--
-- Name: idx_cer_history_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cer_history_entity ON "ob-poc".cbu_entity_roles_history USING btree (entity_id);


--
-- Name: idx_companies_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_companies_name_trgm ON "ob-poc".entity_limited_companies USING gin (company_name public.gin_trgm_ops);


--
-- Name: idx_companies_reg_number; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_companies_reg_number ON "ob-poc".entity_limited_companies USING btree (registration_number);


--
-- Name: idx_cooperatives_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cooperatives_entity_id ON "ob-poc".entity_cooperatives USING btree (entity_id);


--
-- Name: idx_cooperatives_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cooperatives_jurisdiction ON "ob-poc".entity_cooperatives USING btree (jurisdiction);


--
-- Name: idx_cooperatives_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cooperatives_name_trgm ON "ob-poc".entity_cooperatives USING gin (cooperative_name public.gin_trgm_ops);


--
-- Name: idx_cri_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cri_cbu ON "ob-poc".cbu_resource_instances USING btree (cbu_id);


--
-- Name: idx_cri_resource_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cri_resource_type ON "ob-poc".cbu_resource_instances USING btree (resource_type_id);


--
-- Name: idx_cri_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cri_status ON "ob-poc".cbu_resource_instances USING btree (status);


--
-- Name: idx_cri_url; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cri_url ON "ob-poc".cbu_resource_instances USING btree (instance_url);


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
-- Name: idx_csg_rules_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_csg_rules_active ON "ob-poc".csg_validation_rules USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_csg_rules_params; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_csg_rules_params ON "ob-poc".csg_validation_rules USING gin (rule_params);


--
-- Name: idx_csg_rules_target; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_csg_rules_target ON "ob-poc".csg_validation_rules USING btree (target_type, target_code);


--
-- Name: idx_csg_rules_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_csg_rules_type ON "ob-poc".csg_validation_rules USING btree (rule_type);


--
-- Name: idx_currencies_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_currencies_active ON "ob-poc".currencies USING btree (is_active);


--
-- Name: idx_dal_attribute; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dal_attribute ON "ob-poc".document_attribute_links USING btree (attribute_id);


--
-- Name: idx_dal_document; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dal_document ON "ob-poc".document_attribute_links USING btree (document_type_id);


--
-- Name: idx_dal_sink; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dal_sink ON "ob-poc".document_attribute_links USING btree (attribute_id) WHERE ((direction)::text = ANY ((ARRAY['SINK'::character varying, 'BOTH'::character varying])::text[]));


--
-- Name: idx_dal_source; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dal_source ON "ob-poc".document_attribute_links USING btree (document_type_id) WHERE ((direction)::text = ANY ((ARRAY['SOURCE'::character varying, 'BOTH'::character varying])::text[]));


--
-- Name: idx_dam_attribute_uuid; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dam_attribute_uuid ON "ob-poc".document_attribute_mappings USING btree (attribute_uuid);


--
-- Name: idx_dam_document_type_attribute; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dam_document_type_attribute ON "ob-poc".document_attribute_mappings USING btree (document_type_id, attribute_uuid);


--
-- Name: idx_dam_document_type_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dam_document_type_id ON "ob-poc".document_attribute_mappings USING btree (document_type_id);


--
-- Name: idx_delegation_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_delegation_cbu ON "ob-poc".delegation_relationships USING btree (applies_to_cbu_id);


--
-- Name: idx_delegation_delegate; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_delegation_delegate ON "ob-poc".delegation_relationships USING btree (delegate_entity_id);


--
-- Name: idx_delegation_delegator; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_delegation_delegator ON "ob-poc".delegation_relationships USING btree (delegator_entity_id);


--
-- Name: idx_detected_patterns_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_detected_patterns_cbu ON "ob-poc".detected_patterns USING btree (cbu_id);


--
-- Name: idx_detected_patterns_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_detected_patterns_status ON "ob-poc".detected_patterns USING btree (status);


--
-- Name: idx_detected_patterns_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_detected_patterns_type ON "ob-poc".detected_patterns USING btree (pattern_type);


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
-- Name: idx_disc_case; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_disc_case ON "ob-poc".observation_discrepancies USING btree (case_id) WHERE (case_id IS NOT NULL);


--
-- Name: idx_disc_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_disc_entity ON "ob-poc".observation_discrepancies USING btree (entity_id);


--
-- Name: idx_disc_open; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_disc_open ON "ob-poc".observation_discrepancies USING btree (entity_id) WHERE ((resolution_status)::text = 'OPEN'::text);


--
-- Name: idx_disc_severity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_disc_severity ON "ob-poc".observation_discrepancies USING btree (severity) WHERE ((resolution_status)::text = 'OPEN'::text);


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
-- Name: idx_doc_validity_by_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_doc_validity_by_type ON "ob-poc".document_validity_rules USING btree (document_type_id);


--
-- Name: idx_document_catalog_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_catalog_cbu ON "ob-poc".document_catalog USING btree (cbu_id);


--
-- Name: idx_document_catalog_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_catalog_entity ON "ob-poc".document_catalog USING btree (entity_id);


--
-- Name: idx_document_catalog_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_catalog_type ON "ob-poc".document_catalog USING btree (document_type_id);


--
-- Name: idx_document_catalog_type_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_catalog_type_status ON "ob-poc".document_catalog USING btree (document_type_id, extraction_status);


--
-- Name: idx_document_types_applicability; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_types_applicability ON "ob-poc".document_types USING gin (applicability);


--
-- Name: idx_document_types_embedding; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_types_embedding ON "ob-poc".document_types USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='100');


--
-- Name: idx_document_types_semantic_context; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_document_types_semantic_context ON "ob-poc".document_types USING gin (semantic_context);


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
-- Name: idx_dsl_execution_verb_hashes; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_verb_hashes ON "ob-poc".dsl_execution_log USING gin (verb_hashes);


--
-- Name: idx_dsl_execution_version_phase; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_version_phase ON "ob-poc".dsl_execution_log USING btree (version_id, execution_phase);


--
-- Name: idx_dsl_idempotency_verb_hash; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_idempotency_verb_hash ON "ob-poc".dsl_idempotency USING btree (verb_hash) WHERE (verb_hash IS NOT NULL);


--
-- Name: idx_dsl_instance_versions_instance_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_instance_versions_instance_id ON "ob-poc".dsl_instance_versions USING btree (instance_id);


--
-- Name: idx_dsl_instance_versions_version_number; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_instance_versions_version_number ON "ob-poc".dsl_instance_versions USING btree (instance_id, version_number);


--
-- Name: idx_dsl_instances_business_reference; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_instances_business_reference ON "ob-poc".dsl_instances USING btree (business_reference);


--
-- Name: idx_dsl_instances_case_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_instances_case_id ON "ob-poc".dsl_instances USING btree (case_id);


--
-- Name: idx_dsl_ob_cbu_id_created_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_ob_cbu_id_created_at ON "ob-poc".dsl_ob USING btree (cbu_id, created_at DESC);


--
-- Name: idx_dsl_session_events_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_session_events_session ON "ob-poc".dsl_session_events USING btree (session_id);


--
-- Name: idx_dsl_sessions_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_sessions_cbu ON "ob-poc".dsl_sessions USING btree (cbu_id) WHERE (cbu_id IS NOT NULL);


--
-- Name: idx_dsl_sessions_expires; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_sessions_expires ON "ob-poc".dsl_sessions USING btree (expires_at) WHERE ((status)::text = 'active'::text);


--
-- Name: idx_dsl_sessions_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_sessions_status ON "ob-poc".dsl_sessions USING btree (status);


--
-- Name: idx_dsl_snapshots_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_snapshots_session ON "ob-poc".dsl_snapshots USING btree (session_id);


--
-- Name: idx_dsl_verbs_category; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_verbs_category ON "ob-poc".dsl_verbs USING btree (category);


--
-- Name: idx_dsl_verbs_compiler_version; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_verbs_compiler_version ON "ob-poc".dsl_verbs USING btree (compiler_version) WHERE (compiler_version IS NOT NULL);


--
-- Name: idx_dsl_verbs_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_verbs_domain ON "ob-poc".dsl_verbs USING btree (domain);


--
-- Name: idx_dsl_verbs_graph_ctx; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_verbs_graph_ctx ON "ob-poc".dsl_verbs USING gin (graph_contexts);


--
-- Name: idx_dsl_verbs_search; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_verbs_search ON "ob-poc".dsl_verbs USING gin (to_tsvector('english'::regconfig, COALESCE(search_text, ''::text)));


--
-- Name: idx_dsl_verbs_workflow; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_verbs_workflow ON "ob-poc".dsl_verbs USING gin (workflow_phases);


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
-- Name: idx_dsl_versions_unresolved; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_versions_unresolved ON "ob-poc".dsl_instance_versions USING btree (instance_id, unresolved_count) WHERE (unresolved_count > 0);


--
-- Name: idx_edge_types_service; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_edge_types_service ON "ob-poc".edge_types USING btree (show_in_service_view) WHERE (show_in_service_view = true);


--
-- Name: idx_edge_types_trading; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_edge_types_trading ON "ob-poc".edge_types USING btree (show_in_trading_view) WHERE (show_in_trading_view = true);


--
-- Name: idx_edge_types_ubo; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_edge_types_ubo ON "ob-poc".edge_types USING btree (show_in_ubo_view) WHERE (show_in_ubo_view = true);


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
-- Name: idx_entities_type_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_entities_type_name ON "ob-poc".entities USING btree (entity_type_id, name);


--
-- Name: idx_entity_addresses_country; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_addresses_country ON "ob-poc".entity_addresses USING btree (country);


--
-- Name: idx_entity_addresses_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_addresses_entity ON "ob-poc".entity_addresses USING btree (entity_id);


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
-- Name: idx_entity_deps_from; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_deps_from ON "ob-poc".entity_type_dependencies USING btree (from_type, from_subtype);


--
-- Name: idx_entity_deps_priority; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_deps_priority ON "ob-poc".entity_type_dependencies USING btree (from_type, from_subtype, priority) WHERE (is_active = true);


--
-- Name: idx_entity_deps_to; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_deps_to ON "ob-poc".entity_type_dependencies USING btree (to_type, to_subtype);


--
-- Name: idx_entity_events_date; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_events_date ON "ob-poc".entity_lifecycle_events USING btree (effective_date DESC);


--
-- Name: idx_entity_events_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_events_entity ON "ob-poc".entity_lifecycle_events USING btree (entity_id);


--
-- Name: idx_entity_events_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_events_type ON "ob-poc".entity_lifecycle_events USING btree (event_type);


--
-- Name: idx_entity_funds_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_funds_jurisdiction ON "ob-poc".entity_funds USING btree (jurisdiction);


--
-- Name: idx_entity_funds_lei; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_entity_funds_lei ON "ob-poc".entity_funds USING btree (lei) WHERE (lei IS NOT NULL);


--
-- Name: idx_entity_funds_master; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_funds_master ON "ob-poc".entity_funds USING btree (master_fund_id);


--
-- Name: idx_entity_funds_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_funds_parent ON "ob-poc".entity_funds USING btree (parent_fund_id);


--
-- Name: idx_entity_identifiers_lookup; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_identifiers_lookup ON "ob-poc".entity_identifiers USING btree (identifier_type, identifier_value);


--
-- Name: idx_entity_names_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_names_entity ON "ob-poc".entity_names USING btree (entity_id);


--
-- Name: idx_entity_names_search; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_names_search ON "ob-poc".entity_names USING gin (to_tsvector('english'::regconfig, name));


--
-- Name: idx_entity_parents_child; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_parents_child ON "ob-poc".entity_parent_relationships USING btree (child_entity_id);


--
-- Name: idx_entity_parents_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_parents_parent ON "ob-poc".entity_parent_relationships USING btree (parent_entity_id) WHERE (parent_entity_id IS NOT NULL);


--
-- Name: idx_entity_parents_parent_lei; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_parents_parent_lei ON "ob-poc".entity_parent_relationships USING btree (parent_lei) WHERE (parent_lei IS NOT NULL);


--
-- Name: idx_entity_parents_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_parents_type ON "ob-poc".entity_parent_relationships USING btree (relationship_type);


--
-- Name: idx_entity_reg_profile_regulated; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_reg_profile_regulated ON "ob-poc".entity_regulatory_profiles USING btree (is_regulated);


--
-- Name: idx_entity_reg_profile_regulator; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_reg_profile_regulator ON "ob-poc".entity_regulatory_profiles USING btree (regulator_code);


--
-- Name: idx_entity_rel_from; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_rel_from ON "ob-poc".entity_relationships USING btree (from_entity_id);


--
-- Name: idx_entity_rel_temporal; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_rel_temporal ON "ob-poc".entity_relationships USING btree (effective_from, effective_to);


--
-- Name: idx_entity_rel_to; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_rel_to ON "ob-poc".entity_relationships USING btree (to_entity_id);


--
-- Name: idx_entity_rel_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_rel_type ON "ob-poc".entity_relationships USING btree (relationship_type);


--
-- Name: idx_entity_rel_unique_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_entity_rel_unique_active ON "ob-poc".entity_relationships USING btree (from_entity_id, to_entity_id, relationship_type) WHERE (effective_to IS NULL);


--
-- Name: idx_entity_rel_unique_historical; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_entity_rel_unique_historical ON "ob-poc".entity_relationships USING btree (from_entity_id, to_entity_id, relationship_type, effective_to) WHERE (effective_to IS NOT NULL);


--
-- Name: idx_entity_types_embedding; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_embedding ON "ob-poc".entity_types USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='50');


--
-- Name: idx_entity_types_hierarchy; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_hierarchy ON "ob-poc".entity_types USING gin (type_hierarchy_path);


--
-- Name: idx_entity_types_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_name ON "ob-poc".entity_types USING btree (name);


--
-- Name: idx_entity_types_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_parent ON "ob-poc".entity_types USING btree (parent_type_id);


--
-- Name: idx_entity_types_semantic_context; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_semantic_context ON "ob-poc".entity_types USING gin (semantic_context);


--
-- Name: idx_entity_types_table; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_types_table ON "ob-poc".entity_types USING btree (table_name);


--
-- Name: idx_entity_types_type_code; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_entity_types_type_code ON "ob-poc".entity_types USING btree (type_code);


--
-- Name: idx_entity_ubos_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_ubos_entity ON "ob-poc".entity_ubos USING btree (entity_id);


--
-- Name: idx_entity_ubos_person; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_entity_ubos_person ON "ob-poc".entity_ubos USING btree (person_statement_id) WHERE (person_statement_id IS NOT NULL);


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
-- Name: idx_feedback_confidence; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_confidence ON "ob-poc".intent_feedback USING btree (match_confidence);


--
-- Name: idx_feedback_created; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_created ON "ob-poc".intent_feedback USING btree (created_at);


--
-- Name: idx_feedback_input_hash; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_input_hash ON "ob-poc".intent_feedback USING btree (user_input_hash);


--
-- Name: idx_feedback_outcome; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_outcome ON "ob-poc".intent_feedback USING btree (outcome) WHERE (outcome IS NOT NULL);


--
-- Name: idx_feedback_pending; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_pending ON "ob-poc".intent_feedback USING btree (interaction_id) WHERE (outcome IS NULL);


--
-- Name: idx_feedback_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_session ON "ob-poc".intent_feedback USING btree (session_id);


--
-- Name: idx_feedback_verb; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_feedback_verb ON "ob-poc".intent_feedback USING btree (matched_verb) WHERE (matched_verb IS NOT NULL);


--
-- Name: idx_foundations_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_foundations_entity_id ON "ob-poc".entity_foundations USING btree (entity_id);


--
-- Name: idx_foundations_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_foundations_jurisdiction ON "ob-poc".entity_foundations USING btree (jurisdiction);


--
-- Name: idx_foundations_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_foundations_name_trgm ON "ob-poc".entity_foundations USING gin (foundation_name public.gin_trgm_ops);


--
-- Name: idx_fund_investments_investee; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_investments_investee ON "ob-poc".fund_investments USING btree (investee_entity_id);


--
-- Name: idx_fund_investments_investor; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_investments_investor ON "ob-poc".fund_investments USING btree (investor_entity_id);


--
-- Name: idx_fund_investor_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_investor_entity ON "ob-poc".fund_investors USING btree (investor_entity_id);


--
-- Name: idx_fund_investor_fund; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_investor_fund ON "ob-poc".fund_investors USING btree (fund_cbu_id);


--
-- Name: idx_fund_investor_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_investor_status ON "ob-poc".fund_investors USING btree (kyc_status);


--
-- Name: idx_fund_structure_child; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_structure_child ON "ob-poc".fund_structure USING btree (child_entity_id);


--
-- Name: idx_fund_structure_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_fund_structure_parent ON "ob-poc".fund_structure USING btree (parent_entity_id);


--
-- Name: idx_gen_log_created; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_created ON "ob-poc".dsl_generation_log USING btree (created_at DESC);


--
-- Name: idx_gen_log_domain; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_domain ON "ob-poc".dsl_generation_log USING btree (domain_name);


--
-- Name: idx_gen_log_instance; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_instance ON "ob-poc".dsl_generation_log USING btree (instance_id) WHERE (instance_id IS NOT NULL);


--
-- Name: idx_gen_log_intent_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_intent_trgm ON "ob-poc".dsl_generation_log USING gin (user_intent public.gin_trgm_ops);


--
-- Name: idx_gen_log_iterations; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_iterations ON "ob-poc".dsl_generation_log USING gin (iterations);


--
-- Name: idx_gen_log_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_session ON "ob-poc".dsl_generation_log USING btree (session_id) WHERE (session_id IS NOT NULL);


--
-- Name: idx_gen_log_success; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gen_log_success ON "ob-poc".dsl_generation_log USING btree (success) WHERE (success = true);


--
-- Name: idx_gleif_sync_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gleif_sync_entity ON "ob-poc".gleif_sync_log USING btree (entity_id) WHERE (entity_id IS NOT NULL);


--
-- Name: idx_gleif_sync_lei; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_gleif_sync_lei ON "ob-poc".gleif_sync_log USING btree (lei) WHERE (lei IS NOT NULL);


--
-- Name: idx_government_country; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_government_country ON "ob-poc".entity_government USING btree (country_code);


--
-- Name: idx_government_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_government_entity_id ON "ob-poc".entity_government USING btree (entity_id);


--
-- Name: idx_government_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_government_name_trgm ON "ob-poc".entity_government USING gin (entity_name public.gin_trgm_ops);


--
-- Name: idx_government_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_government_type ON "ob-poc".entity_government USING btree (government_type);


--
-- Name: idx_idempotency_actor; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_idempotency_actor ON "ob-poc".dsl_idempotency USING btree (actor_id, actor_type) WHERE (actor_id IS NOT NULL);


--
-- Name: idx_idempotency_request_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_idempotency_request_id ON "ob-poc".dsl_idempotency USING btree (request_id) WHERE (request_id IS NOT NULL);


--
-- Name: idx_instrument_lifecycles_class; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_instrument_lifecycles_class ON "ob-poc".instrument_lifecycles USING btree (instrument_class_id);


--
-- Name: idx_instrument_lifecycles_lifecycle; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_instrument_lifecycles_lifecycle ON "ob-poc".instrument_lifecycles USING btree (lifecycle_id);


--
-- Name: idx_kyc_agreement_sponsor; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_kyc_agreement_sponsor ON "ob-poc".kyc_service_agreements USING btree (sponsor_cbu_id);


--
-- Name: idx_kyc_decisions_case; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_kyc_decisions_case ON "ob-poc".kyc_decisions USING btree (case_id);


--
-- Name: idx_kyc_decisions_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_kyc_decisions_cbu ON "ob-poc".kyc_decisions USING btree (cbu_id);


--
-- Name: idx_kyc_decisions_review; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_kyc_decisions_review ON "ob-poc".kyc_decisions USING btree (next_review_date) WHERE ((status)::text = 'CLEARED'::text);


--
-- Name: idx_kyc_decisions_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_kyc_decisions_status ON "ob-poc".kyc_decisions USING btree (status);


--
-- Name: idx_layout_cache_lookup; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_layout_cache_lookup ON "ob-poc".layout_cache USING btree (cbu_id, view_mode);


--
-- Name: idx_layout_cache_unique; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_layout_cache_unique ON "ob-poc".layout_cache USING btree (cbu_id, view_mode, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid));


--
-- Name: idx_lifecycle_resource_capabilities_lifecycle; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_lifecycle_resource_capabilities_lifecycle ON "ob-poc".lifecycle_resource_capabilities USING btree (lifecycle_id);


--
-- Name: idx_lifecycle_resource_capabilities_resource; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_lifecycle_resource_capabilities_resource ON "ob-poc".lifecycle_resource_capabilities USING btree (resource_type_id);


--
-- Name: idx_lifecycle_resource_types_owner; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_lifecycle_resource_types_owner ON "ob-poc".lifecycle_resource_types USING btree (owner);


--
-- Name: idx_lifecycle_resource_types_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_lifecycle_resource_types_type ON "ob-poc".lifecycle_resource_types USING btree (resource_type);


--
-- Name: idx_lifecycles_category; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_lifecycles_category ON "ob-poc".lifecycles USING btree (category);


--
-- Name: idx_lifecycles_owner; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_lifecycles_owner ON "ob-poc".lifecycles USING btree (owner);


--
-- Name: idx_limited_companies_direct_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_direct_parent ON "ob-poc".entity_limited_companies USING btree (direct_parent_lei) WHERE (direct_parent_lei IS NOT NULL);


--
-- Name: idx_limited_companies_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_entity_id ON "ob-poc".entity_limited_companies USING btree (entity_id);


--
-- Name: idx_limited_companies_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_jurisdiction ON "ob-poc".entity_limited_companies USING btree (jurisdiction);


--
-- Name: idx_limited_companies_lei; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_lei ON "ob-poc".entity_limited_companies USING btree (lei) WHERE (lei IS NOT NULL);


--
-- Name: idx_limited_companies_reg_num; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_reg_num ON "ob-poc".entity_limited_companies USING btree (registration_number);


--
-- Name: idx_limited_companies_ultimate_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_ultimate_parent ON "ob-poc".entity_limited_companies USING btree (ultimate_parent_lei) WHERE (ultimate_parent_lei IS NOT NULL);


--
-- Name: idx_market_csd_mappings_market; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_market_csd_mappings_market ON "ob-poc".market_csd_mappings USING btree (market_id);


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
-- Name: idx_materializations_profile; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_materializations_profile ON "ob-poc".trading_profile_materializations USING btree (profile_id);


--
-- Name: idx_matrix_overlay_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_matrix_overlay_cbu ON "ob-poc".cbu_matrix_product_overlay USING btree (cbu_id);


--
-- Name: idx_matrix_overlay_instrument; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_matrix_overlay_instrument ON "ob-poc".cbu_matrix_product_overlay USING btree (instrument_class_id);


--
-- Name: idx_matrix_overlay_subscription; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_matrix_overlay_subscription ON "ob-poc".cbu_matrix_product_overlay USING btree (subscription_id);


--
-- Name: idx_node_types_service; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_node_types_service ON "ob-poc".node_types USING btree (show_in_service_view) WHERE (show_in_service_view = true);


--
-- Name: idx_node_types_trading; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_node_types_trading ON "ob-poc".node_types USING btree (show_in_trading_view) WHERE (show_in_trading_view = true);


--
-- Name: idx_node_types_ubo; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_node_types_ubo ON "ob-poc".node_types USING btree (show_in_ubo_view) WHERE (show_in_ubo_view = true);


--
-- Name: idx_obs_attribute; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_obs_attribute ON "ob-poc".attribute_observations USING btree (attribute_id);


--
-- Name: idx_obs_entity_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_obs_entity_active ON "ob-poc".attribute_observations USING btree (entity_id) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_obs_entity_attr; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_obs_entity_attr ON "ob-poc".attribute_observations USING btree (entity_id, attribute_id);


--
-- Name: idx_obs_source_doc; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_obs_source_doc ON "ob-poc".attribute_observations USING btree (source_document_id) WHERE (source_document_id IS NOT NULL);


--
-- Name: idx_obs_source_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_obs_source_type ON "ob-poc".attribute_observations USING btree (source_type);


--
-- Name: idx_onboarding_executions_plan; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_executions_plan ON "ob-poc".onboarding_executions USING btree (plan_id);


--
-- Name: idx_onboarding_plans_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_plans_cbu ON "ob-poc".onboarding_plans USING btree (cbu_id);


--
-- Name: idx_onboarding_plans_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_plans_status ON "ob-poc".onboarding_plans USING btree (status);


--
-- Name: idx_onboarding_products_request; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_products_request ON "ob-poc".onboarding_products USING btree (request_id);


--
-- Name: idx_onboarding_request_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_request_cbu ON "ob-poc".onboarding_requests USING btree (cbu_id);


--
-- Name: idx_onboarding_request_state; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_request_state ON "ob-poc".onboarding_requests USING btree (request_state);


--
-- Name: idx_onboarding_tasks_exec; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_tasks_exec ON "ob-poc".onboarding_tasks USING btree (execution_id);


--
-- Name: idx_onboarding_tasks_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_onboarding_tasks_status ON "ob-poc".onboarding_tasks USING btree (status);


--
-- Name: idx_option_choices_def; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_option_choices_def ON "ob-poc".service_option_choices USING btree (option_def_id);


--
-- Name: idx_partnerships_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnerships_entity_id ON "ob-poc".entity_partnerships USING btree (entity_id);


--
-- Name: idx_partnerships_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnerships_jurisdiction ON "ob-poc".entity_partnerships USING btree (jurisdiction);


--
-- Name: idx_partnerships_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_partnerships_type ON "ob-poc".entity_partnerships USING btree (partnership_type);


--
-- Name: idx_persons_first_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_persons_first_name_trgm ON "ob-poc".entity_proper_persons USING gin (first_name public.gin_trgm_ops);


--
-- Name: idx_persons_last_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_persons_last_name_trgm ON "ob-poc".entity_proper_persons USING gin (last_name public.gin_trgm_ops);


--
-- Name: idx_persons_search_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_persons_search_name_trgm ON "ob-poc".entity_proper_persons USING gin (search_name public.gin_trgm_ops);


--
-- Name: idx_products_is_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_products_is_active ON "ob-poc".products USING btree (is_active);


--
-- Name: idx_products_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_products_name ON "ob-poc".products USING btree (name);


--
-- Name: idx_products_product_code; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_products_product_code ON "ob-poc".products USING btree (product_code);


--
-- Name: idx_proofs_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proofs_cbu ON "ob-poc".proofs USING btree (cbu_id);


--
-- Name: idx_proofs_document; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proofs_document ON "ob-poc".proofs USING btree (document_id) WHERE (document_id IS NOT NULL);


--
-- Name: idx_proofs_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proofs_status ON "ob-poc".proofs USING btree (cbu_id, status);


--
-- Name: idx_proper_persons_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_proper_persons_entity_id ON "ob-poc".entity_proper_persons USING btree (entity_id);


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
-- Name: idx_rel_history_changed_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rel_history_changed_at ON "ob-poc".entity_relationships_history USING btree (changed_at);


--
-- Name: idx_rel_history_from_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rel_history_from_entity ON "ob-poc".entity_relationships_history USING btree (from_entity_id);


--
-- Name: idx_rel_history_relationship_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rel_history_relationship_id ON "ob-poc".entity_relationships_history USING btree (relationship_id);


--
-- Name: idx_rel_history_temporal; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rel_history_temporal ON "ob-poc".entity_relationships_history USING btree (effective_from, effective_to);


--
-- Name: idx_rel_history_to_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rel_history_to_entity ON "ob-poc".entity_relationships_history USING btree (to_entity_id);


--
-- Name: idx_resource_deps_on; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_resource_deps_on ON "ob-poc".resource_dependencies USING btree (depends_on_type_id);


--
-- Name: idx_resource_deps_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_resource_deps_type ON "ob-poc".resource_dependencies USING btree (resource_type_id);


--
-- Name: idx_resource_requirements_resource; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_resource_requirements_resource ON "ob-poc".resource_attribute_requirements USING btree (resource_id);


--
-- Name: idx_ria_attribute; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ria_attribute ON "ob-poc".resource_instance_attributes USING btree (attribute_id);


--
-- Name: idx_ria_instance; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ria_instance ON "ob-poc".resource_instance_attributes USING btree (instance_id);


--
-- Name: idx_roles_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_roles_name ON "ob-poc".roles USING btree (name);


--
-- Name: idx_rps_instance; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rps_instance ON "ob-poc".resource_profile_sources USING btree (instance_id);


--
-- Name: idx_rps_profile; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_rps_profile ON "ob-poc".resource_profile_sources USING btree (profile_id);


--
-- Name: idx_screening_lists_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_screening_lists_type ON "ob-poc".screening_lists USING btree (list_type);


--
-- Name: idx_sdm_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sdm_cbu ON "ob-poc".service_delivery_map USING btree (cbu_id);


--
-- Name: idx_sdm_product; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sdm_product ON "ob-poc".service_delivery_map USING btree (product_id);


--
-- Name: idx_sdm_service; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sdm_service ON "ob-poc".service_delivery_map USING btree (service_id);


--
-- Name: idx_sdm_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sdm_status ON "ob-poc".service_delivery_map USING btree (delivery_status);


--
-- Name: idx_semantic_cache_accessed; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_semantic_cache_accessed ON "ob-poc".semantic_match_cache USING btree (last_accessed_at);


--
-- Name: idx_service_capabilities_resource; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_capabilities_resource ON "ob-poc".service_resource_capabilities USING btree (resource_id);


--
-- Name: idx_service_capabilities_service; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_capabilities_service ON "ob-poc".service_resource_capabilities USING btree (service_id);


--
-- Name: idx_service_options_service; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_options_service ON "ob-poc".service_option_definitions USING btree (service_id);


--
-- Name: idx_service_resource_types_dict_group; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_resource_types_dict_group ON "ob-poc".service_resource_types USING btree (dictionary_group);


--
-- Name: idx_service_resource_types_is_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_resource_types_is_active ON "ob-poc".service_resource_types USING btree (is_active);


--
-- Name: idx_service_resource_types_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_resource_types_name ON "ob-poc".service_resource_types USING btree (name);


--
-- Name: idx_service_resource_types_owner; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_resource_types_owner ON "ob-poc".service_resource_types USING btree (owner);


--
-- Name: idx_service_resource_types_resource_code; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_service_resource_types_resource_code ON "ob-poc".service_resource_types USING btree (resource_code);


--
-- Name: idx_services_is_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_services_is_active ON "ob-poc".services USING btree (is_active);


--
-- Name: idx_services_name; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_services_name ON "ob-poc".services USING btree (name);


--
-- Name: idx_services_service_code; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_services_service_code ON "ob-poc".services USING btree (service_code);


--
-- Name: idx_share_classes_parent; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_share_classes_parent ON "ob-poc".entity_share_classes USING btree (parent_fund_id);


--
-- Name: idx_sla_breach_commitment; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sla_breach_commitment ON "ob-poc".sla_breaches USING btree (commitment_id);


--
-- Name: idx_sla_breach_open; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sla_breach_open ON "ob-poc".sla_breaches USING btree (commitment_id) WHERE ((remediation_status)::text = ANY ((ARRAY['OPEN'::character varying, 'IN_PROGRESS'::character varying])::text[]));


--
-- Name: idx_sla_meas_breach; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sla_meas_breach ON "ob-poc".sla_measurements USING btree (commitment_id) WHERE ((status)::text = 'BREACH'::text);


--
-- Name: idx_sla_meas_commitment; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sla_meas_commitment ON "ob-poc".sla_measurements USING btree (commitment_id);


--
-- Name: idx_sla_meas_period; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sla_meas_period ON "ob-poc".sla_measurements USING btree (period_start, period_end);


--
-- Name: idx_sponsor_decision_case; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_sponsor_decision_case ON "ob-poc".kyc_case_sponsor_decisions USING btree (case_id);


--
-- Name: idx_taxonomy_crud_entity; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_taxonomy_crud_entity ON "ob-poc".taxonomy_crud_log USING btree (entity_type, entity_id);


--
-- Name: idx_taxonomy_crud_operation; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_taxonomy_crud_operation ON "ob-poc".taxonomy_crud_log USING btree (operation_type);


--
-- Name: idx_taxonomy_crud_time; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_taxonomy_crud_time ON "ob-poc".taxonomy_crud_log USING btree (created_at);


--
-- Name: idx_taxonomy_crud_user; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_taxonomy_crud_user ON "ob-poc".taxonomy_crud_log USING btree (user_id);


--
-- Name: idx_threshold_factors_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_threshold_factors_active ON "ob-poc".threshold_factors USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_threshold_factors_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_threshold_factors_type ON "ob-poc".threshold_factors USING btree (factor_type);


--
-- Name: idx_threshold_requirements_band; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_threshold_requirements_band ON "ob-poc".threshold_requirements USING btree (risk_band);


--
-- Name: idx_threshold_requirements_role; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_threshold_requirements_role ON "ob-poc".threshold_requirements USING btree (entity_role);


--
-- Name: idx_tpd_doc; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_tpd_doc ON "ob-poc".trading_profile_documents USING btree (doc_id);


--
-- Name: idx_tpd_profile; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_tpd_profile ON "ob-poc".trading_profile_documents USING btree (profile_id);


--
-- Name: idx_tpd_section; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_tpd_section ON "ob-poc".trading_profile_documents USING btree (profile_section);


--
-- Name: idx_trading_profiles_cbu_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trading_profiles_cbu_active ON "ob-poc".cbu_trading_profiles USING btree (cbu_id, status) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_trading_profiles_cbu_version; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trading_profiles_cbu_version ON "ob-poc".cbu_trading_profiles USING btree (cbu_id, version DESC);


--
-- Name: idx_trading_profiles_one_active; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_trading_profiles_one_active ON "ob-poc".cbu_trading_profiles USING btree (cbu_id) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_trading_profiles_one_working_version; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE UNIQUE INDEX idx_trading_profiles_one_working_version ON "ob-poc".cbu_trading_profiles USING btree (cbu_id) WHERE ((status)::text = ANY ((ARRAY['DRAFT'::character varying, 'VALIDATED'::character varying, 'PENDING_REVIEW'::character varying])::text[]));


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
-- Name: idx_trusts_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trusts_entity_id ON "ob-poc".entity_trusts USING btree (entity_id);


--
-- Name: idx_trusts_jurisdiction; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trusts_jurisdiction ON "ob-poc".entity_trusts USING btree (jurisdiction);


--
-- Name: idx_trusts_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trusts_name_trgm ON "ob-poc".entity_trusts USING gin (trust_name public.gin_trgm_ops);


--
-- Name: idx_trusts_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_trusts_type ON "ob-poc".entity_trusts USING btree (trust_type);


--
-- Name: idx_ubo_comparisons_baseline; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_comparisons_baseline ON "ob-poc".ubo_snapshot_comparisons USING btree (baseline_snapshot_id);


--
-- Name: idx_ubo_comparisons_cbu_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_comparisons_cbu_id ON "ob-poc".ubo_snapshot_comparisons USING btree (cbu_id);


--
-- Name: idx_ubo_comparisons_current; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_comparisons_current ON "ob-poc".ubo_snapshot_comparisons USING btree (current_snapshot_id);


--
-- Name: idx_ubo_evidence_document_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_evidence_document_id ON "ob-poc".ubo_evidence USING btree (document_id);


--
-- Name: idx_ubo_evidence_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_evidence_status ON "ob-poc".ubo_evidence USING btree (verification_status);


--
-- Name: idx_ubo_evidence_ubo_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_evidence_ubo_id ON "ob-poc".ubo_evidence USING btree (ubo_id);


--
-- Name: idx_ubo_registry_case_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_registry_case_id ON "ob-poc".ubo_registry USING btree (case_id);


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
-- Name: idx_ubo_registry_workstream_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_registry_workstream_id ON "ob-poc".ubo_registry USING btree (workstream_id);


--
-- Name: idx_ubo_snapshots_captured_at; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_snapshots_captured_at ON "ob-poc".ubo_snapshots USING btree (captured_at DESC);


--
-- Name: idx_ubo_snapshots_case_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_snapshots_case_id ON "ob-poc".ubo_snapshots USING btree (case_id);


--
-- Name: idx_ubo_snapshots_cbu_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ubo_snapshots_cbu_id ON "ob-poc".ubo_snapshots USING btree (cbu_id);


--
-- Name: idx_values_attr_uuid; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_values_attr_uuid ON "ob-poc".attribute_values_typed USING btree (attribute_uuid);


--
-- Name: idx_verb_pattern_agent_bound; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_pattern_agent_bound ON "ob-poc".verb_pattern_embeddings USING btree (is_agent_bound);


--
-- Name: idx_verb_pattern_category; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_pattern_category ON "ob-poc".verb_pattern_embeddings USING btree (category);


--
-- Name: idx_verb_pattern_embedding_ivfflat; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_pattern_embedding_ivfflat ON "ob-poc".verb_pattern_embeddings USING ivfflat (embedding public.vector_cosine_ops) WITH (lists='10');


--
-- Name: idx_verb_pattern_phonetic; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verb_pattern_phonetic ON "ob-poc".verb_pattern_embeddings USING gin (phonetic_codes);


--
-- Name: idx_verification_challenges_case; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verification_challenges_case ON "ob-poc".verification_challenges USING btree (case_id);


--
-- Name: idx_verification_challenges_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verification_challenges_cbu ON "ob-poc".verification_challenges USING btree (cbu_id);


--
-- Name: idx_verification_challenges_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verification_challenges_status ON "ob-poc".verification_challenges USING btree (status);


--
-- Name: idx_verification_escalations_cbu; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verification_escalations_cbu ON "ob-poc".verification_escalations USING btree (cbu_id);


--
-- Name: idx_verification_escalations_level; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verification_escalations_level ON "ob-poc".verification_escalations USING btree (escalation_level);


--
-- Name: idx_verification_escalations_status; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_verification_escalations_status ON "ob-poc".verification_escalations USING btree (status);


--
-- Name: idx_view_state_changes_created; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_view_state_changes_created ON "ob-poc".dsl_view_state_changes USING btree (created_at DESC);


--
-- Name: idx_view_state_changes_idempotency; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_view_state_changes_idempotency ON "ob-poc".dsl_view_state_changes USING btree (idempotency_key);


--
-- Name: idx_view_state_changes_selection_gin; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_view_state_changes_selection_gin ON "ob-poc".dsl_view_state_changes USING gin (selection);


--
-- Name: idx_view_state_changes_session; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_view_state_changes_session ON "ob-poc".dsl_view_state_changes USING btree (session_id);


--
-- Name: idx_view_state_changes_verb; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_view_state_changes_verb ON "ob-poc".dsl_view_state_changes USING btree (verb_name);


--
-- Name: idx_workflow_defs_loaded; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_workflow_defs_loaded ON "ob-poc".workflow_definitions USING btree (loaded_at DESC);


--
-- Name: idx_workflow_state; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_workflow_state ON "ob-poc".workflow_instances USING btree (workflow_id, current_state);


--
-- Name: idx_workflow_subject; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_workflow_subject ON "ob-poc".workflow_instances USING btree (subject_type, subject_id);


--
-- Name: idx_workflow_updated; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_workflow_updated ON "ob-poc".workflow_instances USING btree (updated_at DESC);


--
-- Name: ix_dsl_verbs_compiled_hash; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX ix_dsl_verbs_compiled_hash ON "ob-poc".dsl_verbs USING btree (compiled_hash) WHERE (compiled_hash IS NOT NULL);


--
-- Name: idx_ereg_entity; Type: INDEX; Schema: ob_kyc; Owner: -
--

CREATE INDEX idx_ereg_entity ON ob_kyc.entity_regulatory_registrations USING btree (entity_id);


--
-- Name: idx_ereg_expires; Type: INDEX; Schema: ob_kyc; Owner: -
--

CREATE INDEX idx_ereg_expires ON ob_kyc.entity_regulatory_registrations USING btree (verification_expires);


--
-- Name: idx_ereg_regulator; Type: INDEX; Schema: ob_kyc; Owner: -
--

CREATE INDEX idx_ereg_regulator ON ob_kyc.entity_regulatory_registrations USING btree (regulator_code);


--
-- Name: idx_ereg_status; Type: INDEX; Schema: ob_kyc; Owner: -
--

CREATE INDEX idx_ereg_status ON ob_kyc.entity_regulatory_registrations USING btree (status);


--
-- Name: idx_ereg_type; Type: INDEX; Schema: ob_kyc; Owner: -
--

CREATE INDEX idx_ereg_type ON ob_kyc.entity_regulatory_registrations USING btree (registration_type);


--
-- Name: idx_ereg_verified; Type: INDEX; Schema: ob_kyc; Owner: -
--

CREATE INDEX idx_ereg_verified ON ob_kyc.entity_regulatory_registrations USING btree (registration_verified);


--
-- Name: idx_ob_ref_regulators_jurisdiction; Type: INDEX; Schema: ob_ref; Owner: -
--

CREATE INDEX idx_ob_ref_regulators_jurisdiction ON ob_ref.regulators USING btree (jurisdiction);


--
-- Name: idx_ob_ref_regulators_tier; Type: INDEX; Schema: ob_ref; Owner: -
--

CREATE INDEX idx_ob_ref_regulators_tier ON ob_ref.regulators USING btree (regulatory_tier);


--
-- Name: idx_ob_ref_role_types_category; Type: INDEX; Schema: ob_ref; Owner: -
--

CREATE INDEX idx_ob_ref_role_types_category ON ob_ref.role_types USING btree (category);


--
-- Name: idx_ob_ref_role_types_code; Type: INDEX; Schema: ob_ref; Owner: -
--

CREATE INDEX idx_ob_ref_role_types_code ON ob_ref.role_types USING btree (code);


--
-- Name: idx_business_attrs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_business_attrs_entity ON public.business_attributes USING btree (entity_name);


--
-- Name: idx_credentials_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_credentials_active ON public.credentials_vault USING btree (active);


--
-- Name: idx_credentials_environment; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_credentials_environment ON public.credentials_vault USING btree (environment);


--
-- Name: idx_credentials_expires; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_credentials_expires ON public.credentials_vault USING btree (expires_at);


--
-- Name: idx_derived_attrs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_derived_attrs_entity ON public.derived_attributes USING btree (entity_name);


--
-- Name: idx_executions_rule; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_executions_rule ON public.rule_executions USING btree (rule_id);


--
-- Name: idx_executions_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_executions_time ON public.rule_executions USING btree (execution_time);


--
-- Name: idx_rule_deps_attr; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rule_deps_attr ON public.rule_dependencies USING btree (attribute_id);


--
-- Name: idx_rule_deps_rule; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rule_deps_rule ON public.rule_dependencies USING btree (rule_id);


--
-- Name: idx_rules_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rules_category ON public.rules USING btree (category_id);


--
-- Name: idx_rules_embedding; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rules_embedding ON public.rules USING hnsw (embedding public.vector_cosine_ops);


--
-- Name: idx_rules_search; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rules_search ON public.rules USING gin (search_vector);


--
-- Name: idx_rules_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rules_status ON public.rules USING btree (status);


--
-- Name: idx_rules_target; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rules_target ON public.rules USING btree (target_attribute_id);


--
-- Name: idx_attestations_attester; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_attestations_attester ON teams.access_attestations USING btree (attester_user_id);


--
-- Name: idx_attestations_campaign; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_attestations_campaign ON teams.access_attestations USING btree (campaign_id);


--
-- Name: idx_campaigns_deadline; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_campaigns_deadline ON teams.access_review_campaigns USING btree (deadline);


--
-- Name: idx_campaigns_status; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_campaigns_status ON teams.access_review_campaigns USING btree (status);


--
-- Name: idx_membership_active; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_active ON teams.memberships USING btree (effective_from, effective_to) WHERE (effective_to IS NULL);


--
-- Name: idx_membership_audit_team; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_audit_team ON teams.membership_audit_log USING btree (team_id);


--
-- Name: idx_membership_audit_user; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_audit_user ON teams.membership_audit_log USING btree (user_id);


--
-- Name: idx_membership_function; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_function ON teams.memberships USING btree (function_name);


--
-- Name: idx_membership_history_team; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_history_team ON teams.membership_history USING btree (team_id);


--
-- Name: idx_membership_history_user; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_history_user ON teams.membership_history USING btree (user_id);


--
-- Name: idx_membership_team; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_team ON teams.memberships USING btree (team_id);


--
-- Name: idx_membership_type; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_type ON teams.memberships USING btree (team_type);


--
-- Name: idx_membership_user; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_membership_user ON teams.memberships USING btree (user_id);


--
-- Name: idx_review_items_campaign; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_items_campaign ON teams.access_review_items USING btree (campaign_id);


--
-- Name: idx_review_items_flagged; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_items_flagged ON teams.access_review_items USING btree (campaign_id) WHERE (flag_no_legal_link OR flag_legal_expired OR flag_dormant_account);


--
-- Name: idx_review_items_membership; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_items_membership ON teams.access_review_items USING btree (membership_id);


--
-- Name: idx_review_items_pending; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_items_pending ON teams.access_review_items USING btree (campaign_id, status) WHERE ((status)::text = 'PENDING'::text);


--
-- Name: idx_review_items_reviewer; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_items_reviewer ON teams.access_review_items USING btree (reviewer_user_id);


--
-- Name: idx_review_items_status; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_items_status ON teams.access_review_items USING btree (status);


--
-- Name: idx_review_log_campaign; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_log_campaign ON teams.access_review_log USING btree (campaign_id);


--
-- Name: idx_review_log_item; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_log_item ON teams.access_review_log USING btree (item_id);


--
-- Name: idx_review_log_time; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_review_log_time ON teams.access_review_log USING btree (created_at);


--
-- Name: idx_teams_active; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_teams_active ON teams.teams USING btree (is_active) WHERE (is_active = true);


--
-- Name: idx_teams_entity; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_teams_entity ON teams.teams USING btree (delegating_entity_id);


--
-- Name: idx_teams_type; Type: INDEX; Schema: teams; Owner: -
--

CREATE INDEX idx_teams_type ON teams.teams USING btree (team_type);


--
-- Name: v_case_summary _RETURN; Type: RULE; Schema: kyc; Owner: -
--

CREATE OR REPLACE VIEW kyc.v_case_summary AS
 SELECT c.case_id,
    c.cbu_id,
    c.case_type,
    c.status,
    c.risk_rating,
    c.opened_at,
    c.sla_deadline,
    c.closed_at,
    count(DISTINCT w.workstream_id) AS workstream_count,
    count(DISTINCT w.workstream_id) FILTER (WHERE ((w.status)::text = 'COMPLETED'::text)) AS completed_workstreams,
    count(DISTINCT r.red_flag_id) FILTER (WHERE (r.resolved_at IS NULL)) AS open_red_flags,
    count(DISTINCT d.request_id) FILTER (WHERE ((d.status)::text = 'PENDING'::text)) AS pending_docs
   FROM (((kyc.cases c
     LEFT JOIN kyc.entity_workstreams w ON ((c.case_id = w.case_id)))
     LEFT JOIN kyc.red_flags r ON ((c.case_id = r.case_id)))
     LEFT JOIN kyc.doc_requests d ON ((w.workstream_id = d.workstream_id)))
  GROUP BY c.case_id;


--
-- Name: v_workstream_detail _RETURN; Type: RULE; Schema: kyc; Owner: -
--

CREATE OR REPLACE VIEW kyc.v_workstream_detail AS
 SELECT w.workstream_id,
    w.case_id,
    w.entity_id,
    e.name AS entity_name,
    et.name AS entity_type,
    w.status,
    w.risk_rating,
    w.discovery_depth,
    w.is_ubo,
    w.ownership_percentage,
    w.requires_enhanced_dd,
    w.started_at,
    w.completed_at,
    count(DISTINCT d.request_id) FILTER (WHERE ((d.status)::text = 'PENDING'::text)) AS pending_docs,
    count(DISTINCT s.screening_id) FILTER (WHERE ((s.status)::text = 'PENDING'::text)) AS pending_screenings,
    count(DISTINCT r.red_flag_id) FILTER (WHERE (r.resolved_at IS NULL)) AS open_flags
   FROM (((((kyc.entity_workstreams w
     JOIN "ob-poc".entities e ON ((w.entity_id = e.entity_id)))
     LEFT JOIN "ob-poc".entity_types et ON ((e.entity_type_id = et.entity_type_id)))
     LEFT JOIN kyc.doc_requests d ON ((w.workstream_id = d.workstream_id)))
     LEFT JOIN kyc.screenings s ON ((w.workstream_id = s.workstream_id)))
     LEFT JOIN kyc.red_flags r ON ((w.workstream_id = r.workstream_id)))
  GROUP BY w.workstream_id, e.name, et.name;


--
-- Name: cbu_instrument_universe sync_counterparty_key_trigger; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER sync_counterparty_key_trigger BEFORE INSERT OR UPDATE ON custody.cbu_instrument_universe FOR EACH ROW EXECUTE FUNCTION custody.sync_counterparty_key();


--
-- Name: cbu_cross_border_config update_cbu_cross_border_config_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_cbu_cross_border_config_updated_at BEFORE UPDATE ON custody.cbu_cross_border_config FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: cbu_settlement_chains update_cbu_settlement_chains_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_cbu_settlement_chains_updated_at BEFORE UPDATE ON custody.cbu_settlement_chains FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: cbu_settlement_location_preferences update_cbu_settlement_location_preferences_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_cbu_settlement_location_preferences_updated_at BEFORE UPDATE ON custody.cbu_settlement_location_preferences FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: cbu_tax_reclaim_config update_cbu_tax_reclaim_config_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_cbu_tax_reclaim_config_updated_at BEFORE UPDATE ON custody.cbu_tax_reclaim_config FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: cbu_tax_reporting update_cbu_tax_reporting_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_cbu_tax_reporting_updated_at BEFORE UPDATE ON custody.cbu_tax_reporting FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: cbu_tax_status update_cbu_tax_status_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_cbu_tax_status_updated_at BEFORE UPDATE ON custody.cbu_tax_status FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: settlement_chain_hops update_settlement_chain_hops_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_settlement_chain_hops_updated_at BEFORE UPDATE ON custody.settlement_chain_hops FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: settlement_locations update_settlement_locations_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_settlement_locations_updated_at BEFORE UPDATE ON custody.settlement_locations FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: tax_jurisdictions update_tax_jurisdictions_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_tax_jurisdictions_updated_at BEFORE UPDATE ON custody.tax_jurisdictions FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: tax_treaty_rates update_tax_treaty_rates_updated_at; Type: TRIGGER; Schema: custody; Owner: -
--

CREATE TRIGGER update_tax_treaty_rates_updated_at BEFORE UPDATE ON custody.tax_treaty_rates FOR EACH ROW EXECUTE FUNCTION custody.update_updated_at_column();


--
-- Name: outstanding_requests trg_outstanding_requests_updated; Type: TRIGGER; Schema: kyc; Owner: -
--

CREATE TRIGGER trg_outstanding_requests_updated BEFORE UPDATE ON kyc.outstanding_requests FOR EACH ROW EXECUTE FUNCTION kyc.update_outstanding_request_timestamp();


--
-- Name: entity_workstreams trg_workstream_blocked_days; Type: TRIGGER; Schema: kyc; Owner: -
--

CREATE TRIGGER trg_workstream_blocked_days BEFORE UPDATE ON kyc.entity_workstreams FOR EACH ROW EXECUTE FUNCTION kyc.update_workstream_blocked_days();


--
-- Name: cbu_product_subscriptions trg_auto_create_product_overlay; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_auto_create_product_overlay AFTER INSERT ON "ob-poc".cbu_product_subscriptions FOR EACH ROW WHEN (((new.status)::text = 'ACTIVE'::text)) EXECUTE FUNCTION "ob-poc".fn_auto_create_product_overlay();


--
-- Name: cbu_entity_roles trg_cbu_entity_roles_history; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_cbu_entity_roles_history BEFORE DELETE OR UPDATE ON "ob-poc".cbu_entity_roles FOR EACH ROW EXECUTE FUNCTION "ob-poc".cbu_entity_roles_history_trigger();


--
-- Name: cbus trg_cbu_status_change; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_cbu_status_change AFTER UPDATE ON "ob-poc".cbus FOR EACH ROW EXECUTE FUNCTION "ob-poc".log_cbu_status_change();


--
-- Name: cbu_resource_instances trg_cri_updated; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_cri_updated BEFORE UPDATE ON "ob-poc".cbu_resource_instances FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();


--
-- Name: entity_type_dependencies trg_entity_deps_updated_at; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_entity_deps_updated_at BEFORE UPDATE ON "ob-poc".entity_type_dependencies FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_entity_deps_timestamp();


--
-- Name: entity_relationships trg_entity_relationships_history; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_entity_relationships_history BEFORE DELETE OR UPDATE ON "ob-poc".entity_relationships FOR EACH ROW EXECUTE FUNCTION "ob-poc".entity_relationships_history_trigger();


--
-- Name: cbu_entity_roles trg_invalidate_cache_cbu_entity_roles; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_invalidate_cache_cbu_entity_roles AFTER INSERT OR DELETE OR UPDATE ON "ob-poc".cbu_entity_roles FOR EACH ROW EXECUTE FUNCTION "ob-poc".trigger_invalidate_layout_cache();


--
-- Name: entity_relationships trg_invalidate_cache_entity_relationships; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_invalidate_cache_entity_relationships AFTER INSERT OR DELETE OR UPDATE ON "ob-poc".entity_relationships FOR EACH ROW EXECUTE FUNCTION "ob-poc".trigger_invalidate_layout_cache();


--
-- Name: proofs trg_proofs_updated; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_proofs_updated BEFORE UPDATE ON "ob-poc".proofs FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_proofs_timestamp();


--
-- Name: service_delivery_map trg_sdm_updated; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_sdm_updated BEFORE UPDATE ON "ob-poc".service_delivery_map FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();


--
-- Name: cbus trg_sync_commercial_client; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_sync_commercial_client AFTER INSERT OR UPDATE OF commercial_client_entity_id ON "ob-poc".cbus FOR EACH ROW EXECUTE FUNCTION "ob-poc".sync_commercial_client_role();


--
-- Name: ubo_registry trg_ubo_status_transition; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_ubo_status_transition BEFORE UPDATE ON "ob-poc".ubo_registry FOR EACH ROW EXECUTE FUNCTION "ob-poc".validate_ubo_status_transition();


--
-- Name: dsl_verbs trg_verb_search_text; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_verb_search_text BEFORE INSERT OR UPDATE ON "ob-poc".dsl_verbs FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_verb_search_text();


--
-- Name: workflow_instances trg_workflow_updated; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_workflow_updated BEFORE UPDATE ON "ob-poc".workflow_instances FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_workflow_timestamp();


--
-- Name: dsl_versions trigger_invalidate_ast_cache; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trigger_invalidate_ast_cache AFTER UPDATE ON "ob-poc".dsl_versions FOR EACH ROW EXECUTE FUNCTION "ob-poc".invalidate_ast_cache();


--
-- Name: attribute_registry trigger_update_attribute_registry_timestamp; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trigger_update_attribute_registry_timestamp BEFORE UPDATE ON "ob-poc".attribute_registry FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_attribute_registry_timestamp();


--
-- Name: business_attributes update_business_attributes_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_business_attributes_updated_at BEFORE UPDATE ON public.business_attributes FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: derived_attributes update_derived_attributes_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_derived_attributes_updated_at BEFORE UPDATE ON public.derived_attributes FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: rules update_rules_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_rules_updated_at BEFORE UPDATE ON public.rules FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: memberships trg_membership_history; Type: TRIGGER; Schema: teams; Owner: -
--

CREATE TRIGGER trg_membership_history AFTER INSERT OR UPDATE ON teams.memberships FOR EACH ROW EXECUTE FUNCTION teams.log_membership_change();


--
-- Name: memberships trg_memberships_updated; Type: TRIGGER; Schema: teams; Owner: -
--

CREATE TRIGGER trg_memberships_updated BEFORE UPDATE ON teams.memberships FOR EACH ROW EXECUTE FUNCTION teams.update_timestamp();


--
-- Name: teams trg_teams_updated; Type: TRIGGER; Schema: teams; Owner: -
--

CREATE TRIGGER trg_teams_updated BEFORE UPDATE ON teams.teams FOR EACH ROW EXECUTE FUNCTION teams.update_timestamp();


--
-- Name: clients clients_employer_entity_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.clients
    ADD CONSTRAINT clients_employer_entity_id_fkey FOREIGN KEY (employer_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: commitments commitments_client_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.commitments
    ADD CONSTRAINT commitments_client_id_fkey FOREIGN KEY (client_id) REFERENCES client_portal.clients(client_id) ON DELETE CASCADE;


--
-- Name: credentials credentials_client_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.credentials
    ADD CONSTRAINT credentials_client_id_fkey FOREIGN KEY (client_id) REFERENCES client_portal.clients(client_id) ON DELETE CASCADE;


--
-- Name: escalations escalations_client_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.escalations
    ADD CONSTRAINT escalations_client_id_fkey FOREIGN KEY (client_id) REFERENCES client_portal.clients(client_id);


--
-- Name: escalations escalations_session_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.escalations
    ADD CONSTRAINT escalations_session_id_fkey FOREIGN KEY (session_id) REFERENCES client_portal.sessions(session_id);


--
-- Name: sessions sessions_client_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.sessions
    ADD CONSTRAINT sessions_client_id_fkey FOREIGN KEY (client_id) REFERENCES client_portal.clients(client_id) ON DELETE CASCADE;


--
-- Name: submissions submissions_client_id_fkey; Type: FK CONSTRAINT; Schema: client_portal; Owner: -
--

ALTER TABLE ONLY client_portal.submissions
    ADD CONSTRAINT submissions_client_id_fkey FOREIGN KEY (client_id) REFERENCES client_portal.clients(client_id);


--
-- Name: cbu_cash_sweep_config cbu_cash_sweep_config_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cash_sweep_config
    ADD CONSTRAINT cbu_cash_sweep_config_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_cash_sweep_config cbu_cash_sweep_config_profile_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cash_sweep_config
    ADD CONSTRAINT cbu_cash_sweep_config_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id);


--
-- Name: cbu_cash_sweep_config cbu_cash_sweep_config_sweep_resource_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cash_sweep_config
    ADD CONSTRAINT cbu_cash_sweep_config_sweep_resource_id_fkey FOREIGN KEY (sweep_resource_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: cbu_cross_border_config cbu_cross_border_config_bridge_location_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cross_border_config
    ADD CONSTRAINT cbu_cross_border_config_bridge_location_id_fkey FOREIGN KEY (bridge_location_id) REFERENCES custody.settlement_locations(location_id);


--
-- Name: cbu_cross_border_config cbu_cross_border_config_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cross_border_config
    ADD CONSTRAINT cbu_cross_border_config_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_cross_border_config cbu_cross_border_config_source_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cross_border_config
    ADD CONSTRAINT cbu_cross_border_config_source_market_id_fkey FOREIGN KEY (source_market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_cross_border_config cbu_cross_border_config_target_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_cross_border_config
    ADD CONSTRAINT cbu_cross_border_config_target_market_id_fkey FOREIGN KEY (target_market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_im_assignments cbu_im_assignments_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_im_assignments
    ADD CONSTRAINT cbu_im_assignments_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_im_assignments cbu_im_assignments_instruction_resource_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_im_assignments
    ADD CONSTRAINT cbu_im_assignments_instruction_resource_id_fkey FOREIGN KEY (instruction_resource_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: cbu_im_assignments cbu_im_assignments_manager_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_im_assignments
    ADD CONSTRAINT cbu_im_assignments_manager_entity_id_fkey FOREIGN KEY (manager_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_im_assignments cbu_im_assignments_profile_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_im_assignments
    ADD CONSTRAINT cbu_im_assignments_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id);


--
-- Name: cbu_instrument_universe cbu_instrument_universe_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_instrument_universe cbu_instrument_universe_counterparty_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_counterparty_entity_id_fkey FOREIGN KEY (counterparty_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_instrument_universe cbu_instrument_universe_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: cbu_instrument_universe cbu_instrument_universe_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_pricing_config cbu_pricing_config_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_pricing_config
    ADD CONSTRAINT cbu_pricing_config_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_pricing_config cbu_pricing_config_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_pricing_config
    ADD CONSTRAINT cbu_pricing_config_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: cbu_pricing_config cbu_pricing_config_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_pricing_config
    ADD CONSTRAINT cbu_pricing_config_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_pricing_config cbu_pricing_config_pricing_resource_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_pricing_config
    ADD CONSTRAINT cbu_pricing_config_pricing_resource_id_fkey FOREIGN KEY (pricing_resource_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: cbu_pricing_config cbu_pricing_config_profile_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_pricing_config
    ADD CONSTRAINT cbu_pricing_config_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id);


--
-- Name: cbu_settlement_chains cbu_settlement_chains_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_chains
    ADD CONSTRAINT cbu_settlement_chains_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_settlement_chains cbu_settlement_chains_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_chains
    ADD CONSTRAINT cbu_settlement_chains_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: cbu_settlement_chains cbu_settlement_chains_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_chains
    ADD CONSTRAINT cbu_settlement_chains_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_settlement_location_preferences cbu_settlement_location_preferences_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_location_preferences
    ADD CONSTRAINT cbu_settlement_location_preferences_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_settlement_location_preferences cbu_settlement_location_preferences_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_location_preferences
    ADD CONSTRAINT cbu_settlement_location_preferences_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: cbu_settlement_location_preferences cbu_settlement_location_preferences_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_location_preferences
    ADD CONSTRAINT cbu_settlement_location_preferences_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_settlement_location_preferences cbu_settlement_location_preferences_preferred_location_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_settlement_location_preferences
    ADD CONSTRAINT cbu_settlement_location_preferences_preferred_location_id_fkey FOREIGN KEY (preferred_location_id) REFERENCES custody.settlement_locations(location_id);


--
-- Name: cbu_ssi_agent_override cbu_ssi_agent_override_ssi_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi_agent_override
    ADD CONSTRAINT cbu_ssi_agent_override_ssi_id_fkey FOREIGN KEY (ssi_id) REFERENCES custody.cbu_ssi(ssi_id) ON DELETE CASCADE;


--
-- Name: cbu_ssi cbu_ssi_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi
    ADD CONSTRAINT cbu_ssi_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_ssi cbu_ssi_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi
    ADD CONSTRAINT cbu_ssi_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_tax_reclaim_config cbu_tax_reclaim_config_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reclaim_config
    ADD CONSTRAINT cbu_tax_reclaim_config_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_tax_reclaim_config cbu_tax_reclaim_config_service_provider_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reclaim_config
    ADD CONSTRAINT cbu_tax_reclaim_config_service_provider_entity_id_fkey FOREIGN KEY (service_provider_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_tax_reclaim_config cbu_tax_reclaim_config_source_jurisdiction_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reclaim_config
    ADD CONSTRAINT cbu_tax_reclaim_config_source_jurisdiction_id_fkey FOREIGN KEY (source_jurisdiction_id) REFERENCES custody.tax_jurisdictions(jurisdiction_id);


--
-- Name: cbu_tax_reporting cbu_tax_reporting_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reporting
    ADD CONSTRAINT cbu_tax_reporting_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_tax_reporting cbu_tax_reporting_reporting_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reporting
    ADD CONSTRAINT cbu_tax_reporting_reporting_entity_id_fkey FOREIGN KEY (reporting_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_tax_reporting cbu_tax_reporting_reporting_jurisdiction_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reporting
    ADD CONSTRAINT cbu_tax_reporting_reporting_jurisdiction_id_fkey FOREIGN KEY (reporting_jurisdiction_id) REFERENCES custody.tax_jurisdictions(jurisdiction_id);


--
-- Name: cbu_tax_reporting cbu_tax_reporting_sponsor_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_reporting
    ADD CONSTRAINT cbu_tax_reporting_sponsor_entity_id_fkey FOREIGN KEY (sponsor_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_tax_status cbu_tax_status_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_status
    ADD CONSTRAINT cbu_tax_status_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_tax_status cbu_tax_status_tax_jurisdiction_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_tax_status
    ADD CONSTRAINT cbu_tax_status_tax_jurisdiction_id_fkey FOREIGN KEY (tax_jurisdiction_id) REFERENCES custody.tax_jurisdictions(jurisdiction_id);


--
-- Name: cfi_codes cfi_codes_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cfi_codes
    ADD CONSTRAINT cfi_codes_class_id_fkey FOREIGN KEY (class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: cfi_codes cfi_codes_security_type_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cfi_codes
    ADD CONSTRAINT cfi_codes_security_type_id_fkey FOREIGN KEY (security_type_id) REFERENCES custody.security_types(security_type_id);


--
-- Name: csa_agreements csa_agreements_collateral_ssi_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.csa_agreements
    ADD CONSTRAINT csa_agreements_collateral_ssi_id_fkey FOREIGN KEY (collateral_ssi_id) REFERENCES custody.cbu_ssi(ssi_id);


--
-- Name: csa_agreements csa_agreements_isda_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.csa_agreements
    ADD CONSTRAINT csa_agreements_isda_id_fkey FOREIGN KEY (isda_id) REFERENCES custody.isda_agreements(isda_id) ON DELETE CASCADE;


--
-- Name: entity_settlement_identity entity_settlement_identity_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_settlement_identity
    ADD CONSTRAINT entity_settlement_identity_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_ssi entity_ssi_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_ssi
    ADD CONSTRAINT entity_ssi_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_ssi entity_ssi_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_ssi
    ADD CONSTRAINT entity_ssi_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: entity_ssi entity_ssi_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_ssi
    ADD CONSTRAINT entity_ssi_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: entity_ssi entity_ssi_security_type_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.entity_ssi
    ADD CONSTRAINT entity_ssi_security_type_id_fkey FOREIGN KEY (security_type_id) REFERENCES custody.security_types(security_type_id);


--
-- Name: instruction_paths instruction_paths_instruction_type_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_paths
    ADD CONSTRAINT instruction_paths_instruction_type_id_fkey FOREIGN KEY (instruction_type_id) REFERENCES custody.instruction_types(type_id);


--
-- Name: instruction_paths instruction_paths_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_paths
    ADD CONSTRAINT instruction_paths_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: instruction_paths instruction_paths_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_paths
    ADD CONSTRAINT instruction_paths_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: instruction_paths instruction_paths_resource_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instruction_paths
    ADD CONSTRAINT instruction_paths_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id);


--
-- Name: instrument_classes instrument_classes_parent_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.instrument_classes
    ADD CONSTRAINT instrument_classes_parent_class_id_fkey FOREIGN KEY (parent_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: isda_agreements isda_agreements_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_agreements
    ADD CONSTRAINT isda_agreements_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: isda_agreements isda_agreements_counterparty_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_agreements
    ADD CONSTRAINT isda_agreements_counterparty_entity_id_fkey FOREIGN KEY (counterparty_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: isda_product_coverage isda_product_coverage_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_coverage
    ADD CONSTRAINT isda_product_coverage_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: isda_product_coverage isda_product_coverage_isda_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_coverage
    ADD CONSTRAINT isda_product_coverage_isda_id_fkey FOREIGN KEY (isda_id) REFERENCES custody.isda_agreements(isda_id) ON DELETE CASCADE;


--
-- Name: isda_product_coverage isda_product_coverage_isda_taxonomy_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_coverage
    ADD CONSTRAINT isda_product_coverage_isda_taxonomy_id_fkey FOREIGN KEY (isda_taxonomy_id) REFERENCES custody.isda_product_taxonomy(taxonomy_id);


--
-- Name: isda_product_taxonomy isda_product_taxonomy_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.isda_product_taxonomy
    ADD CONSTRAINT isda_product_taxonomy_class_id_fkey FOREIGN KEY (class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: security_types security_types_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.security_types
    ADD CONSTRAINT security_types_class_id_fkey FOREIGN KEY (class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: settlement_chain_hops settlement_chain_hops_chain_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_chain_hops
    ADD CONSTRAINT settlement_chain_hops_chain_id_fkey FOREIGN KEY (chain_id) REFERENCES custody.cbu_settlement_chains(chain_id) ON DELETE CASCADE;


--
-- Name: settlement_chain_hops settlement_chain_hops_intermediary_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_chain_hops
    ADD CONSTRAINT settlement_chain_hops_intermediary_entity_id_fkey FOREIGN KEY (intermediary_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: settlement_chain_hops settlement_chain_hops_ssi_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.settlement_chain_hops
    ADD CONSTRAINT settlement_chain_hops_ssi_id_fkey FOREIGN KEY (ssi_id) REFERENCES custody.cbu_ssi(ssi_id);


--
-- Name: ssi_booking_rules ssi_booking_rules_cbu_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: ssi_booking_rules ssi_booking_rules_counterparty_entity_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_counterparty_entity_id_fkey FOREIGN KEY (counterparty_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: ssi_booking_rules ssi_booking_rules_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: ssi_booking_rules ssi_booking_rules_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: ssi_booking_rules ssi_booking_rules_security_type_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_security_type_id_fkey FOREIGN KEY (security_type_id) REFERENCES custody.security_types(security_type_id);


--
-- Name: ssi_booking_rules ssi_booking_rules_ssi_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_ssi_id_fkey FOREIGN KEY (ssi_id) REFERENCES custody.cbu_ssi(ssi_id) ON DELETE CASCADE;


--
-- Name: subcustodian_network subcustodian_network_market_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.subcustodian_network
    ADD CONSTRAINT subcustodian_network_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: tax_treaty_rates tax_treaty_rates_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_treaty_rates
    ADD CONSTRAINT tax_treaty_rates_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: tax_treaty_rates tax_treaty_rates_investor_jurisdiction_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_treaty_rates
    ADD CONSTRAINT tax_treaty_rates_investor_jurisdiction_id_fkey FOREIGN KEY (investor_jurisdiction_id) REFERENCES custody.tax_jurisdictions(jurisdiction_id);


--
-- Name: tax_treaty_rates tax_treaty_rates_source_jurisdiction_id_fkey; Type: FK CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.tax_treaty_rates
    ADD CONSTRAINT tax_treaty_rates_source_jurisdiction_id_fkey FOREIGN KEY (source_jurisdiction_id) REFERENCES custody.tax_jurisdictions(jurisdiction_id);


--
-- Name: approval_requests approval_requests_case_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.approval_requests
    ADD CONSTRAINT approval_requests_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: approval_requests approval_requests_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.approval_requests
    ADD CONSTRAINT approval_requests_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE;


--
-- Name: case_events case_events_case_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.case_events
    ADD CONSTRAINT case_events_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: case_events case_events_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.case_events
    ADD CONSTRAINT case_events_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE;


--
-- Name: cases cases_cbu_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.cases
    ADD CONSTRAINT cases_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cases cases_service_agreement_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.cases
    ADD CONSTRAINT cases_service_agreement_id_fkey FOREIGN KEY (service_agreement_id) REFERENCES "ob-poc".kyc_service_agreements(agreement_id);


--
-- Name: cases cases_sponsor_cbu_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.cases
    ADD CONSTRAINT cases_sponsor_cbu_id_fkey FOREIGN KEY (sponsor_cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cases cases_subject_entity_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.cases
    ADD CONSTRAINT cases_subject_entity_id_fkey FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: doc_request_acceptable_types doc_request_acceptable_types_document_type_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_request_acceptable_types
    ADD CONSTRAINT doc_request_acceptable_types_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id);


--
-- Name: doc_request_acceptable_types doc_request_acceptable_types_request_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_request_acceptable_types
    ADD CONSTRAINT doc_request_acceptable_types_request_id_fkey FOREIGN KEY (request_id) REFERENCES kyc.doc_requests(request_id) ON DELETE CASCADE;


--
-- Name: doc_requests doc_requests_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.doc_requests
    ADD CONSTRAINT doc_requests_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE;


--
-- Name: entity_workstreams entity_workstreams_blocker_request_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.entity_workstreams
    ADD CONSTRAINT entity_workstreams_blocker_request_id_fkey FOREIGN KEY (blocker_request_id) REFERENCES kyc.outstanding_requests(request_id);


--
-- Name: entity_workstreams entity_workstreams_case_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.entity_workstreams
    ADD CONSTRAINT entity_workstreams_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: entity_workstreams entity_workstreams_discovery_source_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.entity_workstreams
    ADD CONSTRAINT entity_workstreams_discovery_source_workstream_id_fkey FOREIGN KEY (discovery_source_workstream_id) REFERENCES kyc.entity_workstreams(workstream_id);


--
-- Name: entity_workstreams entity_workstreams_entity_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.entity_workstreams
    ADD CONSTRAINT entity_workstreams_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: holdings holdings_investor_entity_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.holdings
    ADD CONSTRAINT holdings_investor_entity_id_fkey FOREIGN KEY (investor_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: holdings holdings_share_class_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.holdings
    ADD CONSTRAINT holdings_share_class_id_fkey FOREIGN KEY (share_class_id) REFERENCES kyc.share_classes(id);


--
-- Name: movements movements_holding_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.movements
    ADD CONSTRAINT movements_holding_id_fkey FOREIGN KEY (holding_id) REFERENCES kyc.holdings(id);


--
-- Name: outstanding_requests outstanding_requests_case_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.outstanding_requests
    ADD CONSTRAINT outstanding_requests_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: outstanding_requests outstanding_requests_cbu_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.outstanding_requests
    ADD CONSTRAINT outstanding_requests_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: outstanding_requests outstanding_requests_entity_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.outstanding_requests
    ADD CONSTRAINT outstanding_requests_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: outstanding_requests outstanding_requests_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.outstanding_requests
    ADD CONSTRAINT outstanding_requests_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id);


--
-- Name: red_flags red_flags_case_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.red_flags
    ADD CONSTRAINT red_flags_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: red_flags red_flags_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.red_flags
    ADD CONSTRAINT red_flags_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE;


--
-- Name: rule_executions rule_executions_case_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.rule_executions
    ADD CONSTRAINT rule_executions_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: rule_executions rule_executions_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.rule_executions
    ADD CONSTRAINT rule_executions_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE;


--
-- Name: screenings screenings_red_flag_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.screenings
    ADD CONSTRAINT screenings_red_flag_id_fkey FOREIGN KEY (red_flag_id) REFERENCES kyc.red_flags(red_flag_id);


--
-- Name: screenings screenings_workstream_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.screenings
    ADD CONSTRAINT screenings_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE;


--
-- Name: share_classes share_classes_cbu_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.share_classes
    ADD CONSTRAINT share_classes_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: share_classes share_classes_entity_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.share_classes
    ADD CONSTRAINT share_classes_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: share_classes share_classes_issuer_entity_id_fkey; Type: FK CONSTRAINT; Schema: kyc; Owner: -
--

ALTER TABLE ONLY kyc.share_classes
    ADD CONSTRAINT share_classes_issuer_entity_id_fkey FOREIGN KEY (issuer_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: attribute_observations attribute_observations_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: attribute_observations attribute_observations_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: attribute_observations attribute_observations_source_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_source_document_id_fkey FOREIGN KEY (source_document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: attribute_observations attribute_observations_source_screening_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_source_screening_id_fkey FOREIGN KEY (source_screening_id) REFERENCES kyc.screenings(screening_id);


--
-- Name: attribute_observations attribute_observations_source_workstream_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_source_workstream_id_fkey FOREIGN KEY (source_workstream_id) REFERENCES kyc.entity_workstreams(workstream_id);


--
-- Name: attribute_observations attribute_observations_superseded_by_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_observations
    ADD CONSTRAINT attribute_observations_superseded_by_fkey FOREIGN KEY (superseded_by) REFERENCES "ob-poc".attribute_observations(observation_id);


--
-- Name: attribute_values_typed attribute_values_typed_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT attribute_values_typed_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(id);


--
-- Name: case_evaluation_snapshots case_evaluation_snapshots_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".case_evaluation_snapshots
    ADD CONSTRAINT case_evaluation_snapshots_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: case_evaluation_snapshots case_evaluation_snapshots_matched_threshold_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".case_evaluation_snapshots
    ADD CONSTRAINT case_evaluation_snapshots_matched_threshold_id_fkey FOREIGN KEY (matched_threshold_id) REFERENCES "ob-poc".case_decision_thresholds(threshold_id);


--
-- Name: cbu_change_log cbu_change_log_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_change_log
    ADD CONSTRAINT cbu_change_log_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


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
-- Name: cbu_entity_roles cbu_entity_roles_target_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_entity_roles
    ADD CONSTRAINT cbu_entity_roles_target_entity_id_fkey FOREIGN KEY (target_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_evidence cbu_evidence_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_evidence
    ADD CONSTRAINT cbu_evidence_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_evidence cbu_evidence_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_evidence
    ADD CONSTRAINT cbu_evidence_document_id_fkey FOREIGN KEY (document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: cbu_lifecycle_instances cbu_lifecycle_instances_cbu_fk; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_lifecycle_instances
    ADD CONSTRAINT cbu_lifecycle_instances_cbu_fk FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cbu_lifecycle_instances cbu_lifecycle_instances_resource_fk; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_lifecycle_instances
    ADD CONSTRAINT cbu_lifecycle_instances_resource_fk FOREIGN KEY (resource_type_id) REFERENCES "ob-poc".lifecycle_resource_types(resource_type_id);


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_counterparty_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_counterparty_entity_id_fkey FOREIGN KEY (counterparty_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_instrument_class_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_instrument_class_id_fkey FOREIGN KEY (instrument_class_id) REFERENCES custody.instrument_classes(class_id);


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_market_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_matrix_product_overlay cbu_matrix_product_overlay_subscription_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_matrix_product_overlay
    ADD CONSTRAINT cbu_matrix_product_overlay_subscription_id_fkey FOREIGN KEY (subscription_id) REFERENCES "ob-poc".cbu_product_subscriptions(subscription_id) ON DELETE CASCADE;


--
-- Name: cbu_product_subscriptions cbu_product_subscriptions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_product_subscriptions
    ADD CONSTRAINT cbu_product_subscriptions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_product_subscriptions cbu_product_subscriptions_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_product_subscriptions
    ADD CONSTRAINT cbu_product_subscriptions_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: cbu_relationship_verification cbu_relationship_verification_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_relationship_verification
    ADD CONSTRAINT cbu_relationship_verification_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cbu_relationship_verification cbu_relationship_verification_proof_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_relationship_verification
    ADD CONSTRAINT cbu_relationship_verification_proof_document_id_fkey FOREIGN KEY (proof_document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: cbu_relationship_verification cbu_relationship_verification_relationship_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_relationship_verification
    ADD CONSTRAINT cbu_relationship_verification_relationship_id_fkey FOREIGN KEY (relationship_id) REFERENCES "ob-poc".entity_relationships(relationship_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_counterparty_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_counterparty_entity_id_fkey FOREIGN KEY (counterparty_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_market_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_market_id_fkey FOREIGN KEY (market_id) REFERENCES custody.markets(market_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_resource_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_resource_type_id_fkey FOREIGN KEY (resource_type_id) REFERENCES "ob-poc".service_resource_types(resource_id);


--
-- Name: cbu_resource_instances cbu_resource_instances_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id);


--
-- Name: cbu_service_contexts cbu_service_contexts_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_service_contexts
    ADD CONSTRAINT cbu_service_contexts_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_sla_commitments cbu_sla_commitments_bound_resource_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_bound_resource_instance_id_fkey FOREIGN KEY (bound_resource_instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: cbu_sla_commitments cbu_sla_commitments_bound_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_bound_service_id_fkey FOREIGN KEY (bound_service_id) REFERENCES "ob-poc".services(service_id);


--
-- Name: cbu_sla_commitments cbu_sla_commitments_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_sla_commitments cbu_sla_commitments_profile_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id);


--
-- Name: cbu_sla_commitments cbu_sla_commitments_source_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_source_document_id_fkey FOREIGN KEY (source_document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: cbu_sla_commitments cbu_sla_commitments_template_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_sla_commitments
    ADD CONSTRAINT cbu_sla_commitments_template_id_fkey FOREIGN KEY (template_id) REFERENCES "ob-poc".sla_templates(template_id);


--
-- Name: cbu_trading_profiles cbu_trading_profiles_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_trading_profiles
    ADD CONSTRAINT cbu_trading_profiles_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: cbu_trading_profiles cbu_trading_profiles_source_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_trading_profiles
    ADD CONSTRAINT cbu_trading_profiles_source_document_id_fkey FOREIGN KEY (source_document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: cbus cbus_commercial_client_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_commercial_client_entity_id_fkey FOREIGN KEY (commercial_client_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: cbus cbus_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbus
    ADD CONSTRAINT cbus_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: client_allegations client_allegations_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: client_allegations client_allegations_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: client_allegations client_allegations_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: client_allegations client_allegations_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: client_allegations client_allegations_verified_by_observation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_verified_by_observation_id_fkey FOREIGN KEY (verified_by_observation_id) REFERENCES "ob-poc".attribute_observations(observation_id);


--
-- Name: client_allegations client_allegations_workstream_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".client_allegations
    ADD CONSTRAINT client_allegations_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id);


--
-- Name: crud_operations crud_operations_parent_operation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".crud_operations
    ADD CONSTRAINT crud_operations_parent_operation_id_fkey FOREIGN KEY (parent_operation_id) REFERENCES "ob-poc".crud_operations(operation_id);


--
-- Name: delegation_relationships delegation_relationships_applies_to_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".delegation_relationships
    ADD CONSTRAINT delegation_relationships_applies_to_cbu_id_fkey FOREIGN KEY (applies_to_cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: delegation_relationships delegation_relationships_contract_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".delegation_relationships
    ADD CONSTRAINT delegation_relationships_contract_doc_id_fkey FOREIGN KEY (contract_doc_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: delegation_relationships delegation_relationships_delegate_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".delegation_relationships
    ADD CONSTRAINT delegation_relationships_delegate_entity_id_fkey FOREIGN KEY (delegate_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: delegation_relationships delegation_relationships_delegator_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".delegation_relationships
    ADD CONSTRAINT delegation_relationships_delegator_entity_id_fkey FOREIGN KEY (delegator_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: detected_patterns detected_patterns_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".detected_patterns
    ADD CONSTRAINT detected_patterns_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: detected_patterns detected_patterns_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".detected_patterns
    ADD CONSTRAINT detected_patterns_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: document_attribute_links document_attribute_links_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_links
    ADD CONSTRAINT document_attribute_links_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: document_attribute_links document_attribute_links_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_attribute_links
    ADD CONSTRAINT document_attribute_links_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id);


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
-- Name: document_catalog document_catalog_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_catalog
    ADD CONSTRAINT document_catalog_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: document_validity_rules document_validity_rules_document_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".document_validity_rules
    ADD CONSTRAINT document_validity_rules_document_type_id_fkey FOREIGN KEY (document_type_id) REFERENCES "ob-poc".document_types(type_id);


--
-- Name: dsl_execution_log dsl_execution_log_version_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_execution_log
    ADD CONSTRAINT dsl_execution_log_version_id_fkey FOREIGN KEY (version_id) REFERENCES "ob-poc".dsl_versions(version_id) ON DELETE CASCADE;


--
-- Name: dsl_generation_log dsl_generation_log_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_generation_log
    ADD CONSTRAINT dsl_generation_log_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id);


--
-- Name: dsl_session_events dsl_session_events_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_session_events
    ADD CONSTRAINT dsl_session_events_session_id_fkey FOREIGN KEY (session_id) REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE CASCADE;


--
-- Name: dsl_session_locks dsl_session_locks_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_session_locks
    ADD CONSTRAINT dsl_session_locks_session_id_fkey FOREIGN KEY (session_id) REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE CASCADE;


--
-- Name: dsl_sessions dsl_sessions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_sessions
    ADD CONSTRAINT dsl_sessions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: dsl_sessions dsl_sessions_kyc_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_sessions
    ADD CONSTRAINT dsl_sessions_kyc_case_id_fkey FOREIGN KEY (kyc_case_id) REFERENCES kyc.cases(case_id);


--
-- Name: dsl_sessions dsl_sessions_onboarding_request_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_sessions
    ADD CONSTRAINT dsl_sessions_onboarding_request_id_fkey FOREIGN KEY (onboarding_request_id) REFERENCES "ob-poc".onboarding_requests(request_id);


--
-- Name: dsl_snapshots dsl_snapshots_session_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_snapshots
    ADD CONSTRAINT dsl_snapshots_session_id_fkey FOREIGN KEY (session_id) REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE CASCADE;


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
-- Name: entity_addresses entity_addresses_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_addresses
    ADD CONSTRAINT entity_addresses_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_bods_links entity_bods_links_bods_entity_statement_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_bods_links
    ADD CONSTRAINT entity_bods_links_bods_entity_statement_id_fkey FOREIGN KEY (bods_entity_statement_id) REFERENCES "ob-poc".bods_entity_statements(statement_id);


--
-- Name: entity_bods_links entity_bods_links_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_bods_links
    ADD CONSTRAINT entity_bods_links_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_cooperatives entity_cooperatives_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_cooperatives
    ADD CONSTRAINT entity_cooperatives_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_foundations entity_foundations_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_foundations
    ADD CONSTRAINT entity_foundations_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_funds entity_funds_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_funds
    ADD CONSTRAINT entity_funds_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_funds entity_funds_master_fund_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_funds
    ADD CONSTRAINT entity_funds_master_fund_id_fkey FOREIGN KEY (master_fund_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_funds entity_funds_parent_fund_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_funds
    ADD CONSTRAINT entity_funds_parent_fund_id_fkey FOREIGN KEY (parent_fund_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_government entity_government_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_government
    ADD CONSTRAINT entity_government_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_identifiers entity_identifiers_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_identifiers
    ADD CONSTRAINT entity_identifiers_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_lifecycle_events entity_lifecycle_events_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_lifecycle_events
    ADD CONSTRAINT entity_lifecycle_events_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_limited_companies entity_limited_companies_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_manco entity_manco_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_manco
    ADD CONSTRAINT entity_manco_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_names entity_names_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_names
    ADD CONSTRAINT entity_names_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_parent_relationships entity_parent_relationships_child_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_parent_relationships
    ADD CONSTRAINT entity_parent_relationships_child_entity_id_fkey FOREIGN KEY (child_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_parent_relationships entity_parent_relationships_parent_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_parent_relationships
    ADD CONSTRAINT entity_parent_relationships_parent_entity_id_fkey FOREIGN KEY (parent_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_partnerships entity_partnerships_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_partnerships
    ADD CONSTRAINT entity_partnerships_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_proper_persons entity_proper_persons_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_regulatory_profiles entity_regulatory_profiles_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_regulatory_profiles
    ADD CONSTRAINT entity_regulatory_profiles_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_regulatory_profiles entity_regulatory_profiles_regulator_code_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_regulatory_profiles
    ADD CONSTRAINT entity_regulatory_profiles_regulator_code_fkey FOREIGN KEY (regulator_code) REFERENCES "ob-poc".regulators(regulator_code);


--
-- Name: entity_regulatory_profiles entity_regulatory_profiles_regulatory_tier_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_regulatory_profiles
    ADD CONSTRAINT entity_regulatory_profiles_regulatory_tier_fkey FOREIGN KEY (regulatory_tier) REFERENCES "ob-poc".regulatory_tiers(tier_code);


--
-- Name: entity_relationships entity_relationships_from_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_relationships
    ADD CONSTRAINT entity_relationships_from_entity_id_fkey FOREIGN KEY (from_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_relationships entity_relationships_to_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_relationships
    ADD CONSTRAINT entity_relationships_to_entity_id_fkey FOREIGN KEY (to_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_share_classes entity_share_classes_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_share_classes
    ADD CONSTRAINT entity_share_classes_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_share_classes entity_share_classes_parent_fund_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_share_classes
    ADD CONSTRAINT entity_share_classes_parent_fund_id_fkey FOREIGN KEY (parent_fund_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: entity_trusts entity_trusts_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_trusts
    ADD CONSTRAINT entity_trusts_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_types entity_types_parent_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_types
    ADD CONSTRAINT entity_types_parent_type_id_fkey FOREIGN KEY (parent_type_id) REFERENCES "ob-poc".entity_types(entity_type_id);


--
-- Name: entity_ubos entity_ubos_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_ubos
    ADD CONSTRAINT entity_ubos_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: attribute_values_typed fk_attribute_uuid; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed
    ADD CONSTRAINT fk_attribute_uuid FOREIGN KEY (attribute_uuid) REFERENCES "ob-poc".attribute_registry(uuid);


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
-- Name: dsl_view_state_changes fk_idempotency; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_view_state_changes
    ADD CONSTRAINT fk_idempotency FOREIGN KEY (idempotency_key) REFERENCES "ob-poc".dsl_idempotency(idempotency_key) ON DELETE CASCADE;


--
-- Name: dsl_instance_versions fk_instance; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT fk_instance FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


--
-- Name: role_requirements fk_required_role; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_requirements
    ADD CONSTRAINT fk_required_role FOREIGN KEY (required_role) REFERENCES "ob-poc".roles(name);


--
-- Name: role_requirements fk_requiring_role; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_requirements
    ADD CONSTRAINT fk_requiring_role FOREIGN KEY (requiring_role) REFERENCES "ob-poc".roles(name);


--
-- Name: role_incompatibilities fk_role_a; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_incompatibilities
    ADD CONSTRAINT fk_role_a FOREIGN KEY (role_a) REFERENCES "ob-poc".roles(name);


--
-- Name: role_incompatibilities fk_role_b; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".role_incompatibilities
    ADD CONSTRAINT fk_role_b FOREIGN KEY (role_b) REFERENCES "ob-poc".roles(name);


--
-- Name: dsl_view_state_changes fk_session; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_view_state_changes
    ADD CONSTRAINT fk_session FOREIGN KEY (session_id) REFERENCES "ob-poc".dsl_sessions(session_id) ON DELETE SET NULL;


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
-- Name: fund_investments fund_investments_investee_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investments
    ADD CONSTRAINT fund_investments_investee_entity_id_fkey FOREIGN KEY (investee_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: fund_investments fund_investments_investor_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investments
    ADD CONSTRAINT fund_investments_investor_entity_id_fkey FOREIGN KEY (investor_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: fund_investors fund_investors_fund_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investors
    ADD CONSTRAINT fund_investors_fund_cbu_id_fkey FOREIGN KEY (fund_cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: fund_investors fund_investors_investor_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investors
    ADD CONSTRAINT fund_investors_investor_entity_id_fkey FOREIGN KEY (investor_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: fund_investors fund_investors_kyc_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_investors
    ADD CONSTRAINT fund_investors_kyc_case_id_fkey FOREIGN KEY (kyc_case_id) REFERENCES kyc.cases(case_id);


--
-- Name: fund_structure fund_structure_child_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_structure
    ADD CONSTRAINT fund_structure_child_entity_id_fkey FOREIGN KEY (child_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: fund_structure fund_structure_parent_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".fund_structure
    ADD CONSTRAINT fund_structure_parent_entity_id_fkey FOREIGN KEY (parent_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: gleif_sync_log gleif_sync_log_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".gleif_sync_log
    ADD CONSTRAINT gleif_sync_log_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: instrument_lifecycles instrument_lifecycles_lifecycle_fk; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".instrument_lifecycles
    ADD CONSTRAINT instrument_lifecycles_lifecycle_fk FOREIGN KEY (lifecycle_id) REFERENCES "ob-poc".lifecycles(lifecycle_id);


--
-- Name: kyc_case_sponsor_decisions kyc_case_sponsor_decisions_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_case_sponsor_decisions
    ADD CONSTRAINT kyc_case_sponsor_decisions_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id) ON DELETE CASCADE;


--
-- Name: kyc_decisions kyc_decisions_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_decisions
    ADD CONSTRAINT kyc_decisions_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: kyc_decisions kyc_decisions_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_decisions
    ADD CONSTRAINT kyc_decisions_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: kyc_service_agreements kyc_service_agreements_sponsor_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_service_agreements
    ADD CONSTRAINT kyc_service_agreements_sponsor_cbu_id_fkey FOREIGN KEY (sponsor_cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: kyc_service_agreements kyc_service_agreements_sponsor_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".kyc_service_agreements
    ADD CONSTRAINT kyc_service_agreements_sponsor_entity_id_fkey FOREIGN KEY (sponsor_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: lifecycle_resource_capabilities lifecycle_resource_capabilities_lifecycle_fk; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycle_resource_capabilities
    ADD CONSTRAINT lifecycle_resource_capabilities_lifecycle_fk FOREIGN KEY (lifecycle_id) REFERENCES "ob-poc".lifecycles(lifecycle_id);


--
-- Name: lifecycle_resource_capabilities lifecycle_resource_capabilities_resource_fk; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".lifecycle_resource_capabilities
    ADD CONSTRAINT lifecycle_resource_capabilities_resource_fk FOREIGN KEY (resource_type_id) REFERENCES "ob-poc".lifecycle_resource_types(resource_type_id);


--
-- Name: master_entity_xref master_entity_xref_jurisdiction_code_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".master_entity_xref
    ADD CONSTRAINT master_entity_xref_jurisdiction_code_fkey FOREIGN KEY (jurisdiction_code) REFERENCES "ob-poc".master_jurisdictions(jurisdiction_code);


--
-- Name: observation_discrepancies observation_discrepancies_accepted_observation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_accepted_observation_id_fkey FOREIGN KEY (accepted_observation_id) REFERENCES "ob-poc".attribute_observations(observation_id);


--
-- Name: observation_discrepancies observation_discrepancies_attribute_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: observation_discrepancies observation_discrepancies_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: observation_discrepancies observation_discrepancies_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: observation_discrepancies observation_discrepancies_observation_1_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_observation_1_id_fkey FOREIGN KEY (observation_1_id) REFERENCES "ob-poc".attribute_observations(observation_id);


--
-- Name: observation_discrepancies observation_discrepancies_observation_2_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_observation_2_id_fkey FOREIGN KEY (observation_2_id) REFERENCES "ob-poc".attribute_observations(observation_id);


--
-- Name: observation_discrepancies observation_discrepancies_red_flag_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_red_flag_id_fkey FOREIGN KEY (red_flag_id) REFERENCES kyc.red_flags(red_flag_id);


--
-- Name: observation_discrepancies observation_discrepancies_workstream_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id);


--
-- Name: onboarding_executions onboarding_executions_plan_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_executions
    ADD CONSTRAINT onboarding_executions_plan_id_fkey FOREIGN KEY (plan_id) REFERENCES "ob-poc".onboarding_plans(plan_id);


--
-- Name: onboarding_plans onboarding_plans_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_plans
    ADD CONSTRAINT onboarding_plans_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: onboarding_products onboarding_products_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: onboarding_products onboarding_products_request_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_products
    ADD CONSTRAINT onboarding_products_request_id_fkey FOREIGN KEY (request_id) REFERENCES "ob-poc".onboarding_requests(request_id) ON DELETE CASCADE;


--
-- Name: onboarding_requests onboarding_requests_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_requests
    ADD CONSTRAINT onboarding_requests_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: onboarding_tasks onboarding_tasks_execution_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_tasks
    ADD CONSTRAINT onboarding_tasks_execution_id_fkey FOREIGN KEY (execution_id) REFERENCES "ob-poc".onboarding_executions(execution_id);


--
-- Name: onboarding_tasks onboarding_tasks_resource_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".onboarding_tasks
    ADD CONSTRAINT onboarding_tasks_resource_instance_id_fkey FOREIGN KEY (resource_instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


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
-- Name: proofs proofs_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".proofs
    ADD CONSTRAINT proofs_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: proofs proofs_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".proofs
    ADD CONSTRAINT proofs_document_id_fkey FOREIGN KEY (document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: requirement_acceptable_docs requirement_acceptable_docs_document_type_code_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".requirement_acceptable_docs
    ADD CONSTRAINT requirement_acceptable_docs_document_type_code_fkey FOREIGN KEY (document_type_code) REFERENCES "ob-poc".document_types(type_code);


--
-- Name: requirement_acceptable_docs requirement_acceptable_docs_requirement_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".requirement_acceptable_docs
    ADD CONSTRAINT requirement_acceptable_docs_requirement_id_fkey FOREIGN KEY (requirement_id) REFERENCES "ob-poc".threshold_requirements(requirement_id) ON DELETE CASCADE;


--
-- Name: resource_attribute_requirements resource_attribute_requirements_attribute_uuid_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_attribute_uuid_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: resource_attribute_requirements resource_attribute_requirements_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE;


--
-- Name: resource_dependencies resource_dependencies_depends_on_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_dependencies
    ADD CONSTRAINT resource_dependencies_depends_on_type_id_fkey FOREIGN KEY (depends_on_type_id) REFERENCES "ob-poc".service_resource_types(resource_id);


--
-- Name: resource_dependencies resource_dependencies_resource_type_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_dependencies
    ADD CONSTRAINT resource_dependencies_resource_type_id_fkey FOREIGN KEY (resource_type_id) REFERENCES "ob-poc".service_resource_types(resource_id);


--
-- Name: resource_instance_attributes resource_instance_attributes_attribute_uuid_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_attribute_uuid_fkey FOREIGN KEY (attribute_id) REFERENCES "ob-poc".attribute_registry(uuid);


--
-- Name: resource_instance_attributes resource_instance_attributes_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_attributes
    ADD CONSTRAINT resource_instance_attributes_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE;


--
-- Name: resource_instance_dependencies resource_instance_dependencies_depends_on_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_dependencies
    ADD CONSTRAINT resource_instance_dependencies_depends_on_instance_id_fkey FOREIGN KEY (depends_on_instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: resource_instance_dependencies resource_instance_dependencies_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_instance_dependencies
    ADD CONSTRAINT resource_instance_dependencies_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: resource_profile_sources resource_profile_sources_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_profile_sources
    ADD CONSTRAINT resource_profile_sources_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE;


--
-- Name: resource_profile_sources resource_profile_sources_profile_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".resource_profile_sources
    ADD CONSTRAINT resource_profile_sources_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id) ON DELETE CASCADE;


--
-- Name: screening_requirements screening_requirements_risk_band_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".screening_requirements
    ADD CONSTRAINT screening_requirements_risk_band_fkey FOREIGN KEY (risk_band) REFERENCES "ob-poc".risk_bands(band_code);


--
-- Name: service_delivery_map service_delivery_map_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: service_delivery_map service_delivery_map_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".cbu_resource_instances(instance_id);


--
-- Name: service_delivery_map service_delivery_map_product_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_product_id_fkey FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id);


--
-- Name: service_delivery_map service_delivery_map_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_delivery_map
    ADD CONSTRAINT service_delivery_map_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id);


--
-- Name: service_option_choices service_option_choices_option_def_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_option_choices
    ADD CONSTRAINT service_option_choices_option_def_id_fkey FOREIGN KEY (option_def_id) REFERENCES "ob-poc".service_option_definitions(option_def_id) ON DELETE CASCADE;


--
-- Name: service_option_definitions service_option_definitions_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_option_definitions
    ADD CONSTRAINT service_option_definitions_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: service_resource_capabilities service_resource_capabilities_resource_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_resource_id_fkey FOREIGN KEY (resource_id) REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE;


--
-- Name: service_resource_capabilities service_resource_capabilities_service_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".service_resource_capabilities
    ADD CONSTRAINT service_resource_capabilities_service_id_fkey FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;


--
-- Name: sla_breaches sla_breaches_commitment_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_breaches
    ADD CONSTRAINT sla_breaches_commitment_id_fkey FOREIGN KEY (commitment_id) REFERENCES "ob-poc".cbu_sla_commitments(commitment_id);


--
-- Name: sla_breaches sla_breaches_measurement_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_breaches
    ADD CONSTRAINT sla_breaches_measurement_id_fkey FOREIGN KEY (measurement_id) REFERENCES "ob-poc".sla_measurements(measurement_id);


--
-- Name: sla_measurements sla_measurements_commitment_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_measurements
    ADD CONSTRAINT sla_measurements_commitment_id_fkey FOREIGN KEY (commitment_id) REFERENCES "ob-poc".cbu_sla_commitments(commitment_id) ON DELETE CASCADE;


--
-- Name: sla_templates sla_templates_metric_code_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".sla_templates
    ADD CONSTRAINT sla_templates_metric_code_fkey FOREIGN KEY (metric_code) REFERENCES "ob-poc".sla_metric_types(metric_code);


--
-- Name: threshold_requirements threshold_requirements_risk_band_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".threshold_requirements
    ADD CONSTRAINT threshold_requirements_risk_band_fkey FOREIGN KEY (risk_band) REFERENCES "ob-poc".risk_bands(band_code);


--
-- Name: trading_profile_documents trading_profile_documents_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trading_profile_documents
    ADD CONSTRAINT trading_profile_documents_doc_id_fkey FOREIGN KEY (doc_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: trading_profile_documents trading_profile_documents_profile_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trading_profile_documents
    ADD CONSTRAINT trading_profile_documents_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id) ON DELETE CASCADE;


--
-- Name: trading_profile_materializations trading_profile_materializations_profile_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".trading_profile_materializations
    ADD CONSTRAINT trading_profile_materializations_profile_id_fkey FOREIGN KEY (profile_id) REFERENCES "ob-poc".cbu_trading_profiles(profile_id) ON DELETE CASCADE;


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
-- Name: ubo_assertion_log ubo_assertion_log_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_assertion_log
    ADD CONSTRAINT ubo_assertion_log_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: ubo_assertion_log ubo_assertion_log_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_assertion_log
    ADD CONSTRAINT ubo_assertion_log_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: ubo_evidence ubo_evidence_document_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_evidence
    ADD CONSTRAINT ubo_evidence_document_id_fkey FOREIGN KEY (document_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: ubo_evidence ubo_evidence_ubo_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_evidence
    ADD CONSTRAINT ubo_evidence_ubo_id_fkey FOREIGN KEY (ubo_id) REFERENCES "ob-poc".ubo_registry(ubo_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: ubo_registry ubo_registry_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_replacement_ubo_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_replacement_ubo_id_fkey FOREIGN KEY (replacement_ubo_id) REFERENCES "ob-poc".ubo_registry(ubo_id);


--
-- Name: ubo_registry ubo_registry_subject_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_subject_entity_id_fkey FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_superseded_by_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_superseded_by_fkey FOREIGN KEY (superseded_by) REFERENCES "ob-poc".ubo_registry(ubo_id);


--
-- Name: ubo_registry ubo_registry_ubo_proper_person_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_ubo_proper_person_id_fkey FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: ubo_registry ubo_registry_workstream_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_registry
    ADD CONSTRAINT ubo_registry_workstream_id_fkey FOREIGN KEY (workstream_id) REFERENCES kyc.entity_workstreams(workstream_id);


--
-- Name: ubo_snapshot_comparisons ubo_snapshot_comparisons_baseline_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshot_comparisons
    ADD CONSTRAINT ubo_snapshot_comparisons_baseline_snapshot_id_fkey FOREIGN KEY (baseline_snapshot_id) REFERENCES "ob-poc".ubo_snapshots(snapshot_id);


--
-- Name: ubo_snapshot_comparisons ubo_snapshot_comparisons_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshot_comparisons
    ADD CONSTRAINT ubo_snapshot_comparisons_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: ubo_snapshot_comparisons ubo_snapshot_comparisons_current_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshot_comparisons
    ADD CONSTRAINT ubo_snapshot_comparisons_current_snapshot_id_fkey FOREIGN KEY (current_snapshot_id) REFERENCES "ob-poc".ubo_snapshots(snapshot_id);


--
-- Name: ubo_snapshots ubo_snapshots_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshots
    ADD CONSTRAINT ubo_snapshots_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: ubo_snapshots ubo_snapshots_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ubo_snapshots
    ADD CONSTRAINT ubo_snapshots_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: verification_challenges verification_challenges_allegation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_challenges
    ADD CONSTRAINT verification_challenges_allegation_id_fkey FOREIGN KEY (allegation_id) REFERENCES "ob-poc".client_allegations(allegation_id);


--
-- Name: verification_challenges verification_challenges_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_challenges
    ADD CONSTRAINT verification_challenges_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: verification_challenges verification_challenges_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_challenges
    ADD CONSTRAINT verification_challenges_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: verification_challenges verification_challenges_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_challenges
    ADD CONSTRAINT verification_challenges_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: verification_challenges verification_challenges_observation_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_challenges
    ADD CONSTRAINT verification_challenges_observation_id_fkey FOREIGN KEY (observation_id) REFERENCES "ob-poc".attribute_observations(observation_id);


--
-- Name: verification_escalations verification_escalations_case_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_escalations
    ADD CONSTRAINT verification_escalations_case_id_fkey FOREIGN KEY (case_id) REFERENCES kyc.cases(case_id);


--
-- Name: verification_escalations verification_escalations_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_escalations
    ADD CONSTRAINT verification_escalations_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: verification_escalations verification_escalations_challenge_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".verification_escalations
    ADD CONSTRAINT verification_escalations_challenge_id_fkey FOREIGN KEY (challenge_id) REFERENCES "ob-poc".verification_challenges(challenge_id);


--
-- Name: workflow_audit_log workflow_audit_log_instance_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".workflow_audit_log
    ADD CONSTRAINT workflow_audit_log_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES "ob-poc".workflow_instances(instance_id) ON DELETE CASCADE;


--
-- Name: entity_regulatory_registrations entity_regulatory_registrations_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob_kyc; Owner: -
--

ALTER TABLE ONLY ob_kyc.entity_regulatory_registrations
    ADD CONSTRAINT entity_regulatory_registrations_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


--
-- Name: entity_regulatory_registrations entity_regulatory_registrations_home_regulator_code_fkey; Type: FK CONSTRAINT; Schema: ob_kyc; Owner: -
--

ALTER TABLE ONLY ob_kyc.entity_regulatory_registrations
    ADD CONSTRAINT entity_regulatory_registrations_home_regulator_code_fkey FOREIGN KEY (home_regulator_code) REFERENCES ob_ref.regulators(regulator_code);


--
-- Name: entity_regulatory_registrations entity_regulatory_registrations_registration_type_fkey; Type: FK CONSTRAINT; Schema: ob_kyc; Owner: -
--

ALTER TABLE ONLY ob_kyc.entity_regulatory_registrations
    ADD CONSTRAINT entity_regulatory_registrations_registration_type_fkey FOREIGN KEY (registration_type) REFERENCES ob_ref.registration_types(registration_type);


--
-- Name: entity_regulatory_registrations entity_regulatory_registrations_regulator_code_fkey; Type: FK CONSTRAINT; Schema: ob_kyc; Owner: -
--

ALTER TABLE ONLY ob_kyc.entity_regulatory_registrations
    ADD CONSTRAINT entity_regulatory_registrations_regulator_code_fkey FOREIGN KEY (regulator_code) REFERENCES ob_ref.regulators(regulator_code);


--
-- Name: regulators regulators_regulatory_tier_fkey; Type: FK CONSTRAINT; Schema: ob_ref; Owner: -
--

ALTER TABLE ONLY ob_ref.regulators
    ADD CONSTRAINT regulators_regulatory_tier_fkey FOREIGN KEY (regulatory_tier) REFERENCES ob_ref.regulatory_tiers(tier_code);


--
-- Name: business_attributes business_attributes_domain_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES public.data_domains(id);


--
-- Name: business_attributes business_attributes_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.business_attributes
    ADD CONSTRAINT business_attributes_source_id_fkey FOREIGN KEY (source_id) REFERENCES public.attribute_sources(id);


--
-- Name: derived_attributes derived_attributes_domain_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.derived_attributes
    ADD CONSTRAINT derived_attributes_domain_id_fkey FOREIGN KEY (domain_id) REFERENCES public.data_domains(id);


--
-- Name: rule_dependencies rule_dependencies_attribute_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_attribute_id_fkey FOREIGN KEY (attribute_id) REFERENCES public.business_attributes(id);


--
-- Name: rule_dependencies rule_dependencies_rule_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_dependencies
    ADD CONSTRAINT rule_dependencies_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;


--
-- Name: rule_executions rule_executions_rule_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_executions
    ADD CONSTRAINT rule_executions_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id);


--
-- Name: rule_versions rule_versions_rule_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rule_versions
    ADD CONSTRAINT rule_versions_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;


--
-- Name: rules rules_category_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_category_id_fkey FOREIGN KEY (category_id) REFERENCES public.rule_categories(id);


--
-- Name: rules rules_target_attribute_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_target_attribute_id_fkey FOREIGN KEY (target_attribute_id) REFERENCES public.derived_attributes(id);


--
-- Name: access_attestations access_attestations_campaign_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_attestations
    ADD CONSTRAINT access_attestations_campaign_id_fkey FOREIGN KEY (campaign_id) REFERENCES teams.access_review_campaigns(campaign_id);


--
-- Name: access_review_items access_review_items_campaign_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_items
    ADD CONSTRAINT access_review_items_campaign_id_fkey FOREIGN KEY (campaign_id) REFERENCES teams.access_review_campaigns(campaign_id);


--
-- Name: access_review_items access_review_items_membership_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_items
    ADD CONSTRAINT access_review_items_membership_id_fkey FOREIGN KEY (membership_id) REFERENCES teams.memberships(membership_id);


--
-- Name: access_review_log access_review_log_campaign_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_log
    ADD CONSTRAINT access_review_log_campaign_id_fkey FOREIGN KEY (campaign_id) REFERENCES teams.access_review_campaigns(campaign_id);


--
-- Name: access_review_log access_review_log_item_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.access_review_log
    ADD CONSTRAINT access_review_log_item_id_fkey FOREIGN KEY (item_id) REFERENCES teams.access_review_items(item_id);


--
-- Name: membership_audit_log membership_audit_log_team_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.membership_audit_log
    ADD CONSTRAINT membership_audit_log_team_id_fkey FOREIGN KEY (team_id) REFERENCES teams.teams(team_id);


--
-- Name: memberships memberships_team_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.memberships
    ADD CONSTRAINT memberships_team_id_fkey FOREIGN KEY (team_id) REFERENCES teams.teams(team_id);


--
-- Name: memberships memberships_user_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.memberships
    ADD CONSTRAINT memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES client_portal.clients(client_id);


--
-- Name: team_cbu_access team_cbu_access_cbu_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_cbu_access
    ADD CONSTRAINT team_cbu_access_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


--
-- Name: team_cbu_access team_cbu_access_team_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_cbu_access
    ADD CONSTRAINT team_cbu_access_team_id_fkey FOREIGN KEY (team_id) REFERENCES teams.teams(team_id);


--
-- Name: team_service_entitlements team_service_entitlements_team_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.team_service_entitlements
    ADD CONSTRAINT team_service_entitlements_team_id_fkey FOREIGN KEY (team_id) REFERENCES teams.teams(team_id);


--
-- Name: teams teams_delegating_entity_id_fkey; Type: FK CONSTRAINT; Schema: teams; Owner: -
--

ALTER TABLE ONLY teams.teams
    ADD CONSTRAINT teams_delegating_entity_id_fkey FOREIGN KEY (delegating_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- PostgreSQL database dump complete
--

\unrestrict pf3wukrb5CpdDVMpSX0oXeGguoJxqUBr2OLDvKfrIllBTuOTtyT8Gbzalkv5wGV

