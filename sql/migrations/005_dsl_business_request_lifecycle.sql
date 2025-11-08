-- ============================================================================
-- Migration 005: DSL Business Request Lifecycle Management
-- ============================================================================
-- This migration adds the missing business context/lifecycle management for DSL instances.
-- Each business request (KYC.Case, Onboarding.request, etc.) gets a unique request ID
-- that persists through the entire lifecycle of DSL edits and amendments.
--
-- Changes:
-- 1. Create DSL business requests table for request lifecycle management
-- 2. Add request_id foreign key to dsl_versions table
-- 3. Create proper CRUD operations for business request management
-- 4. Add request status tracking and lifecycle management
-- 5. Create helper functions for request creation and management
-- ============================================================================

BEGIN;

-- ============================================================================
-- STEP 1: CREATE DSL BUSINESS REQUESTS TABLE
-- ============================================================================

-- DSL Business Requests Table
-- This is the primary business context that tracks individual cases/requests
-- Examples: KYC.Case.123, Onboarding.Request.456, Account.Opening.789
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_business_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES "ob-poc".dsl_domains (domain_id) ON DELETE RESTRICT,

    -- Business context identifiers
    business_reference VARCHAR(255) NOT NULL, -- External reference (case number, account ID, etc.)
    request_type VARCHAR(100) NOT NULL, -- 'KYC_CASE', 'ONBOARDING_REQUEST', 'ACCOUNT_OPENING', etc.
    client_id VARCHAR(255), -- Client or account identifier

    -- Request lifecycle status
    request_status VARCHAR(50) DEFAULT 'DRAFT', -- 'DRAFT', 'IN_PROGRESS', 'REVIEW', 'APPROVED', 'COMPLETED', 'CANCELLED', 'ERROR'
    priority_level VARCHAR(20) DEFAULT 'NORMAL', -- 'LOW', 'NORMAL', 'HIGH', 'CRITICAL'

    -- Business metadata
    request_title TEXT, -- Human-readable title/description
    request_description TEXT, -- Detailed description of the business request
    business_context JSONB, -- Additional business context (customer data, case details, etc.)

    -- Lifecycle tracking
    created_by VARCHAR(255) NOT NULL, -- User who created the request
    assigned_to VARCHAR(255), -- Current assignee
    reviewed_by VARCHAR(255), -- User who reviewed/approved
    completed_by VARCHAR(255), -- User who completed the request

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    assigned_at TIMESTAMPTZ,
    review_started_at TIMESTAMPTZ,
    approved_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    due_date TIMESTAMPTZ, -- Optional deadline

    -- Audit and compliance
    external_audit_id VARCHAR(255), -- External audit/compliance tracking ID
    regulatory_requirements JSONB, -- Specific regulatory requirements for this request

    -- Constraints
    CONSTRAINT valid_request_status CHECK (
        request_status IN ('DRAFT', 'IN_PROGRESS', 'REVIEW', 'APPROVED', 'COMPLETED', 'CANCELLED', 'ERROR')
    ),
    CONSTRAINT valid_priority_level CHECK (
        priority_level IN ('LOW', 'NORMAL', 'HIGH', 'CRITICAL')
    )
);

