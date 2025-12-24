-- Client Portal Schema
-- Enables client-facing portal for document submission and status tracking

-- =============================================================================
-- SCHEMA
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS client_portal;

-- =============================================================================
-- CLIENT CREDENTIALS
-- =============================================================================

-- Clients who can access the portal
CREATE TABLE client_portal.clients (
    client_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    -- Which CBUs this client can access
    accessible_cbus UUID[] NOT NULL DEFAULT '{}',
    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,
    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ
);

-- Client credentials (separate for security)
CREATE TABLE client_portal.credentials (
    credential_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID NOT NULL REFERENCES client_portal.clients(client_id) ON DELETE CASCADE,
    credential_hash TEXT NOT NULL,  -- bcrypt hash
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_credentials_client ON client_portal.credentials(client_id);

-- =============================================================================
-- CLIENT SESSIONS
-- =============================================================================

CREATE TABLE client_portal.sessions (
    session_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID NOT NULL REFERENCES client_portal.clients(client_id) ON DELETE CASCADE,
    -- Active CBU context
    active_cbu_id UUID,
    -- Collection mode state (JSON for flexibility)
    collection_state JSONB,
    -- Session metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_active_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '24 hours')
);

CREATE INDEX idx_sessions_client ON client_portal.sessions(client_id);
CREATE INDEX idx_sessions_expires ON client_portal.sessions(expires_at);

-- =============================================================================
-- CLIENT COMMITMENTS (for follow-up reminders)
-- =============================================================================

CREATE TABLE client_portal.commitments (
    commitment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID NOT NULL REFERENCES client_portal.clients(client_id) ON DELETE CASCADE,
    request_id UUID NOT NULL,  -- References kyc.outstanding_requests
    -- What they promised
    commitment_text TEXT NOT NULL,
    expected_date DATE,
    -- Reminder tracking
    reminder_date DATE,
    reminder_sent_at TIMESTAMPTZ,
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'FULFILLED', 'OVERDUE', 'CANCELLED')),
    fulfilled_at TIMESTAMPTZ,
    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_commitments_client ON client_portal.commitments(client_id);
CREATE INDEX idx_commitments_request ON client_portal.commitments(request_id);
CREATE INDEX idx_commitments_status ON client_portal.commitments(status) WHERE status = 'PENDING';

-- =============================================================================
-- CLIENT SUBMISSIONS
-- =============================================================================

-- Track what clients have submitted (separate from internal document catalog)
CREATE TABLE client_portal.submissions (
    submission_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID NOT NULL REFERENCES client_portal.clients(client_id),
    request_id UUID NOT NULL,  -- References kyc.outstanding_requests
    -- Submission type
    submission_type VARCHAR(50) NOT NULL
        CHECK (submission_type IN ('DOCUMENT', 'INFORMATION', 'NOTE', 'CLARIFICATION')),
    -- For documents
    document_type VARCHAR(100),
    file_reference TEXT,  -- S3/storage key
    file_name VARCHAR(255),
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),
    -- For information
    info_type VARCHAR(100),
    info_data JSONB,
    -- For notes/clarifications
    note_text TEXT,
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'SUBMITTED'
        CHECK (status IN ('SUBMITTED', 'UNDER_REVIEW', 'ACCEPTED', 'REJECTED', 'SUPERSEDED')),
    review_notes TEXT,
    reviewed_by UUID,
    reviewed_at TIMESTAMPTZ,
    -- Link to internal document catalog once accepted
    cataloged_document_id UUID,  -- References ob-poc.document_catalog
    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_submissions_client ON client_portal.submissions(client_id);
CREATE INDEX idx_submissions_request ON client_portal.submissions(request_id);
CREATE INDEX idx_submissions_status ON client_portal.submissions(status);

-- =============================================================================
-- ESCALATIONS
-- =============================================================================

