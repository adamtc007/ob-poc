-- Migration 126: Multi-workspace runbook plans — compiled orchestration artifacts.

CREATE TABLE IF NOT EXISTS "ob-poc".runbook_plans (
    plan_id         VARCHAR(64) PRIMARY KEY,
    session_id      UUID NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'compiled',
    steps           JSONB NOT NULL,
    bindings        JSONB NOT NULL,
    approval        JSONB,
    compiled_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_runbook_plans_session ON "ob-poc".runbook_plans(session_id);

COMMENT ON TABLE "ob-poc".runbook_plans IS 'Multi-workspace runbook plans compiled from constellation DAG traversal';
