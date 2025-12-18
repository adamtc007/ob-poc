-- UBO Convergence Model Tables
-- Implements the observation-based KYC convergence model
-- See: docs/KYC-UBO-SOLUTION-OVERVIEW.md

-- ═══════════════════════════════════════════════════════════════════════════
-- PROOFS TABLE (Evidence documents linked to edges)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".proofs (
    proof_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- Document reference (links to document_catalog)
    document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),

    -- Proof type classification
    proof_type VARCHAR(50) NOT NULL,
    -- 'passport', 'national_id', 'drivers_license',
    -- 'certificate_of_incorporation', 'shareholder_register',
    -- 'trust_deed', 'partnership_agreement', 'articles_of_association',
    -- 'annual_return', 'board_resolution', 'registry_extract'

    -- Validity period
    valid_from DATE,
    valid_until DATE,
    -- Note: is_expired computed at query time, not stored (CURRENT_DATE not immutable)

    -- Status tracking
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    -- 'pending', 'valid', 'expired', 'dirty', 'superseded', 'rejected'

    -- Dirty flag for re-verification (periodic review)
    marked_dirty_at TIMESTAMPTZ,
    dirty_reason VARCHAR(100),

    -- Audit
    uploaded_by UUID,
    uploaded_at TIMESTAMPTZ DEFAULT NOW(),
    verified_by UUID,
    verified_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_proofs_cbu ON "ob-poc".proofs(cbu_id);
CREATE INDEX IF NOT EXISTS idx_proofs_status ON "ob-poc".proofs(cbu_id, status);
CREATE INDEX IF NOT EXISTS idx_proofs_document ON "ob-poc".proofs(document_id) WHERE document_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".proofs IS 'Evidence documents that prove ownership/control assertions';

-- ═══════════════════════════════════════════════════════════════════════════
-- UBO EDGES TABLE (Ownership/Control Graph with Convergence State)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_edges (
    edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- Graph edge endpoints
    from_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    to_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- Edge type
    edge_type VARCHAR(20) NOT NULL,
    -- 'ownership' - A owns X% of B
    -- 'control'   - A controls B (role: CEO, Director, etc.)
    -- 'trust_role' - A is trustee/beneficiary/settlor of Trust B

    -- For ownership edges
    percentage DECIMAL(5,2),

    -- For control edges
    control_role VARCHAR(50),  -- 'ceo', 'director', 'senior_manager', 'board_member'

    -- For trust role edges
    trust_role VARCHAR(50),    -- 'settlor', 'trustee', 'beneficiary', 'protector'
    interest_type VARCHAR(20), -- 'fixed', 'discretionary'

    -- ═══════════════════════════════════════════════════════════════════════
    -- ALLEGATION TRACKING (What client claims)
    -- ═══════════════════════════════════════════════════════════════════════
    alleged_percentage DECIMAL(5,2),
    alleged_role VARCHAR(50),
    alleged_at TIMESTAMPTZ,
    alleged_by UUID,  -- User who recorded the allegation
    allegation_source VARCHAR(100),  -- 'client_disclosure', 'onboarding_form', 'kyc_questionnaire'

    -- ═══════════════════════════════════════════════════════════════════════
    -- PROOF LINKAGE (Evidence supporting this edge)
    -- ═══════════════════════════════════════════════════════════════════════
    proof_id UUID REFERENCES "ob-poc".proofs(proof_id),

    -- ═══════════════════════════════════════════════════════════════════════
    -- OBSERVATION TRACKING (What proof actually shows)
    -- ═══════════════════════════════════════════════════════════════════════
    proven_percentage DECIMAL(5,2),
    proven_role VARCHAR(50),
    proven_at TIMESTAMPTZ,
    proven_by UUID,  -- User who verified

    -- ═══════════════════════════════════════════════════════════════════════
    -- EDGE STATE (Convergence status)
    -- ═══════════════════════════════════════════════════════════════════════
    status VARCHAR(20) NOT NULL DEFAULT 'alleged',
    -- 'alleged'  - Client claim only, no proof yet
    -- 'pending'  - Proof attached but not yet verified
    -- 'proven'   - Allegation matches observation
    -- 'disputed' - Allegation contradicts observation

    -- ═══════════════════════════════════════════════════════════════════════
    -- DISCREPANCY HANDLING
    -- ═══════════════════════════════════════════════════════════════════════
    discrepancy_notes TEXT,
    resolution_type VARCHAR(30),
    -- 'allegation_corrected' - Client updated their claim
    -- 'proof_accepted' - Accept proof value as truth
    -- 'waived' - Waived with justification
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    resolution_notes TEXT,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Unique constraint: one edge per from→to relationship type per CBU
    UNIQUE(cbu_id, from_entity_id, to_entity_id, edge_type),

    -- Check constraints
    CHECK (edge_type IN ('ownership', 'control', 'trust_role')),
    CHECK (status IN ('alleged', 'pending', 'proven', 'disputed')),
    CHECK (percentage IS NULL OR (percentage >= 0 AND percentage <= 100)),
    CHECK (alleged_percentage IS NULL OR (alleged_percentage >= 0 AND alleged_percentage <= 100)),
    CHECK (proven_percentage IS NULL OR (proven_percentage >= 0 AND proven_percentage <= 100))
);

