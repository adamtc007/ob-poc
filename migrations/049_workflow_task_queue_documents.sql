-- Migration 049: Workflow Task Queue & Document Entity
-- Implements async task return path for workflow engine
-- Design doc: TODO-WORKFLOW-TASK-QUEUE.md (peer-reviewed)

-- ============================================================================
-- SECTION 1: Rejection Reason Codes (reference data)
-- ============================================================================

-- Standardized rejection reasons - drives client messaging
CREATE TABLE IF NOT EXISTS "ob-poc".rejection_reason_codes (
    code TEXT PRIMARY KEY,
    category TEXT NOT NULL,           -- 'quality', 'mismatch', 'validity', 'data', 'format', 'authenticity'
    client_message TEXT NOT NULL,     -- User-facing message
    ops_message TEXT NOT NULL,        -- Internal ops message
    next_action TEXT NOT NULL,        -- What to do next
    is_retryable BOOLEAN DEFAULT true -- Can client retry with different upload?
);

-- Quality issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('UNREADABLE',      'quality', 'Document image is too blurry to read', 'OCR failed - image quality', 'Please re-upload a clear, high-resolution image', true),
('CUTOFF',          'quality', 'Part of the document is cut off', 'Incomplete capture', 'Ensure all four corners are visible in the image', true),
('GLARE',           'quality', 'Glare obscures important information', 'Light reflection on document', 'Avoid flash and direct lighting when photographing', true),
('LOW_RESOLUTION',  'quality', 'Image resolution too low', 'Below minimum DPI', 'Upload a higher resolution scan (300 DPI minimum)', true)
ON CONFLICT (code) DO NOTHING;

-- Wrong document
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('WRONG_DOC_TYPE',  'mismatch', 'This is not the requested document type', 'Doc type mismatch', 'Please upload the correct document type', true),
('WRONG_PERSON',    'mismatch', 'Document belongs to a different person', 'Name/subject mismatch', 'Upload document for the correct person', true),
('SAMPLE_DOC',      'mismatch', 'This appears to be a sample or specimen', 'Specimen/sample detected', 'Please upload your actual document', true)
ON CONFLICT (code) DO NOTHING;

-- Validity issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('EXPIRED',         'validity', 'Document has expired', 'Past expiry date', 'Please provide a current, valid document', true),
('NOT_YET_VALID',   'validity', 'Document is not yet valid', 'Future valid_from date', 'Please provide a currently valid document', true),
('UNDATED',         'validity', 'Document has no issue or expiry date', 'Missing dates', 'Please provide a dated document', true)
ON CONFLICT (code) DO NOTHING;

-- Data issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('DOB_MISMATCH',    'data', 'Date of birth does not match our records', 'DOB mismatch vs entity', 'Please verify the correct document or contact support', false),
('NAME_MISMATCH',   'data', 'Name does not match our records', 'Name mismatch vs entity', 'Please verify spelling or provide supporting name change document', false),
('ADDRESS_MISMATCH','data', 'Address does not match declared address', 'Address mismatch', 'Please provide proof of address at declared address', true)
ON CONFLICT (code) DO NOTHING;

-- Format issues
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('UNSUPPORTED_FORMAT', 'format', 'File format not supported', 'Invalid file type', 'Please upload PDF, JPEG, or PNG', true),
('PASSWORD_PROTECTED', 'format', 'Document is password protected', 'Cannot open file', 'Please upload an unprotected version', true),
('CORRUPTED',       'format', 'File appears to be corrupted', 'Cannot read file', 'Please re-upload or try a different file', true)
ON CONFLICT (code) DO NOTHING;

-- Authenticity (careful with wording - don't accuse)
INSERT INTO "ob-poc".rejection_reason_codes VALUES
('SUSPECTED_ALTERATION', 'authenticity', 'Document requires additional verification', 'Possible tampering detected', 'Our team will contact you for verification', false),
('INCONSISTENT_FONTS',   'authenticity', 'Document requires additional verification', 'Font inconsistency detected', 'Our team will contact you for verification', false)
ON CONFLICT (code) DO NOTHING;

-- ============================================================================
-- SECTION 2: Workflow Pending Tasks (outbound tracking)
-- Must be created BEFORE document_requirements due to FK
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".workflow_pending_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Links to workflow
    instance_id UUID NOT NULL REFERENCES "ob-poc".workflow_instances(instance_id),
    blocker_type TEXT NOT NULL,
    blocker_key TEXT,

    -- What was invoked (source of truth - don't trust external)
    verb TEXT NOT NULL,
    args JSONB,

    -- Expected results (multi-result support)
    expected_cargo_count INT DEFAULT 1,  -- How many results expected
    received_cargo_count INT DEFAULT 0,  -- How many completed with cargo
    failed_count INT DEFAULT 0,          -- How many failed/expired

    -- State
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'partial', 'completed', 'failed', 'expired', 'cancelled')),

    -- Timing
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Errors (last error for display)
    last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_pending_tasks_instance
    ON "ob-poc".workflow_pending_tasks(instance_id);
