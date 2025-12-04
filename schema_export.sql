--
-- PostgreSQL database dump
--

\restrict B2XyN99cke6WN6KofWcvIeIcPj29PHpIsJMoepAJeN3XVheeY0nr5JxeC4SiG0Z

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
-- Name: public; Type: SCHEMA; Schema: -; Owner: -
--

-- *not* creating schema, since initdb creates it


--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON SCHEMA public IS 'Runtime API Endpoints System - Phase 1 Foundation';


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
-- Name: compute_ownership_chains(uuid, uuid, integer); Type: FUNCTION; Schema: ob-poc; Owner: -
--

CREATE FUNCTION "ob-poc".compute_ownership_chains(p_cbu_id uuid, p_target_entity_id uuid DEFAULT NULL::uuid, p_max_depth integer DEFAULT 10) RETURNS TABLE(chain_id integer, ubo_person_id uuid, ubo_name text, path_entities uuid[], path_names text[], ownership_percentages numeric[], effective_ownership numeric, chain_depth integer, is_complete boolean)
    LANGUAGE sql STABLE
    AS $$
WITH RECURSIVE ownership_chain AS (
    -- Base case: direct ownership from persons
    SELECT 
        ROW_NUMBER() OVER ()::INTEGER as chain_id,
        o.owner_entity_id as current_entity,
        o.owned_entity_id as target_entity,
        ARRAY[o.owner_entity_id] as path,
        ARRAY[COALESCE(e.name, 'Unknown')] as names,
        ARRAY[o.ownership_percent::NUMERIC] as percentages,
        o.ownership_percent::NUMERIC as effective_pct,
        1 as depth,
        et.type_code = 'proper_person' as owner_is_person
    FROM "ob-poc".ownership_relationships o
    JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
    WHERE cer.cbu_id = p_cbu_id
      AND (o.effective_to IS NULL OR o.effective_to > CURRENT_DATE)
      AND (p_target_entity_id IS NULL OR o.owned_entity_id = p_target_entity_id)
    
    UNION ALL
    
    -- Recursive case: follow ownership chain upward
    SELECT 
        oc.chain_id,
        o.owner_entity_id,
        oc.target_entity,
        oc.path || o.owner_entity_id,
        oc.names || COALESCE(e.name, 'Unknown'),
        oc.percentages || o.ownership_percent::NUMERIC,
        (oc.effective_pct * o.ownership_percent / 100)::NUMERIC,
        oc.depth + 1,
        et.type_code = 'proper_person'
    FROM ownership_chain oc
    JOIN "ob-poc".ownership_relationships o ON o.owned_entity_id = oc.current_entity
    JOIN "ob-poc".entities e ON o.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE (o.effective_to IS NULL OR o.effective_to > CURRENT_DATE)
      AND oc.depth < p_max_depth
      AND NOT o.owner_entity_id = ANY(oc.path)
)
SELECT 
    chain_id,
    current_entity as ubo_person_id,
    names[array_length(names, 1)] as ubo_name,
    path as path_entities,
    names as path_names,
    percentages as ownership_percentages,
    effective_pct as effective_ownership,
    depth as chain_depth,
    owner_is_person as is_complete
FROM ownership_chain
WHERE owner_is_person = true
ORDER BY effective_pct DESC, chain_id;
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


SET default_tablespace = '';

SET default_table_access_method = heap;

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
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE cbu_instrument_universe; Type: COMMENT; Schema: custody; Owner: -
--

COMMENT ON TABLE custody.cbu_instrument_universe IS 'Layer 1: Declares what instrument classes, markets, currencies a CBU trades. Drives SSI completeness checks.';


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
    CONSTRAINT chk_workstream_status CHECK (((status)::text = ANY ((ARRAY['PENDING'::character varying, 'COLLECT'::character varying, 'VERIFY'::character varying, 'SCREEN'::character varying, 'ASSESS'::character varying, 'COMPLETE'::character varying, 'BLOCKED'::character varying, 'ENHANCED_DD'::character varying, 'REFERRED'::character varying, 'PROHIBITED'::character varying])::text[])))
);


--
-- Name: TABLE entity_workstreams; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON TABLE kyc.entity_workstreams IS 'Per-entity work items within a KYC case';


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

COMMENT ON COLUMN kyc.share_classes.entity_id IS 'The legal entity that issues this share class';


--
-- Name: COLUMN share_classes.class_category; Type: COMMENT; Schema: kyc; Owner: -
--

