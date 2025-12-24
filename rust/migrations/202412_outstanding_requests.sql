-- ═══════════════════════════════════════════════════════════════════════════════════════════════
-- Migration: Outstanding Requests
-- Description: Fire-and-forget operations awaiting response (document requests, verifications, etc.)
-- ═══════════════════════════════════════════════════════════════════════════════════════════════

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Outstanding Requests Table
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS kyc.outstanding_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- ───────────────────────────────────────────────────────────────────────
    -- What is this request attached to?
    -- ───────────────────────────────────────────────────────────────────────
    subject_type VARCHAR(50) NOT NULL,      -- WORKSTREAM, KYC_CASE, ENTITY, CBU
    subject_id UUID NOT NULL,

    -- Link to specific workstream/case for easier queries
    workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    case_id UUID REFERENCES kyc.cases(case_id),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- ───────────────────────────────────────────────────────────────────────
    -- What was requested?
    -- ───────────────────────────────────────────────────────────────────────
    request_type VARCHAR(50) NOT NULL,      -- DOCUMENT, INFORMATION, VERIFICATION, APPROVAL, SIGNATURE
    request_subtype VARCHAR(100) NOT NULL,  -- SOURCE_OF_WEALTH, ID_DOCUMENT, REGISTRY_CHECK, etc.
    request_details JSONB DEFAULT '{}',     -- Flexible payload for request-specific data

    -- ───────────────────────────────────────────────────────────────────────
    -- Who is it from/to?
    -- ───────────────────────────────────────────────────────────────────────
    requested_from_type VARCHAR(50),        -- CLIENT, ENTITY, EXTERNAL_PROVIDER, INTERNAL
    requested_from_entity_id UUID,          -- If requesting from specific entity
    requested_from_label VARCHAR(255),      -- Human-readable: "John Smith (UBO)", "FCA Registry"
    requested_by_user_id UUID,              -- User who triggered request
    requested_by_agent BOOLEAN DEFAULT FALSE,

    -- ───────────────────────────────────────────────────────────────────────
    -- Timing
    -- ───────────────────────────────────────────────────────────────────────
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    due_date DATE,
    grace_period_days INTEGER DEFAULT 3,    -- Days after due_date before escalation

    -- ───────────────────────────────────────────────────────────────────────
    -- Communication tracking
    -- ───────────────────────────────────────────────────────────────────────
    last_reminder_at TIMESTAMPTZ,
    reminder_count INTEGER DEFAULT 0,
    max_reminders INTEGER DEFAULT 3,
    communication_log JSONB DEFAULT '[]',   -- Array of {timestamp, type, channel, reference}

    -- ───────────────────────────────────────────────────────────────────────
    -- Status
    -- ───────────────────────────────────────────────────────────────────────
    status VARCHAR(50) DEFAULT 'PENDING',   -- PENDING, FULFILLED, PARTIAL, CANCELLED, ESCALATED, EXPIRED, WAIVED
    status_reason TEXT,

    -- ───────────────────────────────────────────────────────────────────────
    -- Fulfillment
    -- ───────────────────────────────────────────────────────────────────────
    fulfilled_at TIMESTAMPTZ,
    fulfilled_by_user_id UUID,
    fulfillment_type VARCHAR(50),           -- DOCUMENT_UPLOAD, MANUAL_ENTRY, API_RESPONSE, WAIVER
    fulfillment_reference_type VARCHAR(50), -- DOCUMENT, VERIFICATION_RESULT, etc.
    fulfillment_reference_id UUID,          -- e.g., document_id that fulfilled this
    fulfillment_notes TEXT,

    -- ───────────────────────────────────────────────────────────────────────
    -- Escalation
    -- ───────────────────────────────────────────────────────────────────────
    escalated_at TIMESTAMPTZ,
    escalation_level INTEGER DEFAULT 0,     -- 0=none, 1=first escalation, 2=second, etc.
    escalation_reason VARCHAR(255),
    escalated_to_user_id UUID,

    -- ───────────────────────────────────────────────────────────────────────
    -- Blocking behavior
    -- ───────────────────────────────────────────────────────────────────────
    blocks_subject BOOLEAN DEFAULT TRUE,    -- Does this request block the subject?
    blocker_message VARCHAR(500),           -- "Awaiting source of wealth documentation"

    -- ───────────────────────────────────────────────────────────────────────
    -- DSL tracking
    -- ───────────────────────────────────────────────────────────────────────
    created_by_verb VARCHAR(100),           -- e.g., "document.request"
    created_by_execution_id UUID,           -- Link to DSL execution log if needed

    -- ───────────────────────────────────────────────────────────────────────
    -- Audit
    -- ───────────────────────────────────────────────────────────────────────
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- ───────────────────────────────────────────────────────────────────────
    -- Constraints
    -- ───────────────────────────────────────────────────────────────────────
    CONSTRAINT chk_oreq_subject_type CHECK (subject_type IN ('WORKSTREAM', 'KYC_CASE', 'ENTITY', 'CBU')),
    CONSTRAINT chk_oreq_request_type CHECK (request_type IN ('DOCUMENT', 'INFORMATION', 'VERIFICATION', 'APPROVAL', 'SIGNATURE')),
    CONSTRAINT chk_oreq_status CHECK (status IN ('PENDING', 'FULFILLED', 'PARTIAL', 'CANCELLED', 'ESCALATED', 'EXPIRED', 'WAIVED')),
    CONSTRAINT chk_oreq_fulfillment_type CHECK (fulfillment_type IS NULL OR fulfillment_type IN ('DOCUMENT_UPLOAD', 'MANUAL_ENTRY', 'API_RESPONSE', 'WAIVER')),
    CONSTRAINT chk_oreq_requested_from_type CHECK (requested_from_type IS NULL OR requested_from_type IN ('CLIENT', 'ENTITY', 'EXTERNAL_PROVIDER', 'INTERNAL'))
);

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Indexes
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