CREATE INDEX IF NOT EXISTS idx_ubo_edges_cbu ON "ob-poc".ubo_edges(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_edges_from ON "ob-poc".ubo_edges(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_edges_to ON "ob-poc".ubo_edges(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_edges_status ON "ob-poc".ubo_edges(cbu_id, status);
CREATE INDEX IF NOT EXISTS idx_ubo_edges_proof ON "ob-poc".ubo_edges(proof_id) WHERE proof_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".ubo_edges IS 'Ownership/control graph edges with convergence state tracking';

-- ═══════════════════════════════════════════════════════════════════════════
-- UBO OBSERVATIONS TABLE (What proofs actually say)
-- Links to specific edges to record extracted observations
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_observations (
    observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    proof_id UUID NOT NULL REFERENCES "ob-poc".proofs(proof_id) ON DELETE CASCADE,
    edge_id UUID REFERENCES "ob-poc".ubo_edges(edge_id) ON DELETE SET NULL,

    -- What entity this observation is about
    subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- What was observed
    attribute_code VARCHAR(50) NOT NULL,
    -- 'ownership_percentage', 'name', 'date_of_birth', 'address',
    -- 'role', 'control_type', 'trust_role', 'interest_type'
    observed_value JSONB NOT NULL,

    -- Extraction metadata
    extracted_from JSONB,  -- {page: 3, section: "shareholders", confidence: 0.95}
    extraction_method VARCHAR(50),  -- 'manual', 'ocr', 'api', 'ai_extraction'
    confidence DECIMAL(3,2),  -- 0.00-1.00

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID
);

CREATE INDEX IF NOT EXISTS idx_ubo_observations_cbu ON "ob-poc".ubo_observations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_observations_edge ON "ob-poc".ubo_observations(edge_id);
CREATE INDEX IF NOT EXISTS idx_ubo_observations_proof ON "ob-poc".ubo_observations(proof_id);

COMMENT ON TABLE "ob-poc".ubo_observations IS 'Observations extracted from proofs for edge verification';

-- ═══════════════════════════════════════════════════════════════════════════
-- UBO ASSERTION LOG (Audit trail for declarative gates)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".ubo_assertion_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    case_id UUID REFERENCES kyc.cases(case_id),
    dsl_execution_id UUID,  -- Reference to DSL execution that made this assertion

    -- Assertion details
    assertion_type VARCHAR(50) NOT NULL,
    -- 'converged', 'no-expired-proofs', 'thresholds-pass',
    -- 'no-blocking-flags', 'no-disputed-edges'
    expected_value BOOLEAN NOT NULL,
    actual_value BOOLEAN NOT NULL,
    passed BOOLEAN NOT NULL,

    -- If failed, structured details
    failure_details JSONB,
    -- For converged: {blocking_edges: [...], alleged_count: N, disputed_count: M}
    -- For thresholds: {jurisdiction: "LU", threshold: 25, found: [...]}
    -- For flags: {blocking_flags: [...]}

    -- Audit
    asserted_at TIMESTAMPTZ DEFAULT NOW(),
    asserted_by UUID
);

CREATE INDEX IF NOT EXISTS idx_assertion_log_cbu ON "ob-poc".ubo_assertion_log(cbu_id);
CREATE INDEX IF NOT EXISTS idx_assertion_log_case ON "ob-poc".ubo_assertion_log(case_id);
CREATE INDEX IF NOT EXISTS idx_assertion_log_type ON "ob-poc".ubo_assertion_log(assertion_type, passed);

COMMENT ON TABLE "ob-poc".ubo_assertion_log IS 'Audit log of all KYC assertions (declarative gates)';

-- ═══════════════════════════════════════════════════════════════════════════
-- KYC DECISIONS TABLE (Final decision with evaluation snapshot)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    case_id UUID REFERENCES kyc.cases(case_id),

    -- Decision
    status VARCHAR(20) NOT NULL,
    -- 'CLEARED', 'REJECTED', 'CONDITIONAL', 'PENDING_REVIEW'
    conditions TEXT,  -- If conditional, what conditions apply

    -- Review scheduling
    review_interval INTERVAL,
    next_review_date DATE,

    -- Snapshot of evaluation at decision time (immutable audit record)
    evaluation_snapshot JSONB,
    -- {
    --   thresholds: {...},
    --   beneficial_owners: [...],
    --   control_persons: [...],
    --   red_flags: {blocking: [], warning: [], info: []},
    --   convergence: {total: N, proven: M, disputed: K}
    -- }

    -- Audit
    decided_by UUID NOT NULL,
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    decision_rationale TEXT,

    -- DSL trace for reproducibility
    dsl_execution_id UUID,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    CHECK (status IN ('CLEARED', 'REJECTED', 'CONDITIONAL', 'PENDING_REVIEW'))
);

CREATE INDEX IF NOT EXISTS idx_kyc_decisions_cbu ON "ob-poc".kyc_decisions(cbu_id);
CREATE INDEX IF NOT EXISTS idx_kyc_decisions_case ON "ob-poc".kyc_decisions(case_id);
CREATE INDEX IF NOT EXISTS idx_kyc_decisions_review ON "ob-poc".kyc_decisions(next_review_date)
    WHERE status = 'CLEARED';
CREATE INDEX IF NOT EXISTS idx_kyc_decisions_status ON "ob-poc".kyc_decisions(status);

COMMENT ON TABLE "ob-poc".kyc_decisions IS 'Final KYC decisions with complete evaluation snapshot';

-- ═══════════════════════════════════════════════════════════════════════════
-- CONVERGENCE STATUS VIEW (Computed, not stored)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".ubo_convergence_status AS
SELECT
    cbu_id,
    COUNT(*) AS total_edges,
    COUNT(*) FILTER (WHERE status = 'proven') AS proven_edges,
    COUNT(*) FILTER (WHERE status = 'alleged') AS alleged_edges,
    COUNT(*) FILTER (WHERE status = 'pending') AS pending_edges,
    COUNT(*) FILTER (WHERE status = 'disputed') AS disputed_edges,
    COUNT(*) FILTER (WHERE status = 'proven') = COUNT(*) AS is_converged
FROM "ob-poc".ubo_edges
GROUP BY cbu_id;

COMMENT ON VIEW "ob-poc".ubo_convergence_status IS 'Computed convergence status per CBU';

-- ═══════════════════════════════════════════════════════════════════════════
-- MISSING PROOFS VIEW (Edges needing proof)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".ubo_missing_proofs AS
SELECT
    e.cbu_id,
    e.edge_id,
    e.from_entity_id,
    f.name AS from_entity_name,
    e.to_entity_id,
    t.name AS to_entity_name,
    e.edge_type,
    e.status,
    e.alleged_percentage,
    CASE
        WHEN e.edge_type = 'ownership' THEN 'shareholder_register'
        WHEN e.edge_type = 'control' THEN 'board_resolution'
        WHEN e.edge_type = 'trust_role' THEN 'trust_deed'
    END AS required_proof_type
FROM "ob-poc".ubo_edges e
JOIN "ob-poc".entities f ON f.entity_id = e.from_entity_id
JOIN "ob-poc".entities t ON t.entity_id = e.to_entity_id
LEFT JOIN "ob-poc".proofs p ON e.proof_id = p.proof_id
WHERE e.status IN ('alleged', 'pending')
  AND (e.proof_id IS NULL OR p.status NOT IN ('valid', 'pending'));

COMMENT ON VIEW "ob-poc".ubo_missing_proofs IS 'Edges missing valid proof documents';

-- ═══════════════════════════════════════════════════════════════════════════
-- EXPIRED PROOFS VIEW
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW "ob-poc".ubo_expired_proofs AS
SELECT
    p.cbu_id,
    p.proof_id,
    p.proof_type,
    p.valid_until,
    p.marked_dirty_at,
    p.dirty_reason,
    e.edge_id,
    e.from_entity_id,
    e.to_entity_id,
    e.edge_type
FROM "ob-poc".proofs p
JOIN "ob-poc".ubo_edges e ON e.proof_id = p.proof_id
WHERE (p.valid_until IS NOT NULL AND p.valid_until < CURRENT_DATE) OR p.status = 'dirty';

COMMENT ON VIEW "ob-poc".ubo_expired_proofs IS 'Edges with expired or dirty proofs';

-- ═══════════════════════════════════════════════════════════════════════════
-- TRIGGER: Update updated_at on ubo_edges
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION "ob-poc".update_ubo_edges_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_ubo_edges_updated ON "ob-poc".ubo_edges;
CREATE TRIGGER trg_ubo_edges_updated
    BEFORE UPDATE ON "ob-poc".ubo_edges
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_ubo_edges_timestamp();

-- ═══════════════════════════════════════════════════════════════════════════
-- TRIGGER: Update updated_at on proofs
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION "ob-poc".update_proofs_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_proofs_updated ON "ob-poc".proofs;
CREATE TRIGGER trg_proofs_updated
    BEFORE UPDATE ON "ob-poc".proofs
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_proofs_timestamp();