COMMENT ON COLUMN kyc.share_classes.class_category IS 'CORPORATE = company ownership shares, FUND = investment fund shares';


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
    CONSTRAINT cbus_category_check CHECK (((cbu_category IS NULL) OR ((cbu_category)::text = ANY ((ARRAY['FUND_MANDATE'::character varying, 'CORPORATE_GROUP'::character varying, 'INSTITUTIONAL_ACCOUNT'::character varying, 'RETAIL_CLIENT'::character varying, 'FAMILY_TRUST'::character varying, 'CORRESPONDENT_BANK'::character varying])::text[])))),
    CONSTRAINT chk_cbu_category CHECK (((cbu_category IS NULL) OR ((cbu_category)::text = ANY ((ARRAY['FUND_MANDATE'::character varying, 'CORPORATE_GROUP'::character varying, 'INSTITUTIONAL_ACCOUNT'::character varying, 'RETAIL_CLIENT'::character varying, 'INTERNAL_TEST'::character varying, 'CORRESPONDENT_BANK'::character varying])::text[])))),
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

COMMENT ON COLUMN "ob-poc".cbus.cbu_category IS 'Template discriminator for visualization layout: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST, CORRESPONDENT_BANK';


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
    created_at timestamp with time zone DEFAULT now()
);


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
    embedding_updated_at timestamp with time zone
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
    created_at timestamp with time zone DEFAULT now()
);


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
    created_at timestamp with time zone DEFAULT now()
);


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
    CONSTRAINT dsl_session_events_event_type_check CHECK (((event_type)::text = ANY ((ARRAY['created'::character varying, 'execute_started'::character varying, 'execute_success'::character varying, 'execute_failed'::character varying, 'validation_error'::character varying, 'timeout'::character varying, 'aborted'::character varying, 'expired'::character varying, 'completed'::character varying, 'binding_added'::character varying, 'domain_detected'::character varying, 'error_recovered'::character varying])::text[])))
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
    CONSTRAINT dsl_sessions_status_check CHECK (((status)::text = ANY ((ARRAY['active'::character varying, 'completed'::character varying, 'aborted'::character varying, 'expired'::character varying, 'error'::character varying])::text[])))
);


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
    CONSTRAINT entity_crud_rules_constraint_type_check CHECK (((constraint_type)::text = ANY (ARRAY[('REQUIRED'::character varying)::text, ('UNIQUE'::character varying)::text, ('FOREIGN_KEY'::character varying)::text, ('VALIDATION'::character varying)::text, ('BUSINESS_RULE'::character varying)::text]))),
    CONSTRAINT entity_crud_rules_operation_type_check CHECK (((operation_type)::text = ANY (ARRAY[('CREATE'::character varying)::text, ('READ'::character varying)::text, ('UPDATE'::character varying)::text, ('DELETE'::character varying)::text])))
);


--
-- Name: TABLE entity_crud_rules; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".entity_crud_rules IS 'Entity-specific validation rules and constraints for CRUD operations';


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
    entity_id uuid
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
    entity_id uuid
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
  WHERE (entity_trusts.trust_id IS NOT NULL);


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
    embedding_updated_at timestamp with time zone
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
-- Name: jurisdictions; Type: VIEW; Schema: ob-poc; Owner: -
--

CREATE VIEW "ob-poc".jurisdictions AS
 SELECT jurisdiction_code AS iso_code,
    jurisdiction_name AS name,
    region,
    regulatory_framework AS description
   FROM "ob-poc".master_jurisdictions;


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
-- Name: ownership_relationships; Type: TABLE; Schema: ob-poc; Owner: -
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


--
-- Name: TABLE ownership_relationships; Type: COMMENT; Schema: ob-poc; Owner: -
--

COMMENT ON TABLE "ob-poc".ownership_relationships IS 'Tracks ownership relationships between entities for UBO analysis (OWNERSHIP crud_asset)';


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
    metadata jsonb
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
    CONSTRAINT service_resource_capabilities_performance_rating_check CHECK (((performance_rating >= 1) AND (performance_rating <= 5)))
);


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
    is_active boolean DEFAULT true
);


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