-- Create indexes for business requests
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_domain_id ON "ob-poc".dsl_business_requests (domain_id);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_business_ref ON "ob-poc".dsl_business_requests (business_reference);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_request_type ON "ob-poc".dsl_business_requests (request_type);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_status ON "ob-poc".dsl_business_requests (request_status);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_client_id ON "ob-poc".dsl_business_requests (client_id);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_assigned_to ON "ob-poc".dsl_business_requests (assigned_to);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_created_at ON "ob-poc".dsl_business_requests (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dsl_business_requests_due_date ON "ob-poc".dsl_business_requests (due_date);

-- Create unique constraint for business reference within domain
CREATE UNIQUE INDEX IF NOT EXISTS idx_dsl_business_requests_domain_business_ref
ON "ob-poc".dsl_business_requests (domain_id, business_reference);

-- ============================================================================
-- STEP 2: ADD REQUEST_ID TO DSL_VERSIONS TABLE
-- ============================================================================

-- Add request_id column to dsl_versions table
ALTER TABLE "ob-poc".dsl_versions
ADD COLUMN IF NOT EXISTS request_id UUID REFERENCES "ob-poc".dsl_business_requests (request_id) ON DELETE CASCADE;

-- Create index for the new foreign key
CREATE INDEX IF NOT EXISTS idx_dsl_versions_request_id ON "ob-poc".dsl_versions (request_id);

-- Update existing records to have null request_id (will be populated by migration script if needed)
-- Note: Existing records without business context will remain with NULL request_id

-- ============================================================================
-- STEP 3: CREATE BUSINESS REQUEST WORKFLOW STATES TABLE
-- ============================================================================

-- DSL Request Workflow States Table
-- Tracks the workflow progression for each business request
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_request_workflow_states (
    state_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".dsl_business_requests (request_id) ON DELETE CASCADE,

    -- Workflow state information
    workflow_state VARCHAR(100) NOT NULL, -- 'initial_draft', 'collecting_data', 'review_required', 'approved', etc.
    state_description TEXT,

    -- State transition tracking
    previous_state VARCHAR(100), -- What state we came from
    next_possible_states TEXT[], -- What states we can transition to

    -- State metadata
    state_data JSONB, -- State-specific data and context
    automation_trigger BOOLEAN DEFAULT false, -- Whether this state was entered automatically
    requires_approval BOOLEAN DEFAULT false, -- Whether this state requires manual approval

    -- State timing
    entered_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    entered_by VARCHAR(255) NOT NULL,
    estimated_duration_hours INTEGER, -- How long this state typically takes

    -- Current state tracking
    is_current_state BOOLEAN DEFAULT true,
    exited_at TIMESTAMPTZ,
    exited_by VARCHAR(255)
);

-- Create indexes for workflow states
CREATE INDEX IF NOT EXISTS idx_dsl_workflow_states_request_id ON "ob-poc".dsl_request_workflow_states (request_id);
CREATE INDEX IF NOT EXISTS idx_dsl_workflow_states_current ON "ob-poc".dsl_request_workflow_states (request_id, is_current_state);
CREATE INDEX IF NOT EXISTS idx_dsl_workflow_states_entered_at ON "ob-poc".dsl_request_workflow_states (entered_at DESC);

-- ============================================================================
-- STEP 4: CREATE UPDATED VIEWS WITH BUSINESS REQUEST CONTEXT
-- ============================================================================

-- View for active business requests with latest DSL version
CREATE OR REPLACE VIEW "ob-poc".dsl_active_business_requests AS
SELECT
    br.request_id,
    br.business_reference,
    br.request_type,
    br.client_id,
    br.request_status,
    br.priority_level,
    br.request_title,
    br.created_by as request_created_by,
    br.assigned_to,
    br.created_at as request_created_at,
    br.due_date,

    -- Domain information
    d.domain_name,
    d.description as domain_description,

    -- Latest DSL version for this request
    dv.version_id,
    dv.version_number,
    dv.functional_state,
    dv.compilation_status,
    dv.dsl_source_code,
    dv.created_by as version_created_by,
    dv.created_at as version_created_at,

    -- AST information
    CASE WHEN pa.ast_id IS NOT NULL THEN true ELSE false END as has_compiled_ast,
    pa.parsed_at,
    pa.complexity_score,

    -- Current workflow state
    ws.workflow_state as current_workflow_state,
    ws.state_description as current_state_description,
    ws.entered_at as state_entered_at

FROM "ob-poc".dsl_business_requests br
JOIN "ob-poc".dsl_domains d ON br.domain_id = d.domain_id
LEFT JOIN "ob-poc".dsl_versions dv ON br.request_id = dv.request_id
    AND dv.version_number = (
        SELECT MAX(version_number)
        FROM "ob-poc".dsl_versions dv2
        WHERE dv2.request_id = br.request_id
    )
LEFT JOIN "ob-poc".parsed_asts pa ON dv.version_id = pa.version_id
LEFT JOIN "ob-poc".dsl_request_workflow_states ws ON br.request_id = ws.request_id
    AND ws.is_current_state = true
WHERE br.request_status NOT IN ('COMPLETED', 'CANCELLED')
ORDER BY br.created_at DESC;

-- View for request workflow history
CREATE OR REPLACE VIEW "ob-poc".dsl_request_workflow_history AS
SELECT
    br.request_id,
    br.business_reference,
    br.request_type,
    d.domain_name,

    ws.state_id,
    ws.workflow_state,
    ws.state_description,
    ws.previous_state,
    ws.entered_at,
    ws.entered_by,
    ws.exited_at,
    ws.exited_by,
    ws.is_current_state,

    -- Calculate duration in state
    CASE
        WHEN ws.exited_at IS NOT NULL THEN
            EXTRACT(EPOCH FROM (ws.exited_at - ws.entered_at)) / 3600.0
        ELSE
            EXTRACT(EPOCH FROM (now() - ws.entered_at)) / 3600.0
    END as hours_in_state

FROM "ob-poc".dsl_business_requests br
JOIN "ob-poc".dsl_domains d ON br.domain_id = d.domain_id
JOIN "ob-poc".dsl_request_workflow_states ws ON br.request_id = ws.request_id
ORDER BY br.request_id, ws.entered_at DESC;

-- ============================================================================
-- STEP 5: CREATE HELPER FUNCTIONS FOR BUSINESS REQUEST LIFECYCLE
-- ============================================================================

-- Function to create a new business request and initial DSL version
CREATE OR REPLACE FUNCTION "ob-poc".create_business_request(
    p_domain_name VARCHAR,
    p_business_reference VARCHAR,
    p_request_type VARCHAR,
    p_client_id VARCHAR DEFAULT NULL,
    p_request_title TEXT DEFAULT NULL,
    p_request_description TEXT DEFAULT NULL,
    p_created_by VARCHAR DEFAULT 'SYSTEM',
    p_initial_dsl_code TEXT DEFAULT NULL,
    p_business_context JSONB DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_domain_id UUID;
    v_request_id UUID;
    v_version_id UUID;
    v_version_number INTEGER;
BEGIN
    -- Get domain ID
    SELECT domain_id INTO v_domain_id
    FROM "ob-poc".dsl_domains
    WHERE domain_name = p_domain_name AND active = true;

    IF v_domain_id IS NULL THEN
        RAISE EXCEPTION 'Domain % not found or not active', p_domain_name;
    END IF;

    -- Create the business request
    INSERT INTO "ob-poc".dsl_business_requests (
        domain_id,
        business_reference,
        request_type,
        client_id,
        request_title,
        request_description,
        created_by,
        business_context,
        request_status
    ) VALUES (
        v_domain_id,
        p_business_reference,
        p_request_type,
        p_client_id,
        p_request_title,
        p_request_description,
        p_created_by,
        p_business_context,
        'DRAFT'
    ) RETURNING request_id INTO v_request_id;

    -- Create initial workflow state
    INSERT INTO "ob-poc".dsl_request_workflow_states (
        request_id,
        workflow_state,
        state_description,
        entered_by
    ) VALUES (
        v_request_id,
        'initial_draft',
        'Business request created, initial DSL draft',
        p_created_by
    );

    -- Create initial DSL version if DSL code provided
    IF p_initial_dsl_code IS NOT NULL THEN
        v_version_number := 1;

        INSERT INTO "ob-poc".dsl_versions (
            domain_id,
            request_id,
            version_number,
            functional_state,
            dsl_source_code,
            compilation_status,
            change_description,
            created_by
        ) VALUES (
            v_domain_id,
            v_request_id,
            v_version_number,
            'Create_Case',
            p_initial_dsl_code,
            'DRAFT',
            'Initial DSL version for business request',
            p_created_by
        ) RETURNING version_id INTO v_version_id;
    END IF;

    RETURN v_request_id;
END;
$$ LANGUAGE plpgsql;

-- Function to create new DSL version for existing business request
CREATE OR REPLACE FUNCTION "ob-poc".create_dsl_amendment(
    p_request_id UUID,
    p_dsl_source_code TEXT,
    p_functional_state VARCHAR DEFAULT NULL,
    p_change_description TEXT DEFAULT NULL,
    p_created_by VARCHAR DEFAULT 'SYSTEM'
) RETURNS UUID AS $$
DECLARE
    v_domain_id UUID;
    v_version_number INTEGER;
    v_version_id UUID;
BEGIN
    -- Get domain ID and next version number for this request
    SELECT br.domain_id INTO v_domain_id
    FROM "ob-poc".dsl_business_requests br
    WHERE br.request_id = p_request_id;

    IF v_domain_id IS NULL THEN
        RAISE EXCEPTION 'Business request % not found', p_request_id;
    END IF;

    -- Get next version number for this request
    SELECT COALESCE(MAX(version_number), 0) + 1
    INTO v_version_number
    FROM "ob-poc".dsl_versions
    WHERE request_id = p_request_id;

    -- Create new DSL version
    INSERT INTO "ob-poc".dsl_versions (
        domain_id,
        request_id,
        version_number,
        functional_state,
        dsl_source_code,
        compilation_status,
        change_description,
        created_by
    ) VALUES (
        v_domain_id,
        p_request_id,
        v_version_number,
        COALESCE(p_functional_state, 'Review_Edit'),
        p_dsl_source_code,
        'DRAFT',
        COALESCE(p_change_description, format('DSL amendment version %s', v_version_number)),
        p_created_by
    ) RETURNING version_id INTO v_version_id;

    -- Update business request status if still in draft
    UPDATE "ob-poc".dsl_business_requests
    SET
        request_status = CASE
            WHEN request_status = 'DRAFT' THEN 'IN_PROGRESS'
            ELSE request_status
        END,
        updated_at = now()
    WHERE request_id = p_request_id;

    RETURN v_version_id;
END;
$$ LANGUAGE plpgsql;

-- Function to transition business request workflow state
CREATE OR REPLACE FUNCTION "ob-poc".transition_request_state(
    p_request_id UUID,
    p_new_state VARCHAR,
    p_state_description TEXT DEFAULT NULL,
    p_entered_by VARCHAR DEFAULT 'SYSTEM',
    p_state_data JSONB DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_state_id UUID;
    v_current_state VARCHAR;
BEGIN
    -- Get current state
    SELECT workflow_state INTO v_current_state
    FROM "dsl-ob-poc".dsl_request_workflow_states
    WHERE request_id = p_request_id AND is_current_state = true;

    -- Mark current state as exited
    UPDATE "dsl-ob-poc".dsl_request_workflow_states
    SET
        is_current_state = false,
        exited_at = now(),
        exited_by = p_entered_by
    WHERE request_id = p_request_id AND is_current_state = true;

    -- Create new state
    INSERT INTO "dsl-ob-poc".dsl_request_workflow_states (
        request_id,
        workflow_state,
        state_description,
        previous_state,
        entered_by,
        state_data
    ) VALUES (
        p_request_id,
        p_new_state,
        p_state_description,
        v_current_state,
        p_entered_by,
        p_state_data
    ) RETURNING state_id INTO v_state_id;

    -- Update business request timestamp
    UPDATE "dsl-ob-poc".dsl_business_requests
    SET updated_at = now()
    WHERE request_id = p_request_id;

    RETURN v_state_id;
END;
$$ LANGUAGE plpgsql;

-- Function to get business request summary
CREATE OR REPLACE FUNCTION "ob-poc".get_business_request_summary(p_request_id UUID)
RETURNS TABLE (
    request_id UUID,
    business_reference VARCHAR,
    request_type VARCHAR,
    domain_name VARCHAR,
    request_status VARCHAR,
    current_workflow_state VARCHAR,
    total_versions INTEGER,
    latest_version_number INTEGER,
    created_at TIMESTAMPTZ,
    last_updated TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        br.request_id,
        br.business_reference,
        br.request_type,
        d.domain_name,
        br.request_status,
        ws.workflow_state as current_workflow_state,
        COALESCE(version_count.total_versions, 0) as total_versions,
        COALESCE(version_count.latest_version, 0) as latest_version_number,
        br.created_at,
        br.updated_at as last_updated
    FROM "ob-poc".dsl_business_requests br
    JOIN "ob-poc".dsl_domains d ON br.domain_id = d.domain_id
    LEFT JOIN "ob-poc".dsl_request_workflow_states ws ON br.request_id = ws.request_id
        AND ws.is_current_state = true
    LEFT JOIN (
        SELECT
            request_id,
            COUNT(*) as total_versions,
            MAX(version_number) as latest_version
        FROM "ob-poc".dsl_versions
        GROUP BY request_id
    ) version_count ON br.request_id = version_count.request_id
    WHERE br.request_id = p_request_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- STEP 6: CREATE TRIGGERS FOR AUTOMATIC WORKFLOW MANAGEMENT
-- ============================================================================

-- Trigger function to automatically update business request status
CREATE OR REPLACE FUNCTION "ob-poc".update_business_request_on_version_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Update the business request's updated_at timestamp
    UPDATE "ob-poc".dsl_business_requests
    SET updated_at = now()
    WHERE request_id = NEW.request_id;

    -- Auto-transition workflow state based on compilation status
    IF NEW.compilation_status = 'COMPILED' AND OLD.compilation_status != 'COMPILED' THEN
        PERFORM "ob-poc".transition_request_state(
            NEW.request_id,
            'compiled_ready_for_review',
            'DSL successfully compiled and ready for review',
            'SYSTEM'
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for version changes
CREATE TRIGGER trigger_update_business_request_on_version_change
    AFTER UPDATE ON "ob-poc".dsl_versions
    FOR EACH ROW
    WHEN (NEW.request_id IS NOT NULL)
    EXECUTE FUNCTION "ob-poc".update_business_request_on_version_change();

-- ============================================================================
-- STEP 7: INSERT SAMPLE BUSINESS REQUEST TYPES
-- ============================================================================

-- Create a reference table for standard request types
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_request_types (
    request_type VARCHAR(100) PRIMARY KEY,
    domain_name VARCHAR(100) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    default_workflow_states TEXT[],
    estimated_duration_hours INTEGER,
    requires_approval BOOLEAN DEFAULT true,
    active BOOLEAN DEFAULT true
);

-- Insert standard request types
INSERT INTO "ob-poc".dsl_request_types (request_type, domain_name, display_name, description, default_workflow_states)
VALUES
    ('KYC_CASE', 'KYC', 'KYC Case Investigation', 'Know Your Customer compliance case',
     ARRAY['initial_draft', 'collecting_documents', 'ubo_analysis', 'compliance_review', 'approved', 'completed']),
    ('ONBOARDING_REQUEST', 'Onboarding', 'Customer Onboarding Request', 'New customer onboarding process',
     ARRAY['initial_draft', 'identity_verification', 'document_collection', 'risk_assessment', 'approved', 'completed']),
    ('ACCOUNT_OPENING', 'Account_Opening', 'Account Opening Application', 'New account setup and approval',
     ARRAY['initial_draft', 'application_review', 'document_verification', 'approval_workflow', 'account_setup', 'completed']),
    ('COMPLIANCE_REVIEW', 'Compliance', 'Compliance Review Case', 'Regulatory compliance review',
     ARRAY['initial_draft', 'data_collection', 'analysis', 'review', 'remediation', 'completed'])
ON CONFLICT (request_type) DO NOTHING;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

COMMIT;

-- Show summary of what was created
SELECT 'Migration 005 completed successfully - DSL Business Request Lifecycle' as status;

SELECT
    'Business Requests Table' as component,
    COUNT(*) as record_count
FROM "ob-poc".dsl_business_requests
UNION ALL
SELECT
    'Request Types',
    COUNT(*)
FROM "ob-poc".dsl_request_types
UNION ALL
SELECT
    'Workflow States',
    COUNT(*)
FROM "ob-poc".dsl_request_workflow_states;
