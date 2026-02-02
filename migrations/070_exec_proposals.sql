-- Migration 070: Execution Proposals for Proposal/Confirm Protocol
--
-- Enables DSL execution to be reviewable before commit. Proposals are:
-- 1. Immutable after creation (only status can change)
-- 2. Expire after configurable TTL (default 15 minutes)
-- 3. Scoped to session + user for security
-- 4. Linked for edit chains (parent_proposal_id)

-- Execution proposals table
CREATE TABLE IF NOT EXISTS "ob-poc".exec_proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,
    user_id UUID,  -- Optional, for multi-user audit

    -- Source & parsed form
    source_dsl TEXT NOT NULL,
    canonical_dsl TEXT NOT NULL,  -- Normalized AST as DSL string
    ast_json JSONB NOT NULL,       -- Full AST for exact replay

    -- Resolution state
    resolved_entities JSONB NOT NULL DEFAULT '[]',  -- [{ref: "BlackRock", entity_id: "uuid", confidence: 0.95}]
    unresolved_refs JSONB NOT NULL DEFAULT '[]',    -- [{ref: "Unknown Corp", suggestions: [...]}]

    -- Validation
    validation_passed BOOLEAN NOT NULL,
    validation_errors JSONB NOT NULL DEFAULT '[]',
    warnings JSONB NOT NULL DEFAULT '[]',

    -- Preview data
    affected_entities JSONB NOT NULL DEFAULT '[]',   -- UUIDs that would be modified
    preview_summary TEXT,                             -- Human-readable summary

    -- Narration (generated from verb templates)
    narration TEXT,  -- "This will create a new CBU named 'Acme Fund'..."

    -- State machine
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'confirmed', 'expired', 'superseded', 'cancelled')),

    -- Linking for edits
    parent_proposal_id UUID REFERENCES "ob-poc".exec_proposals(id),

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '15 minutes'),
    confirmed_at TIMESTAMPTZ,

    -- Result (populated after confirmation)
    execution_result JSONB,

    -- Session FK (cascade delete when session is deleted)
    CONSTRAINT fk_session FOREIGN KEY (session_id)
        REFERENCES "ob-poc".agent_sessions(id) ON DELETE CASCADE
);

-- Index for fast session lookup
CREATE INDEX IF NOT EXISTS idx_exec_proposals_session_status
    ON "ob-poc".exec_proposals(session_id, status);

-- Index for expiry cleanup
CREATE INDEX IF NOT EXISTS idx_exec_proposals_expires
    ON "ob-poc".exec_proposals(expires_at)
    WHERE status = 'pending';

-- Index for edit chain traversal
CREATE INDEX IF NOT EXISTS idx_exec_proposals_parent
    ON "ob-poc".exec_proposals(parent_proposal_id)
    WHERE parent_proposal_id IS NOT NULL;

-- Trigger to enforce immutability of proposal content after creation
-- Only status, confirmed_at, and execution_result can change
CREATE OR REPLACE FUNCTION "ob-poc".exec_proposals_immutable()
RETURNS TRIGGER AS $$
BEGIN
    -- Allow status transitions
    IF OLD.status = NEW.status THEN
        -- If status hasn't changed, check if content was modified
        IF NEW.source_dsl IS DISTINCT FROM OLD.source_dsl
           OR NEW.canonical_dsl IS DISTINCT FROM OLD.canonical_dsl
           OR NEW.ast_json IS DISTINCT FROM OLD.ast_json
           OR NEW.resolved_entities IS DISTINCT FROM OLD.resolved_entities
           OR NEW.unresolved_refs IS DISTINCT FROM OLD.unresolved_refs
           OR NEW.validation_passed IS DISTINCT FROM OLD.validation_passed
           OR NEW.validation_errors IS DISTINCT FROM OLD.validation_errors
           OR NEW.warnings IS DISTINCT FROM OLD.warnings
           OR NEW.affected_entities IS DISTINCT FROM OLD.affected_entities
           OR NEW.preview_summary IS DISTINCT FROM OLD.preview_summary
           OR NEW.narration IS DISTINCT FROM OLD.narration
           OR NEW.parent_proposal_id IS DISTINCT FROM OLD.parent_proposal_id
           OR NEW.expires_at IS DISTINCT FROM OLD.expires_at THEN
            RAISE EXCEPTION 'Proposal content is immutable. Create a new proposal with exec.edit.';
        END IF;
    ELSE
        -- Status is changing - validate transition is legal
        IF OLD.status != 'pending' THEN
            RAISE EXCEPTION 'Cannot change status of non-pending proposal (current: %)', OLD.status;
        END IF;

        -- Only these status transitions are allowed from 'pending'
        IF NEW.status NOT IN ('confirmed', 'expired', 'superseded', 'cancelled') THEN
            RAISE EXCEPTION 'Invalid status transition from pending to %', NEW.status;
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS exec_proposals_immutable_trigger ON "ob-poc".exec_proposals;
CREATE TRIGGER exec_proposals_immutable_trigger
    BEFORE UPDATE ON "ob-poc".exec_proposals
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".exec_proposals_immutable();