COMMENT ON TABLE "ob-poc".ubo_registry IS 'UBO identification results with proper entity referential integrity';


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
 WITH role_priorities AS (
         SELECT cer.cbu_id,
            cer.entity_id,
            e.name AS entity_name,
            et.type_code AS entity_type,
            COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) AS jurisdiction,
            r.name AS role_name,
                CASE r.name
                    WHEN 'ULTIMATE_BENEFICIAL_OWNER'::text THEN 100
                    WHEN 'BENEFICIAL_OWNER'::text THEN 95
                    WHEN 'SHAREHOLDER'::text THEN 90
                    WHEN 'LIMITED_PARTNER'::text THEN 85
                    WHEN 'MANAGEMENT_COMPANY'::text THEN 75
                    WHEN 'INVESTMENT_MANAGER'::text THEN 74
                    WHEN 'AIFM'::text THEN 73
                    WHEN 'SETTLOR'::text THEN 72
                    WHEN 'TRUSTEE'::text THEN 71
                    WHEN 'PROTECTOR'::text THEN 68
                    WHEN 'DIRECTOR'::text THEN 70
                    WHEN 'CONDUCTING_OFFICER'::text THEN 68
                    WHEN 'OFFICER'::text THEN 65
                    WHEN 'COMPANY_SECRETARY'::text THEN 60
                    WHEN 'AUTHORIZED_SIGNATORY'::text THEN 55
                    WHEN 'DEPOSITARY'::text THEN 50
                    WHEN 'CUSTODIAN'::text THEN 49
                    WHEN 'ADMINISTRATOR'::text THEN 45
                    WHEN 'FUND_ADMIN'::text THEN 44
                    WHEN 'TRANSFER_AGENT'::text THEN 43
                    WHEN 'AUDITOR'::text THEN 40
                    WHEN 'LEGAL_COUNSEL'::text THEN 35
                    WHEN 'PRIME_BROKER'::text THEN 38
                    WHEN 'BENEFICIARY'::text THEN 30
                    WHEN 'INVESTOR'::text THEN 25
                    WHEN 'SERVICE_PROVIDER'::text THEN 20
                    WHEN 'NOMINEE'::text THEN 15
                    WHEN 'RELATED_PARTY'::text THEN 10
                    WHEN 'PRINCIPAL'::text THEN 80
                    ELSE 5
                END AS role_priority
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
    jurisdiction,
    array_agg(role_name ORDER BY role_priority DESC) AS roles,
    (array_agg(role_name ORDER BY role_priority DESC))[1] AS primary_role,
    max(role_priority) AS max_role_priority
   FROM role_priorities
  GROUP BY cbu_id, entity_id, entity_name, entity_type, jurisdiction;


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
-- Name: attribute_values_typed id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".attribute_values_typed ALTER COLUMN id SET DEFAULT nextval('"ob-poc".attribute_values_typed_id_seq'::regclass);


--
-- Name: dsl_instances id; Type: DEFAULT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instances ALTER COLUMN id SET DEFAULT nextval('"ob-poc".dsl_instances_id_seq'::regclass);


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
-- Name: cbu_instrument_universe cbu_instrument_universe_cbu_id_instrument_class_id_market_i_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_cbu_id_instrument_class_id_market_i_key UNIQUE (cbu_id, instrument_class_id, market_id, counterparty_entity_id);


--
-- Name: cbu_instrument_universe cbu_instrument_universe_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_instrument_universe
    ADD CONSTRAINT cbu_instrument_universe_pkey PRIMARY KEY (universe_id);


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
-- Name: cbu_ssi cbu_ssi_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cbu_ssi
    ADD CONSTRAINT cbu_ssi_pkey PRIMARY KEY (ssi_id);


--
-- Name: cfi_codes cfi_codes_pkey; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.cfi_codes
    ADD CONSTRAINT cfi_codes_pkey PRIMARY KEY (cfi_code);


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
-- Name: ssi_booking_rules ssi_booking_rules_cbu_id_priority_rule_name_key; Type: CONSTRAINT; Schema: custody; Owner: -
--

ALTER TABLE ONLY custody.ssi_booking_rules
    ADD CONSTRAINT ssi_booking_rules_cbu_id_priority_rule_name_key UNIQUE (cbu_id, priority, rule_name);


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
-- Name: entity_proper_persons entity_proper_persons_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_proper_persons
    ADD CONSTRAINT entity_proper_persons_pkey PRIMARY KEY (proper_person_id);


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
-- Name: observation_discrepancies observation_discrepancies_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".observation_discrepancies
    ADD CONSTRAINT observation_discrepancies_pkey PRIMARY KEY (discrepancy_id);


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
-- Name: ownership_relationships ownership_relationships_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_pkey PRIMARY KEY (ownership_id);


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
-- Name: redflag_score_config redflag_score_config_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".redflag_score_config
    ADD CONSTRAINT redflag_score_config_pkey PRIMARY KEY (config_id);


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
-- Name: risk_bands risk_bands_pkey; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".risk_bands
    ADD CONSTRAINT risk_bands_pkey PRIMARY KEY (band_code);


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
-- Name: redflag_score_config uq_redflag_severity; Type: CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".redflag_score_config
    ADD CONSTRAINT uq_redflag_severity UNIQUE (severity);


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
-- Name: idx_booking_rules_lookup; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_booking_rules_lookup ON custody.ssi_booking_rules USING btree (cbu_id, is_active, priority, instrument_class_id, security_type_id, market_id, currency);


