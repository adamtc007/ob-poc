-- Migration: KYC Case Snapshots
CREATE TABLE IF NOT EXISTS kyc.case_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id),
    status VARCHAR(30) NOT NULL,
    escalation_level VARCHAR(30) NOT NULL,
    risk_rating VARCHAR(20),
    assigned_analyst_id UUID,
    assigned_reviewer_id UUID,
    snapshot_reason VARCHAR(100),
    triggered_by_verb VARCHAR(100),
    version INTEGER NOT NULL DEFAULT 1,
    is_current BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by TEXT,
    CONSTRAINT chk_snapshot_status CHECK (status IN (
        'INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW', 
        'APPROVED', 'REJECTED', 'BLOCKED', 'WITHDRAWN', 'EXPIRED'
    )),
    CONSTRAINT chk_snapshot_escalation CHECK (escalation_level IN (
        'STANDARD', 'SENIOR_COMPLIANCE', 'EXECUTIVE', 'BOARD'
    )),
    CONSTRAINT chk_snapshot_risk CHECK (risk_rating IS NULL OR risk_rating IN (
        'LOW', 'MEDIUM', 'HIGH', 'VERY_HIGH', 'PROHIBITED'
    ))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_case_current_snapshot
ON kyc.case_snapshots(case_id) WHERE is_current = true;

CREATE INDEX IF NOT EXISTS idx_case_snapshots_case_id
ON kyc.case_snapshots(case_id, version DESC);