CREATE INDEX IF NOT EXISTS idx_pending_tasks_status
    ON "ob-poc".workflow_pending_tasks(status)
    WHERE status IN ('pending', 'partial');

-- ============================================================================
-- SECTION 3: Document Requirements (Layer A: what we need)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope (workflow or entity level)
    workflow_instance_id UUID REFERENCES "ob-poc".workflow_instances(instance_id),
    subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    subject_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),

    -- What's required
    doc_type TEXT NOT NULL,           -- 'passport', 'proof_of_address', 'articles_of_incorporation'
    required_state TEXT NOT NULL DEFAULT 'verified'
        CHECK (required_state IN ('received', 'verified')),

    -- Current status
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'requested', 'received', 'in_qa', 'verified', 'rejected', 'expired', 'waived')),

    -- Retry tracking
    attempt_count INT DEFAULT 0,
    max_attempts INT DEFAULT 3,
    current_task_id UUID REFERENCES "ob-poc".workflow_pending_tasks(task_id),

    -- Latest document (populated after documents table created)
    latest_document_id UUID,
    latest_version_id UUID,

    -- Rejection details (last failure - for messaging)
    last_rejection_code TEXT REFERENCES "ob-poc".rejection_reason_codes(code),
    last_rejection_reason TEXT,       -- Optional free-text override

    -- Timing
    due_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    satisfied_at TIMESTAMPTZ,         -- When status reached required_state

    -- Uniqueness: one requirement per doc_type per subject per workflow
    UNIQUE NULLS NOT DISTINCT (workflow_instance_id, subject_entity_id, doc_type)
);

-- Find unsatisfied requirements for a workflow
CREATE INDEX IF NOT EXISTS idx_doc_req_workflow_status
    ON "ob-poc".document_requirements(workflow_instance_id, status)
    WHERE status NOT IN ('verified', 'waived');

-- Find requirements for an entity
CREATE INDEX IF NOT EXISTS idx_doc_req_subject
    ON "ob-poc".document_requirements(subject_entity_id, doc_type);

-- Find requirements with active outreach tasks
CREATE INDEX IF NOT EXISTS idx_doc_req_task
    ON "ob-poc".document_requirements(current_task_id)
    WHERE current_task_id IS NOT NULL;

-- ============================================================================
-- SECTION 4: Documents (Layer B: logical identity)
-- ============================================================================

-- Stable identity for "passport for person X" - multiple versions live under this
CREATE TABLE IF NOT EXISTS "ob-poc".documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Classification
    document_type TEXT NOT NULL,      -- 'passport', 'subscription_form', 'lei_record'

    -- Relationships
    subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    subject_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    parent_document_id UUID REFERENCES "ob-poc".documents(document_id),

    -- Requirement linkage (which requirement this satisfies)
    requirement_id UUID REFERENCES "ob-poc".document_requirements(requirement_id),

    -- Provenance
    source TEXT NOT NULL,             -- 'upload', 'ocr', 'api', 'gleif', 'workflow'
    source_ref TEXT,                  -- External system ID

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT
);

-- Indexes for lookups
CREATE INDEX IF NOT EXISTS idx_documents_subject_type
    ON "ob-poc".documents(subject_entity_id, document_type);
CREATE INDEX IF NOT EXISTS idx_documents_requirement
    ON "ob-poc".documents(requirement_id)
    WHERE requirement_id IS NOT NULL;

-- Add FK constraints now that documents table exists
ALTER TABLE "ob-poc".document_requirements
    ADD CONSTRAINT fk_doc_req_latest_doc
    FOREIGN KEY (latest_document_id)
    REFERENCES "ob-poc".documents(document_id);

-- ============================================================================
-- SECTION 5: Document Versions (Layer C: immutable submissions)
-- ============================================================================

