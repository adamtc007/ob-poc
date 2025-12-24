-- ═══════════════════════════════════════════════════════════════════════════════════════════════
-- Migration: Workstream Blocker Extension
-- Description: Add blocker tracking columns to entity_workstreams for outstanding request integration
-- ═══════════════════════════════════════════════════════════════════════════════════════════════

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Add blocker tracking columns to workstreams
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

ALTER TABLE kyc.entity_workstreams ADD COLUMN IF NOT EXISTS blocker_type VARCHAR(50);
ALTER TABLE kyc.entity_workstreams ADD COLUMN IF NOT EXISTS blocker_request_id UUID REFERENCES kyc.outstanding_requests(request_id);
ALTER TABLE kyc.entity_workstreams ADD COLUMN IF NOT EXISTS blocker_message VARCHAR(500);
ALTER TABLE kyc.entity_workstreams ADD COLUMN IF NOT EXISTS blocked_days_total INTEGER DEFAULT 0;

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Function to calculate blocked days when unblocking
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

CREATE OR REPLACE FUNCTION kyc.update_workstream_blocked_days()
RETURNS TRIGGER AS $$
BEGIN
    -- When transitioning from BLOCKED to another status, calculate total blocked days
    IF OLD.status = 'BLOCKED' AND NEW.status != 'BLOCKED' THEN
        NEW.blocked_days_total = COALESCE(OLD.blocked_days_total, 0) +
            EXTRACT(DAY FROM NOW() - COALESCE(OLD.blocked_at, NOW()))::INTEGER;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_workstream_blocked_days ON kyc.entity_workstreams;
CREATE TRIGGER trg_workstream_blocked_days
    BEFORE UPDATE ON kyc.entity_workstreams
    FOR EACH ROW EXECUTE FUNCTION kyc.update_workstream_blocked_days();

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Add constraint for blocker_type values
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_ws_blocker_type'
        AND conrelid = 'kyc.entity_workstreams'::regclass
    ) THEN
        ALTER TABLE kyc.entity_workstreams
        ADD CONSTRAINT chk_ws_blocker_type
        CHECK (blocker_type IS NULL OR blocker_type IN (
            'AWAITING_DOCUMENT',
            'AWAITING_INFORMATION',
            'AWAITING_VERIFICATION',
            'AWAITING_APPROVAL',
            'AWAITING_SIGNATURE',
            'SCREENING_HIT',
            'MANUAL_BLOCK'
        ));
    END IF;
END $$;

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Comments
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

COMMENT ON COLUMN kyc.entity_workstreams.blocker_type IS 'Type of blocker: AWAITING_DOCUMENT, AWAITING_VERIFICATION, etc.';
COMMENT ON COLUMN kyc.entity_workstreams.blocker_request_id IS 'FK to outstanding_requests if blocked by a pending request';
COMMENT ON COLUMN kyc.entity_workstreams.blocker_message IS 'Human-readable description of what is blocking progress';
COMMENT ON COLUMN kyc.entity_workstreams.blocked_days_total IS 'Cumulative days spent in BLOCKED status (for SLA tracking)';