--
-- Name: idx_cbu_ssi_active; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_ssi_active ON custody.cbu_ssi USING btree (cbu_id, status) WHERE ((status)::text = 'ACTIVE'::text);


--
-- Name: idx_cbu_ssi_lookup; Type: INDEX; Schema: custody; Owner: -
--

CREATE INDEX idx_cbu_ssi_lookup ON custody.cbu_ssi USING btree (cbu_id, status);


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
-- Name: idx_cbu_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_cbu_name_trgm ON "ob-poc".cbus USING gin (name public.gin_trgm_ops);


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
-- Name: idx_companies_name_trgm; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_companies_name_trgm ON "ob-poc".entity_limited_companies USING gin (company_name public.gin_trgm_ops);


--
-- Name: idx_companies_reg_number; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_companies_reg_number ON "ob-poc".entity_limited_companies USING btree (registration_number);


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
-- Name: idx_dsl_execution_version_phase; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_dsl_execution_version_phase ON "ob-poc".dsl_execution_log USING btree (version_id, execution_phase);


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
-- Name: idx_limited_companies_entity_id; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_limited_companies_entity_id ON "ob-poc".entity_limited_companies USING btree (entity_id);


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
-- Name: idx_option_choices_def; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_option_choices_def ON "ob-poc".service_option_choices USING btree (option_def_id);


--
-- Name: idx_ownership_owned; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ownership_owned ON "ob-poc".ownership_relationships USING btree (owned_entity_id);


--
-- Name: idx_ownership_owner; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ownership_owner ON "ob-poc".ownership_relationships USING btree (owner_entity_id);


--
-- Name: idx_ownership_type; Type: INDEX; Schema: ob-poc; Owner: -
--

CREATE INDEX idx_ownership_type ON "ob-poc".ownership_relationships USING btree (ownership_type);


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
-- Name: cbus trg_cbu_status_change; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_cbu_status_change AFTER UPDATE ON "ob-poc".cbus FOR EACH ROW EXECUTE FUNCTION "ob-poc".log_cbu_status_change();


--
-- Name: cbu_resource_instances trg_cri_updated; Type: TRIGGER; Schema: ob-poc; Owner: -
--

CREATE TRIGGER trg_cri_updated BEFORE UPDATE ON "ob-poc".cbu_resource_instances FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_timestamp();


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
-- Name: cbu_resource_instances cbu_resource_instances_cbu_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".cbu_resource_instances
    ADD CONSTRAINT cbu_resource_instances_cbu_id_fkey FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id);


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
-- Name: entity_limited_companies entity_limited_companies_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".entity_limited_companies
    ADD CONSTRAINT entity_limited_companies_entity_id_fkey FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;


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
-- Name: dsl_instance_versions fk_instance; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".dsl_instance_versions
    ADD CONSTRAINT fk_instance FOREIGN KEY (instance_id) REFERENCES "ob-poc".dsl_instances(instance_id) ON DELETE CASCADE;


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
-- Name: ownership_relationships ownership_relationships_evidence_doc_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_evidence_doc_id_fkey FOREIGN KEY (evidence_doc_id) REFERENCES "ob-poc".document_catalog(doc_id);


--
-- Name: ownership_relationships ownership_relationships_owned_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_owned_entity_id_fkey FOREIGN KEY (owned_entity_id) REFERENCES "ob-poc".entities(entity_id);


--
-- Name: ownership_relationships ownership_relationships_owner_entity_id_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".ownership_relationships
    ADD CONSTRAINT ownership_relationships_owner_entity_id_fkey FOREIGN KEY (owner_entity_id) REFERENCES "ob-poc".entities(entity_id);


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
-- Name: threshold_requirements threshold_requirements_risk_band_fkey; Type: FK CONSTRAINT; Schema: ob-poc; Owner: -
--

ALTER TABLE ONLY "ob-poc".threshold_requirements
    ADD CONSTRAINT threshold_requirements_risk_band_fkey FOREIGN KEY (risk_band) REFERENCES "ob-poc".risk_bands(band_code);


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
-- PostgreSQL database dump complete
--

\unrestrict B2XyN99cke6WN6KofWcvIeIcPj29PHpIsJMoepAJeN3XVheeY0nr5JxeC4SiG0Z

