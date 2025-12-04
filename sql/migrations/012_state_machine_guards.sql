-- Migration: 012_state_machine_guards.sql
-- Description: Add new case/workstream statuses and transition guards
-- Phase 1 of KYC DSL Transition Plan

-- =============================================================================
-- PART 1: Add new case statuses (REFER_TO_REGULATOR, DO_NOT_ONBOARD)
-- =============================================================================

ALTER TABLE kyc.cases DROP CONSTRAINT IF EXISTS chk_case_status;
ALTER TABLE kyc.cases ADD CONSTRAINT chk_case_status CHECK (
    status IN (
        'INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW',
        'APPROVED', 'REJECTED', 'BLOCKED', 'WITHDRAWN', 'EXPIRED',
        'REFER_TO_REGULATOR', 'DO_NOT_ONBOARD'
    )
);

-- Update case_snapshots constraint to match
ALTER TABLE kyc.case_snapshots DROP CONSTRAINT IF EXISTS chk_snapshot_status;
ALTER TABLE kyc.case_snapshots ADD CONSTRAINT chk_snapshot_status CHECK (
    status IN (
        'INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW',
        'APPROVED', 'REJECTED', 'BLOCKED', 'WITHDRAWN', 'EXPIRED',
        'REFER_TO_REGULATOR', 'DO_NOT_ONBOARD'
    )
);

-- =============================================================================
-- PART 2: Add new workstream statuses (REFERRED, PROHIBITED)
-- =============================================================================

ALTER TABLE kyc.entity_workstreams DROP CONSTRAINT IF EXISTS chk_workstream_status;
ALTER TABLE kyc.entity_workstreams ADD CONSTRAINT chk_workstream_status CHECK (
    status IN (
        'PENDING', 'COLLECT', 'VERIFY', 'SCREEN', 'ASSESS', 'COMPLETE',
        'BLOCKED', 'ENHANCED_DD', 'REFERRED', 'PROHIBITED'
    )
);

-- =============================================================================
-- PART 3: Add DRAFT status to doc_requests
-- =============================================================================

ALTER TABLE kyc.doc_requests DROP CONSTRAINT IF EXISTS chk_doc_status;
ALTER TABLE kyc.doc_requests ADD CONSTRAINT chk_doc_status CHECK (
    status IN (
        'DRAFT', 'REQUIRED', 'REQUESTED', 'RECEIVED', 'UNDER_REVIEW',
        'VERIFIED', 'REJECTED', 'WAIVED', 'EXPIRED'
    )
);

-- =============================================================================
-- PART 4: Enhanced transition validation function for cases
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.is_valid_case_transition(
    p_from_status VARCHAR(30),
    p_to_status VARCHAR(30)
) RETURNS BOOLEAN AS $func$
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
$func$ LANGUAGE plpgsql IMMUTABLE;

-- =============================================================================
-- PART 5: Workstream transition validation function
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.is_valid_workstream_transition(
    p_from_status VARCHAR(30),
    p_to_status VARCHAR(30)
) RETURNS BOOLEAN AS $func$
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
$func$ LANGUAGE plpgsql IMMUTABLE;

-- =============================================================================
-- PART 6: Doc request transition validation function
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.is_valid_doc_request_transition(
    p_from_status VARCHAR(20),
    p_to_status VARCHAR(20)
) RETURNS BOOLEAN AS $func$
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
$func$ LANGUAGE plpgsql IMMUTABLE;

-- Grant permissions
GRANT EXECUTE ON FUNCTION kyc.is_valid_case_transition TO PUBLIC;
GRANT EXECUTE ON FUNCTION kyc.is_valid_workstream_transition TO PUBLIC;
GRANT EXECUTE ON FUNCTION kyc.is_valid_doc_request_transition TO PUBLIC;
