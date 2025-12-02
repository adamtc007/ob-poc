-- Migration: KYC Case Transition Functions and Views

-- Current state view
CREATE OR REPLACE VIEW kyc.v_case_current AS
SELECT 
    c.case_id, c.cbu_id, c.opened_at, c.closed_at,
    c.sla_deadline, c.last_activity_at, c.case_type, c.notes,
    COALESCE(s.status, c.status) as status,
    COALESCE(s.escalation_level, c.escalation_level) as escalation_level,
    COALESCE(s.risk_rating, c.risk_rating) as risk_rating,
    COALESCE(s.assigned_analyst_id, c.assigned_analyst_id) as assigned_analyst_id,
    COALESCE(s.assigned_reviewer_id, c.assigned_reviewer_id) as assigned_reviewer_id,
    COALESCE(s.version, 0) as version,
    s.created_at AS state_changed_at,
    s.snapshot_reason AS last_change_reason
FROM kyc.cases c
LEFT JOIN kyc.case_snapshots s ON s.case_id = c.case_id AND s.is_current = true;

-- Transition validation function
CREATE OR REPLACE FUNCTION kyc.is_valid_case_transition(
    p_from_status VARCHAR(30),
    p_to_status VARCHAR(30)
) RETURNS BOOLEAN AS $f$
BEGIN
    IF p_from_status = p_to_status THEN RETURN true; END IF;
    
    RETURN CASE p_from_status
        WHEN 'INTAKE' THEN p_to_status IN ('DISCOVERY', 'WITHDRAWN')
        WHEN 'DISCOVERY' THEN p_to_status IN ('ASSESSMENT', 'BLOCKED', 'WITHDRAWN')
        WHEN 'ASSESSMENT' THEN p_to_status IN ('REVIEW', 'BLOCKED', 'WITHDRAWN')
        WHEN 'REVIEW' THEN p_to_status IN ('APPROVED', 'REJECTED', 'BLOCKED')
        WHEN 'BLOCKED' THEN p_to_status IN ('DISCOVERY', 'ASSESSMENT', 'REVIEW', 'WITHDRAWN')
        ELSE false
    END;
END;
$f$ LANGUAGE plpgsql IMMUTABLE;

-- Transition function (creates snapshot)
CREATE OR REPLACE FUNCTION kyc.transition_case_status(
    p_case_id UUID,
    p_new_status VARCHAR(30),
    p_reason TEXT DEFAULT NULL,
    p_actor TEXT DEFAULT 'SYSTEM'
) RETURNS UUID AS $f$
DECLARE
    v_current_status VARCHAR(30);
    v_current_version INTEGER;
    v_snapshot_id UUID;
    v_escalation VARCHAR(30);
    v_risk VARCHAR(20);
    v_analyst UUID;
    v_reviewer UUID;
BEGIN
    SELECT 
        COALESCE(s.status, c.status),
        COALESCE(s.version, 0),
        COALESCE(s.escalation_level, c.escalation_level),
        COALESCE(s.risk_rating, c.risk_rating),
        COALESCE(s.assigned_analyst_id, c.assigned_analyst_id),
        COALESCE(s.assigned_reviewer_id, c.assigned_reviewer_id)
    INTO v_current_status, v_current_version, v_escalation, v_risk, v_analyst, v_reviewer
    FROM kyc.cases c
    LEFT JOIN kyc.case_snapshots s ON s.case_id = c.case_id AND s.is_current = true
    WHERE c.case_id = p_case_id;
    
    IF v_current_status IS NULL THEN
        RAISE EXCEPTION 'Case not found: %', p_case_id;
    END IF;
    
    IF NOT kyc.is_valid_case_transition(v_current_status, p_new_status) THEN
        RAISE EXCEPTION 'Invalid case transition: % -> %', v_current_status, p_new_status;
    END IF;
    
    UPDATE kyc.case_snapshots
    SET is_current = false
    WHERE case_id = p_case_id AND is_current = true;
    
    INSERT INTO kyc.case_snapshots (
        case_id, status, escalation_level, risk_rating,
        assigned_analyst_id, assigned_reviewer_id,
        snapshot_reason, triggered_by_verb,
        version, is_current, created_by
    ) VALUES (
        p_case_id, p_new_status, v_escalation, v_risk,
        v_analyst, v_reviewer,
        COALESCE(p_reason, 'STATUS_CHANGE'), 'kyc-case.set-status',
        v_current_version + 1, true, p_actor
    )
    RETURNING snapshot_id INTO v_snapshot_id;
    
    UPDATE kyc.cases
    SET status = p_new_status, last_activity_at = now()
    WHERE case_id = p_case_id;
    
    RETURN v_snapshot_id;
END;
$f$ LANGUAGE plpgsql;

-- Initialize snapshots for existing cases (idempotent)
INSERT INTO kyc.case_snapshots (
    case_id, status, escalation_level, risk_rating,
    assigned_analyst_id, assigned_reviewer_id,
    snapshot_reason, version, is_current, created_by
)
SELECT 
    case_id, status, escalation_level, risk_rating,
    assigned_analyst_id, assigned_reviewer_id,
    'MIGRATION', 1, true, 'SYSTEM'
FROM kyc.cases c
WHERE NOT EXISTS (
    SELECT 1 FROM kyc.case_snapshots s WHERE s.case_id = c.case_id
);
