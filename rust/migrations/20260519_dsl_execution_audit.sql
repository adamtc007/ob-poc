-- P5-T14: DSL Execution Audit Table (v0.5 §13.5)
--
-- Durable, queryable audit history for DSL plan executions.
-- Implements the "audit-as-commit-boundary" commitment from v0.5 §13.5.3:
-- for DurableStep plans (append_fact, append_transition_snapshot effect classes),
-- audit records co-commit with the verb's data writes. A plan is not fully
-- committed until its required audit records are durably written.
--
-- Phase 5 scope: records are written for all steps at commit time.
-- Phase 6: compensation walking, replay, and correlation deduplication
-- queries against this table by execution_id, plan_id, workflow_instance_id.

CREATE TABLE IF NOT EXISTS "ob-poc".dsl_execution_audit (
    -- Identity (v0.5 §10, §13.5.1)
    id               UUID        NOT NULL DEFAULT gen_random_uuid(),
    execution_id     UUID        NOT NULL,   -- identity of the submitted execution
    attempt_id       INTEGER     NOT NULL DEFAULT 1, -- retry counter within execution
    plan_id          UUID,                   -- the compiled ExecutablePlan identity
    sem_os_snapshot_id BIGINT,              -- SDG snapshot compiled against (§3.3)

    -- Node-level info (§13.5.1)
    node_id          INTEGER     NOT NULL,   -- step index within the plan
    verb_fqn         TEXT        NOT NULL,   -- e.g. "cbu.assign-role"
    effect_class     TEXT,                   -- effect_class at time of execution

    -- Timing
    started_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at     TIMESTAMPTZ,

    -- Outcome (§9.1 taxonomy)
    outcome          TEXT        NOT NULL,   -- 'committed', 'rolled_back', 'conflict', etc.

    -- Transaction policy recorded (§8.4)
    transaction_policy TEXT,

    PRIMARY KEY (id)
);

-- Lookup by execution (§13.5.2)
CREATE INDEX IF NOT EXISTS dsl_execution_audit_execution_id_idx
    ON "ob-poc".dsl_execution_audit (execution_id, attempt_id);

-- Lookup by plan (§13.5.2)
CREATE INDEX IF NOT EXISTS dsl_execution_audit_plan_id_idx
    ON "ob-poc".dsl_execution_audit (plan_id)
    WHERE plan_id IS NOT NULL;

-- Lookup by time range + outcome (operational analysis §13.3)
CREATE INDEX IF NOT EXISTS dsl_execution_audit_started_at_idx
    ON "ob-poc".dsl_execution_audit (started_at DESC, outcome);

COMMENT ON TABLE "ob-poc".dsl_execution_audit IS
    'Durable audit trail for DSL plan executions. Records co-commit with verb '
    'data writes for DurableStep plans (v0.5 §13.5.3). Phase 5 scope: written '
    'at plan-commit time for all steps. Phase 6: compensation walking, replay, '
    'and correlation deduplication query this table.';