-- Function to find most recent pending proposal for a session
CREATE OR REPLACE FUNCTION "ob-poc".find_pending_proposal(p_session_id UUID)
RETURNS UUID AS $$
DECLARE
    v_proposal_id UUID;
BEGIN
    SELECT id INTO v_proposal_id
    FROM "ob-poc".exec_proposals
    WHERE session_id = p_session_id
      AND status = 'pending'
      AND expires_at > NOW()
    ORDER BY created_at DESC
    LIMIT 1;

    RETURN v_proposal_id;
END;
$$ LANGUAGE plpgsql;

-- Function to mark proposal as expired
CREATE OR REPLACE FUNCTION "ob-poc".expire_proposal(p_proposal_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    v_updated BOOLEAN;
BEGIN
    UPDATE "ob-poc".exec_proposals
    SET status = 'expired'
    WHERE id = p_proposal_id
      AND status = 'pending';

    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated > 0;
END;
$$ LANGUAGE plpgsql;

-- Function to mark old proposals as superseded when a new edit is created
CREATE OR REPLACE FUNCTION "ob-poc".supersede_proposal(p_proposal_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    v_updated BOOLEAN;
BEGIN
    UPDATE "ob-poc".exec_proposals
    SET status = 'superseded'
    WHERE id = p_proposal_id
      AND status = 'pending';

    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated > 0;
END;
$$ LANGUAGE plpgsql;

-- View for pending proposals with time remaining
CREATE OR REPLACE VIEW "ob-poc".v_pending_proposals AS
SELECT
    p.id,
    p.session_id,
    p.source_dsl,
    p.canonical_dsl,
    p.validation_passed,
    p.narration,
    p.created_at,
    p.expires_at,
    EXTRACT(EPOCH FROM (p.expires_at - NOW()))::INTEGER AS seconds_remaining,
    CASE
        WHEN p.expires_at <= NOW() THEN 'expired'
        ELSE 'active'
    END AS time_status
FROM "ob-poc".exec_proposals p
WHERE p.status = 'pending';

-- View for proposal audit trail (edit chains)
CREATE OR REPLACE VIEW "ob-poc".v_proposal_chain AS
WITH RECURSIVE chain AS (
    -- Start with proposals that have no parent (roots)
    SELECT
        id,
        parent_proposal_id,
        session_id,
        status,
        created_at,
        1 as depth,
        ARRAY[id] as path
    FROM "ob-poc".exec_proposals
    WHERE parent_proposal_id IS NULL

    UNION ALL

    -- Follow the chain
    SELECT
        p.id,
        p.parent_proposal_id,
        p.session_id,
        p.status,
        p.created_at,
        c.depth + 1,
        c.path || p.id
    FROM "ob-poc".exec_proposals p
    JOIN chain c ON p.parent_proposal_id = c.id
)
SELECT * FROM chain;

COMMENT ON TABLE "ob-poc".exec_proposals IS 'Execution proposals for proposal/confirm protocol. Immutable after creation.';
COMMENT ON COLUMN "ob-poc".exec_proposals.source_dsl IS 'Original DSL source as entered by user/LLM';
COMMENT ON COLUMN "ob-poc".exec_proposals.canonical_dsl IS 'Normalized DSL after parsing (whitespace, ordering)';
COMMENT ON COLUMN "ob-poc".exec_proposals.ast_json IS 'Full AST as JSON for exact replay without re-parsing';
COMMENT ON COLUMN "ob-poc".exec_proposals.status IS 'pending=awaiting confirm, confirmed=executed, expired=TTL exceeded, superseded=new edit created, cancelled=user cancelled';