-- Each upload/submission is a new immutable version
CREATE TABLE IF NOT EXISTS "ob-poc".document_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".documents(document_id),
    version_no INT NOT NULL DEFAULT 1,

    -- Content type
    content_type TEXT NOT NULL,       -- MIME: 'application/json', 'image/jpeg', 'application/pdf'

    -- Content (at least one required)
    structured_data JSONB,            -- Parsed JSON/YAML
    blob_ref TEXT,                    -- Pointer to binary: 's3://bucket/key', 'file:///path'
    ocr_extracted JSONB,              -- Indexed fields from OCR/extraction

    -- Workflow linkage (which task produced this version)
    task_id UUID REFERENCES "ob-poc".workflow_pending_tasks(task_id),

    -- Verification status (on VERSION, not document - each submission verified separately)
    verification_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (verification_status IN ('pending', 'in_qa', 'verified', 'rejected')),

    -- Rejection details (if rejected)
    rejection_code TEXT REFERENCES "ob-poc".rejection_reason_codes(code),
    rejection_reason TEXT,            -- Optional free-text override/detail

    -- Verification audit
    verified_by TEXT,
    verified_at TIMESTAMPTZ,

    -- Validity period (from document content)
    valid_from DATE,
    valid_to DATE,

    -- Quality metrics (from OCR/extraction pipeline)
    quality_score NUMERIC(5,2),       -- 0.00 to 100.00
    extraction_confidence NUMERIC(5,2),

    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT,

    UNIQUE(document_id, version_no),
    CONSTRAINT version_has_content CHECK (
        structured_data IS NOT NULL OR blob_ref IS NOT NULL
    )
);

-- Find latest version for a document
CREATE INDEX IF NOT EXISTS idx_doc_versions_document
    ON "ob-poc".document_versions(document_id, version_no DESC);

-- Find versions by verification status
CREATE INDEX IF NOT EXISTS idx_doc_versions_status
    ON "ob-poc".document_versions(verification_status, created_at)
    WHERE verification_status IN ('pending', 'in_qa');

-- Find versions by task
CREATE INDEX IF NOT EXISTS idx_doc_versions_task
    ON "ob-poc".document_versions(task_id)
    WHERE task_id IS NOT NULL;

-- GIN indexes for content search
CREATE INDEX IF NOT EXISTS idx_doc_versions_structured
    ON "ob-poc".document_versions USING gin(structured_data jsonb_path_ops)
    WHERE structured_data IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_doc_versions_ocr
    ON "ob-poc".document_versions USING gin(ocr_extracted jsonb_path_ops)
    WHERE ocr_extracted IS NOT NULL;

-- Add FK constraint for latest_version_id
ALTER TABLE "ob-poc".document_requirements
    ADD CONSTRAINT fk_doc_req_latest_version
    FOREIGN KEY (latest_version_id)
    REFERENCES "ob-poc".document_versions(version_id);

-- ============================================================================
-- SECTION 6: Document Events (audit trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".documents(document_id),
    version_id UUID REFERENCES "ob-poc".document_versions(version_id),

    -- Event type
    event_type TEXT NOT NULL,         -- 'created', 'version_uploaded', 'verified', 'rejected', 'expired'

    -- Event details
    old_status TEXT,
    new_status TEXT,
    rejection_code TEXT,
    notes TEXT,

    -- Actor
    actor TEXT,                       -- 'system', 'qa_user@example.com', 'api:gleif'

    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_doc_events_document
    ON "ob-poc".document_events(document_id, occurred_at DESC);

-- ============================================================================
-- SECTION 7: Task Result Queue (inbound results)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".task_result_queue (
    id BIGSERIAL PRIMARY KEY,

    -- Routing (by UUID, not string)
    task_id UUID NOT NULL,

    -- Outcome
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'expired')),
    error TEXT,

    -- Cargo is always a POINTER (URI)
    cargo_type TEXT,              -- 'document', 'entity', 'screening', 'bundle'
    cargo_ref TEXT,               -- URI: 'document://ob-poc/uuid' or 'version://ob-poc/uuid'

    -- Raw payload for audit/debugging (original webhook body)
    payload JSONB,

    -- Queue management
    queued_at TIMESTAMPTZ DEFAULT now(),
    processed_at TIMESTAMPTZ,

    -- Retry handling
    retry_count INT DEFAULT 0,
    last_error TEXT,

    -- Deduplication: idempotency_key scoped to task (not global)
    idempotency_key TEXT NOT NULL
);

-- Primary deduplication: unique per task
CREATE UNIQUE INDEX IF NOT EXISTS idx_task_result_queue_idempotency
    ON "ob-poc".task_result_queue(task_id, idempotency_key);

-- Secondary dedupe for multi-result safety (backup protection)
CREATE UNIQUE INDEX IF NOT EXISTS idx_task_result_queue_dedupe
    ON "ob-poc".task_result_queue(task_id, cargo_ref, status)
    WHERE cargo_ref IS NOT NULL;