CREATE INDEX IF NOT EXISTS idx_oreq_subject ON kyc.outstanding_requests(subject_type, subject_id);
CREATE INDEX IF NOT EXISTS idx_oreq_workstream ON kyc.outstanding_requests(workstream_id) WHERE workstream_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_oreq_case ON kyc.outstanding_requests(case_id) WHERE case_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_oreq_cbu ON kyc.outstanding_requests(cbu_id) WHERE cbu_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_oreq_entity ON kyc.outstanding_requests(entity_id) WHERE entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_oreq_status ON kyc.outstanding_requests(status);
CREATE INDEX IF NOT EXISTS idx_oreq_status_pending ON kyc.outstanding_requests(due_date) WHERE status = 'PENDING';
CREATE INDEX IF NOT EXISTS idx_oreq_type ON kyc.outstanding_requests(request_type, request_subtype);
CREATE INDEX IF NOT EXISTS idx_oreq_overdue ON kyc.outstanding_requests(due_date, status)
    WHERE status = 'PENDING' AND due_date < CURRENT_DATE;

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Trigger for updated_at
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

CREATE OR REPLACE FUNCTION kyc.update_outstanding_request_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_outstanding_requests_updated ON kyc.outstanding_requests;
CREATE TRIGGER trg_outstanding_requests_updated
    BEFORE UPDATE ON kyc.outstanding_requests
    FOR EACH ROW EXECUTE FUNCTION kyc.update_outstanding_request_timestamp();

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Comments
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

COMMENT ON TABLE kyc.outstanding_requests IS 'Fire-and-forget operations awaiting response (document requests, verifications, etc.)';
COMMENT ON COLUMN kyc.outstanding_requests.subject_type IS 'What is this request attached to: WORKSTREAM, KYC_CASE, ENTITY, CBU';
COMMENT ON COLUMN kyc.outstanding_requests.request_type IS 'Category of request: DOCUMENT, INFORMATION, VERIFICATION, APPROVAL, SIGNATURE';
COMMENT ON COLUMN kyc.outstanding_requests.request_subtype IS 'Specific type within category, e.g., SOURCE_OF_WEALTH, ID_DOCUMENT';
COMMENT ON COLUMN kyc.outstanding_requests.blocks_subject IS 'Whether this pending request blocks the subject from progressing';
COMMENT ON COLUMN kyc.outstanding_requests.grace_period_days IS 'Days after due_date before auto-escalation';
