-- Migration: 014_ubo_proof.sql
-- Purpose: UBO discovery & proof lifecycle with evidence linking
-- Phase 3 of KYC DSL Transition Plan

-- =============================================================================
-- 1. Add/Update verification_status constraint with new values
-- =============================================================================

-- Add constraint if not exists, or update it
DO $$
BEGIN
    -- Drop existing constraint if it exists
    IF EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'chk_ubo_verification_status' 
        AND conrelid = '"ob-poc".ubo_registry'::regclass
    ) THEN
        ALTER TABLE "ob-poc".ubo_registry DROP CONSTRAINT chk_ubo_verification_status;
    END IF;
    
    -- Add new constraint with expanded status values
    ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT chk_ubo_verification_status CHECK (
        verification_status IN (
            'SUSPECTED',    -- Discovered but not yet proven (allegation)
            'PENDING',      -- Legacy - awaiting verification
            'PROVEN',       -- Evidence supports UBO determination
            'VERIFIED',     -- Legacy - externally verified
            'FAILED',       -- Verification attempted but failed
            'DISPUTED',     -- Challenged by subject or evidence conflict
            'REMOVED'       -- No longer a UBO (ownership changed, etc.)
        )
    );
END $$;

-- =============================================================================
-- 2. Add evidence linking columns to ubo_registry
-- =============================================================================

ALTER TABLE "ob-poc".ubo_registry 
    ADD COLUMN IF NOT EXISTS evidence_doc_ids UUID[],
    ADD COLUMN IF NOT EXISTS proof_date TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS proof_method VARCHAR(50),
    ADD COLUMN IF NOT EXISTS proof_notes TEXT,
    ADD COLUMN IF NOT EXISTS replacement_ubo_id UUID REFERENCES "ob-poc".ubo_registry(ubo_id),
    ADD COLUMN IF NOT EXISTS removal_reason VARCHAR(100);

-- Add check constraint for proof_method
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'chk_ubo_proof_method' 
        AND conrelid = '"ob-poc".ubo_registry'::regclass
    ) THEN
        ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT chk_ubo_proof_method CHECK (
            proof_method IS NULL OR proof_method IN (
                'DOCUMENT',           -- Proven via document review
                'REGISTRY_LOOKUP',    -- Proven via official registry
                'SCREENING_MATCH',    -- PEP/sanctions hit confirms identity
                'MANUAL_VERIFICATION', -- Analyst manual confirmation
                'OWNERSHIP_CHAIN',    -- Calculated from ownership data
                'CLIENT_ATTESTATION'  -- Client provided attestation
            )
        );
    END IF;
END $$;

