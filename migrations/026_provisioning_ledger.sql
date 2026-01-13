-- Migration 026: Provisioning Ledger (Append-Only)
--
-- Adds:
-- 1. provisioning_requests - append-only log of provisioning requests
-- 2. provisioning_events - append-only log of events from owner systems
-- 3. Enhances cbu_resource_instances with owner response columns
--
-- These tables are APPEND-ONLY for audit compliance.
-- Updates/deletes are prevented by triggers.
--
-- Part of CBU Resource Pipeline implementation

-- =============================================================================
-- 1. PROVISIONING REQUESTS (Append-Only)
-- =============================================================================
-- One row per provisioning request to an owner system.
-- Status can be updated via a separate status tracking mechanism.

CREATE TABLE IF NOT EXISTS "ob-poc".provisioning_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What we're provisioning for
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    srdef_id TEXT NOT NULL,
    instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    -- Who requested it
    requested_by TEXT NOT NULL DEFAULT 'system'
        CHECK (requested_by IN ('agent', 'user', 'system')),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Full request payload (attrs snapshot, bind_to, evidence)
    request_payload JSONB NOT NULL,

    -- Status tracking (updated via events, not direct UPDATE)
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'sent', 'ack', 'completed', 'failed', 'cancelled')),

    -- Owner system info
    owner_system TEXT NOT NULL,  -- app mnemonic (CUSTODY, SWIFT, IAM, etc.)
    owner_ticket_id TEXT,  -- external ticket/reference number

    -- For parameterized resources
    parameters JSONB DEFAULT '{}'
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_cbu
    ON "ob-poc".provisioning_requests(cbu_id);
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_status
    ON "ob-poc".provisioning_requests(status) WHERE status IN ('queued', 'sent', 'ack');
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_srdef
    ON "ob-poc".provisioning_requests(srdef_id);
CREATE INDEX IF NOT EXISTS idx_provisioning_requests_instance
    ON "ob-poc".provisioning_requests(instance_id) WHERE instance_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".provisioning_requests IS
    'Append-only log of provisioning requests to owner systems.';
COMMENT ON COLUMN "ob-poc".provisioning_requests.request_payload IS
    'Full request snapshot: {attrs: {...}, bind_to: {...}, evidence_refs: [...]}';
COMMENT ON COLUMN "ob-poc".provisioning_requests.owner_system IS
    'Owner app mnemonic: CUSTODY, SWIFT, IAM, TRADING, etc.';

-- =============================================================================
-- 2. PROVISIONING EVENTS (Append-Only)
-- =============================================================================
-- Log of all events related to provisioning requests.
-- Includes outbound (REQUEST_SENT) and inbound (ACK, RESULT, ERROR) events.

CREATE TABLE IF NOT EXISTS "ob-poc".provisioning_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES "ob-poc".provisioning_requests(request_id),

    -- When and what direction
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    direction TEXT NOT NULL CHECK (direction IN ('OUT', 'IN')),

    -- Event type
    kind TEXT NOT NULL
        CHECK (kind IN ('REQUEST_SENT', 'ACK', 'RESULT', 'ERROR', 'STATUS', 'RETRY')),

    -- Full event payload
    payload JSONB NOT NULL,

    -- Content hash for deduplication (SHA256 of payload)
    content_hash TEXT
);

