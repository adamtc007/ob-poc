-- Semantic Registry Phase 8: Agent Control Plane
-- Tables for agent plans, plan steps, decision records,
-- disambiguation prompts, and escalation records.
--
-- All tables are INSERT-only (immutable) except:
--   - agent_plans.status, agent_plans.updated_at (progress tracking)
--   - plan_steps.status, plan_steps.result, plan_steps.error, plan_steps.updated_at
--   - disambiguation_prompts.answered, .chosen_option, .answered_by, .answered_at
--   - escalation_records.resolved_at, .resolution

-- ── Agent Plans ───────────────────────────────────────────────

CREATE TABLE sem_reg.agent_plans (
    plan_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id           UUID,
    goal              TEXT NOT NULL,
    context_resolution_ref JSONB,
    steps             JSONB NOT NULL DEFAULT '[]'::jsonb,
    assumptions       JSONB NOT NULL DEFAULT '[]'::jsonb,
    risk_flags        JSONB NOT NULL DEFAULT '[]'::jsonb,
    security_clearance VARCHAR(50),
    status            VARCHAR(20) NOT NULL DEFAULT 'draft'
                      CHECK (status IN ('draft', 'active', 'completed', 'failed', 'cancelled')),
    created_by        VARCHAR(200) NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ
);

CREATE INDEX idx_agent_plans_case_id ON sem_reg.agent_plans (case_id) WHERE case_id IS NOT NULL;
CREATE INDEX idx_agent_plans_status ON sem_reg.agent_plans (status) WHERE status IN ('draft', 'active');

-- ── Plan Steps ────────────────────────────────────────────────

CREATE TABLE sem_reg.plan_steps (
    step_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id           UUID NOT NULL REFERENCES sem_reg.agent_plans(plan_id),
    seq               INTEGER NOT NULL,
    verb_id           UUID NOT NULL,
    verb_snapshot_id  UUID NOT NULL,
    verb_fqn          VARCHAR(200) NOT NULL,
    params            JSONB NOT NULL DEFAULT '{}'::jsonb,
    expected_postconditions JSONB NOT NULL DEFAULT '[]'::jsonb,
    fallback_steps    JSONB NOT NULL DEFAULT '[]'::jsonb,
    depends_on_steps  JSONB NOT NULL DEFAULT '[]'::jsonb,
    status            VARCHAR(20) NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending', 'running', 'completed', 'failed', 'skipped')),
    result            JSONB,
    error             TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ,

    UNIQUE (plan_id, seq)
);

CREATE INDEX idx_plan_steps_plan_id ON sem_reg.plan_steps (plan_id);

-- ── Decision Records ──────────────────────────────────────────

CREATE TABLE sem_reg.decision_records (
    decision_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id           UUID REFERENCES sem_reg.agent_plans(plan_id),
    step_id           UUID REFERENCES sem_reg.plan_steps(step_id),
    context_ref       JSONB,
    chosen_action     VARCHAR(200) NOT NULL,
    chosen_action_description TEXT NOT NULL,
    alternatives_considered JSONB NOT NULL DEFAULT '[]'::jsonb,
    evidence_for      JSONB NOT NULL DEFAULT '[]'::jsonb,
    evidence_against  JSONB NOT NULL DEFAULT '[]'::jsonb,
    negative_evidence JSONB NOT NULL DEFAULT '[]'::jsonb,
    policy_verdicts   JSONB NOT NULL DEFAULT '[]'::jsonb,
    snapshot_manifest JSONB NOT NULL,
    confidence        DOUBLE PRECISION NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    escalation_flag   BOOLEAN NOT NULL DEFAULT false,
    escalation_id     UUID,
    decided_by        VARCHAR(200) NOT NULL,
    decided_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_decision_records_plan_id ON sem_reg.decision_records (plan_id) WHERE plan_id IS NOT NULL;
CREATE INDEX idx_decision_records_escalated ON sem_reg.decision_records (decided_at DESC) WHERE escalation_flag = true;

-- ── Disambiguation Prompts ────────────────────────────────────

CREATE TABLE sem_reg.disambiguation_prompts (
    prompt_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id       UUID REFERENCES sem_reg.decision_records(decision_id),
    plan_id           UUID REFERENCES sem_reg.agent_plans(plan_id),
    question          TEXT NOT NULL,
    options           JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_snapshot  JSONB,
    answered          BOOLEAN NOT NULL DEFAULT false,
    chosen_option     VARCHAR(200),
    answered_by       VARCHAR(200),
    answered_at       TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_disambiguation_unanswered ON sem_reg.disambiguation_prompts (plan_id, created_at)
    WHERE answered = false;

-- ── Escalation Records ────────────────────────────────────────

CREATE TABLE sem_reg.escalation_records (
    escalation_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id       UUID REFERENCES sem_reg.decision_records(decision_id),
    reason            TEXT NOT NULL,
    severity          VARCHAR(20) NOT NULL DEFAULT 'warning'
                      CHECK (severity IN ('info', 'warning', 'critical')),
    context_snapshot  JSONB,
    required_human_action TEXT NOT NULL,
    assigned_to       VARCHAR(200),
    resolved_at       TIMESTAMPTZ,
    resolution        TEXT,
    created_by        VARCHAR(200) NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_escalation_unresolved ON sem_reg.escalation_records (created_at DESC)
    WHERE resolved_at IS NULL;