-- Optimized index for queue pop (partial index on unprocessed)
CREATE INDEX IF NOT EXISTS idx_task_result_queue_pending
    ON "ob-poc".task_result_queue(id)
    WHERE processed_at IS NULL;

-- Lookup by task
CREATE INDEX IF NOT EXISTS idx_task_result_queue_task
    ON "ob-poc".task_result_queue(task_id);

-- ============================================================================
-- SECTION 8: Dead Letter Queue
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".task_result_dlq (
    id BIGSERIAL PRIMARY KEY,
    original_id BIGINT NOT NULL,
    task_id UUID NOT NULL,
    status TEXT NOT NULL,
    cargo_type TEXT,
    cargo_ref TEXT,
    error TEXT,
    payload JSONB,
    retry_count INT,
    queued_at TIMESTAMPTZ,
    dead_lettered_at TIMESTAMPTZ DEFAULT now(),
    failure_reason TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_result_dlq_task
    ON "ob-poc".task_result_dlq(task_id);

-- ============================================================================
-- SECTION 9: Task Events History (permanent audit trail)
-- ============================================================================

-- Permanent record of all task events (queue is ephemeral, this is audit)
CREATE TABLE IF NOT EXISTS "ob-poc".workflow_task_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES "ob-poc".workflow_pending_tasks(task_id),

    -- Event type: 'created', 'result_received', 'completed', 'failed', 'expired', 'cancelled'
    event_type TEXT NOT NULL,

    -- Result details (for result_received events)
    result_status TEXT,           -- 'completed', 'failed', 'expired'
    cargo_type TEXT,
    cargo_ref TEXT,
    error TEXT,

    -- Raw payload for audit (original webhook body)
    payload JSONB,

    -- Source tracking
    source TEXT,                  -- 'webhook', 'internal', 'timeout_job'
    idempotency_key TEXT,         -- From the original request

    -- Timing
    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_task_events_task
    ON "ob-poc".workflow_task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type
    ON "ob-poc".workflow_task_events(event_type, occurred_at);

-- ============================================================================
-- SECTION 10: Helper Views
-- ============================================================================

-- View: Requirements with latest version status
CREATE OR REPLACE VIEW "ob-poc".v_requirements_with_latest_version AS
SELECT
    dr.requirement_id,
    dr.workflow_instance_id,
    dr.subject_entity_id,
    dr.subject_cbu_id,
    dr.doc_type,
    dr.required_state,
    dr.status AS requirement_status,
    dr.attempt_count,
    dr.max_attempts,
    dr.current_task_id,
    dr.last_rejection_code,
    dr.last_rejection_reason,
    dr.due_date,
    dr.satisfied_at,
    d.document_id,
    d.source AS document_source,
    dv.version_id,
    dv.version_no,
    dv.verification_status AS version_status,
    dv.rejection_code AS version_rejection_code,
    dv.verified_at,
    dv.valid_from,
    dv.valid_to,
    CASE
        WHEN dv.valid_to IS NOT NULL AND dv.valid_to < CURRENT_DATE THEN true
        ELSE false
    END AS is_expired
FROM "ob-poc".document_requirements dr
LEFT JOIN "ob-poc".documents d ON dr.latest_document_id = d.document_id
LEFT JOIN "ob-poc".document_versions dv ON dr.latest_version_id = dv.version_id;

-- View: Pending tasks with cargo summary
CREATE OR REPLACE VIEW "ob-poc".v_pending_tasks_summary AS
SELECT
    pt.task_id,
    pt.instance_id,
    pt.blocker_type,
    pt.verb,
    pt.status,
    pt.expected_cargo_count,
    pt.received_cargo_count,
    pt.failed_count,
    pt.created_at,
    pt.expires_at,
    pt.completed_at,
    pt.last_error,
    wi.workflow_id,
    wi.current_state AS workflow_state
FROM "ob-poc".workflow_pending_tasks pt
JOIN "ob-poc".workflow_instances wi ON pt.instance_id = wi.instance_id;

-- View: Documents with current status (joined)
CREATE OR REPLACE VIEW "ob-poc".v_documents_with_status AS
SELECT
    d.document_id,
    d.document_type,
    d.subject_entity_id,
    d.subject_cbu_id,
    d.requirement_id,
    d.source,
    d.source_ref,
    d.created_at,
    latest.version_id AS latest_version_id,
    latest.version_no AS latest_version_no,
    latest.verification_status AS latest_status,
    latest.verified_at,
    latest.valid_from,
    latest.valid_to