-- Unique index on hash for deduplication
CREATE UNIQUE INDEX IF NOT EXISTS idx_provisioning_events_hash
    ON "ob-poc".provisioning_events(content_hash)
    WHERE content_hash IS NOT NULL;

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_provisioning_events_request
    ON "ob-poc".provisioning_events(request_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_provisioning_events_kind
    ON "ob-poc".provisioning_events(kind);

COMMENT ON TABLE "ob-poc".provisioning_events IS
    'Append-only event log for provisioning requests. Supports idempotent webhook processing.';
COMMENT ON COLUMN "ob-poc".provisioning_events.direction IS
    'OUT = we sent to owner, IN = owner sent to us';
COMMENT ON COLUMN "ob-poc".provisioning_events.content_hash IS
    'SHA256 hash of payload for deduplication. Prevents duplicate webhook processing.';

-- =============================================================================
-- 3. APPEND-ONLY ENFORCEMENT
-- =============================================================================
-- Prevent UPDATE and DELETE on append-only tables.

CREATE OR REPLACE FUNCTION "ob-poc".prevent_modify_append_only()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Table % is append-only. UPDATE and DELETE are not allowed.', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

-- Apply to provisioning_requests
DROP TRIGGER IF EXISTS trg_provisioning_requests_immutable ON "ob-poc".provisioning_requests;
CREATE TRIGGER trg_provisioning_requests_immutable
    BEFORE UPDATE OR DELETE ON "ob-poc".provisioning_requests
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_modify_append_only();

-- Apply to provisioning_events
DROP TRIGGER IF EXISTS trg_provisioning_events_immutable ON "ob-poc".provisioning_events;
CREATE TRIGGER trg_provisioning_events_immutable
    BEFORE UPDATE OR DELETE ON "ob-poc".provisioning_events
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".prevent_modify_append_only();

-- =============================================================================
-- 4. UPDATE provisioning_requests.status VIA EVENTS
-- =============================================================================
-- Since we can't UPDATE directly, we need a function that:
-- 1. Inserts a STATUS event
-- 2. Uses a view or computed column for current status

-- Actually, we'll allow status updates on provisioning_requests since it's
-- tracking workflow state, not audit data. Let's remove the trigger and
-- add a softer constraint.

DROP TRIGGER IF EXISTS trg_provisioning_requests_immutable ON "ob-poc".provisioning_requests;

-- Instead, add an audit column for status changes
ALTER TABLE "ob-poc".provisioning_requests
ADD COLUMN IF NOT EXISTS status_changed_at TIMESTAMPTZ;

-- Trigger to track status changes
CREATE OR REPLACE FUNCTION "ob-poc".track_provisioning_status_change()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status IS DISTINCT FROM NEW.status THEN
        NEW.status_changed_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_provisioning_requests_status ON "ob-poc".provisioning_requests;
CREATE TRIGGER trg_provisioning_requests_status
    BEFORE UPDATE ON "ob-poc".provisioning_requests
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".track_provisioning_status_change();

-- =============================================================================
-- 5. ENHANCE cbu_resource_instances
-- =============================================================================
-- Add columns for owner response data

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS resource_url TEXT;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS owner_ticket_id TEXT;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS last_request_id UUID
    REFERENCES "ob-poc".provisioning_requests(request_id);

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS last_event_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".cbu_resource_instances
ADD COLUMN IF NOT EXISTS srdef_id TEXT;

COMMENT ON COLUMN "ob-poc".cbu_resource_instances.resource_url IS
    'URL to access this resource in the owner system';
COMMENT ON COLUMN "ob-poc".cbu_resource_instances.owner_ticket_id IS
    'External ticket/reference from owner system';
COMMENT ON COLUMN "ob-poc".cbu_resource_instances.last_request_id IS
    'Most recent provisioning request for this instance';
COMMENT ON COLUMN "ob-poc".cbu_resource_instances.srdef_id IS
    'SRDEF that this instance fulfills';

-- =============================================================================
-- 6. CANONICAL PROVISIONING RESULT PAYLOAD
-- =============================================================================
-- Document the expected structure of RESULT events

COMMENT ON COLUMN "ob-poc".provisioning_events.payload IS
$comment$
Event payload structure varies by kind:

REQUEST_SENT:
{
  "srdef_id": "SRDEF::CUSTODY::Account::custody_securities",
  "attrs": {"market_id": "...", "currency": "USD"},
  "bind_to": {"entity_id": "..."},
  "idempotency_key": "..."
}

ACK:
{
  "owner_ticket_id": "INC12345",
  "estimated_completion": "2024-01-15T10:00:00Z"
}

RESULT (success):
{
  "status": "active",
  "srid": "SR::CUSTODY::Account::ACCT-12345678",
  "native_key": "ACCT-12345678",
  "native_key_type": "AccountNo",
  "resource_url": "https://custody.internal/accounts/ACCT-12345678",
  "owner_ticket_id": "INC12345"
}

RESULT (failure):
{
  "status": "failed",
  "explain": {
    "message": "Account creation rejected: duplicate SSI",
    "codes": ["DUPLICATE_SSI", "VALIDATION_ERROR"]
  }
}

ERROR:
{
  "error_code": "TIMEOUT",
  "message": "Request timed out after 30s",
  "retryable": true
}
$comment$;

-- =============================================================================
-- 7. VIEW: Pending Provisioning Requests
-- =============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_provisioning_pending AS
SELECT
    pr.request_id,
    pr.cbu_id,
    c.name AS cbu_name,
    pr.srdef_id,
    pr.status,
    pr.owner_system,
    pr.owner_ticket_id,
    pr.requested_at,
    pr.status_changed_at,
    pr.parameters,
    (SELECT COUNT(*) FROM "ob-poc".provisioning_events pe WHERE pe.request_id = pr.request_id) AS event_count,
    (SELECT MAX(occurred_at) FROM "ob-poc".provisioning_events pe WHERE pe.request_id = pr.request_id) AS last_event_at
FROM "ob-poc".provisioning_requests pr
JOIN "ob-poc".cbus c ON c.cbu_id = pr.cbu_id
WHERE pr.status IN ('queued', 'sent', 'ack')
ORDER BY pr.requested_at ASC;

COMMENT ON VIEW "ob-poc".v_provisioning_pending IS
    'Provisioning requests that are not yet completed or failed.';

-- =============================================================================
-- 8. FUNCTION: Process Provisioning Result (Idempotent)
-- =============================================================================
-- Called by webhook handler. Idempotent via content_hash.

CREATE OR REPLACE FUNCTION "ob-poc".process_provisioning_result(
    p_request_id UUID,
    p_payload JSONB,
    p_content_hash TEXT DEFAULT NULL
) RETURNS TABLE(
    event_id UUID,
    was_duplicate BOOLEAN,
    new_status TEXT
) AS $$
DECLARE
    v_event_id UUID;
    v_existing_event UUID;
    v_new_status TEXT;
    v_instance_id UUID;
    v_result_status TEXT;
BEGIN
    -- Check for duplicate via hash
    IF p_content_hash IS NOT NULL THEN
        SELECT pe.event_id INTO v_existing_event
        FROM "ob-poc".provisioning_events pe
        WHERE pe.content_hash = p_content_hash;

        IF v_existing_event IS NOT NULL THEN
            RETURN QUERY SELECT v_existing_event, TRUE, NULL::TEXT;
            RETURN;
        END IF;
    END IF;

    -- Insert event
    INSERT INTO "ob-poc".provisioning_events (request_id, direction, kind, payload, content_hash)
    VALUES (p_request_id, 'IN', 'RESULT', p_payload, p_content_hash)
    RETURNING provisioning_events.event_id INTO v_event_id;

    -- Extract result status
    v_result_status := p_payload->>'status';

    -- Map to request status
    v_new_status := CASE v_result_status
        WHEN 'active' THEN 'completed'
        WHEN 'pending' THEN 'ack'
        WHEN 'rejected' THEN 'failed'
        WHEN 'failed' THEN 'failed'
        ELSE 'ack'
    END;

    -- Update request status
    UPDATE "ob-poc".provisioning_requests
    SET status = v_new_status,
        owner_ticket_id = COALESCE(p_payload->>'owner_ticket_id', owner_ticket_id)
    WHERE request_id = p_request_id;

    -- If completed, update the resource instance
    IF v_new_status = 'completed' THEN
        SELECT pr.instance_id INTO v_instance_id
        FROM "ob-poc".provisioning_requests pr
        WHERE pr.request_id = p_request_id;

        IF v_instance_id IS NOT NULL THEN
            UPDATE "ob-poc".cbu_resource_instances
            SET status = 'ACTIVE',
                resource_url = p_payload->>'resource_url',
                owner_ticket_id = p_payload->>'owner_ticket_id',
                instance_identifier = COALESCE(p_payload->>'native_key', instance_identifier),
                last_request_id = p_request_id,
                last_event_at = NOW(),
                activated_at = NOW(),
                updated_at = NOW()
            WHERE instance_id = v_instance_id;
        END IF;
    END IF;

    RETURN QUERY SELECT v_event_id, FALSE, v_new_status;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".process_provisioning_result IS
    'Idempotent webhook handler for provisioning results. Uses content_hash for deduplication.';