CREATE TABLE client_portal.escalations (
    escalation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID NOT NULL REFERENCES client_portal.clients(client_id),
    session_id UUID REFERENCES client_portal.sessions(session_id),
    -- Context
    cbu_id UUID,
    reason TEXT,
    preferred_contact VARCHAR(20) CHECK (preferred_contact IN ('CALL', 'EMAIL', 'VIDEO')),
    -- Conversation context (full transcript)
    conversation_context JSONB,
    -- Assignment
    assigned_to_user_id UUID,
    assigned_at TIMESTAMPTZ,
    -- Resolution
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'ASSIGNED', 'IN_PROGRESS', 'RESOLVED', 'CLOSED')),
    resolution_notes TEXT,
    resolved_at TIMESTAMPTZ,
    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_escalations_client ON client_portal.escalations(client_id);
CREATE INDEX idx_escalations_status ON client_portal.escalations(status) WHERE status NOT IN ('RESOLVED', 'CLOSED');

-- =============================================================================
-- ADD WHY COLUMNS TO OUTSTANDING_REQUESTS
-- =============================================================================

-- These columns enable the client portal to explain WHY each document is needed
ALTER TABLE kyc.outstanding_requests
    ADD COLUMN IF NOT EXISTS reason_for_request TEXT,
    ADD COLUMN IF NOT EXISTS compliance_context TEXT,
    ADD COLUMN IF NOT EXISTS acceptable_alternatives TEXT[],
    ADD COLUMN IF NOT EXISTS client_visible BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS client_notes TEXT;

COMMENT ON COLUMN kyc.outstanding_requests.reason_for_request IS 'Plain English explanation of why this is needed';
COMMENT ON COLUMN kyc.outstanding_requests.compliance_context IS 'Regulatory/legal basis for the request';
COMMENT ON COLUMN kyc.outstanding_requests.acceptable_alternatives IS 'Alternative document types that would satisfy this request';
COMMENT ON COLUMN kyc.outstanding_requests.client_visible IS 'Whether this request should be shown to the client';
COMMENT ON COLUMN kyc.outstanding_requests.client_notes IS 'Notes from the client about this request';

-- =============================================================================
-- VIEWS
-- =============================================================================

-- Client-facing view of outstanding requests
CREATE OR REPLACE VIEW client_portal.v_client_outstanding AS
SELECT
    r.request_id,
    r.cbu_id,
    r.entity_id,
    e.name as entity_name,
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
    -- Submission count
    (SELECT COUNT(*) FROM client_portal.submissions s
     WHERE s.request_id = r.request_id) as submission_count
FROM kyc.outstanding_requests r
LEFT JOIN "ob-poc".entities e ON r.entity_id = e.entity_id
WHERE r.client_visible = true
  AND r.status NOT IN ('FULFILLED', 'CANCELLED', 'WAIVED');

-- =============================================================================
-- TEST DATA
-- =============================================================================

-- Insert a test client for development
INSERT INTO client_portal.clients (client_id, name, email, accessible_cbus)
SELECT
    'a1b2c3d4-e5f6-7890-abcd-ef1234567890'::uuid,
    'Test Client Portal User',
    'testclient@example.com',
    ARRAY(SELECT cbu_id FROM "ob-poc".cbus LIMIT 3)
WHERE NOT EXISTS (
    SELECT 1 FROM client_portal.clients WHERE email = 'testclient@example.com'
);

-- Test credential (password: 'test-password')
-- bcrypt hash generated with cost 10
INSERT INTO client_portal.credentials (client_id, credential_hash)
SELECT
    'a1b2c3d4-e5f6-7890-abcd-ef1234567890'::uuid,
    '$2b$10$rQZ7.HJ5JvH5dQOH5L5qUOYJKVDqQMhLvS1JqE5jH5K5mN5oP5rS6'
WHERE NOT EXISTS (
    SELECT 1 FROM client_portal.credentials
    WHERE client_id = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890'::uuid
);
