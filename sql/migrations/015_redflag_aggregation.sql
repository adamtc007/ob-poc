-- Migration: 015_redflag_aggregation.sql
-- Purpose: Red-flag severity aggregation and case decision support
-- Phase 4 of KYC DSL Transition Plan

-- =============================================================================
-- 1. Red-flag score configuration table
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".redflag_score_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    severity VARCHAR(20) NOT NULL,
    weight INTEGER NOT NULL,
    is_blocking BOOLEAN DEFAULT false,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT chk_redflag_severity CHECK (severity IN ('SOFT', 'ESCALATE', 'HARD_STOP')),
    CONSTRAINT uq_redflag_severity UNIQUE (severity)
);

-- Insert default weights (SOFT=1, ESCALATE=2, HARD_STOP=blocking)
INSERT INTO "ob-poc".redflag_score_config (severity, weight, is_blocking, description)
VALUES 
    ('SOFT', 1, false, 'Minor issue requiring documentation'),
    ('ESCALATE', 2, false, 'Requires senior review'),
    ('HARD_STOP', 1000, true, 'Blocking - cannot proceed without resolution')
ON CONFLICT (severity) DO NOTHING;

-- =============================================================================
-- 2. Decision thresholds configuration
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".case_decision_thresholds (
    threshold_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    threshold_name VARCHAR(100) NOT NULL UNIQUE,
    min_score INTEGER,                    -- Minimum score to trigger
    max_score INTEGER,                    -- Maximum score for this threshold
    has_hard_stop BOOLEAN DEFAULT false,  -- Requires hard stop
    escalation_level VARCHAR(30),         -- Required escalation level
    recommended_action VARCHAR(50) NOT NULL,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT chk_recommended_action CHECK (
        recommended_action IN (
            'APPROVE',              -- Can approve at current level
            'APPROVE_WITH_CONDITIONS', -- Approve with monitoring conditions
            'ESCALATE',             -- Escalate to higher authority
            'REFER_TO_REGULATOR',   -- Must refer to regulatory body
            'DO_NOT_ONBOARD',       -- Cannot onboard
            'REJECT'                -- Standard rejection
        )
    )
);

-- Insert default thresholds
INSERT INTO "ob-poc".case_decision_thresholds 
    (threshold_name, min_score, max_score, has_hard_stop, escalation_level, recommended_action, description)
VALUES
    ('clean', 0, 0, false, NULL, 'APPROVE', 'No red flags - standard approval'),
    ('minor_issues', 1, 2, false, NULL, 'APPROVE_WITH_CONDITIONS', '1-2 soft flags - approve with conditions'),
    ('moderate_risk', 3, 4, false, 'SENIOR_COMPLIANCE', 'ESCALATE', 'Multiple soft flags - senior review'),
    ('high_risk', 5, 10, false, 'EXECUTIVE', 'ESCALATE', 'High flag count - executive review'),
    ('escalate_flags', NULL, NULL, false, 'SENIOR_COMPLIANCE', 'ESCALATE', 'Has escalate-level flags'),
    ('hard_stop', NULL, NULL, true, 'EXECUTIVE', 'DO_NOT_ONBOARD', 'Has hard stop flags'),
    ('regulatory_referral', 11, NULL, false, 'BOARD', 'REFER_TO_REGULATOR', 'Extreme risk - regulatory referral')
ON CONFLICT (threshold_name) DO NOTHING;

-- =============================================================================
-- 3. Case evaluation snapshots (audit trail of decisions)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".case_evaluation_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    
    -- Red-flag counts by severity
    soft_count INTEGER NOT NULL DEFAULT 0,
    escalate_count INTEGER NOT NULL DEFAULT 0,
    hard_stop_count INTEGER NOT NULL DEFAULT 0,
    
    -- Calculated scores
    soft_score INTEGER NOT NULL DEFAULT 0,
    escalate_score INTEGER NOT NULL DEFAULT 0,
    has_hard_stop BOOLEAN NOT NULL DEFAULT false,
    total_score INTEGER NOT NULL DEFAULT 0,
    
    -- Unresolved vs total
    open_flags INTEGER NOT NULL DEFAULT 0,
    mitigated_flags INTEGER NOT NULL DEFAULT 0,
    waived_flags INTEGER NOT NULL DEFAULT 0,
    
    -- Decision recommendation
    matched_threshold_id UUID REFERENCES "ob-poc".case_decision_thresholds(threshold_id),
    recommended_action VARCHAR(50),
    required_escalation_level VARCHAR(30),
    
    -- Context
    evaluated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    evaluated_by VARCHAR(255),
    notes TEXT,
    
    -- Decision tracking
    decision_made VARCHAR(50),
    decision_made_at TIMESTAMPTZ,
    decision_made_by VARCHAR(255),
    decision_notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_case_eval_snapshots_case_id ON "ob-poc".case_evaluation_snapshots(case_id);
