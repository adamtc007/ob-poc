-- Migration: 013_cbu_evidence.sql
-- Purpose: Add CBU evidence lifecycle tracking
-- Phase 2 of KYC DSL Transition Plan

-- ============================================================================
-- 1. Add status column to CBUs for lifecycle tracking
-- ============================================================================

ALTER TABLE "ob-poc".cbus ADD COLUMN IF NOT EXISTS status VARCHAR(30) DEFAULT 'DISCOVERED';

-- Add CHECK constraint for valid statuses
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'chk_cbu_status' AND conrelid = '"ob-poc".cbus'::regclass
    ) THEN
        ALTER TABLE "ob-poc".cbus ADD CONSTRAINT chk_cbu_status CHECK (
            status IN (
                'DISCOVERED',           -- Initial state - CBU identified
                'VALIDATION_PENDING',   -- Awaiting evidence/documentation
                'VALIDATED',            -- All required evidence collected and verified
                'UPDATE_PENDING_PROOF', -- Material change requires re-validation
                'VALIDATION_FAILED'     -- Unable to validate (can retry or close)
            )
        );
    END IF;
END $$;

-- ============================================================================
-- 2. CBU Evidence junction table
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_evidence (
    evidence_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    attestation_ref VARCHAR(255),           -- External attestation reference
    evidence_type VARCHAR(50) NOT NULL,     -- DOCUMENT, ATTESTATION, SCREENING, REGISTRY_CHECK
    evidence_category VARCHAR(50),          -- IDENTITY, OWNERSHIP, REGULATORY, FINANCIAL
    description TEXT,
    attached_at TIMESTAMPTZ DEFAULT now(),
    attached_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    verified_by VARCHAR(255),
    verification_status VARCHAR(30) DEFAULT 'PENDING',
    verification_notes TEXT,
    
    CONSTRAINT chk_evidence_type CHECK (
        evidence_type IN ('DOCUMENT', 'ATTESTATION', 'SCREENING', 'REGISTRY_CHECK', 'MANUAL_VERIFICATION')
    ),
    CONSTRAINT chk_evidence_verification_status CHECK (
        verification_status IN ('PENDING', 'VERIFIED', 'REJECTED', 'EXPIRED')
    ),
    CONSTRAINT chk_evidence_source CHECK (
        -- Must have either document_id OR attestation_ref
        (document_id IS NOT NULL) OR (attestation_ref IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_cbu_evidence_cbu_id ON "ob-poc".cbu_evidence(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_evidence_document_id ON "ob-poc".cbu_evidence(document_id);
CREATE INDEX IF NOT EXISTS idx_cbu_evidence_status ON "ob-poc".cbu_evidence(verification_status);

-- ============================================================================
-- 3. CBU Change Audit Log
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_change_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    change_type VARCHAR(50) NOT NULL,       -- STATUS_CHANGE, FIELD_UPDATE, EVIDENCE_ADDED, ROLE_CHANGE
    field_name VARCHAR(100),                -- Which field changed (for FIELD_UPDATE)
    old_value JSONB,
    new_value JSONB,
    evidence_ids UUID[],                    -- Evidence supporting this change
    changed_at TIMESTAMPTZ DEFAULT now(),
    changed_by VARCHAR(255),
    reason TEXT,
    case_id UUID,                           -- Linked KYC case if applicable
    
    CONSTRAINT chk_change_type CHECK (
        change_type IN ('STATUS_CHANGE', 'FIELD_UPDATE', 'EVIDENCE_ADDED', 
                        'EVIDENCE_VERIFIED', 'ROLE_CHANGE', 'UBO_CHANGE', 'PRODUCT_CHANGE')
    )
);

CREATE INDEX IF NOT EXISTS idx_cbu_change_log_cbu_id ON "ob-poc".cbu_change_log(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_change_log_changed_at ON "ob-poc".cbu_change_log(changed_at DESC);
CREATE INDEX IF NOT EXISTS idx_cbu_change_log_type ON "ob-poc".cbu_change_log(change_type);
CREATE INDEX IF NOT EXISTS idx_cbu_change_log_case_id ON "ob-poc".cbu_change_log(case_id);

-- ============================================================================
-- 4. CBU Status Transition Validation Function
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".is_valid_cbu_transition(
    p_from_status VARCHAR(30),
    p_to_status VARCHAR(30)
) RETURNS BOOLEAN AS $func$
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
$func$ LANGUAGE plpgsql IMMUTABLE;

-- ============================================================================
-- 5. Check CBU Evidence Completeness Function
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".check_cbu_evidence_completeness(
    p_cbu_id UUID
) RETURNS TABLE (
    is_complete BOOLEAN,
    missing_categories TEXT[],
    verified_count INTEGER,
    pending_count INTEGER,
    rejected_count INTEGER
) AS $func$
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
$func$ LANGUAGE plpgsql;

-- ============================================================================
-- 6. Trigger to log CBU status changes automatically
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".log_cbu_status_change()
RETURNS TRIGGER AS $func$
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
$func$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cbu_status_change ON "ob-poc".cbus;
CREATE TRIGGER trg_cbu_status_change
    AFTER UPDATE ON "ob-poc".cbus
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".log_cbu_status_change();

-- ============================================================================
-- 7. View for CBU validation status summary
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cbu_validation_summary AS
SELECT 
    c.cbu_id,
    c.name,
    c.status AS cbu_status,
    c.client_type,
    c.jurisdiction,
    COUNT(e.evidence_id) AS total_evidence,
    COUNT(e.evidence_id) FILTER (WHERE e.verification_status = 'VERIFIED') AS verified_evidence,
    COUNT(e.evidence_id) FILTER (WHERE e.verification_status = 'PENDING') AS pending_evidence,
    COUNT(e.evidence_id) FILTER (WHERE e.verification_status = 'REJECTED') AS rejected_evidence,
    ARRAY_AGG(DISTINCT e.evidence_category) FILTER (WHERE e.verification_status = 'VERIFIED') AS verified_categories,
    MAX(e.verified_at) AS last_verification_at,
    (
        SELECT COUNT(*) FROM "ob-poc".cbu_change_log cl 
        WHERE cl.cbu_id = c.cbu_id
    ) AS change_count
FROM "ob-poc".cbus c
LEFT JOIN "ob-poc".cbu_evidence e ON c.cbu_id = e.cbu_id
GROUP BY c.cbu_id, c.name, c.status, c.client_type, c.jurisdiction;

COMMENT ON TABLE "ob-poc".cbu_evidence IS 'Evidence/documentation attached to CBUs for validation';
COMMENT ON TABLE "ob-poc".cbu_change_log IS 'Audit trail of all CBU changes';
COMMENT ON FUNCTION "ob-poc".is_valid_cbu_transition IS 'Validates CBU status transitions';
COMMENT ON FUNCTION "ob-poc".check_cbu_evidence_completeness IS 'Checks if CBU has all required evidence categories verified';
