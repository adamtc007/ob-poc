-- Workflow Orchestration Tables
-- Provides stateful workflow tracking for KYC, UBO, and onboarding processes

-- Main workflow instance table
CREATE TABLE IF NOT EXISTS "ob-poc".workflow_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id VARCHAR(100) NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    subject_type VARCHAR(50) NOT NULL,
    subject_id UUID NOT NULL,
    current_state VARCHAR(100) NOT NULL,
    state_entered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    history JSONB NOT NULL DEFAULT '[]',
    blockers JSONB NOT NULL DEFAULT '[]',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),

    -- One workflow instance per subject
    CONSTRAINT uq_workflow_subject UNIQUE (workflow_id, subject_type, subject_id)
);

COMMENT ON TABLE "ob-poc".workflow_instances IS 'Running workflow instances for KYC/onboarding orchestration';
COMMENT ON COLUMN "ob-poc".workflow_instances.workflow_id IS 'Workflow definition ID (e.g., kyc_onboarding)';
COMMENT ON COLUMN "ob-poc".workflow_instances.subject_type IS 'Type of entity this workflow is for (cbu, entity, case)';
COMMENT ON COLUMN "ob-poc".workflow_instances.subject_id IS 'UUID of the subject entity';
COMMENT ON COLUMN "ob-poc".workflow_instances.current_state IS 'Current state in the workflow state machine';
COMMENT ON COLUMN "ob-poc".workflow_instances.history IS 'JSON array of StateTransition records';
COMMENT ON COLUMN "ob-poc".workflow_instances.blockers IS 'JSON array of current Blocker records';

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_workflow_subject
    ON "ob-poc".workflow_instances (subject_type, subject_id);
CREATE INDEX IF NOT EXISTS idx_workflow_state
    ON "ob-poc".workflow_instances (workflow_id, current_state);
CREATE INDEX IF NOT EXISTS idx_workflow_updated
    ON "ob-poc".workflow_instances (updated_at DESC);

-- Audit log for all state transitions
CREATE TABLE IF NOT EXISTS "ob-poc".workflow_audit_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".workflow_instances(instance_id) ON DELETE CASCADE,
    from_state VARCHAR(100),
    to_state VARCHAR(100) NOT NULL,
    transition_type VARCHAR(20) NOT NULL DEFAULT 'auto', -- auto, manual, system
    transitioned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    transitioned_by VARCHAR(255),
    reason TEXT,
    blockers_at_transition JSONB,
    guard_results JSONB
);

COMMENT ON TABLE "ob-poc".workflow_audit_log IS 'Audit trail of all workflow state transitions';

CREATE INDEX IF NOT EXISTS idx_audit_instance
    ON "ob-poc".workflow_audit_log (instance_id, transitioned_at DESC);

-- Trigger to update updated_at on workflow_instances
CREATE OR REPLACE FUNCTION "ob-poc".update_workflow_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_workflow_updated ON "ob-poc".workflow_instances;
CREATE TRIGGER trg_workflow_updated
    BEFORE UPDATE ON "ob-poc".workflow_instances
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_workflow_timestamp();