CREATE INDEX IF NOT EXISTS idx_case_eval_snapshots_evaluated_at ON "ob-poc".case_evaluation_snapshots(evaluated_at DESC);

-- =============================================================================
-- 4. Function to compute case red-flag score
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".compute_case_redflag_score(
    p_case_id UUID
) RETURNS TABLE (
    soft_count INTEGER,
    escalate_count INTEGER,
    hard_stop_count INTEGER,
    soft_score INTEGER,
    escalate_score INTEGER,
    has_hard_stop BOOLEAN,
    total_score INTEGER,
    open_flags INTEGER,
    mitigated_flags INTEGER,
    waived_flags INTEGER
) AS $func$
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
$func$ LANGUAGE plpgsql;

-- =============================================================================
-- 5. Function to evaluate case and recommend decision
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".evaluate_case_decision(
    p_case_id UUID,
    p_evaluator VARCHAR(255) DEFAULT NULL
) RETURNS UUID AS $func$
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
$func$ LANGUAGE plpgsql;

-- =============================================================================
-- 6. Function to apply decision to case (with validation)
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".apply_case_decision(
    p_case_id UUID,
    p_decision VARCHAR(50),
    p_decided_by VARCHAR(255),
    p_notes TEXT DEFAULT NULL
) RETURNS BOOLEAN AS $func$
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
$func$ LANGUAGE plpgsql;

-- =============================================================================
-- 7. View for case red-flag summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_case_redflag_summary AS
SELECT 
    c.case_id,
    c.cbu_id,
    c.status as case_status,
    c.escalation_level,
    scores.soft_count,
    scores.escalate_count,
    scores.hard_stop_count,
    scores.total_score,
    scores.has_hard_stop,
    scores.open_flags,
    scores.mitigated_flags,
    scores.waived_flags,
    (
        SELECT recommended_action 
        FROM "ob-poc".case_evaluation_snapshots es
        WHERE es.case_id = c.case_id
        ORDER BY evaluated_at DESC
        LIMIT 1
    ) as last_recommendation,
    (
        SELECT evaluated_at 
        FROM "ob-poc".case_evaluation_snapshots es
        WHERE es.case_id = c.case_id
        ORDER BY evaluated_at DESC
        LIMIT 1
    ) as last_evaluated_at
FROM kyc.cases c
CROSS JOIN LATERAL "ob-poc".compute_case_redflag_score(c.case_id) scores;

-- =============================================================================
-- 8. Grants and Comments
-- =============================================================================

GRANT SELECT, INSERT, UPDATE ON "ob-poc".redflag_score_config TO PUBLIC;
GRANT SELECT, INSERT, UPDATE ON "ob-poc".case_decision_thresholds TO PUBLIC;
GRANT SELECT, INSERT, UPDATE ON "ob-poc".case_evaluation_snapshots TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".compute_case_redflag_score TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".evaluate_case_decision TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".apply_case_decision TO PUBLIC;

COMMENT ON TABLE "ob-poc".redflag_score_config IS 'Red-flag severity weights for score calculation';
COMMENT ON TABLE "ob-poc".case_decision_thresholds IS 'Thresholds mapping scores to recommended actions';
COMMENT ON TABLE "ob-poc".case_evaluation_snapshots IS 'Audit trail of case evaluations and decisions';
COMMENT ON FUNCTION "ob-poc".compute_case_redflag_score IS 'Computes aggregated red-flag scores for a case';
COMMENT ON FUNCTION "ob-poc".evaluate_case_decision IS 'Evaluates case and creates recommendation snapshot';
COMMENT ON FUNCTION "ob-poc".apply_case_decision IS 'Applies decision to case with validation';
COMMENT ON VIEW "ob-poc".v_case_redflag_summary IS 'Summary view of case red-flag status';