FROM "ob-poc".documents d
LEFT JOIN LATERAL (
    SELECT version_id, version_no, verification_status, verified_at, valid_from, valid_to
    FROM "ob-poc".document_versions dv
    WHERE dv.document_id = d.document_id
    ORDER BY version_no DESC
    LIMIT 1
) latest ON true;

-- ============================================================================
-- SECTION 11: Helper Functions
-- ============================================================================

-- Function to get next version number for a document
CREATE OR REPLACE FUNCTION "ob-poc".get_next_document_version(p_document_id UUID)
RETURNS INT AS $$
    SELECT COALESCE(MAX(version_no), 0) + 1
    FROM "ob-poc".document_versions
    WHERE document_id = p_document_id;
$$ LANGUAGE SQL;

-- Function to update requirement status when version status changes
CREATE OR REPLACE FUNCTION "ob-poc".fn_sync_requirement_from_version()
RETURNS TRIGGER AS $$
DECLARE
    v_requirement_id UUID;
    v_required_state TEXT;
    v_new_req_status TEXT;
BEGIN
    -- Find the requirement via document
    SELECT d.requirement_id, dr.required_state
    INTO v_requirement_id, v_required_state
    FROM "ob-poc".documents d
    JOIN "ob-poc".document_requirements dr ON d.requirement_id = dr.requirement_id
    WHERE d.document_id = NEW.document_id;

    IF v_requirement_id IS NULL THEN
        RETURN NEW;
    END IF;

    -- Map version status to requirement status
    v_new_req_status := CASE NEW.verification_status
        WHEN 'pending' THEN 'received'
        WHEN 'in_qa' THEN 'in_qa'
        WHEN 'verified' THEN 'verified'
        WHEN 'rejected' THEN 'rejected'
    END;

    -- Update requirement
    UPDATE "ob-poc".document_requirements
    SET
        status = v_new_req_status,
        latest_version_id = NEW.version_id,
        updated_at = now(),
        -- Set satisfied_at when we reach the required state
        satisfied_at = CASE
            WHEN v_new_req_status = 'verified' OR
                 (v_required_state = 'received' AND v_new_req_status IN ('received', 'in_qa', 'verified'))
            THEN COALESCE(satisfied_at, now())
            ELSE satisfied_at
        END,
        -- Copy rejection details if rejected
        last_rejection_code = CASE
            WHEN NEW.verification_status = 'rejected' THEN NEW.rejection_code
            ELSE last_rejection_code
        END,
        last_rejection_reason = CASE
            WHEN NEW.verification_status = 'rejected' THEN NEW.rejection_reason
            ELSE last_rejection_reason
        END
    WHERE requirement_id = v_requirement_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to sync requirement status from version changes
DROP TRIGGER IF EXISTS tr_sync_requirement_from_version ON "ob-poc".document_versions;
CREATE TRIGGER tr_sync_requirement_from_version
    AFTER INSERT OR UPDATE OF verification_status
    ON "ob-poc".document_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".fn_sync_requirement_from_version();

-- Function to create document event on status change
CREATE OR REPLACE FUNCTION "ob-poc".fn_document_version_event()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO "ob-poc".document_events
            (document_id, version_id, event_type, new_status, actor)
        VALUES
            (NEW.document_id, NEW.version_id, 'version_uploaded', NEW.verification_status, NEW.created_by);
    ELSIF OLD.verification_status != NEW.verification_status THEN
        INSERT INTO "ob-poc".document_events
            (document_id, version_id, event_type, old_status, new_status, rejection_code, actor)
        VALUES
            (NEW.document_id, NEW.version_id,
             CASE NEW.verification_status
                WHEN 'verified' THEN 'verified'
                WHEN 'rejected' THEN 'rejected'
                ELSE 'status_changed'
             END,
             OLD.verification_status, NEW.verification_status, NEW.rejection_code, NEW.verified_by);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tr_document_version_event ON "ob-poc".document_versions;
CREATE TRIGGER tr_document_version_event
    AFTER INSERT OR UPDATE OF verification_status
    ON "ob-poc".document_versions
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".fn_document_version_event();

-- Grant permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".rejection_reason_codes TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".workflow_pending_tasks TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".document_requirements TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".documents TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".document_versions TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".document_events TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".task_result_queue TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".task_result_dlq TO public;
GRANT SELECT, INSERT, UPDATE, DELETE ON "ob-poc".workflow_task_events TO public;
GRANT USAGE ON SEQUENCE "ob-poc".task_result_queue_id_seq TO public;
GRANT USAGE ON SEQUENCE "ob-poc".task_result_dlq_id_seq TO public;