-- =============================================================================
-- 3. UBO Evidence junction table (links UBOs to multiple documents)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_evidence (
    ubo_evidence_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ubo_id UUID NOT NULL REFERENCES "ob-poc".ubo_registry(ubo_id) ON DELETE CASCADE,
    document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    attestation_ref VARCHAR(255),
    evidence_type VARCHAR(50) NOT NULL,
    evidence_role VARCHAR(50) NOT NULL,      -- What this evidence proves
    description TEXT,
    attached_at TIMESTAMPTZ DEFAULT now(),
    attached_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    verified_by VARCHAR(255),
    verification_status VARCHAR(30) DEFAULT 'PENDING',
    verification_notes TEXT,
    
    CONSTRAINT chk_ubo_evidence_type CHECK (
        evidence_type IN ('DOCUMENT', 'ATTESTATION', 'SCREENING', 'REGISTRY_LOOKUP', 'OWNERSHIP_RECORD')
    ),
    CONSTRAINT chk_ubo_evidence_role CHECK (
        evidence_role IN (
            'IDENTITY_PROOF',       -- Proves the person's identity
            'OWNERSHIP_PROOF',      -- Proves ownership percentage
            'CONTROL_PROOF',        -- Proves control relationship
            'ADDRESS_PROOF',        -- Proves residential address
            'SOURCE_OF_WEALTH',     -- Explains source of wealth
            'CHAIN_LINK'            -- Links in ownership chain
        )
    ),
    CONSTRAINT chk_ubo_evidence_verification CHECK (
        verification_status IN ('PENDING', 'VERIFIED', 'REJECTED', 'EXPIRED')
    ),
    CONSTRAINT chk_ubo_evidence_source CHECK (
        (document_id IS NOT NULL) OR (attestation_ref IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_ubo_evidence_ubo_id ON "ob-poc".ubo_evidence(ubo_id);
CREATE INDEX IF NOT EXISTS idx_ubo_evidence_document_id ON "ob-poc".ubo_evidence(document_id);
CREATE INDEX IF NOT EXISTS idx_ubo_evidence_status ON "ob-poc".ubo_evidence(verification_status);

-- =============================================================================
-- 4. UBO Transition Validation Function
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".is_valid_ubo_transition(
    p_from_status VARCHAR(50),
    p_to_status VARCHAR(50)
) RETURNS BOOLEAN AS $func$
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
$func$ LANGUAGE plpgsql IMMUTABLE;

-- =============================================================================
-- 5. Function to check UBO can be proven (has required evidence)
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".can_prove_ubo(
    p_ubo_id UUID
) RETURNS TABLE (
    can_prove BOOLEAN,
    has_identity_proof BOOLEAN,
    has_ownership_proof BOOLEAN,
    missing_evidence TEXT[],
    verified_evidence_count INTEGER,
    pending_evidence_count INTEGER
) AS $func$
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
$func$ LANGUAGE plpgsql;

-- =============================================================================
-- 6. Trigger to validate UBO status transitions
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".validate_ubo_status_transition()
RETURNS TRIGGER AS $func$
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
$func$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_ubo_status_transition ON "ob-poc".ubo_registry;
CREATE TRIGGER trg_ubo_status_transition
    BEFORE UPDATE ON "ob-poc".ubo_registry
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".validate_ubo_status_transition();

-- =============================================================================
-- 7. View for UBO evidence summary
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_ubo_evidence_summary AS
SELECT 
    ur.ubo_id,
    ur.cbu_id,
    ur.subject_entity_id,
    ur.ubo_proper_person_id,
    e.name AS ubo_name,
    ur.verification_status,
    ur.proof_date,
    ur.proof_method,
    COUNT(ue.ubo_evidence_id) AS total_evidence,
    COUNT(ue.ubo_evidence_id) FILTER (WHERE ue.verification_status = 'VERIFIED') AS verified_evidence,
    COUNT(ue.ubo_evidence_id) FILTER (WHERE ue.verification_status = 'PENDING') AS pending_evidence,
    ARRAY_AGG(DISTINCT ue.evidence_role) FILTER (WHERE ue.verification_status = 'VERIFIED') AS proven_roles,
    (
        SELECT can_prove FROM "ob-poc".can_prove_ubo(ur.ubo_id) LIMIT 1
    ) AS can_be_proven
FROM "ob-poc".ubo_registry ur
JOIN "ob-poc".entities e ON ur.ubo_proper_person_id = e.entity_id
LEFT JOIN "ob-poc".ubo_evidence ue ON ur.ubo_id = ue.ubo_id
WHERE ur.closed_at IS NULL
  AND ur.superseded_at IS NULL
GROUP BY ur.ubo_id, ur.cbu_id, ur.subject_entity_id, ur.ubo_proper_person_id, 
         e.name, ur.verification_status, ur.proof_date, ur.proof_method;

-- =============================================================================
-- 8. Grants and Comments
-- =============================================================================

GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".ubo_evidence TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".is_valid_ubo_transition TO PUBLIC;
GRANT EXECUTE ON FUNCTION "ob-poc".can_prove_ubo TO PUBLIC;

COMMENT ON TABLE "ob-poc".ubo_evidence IS 'Evidence documents and attestations supporting UBO determinations';
COMMENT ON FUNCTION "ob-poc".is_valid_ubo_transition IS 'Validates UBO verification status transitions';
COMMENT ON FUNCTION "ob-poc".can_prove_ubo IS 'Checks if UBO has sufficient evidence to be proven';
COMMENT ON VIEW "ob-poc".v_ubo_evidence_summary IS 'Summary view of UBO records with evidence status';
