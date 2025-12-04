-- Migration: 010_rfi_extensions.sql
-- Description: Extensions for RFI (Request for Information) workflow
-- 
-- NOTE: The RFI concept works WITH existing kyc.doc_requests table,
-- NOT as a separate table structure. This migration adds supporting
-- functionality to generate doc_requests based on threshold requirements.

-- =============================================================================
-- PART 1: Add batch tracking to doc_requests
-- =============================================================================

-- Add batch_id to track which doc_requests were generated together
ALTER TABLE kyc.doc_requests 
    ADD COLUMN IF NOT EXISTS batch_id UUID,
    ADD COLUMN IF NOT EXISTS batch_reference VARCHAR(50),
    ADD COLUMN IF NOT EXISTS generation_source VARCHAR(30) DEFAULT 'MANUAL';

COMMENT ON COLUMN kyc.doc_requests.batch_id IS 'Groups doc_requests generated together';
COMMENT ON COLUMN kyc.doc_requests.batch_reference IS 'Human-readable batch reference (e.g., RFI-20241204-abc123)';
COMMENT ON COLUMN kyc.doc_requests.generation_source IS 'How request was created: MANUAL, THRESHOLD, PERIODIC_REVIEW';

-- Index for batch queries
CREATE INDEX IF NOT EXISTS idx_doc_requests_batch ON kyc.doc_requests(batch_id) WHERE batch_id IS NOT NULL;

-- =============================================================================
-- PART 2: Link doc_requests to acceptable document types from threshold
-- =============================================================================

-- Table to track which document types can satisfy a doc_request
CREATE TABLE IF NOT EXISTS kyc.doc_request_acceptable_types (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES kyc.doc_requests(request_id) ON DELETE CASCADE,
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(type_id),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(request_id, document_type_id)
);

COMMENT ON TABLE kyc.doc_request_acceptable_types IS 'Document types that can satisfy a doc_request';

CREATE INDEX IF NOT EXISTS idx_doc_request_types_request ON kyc.doc_request_acceptable_types(request_id);

-- =============================================================================
-- PART 3: Function to generate doc_requests from threshold requirements
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.generate_doc_requests_from_threshold(
    p_case_id UUID,
    p_batch_reference VARCHAR(50) DEFAULT NULL
)
RETURNS TABLE (
    batch_id UUID,
    requests_created INTEGER,
    entities_processed INTEGER
) AS $$
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
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.generate_doc_requests_from_threshold IS 
'Generates doc_requests based on threshold requirements for all workstreams in a case';

-- =============================================================================
-- PART 4: Function to check doc_request completion for a case
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.check_case_doc_completion(p_case_id UUID)
RETURNS TABLE (
    total_requests INTEGER,
    pending_requests INTEGER,
    received_requests INTEGER,
    verified_requests INTEGER,
    mandatory_pending INTEGER,
    all_mandatory_complete BOOLEAN
) AS $$
SELECT 
    COUNT(*)::INTEGER as total_requests,
    COUNT(*) FILTER (WHERE status IN ('REQUIRED', 'REQUESTED'))::INTEGER as pending_requests,
    COUNT(*) FILTER (WHERE status = 'RECEIVED')::INTEGER as received_requests,
    COUNT(*) FILTER (WHERE status = 'VERIFIED')::INTEGER as verified_requests,
    COUNT(*) FILTER (WHERE status IN ('REQUIRED', 'REQUESTED') AND is_mandatory)::INTEGER as mandatory_pending,
    NOT EXISTS (
        SELECT 1 FROM kyc.doc_requests dr
        JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
        WHERE w.case_id = p_case_id
        AND dr.is_mandatory = true
        AND dr.status NOT IN ('VERIFIED', 'WAIVED')
    ) as all_mandatory_complete
FROM kyc.doc_requests dr
JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
WHERE w.case_id = p_case_id;
$$ LANGUAGE SQL STABLE;

-- Grant permissions
GRANT SELECT, INSERT, UPDATE ON kyc.doc_request_acceptable_types TO PUBLIC;
GRANT EXECUTE ON FUNCTION kyc.generate_doc_requests_from_threshold TO PUBLIC;
GRANT EXECUTE ON FUNCTION kyc.check_case_doc_completion TO PUBLIC;
